//! Local address book, populated manually or from read-only email headers.
use crate::{model::Result, storage::AppState, validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
const MAX_CONTACTS: usize = 10_000;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub provider: String,
    pub email: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contact {
    pub email: String,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub accounts: Vec<Account>,
}
fn validate(contact: &Contact) -> Result<()> {
    if !validation::valid_email(&contact.email)
        || contact.email.len() > 254
        || contact.email.chars().any(char::is_control)
        || contact.name.chars().count() > 100
        || contact.name.chars().any(char::is_control)
        || !["manual", "gmail", "mailserver"].contains(&contact.source.as_str())
    {
        return Err("Enter a valid email address and a name of up to 100 characters".into());
    }
    Ok(())
}
pub fn list(state: &AppState) -> Result<Vec<Contact>> {
    let mut rows = state.lock()?.recipients.clone();
    rows.sort_by_key(|c| (c.name.to_lowercase(), c.email.to_lowercase()));
    Ok(rows)
}
pub fn save(state: &AppState, email: String, name: String) -> Result<()> {
    let contact = Contact {
        email: email.trim().to_lowercase(),
        name: name.trim().into(),
        source: "manual".into(),
        accounts: Vec::new(),
    };
    validate(&contact)?;
    state.transaction(|store| {
        if let Some(existing) = store
            .recipients
            .iter_mut()
            .find(|c| c.email.eq_ignore_ascii_case(&contact.email))
        {
            existing.name = contact.name;
            existing.source = "manual".into();
        } else {
            if store.recipients.len() >= MAX_CONTACTS {
                return Err("The recipient list is full (10,000 addresses)".into());
            }
            store.recipients.push(contact);
        }
        Ok(())
    })
}
pub fn remove(state: &AppState, email: String) -> Result<()> {
    state.transaction(|store| {
        store
            .recipients
            .retain(|c| !c.email.eq_ignore_ascii_case(&email));
        let email = email.to_lowercase();
        if !validation::valid_email(&email) || email.len() > 254 {
            return Err("Invalid recipient email address".into());
        }
        Ok(())
    })
}
fn parse(header: &str, source: &str) -> Vec<Contact> {
    header
        .parse::<lettre::message::Mailboxes>()
        .map(|rows| {
            rows.into_iter()
                .filter_map(|m| {
                    let name = m
                        .name
                        .unwrap_or_default()
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(100)
                        .collect();
                    let contact = Contact {
                        email: m.email.to_string().to_lowercase(),
                        name,
                        source: source.into(),
                        accounts: Vec::new(),
                    };
                    validate(&contact).ok().map(|_| contact)
                })
                .take(100)
                .collect()
        })
        .unwrap_or_default()
}
fn merge(state: &AppState, rows: Vec<Contact>, own: &str, provider: &str) -> Result<usize> {
    let account = Account {
        provider: provider.into(),
        email: own.to_lowercase(),
    };
    state.transaction(|store| {
        let mut added = 0;
        for mut contact in rows.into_iter().take(MAX_CONTACTS) {
            contact.email = contact.email.to_lowercase();
            if validate(&contact).is_err() || contact.email.eq_ignore_ascii_case(own) {
                continue;
            }
            if let Some(existing) = store
                .recipients
                .iter_mut()
                .find(|c| c.email.eq_ignore_ascii_case(&contact.email))
            {
                if !existing.accounts.contains(&account) {
                    existing.accounts.push(account.clone());
                    added += 1;
                }
                if existing.name.is_empty() {
                    existing.name = contact.name;
                }
            } else {
                if store.recipients.len() >= MAX_CONTACTS {
                    return Err("The recipient list is full (10,000 addresses)".into());
                }
                contact.accounts = vec![account.clone()];
                store.recipients.push(contact);
                added += 1;
            }
        }
        Ok(added)
    })
}
pub fn scan(state: &AppState, provider: &str, sender: &str) -> Result<usize> {
    let _operation = crate::gmail::lock_for_update()?;
    let actual = match provider {
        "gmail" => crate::gmail::status(state)?.email,
        "mailserver" => crate::mailserver::status()?.config.map(|c| c.email),
        _ => return Err("Choose Gmail or IMAP / POP3".into()),
    };
    if !actual
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case(sender))
    {
        return Err("The connected account changed. Select your account and scan again.".into());
    }
    let rows = if provider == "mailserver" {
        serde_json::from_value(crate::mailserver::request("mail_contacts", json!({}))?)
            .map_err(|_| "Could not read mail-server recipients")?
    } else {
        use crate::mail_sources::{endpoint, json_response};
        let started = std::time::Instant::now();
        let (client, token) = crate::gmail::source_token()?;
        let base = "https://gmail.googleapis.com/gmail/v1/users/me";
        let mut url = endpoint(base, &["messages"])?;
        url.query_pairs_mut()
            .append_pair("q", "-in:trash -in:spam")
            .append_pair("maxResults", "50");
        let response = json_response(
            client
                .get(url)
                .bearer_auth(&token)
                .send()
                .map_err(|_| "Could not scan Gmail headers")?,
            2 * 1024 * 1024,
        )?;
        let mut contacts = Vec::new();
        if let Some(messages) = response["messages"].as_array() {
            for message in messages.iter().take(50) {
                if started.elapsed().as_secs() > 90 {
                    return Err(
                        "The email scan took too long. Try again when the connection is faster."
                            .into(),
                    );
                }
                let id = message["id"].as_str().ok_or("Missing email identifier")?;
                let mut url = endpoint(base, &["messages", id])?;
                url.query_pairs_mut().append_pair("format", "metadata");
                for key in ["From", "To", "Cc", "Reply-To"] {
                    url.query_pairs_mut().append_pair("metadataHeaders", key);
                }
                let value = json_response(
                    client
                        .get(url)
                        .bearer_auth(&token)
                        .send()
                        .map_err(|_| "Could not read email address headers")?,
                    2 * 1024 * 1024,
                )?;
                if let Some(headers) = value.pointer("/payload/headers").and_then(Value::as_array) {
                    for header in headers {
                        if header["name"].as_str().is_some_and(|n| {
                            ["from", "to", "cc", "reply-to"].contains(&n.to_lowercase().as_str())
                        }) {
                            contacts.extend(parse(
                                header["value"].as_str().unwrap_or_default(),
                                "gmail",
                            ));
                        }
                    }
                }
            }
        }
        contacts
    };
    merge(state, rows, sender, provider)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rescans_restore_deleted_addresses_and_keep_each_account_membership() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        // Old suppression records must no longer affect rescans.
        let old: crate::model::Store = serde_json::from_value(
            json!({"recipients":[],"hidden_recipients":["a@example.com"],"runs":[],"schedules":[]}),
        )
        .unwrap();
        state
            .transaction(|s| {
                *s = old;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            merge(
                &state,
                parse("a@example.com", "gmail"),
                "one@gmail.com",
                "gmail"
            )
            .unwrap(),
            1
        );
        assert_eq!(
            merge(
                &state,
                parse("a@example.com", "gmail"),
                "two@gmail.com",
                "gmail"
            )
            .unwrap(),
            1
        );
        assert_eq!(
            merge(
                &state,
                parse("a@example.com", "mailserver"),
                "work@example.com",
                "mailserver"
            )
            .unwrap(),
            1
        );
        assert_eq!(
            merge(
                &state,
                parse("a@example.com", "gmail"),
                "one@gmail.com",
                "gmail"
            )
            .unwrap(),
            0
        );
        assert_eq!(list(&state).unwrap()[0].accounts.len(), 3);
        drop(state);
        let state = AppState::initialize(dir.path().into()).unwrap();
        assert_eq!(list(&state).unwrap()[0].accounts.len(), 3);
        remove(&state, "a@example.com".into()).unwrap();
        assert!(list(&state).unwrap().is_empty());
        assert_eq!(
            merge(
                &state,
                parse("a@example.com", "gmail"),
                "one@gmail.com",
                "gmail"
            )
            .unwrap(),
            1
        );
        assert_eq!(list(&state).unwrap().len(), 1);
    }
    #[test]
    fn manual_edits_preserve_account_memberships_and_legacy_rows_remain_readable() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        merge(
            &state,
            parse("Alice <a@example.com>", "gmail"),
            "me@gmail.com",
            "gmail",
        )
        .unwrap();
        save(&state, "a@example.com".into(), "Alicia".into()).unwrap();
        let rows = list(&state).unwrap();
        assert_eq!(rows[0].accounts.len(), 1);
        assert_eq!(rows[0].name, "Alicia");
        let legacy: Contact =
            serde_json::from_value(json!({"email":"old@example.com","name":"","source":"gmail"}))
                .unwrap();
        assert!(legacy.accounts.is_empty());
    }
    #[test]
    fn deleting_a_contact_never_rewrites_routine_recipients() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        let routine:crate::email_routines::EmailRoutine=serde_json::from_value(json!({"id":"r","settings":{"name":"r","recipient":"a@example.com","subject":"s","message":"m","start_date":"2099-01-01","time":"09:00","repeat":"once","enabled":true},"next_due":null,"attempt":null,"notified_due":null})).unwrap();
        state
            .transaction(|s| {
                s.email_routines.push(routine);
                Ok(())
            })
            .unwrap();
        remove(&state, "a@example.com".into()).unwrap();
        assert_eq!(
            state.lock().unwrap().email_routines[0].settings.recipient,
            "a@example.com"
        );
    }
}

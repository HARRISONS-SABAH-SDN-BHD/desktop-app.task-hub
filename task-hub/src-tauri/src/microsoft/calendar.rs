//! One-way sync of private Task Hub events in the connected Outlook calendar.
use super::{oauth, vault};
use crate::{
    email_routines::{self, EmailRoutine},
    gmail::{calendar::CalendarLink, transport},
    model::Result,
    storage::AppState,
};
use chrono::{DateTime, Duration, Local, Utc};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
const PROPERTY: &str = "String {bc4f6478-7067-4e08-a409-afc228667a11} Name TaskHubOwner";
fn digest(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
fn owner(link: &CalendarLink) -> String {
    format!("{}|{}|{}", link.sender, link.routine_id, link.event_id)
}
fn desired(routine: &EmailRoutine, zone: &str) -> Result<Option<Value>> {
    let s = &routine.settings;
    let Some(calendar) = s
        .calendar
        .as_ref()
        .filter(|c| c.enabled && s.enabled && s.provider == "microsoft")
    else {
        return Ok(None);
    };
    let first = email_routines::next_due(s, None)?.ok_or("Routine has no calendar start")?;
    let start = DateTime::parse_from_rfc3339(&first)
        .map_err(|_| "Invalid calendar date")?
        .with_timezone(&Local);
    let pattern = match s.repeat.as_str() {
        "once" => Value::Null,
        "daily" => json!({"type":"daily","interval":1}),
        "weekly" => {
            let names = [
                "monday",
                "tuesday",
                "wednesday",
                "thursday",
                "friday",
                "saturday",
                "sunday",
            ];
            let mut days = s.weekdays.clone();
            days.sort();
            days.dedup();
            let days = days
                .iter()
                .map(|d| names.get(*d as usize).copied().ok_or("Invalid weekday"))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            json!({"type":"weekly","interval":1,"firstDayOfWeek":"monday","daysOfWeek":days})
        }
        "monthly" => {
            json!({"type":"absoluteMonthly","interval":1,"dayOfMonth":s.start_date.rsplit('-').next().and_then(|v|v.parse::<u32>().ok()).ok_or("Invalid calendar day")?})
        }
        _ => return Err("Unsupported recurrence".into()),
    };
    Ok(Some(
        json!({"subject":format!("Email routine: {}",s.name),"body":{"contentType":"text","content":"Managed by Task Hub. Edit the routine in Task Hub. This reminder is not proof of email delivery. Task Hub must be running on an awake, connected computer. Email recipients, message contents and files are not copied here."},"start":{"dateTime":start.format("%Y-%m-%dT%H:%M:%S").to_string(),"timeZone":zone},"end":{"dateTime":(start+Duration::minutes(15)).format("%Y-%m-%dT%H:%M:%S").to_string(),"timeZone":zone},"sensitivity":"private","showAs":"free","isReminderOn":true,"reminderMinutesBeforeStart":calendar.reminder_minutes,"recurrence":if pattern.is_null(){Value::Null}else{json!({"pattern":pattern,"range":{"type":"noEnd","startDate":start.date_naive().to_string(),"recurrenceTimeZone":zone}})}}),
    ))
}
fn check_owned(value: &Value, link: &CalendarLink) -> Result<()> {
    let owned = value
        .get("singleValueExtendedProperties")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props
                .iter()
                .any(|p| p["id"] == PROPERTY && p["value"] == owner(link))
        });
    if !owned
        || value.get("isOrganizer").and_then(Value::as_bool) != Some(true)
        || !value
            .get("attendees")
            .and_then(Value::as_array)
            .is_some_and(|a| a.is_empty())
    {
        return Err("Outlook event ownership or absence of attendees could not be verified. No event was changed.".into());
    }
    Ok(())
}
fn sync_request(
    client: &Client,
    base: &str,
    token: &str,
    link: &CalendarLink,
    payload: Option<&Value>,
) -> Result<()> {
    let collection = format!("{base}/me/calendar/events");
    let mut lookup = url::Url::parse(&collection).map_err(|_| "Invalid calendar API URL")?;
    let marker = owner(link).replace("'", "''");
    lookup.query_pairs_mut().append_pair("$filter",&format!("singleValueExtendedProperties/Any(ep: ep/id eq '{PROPERTY}' and ep/value eq '{marker}')")).append_pair("$expand",&format!("singleValueExtendedProperties($filter=id eq '{PROPERTY}')")).append_pair("$top","2");
    let response = client
        .get(lookup)
        .bearer_auth(token)
        .send()
        .map_err(|_| "Could not check Outlook calendar. Email schedules are unaffected.")?;
    if !response.status().is_success() {
        return Err(format!("Outlook calendar lookup returned HTTP {}. Reconnect with Calendar access and ask IT to approve Calendars.ReadWrite.",response.status().as_u16()));
    }
    let value: Value = response
        .json()
        .map_err(|_| "Unreadable Outlook calendar response")?;
    let entries = value
        .get("value")
        .and_then(Value::as_array)
        .ok_or("Invalid Outlook calendar lookup")?;
    if entries.len() > 1 || value.get("@odata.nextLink").is_some() {
        return Err(
            "Multiple Outlook events match this routine. Sync stopped without changing them."
                .into(),
        );
    }
    let Some(existing) = entries.first() else {
        let Some(payload) = payload else {
            return Ok(());
        };
        let mut body = payload.clone();
        body["transactionId"] = json!(link.event_id);
        body["singleValueExtendedProperties"] = json!([{"id":PROPERTY,"value":owner(link)}]);
        let response=client.post(&collection).bearer_auth(token).json(&body).send().map_err(|_|"Outlook calendar creation outcome is uncertain. The same transaction ID will be used on reconciliation.")?;
        if response.status().as_u16() != 201 {
            return Err(format!(
                "Outlook event creation returned HTTP {}. Email delivery is unaffected.",
                response.status().as_u16()
            ));
        }
        return Ok(());
    };
    check_owned(existing, link)?;
    let id = existing
        .get("id")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or("Outlook event has no ID")?;
    let etag = existing
        .get("@odata.etag")
        .and_then(Value::as_str)
        .ok_or("Outlook event has no version; sync stopped safely")?;
    let mut target =
        url::Url::parse(&format!("{base}/me/events/")).map_err(|_| "Invalid calendar API URL")?;
    target
        .path_segments_mut()
        .map_err(|_| "Invalid calendar path")?
        .pop_if_empty()
        .push(id);
    let request = if let Some(body) = payload {
        client.patch(target).json(body)
    } else {
        client.delete(target)
    };
    let response = request
        .bearer_auth(token)
        .header(reqwest::header::IF_MATCH, etag)
        .send()
        .map_err(|_| "Outlook calendar update could not be confirmed; sync will reconcile later")?;
    if response.status().is_success() || (payload.is_none() && response.status().as_u16() == 404) {
        Ok(())
    } else {
        Err(format!(
            "Outlook calendar update returned HTTP {}; no unrelated event was changed.",
            response.status().as_u16()
        ))
    }
}
fn tick(state: &AppState) -> Result<()> {
    let Ok(_operation) = crate::gmail::lock_for_update() else {
        return Ok(());
    };
    let Some(mut credential) = vault::read()? else {
        return Ok(());
    };
    let sender = credential.email.to_lowercase();
    let now = Utc::now().timestamp();
    let missing = {
        let store = state.lock()?;
        store
            .email_routines
            .iter()
            .filter(|r| {
                r.settings.provider == "microsoft"
                    && r.settings.enabled
                    && r.settings.calendar.as_ref().is_some_and(|c| c.enabled)
                    && r.settings
                        .sender_account
                        .as_deref()
                        .is_some_and(|a| a.eq_ignore_ascii_case(&sender))
                    && !store.calendar_links.iter().any(|link| {
                        link.provider == "microsoft"
                            && link.routine_id == r.id
                            && link.sender == sender
                    })
            })
            .map(|r| r.id.clone())
            .collect::<Vec<_>>()
    };
    if !missing.is_empty() {
        state.transaction(|store| {
            for id in &missing {
                if !store
                    .calendar_links
                    .iter()
                    .any(|l| l.provider == "microsoft" && l.routine_id == *id && l.sender == sender)
                {
                    store.calendar_links.push(CalendarLink {
                        provider: "microsoft".into(),
                        routine_id: id.clone(),
                        sender: sender.clone(),
                        event_id: uuid::Uuid::new_v4().to_string(),
                        synced_hash: None,
                        error: None,
                        retry_after: 0,
                    });
                }
            }
            Ok(())
        })?;
    }
    let zone = iana_time_zone::get_timezone()
        .map_err(|_| "Could not identify the computer's calendar timezone")?;
    let jobs = {
        let store = state.lock()?;
        store
            .calendar_links
            .iter()
            .filter(|l| l.provider == "microsoft" && l.sender == sender && l.retry_after <= now)
            .map(|link| {
                let routine = store.email_routines.iter().find(|r| {
                    r.id == link.routine_id
                        && r.settings
                            .sender_account
                            .as_deref()
                            .is_some_and(|a| a.eq_ignore_ascii_case(&sender))
                });
                let payload = routine.map(|r| desired(r, &zone)).transpose()?.flatten();
                let hash = payload.as_ref().map(digest);
                Ok((link.clone(), payload, hash))
            })
            .collect::<Result<Vec<_>>>()?
    };
    let Some((link, payload, hash)) = jobs
        .into_iter()
        .find(|(l, p, h)| p.is_none() || l.synced_hash != *h)
    else {
        return Ok(());
    };
    let result = if !credential.calendar_access {
        Err("Reconnect Microsoft 365 with Outlook calendar access enabled. Ask IT to approve Calendars.ReadWrite.".into())
    } else {
        (|| {
            let client = transport::client()?;
            let token = oauth::access_token(&client, &mut credential)?;
            sync_request(
                &client,
                "https://graph.microsoft.com/v1.0",
                &token,
                &link,
                payload.as_ref(),
            )
        })()
    };
    state.transaction(|store| {
        if result.is_ok() && payload.is_none() {
            store.calendar_links.retain(|l| l.event_id != link.event_id);
        } else if let Some(record) = store
            .calendar_links
            .iter_mut()
            .find(|l| l.event_id == link.event_id)
        {
            match &result {
                Ok(()) => {
                    record.synced_hash = hash;
                    record.error = None;
                    record.retry_after = 0;
                }
                Err(error) => {
                    record.error = Some(error.clone());
                    record.retry_after = now + 300;
                }
            }
        }
        Ok(())
    })
}
#[cfg(feature = "desktop")]
pub fn start(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(15));
        if let Err(error) = tick(&app.state::<AppState>()) {
            eprintln!("Calendar sync: {error}");
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    fn link() -> CalendarLink {
        CalendarLink {
            provider: "microsoft".into(),
            routine_id: "routine".into(),
            sender: "sender@example.com".into(),
            event_id: "stable-transaction".into(),
            synced_hash: None,
            error: None,
            retry_after: 0,
        }
    }
    fn read_request(stream: &mut std::net::TcpStream) -> String {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut all = Vec::new();
        loop {
            let mut buf = [0; 4096];
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            all.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&all);
            if let Some(end) = text.find("\r\n\r\n") {
                let length = text[..end]
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if all.len() >= end + 4 + length {
                    return text.into_owned();
                }
            }
        }
    }
    #[test]
    fn recurrence_and_privacy_and_legacy_records() {
        let mut routine:EmailRoutine=serde_json::from_value(json!({"id":"routine","settings":{"provider":"microsoft","name":"Report","recipient":"private@example.com","subject":"Confidential","message":"Secret body","start_date":"2026-01-31","time":"09:00","repeat":"monthly","enabled":true,"calendar":{"enabled":true,"reminder_minutes":60}},"next_due":null,"attempt":null,"notified_due":null})).unwrap();
        let value = desired(&routine, "Asia/Kuala_Lumpur").unwrap().unwrap();
        assert_eq!(value["recurrence"]["pattern"]["dayOfMonth"], 31);
        assert_eq!(value["sensitivity"], "private");
        assert_eq!(value["reminderMinutesBeforeStart"], 60);
        assert!(!value.to_string().contains("private@example.com"));
        assert!(!value.to_string().contains("Secret body"));
        assert!(value.get("attendees").is_none());
        routine.settings.repeat = "weekly".into();
        routine.settings.weekdays = vec![0, 4];
        let value = desired(&routine, "UTC").unwrap().unwrap();
        assert_eq!(
            value["recurrence"]["pattern"]["daysOfWeek"],
            json!(["monday", "friday"])
        );
        routine.settings.repeat = "once".into();
        assert!(desired(&routine, "UTC").unwrap().unwrap()["recurrence"].is_null());
        routine.settings.enabled = false;
        assert!(desired(&routine, "UTC").unwrap().is_none());
        let mut legacy = serde_json::to_value(link()).unwrap();
        legacy.as_object_mut().unwrap().remove("provider");
        assert_eq!(
            serde_json::from_value::<CalendarLink>(legacy)
                .unwrap()
                .provider,
            "gmail"
        );
    }
    #[test]
    fn create_update_delete_and_foreign_events_are_guarded() {
        for mode in ["create", "update", "delete", "foreign"] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let record = link();
            let existing = if mode == "create" {
                json!({"value":[]})
            } else {
                json!({"value":[{"id":"event/A","@odata.etag":"version-1","isOrganizer":true,"attendees":[],"singleValueExtendedProperties":[{"id":PROPERTY,"value":if mode=="foreign" {"foreign".into()}else{owner(&record)}}]}]})
            };
            let worker = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                assert!(request.starts_with("GET /me/calendar/events?"));
                assert!(request.contains("%24filter="));
                let body = existing.to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
                drop(stream);
                if mode == "foreign" {
                    listener.set_nonblocking(true).unwrap();
                    thread::sleep(std::time::Duration::from_millis(100));
                    assert!(listener.accept().is_err());
                    return;
                }
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                if mode == "create" {
                    assert!(request.starts_with("POST /me/calendar/events "));
                    assert!(request.contains("stable-transaction"));
                    assert!(request.contains("singleValueExtendedProperties"));
                } else {
                    assert!(request.to_lowercase().contains("if-match: version-1"));
                    assert!(request.starts_with(if mode == "delete" {
                        "DELETE /me/events/event%2FA "
                    } else {
                        "PATCH /me/events/event%2FA "
                    }));
                }
                stream
                    .write_all(if mode == "create" {
                        b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    } else {
                        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    })
                    .unwrap();
            });
            let value = json!({"subject":"Email routine"});
            let result = sync_request(
                &transport::client().unwrap(),
                &format!("http://{address}"),
                "test-token",
                &record,
                if mode == "delete" { None } else { Some(&value) },
            );
            assert_eq!(result.is_ok(), mode != "foreign");
            worker.join().unwrap();
        }
    }
}

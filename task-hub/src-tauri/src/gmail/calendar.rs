//! One-way sync of app-owned private calendar events. Never sends email or invites.
use super::{oauth, transport, vault, OPERATION};
use crate::{
    email_routines::{self, EmailRoutine},
    model::Result,
    storage::AppState,
};
use chrono::{DateTime, Duration, Utc};
use reqwest::{blocking::Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events.owned";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarLink {
    pub routine_id: String,
    pub sender: String,
    pub event_id: String,
    pub synced_hash: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub retry_after: i64,
}
fn digest(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
fn desired(routine: &EmailRoutine, zone: &str) -> Result<Option<Value>> {
    let s = &routine.settings;
    if s.provider != "gmail" {
        return Ok(None);
    }
    let Some(calendar) = s.calendar.as_ref().filter(|c| c.enabled && s.enabled) else {
        return Ok(None);
    };
    let first =
        email_routines::next_due(s, None)?.ok_or("Routine has no valid calendar start date")?;
    let stamp = DateTime::parse_from_rfc3339(&first).map_err(|_| "Invalid calendar start date")?;
    let recurrence = match s.repeat.as_str() {
        "once" => vec![],
        "daily" => vec!["RRULE:FREQ=DAILY".to_owned()],
        "weekly" => {
            let names = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
            let mut weekdays = s.weekdays.clone();
            weekdays.sort();
            weekdays.dedup();
            let days: Vec<_> = weekdays
                .iter()
                .map(|day| names.get(*day as usize).copied().ok_or("Invalid weekday"))
                .collect::<std::result::Result<_, _>>()?;
            vec![format!("RRULE:FREQ=WEEKLY;BYDAY={}", days.join(","))]
        }
        "monthly" => {
            let day = s
                .start_date
                .rsplit('-')
                .next()
                .ok_or("Invalid date")?
                .parse::<u32>()
                .map_err(|_| "Invalid date")?;
            // First of the intended day and month-end: days 29–31 clamp in shorter months.
            vec![if day > 28 {
                format!("RRULE:FREQ=MONTHLY;BYMONTHDAY={day},-1;BYSETPOS=1")
            } else {
                format!("RRULE:FREQ=MONTHLY;BYMONTHDAY={day}")
            }]
        }
        _ => return Err("Unsupported calendar repeat interval".into()),
    };
    Ok(Some(json!({
        "summary": format!("Email routine: {}", s.name),
        "description": format!("Managed by Task Hub. Edit this routine in Task Hub; calendar edits do not change email delivery.\nMode: {}.\nA calendar event is a reminder, not a delivery receipt. Task Hub must be running on an awake, connected computer to send email. Recipients, message contents and attachments are not copied to Calendar.", if s.auto_send { "Automatic send" } else { "Draft when prepared in Task Hub" }),
        "start": {"dateTime": first, "timeZone": zone},
        "end": {"dateTime": (stamp + Duration::minutes(15)).to_rfc3339(), "timeZone": zone},
        "recurrence": recurrence,
        "visibility": "private", "transparency": "transparent",
        "reminders": {"useDefault": false, "overrides": [{"method": "popup", "minutes": calendar.reminder_minutes}]},
        "extendedProperties": {"private": {"taskhub_id": routine.id, "taskhub_owner": s.sender_account.as_deref().unwrap_or_default().to_lowercase()}}
    })))
}
fn check_owned(value: &Value, link: &CalendarLink) -> Result<()> {
    if value
        .pointer("/extendedProperties/private/taskhub_id")
        .and_then(Value::as_str)
        != Some(link.routine_id.as_str())
        || value
            .pointer("/extendedProperties/private/taskhub_owner")
            .and_then(Value::as_str)
            != Some(link.sender.as_str())
    {
        return Err(
            "Calendar event ownership could not be verified. No existing event was changed.".into(),
        );
    }
    Ok(())
}
/// Fixed event IDs and conditional updates make retries safe, unlike email sends.
fn sync_request(
    client: &Client,
    base: &str,
    token: &str,
    link: &CalendarLink,
    payload: Option<&Value>,
) -> Result<()> {
    let collection = format!("{base}/calendar/v3/calendars/primary/events");
    let url = format!("{collection}/{}", link.event_id);
    let existing = client.get(&url).bearer_auth(token).send().map_err(|_| {
        "Could not check the calendar event. Sync will retry; email delivery is unaffected."
    })?;
    let status = existing.status();
    if status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
        if payload.is_none() {
            return Ok(());
        }
        if status == StatusCode::GONE {
            return Err("This event was removed in Calendar. Turn calendar sync off, wait for cleanup, then enable it to create a new event.".into());
        }
        let mut body = payload.unwrap().clone();
        body["id"] = json!(link.event_id);
        let response = client.post(format!("{collection}?sendUpdates=none")).bearer_auth(token).json(&body).send()
            .map_err(|_| "Calendar creation could not be confirmed. The same event ID will be checked before retrying.")?;
        if !response.status().is_success() {
            return Err(format!("Calendar sync returned HTTP {}. Enable Google Calendar API and reconnect with Calendar permission if needed. Email delivery is unaffected.", response.status().as_u16()));
        }
        return Ok(());
    }
    if !status.is_success() {
        return Err(format!("Calendar lookup returned HTTP {}. Enable Google Calendar API and reconnect with Calendar permission if needed. Email delivery is unaffected.", status.as_u16()));
    }
    let value: Value = existing
        .json()
        .map_err(|_| "Could not read the calendar event")?;
    if value.get("status").and_then(Value::as_str) == Some("cancelled") {
        return if payload.is_none() {
            Ok(())
        } else {
            Err("The calendar event was deleted. Turn sync off, wait for cleanup, then enable it to create a new event.".into())
        };
    }
    check_owned(&value, link)?;
    let etag = value
        .get("etag")
        .and_then(Value::as_str)
        .ok_or("Calendar event has no version; sync stopped safely")?;
    let request = if let Some(payload) = payload {
        client.put(format!("{url}?sendUpdates=none")).json(payload)
    } else {
        client.delete(format!("{url}?sendUpdates=none"))
    };
    let response = request
        .header(reqwest::header::IF_MATCH, etag)
        .bearer_auth(token)
        .send()
        .map_err(|_| "Calendar update could not be confirmed. Sync will reconcile it later.")?;
    if response.status().is_success()
        || (payload.is_none()
            && matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::GONE))
    {
        Ok(())
    } else {
        Err(format!(
            "Calendar update returned HTTP {}. Sync will retry; email delivery is unaffected.",
            response.status().as_u16()
        ))
    }
}
pub fn retry(state: &AppState, id: &str) -> Result<()> {
    state.transaction(|store| {
        for record in store
            .calendar_links
            .iter_mut()
            .filter(|l| l.routine_id == id)
        {
            record.retry_after = 0;
            record.error = None;
            record.synced_hash = None;
        }
        Ok(())
    })
}
fn tick(state: &AppState) -> Result<()> {
    let Ok(_operation) = OPERATION.try_lock() else {
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
                r.settings.provider == "gmail"
                    && r.settings.enabled
                    && r.settings.calendar.as_ref().is_some_and(|c| c.enabled)
                    && r.settings
                        .sender_account
                        .as_deref()
                        .is_some_and(|a| a.eq_ignore_ascii_case(&sender))
                    && !store
                        .calendar_links
                        .iter()
                        .any(|link| link.routine_id == r.id && link.sender == sender)
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
                    .any(|l| l.routine_id == *id && l.sender == sender)
                {
                    store.calendar_links.push(CalendarLink {
                        routine_id: id.clone(),
                        sender: sender.clone(),
                        event_id: format!("taskhub{}", uuid::Uuid::new_v4().simple()),
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
            .filter(|l| l.sender == sender && l.retry_after <= now)
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
        Err("Reconnect Google with Calendar access enabled in Gmail connection.".into())
    } else {
        (|| {
            let client = transport::client()?;
            let token = oauth::access_token(&client, &mut credential)?;
            sync_request(
                &client,
                "https://www.googleapis.com",
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
    fn routine() -> EmailRoutine {
        serde_json::from_value(json!({"id":"routine1","settings":{"name":"Monthly report","recipient":"to@example.com","subject":"Report","message":"Private report","start_date":"2026-01-31","time":"09:00","repeat":"monthly","enabled":true,"sender_account":"sender@example.com","calendar":{"enabled":true,"reminder_minutes":1440}},"next_due":null,"attempt":null,"notified_due":null})).unwrap()
    }
    #[test]
    fn calendar_recurrence_is_private_and_month_end_clamps_like_email_schedule() {
        let mut r = routine();
        let event = desired(&r, "Asia/Singapore").unwrap().unwrap();
        assert_eq!(
            event["recurrence"][0],
            "RRULE:FREQ=MONTHLY;BYMONTHDAY=31,-1;BYSETPOS=1"
        );
        assert_eq!(event["visibility"], "private");
        assert!(event.get("attendees").is_none());
        assert!(!event.to_string().contains("Private report"));
        assert!(!event.to_string().contains("to@example.com"));
        assert_eq!(event["reminders"]["overrides"][0]["minutes"], 1440);
        r.settings.repeat = "weekly".into();
        r.settings.weekdays = vec![4, 0];
        assert_eq!(
            desired(&r, "Asia/Singapore").unwrap().unwrap()["recurrence"][0],
            "RRULE:FREQ=WEEKLY;BYDAY=MO,FR"
        );
        r.settings.repeat = "once".into();
        assert_eq!(
            desired(&r, "Asia/Singapore").unwrap().unwrap()["recurrence"],
            json!([])
        );
        r.settings.enabled = false;
        assert!(desired(&r, "Asia/Singapore").unwrap().is_none());
    }
    #[test]
    fn unowned_events_are_never_changed_or_deleted() {
        let link = CalendarLink {
            routine_id: "routine1".into(),
            sender: "sender@example.com".into(),
            event_id: "abc".into(),
            synced_hash: None,
            error: None,
            retry_after: 0,
        };
        assert!(check_owned(&json!({}), &link).is_err());
        assert!(check_owned(&json!({"extendedProperties":{"private":{"taskhub_id":"routine1","taskhub_owner":"wrong@example.com"}}}),&link).is_err());
        assert!(check_owned(&json!({"extendedProperties":{"private":{"taskhub_id":"routine1","taskhub_owner":"sender@example.com"}}}),&link).is_ok());
    }

    fn server(
        replies: Vec<(&'static str, String)>,
    ) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut part = [0u8; 1024];
                loop {
                    let n = stream.read(&mut part).unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&part[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                        let length = head
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        (base, handle)
    }
    fn link() -> CalendarLink {
        CalendarLink {
            routine_id: "routine1".into(),
            sender: "sender@example.com".into(),
            event_id: "taskhub123".into(),
            synced_hash: None,
            error: None,
            retry_after: 0,
        }
    }
    #[test]
    fn creation_recovery_updates_same_id_and_deletion_checks_ownership_and_etag() {
        let event = desired(&routine(), "Asia/Singapore").unwrap().unwrap();
        let mut existing = event.clone();
        existing["etag"] = json!("event-version");
        let (base, handle) = server(vec![
            ("404 Not Found", "{}".into()),
            ("200 OK", "{}".into()),
            ("200 OK", existing.to_string()),
            ("200 OK", "{}".into()),
            ("200 OK", existing.to_string()),
            ("204 No Content", "".into()),
        ]);
        let client = transport::client().unwrap();
        let link = link();
        sync_request(&client, &base, "test-token", &link, Some(&event)).unwrap();
        // Reconciliation after a lost local result uses the same stored ID.
        sync_request(&client, &base, "test-token", &link, Some(&event)).unwrap();
        sync_request(&client, &base, "test-token", &link, None).unwrap();
        let requests = handle.join().unwrap();
        assert_eq!(
            requests.iter().filter(|r| r.starts_with("POST ")).count(),
            1
        );
        assert!(requests[1].contains("\"id\":\"taskhub123\""));
        assert!(requests[3]
            .starts_with("PUT /calendar/v3/calendars/primary/events/taskhub123?sendUpdates=none"));
        assert!(requests[3]
            .to_lowercase()
            .contains("if-match: event-version"));
        assert!(requests[5].starts_with(
            "DELETE /calendar/v3/calendars/primary/events/taskhub123?sendUpdates=none"
        ));
        assert!(!requests.join("").contains("attendees"));
    }
    #[test]
    fn failed_permission_or_unowned_event_never_causes_a_mutation() {
        for (status, body) in [
            ("403 Forbidden", "{}".to_owned()),
            (
                "200 OK",
                json!({"etag":"v","extendedProperties":{"private":{"taskhub_id":"other"}}})
                    .to_string(),
            ),
        ] {
            let (base, handle) = server(vec![(status, body)]);
            assert!(sync_request(
                &transport::client().unwrap(),
                &base,
                "test-token",
                &link(),
                None
            )
            .is_err());
            let requests = handle.join().unwrap();
            assert_eq!(requests.len(), 1);
            assert!(requests[0].starts_with("GET "));
        }
    }
    #[test]
    fn deleted_routines_keep_calendar_cleanup_records_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        state
            .transaction(|store| {
                store.email_routines.push(routine());
                store.calendar_links.push(link());
                Ok(())
            })
            .unwrap();
        email_routines::remove(&state, "routine1").unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        assert!(state.lock().unwrap().email_routines.is_empty());
        assert_eq!(state.lock().unwrap().calendar_links.len(), 1);
    }
}

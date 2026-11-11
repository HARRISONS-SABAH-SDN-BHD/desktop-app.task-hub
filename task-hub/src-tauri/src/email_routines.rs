//! Saved email intent, reminders and explicitly enabled scheduled sending.
use crate::{email_profile, model::*, runner, storage::AppState, validation};
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

fn default_provider() -> String {
    "gmail".into()
}

fn default_calendar_reminder() -> u32 {
    1440
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarSettings {
    pub enabled: bool,
    #[serde(default = "default_calendar_reminder")]
    pub reminder_minutes: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutineSettings {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub report_rules: Vec<crate::attachment_rules::ReportRule>,
    #[serde(default)]
    pub calendar: Option<CalendarSettings>,
    #[serde(default)]
    pub cc: String,
    pub name: String,
    pub recipient: String,
    pub subject: String,
    pub message: String,
    pub start_date: String,
    pub time: String,
    pub repeat: String,
    #[serde(default)]
    pub weekdays: Vec<u32>, // Monday = 0
    pub enabled: bool,
    #[serde(default)]
    pub auto_send: bool,
    #[serde(default)]
    pub sender_account: Option<String>,
    #[serde(default)]
    pub attachments: Vec<crate::attachments::AttachmentRef>,
    #[serde(default)]
    pub recipient_change: Option<RecipientChange>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipientChange {
    pub recipient: String,
    pub from: String,
    pub until: Option<String>, // Inclusive. None means permanent from this date.
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailRoutine {
    pub id: String,
    pub settings: RoutineSettings,
    pub next_due: Option<String>,
    pub attempt: Option<String>,
    pub notified_due: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct RoutinePreview {
    pub id: String,
    pub due: String,
    pub parameters: Parameters,
    pub recipient_note: String,
    pub attachments: Vec<crate::attachments::AttachmentRef>,
}

fn date(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "Use a valid date (YYYY-MM-DD)".into())
}
fn render(template: &str, due: NaiveDate) -> Result<String> {
    let previous = due
        .with_day(1)
        .unwrap()
        .pred_opt()
        .ok_or("Date is out of range")?;
    let values = [
        ("month", due.format("%B").to_string()),
        ("year", due.format("%Y").to_string()),
        ("date", due.format("%d %B %Y").to_string()),
        ("previous_month", previous.format("%B").to_string()),
        ("previous_year", previous.format("%Y").to_string()),
    ];
    let mut remaining = template;
    let mut output = String::new();
    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        let end = remaining[start + 2..]
            .find("}}")
            .ok_or("Unclosed template field")?
            + start
            + 2;
        let key = remaining[start + 2..end].trim();
        let value = values
            .iter()
            .find(|(name, _)| *name == key)
            .ok_or_else(|| format!("Unknown template field: {key}"))?;
        output.push_str(&value.1);
        remaining = &remaining[end + 2..];
    }
    output.push_str(remaining);
    Ok(output)
}
fn validate(settings: &RoutineSettings) -> Result<()> {
    validation::cc_addresses(&settings.cc)?;
    if settings
        .calendar
        .as_ref()
        .is_some_and(|c| c.reminder_minutes > 40320)
    {
        return Err(
            "Calendar reminders must be between 0 and 40320 minutes before the event".into(),
        );
    }
    if !matches!(settings.provider.as_str(), "gmail" | "mailserver") {
        return Err("Choose Gmail or IMAP / POP3 as the email provider".into());
    }
    if settings.provider == "mailserver" && settings.calendar.as_ref().is_some_and(|c| c.enabled) {
        return Err("IMAP/POP3 accounts do not support calendar sync. Use Gmail.".into());
    }
    crate::attachments::validate_list(&settings.attachments)?;
    if settings
        .sender_account
        .as_deref()
        .is_some_and(|email| !validation::valid_email(email))
    {
        return Err("Choose a valid connected sender account".into());
    }
    if settings.auto_send && settings.sender_account.is_none() {
        return Err(
            "Connect Gmail, then choose the sender account before enabling automatic sending"
                .into(),
        );
    }
    if settings.name.trim().is_empty() || settings.name.chars().count() > 100 {
        return Err("Give the routine a name of up to 100 characters".into());
    }
    if !validation::valid_email(&settings.recipient) {
        return Err("Enter a complete recipient email address".into());
    }
    date(&settings.start_date)?;
    NaiveTime::parse_from_str(&settings.time, "%H:%M").map_err(|_| "Choose a valid time")?;
    if !["once", "daily", "weekly", "monthly"].contains(&settings.repeat.as_str()) {
        return Err("Choose a supported repeat interval".into());
    }
    if settings.weekdays.iter().any(|day| *day > 6)
        || (settings.repeat == "weekly" && settings.weekdays.is_empty())
    {
        return Err("Choose at least one weekday for a weekly routine".into());
    }
    if settings.subject.trim().is_empty()
        || settings.subject.chars().count() > 160
        || settings.subject.contains(['\r', '\n'])
        || settings.message.trim().is_empty()
        || settings.message.chars().count() > 12000
    {
        return Err(
            "Enter a subject (up to 160 characters) and message (up to 12000 characters)".into(),
        );
    }
    render(&settings.subject, date(&settings.start_date)?)?;
    render(&settings.message, date(&settings.start_date)?)?;
    if let Some(change) = &settings.recipient_change {
        if !validation::valid_email(&change.recipient) {
            return Err("Enter a complete replacement recipient address".into());
        }
        let from = date(&change.from)?;
        if change
            .until
            .as_deref()
            .map(date)
            .transpose()?
            .is_some_and(|until| until < from)
        {
            return Err("Recipient change end date must be on or after its start date".into());
        }
        if change.note.chars().count() > 500 {
            return Err("Keep the recipient change note under 500 characters".into());
        }
    }
    Ok(())
}
fn scheduled_on(settings: &RoutineSettings, day: NaiveDate, start: NaiveDate) -> bool {
    if day < start {
        return false;
    }
    match settings.repeat.as_str() {
        "once" => day == start,
        "daily" => true,
        "weekly" => settings
            .weekdays
            .contains(&day.weekday().num_days_from_monday()),
        "monthly" => {
            let last = (28..=31)
                .rev()
                .find(|d| NaiveDate::from_ymd_opt(day.year(), day.month(), *d).is_some())
                .unwrap();
            day.day() == start.day().min(last)
        }
        _ => false,
    }
}
pub(crate) fn next_due(
    settings: &RoutineSettings,
    after: Option<DateTime<Utc>>,
) -> Result<Option<String>> {
    let start = date(&settings.start_date)?;
    let time =
        NaiveTime::parse_from_str(&settings.time, "%H:%M").map_err(|_| "Invalid routine time")?;
    let first = after
        .map(|stamp| stamp.with_timezone(&Local).date_naive().max(start))
        .unwrap_or(start);
    if settings.repeat == "once" && first > start {
        return Ok(None);
    }
    for offset in 0..3660 {
        let day = first
            .checked_add_days(Days::new(offset))
            .ok_or("Date is out of range")?;
        if scheduled_on(settings, day, start) {
            // Earliest occurrence on an ambiguous DST date; nonexistent times
            // are skipped. All schedules follow the computer's local timezone.
            if let Some(stamp) = Local.from_local_datetime(&day.and_time(time)).earliest() {
                let stamp = stamp.with_timezone(&Utc);
                if after.is_none_or(|after| stamp > after) {
                    return Ok(Some(stamp.to_rfc3339()));
                }
            }
        }
        if settings.repeat == "once" {
            break;
        }
    }
    Ok(None)
}
pub fn save(state: &AppState, id: Option<String>, settings: RoutineSettings) -> Result<()> {
    validate(&settings)?;
    crate::attachments::load(state, &settings.attachments)?;
    crate::attachment_rules::validate(state, &settings.report_rules, settings.attachments.len())?;
    state.transaction(|store| {
        if let Some(id) = id.as_ref() {
            for link in store
                .calendar_links
                .iter_mut()
                .filter(|l| l.routine_id == *id)
            {
                link.synced_hash = None;
                link.error = None;
                link.retry_after = 0;
            }
        }
        if let Some(id) = id {
            let report_failure = store
                .email_routines
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.attempt.as_ref())
                .is_some_and(|attempt| {
                    store.runs.iter().any(|r| {
                        r.id == *attempt
                            && r.status == "failed"
                            && r.error.as_deref().is_some_and(|e| {
                                e.starts_with(crate::attachment_rules::ISSUE_PREFIX)
                            })
                    })
                });
            let routine = store
                .email_routines
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or("Routine not found")?;
            // Do not erase an attempted occurrence or silently resend after edits.
            if routine.attempt.is_some() && !report_failure {
                return Err("Handle the existing attempt before editing this routine".into());
            }
            let schedule_changed = routine.settings.start_date != settings.start_date
                || routine.settings.time != settings.time
                || routine.settings.repeat != settings.repeat
                || routine.settings.weekdays != settings.weekdays;
            if report_failure && (schedule_changed || routine.settings.auto_send != settings.auto_send) {
                return Err(
                    "Retry or handle this attachment-only failure before changing its schedule or delivery mode."
                        .into(),
                );
            }
            if schedule_changed || (!routine.settings.auto_send && settings.auto_send) {
                routine.next_due = next_due(&settings, None)?;
                if settings.auto_send {
                    routine.next_due = next_due(&settings, Some(Utc::now()))?;
                }
            }
            if settings.auto_send && routine.next_due.is_none() {
                return Err(
                    "Automatic sending needs a future occurrence. Choose a future date and time."
                        .into(),
                );
            }
            routine.settings = settings;
            routine.notified_due = None;
        } else {
            if store.email_routines.len() >= 50 {
                return Err("Routine limit reached (50)".into());
            }
            let due = next_due(
                &settings,
                if settings.auto_send {
                    Some(Utc::now())
                } else {
                    None
                },
            )?;
            if settings.auto_send && due.is_none() {
                return Err(
                    "Automatic sending needs a future occurrence. Choose a future date and time."
                        .into(),
                );
            }
            store.email_routines.push(EmailRoutine {
                id: uuid::Uuid::new_v4().to_string(),
                next_due: due,
                settings,
                attempt: None,
                notified_due: None,
            });
        }
        Ok(())
    })
}
fn preview(routine: &EmailRoutine) -> Result<RoutinePreview> {
    let due = routine
        .next_due
        .clone()
        .ok_or("Routine has no remaining date")?;
    let day = DateTime::parse_from_rfc3339(&due)
        .map_err(|_| "Invalid due date")?
        .with_timezone(&Local)
        .date_naive();
    let mut recipient = &routine.settings.recipient;
    let mut recipient_note = String::new();
    if let Some(change) = &routine.settings.recipient_change {
        if day >= date(&change.from)?
            && change
                .until
                .as_deref()
                .map(date)
                .transpose()?
                .is_none_or(|end| day <= end)
        {
            recipient = &change.recipient;
            recipient_note = format!(
                "Recipient changed from {} to {}. {} {}",
                routine.settings.recipient,
                change.recipient,
                change
                    .until
                    .as_deref()
                    .map(|until| format!(
                        "Temporary through {until}; then the original recipient returns."
                    ))
                    .unwrap_or_else(|| "Permanent from the effective date.".into()),
                change.note
            );
        }
    }
    let parameters = serde_json::from_value(serde_json::json!({
        "recipient": recipient, "cc": routine.settings.cc, "subject": render(&routine.settings.subject, day)?,
        "message": render(&routine.settings.message, day)?, "delivery": if routine.settings.auto_send { "Send directly" } else { "Draft" },
        "_provider": routine.settings.provider,
        "_sender_account": routine.settings.sender_account.as_deref().unwrap_or_default(),
        "_attachment_rules": serde_json::to_string(&routine.settings.report_rules).map_err(|_|"Invalid report rules")?,
        "_attachment_ids": routine.settings.attachments.iter().map(|a| a.id.as_str()).collect::<Vec<_>>().join(",")
    }))
    .map_err(|e| e.to_string())?;
    Ok(RoutinePreview {
        id: routine.id.clone(),
        due,
        parameters,
        recipient_note,
        attachments: routine.settings.attachments.clone(),
    })
}
pub fn get_preview(state: &AppState, id: &str) -> Result<RoutinePreview> {
    let store = state.lock()?;
    preview(
        store
            .email_routines
            .iter()
            .find(|r| r.id == id)
            .ok_or("Routine not found")?,
    )
}
pub fn run(state: &AppState, id: &str, due: &str, expected: Parameters) -> Result<Run> {
    let (task, run, direct) = claim(state, id, due, expected, false, Utc::now(), None)?;
    let outcome = crate::gmail::execute(state, task, run, direct)?;
    if outcome.status == "succeeded" {
        handle(state, id, due)?;
    }
    Ok(outcome)
}
pub fn retry_reports(state: &AppState, id: &str, due: &str) -> Result<Run> {
    let (attempt, settings, plan) = {
        let store = state.lock()?;
        let routine = store
            .email_routines
            .iter()
            .find(|r| r.id == id)
            .ok_or("Routine not found")?;
        let attempt = routine
            .attempt
            .clone()
            .ok_or("No failed report attachment attempt exists")?;
        if routine.next_due.as_deref() != Some(due)
            || !store.runs.iter().any(|r| {
                r.id == attempt
                    && r.status == "failed"
                    && r.error
                        .as_deref()
                        .is_some_and(|e| e.starts_with(crate::attachment_rules::ISSUE_PREFIX))
            })
        {
            return Err("This is not an attachment-only failure. Inspect history and Gmail before handling it.".into());
        }
        (attempt, routine.settings.clone(), preview(routine)?)
    };
    crate::attachment_rules::confirm_completed(state, &settings.report_rules, due)?;
    let (task, run, direct) = claim(
        state,
        id,
        due,
        plan.parameters,
        settings.auto_send,
        Utc::now(),
        Some(&attempt),
    )?;
    let result = crate::gmail::execute(state, task, run, direct)?;
    if result.status == "succeeded" {
        handle(state, id, due)?;
    }
    Ok(result)
}
fn claim(
    state: &AppState,
    id: &str,
    due: &str,
    expected: Parameters,
    automatic: bool,
    now: DateTime<Utc>,
    retry_run: Option<&str>,
) -> Result<(Task, Run, bool)> {
    // Claim occurrence and run together before any visible input. A failed or
    // uncertain attempt stays blocked until the user explicitly handles it.
    state.transaction(|store| {
        let routine = store
            .email_routines
            .iter()
            .find(|r| r.id == id)
            .ok_or("Routine not found")?;
        if !matches!(routine.settings.provider.as_str(), "gmail" | "mailserver") {
            return Err("This provider is unsupported. Choose Gmail or IMAP / POP3 before running this routine.".into());
        }
        let report_retry = retry_run.is_some_and(|id| routine.attempt.as_deref()==Some(id) && store.runs.iter().any(|r|r.id==id && r.parameters.get("_attachment_due").and_then(|v|v.as_str())==Some(due) && r.status=="failed" && r.error.as_deref().is_some_and(|e|e.starts_with(crate::attachment_rules::ISSUE_PREFIX))));
        if retry_run.is_some() && !report_retry { return Err("Only attachment preparation failures can be retried here. Inspect Gmail and history for other failures.".into()); }
        if !routine.settings.enabled || (routine.attempt.is_some() && !report_retry) {
            return Err("Routine is paused or this occurrence has already been attempted".into());
        }
        if routine.settings.auto_send != automatic { return Err("Automatic routines run only from their saved schedule. Turn off automatic sending to prepare a draft manually.".into()); }
        if automatic && !report_retry && !automatic_eligible(routine, now) { return Err("The automatic send window has expired. Reschedule this occurrence; it was not sent.".into()); }
        let plan = preview(routine)?;
        if plan.due != due || plan.parameters != expected {
            return Err("Routine changed. Open its preview again before starting".into());
        }
        if DateTime::parse_from_rfc3339(due)
            .map_err(|_| "Invalid due date")?
            .with_timezone(&Utc)
            > Utc::now()
        {
            return Err("This routine is not due yet".into());
        }
        // The public one-off command still rejects direct sending. Only a
        // claimed routine whose saved setting authorizes it gets this task.
        let mut inputs = plan.parameters.clone();
        inputs.remove("_sender_account");
        inputs.remove("_provider");
        inputs.remove("_attachment_ids");
        inputs.remove("_attachment_rules");
        let mut validated = inputs.clone();
        validated.insert("delivery".into(), serde_json::json!("Draft"));
        email_profile::validate_request(state, "foreground_email", &validated)?;
        let mut task = state
            .load_config()?
            .tasks
            .into_iter()
            .find(|t| t.id == "foreground_email")
            .ok_or("Email workflow missing")?;
        if automatic {
            let step = task.steps.iter_mut().find(|s| s.id == "delivery").ok_or("Delivery step missing")?;
            step.options.get_or_insert_with(Vec::new).push("Send directly".into());
        }
        let mut run = runner::prepare(&task, &inputs)?;
        run.parameters.insert("_attachments".into(), serde_json::json!(serde_json::to_string(&routine.settings.attachments).map_err(|_| "Invalid attachments")?));
        run.parameters.insert("_attachment_rules".into(), serde_json::json!(serde_json::to_string(&routine.settings.report_rules).map_err(|_|"Invalid report rules")?));
        run.parameters.insert("_attachment_due".into(), serde_json::json!(due));
        run.parameters.insert("_routine_id".into(), serde_json::json!(routine.id));
        run.parameters.insert("_provider".into(), serde_json::json!(routine.settings.provider));
        let sender = routine.settings.sender_account.as_deref().ok_or("Connect Gmail, then edit and save this routine with its sender account")?;
        run.parameters.insert("_sender_account".into(), serde_json::json!(sender));
        if automatic { run.logs.push("Automatic sending was enabled in the saved routine. This occurrence is claimed before any browser input; failed or uncertain sends are never retried automatically.".into()); }
        runner::insert(store, run.clone())?;
        store
            .email_routines
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .attempt = Some(run.id.clone());
        Ok((task, run, automatic))
    })
}

// A short grace period accommodates timer jitter and a temporarily busy engine.
// Older occurrences stay visible for rescheduling, never catch up by sending.
pub const AUTO_SEND_GRACE_SECONDS: i64 = 300;
fn automatic_eligible(routine: &EmailRoutine, now: DateTime<Utc>) -> bool {
    routine.settings.enabled
        && routine.settings.auto_send
        && routine.settings.sender_account.is_some()
        && routine.attempt.is_none()
        && routine.next_due.as_deref().is_some_and(|due| {
            DateTime::parse_from_rfc3339(due).is_ok_and(|due| {
                let age = now
                    .signed_duration_since(due.with_timezone(&Utc))
                    .num_seconds();
                due.with_timezone(&Utc) <= now && (0..=AUTO_SEND_GRACE_SECONDS).contains(&age)
            })
        })
}
pub fn automatic_tick(state: &AppState, account_ready: bool) -> Result<Option<Run>> {
    if crate::gmail::busy() {
        return Ok(None);
    }
    let google = if account_ready {
        crate::gmail::status(state)?.email
    } else {
        None
    };
    let custom = crate::mailserver::connected_sender();
    automatic_tick_filtered(
        state,
        account_ready || custom.is_some(),
        Utc::now(),
        crate::gmail::execute,
        |routine| {
            if routine.settings.provider == "mailserver"
                && crate::runtime::progress().is_ok_and(|p| p.active)
            {
                return false;
            }
            let account = if routine.settings.provider == "mailserver" {
                &custom
            } else {
                &google
            };
            account
                .as_deref()
                .zip(routine.settings.sender_account.as_deref())
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        },
    )
}
#[cfg(test)]
fn automatic_tick_with(
    state: &AppState,
    ready: bool,
    now: DateTime<Utc>,
    execute: impl FnOnce(&AppState, Task, Run, bool) -> Result<Run>,
) -> Result<Option<Run>> {
    automatic_tick_filtered(state, ready, now, execute, |_| true)
}
fn automatic_tick_filtered(
    state: &AppState,
    ready: bool,
    now: DateTime<Utc>,
    execute: impl FnOnce(&AppState, Task, Run, bool) -> Result<Run>,
    available: impl Fn(&EmailRoutine) -> bool,
) -> Result<Option<Run>> {
    if !ready {
        return Ok(None);
    }
    let plan = {
        let store = state.lock()?;
        if store.runs.iter().any(|r| r.status == "running") {
            return Ok(None);
        }
        store
            .email_routines
            .iter()
            .find(|r| {
                matches!(r.settings.provider.as_str(), "gmail" | "mailserver")
                    && automatic_eligible(r, now)
                    && available(r)
            })
            .map(preview)
            .transpose()?
    };
    let Some(plan) = plan else {
        return Ok(None);
    };
    let (task, run, direct) = claim(state, &plan.id, &plan.due, plan.parameters, true, now, None)?;
    let outcome = execute(state, task, run, direct)?;
    if outcome.status == "succeeded" {
        handle(state, &plan.id, &plan.due)?;
    }
    Ok(Some(outcome))
}
pub fn handle(state: &AppState, id: &str, due: &str) -> Result<()> {
    state.transaction(|store| {
        if store.runs.iter().any(|r| r.status == "running") {
            return Err("Wait for the current email run to finish".into());
        }
        let routine = store
            .email_routines
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("Routine not found")?;
        if routine.next_due.as_deref() != Some(due) {
            return Err("This occurrence has already been handled or changed".into());
        }
        let stamp = DateTime::parse_from_rfc3339(due)
            .map_err(|_| "Invalid due date")?
            .with_timezone(&Utc);
        routine.next_due = next_due(&routine.settings, Some(Utc::now().max(stamp)))?;
        routine.attempt = None;
        routine.notified_due = None;
        Ok(())
    })
}
pub fn remove(state: &AppState, id: &str) -> Result<()> {
    state.transaction(|store| {
        if store.runs.iter().any(|r| r.status == "running") {
            return Err("Wait for the current email run to finish".into());
        }
        store.email_routines.retain(|r| r.id != id);
        Ok(())
    })
}
#[derive(Clone, Debug)]
pub struct DueReminder {
    pub id: String,
    pub due: String,
    pub name: String,
    pub recipient_changed: bool,
    pub auto_send: bool,
}
pub fn activate_native_reminders(state: &AppState) -> Result<()> {
    // Prior versions used taskbar attention, not native banners. Offer one
    // native reminder for their outstanding occurrences without losing data.
    if state.lock()?.native_notification_version >= 1 {
        return Ok(());
    }
    state.transaction(|store| {
        if store.native_notification_version < 1 {
            for routine in &mut store.email_routines {
                routine.notified_due = None;
            }
            store.native_notification_version = 1;
        }
        Ok(())
    })
}
pub fn pending_reminders(state: &AppState) -> Result<Vec<DueReminder>> {
    let due = |r: &EmailRoutine| {
        r.settings.enabled
            && r.attempt.is_none()
            && r.next_due != r.notified_due
            && r.next_due
                .as_deref()
                .is_some_and(|d| DateTime::parse_from_rfc3339(d).is_ok_and(|d| d <= Utc::now()))
    };
    let store = state.lock()?;
    if store.runs.iter().any(|run| run.status == "running") {
        return Ok(Vec::new());
    }
    store
        .email_routines
        .iter()
        .filter(|r| matches!(r.settings.provider.as_str(), "gmail" | "mailserver") && due(r))
        .map(|r| {
            let plan = preview(r)?;
            Ok(DueReminder {
                id: r.id.clone(),
                due: plan.due,
                name: r.settings.name.clone(),
                recipient_changed: !plan.recipient_note.is_empty(),
                auto_send: r.settings.auto_send,
            })
        })
        .collect()
}
pub fn acknowledge_reminders(state: &AppState, reminders: &[DueReminder]) -> Result<()> {
    state.transaction(|store| {
        for routine in &mut store.email_routines {
            if reminders
                .iter()
                .any(|r| r.id == routine.id && routine.next_due.as_deref() == Some(&r.due))
            {
                routine.notified_due = routine.next_due.clone();
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_routines_default_to_gmail_and_provider_is_bound_to_the_plan() {
        let mut value = serde_json::to_value(settings()).unwrap();
        value.as_object_mut().unwrap().remove("provider");
        let old: RoutineSettings = serde_json::from_value(value).unwrap();
        assert_eq!(old.provider, "gmail");
        let (_dir, state, now) = setup_due_automatic();
        state
            .transaction(|store| {
                store.email_routines[0].settings.provider = "retired-provider".into();
                Ok(())
            })
            .unwrap();
        let routine = state.lock().unwrap().email_routines[0].clone();
        assert_eq!(
            preview(&routine).unwrap().parameters["_provider"],
            "retired-provider"
        );
        assert!(automatic_tick_filtered(
            &state,
            true,
            now,
            |_, _, _, _| panic!("disconnected provider must not execute"),
            |_| true
        )
        .unwrap()
        .is_none());
        assert!(state.lock().unwrap().email_routines[0].attempt.is_none());
        let mut unsupported = settings();
        unsupported.provider = "smtp".into();
        assert!(validate(&unsupported).is_err());
        unsupported.provider = "retired-provider".into();
        unsupported.calendar = Some(CalendarSettings {
            enabled: true,
            reminder_minutes: 60,
        });
        assert!(validate(&unsupported).is_err());
    }
    fn setup_due_automatic() -> (tempfile::TempDir, AppState, DateTime<Utc>) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        let now = Utc::now();
        let mut s = settings();
        s.sender_account = Some("sender@example.com".into());
        s.auto_send = true;
        s.cc = "copy@example.com, team@example.com".into();
        s.repeat = "daily".into();
        s.start_date = (Local::now() + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        s.recipient_change = Some(RecipientChange {
            recipient: "cover@example.com".into(),
            from: "2020-01-01".into(),
            until: None,
            note: "New recipient".into(),
        });
        save(&state, None, s).unwrap();
        state
            .transaction(|store| {
                store.email_routines[0].next_due =
                    Some((now - chrono::Duration::seconds(10)).to_rfc3339());
                Ok(())
            })
            .unwrap();
        (dir, state, now)
    }
    #[test]
    fn automatic_send_claims_the_occurrence_once_and_uses_the_resolved_address() {
        let (_dir, state, now) = setup_due_automatic();
        let outcome = automatic_tick_with(&state, true, now, |state, task, mut run, direct| {
            assert!(direct);
            assert_eq!(run.parameters["delivery"], "Send directly");
            assert_eq!(run.parameters["recipient"], "cover@example.com");
            assert_eq!(run.parameters["cc"], "copy@example.com, team@example.com");
            assert!(task
                .steps
                .iter()
                .find(|s| s.id == "delivery")
                .unwrap()
                .options
                .as_ref()
                .unwrap()
                .contains(&"Send directly".into()));
            assert_eq!(
                state.lock().unwrap().email_routines[0].attempt.as_deref(),
                Some(run.id.as_str())
            );
            // Fake the provider result. No browser input or email transmission.
            run.status = "succeeded".into();
            state
                .transaction(|store| {
                    store.runs[0] = run.clone();
                    Ok(())
                })
                .unwrap();
            Ok(run)
        })
        .unwrap()
        .unwrap();
        assert_eq!(outcome.status, "succeeded");
        assert!(
            automatic_tick_with(&state, true, now, |_, _, _, _| panic!("duplicate send"))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn locked_desktop_and_missed_occurrences_never_launch_or_catch_up_by_sending() {
        let (_dir, state, now) = setup_due_automatic();
        assert!(
            automatic_tick_with(&state, false, now, |_, _, _, _| panic!("locked desktop"))
                .unwrap()
                .is_none()
        );
        assert!(automatic_tick_with(
            &state,
            true,
            now + chrono::Duration::minutes(6),
            |_, _, _, _| panic!("late send")
        )
        .unwrap()
        .is_none());
        assert!(state.lock().unwrap().runs.is_empty());
    }
    #[test]
    fn a_crash_after_claim_blocks_retries_even_after_restart() {
        let (dir, state, now) = setup_due_automatic();
        assert!(automatic_tick_with(&state, true, now, |_, _, _, _| Err(
            "simulated shutdown".into()
        ))
        .is_err());
        drop(state);
        let state = AppState::initialize(dir.path().into()).unwrap();
        assert_eq!(state.lock().unwrap().runs[0].status, "interrupted");
        assert!(
            automatic_tick_with(&state, true, now, |_, _, _, _| panic!("retry after crash"))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn manual_routine_invocation_cannot_bypass_scheduled_send_and_old_settings_default_to_drafts() {
        let (_dir, state, _) = setup_due_automatic();
        let r = state.lock().unwrap().email_routines[0].clone();
        let p = preview(&r).unwrap();
        assert!(run(&state, &p.id, &p.due, p.parameters).is_err());
        assert!(state.lock().unwrap().runs.is_empty());
        let mut old = serde_json::to_value(settings()).unwrap();
        old.as_object_mut().unwrap().remove("auto_send");
        assert!(
            !serde_json::from_value::<RoutineSettings>(old)
                .unwrap()
                .auto_send
        );
    }
    #[test]
    fn enabling_automatic_sending_moves_an_old_recurring_due_date_into_the_future() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        save(&state, None, settings()).unwrap();
        let r = state.lock().unwrap().email_routines[0].clone();
        let mut automatic = r.settings.clone();
        automatic.auto_send = true;
        save(&state, Some(r.id), automatic).unwrap();
        let due = state.lock().unwrap().email_routines[0]
            .next_due
            .clone()
            .unwrap();
        assert!(
            DateTime::parse_from_rfc3339(&due)
                .unwrap()
                .with_timezone(&Utc)
                > Utc::now()
        );
        let mut expired = settings();
        expired.repeat = "once".into();
        expired.auto_send = true;
        assert!(save(&state, None, expired).is_err());
    }
    #[test]
    fn changing_saved_sender_invalidates_an_old_plan_and_unbound_legacy_routines_do_not_run() {
        let (_dir, state, now) = setup_due_automatic();
        let original = state.lock().unwrap().email_routines[0].clone();
        let plan = preview(&original).unwrap();
        let mut changed = original.settings.clone();
        changed.sender_account = Some("different@example.com".into());
        save(&state, Some(original.id.clone()), changed).unwrap();
        assert!(claim(
            &state,
            &plan.id,
            &plan.due,
            plan.parameters,
            true,
            now,
            None
        )
        .unwrap_err()
        .contains("Routine changed"));
        state
            .transaction(|store| {
                store.email_routines[0].settings.sender_account = None;
                Ok(())
            })
            .unwrap();
        assert!(automatic_tick_with(&state, true, now, |_, _, _, _| panic!(
            "unbound sender must not send"
        ))
        .unwrap()
        .is_none());
        assert!(state.lock().unwrap().runs.is_empty());
    }
    #[test]
    fn old_routines_default_to_empty_cc_and_invalid_cc_is_rejected() {
        let mut value = serde_json::to_value(settings()).unwrap();
        value.as_object_mut().unwrap().remove("cc");
        let mut old: RoutineSettings = serde_json::from_value(value).unwrap();
        assert!(old.cc.is_empty());
        validate(&old).unwrap();
        old.cc = "invalid".into();
        assert!(validate(&old).is_err());
    }
    #[test]
    fn only_pre_send_report_failures_can_be_retried_and_each_explicit_retry_claims_once() {
        let (_dir, state, now) = setup_due_automatic();
        let plan = {
            let store = state.lock().unwrap();
            preview(&store.email_routines[0]).unwrap()
        };
        let (_, mut run, _) = claim(
            &state,
            &plan.id,
            &plan.due,
            plan.parameters.clone(),
            true,
            now,
            None,
        )
        .unwrap();
        run.status = "failed".into();
        run.error = Some("Gmail request outcome is uncertain".into());
        state
            .transaction(|store| {
                *store.runs.iter_mut().find(|r| r.id == run.id).unwrap() = run.clone();
                Ok(())
            })
            .unwrap();
        assert!(claim(
            &state,
            &plan.id,
            &plan.due,
            plan.parameters.clone(),
            true,
            now,
            Some(&run.id)
        )
        .is_err());
        run.error = Some(format!(
            "{}Report missing",
            crate::attachment_rules::ISSUE_PREFIX
        ));
        state
            .transaction(|store| {
                *store.runs.iter_mut().find(|r| r.id == run.id).unwrap() = run.clone();
                Ok(())
            })
            .unwrap();
        // Even after the normal automatic grace expires, an explicit report retry
        // is allowed: the previous failure is proven to be before any send.
        let future = now + chrono::Duration::days(1);
        let (_, retry, direct) = claim(
            &state,
            &plan.id,
            &plan.due,
            plan.parameters.clone(),
            true,
            future,
            Some(&run.id),
        )
        .unwrap();
        assert!(direct);
        assert_ne!(retry.id, run.id);
        assert!(claim(
            &state,
            &plan.id,
            &plan.due,
            plan.parameters,
            true,
            future,
            Some(&run.id)
        )
        .is_err());
    }
    #[test]
    fn generated_report_blocks_the_occurrence_and_is_not_retried_automatically() {
        let (dir, state, now) = setup_due_automatic();
        let folder = dir.path().join("reports");
        std::fs::create_dir(&folder).unwrap();
        let template = dir.path().join("blank.txt");
        std::fs::write(&template, b"Fill in actual monthly figures").unwrap();
        let template = crate::attachments::stage(&state, &[template])
            .unwrap()
            .remove(0);
        let rule = crate::attachment_rules::ReportRule {
            id: uuid::Uuid::new_v4().to_string(),
            folder: crate::attachment_sources::register(&state, &folder).unwrap(),
            pattern: "current-report.txt".into(),
            previous_month: false,
            template: Some(template),
            require_changed: true,
        };
        state
            .transaction(|store| {
                store.email_routines[0].settings.report_rules = vec![rule];
                Ok(())
            })
            .unwrap();
        let result = automatic_tick_with(&state, true, now, |state, task, run, _| {
            let error =
                crate::attachment_rules::from_run(state, &run, crate::attachments::MAX_BYTES)
                    .err()
                    .unwrap();
            runner::record_result(state, &task, run, Err(error))
        })
        .unwrap()
        .unwrap();
        assert_eq!(result.status, "failed");
        assert!(result
            .error
            .unwrap()
            .starts_with(crate::attachment_rules::ISSUE_PREFIX));
        assert!(folder.join("current-report.txt").is_file());
        assert!(automatic_tick_with(&state, true, now, |_, _, _, _| panic!(
            "A pending report must never auto-retry"
        ))
        .unwrap()
        .is_none());
        let routine = state.lock().unwrap().email_routines[0].clone();
        let mut updated = routine.settings.clone();
        updated.report_rules[0].require_changed = false;
        save(&state, Some(routine.id.clone()), updated.clone()).unwrap();
        assert_eq!(
            state.lock().unwrap().email_routines[0].attempt,
            routine.attempt
        );
        updated.auto_send = false;
        assert!(save(&state, Some(routine.id), updated).is_err());
    }
    fn settings() -> RoutineSettings {
        RoutineSettings {
            provider: "gmail".into(),
            report_rules: Vec::new(),
            calendar: None,
            cc: String::new(),
            name: "Report".into(),
            recipient: "a@example.com".into(),
            subject: "{{previous_month}} {{previous_year}} report".into(),
            message: "Report for {{month}} {{year}}.\n\nPrepared {{date}}.".into(),
            start_date: "2026-01-31".into(),
            time: "09:00".into(),
            repeat: "monthly".into(),
            weekdays: vec![],
            enabled: true,
            auto_send: false,
            sender_account: Some("sender@example.com".into()),
            attachments: Vec::new(),
            recipient_change: None,
        }
    }
    #[test]
    fn templates_use_occurrence_not_current_month_and_preserve_blank_lines() {
        let day = date("2026-01-01").unwrap();
        assert_eq!(
            render(
                "{{previous_month}} {{previous_year}}\n\n{{month}} {{year}}",
                day
            )
            .unwrap(),
            "December 2025\n\nJanuary 2026"
        );
        assert!(render("{{unknown}}", day).is_err());
        assert!(render("{{month", day).is_err());
    }
    #[test]
    fn permanent_changes_do_not_expire_and_invalid_overrides_cannot_be_saved() {
        let mut s = settings();
        s.recipient_change = Some(RecipientChange {
            recipient: "b@example.com".into(),
            from: "2026-02-01".into(),
            until: None,
            note: "New manager".into(),
        });
        let due = Local
            .from_local_datetime(&date("2027-03-31").unwrap().and_hms_opt(9, 0, 0).unwrap())
            .earliest()
            .unwrap()
            .to_rfc3339();
        let routine = EmailRoutine {
            id: "r".into(),
            settings: s.clone(),
            next_due: Some(due),
            attempt: None,
            notified_due: None,
        };
        assert_eq!(
            preview(&routine).unwrap().parameters["recipient"],
            "b@example.com"
        );
        s.recipient_change.as_mut().unwrap().until = Some("2026-01-01".into());
        assert_eq!(preview(&routine).unwrap().parameters["delivery"], "Draft");
        assert!(validate(&s).is_err());
    }
    #[test]
    fn stale_preview_is_rejected_before_any_run_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        save(&state, None, settings()).unwrap();
        let routine = state.lock().unwrap().email_routines[0].clone();
        let plan = preview(&routine).unwrap();
        let mut changed = routine.settings.clone();
        changed.recipient = "changed@example.com".into();
        save(&state, Some(routine.id.clone()), changed).unwrap();
        assert!(run(&state, &routine.id, &plan.due, plan.parameters)
            .unwrap_err()
            .contains("Routine changed"));
        assert!(state.lock().unwrap().runs.is_empty());
    }
    #[test]
    fn monthly_short_month_does_not_drift_and_weekdays_are_respected() {
        let mut s = settings();
        assert!(scheduled_on(
            &s,
            date("2026-02-28").unwrap(),
            date(&s.start_date).unwrap()
        ));
        assert!(scheduled_on(
            &s,
            date("2026-03-31").unwrap(),
            date(&s.start_date).unwrap()
        ));
        assert!(!scheduled_on(
            &s,
            date("2026-03-28").unwrap(),
            date(&s.start_date).unwrap()
        ));
        s.repeat = "weekly".into();
        s.weekdays = vec![0, 4];
        assert!(scheduled_on(
            &s,
            date("2026-02-02").unwrap(),
            date(&s.start_date).unwrap()
        ));
        assert!(!scheduled_on(
            &s,
            date("2026-02-03").unwrap(),
            date(&s.start_date).unwrap()
        ));
    }
    #[test]
    fn temporary_recipient_change_is_inclusive_and_reverts() {
        let mut s = settings();
        s.recipient_change = Some(RecipientChange {
            recipient: "b@example.com".into(),
            from: "2026-02-01".into(),
            until: Some("2026-02-28".into()),
            note: "Covering leave".into(),
        });
        for (day, expected) in [
            ("2026-01-31", "a@example.com"),
            ("2026-02-28", "b@example.com"),
            ("2026-03-31", "a@example.com"),
        ] {
            let stamp = Local
                .from_local_datetime(&date(day).unwrap().and_hms_opt(9, 0, 0).unwrap())
                .earliest()
                .unwrap()
                .to_rfc3339();
            let routine = EmailRoutine {
                id: "r".into(),
                settings: s.clone(),
                next_due: Some(stamp),
                attempt: None,
                notified_due: None,
            };
            assert_eq!(preview(&routine).unwrap().parameters["recipient"], expected);
        }
    }
    #[test]
    fn reminders_and_duplicate_handling_survive_restart_without_executing_email() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        save(&state, None, settings()).unwrap();
        let pending = pending_reminders(&state).unwrap();
        assert_eq!(pending.len(), 1);
        // A failed notification dispatch must leave the reminder pending.
        assert_eq!(pending_reminders(&state).unwrap().len(), 1);
        acknowledge_reminders(&state, &pending).unwrap();
        assert!(pending_reminders(&state).unwrap().is_empty());
        let r = state.lock().unwrap().email_routines[0].clone();
        state
            .transaction(|store| {
                store.email_routines[0].attempt = Some("uncertain".into());
                Ok(())
            })
            .unwrap();
        assert!(run(
            &state,
            &r.id,
            r.next_due.as_deref().unwrap(),
            preview(&r).unwrap().parameters
        )
        .is_err());
        drop(state);
        let state = AppState::initialize(dir.path().into()).unwrap();
        assert_eq!(
            state.lock().unwrap().email_routines[0].attempt.as_deref(),
            Some("uncertain")
        );
        handle(&state, &r.id, r.next_due.as_deref().unwrap()).unwrap();
        assert!(handle(&state, &r.id, r.next_due.as_deref().unwrap()).is_err());
        assert!(state.lock().unwrap().runs.is_empty());
    }
    #[test]
    fn old_attention_reminders_migrate_once_and_stale_acknowledgement_does_not_hide_new_due_date() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        save(&state, None, settings()).unwrap();
        let pending = pending_reminders(&state).unwrap();
        acknowledge_reminders(&state, &pending).unwrap();
        assert!(pending_reminders(&state).unwrap().is_empty());
        activate_native_reminders(&state).unwrap();
        assert_eq!(pending_reminders(&state).unwrap().len(), 1);
        acknowledge_reminders(&state, &pending).unwrap();
        activate_native_reminders(&state).unwrap();
        assert!(pending_reminders(&state).unwrap().is_empty());
        state
            .transaction(|store| {
                store.email_routines[0].next_due = Some("2026-02-28T09:00:00Z".into());
                store.email_routines[0].notified_due = None;
                Ok(())
            })
            .unwrap();
        acknowledge_reminders(&state, &pending).unwrap();
        assert!(state.lock().unwrap().email_routines[0]
            .notified_due
            .is_none());
    }
}

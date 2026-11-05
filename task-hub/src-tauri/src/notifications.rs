//! Native OS banners; notification timers never start foreground automation.
use crate::{email_routines, model::Result, storage::AppState};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn show(app: &AppHandle, title: &str, body: &str) -> Result<()> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| format!("Could not request a Windows notification: {error}"))
}
pub fn deliver_due(app: &AppHandle, state: &AppState) -> Result<()> {
    let reminders = email_routines::pending_reminders(state)?;
    if reminders.is_empty() {
        return Ok(());
    }
    let names = reminders
        .iter()
        .take(3)
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let change = if reminders.iter().any(|r| r.recipient_changed) {
        " A recipient change applies; see the saved routine."
    } else {
        ""
    };
    let message = if reminders.iter().any(|r| r.auto_send) {
        "An automatic email is pending. Connect its saved Gmail sender in Task Hub and check internet access. Occurrences over five minutes late need rescheduling."
    } else {
        "Open Task Hub to prepare your draft. Nothing will be sent."
    };
    show(
        app,
        "Email routine reminder",
        &format!("{names}. {message}{change}"),
    )?;
    email_routines::acknowledge_reminders(state, &reminders)
}

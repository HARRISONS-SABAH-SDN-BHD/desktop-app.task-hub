//! Background Gmail transport. No UI Automation, keyboard input or browser session.
pub mod calendar;
mod oauth;
mod transport;
mod vault;
use crate::{
    email_profile,
    model::*,
    runner,
    storage::{atomic_write, AppState},
    validation,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
static OPERATION: Mutex<()> = Mutex::new(());
static CANCEL_CONNECT: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
struct Credentials {
    client_id: String,
    client_secret: Option<String>,
    refresh_token: String,
    email: String,
    subject: String,
    #[serde(default)]
    calendar_access: bool,
}
#[derive(Serialize)]
pub struct ConnectionStatus {
    pub calendar_access: bool,
    pub publisher_configured: bool,
    pub connected: bool,
    pub email: Option<String>,
    pub client_id: String,
    pub busy: bool,
}
pub(crate) fn lock_for_update() -> Result<std::sync::MutexGuard<'static, ()>> {
    OPERATION.try_lock().map_err(|_| "A Google or email operation is running. Wait for it to finish before installing an update.".into())
}
pub fn busy() -> bool {
    OPERATION.try_lock().is_err()
}
pub fn status(state: &AppState) -> Result<ConnectionStatus> {
    let busy = busy();
    let credential = vault::read()?;
    let client_id = credential
        .as_ref()
        .map(|c| c.client_id.clone())
        .unwrap_or_else(|| {
            std::fs::read(state.root.join("gmail-client.json"))
                .ok()
                .filter(|v| v.len() < 4096)
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .and_then(|v| {
                    v.get("client_id")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                })
                .unwrap_or_default()
        });
    Ok(ConnectionStatus {
        publisher_configured: publisher_client().is_some(),
        calendar_access: credential.as_ref().is_some_and(|c| c.calendar_access),
        connected: credential.is_some(),
        email: credential.map(|c| c.email),
        client_id,
        busy,
    })
}
pub fn ready() -> bool {
    OPERATION.try_lock().is_ok() && vault::read().ok().flatten().is_some()
}
// Desktop OAuth clients are public clients. Only the publisher's desktop
// registration is bundled; user tokens remain in the Windows credential vault.
fn publisher_client() -> Option<(&'static str, Option<&'static str>)> {
    let id = option_env!("TASK_HUB_GOOGLE_CLIENT_ID")?.trim();
    if oauth::validate_client_id(id).is_err() {
        return None;
    }
    let secret = option_env!("TASK_HUB_GOOGLE_CLIENT_SECRET")
        .map(str::trim)
        .filter(|value| !value.is_empty());
    Some((id, secret))
}
pub fn connect(
    state: &AppState,
    id: String,
    secret: Option<String>,
    calendar_access: bool,
    open: impl FnOnce(String) -> Result<()>,
) -> Result<()> {
    let (id, secret) = match publisher_client() {
        Some((id, secret)) => (id.to_owned(), secret.map(str::to_owned)),
        None => (id, secret),
    };
    oauth::connect(state, id, secret, calendar_access, open)
}
pub fn cancel_connect() {
    CANCEL_CONNECT.store(true, Ordering::SeqCst);
}
pub fn disconnect() -> Result<()> {
    let _operation = OPERATION
        .try_lock()
        .map_err(|_| "Wait for the current Gmail operation before disconnecting")?;
    vault::remove()
}
fn check_sender(actual: &str, expected: &str) -> Result<()> {
    if !validation::valid_email(expected) || !actual.eq_ignore_ascii_case(expected) {
        return Err("The connected Gmail account does not match this email's saved sender. Reconnect that account, or edit and save the routine with the intended sender. Nothing was submitted to Gmail.".into());
    }
    Ok(())
}
fn deliver(state: &AppState, run: &mut Run, direct: bool) -> Result<(String, Vec<String>)> {
    let _operation = OPERATION
        .try_lock()
        .map_err(|_| "Another Gmail operation is in progress; no email request was made")?;
    let mut credential =
        vault::read()?.ok_or("Connect Gmail in Task Hub before preparing or sending email")?;
    let text = |name: &str| {
        run.parameters
            .get(name)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing email field: {name}"))
    };
    check_sender(&credential.email, text("_sender_account")?)?;
    let delivery = text("delivery")?;
    let send = match delivery {
        "Draft" => false,
        "Send after review" => true, // User submitted the one-off review screen.
        "Send directly" if direct => true, // Saved routine's claimed authorization.
        _ => return Err("Email sending is not authorized for this invocation".into()),
    };
    let mut attachments = crate::attachments::from_parameters(state, &run.parameters)?;
    let fixed_bytes = attachments
        .iter()
        .map(|f| f.bytes.len() as u64)
        .sum::<u64>();
    let reports =
        crate::attachment_rules::from_run(state, run, crate::attachments::MAX_BYTES - fixed_bytes)?;
    let report_receipts = reports.receipts;
    attachments.extend(reports.files);
    if attachments.len() > crate::attachments::MAX_FILES {
        return Err(format!(
            "{}Use at most 10 attachments",
            crate::attachment_rules::ISSUE_PREFIX
        ));
    }
    if !report_receipts.is_empty() {
        let mut names = std::collections::BTreeSet::new();
        if attachments
            .iter()
            .any(|a| !names.insert(a.name.to_lowercase()))
        {
            return Err(format!("{}Two attachments have the same filename. Remove the duplicate fixed file or report rule.",crate::attachment_rules::ISSUE_PREFIX));
        }
    }
    let raw = transport::mime(
        &credential.email,
        text("recipient")?,
        run.parameters
            .get("cc")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        text("subject")?,
        text("message")?,
        &run.id,
        &attachments,
    )?;
    if !report_receipts.is_empty() {
        run.parameters.insert(
            "_resolved_reports".into(),
            serde_json::json!(serde_json::to_string(&report_receipts)
                .map_err(|_| "Could not record report attachments")?),
        );
        state.transaction(|store| {
            let current = store
                .runs
                .iter_mut()
                .find(|r| r.id == run.id)
                .ok_or("Email run not found")?;
            current.parameters = run.parameters.clone();
            Ok(())
        })?;
    }
    let client = transport::client()?;
    let token = oauth::access_token(&client, &mut credential)?;
    let id = transport::deliver(&client, "https://gmail.googleapis.com", &token, &raw, send)?;
    let summary = if send {
        "Gmail API accepted the email for sending. Final delivery depends on the recipient's provider."
    } else {
        "Draft created in Gmail through the API. Nothing was sent."
    };
    let mut logs = vec![
        "Using Gmail API in the background; no browser or desktop input is used.".into(),
        format!("Connected sender: {}", credential.email),
        summary.into(),
        format!("Gmail {} ID: {id}", if send { "message" } else { "draft" }),
    ];
    if !attachments.is_empty() {
        logs.push(format!(
            "Attached files: {}",
            attachments
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let receipt = state.output.join(format!("{}.gmail.txt", run.id));
    atomic_write(&receipt, logs.join("\n").as_bytes()).map_err(|_| "Gmail accepted the request, but Task Hub could not write its receipt. Inspect Gmail before retrying; no retry was attempted.")?;
    Ok((receipt.to_string_lossy().into_owned(), logs))
}
pub fn execute(state: &AppState, task: Task, mut run: Run, direct: bool) -> Result<Run> {
    let outcome = deliver(state, &mut run, direct);
    runner::record_result(state, &task, run, outcome)
}
pub fn manual(
    state: &AppState,
    id: &str,
    parameters: Parameters,
    sender: &str,
    attachments: Vec<crate::attachments::AttachmentRef>,
) -> Result<Run> {
    email_profile::validate_request(state, id, &parameters)?;
    crate::attachments::load(state, &attachments)?;
    if !validation::valid_email(sender) {
        return Err("Connect Gmail and select a sender account before starting".into());
    }
    let task = state
        .load_config()?
        .tasks
        .into_iter()
        .find(|t| t.id == id)
        .ok_or("Email workflow missing")?;
    let mut run = runner::prepare(&task, &parameters)?;
    run.parameters
        .insert("_sender_account".into(), serde_json::json!(sender));
    run.parameters.insert(
        "_attachments".into(),
        serde_json::json!(serde_json::to_string(&attachments).map_err(|_| "Invalid attachments")?),
    );
    state.transaction(|store| runner::insert(store, run.clone()))?;
    execute(state, task, run, false)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_different_connected_sender_never_silently_replaces_the_saved_account() {
        assert!(check_sender("expected@gmail.com", "expected@gmail.com").is_ok());
        assert!(check_sender("other@gmail.com", "expected@gmail.com").is_err());
        assert!(check_sender("other@gmail.com", "").is_err());
    }
}

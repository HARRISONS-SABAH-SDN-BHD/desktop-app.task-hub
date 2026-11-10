//! Microsoft 365 delegated OAuth and Graph delivery; no desktop interaction.
pub mod calendar;
mod oauth;
mod transport;
mod vault;
use crate::{model::Result, storage::AppState};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
static CANCEL: AtomicBool = AtomicBool::new(false);
static SIGN_IN_CODE: Mutex<Option<String>> = Mutex::new(None);
#[derive(Serialize, Deserialize)]
pub(crate) struct Credentials {
    client_id: String,
    tenant_id: String,
    refresh_token: String,
    pub email: String,
    subject: String,
    #[serde(default)]
    calendar_access: bool,
}
#[derive(Serialize)]
pub struct Status {
    pub calendar_access: bool,
    pub connected: bool,
    pub email: Option<String>,
    pub client_id: String,
    pub tenant_id: String,
    pub publisher_configured: bool,
    pub user_code: Option<String>,
    pub busy: bool,
}
fn publisher() -> Option<(String, String)> {
    let id = option_env!("TASK_HUB_MICROSOFT_CLIENT_ID")?;
    let tenant = option_env!("TASK_HUB_MICROSOFT_TENANT_ID")?;
    oauth::validate_registration(id, tenant).ok()?;
    Some((id.into(), tenant.into()))
}
pub fn status(state: &AppState) -> Result<Status> {
    let credential = vault::read()?;
    let saved = std::fs::read(state.root.join("microsoft-client.json"))
        .ok()
        .filter(|b| b.len() < 4096)
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    let shared = publisher();
    Ok(Status {
        calendar_access: credential.as_ref().is_some_and(|c| c.calendar_access),
        connected: credential.is_some(),
        email: credential.as_ref().map(|c| c.email.clone()),
        client_id: shared
            .as_ref()
            .map(|c| c.0.clone())
            .or_else(|| credential.as_ref().map(|c| c.client_id.clone()))
            .or_else(|| {
                saved
                    .as_ref()?
                    .get("client_id")?
                    .as_str()
                    .map(str::to_owned)
            })
            .unwrap_or_default(),
        tenant_id: shared
            .as_ref()
            .map(|c| c.1.clone())
            .or_else(|| credential.as_ref().map(|c| c.tenant_id.clone()))
            .or_else(|| {
                saved
                    .as_ref()?
                    .get("tenant_id")?
                    .as_str()
                    .map(str::to_owned)
            })
            .unwrap_or_default(),
        publisher_configured: shared.is_some(),
        user_code: SIGN_IN_CODE
            .lock()
            .map_err(|_| "Microsoft sign-in status unavailable")?
            .clone(),
        busy: crate::gmail::busy(),
    })
}
pub fn connect(
    state: &AppState,
    id: String,
    tenant: String,
    calendar_access: bool,
    open: impl FnOnce(String) -> Result<()>,
) -> Result<()> {
    let _operation = crate::gmail::lock_for_update()?;
    CANCEL.store(false, Ordering::SeqCst);
    let (id, tenant) = publisher().unwrap_or((id, tenant));
    let result = oauth::connect(state, id.trim(), tenant.trim(), calendar_access, open);
    if let Ok(mut code) = SIGN_IN_CODE.lock() {
        *code = None;
    }
    result
}
pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}
pub fn disconnect() -> Result<()> {
    let _operation = crate::gmail::lock_for_update()?;
    vault::remove()
}
pub(crate) fn credentials() -> Result<Credentials> {
    vault::read()?
        .ok_or("Connect the intended Microsoft 365 account before running this routine".into())
}
pub(crate) fn connected_sender() -> Option<String> {
    vault::read().ok().flatten().map(|c| c.email)
}
pub(crate) fn deliver(credential: &mut Credentials, raw: &str, send: bool) -> Result<String> {
    // Check the complete encoded request before requesting a token or mutating mail.
    transport::validate_size(raw)?;
    let client = crate::gmail::transport::client()?;
    let token = oauth::access_token(&client, credential)?;
    transport::deliver(
        &client,
        "https://graph.microsoft.com/v1.0",
        &token,
        raw,
        send,
    )
}

pub(crate) fn source_token() -> Result<(reqwest::blocking::Client, String)> {
    let mut credential = credentials()?;
    let client = crate::gmail::transport::client()?;
    let token = oauth::access_token(&client, &mut credential)?;
    Ok((client, token))
}

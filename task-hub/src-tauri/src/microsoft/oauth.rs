use super::{vault, Credentials, CANCEL, SIGN_IN_CODE};
use crate::{
    model::Result,
    storage::{atomic_write, AppState},
    validation,
};
use serde::Deserialize;
use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
const SCOPE: &str = "offline_access https://graph.microsoft.com/User.Read https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send";
pub(super) fn validate_registration(id: &str, tenant: &str) -> Result<()> {
    if uuid::Uuid::parse_str(id).is_err() || uuid::Uuid::parse_str(tenant).is_err() {
        return Err("Enter the Application (client) ID and Directory (tenant) ID from your company's Microsoft Entra app registration.".into());
    }
    Ok(())
}
#[derive(Deserialize)]
struct Device {
    device_code: String,
    user_code: String,
    expires_in: u64,
    interval: Option<u64>,
}
#[derive(Deserialize)]
struct Token {
    access_token: String,
    refresh_token: Option<String>,
    scope: Option<String>,
}
fn check_scope(token: &Token) -> Result<()> {
    let scopes = token.scope.as_deref().unwrap_or_default();
    if token.access_token.is_empty()
        || !["User.Read", "Mail.ReadWrite", "Mail.Send"]
            .iter()
            .all(|required| {
                scopes.split_whitespace().any(|s| {
                    s.eq_ignore_ascii_case(required)
                        || s.eq_ignore_ascii_case(&format!(
                            "https://graph.microsoft.com/{required}"
                        ))
                })
            })
    {
        return Err("Microsoft did not grant the required profile, draft and send permissions. Ask IT to approve Task Hub and reconnect.".into());
    }
    Ok(())
}
fn token_url(tenant: &str) -> String {
    format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token")
}
fn wait(seconds: u64, end: Instant) -> Result<()> {
    let until = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < until {
        if CANCEL.load(Ordering::SeqCst) {
            return Err("Microsoft sign-in cancelled".into());
        }
        if Instant::now() >= end {
            return Err("Microsoft sign-in expired. Connect again.".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
pub(super) fn connect(
    state: &AppState,
    id: &str,
    tenant: &str,
    open: impl FnOnce(String) -> Result<()>,
) -> Result<()> {
    validate_registration(id, tenant)?;
    let client = crate::gmail::transport::client()?;
    let response = client
        .post(format!(
            "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/devicecode"
        ))
        .form(&[("client_id", id), ("scope", SCOPE)])
        .send()
        .map_err(|_| "Could not start Microsoft sign-in. Check internet access.")?;
    if !response.status().is_success() {
        return Err(format!("Microsoft sign-in returned HTTP {}. Ask IT to check the tenant, app registration and public client flows.",response.status().as_u16()));
    }
    let device: Device = response
        .json()
        .map_err(|_| "Microsoft returned unreadable sign-in instructions")?;
    if device.user_code.is_empty()
        || device.user_code.len() > 32
        || device.device_code.is_empty()
        || device.expires_in == 0
    {
        return Err("Invalid Microsoft sign-in response".into());
    }
    *SIGN_IN_CODE
        .lock()
        .map_err(|_| "Microsoft sign-in unavailable")? = Some(device.user_code);
    // Fixed Microsoft URL: never open arbitrary URLs from an HTTP response.
    open("https://microsoft.com/devicelogin".into())?;
    let end = Instant::now() + Duration::from_secs(device.expires_in.min(900));
    let mut interval = device.interval.unwrap_or(5).clamp(5, 60);
    let token: Token = loop {
        wait(interval, end)?;
        let response = client
            .post(token_url(tenant))
            .form(&[
                ("client_id", id),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", device.device_code.as_str()),
            ])
            .send()
            .map_err(|_| "Microsoft sign-in connection was interrupted. Connect again.")?;
        if response.status().is_success() {
            break response
                .json()
                .map_err(|_| "Microsoft returned unreadable account access")?;
        }
        let error: serde_json::Value = response.json().unwrap_or_default();
        match error.get("error").and_then(|v|v.as_str()) {
            Some("authorization_pending")=>continue,
            Some("slow_down")=>{interval=(interval+5).min(60);continue},
            Some("authorization_declined"|"access_denied")=>return Err("Microsoft sign-in was declined or blocked by company policy. Ask IT to approve Task Hub.".into()),
            Some("expired_token")=>return Err("Microsoft sign-in expired. Connect again.".into()),
            _=>return Err("Microsoft sign-in failed. Ask IT to check Task Hub's app registration and sign-in policy.".into()),
        }
    };
    check_scope(&token)?;
    let refresh = token.refresh_token.filter(|v| !v.is_empty()).ok_or(
        "Microsoft did not grant offline access. Reconnect and ask IT to approve Task Hub.",
    )?;
    #[derive(Deserialize)]
    struct User {
        id: String,
        mail: Option<String>,
    }
    let response = client
        .get("https://graph.microsoft.com/v1.0/me?$select=id,mail")
        .bearer_auth(&token.access_token)
        .send()
        .map_err(|_| "Could not verify Microsoft mailbox")?;
    if !response.status().is_success() {
        return Err(
            "Could not verify Microsoft mailbox. Reconnect or ask IT to check access.".into(),
        );
    }
    let user: User = response
        .json()
        .map_err(|_| "Unreadable Microsoft mailbox identity")?;
    // A UPN is not necessarily the mailbox address. Require Graph's primary mail address.
    let email=user.mail.filter(|v|validation::valid_email(v)).ok_or("Microsoft account has no primary mailbox address. Ask IT to check the Exchange Online mailbox.")?;
    if user.id.is_empty() || CANCEL.load(Ordering::SeqCst) {
        return Err("Microsoft connection cancelled or identity invalid".into());
    }
    atomic_write(
        &state.root.join("microsoft-client.json"),
        &serde_json::to_vec(&serde_json::json!({"client_id":id,"tenant_id":tenant}))
            .map_err(|_| "Could not save Microsoft app settings")?,
    )?;
    vault::write(&Credentials {
        client_id: id.into(),
        tenant_id: tenant.into(),
        refresh_token: refresh,
        email: email.to_lowercase(),
        subject: user.id,
    })
}
pub(super) fn access_token(
    client: &reqwest::blocking::Client,
    credential: &mut Credentials,
) -> Result<String> {
    validate_registration(&credential.client_id, &credential.tenant_id)?;
    let response = client
        .post(token_url(&credential.tenant_id))
        .form(&[
            ("client_id", credential.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", credential.refresh_token.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .map_err(|_| "Could not refresh Microsoft access. Nothing was submitted to Outlook.")?;
    if !response.status().is_success() {
        return Err(format!("Microsoft access refresh returned HTTP {}. Reconnect Microsoft 365; nothing was submitted to Outlook.",response.status().as_u16()));
    }
    let token: Token = response
        .json()
        .map_err(|_| "Unreadable Microsoft access response; nothing was submitted to Outlook")?;
    check_scope(&token)?;
    if let Some(refresh) = token.refresh_token.filter(|v| !v.is_empty()) {
        credential.refresh_token = refresh;
        vault::write(credential)?;
    }
    Ok(token.access_token)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_rejects_paths_and_foreign_endpoints() {
        assert!(validate_registration(
            "00000000-0000-0000-0000-000000000001",
            "00000000-0000-0000-0000-000000000002"
        )
        .is_ok());
        assert!(validate_registration("bad", "../../evil").is_err());
    }
    #[test]
    fn mail_permissions_are_required() {
        let mut token = Token {
            access_token: "test".into(),
            refresh_token: None,
            scope: Some("User.Read Mail.Send".into()),
        };
        assert!(check_scope(&token).is_err());
        token.scope = Some("User.Read Mail.Send Mail.ReadWrite".into());
        assert!(check_scope(&token).is_ok());
    }
}

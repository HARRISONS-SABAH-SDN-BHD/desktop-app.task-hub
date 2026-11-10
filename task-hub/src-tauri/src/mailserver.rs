mod vault;
use crate::{model::Result, storage::AppState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub email: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub folder: String,
    pub draft_folder: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_security: String,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Credentials {
    pub config: Config,
    password: String,
}
#[derive(Serialize)]
pub struct Status {
    pub connected: bool,
    pub config: Option<Config>,
    pub can_send: bool,
}
pub fn status() -> Result<Status> {
    let value = vault::read()?;
    Ok(Status {
        connected: value.is_some(),
        can_send: value
            .as_ref()
            .is_some_and(|c| !c.config.smtp_host.is_empty()),
        config: value.map(|c| c.config),
    })
}
fn validate(config: &Config, password: &str) -> Result<()> {
    if !crate::validation::valid_email(&config.email)
        || !matches!(config.protocol.as_str(), "imap" | "pop3")
        || config.host.is_empty()
        || config.port == 0
        || config.smtp_port == 0
        || !matches!(config.smtp_security.as_str(), "tls" | "starttls")
        || password.is_empty()
        || password.len() > 2048
    {
        return Err("Check the email address, protocol, TLS server settings and password".into());
    }
    for host in [&config.host, &config.smtp_host] {
        if host.len() > 253
            || host
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && !".-".contains(c))
        {
            return Err("Use server hostnames, without URLs or paths".into());
        }
    }
    for field in [
        &config.username,
        &config.smtp_username,
        &config.folder,
        &config.draft_folder,
    ] {
        if field.len() > 512 || field.chars().any(char::is_control) {
            return Err("Invalid mail server account field".into());
        }
    }
    Ok(())
}
pub fn connect(config: Config, password: String) -> Result<()> {
    let _operation = crate::gmail::lock_for_update()?;
    validate(&config, &password)?;
    crate::runtime::request(
        json!({"operation":"mail_connect","config":config,"password":password}),
    )?;
    vault::write(&Credentials { config, password })
}
pub fn disconnect() -> Result<()> {
    let _operation = crate::gmail::lock_for_update()?;
    vault::remove()
}
pub(crate) fn credentials() -> Result<Credentials> {
    vault::read()?.ok_or("Connect your IMAP/POP3 account first".into())
}
pub fn request(operation: &str, fields: Value) -> Result<Value> {
    let value = credentials()?;
    let mut request = fields;
    request["operation"] = json!(operation);
    request["config"] = json!(value.config);
    request["password"] = json!(value.password);
    crate::runtime::request(request)
}
pub fn connected_sender() -> Option<String> {
    vault::read()
        .ok()
        .flatten()
        .filter(|c| !c.config.smtp_host.is_empty())
        .map(|c| c.config.email)
}
pub fn deliver(
    state: &AppState,
    credential: &Credentials,
    raw: &str,
    recipient: &str,
    cc: &str,
    send: bool,
    run_id: &str,
) -> Result<String> {
    use base64::{
        engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
        Engine,
    };
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| "Invalid encoded email")?;
    if !send && credential.config.protocol == "pop3" {
        let path = state.output.join(format!("{run_id}.eml"));
        crate::storage::atomic_write(&path, &bytes)?;
        return Ok(format!(
            "POP3 has no server-side Drafts. Local draft saved at {}. Nothing was sent.",
            path.display()
        ));
    }
    let result = crate::runtime::request(
        json!({"operation":"mail_deliver","config":credential.config,"password":credential.password,"raw":STANDARD.encode(bytes),"recipient":recipient,"cc":crate::validation::cc_addresses(cc)?,"send":send}),
    )?;
    Ok(result["detail"]
        .as_str()
        .unwrap_or("Mail server accepted the operation")
        .into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsafe_server_settings_are_rejected() {
        let config = Config {
            email: "a@example.com".into(),
            protocol: "imap".into(),
            host: "https://example.com/path".into(),
            port: 993,
            username: "user".into(),
            folder: "INBOX".into(),
            draft_folder: "Drafts".into(),
            smtp_host: String::new(),
            smtp_port: 587,
            smtp_username: String::new(),
            smtp_security: "starttls".into(),
        };
        assert!(validate(&config, "pass").is_err());
    }
}

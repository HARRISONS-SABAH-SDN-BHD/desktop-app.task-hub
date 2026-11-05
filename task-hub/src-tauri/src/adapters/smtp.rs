//! SMTP settings belong to the host environment, never to workflow JSON or logs.
use crate::model::Result;
use lettre::{transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport};
use std::{collections::BTreeMap, env, time::Duration};

fn setting(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "Set {name} in the environment before starting Task Hub. See README SMTP setup."
            )
        })
}
fn message(fields: &BTreeMap<String, String>, from: &str) -> Result<Message> {
    Message::builder()
        .from(
            from.parse()
                .map_err(|_| "Invalid TASKHUB_SMTP_FROM mailbox")?,
        )
        .to(fields["recipient"]
            .parse()
            .map_err(|_| "Invalid recipient mailbox")?)
        .subject(fields["title"].replace(['\r', '\n'], " "))
        .body(fields["body"].clone())
        .map_err(|e| format!("Could not prepare email: {e}"))
}
pub fn send(fields: &BTreeMap<String, String>) -> Result<String> {
    let host = setting("TASKHUB_SMTP_HOST")?;
    let from = setting("TASKHUB_SMTP_FROM")?;
    let username = setting("TASKHUB_SMTP_USERNAME")?;
    let password = setting("TASKHUB_SMTP_PASSWORD")?;
    let port = env::var("TASKHUB_SMTP_PORT")
        .unwrap_or_else(|_| "587".into())
        .parse::<u16>()
        .map_err(|_| "Invalid SMTP port")?;
    let mail = message(fields, &from)?;
    let sender = SmtpTransport::starttls_relay(&host)
        .map_err(|_| "Invalid SMTP host or TLS configuration")?
        .port(port)
        .credentials(Credentials::new(username, password))
        .timeout(Some(Duration::from_secs(30)))
        .build();
    sender.send(&mail).map_err(|_| "SMTP delivery failed. Check server settings, network access and credentials. The outcome may be uncertain; inspect your provider before retrying.")?;
    Ok(format!(
        "SMTP server accepted email\nTo: {}\nSubject: {}\nAccepted at: {}\n",
        fields["recipient"],
        fields["title"].replace(['\r', '\n'], " "),
        chrono::Utc::now().to_rfc3339()
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_unicode_email_without_sending() {
        let fields = BTreeMap::from([
            ("recipient".into(), "a@example.com".into()),
            ("title".into(), "Update 日本語".into()),
            ("body".into(), "Hello 👋".into()),
        ]);
        let mail = message(&fields, "sender@example.com").unwrap();
        assert!(String::from_utf8_lossy(&mail.formatted()).contains("To: a@example.com"));
    }
}

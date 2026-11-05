use super::{transport, vault, Credentials, CANCEL_CONNECT, OPERATION};
use crate::{
    model::Result,
    storage::{atomic_write, AppState},
    validation,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use url::Url;

pub(super) const COMPOSE_SCOPE: &str = "https://www.googleapis.com/auth/gmail.compose";
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    scope: Option<String>,
}
pub(super) fn validate_client_id(id: &str) -> Result<()> {
    if id.len() < 30
        || id.len() > 250
        || !id.ends_with(".apps.googleusercontent.com")
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-._".contains(c))
    {
        return Err("Paste the OAuth client ID for a Google Desktop app, ending in .apps.googleusercontent.com".into());
    }
    Ok(())
}
fn random_secret() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn request_head(mut reader: impl Read) -> Result<String> {
    let mut bytes = Vec::new();
    let mut part = [0u8; 1024];
    while bytes.len() < 8192 {
        let count = reader
            .read(&mut part)
            .map_err(|_| "Could not read the local Google callback")?;
        if count == 0 {
            return Err("Incomplete Google callback".into());
        }
        bytes.extend_from_slice(&part[..count]);
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            return String::from_utf8(bytes[..end + 4].to_vec())
                .map_err(|_| "Invalid Google callback request".into());
        }
    }
    Err("Google callback request was too large".into())
}
fn callback_code(url: &Url, state: &str) -> Result<String> {
    let fields: Vec<_> = url.query_pairs().collect();
    let values = |key: &str| {
        fields
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.to_string())
            .collect::<Vec<_>>()
    };
    if values("state") != vec![state.to_string()] {
        return Err("Google sign-in state did not match; connection rejected".into());
    }
    if !values("error").is_empty() {
        return Err("Google sign-in was declined or cancelled".into());
    }
    let codes = values("code");
    if codes.len() != 1 || codes[0].is_empty() || codes[0].len() > 4096 {
        return Err("Google did not return a valid authorization code".into());
    }
    Ok(codes[0].clone())
}
pub(super) fn connect(
    state: &AppState,
    id: String,
    secret: Option<String>,
    calendar_access: bool,
    open: impl FnOnce(String) -> Result<()>,
) -> Result<()> {
    let _operation = OPERATION
        .try_lock()
        .map_err(|_| "Another Gmail operation is in progress")?;
    CANCEL_CONNECT.store(false, Ordering::SeqCst);
    let id = id.trim().to_string();
    validate_client_id(&id)?;
    let secret = secret
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if secret.as_ref().is_some_and(|s| s.len() > 1024) {
        return Err("OAuth client secret is too long".into());
    }
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|_| "Could not open the local Google sign-in callback")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "Could not initialize Google callback")?;
    let address = listener
        .local_addr()
        .map_err(|_| "Could not read callback address")?;
    let redirect = format!("http://127.0.0.1:{}/", address.port());
    let csrf = random_secret();
    let verifier = random_secret();
    let mut auth = Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    auth.query_pairs_mut().extend_pairs([
        ("client_id", id.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("response_type", "code"),
        (
            "scope",
            &format!(
                "openid email {COMPOSE_SCOPE}{}",
                if calendar_access {
                    format!(" {}", super::calendar::SCOPE)
                } else {
                    String::new()
                }
            ),
        ),
        ("state", csrf.as_str()),
        ("code_challenge", &challenge(&verifier)),
        ("code_challenge_method", "S256"),
        ("access_type", "offline"),
        ("prompt", "consent select_account"),
    ]);
    open(auth.to_string())?;
    let started = Instant::now();
    let code = loop {
        if CANCEL_CONNECT.load(Ordering::SeqCst) {
            return Err("Google connection cancelled".into());
        }
        if started.elapsed() > Duration::from_secs(180) {
            return Err("Google sign-in timed out. Click Connect to try again.".into());
        }
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
                let request = match request_head(&mut stream) {
                    Ok(request) => request,
                    Err(_) => continue,
                };
                let mut parts = request
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .split_whitespace();
                let method = parts.next();
                let path = parts.next().unwrap_or_default();
                let url = Url::parse(&format!("{redirect}{}", path.trim_start_matches('/')));
                let matching = method == Some("GET")
                    && path.starts_with("/?")
                    && url.as_ref().is_ok_and(|u| {
                        u.query_pairs()
                            .filter(|(k, _)| k == "state")
                            .map(|(_, v)| v.into_owned())
                            .collect::<Vec<_>>()
                            == vec![csrf.clone()]
                    });
                if !matching {
                    let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                }
                let result = callback_code(&url.unwrap(), &csrf);
                let text = if result.is_ok() {
                    "Google authorization received. Return to Task Hub to check the connection. You may close this tab."
                } else {
                    "Google connection was declined. Return to Task Hub."
                };
                let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
                let _ = stream.write_all(reply.as_bytes());
                break result?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100))
            }
            Err(_) => return Err("Local Google callback failed. Try connecting again.".into()),
        }
    };
    let mut fields = vec![
        ("client_id", id.as_str()),
        ("code", code.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("code_verifier", verifier.as_str()),
        ("grant_type", "authorization_code"),
    ];
    if let Some(secret) = &secret {
        fields.push(("client_secret", secret));
    }
    let client = transport::client()?;
    let response = client.post("https://oauth2.googleapis.com/token").form(&fields).send().map_err(|_| "Could not exchange Google authorization. Check internet access and try connecting again.")?;
    if !response.status().is_success() {
        return Err(format!("Google token exchange returned HTTP {}. Check the desktop client ID/secret and try connecting again.", response.status().as_u16()));
    }
    let token: TokenResponse = response
        .json()
        .map_err(|_| "Google returned an unreadable token response")?;
    if !token
        .scope
        .as_deref()
        .is_some_and(|s| s.split_whitespace().any(|s| s == COMPOSE_SCOPE))
    {
        return Err("Gmail draft/send access was not granted. Reconnect and grant the requested Gmail access.".into());
    }
    let refresh = token.refresh_token.filter(|v| !v.is_empty()).ok_or(
        "Google did not issue offline access. Reconnect and approve offline Gmail access.",
    )?;
    #[derive(Deserialize)]
    struct User {
        email: String,
        email_verified: bool,
        sub: String,
    }
    let response = client
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(&token.access_token)
        .send()
        .map_err(|_| "Could not verify the connected Google account")?;
    if !response.status().is_success() {
        return Err("Could not verify the connected Google account. Reconnect Gmail.".into());
    }
    let user: User = response
        .json()
        .map_err(|_| "Google did not return a readable account identity")?;
    if !user.email_verified || !validation::valid_email(&user.email) || user.sub.is_empty() {
        return Err("Google did not return a verified sender email".into());
    }
    if CANCEL_CONNECT.load(Ordering::SeqCst) {
        return Err("Google connection cancelled".into());
    }
    vault::write(&Credentials {
        client_id: id.clone(),
        client_secret: secret,
        refresh_token: refresh,
        email: user.email.to_lowercase(),
        subject: user.sub,
        calendar_access: token.scope.as_deref().is_some_and(|scopes| {
            scopes
                .split_whitespace()
                .any(|s| s == super::calendar::SCOPE)
        }),
    })?;
    atomic_write(
        &state.root.join("gmail-client.json"),
        &serde_json::to_vec(&serde_json::json!({"client_id": id}))
            .map_err(|_| "Could not encode Gmail client settings")?,
    )?;
    Ok(())
}
pub(super) fn access_token(
    client: &reqwest::blocking::Client,
    credential: &mut Credentials,
) -> Result<String> {
    let mut fields = vec![
        ("grant_type", "refresh_token"),
        ("client_id", credential.client_id.as_str()),
        ("refresh_token", credential.refresh_token.as_str()),
    ];
    if let Some(secret) = &credential.client_secret {
        fields.push(("client_secret", secret));
    }
    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&fields)
        .send()
        .map_err(|_| {
            "Could not refresh Gmail access. Check internet access; nothing was submitted to Gmail."
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "Google access refresh returned HTTP {}. Reconnect Gmail; no email request was made.",
            response.status().as_u16()
        ));
    }
    let token: TokenResponse = response
        .json()
        .map_err(|_| "Could not read refreshed Gmail access; no email request was made")?;
    if token.access_token.is_empty() {
        return Err("Google did not return Gmail access; reconnect the account".into());
    }
    if let Some(refresh) = token.refresh_token {
        credential.refresh_token = refresh;
        vault::write(credential)?;
    }
    Ok(token.access_token)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loopback_callback_handles_fragmented_tcp_headers_and_limits_input() {
        struct OneByte<'a>(&'a [u8]);
        impl Read for OneByte<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                out[0] = self.0[0];
                self.0 = &self.0[1..];
                Ok(1)
            }
        }
        let request = b"GET /?state=s&code=c HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(request_head(OneByte(request)).unwrap().as_bytes(), request);
        assert!(request_head(&b"GET /"[..]).is_err());
        assert!(request_head(&vec![b'x'; 9000][..]).is_err());
    }
    #[test]
    fn pkce_matches_rfc_vector_and_callback_rejects_wrong_or_duplicate_state() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(
            callback_code(
                &Url::parse("http://127.0.0.1/?state=correct&code=abc").unwrap(),
                "correct"
            )
            .unwrap(),
            "abc"
        );
        for query in [
            "state=wrong&code=abc",
            "state=correct&state=wrong&code=abc",
            "state=correct&error=access_denied",
            "state=correct&code=a&code=b",
        ] {
            assert!(callback_code(
                &Url::parse(&format!("http://127.0.0.1/?{query}")).unwrap(),
                "correct"
            )
            .is_err());
        }
    }
}

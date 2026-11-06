use crate::model::Result;
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
pub(super) fn validate_size(raw: &str) -> Result<()> {
    // Microsoft Graph's single MIME request is limited to 4 MiB. Leave room for
    // server-side handling; do not silently drop or split attachments.
    if raw.len() > 3_900_000 {
        return Err("Microsoft 365 email is too large for this version's 4 MB API request limit. Reduce attachments to about 2 MB total. Nothing was submitted to Outlook.".into());
    }
    Ok(())
}
pub(super) fn deliver(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    raw: &str,
    send: bool,
) -> Result<String> {
    validate_size(raw)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| "Could not encode Microsoft email")?;
    let payload = STANDARD.encode(bytes);
    let resource = if send { "sendMail" } else { "messages" };
    // Exactly one mutation; reqwest's client also explicitly disables retries.
    let response=client.post(format!("{base}/me/{resource}")).bearer_auth(token).header("Content-Type","text/plain").body(payload).send().map_err(|_|"Microsoft email request did not finish. Outcome may be uncertain. Inspect Outlook Drafts and Sent Items before retrying; no retry was attempted.")?;
    let code = response.status().as_u16();
    if send && code == 202 {
        return Ok("accepted (Microsoft returns no message ID)".into());
    }
    if !send && code == 201 {
        let value:serde_json::Value=response.json().map_err(|_|"Microsoft accepted the draft but its response was unreadable. Inspect Drafts before retrying.")?;
        return value
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|id| !id.is_empty() && id.len() <= 1024 && !id.chars().any(char::is_control))
            .map(str::to_owned)
            .ok_or(
                "Microsoft accepted the draft without a usable ID. Inspect Drafts before retrying."
                    .into(),
            );
    }
    Err(format!("Microsoft Graph returned HTTP {code}. Check Microsoft connection, IT consent and attachment size. No retry was attempted; inspect Outlook before retrying."))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    #[test]
    fn mime_uses_standard_base64_and_send_accepts_empty_202_without_retry() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            loop {
                let mut buf = [0; 4096];
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request);
                if text.contains("\r\n\r\n") && text.ends_with("SGVsbG8/") {
                    break;
                }
            }
            let text = String::from_utf8(request).unwrap();
            assert!(text.starts_with("POST /me/sendMail "));
            assert!(text.to_lowercase().contains("content-type: text/plain"));
            stream
                .write_all(
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });
        let client = crate::gmail::transport::client().unwrap();
        assert!(deliver(
            &client,
            &format!("http://{addr}"),
            "test",
            &URL_SAFE_NO_PAD.encode(b"Hello?"),
            true
        )
        .is_ok());
        handle.join().unwrap();
    }
    #[test]
    fn oversized_message_stops_before_http() {
        assert!(validate_size(&"x".repeat(3_900_001)).is_err());
    }

    #[test]
    fn draft_is_separate_and_errors_never_retry_the_post() {
        for (send, response, expected) in [(false, "HTTP/1.1 201 Created\r\nContent-Length: 14\r\nConnection: close\r\n\r\n{\"id\":\"draft\"}", true), (true, "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", false)] {
            let listener=TcpListener::bind("127.0.0.1:0").unwrap();let addr=listener.local_addr().unwrap();
            let handle=thread::spawn(move||{
                let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
                let mut request=Vec::new();loop {let mut buf=[0;4096];let count=stream.read(&mut buf).unwrap();assert!(count>0);request.extend_from_slice(&buf[..count]);if request.ends_with(b"SGVsbG8="){break;}}
                let text=String::from_utf8(request).unwrap();assert!(text.starts_with(if send {"POST /me/sendMail "} else {"POST /me/messages "}));
                stream.write_all(response.as_bytes()).unwrap();
                listener.set_nonblocking(true).unwrap();std::thread::sleep(std::time::Duration::from_millis(100));assert!(listener.accept().is_err());
            });
            let result=deliver(&crate::gmail::transport::client().unwrap(),&format!("http://{addr}"),"test",&URL_SAFE_NO_PAD.encode(b"Hello"),send);assert_eq!(result.is_ok(),expected);handle.join().unwrap();
        }
    }
}

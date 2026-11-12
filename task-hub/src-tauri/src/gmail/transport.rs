use crate::{model::Result, validation};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use lettre::{
    message::{Attachment, MultiPart, SinglePart},
    Message,
};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) fn client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .map_err(|_| "Could not initialize secure Gmail HTTPS transport".into())
}
pub(super) fn mime(
    from: &str,
    to: &str,
    cc: &str,
    bcc: &str,
    subject: &str,
    body: &str,
    id: &str,
    attachments: &[crate::attachments::AttachmentData],
) -> Result<String> {
    if !validation::valid_email(from)
        || validation::to_addresses(to).is_err()
        || subject.contains(['\r', '\n'])
        || body.trim().is_empty()
    {
        return Err("Invalid sender, recipient, subject or message".into());
    }
    let mut builder = Message::builder()
        .from(from.parse().map_err(|_| "Invalid sender")?)
        .subject(subject)
        .message_id(Some(format!("<taskhub.{id}@taskhub.local>")));
    for address in validation::to_addresses(to)? {
        builder = builder.to(address.parse().map_err(|_| "Invalid recipient")?);
    }
    for address in validation::cc_addresses(cc)? {
        builder = builder.cc(address.parse().map_err(|_| "Invalid CC email address")?);
    }
    // Gmail API derives its delivery envelope from the raw message headers.
    // SMTP removes Bcc from the wire copy separately while retaining its envelope.
    for address in validation::bcc_addresses(bcc)? {
        builder = builder.bcc(address.parse().map_err(|_| "Invalid BCC email address")?);
    }
    builder = builder.keep_bcc();
    let body = body.replace("\r\n", "\n").replace('\r', "\n");
    let message = if attachments.is_empty() {
        builder.body(body)
    } else {
        let mut parts = MultiPart::mixed().singlepart(SinglePart::plain(body));
        for attachment in attachments {
            parts = parts.singlepart(Attachment::new(attachment.name.clone()).body(
                attachment.bytes.clone(),
                "application/octet-stream".parse().unwrap(),
            ));
        }
        builder.multipart(parts)
    }
    .map_err(|_| "Could not encode the email message")?;
    let bytes = message.formatted();
    if bytes.len() > 25 * 1024 * 1024 {
        return Err("Encoded email is too large. Reduce attachment sizes before sending.".into());
    }
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
pub(super) fn deliver(
    client: &Client,
    base: &str,
    token: &str,
    raw: &str,
    send: bool,
) -> Result<String> {
    let resource = if send { "messages/send" } else { "drafts" };
    let payload = if send {
        json!({"raw": raw})
    } else {
        json!({"message": {"raw": raw}})
    };
    // One mutation request only: never retry a POST, including on 401 or 5xx.
    let response = client.post(format!("{base}/gmail/v1/users/me/{resource}"))
        .bearer_auth(token).json(&payload).send()
        .map_err(|_| "Gmail request did not finish. Its outcome may be uncertain. Inspect Gmail before retrying; no retry was attempted.".to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Gmail API returned HTTP {}. Check the Gmail connection and API setup. No retry was attempted; inspect Gmail before retrying.", status.as_u16()));
    }
    let value: Value = response.json().map_err(|_| "Gmail accepted the request but returned an unreadable response. Inspect Gmail before retrying.".to_string())?;
    value.get("id").and_then(Value::as_str).filter(|id| !id.is_empty() && id.len() <= 256 && id.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c)))
        .map(str::to_owned).ok_or_else(|| "Gmail response did not contain a valid message/draft ID. Inspect Gmail before retrying.".into())
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
    fn bcc_is_retained_for_gmail_envelope_and_rejects_header_injection() {
        let raw = mime(
            "sender@example.com",
            "to@example.com",
            "cc@example.com",
            "hidden@example.com, private@example.com",
            "Report",
            "Body",
            "bcc-test",
            &[],
        )
        .unwrap();
        let mail = String::from_utf8(URL_SAFE_NO_PAD.decode(raw).unwrap()).unwrap();
        assert!(mail.contains("Bcc: hidden@example.com, private@example.com"));
        assert!(mail.contains("To: to@example.com"));
        assert!(mail.contains("Cc: cc@example.com"));
        for bcc in [
            "bad",
            "hidden@example.com,",
            "hidden@example.com\r\nCc: injected@example.com",
        ] {
            assert!(mime(
                "sender@example.com",
                "to@example.com",
                "",
                bcc,
                "Report",
                "Body",
                "bcc-test",
                &[]
            )
            .unwrap_err()
            .contains("BCC"));
        }
    }
    #[test]
    fn multiple_to_recipients_have_separate_mailboxes_and_reject_empty_or_duplicate_selection() {
        let raw = mime(
            "sender@example.com",
            "one@example.com, two@example.com",
            "cc@example.com",
            "",
            "Report",
            "Body",
            "123",
            &[],
        )
        .unwrap();
        let mail = String::from_utf8(URL_SAFE_NO_PAD.decode(raw).unwrap()).unwrap();
        assert!(mail.contains("To: one@example.com, two@example.com"));
        assert!(mail.contains("Cc: cc@example.com"));
        for to in [
            "",
            "one@example.com, ONE@example.com",
            "one@example.com,",
            "one@example.com\r\nBcc: other@example.com",
        ] {
            assert!(mime(
                "sender@example.com",
                to,
                "",
                "",
                "Report",
                "Body",
                "123",
                &[]
            )
            .is_err());
        }
        let task = crate::email_profile::default_config()
            .unwrap()
            .tasks
            .remove(0);
        let values=serde_json::from_value(json!({"recipient":"one@example.com, two@example.com","subject":"Report","message":"Body","delivery":"Draft"})).unwrap();
        validation::parameters(&task, &values).unwrap();
    }
    #[test]
    fn cc_is_encoded_and_optional_and_rejects_injection() {
        let raw = mime(
            "sender@example.com",
            "to@example.com",
            "one@example.com, two@example.com",
            "",
            "Report",
            "Body",
            "123",
            &[],
        )
        .unwrap();
        let mail = String::from_utf8(URL_SAFE_NO_PAD.decode(raw).unwrap()).unwrap();
        assert!(mail.contains("Cc: one@example.com, two@example.com"));
        assert!(mail.contains("To: to@example.com"));
        let raw = mime(
            "sender@example.com",
            "to@example.com",
            "",
            "",
            "Report",
            "Body",
            "123",
            &[],
        )
        .unwrap();
        let mail = String::from_utf8(URL_SAFE_NO_PAD.decode(raw).unwrap()).unwrap();
        assert!(!mail.contains("Cc:"));
        for cc in [
            "invalid",
            "one@example.com,",
            "one@example.com\r\nBcc: hidden@example.com",
        ] {
            assert!(mime(
                "sender@example.com",
                "to@example.com",
                cc,
                "",
                "Report",
                "Body",
                "123",
                &[]
            )
            .is_err());
        }
    }
    #[test]
    fn binary_attachments_are_encoded_as_mime_parts_with_original_filenames() {
        let data: Vec<u8> = (0..=255).collect();
        let items = vec![
            crate::attachments::AttachmentData {
                name: "report.bin".into(),
                bytes: data.clone(),
            },
            crate::attachments::AttachmentData {
                name: "notes.txt".into(),
                bytes: b"Notes".to_vec(),
            },
        ];
        let raw = mime(
            "sender@example.com",
            "to@example.com",
            "",
            "",
            "Report",
            "Hello\n\nAttached",
            "123",
            &items,
        )
        .unwrap();
        let mail = String::from_utf8(URL_SAFE_NO_PAD.decode(raw).unwrap()).unwrap();
        assert!(mail.contains("multipart/mixed"));
        assert!(mail.contains("filename=\"report.bin\""));
        assert!(mail.contains("filename=\"notes.txt\""));
        assert!(mail
            .replace("\r\n", "")
            .contains(&base64::engine::general_purpose::STANDARD.encode(data)));
        assert!(mail.contains("Hello\r\n\r\nAttached"));
    }
    #[test]
    fn mime_keeps_exact_recipient_and_multiline_content_without_header_injection() {
        let raw = mime(
            "sender@example.com",
            "new@example.com",
            "",
            "",
            "Report",
            "Hello\n\nReport body",
            "123",
            &[],
        )
        .unwrap();
        let bytes = URL_SAFE_NO_PAD.decode(raw).unwrap();
        let mail = String::from_utf8(bytes).unwrap();
        assert!(mail.contains("To: new@example.com"));
        assert!(mail.contains("Hello\r\n\r\nReport body"));
        assert!(mime(
            "sender@example.com",
            "new@example.com",
            "",
            "",
            "Hi\r\nBcc: other@example.com",
            "Body",
            "123",
            &[]
        )
        .is_err());
    }
    #[test]
    fn api_sends_one_post_and_draft_payload_never_uses_send_endpoint() {
        for send in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut data = Vec::new();
                let mut buf = [0; 4096];
                loop {
                    let n = stream.read(&mut buf).unwrap();
                    data.extend_from_slice(&buf[..n]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&data[..end]);
                        let len: usize = headers
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|s| s.trim().parse().unwrap())
                            })
                            .unwrap();
                        if data.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let request = String::from_utf8(data).unwrap();
                assert!(request.starts_with(if send {
                    "POST /gmail/v1/users/me/messages/send "
                } else {
                    "POST /gmail/v1/users/me/drafts "
                }));
                let body = request.split_once("\r\n\r\n").unwrap().1;
                let payload: Value = serde_json::from_str(body).unwrap();
                assert_eq!(
                    if send {
                        &payload["raw"]
                    } else {
                        &payload["message"]["raw"]
                    },
                    "encoded-mime"
                );
                let response = if send {
                    "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\n{\"id\":\"abc1\"}"
                };
                stream.write_all(response.as_bytes()).unwrap();
                listener.set_nonblocking(true).unwrap();
                thread::sleep(Duration::from_millis(100));
                assert!(listener.accept().is_err(), "mutation must not be retried");
            });
            let result = deliver(
                &client().unwrap(),
                &base,
                "test-token",
                "encoded-mime",
                send,
            );
            if send {
                assert!(result.unwrap_err().contains("503"));
            } else {
                assert_eq!(result.unwrap(), "abc1");
            }
            server.join().unwrap();
        }
    }
}

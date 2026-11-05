pub mod documents;
pub mod script;
pub mod smtp;
use crate::{model::*, validation};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Duration, Utc};
use std::{collections::BTreeMap, path::Path};

pub fn execute(
    task: &Task,
    parameters: &Parameters,
    output: &Path,
    id: &str,
) -> Result<(String, Vec<String>)> {
    validation::validate_task(task)?;
    validation::parameters(task, parameters)?;
    let (extension, bytes, logs) = match &task.action {
        Action::Foreground { .. } => {
            return Err("Foreground actions require the foreground runner".into())
        }
        Action::Smtp { .. } => {
            let fields = validation::resolve_fields(task, parameters)?;
            let receipt = smtp::send(&fields)?;
            ("txt".to_string(), receipt.into_bytes(), vec!["SMTP server accepted the message. Final delivery depends on the recipient's provider.".into()])
        }
        Action::Script { script_id } => {
            let (content, logs) = script::run(script_id, parameters)?;
            ("txt".to_string(), content.into_bytes(), logs)
        }
        Action::Export { format, .. } => {
            let fields = validation::resolve_fields(task, parameters)?;
            let bytes = match format.as_str() {
                "docx" | "xlsx" | "pptx" => documents::create(format, &fields)?,
                "eml" => email(&fields).into_bytes(),
                "ics" => calendar(&fields, id)?.into_bytes(),
                "html" => format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>{}</title><style>body{{max-width:800px;margin:60px auto;font:17px/1.8 system-ui;padding:24px;color:#213547}}pre{{white-space:pre-wrap;font:inherit}}</style><h1>{}</h1><pre>{}</pre></html>", documents::xml(&fields["title"]), documents::xml(&fields["title"]), documents::xml(&fields["body"])).into_bytes(),
                "csv" => { let rows = documents::csv_rows(&fields["body"])?; let mut writer = csv::Writer::from_writer(Vec::new()); for row in rows { writer.write_record(row.iter().map(|v| if matches!(v.trim_start().chars().next(), Some('=' | '+' | '-' | '@')) { format!("'{v}") } else { v.clone() })).map_err(|e| e.to_string())?; } writer.into_inner().map_err(|e| e.to_string())? },
                "txt" => format!("{}\n\n{}\n", fields["title"], fields["body"]).into_bytes(),
                _ => return Err("Unsupported output format".into()),
            };
            (
                format.clone(),
                bytes,
                vec![format!(
                    "Created {} artifact from validated answers.",
                    format.to_uppercase()
                )],
            )
        }
    };
    let path = output.join(format!("{id}.{extension}"));
    crate::storage::atomic_write(&path, &bytes)?;
    Ok((path.to_string_lossy().into_owned(), logs))
}
fn email(fields: &BTreeMap<String, String>) -> String {
    // Base64 headers and body prevent CRLF injection and preserve Unicode.
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    for ch in fields["title"].replace(['\r', '\n'], " ").chars() {
        if chunk.len() + ch.len_utf8() > 36 {
            chunks.push(STANDARD.encode(&chunk));
            chunk.clear();
        }
        chunk.push(ch);
    }
    if !chunk.is_empty() {
        chunks.push(STANDARD.encode(chunk));
    }
    let subject = chunks
        .iter()
        .map(|part| format!("=?UTF-8?B?{part}?="))
        .collect::<Vec<_>>()
        .join("\r\n ");
    let body = STANDARD.encode(fields["body"].replace("\r\n", "\n").replace('\n', "\r\n"));
    let body = body
        .as_bytes()
        .chunks(76)
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect::<Vec<_>>()
        .join("\r\n");
    format!("To: {}\r\nSubject: {subject}\r\nDate: {}\r\nX-Unsent: 1\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{body}\r\n", fields["recipient"], Utc::now().to_rfc2822())
}
fn ics_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,")
}
fn fold(value: &str) -> String {
    let mut output = String::new();
    let mut column = 0;
    for ch in value.chars() {
        if column + ch.len_utf8() > 75 {
            output.push_str("\r\n ");
            column = 1;
        }
        output.push(ch);
        column += ch.len_utf8();
    }
    output
}
fn calendar(fields: &BTreeMap<String, String>, id: &str) -> Result<String> {
    let start = DateTime::parse_from_rfc3339(&fields["start"])
        .map_err(|e| e.to_string())?
        .with_timezone(&Utc);
    let end = match fields.get("end").filter(|s| !s.is_empty()) {
        Some(value) => DateTime::parse_from_rfc3339(value)
            .map_err(|e| e.to_string())?
            .with_timezone(&Utc),
        None => start + Duration::minutes(30),
    };
    let mut lines = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//Task Hub//Workflow Engine//EN".into(),
        "CALSCALE:GREGORIAN".into(),
        "BEGIN:VEVENT".into(),
        format!("UID:{id}@taskhub.local"),
        format!("DTSTAMP:{}", Utc::now().format("%Y%m%dT%H%M%SZ")),
        format!("DTSTART:{}", start.format("%Y%m%dT%H%M%SZ")),
        format!("DTEND:{}", end.format("%Y%m%dT%H%M%SZ")),
        format!("SUMMARY:{}", ics_escape(&fields["title"])),
    ];
    for (field, property) in [("body", "DESCRIPTION"), ("location", "LOCATION")] {
        if let Some(value) = fields.get(field) {
            lines.push(format!("{property}:{}", ics_escape(value)));
        }
    }
    if let Some(minutes) = fields.get("minutes").filter(|s| !s.is_empty()) {
        lines.extend([
            "BEGIN:VALARM".into(),
            format!("TRIGGER:-PT{minutes}M"),
            "ACTION:DISPLAY".into(),
            format!("DESCRIPTION:{}", ics_escape(&fields["title"])),
            "END:VALARM".into(),
        ]);
    }
    lines.extend(["END:VEVENT".into(), "END:VCALENDAR".into()]);
    Ok(lines
        .iter()
        .map(|l| fold(l))
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_escapes_and_folds_utf8() {
        let fields = BTreeMap::from([
            ("title".into(), "Meeting, one;two\nnext".into()),
            ("start".into(), "2026-11-01T09:00:00+08:00".into()),
            ("minutes".into(), "15".into()),
        ]);
        let text = calendar(&fields, "test").unwrap();
        assert!(text.contains("DTSTART:20261101T010000Z"));
        assert!(text.contains("SUMMARY:Meeting\\, one\\;two\\nnext"));
        assert!(text.contains("TRIGGER:-PT15M"));
        assert!(fold(&"界".repeat(80)).split("\r\n").all(|l| l.len() <= 75));
    }
    #[test]
    fn email_has_no_injected_header() {
        let fields = BTreeMap::from([
            ("title".into(), "Hi\r\nBcc: victim@example.com".into()),
            ("recipient".into(), "a@example.com".into()),
            ("body".into(), "Hello 👋".into()),
        ]);
        let result = email(&fields);
        assert!(!result.contains("\r\nBcc:"));
        assert!(result.contains("X-Unsent: 1"));
    }
}

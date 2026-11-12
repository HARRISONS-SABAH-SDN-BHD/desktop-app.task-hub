//! Read-only mailbox sources; native staging keeps filesystem paths out of UI requests.
use crate::{
    attachment_sources, excelflow,
    model::Result,
    storage::{atomic_write, AppState},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
const MAX_INPUT: u64 = 100 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
pub struct Input {
    pub sha256: String,
    pub id: String,
    pub name: String,
    pub size: u64,
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 512
        && !name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
        && (name.to_lowercase().ends_with(".xlsx") || name.to_lowercase().ends_with(".xlsm"))
}
fn path_for(state: &AppState, id: &str) -> Result<PathBuf> {
    if uuid::Uuid::parse_str(id)
        .map(|u| u.to_string())
        .ok()
        .as_deref()
        != Some(id)
    {
        return Err("Invalid workbook selection".into());
    }
    let dir = state.root.join("excelflow-inputs").join(id);
    let record: Input = serde_json::from_slice(
        &std::fs::read(dir.join("metadata.json"))
            .map_err(|_| "Workbook selection expired. Choose it again.")?,
    )
    .map_err(|_| "Invalid workbook record")?;
    if record.id != id || !valid_name(&record.name) {
        return Err("Invalid workbook metadata".into());
    }
    let path = std::fs::canonicalize(dir.join(&record.name))
        .map_err(|_| "Selected workbook is missing")?;
    if path.parent()
        != Some(
            std::fs::canonicalize(&dir)
                .map_err(|_| "Workbook folder unavailable")?
                .as_path(),
        )
    {
        return Err("Workbook points outside its storage".into());
    }
    use sha2::{Digest, Sha256};
    let bytes = bounded(&path, MAX_INPUT)?;
    if bytes.len() as u64 != record.size || format!("{:x}", Sha256::digest(&bytes)) != record.sha256
    {
        return Err("The selected workbook copy changed. Choose the input again.".into());
    }
    Ok(path)
}
fn stage_bytes(state: &AppState, name: &str, bytes: &[u8]) -> Result<Input> {
    if !valid_name(name) || bytes.len() as u64 > MAX_INPUT || bytes.is_empty() {
        return Err("Choose a nonempty .xlsx/.xlsm workbook up to 100 MB".into());
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "The attachment is not an Excel workbook")?;
    if zip.len() > 50000 {
        return Err("Workbook archive contains too many entries".into());
    }
    let mut expanded = 0u64;
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|_| "Invalid workbook archive")?;
        expanded = expanded
            .checked_add(entry.size())
            .ok_or("Workbook archive size overflow")?;
        if entry.size() > 512 * 1024 * 1024 || expanded > 1024 * 1024 * 1024 {
            return Err("Workbook archive expands beyond the safe processing limit".into());
        }
    }
    if zip.by_name("xl/workbook.xml").is_err() {
        return Err("The selected attachment has no Excel workbook data".into());
    }
    use sha2::{Digest, Sha256};
    let item = Input {
        sha256: format!("{:x}", Sha256::digest(bytes)),
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        size: bytes.len() as u64,
    };
    let dir = state.root.join("excelflow-inputs").join(&item.id);
    atomic_write(&dir.join(name), bytes)?;
    atomic_write(
        &dir.join("metadata.json"),
        &serde_json::to_vec(&item).map_err(|_| "Could not save input metadata")?,
    )?;
    Ok(item)
}
#[cfg(windows)]
pub fn pick(state: &AppState, owner: isize) -> Result<Option<Input>> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let mut buffer = vec![0u16; 32768];
    let title: Vec<u16> = "Choose an Excel workbook\0".encode_utf16().collect();
    let filter: Vec<u16> = "Excel workbooks\0*.xlsx;*.xlsm\0\0"
        .encode_utf16()
        .collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner as _,
        lpstrFile: buffer.as_mut_ptr(),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: title.as_ptr(),
        lpstrFilter: filter.as_ptr(),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        return if unsafe { CommDlgExtendedError() } == 0 {
            Ok(None)
        } else {
            Err("Could not open the workbook picker".into())
        };
    }
    let end = buffer
        .iter()
        .position(|v| *v == 0)
        .ok_or("Invalid workbook path")?;
    let path =
        PathBuf::from(String::from_utf16(&buffer[..end]).map_err(|_| "Unsupported workbook path")?);
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("Invalid workbook name")?;
    let bytes = bounded(&path, MAX_INPUT)?;
    Ok(Some(stage_bytes(state, name, &bytes)?))
}
#[cfg(not(windows))]
pub fn pick(_: &AppState, _: isize) -> Result<Option<Input>> {
    Err("Workbook selection currently requires Windows".into())
}
fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(|_| "Could not open selected workbook")?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read workbook")?;
    if bytes.len() as u64 > limit {
        return Err("Workbook exceeds 100 MB".into());
    }
    Ok(bytes)
}
pub(crate) fn json_response(response: reqwest::blocking::Response, limit: u64) -> Result<Value> {
    if !response.status().is_success() {
        return Err(format!(
            "Mail source returned HTTP {}. Check the connection and permissions.",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read mail source")?;
    if bytes.len() as u64 > limit {
        return Err("Mail response exceeded its limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Mail source returned an unreadable response".into())
}
pub(crate) fn endpoint(base: &str, segments: &[&str]) -> Result<url::Url> {
    let mut url = url::Url::parse(base).map_err(|_| "Invalid mail API URL")?;
    {
        let mut path = url.path_segments_mut().map_err(|_| "Invalid API path")?;
        path.pop_if_empty();
        for segment in segments {
            if segment.len() > 4096 || segment.chars().any(char::is_control) {
                return Err("Invalid message identifier".into());
            }
            path.push(segment);
        }
    }
    Ok(url)
}
pub fn browse(_state: &AppState, provider: &str, mut request: Value) -> Result<Value> {
    supported_source(provider)?;
    let _operation = crate::gmail::lock_for_update()?;
    let operation = request["operation"]
        .as_str()
        .ok_or("Missing mail operation")?
        .to_owned();
    if !matches!(operation.as_str(), "folders" | "messages" | "attachments") {
        return Err("Unsupported mailbox operation".into());
    }
    match provider {
        "mailserver" => {
            if operation == "folders" {
                let value = crate::mailserver::credentials()?;
                return Ok(
                    json!([{"id":value.config.folder,"store_id":"","name":if value.config.protocol=="imap"{value.config.folder.clone()}else{"POP3 Inbox".into()}}]),
                );
            }
            request["operation"] = json!(format!("mail_{operation}"));
            crate::mailserver::request(&format!("mail_{operation}"), request)
        }
        "gmail" => cloud(&operation, &request),
        _ => Err("Choose a connected email source".into()),
    }
}
fn cloud(operation: &str, request: &Value) -> Result<Value> {
    let (client, token) = crate::gmail::source_token()?;
    if operation == "folders" {
        return Ok(json!([{"id":"INBOX","store_id":"","name":"Inbox"}]));
    }
    let id = request["message_id"].as_str().unwrap_or_default();
    if operation == "messages" {
        let mut url = endpoint(
            "https://gmail.googleapis.com/gmail/v1/users/me",
            &["messages"],
        )?;
        url.query_pairs_mut()
            .append_pair("q", "has:attachment (filename:xlsx OR filename:xlsm)")
            .append_pair("maxResults", "25");
        let list = json_response(
            client
                .get(url)
                .bearer_auth(&token)
                .send()
                .map_err(|_| "Could not read Gmail messages")?,
            2 * 1024 * 1024,
        )?;
        let mut rows = Vec::new();
        for item in list["messages"].as_array().unwrap_or(&Vec::new()) {
            let id = item["id"].as_str().ok_or("Missing Gmail message ID")?;
            let mut url = endpoint(
                "https://gmail.googleapis.com/gmail/v1/users/me",
                &["messages", id],
            )?;
            url.query_pairs_mut()
                .append_pair("format", "metadata")
                .append_pair("metadataHeaders", "Subject")
                .append_pair("metadataHeaders", "From")
                .append_pair("metadataHeaders", "Date");
            let message = json_response(
                client
                    .get(url)
                    .bearer_auth(&token)
                    .send()
                    .map_err(|_| "Could not read Gmail message headers")?,
                2 * 1024 * 1024,
            )?;
            let header = |name: &str| {
                message
                    .pointer("/payload/headers")
                    .and_then(Value::as_array)
                    .and_then(|headers| {
                        headers.iter().find(|h| {
                            h["name"]
                                .as_str()
                                .is_some_and(|v| v.eq_ignore_ascii_case(name))
                        })
                    })
                    .and_then(|h| h["value"].as_str())
                    .unwrap_or_default()
                    .to_owned()
            };
            rows.push(json!({"id":id,"subject":header("Subject"),"sender":header("From"),"received":header("Date")}));
        }
        return Ok(Value::Array(rows));
    }
    let mut url = endpoint(
        "https://gmail.googleapis.com/gmail/v1/users/me",
        &["messages", id],
    )?;
    url.query_pairs_mut().append_pair("format", "full");
    let value = json_response(
        client
            .get(url)
            .bearer_auth(&token)
            .send()
            .map_err(|_| "Could not inspect Gmail attachments")?,
        20 * 1024 * 1024,
    )?;
    let mut rows = Vec::new();
    gmail_parts(&value["payload"], &mut rows, 0);
    Ok(Value::Array(rows))
}
fn gmail_parts(part: &Value, out: &mut Vec<Value>, depth: usize) {
    if depth > 20 || out.len() > 200 {
        return;
    }
    if part["filename"].as_str().is_some_and(valid_name) {
        if let Some(id) = part.pointer("/body/attachmentId").and_then(Value::as_str) {
            out.push(json!({"id":id,"name":part["filename"],"size":part.pointer("/body/size").unwrap_or(&Value::Null)}));
        }
    }
    if let Some(parts) = part["parts"].as_array() {
        for part in parts {
            gmail_parts(part, out, depth + 1);
        }
    }
}
#[derive(Deserialize)]
pub struct Download {
    pub provider: String,
    pub message_id: String,
    pub attachment_id: String,
    pub store_id: String,
    pub name: String,
    pub size: u64,
}
pub fn download(state: &AppState, selection: Download) -> Result<Input> {
    supported_source(&selection.provider)?;
    if !valid_name(&selection.name) || selection.size > MAX_INPUT {
        return Err("Choose an Excel attachment up to 100 MB".into());
    }
    let _operation = crate::gmail::lock_for_update()?;
    let scratch = state.root.join("excelflow-downloads");
    std::fs::create_dir_all(&scratch).map_err(|_| "Could not prepare workbook download folder")?;
    let temp = scratch.join(format!(
        "{}.{}",
        uuid::Uuid::new_v4(),
        if selection.name.to_lowercase().ends_with("xlsm") {
            "xlsm"
        } else {
            "xlsx"
        }
    ));
    let result = (|| -> Result<Input> {
        let bytes = match selection.provider.as_str() {
            "mailserver" => {
                let fields = json!({"operation":"download","message_id":selection.message_id,"attachment_id":selection.attachment_id,"store_id":selection.store_id,"expected_name":selection.name,"expected_size":selection.size,"destination":temp});
                let value = crate::mailserver::request("mail_download", fields)?;
                if value["name"] != selection.name || value["size"].as_u64() != Some(selection.size)
                {
                    return Err(
                        "The selected attachment changed. Refresh the email and choose it again."
                            .into(),
                    );
                }
                bounded(&temp, MAX_INPUT)?
            }
            "gmail" => {
                let (client, token) = crate::gmail::source_token()?;
                let url = endpoint(
                    "https://gmail.googleapis.com/gmail/v1/users/me",
                    &[
                        "messages",
                        &selection.message_id,
                        "attachments",
                        &selection.attachment_id,
                    ],
                )?;
                let response = client
                    .get(url)
                    .bearer_auth(token)
                    .send()
                    .map_err(|_| "Could not download the selected email attachment")?;
                use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
                let value = json_response(response, 140 * 1024 * 1024)?;
                URL_SAFE_NO_PAD
                    .decode(
                        value["data"]
                            .as_str()
                            .ok_or("Missing Gmail attachment bytes")?
                            .trim_end_matches('='),
                    )
                    .map_err(|_| "Invalid Gmail attachment encoding")?
            }
            _ => return Err("Unknown email source".into()),
        };
        if bytes.len() as u64 != selection.size {
            return Err("Attachment size changed. Refresh and choose it again.".into());
        }
        stage_bytes(state, &selection.name, &bytes)
    })();
    let _ = std::fs::remove_file(temp);
    result
}
fn supported_source(provider: &str) -> Result<()> {
    if matches!(provider, "gmail" | "mailserver") {
        Ok(())
    } else {
        Err("Choose Gmail or IMAP / POP3.".into())
    }
}
#[derive(Deserialize)]
pub struct Generate {
    pub disc: Option<String>,
    pub ftg: Option<String>,
    pub scope: String,
    pub mode: String,
    pub auto_period: bool,
    pub month: u32,
    pub year: u32,
    pub money_places: u32,
    pub quantity_places: u32,
    pub factor: String,
    pub repair_dates: bool,
    pub validate_only: bool,
    #[serde(default)]
    pub destination_id: Option<String>,
}
pub fn generate(state: &AppState, request: Generate) -> Result<Value> {
    let mut inputs = serde_json::Map::new();
    if let Some(id) = request.disc {
        inputs.insert("DISC".into(), json!(path_for(state, &id)?));
    }
    if let Some(id) = request.ftg {
        inputs.insert("FTG".into(), json!(path_for(state, &id)?));
    }
    if inputs.is_empty() {
        return Err("Choose a DISC or FTG workbook first".into());
    }
    let destination = match request.destination_id {
        Some(id) => attachment_sources::root(state, &id)?,
        None => state.output.join("ExcelFlow"),
    };
    std::fs::create_dir_all(&destination).map_err(|_| "Could not create report output folder")?;
    let result = crate::runtime::request(
        json!({"operation":"generate","inputs":inputs,"destination":destination,"settings_path":state.root.join("excelflow-settings.json"),"scope":request.scope,"mode":request.mode,"auto_period":request.auto_period,"month":request.month,"year":request.year,"money_places":request.money_places,"quantity_places":request.quantity_places,"factor":request.factor,"repair_dates":request.repair_dates,"validate_only":request.validate_only}),
    )?;
    if request.validate_only {
        return Ok(result);
    }
    let folder = result["folder"]
        .as_str()
        .ok_or("Report output folder was not returned")?;
    let canonical = std::fs::canonicalize(folder).map_err(|_| "Report output missing")?;
    if canonical.parent()
        != Some(
            std::fs::canonicalize(destination)
                .map_err(|_| "Report output unavailable")?
                .as_path(),
        )
    {
        return Err("Unexpected report output location".into());
    }
    let registered = attachment_sources::register(state, &canonical)?;
    remember(state, &registered)?;
    Ok(
        json!({"run":excelflow::inspect(state,&registered.id)?,"folder":registered,"details":result.get("details")}),
    )
}
pub fn settings(state: &AppState, agencies: Option<Value>) -> Result<Value> {
    crate::runtime::request(
        json!({"operation":if agencies.is_some(){"save_agencies"}else{"settings"},"settings_path":state.root.join("excelflow-settings.json"),"agencies":agencies}),
    )
}

fn remember(state: &AppState, folder: &attachment_sources::Folder) -> Result<()> {
    let file = state.root.join("excelflow-runs.json");
    let mut folders: Vec<attachment_sources::Folder> = std::fs::read(&file)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    folders.retain(|f| f.id != folder.id);
    folders.insert(0, folder.clone());
    folders.truncate(20);
    atomic_write(
        &file,
        &serde_json::to_vec(&folders).map_err(|_| "Could not save report run history")?,
    )
}
pub fn recent(state: &AppState) -> Result<Vec<excelflow::Run>> {
    let file = state.root.join("excelflow-runs.json");
    if !file.is_file() {
        return Ok(Vec::new());
    }
    let folders: Vec<attachment_sources::Folder> =
        serde_json::from_slice(&std::fs::read(file).map_err(|_| "Report run history unavailable")?)
            .map_err(|_| "Report run history is unreadable")?;
    Ok(folders
        .iter()
        .take(20)
        .filter_map(|f| excelflow::inspect(state, &f.id).ok())
        .collect())
}
pub fn as_attachment(state: &AppState, id: &str) -> Result<Vec<crate::attachments::AttachmentRef>> {
    crate::attachments::stage(state, &[path_for(state, id)?])
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn staged_workbook_is_scoped_and_tampering_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::initialize(temp.path().join("state")).unwrap();
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            zip.start_file("xl/workbook.xml", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"<workbook/>").unwrap();
            zip.finish().unwrap();
        }
        let input = stage_bytes(&state, "ALL_DISC_2610.xlsx", buffer.get_ref()).unwrap();
        let path = path_for(&state, &input.id).unwrap();
        assert!(
            path.starts_with(std::fs::canonicalize(state.root.join("excelflow-inputs")).unwrap())
        );
        std::fs::write(path, b"changed").unwrap();
        assert!(path_for(&state, &input.id).is_err());
        assert!(path_for(&state, "../outside").is_err());
        assert!(stage_bytes(&state, "../private.xlsx", buffer.get_ref()).is_err());
    }
}

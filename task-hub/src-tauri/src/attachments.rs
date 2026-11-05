//! Immutable attachment snapshots. Only a native file-picker selection may
//! introduce local files; email commands accept scoped IDs, never file paths.
use crate::{
    model::Result,
    storage::{atomic_write, AppState},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
pub const MAX_FILES: usize = 10;
pub const MAX_BYTES: u64 = 18 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRef {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
}
pub struct AttachmentData {
    pub name: String,
    pub bytes: Vec<u8>,
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 1024
        && !name.chars().any(|c| c.is_control() || "/\\".contains(c))
}
pub fn validate_list(items: &[AttachmentRef]) -> Result<()> {
    if items.len() > MAX_FILES {
        return Err("Attach at most 10 files per email".into());
    }
    let mut ids = BTreeSet::new();
    let mut total = 0u64;
    for item in items {
        if uuid::Uuid::parse_str(&item.id)
            .map(|id| id.to_string())
            .ok()
            .as_deref()
            != Some(&item.id)
            || !ids.insert(&item.id)
            || !valid_name(&item.name)
            || item.sha256.len() != 64
            || !item.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(
                "Invalid or duplicate attachment reference. Remove the file and add it again."
                    .into(),
            );
        }
        total = total
            .checked_add(item.size)
            .ok_or("Attachment size overflow")?;
    }
    if total > MAX_BYTES {
        return Err("Attachments must total 18 MB or less per email".into());
    }
    Ok(())
}
fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file =
        File::open(path).map_err(|_| "An attachment could not be opened. Add the file again.")?;
    let metadata = file
        .metadata()
        .map_err(|_| "Could not inspect attachment")?;
    if metadata.len() > limit {
        return Err("Attachments must total 18 MB or less per email".into());
    }
    if !metadata.is_file() {
        return Err("Attachments must be regular files".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "An attachment could not be read")?;
    if bytes.len() as u64 > limit {
        return Err("Attachments must total 18 MB or less per email".into());
    }
    Ok(bytes)
}
pub(crate) fn stage(state: &AppState, paths: &[PathBuf]) -> Result<Vec<AttachmentRef>> {
    if paths.len() > MAX_FILES {
        return Err("Select at most 10 files at a time".into());
    }
    let mut staged = Vec::new();
    let mut total = 0;
    for path in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| valid_name(name))
            .ok_or("This attachment has an unsupported filename")?
            .to_owned();
        let before = std::fs::metadata(path).map_err(|_| "Could not inspect the selected file")?;
        let bytes = bounded_read(path, MAX_BYTES - total)?;
        let after =
            std::fs::metadata(path).map_err(|_| "The selected file changed while being copied")?;
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err("The selected file changed while being copied. Finish saving it, then add it again.".into());
        }
        total += bytes.len() as u64;
        let item = AttachmentRef {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        };
        let directory = state.root.join("attachments").join(&item.id);
        atomic_write(&directory.join("content.bin"), &bytes)?;
        atomic_write(
            &directory.join("metadata.json"),
            &serde_json::to_vec(&item).map_err(|_| "Could not store attachment metadata")?,
        )?;
        staged.push(item);
    }
    Ok(staged)
}
pub fn load(state: &AppState, items: &[AttachmentRef]) -> Result<Vec<AttachmentData>> {
    validate_list(items)?;
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let root = std::fs::canonicalize(state.root.join("attachments"))
        .map_err(|_| "Saved attachments are missing. Add the files again.")?;
    items.iter().map(|item| {
        let directory = root.join(&item.id);
        let metadata = std::fs::canonicalize(directory.join("metadata.json")).map_err(|_| format!("Attachment '{}' is missing. Add it again.", item.name))?;
        let content = std::fs::canonicalize(directory.join("content.bin")).map_err(|_| format!("Attachment '{}' is missing. Add it again.", item.name))?;
        if !metadata.starts_with(&root) || !content.starts_with(&root) { return Err("Attachment points outside Task Hub’s attachment storage".into()); }
        let saved: AttachmentRef = serde_json::from_slice(&bounded_read(&metadata, 4096)?).map_err(|_| "Attachment metadata is invalid")?;
        let bytes = bounded_read(&content, MAX_BYTES)?;
        if saved != *item || bytes.len() as u64 != item.size || format!("{:x}", Sha256::digest(&bytes)) != item.sha256 {
            return Err(format!("Attachment '{}' changed or could not be verified. Add it again. No email was submitted.", item.name));
        }
        Ok(AttachmentData { name: item.name.clone(), bytes })
    }).collect()
}
pub fn from_parameters(
    state: &AppState,
    parameters: &crate::model::Parameters,
) -> Result<Vec<AttachmentData>> {
    let refs = parameters
        .get("_attachments")
        .map(|value| {
            let text = value.as_str().ok_or("Invalid attachment metadata")?;
            serde_json::from_str::<Vec<AttachmentRef>>(text)
                .map_err(|_| "Invalid attachment metadata")
        })
        .transpose()?
        .unwrap_or_default();
    load(state, &refs)
}
#[cfg(windows)]
pub fn pick(state: &AppState, owner: isize) -> Result<Vec<AttachmentRef>> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let mut buffer = vec![0u16; 65536];
    let title: Vec<u16> = "Add email attachments\0".encode_utf16().collect();
    let filter: Vec<u16> = "All files\0*.*\0\0".encode_utf16().collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner as _,
        lpstrFile: buffer.as_mut_ptr(),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: title.as_ptr(),
        lpstrFilter: filter.as_ptr(),
        Flags: OFN_EXPLORER | OFN_ALLOWMULTISELECT | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        return if unsafe { CommDlgExtendedError() } == 0 {
            Ok(Vec::new())
        } else {
            Err("Could not open the attachment file picker. Try selecting fewer files.".into())
        };
    }
    stage(state, &selected_paths(&buffer)?)
}
#[cfg(not(windows))]
pub fn pick(_: &AppState, _: isize) -> Result<Vec<AttachmentRef>> {
    Err("Native attachment selection currently requires Windows".into())
}
fn selected_paths(buffer: &[u16]) -> Result<Vec<PathBuf>> {
    let mut parts = Vec::new();
    for part in buffer.split(|c| *c == 0) {
        if part.is_empty() {
            break;
        }
        parts.push(String::from_utf16(part).map_err(|_| "Unsupported filename encoding")?);
    }
    if parts.is_empty() {
        return Ok(Vec::new());
    }
    if parts.len() == 1 {
        return Ok(vec![PathBuf::from(&parts[0])]);
    }
    let directory = Path::new(&parts[0]);
    Ok(parts[1..].iter().map(|name| directory.join(name)).collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_survive_original_changes_and_detect_saved_copy_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().join("app")).unwrap();
        let source = dir.path().join("report.txt");
        std::fs::write(&source, b"Original report").unwrap();
        let refs = stage(&state, &[source.clone()]).unwrap();
        std::fs::write(&source, b"Updated report").unwrap();
        assert_eq!(load(&state, &refs).unwrap()[0].bytes, b"Original report");
        std::fs::write(
            state
                .root
                .join("attachments")
                .join(&refs[0].id)
                .join("content.bin"),
            b"changed",
        )
        .unwrap();
        assert!(load(&state, &refs).err().unwrap().contains("changed"));
    }
    #[test]
    fn invalid_ids_duplicate_files_and_excessive_sizes_are_rejected() {
        let mut item = AttachmentRef {
            id: uuid::Uuid::new_v4().to_string(),
            name: "report.pdf".into(),
            size: 4,
            sha256: "a".repeat(64),
        };
        assert!(validate_list(&[item.clone(), item.clone()]).is_err());
        item.size = MAX_BYTES + 1;
        assert!(validate_list(&[item.clone()]).is_err());
        item.size = 4;
        item.id = "../private".into();
        assert!(validate_list(&[item]).is_err());
    }
    #[test]
    fn native_picker_multiple_file_format_is_parsed_without_truncating_names() {
        let buffer: Vec<u16> = "C:\\Reports\0one.pdf\0two.xlsx\0\0"
            .encode_utf16()
            .collect();
        assert_eq!(
            selected_paths(&buffer).unwrap(),
            vec![
                Path::new("C:\\Reports").join("one.pdf"),
                Path::new("C:\\Reports").join("two.xlsx")
            ]
        );
    }
}

//! Native-selected attachment folders. Commands accept IDs, never arbitrary paths.
use crate::{
    model::Result,
    storage::{atomic_write, AppState},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub path: String,
}
fn valid_id(id: &str) -> Result<()> {
    if uuid::Uuid::parse_str(id)
        .map(|u| u.to_string())
        .ok()
        .as_deref()
        != Some(id)
    {
        return Err("Choose a report folder again".into());
    }
    Ok(())
}
pub fn register(state: &AppState, path: &Path) -> Result<Folder> {
    let canonical = fs::canonicalize(path).map_err(|_| "Report folder could not be opened")?;
    if !canonical.is_dir() {
        return Err("Choose a folder".into());
    }
    let folder = Folder {
        id: uuid::Uuid::new_v4().to_string(),
        path: canonical
            .to_str()
            .ok_or("Unsupported folder path")?
            .to_owned(),
    };
    atomic_write(
        &state
            .root
            .join("attachment-folders")
            .join(format!("{}.json", folder.id)),
        &serde_json::to_vec(&folder).map_err(|_| "Invalid report folder")?,
    )?;
    Ok(folder)
}
pub fn root(state: &AppState, id: &str) -> Result<PathBuf> {
    valid_id(id)?;
    let file = state
        .root
        .join("attachment-folders")
        .join(format!("{id}.json"));
    let bytes = fs::read(file)
        .map_err(|_| "Report folder permission is missing. Choose the folder again.")?;
    if bytes.len() > 8192 {
        return Err("Invalid report folder record".into());
    }
    let folder: Folder =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid report folder record")?;
    let path = fs::canonicalize(&folder.path)
        .map_err(|_| "The report folder is unavailable. Restore it or choose another folder.")?;
    if folder.id != id || path != PathBuf::from(folder.path) || !path.is_dir() {
        return Err("The report folder changed location. Choose it again.".into());
    }
    Ok(path)
}
pub fn files(state: &AppState, id: &str) -> Result<Vec<(String, PathBuf)>> {
    let folder = root(state, id)?;
    let mut out = vec![];
    for (index, item) in fs::read_dir(&folder)
        .map_err(|_| "Could not list the selected report folder")?
        .enumerate()
    {
        if index >= 2000 {
            return Err(
                "The selected folder has more than 2000 entries. Choose a smaller report folder."
                    .into(),
            );
        }
        let item = item.map_err(|_| "Could not inspect a report file")?;
        let name = item
            .file_name()
            .into_string()
            .map_err(|_| "Unsupported report filename")?;
        if name.starts_with('.')
            || name.starts_with("~$")
            || name.ends_with(".tmp")
            || !item
                .file_type()
                .map_err(|_| "Could not inspect a report file")?
                .is_file()
        {
            continue;
        }
        let path = fs::canonicalize(item.path())
            .map_err(|_| "A report moved during folder inspection. Test the rule again.")?;
        if path.parent() != Some(folder.as_path()) {
            return Err("A report points outside the selected folder".into());
        }
        out.push((name, path));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
pub fn read_stable(state: &AppState, id: &str, name: &str, limit: u64) -> Result<Vec<u8>> {
    if name.is_empty()
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:<>\"|*?".contains(c))
    {
        return Err("Use a regular report filename".into());
    }
    let folder = root(state, id)?;
    let source = folder.join(name);
    if !fs::symlink_metadata(&source)
        .map_err(|_| "The report no longer exists")?
        .file_type()
        .is_file()
    {
        return Err("Report links and folders are not supported".into());
    }
    let canonical = fs::canonicalize(&source).map_err(|_| "Could not resolve the report")?;
    if canonical.parent() != Some(folder.as_path()) {
        return Err("A report points outside the selected folder".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let file = options.open(&source).map_err(|_| {
        "The report is locked or unavailable. Finish saving and close it, then retry."
    })?;
    let before = file.metadata().map_err(|_| "Could not inspect report")?;
    if before.len() > limit {
        return Err("Attachments must total 18 MB or less".into());
    }
    let mut bytes = Vec::new();
    (&file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read report")?;
    let after = file.metadata().map_err(|_| "Could not inspect report")?;
    if bytes.len() as u64 > limit
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || fs::canonicalize(&source).ok().as_ref() != Some(&canonical)
    {
        return Err("The report changed while being read. Finish saving it and retry.".into());
    }
    Ok(bytes)
}
#[cfg(windows)]
pub fn pick(state: &AppState, owner: isize) -> Result<Option<Folder>> {
    let selected = std::thread::spawn(move || -> Result<Option<PathBuf>> {
        use windows_sys::Win32::{System::Com::*, UI::Shell::*};
        unsafe {
            if CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) < 0 {
                return Err("Could not initialize the folder picker".into());
            }
            let title: Vec<u16> = "Choose the folder containing your report files\0"
                .encode_utf16()
                .collect();
            let mut display = vec![0u16; 32768];
            let info = BROWSEINFOW {
                hwndOwner: owner as _,
                pszDisplayName: display.as_mut_ptr(),
                lpszTitle: title.as_ptr(),
                ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
                ..Default::default()
            };
            let item = SHBrowseForFolderW(&info);
            if item.is_null() {
                CoUninitialize();
                return Ok(None);
            }
            let mut path = vec![0u16; 32768];
            let ok = SHGetPathFromIDListEx(item, path.as_mut_ptr(), path.len() as u32, 0) != 0;
            CoTaskMemFree(item as _);
            CoUninitialize();
            if !ok {
                return Err("Choose a filesystem folder".into());
            }
            let len = path.iter().position(|c| *c == 0).unwrap_or(path.len());
            Ok(Some(PathBuf::from(
                String::from_utf16(&path[..len]).map_err(|_| "Unsupported folder path")?,
            )))
        }
    })
    .join()
    .map_err(|_| "The folder picker stopped unexpectedly")??;
    selected.map(|path| register(state, &path)).transpose()
}
#[cfg(not(windows))]
pub fn pick(_: &AppState, _: isize) -> Result<Option<Folder>> {
    Err("Report folder selection currently requires Windows".into())
}

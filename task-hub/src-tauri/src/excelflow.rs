//! Read-only adapter for completed Harrison ExcelFlow run_summary.json outputs.
//! ExcelFlow retains all report calculations; Task Hub imports selected snapshots.
use crate::{
    attachment_sources as sources,
    attachments::{self, AttachmentRef},
    model::Result,
    storage::AppState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};
#[derive(Deserialize)]
struct Summary {
    status: String,
    period: String,
    files: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    file: String,
    kind: String,
    branch: String,
    rows: u64,
}
#[derive(Serialize)]
pub struct Report {
    pub key: String,
    pub name: String,
    pub relative_path: String,
    pub kind: String,
    pub branch: String,
    pub version: String,
    pub rows: u64,
    pub size: u64,
    pub stamp: String,
}
#[derive(Serialize)]
pub struct Run {
    pub folder: sources::Folder,
    pub period: String,
    pub summary_hash: String,
    pub reports: Vec<Report>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|_| {
        "Could not open the ExcelFlow report. Finish saving it and choose the run again."
    })?;
    let meta = file
        .metadata()
        .map_err(|_| "Could not inspect ExcelFlow file")?;
    if !meta.is_file() || meta.len() > limit {
        return Err("ExcelFlow file is too large or is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read ExcelFlow file")?;
    if bytes.len() as u64 > limit {
        return Err("ExcelFlow file exceeds the size limit".into());
    }
    Ok(bytes)
}
fn report_path(root: &Path, relative: &str) -> Result<PathBuf> {
    // Normalize legacy Windows separators on every platform, then constrain every component.
    let name = relative.replace('\\', "/");
    if name.is_empty()
        || name.len() > 2048
        || name.split('/').any(|p| {
            p.is_empty()
                || p == "."
                || p == ".."
                || p.chars().any(|c| c.is_control() || ":*?\"<>|".contains(c))
        })
        || Path::new(&name)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("ExcelFlow summary contains an unsafe report path".into());
    }
    let path=fs::canonicalize(root.join(name)).map_err(|_|"A report listed by ExcelFlow is missing. Choose a completed run with all its report folders intact.")?;
    if !path.starts_with(root)
        || !path.is_file()
        || path
            .extension()
            .and_then(|v| v.to_str())
            .is_none_or(|v| !v.eq_ignore_ascii_case("xlsx"))
    {
        return Err("ExcelFlow report must be an .xlsx file inside the selected run folder".into());
    }
    Ok(path)
}
fn stamp(path: &Path) -> Result<String> {
    let m = fs::metadata(path).map_err(|_| "Could not inspect ExcelFlow report")?;
    let time = m
        .modified()
        .map_err(|_| "Could not check report modification time")?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Invalid report modification time")?;
    Ok(format!("{}:{}", m.len(), time.as_nanos()))
}
pub fn inspect(state: &AppState, folder_id: &str) -> Result<Run> {
    let root = sources::root(state, folder_id)?;
    let summary_path=fs::canonicalize(root.join("run_summary.json")).map_err(|_|"Choose the ExcelFlow run folder containing run_summary.json, not the Desktop or an individual DISC/FTG subfolder.")?;
    if summary_path.parent() != Some(root.as_path()) {
        return Err("ExcelFlow summary points outside the selected run folder".into());
    }
    let bytes = read_limited(&summary_path, 2 * 1024 * 1024)?;
    let summary: Summary = serde_json::from_slice(&bytes)
        .map_err(|_| "This is not a readable ExcelFlow run summary")?;
    if summary.status != "complete" {
        return Err("ExcelFlow has not completed this run successfully. Failed, cancelled or running reports cannot be imported.".into());
    }
    let parts = summary.period.split('-').collect::<Vec<_>>();
    if parts.len() != 2
        || parts[0].len() != 4
        || parts[1].len() != 2
        || parts[0]
            .parse::<u32>()
            .ok()
            .is_none_or(|v| !(1900..=2200).contains(&v))
        || parts[1]
            .parse::<u32>()
            .ok()
            .is_none_or(|v| !(1..=12).contains(&v))
    {
        return Err("ExcelFlow summary has an invalid reporting month".into());
    }
    if summary.files.is_empty() || summary.files.len() > 2000 {
        return Err("ExcelFlow run has no reports, or exceeds 2000 reports. Choose a smaller completed run.".into());
    }
    let mut reports = Vec::new();
    let mut used = BTreeSet::new();
    for entry in summary.files {
        if !matches!(entry.kind.as_str(), "DISC" | "FTG")
            || entry.branch.len() != 3
            || !entry.branch.starts_with(|c: char| c.is_ascii_uppercase())
            || !entry.branch[1..].chars().all(|c| c.is_ascii_digit())
        {
            return Err("ExcelFlow summary has an invalid report type or branch".into());
        }
        let relative = entry.file.replace('\\', "/");
        let path = report_path(&root, &relative)?;
        if !used.insert(path.clone()) {
            return Err("ExcelFlow summary lists the same report more than once".into());
        }
        let size = fs::metadata(&path)
            .map_err(|_| "Could not inspect report size")?
            .len();
        let version = if relative
            .split('/')
            .next()
            .unwrap_or_default()
            .contains("DISC with IPCF, FTG with IPCD")
        {
            "Special"
        } else {
            "Normal"
        };
        reports.push(Report {
            key: hash(relative.as_bytes()),
            name: path
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or("Unsupported report filename")?
                .into(),
            relative_path: relative,
            kind: entry.kind,
            branch: entry.branch,
            version: version.into(),
            rows: entry.rows,
            size,
            stamp: stamp(&path)?,
        });
    }
    reports.sort_by(|a, b| (&a.branch, &a.kind, &a.version).cmp(&(&b.branch, &b.kind, &b.version)));
    Ok(Run {
        folder: sources::Folder {
            id: folder_id.into(),
            path: root.to_string_lossy().into(),
        },
        period: summary.period,
        summary_hash: hash(&bytes),
        reports,
    })
}
#[derive(Deserialize)]
pub struct Selection {
    pub key: String,
    pub stamp: String,
}
pub fn import(
    state: &AppState,
    folder_id: &str,
    summary_hash: &str,
    selected: Vec<Selection>,
    existing: Vec<AttachmentRef>,
) -> Result<Vec<AttachmentRef>> {
    if selected.is_empty() || selected.len() + existing.len() > attachments::MAX_FILES {
        return Err(
            "Choose reports within the 10-file attachment limit, including files already attached"
                .into(),
        );
    }
    attachments::load(state, &existing)?;
    let run = inspect(state, folder_id)?;
    if run.summary_hash != summary_hash {
        return Err(
            "ExcelFlow run changed since you opened it. Choose the run again before importing."
                .into(),
        );
    }
    let root = sources::root(state, folder_id)?;
    let mut paths = Vec::new();
    let mut keys = BTreeSet::new();
    let mut names = existing
        .iter()
        .map(|a| a.name.to_lowercase())
        .collect::<BTreeSet<_>>();
    let mut total = existing.iter().map(|a| a.size).sum::<u64>();
    for selection in selected {
        if !keys.insert(selection.key.clone()) {
            return Err("A report was selected twice".into());
        }
        let report = run
            .reports
            .iter()
            .find(|r| r.key == selection.key)
            .ok_or("Selected report is not in this completed ExcelFlow run")?;
        if report.stamp != selection.stamp {
            return Err(
                "A selected report changed after the preview. Choose the run again.".into(),
            );
        }
        if !names.insert(report.name.to_lowercase()) {
            return Err("Two selected attachments have the same filename. Normal and Special reports can share names; choose one version per email or use separate routines.".into());
        }
        total = total
            .checked_add(report.size)
            .ok_or("Attachment size overflow")?;
        if total > attachments::MAX_BYTES {
            return Err("Attachments must total 18 MB or less. Choose fewer reports.".into());
        }
        let path = report_path(&root, &report.relative_path)?;
        let bytes = read_limited(&path, attachments::MAX_BYTES)?;
        let mut workbook = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| {
            "An ExcelFlow report is not a valid Excel workbook. Regenerate it in ExcelFlow."
        })?;
        if workbook.by_name("xl/workbook.xml").is_err() {
            return Err("A selected report has no Excel workbook data".into());
        }
        paths.push((path, report.stamp.clone()));
    }
    let files = attachments::stage(
        state,
        &paths.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
    )?;
    for (path, before) in paths {
        if stamp(&path)? != before {
            return Err(
                "A report changed while being imported. Choose the completed run again.".into(),
            );
        }
    }
    Ok(files)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn setup() -> (tempfile::TempDir, AppState, sources::Folder) {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::initialize(temp.path().join("state")).unwrap();
        let output = temp.path().join("run");
        fs::create_dir_all(&output).unwrap();
        let folder = sources::register(&state, &output).unwrap();
        (temp, state, folder)
    }
    fn workbook(root: &Path, relative: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        zip.start_file("xl/workbook.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"<workbook/>").unwrap();
        zip.finish().unwrap();
    }
    fn summary(root: &Path, status: &str, files: serde_json::Value) {
        fs::write(
            root.join("run_summary.json"),
            serde_json::to_vec(
                &serde_json::json!({"status":status,"period":"2026-10","files":files}),
            )
            .unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn completed_reports_import_as_immutable_snapshots_without_changing_sources() {
        let (_temp, state, folder) = setup();
        let root = sources::root(&state, &folder.id).unwrap();
        let relative = "Formatted October 2026/DISC/01 KK DISC 1026.xlsx";
        workbook(&root, relative);
        summary(
            &root,
            "complete",
            serde_json::json!([{"file":relative,"kind":"DISC","branch":"A01","rows":12}]),
        );
        let run = inspect(&state, &folder.id).unwrap();
        assert_eq!(run.period, "2026-10");
        assert_eq!(run.reports[0].version, "Normal");
        let item = &run.reports[0];
        let files = import(
            &state,
            &folder.id,
            &run.summary_hash,
            vec![Selection {
                key: item.key.clone(),
                stamp: item.stamp.clone(),
            }],
            vec![],
        )
        .unwrap();
        let copied = attachments::load(&state, &files).unwrap();
        assert_eq!(copied[0].bytes, fs::read(root.join(relative)).unwrap());
        fs::write(root.join(relative), b"changed later").unwrap();
        assert_eq!(
            attachments::load(&state, &files).unwrap()[0].bytes,
            copied[0].bytes
        );
        assert!(import(
            &state,
            &folder.id,
            &run.summary_hash,
            vec![Selection {
                key: item.key.clone(),
                stamp: item.stamp.clone()
            }],
            vec![]
        )
        .is_err());
    }
    #[test]
    fn failed_runs_paths_missing_and_duplicate_versions_stop_before_import() {
        let (_temp, state, folder) = setup();
        let root = sources::root(&state, &folder.id).unwrap();
        for status in ["running", "failed", "cancelled"] {
            summary(&root, status, serde_json::json!([]));
            assert!(inspect(&state, &folder.id).is_err());
        }
        for path in [
            "../private.xlsx",
            "C:\\private.xlsx",
            "missing.xlsx",
            "/absolute.xlsx",
        ] {
            summary(
                &root,
                "complete",
                serde_json::json!([{"file":path,"kind":"FTG","branch":"A01","rows":1}]),
            );
            assert!(inspect(&state, &folder.id).is_err());
        }
        let normal = "Formatted October 2026/DISC/01 KK DISC 1026.xlsx";
        let special =
            "Formatted October 2026 (DISC with IPCF, FTG with IPCD)/DISC/01 KK DISC 1026.xlsx";
        workbook(&root, normal);
        workbook(&root, special);
        summary(
            &root,
            "complete",
            serde_json::json!([{"file":normal,"kind":"DISC","branch":"A01","rows":1},{"file":special,"kind":"DISC","branch":"A01","rows":1}]),
        );
        let run = inspect(&state, &folder.id).unwrap();
        assert!(run.reports.iter().any(|r| r.version == "Special"));
        assert!(import(
            &state,
            &folder.id,
            &run.summary_hash,
            run.reports
                .iter()
                .map(|r| Selection {
                    key: r.key.clone(),
                    stamp: r.stamp.clone()
                })
                .collect(),
            vec![]
        )
        .is_err());
        assert!(!state.root.join("attachments").exists());
        assert!(import(
            &state,
            &folder.id,
            "changed",
            vec![Selection {
                key: run.reports[0].key.clone(),
                stamp: run.reports[0].stamp.clone()
            }],
            vec![]
        )
        .is_err());
    }
    #[test]
    fn unlisted_and_invalid_workbooks_cannot_be_imported() {
        let (_temp, state, folder) = setup();
        let root = sources::root(&state, &folder.id).unwrap();
        let relative = "Formatted October 2026/FTG/01 KK FTG 1026.xlsx";
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"not an xlsx").unwrap();
        summary(
            &root,
            "complete",
            serde_json::json!([{"file":relative,"kind":"FTG","branch":"A01","rows":1}]),
        );
        let run = inspect(&state, &folder.id).unwrap();
        let report = &run.reports[0];
        assert!(import(
            &state,
            &folder.id,
            &run.summary_hash,
            vec![Selection {
                key: "unlisted".into(),
                stamp: report.stamp.clone()
            }],
            vec![]
        )
        .is_err());
        assert!(import(
            &state,
            &folder.id,
            &run.summary_hash,
            vec![Selection {
                key: report.key.clone(),
                stamp: report.stamp.clone()
            }],
            vec![]
        )
        .is_err());
        assert!(!state.root.join("attachments").exists());
    }
}

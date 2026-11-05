//! Fresh per-occurrence reports. Missing reports may be copied from an explicit
//! template, but are never emailed until the user completes and confirms them.
use crate::{
    attachment_patterns as patterns, attachment_sources as sources,
    attachments::{self, AttachmentData, AttachmentRef},
    model::{Result, Run},
    storage::{atomic_write, AppState},
};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
pub const ISSUE_PREFIX: &str = "Report attachment preparation stopped before sending: ";
fn changed_default() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportRule {
    pub id: String,
    pub folder: sources::Folder,
    pub pattern: String,
    #[serde(default)]
    pub previous_month: bool,
    #[serde(default)]
    pub template: Option<AttachmentRef>,
    #[serde(default = "changed_default")]
    pub require_changed: bool,
}
#[derive(Clone, Serialize)]
pub struct RuleCheck {
    pub expected: String,
    pub matches: Vec<String>,
    pub status: String,
    pub suggestions: Vec<patterns::Suggestion>,
}
#[derive(Serialize, Deserialize)]
struct Pending {
    folder_id: String,
    name: String,
    initial_sha256: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ReportReceipt {
    pub rule_id: String,
    pub folder_id: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
}
pub struct Prepared {
    pub files: Vec<AttachmentData>,
    pub receipts: Vec<ReportReceipt>,
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn day(due: &str) -> Result<NaiveDate> {
    DateTime::parse_from_rfc3339(due)
        .map(|d| d.with_timezone(&Local).date_naive())
        .or_else(|_| NaiveDate::parse_from_str(due, "%Y-%m-%d"))
        .map_err(|_| "Choose a valid report date".into())
}
fn pending_path(state: &AppState, folder: &str, name: &str) -> PathBuf {
    state.root.join("pending-reports").join(format!(
        "{}.json",
        sha(format!("{folder}\n{}", name.to_lowercase()).as_bytes())
    ))
}
pub fn validate(state: &AppState, rules: &[ReportRule], fixed: usize) -> Result<()> {
    if rules.len() + fixed > attachments::MAX_FILES {
        return Err("Use at most 10 fixed attachments and report rules combined".into());
    }
    let mut ids = BTreeSet::new();
    for rule in rules {
        if uuid::Uuid::parse_str(&rule.id).is_err() || !ids.insert(&rule.id) {
            return Err("Invalid or duplicate report rule".into());
        }
        if sources::root(state, &rule.folder.id)? != PathBuf::from(&rule.folder.path) {
            return Err("Report folder details do not match its selected location. Choose the folder again.".into());
        }
        patterns::render(
            &rule.pattern,
            NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(),
            rule.previous_month,
        )?;
        if let Some(template) = &rule.template {
            attachments::load(state, std::slice::from_ref(template))?;
        }
    }
    Ok(())
}
fn candidates(
    state: &AppState,
    rule: &ReportRule,
    due: &str,
) -> Result<(String, Vec<(String, PathBuf)>)> {
    let expected = patterns::render(&rule.pattern, day(due)?, rule.previous_month)?;
    let found = sources::files(state, &rule.folder.id)?
        .into_iter()
        .filter_map(|item| match patterns::matches(&expected, &item.0) {
            Ok(true) => Some(Ok(item)),
            Ok(false) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((expected, found))
}
pub fn inspect(state: &AppState, rule: &ReportRule, due: &str) -> Result<RuleCheck> {
    validate(state, std::slice::from_ref(rule), 0)?;
    let (expected, found) = candidates(state, rule, due)?;
    let status = if found.is_empty() {
        if rule.template.is_some() {
            "Missing — the scheduled run will create a template copy and stop for completion"
        } else {
            "Missing — email will not be sent"
        }
    } else if found.len() > 1 {
        "Ambiguous — more than one file matches; email will not be sent"
    } else if pending_path(state, &rule.folder.id, &found[0].0).exists() {
        "Template copy awaits completion and explicit retry"
    } else {
        "One matching file; latest bytes will be read and checked at execution"
    }
    .to_owned();
    let names = sources::files(state, &rule.folder.id)?
        .into_iter()
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    Ok(RuleCheck {
        expected,
        matches: found.into_iter().map(|(name, _)| name).collect(),
        status,
        suggestions: patterns::infer(&names),
    })
}
pub fn suggestions(state: &AppState, folder: &str) -> Result<Vec<patterns::Suggestion>> {
    Ok(patterns::infer(
        &sources::files(state, folder)?
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
    ))
}
fn make_missing(state: &AppState, rule: &ReportRule, name: &str) -> Result<()> {
    let template = rule.template.as_ref().ok_or_else(|| {
        format!("No file matches '{name}'. Create the report in the selected folder, then retry.")
    })?;
    if name.contains(['*', '?']) {
        return Err(format!("No file matches '{name}'. Template creation needs an exact filename pattern without * or ?."));
    }
    if Path::new(name)
        .extension()
        .and_then(|x| x.to_str())
        .map(str::to_lowercase)
        != Path::new(&template.name)
            .extension()
            .and_then(|x| x.to_str())
            .map(str::to_lowercase)
    {
        return Err("The template and expected report must have the same file extension".into());
    }
    let mut data = attachments::load(state, std::slice::from_ref(template))?;
    let bytes = data.remove(0).bytes;
    let root = sources::root(state, &rule.folder.id)?;
    let target = root.join(name);
    let record = Pending {
        folder_id: rule.folder.id.clone(),
        name: name.into(),
        initial_sha256: sha(&bytes),
    };
    // Persist the completion barrier before creating the file: a crash can never
    // leave an unmarked template that a subsequent run might mistake for a report.
    atomic_write(
        &pending_path(state, &rule.folder.id, name),
        &serde_json::to_vec(&record).map_err(|_| "Could not protect the new report")?,
    )?;
    let mut output=OpenOptions::new().write(true).create_new(true).open(&target).map_err(|_|format!("Could not create '{name}' without overwriting an existing file. Inspect the folder before retrying."))?;
    output
        .write_all(&bytes)
        .and_then(|_| output.sync_all())
        .map_err(|_| {
            format!(
                "Could not finish creating '{name}'. Complete or replace the file before retrying."
            )
        })?;
    Err(format!("Created '{name}' from your template. Complete and save it, then choose 'Report ready — retry' in this routine. Nothing was sent."))
}
pub fn prepare(
    state: &AppState,
    rules: &[ReportRule],
    due: &str,
    routine_id: &str,
    limit: u64,
) -> Result<Prepared> {
    let mut files = vec![];
    let mut receipts = vec![];
    let mut errors = vec![];
    let mut used = BTreeSet::new();
    let mut total = 0;
    for rule in rules {
        let result = (|| -> Result<()> {
            let (expected, found) = candidates(state, rule, due)?;
            if found.is_empty() {
                return make_missing(state, rule, &expected);
            }
            if found.len() != 1 {
                return Err(format!(
                    "{} files match '{expected}'. Use a more specific pattern; no file was chosen.",
                    found.len()
                ));
            }
            let (name, path) = &found[0];
            if !used.insert(path.clone()) {
                return Err(format!(
                    "'{name}' was selected by more than one report rule"
                ));
            }
            if pending_path(state, &rule.folder.id, name).exists() {
                return Err(format!("'{name}' is a template copy awaiting completion. Finish it and choose 'Report ready — retry'."));
            }
            let bytes = sources::read_stable(state, &rule.folder.id, name, limit - total)?;
            total += bytes.len() as u64;
            let hash = sha(&bytes);
            if rule.template.as_ref().is_some_and(|t| t.sha256 == hash) {
                return Err(format!("'{name}' is still identical to the template. Complete the report before retrying."));
            }
            if rule.require_changed {
                let store = state.lock()?;
                let reused = store
                    .runs
                    .iter()
                    .filter(|run| {
                        run.status == "succeeded"
                            && run.parameters.get("_routine_id").and_then(|v| v.as_str())
                                == Some(routine_id)
                    })
                    .filter_map(|run| {
                        run.parameters
                            .get("_resolved_reports")
                            .and_then(|v| v.as_str())
                    })
                    .filter_map(|value| serde_json::from_str::<Vec<ReportReceipt>>(value).ok())
                    .flatten()
                    .any(|old| old.rule_id == rule.id && old.sha256 == hash);
                if reused {
                    return Err(format!("'{name}' has exactly the same contents as a report already used by this routine. Update the report, or edit the rule if unchanged contents are intended."));
                }
            }
            receipts.push(ReportReceipt {
                rule_id: rule.id.clone(),
                folder_id: rule.folder.id.clone(),
                name: name.clone(),
                size: bytes.len() as u64,
                sha256: hash,
            });
            files.push(AttachmentData {
                name: name.clone(),
                bytes,
            });
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(error);
        }
    }
    if !errors.is_empty() {
        return Err(format!("{ISSUE_PREFIX}{}", errors.join("\n")));
    }
    Ok(Prepared { files, receipts })
}
/// Explicit human completion, never performed by the scheduler. File contents
/// must differ from the original template before its completion barrier is removed.
pub fn confirm_completed(state: &AppState, rules: &[ReportRule], due: &str) -> Result<()> {
    for rule in rules {
        let (_, found) = candidates(state, rule, due)?;
        for (name, _) in found {
            let marker = pending_path(state, &rule.folder.id, &name);
            if !marker.exists() {
                continue;
            }
            let bytes =
                fs::read(&marker).map_err(|_| "Could not read the report completion record")?;
            if bytes.len() > 8192 {
                return Err("Invalid report completion record".into());
            }
            let record: Pending =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid report completion record")?;
            if record.folder_id != rule.folder.id
                || record.name.to_lowercase() != name.to_lowercase()
            {
                return Err("Report completion record does not match this file".into());
            }
            let bytes =
                sources::read_stable(state, &rule.folder.id, &name, attachments::MAX_BYTES)?;
            if sha(&bytes) == record.initial_sha256 {
                return Err(format!("'{name}' is still identical to the template. Complete and save the report before retrying."));
            }
            fs::remove_file(marker).map_err(|_| "Could not confirm report completion")?;
        }
    }
    Ok(())
}
pub fn from_run(state: &AppState, run: &Run, limit: u64) -> Result<Prepared> {
    let rules = run
        .parameters
        .get("_attachment_rules")
        .and_then(|v| v.as_str())
        .map(serde_json::from_str::<Vec<ReportRule>>)
        .transpose()
        .map_err(|_| format!("{ISSUE_PREFIX}Invalid report rules"))?
        .unwrap_or_default();
    if rules.is_empty() {
        return Ok(Prepared {
            files: vec![],
            receipts: vec![],
        });
    }
    let text = |key: &str| {
        run.parameters
            .get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{ISSUE_PREFIX}Missing report occurrence"))
    };
    prepare(
        state,
        &rules,
        text("_attachment_due")?,
        text("_routine_id")?,
        limit,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, AppState, ReportRule) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().join("app")).unwrap();
        let folder = dir.path().join("reports");
        fs::create_dir(&folder).unwrap();
        let rule = ReportRule {
            id: uuid::Uuid::new_v4().to_string(),
            folder: sources::register(&state, &folder).unwrap(),
            pattern: "{{MONTH_SHORT}} report {{year}}.xlsx".into(),
            previous_month: false,
            template: None,
            require_changed: true,
        };
        (dir, state, rule)
    }
    #[test]
    fn resolves_current_period_and_fresh_bytes_without_reusing_earlier_months() {
        let (_dir, state, rule) = setup();
        let root = sources::root(&state, &rule.folder.id).unwrap();
        fs::write(root.join("OCT report 2026.xlsx"), b"October figures").unwrap();
        fs::write(root.join("NOV report 2026.xlsx"), b"November figures").unwrap();
        let result = prepare(
            &state,
            &[rule.clone()],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES,
        )
        .unwrap();
        assert_eq!(result.files[0].bytes, b"November figures");
        fs::write(
            root.join("NOV report 2026.xlsx"),
            b"Updated November figures",
        )
        .unwrap();
        assert_eq!(
            prepare(
                &state,
                &[rule],
                "2026-11-05",
                "routine",
                attachments::MAX_BYTES
            )
            .unwrap()
            .files[0]
                .bytes,
            b"Updated November figures"
        );
    }
    #[test]
    fn missing_report_creates_template_copy_but_never_becomes_ready_without_changed_contents_and_explicit_confirmation(
    ) {
        let (_dir, state, mut rule) = setup();
        let template = state.root.join("blank.xlsx");
        fs::write(&template, b"Template: enter this month's actual figures").unwrap();
        rule.template = Some(attachments::stage(&state, &[template]).unwrap().remove(0));
        let error = prepare(
            &state,
            &[rule.clone()],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES,
        )
        .err()
        .unwrap();
        assert!(error.starts_with(ISSUE_PREFIX));
        assert!(error.contains("Created 'NOV report 2026.xlsx'"));
        let root = sources::root(&state, &rule.folder.id).unwrap();
        let report = root.join("NOV report 2026.xlsx");
        let before = fs::read(&report).unwrap();
        assert!(confirm_completed(&state, &[rule.clone()], "2026-11-05").is_err());
        // Restart retains the completion barrier, even after the user has edited the file.
        fs::write(&report, b"Completed actual November figures").unwrap();
        let restored = AppState::initialize(state.root.clone()).unwrap();
        assert!(prepare(
            &restored,
            &[rule.clone()],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES
        )
        .is_err());
        confirm_completed(&restored, &[rule.clone()], "2026-11-05").unwrap();
        assert_eq!(
            prepare(
                &restored,
                &[rule.clone()],
                "2026-11-05",
                "routine",
                attachments::MAX_BYTES
            )
            .unwrap()
            .files[0]
                .bytes,
            b"Completed actual November figures"
        );
        assert_ne!(fs::read(&report).unwrap(), before);
    }
    #[test]
    fn ambiguous_matches_oversize_files_and_forged_folder_details_stop_resolution() {
        let (_dir, state, mut rule) = setup();
        let root = sources::root(&state, &rule.folder.id).unwrap();
        rule.pattern = "NOV*.xlsx".into();
        fs::write(root.join("NOV first.xlsx"), b"first").unwrap();
        fs::write(root.join("NOV second.xlsx"), b"second").unwrap();
        assert!(prepare(
            &state,
            &[rule.clone()],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES
        )
        .err()
        .unwrap()
        .contains("2 files match"));
        rule.pattern = "large.xlsx".into();
        fs::File::create(root.join("large.xlsx"))
            .unwrap()
            .set_len(attachments::MAX_BYTES + 1)
            .unwrap();
        assert!(prepare(
            &state,
            &[rule.clone()],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES
        )
        .is_err());
        rule.folder.path = state.root.to_string_lossy().into_owned();
        assert!(validate(&state, &[rule], 0).is_err());
    }
    #[test]
    fn existing_reports_are_not_overwritten_and_same_contents_are_rejected_after_a_successful_occurrence(
    ) {
        let (_dir, state, rule) = setup();
        let root = sources::root(&state, &rule.folder.id).unwrap();
        fs::write(root.join("NOV report 2026.xlsx"), b"Report contents").unwrap();
        let result = prepare(
            &state,
            &[rule.clone()],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES,
        )
        .unwrap();
        state.transaction(|store|{store.runs.push(serde_json::from_value(serde_json::json!({"id":"old","task_id":"foreground_email","title":"Report","status":"succeeded","started_at":"2026-10-05T00:00:00Z","parameters":{"_routine_id":"routine","_resolved_reports":serde_json::to_string(&result.receipts).unwrap()},"logs":[]})).unwrap());Ok(())}).unwrap();
        assert!(prepare(
            &state,
            &[rule.clone()],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES
        )
        .err()
        .unwrap()
        .contains("same contents"));
        assert_eq!(
            fs::read(root.join("NOV report 2026.xlsx")).unwrap(),
            b"Report contents"
        );
        let mut relaxed = rule;
        relaxed.require_changed = false;
        assert!(prepare(
            &state,
            &[relaxed],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES
        )
        .is_ok());
    }
    #[test]
    fn no_subfolder_or_temporary_files_are_chosen_and_previous_month_handles_year_boundary() {
        let (_dir, state, mut rule) = setup();
        let root = sources::root(&state, &rule.folder.id).unwrap();
        fs::create_dir(root.join("archive")).unwrap();
        fs::write(root.join("archive/DEC report 2026.xlsx"), b"Archived").unwrap();
        fs::write(root.join("~$DEC report 2026.xlsx"), b"Lock").unwrap();
        rule.previous_month = true;
        assert!(prepare(
            &state,
            &[rule.clone()],
            "2027-01-05",
            "routine",
            attachments::MAX_BYTES
        )
        .is_err());
        fs::write(root.join("DEC report 2026.xlsx"), b"Correct previous year").unwrap();
        assert_eq!(
            prepare(
                &state,
                &[rule],
                "2027-01-05",
                "routine",
                attachments::MAX_BYTES
            )
            .unwrap()
            .files[0]
                .name,
            "DEC report 2026.xlsx"
        );
    }
    #[test]
    fn an_existing_unmodified_template_copy_is_not_treated_as_a_completed_report() {
        let (_dir, state, mut rule) = setup();
        let path = state.root.join("blank.xlsx");
        fs::write(&path, b"Template placeholders").unwrap();
        rule.template = Some(attachments::stage(&state, &[path]).unwrap().remove(0));
        let folder = sources::root(&state, &rule.folder.id).unwrap();
        fs::write(
            folder.join("NOV report 2026.xlsx"),
            b"Template placeholders",
        )
        .unwrap();
        assert!(prepare(
            &state,
            &[rule],
            "2026-11-05",
            "routine",
            attachments::MAX_BYTES
        )
        .err()
        .unwrap()
        .contains("identical to the template"));
    }
}

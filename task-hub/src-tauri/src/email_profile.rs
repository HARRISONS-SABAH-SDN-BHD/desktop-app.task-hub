//! Product scope is separate from the reusable workflow engine and adapters.
use crate::{
    model::*,
    storage::{atomic_write, AppState},
    validation,
};

pub fn default_config() -> Result<Config> {
    validation::config(
        serde_json::from_str(include_str!("../../config/tasks.json")).map_err(|e| e.to_string())?,
    )
}
pub fn activate(state: &AppState) -> Result<()> {
    let path = state.root.join("tasks.json");
    let before = std::fs::read(&path).map_err(|e| e.to_string())?;
    let current: serde_json::Value = serde_json::from_slice(&before).map_err(|e| e.to_string())?;
    let desired = serde_json::to_value(default_config()?).map_err(|e| e.to_string())?;
    if current != desired {
        let backup = state.root.join("tasks.before-email-mvp.json");
        if !backup.exists() {
            atomic_write(&backup, &before)?;
        }
        state.save_config(desired)?;
    }
    state.transaction(|store| {
        // Retain the schedule records and all history, but stop legacy jobs.
        for schedule in &mut store.schedules {
            schedule.enabled = false;
        }
        Ok(())
    })
}
pub fn validate_request(state: &AppState, id: &str, values: &Parameters) -> Result<()> {
    if id != "foreground_email" {
        return Err("This prototype supports email automation only.".into());
    }
    let task = state
        .load_config()?
        .tasks
        .into_iter()
        .find(|task| task.id == id)
        .ok_or("Email workflow missing")?;
    if !matches!(&task.action, Action::Foreground { recipe, .. } if recipe == "web_email") {
        return Err("Only the foreground email adapter is available in this prototype.".into());
    }
    validation::parameters(&task, values)?;
    let fields = validation::resolve_fields(&task, values)?;
    if fields.get("provider").map(String::as_str) != Some("Gmail")
        || fields.get("browser").map(String::as_str) != Some("Google Chrome")
        || !matches!(
            fields.get("delivery").map(String::as_str),
            Some("Draft" | "Send after review")
        )
    {
        return Err(
            "The email MVP supports Gmail in Google Chrome, with Draft or Send after review."
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_rejects_legacy_tasks_and_direct_send() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        activate(&state).unwrap();
        let mut values: Parameters = serde_json::from_value(serde_json::json!({"recipient":"a@example.com","subject":"Hi","message":"Hello","delivery":"Draft"})).unwrap();
        validate_request(&state, "foreground_email", &values).unwrap();
        assert!(validate_request(&state, "foreground_word", &values).is_err());
        values.insert("delivery".into(), serde_json::json!("Send directly"));
        assert!(validate_request(&state, "foreground_email", &values).is_err());
    }
    #[test]
    fn upgrade_archives_config_and_disables_saved_schedules() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        let legacy: serde_json::Value =
            serde_json::from_str(include_str!("../../config/examples/library.tasks.json")).unwrap();
        state.save_config(legacy.clone()).unwrap();
        let old_task = state.load_config().unwrap().tasks[0].clone();
        state
            .transaction(|store| {
                store.schedules.push(Schedule {
                    id: "legacy".into(),
                    task: old_task,
                    parameters: Default::default(),
                    next_run: "2026-01-01T00:00:00Z".into(),
                    repeat: "daily".into(),
                    enabled: true,
                });
                Ok(())
            })
            .unwrap();
        activate(&state).unwrap();
        activate(&state).unwrap();
        assert_eq!(state.load_config().unwrap().tasks.len(), 1);
        assert!(!state.lock().unwrap().schedules[0].enabled);
        let archived: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.path().join("tasks.before-email-mvp.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(archived, legacy);
    }
}

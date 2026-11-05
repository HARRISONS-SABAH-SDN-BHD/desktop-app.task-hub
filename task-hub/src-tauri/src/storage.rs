use crate::{model::*, validation};
use chrono::Utc;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

pub struct AppState {
    pub root: PathBuf,
    pub output: PathBuf,
    pub store: Mutex<Store>,
    pub foreground: crate::foreground::Controller,
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("Invalid storage path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn read_limited(path: &Path, max: u64) -> Result<Vec<u8>> {
    let file =
        fs::File::open(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > max {
        return Err(format!("{} exceeds the storage limit", path.display()));
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err("Storage limit exceeded".into());
    }
    Ok(bytes)
}
impl AppState {
    pub fn initialize(root: PathBuf) -> Result<Self> {
        let output = root.join("outputs");
        fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        let config_file = root.join("tasks.json");
        if !config_file.exists() {
            atomic_write(&config_file, include_bytes!("../../config/tasks.json"))?;
        }
        if let Err(error) = migrate_foreground_config(&root) {
            // Keep the UI available to report an invalid config instead of failing startup.
            eprintln!("Foreground config migration skipped: {error}");
        }
        let store_path = root.join("state.json");
        let mut store: Store = if store_path.exists() {
            serde_json::from_slice(&read_limited(&store_path, 64 * 1024 * 1024)?)
                .map_err(|e| format!("state.json is invalid; preserve a copy and repair it: {e}"))?
        } else {
            Store::default()
        };
        for run in &mut store.runs {
            if run.status == "running" {
                run.status = "interrupted".into();
                run.finished_at = Some(Utc::now().to_rfc3339());
                run.error = Some("Application closed before the result was recorded. Inspect outputs before retrying.".into());
            }
        }
        let state = Self {
            root,
            output,
            store: Mutex::new(store),
            foreground: Default::default(),
        };
        {
            let store = state.lock()?;
            state.persist(&store)?;
        }
        Ok(state)
    }
    pub fn lock(&self) -> Result<MutexGuard<'_, Store>> {
        self.store
            .lock()
            .map_err(|_| "Storage lock is unavailable".into())
    }
    pub fn persist(&self, store: &Store) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("State exceeds 64 MB; archive history before continuing".into());
        }
        atomic_write(&self.root.join("state.json"), &bytes)
    }
    // Commit memory only after durable replacement succeeds.
    pub fn transaction<T>(&self, change: impl FnOnce(&mut Store) -> Result<T>) -> Result<T> {
        let mut store = self.lock()?;
        let mut next = store.clone();
        let result = change(&mut next)?;
        self.persist(&next)?;
        *store = next;
        Ok(result)
    }
    pub fn load_config(&self) -> Result<Config> {
        validation::config(
            serde_json::from_slice(&read_limited(&self.root.join("tasks.json"), 1024 * 1024)?)
                .map_err(|e| e.to_string())?,
        )
    }
    pub fn save_config(&self, value: serde_json::Value) -> Result<()> {
        validation::config(value.clone())?;
        let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Configuration exceeds 1 MB".into());
        }
        atomic_write(&self.root.join("tasks.json"), &bytes)
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let store = self.lock()?;
        Ok(Snapshot {
            calendar_links: store.calendar_links.clone(),
            runs: store.runs.clone(),
            schedules: store.schedules.clone(),
            output_dir: self.output.to_string_lossy().into_owned(),
            foreground: self.foreground.snapshot(),
            email_routines: store.email_routines.clone(),
        })
    }
}

fn migrate_foreground_config(root: &Path) -> Result<()> {
    let marker = root.join("foreground-v1.applied");
    if marker.exists() {
        return Ok(());
    }
    let path = root.join("tasks.json");
    let original = read_limited(&path, 1024 * 1024)?;
    let mut config =
        validation::config(serde_json::from_slice(&original).map_err(|e| e.to_string())?)?;
    let defaults: Config =
        serde_json::from_str(include_str!("../../config/tasks.json")).map_err(|e| e.to_string())?;
    let mut changed = false;
    for task in defaults
        .tasks
        .into_iter()
        .filter(|task| matches!(task.action, Action::Foreground { .. }))
    {
        if !config.tasks.iter().any(|t| t.id == task.id) {
            config.tasks.push(task);
            changed = true;
        }
    }
    if changed {
        let value = serde_json::to_value(config).map_err(|e| e.to_string())?;
        validation::config(value.clone())?;
        let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Config migration exceeds 1 MB; add foreground workflows manually".into());
        }
        if !root.join("tasks.before-foreground.json").exists() {
            atomic_write(&root.join("tasks.before-foreground.json"), &original)?;
        }
        atomic_write(&path, &bytes)?;
    }
    atomic_write(
        &marker,
        b"Foreground recipes added without replacing existing task IDs.\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreground_migration_preserves_customizations_and_runs_once() {
        let root = tempfile::tempdir().unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(include_str!("../../config/examples/library.tasks.json")).unwrap();
        value["tasks"]
            .as_array_mut()
            .unwrap()
            .retain(|task| task["action"]["type"] != "foreground");
        value["tasks"][0]["title"] = serde_json::json!("My custom email");
        atomic_write(
            &root.path().join("tasks.json"),
            &serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        let state = AppState::initialize(root.path().into()).unwrap();
        let mut config = state.load_config().unwrap();
        assert_eq!(config.tasks[0].title, "My custom email");
        assert_eq!(
            config
                .tasks
                .iter()
                .filter(|t| matches!(t.action, Action::Foreground { .. }))
                .count(),
            1
        );
        assert!(root.path().join("tasks.before-foreground.json").is_file());
        config.tasks.retain(|t| t.id != "foreground_email");
        state
            .save_config(serde_json::to_value(config).unwrap())
            .unwrap();
        drop(state);
        let state = AppState::initialize(root.path().into()).unwrap();
        assert!(!state
            .load_config()
            .unwrap()
            .tasks
            .iter()
            .any(|t| t.id == "foreground_email"));
    }
    #[test]
    fn atomic_replace_and_restart_recovery() {
        let root = tempfile::tempdir().unwrap();
        let state = AppState::initialize(root.path().into()).unwrap();
        state
            .transaction(|store| {
                store.runs.push(Run {
                    id: "test".into(),
                    task_id: "test".into(),
                    title: "Test".into(),
                    status: "running".into(),
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: None,
                    parameters: Default::default(),
                    logs: vec![],
                    artifact: None,
                    error: None,
                });
                Ok(())
            })
            .unwrap();
        drop(state);
        let state = AppState::initialize(root.path().into()).unwrap();
        assert_eq!(state.lock().unwrap().runs[0].status, "interrupted");
        let path = root.path().join("replace.txt");
        atomic_write(&path, b"one").unwrap();
        atomic_write(&path, b"two").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"two");
    }
}

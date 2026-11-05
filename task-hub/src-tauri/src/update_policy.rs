//! Update safety policy, independent of the installer and network transport.
use crate::{
    model::{Result, Store},
    storage::{atomic_write, AppState},
};
pub fn ensure_idle(store: &Store) -> Result<()> {
    if store.runs.iter().any(|run| run.status == "running") {
        return Err(
            "An email operation is running. Wait for it to finish before installing the update."
                .into(),
        );
    }
    Ok(())
}
pub fn validate_asset_url(url: &url::Url) -> Result<()> {
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Update downloads must use an HTTPS URL without embedded credentials.".into());
    }
    Ok(())
}
pub fn backup(state: &AppState, store: &Store) -> Result<()> {
    let path = state.root.join("update-backups").join(format!(
        "state-{}-{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        uuid::Uuid::new_v4().simple()
    ));
    atomic_write(
        &path,
        &serde_json::to_vec_pretty(store).map_err(|_| "Could not encode the update backup")?,
    )
    .map_err(|_| {
        "Could not back up local routines and history. The update was not installed.".into()
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_email_blocks_install_and_backup_leaves_original_state_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        let before = std::fs::read(state.root.join("state.json")).unwrap();
        let mut store = state.lock().unwrap().clone();
        ensure_idle(&store).unwrap();
        backup(&state, &store).unwrap();
        assert_eq!(
            std::fs::read(state.root.join("state.json")).unwrap(),
            before
        );
        let file = std::fs::read_dir(state.root.join("update-backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let restored: Store = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(restored).unwrap(),
            serde_json::to_value(&store).unwrap()
        );
        store.runs.push(serde_json::from_value(serde_json::json!({"id":"r","task_id":"foreground_email","title":"Email","status":"running","started_at":"2026-11-05T00:00:00Z","parameters":{},"logs":[]})).unwrap());
        assert!(ensure_idle(&store).is_err());
    }
    #[test]
    fn insecure_or_credential_bearing_update_urls_are_rejected() {
        for url in [
            "http://example.com/setup.exe",
            "file:///tmp/setup.exe",
            "https://user:secret@example.com/setup.exe",
        ] {
            assert!(validate_asset_url(&url::Url::parse(url).unwrap()).is_err());
        }
        validate_asset_url(
            &url::Url::parse(
                "https://github.com/devDaniel0330/task-hub/releases/download/v0.3.0/setup.exe",
            )
            .unwrap(),
        )
        .unwrap();
    }
}

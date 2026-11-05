//! Signed updates are isolated from mail delivery. Installation requires an explicit
//! click and holds the store and Gmail locks until the Windows installer takes over.
use crate::{gmail, model::Result, storage::AppState, update_policy};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
#[derive(Clone, Serialize)]
pub struct UpdateStatus {
    pub enabled: bool,
    pub current_version: String,
    pub phase: String,
    pub available_version: Option<String>,
    pub notes: Option<String>,
    pub last_checked: Option<String>,
    pub error: Option<String>,
}
struct Inner {
    status: UpdateStatus,
    ready: Option<(Update, Arc<Vec<u8>>)>,
}
pub struct UpdateState(Mutex<Inner>);
impl UpdateState {
    pub fn new(version: String) -> Self {
        Self(Mutex::new(Inner {
            status: UpdateStatus {
                enabled: !cfg!(debug_assertions),
                current_version: version,
                phase: "idle".into(),
                available_version: None,
                notes: None,
                last_checked: None,
                error: None,
            },
            ready: None,
        }))
    }
}
pub fn status(app: &AppHandle) -> Result<UpdateStatus> {
    Ok(app
        .state::<UpdateState>()
        .0
        .lock()
        .map_err(|_| "Update status unavailable")?
        .status
        .clone())
}
fn change(app: &AppHandle, edit: impl FnOnce(&mut Inner)) -> Result<()> {
    let state = app.state::<UpdateState>();
    let mut inner = state.0.lock().map_err(|_| "Update status unavailable")?;
    edit(&mut inner);
    Ok(())
}
pub async fn check(app: AppHandle) -> Result<()> {
    {
        let state = app.state::<UpdateState>();
        let mut inner = state.0.lock().map_err(|_| "Update status unavailable")?;
        if !inner.status.enabled {
            return Err("Automatic updates are available in the installed release, not the development preview.".into());
        }
        if ["checking", "downloading", "ready", "installing"].contains(&inner.status.phase.as_str())
        {
            return Ok(());
        }
        inner.status.phase = "checking".into();
        inner.status.error = None;
    }
    let outcome: Result<()> = async {
        let updater=app.updater_builder().timeout(Duration::from_secs(30)).build().map_err(|_| "Could not initialize the signed updater")?;
        let found=updater.check().await.map_err(|_| "Could not check for updates. The release may not be published yet, or the connection is unavailable. Email schedules continue normally.")?;
        change(&app, |inner| inner.status.last_checked=Some(chrono::Utc::now().to_rfc3339()))?;
        if let Some(update)=found {
            update_policy::validate_asset_url(&update.download_url)?;
            change(&app, |inner| { inner.status.phase="downloading".into(); inner.status.available_version=Some(update.version.clone()); inner.status.notes=update.body.clone(); })?;
            let bytes=update.download(|_,_| {}, || {}).await.map_err(|_| "The update could not be downloaded or its signature/version could not be verified. Nothing was installed; email schedules continue normally.")?;
            change(&app, |inner| { inner.ready=Some((update,Arc::new(bytes))); inner.status.phase="ready".into(); })?;
        } else { change(&app, |inner| inner.status.phase="idle".into())?; }
        Ok(())
    }.await;
    if let Err(error) = &outcome {
        let _ = change(&app, |inner| {
            inner.status.phase = "error".into();
            inner.status.error = Some(error.clone());
        });
    }
    outcome
}
pub fn install(app: AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let store = state
        .store
        .try_lock()
        .map_err(|_| "Task Hub is saving changes. Try installing again in a moment.")?;
    update_policy::ensure_idle(&store)?;
    if state.foreground.snapshot().is_some_and(|p| p.active) {
        return Err("Wait for the current email operation to finish.".into());
    }
    // Nonblocking and held through installation: no new send, OAuth, or calendar operation can start.
    let _google = gmail::lock_for_update()?;
    let (update, bytes) = {
        let updates = app.state::<UpdateState>();
        let inner = updates.0.lock().map_err(|_| "Update status unavailable")?;
        inner
            .ready
            .clone()
            .ok_or("No verified update is ready to install")?
    };
    update
        .verify(bytes.as_ref())
        .map_err(|_| "Update signature or version validation failed. Nothing was installed.")?;
    update_policy::backup(&state, &store)?;
    change(&app, |inner| inner.status.phase = "installing".into())?;
    if update.install(bytes.as_ref()).is_err() {
        change(&app, |inner| {
            inner.status.phase = "ready".into();
            inner.status.error=Some("The installer could not start. Your routines and email history were not changed. Close other Task Hub instances and try again.".into());
        })?;
        return Err("The installer could not start; your data has been preserved.".into());
    }
    Ok(())
}
pub fn start(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        loop {
            let _ = tauri::async_runtime::block_on(check(app.clone()));
            std::thread::sleep(Duration::from_secs(6 * 60 * 60));
        }
    });
}

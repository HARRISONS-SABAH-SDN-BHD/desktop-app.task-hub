// Legacy formats/recipe validation remain available for compatibility tests.
#[cfg_attr(not(test), allow(dead_code))]
mod adapters;
mod attachment_patterns;
mod attachment_rules;
mod attachment_sources;
mod attachments;
#[cfg(feature = "desktop")]
mod background;
mod email_profile;
mod email_routines;
mod excelflow;
#[cfg_attr(not(test), allow(dead_code))]
mod foreground;
mod gmail;
mod mail_sources;
mod mailserver;
mod model;
#[cfg(feature = "desktop")]
mod notifications;
mod recipients;
#[cfg(feature = "desktop")]
mod routine_scheduler;
mod runner;
mod runtime;
#[cfg(test)]
mod scheduler;
mod storage;
mod update_policy;
#[cfg(feature = "desktop")]
mod updates;
mod validation;

#[cfg(feature = "desktop")]
mod desktop {
    use super::*;
    use model::*;
    use storage::AppState;
    use tauri::{Manager, State};
    use tauri_plugin_opener::OpenerExt;

    #[tauri::command]
    fn list_recipients(state: State<'_, AppState>) -> Result<Vec<recipients::Contact>> {
        recipients::list(&state)
    }
    #[tauri::command]
    fn save_recipient(state: State<'_, AppState>, email: String, name: String) -> Result<()> {
        recipients::save(&state, email, name)
    }
    #[tauri::command]
    fn delete_recipient(state: State<'_, AppState>, email: String) -> Result<()> {
        recipients::remove(&state, email)
    }
    #[tauri::command]
    async fn scan_recipients(
        app: tauri::AppHandle,
        provider: String,
        sender: String,
    ) -> Result<usize> {
        tauri::async_runtime::spawn_blocking(move || {
            recipients::scan(&app.state::<AppState>(), &provider, &sender)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    fn update_status(app: tauri::AppHandle) -> Result<updates::UpdateStatus> {
        updates::status(&app)
    }
    #[tauri::command]
    async fn check_updates(app: tauri::AppHandle) -> Result<()> {
        updates::check(app).await
    }
    #[tauri::command]
    async fn install_update(app: tauri::AppHandle) -> Result<()> {
        tauri::async_runtime::spawn_blocking(move || updates::install(app))
            .await
            .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    fn load_config(state: State<'_, AppState>) -> Result<Config> {
        state.load_config()
    }
    #[tauri::command]
    fn save_config() -> Result<()> {
        Err("Configuration is managed by the email MVP profile.".into())
    }
    #[tauri::command]
    fn get_snapshot(state: State<'_, AppState>) -> Result<Snapshot> {
        state.snapshot()
    }
    #[tauri::command]
    async fn pick_email_attachments(
        app: tauri::AppHandle,
    ) -> Result<Vec<attachments::AttachmentRef>> {
        #[cfg(windows)]
        let owner = app
            .get_webview_window("main")
            .ok_or("Application window unavailable")?
            .hwnd()
            .map_err(|_| "Could not identify the attachment picker owner")?
            .0 as isize;
        #[cfg(not(windows))]
        let owner = 0;
        tauri::async_runtime::spawn_blocking(move || {
            attachments::pick(&app.state::<AppState>(), owner)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    fn runtime_status() -> Result<runtime::Progress> {
        runtime::progress()
    }
    #[tauri::command]
    fn cancel_report_processing() {
        runtime::cancel();
    }
    #[tauri::command]
    fn mailserver_status() -> Result<mailserver::Status> {
        mailserver::status()
    }
    #[tauri::command]
    async fn connect_mailserver(config: mailserver::Config, password: String) -> Result<()> {
        tauri::async_runtime::spawn_blocking(move || mailserver::connect(config, password))
            .await
            .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    fn disconnect_mailserver() -> Result<()> {
        mailserver::disconnect()
    }
    #[tauri::command]
    async fn browse_mail_source(
        app: tauri::AppHandle,
        provider: String,
        request: serde_json::Value,
    ) -> Result<serde_json::Value> {
        tauri::async_runtime::spawn_blocking(move || {
            mail_sources::browse(&app.state::<AppState>(), &provider, request)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn download_mail_workbook(
        app: tauri::AppHandle,
        selection: mail_sources::Download,
    ) -> Result<mail_sources::Input> {
        tauri::async_runtime::spawn_blocking(move || {
            mail_sources::download(&app.state::<AppState>(), selection)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn pick_excelflow_input(app: tauri::AppHandle) -> Result<Option<mail_sources::Input>> {
        #[cfg(windows)]
        let owner = app
            .get_webview_window("main")
            .ok_or("Window unavailable")?
            .hwnd()
            .map_err(|_| "Window unavailable")?
            .0 as isize;
        #[cfg(not(windows))]
        let owner = 0;
        tauri::async_runtime::spawn_blocking(move || {
            mail_sources::pick(&app.state::<AppState>(), owner)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn excelflow_recent_runs(app: tauri::AppHandle) -> Result<Vec<excelflow::Run>> {
        tauri::async_runtime::spawn_blocking(move || mail_sources::recent(&app.state::<AppState>()))
            .await
            .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn workbook_as_attachment(
        app: tauri::AppHandle,
        id: String,
    ) -> Result<Vec<attachments::AttachmentRef>> {
        tauri::async_runtime::spawn_blocking(move || {
            mail_sources::as_attachment(&app.state::<AppState>(), &id)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn generate_excelflow(
        app: tauri::AppHandle,
        request: mail_sources::Generate,
    ) -> Result<serde_json::Value> {
        tauri::async_runtime::spawn_blocking(move || {
            mail_sources::generate(&app.state::<AppState>(), request)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn excelflow_settings(
        app: tauri::AppHandle,
        agencies: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        tauri::async_runtime::spawn_blocking(move || {
            mail_sources::settings(&app.state::<AppState>(), agencies)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn inspect_excelflow_run(
        app: tauri::AppHandle,
        folder_id: String,
    ) -> Result<excelflow::Run> {
        tauri::async_runtime::spawn_blocking(move || {
            excelflow::inspect(&app.state::<AppState>(), &folder_id)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn import_excelflow_reports(
        app: tauri::AppHandle,
        folder_id: String,
        summary_hash: String,
        selected: Vec<excelflow::Selection>,
        existing: Vec<attachments::AttachmentRef>,
    ) -> Result<Vec<attachments::AttachmentRef>> {
        tauri::async_runtime::spawn_blocking(move || {
            excelflow::import(
                &app.state::<AppState>(),
                &folder_id,
                &summary_hash,
                selected,
                existing,
            )
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn pick_report_folder(
        app: tauri::AppHandle,
    ) -> Result<Option<attachment_sources::Folder>> {
        #[cfg(windows)]
        let owner = app
            .get_webview_window("main")
            .ok_or("Application window unavailable")?
            .hwnd()
            .map_err(|_| "Could not identify the report folder picker owner")?
            .0 as isize;
        #[cfg(not(windows))]
        let owner = 0;
        tauri::async_runtime::spawn_blocking(move || {
            attachment_sources::pick(&app.state::<AppState>(), owner)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    fn open_report_folder(
        app: tauri::AppHandle,
        state: State<'_, AppState>,
        folder_id: String,
    ) -> Result<()> {
        let path = attachment_sources::root(&state, &folder_id)?;
        app.opener()
            .open_path(path.to_string_lossy().as_ref(), None::<&str>)
            .map_err(|_| "Could not open the selected report folder".into())
    }
    #[tauri::command]
    async fn suggest_report_patterns(
        app: tauri::AppHandle,
        folder_id: String,
    ) -> Result<Vec<attachment_patterns::Suggestion>> {
        tauri::async_runtime::spawn_blocking(move || {
            attachment_rules::suggestions(&app.state::<AppState>(), &folder_id)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn inspect_report_rule(
        app: tauri::AppHandle,
        rule: attachment_rules::ReportRule,
        date: String,
    ) -> Result<attachment_rules::RuleCheck> {
        tauri::async_runtime::spawn_blocking(move || {
            attachment_rules::inspect(&app.state::<AppState>(), &rule, &date)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn retry_report_email(app: tauri::AppHandle, id: String, due: String) -> Result<Run> {
        tauri::async_runtime::spawn_blocking(move || {
            email_routines::retry_reports(&app.state::<AppState>(), &id, &due)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    fn stop_foreground(state: State<'_, AppState>, run_id: String) -> Result<()> {
        state.foreground.cancel(&run_id)
    }
    #[tauri::command]
    fn approve_foreground_send(state: State<'_, AppState>, run_id: String) -> Result<()> {
        state.foreground.approve(&run_id)
    }
    #[tauri::command]
    fn gmail_status(state: State<'_, AppState>) -> Result<gmail::ConnectionStatus> {
        gmail::status(&state)
    }
    #[tauri::command]
    async fn connect_gmail(
        app: tauri::AppHandle,
        client_id: String,
        client_secret: Option<String>,
        calendar_access: Option<bool>,
        read_access: Option<bool>,
    ) -> Result<()> {
        tauri::async_runtime::spawn_blocking(move || {
            gmail::connect(
                &app.state::<AppState>(),
                client_id,
                client_secret,
                calendar_access.unwrap_or(false),
                read_access.unwrap_or(false),
                |url| {
                    app.opener()
                        .open_url(url, None::<&str>)
                        .map_err(|_| "Could not open Google sign-in in your browser".into())
                },
            )
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    fn cancel_gmail_connection() {
        gmail::cancel_connect();
    }
    #[tauri::command]
    fn disconnect_gmail() -> Result<()> {
        gmail::disconnect()
    }
    #[tauri::command]
    fn open_gmail_setup(app: tauri::AppHandle, which: String) -> Result<()> {
        let url = match which.as_str() {
            "cloud" => "https://console.cloud.google.com/apis/library/gmail.googleapis.com",
            "calendar" => {
                "https://console.cloud.google.com/apis/library/calendar-json.googleapis.com"
            }
            "oauth" => "https://developers.google.com/identity/protocols/oauth2/native-app",
            _ => return Err("Unknown Gmail setup page".into()),
        };
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|_| "Could not open the Gmail setup page".into())
    }
    #[tauri::command]
    fn save_email_routine(
        state: State<'_, AppState>,
        id: Option<String>,
        settings: email_routines::RoutineSettings,
    ) -> Result<()> {
        email_routines::save(&state, id, settings)
    }
    #[tauri::command]
    fn preview_email_routine(
        state: State<'_, AppState>,
        id: String,
    ) -> Result<email_routines::RoutinePreview> {
        email_routines::get_preview(&state, &id)
    }
    #[tauri::command]
    fn handle_email_routine(state: State<'_, AppState>, id: String, due: String) -> Result<()> {
        email_routines::handle(&state, &id, &due)
    }
    #[tauri::command]
    fn delete_email_routine(state: State<'_, AppState>, id: String) -> Result<()> {
        email_routines::remove(&state, &id)
    }
    #[tauri::command]
    fn retry_calendar_sync(state: State<'_, AppState>, id: String) -> Result<()> {
        gmail::calendar::retry(&state, &id)
    }
    #[tauri::command]
    fn test_email_notification(app: tauri::AppHandle) -> Result<()> {
        notifications::show(
            &app,
            "Task Hub notification test",
            "Email routine reminders appear here. This test does not open Gmail or send an email.",
        )
    }
    #[tauri::command]
    async fn run_email_routine(
        app: tauri::AppHandle,
        id: String,
        due: String,
        parameters: Parameters,
    ) -> Result<Run> {
        tauri::async_runtime::spawn_blocking(move || {
            email_routines::run(&app.state::<AppState>(), &id, &due, parameters)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    async fn run_task(
        app: tauri::AppHandle,
        task_id: String,
        parameters: Parameters,
        allow_direct_send: Option<bool>,
        expected_sender: Option<String>,
        attachments: Option<Vec<attachments::AttachmentRef>>,
    ) -> Result<Run> {
        tauri::async_runtime::spawn_blocking(move || {
            let _ = allow_direct_send;
            gmail::manual(
                &app.state::<AppState>(),
                &task_id,
                parameters,
                expected_sender.as_deref().unwrap_or_default(),
                attachments.unwrap_or_default(),
            )
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[tauri::command]
    fn create_schedule(
        state: State<'_, AppState>,
        task_id: String,
        parameters: Parameters,
        next_run: String,
        repeat: String,
    ) -> Result<()> {
        let _ = (state, task_id, parameters, next_run, repeat);
        Err(
            "Scheduling is unavailable in the email MVP. Start email automation interactively."
                .into(),
        )
    }
    #[tauri::command]
    fn delete_schedule(state: State<'_, AppState>, id: String) -> Result<()> {
        state.transaction(|store| {
            store.schedules.retain(|s| s.id != id);
            Ok(())
        })
    }
    #[tauri::command]
    fn open_artifact(
        app: tauri::AppHandle,
        state: State<'_, AppState>,
        run_id: String,
    ) -> Result<()> {
        let path = {
            let store = state.lock()?;
            store
                .runs
                .iter()
                .find(|r| r.id == run_id)
                .and_then(|r| r.artifact.clone())
                .ok_or("Artifact not found")?
        };
        let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        let root = std::fs::canonicalize(&state.output).map_err(|e| e.to_string())?;
        if !path.starts_with(root) || !path.is_file() {
            return Err("Artifact is outside the output folder".into());
        }
        app.opener()
            .open_path(path.to_string_lossy().as_ref(), None::<&str>)
            .map_err(|e| e.to_string())
    }
    pub fn run() {
        tauri::Builder::default()
            .plugin(tauri_plugin_single_instance::init(|app, _, _| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }))
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_updater::Builder::new().build())
            .setup(|app| {
                runtime::initialize(app.path().resource_dir()?.join("runtime"));
                let root = app.path().app_data_dir()?;
                let state = AppState::initialize(root).map_err(std::io::Error::other)?;
                email_profile::activate(&state).map_err(std::io::Error::other)?;
                email_routines::activate_native_reminders(&state).map_err(std::io::Error::other)?;
                app.manage(state);
                app.manage(updates::UpdateState::new(
                    app.package_info().version.to_string(),
                ));
                background::setup(app.handle())?;
                routine_scheduler::start(app.handle().clone());
                gmail::calendar::start(app.handle().clone());
                updates::start(app.handle().clone());
                Ok(())
            })
            .on_window_event(background::on_window_event)
            .invoke_handler(tauri::generate_handler![
                list_recipients,
                save_recipient,
                delete_recipient,
                scan_recipients,
                update_status,
                check_updates,
                install_update,
                load_config,
                save_config,
                get_snapshot,
                run_task,
                create_schedule,
                delete_schedule,
                open_artifact,
                stop_foreground,
                approve_foreground_send,
                save_email_routine,
                preview_email_routine,
                handle_email_routine,
                delete_email_routine,
                run_email_routine,
                test_email_notification,
                retry_calendar_sync,
                gmail_status,
                connect_gmail,
                cancel_gmail_connection,
                disconnect_gmail,
                open_gmail_setup,
                runtime_status,
                cancel_report_processing,
                mailserver_status,
                connect_mailserver,
                disconnect_mailserver,
                browse_mail_source,
                download_mail_workbook,
                pick_excelflow_input,
                excelflow_recent_runs,
                workbook_as_attachment,
                generate_excelflow,
                excelflow_settings,
                inspect_excelflow_run,
                import_excelflow_reports,
                pick_report_folder,
                open_report_folder,
                suggest_report_patterns,
                inspect_report_rule,
                retry_report_email,
                pick_email_attachments
            ])
            .run(tauri::generate_context!())
            .expect("Could not start Task Hub");
    }
}
#[cfg(feature = "desktop")]
pub use desktop::run;

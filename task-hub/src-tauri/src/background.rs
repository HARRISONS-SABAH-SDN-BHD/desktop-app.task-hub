//! Window close hides the UI; only the tray Quit action stops the process.
use crate::{gmail, notifications, storage::AppState};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Window, WindowEvent,
};
fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Task Hub", true, None::<&str>)?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        "Quit Task Hub (stops schedules)",
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::with_id("task-hub")
        .tooltip("Task Hub — email schedules running")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app),
            "quit" => {
                let state = app.state::<AppState>();
                let busy = state.lock().map(|store| store.runs.iter().any(|r| r.status == "running")).unwrap_or(true)
                    || gmail::busy() || crate::runtime::progress().is_ok_and(|p|p.active);
                if busy {
                    show(app);
                    let _ = notifications::show(app, "Task Hub is finishing an operation", "Wait for the current report or email operation to finish, then choose Quit again.");
                } else { app.exit(0); }
            },
            _ => (),
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) { show(tray.app_handle()); }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        // Never hide unless a working tray exists, so the UI can be recovered.
        if window.app_handle().tray_by_id("task-hub").is_some() && window.hide().is_ok() {
            api.prevent_close();
        }
    }
}

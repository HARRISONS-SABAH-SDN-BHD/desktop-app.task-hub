//! Single scheduler worker: durable occurrence claims prevent duplicate sends.
use crate::{email_routines, gmail, notifications, storage::AppState};
use tauri::{AppHandle, Manager};

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        let state = app.state::<AppState>();
        match email_routines::automatic_tick(&state, gmail::ready()) {
            Ok(Some(run)) => {
                let (title, message) = if run.status == "succeeded" {
                    ("Scheduled email sent", "The mail provider accepted the scheduled email. See Task Hub’s email history for details.")
                } else if run
                    .error
                    .as_deref()
                    .is_some_and(|e| e.starts_with(crate::attachment_rules::ISSUE_PREFIX))
                {
                    ("Report needs your attention", "Email was not sent. Open Task Hub for the report filename and folder. Complete any template copies, save them, then choose Report ready — retry in the routine.")
                } else {
                    ("Scheduled email needs attention", "The scheduled attempt stopped and will not retry automatically. Check Task Hub history and the provider’s Sent folder before trying again.")
                };
                if let Err(error) = notifications::show(&app, title, message) {
                    eprintln!("Email notification: {error}");
                }
            }
            Ok(None) => (),
            Err(error) => {
                eprintln!("Scheduled email: {error}");
            }
        }
        if let Err(error) = notifications::deliver_due(&app, &state) {
            eprintln!("Email reminder: {error}");
        }
    });
}

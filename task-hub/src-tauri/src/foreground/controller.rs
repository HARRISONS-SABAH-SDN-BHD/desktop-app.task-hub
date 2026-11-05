use super::Draft;
use crate::model::Result;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
#[cfg(test)]
use std::{
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub run_id: String,
    pub label: String,
    pub phase: String,
    pub message: String,
    pub logs: Vec<String>,
    pub active: bool,
    pub review: Option<Draft>,
}
#[derive(Default)]
pub struct Controller {
    status: Mutex<Option<Progress>>,
    cancelled: AtomicBool,
    approved: AtomicBool,
}
impl Controller {
    pub fn snapshot(&self) -> Option<Progress> {
        self.status.lock().ok()?.clone()
    }
    pub fn begin(&self, run_id: &str, label: &str) -> Result<()> {
        let mut status = self
            .status
            .lock()
            .map_err(|_| "Foreground controller unavailable")?;
        if status.as_ref().is_some_and(|p| p.active) {
            return Err("Another foreground run is active".into());
        }
        self.cancelled.store(false, Ordering::SeqCst);
        self.approved.store(false, Ordering::SeqCst);
        *status = Some(Progress {
            run_id: run_id.into(),
            label: label.into(),
            phase: "running".into(),
            message: "Preparing a visible application window…".into(),
            logs: vec![],
            active: true,
            review: None,
        });
        Ok(())
    }
    pub fn log(&self, message: &str) {
        if let Ok(mut status) = self.status.lock() {
            if let Some(p) = status.as_mut() {
                p.message = message.into();
                p.logs.push(message.into());
                if p.logs.len() > 100 {
                    p.logs.remove(0);
                }
            }
        }
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err(
                "Automation stopped. Partial work is left open; nothing is retried automatically."
                    .into(),
            )
        } else {
            Ok(())
        }
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let status = self
            .status
            .lock()
            .map_err(|_| "Foreground controller unavailable")?;
        if !status.as_ref().is_some_and(|p| p.active && p.run_id == id) {
            return Err("No matching active automation".into());
        }
        self.cancelled.store(true, Ordering::SeqCst);
        Ok(())
    }
    pub fn approve(&self, id: &str) -> Result<()> {
        let mut status = self
            .status
            .lock()
            .map_err(|_| "Foreground controller unavailable")?;
        let p = status
            .as_mut()
            .filter(|p| p.active && p.run_id == id && p.phase == "awaiting_send")
            .ok_or("No matching send approval is pending")?;
        if self.cancelled.load(Ordering::SeqCst) {
            return Err("Automation was stopped".into());
        }
        p.phase = "resuming".into();
        self.approved.store(true, Ordering::SeqCst);
        Ok(())
    }
    #[cfg(test)]
    pub fn await_send(&self, draft: &Draft, timeout: Duration) -> Result<()> {
        self.approved.store(false, Ordering::SeqCst);
        {
            let mut status = self
                .status
                .lock()
                .map_err(|_| "Foreground controller unavailable")?;
            let p = status.as_mut().ok_or("No active automation")?;
            p.phase = "awaiting_send".into();
            p.review = Some(draft.clone());
            p.message = "Draft filled. Review the account and message in the browser, then approve sending in Task Hub.".into();
        }
        let start = Instant::now();
        loop {
            self.check()?;
            #[cfg(windows)]
            if super::windows::escape_pressed() {
                return Err("Automation stopped with Escape. The draft remains open.".into());
            }
            if self.approved.swap(false, Ordering::SeqCst) {
                if let Ok(mut status) = self.status.lock() {
                    if let Some(p) = status.as_mut() {
                        p.phase = "running".into();
                    }
                }
                return Ok(());
            }
            if start.elapsed() >= timeout {
                return Err(
                    "Send approval expired. The draft was left open and was not sent by Task Hub."
                        .into(),
                );
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    pub fn finish(&self, success: bool) {
        if let Ok(mut status) = self.status.lock() {
            if let Some(p) = status.as_mut() {
                p.active = false;
                p.phase = if success { "completed" } else { "stopped" }.into();
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approvals_are_bound_to_the_active_run_and_send_phase() {
        let c = Controller::default();
        c.begin("run-1", "Email").unwrap();
        assert!(c.approve("run-1").is_err());
        assert!(c.cancel("other").is_err());
        assert!(c.begin("run-2", "Other").is_err());
        c.cancel("run-1").unwrap();
        assert!(c.check().is_err());
        c.finish(false);
        c.begin("run-2", "Other").unwrap();
        assert!(c.check().is_ok());
    }
    #[test]
    fn timeout_never_grants_send_approval() {
        let c = Controller::default();
        c.begin("run-1", "Email").unwrap();
        let d = Draft {
            provider: "Gmail".into(),
            recipient: "a@example.com".into(),
            subject: "Hi".into(),
            body: "Hello".into(),
        };
        assert!(c.await_send(&d, Duration::ZERO).is_err());
    }
    #[test]
    fn send_approval_is_explicit_and_consumed_once() {
        let controller = std::sync::Arc::new(Controller::default());
        controller.begin("send-1", "Email").unwrap();
        let waiting = controller.clone();
        let worker = std::thread::spawn(move || {
            waiting.await_send(
                &Draft {
                    provider: "Gmail".into(),
                    recipient: "a@example.com".into(),
                    subject: "Hi".into(),
                    body: "Hello".into(),
                },
                Duration::from_secs(3),
            )
        });
        let start = Instant::now();
        while !controller
            .snapshot()
            .is_some_and(|p| p.phase == "awaiting_send")
        {
            assert!(start.elapsed() < Duration::from_secs(2));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(controller.approve("wrong-run").is_err());
        controller.approve("send-1").unwrap();
        assert!(controller.approve("send-1").is_err());
        worker.join().unwrap().unwrap();
    }
}

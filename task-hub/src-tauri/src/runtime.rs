//! Bounded JSON worker, bundled with the app. Never executes user-supplied commands.
use crate::model::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
static ROOT: OnceLock<PathBuf> = OnceLock::new();
static JOB: Mutex<()> = Mutex::new(());
static CANCEL: AtomicBool = AtomicBool::new(false);
static PROGRESS: Mutex<Progress> = Mutex::new(Progress {
    active: false,
    operation: String::new(),
    message: String::new(),
    percent: None,
});
#[derive(Clone, Serialize, Deserialize)]
pub struct Progress {
    pub active: bool,
    pub operation: String,
    pub message: String,
    pub percent: Option<f64>,
}
pub fn lock_for_update() -> Result<std::sync::MutexGuard<'static, ()>> {
    JOB.try_lock().map_err(|_|"Wait for report processing or the mail-source operation to finish before installing an update.".into())
}
pub fn initialize(path: PathBuf) {
    let _ = ROOT.set(path);
}
fn root() -> Result<PathBuf> {
    if let Some(path) = ROOT
        .get()
        .filter(|p| p.join("taskhub-runtime.exe").is_file())
    {
        return Ok(path.clone());
    }
    if cfg!(debug_assertions) {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime");
        if path.join("taskhub-runtime.exe").is_file() {
            return Ok(path);
        }
    }
    Err("The bundled ExcelFlow runtime is missing. Reinstall Task Hub; no separate ExcelFlow app folder is needed.".into())
}
pub fn progress() -> Result<Progress> {
    PROGRESS
        .lock()
        .map(|p| p.clone())
        .map_err(|_| "Worker status unavailable".into())
}
pub fn cancel() {
    if PROGRESS.lock().is_ok_and(|p| p.operation == "generate") {
        CANCEL.store(true, Ordering::SeqCst);
    }
}
pub fn request(value: Value) -> Result<Value> {
    let _job = JOB.try_lock().map_err(|_| {
        "Another report or mail-source operation is running. Wait for it to finish."
    })?;
    CANCEL.store(false, Ordering::SeqCst);
    let operation = value
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if let Ok(mut p) = PROGRESS.lock() {
        *p = Progress {
            active: true,
            operation: operation.clone(),
            message: "Starting…".into(),
            percent: None,
        };
    }
    let result = root().and_then(|root| execute(&root, value, &operation));
    if let Ok(mut p) = PROGRESS.lock() {
        p.active = false;
        p.message = if result.is_ok() {
            "Complete"
        } else {
            "Stopped"
        }
        .into();
    }
    result
}
fn execute(root: &Path, value: Value, operation: &str) -> Result<Value> {
    let bytes = serde_json::to_vec(&value).map_err(|_| "Could not prepare worker request")?;
    if bytes.len() > 40 * 1024 * 1024 {
        return Err("Worker request exceeds its size limit".into());
    }
    let mut command = Command::new(root.join("taskhub-runtime.exe"));
    command
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Could not start Task Hub's bundled runtime")?;
    let mut stdin = child.stdin.take().ok_or("Worker input unavailable")?;
    let writer = std::thread::spawn(move || {
        let result = stdin.write_all(&bytes);
        drop(stdin);
        result
    });
    let stdout = child.stdout.take().ok_or("Worker output unavailable")?;
    let reader = std::thread::spawn(move || -> Result<Value> {
        let mut reader = BufReader::new(stdout);
        let mut final_result = None;
        let mut total = 0;
        loop {
            let mut line = Vec::new();
            let count = reader
                .by_ref()
                .take(10 * 1024 * 1024 + 1)
                .read_until(b'\n', &mut line)
                .map_err(|_| "Could not read worker result")?;
            if count == 0 {
                break;
            }
            total += count;
            if total > 20 * 1024 * 1024 || count > 10 * 1024 * 1024 {
                return Err("Worker output exceeded its limit".into());
            }
            let event: Value =
                serde_json::from_slice(&line).map_err(|_| "Worker returned an invalid response")?;
            match event["type"].as_str() {
                Some("progress") => {
                    if let Ok(mut p) = PROGRESS.lock() {
                        p.message = event["message"]
                            .as_str()
                            .unwrap_or("Processing…")
                            .chars()
                            .take(200)
                            .collect();
                        p.percent = event["percent"].as_f64().map(|v| v.clamp(0., 100.));
                    }
                }
                Some("result") => final_result = Some(Ok(event["data"].clone())),
                Some("error") => {
                    final_result = Some(Err(event["message"]
                        .as_str()
                        .unwrap_or("Operation stopped")
                        .to_owned()))
                }
                _ => return Err("Unknown worker response".into()),
            }
        }
        final_result.unwrap_or(Err(
            "The worker stopped without confirming the result".into()
        ))
    });
    let stderr = child
        .stderr
        .take()
        .ok_or("Worker error channel unavailable")?;
    let errors = std::thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stderr.take(1024 * 1024).read_to_end(&mut sink);
    });
    let started = Instant::now();
    let timeout = if operation == "generate" { 1800 } else { 300 };
    let mut stopped = None;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => {
                stopped = Some("Worker status could not be checked");
                break;
            }
        }
        if CANCEL.load(Ordering::SeqCst) {
            stopped = Some("Report processing cancelled. No email was sent.");
            break;
        }
        if started.elapsed() > Duration::from_secs(timeout) {
            stopped=Some("Operation timed out. For an email submission, inspect the provider before retrying; no retry was attempted.");
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if stopped.is_some() {
        #[cfg(windows)]
        {
            let mut stop = Command::new(
                std::path::PathBuf::from(
                    std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
                )
                .join("System32/taskkill.exe"),
            );
            stop.args(["/PID", &child.id().to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            use std::os::windows::process::CommandExt;
            stop.creation_flags(0x08000000);
            let _ = stop.status();
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = writer.join();
    let _ = errors.join();
    let output = reader.join().map_err(|_| "Worker response thread failed")?;
    if let Some(message) = stopped {
        return Err(message.into());
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_worker_protocol_responds_without_mail_access() {
        let result = request(serde_json::json!({"operation":"ping"})).unwrap();
        assert_eq!(result["version"], 1);
        assert!(!progress().unwrap().active);
    }
}

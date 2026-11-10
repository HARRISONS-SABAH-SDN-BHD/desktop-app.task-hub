//! Outlook Classic Object Model adapter. Uses the current Windows profile, including PST stores.
use crate::{
    model::Result,
    storage::{atomic_write, AppState},
};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};
const SCRIPT: &str = include_str!("../../integrations/outlook-classic/bridge.ps1");
pub fn status(state: &AppState) -> bool {
    state.root.join("outlook-classic-connected.json").is_file()
}
pub fn disconnect(state: &AppState) -> Result<()> {
    match std::fs::remove_file(state.root.join("outlook-classic-connected.json")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Could not disconnect Outlook Classic".into()),
    }
}
pub fn request(state: &AppState, request: Value) -> Result<Value> {
    if serde_json::to_vec(&request)
        .map_err(|_| "Invalid Outlook request")?
        .len()
        > 64 * 1024
    {
        return Err("Outlook request is too large".into());
    }
    let operation = request["operation"]
        .as_str()
        .ok_or("Missing Outlook operation")?;
    if operation != "connect" && !status(state) {
        return Err("Connect Outlook Classic first".into());
    }
    let script = state.root.join("integrations/outlook-classic.ps1");
    atomic_write(&script, SCRIPT.as_bytes())?;
    let system = std::env::var_os("SystemRoot").ok_or("Windows system directory unavailable")?;
    let mut command = Command::new(
        std::path::PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    );
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Could not start Outlook Classic integration")?;
    let bytes = serde_json::to_vec(&request).map_err(|_| "Invalid Outlook request")?;
    let mut stdin = child.stdin.take().ok_or("Outlook input unavailable")?;
    stdin
        .write_all(&bytes)
        .map_err(|_| "Could not send Outlook request")?;
    drop(stdin);
    let stdout = child.stdout.take().ok_or("Outlook response unavailable")?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut bytes = Vec::new();
        let result = stdout.take(10 * 1024 * 1024 + 1).read_to_end(&mut bytes);
        (result, bytes)
    });
    let started = std::time::Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|_| "Could not check Outlook request")?
            .is_some()
        {
            break;
        }
        if started.elapsed() > std::time::Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Outlook Classic timed out. Respond to any profile or access prompt in Outlook and try again.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let (read, bytes) = reader.join().map_err(|_| "Outlook response failed")?;
    read.map_err(|_| "Could not read Outlook response")?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err("Outlook response exceeded its limit".into());
    }
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}').trim();
    let value: Value =
        serde_json::from_str(text).map_err(|_| "Outlook Classic returned an unreadable result")?;
    if value["type"] != "result" {
        return Err(value["message"]
            .as_str()
            .unwrap_or("Outlook Classic request failed")
            .into());
    }
    if operation == "connect" {
        atomic_write(
            &state.root.join("outlook-classic-connected.json"),
            b"{\"connected\":true}",
        )?;
    }
    Ok(value["data"].clone())
}

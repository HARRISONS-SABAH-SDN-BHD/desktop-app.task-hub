use crate::model::*;
use std::{
    io::{Read, Seek, SeekFrom},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn run(script_id: &str, parameters: &Parameters) -> Result<(String, Vec<String>)> {
    if script_id != "system-info" {
        return Err("Script is not registered".into());
    }
    let mut script_file = tempfile::Builder::new()
        .suffix(if cfg!(windows) { ".ps1" } else { ".sh" })
        .tempfile()
        .map_err(|e| e.to_string())?;
    use std::io::Write;
    let source = if cfg!(windows) {
        include_str!("../../scripts/system-info.ps1")
    } else {
        include_str!("../../scripts/system-info.sh")
    };
    script_file
        .write_all(source.as_bytes())
        .map_err(|e| e.to_string())?;
    script_file.flush().map_err(|e| e.to_string())?;
    let stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
    let stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut command = if cfg!(windows) {
        let mut c = Command::new("powershell.exe");
        c.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "RemoteSigned",
            "-File",
        ]);
        c
    } else {
        Command::new("sh")
    };
    command
        .arg(script_file.path())
        .env(
            "TASK_HUB_PARAMETERS",
            serde_json::to_string(parameters).map_err(|e| e.to_string())?,
        )
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().map_err(|e| e.to_string())?)
        .stderr(stderr.try_clone().map_err(|e| e.to_string())?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Could not start registered script: {e}"))?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Script timed out after 30 seconds".into());
        }
        if stdout.metadata().map_err(|e| e.to_string())?.len()
            + stderr.metadata().map_err(|e| e.to_string())?.len()
            > 1024 * 1024
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Script exceeded the 1 MB output limit".into());
        }
        thread::sleep(Duration::from_millis(50));
    };
    fn read(mut file: std::fs::File) -> Result<String> {
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(65536)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
    let out = read(stdout)?;
    let err = read(stderr)?;
    if !status.success() {
        return Err(format!(
            "Script exited with {status}: {}",
            err.chars().take(1000).collect::<String>()
        ));
    }
    Ok((
        out.clone(),
        vec![
            "Registered script: system-info".into(),
            out,
            format!("Exit code: 0 ({:.2}s)", start.elapsed().as_secs_f64()),
        ],
    ))
}

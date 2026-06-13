use std::path::PathBuf;
use std::process::Command;

use crate::{
    config::Config,
    ipc::{self, Request, Response, StatusReply},
};
use anyhow::Result;

/// Spawn frescod detached from the GUI process so it outlives it.
/// The daemon binary is responsible for its own daemonization.
pub fn spawn_daemon() -> Result<()> {
    let frescod = frescod_path();
    Command::new(&frescod)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

fn frescod_path() -> PathBuf {
    // Look next to the current executable first (installed .deb), then PATH.
    if let Ok(mut path) = std::env::current_exe() {
        path.set_file_name("frescod");
        if path.exists() {
            return path;
        }
    }
    PathBuf::from("frescod")
}

/// Apply config, starting the daemon first if not running.
pub fn ensure_daemon_and_apply(config: &Config) -> Result<()> {
    config.save()?;

    if ipc::daemon_alive() {
        let resp = ipc::request(&Request::Apply)?;
        if let Response::Err { message } = resp {
            return Err(anyhow::anyhow!("daemon error: {message}"));
        }
    } else {
        spawn_daemon()?;
        // Give the daemon a moment to bind its socket.
        std::thread::sleep(std::time::Duration::from_millis(800));
        if !ipc::daemon_alive() {
            return Err(anyhow::anyhow!(
                "daemon failed to start — check ~/.local/state/fresco/frescod.log"
            ));
        }
    }
    Ok(())
}

pub fn stop_daemon() -> Result<()> {
    if ipc::daemon_alive() {
        ipc::request(&Request::Stop)?;
    }
    Ok(())
}

pub fn pause_daemon() -> Result<()> {
    if ipc::daemon_alive() {
        ipc::request(&Request::Pause)?;
    }
    Ok(())
}

pub fn resume_daemon() -> Result<()> {
    if ipc::daemon_alive() {
        ipc::request(&Request::Resume)?;
    }
    Ok(())
}

/// Poll status; returns None if the daemon is not running.
pub fn get_status() -> Option<StatusReply> {
    match ipc::request(&Request::Status) {
        Ok(Response::Status(s)) if s.running => Some(s),
        _ => None,
    }
}

/// Build a human-readable status line for the status bar.
pub fn status_line(status: Option<&StatusReply>) -> String {
    let Some(s) = status else {
        return "Wallpaper stopped".to_string();
    };
    if s.paused {
        return "Paused".to_string();
    }
    let decode = match s.hwdec.as_deref() {
        Some("no") | None => "Software decode ⚠".to_string(),
        Some(h) => format!("GPU decode: {h} ✓"),
    };
    let cpu = if s.cpu_percent > 0.1 {
        format!(" · CPU {:.1}%", s.cpu_percent)
    } else {
        String::new()
    };
    let ram = if s.rss_mb > 0 {
        format!(" · RAM {} MB", s.rss_mb)
    } else {
        String::new()
    };
    format!("{decode}{cpu}{ram}")
}

/// If hardware decode is not active, return a hint string.
pub fn hwdec_hint(status: Option<&StatusReply>) -> Option<String> {
    let s = status?;
    if matches!(s.hwdec.as_deref(), Some("no") | None) {
        Some(
            "Install intel-media-va-driver or mesa-va-drivers to enable hardware decode"
                .to_string(),
        )
    } else {
        None
    }
}

use std::path::PathBuf;
use std::process::Command;

use crate::{
    config::Config,
    ipc::{self, Request, Response},
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

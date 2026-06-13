use std::path::PathBuf;

use anyhow::Result;

use crate::{is_flatpak, APP_ID};

/// Directory the login session reads autostart entries from.
///
/// In a Flatpak sandbox `XDG_CONFIG_HOME` points inside the sandbox, but the
/// host session reads `~/.config/autostart` — and the manifest grants access to
/// it via `--filesystem=xdg-config/autostart:create` — so we target the host
/// path explicitly there.
fn autostart_dir() -> PathBuf {
    if is_flatpak() {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("autostart")
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("autostart")
    }
}

fn autostart_path() -> PathBuf {
    autostart_dir().join(format!("{APP_ID}.desktop"))
}

/// Command the autostart entry runs to restore the wallpaper on login.
/// Inside Flatpak the daemon must be launched as its own sandbox instance.
fn exec_command() -> String {
    if is_flatpak() {
        format!("flatpak run --command=frescod {APP_ID}")
    } else {
        "frescod".to_string()
    }
}

/// Install the login-restore entry. Delay lets desktop-icon extensions
/// (ding) map their windows first; the daemon's restack watchdog covers races.
pub fn enable() -> Result<()> {
    let path = autostart_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let content = format!(
        "[Desktop Entry]\n\
        Type=Application\n\
        Name=Fresco Wallpaper\n\
        Comment=Restores your live wallpaper on login\n\
        Exec={}\n\
        X-GNOME-Autostart-Delay=3\n\
        NoDisplay=true\n\
        Terminal=false\n",
        exec_command()
    );
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn disable() -> Result<()> {
    let path = autostart_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn is_enabled() -> bool {
    autostart_path().exists()
}

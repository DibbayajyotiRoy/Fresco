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
        // Use an absolute path: the login session's PATH often does not include
        // where frescod lives (e.g. ~/.cargo/bin), so a bare `frescod` would
        // silently fail to start at boot. Fall back to the bare name only if we
        // can't resolve our own location.
        frescod_abs_path().unwrap_or_else(|| "frescod".to_string())
    }
}

/// Absolute path to the `frescod` binary, resolved from the running executable —
/// `frescod` itself when the daemon writes the entry, or its sibling when the GUI
/// does. Returns `None` if neither can be found.
fn frescod_abs_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    if exe.file_name().map(|n| n == "frescod").unwrap_or(false) {
        return Some(exe.to_string_lossy().into_owned());
    }
    let sibling = exe.parent()?.join("frescod");
    sibling
        .is_file()
        .then(|| sibling.to_string_lossy().into_owned())
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

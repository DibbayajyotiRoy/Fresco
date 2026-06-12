use std::path::PathBuf;

use anyhow::Result;

use crate::APP_ID;

fn autostart_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autostart")
        .join(format!("{APP_ID}.desktop"))
}

/// Install the login-restore entry. Delay lets desktop-icon extensions
/// (ding) map their windows first; the daemon's restack watchdog covers races.
pub fn enable() -> Result<()> {
    let path = autostart_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let content = "[Desktop Entry]\n\
        Type=Application\n\
        Name=Fresco Wallpaper\n\
        Comment=Restores your live wallpaper on login\n\
        Exec=frescod\n\
        X-GNOME-Autostart-Delay=3\n\
        NoDisplay=true\n\
        Terminal=false\n";
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

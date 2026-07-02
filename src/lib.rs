pub mod autostart;
pub mod capability;
pub mod cli;
pub mod config;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(feature = "gui")]
pub mod gui;
pub mod ipc;
#[cfg(feature = "gui")]
pub mod supabase;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod update;

/// Application ID used for the desktop file, autostart entry, and GTK app.
pub const APP_ID: &str = "io.github.dibbayajyotiroy.Fresco";
pub const APP_NAME: &str = "Fresco";
/// WM_CLASS of wallpaper windows, so users/extensions can target them.
pub const WALLPAPER_WM_CLASS: &str = "fresco-wallpaper";

/// True when running inside a Flatpak sandbox. Several host-facing paths
/// (autostart, the daemon launch command) differ in that case.
pub fn is_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// The `mpvpaper` command Fresco should run on Wayland. We ship mpvpaper
/// **bundled** so users never install it themselves: prefer an explicit
/// override, then a copy next to our own executable (Flatpak `/app/bin`, or a
/// packaged layout), and only fall back to `mpvpaper` on PATH.
/// Absolute locations to look for the **bundled** mpvpaper, in priority order.
/// Fresco ships it under `<prefix>/lib/fresco/mpvpaper` (e.g. `/usr/lib/fresco`
/// from the .deb, `/app/lib/fresco` in Flatpak) so it never collides with a
/// user-installed `/usr/bin/mpvpaper`.
fn mpvpaper_candidates() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Beside our own binary (dev tree / `cargo install`).
            v.push(dir.join("mpvpaper"));
            // Prefix-relative libexec: /usr/bin → /usr/lib/fresco,
            // /app/bin → /app/lib/fresco, /usr/local/bin → /usr/local/lib/fresco.
            v.push(dir.join("../lib/fresco/mpvpaper"));
        }
    }
    // Absolute safety nets if current_exe() is unavailable.
    v.push(std::path::PathBuf::from("/usr/lib/fresco/mpvpaper"));
    v.push(std::path::PathBuf::from("/app/lib/fresco/mpvpaper"));
    v
}

/// The `mpvpaper` command Fresco runs on Wayland. We ship mpvpaper **bundled**
/// so users never install it: an explicit `FRESCO_MPVPAPER` override wins, then
/// the bundled copies, and only as a last resort `mpvpaper` from `PATH`.
pub fn mpvpaper_command() -> std::ffi::OsString {
    if let Some(p) = std::env::var_os("FRESCO_MPVPAPER") {
        return p;
    }
    for cand in mpvpaper_candidates() {
        if cand.is_file() {
            return cand.into_os_string();
        }
    }
    std::ffi::OsString::from("mpvpaper")
}

/// The resolved mpvpaper path if it actually exists (override, bundled, or on
/// PATH). Used by `fresco doctor` to report availability. `None` when not found.
pub fn mpvpaper_resolved() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("FRESCO_MPVPAPER") {
        let p = std::path::PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    if let Some(found) = mpvpaper_candidates().into_iter().find(|c| c.is_file()) {
        return Some(found);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("mpvpaper"))
            .find(|cand| cand.is_file())
    })
}

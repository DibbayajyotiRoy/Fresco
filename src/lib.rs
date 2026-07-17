pub mod autostart;
pub mod capability;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod catalog;
pub mod cli;
pub mod config;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod download;
#[cfg(feature = "gui")]
pub mod gui;
pub mod ipc;
pub mod schedule;
#[cfg(any(feature = "gui", feature = "daemon"))]
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
/// Bundled mpvpaper basenames to try in each location, best first. Fresco ships
/// one build per libmpv soname generation — `mpvpaper-libmpv2` for distros with
/// mpv ≥ 0.35 (Ubuntu 24.04+, Fedora 38+, Arch) and `mpvpaper-libmpv1` for
/// older LTS bases (Ubuntu 22.04, Debian 12). Plain `mpvpaper` is the legacy
/// single-build name from pre-1.1.1 packages and dev trees.
const MPVPAPER_VARIANTS: [&str; 3] = ["mpvpaper-libmpv2", "mpvpaper-libmpv1", "mpvpaper"];

fn mpvpaper_candidates() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Beside our own binary (dev tree / `cargo install`).
            dirs.push(dir.to_path_buf());
            // Prefix-relative libexec: /usr/bin → /usr/lib/fresco,
            // /app/bin → /app/lib/fresco, /usr/local/bin → /usr/local/lib/fresco.
            dirs.push(dir.join("../lib/fresco"));
        }
    }
    // Absolute safety nets if current_exe() is unavailable.
    dirs.push(std::path::PathBuf::from("/usr/lib/fresco"));
    dirs.push(std::path::PathBuf::from("/app/lib/fresco"));
    dirs.iter()
        .flat_map(|d| MPVPAPER_VARIANTS.iter().map(|v| d.join(v)))
        .collect()
}

/// Whether this mpvpaper binary can actually run here. A build linked against a
/// libmpv soname the OS doesn't ship execs fine but dies in the dynamic linker
/// with exit 127, so a plain "file exists" check is not enough — that's exactly
/// the failure mode behind "renderer failed 5×" on distros whose libmpv
/// generation differs from the build host's.
pub fn mpvpaper_runnable(path: &std::path::Path) -> bool {
    std::process::Command::new(path)
        .arg("--help")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.code() != Some(127))
        .unwrap_or(false)
}

/// First bundled candidate that exists AND loads on this system. Probed once
/// per process (each probe spawns the binary with `--help`).
fn mpvpaper_bundled_runnable() -> Option<&'static std::path::Path> {
    static FOUND: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    FOUND
        .get_or_init(|| {
            mpvpaper_candidates()
                .into_iter()
                .find(|c| c.is_file() && mpvpaper_runnable(c))
        })
        .as_deref()
}

/// A bundled mpvpaper that exists but cannot load (e.g. built against a libmpv
/// soname this OS doesn't ship). Only reported when NO runnable copy exists;
/// used by `fresco doctor` to explain why rendering fails despite the file
/// being present.
pub fn mpvpaper_broken() -> Option<std::path::PathBuf> {
    if mpvpaper_bundled_runnable().is_some() {
        return None;
    }
    mpvpaper_candidates().into_iter().find(|c| c.is_file())
}

/// The `mpvpaper` command Fresco runs on Wayland. We ship mpvpaper **bundled**
/// so users never install it: an explicit `FRESCO_MPVPAPER` override wins, then
/// the first bundled copy that actually loads on this OS, and only as a last
/// resort `mpvpaper` from `PATH`.
pub fn mpvpaper_command() -> std::ffi::OsString {
    if let Some(p) = std::env::var_os("FRESCO_MPVPAPER") {
        return p;
    }
    if let Some(found) = mpvpaper_bundled_runnable() {
        return found.as_os_str().to_os_string();
    }
    std::ffi::OsString::from("mpvpaper")
}

/// The resolved mpvpaper path if it exists and loads (override, bundled, or on
/// PATH). Used by `fresco doctor` to report availability. `None` when not found
/// or when every found copy fails to load (see [`mpvpaper_broken`]).
pub fn mpvpaper_resolved() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("FRESCO_MPVPAPER") {
        let p = std::path::PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    if let Some(found) = mpvpaper_bundled_runnable() {
        return Some(found.to_path_buf());
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("mpvpaper"))
            .find(|cand| cand.is_file() && mpvpaper_runnable(cand))
    })
}

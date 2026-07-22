//! Deepin DDE (X11) quirks — GitHub issue #2.
//!
//! DDE's `dde-shell` paints its own opaque desktop window (WM_CLASS
//! "dde-shell"/"desktop", also `_NET_WM_WINDOW_TYPE_DESKTOP`) that stacks
//! above other DESKTOP-type windows, hiding Fresco's wallpaper entirely.
//!
//! Fix (fantascene-style), additive to the unchanged X11 backend:
//!  1. Primary: make DDE's own wallpaper transparent over DBus (session
//!     Appearance service), after persisting the user's current wallpaper to
//!     the fresco state dir so it can be restored on shutdown — or on a later
//!     startup after a crash.
//!  2. Fallback (DBus unavailable): restack our wallpaper windows directly
//!     ABOVE dde-shell's desktop window. This hides DDE's desktop icons, so
//!     it is only a last resort and logged loudly.
//!
//! No DBus crate: we shell out to `gdbus` (ships with glib on Deepin) and
//! parse its output leniently.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;

use super::x11win::Atoms;

/// 1x1 fully-transparent RGBA PNG, written to the state dir at runtime and
/// handed to DDE as a `file://` wallpaper URI.
const TRANSPARENT_PNG: [u8; 68] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x00, 0x02, 0x00,
    0x00, 0x05, 0x00, 0x01, 0x7a, 0x5e, 0xab, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

/// (dest, object path, interface) for DDE's session Appearance service.
/// Deepin 25 first, then the legacy pre-25 names.
const SERVICES: [(&str, &str, &str); 2] = [
    (
        "org.deepin.dde.Appearance1",
        "/org/deepin/dde/Appearance1",
        "org.deepin.dde.Appearance1",
    ),
    (
        "com.deepin.daemon.Appearance",
        "/com/deepin/daemon/Appearance",
        "com.deepin.daemon.Appearance",
    ),
];

/// How the DDE quirk is currently active on this daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Not on DDE / nothing applied.
    #[default]
    Inactive,
    /// DDE's wallpaper was made transparent over DBus; ours shows through.
    DBus,
    /// DBus failed — our windows are restacked above dde-shell's desktop.
    Restack,
}

/// The user's original DDE wallpaper per monitor, persisted so a crash or a
/// later daemon run can restore it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SavedWallpapers {
    /// monitor name → wallpaper URI (as reported by DDE).
    pub monitors: BTreeMap<String, String>,
}

fn state_dir() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
}

fn saved_path() -> PathBuf {
    state_dir().join("dde-saved-wallpaper.json")
}

/// Write the transparent PNG into the state dir and return its file:// URI.
fn transparent_uri() -> Option<String> {
    let dir = state_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("dde-transparent.png");
    if std::fs::read(&path).ok().as_deref() != Some(&TRANSPARENT_PNG[..]) {
        std::fs::write(&path, TRANSPARENT_PNG).ok()?;
    }
    Some(format!("file://{}", path.display()))
}

/// Run `gdbus call --session` and return stdout on success.
fn gdbus_call(dest: &str, path: &str, iface_method: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("gdbus");
    cmd.args(["call", "--session", "--dest", dest, "--object-path", path])
        .args(["--method", iface_method])
        .args(args);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Leniently pull the first single- or double-quoted string out of gdbus
/// output like `('file:///usr/share/wallpapers/a.jpg',)`.
fn parse_first_string(out: &str) -> Option<String> {
    let out = out.trim();
    let (open, rest) = out
        .char_indices()
        .find(|&(_, c)| c == '\'' || c == '"')
        .map(|(i, c)| (c, &out[i + 1..]))?;
    let end = rest.find(open)?;
    Some(rest[..end].to_string())
}

/// Ask DDE for the current wallpaper of `monitor`, trying each service.
fn get_background(monitor: &str) -> Option<String> {
    for (dest, path, iface) in SERVICES {
        let method = format!("{iface}.GetCurrentWorkspaceBackgroundForMonitor");
        if let Some(out) = gdbus_call(dest, path, &method, &[monitor]) {
            if let Some(uri) = parse_first_string(&out) {
                if !uri.is_empty() {
                    return Some(uri);
                }
            }
        }
    }
    None
}

/// Set the wallpaper of `monitor`, trying each service. True on success.
fn set_background(monitor: &str, uri: &str) -> bool {
    for (dest, path, iface) in SERVICES {
        let method = format!("{iface}.SetMonitorBackground");
        if gdbus_call(dest, path, &method, &[monitor, uri]).is_some() {
            return true;
        }
    }
    false
}

/// Persist the original wallpapers. Never overwrites an existing file: a
/// leftover from a crashed run holds the true original, and the "current"
/// wallpaper now may already be our transparent one.
fn save_original(monitors: &[String], transparent: &str) {
    let path = saved_path();
    if path.exists() {
        return;
    }
    let mut saved = SavedWallpapers::default();
    for m in monitors {
        if let Some(uri) = get_background(m) {
            if uri != transparent {
                saved.monitors.insert(m.clone(), uri);
            }
        }
    }
    if saved.monitors.is_empty() {
        return;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    match serde_json::to_vec_pretty(&saved).map(|b| std::fs::write(&path, b)) {
        Ok(Ok(())) => log::info!("DDE: saved original wallpaper(s) to {}", path.display()),
        _ => log::warn!("DDE: could not persist original wallpaper state"),
    }
}

/// Apply the DDE quirk: transparent DDE wallpaper via DBus, else restack.
/// `monitors` are connector names (they match DDE's monitor names on X11);
/// `windows` are our wallpaper windows. Idempotent — called on every rebuild.
pub fn apply<C: Connection>(
    conn: &C,
    atoms: &Atoms,
    root: Window,
    monitors: &[String],
    windows: &[Window],
) -> Mode {
    // Primary: DBus transparency.
    if let Some(transparent) = transparent_uri() {
        save_original(monitors, &transparent);
        let mut ok = !monitors.is_empty();
        for m in monitors {
            if !set_background(m, &transparent) {
                ok = false;
                break;
            }
        }
        if ok {
            log::info!("DDE: set transparent wallpaper via DBus (desktop icons stay visible)");
            return Mode::DBus;
        }
    }

    // Fallback: restack above dde-shell's desktop window.
    if restack_above_dde_desktop(conn, atoms, root, windows) {
        log::warn!(
            "DDE: Appearance DBus service unavailable; restacking wallpaper above \
             dde-shell's desktop window — desktop icons may be hidden in this mode"
        );
        Mode::Restack
    } else {
        log::warn!("DDE: neither DBus transparency nor restack worked; wallpaper may be covered");
        Mode::Inactive
    }
}

/// Restore the user's original DDE wallpaper from the persisted state, then
/// remove the state file. No-op when nothing was saved. Called on shutdown and
/// on startup paths where the daemon will not be showing a wallpaper (crash
/// recovery).
pub fn restore() {
    let path = saved_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return; // nothing saved — nothing to restore
    };
    let Ok(saved) = serde_json::from_slice::<SavedWallpapers>(&bytes) else {
        log::warn!(
            "DDE: unreadable saved-wallpaper state at {}",
            path.display()
        );
        return;
    };
    let mut all_ok = true;
    for (monitor, uri) in &saved.monitors {
        if set_background(monitor, uri) {
            log::info!("DDE: restored wallpaper on {monitor}");
        } else {
            all_ok = false;
            log::warn!("DDE: failed to restore wallpaper on {monitor}");
        }
    }
    if all_ok {
        std::fs::remove_file(&path).ok();
    }
}

/// Find dde-shell's desktop window and stack each of `windows` directly ABOVE
/// it. True if the sibling was found and configured. Re-asserted alongside the
/// periodic re-lower, since DDE restacks its desktop on stacking changes.
pub fn restack_above_dde_desktop<C: Connection>(
    conn: &C,
    atoms: &Atoms,
    root: Window,
    windows: &[Window],
) -> bool {
    let Some(sibling) = find_dde_desktop_window(conn, atoms, root) else {
        return false;
    };
    let mut ok = false;
    for &w in windows {
        let aux = ConfigureWindowAux::new()
            .sibling(sibling)
            .stack_mode(StackMode::ABOVE);
        if conn.configure_window(w, &aux).is_ok() {
            ok = true;
        }
    }
    let _ = conn.flush();
    ok
}

/// The client window whose WM_CLASS is "dde-shell"/"desktop", from
/// `_NET_CLIENT_LIST_STACKING`.
fn find_dde_desktop_window<C: Connection>(conn: &C, atoms: &Atoms, root: Window) -> Option<Window> {
    let reply = conn
        .get_property(
            false,
            root,
            atoms._NET_CLIENT_LIST_STACKING,
            AtomEnum::WINDOW,
            0,
            u32::MAX,
        )
        .ok()?
        .reply()
        .ok()?;
    let clients: Vec<Window> = reply.value32()?.collect();
    for w in clients {
        let Ok(cookie) = conn.get_property(false, w, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
        else {
            continue;
        };
        let Ok(prop) = cookie.reply() else { continue };
        if wm_class_is_dde_desktop(&prop.value) {
            return Some(w);
        }
    }
    None
}

/// WM_CLASS is two NUL-terminated strings: instance, class.
fn wm_class_is_dde_desktop(value: &[u8]) -> bool {
    let mut parts = value.split(|&b| b == 0).filter(|s| !s.is_empty());
    let instance = parts.next().unwrap_or(&[]);
    let class = parts.next().unwrap_or(&[]);
    let eq = |s: &[u8], t: &str| s.eq_ignore_ascii_case(t.as_bytes());
    (eq(instance, "dde-shell") && eq(class, "desktop"))
        || (eq(instance, "desktop") && eq(class, "dde-shell"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_wallpapers_round_trip() {
        let mut s = SavedWallpapers::default();
        s.monitors.insert(
            "HDMI-0".into(),
            "file:///usr/share/wallpapers/deepin/a.jpg".into(),
        );
        s.monitors
            .insert("eDP-1".into(), "file:///home/u/b.png".into());
        let json = serde_json::to_string(&s).unwrap();
        let back: SavedWallpapers = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.monitors.len(), 2);
        assert_eq!(back.monitors["eDP-1"], "file:///home/u/b.png".to_string());
    }

    #[test]
    fn saved_wallpapers_tolerates_unknown_fields_absent() {
        // Empty object deserializes to an empty map (serde default).
        let back: SavedWallpapers = serde_json::from_str(r#"{"monitors":{}}"#).unwrap();
        assert!(back.monitors.is_empty());
    }

    #[test]
    fn parses_gdbus_tuple_output() {
        assert_eq!(
            parse_first_string("('file:///usr/share/w.jpg',)\n"),
            Some("file:///usr/share/w.jpg".to_string())
        );
        assert_eq!(
            parse_first_string("(\"file:///a b.png\",)"),
            Some("file:///a b.png".to_string())
        );
        assert_eq!(parse_first_string("()"), None);
        assert_eq!(parse_first_string(""), None);
    }

    #[test]
    fn transparent_png_is_valid_signature() {
        assert_eq!(&TRANSPARENT_PNG[..8], b"\x89PNG\r\n\x1a\n");
        // Ends with the IEND chunk + its CRC.
        assert_eq!(&TRANSPARENT_PNG[TRANSPARENT_PNG.len() - 8..][..4], b"IEND");
    }

    #[test]
    fn wm_class_matching() {
        assert!(wm_class_is_dde_desktop(b"dde-shell\0desktop\0"));
        assert!(wm_class_is_dde_desktop(b"desktop\0dde-shell\0"));
        assert!(!wm_class_is_dde_desktop(
            b"fresco-wallpaper\0fresco-wallpaper\0"
        ));
        assert!(!wm_class_is_dde_desktop(b""));
        assert!(!wm_class_is_dde_desktop(b"dde-shell\0dock\0"));
    }
}

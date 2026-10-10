//! Deepin DDE (X11) quirks — GitHub issue #2.
//!
//! DDE's `dde-shell` paints its own opaque desktop window (WM_CLASS
//! "dde-shell"/"desktop", declaring `[DESKTOP, NORMAL]`) that covers Fresco's
//! wallpaper entirely. Measured on a Deepin 25 VM (dde-shell, X11, KWin):
//!
//!  * A DESKTOP-only window is pinned to KWin's bottom desktop layer, always
//!    under dde-shell's desktop window.
//!  * A transparent DDE wallpaper composites onto BLACK, not onto the windows
//!    below it — a solid red root window underneath stayed invisible. Nothing
//!    stacked below that window can ever be seen there.
//!  * A sibling-relative restack (`ConfigureWindow(sibling, Above)`) fails with
//!    BadMatch: KWin reparents both windows, so they are not siblings.
//!  * What works: create our windows as [`WindowKind::DdeRaised`] and raise
//!    them with a sibling-less `ConfigureWindow(Above)`.
//!
//! So on Deepin 25 the raise ("restack") is the only working strategy — and
//! because DDE paints its wallpaper and its icons into that one opaque window,
//! a raise that shows the video necessarily hides the icons. There is no
//! stacking position that shows both. What the raise can do is get out of the
//! way when the icons are actually being used: clicking the desktop makes KWin
//! raise DDE's window above ours, and [`IconPeek`] leaves it there for a few
//! seconds instead of burying it on the next stacking pass.
//!
//! The DBus transparency path is kept for the explicit `transparent`
//! preference — it still serves older DDE (dde-desktop on Deepin 20/23), and it
//! is the only option when no dde-shell desktop window exists at all — and it
//! persists the user's original wallpaper to the fresco state dir so it can be
//! restored on shutdown, or on a later startup after a crash.
//!
//! No DBus crate: we shell out to `gdbus` (ships with glib on Deepin) and
//! parse its output leniently.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;

use super::x11win::{self, Atoms, WindowKind};
use crate::config::DdeMode;

/// 1x1 fully-transparent RGBA PNG, written to the state dir at runtime and
/// handed to DDE as a `file://` wallpaper URI.
const TRANSPARENT_PNG: [u8; 68] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x00, 0x02, 0x00,
    0x00, 0x05, 0x00, 0x01, 0x7a, 0x5e, 0xab, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

/// Side of the solid key-colour PNG. Small, but not 1x1: some wallpaper
/// pipelines refuse degenerate images, and DDE scales it to the screen anyway.
const KEY_PNG_SIDE: u32 = 64;

/// An opaque RGBA PNG of one colour, hand-encoded (stored deflate, no extra
/// dependency: the PNG must be *lossless* so the key survives byte for byte).
fn solid_png(side: u32, rgb: [u8; 3]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    // Filter byte 0 + `side` RGBA pixels per row.
    let mut raw = Vec::new();
    for _ in 0..side {
        raw.extend_from_slice(&[0]);
        for _ in 0..side {
            raw.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 0xff]);
        }
    }
    // zlib: header, stored blocks (<= 65535 bytes each), Adler-32.
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    while let Some(c) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&side.to_be_bytes());
    ihdr.extend_from_slice(&side.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);
    png
}

/// (dest, object path, interface) for DDE's session Appearance service.
/// Deepin 25 first, then the legacy pre-25 names.
pub(super) const SERVICES: [(&str, &str, &str); 2] = [
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
    /// Our windows are raised above dde-shell's desktop window (icons hidden
    /// until the desktop is clicked — see [`IconPeek`]).
    Restack,
    /// MATE: our windows sit above Caja's desktop window, which a mirror
    /// thread keeps below them and whose icons it copies onto the wallpaper —
    /// see `caja_mirror`. Icons stay visible and every click still reaches
    /// Caja.
    CajaMirror,
    /// Deepin, opt-in (`dde_mode = "mirror"`): the same trick as
    /// [`Mode::CajaMirror`] against dde-shell's 32-bit desktop window, with the
    /// DDE wallpaper set to the key colour over DBus. Experimental.
    DdeMirror,
    /// Xfce: [`Mode::CajaMirror`] against xfdesktop's per-monitor desktop
    /// windows, with the backdrop set to the key colour over xfconf. Our
    /// windows are not restacked: xfwm4 already keeps them in a layer above
    /// xfdesktop's, and nothing it does on a click moves xfdesktop above them.
    XfceMirror,
}

impl Mode {
    /// Which desktop's icons the mirror copies in this mode, if it is a mirror
    /// mode at all.
    pub fn mirror_desktop(self) -> Option<super::caja_mirror::Desktop> {
        match self {
            Mode::CajaMirror => Some(super::caja_mirror::Desktop::Caja),
            Mode::DdeMirror => Some(super::caja_mirror::Desktop::Dde),
            Mode::XfceMirror => Some(super::caja_mirror::Desktop::Xfce),
            _ => None,
        }
    }
}

/// The user's original DDE wallpaper per monitor, persisted so a crash or a
/// later daemon run can restore it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SavedWallpapers {
    /// monitor name → wallpaper URI (as reported by DDE).
    pub monitors: BTreeMap<String, String>,
}

pub(super) fn state_dir() -> PathBuf {
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

/// Write the solid key-colour PNG beside the transparent one and return its
/// file:// URI.
fn key_uri() -> Option<String> {
    let dir = state_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("dde-key.png");
    let png = solid_png(KEY_PNG_SIDE, super::caja_mirror::KEY);
    if std::fs::read(&path).ok().as_deref() != Some(&png[..]) {
        std::fs::write(&path, &png).ok()?;
    }
    Some(format!("file://{}", path.display()))
}

/// `FRESCO_DDE_MIRROR_PROBE=alpha`: a diagnostic for the icon mirror. The
/// mirror then sets the transparent wallpaper instead of the key colour and its
/// diagnostics log the alpha range of DDE's desktop window, which tells whether
/// that window ever carries real per-pixel alpha (it would allow smooth icon
/// edges; today's opaque key background cannot).
pub(super) fn probe_alpha() -> bool {
    std::env::var("FRESCO_DDE_MIRROR_PROBE").is_ok_and(|v| v.trim().eq_ignore_ascii_case("alpha"))
}

/// Which D-Bus daemon a `gdbus` call goes to. DDE's Appearance service is on
/// the session bus; its Accounts service — the store of record for the lock
/// screen's background, see `dde_lock` — is on the system bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Bus {
    Session,
    System,
}

/// Run `gdbus call` on `bus` and return stdout on success. `timeout` is
/// gdbus's own `--timeout` in seconds; `None` keeps its 25 s default.
fn gdbus_run(
    bus: Bus,
    timeout: Option<u32>,
    dest: &str,
    path: &str,
    iface_method: &str,
    args: &[&str],
) -> Option<String> {
    let mut cmd = Command::new("gdbus");
    cmd.arg("call").arg(match bus {
        Bus::Session => "--session",
        Bus::System => "--system",
    });
    if let Some(secs) = timeout {
        cmd.args(["--timeout", &secs.to_string()]);
    }
    cmd.args(["--dest", dest, "--object-path", path])
        .args(["--method", iface_method])
        .args(args);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run `gdbus call --session` and return stdout on success.
fn gdbus_call(dest: &str, path: &str, iface_method: &str, args: &[&str]) -> Option<String> {
    gdbus_run(Bus::Session, None, dest, path, iface_method, args)
}

/// Bound for [`gdbus_call_on`].
const GDBUS_TIMEOUT_SECS: u32 = 5;

/// [`gdbus_call`] on either bus, with a bounded wait: for callers that run on
/// the daemon's main loop and must not sit out gdbus's 25 s default when a
/// service is wedged.
pub(super) fn gdbus_call_on(
    bus: Bus,
    dest: &str,
    path: &str,
    iface_method: &str,
    args: &[&str],
) -> Option<String> {
    gdbus_run(
        bus,
        Some(GDBUS_TIMEOUT_SECS),
        dest,
        path,
        iface_method,
        args,
    )
}

/// Leniently pull the first single- or double-quoted string out of gdbus
/// output like `('file:///usr/share/wallpapers/a.jpg',)` or, for a property
/// read, `(<'file:///a.jpg'>,)`. Backslash escapes inside the string are
/// undone, the way GVariant's text format writes them.
pub(super) fn parse_first_string(out: &str) -> Option<String> {
    let out = out.trim();
    let (open, rest) = out
        .char_indices()
        .find(|&(_, c)| c == '\'' || c == '"')
        .map(|(i, c)| (c, &out[i + 1..]))?;
    let mut value = String::new();
    let mut chars = rest.chars();
    loop {
        match chars.next()? {
            c if c == open => return Some(value),
            '\\' => match chars.next()? {
                'n' => value.push('\n'),
                't' => value.push('\t'),
                other => value.push(other),
            },
            c => value.push(c),
        }
    }
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
///
/// `ours` are the URIs Fresco itself installs (transparent / key PNG): a
/// monitor currently showing one of those is not the user's wallpaper.
/// Returns true when a state file exists afterwards, i.e. the real wallpaper is
/// on disk and [`restore`] can put it back.
fn save_original(monitors: &[String], ours: &[&str]) -> bool {
    let path = saved_path();
    if path.exists() {
        return true;
    }
    let mut saved = SavedWallpapers::default();
    for m in monitors {
        if let Some(uri) = get_background(m) {
            if !ours.contains(&uri.as_str()) {
                saved.monitors.insert(m.clone(), uri);
            }
        }
    }
    if saved.monitors.is_empty() {
        return false;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    match serde_json::to_vec_pretty(&saved).map(|b| std::fs::write(&path, b)) {
        Ok(Ok(())) => {
            log::info!("DDE: saved original wallpaper(s) to {}", path.display());
            true
        }
        _ => {
            log::warn!("DDE: could not persist original wallpaper state");
            false
        }
    }
}

/// Set every monitor's DDE wallpaper to the solid key-colour PNG for the icon
/// mirror ([`super::caja_mirror::Desktop::Dde`]). The user's real wallpaper is
/// saved first and [`restore`] puts it back.
///
/// A state file left by the transparent mode or a crashed run is kept as is:
/// it already holds the real wallpaper (`save_original` never records our own
/// PNGs and never overwrites), so whatever DDE shows *now* (transparent PNG,
/// key PNG, or the real one) is irrelevant. When nothing can be saved we
/// refuse to touch the wallpaper at all, since it could not be given back.
///
/// Returns the key PNG's URI, for the attach diagnostics.
pub(super) fn apply_key_background(monitors: &[String]) -> Option<String> {
    let (Some(key), Some(transparent)) = (key_uri(), transparent_uri()) else {
        log::warn!("DDE: cannot write the key-colour PNG to the state dir");
        return None;
    };
    if monitors.is_empty() || !save_original(monitors, &[&key, &transparent]) {
        log::warn!(
            "DDE: could not read the current wallpaper, so it could not be restored; \
             leaving it alone"
        );
        return None;
    }
    // The alpha probe swaps the key PNG for the transparent one; everything
    // else (saving the original, restoring it) is the same.
    let probe = probe_alpha();
    let shown = if probe { &transparent } else { &key };
    if probe {
        log::warn!(
            "DDE: FRESCO_DDE_MIRROR_PROBE=alpha — setting the transparent wallpaper instead of \
             the key colour; the mirror logs the desktop window's alpha range"
        );
    }
    for m in monitors {
        if !set_background(m, shown) {
            log::warn!("DDE: SetMonitorBackground failed on {m}");
            restore();
            return None;
        }
    }
    let readback = monitors.first().and_then(|m| get_background(m));
    log::info!(
        "DDE: {} wallpaper {shown} set; DDE reports {readback:?} (matches: {})",
        if probe {
            "transparent (probe)"
        } else {
            "key-colour"
        },
        readback.as_deref() == Some(shown.as_str())
    );
    Some(shown.clone())
}

/// The strategy chosen for this rebuild, before we try to enact it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Transparent DDE wallpaper via DBus (icons stay visible).
    Transparent,
    /// Raise our windows above dde-shell's desktop (icons may be hidden).
    Restack,
    /// Restack, then copy dde-shell's icons over the video (experimental).
    Mirror,
}

impl Strategy {
    /// The wallpaper-window flavour this strategy needs. The raise only works
    /// with the `[DESKTOP, NORMAL]` declaration, and that declaration is only
    /// ever used when we are going to raise.
    fn window_kind(self) -> WindowKind {
        match self {
            Strategy::Transparent => WindowKind::Desktop,
            Strategy::Restack | Strategy::Mirror => WindowKind::DdeRaised,
        }
    }
}

/// Parse a `FRESCO_DDE_MODE` value. Unknown/empty values mean "no override".
fn parse_mode(s: &str) -> Option<DdeMode> {
    match s.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(DdeMode::Auto),
        "transparent" | "dbus" => Some(DdeMode::Transparent),
        "restack" => Some(DdeMode::Restack),
        "mirror" => Some(DdeMode::Mirror),
        _ => None,
    }
}

/// The effective preference: `FRESCO_DDE_MODE` env var wins over config.
fn effective_pref(config_pref: DdeMode) -> DdeMode {
    match std::env::var("FRESCO_DDE_MODE") {
        Ok(v) => match parse_mode(&v) {
            Some(m) => {
                log::info!("DDE: FRESCO_DDE_MODE={v} overrides config (mode {m:?})");
                m
            }
            None => {
                log::warn!("DDE: ignoring invalid FRESCO_DDE_MODE={v:?} (want auto|transparent|restack|mirror)");
                config_pref
            }
        },
        Err(_) => config_pref,
    }
}

/// Pure mode-selection logic: preference x whether dde-shell's desktop
/// window is present.
///
/// Auto restacks whenever that window exists, because on dde-shell nothing
/// below it is ever visible (see the module docs). Transparency remains the
/// auto choice only when no such window is found — an older DDE, where it is
/// the strategy that actually works.
fn select_strategy(pref: DdeMode, desktop_window_found: bool) -> Strategy {
    match pref {
        DdeMode::Transparent => Strategy::Transparent,
        DdeMode::Restack => Strategy::Restack,
        // The mirror needs dde-shell's window to copy from; without it there is
        // nothing to mirror and the plain transparency path is the right one.
        DdeMode::Mirror => {
            if desktop_window_found {
                Strategy::Mirror
            } else {
                Strategy::Transparent
            }
        }
        DdeMode::Auto => {
            if desktop_window_found {
                Strategy::Restack
            } else {
                Strategy::Transparent
            }
        }
    }
}

/// Visual depth of dde-shell's desktop window, if it can be found. Logged for
/// diagnostics only: on Deepin 25 that window is 32-bit ARGB yet still
/// composites its wallpaper onto black, so depth says nothing about whether
/// transparency can work.
fn desktop_window_depth<C: Connection>(conn: &C, atoms: &Atoms, root: Window) -> Option<u8> {
    let w = find_dde_desktop_window(conn, atoms, root)?;
    let geom = conn.get_geometry(w).ok()?.reply().ok()?;
    Some(geom.depth)
}

/// The strategy this session will use, resolved from the preference and the
/// live stack. Cheap enough to call once per rebuild.
fn current_strategy<C: Connection>(
    conn: &C,
    atoms: &Atoms,
    root: Window,
    config_pref: DdeMode,
) -> Strategy {
    select_strategy(
        effective_pref(config_pref),
        find_dde_desktop_window(conn, atoms, root).is_some(),
    )
}

/// Which flavour of wallpaper window this session must create. Called before
/// any window exists, because the DDE raise only works when the window was
/// created declaring `[DESKTOP, NORMAL]`.
///
/// Off Deepin this returns [`WindowKind::Desktop`] without touching X11, so no
/// other desktop environment sees any change.
pub fn window_kind<C: Connection>(
    conn: &C,
    atoms: &Atoms,
    root: Window,
    config_pref: DdeMode,
) -> WindowKind {
    // MATE always raises: Caja's desktop window covers everything below it,
    // and there is no transparency service to fall back on. Deciding before
    // Caja has even mapped (a login race) is fine — the raised kind simply
    // sits above the root window until Caja arrives.
    if crate::capability::is_mate() {
        return WindowKind::DdeRaised;
    }
    if !crate::capability::is_deepin_dde() {
        return WindowKind::Desktop;
    }
    current_strategy(conn, atoms, root, config_pref).window_kind()
}

/// Apply the DDE quirk. `monitors` are connector names (they match DDE's
/// monitor names on X11); `windows` are our wallpaper windows;
/// `config_pref` is the `dde_mode` config key (env `FRESCO_DDE_MODE`
/// overrides it). Idempotent — called on every rebuild.
///
/// Strategy selection must agree with [`window_kind`], which ran just before
/// the windows were created; both go through [`select_strategy`].
pub fn apply<C: Connection>(
    conn: &C,
    atoms: &Atoms,
    root: Window,
    monitors: &[String],
    windows: &[Window],
    config_pref: DdeMode,
) -> Mode {
    if crate::capability::is_mate() {
        return apply_mate(conn, atoms, root, windows);
    }
    if crate::capability::is_xfce() {
        return apply_xfce(conn);
    }
    let pref = effective_pref(config_pref);
    let depth = desktop_window_depth(conn, atoms, root);
    match depth {
        Some(d) => log::info!("DDE: desktop window found (visual depth {d}-bit)"),
        None => log::info!("DDE: desktop window not found"),
    }
    let strategy = select_strategy(pref, depth.is_some());
    log::info!("DDE: preference {pref:?}, chosen strategy {strategy:?}");

    if strategy != Strategy::Mirror {
        // A mirror run that crashed (or was killed) leaves DDE's desktop window
        // at opacity 0. If this run uses transparency or restack instead, nothing
        // else would ever un-hide it, and the desktop would stay invisible.
        // Idempotent, and a no-op without the mirror's state file.
        super::caja_mirror::restore_desktop_opacity();
    }

    if strategy == Strategy::Mirror {
        // The background is NOT touched here: `apply` runs on every rebuild,
        // and the daemon (`sync_caja_mirror`) sets the key-colour wallpaper
        // once, when it starts the mirror. That is also where any failure
        // (no Composite/Damage, attach fails, the key cannot be set) turns
        // into a restore + Restack, with no retry.
        if !restack_above_dde_desktop(conn, windows) {
            log::warn!("DDE: raise failed; wallpaper may be covered");
            return Mode::Inactive;
        }
        return if has_mirror_extensions(conn) {
            log::warn!(
                "DDE: experimental icon mirror on (dde_mode = \"mirror\"): desktop icons are \
                 copied over the wallpaper; set dde_mode = \"auto\" to turn it off"
            );
            Mode::DdeMirror
        } else {
            log::warn!(
                "DDE: the X server has no Composite/Damage; icon mirror unavailable, \
                 using restack"
            );
            Mode::Restack
        };
    }

    if strategy == Strategy::Restack {
        // Transparency is not in play: if a previous run left the user's
        // desktop on our transparent PNG, put their real wallpaper back.
        restore();
        return if restack_above_dde_desktop(conn, windows) {
            log::warn!(
                "DDE: raising wallpaper above dde-shell's desktop window — desktop \
                 icons are hidden while it plays; clicking the desktop brings them \
                 back for `dde_icon_peek_secs` seconds"
            );
            Mode::Restack
        } else {
            log::warn!("DDE: raise failed; wallpaper may be covered");
            Mode::Inactive
        };
    }

    // Primary: DBus transparency.
    if let Some(transparent) = transparent_uri() {
        save_original(monitors, &[&transparent]);
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

    // Fallback: raise above dde-shell's desktop window.
    if restack_above_dde_desktop(conn, windows) {
        log::warn!(
            "DDE: Appearance DBus service unavailable; raising wallpaper above \
             dde-shell's desktop window — desktop icons may be hidden in this \
             mode, and the raise is best-effort here because the windows were \
             created for transparency (set dde_mode = \"restack\" to make it stick)"
        );
        Mode::Restack
    } else {
        log::warn!("DDE: neither DBus transparency nor restack worked; wallpaper may be covered");
        Mode::Inactive
    }
}

/// Whether the server has the extensions the icon mirror is built on.
fn has_mirror_extensions<C: Connection>(conn: &C) -> bool {
    [
        x11rb::protocol::composite::X11_EXTENSION_NAME,
        x11rb::protocol::damage::X11_EXTENSION_NAME,
    ]
    .iter()
    .all(|name| {
        x11rb::connection::RequestConnection::extension_information(conn, name)
            .ok()
            .flatten()
            .is_some()
    })
}

/// MATE (issue #18): raise the wallpaper above Caja's desktop window and push
/// that window down. Deepin's DBus transparency has no MATE counterpart.
///
/// Returns [`Mode::CajaMirror`] when the server can redirect and track Caja's
/// window (Composite + Damage), so the daemon then mirrors Caja's icons over
/// the wallpaper; otherwise [`Mode::Restack`], where the icons are hidden
/// until the desktop is clicked.
fn apply_mate<C: Connection>(conn: &C, atoms: &Atoms, root: Window, windows: &[Window]) -> Mode {
    let caja = find_dde_desktop_windows(conn, atoms, root);
    if caja.is_empty() {
        log::info!("MATE: Caja's desktop window not found (desktop icons off, or Caja not up yet)");
    } else {
        log::info!("MATE: found {} Caja desktop window(s)", caja.len());
    }
    if !restack_above_desktop(conn, windows, &caja) {
        log::warn!("MATE: raise failed; wallpaper may be covered by Caja's desktop");
        return Mode::Inactive;
    }
    if has_mirror_extensions(conn) {
        Mode::CajaMirror
    } else {
        log::warn!(
            "MATE: the X server has no Composite/Damage, so Caja's icons cannot be drawn \
             over the wallpaper — they are hidden while it plays; clicking the desktop \
             brings them back for `dde_icon_peek_secs` seconds"
        );
        Mode::Restack
    }
}

/// Xfce: nothing to restack. Our windows keep the declaration they were
/// created with (`DESKTOP` + `BELOW`): xfwm4 puts `BELOW` in a layer above the
/// one it keeps xfdesktop's `DESKTOP` window in, layers are strict, and it
/// ignores stacking requests for DESKTOP windows — so we are above xfdesktop
/// for good and a raise or a lower would only stir the stack. What the layer
/// hides is xfdesktop's icons; the mirror draws them onto our windows.
///
/// [`Mode::XfceMirror`] when the server can redirect and track xfdesktop's
/// windows (Composite + Damage), otherwise [`Mode::Inactive`]: the icons stay
/// hidden while the video plays, and there is no click-to-peek.
fn apply_xfce<C: Connection>(conn: &C) -> Mode {
    if has_mirror_extensions(conn) {
        Mode::XfceMirror
    } else {
        log::warn!(
            "Xfce: the X server has no Composite/Damage, so xfdesktop's icons cannot be drawn \
             over the wallpaper — they are hidden while it plays"
        );
        Mode::Inactive
    }
}

/// Best-effort check that our wallpaper window is actually producing frames:
/// grab a small central region twice ~1s apart and compare. Purely
/// diagnostic — logs the outcome, never fails. Blocks ~1s, so callers run it
/// once per daemon lifetime, only in DDE mode.
pub fn render_self_check<C: Connection>(conn: &C, windows: &[Window]) {
    let Some(&w) = windows.first() else { return };
    let grab = |c: &C| -> Option<Vec<u8>> {
        let geom = c.get_geometry(w).ok()?.reply().ok()?;
        let side: u16 = 64;
        let x = (geom.width.saturating_sub(side) / 2) as i16;
        let y = (geom.height.saturating_sub(side) / 2) as i16;
        let img = c
            .get_image(
                ImageFormat::Z_PIXMAP,
                w,
                x,
                y,
                side.min(geom.width),
                side.min(geom.height),
                !0,
            )
            .ok()?
            .reply()
            .ok()?;
        Some(img.data)
    };
    let Some(first) = grab(conn) else {
        log::info!("DDE: render self-check unavailable (GetImage failed)");
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let Some(second) = grab(conn) else {
        log::info!("DDE: render self-check unavailable (GetImage failed)");
        return;
    };
    if first != second {
        log::info!("DDE: render self-check OK — wallpaper window frames are changing");
    } else {
        log::info!(
            "DDE: render self-check — wallpaper window frames unchanged over ~1s \
             (static wallpaper, paused video, or rendering problem)"
        );
    }
}

/// Restore the user's original DDE wallpaper from the persisted state, then
/// remove the state file. No-op when nothing was saved. Called on shutdown and
/// on startup paths where the daemon will not be showing a wallpaper (crash
/// recovery).
pub fn restore() {
    // The mirror hides DDE's desktop window from the compositor while it runs
    // (`caja_mirror::opacity`); a crashed run leaves it hidden. Undo that
    // first: it is a no-op without its state file, and an invisible desktop is
    // worse than a wrong wallpaper.
    super::caja_mirror::restore_desktop_opacity();
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

/// One-shot guard so the periodic re-assert (every ~2s) logs once, not forever.
static RAISE_LOGGED: AtomicBool = AtomicBool::new(false);

/// Raise each of `windows` to the top of the stack — no sibling. Verified on
/// Deepin 25: this lands our windows above dde-shell's desktop while real app
/// windows and the dock still stack above us. A sibling-relative request
/// against dde-shell's window is impossible (KWin reparents both, so
/// `ConfigureWindow(sibling, Above)` returns BadMatch), and finding that window
/// is not needed at all here.
///
/// True when every configure request was sent successfully. Re-asserted on the
/// daemon's periodic stacking pass, since DDE restacks its desktop whenever the
/// stack changes.
pub fn restack_above_dde_desktop<C: Connection>(conn: &C, windows: &[Window]) -> bool {
    restack_above_desktop(conn, windows, &[])
}

/// [`restack_above_dde_desktop`], and then **lower** each of `covering` — the
/// desktop's own icon window — to the bottom as well.
///
/// That second half exists for MATE's window manager, Marco. Like the
/// Metacity it forks, Marco ignores a stacking request from any application
/// that is not the active one once the user has touched something newer — so
/// after a click on Caja's desktop (which makes Caja active) every raise of
/// ours is dropped, and a pager-flagged `_NET_RESTACK_WINDOW` is too. Lowering
/// Caja's window *is* honoured, because it is a request about the active
/// application's own window. Measured on a Linux Mint 22 MATE desktop; Deepin
/// passes nothing here and is unchanged.
pub fn restack_above_desktop<C: Connection>(
    conn: &C,
    windows: &[Window],
    covering: &[Window],
) -> bool {
    if windows.is_empty() {
        return false;
    }
    let mut ok = true;
    for &w in windows {
        if x11win::raise(conn, w).is_err() {
            ok = false;
        }
    }
    for &w in covering {
        if x11win::lower(conn, w).is_err() {
            ok = false;
        }
    }
    let _ = conn.flush();
    if ok && !RAISE_LOGGED.swap(true, Ordering::Relaxed) {
        log::info!(
            "DDE: raised {} wallpaper window(s) above dde-shell's desktop \
             (sibling-less ConfigureWindow Above)",
            windows.len()
        );
    }
    ok
}

/// Parse a `FRESCO_DDE_ICON_PEEK` value (whole seconds). `None` = no override.
fn parse_peek_secs(s: &str) -> Option<u32> {
    s.trim().parse::<u32>().ok()
}

/// The effective peek window: `FRESCO_DDE_ICON_PEEK` (seconds) overrides the
/// `dde_icon_peek_secs` config key. Zero means never yield — the wallpaper
/// re-covers the icons on the next stacking pass, which is what 1.1.37 did.
/// Called on every stacking pass (~2s), so the env override is announced once.
static PEEK_ENV_LOGGED: AtomicBool = AtomicBool::new(false);

pub fn icon_peek(config_secs: u32) -> Duration {
    let secs = match std::env::var("FRESCO_DDE_ICON_PEEK") {
        Ok(v) => {
            let first = !PEEK_ENV_LOGGED.swap(true, Ordering::Relaxed);
            match parse_peek_secs(&v) {
                Some(s) => {
                    if first {
                        log::info!("DDE: FRESCO_DDE_ICON_PEEK={v} overrides config ({s}s)");
                    }
                    s
                }
                None => {
                    if first {
                        log::warn!(
                            "DDE: ignoring invalid FRESCO_DDE_ICON_PEEK={v:?} (want whole seconds)"
                        );
                    }
                    config_secs
                }
            }
        }
        Err(_) => config_secs,
    };
    Duration::from_secs(u64::from(secs))
}

/// What a periodic stacking pass should do in [`Mode::Restack`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeekAction {
    /// Put the wallpaper back on top of DDE's desktop window.
    Raise,
    /// Leave the stack alone: DDE's desktop — and so the icons — is on top.
    Yield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PeekState {
    /// Wallpaper on top, watching for DDE to raise its desktop.
    #[default]
    Idle,
    /// Icons are showing; stay out of the way until this instant.
    Peeking(Instant),
    /// The peek is over and we are raising again. A new peek may only start
    /// once the raise has visibly taken effect — otherwise the very condition
    /// that ends a peek (DDE's desktop still above us, because we just yielded
    /// to it) would immediately start the next one and the wallpaper would
    /// never come back.
    Recovering,
}

/// Restack mode's truce with the desktop icons.
///
/// On Deepin 25 DDE paints its wallpaper and its icons into one opaque window,
/// so a wallpaper that is visible is a wallpaper that covers the icons — there
/// is no stacking position that shows both. Clicking the desktop makes KWin
/// raise that window above ours, which is the only moment the icons are usable;
/// before 1.1.38 the next stacking pass buried them again about two seconds
/// later, which read as "the icons flash and disappear" (reported on Deepin 25).
///
/// So the raise now yields to that click: when DDE's desktop comes up above the
/// wallpaper we leave it there for [`icon_peek`], then take the stack back.
/// Interacting with the desktop again starts a new peek.
#[derive(Debug, Default)]
pub struct IconPeek {
    state: PeekState,
    /// DDE's desktop windows, cached across passes; re-scanned only when none
    /// of them is in the stacking list any more (DDE restarted).
    dde_windows: Vec<Window>,
    /// When the client list was last walked looking for them, so a session with
    /// no DDE desktop window at all (restack forced by hand) doesn't re-read
    /// every client's WM_CLASS every couple of seconds.
    last_scan: Option<Instant>,
    /// One-time log line, so a ~2s cadence doesn't fill the journal.
    logged: bool,
}

/// How long a fruitless search for DDE's desktop window is trusted for.
const RESCAN_AFTER: Duration = Duration::from_secs(30);

impl IconPeek {
    /// Pure state transition, so the timing rules are testable without X11.
    fn step(&mut self, now: Instant, dde_above: bool, peek: Duration) -> PeekAction {
        if peek.is_zero() {
            self.state = PeekState::Idle;
            return PeekAction::Raise;
        }
        match self.state {
            PeekState::Idle => {
                if dde_above {
                    self.state = PeekState::Peeking(now + peek);
                    PeekAction::Yield
                } else {
                    PeekAction::Raise
                }
            }
            PeekState::Peeking(until) => {
                if now < until {
                    PeekAction::Yield
                } else {
                    self.state = PeekState::Recovering;
                    PeekAction::Raise
                }
            }
            PeekState::Recovering => {
                if !dde_above {
                    self.state = PeekState::Idle;
                }
                PeekAction::Raise
            }
        }
    }

    /// One periodic stacking pass in [`Mode::Restack`]: raise the wallpaper
    /// back over DDE's desktop, unless the user is mid-peek at their icons.
    pub fn tick<C: Connection>(
        &mut self,
        conn: &C,
        atoms: &Atoms,
        root: Window,
        windows: &[Window],
        peek: Duration,
    ) {
        if windows.is_empty() {
            return;
        }
        let position = self.dde_position(conn, atoms, root, windows);
        let above = !peek.is_zero() && position == Some(true);
        let was_idle = self.state == PeekState::Idle;
        match self.step(Instant::now(), above, peek) {
            // Only send the raise when it would change something. Every
            // sibling-less ConfigureWindow makes KWin restack a full-screen
            // window and re-announce the stacking order, which dde-shell and
            // the dock react to; issued blindly every two seconds it lands in
            // the middle of window-switch animations (reported on Deepin 25 as
            // CPU rising and animations stuttering on every switch). When the
            // stack can't be read, keep raising as before.
            PeekAction::Raise if position == Some(false) => {}
            PeekAction::Raise => {
                // On MATE a raise alone is ignored once Caja is the active
                // application — see `restack_above_desktop`.
                let covering: &[Window] = if crate::capability::is_mate() {
                    &self.dde_windows
                } else {
                    &[]
                };
                restack_above_desktop(conn, windows, covering);
            }
            PeekAction::Yield if was_idle => {
                if self.logged {
                    log::debug!("DDE: desktop raised; holding the icons visible for {peek:?}");
                } else {
                    self.logged = true;
                    log::info!(
                        "DDE: desktop raised (icons in use) — holding the wallpaper below it for \
                         {peek:?}, then taking the stack back. Tune with the `dde_icon_peek_secs` \
                         config key (0 disables the hold)."
                    );
                }
            }
            PeekAction::Yield => {}
        }
    }

    /// Whether a DDE desktop window currently sits above our wallpaper:
    /// `Some(true)` / `Some(false)` when the stack shows it, `None` when it
    /// can't be told (unreadable stack, no DDE window found, ours not listed).
    fn dde_position<C: Connection>(
        &mut self,
        conn: &C,
        atoms: &Atoms,
        root: Window,
        windows: &[Window],
    ) -> Option<bool> {
        let stack = x11win::stacking_order(conn, atoms, root);
        if stack.is_empty() {
            return None;
        }
        if !self.dde_windows.iter().any(|w| stack.contains(w)) {
            let now = Instant::now();
            match self.last_scan {
                Some(t) if now.duration_since(t) < RESCAN_AFTER => return None,
                _ => {}
            }
            self.last_scan = Some(now);
            self.dde_windows = find_dde_desktop_windows(conn, atoms, root);
        }
        dde_stack_position(&stack, windows, &self.dde_windows)
    }
}

/// True when some DDE desktop window sits above some wallpaper window in
/// `stack` (bottom-most first).
///
/// False whenever either side is absent from the list: an unreadable stack is
/// not evidence that the icons are in use, and the caller must then keep doing
/// what it did before — raising.
#[cfg(test)]
fn dde_above_in_stack(stack: &[Window], ours: &[Window], dde: &[Window]) -> bool {
    dde_stack_position(stack, ours, dde) == Some(true)
}

/// Tri-state form of [`dde_above_in_stack`]: `None` unless every one of our
/// windows and at least one DDE desktop window appear in `stack`, so "already
/// above DDE" is only ever concluded from a stack that actually shows it.
fn dde_stack_position(stack: &[Window], ours: &[Window], dde: &[Window]) -> Option<bool> {
    let pos = |w: &Window| stack.iter().position(|s| s == w);
    let ours_pos: Vec<usize> = ours.iter().filter_map(pos).collect();
    if ours.is_empty() || ours_pos.len() != ours.len() {
        return None;
    }
    let dde_pos: Vec<usize> = dde.iter().filter_map(pos).collect();
    if dde_pos.is_empty() {
        return None;
    }
    let lowest_ours = *ours_pos.iter().min()?;
    Some(dde_pos.iter().any(|&p| p > lowest_ours))
}

/// Every client window whose WM_CLASS marks it as DDE's desktop — one per
/// screen on a multi-monitor session.
fn find_dde_desktop_windows<C: Connection>(conn: &C, atoms: &Atoms, root: Window) -> Vec<Window> {
    let clients = x11win::stacking_order(conn, atoms, root);
    log::debug!("DDE: scanning {} client windows", clients.len());
    let mut found = Vec::new();
    for w in clients {
        let Ok(cookie) = conn.get_property(false, w, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
        else {
            continue;
        };
        let Ok(prop) = cookie.reply() else { continue };
        log::debug!(
            "DDE: window {:#x} WM_CLASS={:?}",
            w,
            String::from_utf8_lossy(&prop.value)
        );
        if wm_class_is_dde_desktop(&prop.value) || wm_class_is_caja_desktop(&prop.value) {
            found.push(w);
        }
    }
    found
}

/// The lowest-stacked DDE desktop window, or `None` when DDE paints no desktop
/// window at all (older DDE, or the desktop disabled).
fn find_dde_desktop_window<C: Connection>(conn: &C, atoms: &Atoms, root: Window) -> Option<Window> {
    find_dde_desktop_windows(conn, atoms, root)
        .into_iter()
        .next()
}

/// Whether `value` (a WM_CLASS) is the desktop window of `desktop`.
pub(super) fn wm_class_is_desktop(desktop: super::caja_mirror::Desktop, value: &[u8]) -> bool {
    match desktop {
        super::caja_mirror::Desktop::Caja => wm_class_is_caja_desktop(value),
        super::caja_mirror::Desktop::Dde => wm_class_is_dde_desktop(value),
        super::caja_mirror::Desktop::Xfce => wm_class_is_xfdesktop(value),
    }
}

/// xfdesktop's desktop window(s) on Xfce: WM_CLASS `"xfdesktop\0Xfdesktop\0"`
/// (GTK's program name and its capitalised class). Matched part by part,
/// exactly, so `xfdesktop-settings` (`"xfdesktop-settings\0Xfdesktop-settings\0"`)
/// and every other program are out. Dialogs xfdesktop itself opens share the
/// class; the mirror tells them from the desktop by window type.
pub(super) fn wm_class_is_xfdesktop(value: &[u8]) -> bool {
    let mut parts = value.split(|&b| b == 0);
    let instance = parts.next().unwrap_or_default();
    let class = parts.next().unwrap_or_default();
    instance.eq_ignore_ascii_case(b"xfdesktop") && class.eq_ignore_ascii_case(b"xfdesktop")
}

/// Caja's desktop window on MATE. Read off a Linux Mint 22 MATE desktop: the
/// property is `"desktop_window\0Caja\0"`. Matched part by part, exactly —
/// Caja's ordinary file-manager windows share the class but never the
/// instance.
pub(super) fn wm_class_is_caja_desktop(value: &[u8]) -> bool {
    let mut parts = value.split(|&b| b == 0);
    let instance = parts.next().unwrap_or_default();
    let class = parts.next().unwrap_or_default();
    instance.eq_ignore_ascii_case(b"desktop_window") && class.eq_ignore_ascii_case(b"caja")
}

/// WM_CLASS is two NUL-terminated strings: instance, class.
pub(super) fn wm_class_is_dde_desktop(value: &[u8]) -> bool {
    // Measured on Deepin 25: the instance is one slash-joined token, so the
    // whole property reads "dde-shell/desktop\0org.deepin.dde-shell\0" — per
    // part equality never matches. Substring matching covers that, the older
    // "dde-desktop" of Deepin 20/23, and keeps dde-shell/dock out (no
    // "desktop" anywhere in it).
    let lower = value.to_ascii_lowercase();
    let has = |needle: &str| {
        let n = needle.as_bytes();
        lower.windows(n.len()).any(|w| w == n)
    };
    has("desktop") && (has("dde-shell") || has("dde-desktop") || has("deepin"))
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
    fn parses_property_variant_output_and_undoes_escapes() {
        // `org.freedesktop.DBus.Properties.Get` wraps the value in a variant.
        assert_eq!(
            parse_first_string("(<'file:///usr/share/backgrounds/a.jpg'>,)\n"),
            Some("file:///usr/share/backgrounds/a.jpg".to_string())
        );
        assert_eq!(parse_first_string("(<''>,)"), Some(String::new()));
        // GVariant text escapes an apostrophe and a backslash with `\`.
        assert_eq!(
            parse_first_string(r"('/home/o\'neil/a\\b.png',)"),
            Some(r"/home/o'neil/a\b.png".to_string())
        );
        // An unterminated string is not a value.
        assert_eq!(parse_first_string("('file:///a.png"), None);
    }

    #[test]
    fn transparent_png_is_valid_signature() {
        assert_eq!(&TRANSPARENT_PNG[..8], b"\x89PNG\r\n\x1a\n");
        // Ends with the IEND chunk + its CRC.
        assert_eq!(&TRANSPARENT_PNG[TRANSPARENT_PNG.len() - 8..][..4], b"IEND");
    }

    #[test]
    fn mode_parsing() {
        assert_eq!(parse_mode("auto"), Some(DdeMode::Auto));
        assert_eq!(parse_mode("transparent"), Some(DdeMode::Transparent));
        assert_eq!(parse_mode("dbus"), Some(DdeMode::Transparent));
        assert_eq!(parse_mode("restack"), Some(DdeMode::Restack));
        assert_eq!(parse_mode("  Restack \n"), Some(DdeMode::Restack));
        assert_eq!(parse_mode("TRANSPARENT"), Some(DdeMode::Transparent));
        assert_eq!(parse_mode(""), None);
        assert_eq!(parse_mode("yes"), None);
    }

    #[test]
    fn strategy_selection_matrix() {
        use DdeMode::*;
        // Auto: dde-shell's desktop window present → restack, the only
        // strategy that works there (transparency composites onto black,
        // measured on Deepin 25). No such window → transparency, since
        // restack would have no sibling to stack above.
        assert_eq!(select_strategy(Auto, true), Strategy::Restack);
        assert_eq!(select_strategy(Auto, false), Strategy::Transparent);
        // Explicit preferences win either way.
        for found in [true, false] {
            assert_eq!(select_strategy(Transparent, found), Strategy::Transparent);
            assert_eq!(select_strategy(Restack, found), Strategy::Restack);
        }
    }

    /// The window flavour and the strategy are two views of one decision: we
    /// only ever declare `[DESKTOP, NORMAL]` when we are going to raise.
    #[test]
    fn strategy_picks_the_matching_window_kind() {
        assert_eq!(Strategy::Restack.window_kind(), WindowKind::DdeRaised);
        assert_eq!(Strategy::Transparent.window_kind(), WindowKind::Desktop);
        // Auto on Deepin 25 (dde-shell desktop window present) ⇒ raised kind.
        assert_eq!(
            select_strategy(DdeMode::Auto, true).window_kind(),
            WindowKind::DdeRaised
        );
        // No dde-shell desktop window ⇒ plain desktop window, as everywhere else.
        assert_eq!(
            select_strategy(DdeMode::Auto, false).window_kind(),
            WindowKind::Desktop
        );
    }

    /// The whole point of the peek: a click that raises DDE's desktop keeps the
    /// icons up for the configured window, and the wallpaper comes back after.
    #[test]
    fn peek_yields_to_a_desktop_raise_then_takes_the_stack_back() {
        let t0 = Instant::now();
        let peek = Duration::from_secs(10);
        let mut p = IconPeek::default();

        // Nobody has touched the desktop: keep the wallpaper on top.
        assert_eq!(p.step(t0, false, peek), PeekAction::Raise);

        // The user clicks the desktop — DDE's window comes up above ours.
        assert_eq!(p.step(t0, true, peek), PeekAction::Yield);
        // ...and stays there for the whole peek, click or no click.
        assert_eq!(
            p.step(t0 + Duration::from_secs(2), true, peek),
            PeekAction::Yield
        );
        assert_eq!(
            p.step(t0 + Duration::from_secs(9), true, peek),
            PeekAction::Yield
        );

        // Peek over: raise, and keep raising while DDE's desktop is still on
        // top — that is exactly the state we ourselves created by yielding, so
        // it must not be read as a fresh click (which would loop forever).
        assert_eq!(
            p.step(t0 + Duration::from_secs(10), true, peek),
            PeekAction::Raise
        );
        assert_eq!(
            p.step(t0 + Duration::from_secs(12), true, peek),
            PeekAction::Raise
        );

        // The raise took effect; we are back to watching.
        assert_eq!(
            p.step(t0 + Duration::from_secs(14), false, peek),
            PeekAction::Raise
        );
        assert_eq!(p.state, PeekState::Idle);

        // A second click gets a full peek of its own.
        assert_eq!(
            p.step(t0 + Duration::from_secs(16), true, peek),
            PeekAction::Yield
        );
        assert_eq!(
            p.step(t0 + Duration::from_secs(25), true, peek),
            PeekAction::Yield
        );
        assert_eq!(
            p.step(t0 + Duration::from_secs(26), true, peek),
            PeekAction::Raise
        );
    }

    /// `dde_icon_peek_secs = 0` restores the pre-1.1.38 behaviour: always raise.
    #[test]
    fn zero_peek_never_yields() {
        let t0 = Instant::now();
        let mut p = IconPeek::default();
        for dde_above in [false, true, true, false] {
            assert_eq!(p.step(t0, dde_above, Duration::ZERO), PeekAction::Raise);
            assert_eq!(p.state, PeekState::Idle);
        }
    }

    /// Turning the peek off mid-peek must not strand the wallpaper below DDE.
    #[test]
    fn zero_peek_cancels_a_running_peek() {
        let t0 = Instant::now();
        let mut p = IconPeek::default();
        assert_eq!(p.step(t0, true, Duration::from_secs(10)), PeekAction::Yield);
        assert_eq!(
            p.step(t0 + Duration::from_secs(1), true, Duration::ZERO),
            PeekAction::Raise
        );
    }

    #[test]
    fn raise_is_skipped_only_when_the_stack_proves_we_are_already_on_top() {
        // Stack is bottom-most first; 10 = DDE desktop, 1/2 = ours, 50 = an app.
        assert_eq!(
            dde_stack_position(&[10, 1, 2, 50], &[1, 2], &[10]),
            Some(false)
        );
        assert_eq!(
            dde_stack_position(&[1, 10, 2, 50], &[1, 2], &[10]),
            Some(true)
        );
        // Unknowns must never read as "already on top", or we'd stop raising.
        assert_eq!(dde_stack_position(&[], &[1], &[10]), None);
        assert_eq!(dde_stack_position(&[10, 1, 50], &[1, 2], &[10]), None);
        assert_eq!(dde_stack_position(&[1, 50], &[1], &[10]), None);
        assert_eq!(dde_stack_position(&[1, 50], &[1], &[]), None);
    }

    #[test]
    fn peek_seconds_parsing() {
        assert_eq!(parse_peek_secs("10"), Some(10));
        assert_eq!(parse_peek_secs(" 30\n"), Some(30));
        assert_eq!(parse_peek_secs("0"), Some(0));
        assert_eq!(parse_peek_secs(""), None);
        assert_eq!(parse_peek_secs("-1"), None);
        assert_eq!(parse_peek_secs("10s"), None);
    }

    /// Stacking is read bottom-most first, so a *higher* index is nearer the
    /// user. Either side missing means "no evidence" — the caller then raises,
    /// which is what it did before the peek existed.
    #[test]
    fn dde_above_detection() {
        // ours at 1, DDE's desktop at 0: the wallpaper is on top, no peek.
        assert!(!dde_above_in_stack(&[10, 20], &[20], &[10]));
        // The user clicked: DDE's desktop is now above the wallpaper.
        assert!(dde_above_in_stack(&[20, 10], &[20], &[10]));
        // Multi-monitor: one raised DDE window is enough, and the comparison is
        // against the lowest wallpaper window.
        assert!(dde_above_in_stack(&[21, 10, 20, 11], &[20, 21], &[10, 11]));
        assert!(!dde_above_in_stack(&[10, 11, 20, 21], &[20, 21], &[10, 11]));
        // Unknown stack, or either side absent from it.
        assert!(!dde_above_in_stack(&[], &[20], &[10]));
        assert!(!dde_above_in_stack(&[10, 20], &[], &[10]));
        assert!(!dde_above_in_stack(&[10, 20], &[20], &[]));
        assert!(!dde_above_in_stack(&[10, 20], &[99], &[10]));
    }

    #[test]
    fn mirror_mode_parsing_and_selection() {
        assert_eq!(parse_mode("mirror"), Some(DdeMode::Mirror));
        assert_eq!(parse_mode(" Mirror\n"), Some(DdeMode::Mirror));
        // Opt-in only: Auto never picks the mirror.
        assert_eq!(select_strategy(DdeMode::Auto, true), Strategy::Restack);
        assert_eq!(select_strategy(DdeMode::Mirror, true), Strategy::Mirror);
        // No dde-shell window to copy from: plain transparency.
        assert_eq!(
            select_strategy(DdeMode::Mirror, false),
            Strategy::Transparent
        );
        assert_eq!(Strategy::Mirror.window_kind(), WindowKind::DdeRaised);
    }

    #[test]
    fn env_override_selects_mirror() {
        // Only this test touches FRESCO_DDE_MODE.
        std::env::set_var("FRESCO_DDE_MODE", "mirror");
        assert_eq!(effective_pref(DdeMode::Auto), DdeMode::Mirror);
        std::env::set_var("FRESCO_DDE_MODE", "bogus");
        assert_eq!(effective_pref(DdeMode::Restack), DdeMode::Restack);
        std::env::remove_var("FRESCO_DDE_MODE");
        assert_eq!(effective_pref(DdeMode::Mirror), DdeMode::Mirror);
    }

    #[test]
    fn mirror_modes_map_to_desktops() {
        use super::super::caja_mirror::Desktop;
        assert_eq!(Mode::CajaMirror.mirror_desktop(), Some(Desktop::Caja));
        assert_eq!(Mode::DdeMirror.mirror_desktop(), Some(Desktop::Dde));
        assert_eq!(Mode::XfceMirror.mirror_desktop(), Some(Desktop::Xfce));
        assert_eq!(Mode::Restack.mirror_desktop(), None);
        assert_eq!(Mode::Inactive.mirror_desktop(), None);
    }

    #[test]
    fn xfdesktop_matching() {
        assert!(wm_class_is_xfdesktop(b"xfdesktop\0Xfdesktop\0"));
        assert!(wm_class_is_xfdesktop(b"Xfdesktop\0Xfdesktop\0"));
        // The settings dialog shares the prefix; our window, Caja's desktop,
        // Deepin's desktop and an unrelated Xfce app are not it.
        assert!(!wm_class_is_xfdesktop(
            b"xfdesktop-settings\0Xfdesktop-settings\0"
        ));
        assert!(!wm_class_is_xfdesktop(b"xfdesktop-settings\0Xfdesktop\0"));
        assert!(!wm_class_is_xfdesktop(
            b"fresco-wallpaper\0fresco-wallpaper\0"
        ));
        assert!(!wm_class_is_xfdesktop(b"desktop_window\0Caja\0"));
        assert!(!wm_class_is_xfdesktop(
            b"dde-shell/desktop\0org.deepin.dde-shell\0"
        ));
        assert!(!wm_class_is_xfdesktop(b"xfce4-panel\0Xfce4-panel\0"));
        assert!(!wm_class_is_xfdesktop(b""));
        // And the per-desktop dispatch keeps each matcher to its own window.
        use super::super::caja_mirror::Desktop;
        assert!(wm_class_is_desktop(
            Desktop::Xfce,
            b"xfdesktop\0Xfdesktop\0"
        ));
        assert!(!wm_class_is_desktop(
            Desktop::Caja,
            b"xfdesktop\0Xfdesktop\0"
        ));
        assert!(!wm_class_is_desktop(
            Desktop::Xfce,
            b"desktop_window\0Caja\0"
        ));
    }

    #[test]
    fn key_png_decodes_to_the_key_colour() {
        let png = solid_png(KEY_PNG_SIDE, super::super::caja_mirror::KEY);
        let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        assert_eq!(img.dimensions(), (KEY_PNG_SIDE, KEY_PNG_SIDE));
        assert!(img.pixels().all(|p| p.0 == [1, 1, 1, 255]));
    }

    #[test]
    fn caja_desktop_matching() {
        assert!(wm_class_is_caja_desktop(b"desktop_window\0Caja\0"));
        // A Caja file-manager window, our own window, and Deepin's desktop.
        assert!(!wm_class_is_caja_desktop(b"caja\0Caja\0"));
        assert!(!wm_class_is_caja_desktop(
            b"fresco-wallpaper\0fresco-wallpaper\0"
        ));
        assert!(!wm_class_is_caja_desktop(
            b"dde-shell/desktop\0org.deepin.dde-shell\0"
        ));
        assert!(!wm_class_is_caja_desktop(b"desktop_window\0Nemo\0"));
        assert!(!wm_class_is_caja_desktop(b""));
    }

    #[test]
    fn wm_class_matching() {
        // The real property read off a Deepin 25 desktop — the instance is one
        // slash-joined token. This is the case that matters.
        assert!(wm_class_is_dde_desktop(
            b"dde-shell/desktop\0org.deepin.dde-shell\0"
        ));
        // Older Deepin, and defensive orderings.
        assert!(wm_class_is_dde_desktop(b"dde-desktop\0dde-desktop\0"));
        assert!(wm_class_is_dde_desktop(b"dde-shell\0desktop\0"));
        assert!(wm_class_is_dde_desktop(b"desktop\0dde-shell\0"));
        // Must not match the dock, which shares the dde-shell prefix, nor our
        // own windows, nor other Deepin apps.
        assert!(!wm_class_is_dde_desktop(
            b"dde-shell/dock\0org.deepin.dde-shell\0"
        ));
        assert!(!wm_class_is_dde_desktop(
            b"deepin-terminal\0deepin-terminal\0"
        ));
        assert!(!wm_class_is_dde_desktop(
            b"fresco-wallpaper\0fresco-wallpaper\0"
        ));
        assert!(!wm_class_is_dde_desktop(b""));
        assert!(!wm_class_is_dde_desktop(b"dde-shell\0dock\0"));
    }
}

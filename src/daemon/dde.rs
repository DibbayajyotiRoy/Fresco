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
    /// Our windows are raised above dde-shell's desktop window (icons hidden
    /// until the desktop is clicked — see [`IconPeek`]).
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

/// The strategy chosen for this rebuild, before we try to enact it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Transparent DDE wallpaper via DBus (icons stay visible).
    Transparent,
    /// Raise our windows above dde-shell's desktop (icons may be hidden).
    Restack,
}

impl Strategy {
    /// The wallpaper-window flavour this strategy needs. The raise only works
    /// with the `[DESKTOP, NORMAL]` declaration, and that declaration is only
    /// ever used when we are going to raise.
    fn window_kind(self) -> WindowKind {
        match self {
            Strategy::Transparent => WindowKind::Desktop,
            Strategy::Restack => WindowKind::DdeRaised,
        }
    }
}

/// Parse a `FRESCO_DDE_MODE` value. Unknown/empty values mean "no override".
fn parse_mode(s: &str) -> Option<DdeMode> {
    match s.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(DdeMode::Auto),
        "transparent" | "dbus" => Some(DdeMode::Transparent),
        "restack" => Some(DdeMode::Restack),
        _ => None,
    }
}

/// The effective preference: `FRESCO_DDE_MODE` env var wins over config.
fn effective_pref(config_pref: DdeMode) -> DdeMode {
    match std::env::var("FRESCO_DDE_MODE") {
        Ok(v) => {
            match parse_mode(&v) {
                Some(m) => {
                    log::info!("DDE: FRESCO_DDE_MODE={v} overrides config (mode {m:?})");
                    m
                }
                None => {
                    log::warn!("DDE: ignoring invalid FRESCO_DDE_MODE={v:?} (want auto|transparent|restack)");
                    config_pref
                }
            }
        }
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
    let pref = effective_pref(config_pref);
    let depth = desktop_window_depth(conn, atoms, root);
    match depth {
        Some(d) => log::info!("DDE: desktop window found (visual depth {d}-bit)"),
        None => log::info!("DDE: desktop window not found"),
    }
    let strategy = select_strategy(pref, depth.is_some());
    log::info!("DDE: preference {pref:?}, chosen strategy {strategy:?}");

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
    if windows.is_empty() {
        return false;
    }
    let mut ok = true;
    for &w in windows {
        if x11win::raise(conn, w).is_err() {
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
        let above = !peek.is_zero() && self.dde_above(conn, atoms, root, windows);
        let was_idle = self.state == PeekState::Idle;
        match self.step(Instant::now(), above, peek) {
            PeekAction::Raise => {
                restack_above_dde_desktop(conn, windows);
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

    /// Whether any DDE desktop window currently sits above our wallpaper.
    fn dde_above<C: Connection>(
        &mut self,
        conn: &C,
        atoms: &Atoms,
        root: Window,
        windows: &[Window],
    ) -> bool {
        let stack = client_list_stacking(conn, atoms, root);
        if stack.is_empty() {
            return false;
        }
        if !self.dde_windows.iter().any(|w| stack.contains(w)) {
            let now = Instant::now();
            match self.last_scan {
                Some(t) if now.duration_since(t) < RESCAN_AFTER => return false,
                _ => {}
            }
            self.last_scan = Some(now);
            self.dde_windows = find_dde_desktop_windows(conn, atoms, root);
        }
        dde_above_in_stack(&stack, windows, &self.dde_windows)
    }
}

/// True when some DDE desktop window sits above some wallpaper window in
/// `stack` (bottom-most first).
///
/// False whenever either side is absent from the list: an unreadable stack is
/// not evidence that the icons are in use, and the caller must then keep doing
/// what it did before — raising.
fn dde_above_in_stack(stack: &[Window], ours: &[Window], dde: &[Window]) -> bool {
    let pos = |w: &Window| stack.iter().position(|s| s == w);
    let Some(lowest_ours) = ours.iter().filter_map(pos).min() else {
        return false;
    };
    dde.iter().filter_map(pos).any(|p| p > lowest_ours)
}

/// `_NET_CLIENT_LIST_STACKING` on `root`, bottom-most window first. Empty when
/// the property cannot be read.
fn client_list_stacking<C: Connection>(conn: &C, atoms: &Atoms, root: Window) -> Vec<Window> {
    // long_length is in 4-byte units and must stay well clear of the server's
    // overflow guard — u32::MAX makes Xorg reject the request outright, which
    // silently defeated the whole scan. 4096 windows is far past any real
    // desktop.
    let reply = match conn
        .get_property(
            false,
            root,
            atoms._NET_CLIENT_LIST_STACKING,
            AtomEnum::WINDOW,
            0,
            4096,
        )
        .map_err(|e| format!("{e:?}"))
        .and_then(|c| c.reply().map_err(|e| format!("{e:?}")))
    {
        Ok(r) => r,
        Err(e) => {
            log::warn!("DDE: could not read _NET_CLIENT_LIST_STACKING: {e}");
            return Vec::new();
        }
    };
    let Some(values) = reply.value32() else {
        log::warn!(
            "DDE: _NET_CLIENT_LIST_STACKING has unexpected format {}",
            reply.format
        );
        return Vec::new();
    };
    values.collect()
}

/// Every client window whose WM_CLASS marks it as DDE's desktop — one per
/// screen on a multi-monitor session.
fn find_dde_desktop_windows<C: Connection>(conn: &C, atoms: &Atoms, root: Window) -> Vec<Window> {
    let clients = client_list_stacking(conn, atoms, root);
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
        if wm_class_is_dde_desktop(&prop.value) {
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

/// WM_CLASS is two NUL-terminated strings: instance, class.
fn wm_class_is_dde_desktop(value: &[u8]) -> bool {
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

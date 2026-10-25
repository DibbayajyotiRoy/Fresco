//! `frescod --saver` — the X11 lock-screen saver module (`docs/plan-lock-screen.md`
//! §4.3/§4.4): draws Fresco's wallpaper into the window a screensaver host
//! hands it, and nothing else.
//!
//! # What this is, and what it is deliberately not
//!
//! This is a **saver module**, in the vocabulary xsecurelock and the
//! MATE/Xfce screensaver family both use: a small, dumb renderer that a
//! locker spawns *behind* its own auth surface and kills on unlock. It never
//! grabs input, never sees a password, and never creates a window of its
//! own — it draws into the window `$XSCREENSAVER_WINDOW` names, which the
//! host already made and already owns. `xsecurelock`'s own core keeps sole
//! ownership of authentication and the input grab (`docs/plan-lock-screen.md`
//! §4.3); this process has no more access to that than any other unprivileged
//! program on the system.
//!
//! It runs as its own OS process, separate from `frescod` — the host spawns
//! it directly (`saver_child.c`'s `WatchSaverChild`, fetched from
//! xsecurelock's source), not the daemon. That is *why* it talks back to the
//! daemon over the same control-socket IPC the GUI uses
//! ([`crate::ipc::Request::LockNotify`]) rather than through some in-process
//! call: there is no process boundary to cross the other way. And it is why
//! the daemon can only reach this instance's mpv over JSON IPC
//! (`input-ipc-server`) rather than in-process function calls the way the
//! desktop X11 backend drives its own embedded `mpv::Player` — the exact same
//! shape `daemon::mpvpaper::MpvIpc` already drives an external `mpvpaper`
//! child's socket with, just pointed at a socket `frescod --saver` opens
//! instead.
//!
//! # `$XSCREENSAVER_WINDOW`: decimal or hex, one process per monitor
//!
//! Verified against xsecurelock's own source (`xscreensaver_api.c`,
//! `env_settings.c`, `helpers/saver_multiplex.c` — all fetched from
//! `github.com/google/xsecurelock`, master branch):
//!
//! - The host always *writes* this as plain decimal (`ExportWindowID`'s
//!   `"%llu"`), but the read side both it and every saver share,
//!   `GetUnsignedLongLongSetting`, parses with `strtoull(value, &endptr, 0)`
//!   — base `0` also accepts a `0x`/`0X` prefix as hex. [`parse_window_id`]
//!   mirrors exactly that (decimal or `0x`-hex; deliberately *not* octal —
//!   see its own doc comment).
//! - xsecurelock's default global saver, `saver_multiplex`, spawns **one
//!   saver process per physical monitor**, each in its own child window
//!   already sized and positioned to that one monitor's geometry
//!   (`SpawnSavers`/`GetMonitors`, built on XRandR — confirmed from
//!   `helpers/monitors.c`'s `#ifdef HAVE_XRANDR_EXT`). It also sets
//!   `$XSCREENSAVER_SAVER_INDEX`, but that is a bare 0-based slot number, not
//!   a connector name — there is no env var naming *which output* a given
//!   saver instance covers. `mate-screensaver`/`xfce4-screensaver` set the
//!   same `$XSCREENSAVER_WINDOW` (their `gs-job.c`'s `get_env_vars`, fetched
//!   from each project's source) but run only one saver for the whole
//!   screen, with no per-monitor split at all.
//!
//! So "which screen does this window cover" is answered the only way it can
//! be: by asking X11 what the window's own on-screen geometry is and
//! matching that against Fresco's own RandR monitor list, the same technique
//! `daemon::x11_fullscreen` already uses to match a fullscreen client window
//! to a monitor. See `pick_connector` (private — this module's own internal
//! resolution logic, not part of its public API).
//!
//! # mate-screensaver strips `$XDG_RUNTIME_DIR`; xfce4-screensaver does not
//!
//! A verified, real divergence between the two forks, not a guess:
//! mate-screensaver's `gs-job.c` (`get_env_vars`) builds the saver's
//! environment from a small allowlist — `PATH`, `SESSION_MANAGER`,
//! `XAUTHORITY`, `XAUTHLOCALHOSTNAME`, `LANG`, `LANGUAGE`,
//! `DBUS_SESSION_BUS_ADDRESS` — plus `DISPLAY`/`HOME`/`XSCREENSAVER_WINDOW`
//! it sets itself, and that allowlist does **not** include
//! `XDG_RUNTIME_DIR`. xfce4-screensaver forked the same file and *added*
//! `"XDG_RUNTIME_DIR"` to its own copy of the allowlist. xsecurelock does a
//! plain `fork()`+`execv()` with no custom environment at all
//! (`saver_child.c`), so it passes everything through unchanged. See
//! `runtime_dir` (private) for how this module copes.
//!
//! # Fail-open is not possible from here, by construction
//!
//! This process has no window of its own, no input grab, and no
//! authentication capability — there is no primitive available to it that
//! could ever leave a session unlocked. Its only failure modes are "nothing
//! is drawn" (exit non-zero before a `Player` exists) or "the wallpaper shows
//! but the daemon never hears about it" (a `LockNotify` that could not
//! reach the daemon — always ignored, never retried, never fatal: see the
//! module-level rule that every `crate::ipc::request_with_timeout` call here
//! discards its result).
//!
//! # Signals: xsecurelock's teardown contract
//!
//! xsecurelock kills a saver with `SIGTERM` on unlock
//! (`saver_child.c`/`wait_pgrp.c`'s `KillPgrp(pid, SIGTERM)`, fetched from its
//! source). `SIGINT`/`SIGHUP` are handled the same way defensively, for
//! manual runs and any other host. `SIGUSR1` is deliberately **not** treated
//! as teardown: that is xsecurelock's own "reset the saver" signal
//! (`saver_multiplex.c`'s `HandleSIGUSR1`), sent while the session is still
//! locked — see the private `TERMINATION_SIGNALS` constant.

use std::io::Read as _;
use std::os::fd::IntoRawFd as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use nix::sys::signal::{self, SaFlags, SigAction, SigHandler, SigSet, Signal};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;

use crate::config::{Config, Wallpaper};
use crate::daemon::monitors::Monitor;
use crate::ipc::{LockSocket, Request};

/// `XSCREENSAVER_WINDOW` is missing, doesn't parse, or doesn't name a real
/// window — the one exit code the wave-2 contract pins down explicitly.
pub const EXIT_BAD_WINDOW: i32 = 2;
/// Could not open any connection to the X server at all (no `$DISPLAY`, or
/// nothing listening on it). Distinct from [`EXIT_BAD_WINDOW`]: the window id
/// itself was never even reached.
pub const EXIT_NO_DISPLAY: i32 = 3;
/// libmpv failed to initialize (missing `libmpv.so`, or mpv itself rejected
/// the wallpaper file at load).
pub const EXIT_MPV_FAILED: i32 = 4;

/// Bounded wait for the daemon to answer a `LockNotify` — short because,
/// per the module docs, the result is discarded either way; this only
/// bounds how long a wallpaper that is already on screen waits before
/// giving up on telling anyone about it.
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(2);

/// Entry point for `frescod --saver`, called from `main` in `bin/frescod.rs`.
/// Never panics; every failure path logs, prints to stderr (xsecurelock
/// captures a saver's stderr into its own log — see its README), and returns
/// a documented exit code instead.
pub fn run() -> i32 {
    let env = match read_env() {
        Ok(e) => e,
        Err(msg) => return fail(EXIT_BAD_WINDOW, &msg),
    };

    let config = Config::load().unwrap_or_default();

    let (xconn, screen_num) = match x11rb::connect(None) {
        Ok(c) => c,
        Err(e) => {
            return fail(
                EXIT_NO_DISPLAY,
                &format!("cannot connect to the X server: {e}"),
            )
        }
    };
    let root = xconn.setup().roots[screen_num].root;

    let geometry = match window_absolute_geometry(&xconn, root, env.window) {
        Ok(g) => g,
        Err(e) => {
            return fail(
                EXIT_BAD_WINDOW,
                &format!(
                    "XSCREENSAVER_WINDOW=0x{:x} is not a usable window: {e}",
                    env.window
                ),
            )
        }
    };

    let monitors = crate::daemon::monitors::list_monitors(&xconn, root).unwrap_or_default();
    let connector = pick_connector(&monitors, geometry);
    if connector.is_none() {
        log::warn!(
            "saver: window 0x{:x} at {geometry:?} didn't match any of the {} monitor(s) Fresco \
             knows about — showing the default wallpaper",
            env.window,
            monitors.len(),
        );
    }

    let wallpaper = resolve_wallpaper(&config, connector.as_deref());
    let scaling = config.scaling;
    let power_saving = wallpaper.effective_power_saving(config.power_saving);

    let rt_dir = runtime_dir();
    if let Err(e) = std::fs::create_dir_all(&rt_dir) {
        log::warn!(
            "saver: could not create {} ({e}) — mpv's own IPC socket will likely fail to bind too",
            rt_dir.display()
        );
    }
    let sock_path = rt_dir.join(format!("lock-saver-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock_path); // best-effort: clear a stale file from a reused pid
    let sock_str = sock_path.to_string_lossy().into_owned();

    let player = match crate::daemon::mpv::Player::new_with_extra_options(
        env.window,
        &wallpaper,
        scaling,
        power_saving,
        config.low_memory,
        &[("input-ipc-server", sock_str.as_str())],
    ) {
        Ok(p) => p,
        Err(e) => return fail(EXIT_MPV_FAILED, &format!("mpv failed to start: {e:#}")),
    };

    // Live-video policy is the lock screen's own (`[lockscreen].live_video`),
    // not the desktop wallpaper's: a locked screen left up for hours is a
    // sustained decode cost the desktop rarely pays the same way (see
    // `crate::lockscreen::LiveVideo`'s own doc comment for the full case).
    // `false` freezes on whatever frame mpv already decoded, exactly the
    // "pause on battery" idiom `daemon::mod` uses elsewhere for the same
    // reason — not a separately-rendered still image.
    let live = wants_live(&config);
    player.set_paused(!live);
    log::info!(
        "saver: window=0x{:x} connector={connector:?} live={live} socket={sock_str}",
        env.window,
    );

    let pipe = install_signal_pipe();

    // Ignore the result, by design (see the module docs): the wallpaper is
    // already on screen either way, and a locked-out daemon must never be
    // this process's problem to solve.
    let _ = crate::ipc::request_with_timeout(
        &Request::LockNotify {
            locked: true,
            sockets: vec![LockSocket {
                connector: connector.unwrap_or_default(),
                path: sock_str.clone(),
            }],
        },
        NOTIFY_TIMEOUT,
    );

    wait_for_termination(pipe);

    let _ = crate::ipc::request_with_timeout(
        &Request::LockNotify {
            locked: false,
            sockets: Vec::new(),
        },
        NOTIFY_TIMEOUT,
    );
    drop(player);
    let _ = std::fs::remove_file(&sock_path);
    0
}

fn fail(code: i32, msg: &str) -> i32 {
    log::error!("saver: {msg}");
    eprintln!("frescod --saver: {msg}");
    code
}

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

/// Everything read from the environment at startup — gathered in one place,
/// mirroring `daemon::lock::hosts`' own `HostInputs`/`detect` split, so the
/// parsing in [`parse_window_id`] stays a pure function tests can drive
/// directly.
pub struct SaverEnv {
    pub window: u32,
}

/// Read and validate `$XSCREENSAVER_WINDOW`. The one impure entry point into
/// this section — everything it delegates to is pure.
pub fn read_env() -> Result<SaverEnv, String> {
    match std::env::var("XSCREENSAVER_WINDOW") {
        Ok(raw) => parse_window_id(&raw).map(|window| SaverEnv { window }).ok_or_else(|| {
            format!("XSCREENSAVER_WINDOW={raw:?} is not a valid, nonzero X11 window id (decimal or 0x-hex)")
        }),
        Err(_) => Err(
            "XSCREENSAVER_WINDOW is not set — frescod --saver must be launched by a screensaver \
             host (xsecurelock's XSECURELOCK_SAVER, or a mate-screensaver/xfce4-screensaver \
             theme), never run directly"
                .to_string(),
        ),
    }
}

/// Parse `$XSCREENSAVER_WINDOW`'s value into a raw X11 window id.
///
/// xsecurelock's own `ExportWindowID` (`xscreensaver_api.c`) always writes
/// this as plain decimal (`"%llu"`), but the read side every screensaver host
/// and saver share, `GetUnsignedLongLongSetting` (`env_settings.c`), parses
/// with `strtoull(value, &endptr, 0)` — base `0` also accepts a `0x`/`0X`
/// prefix as hexadecimal, so a hand-set `XSCREENSAVER_WINDOW=0x2c00007` (for
/// manual testing, say) works against a real host and must work here too.
///
/// Octal (`strtoull`'s third form: a bare leading zero) is deliberately
/// **not** accepted: no real host ever emits a leading-zero window id, and
/// silently reinterpreting one as octal would misread a plausible-looking
/// decimal value nobody intended as octal.
///
/// `0` (X11's `None`) and anything wider than 32 bits (every real XID is a
/// `CARD32`) both come back `None`, matching `saver_multiplex.c`'s own
/// `parent == None` rejection.
pub fn parse_window_id(raw: &str) -> Option<u32> {
    let raw = raw.trim();
    let value = match raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => raw.parse::<u32>().ok()?,
    };
    (value != 0).then_some(value)
}

// ---------------------------------------------------------------------------
// Which monitor, which wallpaper
// ---------------------------------------------------------------------------

/// `window`'s geometry in root-window coordinates: `(x, y, width, height)`.
/// The same translation `daemon::x11_fullscreen::absolute_geometry` does for
/// a fullscreen client window, applied here to the window a screensaver host
/// handed us instead of one Fresco created.
fn window_absolute_geometry<C: Connection>(
    conn: &C,
    root: u32,
    window: u32,
) -> Result<(i32, i32, u32, u32), String> {
    let geo = conn
        .get_geometry(window)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    let abs = conn
        .translate_coordinates(window, root, 0, 0)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    Ok((
        i32::from(abs.dst_x),
        i32::from(abs.dst_y),
        u32::from(geo.width),
        u32::from(geo.height),
    ))
}

/// Which connector (if any) a window at `geometry` belongs to, by matching it
/// against `monitors`' own rectangles — see the module docs for why this is
/// the only way back to a connector name at all under xsecurelock's
/// multiplexer, which hands each saver a window plus a bare numeric index,
/// never a connector name.
///
/// `None` when nothing overlaps (no monitors known, or the window spans more
/// than any single monitor explains — e.g. a whole-desktop saver set via
/// `XSECURELOCK_GLOBAL_SAVER`); callers fall back to the default wallpaper,
/// the same "no per-output override" behaviour the desktop backend itself
/// falls back to.
fn pick_connector(monitors: &[Monitor], geometry: (i32, i32, u32, u32)) -> Option<String> {
    monitors
        .iter()
        .map(|m| (m, overlap_area(geometry, m)))
        .filter(|(_, area)| *area > 0)
        .max_by_key(|(_, area)| *area)
        .map(|(m, _)| m.connector.clone())
}

/// Intersection area (px²) between the rectangle `(x, y, w, h)` and monitor
/// `m` — `0` when they don't overlap at all. Same shape as
/// `daemon::x11_fullscreen::overlap_at_least_half`'s own math, but returning
/// the raw area rather than a yes/no past a 50% threshold: picking the *best*
/// match matters here, since a saver window should exactly equal one
/// monitor's rectangle in the common case and this is only a tie-breaker for
/// when it doesn't line up perfectly.
fn overlap_area((x, y, w, h): (i32, i32, u32, u32), m: &Monitor) -> i64 {
    let (mx, my) = (i32::from(m.x), i32::from(m.y));
    let (mw, mh) = (i32::from(m.width), i32::from(m.height));
    let ix = (x + w as i32).min(mx + mw) - x.max(mx);
    let iy = (y + h as i32).min(my + mh) - y.max(my);
    if ix <= 0 || iy <= 0 {
        return 0;
    }
    i64::from(ix) * i64::from(iy)
}

/// The wallpaper this saver shows for `connector` (or the default, when
/// `connector` is `None` or names a monitor with no override) — always with
/// audio forced off: a locked screen must never play sound, regardless of
/// what the desktop wallpaper's own `mute` setting says.
fn resolve_wallpaper(config: &Config, connector: Option<&str>) -> Wallpaper {
    let mut w = connector
        .map(|c| config.wallpaper_for(c))
        .unwrap_or(&config.wallpaper)
        .clone();
    w.mute = true;
    w
}

/// Whether the lock wallpaper should currently play as live video — the lock
/// screen's own policy (`config.lockscreen.live_video`), not the desktop
/// wallpaper's own always-play behaviour. Absent `[lockscreen]` resolves to
/// [`crate::config::LiveVideo::Ac`] (its documented default), which is a
/// reasonable answer even when the feature itself was never configured: this
/// saver draws the plain wallpaper regardless of `[lockscreen].enabled` (see
/// the module docs) — the *widget* layer is what that flag gates, and that
/// lives on the daemon's side of the socket this module reports, not here.
fn wants_live(config: &Config) -> bool {
    wants_live_given(config, crate::battery::on_battery())
}

/// [`wants_live`], with the battery check passed in — the pure half, tested
/// directly.
fn wants_live_given(config: &Config, on_battery: bool) -> bool {
    config
        .lockscreen
        .as_ref()
        .map(|l| l.live_video)
        .unwrap_or_default()
        .plays(on_battery)
}

// ---------------------------------------------------------------------------
// Runtime directory
// ---------------------------------------------------------------------------

/// `$XDG_RUNTIME_DIR/fresco` — the same directory `crate::ipc::socket_dir()`
/// computes for the daemon's own control socket, with one extra fallback
/// tier beyond what that function has: see the module docs for the verified
/// fact that mate-screensaver's `gs-job.c` does not forward `XDG_RUNTIME_DIR`
/// to the saver process it spawns, while xfce4-screensaver's fork of the same
/// file does. Probing `/run/user/<uid>` directly — the path
/// `XDG_RUNTIME_DIR` names on every systemd-based system anyway — recovers
/// the same directory in that case, before falling through to
/// `crate::ipc::socket_dir`'s own `/tmp/fresco-<uid>` last resort.
///
/// This only fixes where *this* process's own files land (its mpv IPC
/// socket). It cannot fix `crate::ipc::request` finding the daemon's control
/// socket, since that path is computed inside `ipc.rs`, not here — under
/// mate-screensaver, `LockNotify` may still fail to reach the daemon for the
/// same underlying reason, which is exactly why this module never treats
/// that failure as fatal.
fn runtime_dir() -> PathBuf {
    if let Some(dir) = dirs::runtime_dir() {
        return dir.join("fresco");
    }
    if let Some(dir) = fallback_run_user_dir() {
        return dir.join("fresco");
    }
    crate::ipc::socket_dir()
}

/// `/run/user/<uid>`, if it exists — see [`runtime_dir`]. Same
/// no-`libc`-dependency uid lookup as `ipc.rs`'s private `libc_getuid` and
/// `userinfo.rs`'s `current_uid`.
fn fallback_run_user_dir() -> Option<PathBuf> {
    let uid = std::fs::metadata("/proc/self")
        .ok()
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))?;
    let guess = PathBuf::from(format!("/run/user/{uid}"));
    guess.is_dir().then_some(guess)
}

// ---------------------------------------------------------------------------
// Signals
// ---------------------------------------------------------------------------

/// Signals that mean "the host is done with this saver". See the module docs
/// for exactly which hosts send which of these, and why `SIGUSR1` is
/// deliberately excluded. [`install_signal_pipe`] only ever registers a
/// handler for these three, so the handler itself never has to check —
/// pinned down by `termination_signals_are_exactly_term_int_hup` below
/// instead, so "exactly these three" stays a checked fact.
const TERMINATION_SIGNALS: [Signal; 3] = [Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP];

/// Write end of the self-pipe, for the signal handler. `-1` until installed.
static WAKE_FD: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_signal(sig: nix::libc::c_int) {
    let fd = WAKE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = sig as u8;
        // SAFETY: `write` is async-signal-safe; `fd` is the pipe's write end,
        // which this module never closes once installed. Mirrors
        // `daemon::signals::on_signal` exactly, for the same reason.
        unsafe {
            nix::libc::write(fd, (&byte as *const u8).cast(), 1);
        }
    }
}

/// Install handlers for [`TERMINATION_SIGNALS`] and return the self-pipe's
/// read end, or `None` if the pipe itself could not be created — in which
/// case every one of those signals keeps its default "terminate immediately"
/// disposition, which is still safe (see the module docs' "fail-open is not
/// possible" section); it just means [`wait_for_termination`] blocks forever
/// instead, and the best-effort `LockNotify{locked:false}` on the way out
/// never runs.
///
/// Same self-pipe idiom as `daemon::signals::install` (a handler cannot
/// safely do anything beyond an async-signal-safe `write`) — kept as its own,
/// smaller copy rather than shared, because that module's read side turns
/// the byte into *this daemon's* `Request::Stop`, which has nothing to do
/// with this process's very different teardown (report unlocked, drop the
/// player, exit).
fn install_signal_pipe() -> Option<std::fs::File> {
    let (read_end, write_end) = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC).ok()?;
    WAKE_FD.store(write_end.into_raw_fd(), Ordering::Relaxed);
    let action = SigAction::new(
        SigHandler::Handler(on_signal),
        SaFlags::SA_RESTART,
        SigSet::empty(),
    );
    for sig in TERMINATION_SIGNALS {
        // SAFETY: the handler only performs an async-signal-safe `write`.
        if let Err(e) = unsafe { signal::sigaction(sig, &action) } {
            log::warn!(
                "saver: could not install a handler for {} ({e})",
                sig.as_str()
            );
        }
    }
    Some(std::fs::File::from(read_end))
}

/// Block until a termination signal arrives, then restore the default
/// disposition for all of [`TERMINATION_SIGNALS`] so a *second* one takes
/// immediate effect — the way out of a teardown that hangs, exactly
/// `daemon::signals::wait_and_stop`'s own "a second Ctrl-C still gets out"
/// rule.
fn wait_for_termination(pipe: Option<std::fs::File>) {
    let Some(mut pipe) = pipe else {
        // No self-pipe — block forever. The default disposition for every
        // signal in `TERMINATION_SIGNALS` is still "terminate", so this can
        // never leave a stray saver process running; it only means the
        // best-effort `LockNotify{locked:false}` below never gets a chance
        // to run.
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    };
    let mut byte = [0u8; 1];
    if pipe.read_exact(&mut byte).is_err() {
        return; // pipe closed/errored — nothing left to wait for
    }
    for sig in TERMINATION_SIGNALS {
        // SAFETY: restoring the default disposition has no preconditions.
        let _ = unsafe { signal::signal(sig, SigHandler::SigDfl) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LockScreen;

    // -- parse_window_id ------------------------------------------------------

    #[test]
    fn parse_window_id_decimal() {
        assert_eq!(parse_window_id("46137345"), Some(46137345));
        assert_eq!(parse_window_id("  123  "), Some(123), "trims whitespace");
    }

    #[test]
    fn parse_window_id_hex() {
        assert_eq!(parse_window_id("0x2c00001"), Some(0x2c00001));
        assert_eq!(parse_window_id("0X2C00001"), Some(0x2c00001));
    }

    #[test]
    fn parse_window_id_zero_is_invalid() {
        // X11's `None` — `saver_multiplex.c` rejects it the same way.
        assert_eq!(parse_window_id("0"), None);
        assert_eq!(parse_window_id("0x0"), None);
    }

    #[test]
    fn parse_window_id_octal_is_not_special_cased() {
        // A leading zero is read as decimal, never octal — see the doc
        // comment on why. "010" is decimal ten, not octal eight.
        assert_eq!(parse_window_id("010"), Some(10));
    }

    #[test]
    fn parse_window_id_garbage_is_none() {
        for s in ["", "   ", "not-a-number", "0xzz", "-5", "12.5", "0x"] {
            assert_eq!(parse_window_id(s), None, "{s:?}");
        }
    }

    #[test]
    fn parse_window_id_overflow_is_none() {
        // Every real XID is a 32-bit CARD32; wider values are never valid.
        assert_eq!(parse_window_id("99999999999999999999"), None);
        assert_eq!(parse_window_id("0xFFFFFFFFF"), None);
        assert_eq!(parse_window_id("0xFFFFFFFF"), Some(u32::MAX));
    }

    // -- pick_connector / overlap_area -----------------------------------------

    fn mon(connector: &str, x: i16, y: i16, w: u16, h: u16) -> Monitor {
        Monitor {
            connector: connector.into(),
            x,
            y,
            width: w,
            height: h,
            scale_milli: 1000,
        }
    }

    #[test]
    fn pick_connector_exact_match() {
        let monitors = [
            mon("eDP-1", 0, 0, 1920, 1080),
            mon("HDMI-1", 1920, 0, 2560, 1440),
        ];
        assert_eq!(
            pick_connector(&monitors, (0, 0, 1920, 1080)),
            Some("eDP-1".to_string())
        );
        assert_eq!(
            pick_connector(&monitors, (1920, 0, 2560, 1440)),
            Some("HDMI-1".to_string())
        );
    }

    #[test]
    fn pick_connector_picks_the_larger_overlap() {
        // A window mostly on HDMI-1 but a sliver over eDP-1 (adjacent
        // monitors) must resolve to the monitor it mostly covers.
        let monitors = [
            mon("eDP-1", 0, 0, 1920, 1080),
            mon("HDMI-1", 1920, 0, 1920, 1080),
        ];
        assert_eq!(
            pick_connector(&monitors, (1900, 0, 1920, 1080)),
            Some("HDMI-1".to_string())
        );
    }

    #[test]
    fn pick_connector_none_when_nothing_overlaps() {
        assert_eq!(pick_connector(&[], (0, 0, 1920, 1080)), None);
        let monitors = [mon("eDP-1", 0, 0, 1920, 1080)];
        assert_eq!(pick_connector(&monitors, (5000, 5000, 100, 100)), None);
    }

    #[test]
    fn overlap_area_math() {
        let m = mon("A", 0, 0, 1920, 1080);
        assert_eq!(overlap_area((0, 0, 1920, 1080), &m), 1920 * 1080);
        assert_eq!(overlap_area((1920, 0, 100, 100), &m), 0, "no overlap");
        assert_eq!(
            overlap_area((-10, -10, 30, 30), &m),
            400,
            "partial corner overlap"
        );
    }

    // -- resolve_wallpaper ------------------------------------------------------

    fn config_with_override(default_path: &str, connector: &str, override_path: &str) -> Config {
        let mut config = Config::default();
        config.wallpaper.path = Some(default_path.into());
        config.wallpaper.mute = false;
        let mut ov = config.wallpaper.clone();
        ov.path = Some(override_path.into());
        config.monitors.insert(connector.to_string(), ov);
        config
    }

    #[test]
    fn resolve_wallpaper_uses_the_per_monitor_override_when_present() {
        let config = config_with_override("/default.mp4", "eDP-1", "/laptop.mp4");
        let w = resolve_wallpaper(&config, Some("eDP-1"));
        assert_eq!(w.effective_path().unwrap().to_str(), Some("/laptop.mp4"));
    }

    #[test]
    fn resolve_wallpaper_falls_back_to_the_default_wallpaper() {
        let config = config_with_override("/default.mp4", "eDP-1", "/laptop.mp4");
        // No connector identified at all.
        let w = resolve_wallpaper(&config, None);
        assert_eq!(w.effective_path().unwrap().to_str(), Some("/default.mp4"));
        // A connector identified, but with no override configured for it.
        let w = resolve_wallpaper(&config, Some("HDMI-1"));
        assert_eq!(w.effective_path().unwrap().to_str(), Some("/default.mp4"));
    }

    #[test]
    fn resolve_wallpaper_always_forces_mute() {
        let mut config = Config::default();
        config.wallpaper.mute = false;
        assert!(resolve_wallpaper(&config, None).mute);

        let config = config_with_override("/default.mp4", "eDP-1", "/laptop.mp4");
        assert!(resolve_wallpaper(&config, Some("eDP-1")).mute);
    }

    // -- wants_live_given ---------------------------------------------------

    #[test]
    fn wants_live_follows_the_lockscreen_live_video_policy() {
        use crate::config::LiveVideo;

        let mut config = Config {
            lockscreen: Some(LockScreen {
                live_video: LiveVideo::Ac,
                ..LockScreen::default()
            }),
            ..Config::default()
        };
        assert!(wants_live_given(&config, false), "AC power: plays");
        assert!(
            !wants_live_given(&config, true),
            "on battery: falls back to still"
        );

        config.lockscreen.as_mut().unwrap().live_video = LiveVideo::Always;
        assert!(wants_live_given(&config, true));

        config.lockscreen.as_mut().unwrap().live_video = LiveVideo::Never;
        assert!(!wants_live_given(&config, false));
    }

    #[test]
    fn wants_live_defaults_to_ac_policy_when_lockscreen_is_unset() {
        // The saver draws the plain wallpaper regardless of
        // `[lockscreen].enabled` (see the module docs); `LockScreen::default()`
        // still gives a sensible answer when the section was never configured.
        let config = Config::default();
        assert!(config.lockscreen.is_none());
        assert!(wants_live_given(&config, false));
        assert!(!wants_live_given(&config, true));
    }

    // -- signals --------------------------------------------------------------

    #[test]
    fn termination_signals_are_exactly_term_int_hup() {
        assert_eq!(
            TERMINATION_SIGNALS,
            [Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP]
        );
        // SIGUSR1 is xsecurelock's own "reset the saver" signal, sent while
        // still locked — must never be mistaken for teardown.
        assert!(!TERMINATION_SIGNALS.contains(&Signal::SIGUSR1));
        assert!(!TERMINATION_SIGNALS.contains(&Signal::SIGKILL));
    }

    // -- smoke tests for the impure glue --------------------------------------

    #[test]
    fn runtime_dir_does_not_panic() {
        let dir = runtime_dir();
        assert!(dir.ends_with("fresco"));
    }

    #[test]
    fn read_env_without_the_variable_is_a_clear_error_not_a_panic() {
        // Best-effort: only meaningful when nothing else in this test binary
        // has set the variable process-wide (env vars are global state), but
        // never flaky either way since both arms just assert "did not panic
        // and produced a message" rather than a specific Ok/Err.
        let _ = std::env::var("XSCREENSAVER_WINDOW"); // don't touch it either way
        let result = read_env();
        if let Err(msg) = result {
            assert!(!msg.is_empty());
        }
    }
}

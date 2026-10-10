//! Polished command-line surface for the `fresco` binary.
//!
//! `fresco doctor` / `fresco status` / `fresco logs` / `fresco lock` run
//! without launching the GUI. They reflect the running daemon over IPC and
//! the detected session capability — the user never needs to know about
//! layer-shell, EGL, or mpvpaper. Anything Fresco can't do is reported as a
//! plain-language hint, not a stack trace.
//!
//! # `fresco lock`
//!
//! Asks `frescod` to lock the session through this desktop's own lock host
//! (`daemon::lock::hosts` — `loginctl lock-session`, swaylock-plugin,
//! xsecurelock, …), so Fresco's wallpaper/widgets show through it wherever
//! the host allows. If the daemon can't be reached, or its own host adapter
//! fails, `lock_cmd` falls through to `fallback_lock`: a small, independent
//! chain that tries progressively more generic lockers and ends,
//! unconditionally, at `loginctl lock-session` — matching
//! `docs/plan-lock-screen.md` §5's fail-closed invariant that `fresco lock`
//! must always end locked, never exit 0 having started nothing.
//!
//! This module cannot simply call `daemon::lock::hosts::classify` for that
//! fallback: `daemon::lock` is private to the `daemon` module (owned by a
//! different slice of this feature), and the whole `daemon` module tree only
//! exists under the `daemon` Cargo feature, while `cli.rs` must keep
//! compiling under `gui` alone (see `fresco`'s binary target,
//! `required-features = ["gui"]`, with no `daemon` feature implied). So
//! `classify_session` below is a small, deliberately simpler, always-compiled
//! echo of that same idea — see its own doc comment. (These are private
//! functions, referenced here in code font rather than as doc-links, which
//! rustdoc cannot resolve for a private item from a public module's docs.)

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::capability::{
    detect, gnome_shell_version, gnome_x11_session_available, is_gnome_session, Capability,
};
use crate::config::Config;
use crate::ipc::{request, request_with_timeout, Request, Response, StatusReply};

const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Route the command line. Returns `Some(exit_code)` when this is a CLI
/// invocation (the caller must exit with it), or `None` to launch the GUI.
///
/// A typo'd subcommand (`fresco foo`) is a CLI error — it must NOT fall through
/// to GTK. Genuine toolkit options (`fresco --gapplication-service`, `--display`)
/// are left for the GUI's option parser, so D-Bus activation / launch still work.
pub fn dispatch(args: &[String]) -> Option<i32> {
    match args.get(1).map(String::as_str) {
        // No subcommand → launch the GUI.
        None => None,
        Some("doctor") => Some(doctor()),
        Some("status") => Some(status()),
        Some("logs") => Some(logs(args.get(2).map(String::as_str))),
        Some("lock") => Some(lock_cmd()),
        Some("-h") | Some("--help") | Some("help") => {
            print_help();
            Some(0)
        }
        Some("-V") | Some("-v") | Some("--version") | Some("version") => {
            println!("fresco {}", env!("CARGO_PKG_VERSION"));
            Some(0)
        }
        // Toolkit options are not ours — let the GUI parse them.
        Some(opt) if opt.starts_with('-') => None,
        // An unrecognized word is a CLI typo, not a GUI launch.
        Some(other) => {
            eprintln!("error: unknown command '{other}'");
            eprintln!("Run `fresco --help` for usage.");
            Some(2)
        }
    }
}

fn print_help() {
    println!(
        "Fresco — live wallpapers for Linux\n\n\
         Usage:\n  \
         fresco            Launch the app\n  \
         fresco lock       Lock the screen now, through your desktop's own locker\n  \
         fresco doctor     Show session, backend, and health diagnostics\n  \
         fresco status     Show the running wallpaper's status\n  \
         fresco logs [N]   Show the last N daemon log lines (default 50)\n  \
         fresco --version  Show the version (also -V, -v, version)\n  \
         fresco --help     Show this help\n\n\
         Bind `fresco lock` to a key or an idle daemon:\n  \
         Sway:      bindsym $mod+Escape exec fresco lock\n  \
         hypridle:  lock_cmd = fresco lock\n  \
         swayidle:  timeout 300 'fresco lock' before-sleep 'fresco lock'"
    );
}

// ── doctor ───────────────────────────────────────────────────────────────────

fn doctor() -> i32 {
    let cap = detect();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
    let st = daemon_status();
    // The still-frame backend is GNOME's, bar the rare old-muffin Cinnamon (or
    // any other compositor without layer-shell) that lands in it too: only a
    // real GNOME session has a Shell to ask the version of or an Xorg
    // GNOME/Ubuntu session to log into instead.
    let gnome_static = matches!(cap, Capability::WaylandGnomeStatic) && is_gnome_session();
    let x11_session = gnome_static && gnome_x11_session_available();

    println!("{BOLD}Fresco doctor{RESET}\n");
    println!("  Session       {}", session_label(cap));
    println!("  Compositor    {desktop}");
    if gnome_static {
        if let Some(version) = gnome_shell_version() {
            println!("  GNOME Shell   {version}");
        }
    }
    println!("  Backend       {}", backend_label(cap));
    if gnome_static {
        println!(
            "  X11 session   {}",
            if x11_session { "available" } else { "none" }
        );
    }
    if let Some(gpu) = gpu_name() {
        println!("  GPU           {gpu}");
    }
    if let Some(n) = st.as_ref().map(|s| s.monitors.len()).filter(|n| *n > 0) {
        println!("  Outputs       {n}");
    }
    // The GUI's toolkit, as a bug report needs it. The version functions read
    // constants out of the loaded library, so no display is opened and this
    // works over SSH and in a container.
    #[cfg(feature = "gui")]
    println!("  GTK           {}", gtk_version_label());
    if let Some(r) = std::env::var("GSK_RENDERER")
        .ok()
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
    {
        println!("  GSK renderer  {r} {DIM}(from GSK_RENDERER){RESET}");
    }
    if let Ok(cfg) = Config::load() {
        println!(
            "  Hover preview {}",
            if cfg.hover_previews { "on" } else { "off" }
        );
    }

    println!("\n{BOLD}Checks{RESET}");
    let mut problems = 0u32;

    check("Session detected", true, "", &mut problems);
    match cap {
        Capability::X11 | Capability::WaylandLayerShell => {
            check("Live wallpaper supported", true, "", &mut problems)
        }
        Capability::WaylandGnomeStatic => warn(
            "Live wallpaper supported",
            still_frame_hint(gnome_static, x11_session),
        ),
    }
    check(
        "Hardware acceleration",
        hwaccel_available(),
        "install mesa-va-drivers / intel-media-va-driver",
        &mut problems,
    );
    // mpvpaper only matters on layer-shell Wayland (it's how we render there).
    if matches!(cap, Capability::WaylandLayerShell) {
        // Report *which* mpvpaper we picked, not just that one exists. The bug
        // this exists for renders a black wallpaper with no error anywhere, and
        // the only distinguishing facts are the path, the provenance and the
        // version — so print all three rather than making the next reporter
        // strace the daemon to find them.
        match crate::mpvpaper_describe().filter(|c| c.path.is_file()) {
            Some(c) => {
                println!(
                    "  {GREEN}✓{RESET} mpvpaper available {DIM}({}){RESET}",
                    c.path.display()
                );
                println!(
                    "      {DIM}source: {} · version: {}{RESET}",
                    c.source.label(),
                    c.version_label()
                );
                // mpvpaper before 1.6 initialises EGL, reports success and then
                // draws nothing on the NVIDIA proprietary driver — a black
                // wallpaper with a totally clean log. Upstream fixed it in 1.6
                // and reworked the compositor render-loop handshake again in
                // 1.7. We cannot tell 1.4 from 1.6 apart (identical --help), so
                // warn whenever we cannot *prove* the renderer is new enough.
                if c.maybe_predates_nvidia_fix() {
                    warn(
                        "mpvpaper may be too old",
                        "this build predates (or may predate) mpvpaper 1.6/1.7, which fixed black output on the NVIDIA proprietary driver — if your wallpaper is black, install a newer mpvpaper and set FRESCO_MPVPAPER=/path/to/mpvpaper",
                    );
                }
            }
            None => {
                problems += 1;
                match crate::mpvpaper_broken() {
                    Some(p) => println!(
                        "  {RED}✗{RESET} mpvpaper available {DIM}({} exists but fails to load — likely a libmpv version mismatch; update Fresco, install the matching libmpv, or set FRESCO_MPVPAPER=/path/to/mpvpaper){RESET}",
                        p.display()
                    ),
                    None => println!(
                        "  {RED}✗{RESET} mpvpaper available {DIM}install or build mpvpaper (then set FRESCO_MPVPAPER=/path/to/mpvpaper), or use the .deb/Flatpak release which bundles it{RESET}"
                    ),
                }
            }
        }
    }
    // Widget helpers. Both are `warn`, never `problems`: a desktop with no
    // widgets enabled is perfectly healthy without either, so a red ✗ (and a
    // non-zero exit) would be crying wolf. They are listed at all because the
    // failure they cause is silence — a widget that is on in the config and
    // simply never draws — and a diagnostic that cannot explain that is not
    // doing its job. See src/daemon/widgets.rs for the matching runtime logs.
    if which("gdbus") {
        check("Now-playing widgets (gdbus)", true, "", &mut problems);
    } else {
        warn(
            "Now-playing widgets (gdbus)",
            "lyrics / album art / track-synced clock need gdbus — install libglib2.0-bin",
        );
    }
    if which("pw-cat") || which("parec") {
        check("Audio visualiser (pw-cat/parec)", true, "", &mut problems);
    } else {
        warn(
            "Audio visualiser (pw-cat/parec)",
            "install pipewire-bin or pulseaudio-utils to enable the visualiser widget",
        );
    }

    let configured = Config::load()
        .map(|c| {
            c.enabled && (c.wallpaper.effective_path().is_some() || !c.wallpaper.paths.is_empty())
        })
        .unwrap_or(false);
    if configured {
        check("Wallpaper configured", true, "", &mut problems);
    } else {
        warn("Wallpaper configured", "none yet — open Fresco to set one");
    }

    println!();
    if problems == 0 {
        println!("{GREEN}System healthy{RESET}");
        0
    } else {
        println!("{YELLOW}{problems} issue(s) found{RESET}");
        1
    }
}

// ── status ───────────────────────────────────────────────────────────────────

fn status() -> i32 {
    match daemon_status() {
        Some(s) => {
            println!("{BOLD}Fresco{RESET}");
            println!("  Backend     {}", backend_label(detect()));
            println!("  Wallpaper   {}", s.wallpaper.as_deref().unwrap_or("—"));
            if !s.monitors.is_empty() {
                println!("  Outputs     {}", s.monitors.len());
            }
            println!("  Decode      {}", decode_label(s.hwdec.as_deref()));
            if let (Some(w), Some(h)) = (s.source_w, s.source_h) {
                let depth = s
                    .bit_depth
                    .map(|d| format!(" · {d}-bit"))
                    .unwrap_or_default();
                println!("  Source      {w}x{h}{depth}");
            }
            if let Some(n) = s.dropped_frames {
                if n > 0 {
                    println!("  Dropped     {n} frames");
                }
            }
            println!("  Memory      {} MB", s.rss_mb);
            println!("  Paused      {}", if s.paused { "yes" } else { "no" });
            // Decode honesty: software-decoding ≥4K is the classic silent cause
            // of stutter/artifacts — name it instead of leaving it a mystery.
            let software = matches!(s.hwdec.as_deref(), None | Some("no") | Some(""));
            if software && s.source_h.unwrap_or(0) >= 2160 {
                println!(
                    "  {YELLOW}Note        this source is ≥4K and your GPU is not \
                     hardware-decoding it (codec/size unsupported) — expect high CPU \
                     and possible dropped frames{RESET}"
                );
            }
            if let Some(e) = s.error {
                println!("  {YELLOW}Note        {e}{RESET}");
            }
        }
        None => {
            println!("Fresco isn't running. Open the app or set a wallpaper to start it.");
        }
    }
    0
}

// ── logs ─────────────────────────────────────────────────────────────────────

fn logs(arg: Option<&str>) -> i32 {
    let path = daemon_log_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("No daemon log yet at {}", path.display());
        return 0;
    };
    let n: usize = arg.and_then(|a| a.parse().ok()).unwrap_or(50);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    for line in &lines[start..] {
        println!("{line}");
    }
    0
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn daemon_status() -> Option<StatusReply> {
    match request(&Request::Status) {
        Ok(Response::Status(s)) => Some(s),
        _ => None,
    }
}

fn session_label(cap: Capability) -> &'static str {
    match cap {
        Capability::X11 => "X11",
        Capability::WaylandLayerShell | Capability::WaylandGnomeStatic => "Wayland",
    }
}

fn backend_label(cap: Capability) -> String {
    match cap {
        Capability::X11 => "X11 (embedded mpv)".into(),
        Capability::WaylandGnomeStatic => "static frame (GNOME Wayland)".into(),
        Capability::WaylandLayerShell => "mpvpaper (layer-shell)".into(),
    }
}

/// Why a still-frame session has no live wallpaper, and what (if anything) the
/// user can do about it. `gnome` is whether this is a real GNOME session;
/// `x11_session` whether an Xorg GNOME/Ubuntu session is installed to log into.
/// Ubuntu 25.10+ / 26.04 LTS, Fedora 43+ and GNOME 50 ship none, so "use an
/// Xorg session" is only offered when one exists.
fn still_frame_hint(gnome: bool, x11_session: bool) -> &'static str {
    match (gnome, x11_session) {
        (true, true) => {
            "GNOME on Wayland can only show a still frame — log out and choose the GNOME/Ubuntu on Xorg session for live video"
        }
        (true, false) => {
            "GNOME on Wayland can't play video wallpapers yet and no Xorg session is installed to fall back to — live video works on KDE Plasma, COSMIC, Hyprland, Sway and X11 desktops; a Fresco GNOME extension is planned"
        }
        (false, _) => "this Wayland compositor has no layer-shell, so Fresco can only show a still frame",
    }
}

fn decode_label(hwdec: Option<&str>) -> String {
    match hwdec {
        Some("no") | None => "software".into(),
        Some(x) => format!("hardware ({x})"),
    }
}

/// Print a check line. `fail_hint` (a remedy) is shown only when the check
/// fails — never on a passing line, where it would read like unwanted advice.
fn check(label: &str, ok: bool, fail_hint: &str, problems: &mut u32) {
    if ok {
        println!("  {GREEN}✓{RESET} {label}");
    } else {
        *problems += 1;
        if fail_hint.is_empty() {
            println!("  {RED}✗{RESET} {label}");
        } else {
            println!("  {RED}✗{RESET} {label} {DIM}{fail_hint}{RESET}");
        }
    }
}

fn warn(label: &str, hint: &str) {
    println!("  {YELLOW}⚠{RESET} {label} {DIM}{hint}{RESET}");
}

fn hwaccel_available() -> bool {
    std::path::Path::new("/dev/dri/renderD128").exists() || which("vainfo")
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

/// The GTK runtime version, e.g. `4.14.5`. Needs no `gtk::init`.
#[cfg(feature = "gui")]
fn gtk_version_label() -> String {
    format!(
        "{}.{}.{}",
        gtk4::major_version(),
        gtk4::minor_version(),
        gtk4::micro_version()
    )
}

fn gpu_name() -> Option<String> {
    let out = Command::new("sh")
        .arg("-c")
        .arg("lspci | grep -Ei 'vga|3d|display' | sed 's/.*: //'")
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

fn daemon_log_path() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fresco")
        .join("frescod.log")
}

// ── lock ─────────────────────────────────────────────────────────────────────

/// [`request_with_timeout`]'s budget for the initial `Request::Lock`
/// round-trip. Longer than [`crate::ipc`]'s default 5s IPC timeout on
/// purpose: `frescod`'s own wlroots host can itself wait up to 10s for
/// swaylock-plugin to confirm a lock (`daemon::lock::hosts::wlroots`'s
/// `LOCK_TIMEOUT`) before it even replies, so a shorter timeout here would
/// misreport a slow-but-succeeding lock as "daemon unreachable" and trigger
/// the fallback chain needlessly.
const LOCK_REQUEST_TIMEOUT: Duration = Duration::from_secs(12);

/// `fresco lock`: ask the daemon to lock through this desktop's own host
/// adapter, falling back to a small, independent chain of well-known lockers
/// if the daemon can't do it. Fail-closed by construction
/// (`docs/plan-lock-screen.md` §5): every path through this function either
/// prints confirmation of a real, running locker and returns 0, or prints an
/// error and returns 1 — it never returns 0 without something actually
/// holding the lock.
fn lock_cmd() -> i32 {
    match request_with_timeout(&Request::Lock, LOCK_REQUEST_TIMEOUT) {
        Ok(Response::Lock(reply)) if reply.ok => {
            match reply.message {
                Some(m) => eprintln!("Locked via {} ({m})", reply.host),
                None => eprintln!("Locked via {}", reply.host),
            }
            0
        }
        Ok(Response::Lock(reply)) => {
            eprintln!(
                "frescod could not lock via {}: {} — falling back",
                reply.host,
                reply.message.as_deref().unwrap_or("unknown error")
            );
            fallback_lock()
        }
        Ok(Response::Err { message }) => {
            eprintln!("frescod refused the lock request: {message} — falling back");
            fallback_lock()
        }
        Ok(_) => {
            eprintln!("frescod sent an unexpected reply to the lock request — falling back");
            fallback_lock()
        }
        // Every failure used to land here and go straight to `fallback_lock`
        // — correct when the daemon was never reached, but wrong when it WAS
        // reached and simply hadn't replied yet: the wlroots host can
        // legitimately take up to 10s to hear back from swaylock-plugin
        // before it even answers this request, and a slow system can push
        // that past LOCK_REQUEST_TIMEOUT. Falling back blindly there starts a
        // second locker on a session the first one is about to finish
        // locking. `classify_connect_error` tells the two situations apart.
        Err(e) => {
            let outcome = match classify_connect_error(&e) {
                Reachability::Unreachable => LockRequestOutcome::Unreachable,
                Reachability::Connected => {
                    eprintln!(
                        "frescod did not confirm the lock within {LOCK_REQUEST_TIMEOUT:?} — it \
                         may still be finishing its own locker; checking whether the session is \
                         locked already"
                    );
                    match probe_lock_status() {
                        StatusProbeOutcome::Locked => LockRequestOutcome::ConnectedAndLocked,
                        StatusProbeOutcome::NotConfirmed => LockRequestOutcome::ConnectedNotLocked,
                    }
                }
            };
            match decide_lock_action(outcome) {
                LockAction::ReportAlreadyLocked => {
                    eprintln!("Session is already locked");
                    0
                }
                LockAction::Fallback => fallback_lock(),
            }
        }
    }
}

/// [`probe_lock_status`]'s timeout — short, because by the time it runs the
/// daemon has almost certainly already finished handling the original
/// `Request::Lock` (its control loop handles one request at a time, and
/// [`LOCK_REQUEST_TIMEOUT`] already exceeds the wlroots host's own worst-case
/// wait), so there is nothing to wait long for. Long enough that an ordinary,
/// already-idle daemon still has time to answer despite whatever momentary
/// load pushed the original request past its own timeout.
const STATUS_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Whether `lock_cmd`'s `Request::Lock` attempt ever reached a daemon at all
/// — classified from the `anyhow::Error` `request_with_timeout` returns, so
/// [`lock_cmd`] never has to duplicate `ipc.rs`'s own socket handling to make
/// this call. `ipc::request_at` wraps exactly one step —
/// `UnixStream::connect` — with its own identifying context ("daemon not
/// reachable at ..."), and that step is also the only one whose underlying
/// `io::Error` kind is ever `NotFound` (no socket file), `ConnectionRefused`
/// (a stale socket, nothing listening), or `PermissionDenied` (a
/// permission/safety problem with the socket directory itself — the "unsafe
/// socket dir" case, should `ipc.rs` ever add that check ahead of connecting).
/// Every later step (setting a timeout, writing the request, reading the
/// reply, parsing it) can only run AFTER that connect already succeeded, so
/// any other error — a read timeout very much included, the exact shape
/// [`LOCK_REQUEST_TIMEOUT`] exists to bound — means a daemon IS running and
/// may have gone on to lock the session anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reachability {
    /// No connection was made — nothing could possibly have locked the
    /// session yet, so falling back immediately is exactly as safe as
    /// today's blanket "any error means fall back".
    Unreachable,
    /// A connection was made; whatever failed happened strictly after that.
    Connected,
}

fn classify_connect_error(err: &anyhow::Error) -> Reachability {
    let never_connected = err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_err| {
                matches!(
                    io_err.kind(),
                    std::io::ErrorKind::NotFound
                        | std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::PermissionDenied
                )
            })
    });
    if never_connected {
        Reachability::Unreachable
    } else {
        Reachability::Connected
    }
}

/// Outcome of [`lock_cmd`]'s follow-up `Request::Status` probe — the one
/// piece of real IO [`decide_lock_action`] itself never performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusProbeOutcome {
    /// The daemon answered and reports the session is locked right now.
    Locked,
    /// Nothing confirms a lock: the daemon answered but says it is not
    /// locked, sent something other than a status reply, or this probe
    /// itself failed the same way the original request did (dead daemon,
    /// another timeout, …). Conservatively treated the same as "not locked"
    /// — see [`decide_lock_action`].
    NotConfirmed,
}

/// Pure given `result`: turns a `Request::Status` round trip's raw outcome
/// into a [`StatusProbeOutcome`]. Split from [`probe_lock_status`] (which
/// does the actual IO) so this half — the only part with real branching — is
/// directly unit-testable against hand-built [`Response`] values.
fn classify_status_probe(result: anyhow::Result<Response>) -> StatusProbeOutcome {
    match result {
        Ok(Response::Status(StatusReply {
            lockscreen: Some(status),
            ..
        })) if status.locked => StatusProbeOutcome::Locked,
        _ => StatusProbeOutcome::NotConfirmed,
    }
}

/// Ask the daemon, once, with a short timeout, whether the session is locked
/// right now — [`lock_cmd`]'s tiebreaker when its own `Request::Lock` never
/// got a trustworthy reply. Never itself decides what `lock_cmd` should do
/// with the answer; see [`decide_lock_action`].
fn probe_lock_status() -> StatusProbeOutcome {
    classify_status_probe(request_with_timeout(&Request::Status, STATUS_PROBE_TIMEOUT))
}

/// Every way [`lock_cmd`]'s reconciliation after a failed `Request::Lock` can
/// come out — one flat enum, rather than nested `Result`/`Option` shapes, so
/// [`decide_lock_action`] is a pure, exhaustively-matched function with one
/// unit test per variant and not a single fake socket anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockRequestOutcome {
    /// No connection could be made at all.
    Unreachable,
    /// A connection was made, and the follow-up status probe confirms the
    /// session is locked — almost certainly by this very request (e.g. the
    /// wlroots host's own up-to-10s wait outlasting
    /// [`LOCK_REQUEST_TIMEOUT`]). Starting a second locker now would race a
    /// working one on the same session.
    ConnectedAndLocked,
    /// A connection was made but nothing confirms the session is locked —
    /// treated exactly like [`Unreachable`].
    ConnectedNotLocked,
}

/// What [`lock_cmd`] should do once it has classified how its `Request::Lock`
/// attempt failed. Pure: takes the already-classified outcome, never touches
/// a socket, so every branch is a one-line unit test.
fn decide_lock_action(outcome: LockRequestOutcome) -> LockAction {
    match outcome {
        LockRequestOutcome::Unreachable | LockRequestOutcome::ConnectedNotLocked => {
            LockAction::Fallback
        }
        LockRequestOutcome::ConnectedAndLocked => LockAction::ReportAlreadyLocked,
    }
}

/// What [`decide_lock_action`] decided [`lock_cmd`] should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockAction {
    /// A lock is already confirmed by other means — report it and exit 0
    /// without starting anything new.
    ReportAlreadyLocked,
    /// Nothing confirms a lock exists yet — run the same fallback chain as
    /// when the daemon was never reached at all.
    Fallback,
}

/// Which family of session `fallback_chain` should target once the daemon
/// itself couldn't lock. A smaller, `cli`-local echo of
/// `daemon::lock::hosts::{HostKind, classify}` — see this module's top doc
/// comment for why it can't just call that function directly. Unlike the
/// daemon's own classifier, this one has no live Wayland connection to probe
/// `zwlr_layer_shell_v1`/`ext_session_lock_manager_v1` from (that needs a
/// registry roundtrip `daemon::capability` owns), so it guesses
/// [`SessionKind::Wlroots`] from a bare `wayland` session type with no known
/// desktop — the more common case among Fresco's supported wlroots
/// compositors, and a wrong guess costs nothing: [`fallback_chain`] filters
/// every candidate by whether its binary actually exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionKind {
    /// A known desktop environment (GNOME, KDE, COSMIC, Cinnamon, MATE,
    /// Xfce, Deepin) — each already wires its own screen locker to
    /// `loginctl lock-session`, so nothing here should race a second locker
    /// against it.
    Desktop,
    /// A layer-shell Wayland compositor with no recognised desktop.
    Wlroots,
    /// An X11 window manager with no recognised desktop.
    X11,
    /// Neither — the safest assumption is also the only universal one.
    Unknown,
}

/// Pure: `XDG_SESSION_TYPE`/`XDG_CURRENT_DESKTOP` in, a [`SessionKind`] out.
///
/// Desktop-name matching mirrors `daemon::lock::hosts::classify`'s own rule
/// exactly, including *which* names need a substring match rather than a
/// whole-segment one: real desktops spell `XDG_CURRENT_DESKTOP` as
/// `"X-Cinnamon"` (never a bare `"cinnamon"` segment) and COSMIC/Deepin as
/// `"custom:COSMIC"`/`"Deepin:GNOME"`-style combinations, so cosmic, deepin,
/// cinnamon and gnome are matched by substring. KDE, MATE, Xfce and DDE stay
/// whole-segment (`daemon::lock::hosts::classify`'s own test guards MATE
/// against exactly this: `"ultimate"`/`"mate-ish"` contain "mate" as a
/// substring but are not the MATE desktop).
fn classify_session(session_type: Option<&str>, current_desktop: Option<&str>) -> SessionKind {
    let segments: Vec<String> = current_desktop
        .unwrap_or("")
        .split(':')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let has = |seg: &str| segments.iter().any(|s| s == seg);
    let contains = |needle: &str| segments.iter().any(|s| s.contains(needle));

    let is_known_desktop = contains("cosmic")
        || has("kde")
        || has("mate")
        || has("xfce")
        || contains("deepin")
        || has("dde")
        || contains("cinnamon")
        || contains("gnome");
    if is_known_desktop {
        return SessionKind::Desktop;
    }
    match session_type {
        Some("wayland") => SessionKind::Wlroots,
        Some("x11") => SessionKind::X11,
        _ => SessionKind::Unknown,
    }
}

fn detect_session() -> SessionKind {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
    classify_session(session_type.as_deref(), current_desktop.as_deref())
}

/// A locker [`fallback_chain`] can pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Locker {
    /// `swaylock -f`: `-f`/`--daemonize` forks only once the compositor
    /// confirms the lock (same upstream behaviour verified for its fork,
    /// swaylock-plugin, in `daemon::lock::hosts::wlroots`'s doc comment).
    Swaylock,
    /// `hyprlock`, with no flags: it does not daemonize, and blocks in the
    /// foreground until the session unlocks. Must be spawned fully detached
    /// and never waited on — see [`spawn_locker`].
    Hyprlock,
    /// `gtklock -d`: `-d`/`--daemonize`, same shape as `swaylock -f`.
    Gtklock,
    /// `xsecurelock`, detached — like hyprlock, it runs in the foreground
    /// for as long as the screen stays locked.
    Xsecurelock,
    /// `i3lock`, with no flags — forks to the background once it has grabbed
    /// the screen, matching `i3lock`'s own documented default behaviour.
    I3lock,
    /// `loginctl lock-session` — the universal, always-available last
    /// resort every systemd host has; see `daemon::lock::hosts`'s own
    /// `loginctl_lock_session` for the same assumption on the daemon side.
    Loginctl,
}

impl Locker {
    fn binary(self) -> &'static str {
        match self {
            Locker::Swaylock => "swaylock",
            Locker::Hyprlock => "hyprlock",
            Locker::Gtklock => "gtklock",
            Locker::Xsecurelock => "xsecurelock",
            Locker::I3lock => "i3lock",
            Locker::Loginctl => "loginctl",
        }
    }
}

/// Pure: the ordered fallback chain for `kind`, filtered to whatever
/// `available` (normally [`which`]) reports as installed.
///
/// Every branch ends in [`Locker::Loginctl`], unconditionally and
/// unfiltered, so the chain this returns is **never empty** —
/// `docs/plan-lock-screen.md` §5's fail-closed invariant means `fresco lock`
/// must always end locked, and `loginctl lock-session` needs no binary probe
/// of its own (it ships with systemd, which every host this targets already
/// has). [`SessionKind::Desktop`] and [`SessionKind::Unknown`] try nothing
/// before it: racing a second locker against a desktop's own already-wired
/// one only risks two lock surfaces fighting over the same session, and
/// guessing a wlroots/X11-only locker on an unrecognised session is no safer
/// than going straight to the one locker guaranteed to be correct everywhere.
fn fallback_chain(kind: SessionKind, available: &dyn Fn(&str) -> bool) -> Vec<Locker> {
    let candidates: &[Locker] = match kind {
        SessionKind::Desktop | SessionKind::Unknown => &[],
        SessionKind::Wlroots => &[Locker::Swaylock, Locker::Hyprlock, Locker::Gtklock],
        SessionKind::X11 => &[Locker::Xsecurelock, Locker::I3lock],
    };
    let mut chain: Vec<Locker> = candidates
        .iter()
        .copied()
        .filter(|l| available(l.binary()))
        .collect();
    chain.push(Locker::Loginctl);
    chain
}

/// How long [`spawn_locker`] waits after starting a candidate before
/// deciding it is actually running: long enough to catch an immediate
/// failure (binary present per `which` but exits right away — no display,
/// wrong session type, missing config), short enough that a locker which
/// blocks until unlock (hyprlock; swaylock/gtklock/i3lock once they
/// daemonize) never makes `fresco lock` itself wait anywhere close to as
/// long as a person actually takes to unlock.
const FALLBACK_PROBE: Duration = Duration::from_millis(400);

/// Try to start `locker`, detached from this process (its own process
/// group, so `fresco lock` exiting — or a shell killing its job — cannot
/// take the freshly-started locker down with it), and report whether it
/// looks like it is actually running.
///
/// Deliberately does **not** wait for exit: unlike
/// `daemon::lock::hosts::wlroots`'s `wait_for_lock_confirmation` (which can
/// afford to block briefly because it runs inside the daemon, off the CLI's
/// critical path), several of these candidates never exit until the person
/// unlocks — see [`Locker::Hyprlock`]'s doc comment — so waiting for exit
/// would turn `fresco lock` into a command that hangs for as long as the
/// screen stays locked. [`FALLBACK_PROBE`]'s short, bounded sleep is the
/// compromise: enough to catch a same-instant crash, never enough to matter
/// to the person who just ran this.
fn spawn_locker(locker: Locker) -> bool {
    let mut cmd = Command::new(locker.binary());
    match locker {
        Locker::Swaylock => {
            cmd.arg("-f");
        }
        Locker::Gtklock => {
            cmd.arg("-d");
        }
        Locker::Loginctl => {
            cmd.arg("lock-session");
        }
        Locker::Hyprlock | Locker::Xsecurelock | Locker::I3lock => {}
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let Ok(mut child) = cmd.spawn() else {
        return false;
    };
    probe_still_running(&mut child, FALLBACK_PROBE)
}

/// Sleep for `probe`, then report whether `child` is still running (or
/// already exited successfully) rather than having failed fast. Split out
/// from [`spawn_locker`] so the sleep/`try_wait` logic itself is testable
/// against a real short-lived child process without spawning an actual
/// locker.
fn probe_still_running(child: &mut Child, probe: Duration) -> bool {
    std::thread::sleep(probe);
    match child.try_wait() {
        Ok(Some(status)) => status.success(),
        Ok(None) => true,
        // Could not even check — assume it's fine rather than risk starting
        // a second locker on top of one that is actually running.
        Err(_) => true,
    }
}

fn fallback_lock() -> i32 {
    let kind = detect_session();
    let chain = fallback_chain(kind, &which);
    for locker in &chain {
        if spawn_locker(*locker) {
            eprintln!("Locked via {}", locker.binary());
            return 0;
        }
    }
    eprintln!(
        "error: could not lock the screen — none of {} could be started",
        chain
            .iter()
            .map(|l| l.binary())
            .collect::<Vec<_>>()
            .join(", ")
    );
    1
}

#[cfg(test)]
mod lock_tests {
    use super::*;

    // -- classify_session -------------------------------------------------------

    #[test]
    fn classify_session_known_desktops_win_regardless_of_session_type() {
        for d in [
            "GNOME",
            "ubuntu:GNOME",
            "KDE",
            "COSMIC",
            "MATE",
            "XFCE",
            "X-Cinnamon",
            "Deepin",
            "DDE",
        ] {
            assert_eq!(
                classify_session(Some("wayland"), Some(d)),
                SessionKind::Desktop,
                "{d}"
            );
            assert_eq!(
                classify_session(Some("x11"), Some(d)),
                SessionKind::Desktop,
                "{d} (x11)"
            );
        }
    }

    #[test]
    fn classify_session_mate_needs_a_whole_segment_not_a_substring() {
        // Mirrors `daemon::lock::hosts::classify`'s own guard: these contain
        // "mate" as a substring but are not the MATE desktop.
        for d in ["ultimate", "mate-ish"] {
            assert_ne!(
                classify_session(Some("x11"), Some(d)),
                SessionKind::Desktop,
                "{d}"
            );
        }
    }

    #[test]
    fn classify_session_bare_wayland_or_x11_with_no_known_desktop() {
        assert_eq!(
            classify_session(Some("wayland"), None),
            SessionKind::Wlroots
        );
        assert_eq!(
            classify_session(Some("wayland"), Some("sway")),
            SessionKind::Wlroots
        );
        assert_eq!(classify_session(Some("x11"), None), SessionKind::X11);
        assert_eq!(classify_session(Some("x11"), Some("i3")), SessionKind::X11);
    }

    #[test]
    fn classify_session_unknown_when_neither_matches() {
        assert_eq!(classify_session(None, None), SessionKind::Unknown);
        assert_eq!(
            classify_session(Some("something-else"), None),
            SessionKind::Unknown
        );
    }

    // -- fallback_chain -----------------------------------------------------------

    #[test]
    fn fallback_chain_always_ends_in_loginctl() {
        for kind in [
            SessionKind::Desktop,
            SessionKind::Wlroots,
            SessionKind::X11,
            SessionKind::Unknown,
        ] {
            let chain = fallback_chain(kind, &|_| false);
            assert_eq!(chain.last().copied(), Some(Locker::Loginctl), "{kind:?}");
            assert!(!chain.is_empty());
        }
    }

    #[test]
    fn fallback_chain_desktop_and_unknown_try_nothing_else() {
        assert_eq!(
            fallback_chain(SessionKind::Desktop, &|_| true),
            vec![Locker::Loginctl]
        );
        assert_eq!(
            fallback_chain(SessionKind::Unknown, &|_| true),
            vec![Locker::Loginctl]
        );
    }

    #[test]
    fn fallback_chain_wlroots_order_and_filtering() {
        assert_eq!(
            fallback_chain(SessionKind::Wlroots, &|_| true),
            vec![
                Locker::Swaylock,
                Locker::Hyprlock,
                Locker::Gtklock,
                Locker::Loginctl
            ]
        );
        // Only hyprlock installed.
        assert_eq!(
            fallback_chain(SessionKind::Wlroots, &|b| b == "hyprlock"),
            vec![Locker::Hyprlock, Locker::Loginctl]
        );
        assert_eq!(
            fallback_chain(SessionKind::Wlroots, &|_| false),
            vec![Locker::Loginctl]
        );
    }

    #[test]
    fn fallback_chain_x11_order_and_filtering() {
        assert_eq!(
            fallback_chain(SessionKind::X11, &|_| true),
            vec![Locker::Xsecurelock, Locker::I3lock, Locker::Loginctl]
        );
        assert_eq!(
            fallback_chain(SessionKind::X11, &|b| b == "i3lock"),
            vec![Locker::I3lock, Locker::Loginctl]
        );
    }

    // -- probe_still_running --------------------------------------------------------

    fn spawn_sh(script: &str) -> Child {
        // Absolute shell + pinned PATH: other tests temporarily repoint the
        // process-wide PATH, which would make `sleep` vanish under this child.
        Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sh must be available to run this test")
    }

    #[test]
    fn probe_still_running_true_for_a_long_running_child() {
        let mut child = spawn_sh("sleep 5");
        assert!(probe_still_running(&mut child, Duration::from_millis(50)));
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn probe_still_running_true_for_a_successful_quick_exit() {
        let mut child = spawn_sh("exit 0");
        assert!(probe_still_running(&mut child, Duration::from_millis(100)));
    }

    #[test]
    fn probe_still_running_false_for_a_failing_quick_exit() {
        let mut child = spawn_sh("exit 1");
        assert!(!probe_still_running(&mut child, Duration::from_millis(100)));
    }

    // -- Locker::binary -------------------------------------------------------------

    #[test]
    fn every_locker_has_a_distinct_binary_name() {
        let names: std::collections::HashSet<&str> = [
            Locker::Swaylock,
            Locker::Hyprlock,
            Locker::Gtklock,
            Locker::Xsecurelock,
            Locker::I3lock,
            Locker::Loginctl,
        ]
        .iter()
        .map(|l| l.binary())
        .collect();
        assert_eq!(names.len(), 6);
    }

    // -- classify_connect_error -------------------------------------------------

    /// Builds an `anyhow::Error` shaped exactly like the one
    /// `ipc::request_at`'s `UnixStream::connect` failure produces: the same
    /// io error kind, wrapped in the same identifying context string.
    fn connect_phase_error(kind: std::io::ErrorKind) -> anyhow::Error {
        anyhow::Error::new(std::io::Error::new(kind, "synthetic"))
            .context("daemon not reachable at /run/user/1000/fresco/control.sock")
    }

    #[test]
    fn classify_connect_error_unreachable_for_every_real_connect_failure_kind() {
        for kind in [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::PermissionDenied,
        ] {
            assert_eq!(
                classify_connect_error(&connect_phase_error(kind)),
                Reachability::Unreachable,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn classify_connect_error_connected_for_a_post_connect_read_timeout() {
        // Shaped like `ipc::request_at`'s `read_line` failure once the
        // connect already succeeded: same context, a timeout-flavoured kind.
        for kind in [std::io::ErrorKind::WouldBlock, std::io::ErrorKind::TimedOut] {
            let err = anyhow::Error::new(std::io::Error::new(kind, "synthetic"))
                .context("reading daemon reply");
            assert_eq!(
                classify_connect_error(&err),
                Reachability::Connected,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn classify_connect_error_connected_for_a_reply_that_failed_to_parse() {
        // Shaped like a `serde_json` failure: no `io::Error` anywhere in the
        // chain at all, so it can never look like a connect failure.
        let err =
            anyhow::anyhow!("expected value at line 1 column 1").context("parsing daemon reply");
        assert_eq!(classify_connect_error(&err), Reachability::Connected);
    }

    #[test]
    fn classify_connect_error_connected_for_an_unwrapped_post_connect_io_error() {
        // e.g. `write_all` hitting a broken pipe right after a successful
        // connect — an `io::Error`, but with no connect-phase kind and no
        // "daemon not reachable" context around it.
        let err = anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "synthetic",
        ));
        assert_eq!(classify_connect_error(&err), Reachability::Connected);
    }

    // -- classify_status_probe ---------------------------------------------------

    fn lock_status(locked: bool) -> crate::ipc::LockStatus {
        crate::ipc::LockStatus {
            enabled: true,
            host: "wlroots".into(),
            live_video: true,
            widgets: true,
            still_frame: Some(true),
            locked,
            setup: crate::ipc::LockSetupState::NotNeeded,
            notes: Vec::new(),
        }
    }

    #[test]
    fn classify_status_probe_locked_when_lockscreen_reports_locked() {
        let reply = StatusReply {
            lockscreen: Some(lock_status(true)),
            ..StatusReply::default()
        };
        assert_eq!(
            classify_status_probe(Ok(Response::Status(reply))),
            StatusProbeOutcome::Locked
        );
    }

    #[test]
    fn classify_status_probe_not_confirmed_when_lockscreen_reports_unlocked() {
        let reply = StatusReply {
            lockscreen: Some(lock_status(false)),
            ..StatusReply::default()
        };
        assert_eq!(
            classify_status_probe(Ok(Response::Status(reply))),
            StatusProbeOutcome::NotConfirmed
        );
    }

    #[test]
    fn classify_status_probe_not_confirmed_when_lockscreen_is_absent() {
        // A pre-lock-screen daemon's reply has no `lockscreen` field at all
        // (see `ipc`'s own backcompat test) — must never be misread as "the
        // session is locked".
        let reply = StatusReply {
            lockscreen: None,
            ..StatusReply::default()
        };
        assert_eq!(
            classify_status_probe(Ok(Response::Status(reply))),
            StatusProbeOutcome::NotConfirmed
        );
    }

    #[test]
    fn classify_status_probe_not_confirmed_on_an_unexpected_reply_or_an_error() {
        assert_eq!(
            classify_status_probe(Ok(Response::Ok)),
            StatusProbeOutcome::NotConfirmed
        );
        assert_eq!(
            classify_status_probe(Err(anyhow::anyhow!("daemon not reachable"))),
            StatusProbeOutcome::NotConfirmed
        );
    }

    // -- decide_lock_action: pure, one test per branch ---------------------------

    #[test]
    fn decide_lock_action_unreachable_falls_back() {
        assert_eq!(
            decide_lock_action(LockRequestOutcome::Unreachable),
            LockAction::Fallback
        );
    }

    #[test]
    fn decide_lock_action_connected_and_locked_reports_already_locked() {
        assert_eq!(
            decide_lock_action(LockRequestOutcome::ConnectedAndLocked),
            LockAction::ReportAlreadyLocked
        );
    }

    #[test]
    fn decide_lock_action_connected_but_not_locked_falls_back() {
        assert_eq!(
            decide_lock_action(LockRequestOutcome::ConnectedNotLocked),
            LockAction::Fallback
        );
    }
}

#[cfg(test)]
mod still_frame_tests {
    use super::*;

    #[test]
    fn still_frame_hint_only_offers_xorg_when_a_session_exists() {
        let with_xorg = still_frame_hint(true, true);
        assert!(with_xorg.contains("Xorg session"), "{with_xorg}");
        let without = still_frame_hint(true, false);
        assert!(!without.contains("log out"), "{without}");
        assert!(
            without.contains("no Xorg session is installed"),
            "{without}"
        );
        assert!(without.contains("extension is planned"), "{without}");
    }

    #[test]
    fn still_frame_hint_does_not_call_a_non_gnome_compositor_gnome() {
        for x11 in [false, true] {
            let hint = still_frame_hint(false, x11);
            assert!(!hint.contains("GNOME"), "{hint}");
            assert!(hint.contains("layer-shell"), "{hint}");
        }
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;

    #[test]
    fn version_flags_exit_zero_without_launching_the_gui() {
        for flag in ["-V", "-v", "--version", "version"] {
            let args = vec!["fresco".to_string(), flag.to_string()];
            assert_eq!(dispatch(&args), Some(0), "{flag}");
        }
    }
}

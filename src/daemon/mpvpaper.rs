//! Wayland live wallpaper via the external `mpvpaper` process — **one process
//! per output**.
//!
//! On layer-shell compositors (wlroots / KDE / COSMIC) we cannot embed mpv into
//! a background surface the way the X11 backend embeds it into a `DESKTOP`
//! window. Instead we drive `mpvpaper` (layer-shell + EGL + libmpv) and steer it
//! over the embedded mpv JSON IPC socket. Each output gets its own mpvpaper and
//! its own socket `$XDG_RUNTIME_DIR/fresco/mpv-<connector>.sock`.
//!
//! `WaylandPlayer` exposes the **same `&self` control surface as the X11
//! `Player`** (load_path / set_paused / set_zoom_pan / set_gamma / apply_crop /
//! hwdec_current / load_failed), using `RefCell` interior mutability for its IPC
//! stream. That lets `PlayerHandle` (in `mod.rs`) drive one shared engine across
//! both backends with no per-call-site branching.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::config::{Fit, Kind, PowerSaving, Scaling, Wallpaper};

/// One `mpvpaper` process for one output, plus a client for its mpv IPC socket.
pub struct WaylandPlayer {
    socket_path: PathBuf,
    inner: RefCell<Inner>,
    /// The last lines mpvpaper (and the mpv inside it) wrote — to **either**
    /// stdout or stderr — kept by two reader threads so a renderer that dies
    /// mid-run can say why. Merged into one tail because mpvpaper's own
    /// `cflp_error()` wrapper prints to stdout and mpv logs to stdout too
    /// (`terminal=yes` is forced by mpvpaper); splitting them would just as
    /// often split the one line that explains the exit from the lines around
    /// it.
    stderr_tail: OutputTail,
}

struct Inner {
    child: Child,
    ipc: MpvIpc,
}

/// Bounded ring of the renderer's most recent output lines, shared with the
/// threads that drain the pipes. Draining is not optional: an unread pipe
/// would block the renderer once it filled, on stdout as much as stderr.
type OutputTail = Arc<Mutex<VecDeque<String>>>;

/// How many output lines to keep. mpv's own chatter at startup is a handful of
/// lines; the error that matters is always among the last few.
const TAIL_LINES: usize = 40;

/// Drain one pipe (stdout or stderr) into the shared tail for the process's
/// whole lifetime — not just until the first error — so mpv's ongoing info
/// logging on stdout can never fill the pipe and stall the renderer. Generic
/// over `Read` so the same function drives both `ChildStdout` and
/// `ChildStderr` reader threads.
fn drain<R: Read>(connector: String, stream: &'static str, pipe: R, tail: OutputTail) {
    for line in BufReader::new(pipe).lines().map_while(Result::ok) {
        let line = line.trim_end().to_string();
        if line.is_empty() {
            continue;
        }
        log::debug!("[{connector}] mpvpaper {stream}: {line}");
        let mut t = tail.lock().unwrap_or_else(|e| e.into_inner());
        if t.len() >= TAIL_LINES {
            t.pop_front();
        }
        t.push_back(line);
    }
}

fn tail_text(tail: &OutputTail) -> String {
    tail.lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip ANSI SGR escapes (`\x1b[...m` and friends) before matching or
/// hashing. mpvpaper's `cflp_error()` colours every message it prints
/// (`\x1b[1;31m[-] <msg>\x1b[0m`), and a needle match or a content hash taken
/// against the raw bytes would depend on whether the terminal escapes are
/// there at all.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Why mpvpaper exited before its IPC socket appeared, classified from its
/// exit status and stderr into a **content-free** code. Each arm matches a
/// message mpvpaper 1.x prints on that exact path (src/main.c upstream); the
/// dynamic-linker case is the one status alone identifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlyExit {
    /// `wl_display_connect` failed: the session is gone, or `WAYLAND_DISPLAY`
    /// names a socket the compositor no longer serves. Not a renderer fault.
    CompositorUnreachable,
    /// The compositor advertises no `zwlr_layer_shell_v1` (GNOME, weston).
    NoLayerShell,
    /// The compositor advertises no output at all.
    NoOutput,
    /// EGL display / context / surface failure — the GL stack is broken.
    Egl,
    /// `mpv_initialize` failed (an option we passed was rejected, or the
    /// system libmpv is unhappy).
    MpvInit,
    /// `mpv_render_context_create` failed — libmpv could not use the EGL
    /// context (typically a driver or Mesa problem).
    MpvGl,
    /// `loadfile` was refused outright.
    LoadFailed,
    /// Exit 127: the dynamic linker could not load a library (libmpv soname).
    Linker,
    /// Killed by a signal (SIGSEGV in a driver, OOM, …).
    Signal,
    /// A Wayland protocol error — the compositor killed the connection over a
    /// bad request (`<interface>@<id>: error <code>: ...`, or a raw
    /// `Protocol error` from libwayland-client). Usually a compositor/driver
    /// bug rather than anything Fresco's options control.
    WaylandProtocol,
    /// Exited 0 with nothing in its output that matches a known failure —
    /// asked to quit right after starting (e.g. session teardown raced the
    /// spawn) rather than having actually failed.
    CleanExit,
    /// Exited with an error we do not recognise.
    Unknown,
}

impl EarlyExit {
    pub const fn code(self) -> &'static str {
        match self {
            EarlyExit::CompositorUnreachable => "exited_early:compositor_unreachable",
            EarlyExit::NoLayerShell => "exited_early:no_layer_shell",
            EarlyExit::NoOutput => "exited_early:no_output",
            EarlyExit::Egl => "exited_early:egl",
            EarlyExit::MpvInit => "exited_early:mpv_init",
            EarlyExit::MpvGl => "exited_early:mpv_gl",
            EarlyExit::LoadFailed => "exited_early:load_failed",
            EarlyExit::Linker => "exited_early:linker",
            EarlyExit::Signal => "exited_early:signal",
            EarlyExit::WaylandProtocol => "exited_early:wayland_protocol",
            EarlyExit::CleanExit => "exited_early:clean_exit",
            EarlyExit::Unknown => "exited_early:unknown",
        }
    }

    /// A one-line, user-facing explanation for the daemon's status error.
    pub fn hint(self) -> &'static str {
        match self {
            EarlyExit::CompositorUnreachable => "the Wayland compositor is unreachable",
            EarlyExit::NoLayerShell => "this compositor has no layer-shell support",
            EarlyExit::NoOutput => "the compositor reports no display",
            EarlyExit::Egl => "EGL/OpenGL could not be initialised (graphics driver problem)",
            EarlyExit::MpvInit => "mpv refused to initialise (run `fresco doctor`)",
            EarlyExit::MpvGl => "mpv could not use the OpenGL context (graphics driver problem)",
            EarlyExit::LoadFailed => "mpv could not open the media file",
            EarlyExit::Linker => {
                "the renderer cannot load this system's libmpv (run `fresco doctor`)"
            }
            EarlyExit::Signal => "the renderer crashed",
            EarlyExit::WaylandProtocol => {
                "the compositor rejected a Wayland request from the renderer"
            }
            EarlyExit::CleanExit => "the renderer exited right after starting",
            EarlyExit::Unknown => "the renderer exited at startup (run `fresco doctor`)",
        }
    }
}

/// Verbatim needles from mpvpaper's `cflp_error()`/`cflp_info()` output (its
/// upstream `src/main.c`) and from mpv's own log, each paired with the
/// [`EarlyExit`] it means and a short, stable id used as the *token* half of
/// [`ExitDetail::sig`] — content-free, but stable across two runs that hit the
/// same known cause. Ordered most-specific first: several mpv GL failures all
/// contain "OpenGL", so the exact phrase must be tried before any looser one
/// would be (there is none here today, but the ordering keeps that true as
/// rules are added). Loose substrings ("EGL" alone, matching the harmless
/// "libEGL warning: ..." mesa prints) are deliberately not here — see the
/// early_exits_classify_by_status_then_stderr test for the case that bit us.
const RULES: &[(&str, EarlyExit, &str)] = &[
    (
        "Unable to connect to the compositor",
        EarlyExit::CompositorUnreachable,
        "compositor_unreachable",
    ),
    (
        "Missing a required Wayland interface",
        EarlyExit::NoLayerShell,
        "no_layer_shell",
    ),
    (
        "can't seem to find any output",
        EarlyExit::NoOutput,
        "no_output",
    ),
    (
        "OpenGL 2.1 or OpenGL ES 2.0 required",
        EarlyExit::MpvGl,
        "mpv_gl_glver",
    ),
    (
        "Failed to initialize mpv GL context",
        EarlyExit::MpvGl,
        "mpv_gl",
    ),
    ("Failed to init mpv", EarlyExit::MpvInit, "mpv_init"),
    (
        "Failed creating mpv context",
        EarlyExit::MpvInit,
        "mpv_init_ctx",
    ),
    ("Failed to load file", EarlyExit::LoadFailed, "load_failed"),
    ("Failed to get EGL display", EarlyExit::Egl, "egl_display"),
    ("Failed to initialize EGL", EarlyExit::Egl, "egl_init"),
    (
        "Failed to set EGL frame buffer config",
        EarlyExit::Egl,
        "egl_fbconfig",
    ),
    (
        "Failed to create EGL context",
        EarlyExit::Egl,
        "egl_context",
    ),
    (
        "Failed to make context current",
        EarlyExit::Egl,
        "egl_current",
    ),
    ("Failed to load OpenGL", EarlyExit::Egl, "egl_opengl"),
];

/// The rule that fired for an already-ANSI-stripped exit, if any — the shared
/// lookup behind both [`classify_early_exit`] and [`exit_detail`]'s token, so
/// the two can never disagree about what matched.
fn matched_rule(
    status: &std::process::ExitStatus,
    stripped: &str,
) -> Option<(EarlyExit, &'static str)> {
    use std::os::unix::process::ExitStatusExt;
    if status.code() == Some(127) {
        return Some((EarlyExit::Linker, "linker"));
    }
    if status.signal().is_some() {
        return None; // a crash carries no useful text token
    }
    // The loader also fails with 127 under some shells but always prints this.
    if stripped.contains("error while loading shared libraries") {
        return Some((EarlyExit::Linker, "linker"));
    }
    if let Some((_, e, id)) = RULES
        .iter()
        .find(|(needle, _, _)| stripped.contains(needle))
    {
        return Some((*e, id));
    }
    if looks_like_wayland_protocol_error(stripped) {
        return Some((EarlyExit::WaylandProtocol, "wayland_protocol"));
    }
    None
}

/// Classify an early exit. Pure so the fingerprints are unit-testable; the
/// strings are verbatim prefixes of what mpvpaper prints on stdout via
/// `cflp_error()` (`\x1b[1;31m[-] <msg>\x1b[0m`) — see [`strip_ansi`].
pub fn classify_early_exit(status: &std::process::ExitStatus, output: &str) -> EarlyExit {
    use std::os::unix::process::ExitStatusExt;
    if status.code() == Some(127) {
        return EarlyExit::Linker;
    }
    if status.signal().is_some() {
        return EarlyExit::Signal;
    }
    let stripped = strip_ansi(output);
    if let Some((e, _)) = matched_rule(status, &stripped) {
        return e;
    }
    // No text we recognise: a clean exit at status 0 is not a failure of
    // anything, just an mpvpaper that quit right after starting (e.g. it lost
    // a race with session teardown). An exit 0 rules match above still wins —
    // this is only reached once nothing else has.
    if status.code() == Some(0) {
        return EarlyExit::CleanExit;
    }
    EarlyExit::Unknown
}

/// A hand-rolled parser for libwayland's protocol-error line, e.g.
/// `zwlr_layer_shell_v1@12: error 0: invalid layer`: `<ident>@<digits>: error
/// <digits>:`. No regex crate is pulled in for one shape. Also matches the
/// plainer "Protocol error" some builds print instead.
fn looks_like_wayland_protocol_error(text: &str) -> bool {
    if text.contains("Protocol error") {
        return true;
    }
    for (idx, _) in text.match_indices('@') {
        let after_at = &text[idx + 1..];
        let id_end = after_at
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after_at.len());
        if id_end == 0 {
            continue;
        }
        let Some(rest) = after_at[id_end..].strip_prefix(": error ") else {
            continue;
        };
        let code_end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if code_end > 0 && rest[code_end..].starts_with(':') {
            return true;
        }
    }
    false
}

/// A content-free fingerprint of why mpvpaper exited, attached to the spawn
/// error via `anyhow::Context` so [`ExitDetail::of`] can pull it back out at
/// the give-up site. Per `src/telemetry.rs`'s privacy rules: no paths, no file
/// names, no raw message text ever leaves this struct — `status` is just an
/// exit code or signal number, and `sig` is either a static rule id (`tok:`)
/// or an 8-hex FNV-1a hash of a normalized line (`h:`) that keeps words
/// containing a path/URL-shaped character and turns every digit into `#`, so
/// two runs that differ only in a file path or a number hash the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitDetail {
    pub status: String,
    pub sig: String,
}

impl std::fmt::Display for ExitDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit={} sig={}", self.status, self.sig)
    }
}

impl ExitDetail {
    /// Pull a previously attached [`ExitDetail`] back out of an error chain.
    pub fn of(e: &anyhow::Error) -> Option<ExitDetail> {
        e.downcast_ref::<ExitDetail>().cloned()
    }
}

/// Build the fingerprint for one exit. `output` is the merged stdout+stderr
/// tail, already possibly carrying ANSI colour.
fn exit_detail(status: &std::process::ExitStatus, output: &str) -> ExitDetail {
    use std::os::unix::process::ExitStatusExt;
    let status_str = match status.code() {
        Some(c) => format!("c{c}"),
        None => format!("s{}", status.signal().unwrap_or(0)),
    };
    let stripped = strip_ansi(output);
    let sig = match matched_rule(status, &stripped) {
        Some((_, id)) => format!("tok:{id}"),
        None => {
            let last = stripped
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("");
            let normalized = normalize_line(last);
            if normalized.is_empty() {
                "h:none".to_string()
            } else {
                format!("h:{:08x}", fnv1a32(normalized.as_bytes()))
            }
        }
    };
    ExitDetail {
        status: status_str,
        sig,
    }
}

/// Reduce one line to `[a-z# ]*`: lowercase, drop every word containing a
/// path/URL/quote-shaped character (`/ \ . @ = :` or a quote), and turn every
/// remaining digit into `#`. What survives is shape, not content — the whole
/// point is that "mp4 not found at /home/al/wall.mp4" and "mp4 not found at
/// /home/bo/other.mp4" hash identically, and neither ever contains a path.
fn normalize_line(line: &str) -> String {
    let lower = line.to_lowercase();
    let mut words = Vec::new();
    for word in lower.split_whitespace() {
        if word.contains(['/', '\\', '.', '@', '=', ':', '\'', '"']) {
            continue;
        }
        let mapped: String = word
            .chars()
            .map(|c| if c.is_ascii_digit() { '#' } else { c })
            .filter(|c| c.is_ascii_lowercase() || *c == '#')
            .collect();
        if !mapped.is_empty() {
            words.push(mapped);
        }
    }
    words.join(" ")
}

/// FNV-1a, 32-bit. Hand-written rather than `DefaultHasher` because the
/// standard hasher's output is explicitly *not* guaranteed stable across Rust
/// versions — telemetry that compares this hash across installs on different
/// toolchains needs one that is.
fn fnv1a32(bytes: &[u8]) -> u32 {
    const FNV_OFFSET: u32 = 0x811c_9dc5;
    const FNV_PRIME: u32 = 0x0100_0193;
    let mut hash = FNV_OFFSET;
    for &b in bytes {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Why a spawn failed, as a **content-free** code (no paths, no file names) the
/// supervisor can put in telemetry. Attached to the returned `anyhow` error so
/// callers classify by type instead of matching on prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnFail {
    /// No mpvpaper binary could be found anywhere.
    Missing,
    /// A bundled mpvpaper is present but cannot load on this system (built
    /// against a libmpv soname the distro does not ship), and there is no
    /// other copy. Telemetry lumped this in with `Missing` until 1.1.42, which
    /// made a packaging problem look like a user who never installed anything.
    Unloadable,
    /// It started, then exited before its IPC socket appeared. The payload
    /// says why, as far as its stderr and exit status could tell us.
    ExitedEarly(EarlyExit),
    /// It stayed up but never opened its mpv IPC socket.
    IpcTimeout,
}

impl SpawnFail {
    pub const fn code(self) -> &'static str {
        match self {
            SpawnFail::Missing => "mpvpaper_missing",
            SpawnFail::Unloadable => "mpvpaper_unloadable",
            SpawnFail::ExitedEarly(why) => why.code(),
            SpawnFail::IpcTimeout => "ipc_timeout",
        }
    }

    /// Classify an error returned by [`WaylandPlayer::spawn`].
    pub fn of(e: &anyhow::Error) -> Option<SpawnFail> {
        e.downcast_ref::<SpawnFail>().copied()
    }
}

impl std::fmt::Display for SpawnFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for SpawnFail {}

impl WaylandPlayer {
    /// Spawn `mpvpaper <connector> <file>` and connect to its mpv IPC socket.
    /// `file` is the initial media (for a slideshow, the first image — later ones
    /// arrive via `loadfile replace`); the caller resolves it so folder-based
    /// slideshows work too.
    pub fn spawn(
        connector: &str,
        wallpaper: &Wallpaper,
        scaling: Scaling,
        power_saving: PowerSaving,
        file: &Path,
    ) -> Result<WaylandPlayer> {
        let dir = crate::ipc::socket_dir();
        std::fs::create_dir_all(&dir).ok();
        let socket_path = dir.join(format!("mpv-{}.sock", sanitize(connector)));
        std::fs::remove_file(&socket_path).ok();

        let mut opts = build_mpv_opts(wallpaper, scaling, power_saving, &socket_path);
        // Same `-o` encoding limits as hwdec: no spaces or `#` in the value.
        if let Some(log) = crate::config::mpv_log_file(connector) {
            let log = log.to_string_lossy().into_owned();
            if log.contains([' ', '#', '\t', '\n']) {
                log::warn!("mpv log path {log:?} can't be passed to mpvpaper; skipping");
            } else {
                log::info!("[{connector}] mpv log: {log}");
                opts.push_str(&format!(" log-file={log}"));
            }
        }
        let bin = crate::mpvpaper_command();
        log::info!(
            "[{connector}] spawning {} -o \"{opts}\" {connector} {}",
            bin.to_string_lossy(),
            file.display()
        );

        let child = Command::new(&bin)
            .arg("-o")
            .arg(&opts)
            .arg(connector)
            .arg(file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                // "Not found" after the bundled copies were rejected by the
                // load probe is a packaging problem, not an absent install.
                let fail = if crate::mpvpaper_broken().is_some() {
                    SpawnFail::Unloadable
                } else {
                    SpawnFail::Missing
                };
                anyhow::Error::new(fail).context(e)
            })
            .with_context(|| {
                format!(
                    "failed to start mpvpaper at {} for output {connector} — is it bundled next to frescod?",
                    bin.to_string_lossy()
                )
            })?;

        let mut child = child;
        let stderr_tail: OutputTail = Arc::new(Mutex::new(VecDeque::new()));
        // Two reader threads, one bounded tail: mpvpaper's own `cflp_error()`
        // prints to stdout (not stderr), and so does mpv's own logging
        // (`terminal=yes` is forced by mpvpaper) — see the doc comment on
        // `stderr_tail`. Both are drained for the process's whole lifetime,
        // not just while we're waiting for the IPC socket, so mpv's ongoing
        // info logging on stdout can never fill the pipe and stall it.
        let stdout_reader = child.stdout.take().map(|pipe| {
            let (c, t) = (connector.to_string(), Arc::clone(&stderr_tail));
            std::thread::spawn(move || drain(c, "stdout", pipe, t))
        });
        let stderr_reader = child.stderr.take().map(|pipe| {
            let (c, t) = (connector.to_string(), Arc::clone(&stderr_tail));
            std::thread::spawn(move || drain(c, "stderr", pipe, t))
        });
        let mut ipc = MpvIpc::new(socket_path.clone());
        // Wait for the IPC socket, but fast-fail if mpvpaper exits first (e.g. a
        // broken GL/EGL stack after a driver update) instead of blocking ~5s.
        let mut connected = false;
        for _ in 0..50 {
            if let Ok(Some(status)) = child.try_wait() {
                std::fs::remove_file(&socket_path).ok();
                // The process is gone, so both pipes are at EOF and their
                // readers finish on their own; joining just makes the tail
                // complete before we read it.
                if let Some(r) = stdout_reader {
                    let _ = r.join();
                }
                if let Some(r) = stderr_reader {
                    let _ = r.join();
                }
                let tail = tail_text(&stderr_tail);
                let why = classify_early_exit(&status, &tail);
                let detail = exit_detail(&status, &tail);
                log::error!(
                    "[{connector}] mpvpaper exited at startup ({status}, {}); its last output was:\n{tail}",
                    why.code()
                );
                return Err(anyhow::Error::new(SpawnFail::ExitedEarly(why))
                    .context(detail)
                    .context(format!(
                        "mpvpaper for {connector} exited immediately ({status}): {}",
                        why.hint()
                    )));
            }
            if ipc.connect_retry(1).is_ok() {
                connected = true;
                break;
            }
        }
        if !connected {
            let _ = child.kill();
            let _ = child.wait();
            std::fs::remove_file(&socket_path).ok();
            return Err(anyhow::Error::new(SpawnFail::IpcTimeout)
                .context(format!("mpv IPC for {connector} never came up")));
        }
        let player = WaylandPlayer {
            socket_path,
            inner: RefCell::new(Inner { child, ipc }),
            stderr_tail,
        };
        // Crop is a runtime property (matches the X11 Player: post-init).
        player.apply_crop(wallpaper);
        // Playlist: queue the remaining files after the first.
        if wallpaper.kind == Kind::Playlist {
            for p in wallpaper.paths.iter().skip(1) {
                player.command(&[
                    json!("loadfile"),
                    json!(p.to_string_lossy().as_ref()),
                    json!("append"),
                ]);
            }
        }
        Ok(player)
    }

    /// True while the mpvpaper process is still running.
    pub fn is_alive(&self) -> bool {
        matches!(self.inner.borrow_mut().child.try_wait(), Ok(None))
    }

    /// The renderer's most recent stderr lines — what to log when it dies.
    pub fn stderr_tail(&self) -> String {
        tail_text(&self.stderr_tail)
    }

    /// The mpvpaper process id (it renders and decodes in-process), so status
    /// can account its CPU/RSS alongside the daemon's own.
    pub fn pid(&self) -> u32 {
        self.inner.borrow().child.id()
    }

    // ── control surface (mirrors the X11 Player; all &self) ──────────────────

    pub fn load_path(&self, path: &Path) {
        self.command(&[
            json!("loadfile"),
            json!(path.to_string_lossy().as_ref()),
            json!("replace"),
        ]);
    }

    pub fn apply_crop(&self, wallpaper: &Wallpaper) {
        match wallpaper.crop.and_then(|c| c.sanitized()) {
            Some(crop) => {
                let (zoom, pan_x, pan_y) = crop.to_mpv_zoom_pan();
                self.set("video-zoom", json!(zoom));
                self.set("video-pan-x", json!(pan_x));
                self.set("video-pan-y", json!(pan_y));
                self.set("panscan", json!(0.0));
            }
            None => {
                self.set("video-zoom", json!(0));
                self.set("video-pan-x", json!(0));
                self.set("video-pan-y", json!(0));
            }
        }
    }

    pub fn set_gamma(&self, gamma: i32) {
        self.set("gamma", json!(gamma));
    }

    /// Defocus the video by `sigma` logical pixels; `0.0` clears the filter.
    ///
    /// A gaussian blur through `lavfi`, which is the one transition effect that
    /// costs something *while it runs* — gamma and zoom are free VO parameters,
    /// a blur is a real filter pass. It is set for the length of a transition
    /// and cleared at the end, never left on, and `set_blur(0.0)` must always
    /// be reachable on the failure paths or the wallpaper stays soft forever.
    pub fn set_blur(&self, sigma: f64) {
        if sigma <= 0.0 {
            self.set("vf", json!(""));
        } else {
            self.set("vf", json!(format!("lavfi=[gblur=sigma={sigma:.2}]")));
        }
    }

    /// Draw an ASS overlay on the OSD layer; empty `ass` clears it.
    ///
    /// Mirrors [`super::mpv::player::Player::set_overlay`] exactly — same
    /// command, same overlay id, same explicit `res_x`/`res_y`. Keeping the two
    /// backends in lockstep matters here: an overlay that silently no-ops on one
    /// of them is the `raise_demuxer_cache` bug in a far more visible place.
    ///
    /// Note this goes over IPC rather than through a spawn option, which also
    /// sidesteps mpvpaper's `-o` parsing — colours are `#RRGGBB`, and `#` starts
    /// a comment in the mpv config file mpvpaper forwards those through.
    /// Place a raw BGRA bitmap on the OSD layer at `(x, y)`.
    ///
    /// Mirrors [`super::mpv::player::Player::overlay_add`]. ASS carries no
    /// bitmaps, so image-bearing widgets go through mpv's `overlay-add`, which
    /// reads the pixels from `path`; the caller keeps that file alive while the
    /// overlay is shown. `stride` is bytes per row (`w * 4`).
    #[allow(clippy::too_many_arguments)]
    pub fn overlay_add(&self, id: u32, x: i32, y: i32, path: &str, w: u32, h: u32, stride: u32) {
        self.command(&[
            json!("overlay-add"),
            json!(id),
            json!(x),
            json!(y),
            json!(path),
            json!(0),
            json!("bgra"),
            json!(w),
            json!(h),
            json!(stride),
        ]);
    }

    /// Remove a bitmap overlay previously added with [`WaylandPlayer::overlay_add`].
    pub fn overlay_remove(&self, id: u32) {
        self.command(&[json!("overlay-remove"), json!(id)]);
    }

    pub fn set_overlay(&self, id: u32, ass: &str, res_x: u32, res_y: u32) {
        if ass.is_empty() {
            self.command(&[json!("osd-overlay"), json!(id), json!("none"), json!("")]);
        } else {
            self.command(&[
                json!("osd-overlay"),
                json!(id),
                json!("ass-events"),
                json!(ass),
                json!(res_x),
                json!(res_y),
                json!(0),
                json!(false),
            ]);
        }
    }

    pub fn set_zoom_pan(&self, zoom: f64, pan_x: f64, pan_y: f64) {
        self.set("video-zoom", json!(zoom));
        self.set("video-pan-x", json!(pan_x));
        self.set("video-pan-y", json!(pan_y));
    }

    pub fn set_paused(&self, paused: bool) {
        self.set("pause", json!(paused));
    }

    /// Change rotation at runtime — the scheduled swap replaces media in place
    /// (no respawn), so the previous wallpaper's rotation must not leak onto
    /// the next one. Mirrors `Player::set_rotation`: video-rotate + copy-back
    /// hwdec; scalers (incl. the rotation-aware chroma one) are set by
    /// [`WaylandPlayer::apply_scalers`], which the caller invokes right after.
    pub fn set_rotation(&self, rotation: u16) {
        let rotated = !rotation.is_multiple_of(360);
        self.set("video-rotate", json!(rotation % 360));
        self.set("hwdec", json!(crate::config::hwdec(rotated)));
    }

    /// Apply the scaler set (mirrors `Player::apply_scalers`) — the single
    /// runtime owner of every scaler property.
    pub fn apply_scalers(&self, scaling: Scaling, power_saving: PowerSaving, rotation: u16) {
        let rotated = !rotation.is_multiple_of(360);
        for (k, v) in crate::config::video_scalers(scaling, power_saving, rotated).to_options() {
            self.set(k, json!(v));
        }
    }

    /// Seek to an absolute position (seconds). Used to keep clones of the same
    /// video in lockstep across monitors.
    pub fn set_time_pos(&self, secs: f64) {
        self.set("playback-time", json!(secs));
    }

    /// Active hardware decoder, e.g. "vaapi" / "nvdec" / "no", read live.
    ///
    /// This used to be read once at spawn, right after the IPC socket came up
    /// — before mpv had opened the file, when `hwdec-current` is always "no".
    /// Every Wayland status therefore said "software" whatever mpv went on to
    /// pick, and it also went stale after a rotation change (which switches
    /// hwdec) or a new file. One IPC round-trip per status poll is cheap.
    pub fn hwdec_current(&self) -> Option<String> {
        self.inner.borrow_mut().ipc.get("hwdec-current")
    }

    /// Live audio state: (audio track selected, muted, volume), read over the
    /// IPC socket. Mirrors the X11 `Player::audio_status` contract. The track
    /// flag is the ground truth for "will this ever make sound" — mpv reports
    /// `aid=no`/`false` both for our muted-entry optimization and after it
    /// dropped the track because no audio server was reachable at load time.
    pub fn audio_status(&self) -> Option<(bool, bool, u8)> {
        let mut inner = self.inner.borrow_mut();
        let aid = inner.ipc.get("aid")?;
        let mute = inner.ipc.get("mute")?;
        let volume = inner.ipc.get("volume")?;
        let track = aid != "no" && aid != "false";
        let muted = mute == "yes" || mute == "true";
        let vol = volume.trim().parse::<f64>().ok()?.round().clamp(0.0, 100.0) as u8;
        Some((track, muted, vol))
    }

    /// The supervisor tracks real renderer failures via the process exit, so the
    /// per-frame "load failed" notion the X11 path uses is always false here.
    pub fn load_failed(&self) -> bool {
        false
    }

    /// Current playback position in seconds, read live over the IPC socket. Used
    /// by the supervisor to detect a frozen-but-alive mpvpaper (dead GL context /
    /// stopped decode) whose process still passes `is_alive`.
    pub fn time_pos(&self) -> Option<f64> {
        self.inner
            .borrow_mut()
            .ipc
            .get("playback-time")?
            .trim()
            .parse()
            .ok()
    }

    /// The size of the coordinate space [`Self::overlay_add`] places into.
    ///
    /// **Not the output's mode, and the difference is not cosmetic.** `overlay-add`
    /// positions in mpv's OSD space, which is mpvpaper's *buffer*, and a buffer is
    /// the surface's logical size times whatever integer scale the compositor
    /// negotiated. On a 2560x1440 panel at 150% the logical surface is 1707x960,
    /// mpvpaper cannot do fractional scale so it takes buffer scale 2, and the OSD
    /// is 3414x1920 — larger than the mode, not smaller. Placing against the mode
    /// there puts a bottom-centre widget at 28% across and 41% down.
    ///
    /// Asked rather than derived because every term in that chain belongs to
    /// somebody else: the compositor picks the scale, mpvpaper picks how to round
    /// it, and mpv decides what the OSD ends up as. Recomputing it here would be
    /// duplicating three other components' decisions and would drift from all of
    /// them. `None` while the renderer is still starting, which is why callers
    /// keep the mode as a fallback rather than blanking the widget.
    pub fn osd_size(&self) -> Option<(u32, u32)> {
        let mut ipc = self.inner.borrow_mut();
        let w: u32 = ipc.ipc.get("osd-width")?.trim().parse().ok()?;
        let h: u32 = ipc.ipc.get("osd-height")?.trim().parse().ok()?;
        (w > 0 && h > 0).then_some((w, h))
    }

    /// Length of the current file in seconds. A still image reports `0` — the
    /// supervisor uses that to tell a frame held on purpose apart from a wedged
    /// renderer (see `WlOutput::check_stall`).
    pub fn duration(&self) -> Option<f64> {
        self.inner
            .borrow_mut()
            .ipc
            .get("duration")?
            .trim()
            .parse()
            .ok()
    }

    /// Re-select the file's audio track and unmute — recovery for a track mpv
    /// dropped because no audio server was reachable at load time. Mirrors
    /// `Player::try_restore_audio`: returns false when the file has no audio
    /// track at all (nothing to restore; stop retrying).
    pub fn try_restore_audio(&self, volume: u8) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Some(tracks) = inner.ipc.get("track-list") else {
            return true; // transient read failure — worth another attempt
        };
        let Some(id) = crate::daemon::mpv::player::first_audio_track_id(&tracks) else {
            return false;
        };
        inner.ipc.set("aid", json!(id));
        inner.ipc.set("mute", json!(false));
        inner.ipc.set("volume", json!(volume));
        true
    }

    /// Decode-honesty snapshot: (source width, height, bit depth, dropped
    /// frames). Mirrors `Player::video_status`.
    pub fn video_status(&self) -> Option<(u32, u32, u8, u64)> {
        let mut inner = self.inner.borrow_mut();
        let w = inner.ipc.get("video-params/w")?;
        let h = inner.ipc.get("video-params/h")?;
        let pf = inner
            .ipc
            .get("video-params/pixelformat")
            .unwrap_or_default();
        let drops = inner.ipc.get("frame-drop-count").unwrap_or_default();
        Some((
            w.trim().parse().ok()?,
            h.trim().parse().ok()?,
            crate::daemon::mpv::player::pixelformat_bit_depth(&pf),
            drops.trim().parse().unwrap_or(0),
        ))
    }

    fn command(&self, args: &[Value]) {
        let _ = self.inner.borrow_mut().ipc.command(args);
    }

    fn set(&self, name: &str, value: Value) {
        self.inner.borrow_mut().ipc.set(name, value);
    }
}

impl Drop for WaylandPlayer {
    fn drop(&mut self) {
        // Killing the process drops its layer surface; the compositor reaps it.
        let mut inner = self.inner.borrow_mut();
        let _ = inner.child.kill();
        let _ = inner.child.wait();
        std::fs::remove_file(&self.socket_path).ok();
    }
}

/// Make a connector name safe for a socket filename ("DP-1" → "DP-1").
fn sanitize(connector: &str) -> String {
    connector
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Build the space-separated mpv option string passed to `mpvpaper -o`. Mirrors
/// the options the X11 `Player` sets (minus `wid`/`vo`, which mpvpaper owns).
fn build_mpv_opts(
    w: &Wallpaper,
    scaling: Scaling,
    power_saving: PowerSaving,
    sock: &Path,
) -> String {
    build_mpv_opts_with_hwdec(
        w,
        scaling,
        power_saving,
        sock,
        &crate::config::hwdec(!w.rotation.is_multiple_of(360)),
    )
}

/// [`build_mpv_opts`] with the hwdec value injected, so tests can check the
/// encoding of every [`crate::config::select_hwdec`] result without touching
/// sysfs or the environment.
///
/// Commas in the hwdec list (`nvdec,vaapi,auto-safe`) are passed unquoted.
/// mpvpaper splits `-o` on spaces and writes each piece as one line of an mpv
/// config file (the bundled binary's "Failed to create file path for mpv
/// options config" string confirms the config-file route). In mpv's config
/// grammar a line is `key=value` with the value running to end of line, so a
/// comma is plain data there, and `hwdec` itself parses the comma list — just
/// as `--hwdec=nvdec,vaapi` does on the command line. Only spaces (the `-o`
/// separator) and `#` (comment start) are unsafe, and no hwdec value has either;
/// a `FRESCO_HWDEC` override containing them is dropped here rather than
/// silently splitting into bogus options.
fn build_mpv_opts_with_hwdec(
    w: &Wallpaper,
    scaling: Scaling,
    power_saving: PowerSaving,
    sock: &Path,
    hwdec: &str,
) -> String {
    // NOTE: do not pass `background=#000000` — mpvpaper forwards `-o` options
    // through an mpv config file, where `#` begins a comment, so the value is
    // truncated and mpv rejects it. mpv's default letterbox background is black.
    let hwdec = if hwdec.contains([' ', '#', '\t', '\n']) {
        log::warn!(
            "hwdec {hwdec:?} can't be passed through mpvpaper -o; using auto-safe/auto-copy"
        );
        if w.rotation.is_multiple_of(360) {
            "auto-safe"
        } else {
            "auto-copy"
        }
    } else {
        hwdec
    };
    let mut o: Vec<String> = vec![
        format!("input-ipc-server={}", sock.display()),
        // Copy-back decode for rotated video — see the note in mpv/player.rs;
        // config::select_hwdec keeps rotated values on copy-back modes.
        format!("hwdec={hwdec}"),
        "image-display-duration=inf".into(),
    ];
    if w.kind == Kind::Playlist && w.paths.len() > 1 {
        o.push("loop-playlist=inf".into());
    } else {
        o.push("loop-file=inf".into());
    }
    if w.mute {
        o.push("mute=yes".into());
        o.push("aid=no".into());
        // No audio clock → smoother looping (matches the X11 Player).
        o.push("video-sync=display-resample".into());
    } else {
        o.push("mute=no".into());
        o.push(format!("volume={}", w.volume));
    }
    match w.fit {
        Fit::Cover => {
            o.push("keepaspect=yes".into());
            o.push("panscan=1.0".into());
        }
        Fit::Contain => {
            o.push("keepaspect=yes".into());
            o.push("panscan=0.0".into());
        }
        Fit::Stretch => o.push("keepaspect=no".into()),
    }
    // Scalers: quality (spline36/lanczos + linear-light downscaling + dither)
    // at Full, dropping toward cheap bilinear as power saving increases — the
    // Render/3D load that pegs weak GPUs for a video wallpaper is per-frame
    // shading, so this is the honest power lever (see config::video_scalers).
    // cscale stays bilinear on rotated video (green-cast bug, matching player.rs).
    let rotated = !w.rotation.is_multiple_of(360);
    for (k, v) in crate::config::video_scalers(scaling, power_saving, rotated).to_options() {
        o.push(format!("{k}={v}"));
    }
    // Clockwise rotation in degrees; applied before crop (zoom/pan).
    o.push(format!("video-rotate={}", w.rotation % 360));
    o.join(" ")
}

/// Minimal client for mpv's JSON IPC (`--input-ipc-server`). Reconnects on
/// failure; matches replies by `request_id` so async events are ignored.
struct MpvIpc {
    path: PathBuf,
    stream: Option<UnixStream>,
    next_id: i64,
}

impl MpvIpc {
    fn new(path: PathBuf) -> MpvIpc {
        MpvIpc {
            path,
            stream: None,
            next_id: 1,
        }
    }

    /// Connect, retrying every 100ms up to `attempts` times.
    fn connect_retry(&mut self, attempts: u32) -> Result<()> {
        for _ in 0..attempts {
            if let Ok(s) = UnixStream::connect(&self.path) {
                let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
                let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                self.stream = Some(s);
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(anyhow!(
            "mpv IPC socket {} never appeared",
            self.path.display()
        ))
    }

    /// Send `["cmd", arg, ...]` and return the reply matching our request_id.
    fn command(&mut self, args: &[Value]) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({ "command": args, "request_id": id });
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');

        for _ in 0..2 {
            if self.stream.is_none() && self.connect_retry(1).is_err() {
                continue;
            }
            // Owned clone so we don't borrow self across the blocking I/O.
            let sock = match self.stream.as_ref().map(|s| s.try_clone()) {
                Some(Ok(s)) => s,
                _ => {
                    self.stream = None;
                    continue;
                }
            };
            if (&sock).write_all(line.as_bytes()).is_err() {
                self.stream = None;
                continue;
            }
            let mut reader = BufReader::new(&sock);
            let mut found = None;
            // Skip async event lines until our reply (bounded).
            for _ in 0..64 {
                let mut buf = String::new();
                match reader.read_line(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(v) = serde_json::from_str::<Value>(buf.trim()) {
                            if v.get("request_id").and_then(Value::as_i64) == Some(id) {
                                found = Some(v);
                                break;
                            }
                        }
                    }
                    Err(_) => break, // read timeout / closed
                }
            }
            if let Some(v) = found {
                return Ok(v);
            }
            self.stream = None; // no reply → drop and retry once
        }
        Err(anyhow!("mpv IPC: no reply for request {id}"))
    }

    /// Fire-and-forget property set.
    fn set(&mut self, name: &str, value: Value) {
        let _ = self.command(&[json!("set_property"), json!(name), value]);
    }

    /// Read a property as a string (numbers/bools stringified); None on error.
    fn get(&mut self, name: &str) -> Option<String> {
        let v = self.command(&[json!("get_property"), json!(name)]).ok()?;
        match v.get("data")? {
            Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muted_wallpaper_drops_audio_clock() {
        let w = Wallpaper {
            kind: Kind::Video,
            mute: true,
            ..Default::default()
        };
        let opts = build_mpv_opts(
            &w,
            Scaling::Balanced,
            PowerSaving::Full,
            Path::new("/tmp/s.sock"),
        );
        assert!(opts.contains("aid=no"));
        assert!(opts.contains("video-sync=display-resample"));
        // Full → quality scalers, and NEVER a vf filter (the 1.1.32 bug) nor
        // decoder skipping (does nothing on hwdec).
        assert!(opts.contains("scale=spline36"));
        assert!(!opts.contains("vf="));
        assert!(!opts.contains("vd-lavc-skipframe"));
        let unmuted = Wallpaper {
            kind: Kind::Video,
            mute: false,
            ..Default::default()
        };
        let opts = build_mpv_opts(
            &unmuted,
            Scaling::Balanced,
            PowerSaving::Full,
            Path::new("/tmp/s.sock"),
        );
        assert!(!opts.contains("video-sync=display-resample"));
    }

    #[test]
    fn build_mpv_opts_carries_comma_hwdec_as_one_option() {
        let sock = Path::new("/tmp/s.sock");
        for rotation in [0u16, 90] {
            let w = Wallpaper {
                kind: Kind::Video,
                rotation,
                ..Default::default()
            };
            let rotated = rotation != 0;
            let hw = crate::config::select_hwdec(true, rotated, None);
            let opts =
                build_mpv_opts_with_hwdec(&w, Scaling::Balanced, PowerSaving::Full, sock, &hw);
            // Exactly one space-separated token (= one config-file line) holds
            // the whole priority list.
            let tokens: Vec<&str> = opts
                .split(' ')
                .filter(|t| t.starts_with("hwdec="))
                .collect();
            assert_eq!(tokens, [format!("hwdec={hw}").as_str()], "{opts}");
            assert!(!opts.contains('#'));
        }
        // An override with a space would split into bogus options → dropped.
        let w = Wallpaper {
            kind: Kind::Video,
            ..Default::default()
        };
        let opts = build_mpv_opts_with_hwdec(
            &w,
            Scaling::Balanced,
            PowerSaving::Full,
            sock,
            "nvdec vaapi",
        );
        assert!(opts.split(' ').any(|t| t == "hwdec=auto-safe"), "{opts}");
        assert!(!opts.split(' ').any(|t| t == "vaapi"));
    }

    #[test]
    fn build_mpv_opts_power_saving_uses_cheap_scalers_not_a_filter() {
        let w = Wallpaper {
            kind: Kind::Video,
            ..Default::default()
        };
        for level in [PowerSaving::Reduced, PowerSaving::Minimum] {
            let opts = build_mpv_opts(&w, Scaling::High, level, Path::new("/tmp/s.sock"));
            // Cheap bilinear scaling, not the expensive lanczos of High.
            assert!(opts.contains("scale=bilinear"), "{level:?}: {opts}");
            assert!(!opts.contains("scale=lanczos"), "{level:?} overrides High");
            // Never a filter (1.1.32 copy-back bug) nor decoder skipping (no-op).
            assert!(!opts.contains("vf="), "{level:?} must not add a filter");
            assert!(!opts.contains("vd-lavc-skipframe"), "{level:?}");
        }
    }

    /// Every spawn failure must survive its `.context()` wrapper as a code the
    /// supervisor can report — telemetry classifies by type, never by prose.
    #[test]
    fn spawn_failures_carry_a_content_free_code() {
        for f in [
            SpawnFail::Missing,
            SpawnFail::Unloadable,
            SpawnFail::ExitedEarly(EarlyExit::Egl),
            SpawnFail::ExitedEarly(EarlyExit::CompositorUnreachable),
            SpawnFail::IpcTimeout,
        ] {
            let e = anyhow::Error::new(f).context("mpvpaper for DP-1 exited immediately (code 1)");
            assert_eq!(SpawnFail::of(&e), Some(f));
            assert_eq!(SpawnFail::of(&e).map(SpawnFail::code), Some(f.code()));
        }
        // An unrelated error classifies as unknown rather than mis-attributing.
        assert_eq!(SpawnFail::of(&anyhow!("no playable file configured")), None);
    }

    /// mpvpaper's real coloured stdout form — `cflp_error()` in upstream
    /// `src/main.c`. Every classifier test below drives lines shaped exactly
    /// like this (not the old made-up "[ERROR] ..." shape), because that
    /// upstream wrapper prints to **stdout**, not stderr, and prefixes with
    /// this escape rather than a plain "[ERROR]" tag — the mismatch between
    /// the two is what let `classify_early_exit` return `Unknown` for every
    /// real failure until this fix.
    fn cflp_error(msg: &str) -> String {
        format!("\x1b[1;31m[-] {msg}\x1b[0m")
    }

    /// The early-exit fingerprints must map mpvpaper's real messages (verbatim
    /// from upstream src/main.c, in their real ANSI-coloured stdout form) to
    /// the right code, and must never leak the message itself — only the
    /// static code travels.
    #[test]
    fn early_exits_classify_by_status_then_stderr() {
        use std::os::unix::process::ExitStatusExt;
        let exit = |c: i32| std::process::ExitStatus::from_raw(c << 8);
        let cases: &[(i32, String, EarlyExit)] = &[
            (127, String::new(), EarlyExit::Linker),
            (1, "mpvpaper: error while loading shared libraries: libmpv.so.1: cannot open shared object file".into(), EarlyExit::Linker),
            (1, cflp_error("Unable to connect to the compositor.\nIf your compositor is running, check or set the WAYLAND_DISPLAY environment variable."), EarlyExit::CompositorUnreachable),
            (1, cflp_error("Missing a required Wayland interface"), EarlyExit::NoLayerShell),
            (1, cflp_error(":/ sorry about this but we can't seem to find any output."), EarlyExit::NoOutput),
            (1, cflp_error("Failed to get EGL display"), EarlyExit::Egl),
            (1, cflp_error("Failed to initialize EGL"), EarlyExit::Egl),
            (1, cflp_error("Failed to set EGL frame buffer config"), EarlyExit::Egl),
            (1, cflp_error("Failed to create EGL context"), EarlyExit::Egl),
            (1, cflp_error("Failed to make context current"), EarlyExit::Egl),
            (1, cflp_error("Failed to load OpenGL"), EarlyExit::Egl),
            (1, cflp_error("Failed to init mpv, option not found"), EarlyExit::MpvInit),
            (1, cflp_error("Failed creating mpv context"), EarlyExit::MpvInit),
            (1, cflp_error("Failed to initialize mpv GL context, unsupported"), EarlyExit::MpvGl),
            // mpv's own message (not mpvpaper's), also unadorned on stdout.
            (1, "[vo/libmpv] At least OpenGL 2.1 or OpenGL ES 2.0 required.".into(), EarlyExit::MpvGl),
            (1, cflp_error("Failed to load file, error loading file"), EarlyExit::LoadFailed),
            // A harmless mesa warning that merely contains "EGL" must not be
            // mistaken for one of the exact EGL failure messages above.
            (1, "libEGL warning: DRI2: failed to authenticate".into(), EarlyExit::Unknown),
            (1, "something new".into(), EarlyExit::Unknown),
            (0, String::new(), EarlyExit::CleanExit),
            (0, "zwlr_layer_shell_v1@12: error 0: invalid layer".into(), EarlyExit::WaylandProtocol),
            (1, "wl_display@1: error 5: invalid object".into(), EarlyExit::WaylandProtocol),
            (1, "[FATAL] Protocol error".into(), EarlyExit::WaylandProtocol),
        ];
        for (code, output, want) in cases {
            assert_eq!(classify_early_exit(&exit(*code), output), *want, "{output}");
        }
        // A signal death is a crash whatever was printed before it.
        let sig = std::process::ExitStatus::from_raw(11);
        assert_eq!(
            classify_early_exit(&sig, &cflp_error("Failed to init mpv")),
            EarlyExit::Signal
        );
        for e in [
            EarlyExit::Linker,
            EarlyExit::Unknown,
            EarlyExit::MpvInit,
            EarlyExit::WaylandProtocol,
            EarlyExit::CleanExit,
        ] {
            assert!(e.code().starts_with("exited_early:"), "{}", e.code());
            assert!(!e.code().contains('/'), "codes carry no paths");
        }
    }

    /// [`ExitDetail`] must never carry a path, must tell apart genuinely
    /// different outputs, and must fold away exactly the parts (a path, a
    /// number) that differ between two runs of the same underlying failure.
    #[test]
    fn exit_detail_is_content_free_and_stable() {
        use std::os::unix::process::ExitStatusExt;
        let exit = |c: i32| std::process::ExitStatus::from_raw(c << 8);

        // A known rule always wins as a stable token, never a hash.
        let d = exit_detail(
            &exit(1),
            &cflp_error("Failed to init mpv, option not found"),
        );
        assert_eq!(d.status, "c1");
        assert_eq!(d.sig, "tok:mpv_init");
        assert!(!d.sig.contains('/'));

        // Empty output hashes to a fixed sentinel, not a hash of nothing.
        assert_eq!(exit_detail(&exit(0), "").sig, "h:none");

        // Two lines differing only in a path and a number must fold to the
        // same signature: the path-bearing word is dropped whole, and the
        // surviving digits map to '#'.
        let a = exit_detail(&exit(1), "mpv: error opening file /home/al/wall1.mp4");
        let b = exit_detail(&exit(1), "mpv: error opening file /home/bo/wall99.mp4");
        assert_eq!(a.sig, b.sig, "{a:?} vs {b:?}");
        assert!(!a.sig.contains('/'));
        assert_ne!(a.sig, "h:none");

        // A genuinely different message hashes differently.
        let c = exit_detail(&exit(1), "completely unrelated failure text");
        assert_ne!(a.sig, c.sig);

        // Signal death: status carries the signal number, never a path.
        let sig = std::process::ExitStatus::from_raw(11);
        assert_eq!(exit_detail(&sig, "").status, "s11");

        // Round-trips through the anyhow context the same way SpawnFail does.
        let e = anyhow::Error::new(SpawnFail::ExitedEarly(EarlyExit::MpvInit))
            .context(d.clone())
            .context("mpvpaper for DP-1 exited immediately");
        assert_eq!(ExitDetail::of(&e), Some(d));
    }

    fn have(bin: &str) -> bool {
        std::env::var("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
            .unwrap_or(false)
    }

    /// Drives the exact IPC commands `WaylandPlayer` sends against a real mpv
    /// (mpvpaper embeds the same mpv). Proves the steering client end-to-end.
    #[test]
    fn ipc_steers_real_mpv() {
        if !have("mpv") {
            eprintln!("skip ipc_steers_real_mpv: mpv not installed");
            return;
        }
        let sock =
            std::env::temp_dir().join(format!("fresco-ipc-test-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&sock);

        let mut child = Command::new("mpv")
            .args([
                "--idle=yes",
                "--vo=null",
                "--ao=null",
                "--no-config",
                "--no-terminal",
                "--really-quiet",
            ])
            .arg(format!("--input-ipc-server={}", sock.display()))
            .spawn()
            .expect("spawn mpv");

        let mut ipc = MpvIpc::new(sock.clone());
        let connected = ipc.connect_retry(50);

        ipc.set("pause", json!(true));
        let paused = ipc.get("pause");
        ipc.set("volume", json!(50));
        ipc.set("video-zoom", json!(-0.5));
        ipc.set("video-pan-x", json!(0.1));
        let zoom = ipc.get("video-zoom"); // crop control (T4) must read back
        let idle = ipc.get("idle-active");

        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&sock);

        assert!(
            connected.is_ok(),
            "should connect to mpv IPC: {connected:?}"
        );
        assert_eq!(
            paused.as_deref(),
            Some("true"),
            "pause must round-trip via IPC (T3)"
        );
        assert_eq!(
            idle.as_deref(),
            Some("true"),
            "idle-active should read back"
        );
        assert!(
            zoom.as_deref()
                .map(|z| z.starts_with("-0.5"))
                .unwrap_or(false),
            "video-zoom (crop, T4) should read back ~-0.5, got {zoom:?}"
        );
    }

    const FAKE_MPVPAPER: &str = "#!/bin/sh\n\
opts=\"$2\"\n\
file=\"$4\"\n\
sock=\"\"\n\
for tok in $opts; do\n\
  case \"$tok\" in\n\
    input-ipc-server=*) sock=\"${tok#input-ipc-server=}\" ;;\n\
  esac\n\
done\n\
[ -n \"$FRESCO_TEST_PIDFILE\" ] && echo $$ > \"$FRESCO_TEST_PIDFILE\"\n\
exec mpv --idle=yes --vo=null --ao=null --no-config --no-terminal --really-quiet --input-ipc-server=\"$sock\" --loop-file=inf \"$file\"\n";

    /// Supervision primitives behind T6 (death detection) and T8 (graceful
    /// failure): no compositor needed — exercises the process layer directly
    /// with a fake mpvpaper that wraps a headless mpv.
    #[test]
    fn mpvpaper_supervision_primitives() {
        use std::os::unix::fs::PermissionsExt;

        // Serializes against every other test that sets FRESCO_MPVPAPER —
        // std::env::set_var is process-global and cargo test runs threaded.
        let _guard = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let wp = Wallpaper {
            kind: Kind::Video,
            path: Some(std::env::temp_dir().join("fresco-none.mp4")),
            ..Default::default()
        };

        // T8: a missing backend binary fails gracefully (Err, never a panic).
        std::env::set_var("FRESCO_MPVPAPER", "/nonexistent/fresco/mpvpaper");
        assert!(
            WaylandPlayer::spawn(
                "HEADLESS-1",
                &wp,
                Scaling::Balanced,
                PowerSaving::Full,
                &std::env::temp_dir().join("fresco-none.mp4")
            )
            .is_err(),
            "spawn must fail gracefully when the backend binary is missing (T8)"
        );

        // A renderer that dies at startup must be classified from what it
        // printed, and "the compositor is gone" must not cost the supervisor a
        // restart: it parks the output as if the display were away.
        let id = std::process::id();
        let dying = std::env::temp_dir().join(format!("fresco-fake-mpvpaper-dying-{id}.sh"));
        std::fs::write(
            &dying,
            "#!/bin/sh\necho '[-] Unable to connect to the compositor.' >&2\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&dying, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("FRESCO_MPVPAPER", &dying);
        let Err(e) = WaylandPlayer::spawn(
            "HEADLESS-1",
            &wp,
            Scaling::Balanced,
            PowerSaving::Full,
            &std::env::temp_dir().join("fresco-none.mp4"),
        ) else {
            panic!("a renderer that exits at once is a spawn failure");
        };
        assert_eq!(
            SpawnFail::of(&e),
            Some(SpawnFail::ExitedEarly(EarlyExit::CompositorUnreachable)),
            "{e:#}"
        );
        {
            let media = std::env::temp_dir().join(format!("fresco-fake-media-{id}.mp4"));
            std::fs::write(&media, b"not really a video").unwrap();
            const MAX: u32 = 5;
            let mut o = crate::daemon::WlOutput::new(
                "HEADLESS-1".into(),
                Wallpaper {
                    kind: Kind::Video,
                    path: Some(media.clone()),
                    ..Default::default()
                },
                Scaling::Balanced,
                PowerSaving::Full,
            );
            for _ in 0..MAX * 2 {
                // As the daemon loop does: a tick without a probe carries no
                // news, so a parked output stays parked.
                let here = !o.absent;
                o.supervise(false, MAX, here);
            }
            assert!(o.absent, "an unreachable compositor parks the output");
            assert_eq!(o.restarts, 0, "and never spends a restart");
            assert!(!o.static_fallback, "nor reaches the give-up fallback");
            let _ = std::fs::remove_file(&media);
        }
        let _ = std::fs::remove_file(&dying);

        // T6: detect the backend dying. Needs mpv (the engine mpvpaper wraps).
        if !have("mpv") {
            eprintln!("skip T6 death-detection: mpv not installed");
            std::env::remove_var("FRESCO_MPVPAPER");
            return;
        }
        let fake = std::env::temp_dir().join(format!("fresco-fake-mpvpaper-{id}.sh"));
        let pidfile = std::env::temp_dir().join(format!("fresco-fake-pid-{id}"));
        std::fs::write(&fake, FAKE_MPVPAPER).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("FRESCO_MPVPAPER", &fake);
        std::env::set_var("FRESCO_TEST_PIDFILE", &pidfile);

        let player = WaylandPlayer::spawn(
            "HEADLESS-1",
            &wp,
            Scaling::Balanced,
            PowerSaving::Full,
            &std::env::temp_dir().join("fresco-none.mp4"),
        )
        .expect("spawn the fake mpvpaper backend");
        assert!(
            player.is_alive(),
            "backend should be alive right after spawn"
        );

        // Simulate a crash: kill the backend process out from under us.
        let pid = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .to_string();
        let _ = Command::new("kill").arg("-9").arg(&pid).status();
        std::thread::sleep(Duration::from_millis(600));
        assert!(
            !player.is_alive(),
            "supervisor must detect the backend death (basis of T6 restart)"
        );

        std::env::remove_var("FRESCO_MPVPAPER");
        std::env::remove_var("FRESCO_TEST_PIDFILE");
        let _ = std::fs::remove_file(&fake);
        let _ = std::fs::remove_file(&pidfile);
    }
}

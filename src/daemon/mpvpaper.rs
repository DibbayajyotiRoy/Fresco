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
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::collections::VecDeque;
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
    /// The last lines mpvpaper (and the mpv inside it) wrote to stderr, kept
    /// by a reader thread so a renderer that dies mid-run can say why.
    stderr_tail: StderrTail,
}

struct Inner {
    child: Child,
    ipc: MpvIpc,
}

/// Bounded ring of the renderer's most recent stderr lines, shared with the
/// thread that drains the pipe. Draining is not optional: mpv logs to the
/// terminal (`terminal=yes` is forced by mpvpaper), and an undrained pipe
/// would block the renderer once it filled.
type StderrTail = Arc<Mutex<VecDeque<String>>>;

/// How many stderr lines to keep. mpv's own chatter at startup is a handful of
/// lines; the error that matters is always among the last few.
const STDERR_TAIL_LINES: usize = 40;

fn drain_stderr(connector: String, pipe: std::process::ChildStderr, tail: StderrTail) {
    for line in BufReader::new(pipe).lines().map_while(Result::ok) {
        let line = line.trim_end().to_string();
        if line.is_empty() {
            continue;
        }
        log::debug!("[{connector}] mpvpaper: {line}");
        let mut t = tail.lock().unwrap_or_else(|e| e.into_inner());
        if t.len() >= STDERR_TAIL_LINES {
            t.pop_front();
        }
        t.push_back(line);
    }
}

fn tail_text(tail: &StderrTail) -> String {
    tail.lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
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
            EarlyExit::Linker => "the renderer cannot load this system's libmpv (run `fresco doctor`)",
            EarlyExit::Signal => "the renderer crashed",
            EarlyExit::Unknown => "the renderer exited at startup (run `fresco doctor`)",
        }
    }
}

/// Classify an early exit. Pure so the fingerprints are unit-testable; the
/// strings are verbatim prefixes of what mpvpaper prints (its `cflp_error`
/// wrapper adds a coloured `[ERROR]` marker in front, hence `contains`).
pub fn classify_early_exit(status: &std::process::ExitStatus, stderr: &str) -> EarlyExit {
    use std::os::unix::process::ExitStatusExt;
    if status.code() == Some(127) {
        return EarlyExit::Linker;
    }
    if status.signal().is_some() {
        return EarlyExit::Signal;
    }
    // The loader also fails with 127 under some shells but always prints this.
    if stderr.contains("error while loading shared libraries") {
        return EarlyExit::Linker;
    }
    const RULES: &[(&str, EarlyExit)] = &[
        ("Unable to connect to the compositor", EarlyExit::CompositorUnreachable),
        ("Missing a required Wayland interface", EarlyExit::NoLayerShell),
        ("can't seem to find any output", EarlyExit::NoOutput),
        ("Failed to initialize mpv GL context", EarlyExit::MpvGl),
        ("Failed to init mpv", EarlyExit::MpvInit),
        ("Failed creating mpv context", EarlyExit::MpvInit),
        ("Failed to load file", EarlyExit::LoadFailed),
        ("EGL", EarlyExit::Egl),
        ("Failed to load OpenGL", EarlyExit::Egl),
    ];
    RULES
        .iter()
        .find(|(needle, _)| stderr.contains(needle))
        .map_or(EarlyExit::Unknown, |(_, e)| *e)
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

        let opts = build_mpv_opts(wallpaper, scaling, power_saving, &socket_path);
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
            .stdout(Stdio::null())
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
        let stderr_tail: StderrTail = Arc::new(Mutex::new(VecDeque::new()));
        let reader = child.stderr.take().map(|pipe| {
            let (c, t) = (connector.to_string(), Arc::clone(&stderr_tail));
            std::thread::spawn(move || drain_stderr(c, pipe, t))
        });
        let mut ipc = MpvIpc::new(socket_path.clone());
        // Wait for the IPC socket, but fast-fail if mpvpaper exits first (e.g. a
        // broken GL/EGL stack after a driver update) instead of blocking ~5s.
        let mut connected = false;
        for _ in 0..50 {
            if let Ok(Some(status)) = child.try_wait() {
                std::fs::remove_file(&socket_path).ok();
                // The process is gone, so the pipe is at EOF and the reader
                // finishes on its own; joining just makes the tail complete.
                if let Some(r) = reader {
                    let _ = r.join();
                }
                let tail = tail_text(&stderr_tail);
                let why = classify_early_exit(&status, &tail);
                log::error!(
                    "[{connector}] mpvpaper exited at startup ({status}, {}); its last output was:\n{tail}",
                    why.code()
                );
                return Err(anyhow::Error::new(SpawnFail::ExitedEarly(why)).context(format!(
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
        log::warn!("hwdec {hwdec:?} can't be passed through mpvpaper -o; using auto-safe/auto-copy");
        if w.rotation.is_multiple_of(360) { "auto-safe" } else { "auto-copy" }
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
            let tokens: Vec<&str> = opts.split(' ').filter(|t| t.starts_with("hwdec=")).collect();
            assert_eq!(tokens, [format!("hwdec={hw}").as_str()], "{opts}");
            assert!(!opts.contains('#'));
        }
        // An override with a space would split into bogus options → dropped.
        let w = Wallpaper { kind: Kind::Video, ..Default::default() };
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

    /// The early-exit fingerprints must map mpvpaper's real messages (verbatim
    /// from upstream src/main.c) to the right code, and must never leak the
    /// message itself — only the static code travels.
    #[test]
    fn early_exits_classify_by_status_then_stderr() {
        use std::os::unix::process::ExitStatusExt;
        let exit = |c: i32| std::process::ExitStatus::from_raw(c << 8);
        let cases: &[(i32, &str, EarlyExit)] = &[
            (127, "", EarlyExit::Linker),
            (1, "mpvpaper: error while loading shared libraries: libmpv.so.1: cannot open shared object file", EarlyExit::Linker),
            (1, "[ERROR] Unable to connect to the compositor.\nIf your compositor is running, check or set the WAYLAND_DISPLAY environment variable.", EarlyExit::CompositorUnreachable),
            (1, "[ERROR] Missing a required Wayland interface", EarlyExit::NoLayerShell),
            (1, "[ERROR] :/ sorry about this but we can't seem to find any output.", EarlyExit::NoOutput),
            (1, "[ERROR] Failed to initialize EGL EGL_NOT_INITIALIZED", EarlyExit::Egl),
            (1, "[ERROR] Failed to create EGL context EGL_BAD_CONFIG", EarlyExit::Egl),
            (1, "[ERROR] Failed to init mpv, option not found", EarlyExit::MpvInit),
            (1, "[ERROR] Failed to initialize mpv GL context, unsupported", EarlyExit::MpvGl),
            (1, "[ERROR] Failed to load file, error loading file", EarlyExit::LoadFailed),
            (1, "something new", EarlyExit::Unknown),
        ];
        for (code, err, want) in cases {
            assert_eq!(classify_early_exit(&exit(*code), err), *want, "{err}");
        }
        // A signal death is a crash whatever was printed before it.
        let sig = std::process::ExitStatus::from_raw(11);
        assert_eq!(classify_early_exit(&sig, "[ERROR] Failed to init mpv"), EarlyExit::Signal);
        for e in [EarlyExit::Linker, EarlyExit::Unknown, EarlyExit::MpvInit] {
            assert!(e.code().starts_with("exited_early:"), "{}", e.code());
            assert!(!e.code().contains('/'), "codes carry no paths");
        }
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

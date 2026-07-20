//! Fresco wallpaper daemon: owns X11 desktop windows and embedded mpv players,
//! reconciles them against the config, and serves IPC control commands.

mod control;
mod fullscreen;
pub mod monitors;
pub mod mpv;
mod mpvpaper;
mod notifier;
mod overview;
mod wayland_outputs;
mod webbridge;
mod x11_fullscreen;
mod x11win;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Screen;
use x11rb::rust_connection::RustConnection;

use crate::config::{Config, Kind, Scaling, Transition, Wallpaper};
use crate::ipc::{MonitorInfo, Request, Response, StatusReply};

use monitors::Monitor;
use mpv::Player;
use mpvpaper::WaylandPlayer;
use x11win::{Atoms, WallpaperWindow};

const TICK: Duration = Duration::from_millis(100);
const LOWER_INTERVAL: Duration = Duration::from_secs(2);
const MONITOR_INTERVAL: Duration = Duration::from_secs(3);
const BATTERY_INTERVAL: Duration = Duration::from_secs(30);
/// Audio recovery cadence/backoff (see `AudioHeal`).
const AUDIO_RETRY_BASE: Duration = Duration::from_secs(5);
const AUDIO_RETRY_MAX: u8 = 6;
// Cold-boot stall self-heal: how long after login to watch for a frozen video,
// how often to check, and how many recovery rebuilds to attempt.
const HEAL_WINDOW: Duration = Duration::from_secs(60);
const HEAL_INTERVAL: Duration = Duration::from_secs(3);
const MAX_HEALS: u32 = 5;
// Wayland frozen-but-alive: consecutive SUPERVISE ticks (~2s each) with no
// playback progress before treating a still-running mpvpaper as wedged. 3 ≈ 6s,
// high enough that a normally looping clip never trips it.
const STALL_STRIKES: u32 = 3;
// Cross-monitor lockstep: the same video on two outputs plays on independent
// mpv clocks, and per-output pauses (fullscreen on one monitor, workspace
// switches) make them drift further apart forever. Periodically re-seat every
// follower on the leader's clock once the drift exceeds the tolerance.
const SYNC_INTERVAL: Duration = Duration::from_secs(5);
const SYNC_TOLERANCE: f64 = 0.2;

/// During a transition the loop ticks at ~60fps for buttery, eased motion.
const ANIM_TICK: Duration = Duration::from_millis(16);
/// Transition durations in ~16ms steps (≈ FADE 0.37s, CROSSFADE 0.2s, SLIDE 0.45s/side).
const FADE_STEPS: u32 = 22;
const CROSSFADE_STEPS: u32 = 12;
const SLIDE_STEPS: u32 = 28;
/// Ken Burns zoom travel (mpv `video-zoom` log2 units) over one interval.
const KEN_BURNS_ZOOM: f64 = 0.16;
/// Subtle scale "punch" layered onto slide/fade for cinematic depth (~4%).
const SLIDE_PUNCH: f64 = 0.06;

/// Premium ease-in-out (gentle acceleration + deceleration). Linear motion is
/// the #1 tell of amateur animation; everything cinematic eases.
fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Softer ease for the continuous Ken Burns drift.
fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Animation phase of a slideshow's current transition.
#[derive(Clone, Copy)]
enum Phase {
    Hold,
    FadeOut { step: u32, total: u32 },
    FadeIn { step: u32, total: u32 },
    SlideOut { step: u32 },
    SlideIn { step: u32 },
}

struct Slideshow {
    images: Vec<PathBuf>,
    idx: usize,
    interval: Duration,
    last_advance: Instant,
    transition: Transition,
    phase: Phase,
    /// Base zoom/pan from the configured crop; animations compose on top.
    base_zoom: f64,
    base_pan_x: f64,
    base_pan_y: f64,
}

struct Renderer {
    window: WallpaperWindow,
    player: PlayerHandle,
    slideshow: Option<Slideshow>,
    /// Last observed playback position — used to detect a cold-boot VO stall
    /// (a video whose position isn't advancing shortly after login).
    last_time_pos: std::cell::Cell<Option<f64>>,
    audio_heal: AudioHeal,
    /// One-shot: demuxer cache raised after a ≥4K source was detected.
    cache_raised: std::cell::Cell<bool>,
    /// Last pause state actually applied — lets `reconcile_pause` talk to mpv
    /// only on change (mirrors `WlOutput::applied_paused`).
    applied_paused: std::cell::Cell<bool>,
}

/// Backoff state for restoring a dropped audio track. mpv permanently
/// deselects the track when no audio server was reachable at load time — the
/// cold-boot case where frescod starts before PipeWire — so for unmuted
/// wallpapers whose track is gone, both backends periodically re-select it
/// (attempts at ~5/10/20/40/80/160s, then give up until the next apply).
/// A file with no audio track at all disables recovery immediately.
struct AudioHeal {
    attempts: u8,
    next: Instant,
}

impl AudioHeal {
    fn new() -> AudioHeal {
        AudioHeal {
            attempts: 0,
            next: Instant::now() + AUDIO_RETRY_BASE,
        }
    }

    fn due(&self, now: Instant) -> bool {
        self.attempts < AUDIO_RETRY_MAX && now >= self.next
    }

    /// Record one attempt; `file_has_audio == false` disables further tries.
    fn record(&mut self, now: Instant, file_has_audio: bool) {
        if !file_has_audio {
            self.attempts = AUDIO_RETRY_MAX;
            return;
        }
        self.attempts += 1;
        self.next = now + AUDIO_RETRY_BASE * 2u32.pow(u32::from(self.attempts));
    }
}

/// The control surface the slideshow/battery engine drives — identical for both
/// backends, so one engine drives either with no per-call-site branching.
/// X11 = in-process mpv (`Player`); Wayland = mpvpaper over its IPC socket
/// (`WaylandPlayer`). All methods are `&self` (the Wayland side uses interior
/// mutability), matching the X11 `Player` API exactly.
enum PlayerHandle {
    X11(Player),
    Wayland(WaylandPlayer),
}

impl PlayerHandle {
    fn load_path(&self, path: &std::path::Path) {
        match self {
            PlayerHandle::X11(p) => p.load_path(path),
            PlayerHandle::Wayland(p) => p.load_path(path),
        }
    }
    /// Runtime rotation change (scheduled swaps are media-only, no respawn).
    fn set_rotation(&self, rotation: u16, scaling: Scaling) {
        match self {
            PlayerHandle::X11(p) => p.set_rotation(rotation, scaling),
            PlayerHandle::Wayland(p) => p.set_rotation(rotation, scaling),
        }
    }
    fn apply_crop(&self, wallpaper: &Wallpaper) {
        match self {
            PlayerHandle::X11(p) => p.apply_crop(wallpaper),
            PlayerHandle::Wayland(p) => p.apply_crop(wallpaper),
        }
    }
    /// Absolute seek (seconds) — cross-monitor lockstep for cloned videos.
    fn set_time_pos(&self, secs: f64) {
        match self {
            PlayerHandle::X11(p) => p.set_time_pos(secs),
            PlayerHandle::Wayland(p) => p.set_time_pos(secs),
        }
    }
    fn set_zoom_pan(&self, zoom: f64, pan_x: f64, pan_y: f64) {
        match self {
            PlayerHandle::X11(p) => p.set_zoom_pan(zoom, pan_x, pan_y),
            PlayerHandle::Wayland(p) => p.set_zoom_pan(zoom, pan_x, pan_y),
        }
    }
    fn set_gamma(&self, gamma: i32) {
        match self {
            PlayerHandle::X11(p) => p.set_gamma(gamma),
            PlayerHandle::Wayland(p) => p.set_gamma(gamma),
        }
    }
    fn set_paused(&self, paused: bool) {
        match self {
            PlayerHandle::X11(p) => p.set_paused(paused),
            PlayerHandle::Wayland(p) => p.set_paused(paused),
        }
    }
    /// Current playback position in seconds, used by both backends' stall
    /// detectors (X11 cold-boot self-heal; Wayland frozen-but-alive supervision).
    fn time_pos(&self) -> Option<f64> {
        match self {
            PlayerHandle::X11(p) => p.time_pos(),
            PlayerHandle::Wayland(p) => p.time_pos(),
        }
    }
    fn hwdec_current(&self) -> Option<String> {
        match self {
            PlayerHandle::X11(p) => p.hwdec_current(),
            PlayerHandle::Wayland(p) => p.hwdec_current(),
        }
    }
    /// (audio track selected, muted, volume) — see the players' docs.
    fn audio_status(&self) -> Option<(bool, bool, u8)> {
        match self {
            PlayerHandle::X11(p) => p.audio_status(),
            PlayerHandle::Wayland(p) => p.audio_status(),
        }
    }
    /// Re-select a dropped audio track; false = file has no audio track.
    fn try_restore_audio(&self, volume: u8) -> bool {
        match self {
            PlayerHandle::X11(p) => p.try_restore_audio(volume),
            PlayerHandle::Wayland(p) => p.try_restore_audio(volume),
        }
    }
    /// (source w, source h, bit depth, dropped frames) — see the players' docs.
    fn video_status(&self) -> Option<(u32, u32, u8, u64)> {
        match self {
            PlayerHandle::X11(p) => p.video_status(),
            PlayerHandle::Wayland(p) => p.video_status(),
        }
    }
    /// Raise demuxer read-ahead for ≥4K sources. X11 only: its spawn defaults
    /// pin tiny caches for RSS; the Wayland mpvpaper path keeps mpv's own
    /// (much larger) defaults, so there is nothing to raise there.
    fn raise_demuxer_cache(&self) {
        if let PlayerHandle::X11(p) = self {
            p.raise_demuxer_cache()
        }
    }
    /// Renderer child pid (Wayland mpvpaper); the X11 mpv is in-process.
    fn child_pid(&self) -> Option<u32> {
        match self {
            PlayerHandle::X11(_) => None,
            PlayerHandle::Wayland(p) => Some(p.pid()),
        }
    }
    fn load_failed(&self) -> bool {
        match self {
            PlayerHandle::X11(p) => p.load_failed(),
            PlayerHandle::Wayland(p) => p.load_failed(),
        }
    }
    /// X11's in-process mpv lives with the daemon; the Wayland renderer is a
    /// separate process the supervisor must watch.
    fn is_alive(&self) -> bool {
        match self {
            PlayerHandle::X11(_) => true,
            PlayerHandle::Wayland(p) => p.is_alive(),
        }
    }
}

pub struct Daemon {
    conn: RustConnection,
    screen_num: usize,
    atoms: Atoms,
    renderers: Vec<Renderer>,
    config: Config,
    user_paused: bool,
    battery_paused: bool,
    last_lower: Instant,
    last_monitor_check: Instant,
    last_battery_check: Instant,
    last_cache_check: Instant,
    last_sync_check: Instant,
    /// Connectors currently covered by a viewable fullscreen window (EWMH),
    /// with the covering window's title for the log.
    fullscreen_covered: std::collections::HashMap<String, String>,
    last_fullscreen_check: Instant,
    sched: SchedState,
    monitors: Vec<Monitor>,
    started_at: Instant,
    last_heal_check: Instant,
    heals: u32,
}

impl Daemon {
    pub fn new(config: Config) -> Result<Daemon> {
        let (conn, screen_num) =
            x11rb::connect(None).context("connecting to X11 (is DISPLAY set?)")?;
        let atoms = Atoms::new(&conn)?.reply()?;
        Ok(Daemon {
            conn,
            screen_num,
            atoms,
            renderers: Vec::new(),
            config,
            user_paused: false,
            battery_paused: false,
            last_lower: Instant::now(),
            last_monitor_check: Instant::now(),
            last_battery_check: Instant::now() - BATTERY_INTERVAL,
            last_cache_check: Instant::now(),
            last_sync_check: Instant::now(),
            fullscreen_covered: std::collections::HashMap::new(),
            last_fullscreen_check: Instant::now(),
            sched: SchedState::default(),
            monitors: Vec::new(),
            started_at: Instant::now(),
            last_heal_check: Instant::now(),
            heals: 0,
        })
    }

    fn screen(&self) -> Screen {
        self.conn.setup().roots[self.screen_num].clone()
    }

    /// Tear down all renderers and rebuild them from the current config and the
    /// current monitor layout. Reveals the native wallpaper momentarily.
    fn rebuild(&mut self) -> Result<()> {
        self.teardown_renderers();
        let screen = self.screen();
        self.monitors = monitors::list_monitors(&self.conn, screen.root)?;

        for monitor in self.monitors.clone() {
            let wallpaper = self.config.wallpaper_for(&monitor.connector).clone();
            if wallpaper.effective_path().is_none() && wallpaper.kind != Kind::Slideshow {
                continue; // nothing configured for this monitor
            }
            match Self::make_renderer(
                &self.conn,
                &screen,
                &self.atoms,
                &monitor,
                &wallpaper,
                self.config.scaling,
            ) {
                Ok(r) => {
                    self.renderers.push(r);
                }
                Err(e) => log::error!("renderer for {} failed: {e}", monitor.connector),
            }
        }
        // Fresh renderers start unpaused (applied_paused = false); one
        // reconcile applies whatever the folded pause sources currently say.
        self.reconcile_pause();
        Ok(())
    }

    fn make_renderer(
        conn: &RustConnection,
        screen: &Screen,
        atoms: &Atoms,
        monitor: &Monitor,
        wallpaper: &Wallpaper,
        scaling: Scaling,
    ) -> Result<Renderer> {
        let window = WallpaperWindow::create(conn, screen, atoms, monitor)?;
        let player = PlayerHandle::X11(Player::new(window.window, wallpaper, scaling)?);
        let slideshow = build_slideshow(wallpaper, &player);
        Ok(Renderer {
            window,
            player,
            slideshow,
            last_time_pos: std::cell::Cell::new(None),
            audio_heal: AudioHeal::new(),
            cache_raised: std::cell::Cell::new(false),
            applied_paused: std::cell::Cell::new(false),
        })
    }

    /// Main event loop. Returns when a Stop command (or signal) is received.
    pub fn run(&mut self) -> Result<()> {
        let commands = control::start_server()?;
        self.rebuild()?;
        overview::apply(&self.config.wallpaper);
        log::info!("frescod started with {} renderer(s)", self.renderers.len());
        crate::telemetry::heartbeat(
            Some("x11"),
            self.renderers
                .first()
                .and_then(|r| r.player.hwdec_current())
                .as_deref(),
            Some(self.renderers.len() as u32),
        );

        loop {
            while let Ok((req, reply)) = commands.try_recv() {
                let is_stop = matches!(req, Request::Stop);
                let resp = self.handle_request(req);
                let _ = reply.send(resp);
                if is_stop {
                    self.shutdown();
                    return Ok(());
                }
            }

            // Drain X11 events so the queue can't grow unbounded. We must NOT
            // re-lower in response: lowering emits a ConfigureNotify on our own
            // window, which would re-enter and storm the compositor (laptop
            // freeze). The periodic re-lower below handles stacking instead.
            while let Ok(Some(_)) = self.conn.poll_for_event() {}

            let now = Instant::now();
            if now.duration_since(self.last_lower) >= LOWER_INTERVAL {
                self.lower_all();
                self.last_lower = now;
            }
            if now.duration_since(self.last_monitor_check) >= MONITOR_INTERVAL {
                self.check_hotplug();
                self.last_monitor_check = now;
            }
            if now.duration_since(self.last_battery_check) >= BATTERY_INTERVAL {
                self.check_battery();
                self.last_battery_check = now;
            }
            self.check_audio(now);
            if now.duration_since(self.last_fullscreen_check) >= LOWER_INTERVAL {
                self.check_fullscreen();
                self.last_fullscreen_check = now;
            }
            if now.duration_since(self.last_cache_check) >= LOWER_INTERVAL {
                self.check_cache();
                self.check_schedule();
                self.last_cache_check = now;
            }
            if now.duration_since(self.last_sync_check) >= SYNC_INTERVAL {
                self.check_sync();
                self.last_sync_check = now;
            }
            self.check_cold_boot_stall(now);
            let animating = self.advance_slideshows(now);

            std::thread::sleep(if animating { ANIM_TICK } else { TICK });
        }
    }

    fn handle_request(&mut self, req: Request) -> Response {
        match req {
            Request::Apply => {
                self.config = Config::load().unwrap_or_else(|_| self.config.clone());
                self.sched.hold_current(&self.config);
                match self.rebuild() {
                    Ok(_) => {
                        overview::apply(&self.config.wallpaper);
                        Response::Ok
                    }
                    Err(e) => Response::Err {
                        message: e.to_string(),
                    },
                }
            }
            Request::Stop => Response::Ok, // teardown happens in run()
            Request::Pause => {
                self.user_paused = true;
                self.reconcile_pause();
                Response::Ok
            }
            Request::Resume => {
                self.user_paused = false;
                self.reconcile_pause();
                Response::Ok
            }
            Request::Status => Response::Status(self.status()),
            Request::Update => {
                notifier::run_updater_async();
                Response::Ok
            }
        }
    }

    fn status(&self) -> StatusReply {
        let (cpu, rss) = proc_stats(&[]);
        let hwdec = self
            .renderers
            .first()
            .and_then(|r| r.player.hwdec_current());
        let error = self
            .renderers
            .iter()
            .find(|r| r.player.load_failed())
            .map(|r| format!("failed to load media on {}", r.window.connector));
        let audio = self.renderers.first().and_then(|r| r.player.audio_status());
        let video = self.renderers.first().and_then(|r| r.player.video_status());
        StatusReply {
            running: true,
            paused: self.user_paused || self.battery_paused,
            hwdec,
            wallpaper: self.describe_wallpaper(),
            cpu_percent: cpu,
            rss_mb: rss,
            monitors: self.monitors.iter().map(|m| m.connector.clone()).collect(),
            error,
            audio_track: audio.map(|(t, _, _)| t),
            mute: audio.map(|(_, m, _)| m),
            volume: audio.map(|(_, _, v)| v),
            source_w: video.map(|(w, _, _, _)| w),
            source_h: video.map(|(_, h, _, _)| h),
            bit_depth: video.map(|(_, _, d, _)| d),
            dropped_frames: video.map(|(_, _, _, n)| n),
            monitors_info: monitors_info_from(&self.monitors),
        }
    }

    fn describe_wallpaper(&self) -> Option<String> {
        let w = &self.config.wallpaper;
        match w.kind {
            Kind::Video | Kind::Image => w
                .effective_path()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned()),
            Kind::Playlist => Some(format!("Playlist ({} items)", w.paths.len())),
            Kind::Slideshow => w
                .slideshow
                .as_ref()
                .map(|s| format!("Slideshow ({} images)", slideshow_images(s).len())),
        }
    }

    /// Fold the user, battery, and per-monitor fullscreen pause sources into
    /// one decision per renderer, and talk to mpv only on change — the same
    /// single-authority shape as `WlOutput::reconcile_pause`.
    fn reconcile_pause(&self) {
        for r in &self.renderers {
            let desired = self.user_paused
                || self.battery_paused
                || self.fullscreen_covered.contains_key(&r.window.connector);
            if r.applied_paused.get() != desired {
                r.player.set_paused(desired);
                r.applied_paused.set(desired);
            }
        }
    }

    /// Poll EWMH fullscreen state and reconcile per-monitor pause on change.
    fn check_fullscreen(&mut self) {
        let covered = x11_fullscreen::covered_connectors(
            &self.conn,
            self.screen().root,
            &self.atoms,
            &self.monitors,
        );
        if covered != self.fullscreen_covered {
            for (c, title) in &covered {
                if !self.fullscreen_covered.contains_key(c) {
                    log::info!("[{c}] fullscreen window ({title:?}) detected; pausing wallpaper");
                }
            }
            for c in self.fullscreen_covered.keys() {
                if !covered.contains_key(c) {
                    log::info!("[{c}] fullscreen cleared; resuming wallpaper");
                }
            }
            self.fullscreen_covered = covered;
            self.reconcile_pause();
        }
    }

    fn lower_all(&self) {
        for r in &self.renderers {
            let _ = x11win::lower(&self.conn, r.window.window);
        }
        let _ = self.conn.flush();
    }

    /// Tear down all renderers, terminating each mpv instance BEFORE destroying
    /// its X window. mpv's vo=gpu context is bound to the window; destroying the
    /// window first can hang or leak the GPU context (notably on NVIDIA), which
    /// otherwise piles up on every wallpaper change.
    fn teardown_renderers(&mut self) {
        for r in self.renderers.drain(..) {
            let Renderer { window, player, .. } = r;
            drop(player);
            window.destroy(&self.conn);
        }
        let _ = self.conn.flush();
    }

    fn check_hotplug(&mut self) {
        let root = self.screen().root;
        if let Ok(current) = monitors::list_monitors(&self.conn, root) {
            if current != self.monitors {
                log::info!("monitor layout changed → rebuilding");
                let _ = self.rebuild();
            }
        }
    }

    fn check_battery(&mut self) {
        if !self.config.pause_on_battery {
            if self.battery_paused {
                self.battery_paused = false;
                self.reconcile_pause();
            }
            return;
        }
        let discharging = on_battery();
        if discharging != self.battery_paused {
            self.battery_paused = discharging;
            self.reconcile_pause();
            log::info!("battery pause = {discharging}");
        }
    }

    /// Restore dropped audio tracks on unmuted wallpapers (see `AudioHeal`).
    /// Cheap when idle: per renderer it's two field reads until an attempt is due.
    fn check_audio(&mut self, now: Instant) {
        let config = &self.config;
        for r in &mut self.renderers {
            let w = config.wallpaper_for(&r.window.connector);
            if w.mute || !r.audio_heal.due(now) {
                continue;
            }
            if let Some((false, _, _)) = r.player.audio_status() {
                log::info!(
                    "[{}] unmuted wallpaper lost its audio track; restoring (attempt {})",
                    r.window.connector,
                    r.audio_heal.attempts + 1
                );
                let has_audio = r.player.try_restore_audio(w.volume);
                if !has_audio {
                    log::info!(
                        "[{}] file has no audio track; disabling audio recovery",
                        r.window.connector
                    );
                }
                r.audio_heal.record(now, has_audio);
            }
        }
    }

    /// Scheduled wallpaper swap (ROADMAP 3.3): media-only `load_path` on every
    /// renderer showing the DEFAULT wallpaper — never `rebuild()`, so there is
    /// no teardown flash and the restack/NVIDIA machinery stays untouched.
    /// Pause state is a separate authority (`reconcile_pause`) and survives.
    fn check_schedule(&mut self) {
        let Some(want) = self.sched.due(&self.config) else {
            return;
        };
        let Some(path) = want.effective_path().map(|p| p.to_path_buf()) else {
            return;
        };
        log::info!(
            "schedule: switching default wallpaper to {}",
            path.display()
        );
        for r in &self.renderers {
            if !self.config.monitors.contains_key(&r.window.connector) {
                // Rotation and crop are per-wallpaper state on the mpv instance;
                // without resetting them here the previous wallpaper's rotation
                // leaks onto the scheduled one (wrong dimensions on screen).
                r.player.set_rotation(want.rotation, self.config.scaling);
                r.player.apply_crop(&want);
                r.player.load_path(&path);
                r.cache_raised.set(false); // re-check resolution for the new media
            }
        }
        // Keep the in-memory config coherent for status/describe. NEVER saved:
        // the on-disk config remains the user's own intent.
        self.config.wallpaper.path = Some(path.clone());
        self.config.wallpaper.rotation = want.rotation;
        self.config.wallpaper.crop = want.crop;
        self.sched.applied = Some(path);
        overview::apply(&self.config.wallpaper);
    }

    /// Re-seat clones of the same video on one clock (see SYNC_INTERVAL): the
    /// first unpaused renderer in each same-file group is the leader; any other
    /// drifted beyond SYNC_TOLERANCE seeks to the leader's position.
    fn check_sync(&self) {
        let mut groups: std::collections::HashMap<&std::path::Path, Vec<&Renderer>> =
            std::collections::HashMap::new();
        for r in &self.renderers {
            if r.applied_paused.get() || r.slideshow.is_some() {
                continue;
            }
            let w = self.config.wallpaper_for(&r.window.connector);
            if w.kind != Kind::Video {
                continue;
            }
            if let Some(p) = w.effective_path() {
                groups.entry(p).or_default().push(r);
            }
        }
        for group in groups.values() {
            if group.len() < 2 {
                continue;
            }
            let Some(lead) = group[0].player.time_pos() else {
                continue;
            };
            for r in &group[1..] {
                if let Some(pos) = r.player.time_pos() {
                    if (pos - lead).abs() > SYNC_TOLERANCE {
                        log::debug!(
                            "[{}] video {:.2}s out of sync with leader; re-seating",
                            r.window.connector,
                            pos - lead
                        );
                        r.player.set_time_pos(lead);
                    }
                }
            }
        }
    }

    /// One-shot demuxer-cache raise once a ≥4K source is known (its resolution
    /// only becomes readable after the first load). See ROADMAP 1.8.5.
    fn check_cache(&mut self) {
        for r in &self.renderers {
            if r.cache_raised.get() {
                continue;
            }
            if let Some((w, h, _, _)) = r.player.video_status() {
                if h >= 2160 || w >= 3840 {
                    r.player.raise_demuxer_cache();
                    log::info!(
                        "[{}] {}x{} source: raised demuxer cache to 64MiB",
                        r.window.connector,
                        w,
                        h
                    );
                }
                r.cache_raised.set(true); // resolution known — decide once
            }
        }
    }

    /// Recover from the cold-boot VO stall. Right after login the X server / WM
    /// may not have the wallpaper window paint-ready when mpv starts, so a video
    /// can freeze on its first frame and stay static until the user re-selects it.
    /// Here we watch the playback position for the first minute and, if a video
    /// isn't advancing, rebuild it — exactly what a manual reselect does — a few
    /// times at most. Images/slideshows hold a frame on purpose, so they're skipped.
    fn check_cold_boot_stall(&mut self, now: Instant) {
        if self.heals >= MAX_HEALS
            || now.duration_since(self.started_at) > HEAL_WINDOW
            || now.duration_since(self.last_heal_check) < HEAL_INTERVAL
            || self.user_paused
            || self.battery_paused
        {
            return;
        }
        self.last_heal_check = now;

        let mut stalled = false;
        for r in &self.renderers {
            // A paused renderer (e.g. fullscreen auto-pause) holds its frame on
            // purpose — sampling it would misread the freeze as a stall.
            if r.applied_paused.get() {
                r.last_time_pos.set(None);
                continue;
            }
            let kind = self.config.wallpaper_for(&r.window.connector).kind;
            if !matches!(kind, Kind::Video | Kind::Playlist) {
                continue;
            }
            let cur = r.player.time_pos();
            let prev = r.last_time_pos.replace(cur);
            // Two readings the same → position frozen → stalled. (None means mpv
            // hasn't reported a position yet; wait for the next check.)
            if let (Some(p), Some(c)) = (prev, cur) {
                if (c - p).abs() < 1e-3 {
                    stalled = true;
                }
            }
        }

        if stalled {
            self.heals += 1;
            log::warn!(
                "video playback not advancing after start; recovering from cold-boot stall (rebuild {}/{MAX_HEALS})",
                self.heals
            );
            let _ = self.rebuild();
        }
    }

    /// Advance every renderer's slideshow. Returns true while any is mid-
    /// animation, so the caller can tick faster (~30fps).
    fn advance_slideshows(&mut self, now: Instant) -> bool {
        let mut animating = false;
        for r in &mut self.renderers {
            if let Some(s) = r.slideshow.as_mut() {
                animating |= advance_slideshow(&r.player, s, now);
            }
        }
        animating
    }

    fn shutdown(&mut self) {
        overview::restore();
        self.teardown_renderers();
        std::fs::remove_file(crate::ipc::socket_path()).ok();
        log::info!("frescod stopped");
    }
}

/// One slideshow's per-tick step — the shared transition state machine. Both
/// backends call this with their own `PlayerHandle`, so the engine is written
/// once. Returns true while mid-animation.
fn advance_slideshow(player: &PlayerHandle, s: &mut Slideshow, now: Instant) -> bool {
    if s.images.len() <= 1 {
        return false;
    }
    let next = (s.idx + 1) % s.images.len();
    let due = now.duration_since(s.last_advance) >= s.interval;
    let mut animating = false;
    {
        match s.phase {
            Phase::Hold => match s.transition {
                Transition::KenBurns => {
                    // Continuous eased zoom + gentle diagonal drift that
                    // alternates direction each image, so it never feels
                    // mechanical. (smoothstep gives a soft start and finish.)
                    let frac = (now.duration_since(s.last_advance).as_secs_f64()
                        / s.interval.as_secs_f64())
                    .clamp(0.0, 1.0);
                    let e = smoothstep(frac);
                    let dir = if s.idx.is_multiple_of(2) { 1.0 } else { -1.0 };
                    player.set_zoom_pan(
                        s.base_zoom + KEN_BURNS_ZOOM * e,
                        s.base_pan_x + dir * 0.10 * (e - 0.5),
                        s.base_pan_y + dir * 0.05 * (e - 0.5),
                    );
                    animating = true;
                    if due {
                        s.idx = next;
                        player.load_path(&s.images[s.idx]);
                        player.set_zoom_pan(s.base_zoom, s.base_pan_x, s.base_pan_y);
                        s.last_advance = now;
                    }
                }
                Transition::None => {
                    if due {
                        s.idx = next;
                        player.load_path(&s.images[s.idx]);
                        s.last_advance = now;
                    }
                }
                Transition::Fade | Transition::Crossfade => {
                    if due {
                        let total = if matches!(s.transition, Transition::Crossfade) {
                            CROSSFADE_STEPS
                        } else {
                            FADE_STEPS
                        };
                        s.phase = Phase::FadeOut { step: 0, total };
                        animating = true;
                    }
                }
                Transition::Slide => {
                    if due {
                        s.phase = Phase::SlideOut { step: 0 };
                        animating = true;
                    }
                }
            },
            Phase::FadeOut { step, total } => {
                animating = true;
                let e = ease_in_out_cubic(step as f64 / total as f64);
                player.set_gamma((-100.0 * e) as i32);
                // Subtle inward "breath" while dimming — cinematic depth.
                player.set_zoom_pan(s.base_zoom + SLIDE_PUNCH * e, s.base_pan_x, s.base_pan_y);
                if step >= total {
                    s.idx = next;
                    player.load_path(&s.images[s.idx]);
                    s.phase = Phase::FadeIn { step: 0, total };
                } else {
                    s.phase = Phase::FadeOut {
                        step: step + 1,
                        total,
                    };
                }
            }
            Phase::FadeIn { step, total } => {
                animating = true;
                let e = ease_in_out_cubic(step as f64 / total as f64);
                player.set_gamma((-100.0 * (1.0 - e)) as i32);
                // Settle the breath back to base as it brightens.
                player.set_zoom_pan(
                    s.base_zoom + SLIDE_PUNCH * (1.0 - e),
                    s.base_pan_x,
                    s.base_pan_y,
                );
                if step >= total {
                    player.set_gamma(0);
                    player.set_zoom_pan(s.base_zoom, s.base_pan_x, s.base_pan_y);
                    s.phase = Phase::Hold;
                    s.last_advance = now;
                } else {
                    s.phase = Phase::FadeIn {
                        step: step + 1,
                        total,
                    };
                }
            }
            Phase::SlideOut { step } => {
                animating = true;
                // Eased push out with a slight zoom — a "push", not a flat slide.
                let e = ease_in_out_cubic(step as f64 / SLIDE_STEPS as f64);
                player.set_zoom_pan(
                    s.base_zoom + SLIDE_PUNCH * e,
                    s.base_pan_x - e,
                    s.base_pan_y,
                );
                if step >= SLIDE_STEPS {
                    s.idx = next;
                    player.load_path(&s.images[s.idx]);
                    player.set_zoom_pan(
                        s.base_zoom + SLIDE_PUNCH,
                        s.base_pan_x + 1.0,
                        s.base_pan_y,
                    );
                    s.phase = Phase::SlideIn { step: 0 };
                } else {
                    s.phase = Phase::SlideOut { step: step + 1 };
                }
            }
            Phase::SlideIn { step } => {
                animating = true;
                let e = ease_in_out_cubic(step as f64 / SLIDE_STEPS as f64);
                player.set_zoom_pan(
                    s.base_zoom + SLIDE_PUNCH * (1.0 - e),
                    s.base_pan_x + (1.0 - e),
                    s.base_pan_y,
                );
                if step >= SLIDE_STEPS {
                    player.set_zoom_pan(s.base_zoom, s.base_pan_x, s.base_pan_y);
                    s.phase = Phase::Hold;
                    s.last_advance = now;
                } else {
                    s.phase = Phase::SlideIn { step: step + 1 };
                }
            }
        }
    }
    animating
}

/// Build a `Slideshow` state machine for a slideshow wallpaper, loading its
/// first image into `player`. `None` for non-slideshow wallpapers. Shared by
/// both backends so slideshow setup is written once.
fn build_slideshow(wallpaper: &Wallpaper, player: &PlayerHandle) -> Option<Slideshow> {
    if wallpaper.kind != Kind::Slideshow {
        return None;
    }
    let s = wallpaper.slideshow.as_ref()?;
    let images = slideshow_images(s);
    if let Some(first) = images.first() {
        player.load_path(first);
    }
    let (base_zoom, base_pan_x, base_pan_y) = wallpaper
        .crop
        .and_then(|c| c.sanitized())
        .map(|c| c.to_mpv_zoom_pan())
        .unwrap_or((0.0, 0.0, 0.0));
    Some(Slideshow {
        images,
        idx: 0,
        interval: Duration::from_secs(s.interval_s.max(2)),
        last_advance: Instant::now(),
        transition: s.transition,
        phase: Phase::Hold,
        base_zoom,
        base_pan_x,
        base_pan_y,
    })
}

/// Resolve a slideshow's image list: explicit hand-picked `paths`, else a scan
/// of its `folder`.
fn slideshow_images(s: &crate::config::Slideshow) -> Vec<PathBuf> {
    if !s.paths.is_empty() {
        s.paths.clone()
    } else if let Some(folder) = &s.folder {
        list_images(folder)
    } else {
        Vec::new()
    }
}

/// List image files in a folder, sorted by name.
fn list_images(folder: &std::path::Path) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase)
                    .as_deref(),
                Some("jpg" | "jpeg" | "png" | "webp" | "bmp" | "tiff" | "gif")
            )
        })
        .collect();
    v.sort();
    v
}

/// Any power-supply reporting "Discharging" means we're on battery.
fn on_battery() -> bool {
    let Ok(dir) = std::fs::read_dir("/sys/class/power_supply") else {
        return false;
    };
    dir.flatten().any(|entry| {
        std::fs::read_to_string(entry.path().join("status"))
            .map(|s| s.trim() == "Discharging")
            .unwrap_or(false)
    })
}

/// (cpu_percent, rss_megabytes) for the daemon plus any renderer child
/// processes (the Wayland mpvpaper instances — the X11 mpv is in-process).
/// CPU is a real interval sample: total utime+stime ticks are compared with
/// the previous call's snapshot, so the first status poll reports 0 and every
/// later one the true usage since the previous poll (engine-notes item D).
fn proc_stats(child_pids: &[u32]) -> (f32, u64) {
    let mut ticks: u64 =
        parse_stat_ticks(&std::fs::read_to_string("/proc/self/stat").unwrap_or_default())
            .unwrap_or(0);
    let mut rss_pages: u64 = statm_rss_pages("/proc/self/statm");
    for pid in child_pids {
        ticks += parse_stat_ticks(
            &std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default(),
        )
        .unwrap_or(0);
        rss_pages += statm_rss_pages(&format!("/proc/{pid}/statm"));
    }

    // One shared sample slot: all status paths poll from the daemon's control
    // thread, so a plain mutex-guarded (time, ticks, last%) triple suffices.
    static LAST: std::sync::Mutex<Option<(Instant, u64, f32)>> = std::sync::Mutex::new(None);
    let now = Instant::now();
    let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
    let cpu = match *last {
        Some((t0, ticks0, prev_pct)) => {
            let dt = now.duration_since(t0).as_secs_f64();
            if dt < 0.5 {
                // Too soon for a stable sample — keep the previous reading.
                return (prev_pct, rss_pages * 4096 / 1_048_576);
            }
            // /proc stat ticks are in USER_HZ, fixed at 100 on Linux.
            (ticks.saturating_sub(ticks0) as f64 / 100.0 / dt * 100.0) as f32
        }
        None => 0.0,
    };
    *last = Some((now, ticks, cpu));
    (cpu, rss_pages * 4096 / 1_048_576)
}

/// The full wallpaper the configured schedule wants on screen right now —
/// rotation/crop included, so a scheduled swap can reset per-wallpaper player
/// state instead of leaking the previous wallpaper's rotation.
pub(crate) fn schedule_desired_wallpaper(config: &Config) -> Option<Wallpaper> {
    use chrono::Offset as _;
    if config.schedule_paused {
        return None; // paused: keep the schedule config, ignore it entirely
    }
    let sched = config.schedule.as_ref()?;
    let now = chrono::Local::now();
    let off = now.offset().fix().local_minus_utc() / 60;
    crate::schedule::desired(sched, now.naive_local(), off).cloned()
}

/// What the configured schedule wants on screen right now (path only).
fn schedule_desired_path(config: &Config) -> Option<PathBuf> {
    schedule_desired_wallpaper(config).and_then(|w| w.effective_path().map(|p| p.to_path_buf()))
}

/// Scheduler bookkeeping shared by both backends' loops.
#[derive(Default)]
struct SchedState {
    /// Path the scheduler last applied (avoid re-sending loadfile every tick).
    applied: Option<PathBuf>,
    /// Manual-Apply hold: the user's explicit choice wins until the schedule's
    /// desired slot CHANGES (next boundary), then scheduling resumes.
    hold: Option<PathBuf>,
}

impl SchedState {
    /// On a manual Apply: if the user's configured wallpaper DIFFERS from what
    /// the schedule wants right now, that's an explicit override — hold the
    /// current slot so we don't stomp it until the next boundary. When they
    /// match (e.g. the GUI just enabled scheduling and synced the wallpaper),
    /// no hold: the schedule is live immediately.
    fn hold_current(&mut self, config: &Config) {
        let desired = schedule_desired_path(config);
        self.hold = match (&desired, config.wallpaper.effective_path()) {
            (Some(d), Some(w)) if d.as_path() == w => None,
            _ => desired,
        };
        self.applied = None;
    }

    /// The wallpaper to switch to now, if any (None = nothing to do this tick).
    fn due(&mut self, config: &Config) -> Option<Wallpaper> {
        let want = schedule_desired_wallpaper(config)?;
        let path = want.effective_path()?.to_path_buf();
        if self.hold.as_deref() == Some(path.as_path()) {
            return None; // user's manual choice holds this slot
        }
        self.hold = None; // boundary passed — hold expires
        if self.applied.as_deref() == Some(path.as_path())
            || config.wallpaper.effective_path() == Some(path.as_path())
        {
            self.applied = Some(path);
            return None;
        }
        Some(want)
    }
}

/// Neutral Monitor list → wire MonitorInfo list (shared by all status paths).
fn monitors_info_from(monitors: &[Monitor]) -> Vec<MonitorInfo> {
    monitors
        .iter()
        .map(|m| MonitorInfo {
            connector: m.connector.clone(),
            width: m.width,
            height: m.height,
            x: m.x,
            y: m.y,
        })
        .collect()
}

/// Sum of utime+stime (fields 14+15) from a `/proc/<pid>/stat` line. The comm
/// field may contain spaces and parentheses, so fields are counted after the
/// LAST `)`.
fn parse_stat_ticks(stat: &str) -> Option<u64> {
    let rest = stat.rsplit_once(')')?.1;
    let mut fields = rest.split_whitespace();
    // After ')': state is overall field 3, so utime (14) and stime (15) are at
    // 0-based positions 11 and 12 here.
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    Some(utime + stime)
}

/// Resident pages from `/proc/<pid>/statm` (0 when unreadable, e.g. child gone).
fn statm_rss_pages(path: &str) -> u64 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
        .and_then(|pages| pages.parse::<u64>().ok())
        .unwrap_or(0)
}

// ─── Entry points called by frescod.rs ───────────────────────────────────────

/// Normal daemon start: honor `enabled`, guard Wayland, run the loop.
/// Hybrid Intel+NVIDIA laptops are a common Linux config where libva probes the
/// NVIDIA render node (no VA-API) and fails, leaving mpv on software decode —
/// which is what makes the wallpaper eat CPU and RAM. If an Intel GPU is present
/// and no driver is pinned, force the Intel media driver so hardware decode
/// works. No-op on single-GPU / AMD / NVIDIA-only systems.
fn setup_vaapi_env() {
    if std::env::var_os("LIBVA_DRIVER_NAME").is_some() {
        return;
    }
    let Ok(dir) = std::fs::read_dir("/sys/class/drm") else {
        return;
    };
    for entry in dir.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("card") {
            continue;
        }
        let vendor =
            std::fs::read_to_string(entry.path().join("device/vendor")).unwrap_or_default();
        if vendor.trim() == "0x8086" {
            // Intel: iHD (Gen8+/Broadwell and newer, incl. Alder Lake).
            std::env::set_var("LIBVA_DRIVER_NAME", "iHD");
            log::info!("VA-API: pinned Intel iHD driver for hardware decode");
            return;
        }
    }
}

pub fn run() -> Result<()> {
    use crate::capability::{detect, Capability};
    // Event-driven admin notifications + update prompts over Supabase Realtime.
    // Background thread; never blocks the wallpaper loop.
    notifier::spawn();
    // Periodic "send feedback" nudge (config-gated; stops after one submission).
    notifier::spawn_feedback_reminder();

    // Self-heal the login-restore entry: if the user wants the wallpaper restored
    // on login (and hasn't stopped it), make sure the autostart entry actually
    // exists. Fixes installs where config says autostart=true but the .desktop
    // entry was never written, so the daemon silently failed to start on boot.
    if let Ok(cfg) = Config::load() {
        if cfg.autostart && cfg.enabled {
            crate::autostart::enable().ok();
        }
        // Browser bridge: bound at startup only (std TcpListener has no clean
        // async shutdown and this stays dependency-free). Turning the switch
        // OFF takes effect immediately anyway — every request re-reads the
        // config and refuses while disabled; turning it ON needs a daemon
        // restart.
        if cfg.browser_bridge {
            webbridge::spawn(webbridge::PORT);
        }
    }
    let capability = detect();
    log::info!("session capability: {}", capability.id());
    match capability {
        Capability::X11 => run_x11(),
        Capability::WaylandGnomeStatic => run_gnome_static(),
        Capability::WaylandLayerShell => {
            if wayland_backend_enabled() {
                run_wayland_layershell()
            } else {
                // FRESCO_WAYLAND=0 explicitly disables the live backend.
                log::info!(
                    "Wayland layer-shell session detected; FRESCO_WAYLAND=0 disables the live backend"
                );
                Ok(())
            }
        }
    }
}

/// X11 daemon: the original in-process mpv backend (behavior unchanged).
fn run_x11() -> Result<()> {
    setup_vaapi_env();
    let config = Config::load().unwrap_or_default();
    if !config.enabled {
        // Safety net: if a prior run was killed (not Stopped) it may have left
        // our static frame as the background — put the user's original back.
        overview::restore();
        log::info!("wallpaper disabled (enabled=false) — exiting");
        return Ok(());
    }
    let mut daemon = Daemon::new(config)?;
    daemon.run()
}

/// GNOME-on-Wayland fallback: GNOME Mutter has no layer-shell, so a live
/// wallpaper window is impossible. Reuse the existing still-frame path (set as
/// the desktop background via gsettings) and serve IPC so the GUI can
/// apply/stop. Blocks on the control channel between commands → ~0% CPU.
fn run_gnome_static() -> Result<()> {
    let mut config = Config::load().unwrap_or_default();
    if !config.enabled {
        overview::restore();
        log::info!("wallpaper disabled (enabled=false) — exiting");
        return Ok(());
    }
    let commands = control::start_server()?;
    overview::apply(&config.wallpaper);
    log::info!("frescod started (GNOME Wayland static-frame mode)");
    crate::telemetry::heartbeat(Some("gnome-static"), None, None);

    while let Ok((req, reply)) = commands.recv() {
        let is_stop = matches!(req, Request::Stop);
        let resp = match req {
            Request::Apply => {
                config = Config::load().unwrap_or_else(|_| config.clone());
                if config.enabled {
                    overview::apply(&config.wallpaper);
                } else {
                    overview::restore();
                }
                Response::Ok
            }
            // A static frame has nothing to pause.
            Request::Pause | Request::Resume => Response::Ok,
            Request::Status => Response::Status(static_status(&config)),
            Request::Update => {
                notifier::run_updater_async();
                Response::Ok
            }
            Request::Stop => Response::Ok,
        };
        let _ = reply.send(resp);
        if is_stop {
            break;
        }
    }

    overview::restore();
    std::fs::remove_file(crate::ipc::socket_path()).ok();
    log::info!("frescod stopped");
    Ok(())
}

/// Minimal status for the GNOME static-frame fallback mode.
fn static_status(config: &Config) -> StatusReply {
    let (cpu, rss) = proc_stats(&[]);
    let wallpaper = config
        .wallpaper
        .effective_path()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .or_else(|| Some("Static frame".to_string()));
    StatusReply {
        running: true,
        paused: false,
        hwdec: None,
        wallpaper,
        cpu_percent: cpu,
        rss_mb: rss,
        monitors: Vec::new(),
        error: None,
        audio_track: None,
        mute: None,
        volume: None,
        source_w: None,
        source_h: None,
        bit_depth: None,
        dropped_frames: None,
        monitors_info: Vec::new(),
    }
}

/// The experimental Wayland (mpvpaper) backend is opt-in while it stabilizes.
fn wayland_backend_enabled() -> bool {
    // Live Wayland wallpapers are enabled by default on layer-shell compositors.
    // Set FRESCO_WAYLAND=0 (or no/false) to force the old behaviour.
    !matches!(
        std::env::var("FRESCO_WAYLAND"),
        Ok(v) if v.eq_ignore_ascii_case("0")
            || v.eq_ignore_ascii_case("no")
            || v.eq_ignore_ascii_case("false")
    )
}

/// Wayland layer-shell backend: supervise one `mpvpaper ALL` process and steer
/// it over its mpv IPC socket. Self-contained — does not touch the X11 path.
/// Uses `ALL` outputs (no per-monitor enumeration / hotplug in this phase).
fn run_wayland_layershell() -> Result<()> {
    use std::collections::{BTreeMap, HashSet};
    use std::sync::mpsc::RecvTimeoutError;
    const MAX_RESTARTS: u32 = 5;
    const SUPERVISE: Duration = Duration::from_secs(2);
    const TICK: Duration = Duration::from_millis(100);
    const ANIM_TICK: Duration = Duration::from_millis(33);
    // How often to re-poll fullscreen state (coarse — pausing is not latency
    // critical, and this bounds the per-tick roundtrip cost).
    const FS_POLL: Duration = Duration::from_millis(250);

    setup_vaapi_env();
    let mut config = Config::load().unwrap_or_default();
    if !config.enabled {
        log::info!("wallpaper disabled (enabled=false) — exiting");
        return Ok(());
    }

    let commands = control::start_server()?;

    // Enumerate outputs at start; the Apply handler re-enumerates so displays
    // plugged later are assignable (registry-driven hotplug lands with the
    // native backend, ROADMAP 5.3).
    let mut monitors = wayland_outputs::list_outputs().unwrap_or_else(|e| {
        log::warn!("output enumeration failed ({e:#}); targeting all outputs as one");
        vec![Monitor {
            connector: "ALL".into(),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }]
    });
    log::info!(
        "Wayland outputs: [{}]",
        monitors
            .iter()
            .map(|m| m.connector.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut user_paused = false;
    let mut battery_paused = false;
    let mut last_supervise = Instant::now() - SUPERVISE;
    let mut sched = SchedState::default();

    // Pause the wallpaper on any output that has a fullscreen window. Available on
    // wlroots/KWin (wlr protocol) and COSMIC (zcosmic-toplevel-info); absent on
    // GNOME (which uses the static path, not this one).
    let mut fs_watch = fullscreen::FullscreenWatch::new();
    log::info!(
        "fullscreen auto-pause: {}",
        match fs_watch.as_ref().map(|w| w.backend()) {
            Some(fullscreen::Backend::Wlr) => "enabled (wlr-foreign-toplevel)",
            Some(fullscreen::Backend::Cosmic) => "using cosmic-toplevel-info",
            None =>
                "unavailable (compositor lacks wlr-foreign-toplevel-management and cosmic-toplevel-info)",
        }
    );
    let mut hidden: HashSet<String> = HashSet::new();
    let mut last_fs_poll = Instant::now() - FS_POLL;

    // One supervised mpvpaper per output, keyed by connector name.
    let mut outputs: BTreeMap<String, WlOutput> = BTreeMap::new();
    for m in &monitors {
        let wallpaper = config.wallpaper_for(&m.connector).clone();
        if wallpaper.effective_path().is_none()
            && wallpaper.paths.is_empty()
            && wallpaper.kind != Kind::Slideshow
        {
            continue; // nothing configured for this output
        }
        let mut out = WlOutput::new(m.connector.clone(), wallpaper, config.scaling);
        out.respawn(false, false);
        outputs.insert(m.connector.clone(), out);
    }
    log::info!(
        "frescod started (Wayland layer-shell / mpvpaper, {} output(s))",
        outputs.len()
    );
    crate::telemetry::heartbeat(Some("wayland"), None, Some(outputs.len() as u32));

    loop {
        let tick = if outputs.values().any(|o| o.animating) {
            ANIM_TICK
        } else {
            TICK
        };
        match commands.recv_timeout(tick) {
            Ok((req, reply)) => {
                let is_stop = matches!(req, Request::Stop);
                let resp = match req {
                    Request::Apply => {
                        config = Config::load().unwrap_or_else(|_| config.clone());
                        sched.hold_current(&config);
                        let paused = user_paused || battery_paused;
                        // A display plugged in after startup must be reachable
                        // without a daemon restart (interim until the native
                        // backend's registry-driven hotplug, ROADMAP 5.3):
                        // refresh the output list on every Apply.
                        match wayland_outputs::list_outputs() {
                            Ok(m) if !m.is_empty() => {
                                if m.len() != monitors.len() {
                                    log::info!(
                                        "output set changed on apply: {} -> {} output(s)",
                                        monitors.len(),
                                        m.len()
                                    );
                                }
                                monitors = m;
                            }
                            _ => {} // enumeration failed — keep the last snapshot
                        }
                        // Reap renderers whose connector is gone.
                        outputs.retain(|c, _| monitors.iter().any(|m| &m.connector == c));
                        if config.enabled {
                            // Reconcile config × the current output set.
                            for m in &monitors {
                                let wp = config.wallpaper_for(&m.connector).clone();
                                let has = wp.effective_path().is_some()
                                    || !wp.paths.is_empty()
                                    || wp.kind == Kind::Slideshow;
                                match (outputs.get_mut(&m.connector), has) {
                                    (Some(o), true) => {
                                        o.apply_wallpaper(wp, config.scaling, paused)
                                    }
                                    (Some(_), false) => {
                                        outputs.remove(&m.connector);
                                    }
                                    (None, true) => {
                                        let mut o =
                                            WlOutput::new(m.connector.clone(), wp, config.scaling);
                                        o.respawn(paused, false);
                                        outputs.insert(m.connector.clone(), o);
                                    }
                                    (None, false) => {}
                                }
                            }
                        } else {
                            outputs.clear(); // kills every mpvpaper
                        }
                        Response::Ok
                    }
                    Request::Pause => {
                        user_paused = true;
                        Response::Ok
                    }
                    Request::Resume => {
                        user_paused = false;
                        Response::Ok
                    }
                    Request::Status => Response::Status(wayland_status(
                        &monitors,
                        &outputs,
                        user_paused || battery_paused,
                    )),
                    Request::Update => {
                        notifier::run_updater_async();
                        Response::Ok
                    }
                    Request::Stop => Response::Ok,
                };
                let _ = reply.send(resp);
                if is_stop {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let now = Instant::now();

        // Slideshow engine (shared with the X11 path via advance_slideshow).
        for o in outputs.values_mut() {
            o.advance(now);
        }

        // Battery + per-output supervision on a coarse cadence.
        if now.duration_since(last_supervise) >= SUPERVISE {
            last_supervise = now;

            // Scheduled wallpaper swap (ROADMAP 3.3): media-only loadfile on
            // outputs showing the DEFAULT wallpaper — never a respawn.
            if let Some(want) = sched.due(&config) {
                if let Some(path) = want.effective_path().map(|p| p.to_path_buf()) {
                    log::info!(
                        "schedule: switching default wallpaper to {}",
                        path.display()
                    );
                    for (connector, o) in outputs.iter_mut() {
                        if !config.monitors.contains_key(connector) {
                            if let Some(pl) = o.player.as_ref() {
                                // Reset per-wallpaper player state, or the previous
                                // wallpaper's rotation/crop leak onto this one.
                                pl.set_rotation(want.rotation, o.scaling);
                                pl.apply_crop(&want);
                                pl.load_path(&path);
                            }
                            o.wallpaper.path = Some(path.clone());
                            o.wallpaper.rotation = want.rotation;
                            o.wallpaper.crop = want.crop;
                        }
                    }
                    config.wallpaper.path = Some(path.clone());
                    config.wallpaper.rotation = want.rotation;
                    config.wallpaper.crop = want.crop;
                    sched.applied = Some(path);
                }
            }

            if config.pause_on_battery {
                let discharging = on_battery();
                if discharging != battery_paused {
                    battery_paused = discharging;
                    log::info!("battery pause = {discharging}");
                }
            } else if battery_paused {
                battery_paused = false;
            }

            let paused = user_paused || battery_paused;
            for o in outputs.values_mut() {
                o.supervise(paused, MAX_RESTARTS);
            }

            sync_wayland_outputs(&outputs);
        }

        // Refresh fullscreen state on a coarse cadence, then reconcile every
        // output: paused = user || battery || fullscreen-on-this-output. This is
        // the single place pause is applied (reconcile_pause is change-gated), so
        // the three sources never fight over the player's pause property.
        if let Some(w) = fs_watch.as_mut() {
            if now.duration_since(last_fs_poll) >= FS_POLL {
                last_fs_poll = now;
                hidden = w.fullscreen_connectors();
            }
        }
        let base_paused = user_paused || battery_paused;
        for (connector, o) in &outputs {
            o.reconcile_pause(base_paused || hidden.contains(connector));
        }
    }

    outputs.clear(); // kill every mpvpaper before we exit
    std::fs::remove_file(crate::ipc::socket_path()).ok();
    log::info!("frescod stopped");
    Ok(())
}

/// Re-seat clones of the same video on one clock (see SYNC_INTERVAL/X11
/// `check_sync`): per-output pauses leave each mpvpaper's clock wherever it
/// stopped, so the same file on two outputs drifts further apart forever.
fn sync_wayland_outputs(outputs: &std::collections::BTreeMap<String, WlOutput>) {
    let mut groups: std::collections::HashMap<&std::path::Path, Vec<&WlOutput>> =
        std::collections::HashMap::new();
    for o in outputs.values() {
        if o.player.is_none()
            || o.applied_paused.get()
            || o.static_fallback
            || o.slideshow.is_some()
            || o.wallpaper.kind != Kind::Video
        {
            continue;
        }
        if let Some(p) = o.wallpaper.effective_path() {
            groups.entry(p).or_default().push(o);
        }
    }
    for group in groups.values() {
        if group.len() < 2 {
            continue;
        }
        let Some(lead) = group[0].player.as_ref().and_then(|p| p.time_pos()) else {
            continue;
        };
        for o in &group[1..] {
            let Some(pl) = o.player.as_ref() else {
                continue;
            };
            if let Some(pos) = pl.time_pos() {
                if (pos - lead).abs() > SYNC_TOLERANCE {
                    log::debug!(
                        "[{}] video {:.2}s out of sync with leader; re-seating",
                        o.connector,
                        pos - lead
                    );
                    pl.set_time_pos(lead);
                }
            }
        }
    }
}

/// One supervised output: its mpvpaper renderer (or none, in static fallback),
/// its slideshow state, and per-output restart bookkeeping.
struct WlOutput {
    connector: String,
    wallpaper: Wallpaper,
    scaling: Scaling,
    player: Option<PlayerHandle>,
    slideshow: Option<Slideshow>,
    restarts: u32,
    static_fallback: bool,
    error: Option<String>,
    animating: bool,
    /// Last pause state we applied to the player — lets `reconcile_pause` send IPC
    /// only on change. `Cell` so reconcile can stay `&self` like `set_paused`.
    applied_paused: std::cell::Cell<bool>,
    /// Frozen-but-alive detection: consecutive supervise ticks with no playback
    /// progress, plus the last sampled position.
    stall_strikes: u32,
    last_pos: Option<f64>,
    audio_heal: AudioHeal,
}

impl WlOutput {
    fn new(connector: String, wallpaper: Wallpaper, scaling: Scaling) -> WlOutput {
        WlOutput {
            connector,
            wallpaper,
            scaling,
            player: None,
            slideshow: None,
            restarts: 0,
            static_fallback: false,
            error: None,
            animating: false,
            applied_paused: std::cell::Cell::new(false),
            stall_strikes: 0,
            last_pos: None,
            audio_heal: AudioHeal::new(),
        }
    }

    /// (Re)spawn the mpvpaper for this output. `paused` applies the current pause
    /// state; `static_frame` spawns then pauses (holds frame one) — the no-black
    /// per-output fallback when live playback keeps failing.
    fn respawn(&mut self, paused: bool, static_frame: bool) {
        drop(self.player.take());
        self.slideshow = None;
        self.animating = false;
        self.stall_strikes = 0;
        self.last_pos = None;
        self.audio_heal = AudioHeal::new();
        // The initial file mpvpaper opens: for a slideshow it's the first image
        // (subsequent ones arrive via loadfile-replace); otherwise the media.
        let file = if self.wallpaper.kind == Kind::Slideshow {
            self.wallpaper
                .slideshow
                .as_ref()
                .and_then(|s| slideshow_images(s).into_iter().next())
        } else {
            self.wallpaper
                .effective_path()
                .map(|p| p.to_path_buf())
                .or_else(|| self.wallpaper.paths.first().cloned())
        };
        let Some(file) = file else {
            log::error!("[{}] no playable file configured", self.connector);
            self.error = Some(format!("{}: no playable file configured", self.connector));
            self.player = None;
            return;
        };
        match WaylandPlayer::spawn(&self.connector, &self.wallpaper, self.scaling, &file) {
            Ok(p) => {
                let handle = PlayerHandle::Wayland(p);
                if paused || static_frame {
                    handle.set_paused(true);
                }
                if !static_frame {
                    self.slideshow = build_slideshow(&self.wallpaper, &handle);
                    self.error = None;
                }
                self.player = Some(handle);
                self.applied_paused.set(paused || static_frame);
            }
            Err(e) => {
                log::error!("[{}] {e:#}", self.connector);
                if self.error.is_none() {
                    self.error = Some(e.to_string());
                }
                self.player = None;
            }
        }
    }

    /// Apply a (possibly changed) wallpaper. If only the media changed, switch in
    /// place via `loadfile replace`; otherwise respawn (fit/crop/scaling are
    /// spawn-time mpv options).
    fn apply_wallpaper(&mut self, new: Wallpaper, scaling: Scaling, paused: bool) {
        let media_only = self.player.is_some()
            && !self.static_fallback
            && scaling == self.scaling
            && new.fit == self.wallpaper.fit
            && new.rotation == self.wallpaper.rotation
            && new.mute == self.wallpaper.mute
            && new.volume == self.wallpaper.volume
            && new.crop == self.wallpaper.crop
            && new.kind == self.wallpaper.kind
            && new.kind != Kind::Slideshow;
        self.wallpaper = new;
        self.scaling = scaling;
        self.audio_heal = AudioHeal::new();
        if media_only {
            if let (Some(p), Some(path)) = (self.player.as_ref(), self.wallpaper.effective_path()) {
                p.load_path(path);
            }
        } else {
            self.restarts = 0;
            self.static_fallback = false;
            self.respawn(paused, false);
        }
    }

    /// Restore a dropped audio track on an unmuted wallpaper (see `AudioHeal`).
    /// Runs from `supervise` only while the renderer is alive and healthy.
    fn check_audio(&mut self, now: Instant) {
        if self.wallpaper.mute || !self.audio_heal.due(now) {
            return;
        }
        let status = self.player.as_ref().and_then(|p| p.audio_status());
        if let Some((false, _, _)) = status {
            log::info!(
                "[{}] unmuted wallpaper lost its audio track; restoring (attempt {})",
                self.connector,
                self.audio_heal.attempts + 1
            );
            let has_audio = self
                .player
                .as_ref()
                .map(|p| p.try_restore_audio(self.wallpaper.volume))
                .unwrap_or(true);
            if !has_audio {
                log::info!(
                    "[{}] file has no audio track; disabling audio recovery",
                    self.connector
                );
            }
            self.audio_heal.record(now, has_audio);
        }
    }

    fn advance(&mut self, now: Instant) {
        if let (Some(player), Some(s)) = (self.player.as_ref(), self.slideshow.as_mut()) {
            self.animating = advance_slideshow(player, s, now);
        }
    }

    fn set_paused(&self, paused: bool) {
        if let Some(p) = &self.player {
            p.set_paused(paused);
        }
    }

    /// Apply the desired pause state, but only on change and never to a static
    /// fallback frame (which must stay held/paused). This is the supervisor's one
    /// authority over the player's pause property — it folds the user, battery,
    /// and fullscreen sources into a single decision so they never fight.
    fn reconcile_pause(&self, desired: bool) {
        if self.static_fallback {
            return;
        }
        if self.applied_paused.get() != desired {
            self.set_paused(desired);
            self.applied_paused.set(desired);
        }
    }

    /// Sample playback position; returns true once it has failed to advance for
    /// `STALL_STRIKES` consecutive supervise ticks — a wedged-but-alive renderer
    /// (dead GL context / stopped decode that still passes `is_alive`).
    fn check_stall(&mut self) -> bool {
        let pos = self.player.as_ref().and_then(|p| p.time_pos());
        match (pos, self.last_pos) {
            (Some(cur), Some(prev)) if (cur - prev).abs() < 1e-3 => self.stall_strikes += 1,
            (Some(_), _) => self.stall_strikes = 0,
            (None, _) => {} // couldn't read the position; don't penalize
        }
        self.last_pos = pos;
        self.stall_strikes >= STALL_STRIKES
    }

    /// Per-output supervision: restart a dead — or frozen-but-alive — renderer with
    /// an anti-flap cap; after `max` consecutive failures fall back to a paused
    /// static frame (so the output never goes black) and surface it in `Status`.
    fn supervise(&mut self, paused: bool, max: u32) {
        let alive = self.player.as_ref().map(|p| p.is_alive()).unwrap_or(false);
        if alive {
            // A paused or static-fallback frame is not expected to advance — don't
            // sample. Otherwise check for a frozen-but-alive (wedged) renderer.
            let frozen = if self.static_fallback || self.applied_paused.get() {
                self.stall_strikes = 0;
                self.last_pos = None;
                false
            } else {
                self.check_stall()
            };
            if !frozen {
                if !self.static_fallback {
                    self.restarts = 0;
                    self.check_audio(Instant::now());
                }
                return;
            }
            log::warn!(
                "[{}] playback frozen (mpvpaper wedged); respawning",
                self.connector
            );
            // fall through to the restart path below
        }
        // Renderer is dead, never started, or frozen.
        if self.restarts < max {
            self.restarts += 1;
            log::warn!(
                "[{}] renderer exited; restarting ({}/{max})",
                self.connector,
                self.restarts
            );
            self.respawn(paused, false);
        } else if self.restarts == max {
            // Crossed the cap once: try to hold a paused static frame, then stop
            // retrying (anti-flap). If even that can't spawn, the compositor's own
            // background shows — Fresco never paints black itself.
            self.restarts += 1; // sentinel — no further attempts
            self.static_fallback = true;
            self.error = Some(format!(
                "{}: renderer failed {max}× — held a static frame (or fell back to the compositor background)",
                self.connector
            ));
            log::error!(
                "[{}] giving up live playback; attempting a static frame",
                self.connector
            );
            crate::telemetry::error(
                "renderer_giveup",
                &format!("{}: renderer failed {max}x", self.connector),
            );
            self.respawn(true, true);
        }
        // restarts > max → given up; do nothing (anti-flap). Error stays in Status.
    }
}

/// Aggregate `Status` across all Wayland outputs for the GUI / diagnostics.
fn wayland_status(
    monitors: &[Monitor],
    outputs: &std::collections::BTreeMap<String, WlOutput>,
    paused: bool,
) -> StatusReply {
    let child_pids: Vec<u32> = outputs
        .values()
        .filter_map(|o| o.player.as_ref().and_then(|p| p.child_pid()))
        .collect();
    let (cpu, rss) = proc_stats(&child_pids);
    let hwdec = outputs
        .values()
        .find_map(|o| o.player.as_ref().and_then(|p| p.hwdec_current()));
    let wallpaper = outputs
        .values()
        .next()
        .and_then(|o| {
            o.wallpaper
                .effective_path()
                .or_else(|| o.wallpaper.paths.first().map(|p| p.as_path()))
        })
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
    let error = outputs.values().find_map(|o| o.error.clone());
    let audio = outputs
        .values()
        .find_map(|o| o.player.as_ref().and_then(|p| p.audio_status()));
    let video = outputs
        .values()
        .find_map(|o| o.player.as_ref().and_then(|p| p.video_status()));
    StatusReply {
        running: true,
        paused,
        hwdec,
        wallpaper,
        cpu_percent: cpu,
        rss_mb: rss,
        monitors: outputs.keys().cloned().collect(),
        error,
        audio_track: audio.map(|(t, _, _)| t),
        mute: audio.map(|(_, m, _)| m),
        volume: audio.map(|(_, _, v)| v),
        source_w: video.map(|(w, _, _, _)| w),
        source_h: video.map(|(_, h, _, _)| h),
        bit_depth: video.map(|(_, _, d, _)| d),
        dropped_frames: video.map(|(_, _, _, n)| n),
        monitors_info: monitors_info_from(monitors),
    }
}

/// `--once <file>`: render one file on every monitor until Ctrl-C.
/// Used for the M1 renderer spike; ignores config and IPC.
pub fn run_once(file: PathBuf) -> Result<()> {
    setup_vaapi_env();
    let is_image = file
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "bmp"
            )
        })
        .unwrap_or(false);
    let wallpaper = Wallpaper {
        kind: if is_image { Kind::Image } else { Kind::Video },
        path: Some(file),
        ..Default::default()
    };
    let config = Config {
        wallpaper,
        ..Default::default()
    };

    let mut daemon = Daemon::new(config)?;
    daemon.rebuild()?;
    log::info!(
        "--once: rendering on {} monitor(s); Ctrl-C to quit",
        daemon.renderers.len()
    );
    loop {
        while let Ok(Some(_)) = daemon.conn.poll_for_event() {}
        if Instant::now().duration_since(daemon.last_lower) >= LOWER_INTERVAL {
            daemon.lower_all();
            daemon.last_lower = Instant::now();
        }
        std::thread::sleep(TICK);
    }
}

/// `--check`: print a colored diagnostics table and exit.
pub fn check() {
    const G: &str = "\x1b[32m";
    const R: &str = "\x1b[31m";
    const Y: &str = "\x1b[33m";
    const BLD: &str = "\x1b[1m";
    const X: &str = "\x1b[0m";

    println!("{BLD}Fresco diagnostics{X}");
    println!("──────────────────");

    use crate::capability::{detect, Capability};
    let cap = detect();
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());
    let session_color = if session == "x11" { G } else { Y };
    println!(
        "Session         : {session_color}{session}{X} ({})",
        cap.id()
    );

    if matches!(cap, Capability::WaylandLayerShell) {
        match crate::mpvpaper_resolved() {
            Some(p) => println!("mpvpaper        : {G}{}{X}", p.display()),
            None => println!(
                "mpvpaper        : {R}not found{X} (live wallpapers need mpvpaper installed or bundled)"
            ),
        }
        match fullscreen::FullscreenWatch::new().map(|w| w.backend()) {
            Some(fullscreen::Backend::Wlr) => {
                println!("Fullscreen pause: {G}enabled{X} (wlr-foreign-toplevel)")
            }
            Some(fullscreen::Backend::Cosmic) => {
                println!("Fullscreen pause: {G}enabled{X} (cosmic-toplevel-info)")
            }
            None => println!(
                "Fullscreen pause: {Y}unavailable{X} (compositor lacks wlr-foreign-toplevel-management and cosmic-toplevel-info)"
            ),
        }
    }

    match mpv::ffi::fns() {
        Ok(f) => {
            let v = f.client_api_version();
            println!(
                "libmpv          : {G}{}{X} (client API {}.{})",
                f.soname,
                v >> 16,
                v & 0xffff
            );
        }
        Err(e) => println!("libmpv          : {R}NOT LOADED{X} ({e})"),
    }

    if let Ok(out) = std::process::Command::new("sh")
        .arg("-c")
        .arg("lspci | grep -Ei 'vga|3d|display' | sed 's/.*: //'")
        .output()
    {
        for (i, line) in String::from_utf8_lossy(&out.stdout).lines().enumerate() {
            println!("GPU {i}           : {line}");
        }
    }

    let vainfo = which("vainfo");
    if vainfo {
        println!("VA-API (vainfo) : {G}available{X}");
    } else {
        println!("VA-API (vainfo) : {Y}not installed{X} (apt install intel-media-va-driver mesa-va-drivers)");
    }

    match Config::load() {
        Ok(c) => println!("Config          : {G}valid{X} (enabled={})", c.enabled),
        Err(e) => println!("Config          : {R}invalid{X} ({e})"),
    }

    match crate::ipc::request(&Request::Status) {
        Ok(Response::Status(s)) => {
            println!("Daemon          : {G}running{X}");
            println!(
                "  decode        : {}",
                s.hwdec.as_deref().unwrap_or("(none)")
            );
            println!(
                "  wallpaper     : {}",
                s.wallpaper.as_deref().unwrap_or("(none)")
            );
            println!("  RAM           : {} MB", s.rss_mb);
            if let Some(err) = s.error {
                println!("  {R}error{X}         : {err}");
            }
        }
        _ => println!("Daemon          : {Y}not running{X}"),
    }
}

fn which(bin: &str) -> bool {
    std::env::var("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(bin).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::parse_stat_ticks;

    #[test]
    fn stat_ticks_survive_weird_comm() {
        // comm may contain spaces and parens; fields count after the LAST ')'.
        let stat = "1234 (my (weird) comm) S 1 1234 1234 0 -1 4194304 500 0 0 0 700 42 0 0 20 0 4 0 100 0 0";
        assert_eq!(parse_stat_ticks(stat), Some(742));
        assert_eq!(parse_stat_ticks(""), None);
        assert_eq!(parse_stat_ticks("no parens here"), None);
    }
}

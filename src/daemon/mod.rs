//! Fresco wallpaper daemon: owns X11 desktop windows and embedded mpv players,
//! reconciles them against the config, and serves IPC control commands.

mod control;
mod dde;
mod fullscreen;
// Public so the widget engine's API stays visible while the daemon-loop call
// sites are being built out; the module is otherwise internal.
#[allow(dead_code)]
pub mod lyrics_runtime;
pub mod monitors;
pub mod mpv;
mod mpvpaper;
mod notifier;
mod overview;
mod wayland_outputs;
mod webbridge;
#[allow(dead_code)]
pub mod widgets;
mod x11_fullscreen;
mod x11win;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Screen;
use x11rb::rust_connection::RustConnection;

use crate::config::{Config, Kind, PowerSaving, Scaling, Transition, Wallpaper};
use crate::ipc::{MonitorInfo, Request, Response, StatusReply};

use monitors::Monitor;
use mpv::Player;
use mpvpaper::WaylandPlayer;
use x11win::{Atoms, WallpaperWindow, WindowKind};

const TICK: Duration = Duration::from_millis(100);

/// Bitmap overlays (`overlay-add`) use an id space separate from the ASS
/// overlays, whose ids live in `widgets` so engine and daemon can't disagree.
#[allow(dead_code)]
const OVERLAY_BMP_DISC: u32 = 0;

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
    fn set_rotation(&self, rotation: u16) {
        match self {
            PlayerHandle::X11(p) => p.set_rotation(rotation),
            PlayerHandle::Wayland(p) => p.set_rotation(rotation),
        }
    }
    /// Runtime scaler re-apply (scheduled swaps are media-only, so per-wallpaper
    /// rotation and power-saving level must be re-applied like crop). Call after
    /// `set_rotation` — it is the single owner of every scaler property.
    fn apply_scalers(&self, scaling: Scaling, power_saving: PowerSaving, rotation: u16) {
        match self {
            PlayerHandle::X11(p) => p.apply_scalers(scaling, power_saving, rotation),
            PlayerHandle::Wayland(p) => p.apply_scalers(scaling, power_saving, rotation),
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
    /// Draw an ASS overlay over the wallpaper; empty `ass` clears it.
    ///
    /// `id` separates widgets: each owns one overlay slot ([`OVERLAY_LYRICS`],
    /// [`OVERLAY_CLOCK`]) so they compose instead of overwriting each other.
    ///
    /// Deliberately implemented on BOTH arms. `raise_demuxer_cache` below is the
    /// standing example of a handle method that quietly does nothing on one
    /// backend; a widget that rendered only on Wayland would be that bug where
    /// users can see it.
    #[allow(dead_code)] // wired up by the widget runtime (docs/WIDGETS_ROADMAP.md W1)
    fn set_overlay(&self, id: u32, ass: &str, res_x: u32, res_y: u32) {
        match self {
            PlayerHandle::X11(p) => p.set_overlay(id, ass, res_x, res_y),
            PlayerHandle::Wayland(p) => p.set_overlay(id, ass, res_x, res_y),
        }
    }
    /// Place a raw BGRA bitmap over the wallpaper (the album-art disc).
    /// Symmetric across backends for the same reason as `set_overlay`.
    #[allow(dead_code, clippy::too_many_arguments)]
    fn overlay_add(&self, id: u32, x: i32, y: i32, path: &str, w: u32, h: u32, stride: u32) {
        match self {
            PlayerHandle::X11(p) => p.overlay_add(id, x, y, path, w, h, stride),
            PlayerHandle::Wayland(p) => p.overlay_add(id, x, y, path, w, h, stride),
        }
    }
    #[allow(dead_code)]
    fn overlay_remove(&self, id: u32) {
        match self {
            PlayerHandle::X11(p) => p.overlay_remove(id),
            PlayerHandle::Wayland(p) => p.overlay_remove(id),
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
    /// Length of the current file; `0` for a still image (see `check_stall`).
    fn duration(&self) -> Option<f64> {
        match self {
            PlayerHandle::X11(p) => p.duration(),
            PlayerHandle::Wayland(p) => p.duration(),
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
    last_stacking: Instant,
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
    /// Deepin DDE quirk (issue #2): whether/how DDE's covering desktop window
    /// is being handled. `Inactive` on every other desktop.
    dde_mode: dde::Mode,
    /// True once the one-time DDE render self-check has run (it blocks ~1s).
    dde_self_checked: bool,
    /// Restack mode only: lets the desktop icons stay up for a few seconds
    /// after the user clicks the desktop, instead of burying them again on the
    /// next stacking pass.
    dde_peek: dde::IconPeek,
    /// On-wallpaper widgets (lyrics, clock). Owns its own worker thread and
    /// hands back only overlays whose content actually changed, so an idle
    /// desktop costs nothing (see docs/WIDGETS_ROADMAP.md "Power model").
    widgets: widgets::WidgetEngine,
}

impl Daemon {
    pub fn new(config: Config) -> Result<Daemon> {
        let (conn, screen_num) =
            x11rb::connect(None).context("connecting to X11 (is DISPLAY set?)")?;
        let atoms = Atoms::new(&conn)?.reply()?;
        let mut widgets =
            widgets::WidgetEngine::new(config.widgets.as_ref(), accent_hex(config.accent));
        apply_widget_config(&mut widgets, &config);
        Ok(Daemon {
            conn,
            screen_num,
            atoms,
            renderers: Vec::new(),
            config,
            user_paused: false,
            battery_paused: false,
            last_stacking: Instant::now(),
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
            dde_mode: dde::Mode::Inactive,
            dde_self_checked: false,
            dde_peek: dde::IconPeek::default(),
            widgets,
        })
    }

    /// Push any widget overlay whose content changed onto the renderer that
    /// owns them. Returns immediately with nothing to do on the overwhelming
    /// majority of ticks — the engine compares rendered content, so a static
    /// lyric line paints once and a clock showing 14:32 paints nothing until
    /// 14:33.
    fn push_widgets(&mut self) {
        if !self.widgets.is_active() {
            return;
        }
        let updates = self.widgets.tick();
        if updates.is_empty() {
            return;
        }
        // A widget belongs to one display: the configured connector, else the
        // first renderer. Pushing to every renderer would duplicate the lyric
        // across monitors, which reads as a bug rather than a feature.
        // No configured connector = every display, matching the wallpaper
        // itself: the widget is part of the wallpaper, so a two-monitor desktop
        // showing it on one screen reads as half-broken. Naming a connector in
        // `widgets.monitor` narrows it to that one.
        let want = self.widgets.monitor().map(str::to_string);
        let targets: Vec<&Renderer> = self
            .renderers
            .iter()
            .filter(|r| {
                want.as_deref()
                    .is_none_or(|c| c == r.window.connector.as_str())
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        // The bitmap widgets place pixels in real output coordinates, not the
        // ASS PLAY_RES space, so the engine needs this display's actual mode.
        if let Some(m) = self
            .monitors
            .iter()
            .find(|m| m.connector == targets[0].window.connector)
        {
            self.widgets
                .set_output_size(u32::from(m.width), u32::from(m.height));
        }
        for u in updates {
            log::debug!(
                "widget: overlay {} -> {} chars on {} display(s)",
                u.overlay_id,
                u.ass.len(),
                targets.len()
            );
            for r in &targets {
                dispatch_widget(&r.player, &u);
            }
        }
    }

    /// Blank every widget overlay. Called before a rebuild/teardown so an
    /// overlay can never survive onto the next wallpaper — the leak class the
    /// scheduled-swap comments warn about.
    fn clear_widgets(&mut self) {
        for u in self.widgets.clear_all() {
            for r in &self.renderers {
                dispatch_widget(&r.player, &u);
            }
        }
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

        // Deepin DDE (issue #2) needs a differently declared window, and the
        // declaration can only be chosen at creation time. Off Deepin this is
        // `WindowKind::Desktop` — the window Fresco has always created — with
        // no X11 roundtrip.
        let kind = dde::window_kind(&self.conn, &self.atoms, screen.root, self.config.dde_mode);

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
                wallpaper.effective_power_saving(self.config.power_saving),
                kind,
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

        // Deepin DDE (issue #2): dde-shell's own opaque desktop window covers
        // ours. Make DDE's wallpaper transparent (or restack above it).
        if crate::capability::is_deepin_dde() && !self.renderers.is_empty() {
            let monitors: Vec<String> = self.monitors.iter().map(|m| m.connector.clone()).collect();
            let windows: Vec<x11rb::protocol::xproto::Window> =
                self.renderers.iter().map(|r| r.window.window).collect();
            self.dde_mode = dde::apply(
                &self.conn,
                &self.atoms,
                screen.root,
                &monitors,
                &windows,
                self.config.dde_mode,
            );
            // One-time best-effort check that our window actually renders
            // frames — the user's log then tells the whole DDE story.
            if self.dde_mode != dde::Mode::Inactive && !self.dde_self_checked {
                self.dde_self_checked = true;
                dde::render_self_check(&self.conn, &windows);
            }
        }
        Ok(())
    }

    // The X11 primitives (conn/screen/atoms/monitor) plus wallpaper, render
    // prefs, and window kind are all genuinely independent inputs; grouping
    // them would obscure more than it saves for a single internal builder.
    #[allow(clippy::too_many_arguments)]
    fn make_renderer(
        conn: &RustConnection,
        screen: &Screen,
        atoms: &Atoms,
        monitor: &Monitor,
        wallpaper: &Wallpaper,
        scaling: Scaling,
        power_saving: PowerSaving,
        kind: WindowKind,
    ) -> Result<Renderer> {
        let window = WallpaperWindow::create(conn, screen, atoms, monitor, kind)?;
        let player = PlayerHandle::X11(Player::new(
            window.window,
            wallpaper,
            scaling,
            power_saving,
        )?);
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
            if now.duration_since(self.last_stacking) >= LOWER_INTERVAL {
                self.reassert_stacking();
                self.last_stacking = now;
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
            self.push_widgets();
            let animating = self.advance_slideshows(now);

            std::thread::sleep(if animating { ANIM_TICK } else { TICK });
        }
    }

    fn handle_request(&mut self, req: Request) -> Response {
        match req {
            Request::Apply => {
                self.config = Config::load().unwrap_or_else(|_| self.config.clone());
                self.sched.hold_current(&self.config);
                // Widget settings live in the same file, so a GUI toggle arrives
                // here. Re-push unconditionally: a style change must repaint even
                // when the lyric line itself hasn't moved.
                let cfg = self.config.clone();
                apply_widget_config(&mut self.widgets, &cfg);
                self.widgets.invalidate();
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

    /// Re-assert every wallpaper window's place in the stack (~every 2s), since
    /// other clients' stacking changes can shuffle us. Normally that means
    /// lowering back to the bottom; in DDE restack mode our windows must be
    /// RAISED instead — lowering there would drop the wallpaper straight back
    /// under dde-shell's desktop window a couple of seconds after it appeared.
    ///
    /// The DDE raise is not unconditional: when the user clicks the desktop,
    /// DDE's window comes up above ours and the icons become usable, so the
    /// raise waits out `dde_icon_peek_secs` before taking the stack back (see
    /// [`dde::IconPeek`]).
    fn reassert_stacking(&mut self) {
        if self.dde_mode == dde::Mode::Restack {
            let windows: Vec<x11rb::protocol::xproto::Window> =
                self.renderers.iter().map(|r| r.window.window).collect();
            let root = self.screen().root;
            let peek = dde::icon_peek(self.config.dde_icon_peek_secs);
            self.dde_peek
                .tick(&self.conn, &self.atoms, root, &windows, peek);
            return;
        }
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
        // Blank widgets first: the players are about to go, and a fresh mpv
        // starts with no overlays, so state must be re-established after.
        self.clear_widgets();
        self.widgets.invalidate();
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
                // Rotation, scalers (power-saving), and crop are per-wallpaper
                // state on the mpv instance; without resetting them here the
                // previous wallpaper's settings leak onto the scheduled one.
                // apply_scalers must follow set_rotation (it owns cscale).
                r.player.set_rotation(want.rotation);
                r.player.apply_scalers(
                    self.config.scaling,
                    want.effective_power_saving(self.config.power_saving),
                    want.rotation,
                );
                r.player.apply_crop(&want);
                r.player.load_path(&path);
                r.cache_raised.set(false); // re-check resolution for the new media
            }
        }
        // Keep the in-memory config coherent for status/describe. NEVER saved:
        // the on-disk config remains the user's own intent.
        self.config.wallpaper.path = Some(path.clone());
        self.config.wallpaper.rotation = want.rotation;
        self.config.wallpaper.power_saving = want.power_saving;
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
        // Put the user's original DDE wallpaper back (no-op off DDE / when
        // nothing was saved).
        if crate::capability::is_deepin_dde() {
            dde::restore();
        }
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
    notifier::spawn_support_watcher();

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
        // Same for DDE: a crashed run may have left the transparent wallpaper
        // applied with the original saved on disk — restore it (no-op
        // otherwise).
        if crate::capability::is_deepin_dde() {
            dde::restore();
        }
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
    // How often to ask whether a parked display has come back. Nothing is
    // retrying at that point, so this only bounds how long a returning monitor
    // stays blank.
    const PARKED_PROBE: Duration = Duration::from_secs(10);
    // mpvpaper's "every output" target, used when enumeration failed at start.
    // It is not a real connector name, so it can never appear in an output list.
    const ALL_OUTPUTS: &str = "ALL";

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
            connector: ALL_OUTPUTS.into(),
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
    // Display-presence probe: due immediately, then paced by whether anything is
    // still restarting (see the supervise block).
    let mut next_output_probe = Instant::now();
    let mut probed = false;
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
    // Sum of every output's respawn counter. A fresh mpv has no overlays, so a
    // change here means the widget engine must re-push. Summing (rather than
    // tracking per output) is enough: the engine re-pushes everything it owns,
    // and it emits nothing when content is unchanged, so a re-push costs one
    // repaint rather than a stream of them.
    let mut last_generations: u64 = 0;
    // On-wallpaper widgets. Same engine the X11 loop uses, so the two backends
    // cannot drift apart — the failure mode `raise_demuxer_cache` is named for.
    let mut widget_engine =
        widgets::WidgetEngine::new(config.widgets.as_ref(), accent_hex(config.accent));
    apply_widget_config(&mut widget_engine, &config);

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
        let effective_ps = wallpaper.effective_power_saving(config.power_saving);
        let mut out = WlOutput::new(m.connector.clone(), wallpaper, config.scaling, effective_ps);
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
                        // Widget settings ride in the same file, so a GUI toggle
                        // arrives here. invalidate() forces a repaint even when
                        // the content itself (e.g. the lyric line) is unchanged.
                        apply_widget_config(&mut widget_engine, &config);
                        widget_engine.invalidate();
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
                                let effective_ps = wp.effective_power_saving(config.power_saving);
                                match (outputs.get_mut(&m.connector), has) {
                                    (Some(o), true) => {
                                        o.apply_wallpaper(wp, config.scaling, effective_ps, paused)
                                    }
                                    (Some(_), false) => {
                                        outputs.remove(&m.connector);
                                    }
                                    (None, true) => {
                                        let mut o = WlOutput::new(
                                            m.connector.clone(),
                                            wp,
                                            config.scaling,
                                            effective_ps,
                                        );
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
                                // wallpaper's rotation/crop/power-saving leak onto this one.
                                // apply_scalers must follow set_rotation (it owns cscale).
                                pl.set_rotation(want.rotation);
                                pl.apply_scalers(
                                    o.scaling,
                                    want.effective_power_saving(config.power_saving),
                                    want.rotation,
                                );
                                pl.apply_crop(&want);
                                pl.load_path(&path);
                            }
                            o.wallpaper.path = Some(path.clone());
                            o.wallpaper.rotation = want.rotation;
                            o.wallpaper.power_saving = want.power_saving;
                            o.wallpaper.crop = want.crop;
                            o.power_saving = want.effective_power_saving(config.power_saving);
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
            // A renderer that is down may be down because its display went away
            // (monitor asleep, DisplayPort link dropped) — restarting into a
            // connector the compositor no longer advertises can only fail, and
            // burns the anti-flap budget permanently. Enumerating costs a
            // Wayland roundtrip, so ask only when something is actually down.
            let present: Option<HashSet<String>> =
                if outputs.values().any(|o| o.renderer_down()) && now >= next_output_probe {
                    probed = true;
                    wayland_outputs::list_outputs()
                        .ok()
                        .map(|m| m.into_iter().map(|x| x.connector).collect())
                } else {
                    None
                };
            for (connector, o) in outputs.iter_mut() {
                // No probe this tick, or enumeration failed → assume present,
                // i.e. exactly the behaviour before the display check existed.
                let here = connector == ALL_OUTPUTS
                    || present.as_ref().is_none_or(|s| s.contains(connector));
                o.supervise(paused, MAX_RESTARTS, here);
            }
            if probed {
                probed = false;
                // While restarts are still in flight the answer is needed every
                // tick, before the budget is spent; once every down output is
                // parked we are only waiting for a display to come back.
                next_output_probe = now
                    + if outputs.values().any(|o| o.renderer_down() && !o.absent) {
                        Duration::ZERO
                    } else {
                        PARKED_PROBE
                    };
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

        // A respawn anywhere (supervisor heal, static-frame fallback, output
        // re-creation) leaves that mpv with no overlays. Re-push once when the
        // total changes, instead of threading a callback through every heal
        // path — the class of bug where one path gets missed.
        let generations: u64 = outputs.values().map(|o| o.generation).sum();
        if generations != last_generations {
            last_generations = generations;
            widget_engine.invalidate();
        }

        // Widgets: only overlays whose content actually changed come back, so
        // this is a cheap no-op on almost every pass.
        if widget_engine.is_active() {
            let updates = widget_engine.tick();
            if !updates.is_empty() {
                // Every display unless a connector is configured — see the
                // X11 `push_widgets` note; the two backends must agree.
                let want = widget_engine.monitor().map(str::to_string);
                // Bitmap widgets place pixels in real output coordinates, not
                // the ASS PLAY_RES space, so the engine needs the actual mode.
                if let Some(m) = monitors
                    .iter()
                    .find(|m| want.as_deref().is_none_or(|w| w == m.connector.as_str()))
                {
                    widget_engine.set_output_size(u32::from(m.width), u32::from(m.height));
                }
                for u in updates {
                    for (c, o) in &outputs {
                        if want.as_deref().is_some_and(|w| w != c.as_str()) {
                            continue;
                        }
                        if let Some(p) = o.player.as_ref() {
                            dispatch_widget(p, &u);
                        }
                    }
                }
            }
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
    /// Effective power-saving level for this output; applied at spawn.
    power_saving: PowerSaving,
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
    /// Parked because the compositor no longer advertises this connector (the
    /// monitor slept, or a DisplayPort link dropped). Nothing can render to a
    /// display that isn't there, so failures against it must not count.
    absent: bool,
    /// Parked because the wallpaper has nothing playable behind it (empty or
    /// unreadable slideshow folder, media that moved). Latched so the reason is
    /// logged once rather than on every supervise tick.
    no_media: bool,
    /// Why the last renderer went down ("dead"/"frozen") and, if respawning also
    /// failed, its content-free [`SpawnFail`] code — reported with `renderer_giveup`
    /// so a warning in the field says what actually broke.
    last_down: &'static str,
    last_spawn_fail: Option<&'static str>,
    /// Bumped on every [`WlOutput::respawn`]. A fresh mpv carries no overlays,
    /// so the widget engine must re-push after one — but the supervisor has
    /// several heal paths and threading a callback through each is how one gets
    /// missed. The loop instead watches this counter, which cannot go stale
    /// because `respawn` is the only writer.
    generation: u64,
}

impl WlOutput {
    fn new(
        connector: String,
        wallpaper: Wallpaper,
        scaling: Scaling,
        power_saving: PowerSaving,
    ) -> WlOutput {
        WlOutput {
            connector,
            wallpaper,
            scaling,
            power_saving,
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
            absent: false,
            no_media: false,
            last_down: "never_started",
            last_spawn_fail: None,
            generation: 0,
        }
    }

    /// The file a spawn would open: for a slideshow the first image (later ones
    /// arrive via `loadfile replace`), otherwise the configured media.
    ///
    /// Only a file that is actually **there** counts. A slideshow folder that is
    /// empty, unreadable, or holds nothing we recognise as an image resolves to
    /// nothing at all — and so does media that has since been deleted or that
    /// lives on a mount which is currently away.
    fn playable_file(&self) -> Option<PathBuf> {
        let candidates: Vec<PathBuf> = if self.wallpaper.kind == Kind::Slideshow {
            self.wallpaper
                .slideshow
                .as_ref()
                .map(slideshow_images)
                .unwrap_or_default()
        } else {
            self.wallpaper
                .effective_path()
                .map(|p| p.to_path_buf())
                .into_iter()
                .chain(self.wallpaper.paths.iter().cloned())
                .collect()
        };
        candidates.into_iter().find(|p| p.exists())
    }

    /// (Re)spawn the mpvpaper for this output. `paused` applies the current pause
    /// state; `static_frame` spawns then pauses (holds frame one) — the no-black
    /// per-output fallback when live playback keeps failing.
    fn respawn(&mut self, paused: bool, static_frame: bool) {
        // Bumped first: every exit from this function leaves a player that has
        // no overlays, including the failure paths.
        self.generation = self.generation.wrapping_add(1);
        drop(self.player.take());
        self.slideshow = None;
        self.animating = false;
        self.stall_strikes = 0;
        self.last_pos = None;
        self.audio_heal = AudioHeal::new();
        let Some(file) = self.playable_file() else {
            log::error!("[{}] no playable file configured", self.connector);
            self.error = Some(format!("{}: no playable file configured", self.connector));
            self.last_spawn_fail = Some("no_file");
            self.player = None;
            return;
        };
        match WaylandPlayer::spawn(
            &self.connector,
            &self.wallpaper,
            self.scaling,
            self.power_saving,
            &file,
        ) {
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
                self.last_spawn_fail = None;
            }
            Err(e) => {
                log::error!("[{}] {e:#}", self.connector);
                self.last_spawn_fail = Some(
                    crate::daemon::mpvpaper::SpawnFail::of(&e).map_or("spawn_failed", |f| f.code()),
                );
                if self.error.is_none() {
                    self.error = Some(e.to_string());
                }
                self.player = None;
            }
        }
    }

    /// Apply a (possibly changed) wallpaper. If only the media changed, switch in
    /// place via `loadfile replace`; otherwise respawn (fit/crop/scaling/power-saving
    /// are spawn-time mpv options).
    fn apply_wallpaper(
        &mut self,
        new: Wallpaper,
        scaling: Scaling,
        power_saving: PowerSaving,
        paused: bool,
    ) {
        let media_only = self.player.is_some()
            && !self.static_fallback
            && scaling == self.scaling
            && power_saving == self.power_saving
            && new.fit == self.wallpaper.fit
            && new.rotation == self.wallpaper.rotation
            && new.mute == self.wallpaper.mute
            && new.volume == self.wallpaper.volume
            && new.crop == self.wallpaper.crop
            && new.kind == self.wallpaper.kind
            && new.kind != Kind::Slideshow;
        self.wallpaper = new;
        self.scaling = scaling;
        self.power_saving = power_saving;
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
        let strikes = stall_step(self.last_pos, pos, self.stall_strikes, || {
            self.player.as_ref().is_some_and(holds_frame_by_design)
        });
        self.stall_strikes = strikes;
        self.last_pos = pos;
        self.stall_strikes >= STALL_STRIKES
    }

    /// True while this output has no running renderer — the supervisor's cue to
    /// ask the compositor whether the display is still there before spending the
    /// restart budget on it.
    fn renderer_down(&self) -> bool {
        self.player.as_ref().is_none_or(|p| !p.is_alive())
    }

    /// Park an output whose display is gone: kill the renderer and stop counting
    /// failures against it. Idempotent — called on every tick while it's away.
    fn park_absent(&mut self) {
        if !self.absent {
            log::info!(
                "[{}] display is no longer connected; parking until it returns",
                self.connector
            );
            self.absent = true;
            self.error = Some(format!(
                "{}: display disconnected — waiting for it to come back",
                self.connector
            ));
        }
        drop(self.player.take());
        self.slideshow = None;
        self.animating = false;
        self.stall_strikes = 0;
        self.last_pos = None;
    }

    /// Per-output supervision: restart a dead — or frozen-but-alive — renderer with
    /// an anti-flap cap; after `max` consecutive failures fall back to a paused
    /// static frame (so the output never goes black) and surface it in `Status`.
    ///
    /// `output_present` is the compositor's answer to "is this connector still
    /// there?" — only consulted once the renderer is down, so a bad enumeration
    /// can never tear down a healthy one.
    fn supervise(&mut self, paused: bool, max: u32, output_present: bool) {
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
            self.last_down = "frozen";
            // fall through to the restart path below
        } else if self.player.is_some() {
            self.last_down = "dead";
        }
        // Renderer is dead, never started, or frozen.
        if !output_present {
            // Nothing can render to a display the compositor no longer
            // advertises (monitor asleep, DisplayPort link dropped). Spending
            // the restart budget here is what turned a sleeping monitor into a
            // permanent give-up that outlived the display's return.
            self.park_absent();
            return;
        }
        if self.absent {
            // It's back. The failures we counted belonged to the vanished
            // display, not to this renderer — start clean rather than resuming
            // a budget that was already spent.
            log::info!("[{}] display is back; restoring playback", self.connector);
            self.absent = false;
            self.restarts = 0;
            self.static_fallback = false;
            self.error = None;
            // Spawn on the next tick (2s), so a returning display does exactly
            // one thing per tick and the restored state is observable.
            return;
        }
        if self.playable_file().is_none() {
            // Nothing to open — an empty or unreadable slideshow folder, media
            // that was deleted, a mount that is away. No respawn can fix a
            // configuration problem, and counting these as renderer failures
            // spends the budget in ten seconds and then tells the user
            // "renderer failed 5×", which hides the actual cause. Say what is
            // wrong and wait: as soon as a file is there again the normal
            // restart path below picks it up, with a full budget.
            if !self.no_media {
                log::error!(
                    "[{}] nothing to play (no readable media configured); waiting",
                    self.connector
                );
                self.no_media = true;
                self.error = Some(format!(
                    "{}: nothing to play — the wallpaper's file or slideshow folder is empty, missing, or unreadable",
                    self.connector
                ));
            }
            drop(self.player.take());
            self.slideshow = None;
            self.animating = false;
            return;
        }
        self.no_media = false;
        if self.restarts < max {
            self.restarts += 1;
            log::warn!(
                "[{}] renderer down ({}); restarting ({}/{max})",
                self.connector,
                self.last_down,
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
            // Content-free by construction: a connector name, the failure mode,
            // the wallpaper kind and a SpawnFail code — never a path or a file
            // name. Without them a report in the field says only "it failed".
            crate::telemetry::error(
                "renderer_giveup",
                &format!(
                    "{}: renderer failed {max}x (mode={}, kind={:?}, cause={})",
                    self.connector,
                    self.last_down,
                    self.wallpaper.kind,
                    self.last_spawn_fail.unwrap_or("spawn_ok"),
                ),
            );
            self.respawn(true, true);
        }
        // restarts > max → given up; do nothing (anti-flap). Error stays in Status.
    }
}

/// One stall-detector step: the strike count given the previous and current
/// playback positions. Split out of `WlOutput::check_stall` so the decision —
/// including the held-frame exemption — is unit-testable without a renderer.
fn stall_step(
    prev: Option<f64>,
    cur: Option<f64>,
    strikes: u32,
    holds_frame: impl FnOnce() -> bool,
) -> u32 {
    match (cur, prev) {
        // The position hasn't moved. Only media that is *supposed* to advance
        // earns a strike; `holds_frame` costs an IPC round-trip, so it is asked
        // lazily — never on the healthy path.
        (Some(c), Some(p)) if (c - p).abs() < 1e-3 => {
            if holds_frame() {
                0
            } else {
                strikes + 1
            }
        }
        (Some(_), _) => 0,
        (None, _) => strikes, // couldn't read the position; don't penalize
    }
}

/// True when the player is showing media that legitimately never advances its
/// clock. Images are spawned with `image-display-duration=inf`, so mpv holds
/// `time-pos` at 0 forever while reporting `duration` 0 — reading that as a
/// wedged renderer is what made image and slideshow wallpapers respawn until
/// the supervisor gave up on the output. The X11 backend has always skipped
/// stills in `check_cold_boot_stall`; asking the player rather than the
/// configured kind also covers a playlist that mixes stills with video. An
/// unreadable duration counts as held, matching the unreadable-position case.
fn holds_frame_by_design(player: &PlayerHandle) -> bool {
    player.duration().is_none_or(|d| d <= 0.0)
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
        if Instant::now().duration_since(daemon.last_stacking) >= LOWER_INTERVAL {
            daemon.reassert_stacking();
            daemon.last_stacking = Instant::now();
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

    // The widget helpers. Both fail as *silence* — a widget that is enabled in
    // the config and never draws — so the only place a user can find out is a
    // diagnostic like this one. Yellow, not red: a desktop running no widgets
    // is entirely healthy without either.
    if which("gdbus") {
        println!("MPRIS (gdbus)   : {G}available{X}");
    } else {
        println!("MPRIS (gdbus)   : {Y}not installed{X} (apt install libglib2.0-bin — lyrics, album art and the track-synced clock need it)");
    }
    match (which("pw-cat"), which("parec")) {
        (true, _) => println!("Audio capture   : {G}pw-cat{X}"),
        (false, true) => println!("Audio capture   : {G}parec{X}"),
        (false, false) => println!(
            "Audio capture   : {Y}not installed{X} (apt install pipewire-bin or pulseaudio-utils — needed by the audio visualiser widget)"
        ),
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

/// Send one widget update to a player. Text widgets go through `set_overlay`;
/// the album-art disc is a bitmap and goes through `overlay_add`/`overlay_remove`
/// instead — an empty ASS payload does NOT take a bitmap overlay down, which is
/// why this is a match and not a single call.
fn dispatch_widget(p: &PlayerHandle, u: &widgets::WidgetUpdate) {
    match &u.bitmap {
        None => p.set_overlay(u.overlay_id, &u.ass, widgets::RES_X, widgets::RES_Y),
        Some(widgets::BitmapUpdate::Draw(b)) => {
            p.overlay_add(u.overlay_id, b.x, b.y, &b.path_str(), b.w, b.h, b.stride)
        }
        Some(widgets::BitmapUpdate::Remove) => p.overlay_remove(u.overlay_id),
    }
}

/// `config::Visualizer` -> the widget engine's visualiser settings.
fn widget_visual_cfg(v: &crate::config::Visualizer) -> widgets::VisualCfg {
    use crate::config::{GradientMode, VisualizerStyleCfg};
    use crate::visualizer::{Gradient, VisualStyle, VisualStyleCfg};
    widgets::VisualCfg {
        enabled: v.enabled,
        style: VisualStyleCfg {
            style: match v.style {
                VisualizerStyleCfg::Bars => VisualStyle::Bars,
                VisualizerStyleCfg::Mirror => VisualStyle::Mirror,
                VisualizerStyleCfg::Wave => VisualStyle::Wave,
                VisualizerStyleCfg::Dots => VisualStyle::Dots,
                VisualizerStyleCfg::Ring => VisualStyle::Ring,
            },
            anchor: widget_anchor(v.anchor),
            width_pct: v.width_pct as f32,
            height_px: v.height_px,
            margin_px: v.margin_px,
            // The user's own colour, not the accent: `accent_follow` is what
            // decides between the two, and `render_ass` is given the accent
            // separately. Passing the accent here as well made the fill the
            // accent either way, so turning the switch off changed nothing.
            colour: v.colour.clone(),
            accent_follow: v.accent_follow,
            gradient: match v.gradient {
                GradientMode::None => Gradient::None,
                GradientMode::Linear => Gradient::Linear,
                GradientMode::Spectrum => Gradient::Spectrum,
            },
            colour_end: v.colour_end.clone(),
            opacity: v.opacity,
            gap_px: 4,
            rounded: v.rounded,
        },
        bands: v.bands as usize,
        ..widgets::VisualCfg::default()
    }
}

/// `config::Disc` -> the widget engine's album-art settings.
fn widget_disc_cfg(d: &crate::config::Disc) -> widgets::DiscWidgetCfg {
    widgets::DiscWidgetCfg {
        enabled: d.enabled,
        anchor: widget_anchor(d.anchor),
        size_px: d.size_px,
        margin_px: d.margin_px,
        spin: d.spin,
        opacity: d.opacity,
    }
}

/// The one place `config::LyricAnchor` becomes `lyrics::Anchor`. Every widget
/// shares the nine-point grid, so this mapping must exist exactly once.
fn widget_anchor(a: crate::config::LyricAnchor) -> crate::lyrics::Anchor {
    use crate::config::LyricAnchor as C;
    use crate::lyrics::Anchor as A;
    match a {
        C::TopLeft => A::TopLeft,
        C::TopCenter => A::TopCenter,
        C::TopRight => A::TopRight,
        C::MidLeft => A::MidLeft,
        C::MidCenter => A::MidCenter,
        C::MidRight => A::MidRight,
        C::BottomLeft => A::BottomLeft,
        C::BottomCenter => A::BottomCenter,
        C::BottomRight => A::BottomRight,
    }
}

/// Push every widget setting from `config` into `engine`, in one place so the
/// three loops cannot drift on which widgets they remember to update.
fn apply_widget_config(engine: &mut widgets::WidgetEngine, config: &Config) {
    let accent = accent_hex(config.accent);
    let w = config.widgets.as_ref();
    engine.set_config(w, accent);
    engine.set_clock(w.map(|w| widget_clock_cfg(&w.clock)).as_ref());
    engine.set_visualizer(w.map(|w| widget_visual_cfg(&w.visualizer)).as_ref());
    engine.set_disc(w.map(|w| widget_disc_cfg(&w.disc)).as_ref());
}

/// `config::Clock` -> the widget engine's clock settings. The mapping
/// `config::Clock` documents as "the daemon owns the one small mapping".
fn widget_clock_cfg(c: &crate::config::Clock) -> widgets::ClockCfg {
    use crate::clock::{ClockStyle, ClockTheme};
    use crate::config::{ClockThemeCfg, LyricAnchor};
    use crate::lyrics::Anchor;
    widgets::ClockCfg {
        enabled: c.enabled,
        style: ClockStyle {
            theme: match c.theme {
                ClockThemeCfg::Digital => ClockTheme::Digital,
                ClockThemeCfg::Minimal => ClockTheme::Minimal,
                ClockThemeCfg::Segment => ClockTheme::Segment,
                ClockThemeCfg::Stacked => ClockTheme::Stacked,
                ClockThemeCfg::Wordy => ClockTheme::Wordy,
                ClockThemeCfg::Card => ClockTheme::Card,
            },
            anchor: match c.anchor {
                LyricAnchor::TopLeft => Anchor::TopLeft,
                LyricAnchor::TopCenter => Anchor::TopCenter,
                LyricAnchor::TopRight => Anchor::TopRight,
                LyricAnchor::MidLeft => Anchor::MidLeft,
                LyricAnchor::MidCenter => Anchor::MidCenter,
                LyricAnchor::MidRight => Anchor::MidRight,
                LyricAnchor::BottomLeft => Anchor::BottomLeft,
                LyricAnchor::BottomCenter => Anchor::BottomCenter,
                LyricAnchor::BottomRight => Anchor::BottomRight,
            },
            font_size_pt: c.font_size_pt,
            margin_px: c.margin_px,
            show_seconds: c.show_seconds,
            show_date: c.show_date,
            use_24h: c.use_24h,
            // No colour key in the config on purpose; accent-follow is the path.
            colour: "#FFFFFF".to_string(),
            accent_follow: c.accent_follow,
        },
    }
}

/// Accent hex for widget text. `gui::theme::accent_pair` is the same table but
/// lives behind the `gui` feature, and the daemon must not depend on that; these
/// are its dark variants, which read best over video.
fn accent_hex(a: crate::config::Accent) -> &'static str {
    use crate::config::Accent;
    match a {
        Accent::Blue => "#5E6AD2",
        Accent::Teal => "#2BB6A2",
        Accent::Green => "#46B96B",
        Accent::Amber => "#DBA13C",
        Accent::Coral => "#F0708A",
        Accent::Graphite => "#98A1B0",
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_stat_ticks, stall_step, WlOutput, STALL_STRIKES};
    use crate::config::{PowerSaving, Scaling, Wallpaper};

    /// A still image holds `time-pos` at 0 forever (`image-display-duration=inf`),
    /// so the frozen-renderer detector must never strike it — that misread is
    /// what respawned image and slideshow wallpapers until the supervisor gave
    /// up on the output and reported `renderer_giveup`.
    #[test]
    fn a_held_frame_is_never_a_stall() {
        let mut strikes = 0;
        for _ in 0..STALL_STRIKES * 3 {
            strikes = stall_step(Some(0.0), Some(0.0), strikes, || true);
            assert_eq!(strikes, 0, "an image must not accumulate strikes");
        }
    }

    /// A video whose clock stops is a wedged renderer, and must still be caught.
    #[test]
    fn a_stopped_video_still_strikes_out() {
        let mut strikes = 0;
        for expected in 1..=STALL_STRIKES {
            strikes = stall_step(Some(12.5), Some(12.5), strikes, || false);
            assert_eq!(strikes, expected);
        }
        assert!(strikes >= STALL_STRIKES);
        // Progress clears the count, held frame or not.
        assert_eq!(stall_step(Some(12.5), Some(13.0), strikes, || false), 0);
        assert_eq!(stall_step(Some(12.5), Some(13.0), strikes, || true), 0);
    }

    /// A wallpaper with nothing behind it — an empty or unreadable slideshow
    /// folder, media that was deleted — is a configuration problem. Respawning
    /// cannot fix it, and counting the attempts as renderer failures spends the
    /// whole budget in ten seconds and then reports "renderer failed 5×",
    /// burying the real cause. Telemetry showed installs doing exactly that
    /// within ~10s of setting a slideshow.
    #[test]
    fn nothing_to_play_is_not_a_renderer_failure() {
        use crate::config::{Kind, PowerSaving, Scaling, Wallpaper};
        const MAX: u32 = 5;
        let mut o = super::WlOutput::new(
            "DP-1".into(),
            Wallpaper {
                kind: Kind::Slideshow, // no folder, no paths → nothing resolves
                ..Default::default()
            },
            Scaling::Balanced,
            PowerSaving::Full,
        );

        for _ in 0..MAX * 3 {
            o.supervise(false, MAX, true);
        }
        assert!(o.no_media, "the output must park on a media problem");
        assert_eq!(o.restarts, 0, "and never spend a restart on one");
        assert!(!o.static_fallback, "nor reach the give-up fallback");
        let err = o.error.clone().unwrap_or_default();
        assert!(
            err.contains("nothing to play"),
            "the status must name the real cause, got: {err}"
        );
    }

    /// A display that goes away — monitor asleep, DisplayPort link dropped — must
    /// not spend the restart budget: nothing can render to a connector the
    /// compositor no longer advertises, and those failures used to outlive the
    /// display's return as a permanent give-up on that output.
    ///
    /// Hermetic: parking and recovery both return before any spawn, and the
    /// exhausted budget is staged directly rather than by failing five times.
    #[test]
    fn a_vanished_display_does_not_burn_the_restart_budget() {
        use crate::config::{Kind, PowerSaving, Scaling, Wallpaper};
        const MAX: u32 = 5;
        let mut o = super::WlOutput::new(
            "DP-1".into(),
            Wallpaper {
                kind: Kind::Image,
                ..Default::default()
            },
            Scaling::Balanced,
            PowerSaving::Full,
        );

        // Away: every tick parks instead of restarting, however long it lasts.
        for _ in 0..MAX * 3 {
            o.supervise(false, MAX, false);
        }
        assert!(o.absent, "a missing display must park its output");
        assert_eq!(o.restarts, 0, "and must not count as a renderer failure");
        assert!(!o.static_fallback, "nor reach the give-up fallback");

        // Now stage an output that had already exhausted its budget and given
        // up — the state a sleeping monitor used to leave behind for good.
        o.restarts = MAX + 1;
        o.static_fallback = true;
        o.supervise(false, MAX, false);
        o.supervise(false, MAX, true);
        assert!(!o.absent, "the display's return unparks the output");
        assert_eq!(o.restarts, 0, "with a full budget");
        assert!(
            !o.static_fallback,
            "a give-up must never survive the display's return"
        );
    }

    /// An unreadable position is a failed IPC read, not evidence of a freeze.
    #[test]
    fn an_unreadable_position_holds_the_count() {
        assert_eq!(stall_step(Some(4.0), None, 2, || false), 2);
        assert_eq!(stall_step(None, None, 0, || false), 0);
        // First sample after a respawn: nothing to compare against yet.
        assert_eq!(stall_step(None, Some(0.0), 0, || false), 0);
    }

    /// Every respawn must be visible to the widget engine.
    ///
    /// A fresh mpv carries no overlays, so a healed renderer comes back blank
    /// unless something re-pushes. The loop watches the summed generation
    /// counter rather than being told by each heal path, because there are
    /// several of them (supervisor heal, static-frame fallback, output
    /// re-creation, apply) and threading a callback through each is how one
    /// gets missed. `respawn` is the counter's only writer, so a path added
    /// later is covered without being told to be.
    #[test]
    fn every_respawn_bumps_the_generation() {
        let mut o = WlOutput::new(
            "TEST-1".into(),
            Wallpaper::default(),
            Scaling::default(),
            PowerSaving::default(),
        );
        assert_eq!(o.generation, 0, "a fresh output has not respawned yet");

        // Spawning will fail here (no compositor in a unit test) — the counter
        // must still move, because a failed respawn also leaves no overlays.
        o.respawn(false, false);
        assert_eq!(o.generation, 1);
        o.respawn(true, true);
        assert_eq!(o.generation, 2, "the static-frame path counts too");
    }

    #[test]
    fn stat_ticks_survive_weird_comm() {
        // comm may contain spaces and parens; fields count after the LAST ')'.
        let stat = "1234 (my (weird) comm) S 1 1234 1234 0 -1 4194304 500 0 0 0 700 42 0 0 20 0 4 0 100 0 0";
        assert_eq!(parse_stat_ticks(stat), Some(742));
        assert_eq!(parse_stat_ticks(""), None);
        assert_eq!(parse_stat_ticks("no parens here"), None);
    }
}

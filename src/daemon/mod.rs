//! Fresco wallpaper daemon: owns X11 desktop windows and embedded mpv players,
//! reconciles them against the config, and serves IPC control commands.

mod control;
pub mod monitors;
pub mod mpv;
mod mpvpaper;
mod notifier;
mod overview;
mod wayland_outputs;
mod x11win;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Screen;
use x11rb::rust_connection::RustConnection;

use crate::config::{Config, Kind, Scaling, Transition, Wallpaper};
use crate::ipc::{Request, Response, StatusReply};

use monitors::Monitor;
use mpv::Player;
use mpvpaper::WaylandPlayer;
use x11win::{Atoms, WallpaperWindow};

const TICK: Duration = Duration::from_millis(100);
const LOWER_INTERVAL: Duration = Duration::from_secs(2);
const MONITOR_INTERVAL: Duration = Duration::from_secs(3);
const BATTERY_INTERVAL: Duration = Duration::from_secs(30);

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
    fn hwdec_current(&self) -> Option<String> {
        match self {
            PlayerHandle::X11(p) => p.hwdec_current(),
            PlayerHandle::Wayland(p) => p.hwdec_current(),
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
    monitors: Vec<Monitor>,
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
            monitors: Vec::new(),
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
                    if self.user_paused || self.battery_paused {
                        r.player.set_paused(true);
                    }
                    self.renderers.push(r);
                }
                Err(e) => log::error!("renderer for {} failed: {e}", monitor.connector),
            }
        }
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
        })
    }

    /// Main event loop. Returns when a Stop command (or signal) is received.
    pub fn run(&mut self) -> Result<()> {
        let commands = control::start_server()?;
        self.rebuild()?;
        overview::apply(&self.config.wallpaper);
        log::info!("frescod started with {} renderer(s)", self.renderers.len());

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
            let animating = self.advance_slideshows(now);

            std::thread::sleep(if animating { ANIM_TICK } else { TICK });
        }
    }

    fn handle_request(&mut self, req: Request) -> Response {
        match req {
            Request::Apply => {
                self.config = Config::load().unwrap_or_else(|_| self.config.clone());
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
                self.apply_pause();
                Response::Ok
            }
            Request::Resume => {
                self.user_paused = false;
                self.apply_pause();
                Response::Ok
            }
            Request::Status => Response::Status(self.status()),
        }
    }

    fn status(&self) -> StatusReply {
        let (cpu, rss) = proc_stats();
        let hwdec = self
            .renderers
            .first()
            .and_then(|r| r.player.hwdec_current());
        let error = self
            .renderers
            .iter()
            .find(|r| r.player.load_failed())
            .map(|r| format!("failed to load media on {}", r.window.connector));
        StatusReply {
            running: true,
            paused: self.user_paused || self.battery_paused,
            hwdec,
            wallpaper: self.describe_wallpaper(),
            cpu_percent: cpu,
            rss_mb: rss,
            monitors: self.monitors.iter().map(|m| m.connector.clone()).collect(),
            error,
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

    fn apply_pause(&self) {
        let paused = self.user_paused || self.battery_paused;
        for r in &self.renderers {
            r.player.set_paused(paused);
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
                self.apply_pause();
            }
            return;
        }
        let discharging = on_battery();
        if discharging != self.battery_paused {
            self.battery_paused = discharging;
            self.apply_pause();
            log::info!("battery pause = {discharging}");
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
                            player
                                .set_zoom_pan(s.base_zoom, s.base_pan_x, s.base_pan_y);
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
                    player.set_zoom_pan(
                        s.base_zoom + SLIDE_PUNCH * e,
                        s.base_pan_x,
                        s.base_pan_y,
                    );
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
                        player
                            .set_zoom_pan(s.base_zoom, s.base_pan_x, s.base_pan_y);
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
                        player
                            .set_zoom_pan(s.base_zoom, s.base_pan_x, s.base_pan_y);
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

/// (cpu_percent, rss_megabytes) from /proc/self. CPU is left at 0 for now
/// (a meaningful value needs sampling over time); RSS is the key lightweight
/// signal and is reported accurately.
fn proc_stats() -> (f32, u64) {
    let rss_mb = std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
        .and_then(|pages| pages.parse::<u64>().ok())
        .map(|pages| pages * 4096 / 1_048_576)
        .unwrap_or(0);
    (0.0, rss_mb)
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
    let (cpu, rss) = proc_stats();
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
    use std::collections::BTreeMap;
    use std::sync::mpsc::RecvTimeoutError;
    const MAX_RESTARTS: u32 = 5;
    const SUPERVISE: Duration = Duration::from_secs(2);
    const TICK: Duration = Duration::from_millis(100);
    const ANIM_TICK: Duration = Duration::from_millis(33);

    setup_vaapi_env();
    let mut config = Config::load().unwrap_or_default();
    if !config.enabled {
        log::info!("wallpaper disabled (enabled=false) — exiting");
        return Ok(());
    }

    let commands = control::start_server()?;

    // Enumerate outputs once (Phase 2 = static snapshot; live hotplug is Phase 3).
    let monitors = wayland_outputs::list_outputs().unwrap_or_else(|e| {
        log::warn!("output enumeration failed ({e:#}); targeting all outputs as one");
        vec![Monitor { connector: "ALL".into(), x: 0, y: 0, width: 0, height: 0 }]
    });
    log::info!(
        "Wayland outputs: [{}]",
        monitors.iter().map(|m| m.connector.as_str()).collect::<Vec<_>>().join(", ")
    );

    let mut user_paused = false;
    let mut battery_paused = false;
    let mut last_supervise = Instant::now() - SUPERVISE;

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
                        let paused = user_paused || battery_paused;
                        if config.enabled {
                            // Reconcile config × the (static) output set.
                            for m in &monitors {
                                let wp = config.wallpaper_for(&m.connector).clone();
                                let has = wp.effective_path().is_some()
                                    || !wp.paths.is_empty()
                                    || wp.kind == Kind::Slideshow;
                                match (outputs.get_mut(&m.connector), has) {
                                    (Some(o), true) => o.apply_wallpaper(wp, config.scaling, paused),
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
                        for o in outputs.values() {
                            o.set_paused(true);
                        }
                        Response::Ok
                    }
                    Request::Resume => {
                        user_paused = false;
                        for o in outputs.values() {
                            o.set_paused(battery_paused);
                        }
                        Response::Ok
                    }
                    Request::Status => {
                        Response::Status(wayland_status(&outputs, user_paused || battery_paused))
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

            if config.pause_on_battery {
                let discharging = on_battery();
                if discharging != battery_paused {
                    battery_paused = discharging;
                    let paused = user_paused || battery_paused;
                    for o in outputs.values() {
                        o.set_paused(paused);
                    }
                    log::info!("battery pause = {discharging}");
                }
            } else if battery_paused {
                battery_paused = false;
                for o in outputs.values() {
                    o.set_paused(user_paused);
                }
            }

            let paused = user_paused || battery_paused;
            for o in outputs.values_mut() {
                o.supervise(paused, MAX_RESTARTS);
            }
        }
    }

    outputs.clear(); // kill every mpvpaper before we exit
    std::fs::remove_file(crate::ipc::socket_path()).ok();
    log::info!("frescod stopped");
    Ok(())
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
        }
    }

    /// (Re)spawn the mpvpaper for this output. `paused` applies the current pause
    /// state; `static_frame` spawns then pauses (holds frame one) — the no-black
    /// per-output fallback when live playback keeps failing.
    fn respawn(&mut self, paused: bool, static_frame: bool) {
        drop(self.player.take());
        self.slideshow = None;
        self.animating = false;
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
            && new.mute == self.wallpaper.mute
            && new.volume == self.wallpaper.volume
            && new.crop == self.wallpaper.crop
            && new.kind == self.wallpaper.kind
            && new.kind != Kind::Slideshow;
        self.wallpaper = new;
        self.scaling = scaling;
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

    /// Per-output supervision: restart a dead renderer with an anti-flap cap;
    /// after `max` consecutive failures fall back to a paused static frame (so
    /// the output never goes black) and surface the error in `Status`.
    fn supervise(&mut self, paused: bool, max: u32) {
        let alive = self.player.as_ref().map(|p| p.is_alive()).unwrap_or(false);
        if alive {
            if !self.static_fallback {
                self.restarts = 0;
            }
            return;
        }
        // Renderer is dead or never started.
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
            self.respawn(true, true);
        }
        // restarts > max → given up; do nothing (anti-flap). Error stays in Status.
    }
}

/// Aggregate `Status` across all Wayland outputs for the GUI / diagnostics.
fn wayland_status(
    outputs: &std::collections::BTreeMap<String, WlOutput>,
    paused: bool,
) -> StatusReply {
    let (cpu, rss) = proc_stats();
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
    StatusReply {
        running: true,
        paused,
        hwdec,
        wallpaper,
        cpu_percent: cpu,
        rss_mb: rss,
        monitors: outputs.keys().cloned().collect(),
        error,
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

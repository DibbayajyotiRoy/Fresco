//! Fresco wallpaper daemon: owns X11 desktop windows and embedded mpv players,
//! reconciles them against the config, and serves IPC control commands.

mod caja_mirror;
pub mod cinnamon_bg;
mod control;
mod cosmic_bg;
mod dde;
mod dde_lock;
mod fullscreen;
mod kde_desktop;
mod lock;
mod signals;
// Public so the widget engine's API stays visible while the daemon-loop call
// sites are being built out; the module is otherwise internal.
#[allow(dead_code)]
pub mod lyrics_runtime;
pub mod monitors;
pub mod mpv;
mod mpvpaper;
mod notifier;
mod overview;
pub mod saver;
mod transition;
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

use crate::cli::which;
use crate::config::{Config, Kind, PowerSaving, Scaling, Transition, Wallpaper};
use crate::hwdecode::{self, install_hint, HwDecode, Pm};
use crate::ipc::{LockReply, LockSocket, LockStatus, MonitorInfo, Request, Response, StatusReply};

use lock::engine::LockEngine;
use lock::hosts::{HostCtx, HostKind, LockHost, LockTargets, RunningHost};
use lock::preview::PreviewRenderer;
use lock::state::LockMonitor;
use monitors::Monitor;
use mpv::Player;
use mpvpaper::WaylandPlayer;
use transition::{Anim, Step, Surface};
use x11win::{Atoms, WallpaperWindow, WindowKind};

const TICK: Duration = Duration::from_millis(100);

/// Floor on a widget-clamped wait, so a widget that reports itself permanently
/// overdue cannot turn a run loop into a spin. See [`widget_wait`].
const MIN_WIDGET_WAIT: Duration = Duration::from_millis(1);

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
// Startup renderer retry: when autostart launches us before RandR reports the
// monitors (or before the WM can take our window), the first rebuild comes up
// short. Keep rebuilding on this cadence for the first half-minute.
const STARTUP_RETRY_WINDOW: Duration = Duration::from_secs(30);
const STARTUP_RETRY_INTERVAL: Duration = Duration::from_secs(2);
// Wayland frozen-but-alive: consecutive SUPERVISE ticks (~2s each) with no
// playback progress before treating a still-running mpvpaper as wedged. 3 ≈ 6s,
// high enough that a normally looping clip never trips it.
const STALL_STRIKES: u32 = 3;

/// Best-effort human-readable message from a caught panic payload (as handed
/// back by `std::panic::catch_unwind`) — covers the two payload shapes `panic!`
/// actually produces (`&'static str` and `String`); anything else logs a
/// generic message rather than nothing at all.
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// Grace period for [`presentation_confirmed`]'s never-advances case: how long
/// a still image, a paused-at-spawn video, or the static-fallback frame gets
/// to actually render before we trust that its surface is mapped. There is no
/// IPC signal for "first frame rendered" on media that never advances its
/// clock, so this is a deliberately generous fixed wait rather than a poll.
const CONFIRM_GRACE: Duration = Duration::from_secs(6);
// Cross-monitor lockstep: the same video on two outputs plays on independent
// mpv clocks, and per-output pauses (fullscreen on one monitor, workspace
// switches) make them drift further apart forever. Periodically re-seat every
// follower on the leader's clock once the drift exceeds the tolerance.
const SYNC_INTERVAL: Duration = Duration::from_secs(5);
const SYNC_TOLERANCE: f64 = 0.2;

/// After a Wayland output gives up on live playback (falls back to a paused
/// static frame — see `WlOutput::supervise`), how long to wait before trying
/// live playback again. A transient cause (a driver update mid-session, a
/// brief compositor hiccup) must not be a permanent downgrade; five minutes
/// is long enough that a genuinely broken renderer doesn't retry-spam.
const RENDERER_REARM_DELAY: Duration = Duration::from_secs(5 * 60);

/// How often the long-running daemon loops re-offer a telemetry heartbeat.
/// `telemetry::heartbeat`/`minimal_heartbeat` self-throttle to roughly once a
/// day via their own marker file (`heartbeat_due`), so calling this often is
/// cheap and harmless — it exists so a daemon that runs for days without a
/// restart still checks in, instead of looking inactive to anyone watching
/// usage. Well under the marker's 20h window so a long session never misses a
/// day.
const HEARTBEAT_RECHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// During a transition the loop ticks at ~60fps for buttery, eased motion.
const ANIM_TICK: Duration = Duration::from_millis(16);

// ---------------------------------------------------------------------------
// Lock mode: shared wiring for all three run loops
// ---------------------------------------------------------------------------

/// The daemon's `daemon::lock` wiring: host detection, the lock-state
/// monitor, the lock widget engine, and the `Request::Lock*` handlers — one
/// instance per run loop ([`Daemon`], `run_gnome_static`,
/// `run_wayland_layershell`), driven through the exact same handful of
/// methods so the request-handling and transition logic can never drift
/// between backends the way `raise_demuxer_cache` once did between the X11
/// and Wayland players (`widgets.rs`'s own module doc names that failure).
///
/// What this type does *not* own: the desktop widget engine (each loop's
/// own) and the live player handles it must pause/dim on the `Desktop`
/// target — both differ enough per backend (`Vec<Renderer>` vs
/// `BTreeMap<String, WlOutput>` vs none at all in `run_gnome_static`) that
/// unifying them here would cost more than it saves. Each loop calls
/// [`LockRuntime::poll`] every tick and reacts to a transition with its own
/// glue; see `run_wayland_layershell` for the fullest example (COSMIC is the
/// one host with a `Desktop` target today).
struct LockRuntime {
    kind: HostKind,
    host: Box<dyn LockHost>,
    monitor: LockMonitor,
    /// `Some` for exactly as long as the session is believed locked *and*
    /// there is somewhere for it to draw (`LockTargets` other than `None`).
    engine: Option<LockEngine>,
    /// Set by a daemon-driven `Request::Lock` (`host.lock(ctx)` succeeding)
    /// and by `Request::LockNotify` reporting sockets; cleared on unlock or
    /// when its `child` (if any) exits. `RunningHost::targets` — when it is
    /// not `LockTargets::None` — takes priority over
    /// `LockHost::targets_while_locked` the moment either is known, per the
    /// contract's own `targets_while_locked` doc comment ("where the lock
    /// widget engine draws while the session is locked *by the DE itself*
    /// (not by `LockHost::lock`)").
    running: Option<RunningHost>,
    preview: PreviewRenderer,
    /// Mirrors `self.monitor.current().is_locked()` — kept alongside it
    /// rather than recomputed every time so [`LockRuntime::poll`] can tell
    /// "genuinely flipped" from "moved between `Locked` and
    /// `SleepImminent`, both already `is_locked()`", which must not
    /// re-trigger `begin_lock`/`end_lock`.
    locked: bool,
    /// Set by [`LockRuntime::begin_lock`]/cleared by
    /// [`LockRuntime::end_lock`]: whether the *current* lock targets the live
    /// desktop wallpaper surface (COSMIC only). The Wayland loop's live-video/
    /// dim policy (`docs/plan-lock-screen.md` §3.1) applies only then — a
    /// `Sockets`/`LayerFiles` target has no bearing on the desktop's own
    /// mpvpaper properties at all.
    desktop_target: bool,
}

impl LockRuntime {
    fn new() -> Self {
        let kind = lock::hosts::detect();
        let host = lock::hosts::host_for(kind);
        // Read back through the trait rather than trusting `kind` verbatim:
        // `host_for` promises `host_for(k).kind() == k` (see its own tests),
        // and asking confirms `self.kind` always agrees with the adapter
        // actually driving `self.host`, not just with what `detect` returned
        // a moment before `host_for` ran.
        let kind = host.kind();
        LockRuntime {
            kind,
            host,
            monitor: LockMonitor::new(kind, Instant::now()),
            engine: None,
            running: None,
            preview: PreviewRenderer::new(),
            locked: false,
            desktop_target: false,
        }
    }

    /// `Request::Apply`: re-run host detection (cheap — see `hosts::detect`)
    /// and rebuild the host adapter if it changed. The state monitor is left
    /// running under its existing `HostKind` rather than rebuilt — throwing
    /// away its debounce state and `gdbus` children on every Apply would
    /// cost real latency for no benefit, since `hosts::classify` only reads
    /// things (`XDG_SESSION_TYPE`, the Wayland globals) that do not change
    /// while a session runs.
    fn on_apply(&mut self, config: &Config) {
        let kind = lock::hosts::detect();
        if kind != self.kind {
            log::info!("lock: host changed {:?} -> {kind:?}", self.kind);
            self.host = lock::hosts::host_for(kind);
            self.kind = self.host.kind();
        }
        if matches!(self.kind, HostKind::Kde) {
            // Keep the KDE greeter plugin's config (wallpaper paths, live/still,
            // dim) in step with the GUI's Apply. `refresh_config` is a no-op
            // until the user has run setup, and it touches no output geometry,
            // so an empty output list is fine. Best-effort by design: a failed
            // kwriteconfig6 must never turn a good Apply into an error reply.
            let ctx = self.ctx(config, &[]);
            if let Err(e) = lock::hosts::kde::refresh_config(&ctx) {
                log::warn!("lock: kde config refresh failed: {e}");
            }
        }
    }

    fn ctx<'a>(&self, config: &'a Config, outputs: &'a [widgets::OutputGeom]) -> HostCtx<'a> {
        HostCtx {
            config,
            outputs,
            runtime_dir: crate::ipc::socket_dir(),
        }
    }

    /// The resolved lock config, or `None` when the feature is off —
    /// `[lockscreen].enabled = false` is the default, and every call site
    /// below treats "nothing to do" and "disabled" the same way.
    fn resolved(config: &Config) -> Option<crate::lockscreen::ResolvedLock> {
        let cfg = config.lockscreen.as_ref()?;
        cfg.enabled.then(|| crate::lockscreen::resolve(cfg))
    }

    /// Where the lock engine should draw right now: a daemon-spawned host's
    /// own targets when it has any, else the host's own
    /// `targets_while_locked`. See [`LockRuntime::running`]'s doc comment.
    fn targets_now(&self, ctx: &HostCtx) -> LockTargets {
        if let Some(running) = &self.running {
            if running.targets != LockTargets::None {
                return running.targets.clone();
            }
        }
        self.host.targets_while_locked(ctx)
    }

    /// Poll every source `daemon::lock::state` owns directly, plus the one
    /// this type alone can see (a daemon-spawned host's child exiting).
    /// Returns `Some(is_locked)` only on a genuine is-locked transition —
    /// moving between `Locked` and `SleepImminent` (both already
    /// `is_locked()`) is not one, so a caller reacting to `Some` never
    /// double-runs its swap.
    fn poll(&mut self, now: Instant) -> Option<bool> {
        let mut child_exited = false;
        if let Some(running) = &mut self.running {
            if let Some(child) = &mut running.child {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    child_exited = true;
                }
            }
        }
        if child_exited {
            self.running = None;
            self.monitor.on_host_child_exited(now);
        }
        if self
            .engine
            .as_ref()
            .is_some_and(LockEngine::all_sockets_disconnected)
        {
            self.monitor.on_sockets_disconnected(now);
        }
        self.monitor.poll(now);
        let now_locked = self.monitor.current().is_locked();
        if now_locked != self.locked {
            self.locked = now_locked;
            return Some(now_locked);
        }
        None
    }

    /// Start the lock widget engine for a transition into locked mode. The
    /// caller is responsible for the parts this type doesn't own: clearing
    /// the desktop widget engine (`widgets::WidgetEngine::clear_for_lock`)
    /// and, on the `Desktop` target only, the live-video/dim policy on the
    /// player handle.
    fn begin_lock(
        &mut self,
        resolved: &crate::lockscreen::ResolvedLock,
        ctx: &HostCtx,
        theme: crate::widgetkit::Theme,
    ) -> LockTargets {
        let targets = self.targets_now(ctx);
        self.desktop_target = targets == LockTargets::Desktop;
        let mut engine = LockEngine::new(self.kind, resolved, ctx.outputs, theme);
        engine.set_targets(targets.clone());
        self.engine = Some(engine);
        targets
    }

    /// End a locked session: take the engine's own overlays down (for the
    /// `Desktop` target — `Sockets`/`LayerFiles` clean up their own state
    /// inside `LockEngine::clear`) and forget the daemon-spawned host, if
    /// there was one.
    fn end_lock(&mut self) -> Vec<widgets::WidgetUpdate> {
        self.running = None;
        self.desktop_target = false;
        match self.engine.take() {
            Some(mut e) => e.clear(),
            None => Vec::new(),
        }
    }

    // -- Request handling, identical across all three run loops -------------

    /// `Request::Lock`.
    fn lock(&mut self, ctx: &HostCtx) -> Response {
        match self.host.lock(ctx) {
            Ok(running) => {
                self.running = Some(running);
                Response::Lock(LockReply {
                    host: self.kind.id().to_string(),
                    ok: true,
                    message: None,
                })
            }
            Err(e) => {
                // Fail closed even when this host's own adapter can't:
                // `loginctl lock-session` is the universal fallback every
                // `LockHost` in this snapshot already routes through, so a
                // host whose own chain fails (or isn't implemented yet — the
                // wlroots/X11 hosts today) still tries the one mechanism that
                // works everywhere before reporting failure.
                let ok = lock::hosts::loginctl_lock_session().is_ok();
                Response::Lock(LockReply {
                    host: self.kind.id().to_string(),
                    ok,
                    message: Some(e),
                })
            }
        }
    }

    /// `Request::LockNotify`.
    fn lock_notify(&mut self, locked: bool, sockets: Vec<LockSocket>, now: Instant) {
        // Any same-user process can send this, and we connect to whatever
        // paths it names — filter first (see `lock::notify`).
        let sockets = lock::notify::validate_sockets(
            sockets,
            &crate::ipc::socket_dir(),
            &lock::notify::dir_is_symlink_or_unreadable,
        );
        if !sockets.is_empty() {
            let targets = LockTargets::Sockets(sockets);
            match &mut self.running {
                Some(running) => running.targets = targets,
                None => {
                    self.running = Some(RunningHost {
                        child: None,
                        targets,
                    })
                }
            }
        }
        self.monitor.on_lock_notify(locked, now);
    }

    /// `Request::LockSetup`.
    fn lock_setup(&self, ctx: &HostCtx) -> Response {
        match self.host.setup(ctx) {
            Ok(()) => Response::Ok,
            Err(message) => Response::Err { message },
        }
    }

    /// `Request::LockUndo`.
    fn lock_undo(&self, ctx: &HostCtx) -> Response {
        match self.host.undo(ctx) {
            Ok(()) => Response::Ok,
            Err(message) => Response::Err { message },
        }
    }

    /// `Request::LockPreview`.
    fn lock_preview(
        &mut self,
        ctx: &HostCtx,
        np: Option<&widgets::Snapshot>,
        theme: crate::widgetkit::Theme,
        width: u32,
        height: u32,
    ) -> Response {
        let Some(resolved) = Self::resolved(ctx.config) else {
            return Response::Err {
                message: "lock screen is not enabled".to_string(),
            };
        };
        let wallpaper = ctx.config.lock_source(None);
        self.preview
            .set_monitors(ctx.outputs.iter().map(|o| o.connector.clone()));
        match self
            .preview
            .render(self.kind, wallpaper, &resolved, np, theme, width, height)
        {
            Ok(path) => Response::LockPreview {
                path: path.to_string_lossy().into_owned(),
            },
            Err(message) => Response::Err { message },
        }
    }

    /// `StatusReply.lockscreen`.
    fn status(&self, ctx: &HostCtx, config: &Config) -> LockStatus {
        // Live video and Fresco's own widgets are available only where a
        // real surface exists to draw them on today; GNOME/Cinnamon/Deepin
        // (and COSMIC without show-on-lock) get a still frame at most. The
        // matrix lives in `lock::hosts::capabilities`, whose still-frame
        // column is `HostKind::shows_still_frame`.
        let caps = lock::hosts::capabilities(self.kind);
        LockStatus {
            enabled: config.lockscreen.as_ref().is_some_and(|l| l.enabled),
            host: self.kind.id().to_string(),
            live_video: caps.live_video,
            widgets: caps.widgets,
            still_frame: Some(caps.still_frame),
            locked: self.locked,
            setup: self.host.setup_state(ctx),
            notes: self.host.notes(ctx),
        }
    }
}

/// A slideshow's dwell bookkeeping: which image is up and when the next one is
/// due. The animation between them belongs to [`transition::Anim`], which any
/// wallpaper change can drive — a slideshow is one of its callers, not its
/// owner.
struct Slideshow {
    images: Vec<PathBuf>,
    idx: usize,
    interval: Duration,
    last_advance: Instant,
    transition: Transition,
}

struct Renderer {
    window: WallpaperWindow,
    player: PlayerHandle,
    slideshow: Option<Slideshow>,
    /// This output's transition. Independent per renderer: two monitors run
    /// their own players and may be mid-transition at different phases.
    anim: Anim,
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

impl Renderer {
    /// One animation tick for this output. Returns true while animating.
    fn advance(&mut self, now: Instant) -> bool {
        match self.slideshow.as_mut() {
            Some(s) => advance_slideshow(&self.player, s, &mut self.anim, now),
            // No slideshow: the only thing that can be running is a wallpaper
            // change (a scheduled swap), which steps identically.
            None => self.anim.step(&self.player).animating(),
        }
    }
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
    /// Defocus for a transition; `0.0` clears it. See
    /// [`crate::daemon::mpvpaper::WaylandPlayer::set_blur`].
    fn set_blur(&self, sigma: f64) {
        match self {
            PlayerHandle::X11(p) => p.set_blur(sigma),
            PlayerHandle::Wayland(p) => p.set_blur(sigma),
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
    /// Lock-screen dim (`LockRuntime`'s `Desktop`-target policy in
    /// `run_wayland_layershell` — see `docs/plan-lock-screen.md` §3.1's
    /// `dim`). Wayland only: COSMIC, the one host whose lock targets are ever
    /// `Desktop`, always runs on this backend. Mirrors
    /// `raise_demuxer_cache`'s own asymmetric-by-design shape (that one is
    /// X11-only; this one is Wayland-only), not a widget that silently no-ops
    /// on one backend the way that method's own doc comment warns against —
    /// there is nothing for this to do on X11 because nothing on X11 ever
    /// requests it.
    fn set_brightness(&self, brightness: i32) {
        if let PlayerHandle::Wayland(p) = self {
            p.set_brightness(brightness);
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

    /// The renderer's last stderr lines, where there is a separate process
    /// whose output could be captured. Logged when it dies so "renderer down
    /// (dead)" says what mpv or the driver printed on the way out.
    fn stderr_tail(&self) -> Option<String> {
        match self {
            PlayerHandle::X11(_) => None,
            PlayerHandle::Wayland(p) => Some(p.stderr_tail()),
        }
    }

    /// The content-free `exit=`/`sig=` fingerprint for a Wayland renderer that
    /// has just been found dead (call only once `is_alive` has said so — see
    /// [`crate::daemon::mpvpaper::WaylandPlayer::runtime_exit_detail`]).
    /// Always `None` on X11, which never exits out from under the daemon.
    fn runtime_exit_detail(&self) -> Option<crate::daemon::mpvpaper::ExitDetail> {
        match self {
            PlayerHandle::X11(_) => None,
            PlayerHandle::Wayland(p) => p.runtime_exit_detail(),
        }
    }

    /// Renderer pid, where there is a separate process to have one.
    ///
    /// `None` on X11, where mpv is embedded rather than spawned as a paper
    /// process — which also means there is no buffer negotiation to go stale,
    /// so nothing there needs a pid to key a cache on.
    fn pid(&self) -> Option<u32> {
        match self {
            PlayerHandle::X11(_) => None,
            PlayerHandle::Wayland(p) => Some(p.pid()),
        }
    }

    /// The space `overlay-add` places into, when it can differ from the mode.
    ///
    /// `None` on X11: the mpv window is sized in root-window pixels there, so
    /// the output mode already *is* that space and there is nothing to correct.
    /// See [`crate::daemon::mpvpaper::WaylandPlayer::osd_size`] for why Wayland
    /// is different.
    fn osd_size(&self) -> Option<(u32, u32)> {
        match self {
            PlayerHandle::X11(_) => None,
            PlayerHandle::Wayland(p) => p.osd_size(),
        }
    }
}

/// The transition engine's view of a player. Both backends reach it through the
/// same `PlayerHandle`, so an effect cannot behave differently on X11 and
/// Wayland — and the engine itself stays testable against a fake.
impl Surface for PlayerHandle {
    fn load(&self, path: &std::path::Path) {
        self.load_path(path);
    }
    fn gamma(&self, gamma: i32) {
        self.set_gamma(gamma);
    }
    fn zoom_pan(&self, zoom: f64, pan_x: f64, pan_y: f64) {
        self.set_zoom_pan(zoom, pan_x, pan_y);
    }
    fn blur(&self, sigma: f64) {
        self.set_blur(sigma);
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
    /// Connectors currently covered by a viewable fullscreen (or, with
    /// `pause_on_maximized`, maximized) window (EWMH), with a "kind window
    /// (title)" description for the log.
    fullscreen_covered: std::collections::HashMap<String, String>,
    last_fullscreen_check: Instant,
    sched: SchedState,
    monitors: Vec<Monitor>,
    started_at: Instant,
    last_heal_check: Instant,
    heals: u32,
    last_startup_retry: Instant,
    /// Rebuilds attempted by `check_startup_renderers`; 0 = never needed.
    startup_retries: u32,
    /// Deepin DDE quirk (issue #2): whether/how DDE's covering desktop window
    /// is being handled. `Inactive` on every other desktop.
    dde_mode: dde::Mode,
    /// True once the one-time DDE render self-check has run (it blocks ~1s).
    dde_self_checked: bool,
    /// Restack mode only: lets the desktop icons stay up for a few seconds
    /// after the user clicks the desktop, instead of burying them again on the
    /// next stacking pass.
    dde_peek: dde::IconPeek,
    /// MATE (issue #18) [`dde::Mode::CajaMirror`], Deepin (issue #33)
    /// [`dde::Mode::DdeMirror`] and Xfce [`dde::Mode::XfceMirror`] only: the
    /// thread that copies the desktop's icons onto the wallpaper windows (and,
    /// on MATE and Deepin, keeps the desktop window below them). `None` in
    /// every other mode, and before the first rebuild.
    caja_mirror: Option<caja_mirror::Mirror>,
    /// Set once the icon mirror has failed (a desktop window it cannot copy,
    /// a background it cannot set). Every later rebuild then stays in restack mode
    /// instead of re-trying — each retry would repaint the user's desktop in
    /// the key colour and back again just to fail the same way.
    caja_mirror_gave_up: bool,
    /// On-wallpaper widgets (lyrics, clock). Owns its own worker thread and
    /// hands back only overlays whose content actually changed, so an idle
    /// desktop costs nothing (see docs/WIDGETS_ROADMAP.md "Power model").
    widgets: widgets::WidgetEngine,
    /// Set by `handle_request(Apply)` instead of calling `overview::apply`
    /// inline: that call decodes a full-size frame (ffmpegthumbnailer, `-s
    /// 0`) and writes it via gsettings, which is slow enough on weak hardware
    /// to make the GUI's IPC round trip (and thus its "Applying…" toast) last
    /// much longer than it needs to. Deferring it to right after the reply is
    /// sent (see `run`) means the caller — off the GTK main thread already,
    /// see `daemon_ctl::apply_async` — gets its `Ok` back as soon as the
    /// renderers are rebuilt, and the overview frame catches up a moment
    /// later without anyone waiting on it.
    overview_pending: bool,
    /// Last time this loop re-offered a telemetry heartbeat — see
    /// [`HEARTBEAT_RECHECK_INTERVAL`]. The heartbeat itself self-throttles to
    /// roughly daily, so this only needs to be "often enough", not precise.
    last_heartbeat_check: Instant,
    /// Lock-screen host detection, state monitor and widget engine — see
    /// [`LockRuntime`]. X11 has no `Desktop` target of its own (no host in
    /// `docs/plan-lock-screen.md`'s table ever names one on X11), so on this
    /// backend a lock only ever pauses the desktop player for power and
    /// answers `Request::Lock*`/`Status`; see [`Daemon::reconcile_lock`].
    lock: LockRuntime,
    /// mpv `pause` this backend applied for the lock screen's power policy,
    /// remembered so unlock restores exactly what the user had — never
    /// un-pausing a video the user paused themself. `None` = not currently
    /// applied.
    lock_paused: Option<bool>,
}

impl Daemon {
    pub fn new(config: Config) -> Result<Daemon> {
        let (conn, screen_num) =
            x11rb::connect(None).context("connecting to X11 (is DISPLAY set?)")?;
        let atoms = Atoms::new(&conn)?.reply()?;
        let mut widgets = widgets::WidgetEngine::new(config.widgets.as_ref(), config.accent);
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
            last_startup_retry: Instant::now(),
            startup_retries: 0,
            dde_mode: dde::Mode::Inactive,
            dde_self_checked: false,
            dde_peek: dde::IconPeek::default(),
            caja_mirror: None,
            caja_mirror_gave_up: false,
            widgets,
            overview_pending: false,
            // Due immediately at startup would just repeat `run()`'s own
            // heartbeat call a moment later; start the clock instead.
            last_heartbeat_check: Instant::now(),
            lock: LockRuntime::new(),
            lock_paused: None,
        })
    }

    /// Reconcile the lock-screen state on every tick: X11 has no `Desktop`
    /// target (see [`Daemon::lock`]'s doc comment), so the whole of this
    /// backend's reaction to a lock/unlock transition is pausing the desktop
    /// player for power while locked and resuming it on unlock — "unless a
    /// target draws into it", which on X11 never happens today, but the
    /// check is here rather than assumed so a future X11 `Desktop` target
    /// would not silently get paused out from under it.
    fn reconcile_lock(&mut self, now: Instant) {
        let Some(now_locked) = self.lock.poll(now) else {
            return;
        };
        if now_locked {
            let Some(resolved) = LockRuntime::resolved(&self.config) else {
                return;
            };
            let cleared = self.widgets.clear_for_lock(
                resolved
                    .widgets
                    .contains(&crate::lockscreen::LockWidget::Lyrics),
                resolved
                    .widgets
                    .contains(&crate::lockscreen::LockWidget::Visualizer),
            );
            self.dispatch_widget_updates(cleared);
            let geoms = self.output_geoms_all();
            let ctx = self.lock.ctx(&self.config, &geoms);
            let theme = lock_widget_theme(&self.config);
            // X11 never has a `Desktop` target of its own (no host ever
            // names one there), but `begin_lock` still starts the engine for
            // `Sockets`/`LayerFiles` — an X11 WM saver or the MATE/Xfce
            // screensaver theme, once wave 2b's hosts land, still needs
            // widgets pushed somewhere.
            self.widgets.set_lock_album_art(
                resolved
                    .widgets
                    .contains(&crate::lockscreen::LockWidget::AlbumArt),
            );
            let targets = self.lock.begin_lock(&resolved, &ctx, theme);
            if targets != lock::hosts::LockTargets::Desktop {
                self.lock_paused = Some(self.user_paused || self.battery_paused);
                for r in &self.renderers {
                    r.player.set_paused(true);
                }
            }
        } else {
            let cleared = self.lock.end_lock();
            self.dispatch_widget_updates(cleared);
            self.widgets.set_lock_album_art(false);
            if let Some(prev) = self.lock_paused.take() {
                for r in &self.renderers {
                    r.player.set_paused(prev);
                }
            }
            self.widgets.invalidate();
        }
    }

    /// Advance the lock engine while locked — the `Sockets`/`LayerFiles`
    /// targets do their own I/O and return nothing; `Desktop` never occurs on
    /// X11 (see [`Daemon::reconcile_lock`]), so there is nothing to dispatch
    /// here today, but the tick still has to happen for those two targets'
    /// content (clock, battery, media) to ever update while locked.
    fn tick_lock_engine(&mut self) {
        let geoms = self.output_geoms_all();
        let np = self.widgets.now_playing();
        let Some(engine) = &mut self.lock.engine else {
            return;
        };
        engine.set_outputs(&geoms);
        let updates = engine.tick(np.as_ref());
        self.dispatch_widget_updates(updates);
    }

    /// Dispatch a batch of lock-engine `WidgetUpdate`s through the same
    /// per-output routing `push_widgets`/`clear_widgets` already use.
    fn dispatch_widget_updates(&self, updates: Vec<widgets::WidgetUpdate>) {
        for u in &updates {
            for r in &self.renderers {
                if u.is_for(&r.window.connector) {
                    dispatch_widget(&r.player, u);
                }
            }
        }
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
        // A widget belongs to one display: the configured connector, else the
        // first renderer. Pushing to every renderer would duplicate the lyric
        // across monitors, which reads as a bug rather than a feature.
        // No configured connector = every display, matching the wallpaper
        // itself: the widget is part of the wallpaper, so a two-monitor desktop
        // showing it on one screen reads as half-broken. Naming a connector in
        // `widgets.monitor` narrows it to that one.
        let want = self.widgets.monitor().map(str::to_string);
        let targets: Vec<String> = self
            .renderers
            .iter()
            .map(|r| r.window.connector.clone())
            .filter(|c| want.as_deref().is_none_or(|w| w == c.as_str()))
            .collect();
        if targets.is_empty() {
            return;
        }
        // Geometry **before** the tick, not after: a bitmap widget computes its
        // size and position during `tick`, so a mode change told to the engine
        // afterwards would place the first frame after the change against the
        // old resolution.
        self.widgets.set_outputs(&self.output_geoms(&targets));
        let updates = self.widgets.tick();
        if updates.is_empty() {
            return;
        }
        for u in updates {
            log::debug!(
                "widget: overlay {} -> {} chars, target {:?}",
                u.overlay_id,
                u.ass.len(),
                u.target.as_deref().unwrap_or("all")
            );
            for r in &self.renderers {
                let c = &r.window.connector;
                // A covered output shows nothing, so don't make mpv repaint it
                // (clears still go through, so nothing stale survives);
                // `check_fullscreen` re-pushes everything when it's uncovered.
                if targets.iter().any(|t| t == c)
                    && u.is_for(c)
                    && (u.is_clear() || !self.fullscreen_covered.contains_key(c))
                {
                    dispatch_widget(&r.player, &u);
                }
            }
        }
    }

    /// The real pixel mode of each target, in target order.
    ///
    /// A connector RandR has not reported falls back to the first known mode
    /// (else 1080p), which is what the engine assumed before it knew about more
    /// than one output — a widget in roughly the right place beats no widget.
    fn output_geoms(&self, targets: &[String]) -> Vec<widgets::OutputGeom> {
        let fallback = self
            .monitors
            .first()
            .map_or((1920, 1080), |m| (u32::from(m.width), u32::from(m.height)));
        targets
            .iter()
            .map(|c| {
                let (w, h) = self
                    .monitors
                    .iter()
                    .find(|m| &m.connector == c)
                    .map_or(fallback, |m| (u32::from(m.width), u32::from(m.height)));
                widgets::OutputGeom {
                    connector: c.clone(),
                    w,
                    h,
                    scale_milli: 1000,
                }
            })
            .collect()
    }

    /// Blank every widget overlay. Called before a rebuild/teardown so an
    /// overlay can never survive onto the next wallpaper — the leak class the
    /// scheduled-swap comments warn about.
    fn clear_widgets(&mut self) {
        for u in self.widgets.clear_all() {
            for r in &self.renderers {
                if u.is_for(&r.window.connector) {
                    dispatch_widget(&r.player, &u);
                }
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

        // KDE Plasma (issue #44): plasmashell's opaque desktop window covers
        // any window of ours (or hides the icons if we sit above it), so the
        // wallpaper is applied through plasmashell instead — see `kde_desktop`.
        if kde_desktop::enabled() {
            return Ok(());
        }

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
            if slideshow_has_no_images(&wallpaper) {
                log::warn!(
                    "[{}] slideshow has no images to show; leaving the native wallpaper",
                    monitor.connector
                );
                continue;
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
        // ours. Make DDE's wallpaper transparent (or restack above it). MATE
        // (issue #18): Caja's desktop window does the same, and is restacked.
        // Xfce: xfdesktop's windows sit in a layer below ours instead, and
        // only their icons need bringing over.
        if (crate::capability::is_deepin_dde()
            || crate::capability::is_mate()
            || crate::capability::is_xfce())
            && !self.renderers.is_empty()
        {
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
        // Cinnamon (issue #39): muffin's compositor keeps painting a wallpaper
        // mapped after nemo-desktop over the icons, whatever the window stack
        // says, until a normal window appears. Make it re-sort now.
        if crate::capability::is_cinnamon() && !self.renderers.is_empty() {
            match x11win::force_compositor_restack(&self.conn, &screen, &self.atoms) {
                Ok(()) => log::info!(
                    "cinnamon: forced compositor restack; wallpaper windows {:x?}, \
                     window stack (bottom first) {:x?}",
                    self.renderers
                        .iter()
                        .map(|r| r.window.window)
                        .collect::<Vec<_>>(),
                    x11win::stacking_order(&self.conn, &self.atoms, screen.root)
                ),
                Err(e) => log::warn!("cinnamon: compositor restack helper failed: {e:#}"),
            }
        }
        self.sync_caja_mirror();
        Ok(())
    }

    /// MATE (issue #18): bring the Caja icon mirror in line with `dde_mode`
    /// and the freshly built renderers. Runs after every rebuild.
    ///
    /// Outside [`dde::Mode::CajaMirror`] (and with no renderers, where there is
    /// nothing to draw the icons on) any running mirror is stopped and the
    /// user's MATE (or Xfce) background put back. That restore is a no-op when
    /// nothing was saved, so it is also what cleans up the key colour a
    /// crashed run left behind when this run does not mirror.
    fn sync_caja_mirror(&mut self) {
        if let Some(d) = self
            .dde_mode
            .mirror_desktop()
            .filter(|_| self.caja_mirror_gave_up)
        {
            // `dde::apply` re-offers the mirror on every rebuild; it already
            // failed once in this session, and nothing about it has changed.
            // The windows are raised above the desktop already (restack is the
            // first half of the mirror mode), so restack just takes over.
            self.dde_mode = mode_after_mirror_gave_up(d);
        }
        let Some(desktop) = self
            .dde_mode
            .mirror_desktop()
            .filter(|_| !self.renderers.is_empty())
        else {
            if let Some(m) = self.caja_mirror.take() {
                let d = m.desktop();
                m.stop(&self.conn);
                // The mirror on Deepin repainted the DDE wallpaper; give it back.
                caja_mirror::restore_key_background(d);
            }
            caja_mirror::restore_background();
            return;
        };
        // Each wallpaper window with its monitor's rectangle, in root
        // coordinates: that is where the desktop's pixels for it come from.
        let parents: Vec<caja_mirror::Parent> = self
            .renderers
            .iter()
            .filter_map(|r| {
                let m = self
                    .monitors
                    .iter()
                    .find(|m| m.connector == r.window.connector)?;
                Some(caja_mirror::Parent {
                    window: r.window.window,
                    x: m.x,
                    y: m.y,
                    width: m.width,
                    height: m.height,
                })
            })
            .collect();
        if self.caja_mirror.is_none() {
            if desktop == caja_mirror::Desktop::Caja {
                // The restack fallback, or a previous run, may have put our
                // still frame into `org.mate.background`. Put the user's own
                // picture back first, so the "original" `apply_key` saves is
                // theirs and not a Fresco frame that Stop would then leave
                // behind. (Deepin has no still-frame writer.)
                overview::restore();
            }
            cosmic_bg::restore();
            let connectors: Vec<String> =
                self.monitors.iter().map(|m| m.connector.clone()).collect();
            if !caja_mirror::apply_key(desktop, &connectors) {
                self.fall_back_to_restack(
                    desktop,
                    "the desktop background cannot be changed to the key colour",
                );
                return;
            }
            match caja_mirror::Mirror::start(desktop) {
                Ok(m) => self.caja_mirror = Some(m),
                Err(e) => {
                    self.fall_back_to_restack(desktop, &format!("{e:#}"));
                    return;
                }
            }
        }
        if let Some(m) = &self.caja_mirror {
            m.set_parents(&self.conn, parents);
        }
    }

    /// The icon mirror cannot run: stop it, give the desktop the user's
    /// background back, and hide the icons behind the wallpaper the verified
    /// way, [`dde::Mode::Restack`], where a click on the desktop peeks at them.
    /// (Xfce has no peek: its icons stay hidden, and the stacking is left as
    /// it always was — see [`mode_after_mirror_gave_up`].)
    /// No retry: `caja_mirror_gave_up` keeps every later rebuild in restack.
    fn fall_back_to_restack(&mut self, desktop: caja_mirror::Desktop, reason: &str) {
        let name = match desktop {
            caja_mirror::Desktop::Caja => "MATE",
            caja_mirror::Desktop::Dde => "DDE",
            caja_mirror::Desktop::Xfce => "Xfce",
        };
        let peek = if desktop == caja_mirror::Desktop::Xfce {
            ""
        } else {
            " — clicking the desktop brings them back for `dde_icon_peek_secs` seconds"
        };
        log::warn!(
            "{name}: cannot draw the desktop icons over the wallpaper ({reason}); they are \
             hidden while it plays{peek}"
        );
        if let Some(m) = self.caja_mirror.take() {
            m.stop(&self.conn);
        }
        self.caja_mirror_gave_up = true;
        self.dde_mode = mode_after_mirror_gave_up(desktop);
        caja_mirror::restore_key_background(desktop);
        if desktop == caja_mirror::Desktop::Caja {
            // Caja shows the still frame during every peek.
            overview::apply(&self.config.wallpaper);
        }
        cosmic_bg::apply(&self.config);
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
            anim: Anim::new(transition::crop_base(wallpaper)),
            last_time_pos: std::cell::Cell::new(None),
            audio_heal: AudioHeal::new(),
            cache_raised: std::cell::Cell::new(false),
            applied_paused: std::cell::Cell::new(false),
        })
    }

    /// Main event loop. Returns when a Stop command (or signal) is received.
    pub fn run(&mut self) -> Result<()> {
        let commands = control::start_server()?;
        // Not fatal: right after login RandR can still be unavailable, and
        // `check_startup_renderers` retries for the first half-minute.
        if let Err(e) = self.rebuild() {
            log::warn!("initial renderer build failed: {e:#}");
        }
        overview::apply(&self.config.wallpaper);
        cosmic_bg::apply(&self.config);
        dde_lock::apply(&self.config);
        kde_desktop::apply(&self.config);
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
                // Deferred from `handle_request(Apply)`: the caller already
                // has its reply (and, off the GTK thread, has already
                // unblocked the GUI), so the slow full-size overview redecode
                // happens here instead of before the reply went out.
                if std::mem::take(&mut self.overview_pending) {
                    overview::apply(&self.config.wallpaper);
                    cosmic_bg::apply(&self.config);
                    dde_lock::apply(&self.config);
                    kde_desktop::apply(&self.config);
                }
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
            if now.duration_since(self.last_heartbeat_check) >= HEARTBEAT_RECHECK_INTERVAL {
                self.last_heartbeat_check = now;
                // Same arguments as the startup call below; `heartbeat`
                // itself throttles to roughly once a day via its marker
                // file, so a daemon that runs for days without a restart
                // still checks in instead of going quiet after the first one.
                crate::telemetry::heartbeat(
                    Some("x11"),
                    self.renderers
                        .first()
                        .and_then(|r| r.player.hwdec_current())
                        .as_deref(),
                    Some(self.renderers.len() as u32),
                );
            }
            self.check_startup_renderers(now);
            self.check_cold_boot_stall(now);
            self.reconcile_lock(now);
            // While locked, the desktop widget engine is frozen rather than
            // ticked: `clear_for_lock` may have left the clock/disc believing
            // they are "due" (cleared, not merely hidden), and calling
            // `push_widgets` here would immediately redraw and re-push them
            // onto the very surface the lock screen is supposed to own —
            // undoing the clear on the next tick. Lyrics/visualiser
            // overlays kept in place (see `clear_for_lock`'s caller) are
            // therefore a frozen last frame while locked, not a live feed;
            // `reconcile_lock`'s unlock path calls `invalidate()` to bring
            // everything back the moment it's safe to.
            if self.lock.locked {
                self.tick_lock_engine();
            } else {
                self.push_widgets();
            }
            let animating = self.advance_transitions(now);

            // Smart Sleep: the engine knows when the next lyric line, minute
            // boundary or animation frame is due, and this is the only loop
            // that cannot be woken by its command channel — so the deadline has
            // to come out of the sleep itself. It can only shorten it; see
            // `widget_wait`.
            let base = if animating { ANIM_TICK } else { TICK };
            std::thread::sleep(widget_wait(
                base,
                self.widgets.next_deadline(),
                Instant::now(),
            ));
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
                self.lock.on_apply(&self.config);
                match self.rebuild() {
                    Ok(_) => {
                        // See `overview_pending`'s doc comment: run() applies
                        // it right after this reply is on the wire.
                        self.overview_pending = true;
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
            Request::Lock => {
                let geoms = self.output_geoms_all();
                let ctx = self.lock.ctx(&self.config, &geoms);
                self.lock.lock(&ctx)
            }
            Request::LockPreview { width, height } => {
                let geoms = self.output_geoms_all();
                let ctx = self.lock.ctx(&self.config, &geoms);
                let np = self.widgets.now_playing();
                let theme = lock_widget_theme(&self.config);
                self.lock
                    .lock_preview(&ctx, np.as_ref(), theme, width, height)
            }
            Request::LockNotify { locked, sockets } => {
                self.lock.lock_notify(locked, sockets, Instant::now());
                Response::Ok
            }
            Request::LockSetup => {
                let geoms = self.output_geoms_all();
                let ctx = self.lock.ctx(&self.config, &geoms);
                self.lock.lock_setup(&ctx)
            }
            Request::LockUndo => {
                let geoms = self.output_geoms_all();
                let ctx = self.lock.ctx(&self.config, &geoms);
                self.lock.lock_undo(&ctx)
            }
        }
    }

    /// Every currently known output, for [`HostCtx::outputs`] — unlike
    /// [`Daemon::output_geoms`], not filtered to a widget's own target list.
    fn output_geoms_all(&self) -> Vec<widgets::OutputGeom> {
        self.monitors
            .iter()
            .map(|m| widgets::OutputGeom {
                connector: m.connector.clone(),
                w: u32::from(m.width),
                h: u32::from(m.height),
                scale_milli: m.scale_milli,
            })
            .collect()
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
        let geoms = self.output_geoms_all();
        let ctx = self.lock.ctx(&self.config, &geoms);
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
            gave_up: Vec::new(), // X11 backend has no give-up fallback
            lockscreen: Some(self.lock.status(&ctx, &self.config)),
            // `check_schedule` writes the swapped slot into this in-memory copy
            // (never to disk), so it — not config.toml — is what is on screen.
            wallpaper_path: playing_media_path(&self.config.wallpaper),
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
            Kind::Slideshow => w.slideshow.as_ref().map(|_| {
                // Ask the renderer that is playing it, never the disk: the
                // renderer resolved the folder when it was built, and a scan
                // here would run on every status poll (every few seconds,
                // from the main loop) and, with "Include subfolders", walk a
                // whole tree each time. No renderer holding it means nothing
                // could be drawn (the backend skips an output with no
                // images), which is exactly the "no images found" label.
                let images = self
                    .renderers
                    .iter()
                    .filter(|r| std::ptr::eq(self.config.wallpaper_for(&r.window.connector), w))
                    .find_map(|r| r.slideshow.as_ref())
                    .map_or(0, |s| s.images.len());
                slideshow_status_label(images)
            }),
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

    /// Poll EWMH fullscreen (and, if configured, maximized) state and
    /// reconcile per-monitor pause on change.
    fn check_fullscreen(&mut self) {
        let covered = x11_fullscreen::covered_connectors(
            &self.conn,
            self.screen().root,
            &self.atoms,
            &self.monitors,
            self.config.pause_on_maximized,
        );
        if covered != self.fullscreen_covered {
            for (c, what) in &covered {
                if !self.fullscreen_covered.contains_key(c) {
                    log::info!("[{c}] {what} detected; pausing wallpaper");
                }
            }
            for (c, what) in &self.fullscreen_covered {
                if !covered.contains_key(c) {
                    log::info!("[{c}] {what} cleared; resuming wallpaper");
                    self.widgets.invalidate(); // overlays skipped while covered
                }
            }
            self.fullscreen_covered = covered;
            self.reconcile_pause();
        }
    }

    /// Re-assert every wallpaper window's place in the stack (~every 2s), since
    /// other clients' stacking changes can shuffle us. Normally that means
    /// lowering back to the bottom, but only when we're not there already: a
    /// window we own is WM-managed, so `lower` is a ConfigureRequest the WM
    /// answers by restacking and re-announcing the whole stack — on a plain
    /// X11 desktop that stutters window-switch animations for nothing when we
    /// were at the bottom already (issue #17). In DDE restack mode our windows
    /// must be RAISED instead — lowering there would drop the wallpaper
    /// straight back under dde-shell's desktop window a couple of seconds
    /// after it appeared.
    ///
    /// The DDE raise is not unconditional: when the user clicks the desktop,
    /// DDE's window comes up above ours and the icons become usable, so the
    /// raise waits out `dde_icon_peek_secs` before taking the stack back (see
    /// [`dde::IconPeek`]).
    fn reassert_stacking(&mut self) {
        if let Some(desktop) = self.dde_mode.mirror_desktop() {
            // The mirror thread owns stacking here: it lowers the desktop
            // window the moment the window manager raises it, on its own
            // connection, within milliseconds. No icon peek (the icons are
            // always visible), and lowering our windows would put them under
            // the desktop's key-coloured window.
            if self.caja_mirror.as_ref().is_some_and(|m| m.failed()) {
                // The thread has already logged why. Our windows are still
                // above the desktop, so from the next pass on restack keeps
                // them there.
                self.fall_back_to_restack(desktop, "the icon mirror stopped");
            }
            return;
        }
        if self.dde_mode == dde::Mode::Restack {
            let windows: Vec<x11rb::protocol::xproto::Window> =
                self.renderers.iter().map(|r| r.window.window).collect();
            let root = self.screen().root;
            let peek = dde::icon_peek(self.config.dde_icon_peek_secs);
            self.dde_peek
                .tick(&self.conn, &self.atoms, root, &windows, peek);
            return;
        }
        let ours: Vec<x11rb::protocol::xproto::Window> =
            self.renderers.iter().map(|r| r.window.window).collect();
        let stack = x11win::stacking_order(&self.conn, &self.atoms, self.screen().root);
        if x11win::at_bottom(&stack, &ours) == Some(true) {
            return;
        }
        let root = self.screen().root;
        for r in &self.renderers {
            let _ = x11win::lower_wallpaper(&self.conn, &self.atoms, root, r.window.window);
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
        let discharging = crate::battery::on_battery();
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
        // `due` already warned about (and filtered out) path-less wallpapers.
        let Some(path) = want.effective_path().map(|p| p.to_path_buf()) else {
            return;
        };
        log::info!(
            "schedule: switching default wallpaper to {}",
            path.display()
        );
        let effect = want.transition;
        for r in &mut self.renderers {
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
                // The new crop is where the transition must come to rest, so
                // teach the machine about it before it starts easing anywhere.
                r.anim.set_base(transition::crop_base(&want));
                r.anim.start(effect, path.clone(), &r.player);
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
        cosmic_bg::apply(&self.config);
        dde_lock::apply(&self.config);
        kde_desktop::apply(&self.config);
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

    /// How many renderers the current config and monitor layout call for —
    /// mirrors `rebuild`'s skip condition. With no monitors reported yet, the
    /// global wallpaper stands in for "at least one", so an empty RandR answer
    /// at login still counts as short.
    fn expected_renderers(&self) -> usize {
        if kde_desktop::enabled() {
            return 0; // plasmashell draws the wallpaper; we create no window
        }
        let wants = |w: &Wallpaper| w.effective_path().is_some() || w.kind == Kind::Slideshow;
        if self.monitors.is_empty() {
            let any = wants(&self.config.wallpaper) || self.config.monitors.values().any(wants);
            return usize::from(any);
        }
        self.monitors
            .iter()
            .filter(|m| wants(self.config.wallpaper_for(&m.connector)))
            .count()
    }

    /// Retry a short startup build. Autostart runs us a few seconds after login,
    /// when RandR may report no monitors yet or `make_renderer` can fail, and
    /// neither hotplug (fires only on a layout *change*) nor the stall heal
    /// (looks only at existing renderers) would ever recover from that — the
    /// desktop would stay on the native wallpaper until the user reselected.
    fn check_startup_renderers(&mut self, now: Instant) {
        if now.duration_since(self.started_at) > STARTUP_RETRY_WINDOW
            || now.duration_since(self.last_startup_retry) < STARTUP_RETRY_INTERVAL
            || self.user_paused
        {
            return;
        }
        self.last_startup_retry = now;

        let expected = self.expected_renderers();
        if expected == 0 || self.renderers.len() >= expected {
            return; // nothing configured, or already complete
        }
        self.startup_retries += 1;
        log::warn!(
            "only {}/{expected} renderer(s) after start; rebuilding (attempt {})",
            self.renderers.len(),
            self.startup_retries
        );
        if let Err(e) = self.rebuild() {
            log::warn!("startup rebuild failed: {e:#}");
            return;
        }
        // Re-count: the rebuild may have discovered monitors it lacked before.
        let expected = self.expected_renderers();
        if !self.renderers.is_empty() && self.renderers.len() >= expected {
            log::info!(
                "startup rebuild succeeded after {} attempt(s): {} renderer(s)",
                self.startup_retries,
                self.renderers.len()
            );
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

    /// Advance every renderer's animation — a slideshow's dwell, or a plain
    /// wallpaper change mid-transition. Returns true while any is animating, so
    /// the caller can tick faster (~60fps).
    fn advance_transitions(&mut self, now: Instant) -> bool {
        let mut animating = false;
        for r in &mut self.renderers {
            animating |= r.advance(now);
        }
        animating
    }

    fn shutdown(&mut self) {
        self.lock.end_lock(); // drop the engine and any LayerFiles/socket state
        overview::restore();
        cosmic_bg::restore();
        dde_lock::restore();
        kde_desktop::restore();
        // MATE: stop copying Caja's icons (closing the thread's connection
        // undoes the redirect, so Caja renders on screen again), then swap the
        // key colour back for the user's own background. After
        // `overview::restore`, so the last word in `org.mate.background` is
        // the user's saved picture; a no-op when the mirror never ran. The
        // same call puts Xfce's xfconf backdrop back.
        if let Some(m) = self.caja_mirror.take() {
            m.stop(&self.conn);
        }
        caja_mirror::restore_background();
        // Put the user's original DDE wallpaper back (no-op off DDE / when
        // nothing was saved). After the mirror is stopped: restoring first
        // would let the mirror copy the whole photograph over the video for a
        // moment, since it is no longer the key colour.
        if crate::capability::is_deepin_dde() {
            dde::restore();
        }
        caja_mirror::restore_key_background(caja_mirror::Desktop::Dde);
        self.teardown_renderers();
        std::fs::remove_file(crate::ipc::socket_path()).ok();
        log::info!("frescod stopped");
    }
}

/// The stacking mode that takes over once the icon mirror has given up.
/// MATE and Deepin: [`dde::Mode::Restack`], the verified way to keep the
/// wallpaper above the desktop window. Xfce: [`dde::Mode::Inactive`] — the
/// wallpaper is above xfdesktop already, by layer, and the periodic raise
/// would only stir xfwm4's stack for nothing.
fn mode_after_mirror_gave_up(desktop: caja_mirror::Desktop) -> dde::Mode {
    match desktop {
        caja_mirror::Desktop::Xfce => dde::Mode::Inactive,
        caja_mirror::Desktop::Caja | caja_mirror::Desktop::Dde => dde::Mode::Restack,
    }
}

/// One slideshow's per-tick step: run the animation if one is going, otherwise
/// decide whether the dwell is up and hand the next image to [`Anim`]. Both
/// backends call this with their own `PlayerHandle`, so the engine is written
/// once. Returns true while mid-animation.
fn advance_slideshow<S: Surface>(
    player: &S,
    s: &mut Slideshow,
    anim: &mut Anim,
    now: Instant,
) -> bool {
    // A transition already in flight owns the player until it settles; the
    // dwell only restarts once it has.
    if anim.running() {
        let step = anim.step(player);
        if step == Step::Finished {
            s.last_advance = now;
        }
        return step.animating();
    }
    if s.images.len() <= 1 {
        return false;
    }
    let due = now.duration_since(s.last_advance) >= s.interval;
    let next = (s.idx + 1) % s.images.len();

    // Ken Burns is the one effect with no out/in halves: it drifts continuously
    // across the dwell itself, so the dwell — not the machine — drives it.
    if s.transition == Transition::KenBurns {
        let frac = now.duration_since(s.last_advance).as_secs_f64() / s.interval.as_secs_f64();
        anim.ken_burns(player, frac, s.idx.is_multiple_of(2));
        if due {
            s.idx = next;
            anim.start(Transition::KenBurns, s.images[s.idx].clone(), player);
            s.last_advance = now;
        }
        return true;
    }

    if !due {
        return false;
    }
    s.idx = next;
    let step = anim.start(s.transition, s.images[s.idx].clone(), player);
    if step == Step::Idle {
        // A hard cut is over the instant it happens; start the next dwell now.
        s.last_advance = now;
    }
    step.animating()
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
    Some(Slideshow {
        images,
        idx: 0,
        interval: Duration::from_secs(s.interval_s.max(2)),
        last_advance: Instant::now(),
        // The wallpaper-level field is the canonical one; `slideshow.transition`
        // is the legacy copy `Config::migrate` keeps in step with it.
        transition: wallpaper.transition,
    })
}

/// Resolve a slideshow's image list: explicit hand-picked `paths`, else a scan
/// of its `folder` (subfolders too when the slideshow says so). The scan and
/// its extension rules live in [`crate::media`], shared with the GUI so the
/// card, the editor preview and the health check agree with what plays here.
fn slideshow_images(s: &crate::config::Slideshow) -> Vec<PathBuf> {
    if !s.paths.is_empty() {
        s.paths.clone()
    } else if let Some(folder) = &s.folder {
        crate::media::slideshow_frames(folder, s.recursive)
    } else {
        Vec::new()
    }
}

/// True for a slideshow wallpaper that resolves to no image at all: an empty
/// folder, or (issue #36) a folder holding only videos. Nothing can be drawn
/// for it, so the X11 backend skips the output rather than opening a black
/// window that looks like a wallpaper in use.
fn slideshow_has_no_images(w: &Wallpaper) -> bool {
    w.kind == Kind::Slideshow
        && w.slideshow
            .as_ref()
            .is_none_or(|s| slideshow_images(s).is_empty())
}

/// The status-pill text for a slideshow. "(0 images)" read as if something
/// were running; say plainly that nothing was found.
fn slideshow_status_label(images: usize) -> String {
    match images {
        0 => "Slideshow (no images found)".to_string(),
        n => format!("Slideshow ({n} images)"),
    }
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
    /// The path-less-slot warning was already logged for the current slot
    /// (ticks run every 2 s; warn once, not ~43k times a day).
    warned_no_path: bool,
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
        self.due_for(config, schedule_desired_wallpaper(config)?)
    }

    /// [`Self::due`] with the wall-clock lookup factored out, so tests can
    /// feed it the wallpaper a given (possibly jumped) clock time would want.
    fn due_for(&mut self, config: &Config, want: Wallpaper) -> Option<Wallpaper> {
        let Some(path) = want.effective_path().map(|p| p.to_path_buf()) else {
            // Level-triggered and silent-by-default is how a schedule that
            // "never switches" hid in the field: say why, once per slot.
            if !self.warned_no_path {
                self.warned_no_path = true;
                log::warn!(
                    "schedule: the scheduled wallpaper has no usable file path; not switching"
                );
            } else {
                log::debug!("schedule: scheduled wallpaper still has no usable file path");
            }
            return None;
        };
        self.warned_no_path = false; // a slot with a path re-arms the warning
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

/// The single media file a wallpaper is showing, for `StatusReply::wallpaper_path`.
/// Playlists and slideshows have no one file to name (`effective_path` would
/// answer with a playlist's first item, which is not necessarily what is
/// playing), so they report none and the GUI keeps going by its own config.
fn playing_media_path(w: &Wallpaper) -> Option<PathBuf> {
    match w.kind {
        Kind::Video | Kind::Image => w.effective_path().map(|p| p.to_path_buf()),
        Kind::Playlist | Kind::Slideshow => None,
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
///
/// NOT when an NVIDIA GPU is also present: `vo=gpu` then renders on the NVIDIA
/// GL context, and pinning iHD pushed decode onto the iGPU, whose surfaces mpv
/// can't share with that context — it fell back to `vaapi-copy` (decode →
/// readback to RAM → re-upload), ~25% CPU for 1440p30 on an MX130. On those
/// machines [`crate::config::hwdec`] prefers NVDEC instead.
fn setup_vaapi_env() {
    if std::env::var_os("LIBVA_DRIVER_NAME").is_some() {
        return;
    }
    let gpus = crate::config::gpu_vendors();
    if gpus.intel && !gpus.nvidia {
        // Intel: iHD (Gen8+/Broadwell and newer, incl. Alder Lake).
        std::env::set_var("LIBVA_DRIVER_NAME", "iHD");
        log::info!("VA-API: pinned Intel iHD driver for hardware decode");
    } else if gpus.intel {
        log::info!("VA-API: Intel+NVIDIA hybrid, not pinning iHD (NVDEC preferred)");
    }
}

pub fn run() -> Result<()> {
    use crate::capability::{detect, Capability};
    // First, before any thread exists: termination signals become a clean
    // Stop, so a logout or `pkill` still puts the user's desktop back.
    signals::install();
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
        cosmic_bg::restore();
        // And the Deepin lock-screen background (no-op off Deepin).
        dde_lock::restore();
        // And the Plasma desktop wallpaper plugin (no-op off KDE).
        kde_desktop::restore();
        // Same for DDE: a crashed run may have left the transparent wallpaper
        // applied with the original saved on disk — restore it (no-op
        // otherwise).
        if crate::capability::is_deepin_dde() {
            dde::restore();
        }
        // And for MATE and Xfce: a crashed icon-mirror run leaves Caja (or
        // xfdesktop) painting the key colour, with the user's background saved
        // on disk (no-op otherwise, so it needs no desktop check).
        caja_mirror::restore_background();
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
        cosmic_bg::restore();
        log::info!("wallpaper disabled (enabled=false) — exiting");
        return Ok(());
    }
    let commands = control::start_server()?;
    overview::apply(&config.wallpaper);
    cosmic_bg::apply(&config);
    log::info!("frescod started (GNOME Wayland static-frame mode)");
    crate::telemetry::heartbeat(Some("gnome-static"), None, None);

    // GNOME is still-frame-only (`live_video: false, widgets: false` -- see
    // `LockRuntime::status`), so there is no live engine here for a lock/
    // unlock transition to swap -- `LockRuntime` in this loop exists only to
    // answer `Request::Lock*`/`Status.lockscreen` correctly. Polling it is
    // therefore piggybacked on whatever else already wakes this loop
    // (a request, or the heartbeat timeout) rather than adding a fast tick
    // cadence of its own, which would work against this mode's whole point
    // ("blocks on the control channel between commands -> ~0% CPU").
    let mut lock_rt = LockRuntime::new();

    // `recv_timeout` rather than a plain blocking `recv`: this mode otherwise
    // never wakes on its own, so a daemon left running for days without an
    // Apply would only ever have sent the one startup heartbeat above and
    // then gone quiet in the usage numbers. `heartbeat` self-throttles to
    // roughly once a day via its own marker file either way.
    loop {
        let (req, reply) = match commands.recv_timeout(HEARTBEAT_RECHECK_INTERVAL) {
            Ok(pair) => pair,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                lock_rt.poll(Instant::now());
                crate::telemetry::heartbeat(Some("gnome-static"), None, None);
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        lock_rt.poll(Instant::now());
        let is_stop = matches!(req, Request::Stop);
        let is_apply = matches!(req, Request::Apply);
        let resp = match req {
            Request::Apply => {
                config = Config::load().unwrap_or_else(|_| config.clone());
                lock_rt.on_apply(&config);
                Response::Ok
            }
            // A static frame has nothing to pause.
            Request::Pause | Request::Resume => Response::Ok,
            Request::Status => {
                let ctx = lock_rt.ctx(&config, &[]);
                Response::Status(static_status(&config, &lock_rt.status(&ctx, &config)))
            }
            Request::Update => {
                notifier::run_updater_async();
                Response::Ok
            }
            Request::Stop => Response::Ok,
            // GNOME static-frame mode has no live surface and no `Desktop`
            // target host, so `Request::Lock` and friends are handled purely
            // through `LockRuntime` — `loginctl lock-session` for `Lock`, a
            // still-frame `compose_still` preview, and `Status.lockscreen`
            // reporting `live_video: false, widgets: false` (GNOME is not in
            // the capable-host set — see `LockRuntime::status`).
            Request::Lock => {
                let ctx = lock_rt.ctx(&config, &[]);
                lock_rt.lock(&ctx)
            }
            Request::LockPreview { width, height } => {
                let ctx = lock_rt.ctx(&config, &[]);
                let theme = lock_widget_theme(&config);
                lock_rt.lock_preview(&ctx, None, theme, width, height)
            }
            Request::LockNotify { locked, sockets } => {
                lock_rt.lock_notify(locked, sockets, Instant::now());
                Response::Ok
            }
            Request::LockSetup => {
                let ctx = lock_rt.ctx(&config, &[]);
                lock_rt.lock_setup(&ctx)
            }
            Request::LockUndo => {
                let ctx = lock_rt.ctx(&config, &[]);
                lock_rt.lock_undo(&ctx)
            }
        };
        let _ = reply.send(resp);
        // The overview redecode (ffmpegthumbnailer at full size) happens
        // after the reply is sent, not before — the caller (off the GTK
        // thread already; see `daemon_ctl::apply_async`) gets its `Ok` back
        // as soon as the new config is loaded, instead of waiting out the
        // redecode too.
        if is_apply {
            if config.enabled {
                overview::apply(&config.wallpaper);
                cosmic_bg::apply(&config);
            } else {
                overview::restore();
                cosmic_bg::restore();
            }
        }
        if is_stop {
            break;
        }
    }

    lock_rt.end_lock(); // drop any LayerFiles/socket state (no live engine here otherwise)
    overview::restore();
    cosmic_bg::restore();
    std::fs::remove_file(crate::ipc::socket_path()).ok();
    log::info!("frescod stopped");
    Ok(())
}

/// Minimal status for the GNOME static-frame fallback mode.
fn static_status(config: &Config, lockscreen: &LockStatus) -> StatusReply {
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
        gave_up: Vec::new(),
        lockscreen: Some(lockscreen.clone()),
        wallpaper_path: playing_media_path(&config.wallpaper),
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
        // Safety net, same as `run_x11`'s: a prior run killed rather than
        // Stopped may have left cosmic-bg pointed at our still frame.
        cosmic_bg::restore();
        dde_lock::restore();
        kde_desktop::restore();
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
            scale_milli: 1000,
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
    let mut last_heartbeat_check = Instant::now();
    // Issue #28: on Cinnamon's newer, layer-shell-capable muffin, a brand-new
    // mpvpaper surface maps underneath cinnamon-background-daemon's own
    // window unless that daemon is restarted afterwards. `capability::detect`
    // already ran (we're in this function only because it chose
    // WaylandLayerShell), so the gate below only needs to add "and is this
    // actually Cinnamon, with the newer daemon". Cheap to recompute per
    // restack (it's already rate-limited to roughly once per 10s+).
    let is_cinnamon = crate::capability::is_cinnamon();
    let mut restack_scheduler = cinnamon_bg::RestackScheduler::new();
    // How often to check whether Cinnamon's background daemon needs
    // (re-)activating (its bus name has no owner). Independent of restacking
    // — this just guards against Fresco outliving the daemon on a session
    // where something else killed it.
    const ENSURE_DAEMON_PROBE: Duration = Duration::from_secs(30);
    let mut next_ensure_daemon = Instant::now();
    // Display-presence probe: due immediately, then paced by whether anything is
    // still restarting (see the supervise block).
    let mut next_output_probe = Instant::now();
    let mut probed = false;
    let mut sched = SchedState::default();

    // Pause the wallpaper on any output that has a fullscreen window. Available on
    // wlroots compositors (wlr protocol) and COSMIC (zcosmic-toplevel-info); absent
    // on KDE Wayland (KWin does not expose wlr-foreign-toplevel to ordinary
    // clients) and on GNOME (which uses the static path, not this one).
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
    let mut widget_engine = widgets::WidgetEngine::new(config.widgets.as_ref(), config.accent);
    apply_widget_config(&mut widget_engine, &config);

    // Lock-screen host detection, state monitor and widget engine (see
    // `LockRuntime`) — COSMIC is the one host with a `Desktop` target today,
    // and this is the one run loop that ever drives it.
    let mut lock_rt = LockRuntime::new();
    let mut show_on_lock = wants_show_on_lock(&config, lock_rt.kind);
    let mut lock_dim_applied = 0i32;

    // COSMIC only: `overview` doesn't apply here (COSMIC has none of the
    // GNOME/Cinnamon/MATE schemas), but its lock screen has the exact same
    // "can't see the live wallpaper" problem GNOME's overview has — see
    // `cosmic_bg`'s module doc. No-op on every other layer-shell compositor.
    //
    // Synced BEFORE any mpvpaper exists, not after: when this changes
    // `cosmic-bg`'s config (first run, or because shutdown restored the
    // original), `cosmic-bg` recreates its surfaces, and cosmic-comp stacks a
    // newer surface above an older one — so an mpvpaper started first would
    // end up hidden behind a still picture (the 1.1.46 regression). Waiting
    // here, only when something actually changed, means the surfaces below
    // are created last and therefore on top.
    let mut cosmic_reloads = cosmic_bg::ReloadTracker::default();
    let synced = cosmic_bg::apply(&config);
    if let Some(wait) = cosmic_reloads.note_before_spawn(&synced, Instant::now()) {
        log::info!(
            "cosmic-bg: configuration changed; waiting {} ms for it to redraw before starting the wallpaper",
            wait.as_millis()
        );
        std::thread::sleep(wait);
    }

    // One supervised mpvpaper per output, keyed by connector name.
    let mut outputs: BTreeMap<String, WlOutput> = BTreeMap::new();
    // KDE Plasma (issue #44): plasmashell's desktop surface is a layer-shell
    // background too, and an opaque one — an mpvpaper surface is never seen
    // (or hides the icons), so Plasma gets its wallpaper through plasmashell
    // instead (`kde_desktop`) and no output is spawned.
    let plasma = kde_desktop::enabled();
    for m in monitors.iter().filter(|_| !plasma) {
        let wallpaper = config.wallpaper_for(&m.connector).clone();
        if wallpaper.effective_path().is_none()
            && wallpaper.paths.is_empty()
            && wallpaper.kind != Kind::Slideshow
        {
            continue; // nothing configured for this output
        }
        let effective_ps = wallpaper.effective_power_saving(config.power_saving);
        let mut out = WlOutput::new(m.connector.clone(), wallpaper, config.scaling, effective_ps);
        out.set_show_on_lock(show_on_lock);
        out.respawn(false, false);
        outputs.insert(m.connector.clone(), out);
    }
    // Deepin (Treeland) only, and only with the lock feature on: its lock
    // screen draws the user's greeter background, not our surface — see
    // `dde_lock`'s module doc. No-op on every other compositor. (COSMIC's
    // `cosmic-bg` sync already ran above, before any mpvpaper existed.)
    dde_lock::apply(&config);
    kde_desktop::apply(&config);
    log::info!(
        "frescod started (Wayland layer-shell / mpvpaper, {} output(s))",
        outputs.len()
    );
    crate::telemetry::heartbeat(Some("wayland"), None, Some(outputs.len() as u32));

    loop {
        let base = if outputs.values().any(|o| o.animating) {
            ANIM_TICK
        } else {
            TICK
        };
        // Smart Sleep: `recv_timeout` is exactly the interruptible wait the
        // engine's docs ask for, so clamping it to the widget deadline costs
        // nothing and an IPC request still lands immediately. It only ever
        // shortens the wait — see `widget_wait`. The lock engine's own
        // deadline (clock/battery while locked) is folded in the same way, so
        // a locked session with an active clock still wakes on the minute.
        let now0 = Instant::now();
        let lock_deadline = lock_rt.engine.as_ref().and_then(|e| e.next_deadline(now0));
        let deadline = min_instant(widget_engine.next_deadline(), lock_deadline);
        let tick = widget_wait(base, deadline, now0);
        match commands.recv_timeout(tick) {
            Ok((req, reply)) => {
                let is_stop = matches!(req, Request::Stop);
                let resp = match req {
                    Request::Apply => {
                        // Blank every overlay first, against the mpvpaper
                        // processes that are still up. The X11 loop gets this
                        // from `teardown_renderers`; here nothing dies on an
                        // Apply that only changes settings, so without this a
                        // widget being switched off — or one that is about to
                        // be drawn smaller — leaves its old pixels on screen
                        // with nothing left that would ever take them down.
                        clear_wayland_widgets(&mut widget_engine, &outputs);
                        config = Config::load().unwrap_or_else(|_| config.clone());
                        sched.hold_current(&config);
                        // Widget settings ride in the same file, so a GUI toggle
                        // arrives here. invalidate() re-pushes even when the
                        // content itself (e.g. the lyric line) is unchanged.
                        apply_widget_config(&mut widget_engine, &config);
                        widget_engine.invalidate();
                        lock_rt.on_apply(&config);
                        // MPVPAPER_SHOW_ON_LOCK is read once at mpvpaper's own
                        // startup, so a change here can only take effect
                        // through a respawn — `WlOutput::set_show_on_lock`
                        // reports exactly that below, per output.
                        show_on_lock = wants_show_on_lock(&config, lock_rt.kind);
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
                                let has = !plasma
                                    && (wp.effective_path().is_some()
                                        || !wp.paths.is_empty()
                                        || wp.kind == Kind::Slideshow);
                                let effective_ps = wp.effective_power_saving(config.power_saving);
                                match (outputs.get_mut(&m.connector), has) {
                                    (Some(o), true) => {
                                        o.apply_wallpaper(wp, config.scaling, effective_ps, paused);
                                        // `apply_wallpaper` may already have
                                        // respawned for an unrelated reason;
                                        // a second one here only fires when
                                        // the flag actually changed and the
                                        // first respawn (if any) hasn't
                                        // already carried the new value.
                                        if o.set_show_on_lock(show_on_lock) {
                                            o.respawn(paused, false);
                                        }
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
                                        o.set_show_on_lock(show_on_lock);
                                        o.respawn(paused, false);
                                        outputs.insert(m.connector.clone(), o);
                                    }
                                    (None, false) => {}
                                }
                            }
                        } else {
                            outputs.clear(); // kills every mpvpaper
                        }
                        // Same "reply first, sync cosmic-bg after" trade-off
                        // as the X11/GNOME-static paths would use, but this
                        // loop already does the whole reconciliation above
                        // inline before replying, so there is no separate
                        // deferred slot to piggyback on here.
                        //
                        // Usually a no-op for `cosmic-bg`'s config (fixed
                        // frame paths, rewritten only on change). When it
                        // does change — a monitor override added or removed,
                        // a switch between per-output and same-on-all —
                        // `cosmic-bg` recreates its surfaces above the
                        // mpvpaper ones just reconciled, and the tracker
                        // schedules the respawn that puts them back on top.
                        if config.enabled {
                            let synced = cosmic_bg::apply(&config);
                            cosmic_reloads.note(&synced, Instant::now());
                            dde_lock::apply(&config);
                            kde_desktop::apply(&config);
                        } else {
                            cosmic_bg::restore();
                            cosmic_reloads.reset();
                            dde_lock::restore();
                            kde_desktop::restore();
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
                    Request::Status => {
                        let geoms = wayland_all_output_geoms(&monitors);
                        let ctx = lock_rt.ctx(&config, &geoms);
                        Response::Status(wayland_status(
                            &monitors,
                            &outputs,
                            &config.wallpaper,
                            user_paused || battery_paused,
                            lock_rt.status(&ctx, &config),
                        ))
                    }
                    Request::Update => {
                        notifier::run_updater_async();
                        Response::Ok
                    }
                    Request::Stop => Response::Ok,
                    Request::Lock => {
                        let geoms = wayland_all_output_geoms(&monitors);
                        let ctx = lock_rt.ctx(&config, &geoms);
                        lock_rt.lock(&ctx)
                    }
                    Request::LockPreview { width, height } => {
                        let geoms = wayland_all_output_geoms(&monitors);
                        let ctx = lock_rt.ctx(&config, &geoms);
                        let np = widget_engine.now_playing();
                        let theme = lock_widget_theme(&config);
                        lock_rt.lock_preview(&ctx, np.as_ref(), theme, width, height)
                    }
                    Request::LockNotify { locked, sockets } => {
                        lock_rt.lock_notify(locked, sockets, Instant::now());
                        Response::Ok
                    }
                    Request::LockSetup => {
                        let geoms = wayland_all_output_geoms(&monitors);
                        let ctx = lock_rt.ctx(&config, &geoms);
                        lock_rt.lock_setup(&ctx)
                    }
                    Request::LockUndo => {
                        let geoms = wayland_all_output_geoms(&monitors);
                        let ctx = lock_rt.ctx(&config, &geoms);
                        lock_rt.lock_undo(&ctx)
                    }
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

        if now.duration_since(last_heartbeat_check) >= HEARTBEAT_RECHECK_INTERVAL {
            last_heartbeat_check = now;
            // Same arguments as the startup call above; `heartbeat` itself
            // throttles to roughly once a day via its marker file, so a
            // daemon that runs for days without a restart still checks in.
            crate::telemetry::heartbeat(Some("wayland"), None, Some(outputs.len() as u32));
        }

        // Lock screen: react to a lock/unlock transition before anything else
        // this tick touches the desktop widgets or the players, so nothing
        // desktop-only is ever pushed after the screen is believed locked.
        if let Some(now_locked) = lock_rt.poll(now) {
            if now_locked {
                // COSMIC's lock screen shows whatever `cosmic-bg` last drew,
                // and a new wallpaper only reaches it through a `cosmic-bg`
                // reload — which stacks new `cosmic-bg` surfaces above the
                // video, so it is held back until the screen is locked and
                // nobody is looking at the desktop (the respawn that fixes
                // the stacking then happens behind the lock screen too).
                // Skipped when mpvpaper itself is on the lock screen: the
                // live video is what's shown, and respawning it would blink.
                if matches!(lock_rt.kind, HostKind::Cosmic { .. }) && !show_on_lock {
                    let synced = cosmic_bg::refresh_for_lock(&config, cosmic_reloads.frame_stale());
                    cosmic_reloads.note(&synced, Instant::now());
                }
                if let Some(resolved) = LockRuntime::resolved(&config) {
                    let cleared = widget_engine.clear_for_lock(
                        resolved
                            .widgets
                            .contains(&crate::lockscreen::LockWidget::Lyrics),
                        resolved
                            .widgets
                            .contains(&crate::lockscreen::LockWidget::Visualizer),
                    );
                    dispatch_wayland_lock_updates(cleared, &outputs);
                    widget_engine.set_lock_album_art(
                        resolved
                            .widgets
                            .contains(&crate::lockscreen::LockWidget::AlbumArt),
                    );
                    let geoms = wayland_all_output_geoms(&monitors);
                    let ctx = lock_rt.ctx(&config, &geoms);
                    let theme = lock_widget_theme(&config);
                    lock_rt.begin_lock(&resolved, &ctx, theme);
                }
            } else {
                let cleared = lock_rt.end_lock();
                dispatch_wayland_lock_updates(cleared, &outputs);
                widget_engine.set_lock_album_art(false);
                widget_engine.invalidate();
                // Restore whatever the lock's own live-video/dim policy
                // touched — `reconcile_pause`'s own change-gating (below,
                // every tick) picks this up the moment `lock_rt.locked` is
                // false, and this only needs to reset the dim, which has no
                // equivalent "just reconcile it" path of its own.
                if lock_dim_applied != 0 {
                    lock_dim_applied = 0;
                    for o in outputs.values() {
                        if let Some(p) = o.player.as_ref() {
                            p.set_brightness(0);
                        }
                    }
                }
            }
        }

        // Advance the lock engine's own content (clock/battery/media) while
        // locked. `Desktop` returns updates to dispatch here, exactly like
        // the desktop widget engine; `Sockets`/`LayerFiles` do their own I/O
        // and always return an empty `Vec`.
        if lock_rt.locked {
            if let Some(engine) = &mut lock_rt.engine {
                // Geometry before the tick — same rule as the desktop widget
                // engine's own `set_outputs`: a bitmap widget sizes and
                // places itself during `tick`, so telling the engine about a
                // mode change afterwards places the next frame against the
                // stale one.
                engine.set_outputs(&wayland_all_output_geoms(&monitors));
                let np = widget_engine.now_playing();
                let updates = engine.tick(np.as_ref());
                dispatch_wayland_lock_updates(updates, &outputs);
            }
            // COSMIC's `Desktop` target only: the live-video/dim policy is a
            // property of the desktop's own mpvpaper, so it is applied here
            // directly rather than through `LockEngine` (which never touches
            // a player handle at all — see its module docs). Change-gated,
            // same discipline `reconcile_pause` already uses, so an unlocked
            // tick with nothing to do costs one comparison.
            if lock_rt.desktop_target {
                if let Some(resolved) = LockRuntime::resolved(&config) {
                    let want_dim = -((resolved.dim * 100.0).round() as i32);
                    if want_dim != lock_dim_applied {
                        lock_dim_applied = want_dim;
                        for o in outputs.values() {
                            if let Some(p) = o.player.as_ref() {
                                p.set_brightness(want_dim);
                            }
                        }
                    }
                }
            }
        }

        // `cosmic-bg` reloaded a settle period ago and recreated its surfaces
        // on every output, above any mpvpaper that already existed (see the
        // `cosmic_bg` module doc). A fresh mpvpaper surface is created newer,
        // so respawning each running one puts the video back on top. Outputs
        // with no player are skipped: whenever they are next spawned it will
        // be after the settle, so they already land on top.
        if cosmic_reloads.respawn_due(now) {
            let paused = user_paused || battery_paused;
            let mut respawned = 0;
            for o in outputs.values_mut() {
                if o.player.is_some() {
                    let hold_frame = o.static_fallback;
                    o.respawn(paused, hold_frame);
                    respawned += 1;
                }
            }
            if respawned > 0 {
                log::info!(
                    "cosmic-bg: restacked the wallpaper above its background ({respawned} output(s))"
                );
                // The lock dim lives on the player that just died; forget it
                // so the lock block above re-applies it to the new ones.
                lock_dim_applied = 0;
            }
        }

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
                                // The new crop is where the transition rests.
                                o.anim.set_base(transition::crop_base(&want));
                                o.animating =
                                    o.anim.start(want.transition, path.clone(), pl).animating();
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
                    kde_desktop::apply(&config);
                }
            }

            if config.pause_on_battery {
                let discharging = crate::battery::on_battery();
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
                    match wayland_outputs::list_outputs() {
                        Ok(m) => Some(m.into_iter().map(|x| x.connector).collect()),
                        // No compositor at the other end of WAYLAND_DISPLAY:
                        // every display is away, so every output parks. Before
                        // this, a session ending (or a compositor restarting on
                        // a new socket) read as "assume present", and the
                        // restart budget was spent on spawns that could only
                        // fail — a `renderer_giveup` blamed on the renderer.
                        Err(e) if wayland_outputs::is_unreachable(&e) => {
                            log::warn!("compositor unreachable ({e:#}); parking all outputs");
                            Some(HashSet::new())
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                };
            for (connector, o) in outputs.iter_mut() {
                // No probe this tick, or enumeration failed → no news: an
                // output keeps the state it has. For one that is not parked
                // that is "assume present", exactly the behaviour before the
                // display check existed; for a parked one it means staying
                // parked until a probe actually sees the display again, rather
                // than un-parking on every probe-less tick and re-spawning
                // into a compositor that just refused us.
                let here = connector == ALL_OUTPUTS
                    || present
                        .as_ref()
                        .map_or(!o.absent, |s| s.contains(connector));
                let was_confirmed = o.confirmed_live;
                o.supervise(paused, MAX_RESTARTS, here);
                if is_cinnamon && !was_confirmed && o.confirmed_live {
                    // A brand-new mpvpaper surface (first start, stall
                    // respawn, hotplug, static-fallback) just came up on this
                    // output — never a `loadfile` media swap, which reuses
                    // the existing player and so never flips this edge. Debounced
                    // and rate-limited inside the scheduler; the gdbus round
                    // trips run off this thread so a slow restack never stalls
                    // the tick loop.
                    restack_scheduler.note_spawn(Instant::now());
                }
            }
            if restack_scheduler.poll(Instant::now()) {
                std::thread::spawn(|| {
                    if cinnamon_bg::should_restack(crate::capability::Capability::WaylandLayerShell)
                    {
                        // Never let a panic mid-sequence (between SIGTERM and
                        // Start) silently leave the daemon dead — catch it,
                        // log it, and make a best-effort attempt to bring the
                        // daemon back regardless of where the panic landed.
                        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            cinnamon_bg::restack_cycle,
                        )) {
                            log::error!(
                                "cinnamon: restack thread panicked ({}); attempting to \
                                 reactivate the background daemon as a safety net",
                                panic_payload_message(&e)
                            );
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                cinnamon_bg::ensure_daemon_running,
                            ));
                        }
                    }
                });
            }
            if Instant::now() >= next_ensure_daemon {
                next_ensure_daemon = Instant::now() + ENSURE_DAEMON_PROBE;
                if is_cinnamon {
                    std::thread::spawn(|| {
                        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            cinnamon_bg::ensure_daemon_running,
                        )) {
                            log::error!(
                                "cinnamon: ensure_daemon_running thread panicked ({})",
                                panic_payload_message(&e)
                            );
                        }
                    });
                }
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
        // output: paused = user || battery || fullscreen-on-this-output ||
        // locked-and-the-lock-screen's-live-video-policy-says-no. This is the
        // single place pause is applied (reconcile_pause is change-gated), so
        // the four sources never fight over the player's pause property, and
        // unlocking naturally resumes unless `user_paused`/`battery_paused`
        // still apply — nothing here has to remember and restore a "previous"
        // pause value by hand.
        if let Some(w) = fs_watch.as_mut() {
            if now.duration_since(last_fs_poll) >= FS_POLL {
                last_fs_poll = now;
                let now_hidden = w.fullscreen_connectors(config.pause_on_maximized);
                if hidden.iter().any(|c| !now_hidden.contains(c)) {
                    widget_engine.invalidate(); // overlays skipped while covered
                }
                hidden = now_hidden;
            }
        }
        let lock_forces_pause = lock_rt.locked
            && lock_rt.desktop_target
            && LockRuntime::resolved(&config)
                .is_some_and(|r| !r.live_video.plays(crate::battery::on_battery()));
        let base_paused = user_paused || battery_paused || lock_forces_pause;
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
            // A respawned mpvpaper has no overlays for the lock engine's
            // `Desktop` target either — same reasoning, same fix.
            if let Some(engine) = &mut lock_rt.engine {
                engine.invalidate();
            }
        }

        // Widgets: only overlays whose content actually changed come back, so
        // this is a cheap no-op on almost every pass. Skipped while locked —
        // see the X11 loop's `run` for why ticking the desktop engine here
        // would undo `clear_for_lock`'s work the moment the clock (or disc)
        // next becomes "due".
        if widget_engine.is_active() && !lock_rt.locked {
            // Every display unless a connector is configured — see the X11
            // `push_widgets` note; the two backends must agree.
            let want = widget_engine.monitor().map(str::to_string);
            let targets: Vec<String> = outputs
                .keys()
                .filter(|c| want.as_deref().is_none_or(|w| w == c.as_str()))
                .cloned()
                .collect();
            if !targets.is_empty() {
                // Geometry before the tick, and one entry per target: bitmap
                // widgets place pixels in real output coordinates, so a
                // mixed-DPI pair needs a size each, not whichever one the
                // enumeration happened to list first.
                widget_engine.set_outputs(&wayland_output_geoms(&monitors, &targets, |c, mode| {
                    outputs.get(c).and_then(|o| o.osd_size(mode))
                }));
                for u in widget_engine.tick() {
                    for (c, o) in &outputs {
                        // Covered outputs show nothing: skip the repaint, but let
                        // clears through. Re-pushed (invalidate) once uncovered.
                        if !targets.iter().any(|t| t == c)
                            || !u.is_for(c)
                            || (hidden.contains(c) && !u.is_clear())
                        {
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

    lock_rt.end_lock(); // drop the engine and any LayerFiles/socket state
    outputs.clear(); // kill every mpvpaper before we exit
    cosmic_bg::restore();
    dde_lock::restore();
    kde_desktop::restore();
    std::fs::remove_file(crate::ipc::socket_path()).ok();
    log::info!("frescod stopped");
    Ok(())
}

/// The real pixel mode of each Wayland target, in target order.
///
/// Same fallback as the X11 side: an output the enumeration did not describe
/// (including the `ALL` pseudo-connector used when enumeration failed outright)
/// borrows the first known mode, else 1080p.
fn wayland_output_geoms(
    monitors: &[Monitor],
    targets: &[String],
    osd: impl Fn(&str, (u32, u32)) -> Option<(u32, u32)>,
) -> Vec<widgets::OutputGeom> {
    let fallback = monitors
        .iter()
        .find(|m| m.width != 0 && m.height != 0)
        .map_or((1920, 1080), |m| (u32::from(m.width), u32::from(m.height)));
    targets
        .iter()
        .map(|c| {
            let mode = monitors
                .iter()
                .find(|m| &m.connector == c && m.width != 0 && m.height != 0)
                .map_or(fallback, |m| (u32::from(m.width), u32::from(m.height)));
            // The renderer's own answer wins. The mode is only the fallback for
            // the window before mpv has an OSD to report, and on an unscaled
            // output the two agree anyway.
            let (w, h) = osd(c, mode).unwrap_or(mode);
            widgets::OutputGeom {
                connector: c.clone(),
                w,
                h,
                scale_milli: 1000,
            }
        })
        .collect()
}

/// Every currently known output, for [`HostCtx::outputs`] — unlike
/// [`wayland_output_geoms`], not filtered to a widget's own target list or
/// looked up against a live OSD size (`HostCtx` only needs geometry, not
/// exact render-surface pixels).
fn wayland_all_output_geoms(monitors: &[Monitor]) -> Vec<widgets::OutputGeom> {
    monitors
        .iter()
        .map(|m| widgets::OutputGeom {
            connector: m.connector.clone(),
            w: u32::from(m.width),
            h: u32::from(m.height),
            scale_milli: m.scale_milli,
        })
        .collect()
}

/// Dispatch a batch of lock-engine `WidgetUpdate`s to whichever mpvpaper
/// processes are still up — the Wayland twin of the X11 loop's
/// `Daemon::dispatch_widget_updates`.
fn dispatch_wayland_lock_updates(
    updates: Vec<widgets::WidgetUpdate>,
    outputs: &std::collections::BTreeMap<String, WlOutput>,
) {
    for u in &updates {
        for (c, o) in outputs {
            if !u.is_for(c) {
                continue;
            }
            if let Some(p) = o.player.as_ref() {
                dispatch_widget(p, u);
            }
        }
    }
}

/// Blank every widget overlay on every mpvpaper that is still up.
///
/// The Wayland twin of the X11 loop's `clear_widgets`. Overlays used to vanish
/// here only because the mpvpaper process died; on the paths where it survives
/// there was nothing taking them down at all.
fn clear_wayland_widgets(
    engine: &mut widgets::WidgetEngine,
    outputs: &std::collections::BTreeMap<String, WlOutput>,
) {
    for u in engine.clear_all() {
        for (c, o) in outputs {
            if !u.is_for(c) {
                continue;
            }
            if let Some(p) = o.player.as_ref() {
                dispatch_widget(p, &u);
            }
        }
    }
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
/// A remembered OSD reading, together with the facts it is only true under.
///
/// `pid` and the mode are the validity conditions, not data: a respawned
/// mpvpaper renegotiates its buffer and a mode change resizes it, so a reading
/// taken under either of those is about a space that no longer exists.
#[derive(Clone, Copy)]
struct OsdCache {
    w: u32,
    h: u32,
    pid: u32,
    mode_w: u32,
    mode_h: u32,
}

struct WlOutput {
    connector: String,
    wallpaper: Wallpaper,
    scaling: Scaling,
    /// Effective power-saving level for this output; applied at spawn.
    power_saving: PowerSaving,
    /// Whether the next (re)spawn should ask mpvpaper to mark its surface
    /// show-on-lock — see [`WaylandPlayer::spawn`]'s doc comment. `false`
    /// until [`WlOutput::set_show_on_lock`] says otherwise; read only at
    /// spawn time, since it becomes an environment variable on the child
    /// process and cannot be changed on an already-running one.
    show_on_lock: bool,
    player: Option<PlayerHandle>,
    slideshow: Option<Slideshow>,
    /// This output's transition. Per output, never shared: outputs run
    /// independent renderers and may be mid-transition at different phases.
    anim: Anim,
    restarts: u32,
    static_fallback: bool,
    error: Option<String>,
    animating: bool,
    /// Last pause state we applied to the player — lets `reconcile_pause` send IPC
    /// only on change. `Cell` so reconcile can stay `&self` like `set_paused`.
    /// Cached mpv OSD size — the space `overlay-add` places into — with the
    /// renderer pid and output mode it was read under.
    ///
    /// Cached because the widget loop asks every tick, which reaches ~60fps
    /// during a transition, and each read is a blocking IPC round trip. Keyed
    /// so it cannot outlive what it describes: a respawned mpvpaper renegotiates
    /// its buffer, and a mode change resizes it. A scale change at the *same*
    /// mode is the one case this will not notice until the renderer restarts.
    osd_size: std::cell::Cell<Option<OsdCache>>,
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
    /// Parked because no mpvpaper binary exists anywhere; cleared as soon as
    /// one is installed. Not a renderer failure, so it spends no budget.
    no_renderer: bool,
    /// `renderer_missing` already reported for this output (once per daemon).
    missing_reported: bool,
    /// Why the last renderer went down: "never_started", "dead", "frozen", or
    /// — once a respawn attempt has actually run — that attempt's content-free
    /// [`SpawnFail`] code, which is more specific than any of the three and
    /// overwrites them. Reported with `renderer_giveup` so a warning in the
    /// field says what actually broke, not just that *something* did.
    last_down: &'static str,
    last_spawn_fail: Option<&'static str>,
    /// [`crate::daemon::mpvpaper::ExitDetail`] of the most recent failed spawn
    /// (`"exit=... sig=..."`), already content-free — see its doc comment.
    /// `None` before any spawn has failed, or once one has succeeded.
    last_spawn_detail: Option<String>,
    /// `renderer_giveup` (telemetry) and the "give up" desktop notification
    /// have already been sent for this output. Latched for the daemon's whole
    /// run — see [`WlOutput::supervise`]'s give-up arm — so a re-armed output
    /// that fails again does not spam either channel a second time.
    giveup_reported: bool,
    /// Set once `supervise` has real evidence that this output's current
    /// player is actually *presenting* — not merely that the mpv IPC socket
    /// answers, which happens before the first frame is rendered. mpvpaper
    /// only attaches a buffer to its layer-shell surface (the point muffin
    /// maps it) at that first render, so confirming on "IPC alive" alone can
    /// fire the Cinnamon restack (issue #28) before mpvpaper's surface even
    /// exists — landing it under Cinnamon's freshly (re)mapped windows again,
    /// with nothing left to retrigger a fix. See [`presentation_confirmed`]
    /// for the actual evidence required. Cleared on every respawn/park. Read
    /// by the Wayland loop only to detect the false→true edge; a `loadfile`
    /// media swap that reuses the existing process never touches this flag.
    confirmed_live: bool,
    /// When the current player was spawned — the clock [`presentation_confirmed`]
    /// measures its grace period against for media that legitimately never
    /// advances (stills, paused-at-spawn, the static-fallback frame). Reset on
    /// every respawn.
    spawn_at: Instant,
    /// When a re-armed live-playback attempt (see [`RENDERER_REARM_DELAY`])
    /// is due, if this output has given up. `None` while playing normally or
    /// while a give-up is still waiting to be scheduled.
    next_rearm: Option<Instant>,
    /// Re-arms scheduled since the last Apply; doubles the wait each time so
    /// an output that can't recover isn't restarted every five minutes all
    /// session long.
    rearms: u32,
    /// Bumped on every [`WlOutput::respawn`]. A fresh mpv carries no overlays,
    /// so the widget engine must re-push after one — but the supervisor has
    /// several heal paths and threading a callback through each is how one gets
    /// missed. The loop instead watches this counter, which cannot go stale
    /// because `respawn` is the only writer.
    generation: u64,
}

impl WlOutput {
    /// The coordinate space this output's widgets must be placed in, if the
    /// renderer is up and has told us.
    ///
    /// `mode` is the output's current mode, and is part of the cache key rather
    /// than the answer: see [`crate::daemon::mpvpaper::PlayerHandle::osd_size`]
    /// for why the mode is not what `overlay-add` measures against.
    fn osd_size(&self, mode: (u32, u32)) -> Option<(u32, u32)> {
        let p = self.player.as_ref()?;
        let pid = p.pid()?;
        if let Some(c) = self.osd_size.get() {
            if (c.pid, c.mode_w, c.mode_h) == (pid, mode.0, mode.1) {
                return Some((c.w, c.h));
            }
        }
        let (w, h) = p.osd_size()?;
        self.osd_size.set(Some(OsdCache {
            w,
            h,
            pid,
            mode_w: mode.0,
            mode_h: mode.1,
        }));
        Some((w, h))
    }

    fn new(
        connector: String,
        wallpaper: Wallpaper,
        scaling: Scaling,
        power_saving: PowerSaving,
    ) -> WlOutput {
        WlOutput {
            anim: Anim::new(transition::crop_base(&wallpaper)),
            connector,
            wallpaper,
            scaling,
            power_saving,
            show_on_lock: false,
            player: None,
            slideshow: None,
            restarts: 0,
            static_fallback: false,
            error: None,
            animating: false,
            osd_size: std::cell::Cell::new(None),
            applied_paused: std::cell::Cell::new(false),
            stall_strikes: 0,
            last_pos: None,
            audio_heal: AudioHeal::new(),
            absent: false,
            no_media: false,
            no_renderer: false,
            missing_reported: false,
            last_down: "never_started",
            last_spawn_fail: None,
            last_spawn_detail: None,
            giveup_reported: false,
            confirmed_live: false,
            spawn_at: Instant::now(),
            next_rearm: None,
            rearms: 0,
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

    /// Set whether the next (re)spawn should mark mpvpaper's surface
    /// show-on-lock. Returns `true` when this actually changed the flag — the
    /// caller must force a respawn in that case (see the field's doc
    /// comment); returning `false` when it didn't means an in-place
    /// `loadfile`-only update (or no update at all) is still fine.
    fn set_show_on_lock(&mut self, v: bool) -> bool {
        if self.show_on_lock == v {
            return false;
        }
        self.show_on_lock = v;
        true
    }

    /// (Re)spawn the mpvpaper for this output. `paused` applies the current pause
    /// state; `static_frame` spawns then pauses (holds frame one) — the no-black
    /// per-output fallback when live playback keeps failing.
    fn respawn(&mut self, paused: bool, static_frame: bool) {
        // Bumped first: every exit from this function leaves a player that has
        // no overlays, including the failure paths.
        self.generation = self.generation.wrapping_add(1);
        drop(self.player.take());
        // A fresh process is a new, unconfirmed surface — see the field's doc
        // comment. The (re)confirmation happens on the next healthy
        // `supervise` tick.
        self.confirmed_live = false;
        self.slideshow = None;
        // The gamma, zoom and filter all died with that process, so the
        // transition is over — and must be dropped without IPC to a socket
        // nobody is reading any more.
        self.anim.forget();
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
            self.show_on_lock,
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
                self.last_spawn_detail = None;
                // The clock `presentation_confirmed`'s grace period measures
                // against — see the field's doc comment.
                self.spawn_at = Instant::now();
            }
            Err(e) => {
                log::error!("[{}] {e:#}", self.connector);
                self.last_spawn_fail = Some(
                    crate::daemon::mpvpaper::SpawnFail::of(&e).map_or("spawn_failed", |f| f.code()),
                );
                self.last_spawn_detail =
                    crate::daemon::mpvpaper::ExitDetail::of(&e).map(|d| d.to_string());
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
        let effect = new.transition;
        self.wallpaper = new;
        self.scaling = scaling;
        self.power_saving = power_saving;
        self.audio_heal = AudioHeal::new();
        self.anim.set_base(transition::crop_base(&self.wallpaper));
        if media_only {
            // The switch a video wallpaper actually takes: same player, new
            // file. Running it through the machine is what gives video the
            // transitions that used to be a slideshow's alone — and `start`
            // settles any animation still in flight from the last change.
            if let (Some(p), Some(path)) = (self.player.as_ref(), self.wallpaper.effective_path()) {
                let path = path.to_path_buf();
                self.animating = self.anim.start(effect, path, p).animating();
            }
        } else {
            self.restarts = 0;
            self.static_fallback = false;
            self.next_rearm = None;
            self.rearms = 0;
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

    /// One animation tick for this output: a slideshow's dwell, or a wallpaper
    /// change still easing.
    fn advance(&mut self, now: Instant) {
        let Some(player) = self.player.as_ref() else {
            // Nothing to animate on, and nothing left holding an effect.
            self.anim.forget();
            self.animating = false;
            return;
        };
        self.animating = match self.slideshow.as_mut() {
            Some(s) => advance_slideshow(player, s, &mut self.anim, now),
            None => self.anim.step(player).animating(),
        };
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
        self.confirmed_live = false;
        self.slideshow = None;
        self.anim.forget();
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
        // Re-arm: the cooldown after a give-up has elapsed, so drop the held
        // static frame and give live playback a fresh restart budget. Done by
        // dropping the player rather than clearing `static_fallback` in place
        // — the alive-and-well early return just below would otherwise treat
        // the still-running static frame as healthy and never actually
        // respawn into live playback.
        if self.static_fallback {
            if let Some(at) = self.next_rearm {
                if Instant::now() >= at {
                    log::info!(
                        "[{}] retrying live playback after giving up ({}m ago)",
                        self.connector,
                        RENDERER_REARM_DELAY.as_secs() / 60
                    );
                    self.next_rearm = None;
                    self.restarts = 0;
                    self.static_fallback = false;
                    drop(self.player.take());
                    self.slideshow = None;
                    self.anim.forget();
                    self.animating = false;
                    self.stall_strikes = 0;
                    self.last_pos = None;
                }
            }
        }
        let alive = self.player.as_ref().map(|p| p.is_alive()).unwrap_or(false);
        if alive {
            // What the position sampling saw just before this tick's sample —
            // `check_stall` (below) overwrites `last_pos` with the new sample,
            // so capture the old one first if `presentation_confirmed` needs
            // to compare the two.
            let prev_pos = self.last_pos;
            // A paused or static-fallback frame is not expected to advance — don't
            // sample. Otherwise check for a frozen-but-alive (wedged) renderer.
            let frozen = if self.static_fallback || self.applied_paused.get() {
                self.stall_strikes = 0;
                self.last_pos = None;
                false
            } else {
                self.check_stall()
            };
            if !self.confirmed_live {
                // mpv's IPC answers before mpvpaper has rendered a first
                // frame — which is when mpvpaper actually attaches a buffer
                // to its layer-shell surface and muffin maps it. Confirming
                // on "IPC alive" alone can fire the Cinnamon restack (issue
                // #28) before that surface exists, landing it under
                // Cinnamon's freshly remapped windows again with nothing left
                // to retrigger a fix — so wait for real evidence of
                // presentation instead. Idempotent; the Wayland loop diffs
                // this against the value from before the call to catch the
                // false→true edge.
                let never_advances = self.static_fallback
                    || self.applied_paused.get()
                    || self.player.as_ref().is_some_and(holds_frame_by_design);
                let elapsed = Instant::now().duration_since(self.spawn_at);
                if presentation_confirmed(
                    never_advances,
                    prev_pos,
                    self.last_pos,
                    elapsed,
                    CONFIRM_GRACE,
                ) {
                    self.confirmed_live = true;
                }
            }
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
        } else if let Some(p) = self.player.as_ref() {
            self.last_down = "dead";
            // A death after a successful spawn used to leave `renderer_giveup`
            // with cause=spawn_ok and nothing else — no exit status, no
            // fingerprint — because only a failed *spawn* ever populated
            // `last_spawn_detail`. Reap the same content-free detail here so a
            // runtime death is exactly as legible as a spawn failure.
            if let Some(d) = p.runtime_exit_detail() {
                self.last_spawn_detail = Some(d.to_string());
            }
            match p.stderr_tail() {
                Some(tail) if !tail.is_empty() => log::warn!(
                    "[{}] renderer exited; its last output was:\n{tail}",
                    self.connector
                ),
                _ => {}
            }
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
        if self.no_renderer {
            // Waiting for mpvpaper to be installed. Resolution re-scans on its
            // own (see lib.rs ProbeCache), so this is a stat, not a spawn.
            if crate::mpvpaper_resolved().is_none() {
                return;
            }
            log::info!(
                "[{}] mpvpaper is available now; starting playback",
                self.connector
            );
            self.no_renderer = false;
            self.restarts = 0;
            self.static_fallback = false;
            self.error = None;
        }
        if self.restarts < max {
            self.restarts += 1;
            log::warn!(
                "[{}] renderer down ({}); restarting ({}/{max})",
                self.connector,
                self.last_down,
                self.restarts
            );
            self.respawn(paused, false);
            if self.last_spawn_fail == Some(COMPOSITOR_UNREACHABLE) {
                // mpvpaper could not even open the Wayland display: the
                // session is ending, or the compositor restarted on a new
                // socket. That is the display being away, not the renderer
                // failing — the budget must survive it, exactly as it does
                // when the output-enumeration probe sees the same thing.
                self.restarts -= 1;
                self.park_absent();
            } else if self.last_spawn_fail == Some(MPVPAPER_MISSING) {
                // No renderer binary anywhere. Retrying cannot conjure one, and
                // spending the budget ended in "renderer failed 5×", which hid
                // the cause. Say what to install and wait for it to appear.
                self.restarts -= 1;
                self.no_renderer = true;
                self.error = Some(format!(
                    "{}: {}",
                    self.connector,
                    spawn_fail_hint(MPVPAPER_MISSING)
                ));
                if !self.missing_reported {
                    self.missing_reported = true;
                    crate::telemetry::error(
                        "renderer_missing",
                        &format!("{}: kind={:?}", self.connector, self.wallpaper.kind),
                    );
                }
            }
        } else if self.restarts == max {
            // Crossed the cap once: try to hold a paused static frame, then stop
            // retrying (anti-flap). If even that can't spawn, the compositor's own
            // background shows — Fresco never paints black itself.
            self.restarts += 1; // sentinel — no further attempts
            self.static_fallback = true;
            let why = match self.last_spawn_fail {
                Some(code) => format!(" ({})", spawn_fail_hint(code)),
                None => String::new(),
            };
            self.error = Some(format!(
                "{}: renderer failed {max}×{why} — held a static frame (or fell back to the compositor background)",
                self.connector
            ));
            log::error!(
                "[{}] giving up live playback; attempting a static frame",
                self.connector
            );
            // Once per output per daemon run: telemetry and the desktop
            // notification both said this on *every* failing Apply/restart
            // before this dedupe, which is what turned one stuck renderer
            // into a flood of identical reports.
            if !self.giveup_reported {
                self.giveup_reported = true;
                // Content-free by construction: a connector name, the failure
                // mode, the wallpaper kind, a SpawnFail code, and an
                // ExitDetail (exit status + fingerprint, see its doc comment)
                // — never a path or a file name. Without them a report in the
                // field says only "it failed".
                let mut detail = format!(
                    "{}: renderer failed {max}x (mode={}, kind={:?}, cause={})",
                    self.connector,
                    self.last_down,
                    self.wallpaper.kind,
                    self.last_spawn_fail.unwrap_or("spawn_ok"),
                );
                if let Some(d) = &self.last_spawn_detail {
                    detail.push(' ');
                    detail.push_str(d);
                }
                crate::telemetry::error("renderer_giveup", &detail);
                let hint = self
                    .last_spawn_fail
                    .map(spawn_fail_hint)
                    .unwrap_or_else(|| "the renderer kept failing".to_string());
                notifier::renderer_gave_up(&self.connector, &hint);
            }
            // Try again later rather than staying on the static frame for
            // good — see [`RENDERER_REARM_DELAY`].
            // Causes a retry can't fix (a broken install, a compositor
            // without layer-shell, a file mpv can't load) are not re-armed at
            // all; the rest back off 5, 10, 20, 40, then 80 minutes.
            let permanent = self.last_spawn_fail.is_some_and(|c| {
                c == MPVPAPER_MISSING
                    || c.ends_with(":linker")
                    || c.ends_with(":no_layer_shell")
                    || c.ends_with(":load_failed")
            });
            self.next_rearm = (!permanent).then(|| {
                let delay = RENDERER_REARM_DELAY * 2u32.pow(self.rearms.min(4));
                self.rearms += 1;
                Instant::now() + delay
            });
            self.respawn(true, true);
        }
        // restarts > max → given up; do nothing (anti-flap). Error stays in Status.
    }
}

/// The spawn-failure code for "mpvpaper could not open the Wayland display".
const COMPOSITOR_UNREACHABLE: &str =
    crate::daemon::mpvpaper::EarlyExit::CompositorUnreachable.code();

/// The spawn-failure code for "no mpvpaper binary anywhere".
const MPVPAPER_MISSING: &str = crate::daemon::mpvpaper::SpawnFail::Missing.code();

/// A human line for a spawn-failure code, for the status error a user reads.
fn spawn_fail_hint(code: &str) -> String {
    use crate::daemon::mpvpaper::EarlyExit as E;
    let all = [
        E::CompositorUnreachable,
        E::NoLayerShell,
        E::NoOutput,
        E::Egl,
        E::MpvInit,
        E::MpvGl,
        E::LoadFailed,
        E::Linker,
        E::Signal,
        E::WaylandProtocol,
        E::CleanExit,
        E::Unknown,
    ];
    if let Some(e) = all.iter().find(|e| e.code() == code) {
        return e.hint().to_string();
    }
    match code {
        "mpvpaper_missing" => "no mpvpaper renderer found — install your distro's mpvpaper package (or build it with scripts/build-mpvpaper.sh); playback starts on its own once it is there".into(),
        "mpvpaper_unloadable" => {
            "the bundled renderer cannot load this system's libmpv — run `fresco doctor`".into()
        }
        "ipc_timeout" => "the renderer started but never answered".into(),
        "no_file" => "no playable file".into(),
        other => other.into(),
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

/// Whether a freshly (re)spawned player has produced real evidence that it is
/// actually presenting frames — as opposed to merely answering mpv's IPC
/// socket, which happens before mpvpaper renders (and so attaches a buffer
/// to, and muffin maps) its first frame. Issue #28's Cinnamon restack must
/// only fire once this is true, or it can run before mpvpaper's surface
/// exists and land it under Cinnamon's own windows with nothing left to
/// retrigger a fix.
///
/// `never_advances` is media that legitimately never moves its playback
/// clock — a still image, a video paused at spawn, or the static-fallback
/// frame — for which the only available evidence is time: whether `grace` has
/// elapsed since spawn. Otherwise (moving media), evidence is a playback
/// position that strictly increased between two consecutive supervise
/// samples — reusing the
/// same sampling `check_stall` already does, so this never adds IPC traffic
/// (`prev_pos`/`cur_pos` should be the values immediately before/after that
/// call). A single sample is never enough (`prev_pos` is `None` right after a
/// respawn), so confirmation always needs at least one full supervise
/// interval of real playback.
fn presentation_confirmed(
    never_advances: bool,
    prev_pos: Option<f64>,
    cur_pos: Option<f64>,
    elapsed_since_spawn: Duration,
    grace: Duration,
) -> bool {
    if never_advances {
        elapsed_since_spawn >= grace
    } else {
        matches!((prev_pos, cur_pos), (Some(p), Some(c)) if c > p + 1e-3)
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
    default_wallpaper: &Wallpaper,
    paused: bool,
    lockscreen: LockStatus,
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
    let gave_up: Vec<String> = outputs
        .values()
        .filter(|o| o.static_fallback && o.giveup_reported)
        .map(|o| o.connector.clone())
        .collect();
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
        gave_up,
        lockscreen: Some(lockscreen),
        // The DEFAULT wallpaper, not whichever output sorts first — that one
        // may carry a per-monitor override. The loop's `config.wallpaper` is
        // where a schedule swap lands (and is never saved).
        wallpaper_path: playing_media_path(default_wallpaper),
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
    // KDE Plasma (issue #44): the wallpaper goes through plasmashell, not a
    // window — say which path this session is on and what plasmashell shows.
    if crate::capability::is_kde() {
        println!("KDE Plasma      : {}", kde_desktop::report());
    }

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

    let pm = Pm::detect();
    match hwdecode::probe() {
        HwDecode::Nvdec => println!(
            "NVDEC           : {G}available{X} (NVIDIA GPU with libnvcuvid; Fresco decodes with NVDEC here)"
        ),
        HwDecode::Vainfo => println!("VA-API (vainfo) : {G}available{X}"),
        HwDecode::DriversPresent => println!(
            "VA-API (vainfo) : {G}drivers present{X} (render node and VA driver found; the vainfo diagnostic tool is not installed - {} to verify)",
            install_hint(pm, hwdecode::VAINFO_PKG)
        ),
        HwDecode::Missing => println!(
            "VA-API (vainfo) : {Y}no render node or VA driver found{X} ({})",
            install_hint(pm, hwdecode::DRIVER_PKGS)
        ),
    }

    // The widget helpers. Both fail as *silence* — a widget that is enabled in
    // the config and never draws — so the only place a user can find out is a
    // diagnostic like this one. Yellow, not red: a desktop running no widgets
    // is entirely healthy without either.
    if which("gdbus") {
        println!("MPRIS (gdbus)   : {G}available{X}");
    } else {
        println!("MPRIS (gdbus)   : {Y}not installed{X} ({} — lyrics, album art and the track-synced clock need it)", install_hint(pm, hwdecode::GDBUS_PKG));
    }
    match (which("pw-cat"), which("parec")) {
        (true, _) => println!("Audio capture   : {G}pw-cat{X}"),
        (false, true) => println!("Audio capture   : {G}parec{X}"),
        (false, false) => println!(
            "Audio capture   : {Y}not installed{X} ({} — needed by the audio visualiser widget)",
            install_hint(pm, hwdecode::AUDIO_PKGS)
        ),
    }

    match Config::load() {
        Ok(c) => println!("Config          : {G}valid{X} (enabled={})", c.enabled),
        Err(e) => println!("Config          : {R}invalid{X} ({e})"),
    }
    println!(
        "Log file        : {}",
        dde::state_dir().join("frescod.log").display()
    );

    match crate::ipc::request(&Request::Status) {
        Ok(Response::Status(s)) => {
            println!("Daemon          : {G}running{X}");
            println!("  decode        : {}", decode_display(s.hwdec.as_deref()));
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

/// The `decode` line of `--check`: mpv's `hwdec-current` is the truthful
/// source, so `no` is a real software-decode verdict and is labelled as one.
fn decode_display(hwdec: Option<&str>) -> String {
    match hwdec {
        None => "no wallpaper playing".into(),
        Some("no" | "") => "software (mpv is not using hardware decode)".into(),
        Some(h) => h.into(),
    }
}

/// How long a run loop may wait, given what it wants for itself and what the
/// widget engine says is coming.
///
/// **Smart Sleep, wired up.** `base` is the loop's own cadence — `ANIM_TICK`
/// while a transition is running, `TICK` otherwise — and already accounts for
/// hotplug, stacking and battery, all of which are polled on much coarser
/// intervals and so are satisfied by any wait at or under `base`.
///
/// The widget deadline can therefore only ever **shorten** the wait, never
/// extend it: a lyric that lands 30 ms from now must not sit unpushed for the
/// rest of a 100 ms tick, but a lyric that is 30 s away must not stop the loop
/// checking whether a monitor was unplugged. Anything else would make widgets
/// able to starve the rest of the daemon, which is exactly the class of bug
/// the engine's own docs call the result "advisory" to avoid.
///
/// The floor is a spin guard: `next_deadline` reports "now" for a widget that
/// is due, and a due widget is pushed by the very next `tick()`, but a zero
/// wait on a widget that somehow stayed due would be a busy loop.
/// The earlier of two optional deadlines — `None` only when both are.
fn min_instant(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn widget_wait(base: Duration, deadline: Option<Instant>, now: Instant) -> Duration {
    match deadline {
        Some(d) => base
            .min(d.saturating_duration_since(now))
            .max(MIN_WIDGET_WAIT),
        None => base,
    }
}

/// Send one widget update to a player. Text widgets go through `set_overlay`;
/// a bitmap widget goes through `overlay_add`/`overlay_remove` instead — an
/// empty ASS payload does NOT take a bitmap overlay down, which is why this is
/// a match and not a single call.
///
/// Routing is the caller's: an update carrying a `target` was rasterised
/// against that one output's mode and is wrong anywhere else
/// (`widgets::WidgetUpdate::is_for`).
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

/// Whether the Wayland layer-shell loop should spawn mpvpaper with
/// `MPVPAPER_SHOW_ON_LOCK=1` — see [`WaylandPlayer::spawn`]'s doc comment.
/// Only meaningful for `HostKind::Cosmic { live: true }`, and gated behind
/// the user's own opt-in besides (`[lockscreen].enabled`): unpatched mpvpaper
/// ignores the env var either way, so the only cost of this being `true` on
/// a host/mpvpaper build that doesn't understand it is one inert environment
/// variable on the child process.
fn wants_show_on_lock(config: &Config, host: HostKind) -> bool {
    config.lockscreen.as_ref().is_some_and(|l| l.enabled)
        && matches!(host, HostKind::Cosmic { live: true })
}

/// The palette the lock engine/preview should draw in for `config` — the
/// exact same rule the desktop widget engine's own
/// [`apply_widget_config`]/[`widgets::WidgetEngine::set_config`] use, pulled
/// out so `LockRuntime::lock_preview` and `LockRuntime::begin_lock`'s callers
/// (all three run loops) can never pick a different one for the same config.
fn lock_widget_theme(config: &Config) -> crate::widgetkit::Theme {
    let theme_cfg = config
        .widgets
        .as_ref()
        .map_or_else(crate::config::WidgetTheme::default, |w| w.theme);
    crate::widgetkit::Theme::for_accent(widgets::widget_mode(theme_cfg), config.accent)
}

/// Push every widget setting from `config` into `engine`, in one place so the
/// three loops cannot drift on which widgets they remember to update.
fn apply_widget_config(engine: &mut widgets::WidgetEngine, config: &Config) {
    let w = config.widgets.as_ref();
    engine.set_config(w, config.accent);
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
                ClockThemeCfg::Nos => ClockTheme::Nos,
                ClockThemeCfg::Lock => ClockTheme::Lock,
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

#[cfg(test)]
mod tests {
    use super::{
        decode_display, parse_stat_ticks, presentation_confirmed, stall_step, widget_wait,
        WlOutput, ANIM_TICK, CONFIRM_GRACE, MIN_WIDGET_WAIT, MONITOR_INTERVAL, STALL_STRIKES, TICK,
    };
    use crate::config::{Kind, PowerSaving, Scaling, Wallpaper};
    use std::time::{Duration, Instant};

    #[test]
    fn a_failed_mirror_restacks_everywhere_but_xfce() {
        use super::{caja_mirror::Desktop, dde::Mode, mode_after_mirror_gave_up as after};
        assert_eq!(after(Desktop::Caja), Mode::Restack);
        assert_eq!(after(Desktop::Dde), Mode::Restack);
        // Xfce has no peek and nothing to raise above: the stack stays alone.
        assert_eq!(after(Desktop::Xfce), Mode::Inactive);
    }

    /// Smart Sleep, from the loops' side. The widget engine knows when the next
    /// lyric line or minute boundary is due; the loops know when they next have
    /// to look at monitors, batteries and animation frames. The deadline may
    /// only ever *shorten* the loop's wait.
    #[test]
    fn the_widget_deadline_clamps_the_wait_and_never_extends_it() {
        let now = Instant::now();

        // Nothing pending: the loop keeps its own cadence exactly.
        assert_eq!(widget_wait(TICK, None, now), TICK);
        assert_eq!(widget_wait(ANIM_TICK, None, now), ANIM_TICK);

        // A widget due inside the tick pulls the wait in.
        let soon = now + Duration::from_millis(30);
        assert_eq!(
            widget_wait(TICK, Some(soon), now),
            Duration::from_millis(30)
        );

        // A widget due *after* the tick changes nothing — a 30s instrumental
        // gap must not stop the loop noticing a monitor being unplugged, and
        // hotplug is checked every `MONITOR_INTERVAL` off the same wait.
        let far = now + Duration::from_secs(30);
        assert_eq!(widget_wait(TICK, Some(far), now), TICK);
        assert!(TICK < MONITOR_INTERVAL);

        // Nor may it stretch an animation frame: a transition runs at
        // `ANIM_TICK` and a widget 100ms out must not cost it six frames.
        assert_eq!(widget_wait(ANIM_TICK, Some(now + TICK), now), ANIM_TICK);
        assert!(ANIM_TICK < TICK);

        // A deadline already past waits the floor, not zero: a widget that
        // somehow stayed due must not turn the loop into a spin.
        assert_eq!(widget_wait(TICK, Some(now), now), MIN_WIDGET_WAIT);
        let overdue = now - Duration::from_secs(5);
        assert_eq!(widget_wait(TICK, Some(overdue), now), MIN_WIDGET_WAIT);
        assert!(MIN_WIDGET_WAIT > Duration::ZERO);

        // Whatever the inputs, the result is bounded by the loop's own need.
        for base in [ANIM_TICK, TICK] {
            for ms in [0u64, 1, 5, 16, 17, 99, 100, 101, 30_000] {
                let w = widget_wait(base, Some(now + Duration::from_millis(ms)), now);
                assert!(w <= base, "{base:?} {ms}ms -> {w:?}");
                assert!(w >= MIN_WIDGET_WAIT, "{base:?} {ms}ms -> {w:?}");
            }
        }
    }

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

    /// `presentation_confirmed` (issue #28): mpv's IPC answers before mpvpaper
    /// has actually rendered — and so before its layer-shell surface is
    /// mapped — so "the socket is alive" must never be read as "presenting".
    #[test]
    fn presentation_confirmed_requires_real_evidence() {
        // IPC alive but position still stuck at 0 (mpv hasn't rendered its
        // first frame yet): not confirmed, whatever `elapsed` says — moving
        // media is confirmed by advance, not by time.
        assert!(!presentation_confirmed(
            false,
            Some(0.0),
            Some(0.0),
            Duration::from_secs(60),
            CONFIRM_GRACE
        ));
        // Only one sample so far (fresh respawn) — nothing to compare against.
        assert!(!presentation_confirmed(
            false,
            None,
            Some(0.0),
            Duration::ZERO,
            CONFIRM_GRACE
        ));
        // The position advanced between two consecutive samples: confirmed
        // immediately, regardless of elapsed time.
        assert!(presentation_confirmed(
            false,
            Some(1.0),
            Some(1.5),
            Duration::ZERO,
            CONFIRM_GRACE
        ));
        // A position that merely jitters within tolerance must not confirm.
        assert!(!presentation_confirmed(
            false,
            Some(1.0),
            Some(1.0 + 1e-4),
            Duration::from_secs(60),
            CONFIRM_GRACE
        ));
        // A position that goes backward (e.g. a loop restart) must not confirm.
        assert!(!presentation_confirmed(
            false,
            Some(5.0),
            Some(1.0),
            Duration::from_secs(60),
            CONFIRM_GRACE
        ));

        // Still image / paused-at-spawn / static-fallback: confirmed only
        // after the grace period, never before.
        assert!(!presentation_confirmed(
            true,
            Some(0.0),
            Some(0.0),
            CONFIRM_GRACE - Duration::from_millis(1),
            CONFIRM_GRACE
        ));
        assert!(presentation_confirmed(
            true,
            Some(0.0),
            Some(0.0),
            CONFIRM_GRACE,
            CONFIRM_GRACE
        ));
        // Also confirmed with no position samples at all (the static/paused
        // branch of `supervise` never samples).
        assert!(presentation_confirmed(
            true,
            None,
            None,
            CONFIRM_GRACE + Duration::from_secs(1),
            CONFIRM_GRACE
        ));
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

    /// The slideshow is now a *caller* of the transition machine rather than
    /// its owner, so the seam between the two has to hold: it hands over at the
    /// end of a dwell, keeps its hands off the player until the animation
    /// settles, and only then starts timing the next image.
    #[test]
    fn a_slideshow_hands_its_dwell_to_the_transition_and_takes_it_back() {
        use super::transition::{Anim, Recorder};
        use crate::config::Transition;

        for effect in [
            Transition::Fade,
            Transition::Slide,
            Transition::Zoom,
            Transition::Blur,
        ] {
            let r = Recorder::default();
            let mut anim = Anim::new((0.0, 0.0, 0.0));
            let start = Instant::now();
            let mut s = super::Slideshow {
                images: vec!["/pics/a.png".into(), "/pics/b.png".into()],
                idx: 0,
                interval: Duration::from_secs(10),
                last_advance: start,
                transition: effect,
            };

            // Mid-dwell: nothing happens at all, and nothing is animating.
            let now = start + Duration::from_secs(1);
            assert!(!super::advance_slideshow(&r, &mut s, &mut anim, now));
            assert!(r.loads().is_empty(), "{effect:?} advanced early");

            // The dwell is up: the machine takes over.
            let due = start + Duration::from_secs(10);
            assert!(
                super::advance_slideshow(&r, &mut s, &mut anim, due),
                "{effect:?} must animate once due"
            );
            assert_eq!(s.idx, 1);

            // Run it out. The dwell must not restart until it settles.
            let mut ticks = 0;
            while anim.running() {
                assert!(ticks < 500, "{effect:?} never finished");
                assert_eq!(
                    s.last_advance, start,
                    "{effect:?} restarted the dwell early"
                );
                let t = due + Duration::from_millis(16 * ticks);
                assert!(super::advance_slideshow(&r, &mut s, &mut anim, t));
                ticks += 1;
            }
            assert_ne!(
                s.last_advance, start,
                "{effect:?} never restarted the dwell"
            );
            assert_eq!(r.loads(), vec![std::path::PathBuf::from("/pics/b.png")]);
            assert!(
                r.is_neutral((0.0, 0.0, 0.0)),
                "{effect:?} left the player dirty"
            );
            // And the next image is a full interval away, not instantly due.
            let soon = s.last_advance + Duration::from_secs(1);
            assert!(!super::advance_slideshow(&r, &mut s, &mut anim, soon));
        }
    }

    /// A slideshow with no transition must behave exactly as it always has: a
    /// hard cut that never puts the loop into animation cadence.
    #[test]
    fn a_hard_cut_slideshow_never_animates() {
        use super::transition::{Anim, Recorder};
        use crate::config::Transition;

        let r = Recorder::default();
        let mut anim = Anim::new((0.0, 0.0, 0.0));
        let start = Instant::now();
        let mut s = super::Slideshow {
            images: vec!["/pics/a.png".into(), "/pics/b.png".into()],
            idx: 0,
            interval: Duration::from_secs(10),
            last_advance: start,
            transition: Transition::None,
        };
        let due = start + Duration::from_secs(10);
        assert!(
            !super::advance_slideshow(&r, &mut s, &mut anim, due),
            "a hard cut must not hold the loop at 60fps"
        );
        assert_eq!(s.idx, 1);
        assert_eq!(s.last_advance, due, "the next dwell starts immediately");
        assert_eq!(r.loads(), vec![std::path::PathBuf::from("/pics/b.png")]);
        assert!(!anim.running());

        // A one-image slideshow has nothing to advance to, ever.
        let mut single = super::Slideshow {
            images: vec!["/pics/a.png".into()],
            idx: 0,
            interval: Duration::from_secs(1),
            last_advance: start,
            transition: Transition::Fade,
        };
        assert!(!super::advance_slideshow(
            &r,
            &mut single,
            &mut anim,
            start + Duration::from_secs(60)
        ));
        assert!(!anim.running());
    }

    /// The wallpaper changing on top of a running slideshow transition — an
    /// Apply landing mid-fade. The new wallpaper wins, and none of the old
    /// effect may outlive it: a dimmed or blurred desktop *persists*, which is
    /// strictly worse than never having animated at all.
    #[test]
    fn a_wallpaper_change_mid_transition_leaves_no_effect_behind() {
        use super::transition::{Anim, Recorder};
        use crate::config::Transition;

        for effect in [
            Transition::Fade,
            Transition::Slide,
            Transition::Zoom,
            Transition::Blur,
        ] {
            let r = Recorder::default();
            let base = (0.2, -0.1, 0.05);
            let mut anim = Anim::new(base);
            let start = Instant::now();
            let mut s = super::Slideshow {
                images: vec!["/pics/a.png".into(), "/pics/b.png".into()],
                idx: 0,
                interval: Duration::from_secs(10),
                last_advance: start,
                transition: effect,
            };
            let due = start + Duration::from_secs(10);
            super::advance_slideshow(&r, &mut s, &mut anim, due);
            for i in 0..4 {
                super::advance_slideshow(
                    &r,
                    &mut s,
                    &mut anim,
                    due + Duration::from_millis(16 * i),
                );
            }
            assert!(anim.running(), "staging: {effect:?} is mid-flight");

            // The user picks a video instead. This is the exact call the
            // Wayland apply path makes.
            anim.start(Transition::None, "/videos/new.mp4".into(), &r);
            assert!(
                r.is_neutral(base),
                "{effect:?} survived the change: gamma {} zoom {:?} sigma {}",
                r.gamma(),
                r.zoom(),
                r.sigma()
            );
            assert_eq!(
                r.loads().last(),
                Some(&std::path::PathBuf::from("/videos/new.mp4"))
            );
        }
    }

    #[test]
    fn stat_ticks_survive_weird_comm() {
        // comm may contain spaces and parens; fields count after the LAST ')'.
        let stat = "1234 (my (weird) comm) S 1 1234 1234 0 -1 4194304 500 0 0 0 700 42 0 0 20 0 4 0 100 0 0";
        assert_eq!(parse_stat_ticks(stat), Some(742));
        assert_eq!(parse_stat_ticks(""), None);
        assert_eq!(parse_stat_ticks("no parens here"), None);
    }

    fn slideshow_wallpaper(folder: &std::path::Path, recursive: bool) -> Wallpaper {
        Wallpaper {
            kind: Kind::Slideshow,
            slideshow: Some(crate::config::Slideshow {
                folder: Some(folder.to_path_buf()),
                paths: Vec::new(),
                interval_s: 30,
                recursive,
                transition: Default::default(),
            }),
            ..Wallpaper::default()
        }
    }

    /// Issue #36: a folder of only videos is a slideshow with nothing to show.
    /// It must say so (not "(0 images)") and must be recognised as empty so the
    /// X11 backend does not open a black window for it.
    #[test]
    fn a_video_only_folder_is_an_empty_slideshow() {
        let dir = std::env::temp_dir().join(format!("fresco-daemon-ss-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.mp4"), b"x").unwrap();
        std::fs::write(dir.join("b.mkv"), b"x").unwrap();

        let w = slideshow_wallpaper(&dir, false);
        assert!(super::slideshow_has_no_images(&w));
        assert_eq!(
            super::slideshow_status_label(0),
            "Slideshow (no images found)"
        );
        assert_eq!(super::slideshow_status_label(3), "Slideshow (3 images)");

        // The first image makes it playable again.
        std::fs::write(dir.join("c.png"), b"x").unwrap();
        assert!(!super::slideshow_has_no_images(&w));

        // A non-slideshow wallpaper is never "an empty slideshow".
        assert!(!super::slideshow_has_no_images(&Wallpaper::default()));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A slideshow made with "Include subfolders" plays the subfolders; one
    /// without stays flat, exactly as before the flag existed.
    #[test]
    fn a_recursive_slideshow_scans_subfolders_and_a_flat_one_does_not() {
        let dir = std::env::temp_dir().join(format!("fresco-daemon-rec-{}", std::process::id()));
        let sub = dir.join("2024");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("x.jpg"), b"x").unwrap();

        assert!(super::slideshow_has_no_images(&slideshow_wallpaper(
            &dir, false
        )));
        let deep = slideshow_wallpaper(&dir, true);
        assert!(!super::slideshow_has_no_images(&deep));
        let images = super::slideshow_images(deep.slideshow.as_ref().unwrap());
        assert_eq!(images, vec![sub.join("x.jpg")]);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Points `FRESCO_MPVPAPER` at a throwaway script that prints `body` in
    /// mpvpaper's real coloured `cflp_error()` form on stdout and exits 1.
    /// Caller holds `crate::ENV_LOCK` for as long as the override is set.
    fn write_dying_fake_mpvpaper(tag: &str, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let script = std::env::temp_dir().join(format!(
            "fresco-fake-mpvpaper-{tag}-{}.sh",
            std::process::id()
        ));
        std::fs::write(
            &script,
            format!("#!/bin/sh\nprintf '\\033[1;31m[-] {body}\\033[0m\\n'\nexit 1\n"),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    /// `last_down` used to stay "never_started" through every retry of an
    /// output that had never once come up, however many different ways each
    /// attempt actually failed — the renderer never got a chance to overwrite
    /// it because only the "alive"/"already had a player" paths updated it.
    /// `last_down` keeps the *mode* (never started / dead / frozen) so the
    /// give-up report can still tell "never came up" from "ran, then died";
    /// what the latest attempt actually hit rides in `last_spawn_fail`, which
    /// the report sends as `cause=`.
    #[test]
    fn spawn_failure_cause_is_kept_apart_from_the_mode() {
        let _guard = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let script = write_dying_fake_mpvpaper("last-down", "Failed to initialize EGL, oops");
        std::env::set_var("FRESCO_MPVPAPER", &script);

        let id = std::process::id();
        let media = std::env::temp_dir().join(format!("fresco-last-down-media-{id}.mp4"));
        std::fs::write(&media, b"not really a video").unwrap();

        let mut o = WlOutput::new(
            "DP-1".into(),
            Wallpaper {
                kind: Kind::Video,
                path: Some(media.clone()),
                ..Default::default()
            },
            Scaling::Balanced,
            PowerSaving::Full,
        );
        assert_eq!(o.last_down, "never_started");
        o.supervise(false, 5, true);
        assert_eq!(o.last_down, "never_started", "the mode is kept");
        assert_eq!(
            o.last_spawn_fail,
            Some(crate::daemon::mpvpaper::EarlyExit::Egl.code()),
            "the cause is what this attempt actually hit"
        );

        std::env::remove_var("FRESCO_MPVPAPER");
        let _ = std::fs::remove_file(&script);
        let _ = std::fs::remove_file(&media);
    }

    /// Drives `supervise()` to a give-up and checks the three fixes together:
    /// telemetry (and by extension the desktop notification, gated on the
    /// same latch) fires once and only once for the output's whole run, and
    /// the [`crate::daemon::mpvpaper::ExitDetail`] riding along with it is
    /// content-free — an `exit=`/`sig=` pair, never a path.
    #[test]
    fn renderer_giveup_is_reported_once_with_a_content_free_detail() {
        let _guard = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let script = write_dying_fake_mpvpaper("giveup", "Failed to init mpv, bad option");
        std::env::set_var("FRESCO_MPVPAPER", &script);

        let id = std::process::id();
        let media = std::env::temp_dir().join(format!("fresco-giveup-media-{id}.mp4"));
        std::fs::write(&media, b"not really a video").unwrap();

        const MAX: u32 = 5;
        let mut o = WlOutput::new(
            "DP-1".into(),
            Wallpaper {
                kind: Kind::Video,
                path: Some(media.clone()),
                ..Default::default()
            },
            Scaling::Balanced,
            PowerSaving::Full,
        );
        assert!(!o.giveup_reported);
        // MAX failing restarts, then the tick that crosses the cap.
        for _ in 0..=MAX {
            o.supervise(false, MAX, true);
        }
        assert!(o.static_fallback, "budget exhausted must fall back");
        assert!(o.giveup_reported, "the give-up must be latched");

        let detail = o
            .last_spawn_detail
            .clone()
            .expect("a failed spawn must leave an ExitDetail behind");
        assert!(detail.contains("exit="), "{detail}");
        assert!(detail.contains("sig="), "{detail}");
        assert!(!detail.contains('/'), "content-free: {detail}");

        // Further ticks (still failing) must not flip the latch again — one
        // report per output per daemon run, not one per failing tick.
        for _ in 0..3 {
            o.supervise(false, MAX, true);
        }
        assert!(o.giveup_reported);

        std::env::remove_var("FRESCO_MPVPAPER");
        let _ = std::fs::remove_file(&script);
        let _ = std::fs::remove_file(&media);
    }

    fn have(bin: &str) -> bool {
        std::process::Command::new("which")
            .arg(bin)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Fake mpvpaper wrapping a real headless mpv, exactly like
    /// [`crate::daemon::mpvpaper`]'s own `FAKE_MPVPAPER` fixture — an mpv
    /// spawned with `--idle=yes` stays alive (and answers IPC) even though the
    /// "image" it's pointed at is not a real one, which is all a rapid-Set
    /// test needs: a genuinely live, genuinely supervised process.
    const FAKE_MPVPAPER_IMAGE: &str = "#!/bin/sh\n\
opts=\"$2\"\n\
file=\"$4\"\n\
sock=\"\"\n\
for tok in $opts; do\n\
  case \"$tok\" in\n\
    input-ipc-server=*) sock=\"${tok#input-ipc-server=}\" ;;\n\
  esac\n\
done\n\
exec mpv --idle=yes --vo=null --ao=null --no-config --no-terminal --really-quiet --input-ipc-server=\"$sock\" --image-display-duration=inf --loop-file=inf \"$file\"\n";

    /// Reproduction attempt for the production race: a burst of GUI `Set`
    /// clicks on an already-live image wallpaper, 6s and then 3s apart (the
    /// telemetry's cadence), each going through `apply_wallpaper` exactly as
    /// the Wayland IPC loop's `Request::Apply` handler does.
    ///
    /// `apply_wallpaper` takes the `media_only` branch for every one of these
    /// (same kind/fit/rotation/mute/volume/crop/scaling/power-saving, just a
    /// new path) — which only sends an IPC `loadfile replace` and never calls
    /// `respawn`, never kills the child, and never touches `restarts`. If the
    /// telemetry's 5 counted failures were the daemon counting its own kills
    /// or replacements, this rapid-Set burst — interleaved with `supervise`
    /// ticks exactly as the real 2s SUPERVISE cadence would run them — must
    /// grow `restarts` and/or replace the child pid. It does neither: the
    /// process spawned once at the top survives every Set untouched, and
    /// `restarts` never leaves 0. That rules the hypothesis out for the
    /// image/image, same-settings case telemetry showed (`kind=Image`,
    /// `cause=spawn_ok`) — the counted failures must have been the child
    /// actually exiting on its own between supervise ticks, not the daemon
    /// mistaking its own actions for deaths.
    #[test]
    fn rapid_sets_do_not_inflate_restarts_or_replace_the_child() {
        use std::os::unix::fs::PermissionsExt;
        if !have("mpv") {
            eprintln!(
                "skip rapid_sets_do_not_inflate_restarts_or_replace_the_child: mpv not installed"
            );
            return;
        }
        let _guard = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let id = std::process::id();
        let fake = std::env::temp_dir().join(format!("fresco-fake-mpvpaper-rapid-{id}.sh"));
        std::fs::write(&fake, FAKE_MPVPAPER_IMAGE).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("FRESCO_MPVPAPER", &fake);

        let img_a = std::env::temp_dir().join(format!("fresco-rapid-a-{id}.png"));
        let img_b = std::env::temp_dir().join(format!("fresco-rapid-b-{id}.png"));
        std::fs::write(&img_a, b"not really a png").unwrap();
        std::fs::write(&img_b, b"also not really a png").unwrap();

        let mut o = WlOutput::new(
            "DP-1".into(),
            Wallpaper {
                kind: Kind::Image,
                path: Some(img_a.clone()),
                ..Default::default()
            },
            Scaling::Balanced,
            PowerSaving::Full,
        );
        o.respawn(false, false);
        assert!(
            o.player.as_ref().is_some_and(|p| p.is_alive()),
            "the fake backend must come up for the test to mean anything"
        );
        let pid = o.player.as_ref().and_then(|p| p.child_pid());
        assert!(pid.is_some(), "must be able to observe the child's pid");

        const MAX: u32 = 5;
        // GUI Set, GUI Set 6s later, a supervise tick, then two more Sets 3s
        // apart with a tick after each — the telemetry's exact shape, minus
        // the wall-clock sleeps (nothing here is time-based).
        let mut toggle = false;
        for _ in 0..6 {
            toggle = !toggle;
            let next = if toggle { img_b.clone() } else { img_a.clone() };
            o.apply_wallpaper(
                Wallpaper {
                    kind: Kind::Image,
                    path: Some(next),
                    ..Default::default()
                },
                Scaling::Balanced,
                PowerSaving::Full,
                false,
            );
            assert_eq!(
                o.restarts, 0,
                "a media-only Set must never spend the restart budget"
            );
            o.supervise(false, MAX, true);
            assert_eq!(
                o.restarts, 0,
                "supervising a still-alive, self-swapped player must not count a death"
            );
        }
        assert!(
            o.player.as_ref().is_some_and(|p| p.is_alive()),
            "the same process must still be running after the whole burst"
        );
        assert_eq!(
            o.player.as_ref().and_then(|p| p.child_pid()),
            pid,
            "rapid Sets must reuse the child, never kill and replace it"
        );
        assert!(!o.giveup_reported);
        assert!(!o.static_fallback);

        std::env::remove_var("FRESCO_MPVPAPER");
        let _ = std::fs::remove_file(&fake);
        let _ = std::fs::remove_file(&img_a);
        let _ = std::fs::remove_file(&img_b);
    }

    /// Issue #41: mpv's `no` is a real software verdict, whatever tools are
    /// installed, and no player is not a verdict at all.
    #[test]
    fn decode_label_reports_what_mpv_says() {
        assert_eq!(decode_display(Some("vaapi")), "vaapi");
        assert_eq!(
            decode_display(Some("no")),
            "software (mpv is not using hardware decode)"
        );
        assert_eq!(decode_display(None), "no wallpaper playing");
    }
}

#[cfg(test)]
mod sched_clock_jump_tests {
    use super::*;
    use crate::config::{Kind, Schedule, ScheduleMode};
    use chrono::NaiveDate;

    fn wp(path: &str) -> Wallpaper {
        Wallpaper {
            kind: Kind::Video,
            path: Some(PathBuf::from(path)),
            ..Default::default()
        }
    }
    fn schedule() -> Schedule {
        Schedule {
            mode: ScheduleMode::Daynight,
            day: Some(wp("/day.mp4")),
            night: Some(wp("/night.mp4")),
            day_start: "07:00".into(),
            night_start: "17:16".into(),
            lat: None,
            lon: None,
            at: vec![],
        }
    }
    /// What the scheduler wants when the wall clock reads `h:m` (the daemon's
    /// `due` feeds exactly this into `due_for`).
    fn want_at(s: &Schedule, h: u32, m: u32) -> Wallpaper {
        let now = NaiveDate::from_ymd_opt(2026, 9, 30)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap();
        crate::schedule::desired(s, now, 0).unwrap().clone()
    }
    fn cfg_showing(path: &str) -> Config {
        Config {
            wallpaper: wp(path),
            ..Default::default()
        }
    }

    #[test]
    fn resume_across_boundary_switches_once() {
        let s = schedule();
        let cfg = cfg_showing("/day.mp4");
        let mut st = SchedState::default();
        assert!(st.due_for(&cfg, want_at(&s, 16, 0)).is_none());
        // Suspend at 16:00, wake at 17:20: the boundary was missed entirely.
        let got = st.due_for(&cfg, want_at(&s, 17, 20)).unwrap();
        assert_eq!(got.effective_path().unwrap().to_str(), Some("/night.mp4"));
        st.applied = Some(PathBuf::from("/night.mp4"));
        assert!(st.due_for(&cfg, want_at(&s, 17, 21)).is_none());
    }

    #[test]
    fn hold_expires_when_a_jump_crosses_the_boundary() {
        let s = schedule();
        // User manually applied /other.mp4 during the day slot: hold = day.
        let cfg = cfg_showing("/other.mp4");
        let mut st = SchedState {
            applied: None,
            hold: Some(PathBuf::from("/day.mp4")),
            warned_no_path: false,
        };
        assert!(st.due_for(&cfg, want_at(&s, 12, 0)).is_none());
        assert!(st.hold.is_some());
        let got = st.due_for(&cfg, want_at(&s, 20, 0)).unwrap();
        assert_eq!(got.effective_path().unwrap().to_str(), Some("/night.mp4"));
        assert!(st.hold.is_none());
    }

    #[test]
    fn clock_set_backwards_switches_back() {
        let s = schedule();
        let cfg = cfg_showing("/night.mp4");
        let mut st = SchedState {
            applied: Some(PathBuf::from("/night.mp4")),
            hold: None,
            warned_no_path: false,
        };
        let got = st.due_for(&cfg, want_at(&s, 9, 0)).unwrap();
        assert_eq!(got.effective_path().unwrap().to_str(), Some("/day.mp4"));
    }

    #[test]
    fn wallpaper_without_a_path_is_skipped() {
        let cfg = cfg_showing("/day.mp4");
        let mut st = SchedState::default();
        assert!(st.due_for(&cfg, Wallpaper::default()).is_none());
    }

    #[test]
    fn pathless_slot_warns_once_until_a_slot_with_a_path() {
        let cfg = cfg_showing("/day.mp4");
        let mut st = SchedState::default();
        assert!(!st.warned_no_path);
        assert!(st.due_for(&cfg, Wallpaper::default()).is_none());
        assert!(st.warned_no_path); // first tick took the warn path
        assert!(st.due_for(&cfg, Wallpaper::default()).is_none());
        assert!(st.warned_no_path); // later ticks stay quiet (flag unchanged)
        st.due_for(&cfg, wp("/day.mp4"));
        assert!(!st.warned_no_path); // re-armed by a slot with a path
    }

    /// A one-slot `times` schedule wants `/day.mp4` at every hour of the day,
    /// so `hold_current` — which reads the real wall clock — is deterministic.
    fn always_day_config(showing: &str) -> Config {
        Config {
            wallpaper: wp(showing),
            schedule: Some(Schedule {
                mode: ScheduleMode::Times,
                day: None,
                night: None,
                day_start: "07:00".into(),
                night_start: "19:00".into(),
                lat: None,
                lon: None,
                at: vec![crate::config::TimeSlot {
                    time: "00:00".into(),
                    wallpaper: wp("/day.mp4"),
                }],
            }),
            ..Default::default()
        }
    }

    /// Resuming a paused schedule: the GUI points `config.wallpaper` at the
    /// slot the schedule wants (`sync_wallpaper_to_schedule`) before applying,
    /// so the daemon must treat that as "schedule live now", not as a manual
    /// override to hold until the next boundary.
    #[test]
    fn hold_current_does_not_hold_after_a_resume_sync() {
        let cfg = always_day_config("/day.mp4");
        let mut st = SchedState::default();
        st.hold_current(&cfg);
        assert!(st.hold.is_none());
        assert!(st.applied.is_none());
    }

    /// The failure the resume sync prevents: a stale wallpaper that differs
    /// from the slot is an explicit user choice and is held.
    #[test]
    fn hold_current_holds_a_wallpaper_that_differs_from_the_slot() {
        let cfg = always_day_config("/stale.mp4");
        let mut st = SchedState::default();
        st.hold_current(&cfg);
        assert_eq!(st.hold.as_deref(), Some(std::path::Path::new("/day.mp4")));
    }

    #[test]
    fn playing_media_path_names_single_files_only() {
        assert_eq!(
            playing_media_path(&wp("/day.mp4")),
            Some(PathBuf::from("/day.mp4"))
        );
        let image = Wallpaper {
            kind: Kind::Image,
            path: Some(PathBuf::from("/a.png")),
            ..Default::default()
        };
        assert_eq!(playing_media_path(&image), Some(PathBuf::from("/a.png")));
        // A playlist's first item is not necessarily what is playing.
        let playlist = Wallpaper {
            kind: Kind::Playlist,
            paths: vec![PathBuf::from("/a.mp4"), PathBuf::from("/b.mp4")],
            ..Default::default()
        };
        assert_eq!(playing_media_path(&playlist), None);
        let slideshow = Wallpaper {
            kind: Kind::Slideshow,
            ..Default::default()
        };
        assert_eq!(playing_media_path(&slideshow), None);
        // A schedule swap only rewrites `path`, so it wins over a stale list.
        let swapped = Wallpaper {
            kind: Kind::Video,
            path: Some(PathBuf::from("/night.mp4")),
            paths: vec![PathBuf::from("/day.mp4")],
            ..Default::default()
        };
        assert_eq!(
            playing_media_path(&swapped),
            Some(PathBuf::from("/night.mp4"))
        );
    }
}

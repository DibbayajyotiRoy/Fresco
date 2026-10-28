//! Live "now playing" status pill + pause/resume toggle, backed by
//! `ipc::request(&Request::Status)`.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{glib, glib::ControlFlow};

use crate::config::{Config, Wallpaper};
use crate::ipc::{self, MonitorInfo, PauseReason, Request, StatusReply};
use crate::{t, tf};

use super::daemon_ctl;
use super::lockscreen::{lock_support_of, LockSupport};

thread_local! {
    /// Last `monitors_info` this poll loop saw, so other GUI code (e.g. the
    /// card menu's "move to display" list) can read connected displays
    /// without its own blocking IPC round trip — see `cached_monitors`.
    static LAST_MONITORS: RefCell<Vec<MonitorInfo>> = const { RefCell::new(Vec::new()) };
}

/// Connected displays as of the last status poll. Empty before the first
/// poll lands, or while the daemon isn't running. Never blocks: it's a plain
/// read of state `poll_once` already fetched in the background.
pub(crate) fn cached_monitors() -> Vec<MonitorInfo> {
    LAST_MONITORS.with(|m| m.borrow().clone())
}

thread_local! {
    /// Media file the daemon last said the default wallpaper is playing
    /// (`StatusReply::wallpaper_path`). A day/night swap changes this without
    /// touching config.toml, so it is the only place the GUI can learn which
    /// card is really on screen — see `cached_playing_path`.
    static LAST_PLAYING: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// Redraws the library when `LAST_PLAYING` changes; installed once by the
    /// library view (mirrors `NOTICE_HOOK`).
    static PLAYING_HOOK: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    /// Library name for a playing file, installed by the library view (it owns
    /// the entries). Unset until then, so the pill falls back to the file name.
    static NAME_LOOKUP: RefCell<Option<NameLookup>> = const { RefCell::new(None) };
    /// The pill the poll loop feeds, kept so [`poll_soon`] can poll on demand.
    static POLL_WIDGETS: RefCell<Option<Rc<PillWidgets>>> = const { RefCell::new(None) };
    /// What the last status poll said reaches this desktop's real lock screen,
    /// for the app menu's "Lock Screen…" row — see `cached_lock_support`.
    static LAST_LOCK_SUPPORT: Cell<Option<LockSupport>> = const { Cell::new(None) };
}

thread_local! {
    /// Whether the last status poll found the daemon running. `None` before the
    /// first poll lands. The library's "active" card is judged by this as well
    /// as by `enabled`: after the tray's "Quit" the config still says on, but
    /// nothing is playing (#98) — see `cached_daemon_running`.
    static LAST_RUNNING: Cell<Option<bool>> = const { Cell::new(None) };
    /// Redraws the library when `LAST_RUNNING` changes; installed once by the
    /// library view.
    static RUNNING_HOOK: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Whether the daemon answered the last status poll; `None` until one lands.
pub(crate) fn cached_daemon_running() -> Option<bool> {
    LAST_RUNNING.with(Cell::get)
}

/// Register the callback fired when [`cached_daemon_running`] changes in a way
/// the library must show (not for the first poll finding the daemon up, which
/// is what the library already assumes).
pub(crate) fn on_daemon_running_changed(hook: impl Fn() + 'static) {
    RUNNING_HOOK.with(|h| *h.borrow_mut() = Some(Rc::new(hook)));
}

/// Cache a poll's verdict; true when the library's picture of it changes.
fn set_cached_daemon_running(running: bool) -> bool {
    // Until the first poll the library draws from config.toml alone, which is
    // the same picture as "running".
    LAST_RUNNING
        .with(|r| r.replace(Some(running)))
        .unwrap_or(true)
        != running
}

fn note_daemon_running(running: bool) {
    if set_cached_daemon_running(running) {
        let hook = RUNNING_HOOK.with(|h| h.borrow().clone());
        if let Some(hook) = hook {
            hook();
        }
    }
}

/// Resolves the playing file to the name its library card shows.
type NameLookup = Rc<dyn Fn(&Path) -> Option<String>>;

/// Install the lookup the pill uses to show a wallpaper's library name (what the
/// user renamed it to) instead of its file name.
pub(crate) fn set_name_lookup(f: impl Fn(&Path) -> Option<String> + 'static) {
    NAME_LOOKUP.with(|l| *l.borrow_mut() = Some(Rc::new(f)));
}

/// How long [`poll_soon`] waits: long enough for the daemon to have finished
/// the apply that prompted it, short enough to feel immediate.
const POLL_SOON_MS: u64 = 500;

/// Poll the daemon shortly, instead of at the next 4 s tick. Call after the GUI
/// changed what plays (see [`forget_playing_path`]) so the pill and the library
/// learn the new state promptly. A no-op before the pill exists.
pub(crate) fn poll_soon() {
    let Some(widgets) = POLL_WIDGETS.with(|w| w.borrow().clone()) else {
        return;
    };
    glib::timeout_add_local_once(Duration::from_millis(POLL_SOON_MS), move || {
        poll_once(widgets);
    });
}

/// What the daemon reported as playing on the last status poll. `None` before
/// the first poll lands, while the daemon isn't running, or when it has no
/// single file to name (playlist/slideshow, or an older daemon). Never blocks.
pub(crate) fn cached_playing_path() -> Option<PathBuf> {
    LAST_PLAYING.with(|p| p.borrow().clone())
}

/// Drop the cached playing path *without* running the hook, because the GUI is
/// about to change which wallpaper plays (a Set, a schedule being switched on)
/// and the cache describes the one it is replacing.
///
/// The cache is only refreshed by the 4 s status poll. Left alone after a Set,
/// it would keep naming the old file for up to that long: the library would
/// highlight the old card for a schedule's sake, and an Off or Pause in that
/// window would adopt the old wallpaper as "what is playing" and re-apply it,
/// flipping the screen back. Unknown (`None`) is the safe answer meanwhile —
/// callers then fall back to the config / the schedule's clock — and the caller
/// redraws on its own, so no hook is needed. Follow with [`poll_soon`] to
/// learn the truth sooner than the next tick.
pub(crate) fn forget_playing_path() {
    LAST_PLAYING.with(|p| *p.borrow_mut() = None);
}

/// Register the callback fired when [`cached_playing_path`] changes — and only
/// then, so a 4 s poll that sees the same wallpaper never rebuilds the grid.
pub(crate) fn on_playing_path_changed(hook: impl Fn() + 'static) {
    PLAYING_HOOK.with(|h| *h.borrow_mut() = Some(Rc::new(hook)));
}

/// Store the polled path; true when it differs from the previous one.
fn set_cached_playing_path(path: Option<PathBuf>) -> bool {
    LAST_PLAYING.with(|p| {
        let mut slot = p.borrow_mut();
        if *slot == path {
            false
        } else {
            *slot = path;
            true
        }
    })
}

/// Cache a poll's result and tell the library only if it moved. The hook is
/// cloned out before it runs: it re-renders the grid, which reads the cache.
fn note_playing_path(path: Option<PathBuf>) {
    if set_cached_playing_path(path) {
        let hook = PLAYING_HOOK.with(|h| h.borrow().clone());
        if let Some(hook) = hook {
            hook();
        }
    }
}

/// What reaches the real lock screen on this desktop, as of the last status
/// poll. `None` before the first poll lands, while the daemon isn't running,
/// or when it predates the capability data. Never blocks, like
/// [`cached_monitors`].
pub(crate) fn cached_lock_support() -> Option<LockSupport> {
    LAST_LOCK_SUPPORT.with(Cell::get)
}

/// Callback that shows/hides the service notice from a reachability result.
type NoticeHook = Rc<dyn Fn(bool)>;

thread_local! {
    /// Updates the "service not running" notice from a reachability result.
    /// Set once by `build_service_notice`; the status poll and
    /// `refresh_service_notice` feed it.
    static NOTICE_HOOK: RefCell<Option<NoticeHook>> = const { RefCell::new(None) };
}

fn notify_reachable(reachable: bool) {
    let hook = NOTICE_HOOK.with(|h| h.borrow().clone());
    if let Some(hook) = hook {
        hook(reachable);
    }
}

/// Ask whether the daemon is alive without blocking the GTK thread
/// (`daemon_alive` is a blocking IPC round trip); `done` runs on the GTK
/// thread with the answer.
pub(crate) fn probe_daemon(done: impl FnOnce(bool) + 'static) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(ipc::daemon_alive());
    });
    glib::spawn_future_local(async move {
        if let Ok(alive) = rx.recv().await {
            done(alive);
        }
    });
}

/// Re-evaluate the main-window notice now (after the schedule was edited or
/// the service was started) instead of waiting for the next status poll.
pub(crate) fn refresh_service_notice() {
    probe_daemon(notify_reachable);
}

/// Compact dismissible "service not running" notice for the main window.
/// `schedule_active` says whether a schedule is enabled and not paused (the
/// only case where a dead daemon silently breaks something); `start` runs the
/// Start button's action. Deliberately no auto-start: the user may have
/// pressed Stop on purpose. Not an `adw::Banner` (needs libadwaita 1.3).
pub(crate) fn build_service_notice(
    schedule_active: Rc<dyn Fn() -> bool>,
    start: Rc<dyn Fn()>,
) -> gtk4::Widget {
    let revealer = gtk4::Revealer::new();
    revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.add_css_class("capability-banner");
    row.set_margin_start(12);
    row.set_margin_end(12);
    row.set_margin_top(10);
    let icon = gtk4::Image::from_icon_name("dialog-warning-symbolic");
    icon.set_valign(gtk4::Align::Center);
    let label = gtk4::Label::new(Some(t!(
        "Fresco's background service isn't running, so the schedule can't switch wallpapers."
    )));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let start_btn = gtk4::Button::with_label(t!("Start"));
    start_btn.set_valign(gtk4::Align::Center);
    let dismiss = gtk4::Button::from_icon_name("window-close-symbolic");
    dismiss.add_css_class("flat");
    dismiss.set_valign(gtk4::Align::Center);
    row.append(&icon);
    row.append(&label);
    row.append(&start_btn);
    row.append(&dismiss);
    revealer.set_child(Some(&row));

    // Dismissal lasts until the daemon has been seen running again, so it
    // does not nag every 4 s poll but does return after the next real outage.
    let dismissed = Rc::new(Cell::new(false));
    {
        let dismissed = dismissed.clone();
        let revealer = revealer.clone();
        dismiss.connect_clicked(move |_| {
            dismissed.set(true);
            revealer.set_reveal_child(false);
        });
    }
    start_btn.connect_clicked(move |_| start());
    let hook: Rc<dyn Fn(bool)> = {
        let revealer = revealer.clone();
        Rc::new(move |reachable| {
            if reachable {
                dismissed.set(false);
            }
            revealer.set_reveal_child(!reachable && !dismissed.get() && schedule_active());
        })
    };
    NOTICE_HOOK.with(|h| *h.borrow_mut() = Some(hook));
    revealer.upcast()
}

/// How often to poll the daemon while the window is open. A live status
/// surface doesn't need sub-second freshness, and this keeps the background
/// thread churn low.
const POLL_INTERVAL_S: u32 = 4;

/// Widgets the polling loop updates in place.
struct PillWidgets {
    dot: gtk4::Label,
    /// Tiny "PLAYING" / "PAUSED" overline above the name.
    overline: gtk4::Label,
    /// Prettified wallpaper name (CPU% lives in the tooltip).
    label: gtk4::Label,
    /// Why playback is held ("On battery"…); hidden while nothing holds it.
    reason: gtk4::Label,
    hwdec: gtk4::Label,
    toggle: gtk4::Button,
    pill: gtk4::Box,
    /// The toggle currently means "Start" (the daemon is gone but the config
    /// wants it), not Pause/Resume.
    starts: Cell<bool>,
    /// This session only ever paints a still frame (GNOME Wayland): the daemon
    /// reports "running" for it, but nothing is playing and Pause/Resume are
    /// no-ops, so the pill must not say PLAYING or offer a pause button.
    /// Derived from the session capability once at build — `StatusReply`
    /// cannot say it, and no IPC change is needed.
    still_frame: bool,
}

/// Build the status pill (dot + wallpaper name + hwdec badge + CPU% + a
/// pause/resume toggle) and start polling the daemon in the background.
/// Returns the root widget to place in the header.
pub fn build_status_pill() -> gtk4::Widget {
    let pill = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    pill.add_css_class("status-pill");

    let dot = gtk4::Label::new(Some("●"));
    dot.add_css_class("dot-off");
    pill.append(&dot);

    // Two stacked lines: a tiny state overline + the wallpaper name.
    let text_col = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    text_col.set_valign(gtk4::Align::Center);
    let overline = gtk4::Label::new(None);
    overline.add_css_class("pill-overline");
    overline.set_xalign(0.0);
    overline.set_visible(false);
    text_col.append(&overline);
    let label = gtk4::Label::new(Some(t!("Not running")));
    label.add_css_class("pill-name");
    label.set_xalign(0.0);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_max_width_chars(24);
    text_col.append(&label);
    pill.append(&text_col);

    let reason = gtk4::Label::new(None);
    reason.add_css_class("dim");
    reason.set_visible(false);
    pill.append(&reason);

    let hwdec = gtk4::Label::new(None);
    hwdec.add_css_class("dim");
    hwdec.set_visible(false);
    pill.append(&hwdec);

    let toggle = gtk4::Button::from_icon_name("media-playback-pause-symbolic");
    toggle.add_css_class("flat");
    toggle.set_tooltip_text(Some(t!("Pause")));
    toggle.set_visible(false);
    pill.append(&toggle);

    let widgets = Rc::new(PillWidgets {
        dot,
        overline,
        label,
        reason,
        hwdec,
        toggle: toggle.clone(),
        pill: pill.clone(),
        starts: Cell::new(false),
        still_frame: crate::capability::detect()
            == crate::capability::Capability::WaylandGnomeStatic,
    });

    {
        let widgets = widgets.clone();
        toggle.connect_clicked(move |btn| {
            if widgets.starts.get() {
                start_now();
                return;
            }
            let paused = btn.icon_name().as_deref() == Some("media-playback-start-symbolic");
            let req = if paused {
                Request::Resume
            } else {
                Request::Pause
            };
            send_fire_and_forget(req);
            // Re-poll shortly after so the pill reflects the new state without
            // waiting a full interval.
            let widgets = widgets.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || {
                poll_once(widgets);
            });
        });
    }

    POLL_WIDGETS.with(|w| *w.borrow_mut() = Some(widgets.clone()));
    poll_once(widgets.clone());
    // Runs for the life of the process: the single window (build_ui guards
    // against duplicates) closing quits the app, taking the timer with it.
    glib::timeout_add_local(Duration::from_secs(POLL_INTERVAL_S as u64), move || {
        poll_once(widgets.clone());
        ControlFlow::Continue
    });

    pill.upcast()
}

/// Fetch `Request::Status` on a background thread and apply the result to the
/// pill once it lands back on the main thread. Mirrors the
/// thread + `async_channel` + `glib::spawn_future_local` pattern used by
/// `poll_notifications` / `check_for_updates` — never blocks the GTK thread.
fn poll_once(widgets: Rc<PillWidgets>) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = ipc::request(&Request::Status);
        // What config.toml says matters only when nothing is answering.
        let off = match &result {
            Ok(crate::ipc::Response::Status(s)) if s.running => Off::Idle,
            _ => Config::load().map_or(Off::Idle, |c| off_state(&c)),
        };
        // A daemon that runs with the desktop wallpaper off runs for the lock
        // screen alone (it exits when neither is wanted).
        let lock_only = matches!(&result, Ok(crate::ipc::Response::Status(s)) if s.running)
            && Config::load().is_ok_and(|c| !c.enabled);
        let _ = tx.send_blocking((result, off, lock_only));
    });

    glib::spawn_future_local(async move {
        let Ok((result, off, lock_only)) = rx.recv().await else {
            return;
        };
        match result {
            Ok(crate::ipc::Response::Status(status)) => {
                note_daemon_running(status.running);
                LAST_MONITORS.with(|m| *m.borrow_mut() = status.monitors_info.clone());
                // Nothing plays on the desktop then, whatever the config names.
                note_playing_path(status.wallpaper_path.clone().filter(|_| !lock_only));
                LAST_LOCK_SUPPORT
                    .with(|c| c.set(status.lockscreen.as_ref().and_then(lock_support_of)));
                apply_status(&widgets, &status, off, lock_only);
                notify_reachable(status.running);
            }
            Ok(_) => {}
            Err(e) => {
                // Connected but no answer in time: busy, not gone. Keep the
                // last picture rather than flip the pill and redraw the library.
                if timed_out(&e) {
                    log::debug!("status poll: daemon slow to answer: {e:#}");
                    return;
                }
                // Daemon not running — expected and common, not an error.
                log::debug!("status poll: daemon unreachable: {e:#}");
                note_daemon_running(false);
                LAST_MONITORS.with(|m| m.borrow_mut().clear());
                note_playing_path(None);
                LAST_LOCK_SUPPORT.with(|c| c.set(None));
                apply_off(&widgets, off);
                notify_reachable(false);
            }
        }
    });
}

/// Whether a request failed by reaching the daemon but timing out waiting on it
/// (`ipc::request_at` sets a read/write timeout), as opposed to finding no daemon.
fn timed_out(e: &anyhow::Error) -> bool {
    e.chain()
        .filter_map(|c| c.downcast_ref::<std::io::Error>())
        .any(|io| {
            matches!(
                io.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            )
        })
}

/// Fire a Pause/Resume request on a background thread; the result isn't
/// awaited (the next poll picks up the new state), matching the plan's
/// "fire-and-forget is fine" guidance for the toggle.
fn send_fire_and_forget(req: Request) {
    std::thread::spawn(move || {
        if let Err(e) = ipc::request(&req) {
            log::warn!("pause/resume request failed: {e:#}");
        }
    });
}

/// What the saved config says about a daemon that is not running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Off {
    /// Stopped on purpose (`enabled = false`), or never set up.
    Idle,
    /// `enabled = true` yet no daemon: the tray's "Quit", or a crash. Start
    /// brings it back now; `resumes_at_login` says the autostart entry will too.
    Wanted { resumes_at_login: bool },
}

fn off_state(cfg: &Config) -> Off {
    // `enabled` is on by default: with nothing to play (a fresh install) there
    // is nothing to resume or start.
    let configured =
        cfg.wallpaper.is_configured() || cfg.monitors.values().any(Wallpaper::is_configured);
    // With the desktop wallpaper off, only the lock screen can want the daemon.
    if (cfg.enabled && configured) || (!cfg.enabled && cfg.daemon_wanted()) {
        Off::Wanted {
            resumes_at_login: cfg.autostart,
        }
    } else {
        Off::Idle
    }
}

/// The pill's overline for a stopped daemon; only a wallpaper that login will
/// bring back says so (#98: it must not read as playing, nor as gone for good).
fn off_overline(off: Off) -> Option<&'static str> {
    matches!(
        off,
        Off::Wanted {
            resumes_at_login: true
        }
    )
    .then(|| t!("RESUMES AT NEXT LOGIN"))
}

/// The pill's Start button: apply the saved config, which starts the daemon.
fn start_now() {
    // `apply_async` saves what it is given, so never hand it a default made
    // from a config that failed to load.
    if let Ok(cfg) = Config::load() {
        daemon_ctl::apply_async(&cfg, |outcome| {
            if let Err(e) = outcome.result {
                log::warn!("start from the status pill failed: {e}");
            }
            poll_soon();
        });
    }
}

fn apply_off(w: &PillWidgets, off: Off) {
    w.dot.remove_css_class("dot-ok");
    w.dot.remove_css_class("dot-warn");
    w.dot.add_css_class("dot-off");
    match off_overline(off) {
        Some(text) => {
            w.overline.set_label(text);
            w.overline.set_visible(true);
        }
        None => w.overline.set_visible(false),
    }
    w.label.set_label(t!("Not running"));
    w.reason.set_visible(false);
    w.hwdec.set_visible(false);
    let wanted = off != Off::Idle;
    w.starts.set(wanted);
    w.toggle.set_visible(wanted);
    if wanted {
        w.toggle.set_icon_name("media-playback-start-symbolic");
        w.toggle.set_tooltip_text(Some(t!("Start")));
    }
    w.pill.set_tooltip_text(None);
}

fn apply_status(w: &PillWidgets, status: &StatusReply, off: Off, lock_only: bool) {
    if !status.running {
        apply_off(w, off);
        return;
    }
    w.starts.set(false);

    let warn = status.paused || status.error.is_some();
    w.dot.remove_css_class("dot-ok");
    w.dot.remove_css_class("dot-warn");
    w.dot.remove_css_class("dot-off");
    w.dot
        .add_css_class(if warn { "dot-warn" } else { "dot-ok" });

    // Presentation-only restyle: overline state + prettified name in the pill,
    // CPU% relegated to the tooltip (with any daemon error). A renderer that
    // gave up on live playback and is holding a paused static frame (see
    // `daemon::WlOutput::supervise`) must not say PLAYING — the dot already
    // turns amber for it via `status.error`, but the overline used to keep
    // claiming the wallpaper was animating right through a give-up.
    // A covering window or the lock screen holds playback without `paused`
    // (the toggle's user/battery flag) being set; the overline still says so.
    w.overline.set_label(pill_overline(
        !status.gave_up.is_empty(),
        status.paused || status.paused_reason.is_some(),
        w.still_frame,
    ));
    match status.paused_reason {
        Some(r) => {
            w.reason.set_label(pause_reason_label(r));
            w.reason.set_visible(true);
        }
        None => w.reason.set_visible(false),
    }
    w.overline.set_visible(true);
    // Clone the lookup out first: it borrows the app state.
    let lookup = NAME_LOOKUP.with(|l| l.borrow().clone());
    let library_name = lookup
        .zip(status.wallpaper_path.as_deref())
        .and_then(|(f, p)| f(p));
    let (title, file) = pill_title(library_name, status.wallpaper.as_deref());
    let (title, file) = if lock_only {
        w.overline.set_label(t!("LOCK SCREEN ONLY"));
        (t!("Lock screen").to_string(), None)
    } else {
        (title, file)
    };
    w.label.set_label(&title);

    match status.hwdec.as_deref() {
        Some(raw) if raw != "no" => {
            w.hwdec.set_label(hwdec_label(raw));
            w.hwdec.set_visible(true);
        }
        _ => w.hwdec.set_visible(false),
    }

    w.toggle
        .set_visible(pill_shows_toggle(w.still_frame) && !lock_only);
    if status.paused {
        w.toggle.set_icon_name("media-playback-start-symbolic");
        w.toggle.set_tooltip_text(Some(t!("Resume")));
    } else {
        w.toggle.set_icon_name("media-playback-pause-symbolic");
        w.toggle.set_tooltip_text(Some(t!("Pause")));
    }

    let mut tip = tf!("CPU {percent}%", "percent" => format!("{:.0}", status.cpu_percent));
    if let Some(file) = file {
        tip.insert_str(0, &format!("{file}\n"));
    }
    if let Some(err) = status.error.as_deref() {
        tip.push('\n');
        tip.push_str(err);
    }
    w.pill.set_tooltip_text(Some(&tip));
}

/// The pill's state overline for a running daemon. A give-up outranks
/// everything; after that a still-frame session says so rather than PLAYING
/// (GNOME Wayland paints one frame and never animates it, whatever `paused`
/// says), then paused, then playing.
fn pill_overline(gave_up: bool, paused: bool, still_frame: bool) -> &'static str {
    if gave_up {
        t!("NEEDS ATTENTION")
    } else if still_frame {
        t!("STILL FRAME")
    } else if paused {
        t!("PAUSED")
    } else {
        t!("PLAYING")
    }
}

/// Short, already-localized text for why playback is held, shown beside the
/// PAUSED overline (the CLI spells the same causes out in `cli::paused_label`).
fn pause_reason_label(reason: PauseReason) -> &'static str {
    match reason {
        PauseReason::User => t!("Paused by you"),
        PauseReason::Battery => t!("On battery"),
        PauseReason::Locked => t!("Screen locked"),
        PauseReason::Fullscreen => t!("Fullscreen app open"),
        PauseReason::Covered => t!("Window covers screen"),
    }
}

/// Pause/Resume only mean something when something is playing; on a
/// still-frame session the daemon ignores both, so don't offer the button.
fn pill_shows_toggle(still_frame: bool) -> bool {
    !still_frame
}

/// The pill's wallpaper title, plus the file name for the tooltip when the
/// title doesn't already say it. A wallpaper in the library is shown under the
/// name the user gave it (#99), as on its card; anything else falls back to the
/// daemon's file name, prettified.
fn pill_title(library_name: Option<String>, file: Option<&str>) -> (String, Option<&str>) {
    let title =
        library_name.unwrap_or_else(|| pretty_status_name(file.unwrap_or(t!("Wallpaper active"))));
    let file = file.filter(|f| *f != title);
    (title, file)
}

/// Prettify the daemon-reported wallpaper name for the pill: strip a trailing
/// media extension ("CAR.mp4" → "CAR") and middle-truncate very long names.
fn pretty_status_name(raw: &str) -> String {
    let mut n = raw.trim().to_string();
    if let Some((stem, ext)) = n.rsplit_once('.') {
        if !stem.is_empty()
            && matches!(
                ext.to_ascii_lowercase().as_str(),
                "mp4"
                    | "webm"
                    | "mkv"
                    | "avi"
                    | "mov"
                    | "flv"
                    | "gif"
                    | "jpg"
                    | "jpeg"
                    | "png"
                    | "webp"
                    | "bmp"
                    | "tiff"
            )
        {
            n = stem.to_string();
        }
    }
    let chars: Vec<char> = n.chars().collect();
    if chars.len() > 28 {
        let head: String = chars[..18].iter().collect();
        let tail: String = chars[chars.len() - 8..].iter().collect();
        n = format!("{}…{}", head.trim_end(), tail.trim_start());
    }
    n
}

/// Map a raw hwdec value from frescod to a friendly badge label.
fn hwdec_label(raw: &str) -> &str {
    match raw {
        "vaapi" => "VA-API",
        "nvdec" => "NVDEC",
        "vdpau" => "VDPAU",
        "drm" => "DRM",
        _ => raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overline_says_playing_or_paused_on_a_live_session() {
        assert_eq!(pill_overline(false, false, false), "PLAYING");
        assert_eq!(pill_overline(false, true, false), "PAUSED");
    }

    /// GNOME Wayland reports `running: true, paused: false` for a wallpaper
    /// that is one frozen frame; the pill must not call that PLAYING.
    #[test]
    fn overline_says_still_frame_on_a_still_frame_session() {
        assert_eq!(pill_overline(false, false, true), "STILL FRAME");
        // `paused` is meaningless there (Pause is a no-op) and must not leak.
        assert_eq!(pill_overline(false, true, true), "STILL FRAME");
    }

    #[test]
    fn a_give_up_outranks_every_other_state() {
        for paused in [false, true] {
            for still_frame in [false, true] {
                assert_eq!(pill_overline(true, paused, still_frame), "NEEDS ATTENTION");
            }
        }
    }

    /// #98: after the tray's "Quit" the config still says on but nothing runs.
    /// The pill must say so (and offer Start), not claim it is playing or that
    /// the wallpaper is gone for good; a deliberate Stop stays plain "Not running".
    #[test]
    fn pill_says_a_quit_wallpaper_resumes_at_login() {
        let cfg = |enabled, autostart| Config {
            enabled,
            autostart,
            wallpaper: Wallpaper {
                path: Some("/w/a.mp4".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let wanted = off_state(&cfg(true, true));
        assert_eq!(
            wanted,
            Off::Wanted {
                resumes_at_login: true
            }
        );
        assert_eq!(off_overline(wanted), Some("RESUMES AT NEXT LOGIN"));

        // No autostart entry: Start is still offered, but login won't help.
        let manual = off_state(&cfg(true, false));
        assert_eq!(
            manual,
            Off::Wanted {
                resumes_at_login: false
            }
        );
        assert_eq!(off_overline(manual), None);

        // Stopped on purpose: nothing resumes, nothing to start.
        assert_eq!(off_state(&cfg(false, true)), Off::Idle);
        assert_eq!(off_overline(Off::Idle), None);

        // A per-monitor wallpaper alone counts as configured.
        let mut per_monitor = Config::default();
        per_monitor
            .monitors
            .insert("DP-1".into(), cfg(true, true).wallpaper);
        assert!(matches!(off_state(&per_monitor), Off::Wanted { .. }));
    }

    /// A fresh install defaults to enabled + autostart but has nothing to play:
    /// no "RESUMES AT NEXT LOGIN", no Start.
    #[test]
    fn pill_is_idle_when_no_wallpaper_is_configured() {
        let fresh = Config::default();
        assert!(fresh.enabled && fresh.autostart);
        assert_eq!(off_state(&fresh), Off::Idle);
    }

    /// With the desktop wallpaper off the lock screen alone wants the daemon:
    /// a dead one offers Start, a stopped-on-purpose one with no lock screen
    /// stays idle.
    #[test]
    fn pill_offers_start_for_a_lock_screen_only_daemon() {
        let mut cfg = Config {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(off_state(&cfg), Off::Idle);
        cfg.lockscreen = Some(crate::config::LockScreen {
            enabled: true,
            ..Default::default()
        });
        assert!(matches!(off_state(&cfg), Off::Wanted { .. }));
    }

    /// A status poll that connects but gets no reply in time is a busy daemon,
    /// not an absent one; a refused or missing socket is absent.
    #[test]
    fn timed_out_tells_a_slow_daemon_from_an_absent_one() {
        use std::io::{Error, ErrorKind};
        let io = |kind| anyhow::Error::new(Error::new(kind, "x")).context("ctx");
        assert!(timed_out(&io(ErrorKind::WouldBlock)));
        assert!(timed_out(&io(ErrorKind::TimedOut)));
        assert!(!timed_out(&io(ErrorKind::NotFound)));
        assert!(!timed_out(&io(ErrorKind::ConnectionRefused)));
    }

    /// The library only redraws when its picture of the daemon changes: the
    /// first poll finding it up changes nothing (it already assumed so).
    #[test]
    fn running_cache_reports_what_the_library_must_redraw() {
        assert_eq!(cached_daemon_running(), None);
        assert!(
            !set_cached_daemon_running(true),
            "first poll: up, as assumed"
        );
        assert!(!set_cached_daemon_running(true), "still up");
        assert!(set_cached_daemon_running(false), "daemon quit");
        assert!(!set_cached_daemon_running(false), "still gone");
        assert!(set_cached_daemon_running(true), "daemon back");
        assert_eq!(cached_daemon_running(), Some(true));
    }

    /// Opening the app with the daemon already quit: the library has drawn its
    /// active card from config.toml and must now take it back.
    #[test]
    fn a_first_poll_finding_no_daemon_redraws_the_library() {
        assert!(set_cached_daemon_running(false));
    }

    #[test]
    fn every_pause_reason_has_distinct_text() {
        let all = [
            PauseReason::User,
            PauseReason::Battery,
            PauseReason::Locked,
            PauseReason::Fullscreen,
            PauseReason::Covered,
        ];
        let texts: std::collections::HashSet<_> = all.into_iter().map(pause_reason_label).collect();
        assert_eq!(texts.len(), all.len());
        assert!(texts.iter().all(|t| !t.is_empty()));
    }

    /// #99: a renamed library wallpaper reads as its custom name, with the file
    /// name moved to the tooltip; one the library doesn't know keeps the
    /// prettified file name.
    #[test]
    fn pill_title_prefers_the_library_name() {
        assert_eq!(
            pill_title(Some("自在".into()), Some("动画2-42054c1530dc.mp4")),
            ("自在".to_string(), Some("动画2-42054c1530dc.mp4"))
        );
        assert_eq!(
            pill_title(None, Some("CAR.mp4")),
            ("CAR".to_string(), Some("CAR.mp4"))
        );
        // Nothing to add to the tooltip when the title already is the name.
        assert_eq!(
            pill_title(None, Some("Playlist (3 items)")),
            ("Playlist (3 items)".to_string(), None)
        );
        assert_eq!(
            pill_title(None, None),
            ("Wallpaper active".to_string(), None)
        );
    }

    #[test]
    fn pause_toggle_is_hidden_only_on_a_still_frame_session() {
        assert!(pill_shows_toggle(false));
        assert!(!pill_shows_toggle(true));
    }

    /// The poll runs every few seconds; the library must only be told when the
    /// playing file actually changes (including going to/from "unknown").
    #[test]
    fn playing_path_cache_reports_changes_only() {
        // thread_local: each test thread starts empty.
        assert_eq!(cached_playing_path(), None);
        assert!(!set_cached_playing_path(None), "unknown -> unknown");

        let day = Some(PathBuf::from("/walls/day.mp4"));
        assert!(set_cached_playing_path(day.clone()));
        assert_eq!(cached_playing_path(), day);
        assert!(!set_cached_playing_path(day), "same file again");

        assert!(set_cached_playing_path(Some(PathBuf::from(
            "/walls/night.mp4"
        ))));
        assert!(set_cached_playing_path(None), "daemon went away");
        assert_eq!(cached_playing_path(), None);
    }

    /// After the GUI's own Set the cache still names the wallpaper that was
    /// replaced. Forgetting it must make the answer "unknown" at once, and must
    /// not run the hook: the caller redraws itself, and a redraw from inside
    /// the hook would read the half-updated state.
    #[test]
    fn forgetting_the_playing_path_clears_it_without_firing_the_hook() {
        let calls = Rc::new(Cell::new(0u32));
        {
            let calls = calls.clone();
            on_playing_path_changed(move || calls.set(calls.get() + 1));
        }
        note_playing_path(Some(PathBuf::from("/walls/old.mp4")));
        assert_eq!(calls.get(), 1);
        assert_eq!(cached_playing_path(), Some(PathBuf::from("/walls/old.mp4")));

        forget_playing_path();
        assert_eq!(
            cached_playing_path(),
            None,
            "still naming the replaced file"
        );
        assert_eq!(calls.get(), 1, "forgetting must not fire the hook");

        // The next poll reports the new wallpaper and is a change again.
        note_playing_path(Some(PathBuf::from("/walls/new.mp4")));
        assert_eq!(cached_playing_path(), Some(PathBuf::from("/walls/new.mp4")));
        assert_eq!(calls.get(), 2);

        // Forgetting an already-unknown cache is harmless.
        forget_playing_path();
        forget_playing_path();
        assert_eq!(cached_playing_path(), None);
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn note_playing_path_fires_the_hook_once_per_change() {
        let calls = Rc::new(Cell::new(0u32));
        {
            let calls = calls.clone();
            on_playing_path_changed(move || {
                // The hook re-renders the grid, which reads the cache: it must
                // already hold the new value by the time the hook runs.
                assert!(cached_playing_path().is_some());
                calls.set(calls.get() + 1);
            });
        }
        let p = Some(PathBuf::from("/walls/day.mp4"));
        note_playing_path(p.clone());
        note_playing_path(p);
        assert_eq!(calls.get(), 1);
    }
}

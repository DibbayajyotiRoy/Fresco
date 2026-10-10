//! Reusable "hover to play" preview behaviour for video/GIF library cards.
//!
//! While the pointer is over a card we want it to come alive: a muted, looping
//! inline video preview fades in over the static thumbnail, and on leave the
//! thumbnail shows again.
//!
//! The trick is that the preview must NEVER influence the card's measured
//! size. A raw video paintable reports its full resolution (an 8K wallpaper
//! reports 7680×4320) as its natural size; putting it where layout can see it
//! makes the FlowBox re-measure, the card grows and shifts, the pointer falls
//! outside it, hover "leaves", the card snaps back, hover "enters" — an
//! endless glitch loop. So the card's base child stays the thumbnail
//! [`gtk4::Picture`] (constant intrinsic size), and the video lives in a
//! separate `Picture` stacked in a [`gtk4::Overlay`] — overlay children are
//! excluded from size measurement, so the card's geometry never changes.
//!
//! A [`gtk4::MediaFile`] is created lazily on the hover that actually settles,
//! and destroyed again the moment the pointer leaves for good: `clear()`, unset
//! the paintable, drop. An earlier version kept it alive "for snappy re-hover",
//! which is why a library browse ended with one decoder, one VAAPI/NVDEC
//! context and one set of GL textures resident per card ever hovered. A hover
//! preview is worth exactly one live decoder and not one byte more, so the
//! decision of which card may hold that decoder is centralised in
//! [`PreviewPolicy`] below — process-wide, not per card.
//!
//! What that decoder is given matters as much as how many there are. GTK
//! decodes at the file's own resolution, so previewing a 4K clip turned every
//! hover into 30 fresh 30 MB textures a second for a 300 px card — enough to
//! exhaust a Vulkan device and take the whole GUI down inside GSK. A card
//! therefore never plays a large source: it asks [`preview_proxy`] for a small
//! stand-in clip, and if that has not been made yet it keeps showing the
//! thumbnail while one is built in the background.
//!
//! Three rules keep it honest. A hover must *settle* before it starts anything
//! ([`START_DEBOUNCE`]): sweeping the pointer across a shelf of cards must start
//! nothing. A leave is forgiven for a moment ([`HOVER_GRACE`]): a flicker across
//! a card's own Edit button must stop nothing. And a card that leaves the widget
//! tree releases its pipeline on the way out, so `populate_library`'s "remove
//! every child" sweep can never strand one mid-decode.
//!
//! One subtlety is load-bearing enough to spell out: the `Picture` holds a
//! strong reference to the `MediaFile` (it is its paintable), so anything the
//! `MediaFile` holds pointing back at the card closes a reference cycle, and
//! GObject has no cycle collector. The `invalidate_contents` handler below
//! therefore captures the card **weakly**. Without that, none of the rest of
//! this module frees a thing.
//!
//! A preview nobody can see is pure cost, and on a machine without GStreamer
//! hardware-decode plugins (stock Deepin, for one) it is a large one. So the
//! toplevel window is watched too — losing focus, being minimised or being
//! hidden stops and releases the live preview through the same policy
//! ([`PreviewPolicy::on_window_hidden`]), and no hover may start a new one
//! until the window is back ([`PreviewPolicy::on_window_shown`]). The Settings
//! switch is the same idea one level up ([`PreviewPolicy::set_disabled`]).
//!
//! Finally, a crash sentinel ([`startup`]): while a `MediaFile` exists we leave
//! a file in the state directory naming our pid. A normal exit or a termination
//! signal removes it; if the next launch finds it with that pid dead, we died
//! with a preview showing — whatever the cause, the next hover would do it
//! again — so previews are switched off and the user is told, instead of
//! crashing every time they touch the library.
//!
//! Decoding stays best-effort: if no GStreamer plugins are installed the media
//! simply never produces frames and the card shows no motion — nothing breaks.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Once, OnceLock};
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::{gio, glib, EventControllerMotion};

use super::preview_proxy::{self, PreviewSource, Priority};
use super::window::{show_sticky_toast, AppState};
use crate::t;

/// Grace period for the *leave* edge.
///
/// Moving the pointer across the card's revealed Edit button / overlays emits
/// brief leave→enter crossings, and without the delay the preview swaps back
/// and forth.
const HOVER_GRACE: Duration = Duration::from_millis(140);

/// How long a hover must stay put before it earns a preview.
///
/// Longer than [`HOVER_GRACE`] on purpose. Leaving is forgiven quickly because
/// a stray crossing should not interrupt something playing; starting is
/// deliberate because a start is the expensive edge — a decoder, a GPU
/// context, and now possibly a proxy transcode. Five cards crossed inside one
/// debounce period spawn zero of them, not five, which is also what protects a
/// GPU with only a handful of hardware-decode sessions.
const START_DEBOUNCE: Duration = Duration::from_millis(300);

// ---------------------------------------------------------------------------
// Policy: which card may hold the one live decoder. No GTK in this half.
// ---------------------------------------------------------------------------

/// Identifies one attached card within the process-wide [`PreviewPolicy`].
type CardId = u64;

/// How the GTK layer carries out one [`Action`] on one card: the whole of this
/// module's glue, boxed per card so the policy can address cards by id alone.
type Apply = Rc<dyn Fn(Action)>;

/// What the GTK layer must do to a card. Emitted in the order it must happen.
///
/// The translation is deliberately trivial — `Start` → `play()`, `Stop` →
/// `pause()`, `Release` → `clear()` + drop the `MediaFile` — so that the glue
/// is correct by inspection and everything with a decision in it lives in
/// [`PreviewPolicy`], where a test can drive it without a display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Start(CardId),
    Stop(CardId),
    Release(CardId),
}

impl Action {
    fn card(self) -> CardId {
        match self {
            Action::Start(c) | Action::Stop(c) | Action::Release(c) => c,
        }
    }
}

/// The single-active-preview rule, as a plain state machine.
///
/// `now` is a parameter rather than an `Instant::now()` call so the tests can
/// step time by hand instead of sleeping through five grace periods.
#[derive(Default)]
struct PreviewPolicy {
    /// The one card currently holding a decoder, if any. That this is an
    /// `Option` and not a set is the concurrency cap.
    live: Option<CardId>,
    /// A hover that has not yet settled: `(card, when it may start)`. At most
    /// one, so a sweep replaces rather than accumulates.
    pending: Option<(CardId, Instant)>,
    /// The live card's pointer has left: `(card, when to release it)`. Cleared
    /// if the pointer comes back inside the grace window.
    expiring: Option<(CardId, Instant)>,
    /// The toplevel window is unfocused, minimised or hidden. While set, no
    /// hover may settle into a decoder: an unfocused window still receives
    /// crossing events on most compositors, and a preview playing behind
    /// another app's window is software decode for nobody.
    suppressed: bool,
    /// The user (or the crash sentinel) switched hover previews off. While
    /// set, nothing hovers into a decoder at all. Separate from `suppressed`
    /// because the two end for different reasons: focus returns by itself, this
    /// stays until the switch is flipped back.
    disabled: bool,
}

impl PreviewPolicy {
    fn on_enter(&mut self, card: CardId, now: Instant) -> Vec<Action> {
        if self.live == Some(card) {
            // Re-hover inside the grace window (or a crossing over the card's
            // own overlay children): cancel the release, keep playing, and
            // abandon any other card the sweep had queued up.
            self.expiring = None;
            self.pending = None;
            return Vec::new();
        }
        if self.suppressed || self.disabled {
            return Vec::new();
        }
        // Nothing starts on the enter edge itself; the tick decides.
        self.pending = Some((card, now + START_DEBOUNCE));
        Vec::new()
    }

    fn on_leave(&mut self, card: CardId, now: Instant) -> Vec<Action> {
        if matches!(self.pending, Some((c, _)) if c == card) {
            // Left before it ever settled — it never allocated anything, so
            // there is nothing to stop and nothing to release.
            self.pending = None;
        }
        if self.live == Some(card) {
            self.expiring = Some((card, now + HOVER_GRACE));
        }
        Vec::new()
    }

    /// The card is leaving the widget tree. Whatever it holds goes with it,
    /// grace period or not — the pipeline must not outlive the widget.
    fn on_gone(&mut self, card: CardId) -> Vec<Action> {
        if matches!(self.pending, Some((c, _)) if c == card) {
            self.pending = None;
        }
        if matches!(self.expiring, Some((c, _)) if c == card) {
            self.expiring = None;
        }
        if self.live == Some(card) {
            self.live = None;
            return vec![Action::Stop(card), Action::Release(card)];
        }
        Vec::new()
    }

    /// The window stopped being something the user is looking at (focus lost,
    /// minimised, hidden). Tear the live preview down now — no grace period,
    /// since there is no flicker to debounce — and forget any hover that was
    /// about to settle, so the returning window does not start one on its own.
    fn on_window_hidden(&mut self) -> Vec<Action> {
        self.suppressed = true;
        self.pending = None;
        self.release_live()
    }

    /// The window is focused and on screen again. Nothing restarts by itself:
    /// the next hover that settles is what earns a decoder.
    fn on_window_shown(&mut self) -> Vec<Action> {
        self.suppressed = false;
        Vec::new()
    }

    /// Hover previews were switched off (or back on). Switching off tears the
    /// live preview down at once and forgets the hover in flight; switching on
    /// starts nothing — the next hover that settles does.
    fn set_disabled(&mut self, disabled: bool) -> Vec<Action> {
        self.disabled = disabled;
        if !disabled {
            return Vec::new();
        }
        self.pending = None;
        self.release_live()
    }

    fn on_tick(&mut self, now: Instant) -> Vec<Action> {
        let mut actions = Vec::new();
        if let Some((card, deadline)) = self.expiring {
            if now >= deadline {
                self.expiring = None;
                if self.live == Some(card) {
                    self.live = None;
                    actions.push(Action::Stop(card));
                    actions.push(Action::Release(card));
                }
            }
        }
        if let Some((card, deadline)) = self.pending {
            if now >= deadline {
                self.pending = None;
                if self.suppressed || self.disabled {
                    // Settled while the window was away (or the enter raced the
                    // focus change, or the switch): drop it rather than decode
                    // unseen or unwanted.
                    return actions;
                }
                // Whatever was playing loses its decoder before the new card
                // gets one, never the other way round: at no point in this
                // list are two pipelines alive.
                actions.append(&mut self.release_live());
                self.live = Some(card);
                actions.push(Action::Start(card));
            }
        }
        actions
    }

    /// When the next deadline falls, so the caller can arm one timer for it
    /// rather than polling.
    fn next_deadline(&self) -> Option<Instant> {
        match (self.pending, self.expiring) {
            (Some((_, a)), Some((_, b))) => Some(a.min(b)),
            (Some((_, a)), None) => Some(a),
            (None, Some((_, b))) => Some(b),
            (None, None) => None,
        }
    }

    fn release_live(&mut self) -> Vec<Action> {
        self.expiring = None;
        match self.live.take() {
            Some(prev) => vec![Action::Stop(prev), Action::Release(prev)],
            None => Vec::new(),
        }
    }
}

/// Bookkeeping for the one timer that wakes [`PreviewPolicy::on_tick`].
///
/// With a single debounce for both edges a later event could only push the
/// deadline *out*, so "a timer is already armed" was enough. Starts and leaves
/// now wait different lengths, so a leave can fall due *before* a start that is
/// already being timed — and the release must not be held back until the start
/// timer fires. So the plan is per deadline: an earlier one supersedes the
/// armed timer, and the superseded timer, when it fires, recognises itself as
/// stale by its generation number and does nothing.
#[derive(Default)]
struct TickArm {
    /// `(generation, deadline)` of the timer that is currently wanted.
    armed: Option<(u64, Instant)>,
    generation: u64,
}

impl TickArm {
    /// Given the policy's next deadline, say which timer (if any) to start.
    fn plan(&mut self, next: Option<Instant>) -> Option<(u64, Instant)> {
        let next = next?;
        if matches!(self.armed, Some((_, at)) if at <= next) {
            return None;
        }
        self.generation += 1;
        self.armed = Some((self.generation, next));
        Some((self.generation, next))
    }

    /// The timer of `generation` fired. True if it is still the wanted one.
    fn fired(&mut self, generation: u64) -> bool {
        if matches!(self.armed, Some((g, _)) if g == generation) {
            self.armed = None;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Crash sentinel. No GTK in this half either.
// ---------------------------------------------------------------------------

/// What the sentinel file left by the previous run says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// No sentinel, or one that names no pid: the last run ended normally.
    Clean,
    /// The sentinel names a Fresco that is no longer running: it died with a
    /// preview showing.
    Crashed,
    /// The sentinel names a Fresco that is still alive — another instance owns
    /// it, and it is not ours to clear or to judge.
    Running,
}

/// Read the sentinel. `pid_alive` is a parameter so the decision is testable
/// without real processes.
///
/// Our own pid counts as dead: a sentinel naming us at startup can only be left
/// over from an earlier process that happened to have the same pid (a reboot
/// after a crash, say), since we have not written one yet.
fn sentinel_verdict(
    contents: Option<&str>,
    my_pid: u32,
    pid_alive: impl Fn(u32) -> bool,
) -> Verdict {
    let Some(pid) = contents.and_then(|c| c.trim().parse::<u32>().ok()) else {
        return Verdict::Clean;
    };
    if pid != my_pid && pid_alive(pid) {
        Verdict::Running
    } else {
        Verdict::Crashed
    }
}

/// What to do about a [`Verdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recovery {
    Nothing,
    /// Remove the stale sentinel, say nothing: previews were already off, so
    /// the crash cannot have been theirs to prevent.
    ClearOnly,
    /// Turn previews off, remember that, and tell the user why.
    DisableAndTell,
}

fn recovery(verdict: Verdict, previews_on: bool) -> Recovery {
    match verdict {
        Verdict::Clean | Verdict::Running => Recovery::Nothing,
        Verdict::Crashed if previews_on => Recovery::DisableAndTell,
        Verdict::Crashed => Recovery::ClearOnly,
    }
}

/// Whether `pid` is a running `fresco`. The name check is what stops a pid
/// recycled by some unrelated process, after a reboot, from reading as alive.
fn pid_is_fresco(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|c| c.trim() == "fresco")
}

/// Next to `frescod.log` and the other state-dir markers.
fn sentinel_path() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("hover-active")
}

fn write_sentinel(path: &Path, pid: u32) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(path, pid.to_string()).ok();
}

/// Mark that a preview decoder now exists. Written *before* the `MediaFile`,
/// because the crash this guards against happens inside GTK while it plays.
fn arm_sentinel() {
    write_sentinel(&sentinel_path(), std::process::id());
}

/// The preview was released; a crash from here on is not the preview's.
fn disarm_sentinel() {
    std::fs::remove_file(sentinel_path()).ok();
}

/// The sentinel's path as a C string, prepared before any signal can arrive so
/// that [`clear_sentinel_and_die`] never allocates.
static SENTINEL_C: OnceLock<CString> = OnceLock::new();

const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;
const SIG_DFL: usize = 0;

extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
    fn unlink(path: *const std::ffi::c_char) -> i32;
    fn raise(signum: i32) -> i32;
}

/// Delete the sentinel, then die of the signal exactly as we would have.
///
/// Only async-signal-safe calls (`unlink`, `signal`, `raise`) and one atomic
/// load; nothing here allocates, locks or touches GLib.
extern "C" fn clear_sentinel_and_die(sig: i32) {
    if let Some(path) = SENTINEL_C.get() {
        // SAFETY: `unlink` is async-signal-safe and `path` is a NUL-terminated
        // string that lives in a static for the rest of the process.
        unsafe {
            unlink(path.as_ptr());
        }
    }
    // SAFETY: `signal` and `raise` are async-signal-safe. Restoring the default
    // disposition and re-raising ends the process the way the signal would have
    // without this handler, status and all.
    unsafe {
        signal(sig, SIG_DFL);
        raise(sig);
    }
}

/// Make `SIGTERM`, `SIGINT` and `SIGHUP` clear the sentinel on the way out.
///
/// A logout, `pkill fresco` or Ctrl+C is the user (or the session) ending
/// Fresco, not Fresco dying, and must not be read next launch as a crash that
/// turns previews off. Only a death with no chance to clean up — `SIGKILL`, a
/// segfault inside GSK — should leave the file behind.
///
/// A plain handler rather than a GLib signal source, deliberately: a GLib
/// source runs on the main loop, so a GUI whose loop is wedged would swallow
/// the signal and become killable only by `SIGKILL`. This handler never needs
/// the loop, and leaves the process dying of the signal just as before; the
/// window state, as before, is not saved on a signal death.
fn install_signal_cleanup() {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = CString::new(sentinel_path().as_os_str().as_bytes()) else {
        return;
    };
    if SENTINEL_C.set(path).is_err() {
        return; // already installed
    }
    for sig in [SIGHUP, SIGINT, SIGTERM] {
        // SAFETY: the handler only makes async-signal-safe calls.
        unsafe {
            signal(sig, clear_sentinel_and_die as *const () as usize);
        }
    }
}

// ---------------------------------------------------------------------------
// GTK glue: a translator over the policy, plus the registry it addresses.
// ---------------------------------------------------------------------------

/// Everything hover preview knows, shared by every card in the process.
///
/// Thread-local rather than passed in: the policy has to be *global* to cap
/// concurrency at one decoder, and threading a handle down to `attach` would
/// mean touching `window.rs`'s card construction for no behavioural gain. GTK
/// is single-threaded, so a `thread_local!` on the main loop is the whole story.
#[derive(Default)]
struct Previews {
    policy: PreviewPolicy,
    /// How to carry out an [`Action`] on each live card. Dropped when the card
    /// leaves the tree, which is what stops this map from pinning `Picture`s.
    cards: HashMap<CardId, Apply>,
    next_id: CardId,
    /// Toplevel windows whose focus/visibility already feeds the policy. Weak,
    /// so the registry never keeps a closed window alive; checked so that a
    /// library of 200 cards connects one set of handlers, not 200.
    watched: Vec<gtk4::glib::WeakRef<gtk4::Window>>,
    /// The timer that will call [`PreviewPolicy::on_tick`].
    tick: TickArm,
}

thread_local! {
    static PREVIEWS: RefCell<Previews> = RefCell::new(Previews::default());
}

/// Feed one event to the shared policy and carry out what it asks for.
///
/// The registry borrow is released before any action runs. `play`, `pause` and
/// `clear` re-enter GTK, and a paintable signal or a synthesised crossing event
/// that comes back synchronously must not find the `RefCell` already borrowed.
fn dispatch(event: impl FnOnce(&mut PreviewPolicy) -> Vec<Action>) {
    let work: Vec<(Apply, Action)> = PREVIEWS.with(|previews| {
        let mut previews = previews.borrow_mut();
        let actions = event(&mut previews.policy);
        actions
            .into_iter()
            .filter_map(|action| {
                previews
                    .cards
                    .get(&action.card())
                    .cloned()
                    .map(|apply| (apply, action))
            })
            .collect()
    });
    for (apply, action) in work {
        apply(action);
    }
    arm_tick();
}

/// Make sure a timer exists for the policy's next deadline. See [`TickArm`] for
/// why this is more than "is one armed".
fn arm_tick() {
    let plan = PREVIEWS.with(|previews| {
        let mut previews = previews.borrow_mut();
        let next = previews.policy.next_deadline();
        previews.tick.plan(next)
    });
    let Some((generation, deadline)) = plan else {
        return;
    };
    let delay = deadline.saturating_duration_since(Instant::now());
    gtk4::glib::timeout_add_local_once(delay, move || {
        let current = PREVIEWS.with(|previews| previews.borrow_mut().tick.fired(generation));
        if current {
            dispatch(|policy| policy.on_tick(Instant::now()));
        }
    });
}

/// Whether `window` is something the user can currently see and is using.
///
/// Focus is the practical proxy: GTK4 has no portable "occluded" signal, and
/// on Wayland a minimised window is not unmapped, so `is_active` (which drops
/// when the window is minimised or another app takes focus) plus visibility
/// plus the surface's MINIMIZED bit — where the backend reports it — is the
/// most honest reading available.
fn window_is_showing(window: &gtk4::Window) -> bool {
    let minimized = window
        .surface()
        .and_then(|s| s.downcast::<gtk4::gdk::Toplevel>().ok())
        .is_some_and(|t| t.state().contains(gtk4::gdk::ToplevelState::MINIMIZED));
    window.is_visible() && window.is_mapped() && window.is_active() && !minimized
}

/// Feed the window's current visibility to the policy.
fn sync_window(window: &gtk4::Window) {
    if window_is_showing(window) {
        dispatch(|policy| policy.on_window_shown());
    } else {
        dispatch(|policy| policy.on_window_hidden());
    }
}

/// Connect `window`'s focus/visibility edges to the policy, once per window.
fn watch_window(window: &gtk4::Window) {
    let fresh = PREVIEWS.with(|previews| {
        let mut previews = previews.borrow_mut();
        previews.watched.retain(|w| w.upgrade().is_some());
        if previews
            .watched
            .iter()
            .any(|w| w.upgrade().as_ref() == Some(window))
        {
            return false;
        }
        previews.watched.push(window.downgrade());
        true
    });
    if !fresh {
        return;
    }
    // The handlers own nothing but the emitting window (passed in by GTK), so
    // no cycle forms between the window and the closures it stores.
    window.connect_is_active_notify(sync_window);
    window.connect_visible_notify(sync_window);
    window.connect_map(sync_window);
    window.connect_unmap(|_| dispatch(|policy| policy.on_window_hidden()));
    let hook_surface = |window: &gtk4::Window| {
        if let Some(surface) = window.surface() {
            let weak = window.downgrade();
            surface.connect_notify_local(Some("state"), move |_, _| {
                if let Some(window) = weak.upgrade() {
                    sync_window(&window);
                }
            });
        }
    };
    if window.is_realized() {
        hook_surface(window);
    } else {
        window.connect_realize(hook_surface);
    }
    sync_window(window);
}

/// Release the card's pipeline and forget it. Order matters: the policy's
/// `Release` has to be dispatched while the card can still be looked up.
fn forget_card(id: CardId) {
    dispatch(|policy| policy.on_gone(id));
    PREVIEWS.with(|previews| {
        previews.borrow_mut().cards.remove(&id);
    });
}

/// Turn hover previews on or off for every card, now. Off releases whatever is
/// playing; on starts nothing by itself.
pub fn set_enabled(enabled: bool) {
    dispatch(|policy| policy.set_disabled(!enabled));
}

/// One-time setup at launch: honour the saved switch, deal with a crash left
/// by the previous run, tidy `library/previews`, and make sure a normal exit
/// clears the sentinel. Call before the library view is built.
pub(super) fn startup(app: &gio::Application, state: &Rc<RefCell<AppState>>) {
    let (previews_on, known_ids) = {
        let s = state.borrow();
        (
            s.config.hover_previews,
            s.entries.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
        )
    };

    let path = sentinel_path();
    let contents = std::fs::read_to_string(&path).ok();
    let verdict = sentinel_verdict(contents.as_deref(), std::process::id(), pid_is_fresco);
    if contents.is_some() && verdict != Verdict::Running {
        std::fs::remove_file(&path).ok();
    }

    let mut enabled = previews_on;
    if recovery(verdict, previews_on) == Recovery::DisableAndTell {
        log::warn!(
            "hover previews: the previous Fresco died with a preview showing; turning them off"
        );
        {
            let mut s = state.borrow_mut();
            s.config.hover_previews = false;
            s.config.save().ok();
        }
        enabled = false;
        let state = state.clone();
        glib::idle_add_local_once(move || {
            show_sticky_toast(
                &state,
                t!("Video previews on hover were turned off because Fresco closed unexpectedly while showing one. You can turn them back on in the menu."),
            );
        });
    }
    set_enabled(enabled);

    // The card teardown clears the sentinel as each preview is released; this
    // is the backstop for a quit path that skips it.
    app.connect_shutdown(|_| disarm_sentinel());
    // And a signal is not a crash: see `install_signal_cleanup`.
    install_signal_cleanup();

    preview_proxy::prune_orphans(known_ids);
}

/// Log why `m` cannot play, if it cannot (issue #42).
///
/// On Debian, Ubuntu and deepin the GTK media backend is its own package,
/// `libgtk-4-media-gstreamer`, which `libgtk-4-1` only *Recommends*. Without it
/// GTK hands out a `GtkNoMediaFile` that sets "GTK could not find a media
/// module" on the stream and says nothing else — no `g_warning`, no frames, so
/// hover previews silently showed the still frame forever on deepin.
fn report_media_error(m: &gtk4::MediaFile) {
    static HINT: Once = Once::new();
    let Some(err) = m.error() else { return };
    log::warn!("hover preview: {err}");
    HINT.call_once(|| {
        log::warn!(
            "hover preview: install libgtk-4-media-gstreamer (the GTK media backend) to enable \
             hover previews; Debian/Ubuntu/deepin: sudo apt install libgtk-4-media-gstreamer \
             gstreamer1.0-plugins-good gstreamer1.0-libav"
        );
    });
}

/// What one attached card holds. Shared by the policy's action closure, the
/// first-frame callback (weakly) and the proxy-ready callback (weakly).
struct CardView {
    /// The video layer stacked above the thumbnail.
    video_pic: gtk4::Picture,
    /// The card's decoder, for as long as it is the live one. `None` between
    /// hovers is the point: an idle card owns no pipeline.
    media: RefCell<Option<gtk4::MediaFile>>,
    /// True once the current MediaFile has produced its first frame. Showing the
    /// video layer BEFORE that blanks the card — for however long the decoder
    /// takes, or forever when the codec's GStreamer plugin is missing. The
    /// thumbnail must stay visible until real frames exist. Resets on release,
    /// because the next hover starts a fresh MediaFile with no frames yet.
    ready: Cell<bool>,
    /// Whether this card is the live one, as far as the policy is concerned.
    /// The first-frame callback consults it so a frame that lands after the
    /// pointer has already moved on doesn't flash the preview up.
    live: Cell<bool>,
    /// What to play, decided at hover time rather than when the card is built:
    /// a proxy that did not exist at build time may exist now.
    source: Box<dyn Fn() -> PreviewSource>,
}

impl CardView {
    fn apply(self: &Rc<Self>, action: Action) {
        match action {
            Action::Start(_) => self.start(),
            Action::Stop(_) => self.stop(),
            Action::Release(_) => self.release(),
        }
    }

    fn start(self: &Rc<Self>) {
        self.live.set(true);
        // Clone out of the slot and let go: `play()` and `set_visible()` can
        // call back into GTK, and a callback that reaches another Action must
        // find `media` free. Cloning the MediaFile is a refcount bump.
        let existing = self.media.borrow().clone();
        if let Some(m) = existing {
            if self.ready.get() {
                self.video_pic.set_visible(true);
            }
            m.play();
            return;
        }
        match (self.source)() {
            PreviewSource::Direct(file) | PreviewSource::Proxy(file) => self.open(&file),
            PreviewSource::Build(request) => {
                // No clip yet: the thumbnail stays up, and when the transcode
                // lands the preview begins — if the pointer is still here.
                let weak = Rc::downgrade(self);
                preview_proxy::enqueue(
                    request,
                    Priority::Hover,
                    Some(Box::new(move |ok| {
                        let Some(card) = weak.upgrade() else { return };
                        if ok && card.live.get() && card.media.borrow().is_none() {
                            card.start();
                        }
                    })),
                );
            }
            PreviewSource::None => {}
        }
    }

    fn open(self: &Rc<Self>, file: &Path) {
        // Before the decoder exists, not after: the crash this records happens
        // inside GTK while the first frames are being painted.
        arm_sentinel();
        let m = gtk4::MediaFile::for_filename(file.to_string_lossy().as_ref());
        m.set_muted(true);
        m.set_loop(true);
        self.video_pic.set_paintable(Some(&m));

        // With no GTK media backend installed `for_filename` has already failed
        // (the error is set before we can connect); a missing codec plugin
        // fails later, through the same property. Either way the card would
        // wait for a first frame forever and look like the feature never ran.
        report_media_error(&m);
        m.connect_error_notify(report_media_error);

        // WEAK, and this is the leak fix. `set_paintable` above gave the
        // Picture a strong reference to the MediaFile; a strong reference from
        // this handler back to the card would close a cycle, and GObject has no
        // cycle collector, so neither object would ever be finalised — the
        // decoder, its hardware context and its textures would outlive the
        // card, unreachable, for the life of the process.
        let weak = Rc::downgrade(self);
        m.connect_invalidate_contents(move |_| {
            let Some(card) = weak.upgrade() else { return };
            if card.ready.get() {
                return;
            }
            card.ready.set(true);
            if card.live.get() {
                card.video_pic.set_visible(true);
            }
        });
        *self.media.borrow_mut() = Some(m.clone());
        m.play();
    }

    fn stop(&self) {
        self.live.set(false);
        let m = self.media.borrow().clone();
        self.video_pic.set_visible(false);
        if let Some(m) = m {
            m.pause();
        }
    }

    fn release(&self) {
        self.live.set(false);
        self.ready.set(false);
        let m = self.media.borrow_mut().take();
        self.video_pic.set_visible(false);
        // Drop the Picture's half of the pair too, or the cleared MediaFile
        // stays referenced until the next hover replaces it.
        self.video_pic.set_paintable(None::<&gtk4::gdk::Paintable>);
        if let Some(m) = m {
            m.pause();
            // `clear()` — gtk_media_file_clear — is the public teardown: it
            // closes the stream and unsets the file. (There is no `close()` in
            // the bindings; gtk_media_file_close is a class vfunc.) The
            // MediaFile itself is freed as `m` drops here.
            m.clear();
            disarm_sentinel();
        }
    }
}

/// Attach hover-to-play to a card.
///
/// - `card`: the card root `gtk4::Overlay` (the hover target spanning the card).
///   Its base child must currently be `thumb`.
/// - `thumb`: the `gtk4::Picture` showing the static thumbnail. It stays the
///   card's size-driving base child forever; the video preview is layered above
///   it inside an inner `Overlay` so it can never trigger a relayout.
/// - `source`: asked what to play each time a hover settles — see
///   [`PreviewSource`]. It is deliberately a function and not a path: the
///   answer changes as proxy clips get built.
///
/// Plays muted + looping while hovered, one card at a time process-wide, and
/// releases the decoder on leave. Degrades gracefully: if the media can't be
/// decoded (e.g. no GStreamer plugins installed) nothing bad happens — the card
/// simply shows no motion.
pub fn attach(
    card: &gtk4::Overlay,
    thumb: &gtk4::Picture,
    source: impl Fn() -> PreviewSource + 'static,
) {
    // Re-parent the thumbnail into an inner overlay and stack the (initially
    // hidden) video layer above it. Only the thumbnail is measured.
    let inner = gtk4::Overlay::new();
    card.set_child(None::<&gtk4::Widget>);
    inner.set_child(Some(thumb));
    let video_pic = gtk4::Picture::new();
    video_pic.set_can_shrink(true);
    video_pic.set_keep_aspect_ratio(true);
    video_pic.set_can_target(false);
    video_pic.set_visible(false);
    inner.add_overlay(&video_pic);
    card.set_child(Some(&inner));

    let view = Rc::new(CardView {
        video_pic,
        media: RefCell::new(None),
        ready: Cell::new(false),
        live: Cell::new(false),
        source: Box::new(source),
    });
    let apply: Apply = {
        let view = view.clone();
        Rc::new(move |action| view.apply(action))
    };

    let id = PREVIEWS.with(|previews| {
        let mut previews = previews.borrow_mut();
        previews.next_id += 1;
        let id = previews.next_id;
        previews.cards.insert(id, apply.clone());
        id
    });

    let controller = EventControllerMotion::new();
    controller.connect_enter(move |_controller, _x, _y| {
        dispatch(|policy| policy.on_enter(id, Instant::now()));
    });
    controller.connect_leave(move |_controller| {
        dispatch(|policy| policy.on_leave(id, Instant::now()));
    });
    card.add_controller(controller);

    // Tear down with the card. `populate_library` empties the library with a
    // `while let Some(c) = first_child()` sweep and never stops anything first,
    // so a card can be pulled out of the tree mid-decode; unrooting is the edge
    // that fires synchronously when that happens, whatever the refcounts are.
    // Doing this here rather than at the call site is what keeps the fix inside
    // this module.
    {
        let apply = apply.clone();
        card.connect_root_notify(move |card| {
            if let Some(root) = card.root() {
                if let Ok(window) = root.downcast::<gtk4::Window>() {
                    watch_window(&window);
                }
                // Re-added to a window: the card works again from here. Cards
                // aren't re-parented today, but silently going dead if that
                // ever changed would be a miserable bug to find.
                PREVIEWS.with(|previews| {
                    previews.borrow_mut().cards.insert(id, apply.clone());
                });
                return;
            }
            forget_card(id);
        });
    }
    if let Some(window) = card.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
        watch_window(&window);
    }
    // Backstop for any path that disposes the card without unrooting it first.
    // Both are idempotent; whichever arrives first does the work.
    card.connect_destroy(move |_| forget_card(id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The tests step this clock by hand; the policy never reads the real one.
    fn t(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    /// Long enough that every outstanding deadline has certainly passed.
    const SETTLED: u64 = 1000;

    /// Milliseconds a hover must stay put before it starts (`START_DEBOUNCE`).
    const START: u64 = 300;
    /// Milliseconds a leave is forgiven for (`HOVER_GRACE`).
    const GRACE: u64 = 140;

    /// A stand-in for the GTK half, asserting the contract the real translator
    /// relies on: never two live pipelines, never a `Start` on a card that is
    /// already playing, never a `Release` for a card that holds nothing.
    #[derive(Default)]
    struct FakeGtk {
        /// The card currently playing, mirroring `live.set(true)`.
        playing: Option<CardId>,
        /// Cards holding a `MediaFile`, mirroring the `media` slot.
        decoders: HashSet<CardId>,
        starts: usize,
    }

    impl FakeGtk {
        fn apply(&mut self, actions: Vec<Action>) {
            for action in actions {
                match action {
                    Action::Start(c) => {
                        assert_eq!(self.playing, None, "started {c} while another card played");
                        self.playing = Some(c);
                        self.decoders.insert(c);
                        self.starts += 1;
                    }
                    Action::Stop(c) => {
                        assert_eq!(self.playing, Some(c), "stopped {c}, which wasn't playing");
                        self.playing = None;
                    }
                    Action::Release(c) => {
                        assert!(
                            self.decoders.remove(&c),
                            "released {c}, which held no decoder"
                        );
                    }
                }
                assert!(self.decoders.len() <= 1, "more than one decoder allocated");
            }
        }
    }

    #[test]
    fn a_settled_hover_starts_exactly_one_preview() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        // The enter edge itself must allocate nothing.
        gtk.apply(policy.on_enter(1, base));
        assert_eq!(gtk.starts, 0, "a hover started decoding before it settled");

        gtk.apply(policy.on_tick(t(base, START)));
        assert_eq!(gtk.playing, Some(1));
        assert_eq!(gtk.starts, 1);
    }

    #[test]
    fn a_pointer_sweep_starts_nothing_until_it_settles() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        // Five cards crossed inside one grace period, ticking throughout as the
        // real timer would.
        for (i, card) in (1..=5).enumerate() {
            let now = t(base, i as u64 * 20);
            gtk.apply(policy.on_enter(card, now));
            gtk.apply(policy.on_tick(now));
            gtk.apply(policy.on_leave(card, t(base, i as u64 * 20 + 10)));
            gtk.apply(policy.on_tick(t(base, i as u64 * 20 + 10)));
        }
        assert_eq!(gtk.starts, 0, "a sweep spun up decoders it never showed");

        // The card the pointer actually stops on is the one that plays.
        gtk.apply(policy.on_enter(6, t(base, 100)));
        gtk.apply(policy.on_tick(t(base, SETTLED)));
        assert_eq!(gtk.starts, 1);
        assert_eq!(gtk.playing, Some(6));
    }

    #[test]
    fn entering_a_second_card_stops_and_releases_the_first() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_tick(t(base, START)));
        assert_eq!(gtk.playing, Some(1));

        gtk.apply(policy.on_leave(1, t(base, 400)));
        gtk.apply(policy.on_enter(2, t(base, 400)));
        // Card 1 loses its decoder before card 2 gets one — the order is the
        // cap: there is no instant at which both exist.
        let actions = policy.on_tick(t(base, 400 + START));
        assert_eq!(
            actions,
            vec![Action::Stop(1), Action::Release(1), Action::Start(2)]
        );
        gtk.apply(actions);
        assert_eq!(gtk.playing, Some(2));
        assert_eq!(gtk.decoders, HashSet::from([2]), "card 1 kept its decoder");
    }

    #[test]
    fn re_entering_inside_the_grace_window_keeps_the_preview_playing() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_tick(t(base, START)));

        // The flicker across the card's own Edit button: leave, then back
        // inside the grace window. Nothing may stop.
        gtk.apply(policy.on_leave(1, t(base, 400)));
        gtk.apply(policy.on_enter(1, t(base, 460)));
        gtk.apply(policy.on_tick(t(base, SETTLED)));
        assert_eq!(gtk.playing, Some(1), "a flicker tore down a live preview");
        assert_eq!(gtk.starts, 1, "a flicker restarted the decoder");
    }

    #[test]
    fn leaving_for_good_releases_the_decoder_rather_than_pausing_it() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_tick(t(base, START)));
        gtk.apply(policy.on_leave(1, t(base, 400)));

        let actions = policy.on_tick(t(base, 400 + GRACE));
        assert_eq!(actions, vec![Action::Stop(1), Action::Release(1)]);
        gtk.apply(actions);
        assert!(gtk.decoders.is_empty(), "the MediaFile survived the leave");
    }

    #[test]
    fn a_card_leaving_the_tree_releases_its_decoder() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_tick(t(base, START)));

        // `populate_library` pulls the card out mid-play; no leave ever arrives.
        gtk.apply(policy.on_gone(1));
        assert!(
            gtk.decoders.is_empty(),
            "a destroyed card stranded its pipeline"
        );

        // And the policy doesn't still believe card 1 owns the slot.
        gtk.apply(policy.on_enter(2, t(base, 400)));
        gtk.apply(policy.on_tick(t(base, SETTLED)));
        assert_eq!(gtk.playing, Some(2));
    }

    #[test]
    fn losing_the_window_stops_and_releases_the_live_preview_immediately() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_tick(t(base, START)));
        assert_eq!(gtk.playing, Some(1));

        // Focus lost / minimised with the pointer still on the card: no leave
        // arrives, and no grace period applies.
        let actions = policy.on_window_hidden();
        assert_eq!(actions, vec![Action::Stop(1), Action::Release(1)]);
        gtk.apply(actions);
        assert!(gtk.decoders.is_empty(), "an unseen preview kept decoding");
        assert_eq!(policy.next_deadline(), None, "a timer outlived the window");
    }

    #[test]
    fn no_hover_settles_while_the_window_is_away() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        // A hover in flight when focus goes must not start afterwards…
        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_window_hidden());
        gtk.apply(policy.on_tick(t(base, SETTLED)));
        // …nor may crossings delivered to the unfocused window.
        gtk.apply(policy.on_enter(2, t(base, 600)));
        gtk.apply(policy.on_tick(t(base, 600 + SETTLED)));
        assert_eq!(gtk.starts, 0, "decoded for a window nobody was looking at");

        // Back in focus: nothing restarts on its own, the next hover does.
        gtk.apply(policy.on_window_shown());
        gtk.apply(policy.on_tick(t(base, 2000)));
        assert_eq!(gtk.starts, 0);
        gtk.apply(policy.on_enter(2, t(base, 2000)));
        gtk.apply(policy.on_tick(t(base, 2000 + SETTLED)));
        assert_eq!(gtk.playing, Some(2));
    }

    #[test]
    fn hiding_an_idle_window_is_harmless_and_repeatable() {
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();
        // Focus notifications arrive in bursts; each must be idempotent.
        gtk.apply(policy.on_window_hidden());
        gtk.apply(policy.on_window_hidden());
        gtk.apply(policy.on_window_shown());
        gtk.apply(policy.on_window_shown());
        assert!(gtk.decoders.is_empty());
    }

    /// The invariant that would have caught the original bug: whatever the
    /// pointer does, at most one card holds a decoder, and once everything has
    /// settled with the pointer off every card, *nothing* does.
    #[test]
    fn at_most_one_decoder_is_live_after_any_sequence_of_events() {
        // Deterministic pseudo-random events; no rand dependency (see dsp.rs).
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();
        let mut ms = 0u64;
        // Which cards the pointer is currently inside, so the sequence stays
        // physically possible (no leave without an enter).
        let mut inside: HashSet<CardId> = HashSet::new();

        for _ in 0..2000 {
            ms += next() % 200;
            let now = t(base, ms);
            let card = next() % 6 + 1;
            match next() % 8 {
                0 | 1 => {
                    if inside.insert(card) {
                        gtk.apply(policy.on_enter(card, now));
                    }
                }
                2 => {
                    if inside.remove(&card) {
                        gtk.apply(policy.on_leave(card, now));
                    }
                }
                3 => gtk.apply(policy.on_window_hidden()),
                4 => gtk.apply(policy.on_window_shown()),
                5 => gtk.apply(policy.set_disabled(next() % 2 == 0)),
                _ => {
                    inside.remove(&card);
                    gtk.apply(policy.on_gone(card));
                }
            }
            // The real timer fires somewhere in here; the assertions live in
            // `FakeGtk::apply`, which sees every action either way.
            gtk.apply(policy.on_tick(now));
        }

        // Pointer off everything, all timers drained: no decoder may remain.
        gtk.apply(policy.on_window_shown());
        gtk.apply(policy.set_disabled(false));
        for card in inside.clone() {
            gtk.apply(policy.on_leave(card, t(base, ms)));
        }
        gtk.apply(policy.on_tick(t(base, ms + SETTLED)));
        assert_eq!(gtk.playing, None);
        assert!(
            gtk.decoders.is_empty(),
            "a decoder outlived the hover that created it"
        );
        assert!(
            gtk.starts > 0,
            "the generated sequence never started anything"
        );
    }

    #[test]
    fn a_hover_starts_after_the_start_debounce_and_not_a_moment_before() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        // The leave grace is shorter than the start debounce; hovering for the
        // length of the grace must not be enough.
        gtk.apply(policy.on_tick(t(base, GRACE)));
        gtk.apply(policy.on_tick(t(base, START - 1)));
        assert_eq!(gtk.starts, 0, "started before the debounce elapsed");
        gtk.apply(policy.on_tick(t(base, START)));
        assert_eq!(gtk.playing, Some(1));
    }

    #[test]
    fn a_pointer_that_lingers_less_than_the_debounce_starts_nothing() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        // Card after card, each held 250 ms: longer than the old 140 ms
        // debounce, shorter than the new one. A reader scanning a shelf does
        // exactly this, and it must cost nothing.
        for card in 1..=8u64 {
            let at = card * 250;
            gtk.apply(policy.on_enter(card, t(base, at)));
            gtk.apply(policy.on_tick(t(base, at + 249)));
            gtk.apply(policy.on_leave(card, t(base, at + 250)));
        }
        gtk.apply(policy.on_tick(t(base, 20_000)));
        assert_eq!(gtk.starts, 0, "a scan across the shelf started a decoder");
    }

    #[test]
    fn the_leave_grace_is_still_short() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_tick(t(base, START)));
        gtk.apply(policy.on_leave(1, t(base, 1000)));
        gtk.apply(policy.on_tick(t(base, 1000 + GRACE - 1)));
        assert_eq!(gtk.playing, Some(1), "released inside the grace window");
        gtk.apply(policy.on_tick(t(base, 1000 + GRACE)));
        assert_eq!(gtk.playing, None, "the leave waited for the start debounce");
        assert!(gtk.decoders.is_empty());
    }

    #[test]
    fn the_earlier_of_the_two_deadlines_is_the_one_to_wake_for() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        policy.on_enter(1, base);
        policy.on_tick(t(base, START));
        assert_eq!(policy.next_deadline(), None);

        // Card 1 plays; the pointer leaves it for card 2 ten ms later. The
        // release (grace) falls due long before card 2's start (debounce).
        policy.on_leave(1, t(base, 400));
        policy.on_enter(2, t(base, 410));
        assert_eq!(policy.next_deadline(), Some(t(base, 400 + GRACE)));
    }

    #[test]
    fn a_disabled_policy_never_starts_anything() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.set_disabled(true));
        gtk.apply(policy.on_enter(1, base));
        assert_eq!(
            policy.next_deadline(),
            None,
            "a disabled hover armed a timer"
        );
        gtk.apply(policy.on_tick(t(base, SETTLED)));
        assert_eq!(gtk.starts, 0);
        gtk.apply(policy.on_leave(1, t(base, SETTLED)));
        gtk.apply(policy.on_tick(t(base, 2 * SETTLED)));
        assert!(gtk.decoders.is_empty());
    }

    #[test]
    fn switching_previews_off_releases_the_live_card_and_the_hover_in_flight() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_tick(t(base, START)));
        gtk.apply(policy.on_enter(2, t(base, 400)));
        assert_eq!(gtk.playing, Some(1));

        let actions = policy.set_disabled(true);
        assert_eq!(actions, vec![Action::Stop(1), Action::Release(1)]);
        gtk.apply(actions);
        assert!(gtk.decoders.is_empty(), "the switch left a decoder running");
        // Card 2's hover was about to settle; the switch cancels it too.
        gtk.apply(policy.on_tick(t(base, 400 + SETTLED)));
        assert_eq!(gtk.starts, 1, "a hover settled after the switch went off");
        assert_eq!(policy.next_deadline(), None);
    }

    #[test]
    fn switching_previews_on_starts_nothing_until_the_next_hover() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.set_disabled(true));
        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.set_disabled(false));
        gtk.apply(policy.on_tick(t(base, SETTLED)));
        assert_eq!(gtk.starts, 0, "turning the switch on resurrected a hover");

        gtk.apply(policy.on_enter(1, t(base, 2000)));
        gtk.apply(policy.on_tick(t(base, 2000 + START)));
        assert_eq!(gtk.playing, Some(1));
        // Idempotent in both directions.
        gtk.apply(policy.set_disabled(false));
        assert_eq!(gtk.playing, Some(1));
    }

    #[test]
    fn disabling_an_idle_policy_is_harmless_and_repeatable() {
        let mut policy = PreviewPolicy::default();
        assert!(policy.set_disabled(true).is_empty());
        assert!(policy.set_disabled(true).is_empty());
        assert!(policy.set_disabled(false).is_empty());
    }

    #[test]
    fn a_superseded_timer_is_recognised_as_stale() {
        let base = Instant::now();
        let mut arm = TickArm::default();
        assert_eq!(
            arm.plan(None),
            None,
            "armed a timer with nothing to wait for"
        );

        // A start is timed first...
        let first = arm.plan(Some(t(base, 300))).expect("first timer");
        // ...a later deadline needs no second timer...
        assert_eq!(arm.plan(Some(t(base, 500))), None);
        assert_eq!(arm.plan(Some(t(base, 300))), None);
        // ...but an earlier one (a leave falling due) must supersede it.
        let second = arm.plan(Some(t(base, 150))).expect("earlier timer");
        assert_ne!(first.0, second.0);
        assert_eq!(second.1, t(base, 150));

        // The superseded timer fires first or last; either way it is ignored.
        assert!(!arm.fired(first.0));
        assert!(arm.fired(second.0));
        assert!(!arm.fired(second.0), "a timer fired twice");

        // Once it has fired the next deadline arms afresh.
        assert!(arm.plan(Some(t(base, 300))).is_some());
    }

    // ---- crash sentinel -----------------------------------------------------

    #[test]
    fn no_sentinel_means_the_last_run_ended_normally() {
        let alive = |_| panic!("no pid to look up");
        assert_eq!(sentinel_verdict(None, 100, alive), Verdict::Clean);
        // Garbage names no pid, so it cannot name a crash either.
        assert_eq!(sentinel_verdict(Some(""), 100, alive), Verdict::Clean);
        assert_eq!(sentinel_verdict(Some("   \n"), 100, alive), Verdict::Clean);
        assert_eq!(sentinel_verdict(Some("fresco"), 100, alive), Verdict::Clean);
    }

    #[test]
    fn a_sentinel_for_a_dead_pid_is_a_crash() {
        assert_eq!(
            sentinel_verdict(Some("4242\n"), 100, |_| false),
            Verdict::Crashed
        );
        assert_eq!(
            sentinel_verdict(Some(" 4242 "), 100, |_| false),
            Verdict::Crashed
        );
    }

    #[test]
    fn a_sentinel_for_a_live_fresco_is_left_alone() {
        assert_eq!(
            sentinel_verdict(Some("4242"), 100, |pid| pid == 4242),
            Verdict::Running
        );
    }

    #[test]
    fn a_sentinel_naming_this_very_process_is_stale() {
        // We have not written one yet, so it is a previous run's, pid reused.
        assert_eq!(
            sentinel_verdict(Some("100"), 100, |_| true),
            Verdict::Crashed
        );
    }

    #[test]
    fn only_a_crash_with_previews_on_turns_them_off_and_says_so() {
        assert_eq!(recovery(Verdict::Clean, true), Recovery::Nothing);
        assert_eq!(recovery(Verdict::Clean, false), Recovery::Nothing);
        assert_eq!(recovery(Verdict::Running, true), Recovery::Nothing);
        assert_eq!(recovery(Verdict::Crashed, true), Recovery::DisableAndTell);
        // Already off: nothing to turn off, nothing to announce - just tidy up.
        assert_eq!(recovery(Verdict::Crashed, false), Recovery::ClearOnly);
    }

    #[test]
    fn the_sentinel_file_round_trips_through_the_decision() {
        let dir = std::env::temp_dir().join(format!("fresco-sentinel-{}", std::process::id()));
        let path = dir.join("fresco").join("hover-active");
        std::fs::remove_dir_all(&dir).ok();

        write_sentinel(&path, 31337);
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, "31337");
        // A later launch, the writer long gone:
        assert_eq!(
            sentinel_verdict(Some(&on_disk), 1, |_| false),
            Verdict::Crashed
        );
        std::fs::remove_dir_all(&dir).ok();
        assert!(std::fs::read_to_string(&path).is_err());
    }

    // ---- signal cleanup -------------------------------------------------------

    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    /// Set in the re-executed copy of this test binary that plays the part of
    /// Fresco with a preview showing.
    const SIGNAL_CHILD_ENV: &str = "FRESCO_TEST_HOVER_SIGNAL_CHILD";

    /// `install_signal_cleanup` replaces the process-wide disposition of three
    /// signals, which a test must not do to the harness it runs in. So the test
    /// re-runs itself as a child process that installs the handler and arms the
    /// sentinel exactly as a preview does, and the parent signals that child
    /// from outside, the way a logout, `pkill` or Ctrl+C would.
    #[test]
    fn a_terminating_signal_clears_the_sentinel_and_still_ends_the_process() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::{Command, Stdio};

        if std::env::var_os(SIGNAL_CHILD_ENV).is_some() {
            install_signal_cleanup();
            arm_sentinel();
            // Wait to be signalled; only reached again if no signal kills us.
            std::thread::sleep(Duration::from_secs(30));
            std::process::exit(99);
        }

        for sig in [SIGHUP, SIGINT, SIGTERM] {
            let state =
                std::env::temp_dir().join(format!("fresco-signal-{}-{sig}", std::process::id()));
            std::fs::remove_dir_all(&state).ok();
            let sentinel = state.join("fresco").join("hover-active");

            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "gui::hover_preview::tests::a_terminating_signal_clears_the_sentinel_and_still_ends_the_process",
                    "--exact",
                    "--test-threads=1",
                ])
                .env(SIGNAL_CHILD_ENV, "1")
                .env("XDG_STATE_HOME", &state)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();

            // The child writes the sentinel after the handler is installed, so
            // its appearance means a signal is now safe to send.
            let deadline = Instant::now() + Duration::from_secs(20);
            while !sentinel.exists() {
                assert!(
                    Instant::now() < deadline,
                    "signal {sig}: the child never armed the sentinel"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            // SAFETY: plain `kill(2)` on a child we spawned and still hold.
            assert_eq!(unsafe { kill(child.id() as i32, sig) }, 0);

            let status = child.wait().unwrap();
            assert_eq!(
                status.signal(),
                Some(sig),
                "signal {sig}: the child must die of the signal, not exit ({status:?})"
            );
            assert!(
                !sentinel.exists(),
                "signal {sig}: the sentinel was left behind and would read as a crash"
            );
            std::fs::remove_dir_all(&state).ok();
        }
    }

    #[test]
    fn this_test_process_is_not_mistaken_for_fresco() {
        // The cargo test binary is not named `fresco`; the name check is what
        // keeps a recycled pid from reading as a live instance.
        assert!(!pid_is_fresco(std::process::id()));
        assert!(!pid_is_fresco(u32::MAX));
    }
}

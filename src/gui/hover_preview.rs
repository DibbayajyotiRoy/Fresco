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
//! Two rules keep it honest. Every hover edge is debounced, both of them, by
//! the same [`HOVER_GRACE`]: sweeping the pointer across a shelf of cards must
//! start *nothing*, and a flicker across a card's own Edit button must stop
//! nothing. And a card that leaves the widget tree releases its pipeline on the
//! way out, so `populate_library`'s "remove every child" sweep can never strand
//! one mid-decode.
//!
//! One subtlety is load-bearing enough to spell out: the `Picture` holds a
//! strong reference to the `MediaFile` (it is its paintable), so anything the
//! `MediaFile` holds pointing back at the `Picture` closes a reference cycle,
//! and GObject has no cycle collector. The `invalidate_contents` handler below
//! therefore captures the `Picture` **weakly**. Without that, none of the rest
//! of this module frees a thing.
//!
//! A preview nobody can see is pure cost, and on a machine without GStreamer
//! hardware-decode plugins (stock Deepin, for one) it is a large one: the
//! hovered file decodes at full source resolution in software, inside the GUI
//! process. So the toplevel window is watched too — losing focus, being
//! minimised or being hidden stops and releases the live preview through the
//! same policy ([`PreviewPolicy::on_window_hidden`]), and no hover may start a
//! new one until the window is back ([`PreviewPolicy::on_window_shown`]).
//!
//! Decoding stays best-effort: if no GStreamer plugins are installed the media
//! simply never produces frames and the card shows no motion — nothing breaks.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::EventControllerMotion;

/// Grace period applied to *both* hover edges.
///
/// On leave it debounces the glitch: moving the pointer across the card's
/// revealed Edit button / overlays emits brief leave→enter crossings, and
/// without the delay the preview swaps back and forth. On enter it is what
/// makes a pointer sweep across the library free — five cards crossed inside
/// one grace period spawn zero decoders, not five — which is also the thing
/// that protects a GPU with a handful of hardware-decode sessions.
const HOVER_GRACE: Duration = Duration::from_millis(140);

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
        if self.suppressed {
            return Vec::new();
        }
        // Nothing starts on the enter edge itself; the tick decides.
        self.pending = Some((card, now + HOVER_GRACE));
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
                if self.suppressed {
                    // Settled while the window was away (or the enter raced the
                    // focus change): drop it rather than decode unseen.
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
    /// Whether a timer is already on its way to call [`PreviewPolicy::on_tick`].
    tick_armed: bool,
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

/// Make sure a timer exists for the policy's next deadline.
///
/// At most one is ever outstanding. Every deadline is `now + HOVER_GRACE`, so a
/// later event can only ever push a deadline further out than the armed one —
/// which is why one timer is enough and no wakeup can be missed.
fn arm_tick() {
    let delay = PREVIEWS.with(|previews| {
        let mut previews = previews.borrow_mut();
        if previews.tick_armed {
            return None;
        }
        let deadline = previews.policy.next_deadline()?;
        previews.tick_armed = true;
        Some(deadline.saturating_duration_since(Instant::now()))
    });
    let Some(delay) = delay else { return };
    gtk4::glib::timeout_add_local_once(delay, || {
        PREVIEWS.with(|previews| previews.borrow_mut().tick_armed = false);
        dispatch(|policy| policy.on_tick(Instant::now()));
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

/// Attach hover-to-play to a card.
///
/// - `card`: the card root `gtk4::Overlay` (the hover target spanning the card).
///   Its base child must currently be `thumb`.
/// - `thumb`: the `gtk4::Picture` showing the static thumbnail. It stays the
///   card's size-driving base child forever; the video preview is layered above
///   it inside an inner `Overlay` so it can never trigger a relayout.
/// - `video`: the video/GIF file to preview.
///
/// Plays muted + looping while hovered, one card at a time process-wide, and
/// releases the decoder on leave. Degrades gracefully: if the media can't be
/// decoded (e.g. no GStreamer plugins installed) nothing bad happens — the card
/// simply shows no motion.
pub fn attach(card: &gtk4::Overlay, thumb: &gtk4::Picture, video: PathBuf) {
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

    // The card's decoder, for as long as it is the live one. `None` between
    // hovers is the point: an idle card owns no pipeline.
    let media: Rc<RefCell<Option<gtk4::MediaFile>>> = Rc::new(RefCell::new(None));
    // True once the current MediaFile has produced its first frame. Showing the
    // video layer BEFORE that blanks the card — for however long the decoder
    // takes, or forever when the codec's GStreamer plugin is missing. The
    // thumbnail must stay visible until real frames exist. Resets on release,
    // because the next hover starts a fresh MediaFile with no frames yet.
    let ready = Rc::new(Cell::new(false));
    // Whether this card is the live one, as far as the policy is concerned.
    // The first-frame callback consults it so a frame that lands after the
    // pointer has already moved on doesn't flash the preview up.
    let live = Rc::new(Cell::new(false));

    let apply: Apply = {
        let video_pic = video_pic.clone();
        let media = media.clone();
        let ready = ready.clone();
        let live = live.clone();
        Rc::new(move |action| match action {
            Action::Start(_) => {
                live.set(true);
                // Build (if needed) under a scoped borrow, then let go of it:
                // `play()` and `set_visible()` can call back into GTK, and a
                // callback that reaches another Action must find `media` free.
                // Cloning the MediaFile is a refcount bump, not a copy.
                let m = {
                    let mut slot = media.borrow_mut();
                    if slot.is_none() {
                        let m = gtk4::MediaFile::for_filename(video.to_string_lossy().as_ref());
                        m.set_muted(true);
                        m.set_loop(true);
                        video_pic.set_paintable(Some(&m));

                        // WEAK, and this is the leak fix. `set_paintable` above
                        // gave the Picture a strong reference to the MediaFile;
                        // capturing the Picture strongly here would point one
                        // back, and GObject has no cycle collector, so neither
                        // object would ever be finalised — the decoder, its
                        // hardware context and its textures would outlive the
                        // card, unreachable, for the life of the process.
                        let weak_pic = video_pic.downgrade();
                        let ready = ready.clone();
                        let live = live.clone();
                        m.connect_invalidate_contents(move |_| {
                            if ready.get() {
                                return;
                            }
                            ready.set(true);
                            // Card destroyed mid-decode: nothing left to reveal.
                            let Some(pic) = weak_pic.upgrade() else {
                                return;
                            };
                            if live.get() {
                                pic.set_visible(true);
                            }
                        });
                        *slot = Some(m);
                    }
                    slot.clone().expect("just inserted")
                };
                if ready.get() {
                    video_pic.set_visible(true);
                }
                m.play();
            }
            Action::Stop(_) => {
                live.set(false);
                let m = media.borrow().clone();
                video_pic.set_visible(false);
                if let Some(m) = m {
                    m.pause();
                }
            }
            Action::Release(_) => {
                live.set(false);
                ready.set(false);
                let m = media.borrow_mut().take();
                video_pic.set_visible(false);
                // Drop the Picture's half of the pair too, or the cleared
                // MediaFile stays referenced until the next hover replaces it.
                video_pic.set_paintable(None::<&gtk4::gdk::Paintable>);
                if let Some(m) = m {
                    m.pause();
                    // `clear()` — gtk_media_file_clear — is the public teardown:
                    // it closes the stream and unsets the file. (There is no
                    // `close()` in the bindings; gtk_media_file_close is a class
                    // vfunc.) The MediaFile itself is freed as `m` drops here.
                    m.clear();
                }
            }
        })
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
    const SETTLED: u64 = 500;

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

        gtk.apply(policy.on_tick(t(base, 140)));
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
        gtk.apply(policy.on_tick(t(base, 140)));
        assert_eq!(gtk.playing, Some(1));

        gtk.apply(policy.on_leave(1, t(base, 200)));
        gtk.apply(policy.on_enter(2, t(base, 200)));
        // Card 1 loses its decoder before card 2 gets one — the order is the
        // cap: there is no instant at which both exist.
        let actions = policy.on_tick(t(base, 400));
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
        gtk.apply(policy.on_tick(t(base, 140)));

        // The flicker across the card's own Edit button: leave, then back
        // inside the grace window. Nothing may stop.
        gtk.apply(policy.on_leave(1, t(base, 200)));
        gtk.apply(policy.on_enter(1, t(base, 260)));
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
        gtk.apply(policy.on_tick(t(base, 140)));
        gtk.apply(policy.on_leave(1, t(base, 200)));

        let actions = policy.on_tick(t(base, 400));
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
        gtk.apply(policy.on_tick(t(base, 140)));

        // `populate_library` pulls the card out mid-play; no leave ever arrives.
        gtk.apply(policy.on_gone(1));
        assert!(
            gtk.decoders.is_empty(),
            "a destroyed card stranded its pipeline"
        );

        // And the policy doesn't still believe card 1 owns the slot.
        gtk.apply(policy.on_enter(2, t(base, 200)));
        gtk.apply(policy.on_tick(t(base, SETTLED)));
        assert_eq!(gtk.playing, Some(2));
    }

    #[test]
    fn losing_the_window_stops_and_releases_the_live_preview_immediately() {
        let base = Instant::now();
        let mut policy = PreviewPolicy::default();
        let mut gtk = FakeGtk::default();

        gtk.apply(policy.on_enter(1, base));
        gtk.apply(policy.on_tick(t(base, 140)));
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
            match next() % 6 {
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
}

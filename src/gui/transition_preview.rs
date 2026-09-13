//! A looping, in-editor demonstration of a wallpaper transition.
//!
//! Given two demo frames (typically the slideshow's first two images) this
//! widget plays the chosen [`Transition`] over and over so the user can see the
//! effect before applying it. Animation is purely client-side: it nudges child
//! `opacity`, a CSS `filter: blur()` class, and `gsk` transforms (translate /
//! scale) on two stacked [`gtk4::Picture`]s laid out in a [`gtk4::Fixed`]
//! "stage". No media decoding, no daemon — just a ~30fps `glib` timeout
//! driving one frame at a time.
//!
//! Everything here is an *approximation* of what the daemon does to the one
//! running mpv player, and the approximations are chosen to be honest about
//! it: the fade dips through black because mpv drives `gamma` and cannot
//! cross-dissolve two files from a single decoder; zoom cuts at the peak of the
//! punch-in because that is where `loadfile` lands; blur softens out and back
//! because the daemon pushes a `gblur` filter and then clears it.
//!
//! The timer runs only while the stage is actually on screen. It used to be
//! armed whenever a transition was picked and to stop only on an explicit
//! [`TransitionPreview::stop`] or once the widget left the tree — and the
//! "left the tree" check sat *behind* a zero-size early return, so a stage
//! that was hidden (a non-slideshow entry's editor, where the frame is
//! invisible but the transition combo still drives this) woke the GUI 30
//! times a second forever. Now the stage's `unmap` removes the timer and its
//! `map` re-arms it, and each tick consults [`gate`] — a pure function, tested
//! below — which ends the loop the moment the stage is unrooted or unmapped
//! and skips the frame's work while the window is minimised. The editor still
//! calls [`TransitionPreview::stop`] explicitly when leaving.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{gio, glib, graphene, gsk};

use crate::config::Transition;

/// Length of the moving part of the effect, in seconds.
const DUR: f64 = 1.2;
/// How long the finished frame is held before the loop restarts, in seconds.
const HOLD: f64 = 0.6;
/// Full loop length: animate, then hold.
const CYCLE: f64 = DUR + HOLD;
/// Per-tick advance (~30fps).
const STEP: f64 = 0.033;
/// How far [`Transition::Zoom`] punches in before it cuts to the next frame.
/// The daemon travels 0.22 in mpv's log2 `video-zoom` units, which is this in
/// linear scale (`2^0.22`) — the same ~16% the desktop actually moves.
const ZOOM_PEAK: f32 = 1.165;
/// Defocus ladder, softest last. GTK's CSS `filter` takes a literal length, so
/// the steps are pre-declared in the stylesheet (see `theme.rs`) and picked by
/// class rather than rebuilt into a provider 30 times a second.
const BLUR_CLASSES: [&str; 6] = [
    "tp-blur-1",
    "tp-blur-2",
    "tp-blur-3",
    "tp-blur-4",
    "tp-blur-5",
    "tp-blur-6",
];

/// Mutable, shared animation state. GTK objects are refcounted, so the stored
/// widgets are cheap clones of the live ones.
struct State {
    first: Option<PathBuf>,
    second: Option<PathBuf>,
    transition: Transition,
    /// Phase within the loop, `0.0..=CYCLE`.
    t: f64,
    /// The running animation timer, if any.
    source: Option<glib::SourceId>,
    stage: gtk4::Fixed,
    pic_a: gtk4::Picture,
    pic_b: gtk4::Picture,
    /// Last (w, h) pushed to the pictures; avoids a relayout on every frame.
    last_size: (i32, i32),
    /// Defocus step currently applied to each picture, `0` for none. Tracked so
    /// a frame that does not change the blur touches no CSS class at all —
    /// swapping classes forces a style recompute, and doing that twice a frame
    /// for no reason is exactly the sort of cost this preview should not have.
    blur_step: (u8, u8),
    /// A transition was chosen and not since stopped: the loop *should* run
    /// whenever the stage is mapped. Separate from `source`, which says whether
    /// it *is* running — the difference is what lets `map` resume it.
    wanted: bool,
}

/// What a tick should do, decided from facts about the stage alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    /// The stage cannot be seen and will not be without a `map` (which re-arms
    /// the timer): end the loop so nothing wakes the process.
    Stop,
    /// Keep the timer but do no work this frame: not allocated yet, or the
    /// window is minimised (GTK4 does not unmap a minimised window, so there is
    /// no edge to stop on — skipping the frame is the cheap honest option).
    Idle,
    Animate,
}

/// The order matters and is the bug this replaced: an unrooted or unmapped
/// stage also reports a zero size, so the size check must come last or it
/// masks the "gone" cases and the timer never ends.
fn gate(rooted: bool, mapped: bool, minimized: bool, w: i32, h: i32) -> Gate {
    if !rooted || !mapped {
        Gate::Stop
    } else if minimized || w <= 0 || h <= 0 {
        Gate::Idle
    } else {
        Gate::Animate
    }
}

/// A looping preview of a slideshow transition between two images.
pub struct TransitionPreview {
    pub root: gtk4::Widget,
    state: Rc<RefCell<State>>,
}

impl TransitionPreview {
    pub fn new() -> Self {
        let stage = gtk4::Fixed::new();
        stage.set_overflow(gtk4::Overflow::Hidden);
        stage.add_css_class("wp-thumb");
        stage.add_css_class("crop-frame");
        stage.set_hexpand(true);
        stage.set_vexpand(true);

        // pic_a is the bottom layer, pic_b sits on top.
        let pic_a = gtk4::Picture::new();
        pic_a.set_can_shrink(true);
        pic_a.set_keep_aspect_ratio(true);
        let pic_b = gtk4::Picture::new();
        pic_b.set_can_shrink(true);
        pic_b.set_keep_aspect_ratio(true);

        stage.put(&pic_a, 0.0, 0.0);
        stage.put(&pic_b, 0.0, 0.0);

        let state = Rc::new(RefCell::new(State {
            first: None,
            second: None,
            transition: Transition::None,
            t: 0.0,
            source: None,
            stage: stage.clone(),
            pic_a,
            pic_b,
            last_size: (0, 0),
            blur_step: (0, 0),
            wanted: false,
        }));

        // Weak: the stage stores these closures and `State` stores the stage,
        // so a strong capture would be a cycle GObject never collects.
        {
            let weak = Rc::downgrade(&state);
            stage.connect_unmap(move |_| {
                if let Some(state) = weak.upgrade() {
                    if let Some(source) = state.borrow_mut().source.take() {
                        source.remove();
                    }
                }
            });
        }
        {
            let weak = Rc::downgrade(&state);
            stage.connect_map(move |_| {
                if let Some(state) = weak.upgrade() {
                    arm(&state);
                }
            });
        }

        let root = stage.upcast::<gtk4::Widget>();

        TransitionPreview { root, state }
    }

    /// Set the two demo frames (typically the slideshow's first two images).
    /// Either may be None.
    pub fn set_images(&self, first: Option<PathBuf>, second: Option<PathBuf>) {
        let mut state = self.state.borrow_mut();
        state.first = first;
        state.second = second;
        state.t = 0.0;
        load_into(&state.pic_a, state.first.as_ref());
        load_into(&state.pic_b, state.second.as_ref());
    }

    /// Choose which transition to demo and (re)start the loop. `Transition::None`
    /// shows a static first image with no animation.
    pub fn set_transition(&self, transition: Transition) {
        // Always cancel any running timer and reset to identity first.
        self.stop();

        {
            let mut state = self.state.borrow_mut();
            state.transition = transition;
            state.t = 0.0;
        }

        if transition == Transition::None {
            // Static first frame: pic_a visible, pic_b hidden, no animation.
            let state = self.state.borrow();
            state.pic_a.set_opacity(1.0);
            state.pic_b.set_opacity(0.0);
            state.stage.set_child_transform(&state.pic_a, None);
            state.stage.set_child_transform(&state.pic_b, None);
            clear_blur(&state.pic_a);
            clear_blur(&state.pic_b);
            return;
        }

        self.state.borrow_mut().wanted = true;
        arm(&self.state);
    }

    /// Stop the animation timer (call when leaving the editor).
    pub fn stop(&self) {
        let mut state = self.state.borrow_mut();
        state.wanted = false;
        if let Some(source) = state.source.take() {
            source.remove();
        }
        // Reset to sensible defaults: first frame shown, transforms cleared,
        // and — the one that would otherwise persist — the defocus removed. A
        // leftover blur class would leave the editor's still preview soft.
        state.pic_a.set_opacity(1.0);
        state.pic_b.set_opacity(0.0);
        state.stage.set_child_transform(&state.pic_a, None);
        state.stage.set_child_transform(&state.pic_b, None);
        clear_blur(&state.pic_a);
        clear_blur(&state.pic_b);
        state.blur_step = (0, 0);
    }
}

impl Default for TransitionPreview {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the frame timer if the loop is wanted, not already running, and the
/// stage is mapped. An unmapped stage arms nothing; its `map` calls back here.
fn arm(state_rc: &Rc<RefCell<State>>) {
    let mut state = state_rc.borrow_mut();
    if !state.wanted || state.source.is_some() || !state.stage.is_mapped() {
        return;
    }
    let weak = Rc::downgrade(state_rc);
    state.source = Some(glib::timeout_add_local(
        Duration::from_millis(33),
        move || match weak.upgrade() {
            Some(state) => tick(&state),
            None => glib::ControlFlow::Break,
        },
    ));
}

/// Whether the window holding `widget` reports itself minimised.
fn is_minimized(widget: &impl IsA<gtk4::Widget>) -> bool {
    widget
        .native()
        .and_then(|n| n.surface())
        .and_then(|s| s.downcast::<gtk4::gdk::Toplevel>().ok())
        .is_some_and(|t| t.state().contains(gtk4::gdk::ToplevelState::MINIMIZED))
}

/// Load `path` into `pic`, or clear it if `path` is None.
fn load_into(pic: &gtk4::Picture, path: Option<&PathBuf>) {
    match path {
        Some(path) => pic.set_file(Some(&gio::File::for_path(path))),
        None => pic.set_file(gio::File::NONE),
    }
}

/// Remove every defocus class from `pic`.
fn clear_blur(pic: &gtk4::Picture) {
    for class in BLUR_CLASSES {
        pic.remove_css_class(class);
    }
}

/// Apply defocus step `step` (`0` = sharp, `BLUR_CLASSES.len()` = softest) to
/// `pic`, doing nothing when it already carries that step. `cur` is the caller's
/// record of what is on the widget now and is updated in place.
fn set_blur(pic: &gtk4::Picture, cur: &mut u8, step: u8) {
    if *cur == step {
        return;
    }
    if let Some(old) = blur_class(*cur) {
        pic.remove_css_class(old);
    }
    if let Some(new) = blur_class(step) {
        pic.add_css_class(new);
    }
    *cur = step;
}

/// The CSS class for defocus step `step`, or `None` for step `0` (sharp).
/// Indexed through `get` rather than `[]`: the crate builds with
/// `panic = "abort"`, so an out-of-range step would take the whole app down
/// rather than merely showing the wrong amount of blur.
fn blur_class(step: u8) -> Option<&'static str> {
    BLUR_CLASSES.get((step as usize).checked_sub(1)?).copied()
}

/// `0.0..=1.0` softness onto a defocus step on the [`BLUR_CLASSES`] ladder.
fn blur_step_for(softness: f64) -> u8 {
    let steps = BLUR_CLASSES.len() as f64;
    (softness.clamp(0.0, 1.0) * steps).round().min(steps) as u8
}

/// Gentle acceleration and deceleration over `0.0..=1.0`.
///
/// The same curve the daemon eases every transition with
/// (`daemon::transition::ease_in_out_cubic`), duplicated rather than shared
/// because that module is behind the `daemon` feature and this one is behind
/// `gui` — a build with only the GUI must still compile. Copied exactly so the
/// preview's pacing is the pacing, not a lookalike.
fn ease_in_out(x: f64) -> f64 {
    if x < 0.5 {
        4.0 * x * x * x
    } else {
        1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
    }
}

/// A scale-about-center transform for a stage of `w` x `h`.
fn scale_about_center(w: i32, h: i32, s: f32) -> gsk::Transform {
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    gsk::Transform::new()
        .translate(&graphene::Point::new(cx, cy))
        .scale(s, s)
        .translate(&graphene::Point::new(-cx, -cy))
}

/// Drive one animation frame (~30fps). Returns `Break` once the widget leaves
/// the window so the timer cleans itself up.
fn tick(state_rc: &Rc<RefCell<State>>) -> glib::ControlFlow {
    let mut state = state_rc.borrow_mut();

    let w = state.stage.width();
    let h = state.stage.height();
    match gate(
        state.stage.root().is_some(),
        state.stage.is_mapped(),
        is_minimized(&state.stage),
        w,
        h,
    ) {
        Gate::Stop => {
            // Returning Break destroys the source; forget its id so `stop()`
            // does not try to remove it a second time. `map` re-arms.
            state.source = None;
            return glib::ControlFlow::Break;
        }
        Gate::Idle => return glib::ControlFlow::Continue,
        Gate::Animate => {}
    }

    // Advance the phase; on wrap, swap the two frames so the loop keeps moving
    // forward (B becomes the new A, etc.).
    state.t += STEP;
    if state.t >= CYCLE {
        state.t = 0.0;
        let st = &mut *state;
        std::mem::swap(&mut st.first, &mut st.second);
        load_into(&st.pic_a, st.first.as_ref());
        load_into(&st.pic_b, st.second.as_ref());
    }

    // Size both pictures to fill the stage, but only when the size actually
    // changes — doing it every frame would queue a relayout each tick and make
    // the whole editor jitter/resize during the animation.
    if (w, h) != state.last_size {
        state.last_size = (w, h);
        state.pic_a.set_size_request(w, h);
        state.pic_b.set_size_request(w, h);
    }

    // Progress through the moving part; stays at 1.0 during the hold.
    let p = (state.t / DUR).min(1.0);

    match state.transition {
        Transition::None => {}
        // Crossfade is a fade. It has never been anything else — mpv drives
        // `gamma` on one decoder and cannot show two files at once — and this
        // preview used to cross-dissolve them, promising an effect the daemon
        // could not deliver. Same arm, same dip through black.
        Transition::Fade | Transition::Crossfade => {
            // Fade out to black, then fade the next frame in.
            if p < 0.5 {
                state.pic_a.set_opacity(1.0 - p * 2.0);
                state.pic_b.set_opacity(0.0);
            } else {
                state.pic_a.set_opacity(0.0);
                state.pic_b.set_opacity((p - 0.5) * 2.0);
            }
            state.stage.set_child_transform(&state.pic_a, None);
            state.stage.set_child_transform(&state.pic_b, None);
        }
        Transition::Slide => {
            state.pic_a.set_opacity(1.0);
            state.pic_b.set_opacity(1.0);
            let a =
                gsk::Transform::new().translate(&graphene::Point::new(-(w as f32) * p as f32, 0.0));
            let b = gsk::Transform::new()
                .translate(&graphene::Point::new((w as f32) * (1.0 - p as f32), 0.0));
            state.stage.set_child_transform(&state.pic_a, Some(&a));
            state.stage.set_child_transform(&state.pic_b, Some(&b));
        }
        Transition::KenBurns => {
            // Slow zoom on the first frame only, scaled about its center.
            state.pic_a.set_opacity(1.0);
            state.pic_b.set_opacity(0.0);
            let s = 1.0 + 0.22 * p as f32;
            let cx = w as f32 / 2.0;
            let cy = h as f32 / 2.0;
            let a = gsk::Transform::new()
                .translate(&graphene::Point::new(cx, cy))
                .scale(s, s)
                .translate(&graphene::Point::new(-cx, -cy));
            state.stage.set_child_transform(&state.pic_a, Some(&a));
            state.stage.set_child_transform(&state.pic_b, None);
        }
        Transition::Zoom => {
            // Punch in on the outgoing frame, cut at the peak, settle out on
            // the incoming one — the daemon ramps `video-zoom` and issues its
            // `loadfile` at the top of the ramp, so the cut belongs there and
            // not at a dissolve. The incoming frame starts at the peak the
            // outgoing one reached, which is what puts the cut inside one
            // continuous move rather than between two.
            let (front, back, s) = if p < 0.5 {
                let k = ease_in_out(p * 2.0) as f32;
                (&state.pic_a, &state.pic_b, 1.0 + (ZOOM_PEAK - 1.0) * k)
            } else {
                let k = ease_in_out((p - 0.5) * 2.0) as f32;
                (
                    &state.pic_b,
                    &state.pic_a,
                    ZOOM_PEAK - (ZOOM_PEAK - 1.0) * k,
                )
            };
            front.set_opacity(1.0);
            back.set_opacity(0.0);
            let t = scale_about_center(w, h, s);
            state.stage.set_child_transform(front, Some(&t));
            state.stage.set_child_transform(back, None);
        }
        Transition::Blur => {
            // Defocus out, swap at the softest point, focus back in — sigma
            // ramping up over the first half and back down over the second,
            // exactly as the daemon ramps `lavfi=[gblur=sigma=…]` and then
            // clears it. `vf` is a player property rather than a per-file one,
            // so over there the incoming media arrives already defocused and
            // the swap is never seen; here the swap lands at peak blur for the
            // same reason. Nothing scales: the real effect is defocus only.
            let soft = if p < 0.5 {
                ease_in_out(p * 2.0)
            } else {
                ease_in_out((1.0 - p) * 2.0)
            };
            let step = blur_step_for(soft);
            let st = &mut *state;
            let (front, back, front_step, back_step) = if p < 0.5 {
                (
                    &st.pic_a,
                    &st.pic_b,
                    &mut st.blur_step.0,
                    &mut st.blur_step.1,
                )
            } else {
                (
                    &st.pic_b,
                    &st.pic_a,
                    &mut st.blur_step.1,
                    &mut st.blur_step.0,
                )
            };
            front.set_opacity(1.0);
            back.set_opacity(0.0);
            set_blur(front, front_step, step);
            set_blur(back, back_step, 0);
            st.stage.set_child_transform(front, None);
            st.stage.set_child_transform(back, None);
        }
    }

    glib::ControlFlow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An off-screen stage must end the loop even though it also reports a zero
    /// size — the old ordering let the size check swallow that and spin forever.
    #[test]
    fn a_hidden_or_detached_stage_stops_the_timer_whatever_its_size() {
        assert_eq!(gate(true, false, false, 0, 0), Gate::Stop, "hidden stage kept ticking");
        assert_eq!(gate(false, false, false, 0, 0), Gate::Stop, "detached stage kept ticking");
        assert_eq!(gate(false, true, false, 640, 360), Gate::Stop);
        assert_eq!(gate(true, false, false, 640, 360), Gate::Stop);
        assert_eq!(gate(true, true, true, 640, 360), Gate::Idle, "minimised window animated");
        assert_eq!(gate(true, true, false, 0, 360), Gate::Idle);
        assert_eq!(gate(true, true, false, 640, 360), Gate::Animate);
    }

    /// The defocus must start sharp, peak exactly where the frames swap, and
    /// finish sharp. The last of those is the one that matters beyond looks: a
    /// ramp that ended anywhere but zero would leave a blur class on the
    /// picture, and the editor's still preview would stay soft — the same
    /// "leave nothing behind" rule the daemon's own machine is built around.
    #[test]
    fn the_defocus_ramp_starts_and_ends_sharp_and_peaks_at_the_swap() {
        let softness = |p: f64| {
            if p < 0.5 {
                ease_in_out(p * 2.0)
            } else {
                ease_in_out((1.0 - p) * 2.0)
            }
        };
        assert_eq!(blur_step_for(softness(0.0)), 0);
        assert_eq!(blur_step_for(softness(1.0)), 0);
        assert_eq!(
            blur_step_for(softness(0.5)),
            BLUR_CLASSES.len() as u8,
            "the swap must land on the softest step, which is what hides it"
        );

        // Monotonic up to the swap and back down after it, so the ramp never
        // visibly stutters.
        let mut prev = 0;
        for i in 0..=50 {
            let step = blur_step_for(softness(i as f64 / 100.0));
            assert!(step >= prev, "softness went backwards before the swap");
            prev = step;
        }
        for i in 50..=100 {
            let step = blur_step_for(softness(i as f64 / 100.0));
            assert!(step <= prev, "softness went up again after the swap");
            prev = step;
        }
    }

    /// Every step the ramp can produce must name a class the stylesheet
    /// actually declares, and step 0 must name none at all.
    #[test]
    fn every_blur_step_maps_to_a_declared_class() {
        assert_eq!(blur_class(0), None, "step 0 is sharp and carries no class");
        for step in 1..=BLUR_CLASSES.len() as u8 {
            assert_eq!(blur_class(step), Some(BLUR_CLASSES[step as usize - 1]));
        }
        // Out of range degrades to "no blur" instead of aborting the process.
        assert_eq!(blur_class(BLUR_CLASSES.len() as u8 + 1), None);
    }

    /// The zoom must be a single continuous move through the cut: both halves
    /// meet at the punch peak, and both ends sit at rest.
    #[test]
    fn the_zoom_punch_is_continuous_across_the_cut() {
        let out = |p: f64| 1.0 + (ZOOM_PEAK as f64 - 1.0) * ease_in_out(p * 2.0);
        let back =
            |p: f64| ZOOM_PEAK as f64 - (ZOOM_PEAK as f64 - 1.0) * ease_in_out((p - 0.5) * 2.0);
        assert!((out(0.0) - 1.0).abs() < 1e-9);
        assert!((out(0.5) - ZOOM_PEAK as f64).abs() < 1e-9);
        assert!((back(0.5) - ZOOM_PEAK as f64).abs() < 1e-9);
        assert!((back(1.0) - 1.0).abs() < 1e-9);
    }
}

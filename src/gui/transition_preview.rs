//! A looping, in-editor demonstration of a slideshow transition.
//!
//! Given two demo frames (typically the slideshow's first two images) this
//! widget plays the chosen [`Transition`] over and over so the user can see the
//! effect before applying it. Animation is purely client-side: it nudges child
//! `opacity` and `gsk` transforms (translate / scale) on two stacked
//! [`gtk4::Picture`]s laid out in a [`gtk4::Fixed`] "stage". No media decoding,
//! no daemon — just a ~30fps `glib` timeout driving one frame at a time.
//!
//! The loop is self-capping: each tick checks whether the stage is still rooted
//! in a window and returns [`gtk4::glib::ControlFlow::Break`] once the widget
//! leaves the tree, so the timer dies with the widget rather than leaking. The
//! editor should also call [`TransitionPreview::stop`] explicitly when leaving.

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
        }));

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
            return;
        }

        let state_rc = self.state.clone();
        let source = glib::timeout_add_local(Duration::from_millis(33), move || tick(&state_rc));
        self.state.borrow_mut().source = Some(source);
    }

    /// Stop the animation timer (call when leaving the editor).
    pub fn stop(&self) {
        let mut state = self.state.borrow_mut();
        if let Some(source) = state.source.take() {
            source.remove();
        }
        // Reset to sensible defaults: first frame shown, transforms cleared.
        state.pic_a.set_opacity(1.0);
        state.pic_b.set_opacity(0.0);
        state.stage.set_child_transform(&state.pic_a, None);
        state.stage.set_child_transform(&state.pic_b, None);
    }
}

impl Default for TransitionPreview {
    fn default() -> Self {
        Self::new()
    }
}

/// Load `path` into `pic`, or clear it if `path` is None.
fn load_into(pic: &gtk4::Picture, path: Option<&PathBuf>) {
    match path {
        Some(path) => pic.set_file(Some(&gio::File::for_path(path))),
        None => pic.set_file(gio::File::NONE),
    }
}

/// Drive one animation frame (~30fps). Returns `Break` once the widget leaves
/// the window so the timer cleans itself up.
fn tick(state_rc: &Rc<RefCell<State>>) -> glib::ControlFlow {
    let mut state = state_rc.borrow_mut();

    // Stage not realized yet: wait for a real size before animating.
    let w = state.stage.width();
    let h = state.stage.height();
    if w == 0 || h == 0 {
        return glib::ControlFlow::Continue;
    }
    // Widget removed from the window: stop the timer so it doesn't leak.
    if state.stage.root().is_none() {
        return glib::ControlFlow::Break;
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
        Transition::Crossfade => {
            state.pic_a.set_opacity(1.0);
            state.pic_b.set_opacity(p);
            state.stage.set_child_transform(&state.pic_a, None);
            state.stage.set_child_transform(&state.pic_b, None);
        }
        Transition::Fade => {
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
    }

    glib::ControlFlow::Continue
}

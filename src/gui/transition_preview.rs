//! A looping, in-editor demonstration of a wallpaper transition.
//!
//! Given two demo frames (typically the slideshow's first two images) this
//! widget plays the chosen [`Transition`] over and over so the user can see the
//! effect before applying it. Animation is purely client-side: a ~30fps `glib`
//! timeout advances a phase, turns it into per-frame parameters (opacity, slide
//! offset, scale, defocus) and asks a small custom widget, [`Stage`], to
//! repaint. The stage draws both frames itself in `snapshot()` — cover-fit,
//! clipped to its own bounds — so no child widget, CSS filter or
//! `gtk4::Fixed` is involved. No media decoding, no daemon.
//!
//! Why a custom widget: the stage used to be a `Fixed` holding two `Picture`s,
//! and every tick pinned the pictures' minimum size to the stage's own
//! allocation. `Fixed` measures the *transformed* bounds of its children, so a
//! Ken Burns / Zoom / Slide transform pushed the minimum above the allocation,
//! the stage grew, the next tick pinned the pictures even larger, and the whole
//! window ballooned until it covered the screen (issue #31). [`Stage`] now
//! answers a constant from `measure` that depends on neither its content nor
//! its allocation, and the tick only ever calls `queue_draw`, so the enclosing
//! 16:9 `AspectFrame` alone decides the size and nothing can feed back.
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

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{gdk, glib, graphene, gsk};

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
/// Defocus ladder, softest last, as blur radii in px. These are the values the
/// `.tp-blur-N` CSS classes used to declare (`filter: blur(Npx)`); the ladder
/// stays a set of discrete steps so the softness is quantised exactly as it
/// was, now applied with `push_blur` in the snapshot instead of a class swap.
const BLUR_RADII: [f64; 6] = [2.0, 4.0, 7.0, 10.0, 14.0, 18.0];
/// Preview textures are decoded to fit this box. The preview shows a few
/// hundred pixels across; keeping a 4K or 8K original as a texture would cost
/// tens of MB of RAM and GPU memory per frame for nothing.
const DECODE_W: i32 = 1280;
const DECODE_H: i32 = 720;
/// Corner radius of the stage's frame, matching `.crop-frame` in `theme.rs`.
const CORNER_RADIUS: f32 = 12.0;

/// How one of the two frames is drawn this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameParams {
    opacity: f64,
    /// Horizontal offset as a fraction of the stage width (slide).
    dx: f32,
    /// Scale about the stage's center.
    scale: f32,
    /// Defocus step on the [`BLUR_RADII`] ladder, `0` for sharp.
    blur_step: u8,
}

impl FrameParams {
    const SHOWN: Self = Self {
        opacity: 1.0,
        dx: 0.0,
        scale: 1.0,
        blur_step: 0,
    };
    const HIDDEN: Self = Self {
        opacity: 0.0,
        ..Self::SHOWN
    };
}

/// The resting look: first frame shown, second hidden, nothing transformed.
const REST: [FrameParams; 2] = [FrameParams::SHOWN, FrameParams::HIDDEN];

mod imp {
    use super::*;

    /// Paints the two demo frames. Holds only what `snapshot` reads; the tick
    /// updates it and calls `queue_draw`, never anything that could resize.
    pub struct Stage {
        pub textures: RefCell<[Option<gdk::Texture>; 2]>,
        pub frames: Cell<[FrameParams; 2]>,
    }

    impl Default for Stage {
        fn default() -> Self {
            Self {
                textures: RefCell::new([None, None]),
                frames: Cell::new(REST),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Stage {
        const NAME: &'static str = "FrescoTransitionStage";
        type Type = super::Stage;
        type ParentType = gtk4::Widget;
    }

    impl ObjectImpl for Stage {}

    impl WidgetImpl for Stage {
        /// A constant, whatever the orientation, `for_size`, textures or current
        /// allocation. This is the whole fix for #31: the enclosing
        /// `AspectFrame` decides the stage's size, and nothing the stage paints
        /// (or how it is transformed while painting) can ask for more room.
        fn measure(&self, orientation: gtk4::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let (min, nat) = match orientation {
                gtk4::Orientation::Horizontal => (32, 320),
                _ => (18, 180),
            };
            (min, nat, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            let ctx = widget.style_context();
            snapshot.render_background(&ctx, 0.0, 0.0, w as f64, h as f64);

            // Clip to the rounded frame: Ken Burns / Zoom scale past the
            // bounds and Slide parks a frame entirely outside them, and none of
            // that may show.
            let bounds = graphene::Rect::new(0.0, 0.0, w, h);
            snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(bounds, CORNER_RADIUS));

            let textures = self.textures.borrow();
            let frames = self.frames.get();
            // Index 0 is the bottom layer, 1 sits on top.
            for (texture, frame) in textures.iter().zip(frames.iter()) {
                let Some(texture) = texture else { continue };
                if frame.opacity <= 0.0 {
                    continue;
                }
                let Some((x, y, tw, th)) =
                    cover_rect(texture.width() as f32, texture.height() as f32, w, h)
                else {
                    continue;
                };
                snapshot.push_opacity(frame.opacity);
                let radius = blur_radius(frame.blur_step);
                if radius > 0.0 {
                    snapshot.push_blur(radius);
                }
                snapshot.save();
                snapshot.translate(&graphene::Point::new(frame.dx * w, 0.0));
                let (cx, cy) = (w / 2.0, h / 2.0);
                snapshot.translate(&graphene::Point::new(cx, cy));
                snapshot.scale(frame.scale, frame.scale);
                snapshot.translate(&graphene::Point::new(-cx, -cy));
                snapshot.append_texture(texture, &graphene::Rect::new(x, y, tw, th));
                snapshot.restore();
                if radius > 0.0 {
                    snapshot.pop();
                }
                snapshot.pop();
            }

            snapshot.pop();
            snapshot.render_frame(&ctx, 0.0, 0.0, w as f64, h as f64);
        }
    }
}

glib::wrapper! {
    /// The preview's drawing surface; see the module docs.
    pub struct Stage(ObjectSubclass<imp::Stage>) @extends gtk4::Widget;
}

impl Stage {
    fn new() -> Self {
        glib::Object::new()
    }

    fn set_textures(&self, a: Option<gdk::Texture>, b: Option<gdk::Texture>) {
        *self.imp().textures.borrow_mut() = [a, b];
        self.queue_draw();
    }

    /// Swap the two frames (B becomes the new A) without re-decoding anything.
    fn swap_textures(&self) {
        self.imp().textures.borrow_mut().swap(0, 1);
    }

    fn set_frames(&self, frames: [FrameParams; 2]) {
        self.imp().frames.set(frames);
        self.queue_draw();
    }
}

/// Mutable, shared animation state. GTK objects are refcounted, so the stored
/// stage is a cheap clone of the live one.
struct State {
    transition: Transition,
    /// Phase within the loop, `0.0..=CYCLE`.
    t: f64,
    /// The running animation timer, if any.
    source: Option<glib::SourceId>,
    stage: Stage,
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
/// masks the "gone" cases and the timer never ends. The size is only asked
/// "is there one yet", never used to size anything.
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
        let stage = Stage::new();
        stage.add_css_class("wp-thumb");
        stage.add_css_class("crop-frame");
        stage.set_hexpand(true);
        stage.set_vexpand(true);

        let state = Rc::new(RefCell::new(State {
            transition: Transition::None,
            t: 0.0,
            source: None,
            stage: stage.clone(),
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
        state.t = 0.0;
        // Decoded on the main thread, as before: `Pixbuf` is not `Send`, and
        // `from_file_at_scale` uses the decoder's own downscaling, so a 4K JPEG
        // costs about what a 720p one does.
        state.stage.set_textures(
            first.as_deref().and_then(load_texture),
            second.as_deref().and_then(load_texture),
        );
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

        // None is a static first frame, which `stop` already left on the stage.
        if transition == Transition::None {
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
        // Back to the first frame, untransformed and — the one that would
        // otherwise persist — undefocused, so the editor's still preview is
        // sharp. Nothing here touches the stage's size: it never has any
        // requests to undo.
        state.stage.set_frames(REST);
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
        .and_then(|s| s.downcast::<gdk::Toplevel>().ok())
        .is_some_and(|t| t.state().contains(gdk::ToplevelState::MINIMIZED))
}

/// Decode `path` for display: downscaled to fit [`DECODE_W`]x[`DECODE_H`]
/// (aspect kept), EXIF-rotated, and only then uploaded, so a full-resolution
/// texture is never retained. `None` when the file cannot be decoded — the
/// stage then simply draws nothing for that frame.
fn load_texture(path: &Path) -> Option<gdk::Texture> {
    let pixbuf =
        gdk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, DECODE_W, DECODE_H, true).ok()?;
    let pixbuf = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

/// Blur radius for defocus step `step`, `0.0` for step `0` (sharp).
/// Indexed through `get` rather than `[]`: the crate builds with
/// `panic = "abort"`, so an out-of-range step would take the whole app down
/// rather than merely showing the wrong amount of blur.
fn blur_radius(step: u8) -> f64 {
    (step as usize)
        .checked_sub(1)
        .and_then(|i| BLUR_RADII.get(i))
        .copied()
        .unwrap_or(0.0)
}

/// `0.0..=1.0` softness onto a defocus step on the [`BLUR_RADII`] ladder.
fn blur_step_for(softness: f64) -> u8 {
    let steps = BLUR_RADII.len() as f64;
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

/// The rectangle `(x, y, w, h)` that covers a `dw` x `dh` area with a `tw` x `th`
/// texture, aspect kept and centred (the overflow is cropped by the stage's
/// clip). `None` for any non-positive dimension.
fn cover_rect(tw: f32, th: f32, dw: f32, dh: f32) -> Option<(f32, f32, f32, f32)> {
    if tw <= 0.0 || th <= 0.0 || dw <= 0.0 || dh <= 0.0 {
        return None;
    }
    let s = (dw / tw).max(dh / th);
    let (w, h) = (tw * s, th * s);
    Some(((dw - w) / 2.0, (dh - h) / 2.0, w, h))
}

/// `0.0..=1.0` softness of the [`Transition::Blur`] ramp at progress `p`.
fn blur_softness(p: f64) -> f64 {
    if p < 0.5 {
        ease_in_out(p * 2.0)
    } else {
        ease_in_out((1.0 - p) * 2.0)
    }
}

/// Both frames' parameters for `transition` at progress `p` (`0.0..=1.0`
/// through the moving part; it stays `1.0` during the hold).
fn frame_params(transition: Transition, p: f64) -> [FrameParams; 2] {
    let shown = FrameParams::SHOWN;
    let hidden = FrameParams::HIDDEN;
    match transition {
        Transition::None => REST,
        // Crossfade is a fade. It has never been anything else — mpv drives
        // `gamma` on one decoder and cannot show two files at once — and this
        // preview used to cross-dissolve them, promising an effect the daemon
        // could not deliver. Same arm, same dip through black.
        Transition::Fade | Transition::Crossfade => {
            // Fade out to black, then fade the next frame in.
            if p < 0.5 {
                [
                    FrameParams {
                        opacity: 1.0 - p * 2.0,
                        ..shown
                    },
                    hidden,
                ]
            } else {
                [
                    hidden,
                    FrameParams {
                        opacity: (p - 0.5) * 2.0,
                        ..shown
                    },
                ]
            }
        }
        Transition::Slide => [
            FrameParams {
                dx: -(p as f32),
                ..shown
            },
            FrameParams {
                dx: 1.0 - p as f32,
                ..shown
            },
        ],
        // Slow zoom on the first frame only, scaled about its center.
        Transition::KenBurns => [
            FrameParams {
                scale: 1.0 + 0.22 * p as f32,
                ..shown
            },
            hidden,
        ],
        Transition::Zoom => {
            // Punch in on the outgoing frame, cut at the peak, settle out on
            // the incoming one — the daemon ramps `video-zoom` and issues its
            // `loadfile` at the top of the ramp, so the cut belongs there and
            // not at a dissolve. The incoming frame starts at the peak the
            // outgoing one reached, which is what puts the cut inside one
            // continuous move rather than between two.
            if p < 0.5 {
                let k = ease_in_out(p * 2.0) as f32;
                [
                    FrameParams {
                        scale: 1.0 + (ZOOM_PEAK - 1.0) * k,
                        ..shown
                    },
                    hidden,
                ]
            } else {
                let k = ease_in_out((p - 0.5) * 2.0) as f32;
                [
                    hidden,
                    FrameParams {
                        scale: ZOOM_PEAK - (ZOOM_PEAK - 1.0) * k,
                        ..shown
                    },
                ]
            }
        }
        Transition::Blur => {
            // Defocus out, swap at the softest point, focus back in — sigma
            // ramping up over the first half and back down over the second,
            // exactly as the daemon ramps `lavfi=[gblur=sigma=…]` and then
            // clears it. `vf` is a player property rather than a per-file one,
            // so over there the incoming media arrives already defocused and
            // the swap is never seen; here the swap lands at peak blur for the
            // same reason. Nothing scales: the real effect is defocus only.
            let blurred = FrameParams {
                blur_step: blur_step_for(blur_softness(p)),
                ..shown
            };
            if p < 0.5 {
                [blurred, hidden]
            } else {
                [hidden, blurred]
            }
        }
    }
}

/// Drive one animation frame (~30fps). Returns `Break` once the widget leaves
/// the window so the timer cleans itself up.
fn tick(state_rc: &Rc<RefCell<State>>) -> glib::ControlFlow {
    let mut state = state_rc.borrow_mut();

    match gate(
        state.stage.root().is_some(),
        state.stage.is_mapped(),
        is_minimized(&state.stage),
        state.stage.width(),
        state.stage.height(),
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
        state.stage.swap_textures();
    }

    // Progress through the moving part; stays at 1.0 during the hold. Only
    // state and a repaint request follow — never a resize request.
    let p = (state.t / DUR).min(1.0);
    state.stage.set_frames(frame_params(state.transition, p));

    glib::ControlFlow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An off-screen stage must end the loop even though it also reports a zero
    /// size — the old ordering let the size check swallow that and spin forever.
    #[test]
    fn a_hidden_or_detached_stage_stops_the_timer_whatever_its_size() {
        assert_eq!(
            gate(true, false, false, 0, 0),
            Gate::Stop,
            "hidden stage kept ticking"
        );
        assert_eq!(
            gate(false, false, false, 0, 0),
            Gate::Stop,
            "detached stage kept ticking"
        );
        assert_eq!(gate(false, true, false, 640, 360), Gate::Stop);
        assert_eq!(gate(true, false, false, 640, 360), Gate::Stop);
        assert_eq!(
            gate(true, true, true, 640, 360),
            Gate::Idle,
            "minimised window animated"
        );
        assert_eq!(gate(true, true, false, 0, 360), Gate::Idle);
        assert_eq!(gate(true, true, false, 640, 360), Gate::Animate);
    }

    /// The defocus must start sharp, peak exactly where the frames swap, and
    /// finish sharp. The last of those is the one that matters beyond looks: a
    /// ramp that ended anywhere but zero would leave the picture soft, and the
    /// editor's still preview would stay soft — the same "leave nothing behind"
    /// rule the daemon's own machine is built around.
    #[test]
    fn the_defocus_ramp_starts_and_ends_sharp_and_peaks_at_the_swap() {
        let softness = blur_softness;
        assert_eq!(blur_step_for(softness(0.0)), 0);
        assert_eq!(blur_step_for(softness(1.0)), 0);
        assert_eq!(
            blur_step_for(softness(0.5)),
            BLUR_RADII.len() as u8,
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

    /// Every step the ramp can produce must name a radius, step 0 must name
    /// none, and the ladder must only ever get softer.
    #[test]
    fn every_blur_step_maps_to_a_radius() {
        assert_eq!(blur_radius(0), 0.0, "step 0 is sharp");
        for step in 1..=BLUR_RADII.len() as u8 {
            assert_eq!(blur_radius(step), BLUR_RADII[step as usize - 1]);
        }
        assert!(BLUR_RADII.windows(2).all(|w| w[0] < w[1]));
        // Out of range degrades to "no blur" instead of aborting the process.
        assert_eq!(blur_radius(BLUR_RADII.len() as u8 + 1), 0.0);
    }

    /// The zoom must be a single continuous move through the cut: both halves
    /// meet at the punch peak, and both ends sit at rest.
    #[test]
    fn the_zoom_punch_is_continuous_across_the_cut() {
        let scale_at = |p: f64| {
            let f = frame_params(Transition::Zoom, p);
            if p < 0.5 {
                f[0].scale
            } else {
                f[1].scale
            }
        };
        assert!((scale_at(0.0) - 1.0).abs() < 1e-6);
        assert!((scale_at(0.4999) - ZOOM_PEAK).abs() < 1e-3);
        assert!((scale_at(0.5) - ZOOM_PEAK).abs() < 1e-6);
        assert!((scale_at(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cover_rect_fills_the_area_and_centres_the_overflow() {
        // Wider than the area: height matches, sides are cropped equally.
        let (x, y, w, h) = cover_rect(200.0, 100.0, 100.0, 100.0).unwrap();
        assert_eq!((y, h), (0.0, 100.0));
        assert_eq!((x, w), (-50.0, 200.0));
        // Taller than the area: width matches, top and bottom are cropped.
        let (x, y, w, h) = cover_rect(100.0, 200.0, 100.0, 100.0).unwrap();
        assert_eq!((x, w), (0.0, 100.0));
        assert_eq!((y, h), (-50.0, 200.0));
        // Same aspect: exact fit.
        assert_eq!(
            cover_rect(1280.0, 720.0, 640.0, 360.0),
            Some((0.0, 0.0, 640.0, 360.0))
        );
        assert_eq!(cover_rect(0.0, 10.0, 10.0, 10.0), None);
        assert_eq!(cover_rect(10.0, 10.0, 0.0, 10.0), None);
    }

    #[test]
    fn each_transition_starts_at_rest_and_moves_as_documented() {
        // Fade dips through black at the half-way point.
        let f = frame_params(Transition::Fade, 0.5);
        assert_eq!((f[0].opacity, f[1].opacity), (0.0, 0.0));
        let f = frame_params(Transition::Fade, 0.25);
        assert!((f[0].opacity - 0.5).abs() < 1e-9 && f[1].opacity == 0.0);
        let f = frame_params(Transition::Crossfade, 1.0);
        assert_eq!((f[0].opacity, f[1].opacity), (0.0, 1.0));

        // Slide: A leaves left as B arrives from the right, both fully opaque.
        let f = frame_params(Transition::Slide, 0.25);
        assert!((f[0].dx + 0.25).abs() < 1e-6 && (f[1].dx - 0.75).abs() < 1e-6);
        assert_eq!((f[0].opacity, f[1].opacity), (1.0, 1.0));

        // Ken Burns: only the first frame, growing to 1.22.
        let f = frame_params(Transition::KenBurns, 0.0);
        assert_eq!((f[0].scale, f[1].opacity), (1.0, 0.0));
        let f = frame_params(Transition::KenBurns, 1.0);
        assert!((f[0].scale - 1.22).abs() < 1e-6);

        // Blur: sharp at the ends, softest at the swap, and the frame that is
        // shown flips with it; nothing scales or slides.
        assert_eq!(frame_params(Transition::Blur, 0.0)[0].blur_step, 0);
        let f = frame_params(Transition::Blur, 0.75);
        assert_eq!(f[0].opacity, 0.0);
        assert!(f[1].blur_step > 0 && f[1].scale == 1.0 && f[1].dx == 0.0);
        assert_eq!(frame_params(Transition::Blur, 0.4999)[0].blur_step, 6);
        assert_eq!(frame_params(Transition::None, 0.7), REST);
    }

    /// Regression for #31: the stage must not grow while Ken Burns / Zoom /
    /// Slide run, and must not stay enlarged afterwards. Needs a display, so
    /// run it with `xvfb-run -a cargo test transition_preview_does_not_grow --
    /// --ignored`.
    #[gtk4::test]
    #[ignore = "needs a display"]
    fn transition_preview_does_not_grow_the_window() {
        let dir = std::env::temp_dir().join(format!("fresco-tp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut paths = Vec::new();
        for (i, shade) in [60u8, 200].into_iter().enumerate() {
            // 1080p is what inflated the old minimum height.
            let pb = gdk::gdk_pixbuf::Pixbuf::new(
                gdk::gdk_pixbuf::Colorspace::Rgb,
                false,
                8,
                1920,
                1080,
            )
            .unwrap();
            pb.fill(u32::from_be_bytes([shade, 90, 255 - shade, 255]));
            let path = dir.join(format!("{i}.png"));
            pb.savev(&path, "png", &[]).unwrap();
            paths.push(path);
        }

        let window = gtk4::Window::new();
        window.set_default_size(1000, 600);
        window.set_resizable(false);
        let pane = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        pane.set_hexpand(true);
        pane.set_valign(gtk4::Align::Center);
        pane.set_margin_start(20);
        pane.set_margin_end(20);
        let preview = TransitionPreview::new();
        let frame = gtk4::AspectFrame::new(0.5, 0.5, 16.0 / 9.0, false);
        frame.set_child(Some(&preview.root));
        frame.set_hexpand(true);
        frame.set_vexpand(false);
        pane.append(&frame);
        window.set_child(Some(&pane));
        window.present();
        preview.set_images(Some(paths[0].clone()), Some(paths[1].clone()));

        let ctx = glib::MainContext::default();
        let pump = |ticks: u32, sizes: &mut Vec<(i32, i32, i32, i32)>| {
            for _ in 0..ticks {
                let until = std::time::Instant::now() + Duration::from_millis(33);
                while std::time::Instant::now() < until {
                    ctx.iteration(false);
                    std::thread::sleep(Duration::from_millis(2));
                }
                sizes.push((
                    window.width(),
                    window.height(),
                    preview.root.width(),
                    preview.root.height(),
                ));
            }
        };

        let mut settle = Vec::new();
        pump(15, &mut settle);
        let base = *settle.last().unwrap();
        assert!(base.2 > 0 && base.3 > 0, "stage never got an allocation");
        println!(
            "baseline window {}x{} stage {}x{}",
            base.0, base.1, base.2, base.3
        );

        for t in [
            Transition::KenBurns,
            Transition::Zoom,
            Transition::Slide,
            Transition::Blur,
            Transition::Fade,
        ] {
            preview.set_transition(t);
            let mut sizes = Vec::new();
            pump(60, &mut sizes);
            let max = sizes.iter().fold(base, |m, s| {
                (m.0.max(s.0), m.1.max(s.1), m.2.max(s.2), m.3.max(s.3))
            });
            println!(
                "{t:?}: max window {}x{} stage {}x{}",
                max.0, max.1, max.2, max.3
            );
            for (i, s) in [(0, max.0), (1, max.1), (2, max.2), (3, max.3)] {
                let b = [base.0, base.1, base.2, base.3][i];
                assert!(s - b <= 1, "{t:?}: size component {i} grew from {b} to {s}");
            }
        }
        // Back on Fade after the big ones the size is unchanged, not stuck.
        let last = *{
            let mut v = Vec::new();
            pump(3, &mut v);
            v
        }
        .last()
        .unwrap();
        assert!(
            (last.2 - base.2).abs() <= 1 && (last.3 - base.3).abs() <= 1,
            "stage stayed enlarged: {last:?} vs {base:?}"
        );
        preview.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

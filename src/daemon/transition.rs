//! The wallpaper transition state machine.
//!
//! This used to live inside `Slideshow`, which is the only reason transitions
//! were an images-only feature: `Phase`, the eased stepping and the base
//! zoom/pan were all fields of a struct that exists solely for slideshows.
//! Nothing in the animation itself is image-specific — every effect here is a
//! property change on the one running player (`gamma`, `video-zoom`,
//! `video-pan-*`, `vf`), and a video answers all four exactly like a still.
//!
//! So the machine lives here instead, in [`Anim`], owned per output. A
//! slideshow advancing to its next image is now one caller among others; a
//! scheduled swap and a user changing the wallpaper are the rest.
//!
//! # Leaving nothing behind
//!
//! Every effect is a *state* the player is left in, so the one failure mode
//! that matters is an animation that stops half-way: a permanently dimmed
//! desktop, a wallpaper stuck off-centre, or — worst, because it also costs
//! GPU forever — a blur that never lifts. [`Anim::settle`] is the single
//! function that returns the player to rest, and it is reachable from every
//! exit: the end of a transition ([`Anim::finish`]), the start of the next one
//! ([`Anim::start`] settles before it arms anything), and a player that is
//! about to be destroyed ([`Anim::forget`], which needs no IPC because the
//! process dies with the filter). There is no fourth way out.

use std::path::{Path, PathBuf};

use crate::config::{Transition, Wallpaper};

/// Transition durations in ~16ms steps (≈ FADE 0.37s, CROSSFADE 0.2s, SLIDE 0.45s/side).
const FADE_STEPS: u32 = 22;
const CROSSFADE_STEPS: u32 = 12;
const SLIDE_STEPS: u32 = 28;
/// Ken Burns zoom travel (mpv `video-zoom` log2 units) over one interval.
const KEN_BURNS_ZOOM: f64 = 0.16;
/// Subtle scale "punch" layered onto slide/fade for cinematic depth (~4%).
const SLIDE_PUNCH: f64 = 0.06;
/// `Zoom` is the punch *itself* rather than a garnish on another effect, so it
/// travels further (~16%) and takes ~0.29s a side.
const ZOOM_PUNCH: f64 = 0.22;
const ZOOM_STEPS: u32 = 18;
/// `Blur` defocus depth, in `gblur` sigma (logical pixels), and ~0.26s a side.
/// Short on purpose: this is the one effect that costs GPU while it runs.
const BLUR_SIGMA: f64 = 18.0;
const BLUR_STEPS: u32 = 16;

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

/// The player properties a transition drives, and nothing else.
///
/// A trait rather than a direct `PlayerHandle` call so the machine can be
/// stepped — and its cleanup asserted — without an mpv instance, which a unit
/// test has no way to build. The daemon's only implementor is `PlayerHandle`,
/// so both backends get the identical engine.
pub(super) trait Surface {
    /// `loadfile replace` — swap the media without touching anything else.
    fn load(&self, path: &Path);
    /// VO gamma, -100..=100; -100 is true black.
    fn gamma(&self, gamma: i32);
    /// `video-zoom` (log2 units) plus `video-pan-x`/`-y`.
    fn zoom_pan(&self, zoom: f64, pan_x: f64, pan_y: f64);
    /// Gaussian defocus by `sigma` logical pixels; `0.0` clears the filter.
    fn blur(&self, sigma: f64);
}

/// Where a transition currently is. `*Out` animates the outgoing wallpaper,
/// the media swap happens between the halves, and `*In` settles the incoming
/// one — so every effect is symmetric around one `loadfile`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    /// Nothing is animating; the player is at rest.
    Hold,
    FadeOut {
        step: u32,
        total: u32,
    },
    FadeIn {
        step: u32,
        total: u32,
    },
    SlideOut {
        step: u32,
    },
    SlideIn {
        step: u32,
    },
    ZoomOut {
        step: u32,
    },
    ZoomIn {
        step: u32,
    },
    BlurOut {
        step: u32,
    },
    BlurIn {
        step: u32,
    },
}

/// What one tick of the machine did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// Nothing is running (or the change needed no animation at all).
    Idle,
    /// Mid-animation — the caller should keep ticking at `ANIM_TICK`.
    Running,
    /// This tick completed the transition and put the player back at rest.
    Finished,
}

impl Step {
    /// True while the loop must run at animation cadence. The finishing tick
    /// counts, matching the pre-lift behaviour exactly.
    pub(super) fn animating(self) -> bool {
        !matches!(self, Step::Idle)
    }
}

/// A wallpaper change being animated on one player.
///
/// One per output — never shared, because two outputs run independent
/// renderers and may be mid-transition at different phases.
pub(super) struct Anim {
    phase: Phase,
    /// Base zoom/pan from the configured crop; animations compose on top, and
    /// it is what [`Anim::settle`] returns the player to.
    base_zoom: f64,
    base_pan_x: f64,
    base_pan_y: f64,
    /// Loaded at the animation's midpoint, then taken — so a swap can happen
    /// at most once per transition even if a phase is re-entered.
    next: Option<PathBuf>,
    /// Whether a blur filter is currently on the player. Tracked so `settle`
    /// clears `vf` only when this machine actually set it: writing `vf` forces
    /// a filter-chain reinit, and doing that on every slideshow advance would
    /// buy a frame hitch for nothing.
    blurred: bool,
}

/// The zoom/pan a wallpaper rests at — its configured crop, or dead centre.
pub(super) fn crop_base(wallpaper: &Wallpaper) -> (f64, f64, f64) {
    wallpaper
        .crop
        .and_then(|c| c.sanitized())
        .map(|c| c.to_mpv_zoom_pan())
        .unwrap_or((0.0, 0.0, 0.0))
}

impl Anim {
    pub(super) fn new(base: (f64, f64, f64)) -> Anim {
        Anim {
            phase: Phase::Hold,
            base_zoom: base.0,
            base_pan_x: base.1,
            base_pan_y: base.2,
            next: None,
            blurred: false,
        }
    }

    /// The wallpaper's crop changed, so the resting position did too.
    pub(super) fn set_base(&mut self, base: (f64, f64, f64)) {
        self.base_zoom = base.0;
        self.base_pan_x = base.1;
        self.base_pan_y = base.2;
    }

    pub(super) fn running(&self) -> bool {
        !matches!(self.phase, Phase::Hold)
    }

    #[cfg(test)]
    pub(super) fn phase(&self) -> Phase {
        self.phase
    }

    /// Begin a transition to `next` on `surface`.
    ///
    /// This is also the interruption path: a wallpaper changed again while the
    /// last change was still animating arrives here, and the first thing it
    /// does is put the player back at rest, so no effect can survive into the
    /// new transition — or outlive it, if the new one is
    /// [`Transition::None`].
    ///
    /// Returns [`Step::Idle`] when the change needed no animation (the media
    /// is already swapped by then), otherwise [`Step::Running`].
    pub(super) fn start<S: Surface>(
        &mut self,
        transition: Transition,
        next: PathBuf,
        surface: &S,
    ) -> Step {
        self.settle(surface);
        self.next = None;
        // Cleared before the match, not by it: the arms that need no animation
        // return early, and leaving a stale phase behind there would have the
        // interrupted transition carry on over the new wallpaper.
        self.phase = Phase::Hold;
        let phase = match transition {
            // Nothing to animate: swap now. Ken Burns has no swap
            // choreography of its own — its drift is a property of the *dwell*
            // and belongs to the caller — so a change under it is a plain load
            // onto a player that `settle` has just re-centred.
            Transition::None | Transition::KenBurns => {
                surface.load(&next);
                return Step::Idle;
            }
            Transition::Fade => Phase::FadeOut {
                step: 0,
                total: FADE_STEPS,
            },
            Transition::Crossfade => Phase::FadeOut {
                step: 0,
                total: CROSSFADE_STEPS,
            },
            Transition::Slide => Phase::SlideOut { step: 0 },
            Transition::Zoom => Phase::ZoomOut { step: 0 },
            Transition::Blur => Phase::BlurOut { step: 0 },
        };
        self.phase = phase;
        self.next = Some(next);
        Step::Running
    }

    /// One ~16ms tick. Each arm sets exactly the properties its effect needs —
    /// the loop runs at 60fps while this returns [`Step::Running`], so an
    /// extra property per tick is an extra 60 IPC round trips a second.
    pub(super) fn step<S: Surface>(&mut self, surface: &S) -> Step {
        match self.phase {
            Phase::Hold => Step::Idle,
            Phase::FadeOut { step, total } => {
                let e = ease_in_out_cubic(step as f64 / total as f64);
                surface.gamma((-100.0 * e) as i32);
                // Subtle inward "breath" while dimming — cinematic depth.
                surface.zoom_pan(
                    self.base_zoom + SLIDE_PUNCH * e,
                    self.base_pan_x,
                    self.base_pan_y,
                );
                if step >= total {
                    self.swap(surface);
                    self.phase = Phase::FadeIn { step: 0, total };
                } else {
                    self.phase = Phase::FadeOut {
                        step: step + 1,
                        total,
                    };
                }
                Step::Running
            }
            Phase::FadeIn { step, total } => {
                let e = ease_in_out_cubic(step as f64 / total as f64);
                surface.gamma((-100.0 * (1.0 - e)) as i32);
                // Settle the breath back to base as it brightens.
                surface.zoom_pan(
                    self.base_zoom + SLIDE_PUNCH * (1.0 - e),
                    self.base_pan_x,
                    self.base_pan_y,
                );
                if step >= total {
                    self.finish(surface)
                } else {
                    self.phase = Phase::FadeIn {
                        step: step + 1,
                        total,
                    };
                    Step::Running
                }
            }
            Phase::SlideOut { step } => {
                // Eased push out with a slight zoom — a "push", not a flat slide.
                let e = ease_in_out_cubic(step as f64 / SLIDE_STEPS as f64);
                surface.zoom_pan(
                    self.base_zoom + SLIDE_PUNCH * e,
                    self.base_pan_x - e,
                    self.base_pan_y,
                );
                if step >= SLIDE_STEPS {
                    self.swap(surface);
                    surface.zoom_pan(
                        self.base_zoom + SLIDE_PUNCH,
                        self.base_pan_x + 1.0,
                        self.base_pan_y,
                    );
                    self.phase = Phase::SlideIn { step: 0 };
                } else {
                    self.phase = Phase::SlideOut { step: step + 1 };
                }
                Step::Running
            }
            Phase::SlideIn { step } => {
                let e = ease_in_out_cubic(step as f64 / SLIDE_STEPS as f64);
                surface.zoom_pan(
                    self.base_zoom + SLIDE_PUNCH * (1.0 - e),
                    self.base_pan_x + (1.0 - e),
                    self.base_pan_y,
                );
                if step >= SLIDE_STEPS {
                    self.finish(surface)
                } else {
                    self.phase = Phase::SlideIn { step: step + 1 };
                    Step::Running
                }
            }
            Phase::ZoomOut { step } => {
                let e = ease_in_out_cubic(step as f64 / ZOOM_STEPS as f64);
                surface.zoom_pan(
                    self.base_zoom + ZOOM_PUNCH * e,
                    self.base_pan_x,
                    self.base_pan_y,
                );
                if step >= ZOOM_STEPS {
                    self.swap(surface);
                    // The incoming frame picks the move up exactly where the
                    // outgoing one dropped it, so the cut lands inside a
                    // continuous motion instead of interrupting one.
                    surface.zoom_pan(
                        self.base_zoom + ZOOM_PUNCH,
                        self.base_pan_x,
                        self.base_pan_y,
                    );
                    self.phase = Phase::ZoomIn { step: 0 };
                } else {
                    self.phase = Phase::ZoomOut { step: step + 1 };
                }
                Step::Running
            }
            Phase::ZoomIn { step } => {
                let e = ease_in_out_cubic(step as f64 / ZOOM_STEPS as f64);
                surface.zoom_pan(
                    self.base_zoom + ZOOM_PUNCH * (1.0 - e),
                    self.base_pan_x,
                    self.base_pan_y,
                );
                if step >= ZOOM_STEPS {
                    self.finish(surface)
                } else {
                    self.phase = Phase::ZoomIn { step: step + 1 };
                    Step::Running
                }
            }
            Phase::BlurOut { step } => {
                let e = ease_in_out_cubic(step as f64 / BLUR_STEPS as f64);
                self.set_blur(surface, BLUR_SIGMA * e);
                if step >= BLUR_STEPS {
                    // `vf` is a player property, not a per-file one, so the
                    // incoming media arrives already defocused and the swap
                    // itself is never seen.
                    self.swap(surface);
                    self.phase = Phase::BlurIn { step: 0 };
                } else {
                    self.phase = Phase::BlurOut { step: step + 1 };
                }
                Step::Running
            }
            Phase::BlurIn { step } => {
                let e = ease_in_out_cubic(step as f64 / BLUR_STEPS as f64);
                self.set_blur(surface, BLUR_SIGMA * (1.0 - e));
                if step >= BLUR_STEPS {
                    self.finish(surface)
                } else {
                    self.phase = Phase::BlurIn { step: step + 1 };
                    Step::Running
                }
            }
        }
    }

    /// Continuous Ken Burns drift across a slideshow's dwell. `frac` is how far
    /// through the interval we are; `forward` alternates the diagonal each
    /// image so it never feels mechanical.
    pub(super) fn ken_burns<S: Surface>(&self, surface: &S, frac: f64, forward: bool) {
        let e = smoothstep(frac.clamp(0.0, 1.0));
        let dir = if forward { 1.0 } else { -1.0 };
        surface.zoom_pan(
            self.base_zoom + KEN_BURNS_ZOOM * e,
            self.base_pan_x + dir * 0.10 * (e - 0.5),
            self.base_pan_y + dir * 0.05 * (e - 0.5),
        );
    }

    /// Drop transition state without touching the player — for a player that
    /// is going away (respawned, killed, its output parked). The filter and
    /// the VO parameters die with the process, so talking to it would be both
    /// pointless and, mid-teardown, a blocking write to a socket nobody is
    /// reading.
    pub(super) fn forget(&mut self) {
        self.phase = Phase::Hold;
        self.next = None;
        self.blurred = false;
    }

    /// Put the player back at rest: undimmed, at its crop's zoom/pan, and
    /// unblurred. The one way out — see the module docs.
    fn settle<S: Surface>(&mut self, surface: &S) {
        surface.gamma(0);
        surface.zoom_pan(self.base_zoom, self.base_pan_x, self.base_pan_y);
        if self.blurred {
            surface.blur(0.0);
            self.blurred = false;
        }
    }

    fn finish<S: Surface>(&mut self, surface: &S) -> Step {
        self.settle(surface);
        self.phase = Phase::Hold;
        self.next = None;
        Step::Finished
    }

    fn swap<S: Surface>(&mut self, surface: &S) {
        if let Some(path) = self.next.take() {
            surface.load(&path);
        }
    }

    fn set_blur<S: Surface>(&mut self, surface: &S, sigma: f64) {
        if sigma <= 0.0 {
            if self.blurred {
                surface.blur(0.0);
                self.blurred = false;
            }
        } else {
            surface.blur(sigma);
            self.blurred = true;
        }
    }
}

/// A `Surface` that records what a transition asked of it, so the machine can
/// be driven and inspected with no mpv anywhere.
#[cfg(test)]
#[derive(Default)]
pub(super) struct Recorder {
    inner: std::cell::RefCell<RecorderState>,
}

#[cfg(test)]
#[derive(Default)]
struct RecorderState {
    gamma: i32,
    zoom: (f64, f64, f64),
    sigma: f64,
    loads: Vec<PathBuf>,
    blur_calls: u32,
}

#[cfg(test)]
impl Recorder {
    pub(super) fn gamma(&self) -> i32 {
        self.inner.borrow().gamma
    }
    pub(super) fn zoom(&self) -> (f64, f64, f64) {
        self.inner.borrow().zoom
    }
    pub(super) fn sigma(&self) -> f64 {
        self.inner.borrow().sigma
    }
    pub(super) fn loads(&self) -> Vec<PathBuf> {
        self.inner.borrow().loads.clone()
    }
    pub(super) fn blur_calls(&self) -> u32 {
        self.inner.borrow().blur_calls
    }
    /// Every effect neutral: undimmed, at `base`, unblurred.
    pub(super) fn is_neutral(&self, base: (f64, f64, f64)) -> bool {
        let s = self.inner.borrow();
        s.gamma == 0 && s.zoom == base && s.sigma == 0.0
    }
}

#[cfg(test)]
impl Surface for Recorder {
    fn load(&self, path: &Path) {
        self.inner.borrow_mut().loads.push(path.to_path_buf());
    }
    fn gamma(&self, gamma: i32) {
        self.inner.borrow_mut().gamma = gamma;
    }
    fn zoom_pan(&self, zoom: f64, pan_x: f64, pan_y: f64) {
        self.inner.borrow_mut().zoom = (zoom, pan_x, pan_y);
    }
    fn blur(&self, sigma: f64) {
        let mut s = self.inner.borrow_mut();
        s.sigma = sigma;
        s.blur_calls += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Transition; 7] = [
        Transition::None,
        Transition::Crossfade,
        Transition::Fade,
        Transition::Slide,
        Transition::KenBurns,
        Transition::Zoom,
        Transition::Blur,
    ];
    /// The animated ones — the variants that arm a phase rather than swapping
    /// on the spot.
    const ANIMATED: [Transition; 5] = [
        Transition::Crossfade,
        Transition::Fade,
        Transition::Slide,
        Transition::Zoom,
        Transition::Blur,
    ];

    const BASE: (f64, f64, f64) = (0.0, 0.0, 0.0);
    /// A cropped wallpaper: rest is *not* the origin, so a machine that
    /// "resets" by writing zeros would be caught.
    const CROPPED: (f64, f64, f64) = (0.35, -0.2, 0.1);

    fn next() -> PathBuf {
        PathBuf::from("/tmp/next.mp4")
    }

    /// Drive one transition to completion. Returns the ticks it took.
    fn run(anim: &mut Anim, r: &Recorder, t: Transition) -> u32 {
        anim.start(t, next(), r);
        let mut ticks = 0;
        while anim.running() {
            assert!(ticks < 1000, "{t:?} never finished");
            anim.step(r);
            ticks += 1;
        }
        ticks
    }

    /// Whatever the effect, the wallpaper it leaves behind is a wallpaper the
    /// user can live with: undimmed, at its crop, in focus. This is the whole
    /// safety property — a half-applied effect *persists*, which is strictly
    /// worse than having no transition at all.
    #[test]
    fn every_transition_ends_with_the_player_back_at_rest() {
        for base in [BASE, CROPPED] {
            for t in ALL {
                let r = Recorder::default();
                let mut anim = Anim::new(base);
                run(&mut anim, &r, t);
                assert!(
                    r.is_neutral(base),
                    "{t:?} at base {base:?} left gamma {} zoom {:?} sigma {}",
                    r.gamma(),
                    r.zoom(),
                    r.sigma()
                );
                assert_eq!(r.loads(), vec![next()], "{t:?} must swap exactly once");
                assert!(!anim.running(), "{t:?} must end in Hold");
            }
        }
    }

    /// The media swap sits between the two halves — never at the start (which
    /// would show the new wallpaper being animated out) and never at the end.
    #[test]
    fn the_swap_lands_between_the_out_and_in_halves() {
        for t in ANIMATED {
            let r = Recorder::default();
            let mut anim = Anim::new(BASE);
            anim.start(t, next(), &r);
            let mut outs = 0;
            let mut swapped_after = None;
            let mut ticks = 0;
            while anim.running() {
                let outgoing = matches!(
                    anim.phase(),
                    Phase::FadeOut { .. }
                        | Phase::SlideOut { .. }
                        | Phase::ZoomOut { .. }
                        | Phase::BlurOut { .. }
                );
                anim.step(&r);
                ticks += 1;
                if outgoing {
                    outs += 1;
                }
                if swapped_after.is_none() && !r.loads().is_empty() {
                    swapped_after = Some(ticks);
                }
            }
            let swap = swapped_after.expect("{t:?} never loaded the new media");
            assert_eq!(swap, outs, "{t:?} must swap on the last outgoing tick");
            assert!(swap > 1 && swap < ticks, "{t:?} swapped at {swap}/{ticks}");
        }
    }

    /// The dip goes all the way to black and all the way back, monotonically —
    /// the look the fade has always had.
    #[test]
    fn the_fade_dips_to_true_black_and_returns() {
        let r = Recorder::default();
        let mut anim = Anim::new(BASE);
        anim.start(Transition::Fade, next(), &r);
        let mut darkest = 0;
        let mut prev = 0;
        let mut brightening = false;
        while anim.running() {
            anim.step(&r);
            let g = r.gamma();
            darkest = darkest.min(g);
            if g > prev {
                brightening = true;
            }
            assert!(!brightening || g >= prev, "the fade must not re-dim");
            prev = g;
        }
        assert_eq!(darkest, -100, "the dip must reach true black");
        assert_eq!(r.gamma(), 0, "and come all the way back");
        // Crossfade is the same shape, just fewer steps.
        let mut fast = Anim::new(BASE);
        let short = run(&mut fast, &Recorder::default(), Transition::Crossfade);
        let mut slow = Anim::new(BASE);
        let long = run(&mut slow, &Recorder::default(), Transition::Fade);
        assert!(
            short < long,
            "crossfade is the quicker dip: {short} vs {long}"
        );
    }

    /// `Zoom` is a single continuous move through the cut: in on the outgoing
    /// frame, and out again from exactly where it stopped.
    #[test]
    fn zoom_punches_in_and_settles_back_out() {
        let r = Recorder::default();
        let mut anim = Anim::new(CROPPED);
        anim.start(Transition::Zoom, next(), &r);
        let mut peak = f64::MIN;
        let mut at_swap = None;
        while anim.running() {
            let last_out = matches!(anim.phase(), Phase::ZoomOut { step } if step >= ZOOM_STEPS);
            anim.step(&r);
            peak = peak.max(r.zoom().0);
            if last_out {
                at_swap = Some(r.zoom().0);
            }
        }
        assert!(peak > CROPPED.0, "the punch must actually zoom in");
        assert_eq!(
            at_swap,
            Some(peak),
            "the incoming frame must start at the punch, not at rest"
        );
        assert_eq!(r.zoom(), CROPPED, "and settle back onto the crop");
        assert_eq!(r.blur_calls(), 0, "zoom is free — it must not touch vf");
    }

    /// `Blur` ramps the sigma with the same easing as everything else, and the
    /// filter is off again at the end. It is the only effect that costs
    /// anything while it runs, so it must also be the only one that sets `vf`.
    #[test]
    fn blur_defocuses_and_always_lifts() {
        let r = Recorder::default();
        let mut anim = Anim::new(CROPPED);
        anim.start(Transition::Blur, next(), &r);
        let mut peak: f64 = 0.0;
        while anim.running() {
            anim.step(&r);
            peak = peak.max(r.sigma());
        }
        assert!(peak > 1.0, "the defocus must be visible, got sigma {peak}");
        assert_eq!(r.sigma(), 0.0, "and the filter must be gone at the end");
        assert!(r.blur_calls() > 2, "the sigma must ramp, not jump");

        // Nobody else may write `vf`: a filter reinit on every slideshow
        // advance would buy a frame hitch for no effect at all.
        for t in ALL.into_iter().filter(|t| *t != Transition::Blur) {
            let r = Recorder::default();
            let mut anim = Anim::new(CROPPED);
            run(&mut anim, &r, t);
            assert_eq!(r.blur_calls(), 0, "{t:?} must never touch vf");
        }
    }

    /// A wallpaper changed again mid-transition — the interruption the daemon
    /// actually sees, because a second change arrives as another `start`. The
    /// interrupted effect must not survive into the new one, and if the new
    /// one is `None` it must not survive at all.
    #[test]
    fn an_interrupted_transition_leaves_nothing_behind() {
        for t in ANIMATED {
            // Interrupt at every single tick, not just a plausible one.
            for cut in 0..200 {
                let r = Recorder::default();
                let mut anim = Anim::new(CROPPED);
                anim.start(t, next(), &r);
                let mut ticks = 0;
                while anim.running() && ticks < cut {
                    anim.step(&r);
                    ticks += 1;
                }
                if !anim.running() {
                    break; // ran past the end; earlier cuts already covered it
                }
                // The user picks a different wallpaper, with no transition.
                anim.start(Transition::None, PathBuf::from("/tmp/other.mp4"), &r);
                assert!(
                    r.is_neutral(CROPPED),
                    "{t:?} interrupted at tick {cut} left gamma {} zoom {:?} sigma {}",
                    r.gamma(),
                    r.zoom(),
                    r.sigma()
                );
                assert!(!anim.running());
                assert_eq!(
                    r.loads().last(),
                    Some(&PathBuf::from("/tmp/other.mp4")),
                    "the newest wallpaper must be the one showing"
                );
            }
        }
    }

    /// The same interruption, but into another *animated* transition: the new
    /// effect owns the player from its first tick, and the old one's residue
    /// must be gone before that tick — most of all the blur, which the new
    /// transition may have no reason to ever write.
    #[test]
    fn one_transition_interrupting_another_still_clears_the_blur() {
        for first in ANIMATED {
            for second in ANIMATED {
                let r = Recorder::default();
                let mut anim = Anim::new(BASE);
                anim.start(first, next(), &r);
                for _ in 0..5 {
                    anim.step(&r);
                }
                anim.start(second, PathBuf::from("/tmp/other.mp4"), &r);
                if second != Transition::Blur {
                    assert_eq!(
                        r.sigma(),
                        0.0,
                        "{first:?} -> {second:?} carried a blur across"
                    );
                }
                while anim.running() {
                    anim.step(&r);
                }
                assert!(
                    r.is_neutral(BASE),
                    "{first:?} -> {second:?} ended dirty: gamma {} zoom {:?} sigma {}",
                    r.gamma(),
                    r.zoom(),
                    r.sigma()
                );
            }
        }
    }

    /// A renderer that dies mid-transition takes the filter with it, so
    /// `forget` must drop the state *without* IPC — and must not leave the
    /// machine believing a blur is still on, which would make the next
    /// transition clear a filter on a player that never had one.
    #[test]
    fn forgetting_a_dead_renderer_needs_no_ipc_and_leaves_no_debt() {
        let r = Recorder::default();
        let mut anim = Anim::new(CROPPED);
        anim.start(Transition::Blur, next(), &r);
        for _ in 0..6 {
            anim.step(&r);
        }
        assert!(r.sigma() > 0.0, "staging: the blur is on");
        let before = r.blur_calls();
        anim.forget();
        assert!(!anim.running(), "a dead renderer's transition is over");
        assert_eq!(r.blur_calls(), before, "forget must not talk to the player");

        // The replacement renderer is clean; the machine must agree.
        let fresh = Recorder::default();
        anim.start(Transition::Fade, next(), &fresh);
        assert_eq!(
            fresh.blur_calls(),
            0,
            "a fresh player must not be told to clear a filter it never had"
        );
        while anim.running() {
            anim.step(&fresh);
        }
        assert!(fresh.is_neutral(CROPPED));
    }

    /// A crop change moves where "rest" is, and the machine must settle onto
    /// the new one — not the crop the transition started under.
    #[test]
    fn the_resting_position_follows_the_crop() {
        let r = Recorder::default();
        let mut anim = Anim::new(BASE);
        anim.set_base(CROPPED);
        run(&mut anim, &r, Transition::Slide);
        assert_eq!(r.zoom(), CROPPED);
    }

    /// Ken Burns drifts across the dwell and hands back a centred frame.
    #[test]
    fn ken_burns_drifts_and_then_recentres() {
        let r = Recorder::default();
        let anim = Anim::new(BASE);
        anim.ken_burns(&r, 0.0, true);
        let start = r.zoom();
        anim.ken_burns(&r, 1.0, true);
        let end = r.zoom();
        assert!(end.0 > start.0, "the drift must zoom in over the interval");
        assert!(end.1 > start.1, "and pan");
        // The alternate image drifts the other way.
        anim.ken_burns(&r, 1.0, false);
        assert!(r.zoom().1 < end.1, "the diagonal must alternate");

        let mut anim = Anim::new(BASE);
        run(&mut anim, &r, Transition::KenBurns);
        assert!(r.is_neutral(BASE), "the next image starts centred");
    }

    #[test]
    fn the_easing_is_symmetric_and_bounded() {
        assert_eq!(ease_in_out_cubic(0.0), 0.0);
        assert_eq!(ease_in_out_cubic(1.0), 1.0);
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 1e-12);
        for i in 0..=100 {
            let t = f64::from(i) / 100.0;
            let e = ease_in_out_cubic(t);
            assert!((0.0..=1.0).contains(&e), "{t} -> {e}");
            assert!(
                (e + ease_in_out_cubic(1.0 - t) - 1.0).abs() < 1e-12,
                "the ease must be symmetric at {t}"
            );
        }
    }
}

//! Audio-spectrum visualiser: the settings, the envelopes and the box the bars
//! are drawn in (WIDGETS_ROADMAP W3).
//!
//! Pure arithmetic over a slice of band magnitudes — no I/O, no globals, no
//! audio capture, and since the bitmap migration no pixels either — so every
//! decision here is unit-testable and the module stays decoupled from whatever
//! the FFT side happens to call its types. The contract is a deliberately
//! narrow `&[f32]` in `0.0..=1.0`: the capture/FFT module owns the samples,
//! `crate::widgetkit::cards::visualizer` owns the picture, and this module owns
//! everything in between.
//!
//! # What is here, and what moved
//!
//! The widget used to be an ASS drawing built in this file: vector paths inside
//! `{\p1}` blocks, a bounding-box pin so a rising bar could not slide the
//! overlay around, and a hard rule that no float was ever formatted, because
//! libass discards a whole drawing it cannot parse. All of that is gone with
//! the ASS substrate. What is left is the three things that are about a
//! *spectrum* rather than about a renderer:
//!
//! * [`VisualStyleCfg`] — the resolved look, the output of preset resolution
//!   rather than the preset itself, so the widget is a pure function of one
//!   struct.
//! * [`Motion`] — the bar and peak-cap envelopes. Frame-rate independent by
//!   construction, because this widget deliberately runs at two different
//!   rates.
//! * [`box_size`] and [`is_silent`] — how big the bar area is on a given
//!   screen, and whether there is anything in it worth drawing.
//!
//! # Units
//!
//! Everything in this module is in **logical** units — the same units
//! `crate::widgetkit` lays a card out in, where one unit is one pixel at 1080p
//! and the rasteriser scales to the real output. `height_px`, `margin_px` and
//! [`Motion::advance`]'s `height_lu` are all in them. There is exactly one
//! coordinate system in this file, so a margin, a bar height and a fall rate can
//! be compared against each other with no conversion for anyone to get wrong.
//!
//! # Values that must never reach the screen
//!
//! `NaN`/`inf` are exactly what an FFT yields from a silent buffer or a broken
//! capture, and an envelope that swallowed one would stay poisoned for the
//! lifetime of the daemon — the widget does not restart when the music does. So
//! every band value entering [`Motion`] goes through `clamp01`, and every
//! quantity leaving it is finite and inside `0.0..=1.0` whatever went in.
//!
//! # Power
//!
//! [`is_silent`] exists so the daemon can stop pushing frames entirely when
//! nothing is playing. The roadmap's power model is not negotiable: no audio
//! must mean *no redraw*, not a redraw of an empty widget.
//!
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::lyrics::Anchor;

/// The five looks the visualiser can take.
///
/// These are five *silhouettes*, not five skins: a bar chart, a symmetric
/// equaliser, a continuous curve, a row of floating dots and a polar burst read
/// as different widgets from across a room, which is the only reason to offer a
/// choice at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VisualStyle {
    /// Classic spectrum bars rising from a floor.
    #[default]
    Bars,
    /// Bars mirrored about a centre line — the symmetric "equaliser" look.
    Mirror,
    /// One continuous filled silhouette instead of discrete bars.
    Wave,
    /// A row of dots that ride up and down and grow with their band.
    Dots,
    /// Bars radiating outward from a hub ring (polar layout).
    Ring,
}

impl VisualStyle {
    /// Every style, in menu order — for populating a GUI without hand-listing
    /// the variants a second time.
    pub const ALL: [VisualStyle; 5] = [
        VisualStyle::Bars,
        VisualStyle::Mirror,
        VisualStyle::Wave,
        VisualStyle::Dots,
        VisualStyle::Ring,
    ];
}

/// How the fill colour varies from one end of the spectrum to the other.
///
/// **ASS has no gradient primitive.** `\c` sets one flat fill for one drawing,
/// and no tag varies a colour across a shape; a smooth gradient inside a single
/// contour is simply not expressible. What ASS does have is cheap events — mpv
/// makes one out of every newline in an `osd-overlay` payload — and a spectrum
/// is already dozens of separate contours. So a gradient here is *per bar*: the
/// bands are split into runs, each run becomes its own drawing with its own
/// `\c`, and the colour steps along the ramp from one run to the next. At the
/// 32–64 bands a spectrum is normally drawn with, each step is one bar wide and
/// the row reads as a gradient. This is the standard technique for the format,
/// not a workaround for it.
///
/// What it is *not* is a gradient within one bar: that would mean slicing every
/// bar into horizontal bands, multiplying both the contour count and the event
/// count by the number of slices, for an effect that is invisible on a
/// four-unit-wide bar. It is not implemented, and the payload cost is the
/// reason.
///
/// [`VisualStyle::Wave`] cannot take part at all — it is one continuous
/// silhouette, and cutting it into per-band columns would both destroy that
/// (the whole point of the style) and show a seam at every cut, because libass
/// antialiases each contour's edges independently and two abutting fills leave
/// a hairline between them. Wave therefore stays flat whatever this is set to,
/// which is honest; a fake gradient would be neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Gradient {
    /// One flat colour for the whole widget: today's look, and one event.
    #[default]
    None,
    /// Step from the base colour to [`VisualStyleCfg::colour_end`] across the
    /// bars. The base is the accent when [`VisualStyleCfg::accent_follow`] is
    /// on and [`VisualStyleCfg::colour`] when it is not — that is what
    /// `accent_follow` has always meant, and a gradient does not change it.
    Linear,
    /// A fixed hue sweep from red round to violet, ignoring both colours.
    ///
    /// Worth having as its own mode rather than as a preset pair of hexes: it
    /// is the look most people picture when they hear "visualiser", and it is
    /// the one gradient that needs no colour picking to be worth turning on.
    Spectrum,
}

impl Gradient {
    /// Every mode, in menu order — so a GUI need not hand-list the variants a
    /// second time.
    pub const ALL: [Gradient; 3] = [Gradient::None, Gradient::Linear, Gradient::Spectrum];
}

/// A resolved visualiser look: everything the rasteriser needs, with nothing
/// left to look up.
///
/// The *output* of preset resolution rather than the preset itself, which is
/// what lets the whole widget be a pure function of one struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualStyleCfg {
    /// Which silhouette to draw.
    #[serde(default)]
    pub style: VisualStyle,
    /// Where on the screen the widget box sits.
    #[serde(default = "default_anchor")]
    pub anchor: Anchor,
    /// Box width as a percentage of the screen width. A percentage rather than
    /// pixels because this is the axis a user thinks about proportionally
    /// ("half the screen"), and because it stays right on an ultrawide.
    #[serde(default = "default_width_pct")]
    pub width_pct: f32,
    /// Box height in logical units, i.e. pixels at 1080p.
    #[serde(default = "default_height_px")]
    pub height_px: u32,
    /// Distance from the anchored edge(s), in logical units. Ignored on
    /// whichever axis the anchor is centred, exactly as in the lyric widget.
    #[serde(default = "default_margin_px")]
    pub margin_px: u32,
    /// Fill colour as `#RRGGBB`. Used when
    /// `accent_follow` is off, and as the fallback when it is on. With a
    /// gradient it is the ramp's **near** end.
    #[serde(default = "default_colour")]
    pub colour: String,
    /// Take the colour from the desktop accent instead of `colour`. Wins over
    /// `colour` when set, gradient or not: with [`Gradient::Linear`] the ramp
    /// then runs from the accent to `colour_end`.
    #[serde(default = "default_accent_follow")]
    pub accent_follow: bool,
    /// How the colour varies across the bars. [`Gradient::None`] — one flat
    /// fill — is the default and is exactly what this module rendered before
    /// gradients existed, down to the byte.
    #[serde(default)]
    pub gradient: Gradient,
    /// The ramp's **far** end as `#RRGGBB`, used by [`Gradient::Linear`] only.
    ///
    /// Defaults to the same white as `colour`, and a ramp between two equal
    /// colours is flat — so turning the mode on without picking a colour costs
    /// nothing and changes nothing, except in the default configuration where
    /// `accent_follow` makes the near end the accent and the ramp runs accent
    /// to white.
    #[serde(default = "default_colour_end")]
    pub colour_end: String,
    /// 0 (invisible) to 255 (solid). Inverted on the way out, because ASS
    /// alpha runs the other way: `&H00&` is opaque and `&HFF&` invisible.
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    /// Space between adjacent bars/dots, in drawing units. Capped against the
    /// cell width so it can never swallow the thing it separates.
    #[serde(default = "default_gap_px")]
    pub gap_px: u32,
    /// Round the ends of shapes: rounded bar caps, circular dots, curved wave
    /// segments, an arced outer rim on [`VisualStyle::Ring`]. Off gives the
    /// same layout with hard edges.
    #[serde(default = "default_rounded")]
    pub rounded: bool,
}

fn default_anchor() -> Anchor {
    Anchor::BottomCenter
}

fn default_width_pct() -> f32 {
    60.0
}

fn default_height_px() -> u32 {
    120
}

fn default_margin_px() -> u32 {
    48
}

fn default_colour() -> String {
    "#FFFFFF".to_string()
}

fn default_accent_follow() -> bool {
    true
}

fn default_colour_end() -> String {
    "#FFFFFF".to_string()
}

fn default_opacity() -> u8 {
    220
}

fn default_gap_px() -> u32 {
    4
}

fn default_rounded() -> bool {
    true
}

impl Default for VisualStyleCfg {
    fn default() -> Self {
        VisualStyleCfg {
            style: VisualStyle::default(),
            anchor: default_anchor(),
            width_pct: default_width_pct(),
            height_px: default_height_px(),
            margin_px: default_margin_px(),
            colour: default_colour(),
            accent_follow: default_accent_follow(),
            gradient: Gradient::default(),
            colour_end: default_colour_end(),
            opacity: default_opacity(),
            gap_px: default_gap_px(),
            rounded: default_rounded(),
        }
    }
}

/// Smallest box width, as a percentage of the screen. Below this the bars are
/// narrower than the gaps between them.
const MIN_WIDTH_PCT: f32 = 5.0;
/// A box cannot be wider than the screen.
const MAX_WIDTH_PCT: f32 = 100.0;
/// Smallest box height in drawing units.
const MIN_HEIGHT_PX: u32 = 8;

/// How many bands are actually drawn. An FFT can hand over thousands of bins,
/// and a bar narrower than a screen pixel is invisible work — done at 15–30Hz,
/// on a payload that crosses an IPC socket every frame. Excess bands are folded
/// rather than dropped; see [`sanitise`].
const MAX_BANDS: usize = 192;

// ---------------------------------------------------------------------------
// Motion (spec §10)
// ---------------------------------------------------------------------------

/// Time constant of the **rise**. Short, because a spectrum that lags the
/// transient it is drawing does not read as music.
const ATTACK: Duration = Duration::from_millis(45);

/// Time constant of the **fall**. Nearly five times the attack: bars that drop
/// as fast as they rise flicker, and the asymmetry is what makes an array of
/// rectangles look like it is responding to sound rather than to noise.
const RELEASE: Duration = Duration::from_millis(220);

/// How long a peak cap sits still before it starts to fall.
const PEAK_HOLD: Duration = Duration::from_millis(380);

/// How fast a peak cap falls once the hold is over, in **logical units per
/// frame** at the nominal frame rate.
///
/// Quoted per frame rather than per second because that is how the spec quotes
/// it and how it was judged — but [`Motion::advance`] is given the real elapsed
/// time and the nominal frame period, so a dropped frame moves the cap by the
/// distance it should have travelled rather than by one step.
const PEAK_FALL_LU: f32 = 0.9;

/// The visualiser's animation state, **owned by the caller**.
///
/// `widgetkit`'s bar renderer takes `values` and `peaks` and draws exactly what
/// it is given: it never smooths, never holds and never allocates, because it
/// is the one widget that redraws every frame while audio plays and per-frame
/// allocation there is tens of MB/s for nothing. All of the motion is here.
///
/// Three things move, on three different schedules:
///
/// | | rise | fall |
/// |---|---|---|
/// | bar | `ATTACK` | `RELEASE` |
/// | peak cap | instant | after `PEAK_HOLD`, at `PEAK_FALL_LU` |
///
/// Both envelopes are exponential and both are driven by the **real** elapsed
/// time rather than by a frame count, so the widget looks the same at 24 Hz as
/// it does at 4 Hz — which matters, because the daemon deliberately drops to
/// the silent cadence and back.
///
/// Every buffer is allocated once, in [`Motion::new`], and reused for the life
/// of the widget.
#[derive(Debug, Clone, Default)]
pub struct Motion {
    /// The smoothed bar heights, 0..1 — what the renderer draws.
    levels: Vec<f32>,
    /// Peak cap positions, 0..1.
    peaks: Vec<f32>,
    /// Seconds each cap has been sitting at its current height.
    held: Vec<f32>,
}

impl Motion {
    /// State for `bands` bars, all at rest.
    pub fn new(bands: usize) -> Self {
        let n = bands.clamp(1, MAX_BANDS);
        Motion {
            levels: vec![0.0; n],
            peaks: vec![0.0; n],
            held: vec![0.0; n],
        }
    }

    /// The smoothed bar heights.
    pub fn levels(&self) -> &[f32] {
        &self.levels
    }

    /// The peak cap positions.
    pub fn peaks(&self) -> &[f32] {
        &self.peaks
    }

    /// Drop everything to rest without reallocating.
    ///
    /// For the transition into silence: the last frame the daemon pushes has to
    /// be the resting one, and coming back from silence must not resume a
    /// half-fallen cap from a minute ago.
    pub fn reset(&mut self) {
        self.levels.iter_mut().for_each(|v| *v = 0.0);
        self.peaks.iter_mut().for_each(|v| *v = 0.0);
        self.held.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Advance one frame from `bands`.
    ///
    /// * `dt` — real time since the last advance. Clamped, so a resume from
    ///   suspend snaps to the new spectrum instead of integrating an hour of
    ///   decay.
    /// * `frame` — the nominal frame period, which is what `PEAK_FALL_LU` is
    ///   quoted against.
    /// * `height_lu` — the bar area's height in logical units, which is what
    ///   turns a fall quoted in logical units into one in the renderer's 0..1.
    ///
    /// Resizes to match `bands` when the band count changes, and never
    /// allocates when it has not.
    pub fn advance(&mut self, bands: &[f32], dt: Duration, frame: Duration, height_lu: f32) {
        let n = folded_len(bands.len());
        if self.levels.len() != n {
            self.levels.resize(n, 0.0);
            self.peaks.resize(n, 0.0);
            self.held.resize(n, 0.0);
        }
        if n == 0 {
            return;
        }
        // A tick longer than a second is a resume, not a frame.
        let dt = dt.as_secs_f32().clamp(0.0, 1.0);
        let attack = smoothing(dt, ATTACK);
        let release = smoothing(dt, RELEASE);
        // The fall, converted out of "logical units per nominal frame" into the
        // renderer's 0..1, and scaled by how much time actually passed.
        let frames = if frame.is_zero() {
            1.0
        } else {
            dt / frame.as_secs_f32()
        };
        let fall = if height_lu.is_finite() && height_lu > 0.0 {
            PEAK_FALL_LU / height_lu * frames
        } else {
            0.0
        };
        for i in 0..n {
            let target = fold_band(bands, i, n);
            let level = self.levels[i];
            // Asymmetric: the transient is the signal, the decay is the taste.
            let k = if target > level { attack } else { release };
            let level = level + (target - level) * k;
            self.levels[i] = clamp01(level);

            if level >= self.peaks[i] {
                self.peaks[i] = level;
                self.held[i] = 0.0;
            } else {
                self.held[i] += dt;
                if self.held[i] >= PEAK_HOLD.as_secs_f32() {
                    // Never below the bar: a cap under its own bar is not a
                    // peak, it is a stripe.
                    self.peaks[i] = (self.peaks[i] - fall).max(level).max(0.0);
                }
            }
        }
    }
}

/// One minus the exponential decay over `dt` for time constant `tau` — the
/// frame-rate-independent form of a one-pole filter.
///
/// A plain `level += (target - level) * 0.3` is the usual shortcut and it makes
/// the widget's motion a function of the frame rate, which is exactly wrong
/// here: this widget deliberately runs at two different rates.
fn smoothing(dt: f32, tau: Duration) -> f32 {
    let tau = tau.as_secs_f32();
    if tau <= 0.0 || !dt.is_finite() {
        return 1.0;
    }
    1.0 - (-dt / tau).exp()
}

/// How many bars `len` raw bands are drawn as. See [`fold_band`].
fn folded_len(len: usize) -> usize {
    len.min(MAX_BANDS)
}

/// The magnitude of drawn bar `i` of `n`, folded from the raw `bands`.
///
/// Over [`MAX_BANDS`] the spectrum is folded by taking the **maximum** of each
/// group rather than truncating or averaging: peaks are the signal in a
/// spectrum display, so averaging flattens the picture and truncating silently
/// deletes the treble half of it.
///
/// Written as an indexed read rather than as a `Vec`-returning `sanitise` so the
/// per-frame path allocates nothing at all.
fn fold_band(bands: &[f32], i: usize, n: usize) -> f32 {
    if bands.len() <= n {
        return bands.get(i).copied().map(clamp01).unwrap_or(0.0);
    }
    // u64 throughout: `i * bands.len()` overflows a 32-bit usize for a large
    // enough slice, and this is the only multiplication here that is not
    // bounded by the screen.
    let total = bands.len() as u64;
    let lo = (i as u64 * total / n as u64) as usize;
    let hi = (((i as u64 + 1) * total / n as u64) as usize).max(lo + 1);
    bands[lo..hi.min(bands.len())]
        .iter()
        .copied()
        .map(clamp01)
        .fold(0.0f32, f32::max)
}

/// Whether every band is at or below `threshold` — i.e. there is nothing worth
/// drawing.
///
/// This is the power-model hook, not a convenience. The roadmap forbids a
/// render loop: with no audio the daemon must push **nothing**, so the correct
/// use is to skip the whole render-and-send path while this is true, and to
/// send one `overlay-remove` on the transition into silence.
///
/// Inclusive at the boundary — a band sitting exactly on the noise floor is
/// silence, not signal — and `NaN` counts as silence, since a capture producing
/// them has nothing to say either. An empty slice is silent. A `NaN` threshold
/// is treated as zero rather than swallowing the whole spectrum.
pub fn is_silent(bands: &[f32], threshold: f32) -> bool {
    let threshold = if threshold.is_nan() { 0.0 } else { threshold };
    bands.iter().all(|&b| clamp01(b) <= threshold)
}

/// One band magnitude, forced into `0.0..=1.0`.
///
/// `NaN` becomes zero and not one: a broken capture should read as silence, so
/// [`is_silent`] can shut the widget down instead of pinning it at full scale.
/// The explicit test is required — `f32::clamp` propagates `NaN`.
fn clamp01(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// Widget box size in **logical units** (pixels at 1080p), with every
/// hand-editable number clamped.
///
/// `screen_w` is the output's width in the same units — `output_width / scale`,
/// not [`crate::widgetkit::REFERENCE_HEIGHT`]'s 1920. The two are the same on a
/// 16:9 screen and are not on an ultrawide, where a fixed 1920 would make
/// `width_pct = 60` mean 60% of a screen the user does not have.
pub fn box_size(cfg: &VisualStyleCfg, screen_w: f32) -> (f32, f32) {
    let pct = if cfg.width_pct.is_finite() {
        cfg.width_pct.clamp(MIN_WIDTH_PCT, MAX_WIDTH_PCT)
    } else {
        // A NaN width would otherwise poison every coordinate downstream.
        default_width_pct()
    };
    let screen_w = if screen_w.is_finite() && screen_w > 0.0 {
        screen_w
    } else {
        1920.0
    };
    let w = screen_w * pct / 100.0;
    let h = cfg.height_px.clamp(MIN_HEIGHT_PX, 2048) as f32;
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A band set with something happening in it, in the shape a real spectrum
    /// has: loud low end, quiet top, one peak in the middle.
    fn spectrum() -> Vec<f32> {
        vec![
            0.9, 0.75, 0.6, 0.5, 0.85, 1.0, 0.4, 0.3, 0.25, 0.2, 0.15, 0.1, 0.08, 0.05, 0.03, 0.0,
        ]
    }

    /// One nominal frame at [`crate::daemon::widgets::VISUAL_FPS`].
    const FRAME: Duration = Duration::from_millis(1000 / 24);

    /// Run `n` frames of `bands` through a fresh [`Motion`].
    fn settle(bands: &[f32], n: usize, height: f32) -> Motion {
        let mut m = Motion::new(bands.len());
        for _ in 0..n {
            m.advance(bands, FRAME, FRAME, height);
        }
        m
    }

    // -- silence -------------------------------------------------------------

    #[test]
    fn is_silent_is_inclusive_at_the_threshold() {
        // Exclusive here would leave the widget redrawing forever on a noise
        // floor that never quite reaches zero — the power bug this exists to
        // prevent.
        assert!(is_silent(&[0.02, 0.02, 0.02], 0.02), "at threshold");
        assert!(is_silent(&[0.019_9, 0.0, 0.01], 0.02), "below threshold");
        assert!(!is_silent(&[0.0, 0.020_1, 0.0], 0.02), "above threshold");
        // One loud band out of many is not silence.
        let mut bands = vec![0.0f32; 64];
        bands[40] = 0.9;
        assert!(!is_silent(&bands, 0.02));
        // Degenerate inputs must not make the caller decide anything unsafe.
        assert!(is_silent(&[], 0.02), "nothing to draw is silence");
        assert!(is_silent(&[0.0], 0.0), "zero threshold, zero signal");
        assert!(!is_silent(&[f32::EPSILON], 0.0));
        assert!(
            is_silent(&[f32::NAN; 4], 0.0),
            "a broken capture is silence"
        );
        assert!(is_silent(&[-1.0, -5.0], 0.0), "negatives clamp to silence");
        assert!(
            !is_silent(&[f32::INFINITY], 0.5),
            "inf clamps to full scale"
        );
        // A NaN threshold must not swallow a live spectrum.
        assert!(!is_silent(&[0.5], f32::NAN));
        assert!(is_silent(&[0.0], f32::NAN));
    }

    #[test]
    fn clamp01_maps_broken_input_to_silence_not_to_full_scale() {
        assert_eq!(clamp01(0.5), 0.5);
        assert_eq!(clamp01(-0.5), 0.0);
        assert_eq!(clamp01(1.5), 1.0);
        assert_eq!(clamp01(f32::NAN), 0.0);
        assert_eq!(clamp01(f32::INFINITY), 1.0);
        assert_eq!(clamp01(f32::NEG_INFINITY), 0.0);
    }

    // -- motion (spec §10) ---------------------------------------------------

    #[test]
    fn the_rise_is_fast_and_the_fall_is_slow() {
        // The whole of why the envelope is asymmetric: bars that drop as fast
        // as they rise flicker, and a spectrum that lags its own transient does
        // not read as music. 45ms up against 220ms down.
        let mut m = Motion::new(1);
        m.advance(&[1.0], FRAME, FRAME, 100.0);
        let after_one_up = m.levels()[0];
        // One 42ms frame is very nearly one attack constant, so most of the way.
        assert!(after_one_up > 0.5, "rise was {after_one_up}");

        let mut m = settle(&[1.0], 40, 100.0);
        assert!(m.levels()[0] > 0.99, "did not reach the top");
        m.advance(&[0.0], FRAME, FRAME, 100.0);
        let dropped = 1.0 - m.levels()[0];
        assert!(dropped < 0.25, "fall was {dropped}, too fast");
        assert!(
            dropped < after_one_up,
            "the fall must be slower than the rise"
        );
    }

    #[test]
    fn the_envelopes_are_the_same_shape_at_any_frame_rate() {
        // The widget runs at 24 Hz with audio and 4 Hz when it is nearly quiet,
        // and it must not look different at the two. A plain
        // `level += (target - level) * k` would.
        let fast = {
            let mut m = Motion::new(1);
            for _ in 0..24 {
                m.advance(&[1.0], Duration::from_millis(10), FRAME, 100.0);
            }
            m.levels()[0]
        };
        let slow = {
            let mut m = Motion::new(1);
            for _ in 0..4 {
                m.advance(&[1.0], Duration::from_millis(60), FRAME, 100.0);
            }
            m.levels()[0]
        };
        // Same 240ms of real time either way.
        assert!((fast - slow).abs() < 0.02, "{fast} vs {slow}");
    }

    #[test]
    fn a_peak_cap_snaps_up_holds_and_then_falls() {
        // 380ms of hold, then 0.9 logical units a frame. The hold is what makes
        // the cap readable at all; without it the eye never catches it.
        let height = 90.0_f32;
        let mut m = settle(&[1.0], 40, height);
        assert!(m.peaks()[0] > 0.99, "the cap did not follow the bar up");
        // Drop the input. The bar falls; the cap must not, yet.
        for _ in 0..8 {
            m.advance(&[0.0], FRAME, FRAME, height);
        }
        // 8 frames is 333ms, inside the hold.
        assert!(m.peaks()[0] > 0.99, "the cap moved during the hold");
        assert!(m.levels()[0] < 0.5, "the bar did not fall");
        // Past the hold it comes down, and at the documented rate.
        let before = m.peaks()[0];
        for _ in 0..10 {
            m.advance(&[0.0], FRAME, FRAME, height);
        }
        let fell = before - m.peaks()[0];
        let want = 10.0 * PEAK_FALL_LU / height;
        assert!(
            (fell - want).abs() < want * 0.3,
            "fell {fell}, wanted about {want}"
        );
        // And it never sinks below its own bar, which would draw a stripe
        // across it rather than a cap above it.
        for _ in 0..400 {
            m.advance(&[0.3], FRAME, FRAME, height);
        }
        assert!(m.peaks()[0] >= m.levels()[0] - 1e-6);
    }

    #[test]
    fn reset_drops_everything_without_reallocating() {
        // The transition into silence. Coming back a minute later must not
        // resume a half-fallen cap from before it.
        let mut m = settle(&spectrum(), 40, 90.0);
        assert!(m.levels().iter().any(|&v| v > 0.5));
        let n = m.levels().len();
        m.reset();
        assert_eq!(m.levels().len(), n);
        assert!(m.levels().iter().all(|&v| v == 0.0), "{:?}", m.levels());
        assert!(m.peaks().iter().all(|&v| v == 0.0), "{:?}", m.peaks());
    }

    #[test]
    fn hostile_band_values_stay_inside_the_renderer_s_range() {
        // A broken capture must cost the user their visualiser at worst, never
        // a bar drawn a mile off the top of the screen — `widgetkit` clamps
        // too, but a NaN that reached the envelope would poison it for good.
        let junk = [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -5.0,
            9e30,
            0.5,
            f32::MIN_POSITIVE,
        ];
        let mut m = Motion::new(junk.len());
        for _ in 0..50 {
            m.advance(&junk, FRAME, FRAME, 90.0);
        }
        for (i, &v) in m.levels().iter().enumerate() {
            assert!(v.is_finite() && (0.0..=1.0).contains(&v), "band {i} = {v}");
        }
        for (i, &v) in m.peaks().iter().enumerate() {
            assert!(v.is_finite() && (0.0..=1.0).contains(&v), "peak {i} = {v}");
        }
        // A NaN band is silence, not full scale: `is_silent` has to be able to
        // shut the widget down rather than pin it at the ceiling.
        assert_eq!(m.levels()[0], 0.0);
        // Degenerate geometry must not divide by anything.
        let mut m = Motion::new(4);
        for h in [0.0_f32, -1.0, f32::NAN, f32::INFINITY] {
            m.advance(&[1.0, 0.0, 1.0, 0.0], FRAME, FRAME, h);
        }
        assert!(m.levels().iter().all(|v| v.is_finite()));
        // A zero nominal frame is a caller bug, not a panic.
        m.advance(&[0.0; 4], FRAME, Duration::ZERO, 90.0);
        assert!(m.peaks().iter().all(|v| v.is_finite()));
        // And an empty spectrum is a no-op, not an index panic.
        Motion::new(0).advance(&[], FRAME, FRAME, 90.0);
    }

    #[test]
    fn a_resume_from_suspend_snaps_rather_than_integrating_an_hour() {
        // `dt` comes from a monotonic clock the daemon does not control the
        // pace of: a suspend, a stopped process, a busy box. The clamp in
        // `advance` is what stops the first frame back integrating hours' worth
        // of decay and peak fall in one step.
        //
        // The clamp is asserted as an *equivalence*, which is the only thing
        // that actually distinguishes it from its absence: an hour and a second
        // must produce identical state, because an hour is a second as far as
        // this filter is concerned. Asserting "the level is nearly zero"
        // instead would pass either way — an unclamped hour drives it to
        // exactly zero.
        let mut resumed = settle(&[1.0], 40, 90.0);
        let mut one_second = settle(&[1.0], 40, 90.0);
        resumed.advance(&[0.0], Duration::from_secs(3600), FRAME, 90.0);
        one_second.advance(&[0.0], Duration::from_secs(1), FRAME, 90.0);
        assert_eq!(resumed.levels(), one_second.levels());
        assert_eq!(resumed.peaks(), one_second.peaks());

        // And the state it lands in is a sane one: a bar well down from where
        // it was, never negative, and a peak cap still inside the renderer's
        // range rather than wrapped through it.
        let settled = settle(&[1.0], 40, 90.0);
        assert!(
            resumed.levels()[0] < settled.levels()[0] / 10.0,
            "{} did not fall from {}",
            resumed.levels()[0],
            settled.levels()[0]
        );
        assert!(resumed.levels()[0] >= 0.0, "{}", resumed.levels()[0]);
        assert!((0.0..=1.0).contains(&resumed.peaks()[0]));
    }

    #[test]
    fn too_many_bands_are_folded_by_peak_not_truncated() {
        // Averaging flattens a spectrum and truncating silently deletes its
        // treble half. Neither is acceptable in a *peak* display.
        let mut bands = vec![0.0f32; MAX_BANDS * 3];
        // One spike near the top of the range, in the third of each group.
        bands[MAX_BANDS * 3 - 1] = 1.0;
        let m = settle(&bands, 60, 90.0);
        assert_eq!(m.levels().len(), MAX_BANDS);
        assert!(
            m.levels()[MAX_BANDS - 1] > 0.9,
            "the spike was folded away: {}",
            m.levels()[MAX_BANDS - 1]
        );
        // Below the cap nothing is folded and the bands map one to one.
        let m = settle(&[0.25, 0.5, 1.0], 60, 90.0);
        assert_eq!(m.levels().len(), 3);
        assert!((m.levels()[0] - 0.25).abs() < 0.01, "{:?}", m.levels());
        // A band count that changes under us resizes rather than mismatching
        // the array the renderer is handed.
        let mut m = Motion::new(3);
        m.advance(&[0.5; 64], FRAME, FRAME, 90.0);
        assert_eq!(m.levels().len(), 64);
        assert_eq!(m.peaks().len(), 64);
    }

    // -- geometry ------------------------------------------------------------

    #[test]
    fn hostile_config_cannot_produce_a_nonsense_box() {
        // `config.toml` is hand-editable and every one of these has shipped in
        // a bug report against some project or other.
        let hostile = [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -100.0,
            0.0,
            1e30,
            100.0,
        ];
        for pct in hostile {
            for screen in [1920.0_f32, 3440.0, 0.0, f32::NAN, -1.0] {
                let cfg = VisualStyleCfg {
                    width_pct: pct,
                    height_px: u32::MAX,
                    ..VisualStyleCfg::default()
                };
                let (w, h) = box_size(&cfg, screen);
                assert!(w.is_finite() && w > 0.0, "pct {pct} screen {screen}: {w}");
                assert!(h.is_finite() && h > 0.0, "pct {pct} screen {screen}: {h}");
            }
        }
        let tiny = VisualStyleCfg {
            height_px: 0,
            ..VisualStyleCfg::default()
        };
        assert_eq!(box_size(&tiny, 1920.0).1, MIN_HEIGHT_PX as f32);
    }

    #[test]
    fn the_box_is_a_share_of_the_screen_and_not_of_a_fixed_1920() {
        // The bug this replaced: sizing against the ASS coordinate space made
        // `width_pct = 60` mean 60% of a 1920-wide screen the ultrawide user
        // does not have.
        let c = VisualStyleCfg::default();
        assert_eq!(box_size(&c, 1920.0), (1152.0, 120.0));
        assert_eq!(box_size(&c, 3440.0), (2064.0, 120.0));
        assert_eq!(box_size(&c, 1280.0), (768.0, 120.0));
    }

    // -- config --------------------------------------------------------------

    #[test]
    fn defaults_are_the_documented_ones() {
        // The config stores these numbers too; if the two drift, the GUI shows
        // one thing and the overlay renders another.
        let c = VisualStyleCfg::default();
        assert_eq!(c.style, VisualStyle::Bars);
        assert_eq!(c.anchor, Anchor::BottomCenter);
        assert_eq!(c.width_pct, 60.0);
        assert_eq!(c.height_px, 120);
        assert_eq!(c.margin_px, 48);
        assert_eq!(c.colour, "#FFFFFF");
        assert!(c.accent_follow);
        assert_eq!(c.opacity, 220);
        assert_eq!(c.gap_px, 4);
        assert!(c.rounded);
        assert_eq!(VisualStyle::ALL.len(), 5);
    }

    #[test]
    fn style_and_gradient_spellings_are_stable_in_both_directions() {
        // These land in `config.toml`. A rename here silently resets every
        // existing user's choice to the default.
        for (style, name) in [
            (VisualStyle::Bars, "bars"),
            (VisualStyle::Mirror, "mirror"),
            (VisualStyle::Wave, "wave"),
            (VisualStyle::Dots, "dots"),
            (VisualStyle::Ring, "ring"),
        ] {
            let json = format!("\"{name}\"");
            assert_eq!(
                serde_json::from_str::<VisualStyle>(&json).expect(name),
                style
            );
            assert_eq!(serde_json::to_string(&style).expect(name), json);
        }
        for (grad, name) in [
            (Gradient::None, "none"),
            (Gradient::Linear, "linear"),
            (Gradient::Spectrum, "spectrum"),
        ] {
            let json = format!("\"{name}\"");
            assert_eq!(serde_json::from_str::<Gradient>(&json).expect(name), grad);
            assert_eq!(serde_json::to_string(&grad).expect(name), json);
        }
        assert_eq!(VisualStyle::default(), VisualStyle::Bars);
        assert_eq!(Gradient::default(), Gradient::None);
    }
}

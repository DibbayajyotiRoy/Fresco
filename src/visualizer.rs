//! Audio-spectrum visualiser: band magnitudes to one ASS drawing event
//! (WIDGETS_ROADMAP W3).
//!
//! Pure geometry over a slice of band magnitudes — no I/O, no globals, no audio
//! capture — so every style is unit-testable and the module stays decoupled
//! from whatever the FFT side happens to call its types. The contract is a
//! deliberately narrow `&[f32]` in `0.0..=1.0`: the capture/FFT module owns the
//! samples, this one owns the picture, and neither has to be rebuilt when the
//! other changes shape.
//!
//! # The mpv contract
//!
//! [`render_ass`] emits the `Text` field of an ASS dialogue event — the same
//! payload `crate::lyrics::render_ass` produces, and the same one mpv's
//! `osd-overlay` command consumes with `format: "ass-events"`. Callers must
//! pass `res_x: PLAY_RES_X` / `res_y: PLAY_RES_Y`, because everything here is
//! authored in that fixed [`PLAY_RES_X`]×[`PLAY_RES_Y`] space and libass maps
//! it onto the real output proportionally — so the widget keeps its proportions
//! on a 4K screen instead of shrinking into a corner.
//!
//! mpv splits that payload on newlines and makes each piece its own event with
//! mpv's own OSD style behind it, so a stray newline used to be a bug: the
//! second event would inherit a font, an outline and a colour this module never
//! chose. A flat spectrum is therefore still exactly one event. A gradient one
//! is deliberately several — see [`Gradient`] — and the rule that made the
//! newline dangerous is what makes the split safe: **every** event this module
//! emits sets every visual property explicitly and carries its own copy of the
//! bounding-box pin, so no event can inherit anything, and all of them land on
//! the same pixel.
//!
//! # Drawing units
//!
//! Shapes are vectors, not pixels: ASS has no bitmap, so bars and waves are
//! paths inside `{\p1}` … `{\p0}`. The number after `\p` is a **scale
//! exponent** — libass divides every coordinate by `2^(n-1)` — so `\p1` is a
//! divisor of one and **one drawing unit is one unit of the PLAY_RES space**,
//! the very unit `\pos`, `height_px` and `margin_px` are already expressed in.
//! That is the whole reason for choosing it: there is exactly one coordinate
//! system in this file, so a margin, a bar height and a path vertex can be
//! compared and clamped against each other with no conversion for anyone to get
//! wrong. A finer scale (`\p2`, `\p3`) would buy sub-unit curve precision that
//! at 1080p-and-up sits below one output pixel, and would cost a conversion at
//! every one of the few hundred vertices in every frame.
//!
//! # Placement, and the bounding-box pin
//!
//! libass lays a drawing out like a glyph: its bounding box is its extent, so
//! `\an` + `\pos` place *the box*, not the coordinate origin. A spectrum's
//! bounding box changes with the music, so the obvious implementation slides
//! around the screen as the bars rise and fall. Every path here therefore opens
//! with two zero-area contours at opposite corners of the widget box
//! (`Path::pin`), fixing the bounding box to exactly the configured size for
//! every frame and every style. Placement is then one rule shared by all five
//! styles, the same rule the lyric widget uses. `Path::pt` clamps every
//! coordinate into that box as well, so the pin cannot be defeated by a
//! geometry mistake either.
//!
//! # Values that must never reach the screen
//!
//! One malformed number does not cost one bar, it costs the whole overlay:
//! libass discards a drawing it cannot parse, and `NaN`/`inf` are exactly what
//! an FFT yields from a silent buffer or a broken capture. So no float in this
//! module is ever formatted. Band values go through `clamp01`, coordinates go
//! through `Path::pt`, and both funnel into `du`, which returns `i32` — Rust
//! defines float-to-integer casts as saturating with `NaN` mapping to zero, so
//! the type system carries the guarantee instead of a review comment.
//!
//! # Power
//!
//! [`is_silent`] exists so the daemon can stop pushing frames entirely when
//! nothing is playing. The roadmap's power model is not negotiable: no audio
//! must mean *no redraw*, not a redraw of an empty widget.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::fmt::Write as _;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::lyrics::{hex_to_ass_colour, Anchor, PLAY_RES_X, PLAY_RES_Y};

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

/// A resolved visualiser look: everything [`render_ass`] needs, with nothing
/// left to look up.
///
/// Like `crate::lyrics::LyricStyle` this is the *output* of preset resolution
/// rather than the preset itself, which is what lets the whole ASS payload be a
/// pure function of one struct.
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
    /// Box height in [`PLAY_RES_Y`] units, i.e. pixels at 1080p.
    #[serde(default = "default_height_px")]
    pub height_px: u32,
    /// Distance from the anchored edge(s), in [`PLAY_RES_Y`] units. Ignored on
    /// whichever axis the anchor is centred, exactly as in the lyric widget.
    #[serde(default = "default_margin_px")]
    pub margin_px: u32,
    /// Fill colour as `#RRGGBB`; converted to ASS on render. Used when
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

/// The `\p` scale exponent. One means a divisor of `2^0`, i.e. drawing units
/// are PLAY_RES units — see the module docs.
const DRAW_SCALE: u32 = 1;

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

/// Every shape keeps at least this much extent, so a silent spectrum rests as a
/// hairline rather than vanishing mid-track and looking like a crash.
const MIN_BAR_PX: f32 = 2.0;

/// Largest share of a cell the gap may take. Past this the widget reads as
/// whitespace with a few splinters in it.
const MAX_GAP_FRACTION: f32 = 0.6;

/// Circle-to-Bézier constant: `4/3 · tan(π/8)`, the control-point offset that
/// makes a cubic match a quarter circle to within 0.03% of its radius.
const KAPPA: f32 = 0.552_284_8;

/// Hub radius of [`VisualStyle::Ring`] as a share of the enclosing circle. Big
/// enough that the bars read as radiating from something, small enough to leave
/// them room to move.
const RING_HUB_FRACTION: f32 = 0.42;

/// Most separate events one gradient frame may cost.
///
/// A colour step is an event, and an event is not free: its own override block
/// and its own copy of the bounding-box pin come to about 130 bytes before any
/// geometry, on a payload that crosses an IPC socket 15–30 times a second. Up
/// to this many bands each get their own colour; past it, adjacent bands share
/// one, so the cost of the gradient stops growing while the picture does not
/// visibly change — 48 steps put well under two degrees of hue between
/// neighbours on a full sweep, which is below what anyone can see on bars a few
/// pixels wide.
const MAX_GRADIENT_EVENTS: usize = 48;

/// Hue span of [`Gradient::Spectrum`], in degrees: red round to violet, the
/// order the visible spectrum comes in. Stops short of 360 so the sweep does
/// not arrive back at the red it started from.
const SPECTRUM_HUE_SPAN: f32 = 300.0;

/// Saturation and lightness [`Gradient::Spectrum`] holds while the hue sweeps.
/// Full saturation at mid lightness is neon; this is one step back from it, so
/// the widget is vivid without becoming the brightest thing on the desktop.
const SPECTRUM_SAT: f32 = 0.85;
/// See [`SPECTRUM_SAT`].
const SPECTRUM_LIGHT: f32 = 0.55;

/// The colour [`parse_rgb`] falls back to, matching what `hex_to_ass_colour`
/// does with input it cannot read: an unusable hex costs the tint, never the
/// widget.
const FALLBACK_RGB: (u8, u8, u8) = (255, 255, 255);

/// Render a spectrum frame as a complete `ass-events` payload for mpv's
/// `osd-overlay`.
///
/// `bands` are magnitudes in `0.0..=1.0`, low frequency first; anything else —
/// `NaN`, negative, above one — is clamped, because a broken capture must cost
/// the user their visualiser at worst and never their whole overlay.
///
/// `accent_hex` is the desktop accent as `#RRGGBB`, used when
/// [`VisualStyleCfg::accent_follow`] is set. An unparsable colour falls back to
/// white through [`hex_to_ass_colour`], the same way the lyric widget degrades.
///
/// An empty `bands` returns an **empty string**, which the daemon should treat
/// as "clear the overlay" — never a `{…}` block with nothing in it, which would
/// still cost libass a re-render to draw nothing.
///
/// With [`Gradient::None`] the result is **one** event and contains no
/// newlines. With a gradient it is one event per colour step, newline
/// separated, which is how mpv is asked for several events in one payload; each
/// carries its own complete override block and its own bounding-box pin, so
/// nothing is inherited from mpv's OSD style and every event lands on the same
/// pixel. See [`Gradient`] for why a gradient has to be spelled this way at
/// all.
pub fn render_ass(bands: &[f32], cfg: &VisualStyleCfg, accent_hex: &str) -> String {
    let bands = sanitise(bands);
    if bands.is_empty() {
        return String::new();
    }

    let (w, h) = box_size(cfg);
    let base = if cfg.accent_follow {
        accent_hex
    } else {
        cfg.colour.as_str()
    };
    let (x, y) = anchor_pos(cfg.anchor, cfg.margin_px);
    let head = Head {
        an: cfg.anchor.an(),
        x,
        y,
        alpha: ass_alpha(cfg.opacity),
    };

    let n = bands.len();
    // One drawing, one colour: the flat case stays a single event, byte for
    // byte what this module emitted before gradients existed.
    let Some(ramp) = ramp(cfg, base) else {
        let mut path = Path::new(w, h);
        path.pin();
        draw(&mut path, &bands, cfg, 0..n);
        return head.event(&hex_to_ass_colour(base), &path.finish());
    };

    let mut out = String::new();
    for (run, t) in colour_runs(n) {
        let mut path = Path::new(w, h);
        path.pin();
        draw(&mut path, &bands, cfg, run);
        if !out.is_empty() {
            out.push('\n');
        }
        // Back through the same `#RRGGBB` -> BGR conversion the flat path uses:
        // there is one colour formatter in the widget layer, and interpolation
        // does not get to be a second one.
        out.push_str(&head.event(&hex_to_ass_colour(&ramp.at(t)), &path.finish()));
    }
    out
}

/// Everything an event's override block needs except the colour — the parts
/// that are the same for every event of one frame.
struct Head {
    /// ASS `\an` alignment, 1–9.
    an: u8,
    /// `\pos` x in [`PLAY_RES_X`] units.
    x: u32,
    /// `\pos` y in [`PLAY_RES_Y`] units.
    y: u32,
    /// Already-inverted ASS alpha literal.
    alpha: String,
}

impl Head {
    /// One complete event: override block, drawing, and the `\p0` that closes
    /// drawing mode.
    fn event(&self, colour: &str, body: &str) -> String {
        format!(
            // Every visual property is set explicitly rather than inherited:
            // the OSD style this draws against is mpv's, not ours, and with a
            // gradient there are several events each of which would inherit it
            // separately. `\bord0\shad0` matters more here than for text — an
            // outline would trace all several hundred bar edges, and would give
            // the zero-area pin contours something to stroke. `\fscx\fscy\frz`
            // are set because libass scales and rotates drawings by them just
            // as it does glyphs.
            "{{\\an{an}\\pos({x},{y})\\bord0\\shad0\\fscx100\\fscy100\\frz0\
             \\1c{colour}\\alpha{alpha}\\p{DRAW_SCALE}}}{body}{{\\p0}}",
            an = self.an,
            x = self.x,
            y = self.y,
            alpha = self.alpha,
        )
    }
}

/// Draw `sel`'s worth of `bands` in the style `cfg` asks for.
///
/// The selection is what makes a gradient possible: every style except
/// [`VisualStyle::Wave`] draws each band independently, so a run of bands can
/// be drawn on its own into its own event without any of them moving. Passing
/// `0..bands.len()` reproduces the whole picture exactly.
fn draw(path: &mut Path, bands: &[f32], cfg: &VisualStyleCfg, sel: Range<usize>) {
    match cfg.style {
        VisualStyle::Bars => draw_bars(path, bands, cfg, sel),
        VisualStyle::Mirror => draw_mirror(path, bands, cfg, sel),
        // One continuous contour: it is always drawn whole, and `ramp` has
        // already refused to give this style more than one event.
        VisualStyle::Wave => draw_wave(path, bands, cfg),
        VisualStyle::Dots => draw_dots(path, bands, cfg, sel),
        VisualStyle::Ring => draw_ring(path, bands, cfg, sel),
    }
}

/// The runs of bands that share a colour, each with its position on the ramp.
///
/// Runs are laid out by the same integer split [`sanitise`] folds with, so they
/// tile the spectrum exactly: no band is drawn twice (which would show, at
/// partial opacity, as a brighter bar) and none is dropped. `t` is monotonic in
/// the run index, so the ramp always walks one way across the widget.
fn colour_runs(n: usize) -> impl Iterator<Item = (Range<usize>, f32)> {
    let runs = n.clamp(1, MAX_GRADIENT_EVENTS);
    (0..runs).map(move |g| {
        // `n` is capped at MAX_BANDS long before here, so this cannot overflow.
        let lo = g * n / runs;
        let hi = ((g + 1) * n / runs).clamp(lo + 1, n);
        let t = if runs < 2 {
            0.0
        } else {
            g as f32 / (runs - 1) as f32
        };
        (lo..hi, t)
    })
}

/// Whether every band is at or below `threshold` — i.e. there is nothing worth
/// drawing.
///
/// This is the power-model hook, not a convenience. The roadmap forbids a
/// render loop: with no audio the daemon must push **nothing**, so the correct
/// use is to skip the whole render-and-send path while this is true, and to
/// send one clear (an empty [`render_ass`]) on the transition into silence.
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

/// Clamp every band and cap how many of them are drawn.
///
/// Over [`MAX_BANDS`] the spectrum is folded by taking the **maximum** of each
/// group rather than truncating or averaging: peaks are the signal in a
/// spectrum display, so averaging flattens the picture and truncating silently
/// deletes the treble half of it.
fn sanitise(bands: &[f32]) -> Vec<f32> {
    if bands.len() <= MAX_BANDS {
        return bands.iter().copied().map(clamp01).collect();
    }
    let n = bands.len() as u64;
    (0..MAX_BANDS)
        .map(|i| {
            // u64 throughout: `i * bands.len()` overflows a 32-bit usize for a
            // large enough slice, and this is the only multiplication here that
            // is not bounded by the screen.
            let lo = (i as u64 * n / MAX_BANDS as u64) as usize;
            let hi = (((i as u64 + 1) * n / MAX_BANDS as u64) as usize).max(lo + 1);
            bands[lo..hi.min(bands.len())]
                .iter()
                .copied()
                .map(clamp01)
                .fold(0.0f32, f32::max)
        })
        .collect()
}

/// Widget box size in drawing units, with every hand-editable number clamped.
///
/// [`VisualStyle::Ring`] is squared off to the smaller side. The box is what
/// the anchor positions, so a round widget inside a 1152×120 box would be
/// bottom-*centre* correct and bottom-*left* wrong by half a screen of
/// invisible padding. Both dimensions still act as constraints; neither is
/// ignored.
fn box_size(cfg: &VisualStyleCfg) -> (f32, f32) {
    let pct = if cfg.width_pct.is_finite() {
        cfg.width_pct.clamp(MIN_WIDTH_PCT, MAX_WIDTH_PCT)
    } else {
        // A NaN width would otherwise poison every coordinate downstream.
        default_width_pct()
    };
    let w = PLAY_RES_X as f32 * pct / 100.0;
    let h = cfg.height_px.clamp(MIN_HEIGHT_PX, PLAY_RES_Y) as f32;
    if cfg.style == VisualStyle::Ring {
        let side = w.min(h);
        return (side, side);
    }
    (w, h)
}

/// Anchor point for `\pos`, in the [`PLAY_RES_X`]×[`PLAY_RES_Y`] space.
///
/// Deliberately the same rule as the lyric widget's private `anchor_pos`, so a
/// bottom-centre visualiser and a bottom-centre lyric line agree about what
/// "bottom centre" means. Duplicated rather than shared because that function
/// is private to a module this one has no business widening the API of, and
/// twelve lines of arithmetic is a cheaper coupling than a new public helper.
///
/// ASS has no per-event margin override (`\marginl`/`\marginv` are style
/// fields, not tags), so honouring `margin_px` means positioning explicitly.
fn anchor_pos(anchor: Anchor, margin: u32) -> (u32, u32) {
    // Derived from `an()` rather than a second match, so the alignment and the
    // position it is paired with cannot disagree.
    let n = anchor.an() - 1;
    // A margin past the centre would flip the sides; clamp instead.
    let mx = margin.min(PLAY_RES_X / 2);
    let my = margin.min(PLAY_RES_Y / 2);
    let x = match n % 3 {
        0 => mx,
        1 => PLAY_RES_X / 2,
        _ => PLAY_RES_X - mx,
    };
    let y = match n / 3 {
        0 => PLAY_RES_Y - my,
        1 => PLAY_RES_Y / 2,
        _ => my,
    };
    (x, y)
}

/// `opacity` (0 invisible … 255 solid) as an ASS alpha literal.
///
/// ASS alpha runs the other way — `&H00&` is opaque and `&HFF&` invisible —
/// which is exactly the inversion that ships as "the opacity slider works
/// backwards".
fn ass_alpha(opacity: u8) -> String {
    format!("&H{:02X}&", 255 - opacity)
}

/// `#RGB` / `#RRGGBB` → `(r, g, b)`, falling back to [`FALLBACK_RGB`].
///
/// Deliberately a copy of the rule `crate::lyrics::hex_to_ass_colour` parses
/// with, and not a call into it: that function's parser is private and returns
/// an ASS literal, which is the one thing interpolation cannot start from —
/// the ramp needs numbers. Twelve lines of hex arithmetic is a cheaper coupling
/// than widening another module's API, and the shared test
/// `interpolated_colours_use_the_same_conversion_as_flat_ones` holds the two
/// spellings together.
fn parse_rgb(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return FALLBACK_RGB;
    }
    match h.len() {
        // Shorthand doubles each nibble, so `f` is 0xFF and not 0xF0.
        3 => match u16::from_str_radix(h, 16) {
            Ok(v) => {
                let nib = |shift: u32| (((v >> shift) & 0xF) as u8) * 0x11;
                (nib(8), nib(4), nib(0))
            }
            Err(_) => FALLBACK_RGB,
        },
        6 => {
            let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
            (byte(0), byte(2), byte(4))
        }
        _ => FALLBACK_RGB,
    }
}

/// One colour as hue in degrees, saturation and lightness in `0.0..=1.0`.
#[derive(Debug, Clone, Copy)]
struct Hsl {
    /// Hue in degrees, `0.0..360.0`.
    h: f32,
    /// Saturation, `0.0..=1.0`.
    s: f32,
    /// Lightness, `0.0..=1.0`.
    l: f32,
}

/// sRGB bytes → HSL.
fn rgb_to_hsl((r, g, b): (u8, u8, u8)) -> Hsl {
    let (r, g, b) = (
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let chroma = max - min;
    if chroma <= 0.0 {
        // Grey has no hue. Left at zero here and fixed up in `Ramp::linear`,
        // which borrows the other end's hue so a ramp into white or black stays
        // on one hue instead of swinging through red.
        return Hsl { h: 0.0, s: 0.0, l };
    }
    // The standard sextant construction. `max` is one of the three by
    // definition, so the final arm is the blue one and not a fallback.
    let sextant = if max == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    Hsl {
        h: (sextant * 60.0).rem_euclid(360.0),
        s: clamp01(chroma / (1.0 - (2.0 * l - 1.0).abs()).max(f32::EPSILON)),
        l,
    }
}

/// HSL → sRGB bytes. Every arithmetic path ends in a saturating `as u8`, so no
/// value of the input can produce anything but three bytes.
fn hsl_to_rgb(c: Hsl) -> (u8, u8, u8) {
    let h = if c.h.is_finite() {
        c.h.rem_euclid(360.0) / 60.0
    } else {
        0.0
    };
    let (s, l) = (clamp01(c.s), clamp01(c.l));
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = chroma * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = l - chroma / 2.0;
    let q = |v: f32| (clamp01(v + m) * 255.0).round() as u8;
    (q(r), q(g), q(b))
}

/// A colour ramp: where it starts, and how far it travels.
///
/// **Interpolated in HSL, not in sRGB.** A straight lerp between two sRGB
/// triples walks through the middle of the colour cube, and for two colours on
/// opposite sides of it — pink and cyan, the pair people actually pick — the
/// middle of the cube is grey. The result is a ramp that goes vivid, muddy,
/// vivid, which is exactly the fault that had to be taken back out of the
/// clock's border. Linear-light RGB fixes the *brightness* of that midpoint but
/// not its greyness: it is still the same straight line through the same
/// desaturated centre. Rotating the hue instead never leaves the outside of the
/// cube, so every step of the ramp is as saturated as the ends are, which is
/// what a visualiser gradient is for.
#[derive(Debug, Clone, Copy)]
struct Ramp {
    /// The near end.
    a: Hsl,
    /// The far end. Its hue is unused — `dh` carries the rotation — but its
    /// saturation and lightness are interpolated towards.
    b: Hsl,
    /// Signed hue rotation in degrees from `a` to `b`.
    dh: f32,
}

impl Ramp {
    /// The ramp between two colours, taking the **short** way round the hue
    /// circle: red to blue goes through magenta rather than through the whole
    /// spectrum, because a user who picks two colours asked for those two
    /// colours and not for a rainbow. [`Gradient::Spectrum`] is where the long
    /// way round lives.
    fn linear(from: (u8, u8, u8), to: (u8, u8, u8)) -> Ramp {
        let (mut a, mut b) = (rgb_to_hsl(from), rgb_to_hsl(to));
        // A grey end has no hue of its own, so it borrows the other's: white to
        // blue then stays blue and pales out, instead of sliding through red on
        // the way.
        if a.s <= 0.0 {
            a.h = b.h;
        }
        if b.s <= 0.0 {
            b.h = a.h;
        }
        let mut dh = b.h - a.h;
        if dh > 180.0 {
            dh -= 360.0;
        } else if dh < -180.0 {
            dh += 360.0;
        }
        Ramp { a, b, dh }
    }

    /// The fixed hue sweep of [`Gradient::Spectrum`].
    fn spectrum() -> Ramp {
        let end = Hsl {
            h: SPECTRUM_HUE_SPAN,
            s: SPECTRUM_SAT,
            l: SPECTRUM_LIGHT,
        };
        Ramp {
            a: Hsl { h: 0.0, ..end },
            b: end,
            // Explicitly the long way round: the visible spectrum runs red,
            // yellow, green, cyan, blue, violet, and the short way from red to
            // violet is the 60° through pink that skips all of it.
            dh: SPECTRUM_HUE_SPAN,
        }
    }

    /// Position `t` (`0.0..=1.0`) on the ramp, as `#RRGGBB`.
    fn at(&self, t: f32) -> String {
        let t = clamp01(t);
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        let (r, g, b) = hsl_to_rgb(Hsl {
            h: self.a.h + self.dh * t,
            s: lerp(self.a.s, self.b.s),
            l: lerp(self.a.l, self.b.l),
        });
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// Whether the two ends are the same colour, i.e. whether this ramp is
    /// worth paying for. A flat "gradient" would cost one event per bar and
    /// look exactly like one event.
    fn is_flat(&self) -> bool {
        parse_rgb(&self.at(0.0)) == parse_rgb(&self.at(1.0)) && self.dh.abs() < f32::EPSILON
    }
}

/// The ramp this configuration calls for, or `None` for a single flat event.
///
/// Three ways to get `None`, and each is a deliberate saving rather than a
/// failure: the mode is off; the style is [`VisualStyle::Wave`], whose single
/// contour cannot carry a per-bar colour (see [`Gradient`]); or the two ends
/// are the same colour, so every event would be the same colour too.
fn ramp(cfg: &VisualStyleCfg, base_hex: &str) -> Option<Ramp> {
    if cfg.style == VisualStyle::Wave {
        return None;
    }
    let r = match cfg.gradient {
        Gradient::None => return None,
        Gradient::Linear => Ramp::linear(parse_rgb(base_hex), parse_rgb(&cfg.colour_end)),
        Gradient::Spectrum => Ramp::spectrum(),
    };
    (!r.is_flat()).then_some(r)
}

/// One geometry value as one emittable drawing unit.
///
/// The cast is the guarantee, not a convenience: Rust defines float-to-integer
/// casts as saturating, with `NaN` mapping to zero, so no value of `v` — `NaN`,
/// `±inf`, `1e30` — can produce anything but an integer. Every number in the
/// output passes through here.
fn du(v: f32) -> i32 {
    v.round() as i32
}

/// Cartesian point at `angle` on the circle `(cx, cy, r)`. Angles are ASS
/// screen angles: y grows downward, so they run clockwise from three o'clock.
fn polar(cx: f32, cy: f32, r: f32, angle: f32) -> (f32, f32) {
    (cx + r * angle.cos(), cy + r * angle.sin())
}

/// Which ends of a bar are rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cap {
    /// Both ends square.
    Square,
    /// The top edge only — a bar standing on a floor.
    Top,
    /// Both ends — a bar hanging in space around a centre line.
    Both,
}

/// Accumulates one ASS drawing path.
///
/// Two invariants live here and nowhere else, which is the point of the type:
///
/// 1. **No float is ever formatted.** Every coordinate enters through
///    [`Path::pt`] and leaves as `i32`, so `NaN`/`inf` cannot reach libass.
/// 2. **Nothing is drawn outside the box.** `pt` clamps into `0..=w` / `0..=h`,
///    so the bounding box stays exactly the one [`Path::pin`] declared no
///    matter what the style code computes — a geometry bug clips a shape
///    instead of teleporting the widget.
struct Path {
    /// Box width in drawing units, as the styles want it.
    w: f32,
    /// Box height in drawing units, as the styles want it.
    h: f32,
    /// Box width as the clamp bound.
    xmax: i32,
    /// Box height as the clamp bound.
    ymax: i32,
    /// The path built so far, space-separated.
    d: String,
}

impl Path {
    fn new(w: f32, h: f32) -> Self {
        Path {
            w,
            h,
            xmax: du(w).clamp(1, PLAY_RES_X as i32),
            ymax: du(h).clamp(1, PLAY_RES_Y as i32),
            d: String::new(),
        }
    }

    /// The only float-to-text boundary in the module.
    fn pt(&self, x: f32, y: f32) -> (i32, i32) {
        (du(x).clamp(0, self.xmax), du(y).clamp(0, self.ymax))
    }

    /// Fix the drawing's bounding box to the whole widget box.
    ///
    /// Two zero-area contours at opposite corners. libass sizes a drawing from
    /// the bounding box of its points, and drops zero-length segments before
    /// rasterising, so these cost nothing on screen and cost the widget its
    /// content-dependent wobble — see the module docs. Three coincident points
    /// each, rather than two, because a two-point contour is a degenerate case
    /// worth not relying on.
    fn pin(&mut self) {
        let (x, y) = (self.xmax, self.ymax);
        let _ = write!(self.d, "m 0 0 l 0 0 l 0 0 m {x} {y} l {x} {y} l {x} {y} ");
    }

    fn move_to(&mut self, p: (f32, f32)) {
        let (x, y) = self.pt(p.0, p.1);
        let _ = write!(self.d, "m {x} {y} ");
    }

    fn line_to(&mut self, p: (f32, f32)) {
        let (x, y) = self.pt(p.0, p.1);
        let _ = write!(self.d, "l {x} {y} ");
    }

    fn curve_to(&mut self, c1: (f32, f32), c2: (f32, f32), p: (f32, f32)) {
        let (x1, y1) = self.pt(c1.0, c1.1);
        let (x2, y2) = self.pt(c2.0, c2.1);
        let (x3, y3) = self.pt(p.0, p.1);
        let _ = write!(self.d, "b {x1} {y1} {x2} {y2} {x3} {y3} ");
    }

    /// One cubic approximating the arc from `a0` to `a1` on `(cx, cy, r)`.
    /// Assumes the current point is already the arc's start.
    ///
    /// Control points sit on the endpoint tangents at `4/3·tan(θ/4)·r`, the
    /// standard construction; it is exact at both ends and its worst radial
    /// error over a quarter turn is under 0.03%.
    fn arc_to(&mut self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let k = 4.0 / 3.0 * ((a1 - a0) / 4.0).tan() * r;
        let p0 = polar(cx, cy, r, a0);
        let p1 = polar(cx, cy, r, a1);
        let c1 = (p0.0 - k * a0.sin(), p0.1 + k * a0.cos());
        let c2 = (p1.0 + k * a1.sin(), p1.1 - k * a1.cos());
        self.curve_to(c1, c2, p1);
    }

    /// A full circle as four cubic arcs.
    ///
    /// `reverse` walks it the other way round. That is how the ring hub becomes
    /// a ring instead of a disc: libass fills by non-zero winding, so a
    /// reversed inner circle inside a forward outer one cancels to a hole.
    fn circle(&mut self, cx: f32, cy: f32, r: f32, reverse: bool) {
        let step = if reverse { -FRAC_PI_2 } else { FRAC_PI_2 };
        self.move_to(polar(cx, cy, r, 0.0));
        for i in 0..4 {
            let a0 = i as f32 * step;
            self.arc_to(cx, cy, r, a0, a0 + step);
        }
    }

    /// An axis-aligned rectangle.
    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.move_to((x0, y0));
        self.line_to((x1, y0));
        self.line_to((x1, y1));
        self.line_to((x0, y1));
    }

    /// A vertical bar from `y0` (top) to `y1` (bottom), with the requested ends
    /// rounded.
    ///
    /// The corner radius is capped so the two corners of a rounded edge can
    /// never overlap and fold the contour through itself — a short wide bar
    /// therefore ends in a lozenge rather than in a knot. Below one drawing
    /// unit the curves would round to nothing, so the bar degrades to a plain
    /// rectangle instead of emitting four Béziers that draw straight lines.
    fn bar(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, cap: Cap) {
        let (w, h) = (x1 - x0, y1 - y0);
        let r = match cap {
            Cap::Square => 0.0,
            // A top-only cap may use the full height for its radius; a
            // both-ends cap has to share it.
            Cap::Top => (w / 2.0).min(h).max(0.0),
            Cap::Both => (w / 2.0).min(h / 2.0).max(0.0),
        };
        if r < 1.0 {
            self.rect(x0, y0, x1, y1);
            return;
        }
        let k = r * (1.0 - KAPPA);
        match cap {
            Cap::Square => unreachable!("handled by the radius test above"),
            Cap::Top => {
                self.move_to((x0, y1));
                self.line_to((x0, y0 + r));
                self.curve_to((x0, y0 + k), (x0 + k, y0), (x0 + r, y0));
                self.line_to((x1 - r, y0));
                self.curve_to((x1 - k, y0), (x1, y0 + k), (x1, y0 + r));
                self.line_to((x1, y1));
            }
            Cap::Both => {
                self.move_to((x0, y1 - r));
                self.curve_to((x0, y1 - k), (x0 + k, y1), (x0 + r, y1));
                self.line_to((x1 - r, y1));
                self.curve_to((x1 - k, y1), (x1, y1 - k), (x1, y1 - r));
                self.line_to((x1, y0 + r));
                self.curve_to((x1, y0 + k), (x1 - k, y0), (x1 - r, y0));
                self.line_to((x0 + r, y0));
                self.curve_to((x0 + k, y0), (x0, y0 + k), (x0, y0 + r));
            }
        }
    }

    /// The finished path, with the trailing separator removed.
    fn finish(self) -> String {
        let mut d = self.d;
        while d.ends_with(' ') {
            d.pop();
        }
        d
    }
}

/// Width of one band's slot, and the space between slots.
///
/// The gap is capped at [`MAX_GAP_FRACTION`] of the cell so that a hand-edited
/// `gap_px` of 400 with 64 bands still leaves bars to look at, and the bar
/// keeps at least one unit of width so it is never a zero-area contour.
fn cell_metrics(cfg: &VisualStyleCfg, w: f32, n: usize) -> (f32, f32) {
    let cell = w / n.max(1) as f32;
    let gap = (cfg.gap_px as f32).clamp(0.0, cell * MAX_GAP_FRACTION);
    (cell, (cell - gap).max(1.0))
}

/// Extent of one band, never below [`MIN_BAR_PX`] and never above `full`.
fn extent(v: f32, full: f32) -> f32 {
    (v * full).clamp(MIN_BAR_PX.min(full), full)
}

/// Classic bars, standing on the floor of the box and growing upward.
///
/// Only the bands in `sel` are emitted; the geometry of each is a function of
/// its own index, so a partial draw is a subset of the whole picture and never
/// a rearrangement of it. Same for [`draw_mirror`], [`draw_dots`] and
/// [`draw_ring`].
fn draw_bars(p: &mut Path, bands: &[f32], cfg: &VisualStyleCfg, sel: Range<usize>) {
    let (w, h) = (p.w, p.h);
    let (cell, bw) = cell_metrics(cfg, w, bands.len());
    let cap = if cfg.rounded { Cap::Top } else { Cap::Square };
    for i in sel {
        let v = bands[i];
        // Centre the bar in its cell so the row stays symmetric about the box
        // even when the gap does not divide evenly.
        let x0 = i as f32 * cell + (cell - bw) / 2.0;
        let bh = extent(v, h);
        p.bar(x0, h - bh, x0 + bw, h, cap);
    }
}

/// Bars mirrored about the box's horizontal centre line.
fn draw_mirror(p: &mut Path, bands: &[f32], cfg: &VisualStyleCfg, sel: Range<usize>) {
    let (w, h) = (p.w, p.h);
    let (cell, bw) = cell_metrics(cfg, w, bands.len());
    let cap = if cfg.rounded { Cap::Both } else { Cap::Square };
    let mid = h / 2.0;
    for i in sel {
        let v = bands[i];
        let x0 = i as f32 * cell + (cell - bw) / 2.0;
        // Half above, half below: the bar's total extent still spans the box at
        // full scale, so Mirror and Bars respond to the same band identically.
        let half = extent(v, h) / 2.0;
        p.bar(x0, mid - half, x0 + bw, mid + half, cap);
    }
}

/// One continuous filled silhouette under the spectrum's outline.
///
/// The samples sit at the box edges rather than at cell centres: a silhouette
/// that starts half a cell in reads as a rendering bug, where a bar chart with
/// the same inset reads as padding.
///
/// `rounded` picks the joins. Curved uses a cubic per segment with both control
/// points on the horizontal thirds at their own endpoint's height — a smooth
/// S-curve that is flat at every sample and, because both control points lie
/// between the endpoint heights, **cannot overshoot the box**. Straight gives
/// the same outline as a polyline, an angular mountain range. `gap_px` has no
/// meaning for a continuous curve and is ignored.
fn draw_wave(p: &mut Path, bands: &[f32], cfg: &VisualStyleCfg) {
    let (w, h) = (p.w, p.h);
    let n = bands.len();
    let top = |v: f32| h - extent(v, h);
    let x_at = |i: usize| {
        if n < 2 {
            0.0
        } else {
            i as f32 * w / (n - 1) as f32
        }
    };

    // Start and finish on the floor so the fill is a silhouette, not a ribbon.
    p.move_to((0.0, h));
    p.line_to((0.0, top(bands[0])));
    for i in 1..n {
        let (x0, y0) = (x_at(i - 1), top(bands[i - 1]));
        let (x1, y1) = (x_at(i), top(bands[i]));
        if cfg.rounded {
            let third = (x1 - x0) / 3.0;
            p.curve_to((x0 + third, y0), (x1 - third, y1), (x1, y1));
        } else {
            p.line_to((x1, y1));
        }
    }
    // With one band the loop above drew nothing, so square the top off. With
    // more, the last segment already ended on the right edge.
    if n < 2 {
        p.line_to((w, top(bands[0])));
    }
    p.line_to((w, h));
}

/// A row of dots that ride from floor to ceiling and grow with their band.
///
/// Deliberately *not* a bar with a dot on top: the dots float free, so the
/// silhouette is a scatter rather than a skyline, which is the point of having
/// the style at all. Size tracks the band as well as height, so a quiet band
/// recedes instead of just sinking.
fn draw_dots(p: &mut Path, bands: &[f32], cfg: &VisualStyleCfg, sel: Range<usize>) {
    let (w, h) = (p.w, p.h);
    let (cell, slot) = cell_metrics(cfg, w, bands.len());
    // A dot may be neither wider than its slot nor taller than the box.
    let dmax = slot.min(h).max(1.0);
    let dmin = (dmax * 0.35).max(1.0);
    for i in sel {
        let v = bands[i];
        let d = dmin + (dmax - dmin) * v;
        let cx = (i as f32 + 0.5) * cell;
        // The centre travels between one radius off each edge, so the dot is
        // fully inside the box at both extremes instead of half-clipped.
        let cy = (h - d / 2.0) - v * (h - d).max(0.0);
        if cfg.rounded {
            p.circle(cx, cy, d / 2.0, false);
        } else {
            p.rect(cx - d / 2.0, cy - d / 2.0, cx + d / 2.0, cy + d / 2.0);
        }
    }
}

/// Bars radiating outward from a hub ring.
///
/// The layout is polar: band zero at twelve o'clock, running clockwise, so the
/// spectrum reads low-to-high the same way it does left-to-right everywhere
/// else. The hub is a true annulus — an outer circle with a reversed inner one
/// punched through it — and each bar is the region between two radii over its
/// angular slice. `gap_px` is converted to an angle at the hub radius, so a
/// given gap looks about the same here as it does between two upright bars.
///
/// [`box_size`] has already squared the box off to the smaller of the
/// configured dimensions, so the circle fills it and the anchor has nothing
/// invisible to position around.
fn draw_ring(p: &mut Path, bands: &[f32], cfg: &VisualStyleCfg, sel: Range<usize>) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let outer = (w.min(h) / 2.0).max(2.0);
    let hub = (outer * RING_HUB_FRACTION).max(1.0);
    let thick = (hub * 0.12).clamp(1.0, 8.0);
    let span = (outer - hub).max(1.0);

    // Outer forward, inner reversed: non-zero winding turns the pair into a
    // ring. Drawn in the same contour set as the bars because one drawing is
    // one glyph — a second `\p` block would be laid out *after* this one.
    //
    // With a gradient the bars are split across several events, and the hub
    // belongs to exactly one of them: drawn once per event it would be
    // overpainted in every colour of the ramp, and at partial opacity each pass
    // would darken it further. The one it belongs to is the run holding the
    // middle band, so the hub takes the middle of the ramp — a ring whose hub
    // is the colour of the first bar looks like a mistake, and one coloured
    // halfway along reads as the average of the thing it sits inside.
    if sel.contains(&(bands.len() / 2)) {
        p.circle(cx, cy, hub, false);
        p.circle(cx, cy, (hub - thick).max(0.5), true);
    }

    let sector = TAU / bands.len() as f32;
    let gap = (cfg.gap_px as f32 / hub).clamp(0.0, sector * MAX_GAP_FRACTION);
    let arc = (sector - gap).max(sector * 0.2);
    for i in sel {
        let v = bands[i];
        // Band zero is *centred* on twelve o'clock rather than starting there,
        // so the figure is symmetric about the vertical for an even band count.
        let mid = -FRAC_PI_2 + i as f32 * sector;
        let (a0, a1) = (mid - arc / 2.0, mid + arc / 2.0);
        let r1 = hub + extent(v, span);
        p.move_to(polar(cx, cy, hub, a0));
        p.line_to(polar(cx, cy, r1, a0));
        if cfg.rounded {
            // Follow the circle, so the rim reads as round rather than as a
            // polygon with as many sides as there are bands.
            p.arc_to(cx, cy, r1, a0, a1);
        } else {
            p.line_to(polar(cx, cy, r1, a1));
        }
        // The inner edge stays a chord either way: it abuts the hub, which
        // already covers it.
        p.line_to(polar(cx, cy, hub, a1));
    }
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

    /// Split a rendered event into (override tags, drawing body).
    ///
    /// Also asserts the frame every test depends on: one leading override
    /// block, one trailing `{\p0}`, and no newline anywhere (mpv would turn one
    /// into a second, unstyled event).
    fn split(s: &str) -> (String, String) {
        assert!(!s.contains('\n'), "payload must be one event: {s}");
        assert!(s.starts_with('{'), "no leading override block: {s}");
        assert!(s.ends_with("{\\p0}"), "drawing mode left open: {s}");
        assert_eq!(
            s.matches('{').count(),
            2,
            "expected exactly two blocks: {s}"
        );
        assert_eq!(s.matches('}').count(), 2, "unbalanced braces: {s}");
        assert_eq!(s.matches("\\p1").count(), 1, "expected one \\p1: {s}");
        assert_eq!(s.matches("\\p0").count(), 1, "expected one \\p0: {s}");
        let open = s.find('}').expect("an override block");
        let close = s.rfind('{').expect("the closing block");
        (s[1..open].to_string(), s[open + 1..close].to_string())
    }

    /// Full structural validation of a rendered event.
    ///
    /// Checks the thing that actually breaks in production: libass discards a
    /// drawing it cannot parse, taking the whole overlay with it. So every
    /// token must be a known command or an integer, every command must have its
    /// exact arity, and no coordinate may sit outside the box the bounding-box
    /// pin declared.
    fn assert_valid(s: &str, cfg: &VisualStyleCfg) {
        assert!(
            !s.to_ascii_lowercase().contains("nan") && !s.to_ascii_lowercase().contains("inf"),
            "float garbage reached the overlay: {s}"
        );
        let (tags, body) = split(s);
        assert!(tags.contains("\\an"), "no alignment: {tags}");
        assert!(tags.contains("\\pos("), "no position: {tags}");
        assert!(!body.trim().is_empty(), "empty drawing");

        let (w, h) = box_size(cfg);
        let (xmax, ymax) = (du(w), du(h));

        let mut toks = body.split_ascii_whitespace().peekable();
        assert_eq!(toks.peek().copied(), Some("m"), "a path must open with m");
        let mut commands = 0usize;
        while let Some(t) = toks.next() {
            let arity = match t {
                "m" | "l" => 2,
                "b" => 6,
                other => panic!("unknown drawing command {other:?} in {body}"),
            };
            commands += 1;
            for k in 0..arity {
                let n = toks
                    .next()
                    .unwrap_or_else(|| panic!("{t} short of arguments in {body}"));
                let v: i32 = n
                    .parse()
                    .unwrap_or_else(|e| panic!("non-integer coordinate {n:?} ({e}) in {body}"));
                let limit = if k % 2 == 0 { xmax } else { ymax };
                assert!(
                    (0..=limit).contains(&v),
                    "coordinate {v} outside the pinned 0..={limit} box in {body}"
                );
            }
        }
        assert!(commands >= 3, "suspiciously short path: {body}");
    }

    /// A payload's events: one for a flat spectrum, one per colour step for a
    /// gradient. mpv does exactly this split before handing them to libass.
    fn events(s: &str) -> Vec<String> {
        s.split('\n').map(str::to_string).collect()
    }

    /// Every event of a payload, validated the way a single one is, plus the
    /// two things only a multi-event payload can get wrong: a missing pin, and
    /// events that do not land on the same spot.
    fn assert_valid_payload(s: &str, cfg: &VisualStyleCfg) {
        let evs = events(s);
        assert!(!evs.is_empty(), "no events");
        let head = |e: &str| split(e).0[..split(e).0.find("\\1c").expect("a colour")].to_string();
        for e in &evs {
            assert_valid(e, cfg);
            assert_eq!(head(e), head(&evs[0]), "events disagree about placement");
            let (w, h) = box_size(cfg);
            let (x, y) = (du(w), du(h));
            assert!(
                split(e)
                    .1
                    .starts_with(&format!("m 0 0 l 0 0 l 0 0 m {x} {y} l {x} {y} l {x} {y} ")),
                "event is not pinned to the widget box: {e}"
            );
        }
    }

    /// The `\1c` fill of one event as `(r, g, b)` — undoing the BGR order ASS
    /// stores it in, so a test can talk about colours in the order humans do.
    fn event_rgb(ev: &str) -> (u8, u8, u8) {
        let at = ev.find("\\1c&H").expect("a colour tag") + 5;
        let hex = &ev[at..at + 6];
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex");
        (byte(4), byte(2), byte(0))
    }

    /// How many contours a drawing has, i.e. `m` commands including the two
    /// bounding-box pins.
    fn contours(s: &str) -> usize {
        split(s)
            .1
            .split_ascii_whitespace()
            .filter(|t| *t == "m")
            .count()
    }

    /// Vertices the bounding-box pin contributes, and which no test about the
    /// *picture* should look at: three coincident points per corner.
    const PIN_POINTS: usize = 6;

    /// One vertex read back out of a rendered path.
    type Pt = (i32, i32);

    /// Every vertex of the picture, pin excluded. Safe to chunk blindly because
    /// every command in the grammar takes an even number of coordinates.
    fn shape_points(s: &str) -> Vec<Pt> {
        split(s)
            .1
            .split_ascii_whitespace()
            .filter_map(|t| t.parse::<i32>().ok())
            .collect::<Vec<_>>()
            .chunks_exact(2)
            .skip(PIN_POINTS)
            .map(|c| (c[0], c[1]))
            .collect()
    }

    /// The picture's points split into (on-curve, off-curve control), pin
    /// excluded.
    ///
    /// The distinction matters for anything curved: a cubic's control points
    /// sit *outside* the arc they approximate — by about 0.9% of the radius
    /// over a quarter turn — so a test that treated them as points on the shape
    /// would be measuring the wrong circle.
    fn curve_points(s: &str) -> (Vec<Pt>, Vec<Pt>) {
        let (mut on, mut off) = (Vec::new(), Vec::new());
        let body = split(s).1;
        let mut toks = body.split_ascii_whitespace();
        let mut seen = 0usize;
        while let Some(cmd) = toks.next() {
            let arity = if cmd == "b" { 6 } else { 2 };
            let mut pts = Vec::new();
            for _ in 0..arity / 2 {
                let x: i32 = toks.next().expect("an x").parse().expect("an integer");
                let y: i32 = toks.next().expect("a y").parse().expect("an integer");
                pts.push((x, y));
            }
            for (i, p) in pts.iter().enumerate() {
                seen += 1;
                if seen <= PIN_POINTS {
                    continue;
                }
                // In `b c1 c2 p`, only the last pair lies on the curve.
                if cmd == "b" && i < 2 {
                    off.push(*p);
                } else {
                    on.push(*p);
                }
            }
        }
        (on, off)
    }

    fn cfg_for(style: VisualStyle) -> VisualStyleCfg {
        VisualStyleCfg {
            style,
            ..Default::default()
        }
    }

    #[test]
    fn every_style_renders_a_well_formed_drawing() {
        for style in VisualStyle::ALL {
            let cfg = cfg_for(style);
            let out = render_ass(&spectrum(), &cfg, "#3584E4");
            assert_valid(&out, &cfg);
            // And with the hard-edged variant, which takes different branches
            // in every one of the five.
            let square = VisualStyleCfg {
                rounded: false,
                ..cfg.clone()
            };
            let out = render_ass(&spectrum(), &square, "#3584E4");
            assert_valid(&out, &square);
        }
    }

    #[test]
    fn the_styles_are_actually_different_pictures() {
        // Cheap guard against a style falling through to another's arm: five
        // menu entries that render the same thing is the failure a user sees
        // before any test does.
        let bodies: Vec<String> = VisualStyle::ALL
            .iter()
            .map(|s| split(&render_ass(&spectrum(), &cfg_for(*s), "#FFFFFF")).1)
            .collect();
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                assert_ne!(
                    bodies[i],
                    bodies[j],
                    "{:?} and {:?} draw the same path",
                    VisualStyle::ALL[i],
                    VisualStyle::ALL[j]
                );
            }
        }
    }

    #[test]
    fn empty_bands_clear_the_overlay() {
        // An empty string is how the daemon says "remove this overlay". A
        // `{...}` with no drawing would still cost libass a re-render.
        for style in VisualStyle::ALL {
            assert_eq!(render_ass(&[], &cfg_for(style), "#FFFFFF"), "");
        }
    }

    #[test]
    fn silence_rests_as_a_hairline_rather_than_vanishing() {
        // All-zero bands are a normal frame, not an error: the widget should
        // sit at its floor. Vanishing mid-track reads as a crash.
        for style in VisualStyle::ALL {
            let cfg = cfg_for(style);
            let out = render_ass(&[0.0; 16], &cfg, "#FFFFFF");
            assert_valid(&out, &cfg);
        }
        // For Bars specifically, every bar hugs the floor of a 120-unit box:
        // MIN_BAR_PX tall, and nothing anywhere near the ceiling.
        let cfg = cfg_for(VisualStyle::Bars);
        let out = render_ass(&[0.0; 8], &cfg, "#FFFFFF");
        assert!(
            shape_points(&out).iter().all(|(_, y)| *y >= 118),
            "resting bars should hug the floor: {out}"
        );
    }

    #[test]
    fn hostile_band_values_cannot_produce_malformed_ass() {
        // The bug this whole module is shaped around: one `NaN` in the payload
        // and libass drops the drawing, so the user loses the overlay rather
        // than one bar. Negative and >1 values come from an unnormalised FFT.
        let evil = [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1.0,
            -0.0,
            5.0,
            1e30,
            -1e30,
            f32::MIN,
            f32::MAX,
            f32::EPSILON,
            0.5,
        ];
        for style in VisualStyle::ALL {
            for rounded in [true, false] {
                let cfg = VisualStyleCfg {
                    style,
                    rounded,
                    ..Default::default()
                };
                let out = render_ass(&evil, &cfg, "#FFFFFF");
                assert_valid(&out, &cfg);
            }
        }
        // A whole spectrum of NaN must still render, and render as silence.
        let cfg = cfg_for(VisualStyle::Bars);
        let all_nan = [f32::NAN; 12];
        assert_valid(&render_ass(&all_nan, &cfg, "#FFFFFF"), &cfg);
        assert_eq!(
            split(&render_ass(&all_nan, &cfg, "#FFFFFF")).1,
            split(&render_ass(&[0.0; 12], &cfg, "#FFFFFF")).1,
            "NaN must read as silence, not as full scale"
        );
    }

    #[test]
    fn hostile_config_cannot_produce_malformed_ass() {
        // config.toml is hand-editable and the GUI is not the only writer.
        let cases = [
            VisualStyleCfg {
                width_pct: f32::NAN,
                ..Default::default()
            },
            VisualStyleCfg {
                width_pct: -1e9,
                ..Default::default()
            },
            VisualStyleCfg {
                width_pct: 1e9,
                ..Default::default()
            },
            VisualStyleCfg {
                height_px: 0,
                ..Default::default()
            },
            VisualStyleCfg {
                height_px: u32::MAX,
                ..Default::default()
            },
            VisualStyleCfg {
                margin_px: u32::MAX,
                ..Default::default()
            },
            VisualStyleCfg {
                gap_px: u32::MAX,
                ..Default::default()
            },
            VisualStyleCfg {
                opacity: 0,
                ..Default::default()
            },
            VisualStyleCfg {
                colour: "not a colour".into(),
                accent_follow: false,
                ..Default::default()
            },
        ];
        for base in cases {
            for style in VisualStyle::ALL {
                let cfg = VisualStyleCfg {
                    style,
                    ..base.clone()
                };
                assert_valid(&render_ass(&spectrum(), &cfg, "#3584E4"), &cfg);
            }
        }
    }

    #[test]
    fn a_swallowing_gap_still_leaves_bars() {
        // `gap_px` larger than a cell would otherwise produce zero-width bars,
        // i.e. a widget that is present, costs a push, and shows nothing.
        let cfg = VisualStyleCfg {
            gap_px: 4000,
            ..Default::default()
        };
        let out = render_ass(&spectrum(), &cfg, "#FFFFFF");
        assert_valid(&out, &cfg);
        assert_eq!(contours(&out), 2 + spectrum().len());
    }

    #[test]
    fn anchor_and_margin_move_the_overlay_but_not_the_shape() {
        // The design in one assertion: the drawing is anchor-independent
        // because its bounding box is pinned, so placement is entirely `\an` +
        // `\pos`. If the shape ever starts varying with the anchor, the pin has
        // stopped working.
        let bottom = VisualStyleCfg::default();
        let top = VisualStyleCfg {
            anchor: Anchor::TopLeft,
            ..Default::default()
        };
        let a = render_ass(&spectrum(), &bottom, "#FFFFFF");
        let b = render_ass(&spectrum(), &top, "#FFFFFF");
        assert_eq!(
            split(&a).1,
            split(&b).1,
            "geometry must not follow the anchor"
        );
        assert!(split(&a).0.contains("\\an2\\pos(960,1032)"), "{a}");
        assert!(split(&b).0.contains("\\an7\\pos(48,48)"), "{b}");

        // The margin moves the anchored axis and leaves a centred one alone.
        let wide = VisualStyleCfg {
            margin_px: 200,
            ..Default::default()
        };
        assert!(
            split(&render_ass(&spectrum(), &wide, "#FFFFFF"))
                .0
                .contains("\\pos(960,880)"),
            "margin must move a bottom anchor up, and not sideways"
        );
        // An absurd margin must not flip the sides or wrap the coordinate.
        let absurd = VisualStyleCfg {
            anchor: Anchor::BottomRight,
            margin_px: 99_999,
            ..Default::default()
        };
        assert!(split(&render_ass(&spectrum(), &absurd, "#FFFFFF"))
            .0
            .contains("\\pos(960,540)"));
    }

    #[test]
    fn every_anchor_produces_a_distinct_position() {
        let mut seen = Vec::new();
        for anchor in Anchor::ALL {
            let cfg = VisualStyleCfg {
                anchor,
                ..Default::default()
            };
            let tags = split(&render_ass(&spectrum(), &cfg, "#FFFFFF")).0;
            let pos = tags[tags.find("\\pos(").expect("a position")..].to_string();
            assert!(!seen.contains(&pos), "{anchor:?} duplicates a position");
            seen.push(pos);
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn colours_are_emitted_in_ass_bgr_form() {
        // ASS is little-endian BGR; a straight copy swaps red and blue on every
        // bar, which looks deliberate and is therefore easy to ship.
        let follow = VisualStyleCfg {
            accent_follow: true,
            colour: "#FF0000".into(),
            ..Default::default()
        };
        let out = render_ass(&spectrum(), &follow, "#3584E4");
        assert!(out.contains("\\1c&HE48435&"), "accent not used: {out}");

        let fixed = VisualStyleCfg {
            accent_follow: false,
            ..follow.clone()
        };
        let out = render_ass(&spectrum(), &fixed, "#3584E4");
        assert!(
            out.contains("\\1c&H0000FF&"),
            "configured colour not used: {out}"
        );

        // An unusable colour costs the tint, not the widget — as in lyrics.rs.
        let junk = VisualStyleCfg {
            accent_follow: false,
            colour: "rgb(1,2,3)".into(),
            ..Default::default()
        };
        assert!(render_ass(&spectrum(), &junk, "#3584E4").contains("\\1c&HFFFFFF&"));
    }

    #[test]
    fn opacity_maps_to_inverted_ass_alpha() {
        // `&H00&` is opaque in ASS. Getting this backwards ships as an opacity
        // slider that fades the widget in as you drag it towards zero.
        let solid = VisualStyleCfg {
            opacity: 255,
            ..Default::default()
        };
        assert!(render_ass(&spectrum(), &solid, "#FFFFFF").contains("\\alpha&H00&"));
        let gone = VisualStyleCfg {
            opacity: 0,
            ..Default::default()
        };
        assert!(render_ass(&spectrum(), &gone, "#FFFFFF").contains("\\alpha&HFF&"));
        assert_eq!(ass_alpha(220), "&H23&");
    }

    #[test]
    fn the_outline_is_switched_off_explicitly() {
        // The OSD style is mpv's. An inherited border would trace every bar
        // edge and, worse, give the zero-area bounding-box pins something to
        // stroke — two visible dots in the corners of the widget.
        let out = render_ass(&spectrum(), &VisualStyleCfg::default(), "#FFFFFF");
        assert!(out.contains("\\bord0\\shad0"), "{out}");
        assert!(out.contains("\\fscx100\\fscy100\\frz0"), "{out}");
    }

    #[test]
    fn the_bounding_box_pin_is_present_and_content_independent() {
        // Two zero-area contours at the box corners, identical for a loud frame
        // and a silent one. Without them the widget drifts as the music plays.
        let cfg = VisualStyleCfg::default();
        for bands in [vec![0.0; 8], vec![1.0; 8], spectrum()] {
            let body = split(&render_ass(&bands, &cfg, "#FFFFFF")).1;
            assert!(
                body.starts_with("m 0 0 l 0 0 l 0 0 m 1152 120 l 1152 120 l 1152 120 "),
                "pin missing or wrong: {body}"
            );
        }
    }

    #[test]
    fn bar_counts_follow_band_counts_without_panicking() {
        // One band is a real case (a VU meter), 256 is a real case (a fine FFT
        // with no binning), and both must survive every style.
        for n in [1usize, 2, 3, 4, 7, 16, 64, 128, 192, 256, 1000] {
            let bands: Vec<f32> = (0..n).map(|i| (i % 11) as f32 / 10.0).collect();
            for style in VisualStyle::ALL {
                let cfg = cfg_for(style);
                let out = render_ass(&bands, &cfg, "#FFFFFF");
                assert_valid(&out, &cfg);
                let drawn = n.min(MAX_BANDS);
                let expect = match style {
                    // Two pins, plus one contour per band…
                    VisualStyle::Bars | VisualStyle::Mirror | VisualStyle::Dots => 2 + drawn,
                    // …plus the two hub circles for Ring…
                    VisualStyle::Ring => 4 + drawn,
                    // …and Wave is a single silhouette however many bands it
                    // was built from.
                    VisualStyle::Wave => 3,
                };
                assert_eq!(contours(&out), expect, "{style:?} with {n} bands: {out}");
            }
        }
    }

    #[test]
    fn too_many_bands_are_folded_by_peak_not_truncated() {
        // Truncating would silently delete the top of the spectrum; averaging
        // would flatten the peaks that are the whole picture.
        let mut bands = vec![0.0f32; 2000];
        bands[1999] = 1.0; // A peak in the very last bin.
        let folded = sanitise(&bands);
        assert_eq!(folded.len(), MAX_BANDS);
        assert_eq!(
            folded[MAX_BANDS - 1],
            1.0,
            "the top of the spectrum was lost"
        );
        assert!(folded[..MAX_BANDS - 1].iter().all(|v| *v == 0.0));
        // And the fold covers the input exactly once, with no gap at the seams.
        let ramp: Vec<f32> = (0..1000).map(|i| i as f32 / 999.0).collect();
        let folded = sanitise(&ramp);
        assert_eq!(folded.len(), MAX_BANDS);
        assert!(
            folded.windows(2).all(|w| w[0] < w[1]),
            "fold lost monotonicity"
        );
        assert_eq!(folded[MAX_BANDS - 1], 1.0);
        // Under the cap nothing is folded at all.
        assert_eq!(sanitise(&[0.25, 0.5]), vec![0.25, 0.5]);
        assert_eq!(sanitise(&[-1.0, 2.0, f32::NAN]), vec![0.0, 1.0, 0.0]);
    }

    #[test]
    fn rounded_selects_curves_and_square_selects_lines() {
        // The flag has to reach the geometry, not just the config file.
        for style in VisualStyle::ALL {
            let round = VisualStyleCfg {
                style,
                ..Default::default()
            };
            let square = VisualStyleCfg {
                style,
                rounded: false,
                ..Default::default()
            };
            let curved = split(&render_ass(&spectrum(), &round, "#FFFFFF")).1;
            let angular = split(&render_ass(&spectrum(), &square, "#FFFFFF")).1;
            assert!(curved.contains(" b "), "{style:?} rounded has no curve");
            assert_ne!(curved, angular, "{style:?} ignores `rounded`");
            // Only Ring keeps curves when squared off — its hub is a circle
            // whatever the bars do.
            if style == VisualStyle::Ring {
                assert!(angular.contains(" b "), "the hub must stay round");
            } else {
                assert!(
                    !angular.contains(" b "),
                    "{style:?} square should be all straight lines: {angular}"
                );
            }
        }
    }

    #[test]
    fn mirror_is_symmetric_about_the_centre_line() {
        // The defining property of the style. An off-by-one here produces a
        // "mirror" that sits a few units low, which is very hard to see.
        let cfg = VisualStyleCfg {
            style: VisualStyle::Mirror,
            rounded: false,
            height_px: 200,
            ..Default::default()
        };
        let out = render_ass(&[1.0, 0.5, 0.25], &cfg, "#FFFFFF");
        let ys: Vec<i32> = shape_points(&out).iter().map(|(_, y)| *y).collect();
        // Every vertex must have a partner reflected about the centre line.
        for y in &ys {
            assert!(
                ys.iter().any(|o| *o == 200 - *y),
                "y={y} has no reflection in {out}"
            );
        }
        // And the reflection must be non-trivial: something above and below.
        assert!(ys.iter().any(|y| *y < 100) && ys.iter().any(|y| *y > 100));
    }

    #[test]
    fn wave_is_one_continuous_closed_silhouette() {
        // A wave broken into per-band contours would be bars with extra steps.
        let cfg = cfg_for(VisualStyle::Wave);
        let out = render_ass(&spectrum(), &cfg, "#FFFFFF");
        assert_valid(&out, &cfg);
        assert_eq!(contours(&out), 3, "two pins and exactly one silhouette");
        let body = split(&out).1;
        // It starts and ends on the floor of a 120-unit box.
        assert!(body.contains("m 0 120 l 0 "), "{body}");
        assert!(body.ends_with("l 1152 120"), "{body}");
        // A single band is a degenerate curve; it must still be a slab.
        let one = render_ass(&[0.5], &cfg, "#FFFFFF");
        assert_valid(&one, &cfg);
        assert_eq!(contours(&one), 3);
    }

    #[test]
    fn ring_stays_inside_its_circle_and_keeps_its_hub() {
        // Polar geometry is the easiest thing here to get subtly wrong, so
        // check it against the definition rather than against a golden string:
        // every vertex must lie within the largest circle that fits the box.
        let cfg = VisualStyleCfg {
            style: VisualStyle::Ring,
            width_pct: 20.0,
            height_px: 300,
            ..Default::default()
        };
        let out = render_ass(&spectrum(), &cfg, "#FFFFFF");
        assert_valid(&out, &cfg);
        let (w, h) = box_size(&cfg);
        // Squared off to the smaller side, so there is no invisible padding for
        // a corner anchor to place the ring against.
        assert_eq!((w, h), (300.0, 300.0));
        let (cx, cy) = (w / 2.0, h / 2.0);
        let outer = w.min(h) / 2.0;
        let (on, off) = curve_points(&out);
        let radius = |(x, y): &(i32, i32)| {
            let (dx, dy) = (*x as f32 - cx, *y as f32 - cy);
            (dx * dx + dy * dy).sqrt()
        };
        for p in &on {
            // A unit of slack for the rounding in `du`.
            let r = radius(p);
            assert!(r <= outer + 1.0, "vertex {p:?} at r={r} escapes {outer}");
        }
        for p in &off {
            // Control points ride `k·r` off the arc by construction; they are
            // still bounded, and still inside the pinned box, which is what the
            // placement actually depends on.
            let r = radius(p);
            assert!(
                r <= outer * 1.02 + 1.0,
                "control point {p:?} at r={r} is further out than the cubic \
                 construction allows"
            );
        }
        assert!(!on.is_empty() && !off.is_empty());
        // Hub present as two circles of four cubics each, bars on top.
        assert_eq!(contours(&out), 4 + spectrum().len());
        // Band zero points at twelve o'clock. Checked with a spectrum that has
        // exactly one loud band, so the furthest vertex from the centre can
        // only belong to it — an inverted sine or a swapped axis would put that
        // vertex somewhere else on the dial.
        let mut solo = vec![0.0f32; 16];
        solo[0] = 1.0;
        let (on, _) = curve_points(&render_ass(&solo, &cfg, "#FFFFFF"));
        let peak = on
            .iter()
            .max_by(|a, b| radius(a).total_cmp(&radius(b)))
            .copied()
            .expect("a loud band");
        assert!(
            (peak.0 as f32 - cx).abs() < outer * 0.25 && (peak.1 as f32) < cy - outer * 0.5,
            "band zero is not at twelve o'clock: peak at {peak:?}, centre ({cx},{cy})"
        );
    }

    #[test]
    fn dots_stay_fully_inside_the_box_at_both_extremes() {
        // The dot's *centre* travels, not its edge, so a naive implementation
        // clips half a dot off the top at full scale.
        let cfg = VisualStyleCfg {
            style: VisualStyle::Dots,
            rounded: false,
            height_px: 100,
            ..Default::default()
        };
        let out = render_ass(&[0.0, 1.0], &cfg, "#FFFFFF");
        assert_valid(&out, &cfg);
        let ys: Vec<i32> = shape_points(&out).iter().map(|(_, y)| *y).collect();
        assert!(
            ys.contains(&0),
            "the loud dot should touch the ceiling: {out}"
        );
        assert!(
            ys.contains(&100),
            "the quiet dot should touch the floor: {out}"
        );
        // …and nothing may be clipped by the box on the way there.
        assert!(ys.iter().all(|y| (0..=100).contains(y)));
    }

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
    fn defaults_are_the_documented_ones() {
        // The config stores these numbers too; if the two drift, the GUI shows
        // one thing and the overlay renders another.
        let c = VisualStyleCfg::default();
        assert_eq!(c.style, VisualStyle::Bars);
        assert_eq!(c.anchor, Anchor::BottomCenter);
        assert_eq!(c.width_pct, 60.0);
        assert_eq!(c.height_px, 120);
        assert_eq!(c.margin_px, 48); // matches LyricStyle, so the two line up
        assert_eq!(c.colour, "#FFFFFF");
        assert!(c.accent_follow);
        assert_eq!(c.opacity, 220);
        assert_eq!(c.gap_px, 4);
        assert!(c.rounded);
        assert_eq!(VisualStyle::ALL.len(), 5);
        // The default box is 60% of 1920 by 120 units.
        assert_eq!(box_size(&c), (1152.0, 120.0));
    }

    /// The exact bytes each style rendered *before* gradients existed, from
    /// `[0.25, 0.5, 1.0]` and the Adwaita blue accent.
    ///
    /// Structural tests pass happily on a picture that has moved by a unit, and
    /// a new code path through old drawing code is exactly how a picture moves
    /// by a unit. This table is the pin: with [`Gradient::None`] — the default,
    /// and what every existing config has — the output must be these strings,
    /// byte for byte.
    const FLAT_GOLDEN: [(VisualStyle, &str); 5] = [
        (VisualStyle::Bars, "{\\an2\\pos(960,1032)\\bord0\\shad0\\fscx100\\fscy100\\frz0\\1c&HE48435&\\alpha&H23&\\p1}m 0 0 l 0 0 l 0 0 m 1152 120 l 1152 120 l 1152 120 m 2 120 l 2 120 b 2 103 15 90 32 90 l 352 90 b 369 90 382 103 382 120 l 382 120 m 386 120 l 386 120 b 386 87 413 60 446 60 l 706 60 b 739 60 766 87 766 120 l 766 120 m 770 120 l 770 120 b 770 54 824 0 890 0 l 1030 0 b 1096 0 1150 54 1150 120 l 1150 120{\\p0}"),

        (VisualStyle::Mirror, "{\\an2\\pos(960,1032)\\bord0\\shad0\\fscx100\\fscy100\\frz0\\1c&HE48435&\\alpha&H23&\\p1}m 0 0 l 0 0 l 0 0 m 1152 120 l 1152 120 l 1152 120 m 2 60 b 2 68 9 75 17 75 l 367 75 b 375 75 382 68 382 60 l 382 60 b 382 52 375 45 367 45 l 17 45 b 9 45 2 52 2 60 m 386 60 b 386 77 399 90 416 90 l 736 90 b 753 90 766 77 766 60 l 766 60 b 766 43 753 30 736 30 l 416 30 b 399 30 386 43 386 60 m 770 60 b 770 93 797 120 830 120 l 1090 120 b 1123 120 1150 93 1150 60 l 1150 60 b 1150 27 1123 0 1090 0 l 830 0 b 797 0 770 27 770 60{\\p0}"),

        (VisualStyle::Wave, "{\\an2\\pos(960,1032)\\bord0\\shad0\\fscx100\\fscy100\\frz0\\1c&HE48435&\\alpha&H23&\\p1}m 0 0 l 0 0 l 0 0 m 1152 120 l 1152 120 l 1152 120 m 0 120 l 0 90 b 192 90 384 60 576 60 b 768 60 960 0 1152 0 l 1152 120{\\p0}"),

        (VisualStyle::Dots, "{\\an2\\pos(960,1032)\\bord0\\shad0\\fscx100\\fscy100\\frz0\\1c&HE48435&\\alpha&H23&\\p1}m 0 0 l 0 0 l 0 0 m 1152 120 l 1152 120 l 1152 120 m 223 75 b 223 92 209 105 192 105 b 175 105 161 92 161 75 b 161 58 175 44 192 44 b 209 44 223 58 223 75 m 617 60 b 617 82 598 101 576 101 b 554 101 536 82 536 60 b 536 38 554 20 576 20 b 598 20 617 38 617 60 m 1020 60 b 1020 93 993 120 960 120 b 927 120 900 93 900 60 b 900 27 927 0 960 0 b 993 0 1020 27 1020 60{\\p0}"),

        (VisualStyle::Ring, "{\\an2\\pos(960,1032)\\bord0\\shad0\\fscx100\\fscy100\\frz0\\1c&HE48435&\\alpha&H23&\\p1}m 0 0 l 0 0 l 0 0 m 120 120 l 120 120 l 120 120 m 85 60 b 85 74 74 85 60 85 b 46 85 35 74 35 60 b 35 46 46 35 60 35 b 74 35 85 46 85 60 m 82 60 b 82 48 72 38 60 38 b 48 38 38 48 38 60 b 38 72 48 82 60 82 b 72 82 82 72 82 60 m 39 46 l 32 41 b 46 21 74 21 88 41 l 81 46 m 83 49 l 98 42 b 111 69 93 100 63 102 l 62 85 m 58 85 l 55 120 b 13 116 0 72 6 34 l 37 49{\\p0}"),
    ];

    #[test]
    fn a_flat_spectrum_is_byte_for_byte_what_it_always_was() {
        for (style, want) in FLAT_GOLDEN {
            let got = render_ass(&[0.25, 0.5, 1.0], &cfg_for(style), "#3584E4");
            assert_eq!(got, want, "{style:?} no longer draws what it drew");
            assert!(!got.contains('\n'), "{style:?}: flat is one event");
        }
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

    #[test]
    fn du_never_yields_anything_but_an_integer() {
        // The single guarantee the no-NaN invariant rests on.
        assert_eq!(du(1.4), 1);
        assert_eq!(du(1.5), 2);
        assert_eq!(du(-1.5), -2);
        assert_eq!(du(f32::NAN), 0);
        assert_eq!(du(f32::INFINITY), i32::MAX);
        assert_eq!(du(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(du(1e30), i32::MAX);
    }

    #[test]
    fn end_to_end_matches_what_the_daemon_would_push() {
        // The daemon's actual sequence: check for silence, skip if silent,
        // otherwise render one event and push it with res_x/res_y.
        let cfg = VisualStyleCfg::default();
        let quiet = [0.001, 0.0, 0.002, 0.0];
        assert!(is_silent(&quiet, 0.01), "this frame should cost nothing");

        let loud = spectrum();
        assert!(!is_silent(&loud, 0.01));
        let out = render_ass(&loud, &cfg, "#3584E4");
        assert_valid(&out, &cfg);
        assert!(out.starts_with("{\\an2\\pos(960,1032)"), "{out}");
        assert!(out.contains("\\1c&HE48435&\\alpha&H23&\\p1}"), "{out}");
        assert!(out.ends_with("{\\p0}"), "{out}");
        // And clearing is a distinct, empty payload.
        assert_eq!(render_ass(&[], &cfg, "#3584E4"), "");
    }

    // ── Gradients ─────────────────────────────────────────────────────────

    /// A gradient config with two colours far enough apart to be unmistakable,
    /// and the accent switched off so the test controls both ends.
    fn grad_cfg(style: VisualStyle, gradient: Gradient) -> VisualStyleCfg {
        VisualStyleCfg {
            style,
            gradient,
            accent_follow: false,
            colour: "#FF0000".into(),
            colour_end: "#00FF00".into(),
            ..Default::default()
        }
    }

    #[test]
    fn every_style_stays_well_formed_under_every_gradient_mode() {
        for style in VisualStyle::ALL {
            for gradient in Gradient::ALL {
                for rounded in [true, false] {
                    let cfg = VisualStyleCfg {
                        rounded,
                        ..grad_cfg(style, gradient)
                    };
                    let out = render_ass(&spectrum(), &cfg, "#3584E4");
                    assert_valid_payload(&out, &cfg);
                }
            }
        }
    }

    #[test]
    fn a_gradient_paints_the_two_ends_in_the_colours_that_were_asked_for() {
        // The whole feature in one assertion: the first bar is the colour the
        // user picked for the near end, the last bar the one for the far end.
        let cfg = grad_cfg(VisualStyle::Bars, Gradient::Linear);
        let evs = events(&render_ass(&spectrum(), &cfg, "#3584E4"));
        assert!(evs.len() > 1, "a gradient needs more than one event");
        assert_eq!(event_rgb(&evs[0]), (255, 0, 0), "near end is not the red");
        assert_eq!(
            event_rgb(evs.last().expect("an event")),
            (0, 255, 0),
            "far end is not the green"
        );
        // And it is really in the payload in ASS's BGR order, not RGB.
        assert!(evs[0].contains("\\1c&H0000FF&"), "{}", evs[0]);
    }

    #[test]
    fn the_ramp_between_the_ends_is_monotonic() {
        // A ramp that wanders is worse than no ramp: it reads as a rendering
        // fault rather than as a colour choice. Red (hue 0) to green (hue 120)
        // is the short way round, so the hue must climb, step by step, with no
        // two neighbours out of order.
        let cfg = grad_cfg(VisualStyle::Bars, Gradient::Linear);
        let hues: Vec<f32> = events(&render_ass(&[0.5; 32], &cfg, "#3584E4"))
            .iter()
            .map(|e| rgb_to_hsl(event_rgb(e)).h)
            .collect();
        assert_eq!(hues.len(), 32, "one colour per band at this width");
        assert!(
            hues.windows(2).all(|w| w[1] >= w[0]),
            "the ramp doubles back: {hues:?}"
        );
        assert!(
            hues.windows(2).any(|w| w[1] > w[0]),
            "the ramp never moves: {hues:?}"
        );
        assert!(hues[0] < 1.0 && hues[31] > 119.0, "{hues:?}");
    }

    #[test]
    fn a_gradient_does_not_move_a_single_bar() {
        // The events are a re-cut of one picture, not a different picture:
        // every vertex of the flat drawing must appear, in order, across the
        // gradient's events. If this ever fails the bars have been re-laid out
        // per event, which shows as a spectrum that jitters when you tint it.
        for style in [
            VisualStyle::Bars,
            VisualStyle::Mirror,
            VisualStyle::Dots,
            VisualStyle::Ring,
        ] {
            let flat = grad_cfg(style, Gradient::None);
            let ramped = grad_cfg(style, Gradient::Linear);
            let mut one = shape_points(&render_ass(&spectrum(), &flat, "#3584E4"));
            let mut many: Vec<Pt> = events(&render_ass(&spectrum(), &ramped, "#3584E4"))
                .iter()
                .flat_map(|e| shape_points(e))
                .collect();
            if style == VisualStyle::Ring {
                // The one style whose events are not in drawing order: its hub
                // moves to the run that owns the middle band, so that it can be
                // the middle colour. Same vertices, different sequence — and
                // the order does not matter because nothing here overlaps.
                one.sort_unstable();
                many.sort_unstable();
            }
            assert_eq!(one, many, "{style:?} draws a different picture tinted");
        }
    }

    #[test]
    fn the_hub_of_a_ring_is_drawn_once_in_the_middle_colour() {
        // Repeated per event it would be overpainted in every colour of the
        // ramp, and at partial opacity each pass would darken it further. Drawn
        // in the *first* event it would be the colour of the first bar, which
        // reads as a mistake rather than as a choice.
        let cfg = grad_cfg(VisualStyle::Ring, Gradient::Spectrum);
        let evs = events(&render_ass(&spectrum(), &cfg, "#3584E4"));
        let shapes: Vec<usize> = evs.iter().map(|e| contours(e) - 2).collect();
        assert_eq!(
            shapes.iter().sum::<usize>(),
            spectrum().len() + 2,
            "the hub annulus is two contours, drawn exactly once"
        );
        let hub_at = shapes
            .iter()
            .position(|c| *c > 1)
            .expect("some event owns the hub");
        assert_eq!(hub_at, evs.len() / 2, "the hub is not the middle colour");
        let hue = rgb_to_hsl(event_rgb(&evs[hub_at])).h;
        assert!(
            (SPECTRUM_HUE_SPAN / 2.0 - hue).abs() < 20.0,
            "hub hue {hue} is not near the middle of the sweep"
        );
    }

    #[test]
    fn wave_cannot_carry_a_gradient_and_does_not_pretend_to() {
        // One continuous contour: there is nothing to give a second colour to
        // without cutting the silhouette up, which would both stop it being a
        // silhouette and leave a seam at every cut. So it stays flat, in the
        // colour it would have had anyway.
        for gradient in Gradient::ALL {
            let cfg = grad_cfg(VisualStyle::Wave, gradient);
            let out = render_ass(&spectrum(), &cfg, "#3584E4");
            assert!(!out.contains('\n'), "wave must stay one event: {out}");
            assert_eq!(event_rgb(&out), (255, 0, 0), "wave lost its own colour");
            assert_eq!(contours(&out), 3, "two pins and one silhouette");
        }
    }

    #[test]
    fn spectrum_sweeps_the_long_way_round_and_ignores_both_colours() {
        // The point of the mode: it is the classic look and it needs no colour
        // picking, so neither colour key nor the accent may reach it.
        let cfg = grad_cfg(VisualStyle::Bars, Gradient::Spectrum);
        let a = render_ass(&[0.5; 24], &cfg, "#3584E4");
        let b = render_ass(
            &[0.5; 24],
            &VisualStyleCfg {
                colour: "#123456".into(),
                colour_end: "#ABCDEF".into(),
                accent_follow: true,
                ..cfg.clone()
            },
            "#FF00FF",
        );
        assert_eq!(a, b, "Spectrum must not depend on any colour setting");

        let hues: Vec<f32> = events(&a)
            .iter()
            .map(|e| rgb_to_hsl(event_rgb(e)).h)
            .collect();
        // Red at one end, violet at the other, and green in the middle — which
        // is what "the long way round" means. The short way from red to violet
        // is 60° through pink and would skip the entire spectrum.
        assert!(hues[0] < 2.0, "starts at red: {hues:?}");
        assert!(hues[23] > 290.0, "ends at violet: {hues:?}");
        assert!(
            hues.iter().any(|h| (100.0..140.0).contains(h)),
            "the sweep never passes through green: {hues:?}"
        );
        assert!(
            hues.windows(2).all(|w| w[1] > w[0]),
            "the sweep doubles back: {hues:?}"
        );
    }

    #[test]
    fn a_ramp_between_two_equal_colours_costs_exactly_one_event() {
        // Every event is an override block and a copy of the pin. Paying for
        // 32 of them to draw one colour is the kind of waste that only shows up
        // as a warm laptop.
        let cfg = VisualStyleCfg {
            colour_end: "#ff0000".into(),
            ..grad_cfg(VisualStyle::Bars, Gradient::Linear)
        };
        let out = render_ass(&spectrum(), &cfg, "#3584E4");
        assert!(!out.contains('\n'), "an equal-ended ramp is flat: {out}");
        // Including the default configuration, where both ends are white.
        let default_ends = VisualStyleCfg {
            gradient: Gradient::Linear,
            accent_follow: false,
            ..Default::default()
        };
        assert!(!render_ass(&spectrum(), &default_ends, "#3584E4").contains('\n'));
    }

    #[test]
    fn the_event_count_is_capped_and_the_payload_stays_affordable() {
        // This crosses an IPC socket 15-30 times a second, so the cost of the
        // gradient is a real number and not an aesthetic one.
        for n in [1usize, 2, 8, 32, 64, 192, 1000] {
            let bands: Vec<f32> = (0..n).map(|i| (i % 11) as f32 / 10.0).collect();
            let cfg = grad_cfg(VisualStyle::Bars, Gradient::Spectrum);
            let out = render_ass(&bands, &cfg, "#3584E4");
            let drawn = n.min(MAX_BANDS);
            assert_eq!(
                events(&out).len(),
                drawn.min(MAX_GRADIENT_EVENTS),
                "{n} bands"
            );
            let flat = render_ass(
                &bands,
                &grad_cfg(VisualStyle::Bars, Gradient::None),
                "#3584E4",
            );
            assert!(
                out.len() < flat.len() * 4,
                "{n} bands: gradient {} bytes against flat {}",
                out.len(),
                flat.len()
            );
        }
    }

    #[test]
    fn colour_runs_tile_the_spectrum_exactly_once() {
        // A band drawn twice is a brighter bar at partial opacity; a band
        // dropped is a hole. Neither is a thing to discover by looking.
        for n in [1usize, 2, 5, 31, 32, 47, 48, 49, 100, 192] {
            let runs: Vec<Range<usize>> = colour_runs(n).map(|(r, _)| r).collect();
            assert_eq!(runs.len(), n.min(MAX_GRADIENT_EVENTS), "{n} bands");
            let covered: Vec<usize> = runs.iter().flat_map(|r| r.clone()).collect();
            assert_eq!(covered, (0..n).collect::<Vec<_>>(), "{n} bands");
            let ts: Vec<f32> = colour_runs(n).map(|(_, t)| t).collect();
            assert!(ts.iter().all(|t| (0.0..=1.0).contains(t)), "{ts:?}");
            assert!(ts.windows(2).all(|w| w[1] > w[0]), "{ts:?}");
            assert_eq!(*ts.first().expect("a run"), 0.0);
            assert_eq!(*ts.last().expect("a run"), if n > 1 { 1.0 } else { 0.0 });
        }
    }

    #[test]
    fn the_ramp_is_interpolated_in_hsl_so_it_never_goes_through_grey() {
        // The bug this choice exists to avoid: a straight line between two
        // sRGB triples runs through the middle of the colour cube, and for two
        // colours on opposite sides of it the middle is grey. Pink to cyan is
        // the pair people actually pick, so it is the pair that gets tested.
        let ramp = Ramp::linear(parse_rgb("#FF00FF"), parse_rgb("#00FFFF"));
        for step in 0..=10 {
            let t = step as f32 / 10.0;
            let c = rgb_to_hsl(parse_rgb(&ramp.at(t)));
            assert!(c.s > 0.9, "the ramp desaturates to {} at t={t}", c.s);
        }
        // For comparison, the naive sRGB midpoint of those two is (128,128,255)
        // — a washed-out lavender with a quarter of the saturation.
        let naive = rgb_to_hsl((128, 128, 255));
        assert!(naive.s < 1.0);
        // Interpolating into white keeps the hue it started on rather than
        // sliding through red on the way, because white has no hue of its own.
        let pale = Ramp::linear(parse_rgb("#0000FF"), parse_rgb("#FFFFFF"));
        for step in 0..=10 {
            let c = rgb_to_hsl(parse_rgb(&pale.at(step as f32 / 10.0)));
            assert!(
                c.s <= 0.0 || (239.0..=241.0).contains(&c.h),
                "hue drifted to {}",
                c.h
            );
        }
    }

    #[test]
    fn accent_follow_still_wins_and_becomes_the_near_end() {
        // `accent_follow` means the same thing it always did; a gradient just
        // starts from whichever colour it picked.
        let cfg = VisualStyleCfg {
            accent_follow: true,
            ..grad_cfg(VisualStyle::Bars, Gradient::Linear)
        };
        let evs = events(&render_ass(&spectrum(), &cfg, "#3584E4"));
        assert_eq!(event_rgb(&evs[0]), (0x35, 0x84, 0xE4), "not the accent");
        assert_eq!(event_rgb(evs.last().expect("an event")), (0, 255, 0));
    }

    #[test]
    fn hostile_colours_cannot_produce_a_malformed_gradient() {
        // config.toml is hand-editable, and a gradient multiplies every colour
        // mistake by the number of events it is drawn in.
        let cases = [
            "",
            "#",
            "#12345",
            "rgb(1,2,3)",
            "#GGGGGG",
            "  #abc  ",
            "fff",
        ];
        for junk in cases {
            for gradient in Gradient::ALL {
                for style in VisualStyle::ALL {
                    let cfg = VisualStyleCfg {
                        colour: junk.into(),
                        colour_end: junk.into(),
                        accent_follow: false,
                        gradient,
                        style,
                        ..Default::default()
                    };
                    let out = render_ass(&spectrum(), &cfg, junk);
                    assert_valid_payload(&out, &cfg);
                }
            }
        }
        // A ramp from an unreadable colour degrades to white at that end, the
        // same fallback the flat path has always used.
        let cfg = VisualStyleCfg {
            colour: "not a colour".into(),
            colour_end: "#000000".into(),
            accent_follow: false,
            ..grad_cfg(VisualStyle::Bars, Gradient::Linear)
        };
        let evs = events(&render_ass(&spectrum(), &cfg, "#3584E4"));
        assert_eq!(event_rgb(&evs[0]), (255, 255, 255));
        assert_eq!(event_rgb(evs.last().expect("an event")), (0, 0, 0));
    }

    #[test]
    fn no_float_ever_reaches_a_gradient_payload() {
        // The module's founding guarantee, restated for the code path that did
        // not exist when it was written: colours are interpolated in floats and
        // must leave as integers, or one silent capture takes the overlay down.
        let evil = [f32::NAN, f32::INFINITY, -1e30, 0.5, 1e30, f32::NEG_INFINITY];
        for style in VisualStyle::ALL {
            for gradient in Gradient::ALL {
                let cfg = VisualStyleCfg {
                    width_pct: f32::NAN,
                    ..grad_cfg(style, gradient)
                };
                let out = render_ass(&evil, &cfg, "#3584E4");
                let low = out.to_ascii_lowercase();
                for bad in ["nan", "inf", ".", "e-", "e+"] {
                    assert!(!low.contains(bad), "float text {bad:?} in {out}");
                }
                assert_valid_payload(&out, &cfg);
            }
        }
    }

    #[test]
    fn gradient_spellings_are_stable_in_both_directions() {
        // The renderer's own serde form. `config::GradientMode` is the
        // config-file spelling and has its own test; these two must agree, and
        // the daemon's mapping is what holds them together.
        for (text, mode) in [
            ("none", Gradient::None),
            ("linear", Gradient::Linear),
            ("spectrum", Gradient::Spectrum),
        ] {
            let cfg = VisualStyleCfg {
                gradient: mode,
                ..Default::default()
            };
            let toml = toml::to_string(&cfg).expect("serialise");
            assert!(
                toml.contains(&format!("gradient = \"{text}\"")),
                "{mode:?} did not serialise as {text:?}: {toml}"
            );
            let back: VisualStyleCfg = toml::from_str(&toml).expect("deserialise");
            assert_eq!(back.gradient, mode);
            assert_eq!(back, cfg);
        }
        assert_eq!(Gradient::default(), Gradient::None);
        assert_eq!(Gradient::ALL.len(), 3);
        // Defaults, pinned beside the rest of them.
        let c = VisualStyleCfg::default();
        assert_eq!(c.gradient, Gradient::None);
        assert_eq!(c.colour_end, "#FFFFFF");
    }
}

#[cfg(test)]
mod scratch_dump {
    use super::*;

    /// Write one payload per style and mode to `$CARD_DUMP`, for rasterising
    /// through libass. A colour ramp is the one thing here that no assertion
    /// can approve: the tests can prove the ends are right and the middle is
    /// monotonic, and the picture can still be ugly.
    #[test]
    #[ignore]
    fn dump() {
        let dir = std::env::var("CARD_DUMP").unwrap_or_default();
        if dir.is_empty() {
            return;
        }
        // A plausible spectrum rather than a ramp: a flat test signal hides
        // exactly the fault a gradient introduces, which is a colour that
        // tracks the bar's height instead of its position.
        let bands: Vec<f32> = (0..32)
            .map(|i| {
                let t = i as f32 / 31.0;
                (1.0 - t * 0.75) * (0.55 + 0.45 * (t * 19.0).sin())
            })
            .collect();
        for style in VisualStyle::ALL {
            for (tag, gradient, a, b) in [
                ("flat", Gradient::None, "#FF3B6B", "#FF3B6B"),
                ("linear", Gradient::Linear, "#FF3B6B", "#22D3EE"),
                ("spectrum", Gradient::Spectrum, "#FFFFFF", "#FFFFFF"),
            ] {
                let cfg = VisualStyleCfg {
                    style,
                    gradient,
                    accent_follow: false,
                    colour: a.into(),
                    colour_end: b.into(),
                    anchor: Anchor::MidCenter,
                    width_pct: 50.0,
                    height_px: 220,
                    opacity: 255,
                    ..Default::default()
                };
                let name = format!("vis_{style:?}_{tag}").to_ascii_lowercase();
                std::fs::write(
                    format!("{dir}/{name}.txt"),
                    render_ass(&bands, &cfg, "#3584E4"),
                )
                .unwrap();
            }
        }
    }
}

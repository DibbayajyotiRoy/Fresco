//! The components every card is assembled from: card, scrim, well, progress
//! bar, arc gauge, bar array, chip and badge.
//!
//! `docs/widget-design-spec.md` §7 and §8 are the authority.
//!
//! # Why these are functions and not a widget tree
//!
//! Nothing here retains state, allocates per frame or owns a surface. Each one
//! takes a rectangle and draws into a [`Canvas`] the caller already owns, which
//! is what keeps the `reset()` → draw → `write_bgra()` loop allocation-free.
//! A retained tree would need per-node storage and a diff, and the four widgets
//! Fresco has do not have enough structure to pay for either.
//!
//! # The three surfaces, and which one a thing is allowed to sit on
//!
//! | Surface | Drawn by | What may sit on it |
//! |---|---|---|
//! | **card** | [`card`] | large primary ink, and nothing else in dark mode |
//! | **scrim** | [`text_scrim`] | every text block; **mandatory** in dark mode |
//! | **well** | [`well`] | every accent-filled data graphic, always |
//!
//! Those are not stylistic groupings. On the worst dark card the secondary ink
//! scores 4.32:1 and every raw accent scores under 3:1; over the scrim and the
//! well respectively they clear 7.30:1 and 3.84:1. See
//! [`super::theme`] for the full derivation.

use super::canvas::Canvas;
use super::color::Color;
use super::geom::{HAlign, Point, Rect, Size, VAlign};
use super::paint::{Fill, Stop};
use super::text::{FontStack, TextRun};
use super::theme::{radius_nested, Elevation, Theme};
use super::typo::{self, Script, Step};

/// How large a widget is and how much room its shadow needs around it.
///
/// The engine sizes its buffer from [`WidgetSize::buffer`] and anchors the
/// widget by [`WidgetSize::card_rect`] — **not** by the buffer's edge. Anchoring
/// to the buffer makes every widget appear to drift inward as the shadow grows
/// with density (spec §7.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidgetSize {
    /// The card itself, in logical units.
    pub card: Size,
    /// Margin the shadow needs on all four sides.
    pub bleed: f32,
}

impl WidgetSize {
    /// A size with the bleed implied by an elevation level.
    pub fn new(card: Size, elevation: Elevation) -> Self {
        Self {
            card,
            bleed: elevation.bleed(),
        }
    }

    /// The buffer to allocate: the card plus the bleed on all four sides.
    pub fn buffer(&self) -> Size {
        Size::new(
            self.card.w + self.bleed * 2.0,
            self.card.h + self.bleed * 2.0,
        )
    }

    /// Where the card sits inside that buffer. This is the rect `margin_px` is
    /// measured to.
    pub fn card_rect(&self) -> Rect {
        Rect::new(self.bleed, self.bleed, self.card.w, self.card.h)
    }

    /// The card rect for a canvas that may be larger than the measured size —
    /// the card is centred in whatever room it is given, never stretched, so a
    /// mis-sized buffer degrades to a correctly proportioned card with slack
    /// around it rather than to a distorted one.
    pub fn card_in(&self, bounds: Rect) -> Rect {
        bounds
            .inset(self.bleed)
            .align(self.card, HAlign::Center, VAlign::Middle)
    }
}

/// Draw an elevation's shadows under a rounded rect, key first.
///
/// Shadows clip to the canvas like every other primitive, so the rect must sit
/// at least [`Elevation::bleed`] from every edge or the halo is cut.
pub fn elevation(c: &mut Canvas, r: Rect, radius: f32, t: &Theme, e: Elevation) {
    if r.is_empty() {
        return;
    }
    c.drop_shadow(
        r,
        radius,
        e.key.blur,
        e.key.dy,
        t.shadow.with_alpha(e.key.alpha),
    );
    if let Some(s) = e.contact {
        c.drop_shadow(r, radius, s.blur, s.dy, t.shadow.with_alpha(s.alpha));
    }
}

/// The glass card: E2 shadow, the 160° gradient body, the perimeter hairline
/// and the top edge light.
///
/// The four layers are not interchangeable. The gradient is what stops the card
/// reading as a flat sticker; the hairline is a *material* cue at 1.4–1.6:1 and
/// is deliberately too faint to be a boundary; the top highlight is the single
/// strongest "this is glass" signal available without a backdrop blur; and the
/// shadow is the actual boundary, because on a bright wallpaper a light card
/// has no luminance step at its edge at all.
pub fn card(c: &mut Canvas, r: Rect, radius: f32, t: &Theme) {
    if r.is_empty() {
        return;
    }
    let m = t.metrics;
    elevation(c, r, radius, t, t.e2());
    c.rounded_rect(r, radius, &t.card_fill(r));
    c.hairline(r, radius, t.edge, m.hairline);
    c.top_highlight(r, radius, t.edge_highlight, m.hairline);
}

/// Everything a scrim needs to know about the card it is being laid on.
///
/// Bundled rather than passed loose because the five values must agree: a
/// scrim sized against one card's radius and clamped against another's rect is
/// a bug that only shows up at one corner, on one card, at one type size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrimSpec {
    /// The card the scrim must stay inside.
    pub card: Rect,
    /// The card's corner radius.
    pub radius: f32,
    /// The card's padding.
    pub pad: f32,
    /// The type size of the block's tallest row. Both inflations come from it.
    pub largest: f32,
    /// The script of that row, which changes its cap height and leading.
    pub script: Script,
}

/// The scrim rectangle for a text block, per spec §4.4.
///
/// `block` is the union of the block's rows. `spec.largest` is the type size of
/// its tallest row, which sets both inflations — a scrim sized to a 67 lu hero
/// and a scrim sized to an 11 lu caption are not the same shape, and using one
/// number for both leaves the hero's descenders outside the plate.
///
/// The result never exceeds the card's inner rect (the card deflated by 2 lu).
/// On a text-only card that clamp usually binds, and the translucency shows
/// only in the 2 lu ring plus the feather. That is the honest cost of having no
/// backdrop blur, and it is why the visualiser and the disc — which have large
/// text-free areas — are where the glass actually reads.
pub fn scrim_rect(block: Rect, spec: ScrimSpec) -> (Rect, f32) {
    let cap = typo::cap_height(spec.largest, spec.script);
    let line = typo::line_height_ratio(spec.largest, spec.script) * spec.largest.max(0.0);
    let dx = (0.55 * cap).max(8.0);
    let dy = (0.35 * line).max(6.0);
    let want = block.inset_xy(-dx, -dy);
    let inner = spec.card.inset(2.0);
    let r = want.intersect(inner);
    // The nesting rule (§6.3), applied to the scrim's own inset from the card
    // edge, so the two curves stay concentric all the way round the corner.
    let gap_edge = (r.x - spec.card.x).max(0.0);
    let radius = radius_nested(spec.radius, gap_edge).max(8.0);
    (r, radius)
}

/// Draw the feathered plate behind a text block and return the rect it covered.
///
/// **In dark mode this is not optional.** There is no code path in this toolkit
/// that draws [`Theme::text_secondary`] or [`Theme::text_tertiary`] on a dark
/// card without one, and there must not be: unscrimmed they score 4.32:1 and
/// 3.15:1 on the worst dark card, both fails.
///
/// In light mode it is drawn for texture rather than contrast — a 10% wallpaper
/// mottle starts to break Inter's counters below about 18 lu — **except** behind
/// accent-coloured text, where it is mandatory in both themes because three of
/// the six light accents fail AA on a bare light card.
pub fn text_scrim(c: &mut Canvas, t: &Theme, block: Rect, spec: ScrimSpec) -> Rect {
    if block.is_empty() || spec.card.is_empty() {
        return Rect::ZERO;
    }
    let (r, radius) = scrim_rect(block, spec);
    if r.is_empty() {
        return Rect::ZERO;
    }
    c.soft_plate(r, radius, t.metrics.scrim_feather, t.scrim);
    r
}

/// Which of a card's own edges a [`zone_scrim`] is flush with.
///
/// Both carry the y the caller wants the plate's **free** edge at, so a card
/// with two zones can centre the un-scrimmed gap on whatever falls in it (the
/// now-playing card's divider). The value only ever *adds* coverage: a free
/// edge that would cut into the §4.4 margin is pushed back out to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrimZone {
    /// Flush with the card's top, left and right inner edges; the free edge is
    /// the bottom one.
    Top {
        /// Requested y of the plate's bottom edge.
        free_edge: f32,
    },
    /// Flush with the card's bottom, left and right inner edges; the free edge
    /// is the top one.
    Bottom {
        /// Requested y of the plate's top edge.
        free_edge: f32,
    },
}

/// A scrim that is a **zone of the card** rather than a plate floating inside
/// it (spec §9.2).
///
/// [`text_scrim`] sizes its plate to the ink and stops there, which is right
/// when the plate then clamps to the card's inner rect — the clock card's does,
/// and the result is invisible. It is wrong when the block is short enough that
/// the plate clears the clamp on every side: four rounded corners inset from
/// the card edge is not a scrim any more, it is a second card, and on the
/// now-playing card it put a visible seam under the header.
///
/// So a zone scrim runs to the card's inner rect on three sides, takes the
/// card's own (nested) radius on the two corners it shares with it, and takes
/// **square** corners on its free edge. Its only visible boundary is a straight
/// full-width feathered edge facing the gap, which is what §9.2 asks the gap to
/// be.
///
/// Coverage is never less than [`scrim_rect`] would give, so the §4 contrast
/// figures carry over unchanged — the plate only ever grows.
pub fn zone_scrim(
    c: &mut Canvas,
    t: &Theme,
    block: Rect,
    spec: ScrimSpec,
    zone: ScrimZone,
) -> Rect {
    zone_scrim_in(c, t, block, spec, zone, t.scrim)
}

/// How much of the scrim survives across the gap between two zones (§9.2).
///
/// §9.2 asks for two scrims with the wallpaper reading through between them.
/// Taken literally — full scrim, *nothing*, full scrim — the card composites as
/// bright band / dark band / bright band, and a full-width tonal stripe across
/// the middle of a card is read as the seam between two cards no matter how far
/// its edges are feathered. Measured on the light now-playing card over a
/// near-black wallpaper the gap sat 0.138 in relative luminance below the two
/// zones either side of it.
///
/// So the gap keeps a fraction of the scrim rather than none of it. At 0.45 the
/// same measurement is 0.063 — a shading rather than a boundary — and the
/// wallpaper still leaks 21.7% there against 14.0% under the text in dark mode,
/// 7.5% against 4.5% in light. The glass §9.2 is protecting is still what reads
/// in the gap; it is no longer the *only* thing that does.
pub const SCRIM_WAIST: f32 = 0.45;

/// Lay the base of a waisted scrim across the card's whole inner rect, and
/// return the colour its two [`zone_scrim_in`] plates must then be drawn in.
///
/// The returned alpha solves `1 − (1 − base)(1 − zone) = t.scrim.a`, so base and
/// zone composite to **exactly** [`Theme::scrim`] wherever they overlap. Every
/// figure in §4.1 and §4.2 therefore carries over untouched: the text still sits
/// on the scrimmed surface those tables were computed against, and only the gap
/// between the two blocks changes.
///
/// Costs no blur. The plate's boundary *is* the card's inner edge, so it needs
/// no feather, and a feathered one would have added a fifth full-canvas blur to
/// a card that already pays for four.
pub fn scrim_waist(c: &mut Canvas, t: &Theme, card: Rect, radius: f32) -> Color {
    let inner = card.inset(2.0);
    let full = t.scrim.a;
    if inner.is_empty() || !full.is_finite() || full <= 0.0 {
        return t.scrim;
    }
    let base = (full * SCRIM_WAIST).clamp(0.0, full);
    c.rounded_rect(
        inner,
        radius_nested(radius, 2.0).max(0.0),
        &Fill::solid(t.scrim.with_alpha(base)),
    );
    let zone = if base >= 1.0 {
        0.0
    } else {
        ((full - base) / (1.0 - base)).clamp(0.0, 1.0)
    };
    t.scrim.with_alpha(zone)
}

/// [`zone_scrim`], drawn in an explicit colour rather than [`Theme::scrim`].
///
/// The one caller that needs this is a card with two zones and a waist between
/// them: [`scrim_waist`] lays the base and hands back the residual its zones
/// must use so the pair still composites to the §4 surface.
pub fn zone_scrim_in(
    c: &mut Canvas,
    t: &Theme,
    block: Rect,
    spec: ScrimSpec,
    zone: ScrimZone,
    scrim: Color,
) -> Rect {
    if block.is_empty() || spec.card.is_empty() {
        return Rect::ZERO;
    }
    let (want, _) = scrim_rect(block, spec);
    let inner = spec.card.inset(2.0);
    if want.is_empty() || inner.is_empty() {
        return Rect::ZERO;
    }
    // The nesting rule (§6.3) at the scrim's own 2 lu inset, so the shared
    // corners stay concentric with the card's all the way round.
    let cr = radius_nested(spec.radius, 2.0).max(0.0);
    // Square corners only where the plate has a free edge. Where it reaches the
    // card's far edge as well — a header-only card, where there is no gap to
    // face — it takes the card's rounding on all four, or its square corners
    // would poke out through the card's rounded ones.
    let (r, corners) = match zone {
        ScrimZone::Top { free_edge } => {
            let bottom = free_edge.max(want.bottom()).min(inner.bottom());
            let far = if bottom >= inner.bottom() - 0.01 {
                cr
            } else {
                0.0
            };
            (
                Rect::ltrb(inner.x, inner.y, inner.right(), bottom),
                [cr, cr, far, far],
            )
        }
        ScrimZone::Bottom { free_edge } => {
            let top = free_edge.min(want.y).max(inner.y);
            let far = if top <= inner.y + 0.01 { cr } else { 0.0 };
            (
                Rect::ltrb(inner.x, top, inner.right(), inner.bottom()),
                [far, far, cr, cr],
            )
        }
    };
    if r.is_empty() {
        return Rect::ZERO;
    }
    c.soft_plate_corners(r, corners, t.metrics.scrim_feather, scrim);
    r
}

/// The inset panel (spec §8.1) — fill, then the two inner edges that carry
/// "inset".
///
/// The bevel is what reads as sunken, not the fill: in light mode the well is
/// *lighter* than the card and still reads as inset, because a dark hairline at
/// the top inner edge and a bright one at the bottom is how a surface says it
/// is below the plane. A darker light-mode well would drive the track toward
/// mid-grey, where four of the six accents fail 3:1.
///
/// Below 16 lu tall the two 1 lu edges plus the blur eat the fill and it reads
/// as a smudge, so the bevel is skipped and only the fill is drawn.
pub fn well(c: &mut Canvas, r: Rect, radius: f32, t: &Theme) {
    if r.is_empty() {
        return;
    }
    let m = t.metrics;
    c.rounded_rect(r, radius, &Fill::solid(t.well));
    if r.h < 16.0 {
        return;
    }
    // A 3 lu blur, approximated by three inset 1 lu arcs. A real inner-shadow
    // primitive would need a second mask and a clip; at this width the stack is
    // visually identical and costs three strokes.
    for (i, k) in [(0.0_f32, 1.00_f32), (1.0, 0.45), (2.0, 0.20)] {
        let inset = r.inset(i);
        if inset.is_empty() {
            break;
        }
        c.top_highlight(
            inset.offset(0.0, 1.0),
            radius_nested(radius, i),
            t.edge_well_top.scale_alpha(k),
            m.hairline,
        );
    }
    c.bottom_highlight(r.offset(0.0, -1.0), radius, t.edge_well_bottom, m.hairline);
}

/// A progress bar (spec §8.2): a well track, an accent fill, and a knob only
/// when the bar is thick enough to carry one.
///
/// `fraction` outside `0.0..=1.0` is clamped; a non-finite one is treated as
/// zero. The fill has a minimum visible width of one track height, so 0.4%
/// progress is a dot rather than nothing.
///
/// There is deliberately no indeterminate state. Fresco either knows the
/// position or has nothing to say, and a bar pinned at zero says "this track is
/// stuck", which is a lie — the caller hides the whole row instead.
pub fn progress_bar(c: &mut Canvas, r: Rect, fraction: f32, t: &Theme) {
    if r.is_empty() {
        return;
    }
    let h = r.h;
    let radius = h / 2.0;
    well(c, r, radius, t);
    c.rounded_rect(r, radius, &Fill::solid(t.track_empty));
    let f = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let w = (r.w * f).max(h).min(r.w);
    let done = Rect::new(r.x, r.y, w, h);
    c.rounded_rect(
        done,
        radius,
        &Fill::linear(
            Point::new(done.x, done.y),
            Point::new(done.right(), done.y),
            t.accent_dim.over(t.accent_fill),
            t.accent_fill,
        ),
    );
    // The knob is the honest marker for "you are here", and it only exists when
    // the bar can carry it. The reference's check-mark is not adopted: a check
    // means *done*, and on a playback bar the current position is the opposite.
    if h >= 6.0 && f > 0.0 {
        let cx = done.right().min(r.right() - radius).max(r.x + radius);
        let kr = h * 0.9;
        let k = Rect::new(cx - kr, r.center().y - kr, kr * 2.0, kr * 2.0);
        elevation(c, k, kr, t, t.e1());
        c.rounded_rect(k, kr, &Fill::solid(t.accent_fill));
    }
}

/// Track height for a bar sitting under type of size `title` (spec §8.2).
pub fn progress_height(title: f32) -> f32 {
    if !title.is_finite() {
        return 4.0;
    }
    (0.14 * title).round().clamp(4.0, 10.0)
}

/// The smallest radius at which an arc gauge still reads. Below it, degrade to
/// a [`progress_bar`] (spec §8.3).
pub const MIN_GAUGE_RADIUS: f32 = 28.0;

/// The gauge's sweep, expressed the way [`Canvas::arc`] wants it: degrees
/// clockwise from 12 o'clock.
///
/// The spec quotes −210°..+30° in screen coordinates, where 0° is 3 o'clock.
/// The conversion is a fixed +90°, done once, here, instead of at every call
/// site — which is exactly the mistake `Canvas::arc`'s own convention exists to
/// prevent.
pub const GAUGE_START_DEG: f32 = -120.0;
/// The gauge's sweep in degrees.
pub const GAUGE_SWEEP_DEG: f32 = 240.0;

/// An arc gauge (spec §8.3) filling `area`, with the value dot at `value`.
///
/// Returns `false` and draws nothing when `area` is too small for the gauge to
/// read — the caller should fall back to a linear bar.
pub fn arc_gauge(c: &mut Canvas, area: Rect, value: f32, t: &Theme) -> bool {
    let side = area.min_side();
    let r = side / 2.0 - 2.0;
    if !r.is_finite() || r < MIN_GAUGE_RADIUS {
        return false;
    }
    let centre = area.center();
    let w = (0.085 * r).round().clamp(3.0, 14.0);
    let v = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    // The well ring is what buys the gauge its 3:1 — an accent arc straight on
    // the card is 1.45–2.98:1 and invisible over a bright wallpaper.
    c.arc(
        centre,
        r,
        GAUGE_START_DEG,
        GAUGE_SWEEP_DEG,
        w + 4.0,
        &Fill::solid(t.well),
    );
    c.arc(
        centre,
        r,
        GAUGE_START_DEG,
        GAUGE_SWEEP_DEG,
        w,
        &Fill::solid(t.track_empty),
    );
    c.arc(
        centre,
        r,
        GAUGE_START_DEG,
        GAUGE_SWEEP_DEG,
        w,
        &Fill::solid(t.accent_dim),
    );
    if v > 0.0 {
        // tiny-skia has no conic gradient, and a chord-aligned linear one over
        // a 240° sweep is visually indistinguishable — and honest about the
        // substrate, which a fake conic built from 60 stroked segments is not.
        c.arc(
            centre,
            r,
            GAUGE_START_DEG,
            GAUGE_SWEEP_DEG * v,
            w,
            &Fill::linear(
                Point::new(area.x, area.bottom()),
                Point::new(area.right(), area.y),
                t.accent_dim.over(t.accent_fill),
                t.accent_fill,
            ),
        );
        let a = (GAUGE_START_DEG + GAUGE_SWEEP_DEG * v - 90.0).to_radians();
        let dot = Point::new(centre.x + r * a.cos(), centre.y + r * a.sin());
        let dr = w * 0.85;
        let db = Rect::new(dot.x - dr, dot.y - dr, dr * 2.0, dr * 2.0);
        elevation(c, db, dr, t, t.e1());
        c.rounded_rect(db, dr, &Fill::solid(t.accent_fill));
    }
    true
}

/// Format `fraction` as a whole-number percentage into `buf`, with no
/// allocation and no way to panic.
///
/// `0.41 → "41%"`. Non-finite is zero and anything outside `0..=1` clamps, so
/// the readout can never disagree with the arc beside it.
fn percent_into(buf: &mut [u8; 4], fraction: f32) -> &str {
    let p = if fraction.is_finite() {
        (fraction * 100.0).round().clamp(0.0, 100.0) as u32
    } else {
        0
    };
    let n = if p >= 100 {
        buf[0] = b'1';
        buf[1] = b'0';
        buf[2] = b'0';
        3
    } else if p >= 10 {
        buf[0] = b'0' + (p / 10) as u8;
        buf[1] = b'0' + (p % 10) as u8;
        2
    } else {
        buf[0] = b'0' + p as u8;
        1
    };
    buf[n] = b'%';
    std::str::from_utf8(&buf[..n + 1]).unwrap_or("")
}

/// The arc gauge **with the centre readout spec §8.3 asks it to carry**.
///
/// An arc with nothing in it is not a gauge, it is a shape: it has no number,
/// no tick and no unit, and next to a clock hero it reads as a rendering
/// artifact rather than as information. §8.3's answer — and §11's, which
/// rejected text curved along the arc as illegible at Fresco's radii — is a
/// centre label: the value, and a caption under it saying what the value is of.
///
/// The value is `fraction × 100` as a whole percent, formatted on the stack.
/// `caption` is the caller's, because every user-visible string in this toolkit
/// is (it has to survive translation); empty draws the number alone, which is
/// still information.
///
/// Both rows sit on the gauge's own circular scrim, because the caption is
/// [`Theme::text_tertiary`] and tertiary ink on a bare dark card is 3.15:1 —
/// a fail. On the scrim it is the §4.1 figure, 4.77:1.
///
/// Returns whether anything was drawn at all; `false` means the gauge was below
/// [`MIN_GAUGE_RADIUS`] and the caller should fall back to a linear bar.
///
/// Costs one extra full-canvas blur over [`arc_gauge`] — the scrim — on a card
/// that repaints once a minute.
pub fn arc_gauge_with_label(
    c: &mut Canvas,
    fonts: &mut FontStack,
    area: Rect,
    value: f32,
    t: &Theme,
    caption: &str,
) -> bool {
    let r = area.min_side() / 2.0 - 2.0;
    if !r.is_finite() || r < MIN_GAUGE_RADIUS {
        return false;
    }
    let centre = area.center();
    let w = (0.085 * r).round().clamp(3.0, 14.0);
    // Inside the well ring, which is `w + 4` wide and centred on `r`.
    let dial = r - (w + 4.0) / 2.0 - 1.0;
    if dial > 8.0 {
        // Drawn *under* the arc, so the arc and its dot stay crisp on top of
        // their own backdrop.
        c.soft_plate(
            Rect::new(centre.x - dial, centre.y - dial, dial * 2.0, dial * 2.0),
            dial,
            t.metrics.scrim_feather,
            t.scrim,
        );
    }
    arc_gauge(c, area, value, t);
    if dial <= 8.0 {
        return true;
    }

    // Half the chord of the dial circle at `dy` from its centre, less a 2 lu
    // margin either side: how wide a row may be at that height without running
    // off the face.
    let chord = |dy: f32| ((dial * dial - dy * dy).max(0.0)).sqrt() * 2.0 - 4.0;

    let mut buf = [0u8; 4];
    let text = percent_into(&mut buf, value);
    // `hero-s` is §8.3's step and the ceiling; below it the readout is sized
    // from the face it has to fit inside rather than from a fixed step, so a
    // gauge at the 28 lu minimum still gets a number rather than an ellipsis.
    let mut vs =
        typo::nearest_ladder_step((0.60 * dial).clamp(Step::Micro.size(), Step::HeroS.size()));
    let mut value_run = typo::styled(text, vs, 600, false, fonts).color(t.text_primary);
    let mut vm = fonts.measure(&value_run, c.scale());
    if vm.width > chord(typo::cap_height(vs, Script::Latin) * 0.5) && vs > Step::Micro.size() {
        vs = Step::Micro.size();
        value_run = typo::styled(text, vs, 600, false, fonts).color(t.text_primary);
        vm = fonts.measure(&value_run, c.scale());
    }
    let value_cap = typo::cap_height(vs, Script::Latin);

    // The caption is the row that gets dropped when the face cannot hold two.
    // The value never is: it is the whole reason the gauge is legible.
    let cased = typo::micro_case(caption);
    let cs = Step::Micro.size();
    let cap_script = Script::of(cased.as_ref());
    let cap_cap = typo::cap_height(cs, cap_script);
    let gap = t.metrics.gap_xs;
    let mut caption_run = None;
    if !cased.is_empty() {
        let run = typo::styled(cased.as_ref(), cs, 600, true, fonts).color(t.text_tertiary);
        let m = fonts.measure(&run, c.scale());
        let total = value_cap + gap + cap_cap;
        // Both rows measured at the stack's outer edge, where the face is
        // narrowest — a row that fits at the centre can still overhang there.
        let avail = chord(total / 2.0);
        if total <= dial * 1.7 && m.width <= avail && vm.width <= avail {
            caption_run = Some((run, m.width));
        }
    }
    if caption_run.is_none() && vm.width > chord(value_cap * 0.5) {
        // Nothing legible fits, and an ellipsised percentage is worse than
        // none at all.
        return true;
    }

    let total = value_cap + caption_run.as_ref().map_or(0.0, |_| gap + cap_cap);
    let mut y = centre.y - total / 2.0;
    c.text(
        fonts,
        &value_run,
        Point::new(
            centre.x - vm.width / 2.0,
            y - typo::cap_gap(vs, Script::Latin),
        ),
    );
    y += value_cap + gap;
    if let Some((run, width)) = caption_run {
        c.text(
            fonts,
            &run,
            Point::new(centre.x - width / 2.0, y - typo::cap_gap(cs, cap_script)),
        );
    }
    true
}

/// How a bar array is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BarPaint {
    /// A per-bar vertical ramp from `accent.dim` at the base to `accent.fill`
    /// at the cap. The panel default: it makes the caps read as the data and
    /// the bases as the floor.
    #[default]
    Vertical,
    /// One flat `accent.fill` across the array.
    Flat,
    /// A ramp across the array from `accent.fill` to a second colour.
    Across(Color),
    /// One flat colour that is **not** the accent. The `Chassis` alternate's
    /// `#F5A623` is the only user: that theme's identity is its colour, so it
    /// opts out of `accent_follow` entirely.
    Fixed(Color),
    /// One flat colour per bar, interpolated between the two by that bar's own
    /// **magnitude** — quiet bands take the first colour, loud ones the second.
    ///
    /// The media chassis's magenta-into-purple panel. Deliberately *not* a
    /// per-bar vertical gradient like [`BarPaint::Vertical`], which allocates a
    /// stop vector per bar per frame: this is the one array that repaints at
    /// frame rate beside a card full of cached chrome, and a hundred and sixty
    /// `Vec`s a frame is exactly the kind of cost the toolkit's allocation
    /// budget exists to refuse. Colouring by level rather than by height also
    /// says something true — the hue is the band's energy — where a vertical
    /// ramp only says "this is a bar".
    Level(Color, Color),
    /// The classic hue sweep, red round to violet, ignoring the accent
    /// entirely — which is the point of having it.
    Spectrum,
}

/// Everything about how a bar array is drawn that is not its data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarStyle {
    /// How the bars are coloured.
    pub paint: BarPaint,
    /// Round the bar caps.
    pub rounded: bool,
    /// Draw the 1 lu baseline. Always on inside a panel: it is what makes a
    /// silent spectrum read as "silent" rather than "broken".
    pub baseline: bool,
    /// Draw peak caps. Off without a well, where they read as debris.
    pub peaks: bool,
    /// Alpha applied to the bars only, never to the card. Fading the card as
    /// well makes the panel vanish and the bars float, which looks like a bug.
    pub opacity: f32,
    /// E1 under each bar. Only for the card-less variant, where it is what
    /// keeps a bar visible across a same-tone region of the photo.
    pub shadow: bool,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            paint: BarPaint::Vertical,
            rounded: true,
            baseline: true,
            peaks: true,
            opacity: 1.0,
            shadow: false,
        }
    }
}

/// The widest and narrowest band counts a bar array will draw (spec §8.4).
///
/// Above 160 the bar is thinner than the gap and the array reads as noise; the
/// existing renderer already folds anything past 200, and 160 is the optical
/// limit at 1080p.
pub const BAND_RANGE: std::ops::RangeInclusive<usize> = 8..=160;

/// Bar width and gap for `n` bands across `w`, solved the way spec §8.4
/// specifies: the gap is a fraction of the bar width, which depends on the gap.
///
/// Two passes converge. A single pass sizes the gap from `w / n` and leaves the
/// array 2–3% narrow at high band counts, which is visible as an unequal margin
/// at the right-hand end.
///
/// The 2 lu minimum gap is a *preference*, not an invariant: 160 bands across a
/// 200 lu panel cannot have it, and honouring it there would make the array
/// 30% wider than the box it was given. Below the point where the bars would
/// fall under 1 lu the gap gives way instead, down to nothing, so the array
/// always fits its width exactly.
pub fn bar_geometry(w: f32, n: usize) -> (f32, f32) {
    if !w.is_finite() || w <= 0.0 || n == 0 {
        return (0.0, 0.0);
    }
    let n_f = n as f32;
    if n == 1 {
        return (w, 0.0);
    }
    let mut bw = w / n_f;
    let mut g = 0.0;
    for _ in 0..2 {
        // Never let the gaps take more than half the array: past that the bars
        // are thinner than the space between them and the spectrum reads as
        // noise rather than as a shape.
        g = (bw * 0.34)
            .round()
            .clamp(2.0, 10.0)
            .min(w * 0.5 / (n_f - 1.0));
        bw = (w - (n_f - 1.0) * g) / n_f;
    }
    (bw.max(0.0), g.max(0.0))
}

/// Draw a bar array (spec §8.4) bottom-aligned in `area`.
///
/// `values` are 0..1 magnitudes; anything outside that, or non-finite, is
/// clamped. `peaks`, when given, is caller-owned state the same length as
/// `values` — the renderer never allocates it, because this is the one widget
/// that redraws every frame while audio plays.
///
/// A silent spectrum still draws: every bar has a 2 lu floor, so silence is a
/// row of dots rather than an empty box. The last frame drawn when the daemon
/// stops pushing must be that resting one, not whatever the final sample was.
pub fn bars(
    c: &mut Canvas,
    area: Rect,
    values: &[f32],
    peaks: Option<&[f32]>,
    t: &Theme,
    style: BarStyle,
) {
    if area.is_empty() || values.is_empty() {
        return;
    }
    let n = values.len().min(*BAND_RANGE.end());
    let (bw, g) = bar_geometry(area.w, n);
    if bw <= 0.0 {
        return;
    }
    let alpha = if style.opacity.is_finite() {
        style.opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let radius = if style.rounded {
        (bw / 2.0).min(3.0)
    } else {
        0.0
    };
    let base = area.bottom();
    for (i, &v) in values.iter().take(n).enumerate() {
        let v = if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let h = (area.h * v).max(2.0).min(area.h);
        let x = area.x + i as f32 * (bw + g);
        let bar = Rect::new(x, base - h, bw, h);
        let fill = bar_fill(bar, i, n, v, t, style.paint, alpha);
        if style.shadow {
            elevation(c, bar, radius, t, t.e1());
        }
        c.rounded_rect(bar, radius, &fill);
        if style.peaks {
            if let Some(p) = peaks.and_then(|p| p.get(i)) {
                let p = if p.is_finite() {
                    p.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let py = base - (area.h * p).max(2.0).min(area.h);
                c.rounded_rect(
                    Rect::new(x, py - 2.0, bw, 2.0),
                    radius.min(1.0),
                    &Fill::solid(t.text_primary.with_alpha(0.55 * alpha)),
                );
            }
        }
    }
    if style.baseline {
        c.rounded_rect(
            Rect::new(area.x, base, area.w, t.metrics.hairline),
            0.0,
            &Fill::solid(t.gridline),
        );
    }
}

/// The fill for one bar, given the array's paint mode.
fn bar_fill(
    bar: Rect,
    i: usize,
    n: usize,
    level: f32,
    t: &Theme,
    paint: BarPaint,
    alpha: f32,
) -> Fill {
    let top = t.accent_fill.scale_alpha(alpha);
    match paint {
        BarPaint::Flat => Fill::solid(top),
        BarPaint::Level(low, high) => {
            Fill::solid(low.lerp(high, level.clamp(0.0, 1.0)).scale_alpha(alpha))
        }
        BarPaint::Fixed(c) => Fill::solid(c.scale_alpha(alpha)),
        BarPaint::Vertical => {
            Fill::vertical(bar, top, t.accent_dim.over(t.well).scale_alpha(alpha))
        }
        BarPaint::Across(end) => {
            let f = if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.0
            };
            Fill::solid(top.lerp(end.scale_alpha(alpha), f))
        }
        BarPaint::Spectrum => {
            let f = if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.0
            };
            Fill::solid(hue_sweep(f).scale_alpha(alpha))
        }
    }
}

/// Red round to violet, as a function of position in the array.
fn hue_sweep(f: f32) -> Color {
    // Six flat stops interpolated, rather than an HSV conversion: the stops are
    // hand-placed so the yellow-green region does not dominate, which a linear
    // hue ramp always does.
    const STOPS: [(f32, u32); 6] = [
        (0.00, 0xFF4D4D),
        (0.20, 0xFFA24D),
        (0.40, 0xF2E14D),
        (0.60, 0x4DD98C),
        (0.80, 0x4DA6FF),
        (1.00, 0xA84DFF),
    ];
    let stops: Vec<Stop> = STOPS
        .iter()
        .map(|&(at, rgb)| {
            Stop::new(
                at,
                Color::rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8),
            )
        })
        .collect();
    super::paint::sample_stops(&stops, f)
}

/// A pill chip (spec §8.5). Returns the rect it occupied.
///
/// A chip never wraps and never shrinks its type; if the label does not fit it
/// is ellipsised. `accent` swaps the neutral well fill for [`Theme::accent_fill`]
/// with [`Theme::text_on_accent`] inside it, which clears 4.5:1 for every accent
/// in both themes.
pub fn chip(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    at: Point,
    text: &str,
    accent: bool,
    max_width: f32,
) -> Rect {
    if text.is_empty() {
        return Rect::at(at, Size::ZERO);
    }
    let h = Step::Micro.size() * 2.0;
    let pad_x = h * 0.45;
    let mut run = typo::step_run(text, Step::Micro, fonts);
    run = run.color(if accent {
        t.text_on_accent
    } else {
        t.accent_ink
    });
    if max_width.is_finite() && max_width > pad_x * 2.0 {
        run = run.max_width(max_width - pad_x * 2.0);
    }
    let m = fonts.measure(&run, c.scale());
    let r = Rect::at(at, Size::new(m.width + pad_x * 2.0, h));
    elevation(c, r, h / 2.0, t, t.e1());
    if accent {
        c.rounded_rect(r, h / 2.0, &Fill::solid(t.accent_fill));
    } else {
        c.rounded_rect(r, h / 2.0, &Fill::solid(t.well));
        c.hairline(r, h / 2.0, t.edge, t.metrics.hairline);
    }
    let inner = r.align(m.size(), HAlign::Center, VAlign::Middle);
    c.text(fonts, &run, inner.origin());
    r
}

/// A circular badge (spec §8.6) carrying `icon`, or a well disc with the app
/// name's first grapheme when there is no icon.
///
/// `cutout` punches a hole in whatever is beneath before drawing, so an
/// overlapping stack reads as separated without the heavy ring that a bright
/// wallpaper turns into a halo.
pub fn badge(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    r: Rect,
    icon: Option<&image::RgbaImage>,
    label: &str,
) {
    let d = r.min_side();
    if d <= 0.0 {
        return;
    }
    let sq = r.align(Size::new(d, d), HAlign::Center, VAlign::Middle);
    elevation(c, sq, d / 2.0, t, t.e1());
    match icon {
        Some(img) => c.image(img, sq, d / 2.0),
        None => {
            c.rounded_rect(sq, d / 2.0, &Fill::solid(t.well));
            if let Some(g) = first_grapheme(label) {
                let run = typo::step_run(g, Step::Micro, fonts).color(t.text_secondary);
                let m = fonts.measure(&run, c.scale());
                c.text(
                    fonts,
                    &run,
                    sq.align(m.size(), HAlign::Center, VAlign::Middle).origin(),
                );
            }
        }
    }
    c.hairline(sq, d / 2.0, t.edge, t.metrics.hairline);
}

/// The first grapheme cluster of `s`, approximated by its first `char`.
///
/// Approximated deliberately: a full cluster segmenter would pull in a table
/// for one badge fallback that is only ever shown when an app has no icon.
/// Where the distinction actually matters — cutting a title to fit — the
/// ellipsis is done by the shaper, which does segment properly.
fn first_grapheme(s: &str) -> Option<&str> {
    let mut it = s.char_indices();
    let (_, c) = it.next()?;
    Some(&s[..c.len_utf8()])
}

/// A bevelled bezel (spec §8.7), for the `Chassis` alternate only.
///
/// This is Reference B's bevelled *button* repurposed as a non-interactive
/// frame. The bevel geometry survives; the button does not, because this
/// surface has no input region and a control that cannot be pressed is an
/// affordance that lies.
pub fn bezel(c: &mut Canvas, r: Rect, t: &Theme) -> Rect {
    if r.is_empty() {
        return Rect::ZERO;
    }
    let radius = 0.22 * r.min_side();
    elevation(c, r, radius, t, t.e1());
    c.rounded_rect(r, radius, &Fill::solid(t.chassis));
    c.top_highlight(r, radius, Color::WHITE.with_alpha(0.14), 1.5);
    c.bottom_highlight(r, radius, Color::BLACK.with_alpha(0.55), 1.5);
    let face = r.inset(3.0);
    if face.is_empty() {
        return r;
    }
    let fr = radius_nested(radius, 3.0);
    c.rounded_rect(face, fr, &Fill::solid(t.chassis_well));
    c.top_highlight(
        face.offset(0.0, 1.0),
        fr,
        Color::BLACK.with_alpha(0.70),
        1.0,
    );
    c.bottom_highlight(
        face.offset(0.0, -1.0),
        fr,
        Color::WHITE.with_alpha(0.07),
        1.0,
    );
    face
}

/// A bottom-up gradient scrim for text or bars drawn with **no card at all**
/// (spec §9.3.2b, §4.5).
///
/// The card-less fallback: `α 0.42` at `base` fading to nothing `1.35 × h`
/// above it, plus 24 lu of horizontal feather at each end so it has no visible
/// start or stop. This is the only legibility instrument available when there
/// is no surface, and it is why the bare visualiser is legal at all.
pub fn gradient_scrim(c: &mut Canvas, area: Rect, t: &Theme) {
    if area.is_empty() {
        return;
    }
    let top = area.bottom() - area.h * 1.35;
    let feather = 24.0_f32;
    let plate = Rect::ltrb(area.x - feather, top, area.right() + feather, area.bottom());
    if plate.is_empty() {
        return;
    }
    let base = if t.mode.is_dark() {
        Color::rgb8(0x04, 0x06, 0x0A)
    } else {
        Color::WHITE
    };
    // The vertical ramp is the contrast; the horizontal one is what stops the
    // plate having two visible ends. tiny-skia cannot multiply two gradients,
    // so the plate is cut into columns and each column's peak alpha carries the
    // horizontal falloff. Sixteen is enough that the seams are below one code
    // point of alpha at the shallowest part of the ramp.
    const COLS: usize = 16;
    let cw = plate.w / COLS as f32;
    for i in 0..COLS {
        let x = plate.x + i as f32 * cw;
        let col = Rect::new(x, plate.y, cw + 0.5, plate.h);
        // Distance from the plate's ends, in feather widths.
        let left = (x + cw * 0.5 - plate.x) / feather;
        let right = (plate.right() - x - cw * 0.5) / feather;
        let k = left.min(right).clamp(0.0, 1.0);
        if k <= 0.0 {
            continue;
        }
        c.rounded_rect(
            col,
            0.0,
            &Fill::vertical(col, base.with_alpha(0.0), base.with_alpha(0.42 * k)),
        );
    }
}

/// A text run outlined in its theme's counter-colour, for text with no surface
/// under it (spec §4.5).
///
/// Draws the outline as eight offset copies and then the fill, which is what
/// libass's `bord` does and what `LyricStylePreset::Subtitle` already relies
/// on. The stroke is never thicker than 6% of the type size: beyond that it
/// closes Inter's counters and the text becomes a smear.
pub fn outlined_text(
    c: &mut Canvas,
    fonts: &mut FontStack,
    run: &TextRun,
    at: Point,
    t: &Theme,
) -> Rect {
    let w = (0.055 * run.size).max(1.5).min(0.06 * run.size).max(1.0);
    let ink = if t.mode.is_dark() {
        Color::BLACK.with_alpha(0.55)
    } else {
        Color::WHITE.with_alpha(0.70)
    };
    let halo = run.clone().color(ink);
    for (dx, dy) in [
        (-1.0_f32, -1.0_f32),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ] {
        c.text(fonts, &halo, at.offset(dx * w, dy * w));
    }
    c.text(fonts, run, at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgetkit::theme::Mode;

    fn theme(mode: Mode) -> Theme {
        Theme::for_accent(mode, crate::config::Accent::Blue)
    }

    fn canvas() -> Canvas {
        Canvas::for_logical(Size::new(400.0, 300.0), 1.0).expect("canvas")
    }

    #[test]
    fn widget_size_puts_the_card_inside_the_bleed_and_anchors_to_the_card() {
        let t = theme(Mode::Dark);
        let s = WidgetSize::new(Size::new(207.0, 132.0), t.e2());
        assert_eq!(s.bleed, 52.0);
        assert_eq!(s.buffer(), Size::new(311.0, 236.0));
        let card = s.card_rect();
        assert_eq!((card.x, card.y, card.w, card.h), (52.0, 52.0, 207.0, 132.0));
        // Given a larger canvas the card is centred, never stretched.
        let big = s.card_in(Rect::new(0.0, 0.0, 500.0, 400.0));
        assert_eq!(big.size(), s.card);
        assert!(big.x > s.bleed);
    }

    #[test]
    fn the_scrim_clamps_to_the_cards_inner_rect_and_stays_concentric() {
        // Spec §4.4's worked example: a 207 x 132 clock card at H = 64.
        let card = Rect::new(0.0, 0.0, 207.0, 132.0);
        let block = Rect::new(20.0, 20.0, 167.0, 92.0);
        let spec = ScrimSpec {
            card,
            radius: 28.0,
            pad: 20.0,
            largest: 64.0,
            script: Script::Latin,
        };
        let (r, radius) = scrim_rect(block, spec);
        // The inflation wants 219 x 134, which clamps to the inner rect.
        let inner = card.inset(2.0);
        assert!(r.w <= inner.w + 0.01 && r.h <= inner.h + 0.01);
        assert!(r.x >= inner.x - 0.01 && r.bottom() <= inner.bottom() + 0.01);
        // In practice it fills the card, which is the honest cost of having no
        // backdrop blur on a text-only card.
        assert!(r.w > card.w * 0.95, "{r:?}");
        assert!(radius >= 8.0);
        // A tiny block gets a scrim bigger than itself but still inside.
        let (tiny, _) = scrim_rect(
            Rect::new(90.0, 60.0, 10.0, 10.0),
            ScrimSpec {
                largest: 11.0,
                ..spec
            },
        );
        assert!(tiny.w > 10.0 && tiny.w < card.w);
    }

    #[test]
    fn a_zone_scrim_is_flush_with_the_card_and_never_covers_less_than_ss_4_4() {
        let mut c = canvas();
        let card = Rect::new(10.0, 10.0, 420.0, 213.0);
        let inner = card.inset(2.0);
        let spec = ScrimSpec {
            card,
            radius: 12.0,
            pad: 20.0,
            largest: 28.0,
            script: Script::Latin,
        };
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            // The lyric block of the worked §9.2 card.
            let block = Rect::new(30.0, 151.0, 380.0, 52.0);
            let (want, _) = scrim_rect(block, spec);
            let r = zone_scrim(
                &mut c,
                &t,
                block,
                spec,
                ScrimZone::Bottom { free_edge: 128.0 },
            );
            // Flush on three sides.
            assert!((r.x - inner.x).abs() < 0.01, "{r:?}");
            assert!((r.right() - inner.right()).abs() < 0.01, "{r:?}");
            assert!((r.bottom() - inner.bottom()).abs() < 0.01, "{r:?}");
            // And never less coverage than the plate §4.4 asked for, which is
            // what lets the §4 contrast figures carry over unchanged.
            assert!(r.y <= want.y + 0.01, "{r:?} vs {want:?}");

            let header = Rect::new(118.0, 30.0, 300.0, 72.0);
            let (hwant, _) = scrim_rect(header, spec);
            let h = zone_scrim(
                &mut c,
                &t,
                header,
                spec,
                ScrimZone::Top { free_edge: 104.0 },
            );
            assert!((h.x - inner.x).abs() < 0.01, "{h:?}");
            assert!((h.y - inner.y).abs() < 0.01, "{h:?}");
            assert!(h.bottom() >= hwant.bottom() - 0.01, "{h:?} vs {hwant:?}");
            // The two zones leave a gap rather than meeting.
            assert!(h.bottom() < r.y, "the zones met: {h:?} {r:?}");
        }
    }

    #[test]
    fn a_free_edge_inside_the_ink_margin_is_pushed_back_out_to_it() {
        let mut c = canvas();
        let card = Rect::new(0.0, 0.0, 420.0, 213.0);
        let spec = ScrimSpec {
            card,
            radius: 12.0,
            pad: 20.0,
            largest: 28.0,
            script: Script::Latin,
        };
        let t = theme(Mode::Dark);
        let block = Rect::new(20.0, 141.0, 380.0, 52.0);
        let (want, _) = scrim_rect(block, spec);
        // A caller asking for a free edge well inside the glyphs gets the §4.4
        // margin anyway — a scrim may grow, never shrink.
        let r = zone_scrim(
            &mut c,
            &t,
            block,
            spec,
            ScrimZone::Bottom { free_edge: 180.0 },
        );
        assert!(r.y <= want.y + 0.01, "{r:?} vs {want:?}");
        // An empty card or block draws nothing at all.
        assert_eq!(
            zone_scrim(
                &mut c,
                &t,
                Rect::ZERO,
                spec,
                ScrimZone::Top { free_edge: 0.0 }
            ),
            Rect::ZERO
        );
    }

    #[test]
    fn the_waist_and_its_zones_composite_to_exactly_the_ss_4_scrim() {
        let mut c = canvas();
        let card = Rect::new(10.0, 10.0, 420.0, 213.0);
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            let zone = scrim_waist(&mut c, &t, card, 12.0);
            // Same ink, so base-then-zone is a single alpha composite.
            assert_eq!((zone.r, zone.g, zone.b), (t.scrim.r, t.scrim.g, t.scrim.b));
            let base = t.scrim.a * SCRIM_WAIST;
            let both = 1.0 - (1.0 - base) * (1.0 - zone.a);
            assert!(
                (both - t.scrim.a).abs() < 1e-4,
                "{mode:?}: base {base} + zone {} = {both}, want {}",
                zone.a,
                t.scrim.a
            );
            // The gap is scrimmed, but never as much as the text is: that
            // difference is the glass §9.2 asks the gap to show.
            assert!(base > 0.0 && base < t.scrim.a, "{mode:?}: {base}");
        }
    }

    #[test]
    fn a_waist_on_a_degenerate_card_falls_back_to_the_plain_scrim() {
        let mut c = canvas();
        let t = theme(Mode::Dark);
        for card in [Rect::ZERO, Rect::new(0.0, 0.0, 3.0, 3.0)] {
            let zone = scrim_waist(&mut c, &t, card, 12.0);
            assert_eq!(zone, t.scrim, "{card:?} lost its scrim");
        }
        // A card with no scrim at all (there is none in the shipped themes, but
        // a tinted one could reach zero) must not divide by it.
        let mut flat = theme(Mode::Light);
        flat.scrim = flat.scrim.with_alpha(0.0);
        assert_eq!(
            scrim_waist(&mut c, &flat, Rect::new(0.0, 0.0, 200.0, 200.0), 12.0).a,
            0.0
        );
    }

    #[test]
    fn the_percent_readout_is_whole_clamped_and_allocation_free() {
        let mut b = [0u8; 4];
        assert_eq!(percent_into(&mut b, 0.41), "41%");
        assert_eq!(percent_into(&mut b, 0.0), "0%");
        assert_eq!(percent_into(&mut b, 1.0), "100%");
        assert_eq!(percent_into(&mut b, 0.005), "1%");
        assert_eq!(percent_into(&mut b, 0.094), "9%");
        // Degenerate input reads zero rather than disagreeing with the arc.
        assert_eq!(percent_into(&mut b, f32::NAN), "0%");
        assert_eq!(percent_into(&mut b, -4.0), "0%");
        assert_eq!(percent_into(&mut b, 12.0), "100%");
        // Non-finite is zero, not saturated: it matches how the arc beside it
        // treats the same value, and the two must never disagree.
        assert_eq!(percent_into(&mut b, f32::INFINITY), "0%");
    }

    #[test]
    fn a_labelled_gauge_carries_its_value_and_degrades_rather_than_overflowing() {
        let mut fonts = FontStack::system();
        let mut c = canvas();
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            // Below the minimum radius it is still a no-op, exactly as the
            // unlabelled gauge is: the caller falls back to a linear bar.
            assert!(!arc_gauge_with_label(
                &mut c,
                &mut fonts,
                Rect::new(0.0, 0.0, 40.0, 40.0),
                0.5,
                &t,
                "of day"
            ));
            assert!(arc_gauge_with_label(
                &mut c,
                &mut fonts,
                Rect::new(0.0, 0.0, 120.0, 120.0),
                0.41,
                &t,
                "of day"
            ));
            // Nothing about a degenerate value, caption or rect can panic.
            for v in [f32::NAN, f32::NEG_INFINITY, -3.0, 7.0, 0.0, 1.0] {
                for cap in ["", "of day", "a caption far too long for any dial", "日"] {
                    arc_gauge_with_label(
                        &mut c,
                        &mut fonts,
                        Rect::new(0.0, 0.0, 74.0, 74.0),
                        v,
                        &t,
                        cap,
                    );
                }
            }
            for r in [
                Rect::ZERO,
                Rect::new(0.0, 0.0, 0.0, 90.0),
                Rect::new(-40.0, -40.0, 90.0, 90.0),
                Rect::new(0.0, 0.0, 1e6, 1e6),
                Rect::new(f32::NAN, 0.0, 90.0, 90.0),
            ] {
                arc_gauge_with_label(&mut c, &mut fonts, r, 0.5, &t, "of day");
            }
        }
    }

    #[test]
    fn bar_geometry_converges_and_fills_its_width() {
        for n in [1usize, 2, 8, 32, 64, 160] {
            for w in [80.0_f32, 320.0, 1200.0] {
                let (bw, g) = bar_geometry(w, n);
                assert!(bw > 0.0, "n={n} w={w}");
                let used = bw * n as f32 + g * (n as f32 - 1.0);
                assert!(
                    (used - w).abs() < 0.51,
                    "n={n} w={w}: used {used}, have {w}"
                );
                if n > 1 {
                    assert!((0.0..=10.0).contains(&g), "gap {g} out of range");
                    // The gaps never take more than half the array.
                    assert!(g * (n as f32 - 1.0) <= w * 0.5 + 0.01, "gaps eat the bars");
                }
            }
        }
        // Degenerate input yields nothing to draw rather than a NaN rect.
        assert_eq!(bar_geometry(0.0, 32), (0.0, 0.0));
        assert_eq!(bar_geometry(100.0, 0), (0.0, 0.0));
        assert_eq!(bar_geometry(f32::NAN, 4), (0.0, 0.0));
    }

    #[test]
    fn progress_height_tracks_the_type_it_sits_under() {
        assert_eq!(progress_height(18.0), 4.0);
        assert_eq!(progress_height(27.0), 4.0);
        assert_eq!(progress_height(64.0), 9.0);
        assert_eq!(progress_height(1000.0), 10.0);
        assert_eq!(progress_height(f32::NAN), 4.0);
    }

    #[test]
    fn the_gauge_conversion_lands_where_the_spec_says() {
        // Screen -210 deg becomes -120 deg clockwise from 12 o'clock, and the
        // sweep ends at screen +30.
        assert_eq!(GAUGE_START_DEG, -210.0 + 90.0);
        assert_eq!(GAUGE_START_DEG + GAUGE_SWEEP_DEG, 30.0 + 90.0);
    }

    #[test]
    fn the_gauge_refuses_to_draw_below_its_minimum_radius() {
        let t = theme(Mode::Dark);
        let mut c = canvas();
        assert!(!arc_gauge(&mut c, Rect::new(0.0, 0.0, 40.0, 40.0), 0.5, &t));
        assert!(arc_gauge(
            &mut c,
            Rect::new(0.0, 0.0, 120.0, 120.0),
            0.5,
            &t
        ));
        // And nothing about a degenerate value can panic.
        for v in [f32::NAN, -3.0, 7.0, 0.0, 1.0] {
            arc_gauge(&mut c, Rect::new(0.0, 0.0, 120.0, 120.0), v, &t);
        }
    }

    #[test]
    fn no_component_panics_on_degenerate_geometry() {
        let mut fonts = FontStack::from_font_data("en-US", []);
        let mut c = canvas();
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            for r in [
                Rect::ZERO,
                Rect::new(0.0, 0.0, 0.0, 40.0),
                Rect::new(0.0, 0.0, 40.0, 0.0),
                Rect::new(-100.0, -100.0, 20.0, 20.0),
                Rect::new(0.0, 0.0, 1e6, 1e6),
                Rect::new(f32::NAN, 0.0, 10.0, 10.0),
            ] {
                card(&mut c, r, 12.0, &t);
                well(&mut c, r, 4.0, &t);
                progress_bar(&mut c, r, 0.5, &t);
                arc_gauge(&mut c, r, 0.5, &t);
                arc_gauge_with_label(&mut c, &mut fonts, r, 0.5, &t, "of day");
                zone_scrim(
                    &mut c,
                    &t,
                    r,
                    ScrimSpec {
                        card: r,
                        radius: 12.0,
                        pad: 20.0,
                        largest: 28.0,
                        script: Script::Latin,
                    },
                    ScrimZone::Bottom { free_edge: r.y },
                );
                bezel(&mut c, r, &t);
                gradient_scrim(&mut c, r, &t);
                badge(&mut c, &mut fonts, &t, r, None, "Spotify");
                chip(&mut c, &mut fonts, &t, r.origin(), "FLAC", true, r.w);
                bars(&mut c, r, &[0.5, 0.2, 0.9], None, &t, BarStyle::default());
                text_scrim(
                    &mut c,
                    &t,
                    r,
                    ScrimSpec {
                        card: r,
                        radius: 12.0,
                        pad: 20.0,
                        largest: 14.0,
                        script: Script::Latin,
                    },
                );
            }
            // Empty and absurd data.
            let a = Rect::new(10.0, 10.0, 200.0, 80.0);
            bars(&mut c, a, &[], None, &t, BarStyle::default());
            bars(&mut c, a, &[f32::NAN; 8], None, &t, BarStyle::default());
            bars(
                &mut c,
                a,
                &[1e9; 8],
                Some(&[f32::NAN; 8]),
                &t,
                BarStyle::default(),
            );
            bars(&mut c, a, &vec![0.5; 4000], None, &t, BarStyle::default());
            // A peak slice shorter than the values.
            bars(
                &mut c,
                a,
                &[0.5; 8],
                Some(&[0.9; 2]),
                &t,
                BarStyle::default(),
            );
            chip(
                &mut c,
                &mut fonts,
                &t,
                Point::new(0.0, 0.0),
                "",
                true,
                100.0,
            );
            badge(&mut c, &mut fonts, &t, a, None, "");
        }
    }

    #[test]
    fn every_bar_paint_mode_produces_a_visible_fill() {
        let t = theme(Mode::Dark);
        let bar = Rect::new(0.0, 0.0, 10.0, 40.0);
        for paint in [
            BarPaint::Vertical,
            BarPaint::Flat,
            BarPaint::Across(Color::rgb8(0xFF, 0x00, 0x88)),
            BarPaint::Fixed(Color::rgb8(0xF5, 0xA6, 0x23)),
            BarPaint::Level(Color::rgb8(0x8B, 0x3B, 0xE8), Color::rgb8(0xD9, 0x4F, 0xE0)),
            BarPaint::Spectrum,
        ] {
            for i in [0usize, 5, 31] {
                for level in [0.0, 0.5, 1.0, f32::NAN] {
                    let f = bar_fill(bar, i, 32, level, &t, paint, 1.0);
                    assert!(!f.is_invisible(), "{paint:?} bar {i} is invisible");
                }
            }
        }
        // `Level` really does colour by magnitude, or it is `Fixed` with extra
        // steps.
        let pair = BarPaint::Level(Color::rgb8(0x8B, 0x3B, 0xE8), Color::rgb8(0xD9, 0x4F, 0xE0));
        assert_ne!(
            bar_fill(bar, 0, 32, 0.0, &t, pair, 1.0).sample(0.0),
            bar_fill(bar, 0, 32, 1.0, &t, pair, 1.0).sample(0.0)
        );
        // Opacity applies to the bars and nothing else.
        assert!(bar_fill(bar, 0, 32, 1.0, &t, BarPaint::Flat, 0.0).is_invisible());
        // The hue sweep really does sweep.
        assert_ne!(hue_sweep(0.0), hue_sweep(1.0));
        assert!(hue_sweep(0.0).r > hue_sweep(1.0).r);
    }

    #[test]
    fn first_grapheme_handles_multibyte_and_empty_input() {
        assert_eq!(first_grapheme("Spotify"), Some("S"));
        assert_eq!(first_grapheme("网易云音乐"), Some("网"));
        assert_eq!(first_grapheme(""), None);
    }
}

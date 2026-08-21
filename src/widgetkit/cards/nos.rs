//! The **NOS** clock (spec §9.5) — the squircle, the dot-matrix face and the
//! dotted ring.
//!
//! A second clock *look*, not a second clock. It shares [`ClockData`] with
//! [`super::clock`] and shows the same four strings; what changes is the form
//! language, which is the author's own rather than the reference set §9.1 was
//! drawn from: a near-square card with heavy continuous rounding, monochrome
//! plus one red, flat — nothing bevelled, glossy or gradient-lit — sparse and
//! typographic, with the secondary values kept small.
//!
//! ```text
//!    ·                            ·          ← corner markers, purely graphic
//!         ● ● ● ● ● · · · · ·
//!      ●                       ·             ● lit, ·  unlit — three channels
//!    ●        TUESDAY  28 JULY    ·            apart, not one (see below)
//!    ●                            ·
//!    ●          14:32             ·          ← dot matrix, sized from the
//!    ●                            ·            widest reachable time
//!     ●     ● 9h 27m left today  ·           ← the legend dot is what binds
//!       ●                      ·               the caption to the arc
//!         · · · · · · · · · · ·
//!    ·                            ·
//! ```
//!
//! # The ring cannot be carried by hue, and is not
//!
//! The elapsed arc is red and the remainder is grey. Fitted the obvious way —
//! red against the tertiary ink — those two colours land **1.23:1** apart in
//! dark and **1.13:1** in light: within fifteen per cent of the same luminance,
//! differing in hue and in nothing else. That is a ring that reads as uniform
//! in greyscale, and as uniform to the ~8% of men with a red-green deficiency,
//! on a wallpaper meant to be read from across a room. Hue alone may not carry
//! the only meaningful state in a design.
//!
//! So the arc is separated on **three** channels at once:
//!
//! | Channel | Elapsed | Remainder |
//! |---|---|---|
//! | Hue | [`Theme::nos_red`] | neutral |
//! | Luminance | — | [`Theme::nos_dim`], **1.93:1 / 2.09:1** away from the red |
//! | Size | diameter `d`, and `1.35 d` at the head | diameter [`UNLIT_RATIO`] `· d` |
//!
//! Any one of the three, on its own, locates the value. The head dot is the
//! fourth: a single larger dot at the leading edge, so "where is it now" is a
//! fixation rather than a scan. `theme::tests` asserts the luminance step,
//! `tests::the_ring_reads_with_every_colour_removed` asserts the size one by
//! rendering the ring and throwing the colour away.
//!
//! # Why the arc needs no label of its own
//!
//! An unlabelled arc reads as a rendering artifact — that is why §8.3's gauge
//! grew a value and an `OF DAY` caption in its centre. Here the centre is
//! occupied by the time, so the meaning is carried differently: the caption row
//! **states the value in words** (`9h 27m left today`) and is preceded by a
//! legend dot in the arc's own red at the arc's own lit diameter. Number and
//! picture change together and sit two lines apart; the dot is what says they
//! are the same fact. That is a chart legend, which is the oldest solution to
//! this problem and the only one that survives the centre being full.
//!
//! # Dot-matrix type is a rasterisation problem, and it is only solved for
//! numerals
//!
//! [`crate::widgetkit::dotmatrix`] draws a real 5 × 7 grid — no font in the
//! fallback chain has one — and the honest limits are stated there:
//!
//! * A seven-row grid at cap height `H` has a pitch of `H / 7`, and below about
//!   two device pixels of pitch the counters of `8` and `0` merge. So the face
//!   is used only where the pitch clears
//!   [`dotmatrix::MIN_PITCH_PX`]
//!   **at the scale being drawn**, which is a per-render decision, not a
//!   per-design one. Below it the hero falls back to the mono face at the same
//!   cap height, which is a smaller change than it sounds: the grid is tabular
//!   by construction and so is `tnum`.
//! * **CJK cannot be dot-matrixed at all.** A 5 × 7 cell holds about ten
//!   strokes; `零九:四一` needs far more, and Fresco ships a Simplified-Chinese
//!   UI as a first-class target. The face covers a closed ASCII set and
//!   `supported()` is checked before it is chosen, so a Chinese locale gets
//!   real type rather than a smear.
//!
//! The micro-label and the caption are **never** dot-matrixed. At 11 lu their
//! pitch would be 1.14 device px at 1×, which is not a grid, it is a stain.
//! They are set in the mono face tracked wide instead, which is what a matrix
//! label *looks* like at that size without pretending to be one.
//!
//! # The two faults this look was asked to fix
//!
//! * **`Week 34 · GMT+05:30`.** An ISO week number and a UTC offset are
//!   developer trivia — nobody reads them and nobody acts on them.
//!   `crate::clock::secondary_line` now says how much of the day is left, which
//!   is a number a person actually uses, and it is the number the ring draws.
//! * **`17:07:05` is eight glyphs where the layout was tuned against five.**
//!   The pitch is solved from [`ClockData::widest_time`] — the widest string the
//!   *settings* can ever produce — against the ring's inner chord, so turning
//!   seconds on makes the numerals smaller once and then never moves them
//!   again. A clock that re-fits itself every second is a clock that shimmers.

use crate::widgetkit::canvas::Canvas;
use crate::widgetkit::color::Color;
use crate::widgetkit::dotmatrix;
use crate::widgetkit::geom::{Point, Rect, Size};
use crate::widgetkit::paint::Fill;
use crate::widgetkit::surface::{self, WidgetSize};
use crate::widgetkit::text::{FontStack, TextRun};
use crate::widgetkit::theme::{card_padding, Theme};
use crate::widgetkit::typo::{self, Script, Step};

use super::clock::ClockData;

/// Card side as a multiple of `font_size_pt`.
///
/// Fitted rather than picked. On this card `font_size_pt` does not set a glyph
/// size — the hero is solved from the ring's inner chord — so the ratio is
/// chosen to make the matrix hero land on the cap height every *other* clock
/// theme gives at the same setting (`0.727 · H`, Inter's cap ratio). Working
/// backwards through the chord, the ring and the padding puts that at 4.75, and
/// a 64 lu clock therefore draws a 304 lu card with 46 lu numerals in it. A
/// smaller ratio makes a card whose digits are visibly smaller than the same
/// setting produces everywhere else, which reads as a bug rather than as a
/// look.
const SIDE_PER_H: f32 = 4.75;
/// The smallest card that can hold a ring, a matrix and two label rows.
const MIN_SIDE: f32 = 96.0;
/// The largest. Past this the ring is bigger than the shorter side of a 1080p
/// screen and nothing about the design is improved by it.
const MAX_SIDE: f32 = 720.0;
/// Corner radius as a fraction of the side. The squircle's own clamp allows up
/// to `1 / (2 · SQUIRCLE_SPREAD)` = 0.39; 0.30 is where an iOS widget sits.
const RADIUS_RATIO: f32 = 0.30;
/// Lit dot diameter as a fraction of the side.
const DOT_RATIO: f32 = 0.042;
/// Unlit dot diameter, as a fraction of the lit one. **A load-bearing number**:
/// this is the size channel that keeps the ring legible with the colour thrown
/// away.
pub const UNLIT_RATIO: f32 = 0.55;
/// The head dot's diameter, as a multiple of a lit dot's.
pub const HEAD_RATIO: f32 = 1.35;
/// Centre-to-centre spacing along the ring, as a multiple of the lit diameter.
const PITCH_RATIO: f32 = 1.9;
/// The ring's dot count, clamped. Below 24 the arc quantises to a quarter of an
/// hour and jumps; above 72 the dots touch at any card size Fresco draws.
const RING_DOTS: std::ops::RangeInclusive<usize> = 24..=72;
/// How much of the ring's inner diameter the **hero** may use across, and how
/// much of it the whole block may use down. Less than 1 because a chord is
/// shorter than a diameter everywhere but the middle, and the block is three
/// rows tall. The two label rows do not use this — they get their own chord,
/// measured at the height they actually sit at (see [`chord`]).
const CONTENT_W: f32 = 0.86;
const CONTENT_H: f32 = 0.82;
/// Clearance between a row's ink and the ring's dots, as a fraction of the
/// chord at that height.
const CHORD_CLEAR: f32 = 0.94;

/// The resolved geometry of one NOS card.
#[derive(Debug, Clone, Copy)]
struct Layout {
    side: f32,
    radius: f32,
    /// Lit dot diameter.
    dot: f32,
    /// Centre radius of the ring of dots.
    ring_r: f32,
    /// Radius of the circle the content block has to live inside.
    inner_r: f32,
    dots: usize,
    /// Dot-matrix pitch, or 0 when the hero falls back to a real font.
    pitch: f32,
    /// Cap height of the hero row, matrix or font.
    hero_cap: f32,
    micro: f32,
    /// Content block, relative to the card origin.
    block: Rect,
    has_micro: bool,
    has_caption: bool,
}

fn hero_size(d: &ClockData) -> f32 {
    if d.font_size.is_finite() && d.font_size > 0.0 {
        d.font_size.clamp(6.0, 400.0)
    } else {
        64.0
    }
}

/// The micro-label: weekday, then the date if there is one. Never a hole.
fn micro_text(d: &ClockData) -> String {
    match (d.weekday.is_empty(), d.date.is_empty()) {
        (true, true) => String::new(),
        (false, true) => d.weekday.to_string(),
        (true, false) => d.date.to_string(),
        (false, false) => format!("{} · {}", d.weekday, d.date),
    }
}

/// The longest form of the micro-label that fits `max_w`.
///
/// Ellipsis is the wrong failure mode for this row. Fitted to a chord it
/// produced `THURSDAY · 2…` on a real desktop, which names neither the weekday
/// nor the date: the row kept its space and returned nothing. Dropping a whole
/// field instead leaves something that can still be read, which is the only
/// reason the row is on the card at all.
///
/// The order is deliberate. When only one field survives it is the date, not
/// the weekday — the weekday can be recovered from a date and not the other
/// way round. The shortest form is drawn whether or not it fits, because at
/// that point there is nothing left to drop and an ellipsised word still beats
/// an empty row.
fn fit_micro(
    d: &ClockData,
    max_w: f32,
    size: f32,
    fonts: &mut FontStack,
    scale: f32,
) -> Option<String> {
    let wd = d.weekday.trim();
    let dt = d.date.trim();
    let mut forms: Vec<String> = Vec::new();
    if !wd.is_empty() && !dt.is_empty() {
        forms.push(format!("{wd} · {dt}"));
    }
    if !dt.is_empty() {
        forms.push(dt.to_string());
    }
    if !wd.is_empty() {
        forms.push(wd.to_string());
    }

    let last = forms.len().checked_sub(1)?;
    for (i, form) in forms.iter().enumerate() {
        let cased = typo::micro_case(form).into_owned();
        if i == last {
            return Some(cased);
        }
        let run = typo::styled(&cased, size, 600, true, fonts);
        if fonts.measure(&run, scale).width <= max_w {
            return Some(cased);
        }
    }
    None
}

/// The caption under the hero — the ring's value, in words.
///
/// `ClockData::secondary` carries it (`crate::clock::secondary_line` writes it
/// for this theme whatever `show_date` says, because on this card it is not a
/// third row of trivia, it is the arc's label). The fallback is §8.3's own
/// centre caption, reused rather than invented: a percentage and what it is a
/// percentage *of*, which is the one thing an arc must never leave unsaid.
fn caption_text(d: &ClockData) -> String {
    if !d.secondary.is_empty() {
        return d.secondary.to_string();
    }
    let pct = (day_fraction(d) * 100.0).round() as i32;
    format!("{pct}% {}", crate::t!("Of day"))
}

fn day_fraction(d: &ClockData) -> f32 {
    if d.day_fraction.is_finite() {
        d.day_fraction.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The string the card is *sized* from — never the one it currently shows.
fn sizing_time<'a>(d: &'a ClockData<'a>) -> &'a str {
    if d.widest_time.is_empty() {
        d.time
    } else {
        d.widest_time
    }
}

/// How many dots are lit, and which one is the head.
///
/// Returned rather than computed inline so the greyscale test can ask the same
/// question the renderer does.
fn lit_count(dots: usize, fraction: f32) -> usize {
    if dots == 0 {
        return 0;
    }
    let f = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((dots as f32 * f).round() as usize).min(dots)
}

/// The width available to a row whose furthest edge is `y` from the centre of
/// a circle of radius `r`.
///
/// The reason the micro-label and the caption are not clipped to the hero's
/// box: a chord is only as long as the diameter at the middle, and those two
/// rows sit above and below it, where there is *more* room than the hero's
/// conservative 0.86 allows — a label ellipsised against the hero's width is
/// ellipsised against a number that was never about it.
fn chord(r: f32, y: f32) -> f32 {
    if !r.is_finite() || !y.is_finite() || r <= 0.0 {
        return 0.0;
    }
    let inside = (r * r - y * y).max(0.0);
    2.0 * inside.sqrt() * CHORD_CLEAR
}

/// The three dot diameters, in the order **unlit, lit, head**.
///
/// The size channel of §9.5's ring, in one place so it can be asserted. Every
/// value is a multiple of the lit diameter and none of them is optional.
pub fn ring_dots(lit_diameter: f32) -> (f32, f32, f32) {
    let d = if lit_diameter.is_finite() && lit_diameter > 0.0 {
        lit_diameter
    } else {
        0.0
    };
    (d * UNLIT_RATIO, d, d * HEAD_RATIO)
}

/// The card's fill: the gradient with the scrim **composited into it**.
///
/// Not a feathered plate inside the card, for two reasons. The whole card is
/// one text block, so §4.4's scrim rect clamps to the inner rect anyway — the
/// clock card already discovers this and says so. And a rounded-rect plate
/// inside a squircle shows its own corners, which is precisely the join the
/// continuous curve exists to avoid.
///
/// Compositing is associative, so `scrim.over(surface).over(wallpaper)` is
/// exactly `scrim.over(surface.over(wallpaper))` — the surface §4 scores every
/// ink against, unchanged. The scrim's alpha is constant across the gradient,
/// so §2.2 holds too: colour varies, alpha does not.
fn card_fill(t: &Theme, r: Rect) -> Fill {
    let (from, to) = crate::widgetkit::theme::gradient_line(r, 160.0);
    Fill::linear(
        from,
        to,
        t.scrim.over(t.surface),
        t.scrim.over(t.surface_far),
    )
}

fn layout(fonts: &mut FontStack, t: &Theme, d: &ClockData, scale: f32) -> Layout {
    let h = hero_size(d);
    let side = (4.0 * (SIDE_PER_H * h / 4.0).round()).clamp(MIN_SIDE, MAX_SIDE);
    let radius = side * RADIUS_RATIO;
    let pad = card_padding(side);
    let dot = (DOT_RATIO * side).clamp(3.0, 14.0);
    let (_, _, head) = ring_dots(dot);

    // The ring sits inside the card's padding, with room for the head dot.
    let ring_r = (side / 2.0 - pad - head / 2.0).max(dot);
    let want = (2.0 * std::f32::consts::PI * ring_r / (PITCH_RATIO * dot)).round();
    let dots = if want.is_finite() && want > 0.0 {
        (want as usize).clamp(*RING_DOTS.start(), *RING_DOTS.end())
    } else {
        *RING_DOTS.start()
    };

    // Everything else lives inside the ring, in a rectangle the ring's chords
    // can actually contain.
    let inner_r = (ring_r - head / 2.0 - t.metrics.gap_s).max(1.0);
    let content_w = (2.0 * inner_r * CONTENT_W).max(1.0);
    let content_h = (2.0 * inner_r * CONTENT_H).max(1.0);

    let micro = typo::nearest_ladder_step(0.05 * side).max(Step::Micro.size());
    let has_micro = !micro_text(d).is_empty();
    let has_caption = true;
    let micro_cap = typo::cap_height(micro, Script::Latin);
    let gap = t.metrics.gap_m;

    // The hero gets whatever the two label rows leave, and no more.
    let rows_h = if has_micro { micro_cap + gap } else { 0.0 } + micro_cap + gap;
    let hero_room = (content_h - rows_h).max(6.0);

    let widest = sizing_time(d);
    let cols = dotmatrix::advance_cols(widest).max(1.0);
    let pitch_w = content_w / cols;
    let pitch_h = hero_room / dotmatrix::ROWS as f32;
    let pitch = pitch_w.min(pitch_h).max(0.0);
    // The matrix is chosen only where it is legible *at this density*: a pitch
    // is logical units, and the floor is device pixels.
    let matrix = dotmatrix::supported(widest)
        && dotmatrix::supported(d.time)
        && pitch * scale.max(0.05) >= dotmatrix::MIN_PITCH_PX;
    let hero_cap = pitch * dotmatrix::ROWS as f32;

    let mut l = Layout {
        side,
        radius,
        dot,
        ring_r,
        inner_r,
        dots,
        pitch: if matrix { pitch } else { 0.0 },
        hero_cap,
        micro,
        block: Rect::ZERO,
        has_micro,
        has_caption,
    };

    // A font hero has to be measured, because unlike the grid it is not
    // tabular by width: the size that makes the widest string fit is what the
    // current one is drawn at.
    if !matrix {
        let size = font_hero_size(hero_cap, Script::of(widest));
        let run = font_hero_run(widest, &l, t, fonts, size);
        let w = fonts.measure(&run, scale).width.max(1.0);
        if w > content_w {
            let shrunk = size * (content_w / w);
            l.hero_cap = typo::cap_height(shrunk, Script::of(widest));
        }
    }

    let block_h = if has_micro { micro_cap + gap } else { 0.0 } + l.hero_cap + gap + micro_cap;
    l.block = Rect::new(
        side / 2.0 - content_w / 2.0,
        side / 2.0 - block_h / 2.0,
        content_w,
        block_h,
    );
    l
}

/// The font size whose cap height is `cap`, in the hero's script.
fn font_hero_size(cap: f32, script: Script) -> f32 {
    let ratio = typo::cap_height(100.0, script) / 100.0;
    if ratio > 0.0 {
        (cap / ratio).clamp(6.0, 400.0)
    } else {
        cap.max(6.0)
    }
}

/// The hero as a real font run — the fallback when the grid cannot be drawn.
///
/// Mono, not the Latin stack: the matrix is tabular by construction, and the
/// fallback has to be too or the clock jitters the moment it degrades.
fn font_hero_run(text: &str, _l: &Layout, t: &Theme, fonts: &mut FontStack, size: f32) -> TextRun {
    if Script::of(text) == Script::Latin {
        typo::mono_run(text, size, fonts).color(t.text_primary)
    } else {
        // CJK has no mono stack worth asking for and no dot grid at all; it
        // gets the real face, at the weight §5.3 allows.
        typo::styled(text, size, 700, false, fonts).color(t.text_primary)
    }
}

/// How big this card is, and how much shadow margin it needs.
pub fn measure(fonts: &mut FontStack, t: &Theme, d: &ClockData, scale: f32) -> WidgetSize {
    let l = layout(fonts, t, d, scale);
    WidgetSize::new(Size::new(l.side, l.side), t.e2())
}

/// Draw the card, centred in whatever room `canvas` provides.
pub fn draw(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &ClockData) {
    let size = measure(fonts, t, d, c.scale());
    let rect = size.card_in(c.bounds());
    draw_at(c, fonts, t, d, rect);
}

/// Draw the card with its card rect at `card`.
///
/// `card` is used for its origin and clamped to a square of the measured side,
/// so a caller that hands over a landscape rectangle gets a NOS card rather
/// than a stretched one — the proportions *are* the design.
pub fn draw_at(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &ClockData, card: Rect) {
    if card.is_empty() {
        return;
    }
    let l = layout(fonts, t, d, c.scale());
    let side = l.side.min(card.w).min(card.h);
    if side <= 0.0 {
        return;
    }
    let card = Rect::new(
        card.x + (card.w - side) / 2.0,
        card.y + (card.h - side) / 2.0,
        side,
        side,
    );
    let k = side / l.side;
    let radius = l.radius * k;

    // The shadow mask is a circular-cornered rounded rect at the same radius.
    // The two outlines agree to within a per cent at the diagonal — a squircle
    // of radius `r` and a circle of radius `1.006 r` put their apex in the same
    // place — and after a 28 lu blur that is a fraction of the sigma. Giving
    // `drop_shadow` a second path shape would buy nothing and cost a second
    // full-canvas mask.
    surface::elevation(c, card, radius, t, t.e2());
    c.squircle(card, radius, &card_fill(t, card));
    c.squircle_hairline(card, radius, t.edge, t.metrics.hairline);

    draw_ring(c, t, &l, card, k, day_fraction(d));
    draw_content(c, fonts, t, d, &l, card, k);
}

/// The signature: a ring of discrete dots, an arc of them lit.
///
/// Starts at twelve o'clock and runs clockwise, which is the only direction a
/// ring beside a clock can run without being read backwards.
fn draw_ring(c: &mut Canvas, t: &Theme, l: &Layout, card: Rect, k: f32, fraction: f32) {
    if l.dots == 0 {
        return;
    }
    let centre = card.center();
    let ring_r = l.ring_r * k;
    let (unlit, lit_d, head_d) = ring_dots(l.dot * k);
    let lit = lit_count(l.dots, fraction);
    let dim = Fill::solid(t.nos_dim);
    let red = Fill::solid(t.nos_red);
    for i in 0..l.dots {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * (i as f32 / l.dots as f32);
        let (cx, cy) = (centre.x + ring_r * a.cos(), centre.y + ring_r * a.sin());
        let (d, fill) = if i + 1 == lit {
            (head_d, &red)
        } else if i < lit {
            (lit_d, &red)
        } else {
            (unlit, &dim)
        };
        if d <= 0.0 {
            continue;
        }
        c.rounded_rect(Rect::new(cx - d / 2.0, cy - d / 2.0, d, d), d / 2.0, fill);
    }
}

fn draw_content(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &ClockData,
    l: &Layout,
    card: Rect,
    k: f32,
) {
    let block = Rect::new(
        card.x + l.block.x * k,
        card.y + l.block.y * k,
        l.block.w * k,
        l.block.h * k,
    );
    if block.is_empty() {
        return;
    }
    let micro = l.micro * k;
    let gap = t.metrics.gap_m * k;
    let inner_r = l.inner_r * k;
    let mid = card.center().y;
    let scale = c.scale();
    let mut y = block.y;

    if l.has_micro {
        // This row's own chord, measured at its cap top — the point of it
        // furthest from the centre, and therefore the narrowest the ring gets
        // anywhere the row has ink.
        let w = chord(inner_r, y - mid).max(1.0);
        if let Some(cased) = fit_micro(d, w, micro, fonts, scale) {
            let s = Script::of(&cased);
            let run = typo::styled(&cased, micro, 600, true, fonts)
                .color(t.text_tertiary)
                .max_width(w);
            centred(c, fonts, &run, band(block, w), y - typo::cap_gap(micro, s));
        }
        y += typo::cap_height(micro, Script::Latin) + gap;
    }

    // The hero: the grid where it reads, a real face where it would not.
    let hero_cap = l.hero_cap * k;
    if l.pitch > 0.0 && dotmatrix::supported(d.time) {
        let pitch = l.pitch * k;
        let w = dotmatrix::width(d.time, pitch);
        let x = block.x + (block.w - w) / 2.0;
        dotmatrix::draw(c, d.time, Point::new(x, y), pitch, t.text_primary);
    } else if !d.time.is_empty() {
        let s = Script::of(d.time);
        let size = font_hero_size(hero_cap, s);
        let run = font_hero_run(d.time, l, t, fonts, size).max_width(block.w);
        centred(c, fonts, &run, block, y - typo::cap_gap(size, s));
    }
    y += hero_cap + gap;

    if !l.has_caption {
        return;
    }
    // The caption, with the legend dot that binds it to the arc. Measured as
    // one unit and centred as one, or the dot drifts off the phrase it labels.
    let text = caption_text(d);
    let s = Script::of(&text);
    let dot = l.dot * k;
    // The baseline end of the caption is its furthest point from the centre.
    let w = chord(inner_r, y + typo::cap_height(micro, s) - mid).max(1.0);
    let run = typo::styled(&text, micro, 500, false, fonts)
        .color(t.text_secondary)
        .max_width((w - dot * 2.0).max(1.0));
    let m = fonts.measure(&run, c.scale());
    let total = m.width + dot + t.metrics.gap_xs * k;
    let band = band(block, w);
    let x = band.x + ((band.w - total) / 2.0).max(0.0);
    let cy = y + typo::cap_height(micro, s) / 2.0;
    c.rounded_rect(
        Rect::new(x, cy - dot / 2.0, dot, dot),
        dot / 2.0,
        &Fill::solid(t.nos_red),
    );
    c.text(
        fonts,
        &run,
        Point::new(x + dot + t.metrics.gap_xs * k, y - typo::cap_gap(micro, s)),
    );
}

/// `block` widened (or narrowed) to `w`, about the same centre.
fn band(block: Rect, w: f32) -> Rect {
    Rect::new(block.x + (block.w - w) / 2.0, block.y, w, block.h)
}

/// Draw `run` centred across `block`, with its cap top already resolved into
/// `y`.
fn centred(c: &mut Canvas, fonts: &mut FontStack, run: &TextRun, block: Rect, y: f32) {
    let m = fonts.measure(run, c.scale());
    let x = block.x + ((block.w - m.width) / 2.0).max(0.0);
    c.text(fonts, run, Point::new(x, y));
}

/// A flat colour for a caller that wants the card's composited body ink.
pub fn body_colour(t: &Theme) -> Color {
    t.scrim.over(t.surface)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgetkit::theme::Mode;

    fn fonts() -> FontStack {
        FontStack::system()
    }

    fn theme(mode: Mode) -> Theme {
        Theme::for_accent(mode, crate::config::Accent::Blue)
    }

    fn sample() -> ClockData<'static> {
        ClockData {
            time: "14:32",
            widest_time: "00:00",
            weekday: "Tuesday",
            date: "28 July",
            secondary: "9h 27m left today",
            font_size: 64.0,
            variant: crate::widgetkit::ClockVariant::Nos,
            accent_follow: false,
            day_fraction: 0.605,
        }
    }

    #[test]
    fn the_card_is_square_and_heavily_rounded() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let l = layout(&mut f, &t, &sample(), 1.0);
        assert_eq!(l.side, 304.0, "4.75 x 64, rounded to the 4 lu ladder");
        assert!((l.radius - 0.30 * l.side).abs() < 0.01);
        let m = measure(&mut f, &t, &sample(), 1.0);
        assert_eq!(m.card.w, m.card.h, "a NOS card is square");
    }

    /// The size channel, asserted as geometry: three diameters, in order, with
    /// real distance between them.
    #[test]
    fn the_ring_dots_differ_in_size_and_not_only_in_colour() {
        let (unlit, lit, head) = ring_dots(8.0);
        assert!(lit / unlit >= 1.6, "lit {lit} vs unlit {unlit}");
        assert!(head / lit >= 1.2, "head {head} vs lit {lit}");
        // Degenerate input answers rather than propagating.
        for bad in [f32::NAN, 0.0, -4.0] {
            let (a, b, c) = ring_dots(bad);
            assert!(a.is_finite() && b.is_finite() && c.is_finite());
        }
    }

    /// **The greyscale proof.** Render the card twice — once at a fraction of
    /// zero, once at a half — throw every channel away but luminance, and the
    /// two must still differ across the top-right quadrant, which is the arc's
    /// first quarter. If the ring were carried by hue alone this is exactly the
    /// test that would pass in colour and fail here.
    #[test]
    fn the_ring_reads_with_every_colour_removed() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            let mut grey = |fraction: f32| -> f64 {
                let d = ClockData {
                    day_fraction: fraction,
                    ..sample()
                };
                let size = measure(&mut f, &t, &d, 1.0);
                let mut c = Canvas::for_logical(size.buffer(), 1.0).expect("canvas");
                let card = size.card_rect();
                draw_at(&mut c, &mut f, &t, &d, card);
                let px = c.to_bgra();
                // The top-right quadrant of the ring band, in device pixels.
                let (x0, x1) = (px.w / 2, px.w);
                let (y0, y1) = (0, px.h / 2);
                let mut sum = 0.0f64;
                let mut n = 0u32;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let o = ((y * px.w + x) * 4) as usize;
                        let a = f64::from(px.data[o + 3]);
                        if a <= 0.0 {
                            continue;
                        }
                        // Premultiplied: un-premultiply before weighing, so a
                        // transparent margin does not dilute the reading.
                        let l = 0.0722 * f64::from(px.data[o])
                            + 0.7152 * f64::from(px.data[o + 1])
                            + 0.2126 * f64::from(px.data[o + 2]);
                        sum += l * 255.0 / a;
                        n += 1;
                    }
                }
                if n == 0 {
                    0.0
                } else {
                    sum / f64::from(n)
                }
            };
            let empty = grey(0.0);
            let half = grey(0.5);
            assert!(
                (empty - half).abs() > 0.4,
                "{mode:?}: the arc is invisible in greyscale ({empty:.3} vs {half:.3})"
            );
        }
    }

    #[test]
    fn the_matrix_is_used_only_where_it_can_be_read() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        // A default card at 1x: the grid clears the pitch floor comfortably.
        let l = layout(&mut f, &t, &sample(), 1.0);
        assert!(l.pitch * 1.0 >= dotmatrix::MIN_PITCH_PX, "{}", l.pitch);
        // Seconds cost pitch, because the card is sized from the widest string
        // the settings can reach and `00:00:00` is eight glyphs, not five.
        let secs = layout(
            &mut f,
            &t,
            &ClockData {
                time: "17:07:05",
                widest_time: "00:00:00",
                ..sample()
            },
            1.0,
        );
        assert!(secs.pitch < l.pitch, "seconds did not re-fit the hero");
        assert_eq!(secs.side, l.side, "the card resized for seconds");
        // CJK has no grid at all and must fall back rather than smear.
        let cjk = layout(
            &mut f,
            &t,
            &ClockData {
                time: "零九:四一",
                widest_time: "零九:四一",
                weekday: "星期二",
                date: "七月二十八日",
                secondary: "今天还剩 9 小时 27 分",
                ..sample()
            },
            1.0,
        );
        assert_eq!(cjk.pitch, 0.0, "CJK was dot-matrixed");
        // And a card too small for a two-pixel pitch falls back too.
        let tiny = layout(
            &mut f,
            &t,
            &ClockData {
                font_size: 12.0,
                widest_time: "00:00:00 PM",
                ..sample()
            },
            0.75,
        );
        assert_eq!(tiny.pitch, 0.0, "an illegible grid was drawn anyway");
    }

    /// The time is the only thing on the card that moves, so it is the only
    /// thing the card may not be sized from.
    #[test]
    fn the_card_never_re_fits_as_the_clock_ticks() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let base = ClockData {
            widest_time: "00:00:00",
            ..sample()
        };
        let a = layout(
            &mut f,
            &t,
            &ClockData {
                time: "1:11:11",
                ..base
            },
            1.0,
        );
        let b = layout(
            &mut f,
            &t,
            &ClockData {
                time: "23:48:08",
                ..base
            },
            1.0,
        );
        assert_eq!(a.side, b.side);
        assert!((a.pitch - b.pitch).abs() < 1e-4, "{} {}", a.pitch, b.pitch);
    }

    #[test]
    fn the_caption_never_leaves_the_arc_unlabelled() {
        // The clock supplies the phrase; when it does not, the fallback still
        // says what the arc is a fraction of.
        let with = caption_text(&sample());
        assert_eq!(with, "9h 27m left today");
        let without = caption_text(&ClockData {
            secondary: "",
            ..sample()
        });
        assert!(without.contains("61"), "{without}");
        assert!(without.len() > 4, "{without}");
    }

    #[test]
    fn the_lit_arc_tracks_the_value_and_clamps_at_both_ends() {
        assert_eq!(lit_count(48, 0.0), 0);
        assert_eq!(lit_count(48, 1.0), 48);
        assert_eq!(lit_count(48, 0.5), 24);
        assert_eq!(lit_count(48, f32::NAN), 0);
        assert_eq!(lit_count(48, 9.0), 48);
        assert_eq!(lit_count(48, -3.0), 0);
        assert_eq!(lit_count(0, 0.5), 0);
    }

    #[test]
    fn no_combination_of_settings_can_panic() {
        let mut f = fonts();
        let mut c = Canvas::for_logical(Size::new(260.0, 260.0), 1.0).unwrap();
        let strings = [
            ("", "", "", ""),
            ("14:32", "Tuesday", "28 July", "9h 27m left today"),
            ("零九:四一", "星期二", "七月二十八日", "今天还剩 9 小时"),
            (
                "a very long time string that will never fit anywhere",
                "Wednesday",
                "31 December",
                "a caption that also does not fit anywhere at all",
            ),
            ("🎵", "🎶", "🎼", "🎹"),
        ];
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            for size in [f32::NAN, 0.0, 1.0, 64.0, 300.0] {
                for frac in [f32::NAN, -1.0, 0.0, 0.5, 1.0, 9.0] {
                    for (time, week, date, sec) in strings {
                        let d = ClockData {
                            time,
                            widest_time: time,
                            weekday: week,
                            date,
                            secondary: sec,
                            font_size: size,
                            variant: crate::widgetkit::ClockVariant::Nos,
                            accent_follow: true,
                            day_fraction: frac,
                        };
                        let m = measure(&mut f, &t, &d, 1.0);
                        assert!(m.buffer().w.is_finite() && m.buffer().h.is_finite());
                        c.reset();
                        draw_at(&mut c, &mut f, &t, &d, Rect::new(10.0, 10.0, 240.0, 240.0));
                        draw_at(&mut c, &mut f, &t, &d, Rect::new(-80.0, -80.0, 200.0, 90.0));
                        draw_at(&mut c, &mut f, &t, &d, Rect::ZERO);
                    }
                }
            }
            c.reset();
            draw(&mut c, &mut f, &t, &sample());
        }
    }
}

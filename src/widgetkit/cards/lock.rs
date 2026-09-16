//! The **Lock screen** clock — the date over a large, centred time, with no
//! card, the way a phone or laptop lock screen shows it.
//!
//! ```text
//!           Tuesday 15 September        ← semibold, ~0.24 H
//!
//!                 09:41                 ← H, bold, Inter Display, set tight
//! ```
//!
//! # Legibility without a card
//!
//! Every other clock look carries its own ground — a card, a scrim, a squircle
//! — and gets its contrast from it. This one deliberately has none, so the
//! wallpaper sits directly behind the glyphs, and spec §0 rules out the
//! backdrop blur a real lock screen leans on. What is left is instrument 4, the
//! shadow, applied to the *type* rather than to a box: one Gaussian shadow cast
//! by both rows, dark under light ink and light under dark ink. It is soft
//! enough to read as depth rather than as an outline, and it is the difference
//! between white type that survives a sky and white type that vanishes into
//! one.
//!
//! The 8-offset halo [`crate::widgetkit::surface::outlined_text`] draws for
//! the tiny Bare clock would have been cheaper. At hero sizes it reads as a
//! stroke round every glyph — exactly the cheapness this look exists to avoid.
//!
//! # Centred, and still not moving
//!
//! Both rows are centred, as on a lock screen. The *box* is sized from
//! [`ClockData::widest_time`], so the anchored widget never changes size and
//! never walks across the desktop as the minutes change; inside it the current
//! time is centred, so a proportional `1` sits in the middle rather than
//! hard against one side.
//!
//! # Cost
//!
//! One blur per repaint, over a buffer the size of the text, for the shadow of
//! both rows together. The clock repaints once a minute by default, so this
//! look costs one blur a minute more than drawing plain text would.

use crate::widgetkit::canvas::Canvas;
use crate::widgetkit::color::Color;
use crate::widgetkit::geom::{Point, Rect, Size};
use crate::widgetkit::surface::WidgetSize;
use crate::widgetkit::text::{FontStack, TextRun};
use crate::widgetkit::theme::Theme;
use crate::widgetkit::typo::{self, Script};

use super::clock::{hero_size, sizing_time, ClockData};

/// Faces for the time, best first.
///
/// Inter Display is Inter's cut for large sizes — tighter default spacing and
/// finer joins — which is most of what makes a big time look drawn rather than
/// typed. Everything after it is the ordinary Latin stack, so a machine without
/// the display cut still gets a good face.
pub const DISPLAY_FAMILIES: [&str; 5] = [
    "Inter Display",
    "Inter",
    "Inter Variable",
    "Noto Sans Display",
    "Noto Sans",
];

/// The time's weight. Bold rather than black: heavier fills the counters in
/// at desktop sizes and starts to read as a headline rather than a clock.
const HERO_WEIGHT: u16 = 700;
/// The date's weight — enough to hold its own at a quarter of the size.
const DATE_WEIGHT: u16 = 600;
/// The date never drops below this, whatever the time's size.
const MIN_DATE: f32 = 13.0;
/// The date's size as a fraction of the time's. A lock screen's date is a
/// caption to the time, not a second headline.
const DATE_RATIO: f32 = 0.20;
/// Clear space from the bottom of the date's descenders to the top of the
/// time's figures, as a fraction of the time's size.
const ROW_GAP: f32 = 0.12;
/// Space between the figures and a 12-hour meridiem, as a fraction of the
/// time's size.
const MERIDIEM_GAP: f32 = 0.10;

/// The resolved geometry. Shared by `measure` and `draw_at` so the two cannot
/// disagree.
///
/// Every row is placed from its **measured ink** ([`FontStack::ink_bounds`]),
/// not from font-metric ratios. The time is a display cut at a large size and
/// a tight line box, and ratios tuned for text put its figures several pixels
/// off — enough to let the date's descenders touch them and to drop a
/// meridiem below the baseline.
#[derive(Debug, Clone, Copy)]
struct Layout {
    hero: f32,
    date: f32,
    /// The ink of the text block. The shadow lives in the bleed, not in here,
    /// so `margin_px` measures to the type and not to empty padding.
    size: Size,
    /// Where each row's line box starts, measured down from the block's top.
    date_y: f32,
    hero_y: f32,
    /// The figures' ink bottom — their baseline — below `hero_y`. A meridiem
    /// sits on it.
    hero_base: f32,
    blur: f32,
    dy: f32,
}

/// `"Tuesday 15 September"`, or whichever half there is.
fn date_text(d: &ClockData) -> String {
    match (d.weekday.is_empty(), d.date.is_empty()) {
        (true, true) => String::new(),
        (false, true) => d.weekday.to_string(),
        (true, false) => d.date.to_string(),
        (false, false) => format!("{} {}", d.weekday, d.date),
    }
}

/// `"9:41 AM"` → `("9:41", "AM")`; a 24-hour time comes back whole.
///
/// A meridiem set at the full size of the figures doubles the width of the
/// time and reads as two words shouting. A lock screen sets it small, on the
/// figures' baseline, and so does this.
fn split_meridiem(time: &str) -> (&str, &str) {
    match time.rsplit_once(' ') {
        Some((figures, suffix))
            if !figures.is_empty()
                && !suffix.is_empty()
                && suffix.chars().all(char::is_alphabetic) =>
        {
            (figures, suffix)
        }
        _ => (time, ""),
    }
}

/// Swap in the display cut for Latin text. CJK keeps the family
/// [`typo::styled`] chose, which is the one that has the glyphs.
fn display_face(run: TextRun, text: &str, fonts: &mut FontStack) -> TextRun {
    if Script::of(text) != Script::Latin {
        return run;
    }
    match fonts.first_installed(&DISPLAY_FAMILIES) {
        Some(family) => run.family(Some(family)),
        None => run,
    }
}

fn hero_ink(d: &ClockData, t: &Theme) -> Color {
    if d.accent_follow {
        t.accent_ink
    } else {
        t.text_primary
    }
}

/// The time as runs: the figures, and the meridiem when there is one, with
/// the width the pair occupies set side by side.
fn time_runs(
    time: &str,
    d: &ClockData,
    l: &Layout,
    t: &Theme,
    fonts: &mut FontStack,
    scale: f32,
) -> (TextRun, f32, Option<(TextRun, f32)>, f32) {
    let (figures, suffix) = split_meridiem(time);
    let ink = hero_ink(d, t);
    let run = typo::styled(figures, l.hero, HERO_WEIGHT, false, fonts);
    let run = display_face(run, figures, fonts).color(ink);
    let fig_w = fonts.measure(&run, scale).width;
    if suffix.is_empty() {
        return (run, fig_w, None, fig_w);
    }
    let small = typo::styled(suffix, l.date, DATE_WEIGHT, false, fonts).color(ink);
    let suf_w = fonts.measure(&small, scale).width;
    let total = fig_w + MERIDIEM_GAP * l.hero + suf_w;
    (run, fig_w, Some((small, suf_w)), total)
}

fn date_run(text: &str, size: f32, t: &Theme, fonts: &mut FontStack) -> TextRun {
    typo::styled(text, size, DATE_WEIGHT, false, fonts).color(t.text_primary)
}

/// Blur radius and drop of the text shadow for a time of size `hero`.
///
/// Scaled with the type, because a fixed shadow is a halo on a large time and
/// a smudge on a small one. The drop is a touch of light from above and no
/// more: a long drop reads as a sticker lifted off the wallpaper.
fn shadow(hero: f32) -> (f32, f32) {
    let blur = (0.09 * hero).clamp(3.0, 28.0);
    let dy = (0.015 * hero).clamp(0.5, 4.0);
    (blur, dy)
}

/// Dark shadow under light ink, light shadow under dark ink — whatever the
/// wallpaper is, the ink sits on the side of it that contrasts.
fn shadow_colour(t: &Theme) -> Color {
    if t.mode.is_dark() {
        Color::BLACK.with_alpha(0.45)
    } else {
        Color::WHITE.with_alpha(0.60)
    }
}

/// `(top, bottom)` of `run`'s ink, or the line box's when nothing inks.
fn ink_rows(fonts: &mut FontStack, run: &TextRun, scale: f32) -> (f32, f32) {
    fonts
        .ink_bounds(run, scale)
        .map_or((0.0, run.size), |r| (r.y, r.y + r.h))
}

fn layout(fonts: &mut FontStack, t: &Theme, d: &ClockData, scale: f32) -> Layout {
    let hero = hero_size(d);
    let date = typo::nearest_ladder_step(DATE_RATIO * hero).max(MIN_DATE);
    let date_str = date_text(d);
    let widest = sizing_time(d);

    // The figures of the widest time: their ink is the same height as any
    // other time's, and sizing from it keeps the box still as minutes change.
    let (figures, _) = split_meridiem(widest);
    let run = typo::styled(figures, hero, HERO_WEIGHT, false, fonts);
    let run = display_face(run, figures, fonts);
    let (fig_top, fig_bottom) = ink_rows(fonts, &run, scale);

    let (date_y, time_top) = if date_str.is_empty() {
        (0.0, 0.0)
    } else {
        let run = date_run(&date_str, date, t, fonts);
        let (top, bottom) = ink_rows(fonts, &run, scale);
        // The date's ink top is the block's top; the figures start a fixed gap
        // below its lowest descender.
        (-top, bottom - top + ROW_GAP * hero)
    };
    let hero_y = time_top - fig_top;
    let (blur, dy) = shadow(hero);
    let mut l = Layout {
        hero,
        date,
        size: Size::new(1.0, (hero_y + fig_bottom).max(1.0).ceil()),
        date_y,
        hero_y,
        hero_base: fig_bottom,
        blur,
        dy,
    };

    let (_, _, _, time_w) = time_runs(widest, d, &l, t, fonts, scale);
    let date_w = if date_str.is_empty() {
        0.0
    } else {
        let run = date_run(&date_str, date, t, fonts);
        fonts.measure(&run, scale).width
    };
    l.size.w = time_w.max(date_w).max(1.0).ceil();
    l
}

/// How big this clock is, and how much room its shadow needs around it.
pub fn measure(fonts: &mut FontStack, t: &Theme, d: &ClockData, scale: f32) -> WidgetSize {
    let l = layout(fonts, t, d, scale);
    WidgetSize {
        card: l.size,
        // Same reach as `Shadow::bleed`: the blur's visible tail plus the drop.
        bleed: (l.blur * 1.5 + l.dy).ceil(),
    }
}

/// Draw the clock centred in whatever room `canvas` provides.
pub fn draw(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &ClockData) {
    let size = measure(fonts, t, d, c.scale());
    let rect = size.card_in(c.bounds());
    draw_at(c, fonts, t, d, rect);
}

/// Draw the clock with its text block at `card`.
pub fn draw_at(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &ClockData, card: Rect) {
    if card.is_empty() || !fonts.has_fonts() {
        return;
    }
    let scale = c.scale();
    let l = layout(fonts, t, d, scale);
    let date_str = date_text(d);

    let mut rows: Vec<(TextRun, Point)> = Vec::with_capacity(3);
    if !date_str.is_empty() {
        let run = date_run(&date_str, l.date, t, fonts);
        let w = fonts.measure(&run, scale).width;
        let x = card.x + ((card.w - w) / 2.0).max(0.0);
        rows.push((run, Point::new(x, card.y + l.date_y)));
    }
    if !d.time.is_empty() {
        let (figures, fig_w, suffix, total) = time_runs(d.time, d, &l, t, fonts, scale);
        // The figures and the meridiem centre as one unit.
        let x = card.x + ((card.w - total) / 2.0).max(0.0);
        let y = card.y + l.hero_y;
        rows.push((figures, Point::new(x, y)));
        if let Some((small, _)) = suffix {
            // On the figures' baseline, measured: a meridiem has no
            // descenders, so its ink bottom is its baseline too.
            let (_, small_bottom) = ink_rows(fonts, &small, scale);
            let sx = x + fig_w + MERIDIEM_GAP * l.hero;
            let sy = y + l.hero_base - small_bottom;
            rows.push((small, Point::new(sx, sy)));
        }
    }
    if rows.is_empty() {
        return;
    }

    // One shadow for every row: one blur, and no darker patch where the rows'
    // shadows would otherwise overlap.
    let casts: Vec<(&TextRun, Point)> = rows.iter().map(|(run, at)| (run, *at)).collect();
    c.text_shadow(fonts, &casts, l.blur, l.dy, shadow_colour(t));
    for (run, at) in &rows {
        c.text(fonts, run, *at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgetkit::cards::clock::ClockVariant;
    use crate::widgetkit::theme::Mode;

    fn data(time: &'static str) -> ClockData<'static> {
        ClockData {
            time,
            widest_time: "00:00",
            weekday: "Tuesday",
            date: "15 September",
            secondary: "9h left today",
            font_size: 83.0,
            variant: ClockVariant::Lock,
            accent_follow: false,
            day_fraction: 0.5,
        }
    }

    fn theme() -> Theme {
        Theme::for_accent(Mode::Dark, crate::config::Accent::Blue)
    }

    #[test]
    fn the_box_does_not_follow_the_digits() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = theme();
        let narrow = measure(&mut f, &t, &data("11:11"), 1.0);
        let wide = measure(&mut f, &t, &data("08:48"), 1.0);
        assert_eq!(
            narrow, wide,
            "the widget would resize as the minutes change"
        );
    }

    #[test]
    fn the_date_sits_above_the_time() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let l = layout(&mut f, &theme(), &data("09:41"), 1.0);
        assert!(l.date_y < l.hero_y, "{l:?}");
        assert!(l.date < l.hero, "the date must be the smaller row: {l:?}");
        assert!(l.date >= MIN_DATE, "{l:?}");
    }

    #[test]
    fn missing_rows_neither_panic_nor_leave_a_gap() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = theme();
        let full = measure(&mut f, &t, &data("09:41"), 1.0);
        let mut bare = data("09:41");
        bare.weekday = "";
        bare.date = "";
        let time_only = measure(&mut f, &t, &bare, 1.0);
        assert!(
            time_only.card.h < full.card.h,
            "no date must mean a shorter box"
        );

        let mut nothing = bare;
        nothing.time = "";
        nothing.widest_time = "";
        let mut c = Canvas::new(64, 64, 1.0).unwrap();
        draw(&mut c, &mut f, &t, &nothing);
    }

    #[test]
    fn a_meridiem_is_split_off_and_nothing_else_is() {
        assert_eq!(split_meridiem("9:41 AM"), ("9:41", "AM"));
        assert_eq!(split_meridiem("11:05:09 pm"), ("11:05:09", "pm"));
        assert_eq!(split_meridiem("23:55"), ("23:55", ""));
        assert_eq!(split_meridiem(" AM"), (" AM", ""));
        assert_eq!(split_meridiem("9:41 "), ("9:41 ", ""));
    }

    #[test]
    fn the_shadow_scales_with_the_type() {
        let (small_blur, small_dy) = shadow(24.0);
        let (big_blur, big_dy) = shadow(160.0);
        assert!(big_blur > small_blur && big_dy > small_dy);
        // And stays within its clamps at absurd sizes from a hand-edited config.
        assert_eq!(shadow(10_000.0), (28.0, 4.0));
    }
}

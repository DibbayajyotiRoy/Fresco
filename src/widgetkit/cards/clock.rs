//! The clock card (spec §9.1), in four density variants.
//!
//! # Why the variant is chosen from the type size
//!
//! A clock too small for three rows must **lose rows, not shrink them**. The
//! micro-label is 11 lu and 11 lu is the floor — below it Inter's counters
//! close and an uppercase tracked label becomes a grey smear. So the variant
//! falls out of `font_size_pt` rather than being a separate setting the user
//! can get wrong:
//!
//! | `H` | Variant | Rows |
//! |---|---|---|
//! | `< 24` | [`ClockVariant::Bare`] | hero only, **no card**, outlined |
//! | `24 ≤ H < 35` | [`ClockVariant::Compact`] | micro + hero |
//! | `≥ 35` | [`ClockVariant::Standard`] | micro + hero + secondary |
//! | (opt-in) | [`ClockVariant::Expanded`] | Standard plus an arc gauge |
//!
//! # Width stability — the bug a reference design cannot warn you about
//!
//! `show_seconds` makes the hero change every second, and 12-hour time makes it
//! change at noon. A card sized from the *current* string resizes under the
//! user's cursor once a second, which looks like a rendering fault.
//!
//! So the card's width comes from [`ClockData::widest_time`] — the widest string
//! the current settings can **ever** produce (`00:00:00`, `00:00:00 PM`,
//! `00:00 PM`) — and not from what the clock says right now. Combined with
//! tabular figures, nothing on this card moves horizontally, ever.
//!
//! # Everything is left-aligned to the same x
//!
//! Including the hero. Centring a hero over a left-aligned label is the single
//! most common way this layout goes wrong: the eye reads the label's left edge
//! as the card's text axis, and a centred hero then looks accidentally
//! indented. The one exception is Compact when the label is wider than the
//! hero, where the block centres as a unit.

use crate::widgetkit::canvas::Canvas;
use crate::widgetkit::geom::{HAlign, Point, Rect, Size, VAlign};
use crate::widgetkit::surface::{self, WidgetSize};
use crate::widgetkit::text::{FontStack, TextRun};
use crate::widgetkit::theme::Theme;
use crate::widgetkit::typo::{self, Script};

/// Which density the card is drawn at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockVariant {
    /// Pick from `font_size` — the default, and what the config maps onto.
    #[default]
    Auto,
    /// Hero only, drawn straight onto the wallpaper with a text outline.
    Bare,
    /// Card, micro-label and hero.
    Compact,
    /// Card, micro-label, hero and a secondary line.
    Standard,
    /// Standard plus the day-progress arc gauge in a second column.
    Expanded,
    /// The **NOS** look (spec §9.5): a squircle, a dot-matrix hero and the
    /// dotted progress ring. A different form language, not a density step —
    /// see [`crate::widgetkit::cards::nos`].
    Nos,
}

/// What a clock card draws. Plain data: a mirror of what `crate::clock`
/// computes, deliberately **not** imported from it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ClockData<'a> {
    /// The time to show right now, e.g. `"09:41"`.
    pub time: &'a str,
    /// The widest string the current settings can ever produce. Empty means
    /// "use `time`", which is correct only when the format is fixed-width.
    pub widest_time: &'a str,
    /// Weekday name for the micro-label, e.g. `"Monday"`.
    pub weekday: &'a str,
    /// Date for the micro-label, e.g. `"28 July"`. Empty drops it and keeps the
    /// weekday: the row is never empty, because a card with a hole where a row
    /// goes is a different design.
    pub date: &'a str,
    /// The secondary line, e.g. `"Week 31 · GMT+05:30"`. Empty removes the row
    /// and the card height recomputes; it does not render an empty 14 lu band.
    pub secondary: &'a str,
    /// Hero size in logical units. `config::Clock::font_size_pt`, default 64.
    pub font_size: f32,
    /// Which variant to draw.
    pub variant: ClockVariant,
    /// Draw the hero in [`Theme::accent_ink`]. Honoured only at 24 lu and
    /// above: accent ink is fitted for body text on a scrim, and the hero is
    /// the only row large enough to carry colour without becoming decoration.
    pub accent_follow: bool,
    /// Fraction of the local day elapsed, for the Expanded gauge, which draws
    /// it as a percentage in its centre (spec §8.3).
    pub day_fraction: f32,
}

/// The resolved geometry of one clock card.
#[derive(Debug, Clone, Copy)]
struct Layout {
    variant: ClockVariant,
    hero: f32,
    micro: f32,
    secondary: f32,
    pad: f32,
    radius: f32,
    /// Card size, gauge column included.
    size: Size,
    /// Width of the text column alone.
    text_w: f32,
    gauge: Option<f32>,
    y_micro_cap: f32,
    y_hero_cap: f32,
    y_sec_cap: f32,
    block_h: f32,
}

/// The floor for every derived size on this card.
const MIN_STEP: f32 = 11.0;
/// Below this hero size accent ink is not used: colour on small type reads as
/// decoration and `accent_ink` is fitted for text, not for hairlines.
const ACCENT_MIN: f32 = 24.0;
/// The gauge column is dropped entirely below this card width.
const GAUGE_MIN_CARD_W: f32 = 260.0;

fn hero_size(d: &ClockData) -> f32 {
    if d.font_size.is_finite() && d.font_size > 0.0 {
        d.font_size.clamp(6.0, 400.0)
    } else {
        64.0
    }
}

fn variant_for(d: &ClockData) -> ClockVariant {
    match d.variant {
        ClockVariant::Auto => {
            let h = hero_size(d);
            if h < 24.0 {
                ClockVariant::Bare
            } else if h < 35.0 {
                ClockVariant::Compact
            } else {
                ClockVariant::Standard
            }
        }
        v => v,
    }
}

/// The micro-label: weekday, then the date if there is one.
fn micro_text(d: &ClockData) -> String {
    match (d.weekday.is_empty(), d.date.is_empty()) {
        (true, true) => String::new(),
        (false, true) => d.weekday.to_string(),
        (true, false) => d.date.to_string(),
        (false, false) => format!("{} · {}", d.weekday, d.date),
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

fn micro_run(d: &ClockData, l: &Layout, t: &Theme, fonts: &mut FontStack) -> TextRun {
    let text = micro_text(d);
    let cased = typo::micro_case(&text).into_owned();
    typo::styled(&cased, l.micro, 600, true, fonts).color(t.text_tertiary)
}

fn hero_run(text: &str, d: &ClockData, l: &Layout, t: &Theme, fonts: &mut FontStack) -> TextRun {
    let colour = if d.accent_follow && l.hero >= ACCENT_MIN {
        t.accent_ink
    } else {
        t.text_primary
    };
    typo::styled(text, l.hero, 700, false, fonts).color(colour)
}

fn secondary_run(d: &ClockData, l: &Layout, t: &Theme, fonts: &mut FontStack) -> TextRun {
    typo::styled(d.secondary, l.secondary, 500, false, fonts).color(t.text_secondary)
}

/// Resolve the card's geometry. Shared by `measure` and `draw` so the two can
/// never disagree — a measured size the draw path does not honour is the
/// classic source of a widget that drifts by a pixel per repaint.
fn layout(fonts: &mut FontStack, t: &Theme, d: &ClockData, scale: f32) -> Layout {
    let hero = hero_size(d);
    let variant = variant_for(d);
    let micro = typo::nearest_ladder_step(0.17 * hero).max(MIN_STEP);
    let secondary = typo::nearest_ladder_step(0.22 * hero).max(MIN_STEP);
    let pad = (4.0 * (0.30 * hero / 4.0).round()).clamp(12.0, 28.0);
    let radius = crate::widgetkit::theme::radius_card(hero);

    let mut l = Layout {
        variant,
        hero,
        micro,
        secondary,
        pad,
        radius,
        size: Size::ZERO,
        text_w: 0.0,
        gauge: None,
        y_micro_cap: 0.0,
        y_hero_cap: 0.0,
        y_sec_cap: 0.0,
        block_h: 0.0,
    };

    let has_micro = variant != ClockVariant::Bare && !micro_text(d).is_empty();
    let has_secondary = matches!(variant, ClockVariant::Standard | ClockVariant::Expanded)
        && !d.secondary.is_empty();

    let hero_script = Script::of(sizing_time(d));
    let micro_script = Script::of(&micro_text(d));
    let sec_script = Script::of(d.secondary);

    // Vertical rhythm, measured in cap tops and baselines — never em boxes.
    let mut y = pad;
    if has_micro {
        l.y_micro_cap = y;
        y += typo::cap_height(micro, micro_script) + 0.22 * hero;
    }
    l.y_hero_cap = y;
    y += typo::cap_height(hero, hero_script);
    if has_secondary {
        y += 0.19 * hero;
        l.y_sec_cap = y;
        y += typo::cap_height(secondary, sec_script) + typo::descender(secondary, sec_script);
    } else {
        y += typo::descender(hero, hero_script);
    }
    l.block_h = y - pad;
    let height = y + pad;

    // Width from the widest reachable string, not the current one.
    let mut text_w: f32 = 0.0;
    if has_micro {
        let r = micro_run(d, &l, t, fonts);
        text_w = text_w.max(fonts.measure(&r, scale).width);
    }
    let r = hero_run(sizing_time(d), d, &l, t, fonts);
    text_w = text_w.max(fonts.measure(&r, scale).width);
    if has_secondary {
        let r = secondary_run(d, &l, t, fonts);
        text_w = text_w.max(fonts.measure(&r, scale).width);
    }
    l.text_w = text_w;

    if variant == ClockVariant::Bare {
        l.size = Size::new(text_w, typo::cap_height(hero, hero_script) * 1.4);
        return l;
    }

    let mut width = (text_w + 2.0 * pad).max(3.1 * hero);
    if variant == ClockVariant::Expanded {
        let gr = 1.15 * hero / 2.0;
        // The gauge column is dropped rather than squeezed: a gauge below its
        // minimum radius degrades to a linear bar, and there is no room for one
        // of those either.
        if width + t.metrics.gap_xl + gr * 2.0 >= GAUGE_MIN_CARD_W
            && gr >= surface::MIN_GAUGE_RADIUS
        {
            l.gauge = Some(gr);
            width += t.metrics.gap_xl + gr * 2.0;
        }
    }
    l.size = Size::new(width, height.max(2.0 * pad + hero * 0.5));
    l
}

/// How big this clock card is, and how much shadow margin it needs.
pub fn measure(fonts: &mut FontStack, t: &Theme, d: &ClockData, scale: f32) -> WidgetSize {
    if variant_for(d) == ClockVariant::Nos {
        return super::nos::measure(fonts, t, d, scale);
    }
    let l = layout(fonts, t, d, scale);
    let e = if l.variant == ClockVariant::Bare {
        t.e1()
    } else {
        t.e2()
    };
    WidgetSize::new(l.size, e)
}

/// Draw the card, centred in whatever room `canvas` provides.
pub fn draw(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &ClockData) {
    let size = measure(fonts, t, d, c.scale());
    let rect = size.card_in(c.bounds());
    draw_at(c, fonts, t, d, rect);
}

/// Draw the card with its card rect at `card`.
pub fn draw_at(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &ClockData, card: Rect) {
    if card.is_empty() {
        return;
    }
    if variant_for(d) == ClockVariant::Nos {
        super::nos::draw_at(c, fonts, t, d, card);
        return;
    }
    let l = layout(fonts, t, d, c.scale());
    if l.variant == ClockVariant::Bare {
        draw_bare(c, fonts, t, d, &l, card);
        return;
    }

    surface::card(c, card, l.radius, t);

    // The text column is the card minus the gauge column, so the block and its
    // scrim never run under the gauge.
    let text_col = match l.gauge {
        Some(gr) => card.inset_ltrb(0.0, 0.0, gr * 2.0 + t.metrics.gap_xl, 0.0),
        None => card,
    };

    let has_micro = !micro_text(d).is_empty();
    let has_secondary = matches!(l.variant, ClockVariant::Standard | ClockVariant::Expanded)
        && !d.secondary.is_empty();

    // The scrim covers the whole block: mandatory in dark mode, and drawn in
    // light mode too because the micro-label is 11 lu, well below the 18 lu
    // where a wallpaper mottle starts breaking counters.
    let block = Rect::new(
        text_col.x + l.pad,
        card.y
            + (if has_micro {
                l.y_micro_cap
            } else {
                l.y_hero_cap
            }),
        l.text_w.max(1.0),
        l.block_h,
    );
    surface::text_scrim(
        c,
        t,
        block,
        surface::ScrimSpec {
            card: text_col,
            radius: l.radius,
            pad: l.pad,
            largest: l.hero,
            script: Script::of(d.time),
        },
    );

    let x = text_col.x + l.pad;
    let avail = (text_col.w - 2.0 * l.pad).max(1.0);

    if has_micro {
        let run = micro_run(d, &l, t, fonts).max_width(avail);
        let s = Script::of(&micro_text(d));
        let y = card.y + l.y_micro_cap - typo::cap_gap(l.micro, s);
        // Compact centres the block as a unit when the label is wider than the
        // hero; every other case is flush left.
        let dx = if l.variant == ClockVariant::Compact {
            let m = fonts.measure(&run, c.scale());
            ((avail - m.width) / 2.0).max(0.0).min(avail)
        } else {
            0.0
        };
        c.text(fonts, &run, Point::new(x + dx, y));
    }

    let hero_script = Script::of(d.time);
    let run = hero_run(d.time, d, &l, t, fonts).max_width(avail);
    let y = card.y + l.y_hero_cap - typo::cap_gap(l.hero, hero_script);
    c.text(fonts, &run, Point::new(x, y));

    if has_secondary {
        let s = Script::of(d.secondary);
        let run = secondary_run(d, &l, t, fonts).max_width(avail);
        let y = card.y + l.y_sec_cap - typo::cap_gap(l.secondary, s);
        c.text(fonts, &run, Point::new(x, y));
    }

    if let Some(gr) = l.gauge {
        let area = Rect::new(
            card.right() - l.pad - gr * 2.0,
            card.center().y - gr,
            gr * 2.0,
            gr * 2.0,
        );
        // §8.3's centre label in full: the value, and the `micro` tertiary row
        // under it saying what the value is *of*. The percentage alone stops
        // the arc reading as a stray shape, but "41%" beside a clock is still
        // ambiguous — of the day, of a battery, of a volume? — and §11 rejected
        // the reference's curved label as illegible at a 37 lu radius, so the
        // unit goes in the centre with the number.
        //
        // The string comes from `t!` rather than from `ClockData`, which cannot
        // gain a field without breaking `crate::clock`'s exhaustive literal.
        // It is the one user-visible word this toolkit owns; an untranslated
        // catalog degrades it to English, which is the i18n module's contract.
        surface::arc_gauge_with_label(c, fonts, area, d.day_fraction, t, crate::t!("Of day"));
    }
}

/// The card-less variant: no surface, so the outline is the only legibility
/// instrument there is (spec §4.5).
fn draw_bare(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &ClockData,
    l: &Layout,
    at: Rect,
) {
    let run = hero_run(d.time, d, l, t, fonts).color(t.text_primary);
    let m = fonts.measure(&run, c.scale());
    let origin = at.align(m.size(), HAlign::Center, VAlign::Middle).origin();
    surface::outlined_text(c, fonts, &run, origin, t);
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

    fn standard() -> ClockData<'static> {
        ClockData {
            time: "09:41",
            widest_time: "00:00",
            weekday: "Monday",
            date: "28 July",
            secondary: "Week 31 · GMT+05:30",
            font_size: 64.0,
            variant: ClockVariant::Auto,
            accent_follow: false,
            day_fraction: 0.4,
        }
    }

    #[test]
    fn the_variant_falls_out_of_the_type_size() {
        let mut d = standard();
        for (h, want) in [
            (12.0, ClockVariant::Bare),
            (23.9, ClockVariant::Bare),
            (24.0, ClockVariant::Compact),
            (34.9, ClockVariant::Compact),
            (35.0, ClockVariant::Standard),
            (64.0, ClockVariant::Standard),
        ] {
            d.font_size = h;
            assert_eq!(variant_for(&d), want, "H = {h}");
        }
        // An explicit variant always wins.
        d.variant = ClockVariant::Expanded;
        d.font_size = 12.0;
        assert_eq!(variant_for(&d), ClockVariant::Expanded);
    }

    #[test]
    fn the_worked_geometry_matches_the_spec_at_the_default_size() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let l = layout(&mut f, &t, &standard(), 1.0);
        // Spec §9.1: at H = 64, micro 11, secondary 14, pad 20, R 28.
        assert_eq!(l.micro, 11.0);
        assert_eq!(l.secondary, 14.0);
        assert_eq!(l.pad, 20.0);
        assert_eq!(l.radius, 28.0);
        // The vertical rhythm, to within the rounding the spec itself uses.
        assert!((l.y_micro_cap - 20.0).abs() < 0.01, "{}", l.y_micro_cap);
        assert!((l.y_hero_cap - 42.1).abs() < 0.2, "{}", l.y_hero_cap);
        assert!((l.y_sec_cap - 100.8).abs() < 0.4, "{}", l.y_sec_cap);
        assert!((l.size.h - 132.0).abs() < 1.5, "height {}", l.size.h);
        // Width has a floor of 3.1 H even when the strings are short.
        assert!(l.size.w >= 3.1 * 64.0 - 0.01, "width {}", l.size.w);
    }

    #[test]
    fn the_card_is_sized_from_the_widest_reachable_string_not_the_current_one() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        // Same settings, different instants: the width must not move.
        let base = ClockData {
            widest_time: "00:00:00 PM",
            ..standard()
        };
        let a = layout(
            &mut f,
            &t,
            &ClockData {
                time: "1:11:11 AM",
                ..base
            },
            1.0,
        );
        let b = layout(
            &mut f,
            &t,
            &ClockData {
                time: "12:48:08 PM",
                ..base
            },
            1.0,
        );
        assert_eq!(a.size, b.size, "the card resized as the clock ticked");
        // And a longer reachable string does make it wider, or the setting
        // would be doing nothing.
        let narrow = layout(
            &mut f,
            &t,
            &ClockData {
                widest_time: "00:00",
                ..base
            },
            1.0,
        );
        assert!(narrow.size.w <= a.size.w);
    }

    #[test]
    fn missing_rows_shrink_the_card_rather_than_leaving_a_band() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let full = layout(&mut f, &t, &standard(), 1.0);
        let no_sec = layout(
            &mut f,
            &t,
            &ClockData {
                secondary: "",
                ..standard()
            },
            1.0,
        );
        assert!(no_sec.size.h < full.size.h, "an empty row left a band");
        // Dropping the date keeps the weekday: the row is never empty.
        assert_eq!(
            micro_text(&ClockData {
                date: "",
                ..standard()
            }),
            "Monday"
        );
        assert_eq!(
            micro_text(&ClockData {
                weekday: "",
                ..standard()
            }),
            "28 July"
        );
        assert_eq!(
            micro_text(&ClockData {
                weekday: "",
                date: "",
                ..standard()
            }),
            ""
        );
    }

    #[test]
    fn accent_ink_is_only_used_where_it_is_large_enough_to_be_legal() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let d = ClockData {
            accent_follow: true,
            font_size: 64.0,
            ..standard()
        };
        let l = layout(&mut f, &t, &d, 1.0);
        assert_eq!(hero_run("09:41", &d, &l, &t, &mut f).color, t.accent_ink);
        // Below the floor it falls back to primary rather than drawing
        // small accent-coloured type.
        let small = ClockData {
            font_size: 18.0,
            ..d
        };
        let ls = layout(&mut f, &t, &small, 1.0);
        assert_eq!(
            hero_run("09:41", &small, &ls, &t, &mut f).color,
            t.text_primary
        );
    }

    #[test]
    fn the_gauge_column_is_dropped_when_the_card_is_too_narrow_for_it() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let big = layout(
            &mut f,
            &t,
            &ClockData {
                variant: ClockVariant::Expanded,
                font_size: 64.0,
                ..standard()
            },
            1.0,
        );
        assert!(big.gauge.is_some(), "a 64 lu Expanded card should gauge");
        let small = layout(
            &mut f,
            &t,
            &ClockData {
                variant: ClockVariant::Expanded,
                font_size: 36.0,
                weekday: "",
                date: "",
                secondary: "",
                widest_time: "0:00",
                ..standard()
            },
            1.0,
        );
        assert!(small.gauge.is_none(), "a narrow card kept its gauge");
    }

    #[test]
    fn no_combination_of_settings_can_panic() {
        let mut f = fonts();
        let mut c = Canvas::for_logical(Size::new(200.0, 150.0), 1.0).unwrap();
        let strings = [
            ("", "", "", ""),
            ("09:41", "Monday", "28 July", "Week 31 · GMT+05:30"),
            ("零九:四一", "星期一", "七月二十八日", "第三十一周"),
            (
                "a very long time string that will never fit anywhere at all",
                "Wednesday",
                "31 December",
                "a secondary line that also does not fit",
            ),
            ("🎵", "🎶", "🎼", "🎹"),
        ];
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            for variant in [
                ClockVariant::Auto,
                ClockVariant::Bare,
                ClockVariant::Compact,
                ClockVariant::Standard,
                ClockVariant::Expanded,
            ] {
                for size in [f32::NAN, 0.0, 1.0, 64.0, 300.0] {
                    for (time, week, date, sec) in strings {
                        let d = ClockData {
                            time,
                            widest_time: time,
                            weekday: week,
                            date,
                            secondary: sec,
                            font_size: size,
                            variant,
                            accent_follow: true,
                            day_fraction: f32::NAN,
                        };
                        let m = measure(&mut f, &t, &d, 1.0);
                        assert!(m.buffer().w.is_finite() && m.buffer().h.is_finite());
                        c.reset();
                        draw_at(&mut c, &mut f, &t, &d, Rect::new(16.0, 16.0, 160.0, 110.0));
                        draw_at(
                            &mut c,
                            &mut f,
                            &t,
                            &d,
                            Rect::new(-50.0, -50.0, 200.0, 100.0),
                        );
                        draw_at(&mut c, &mut f, &t, &d, Rect::ZERO);
                    }
                }
            }
            c.reset();
            draw(&mut c, &mut f, &t, &standard());
        }
    }
}

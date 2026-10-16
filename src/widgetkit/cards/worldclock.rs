//! The **world clock**: a few cities, each as `Tokyo  9:41 PM`, with no card.
//!
//! ```text
//!   Tokyo         9:41 PM       ← label: small, medium weight, left
//!   New York      8:41 AM       ← time: large, bold, right-aligned
//!   London       12:41 AM
//! ```
//!
//! # Legibility without a ground
//!
//! No card, scrim or well — the glyphs sit directly on the wallpaper (or, for
//! the floating overlay, on whatever window is underneath), and one soft
//! Gaussian shadow cast by all the text is the only instrument, the same as
//! [`super::lock`]. The shadow is tuned tighter and darker than the lock
//! screen's because this type is a quarter of the size.
//!
//! # Columns that do not move
//!
//! Labels are left-aligned and times right-aligned to the block's edge, so the
//! minutes line up down the right side whatever the digits' widths. The time
//! column is sized from the widest string a 12-hour clock can show, not from
//! the current rows, so the block does not resize once a minute and drift the
//! labels. Inter's default digits are proportional and [`TextRun`] has no
//! `tnum` switch; right alignment is what makes that invisible.
//!
//! # Two callers
//!
//! The wallpaper widget layer uses [`measure`] / [`draw_at`] like every other
//! card. The always-on-top overlays have no mpv to hand pixels to and want a
//! bare pixmap: [`render_pixmap`] and [`to_bgra_premul`] are theirs.

use tiny_skia::Pixmap;

use crate::widgetkit::canvas::{to_bgra_in_place, Canvas};
use crate::widgetkit::color::Color;
use crate::widgetkit::geom::{Point, Rect, Size};
use crate::widgetkit::surface::WidgetSize;
use crate::widgetkit::text::{FontStack, TextRun};
use crate::widgetkit::theme::{Mode, Theme};
use crate::widgetkit::typo::{self, Script};
use crate::worldclock::Row;

/// What the card draws: plain data, like [`super::ClockData`].
#[derive(Debug, Clone, Copy)]
pub struct WorldClockData<'a> {
    /// One line each, top to bottom. Empty draws nothing.
    pub rows: &'a [Row],
    /// Time size in logical units. `config::WorldClock::font_size_pt`.
    pub font_size: f32,
}

const TIME_WEIGHT: u16 = 700;
const LABEL_WEIGHT: u16 = 500;
/// The label's size as a fraction of the time's, snapped to the type ladder.
const LABEL_RATIO: f32 = 0.62;
const MIN_LABEL: f32 = 11.0;
/// Clear space between one row's baseline and the next row's cap top, as a
/// fraction of the time's size.
const ROW_GAP: f32 = 0.55;
/// Space between the label and time columns, as a fraction of the time's size.
const COL_GAP: f32 = 0.9;
/// The label column never grows past this many label-sizes; a longer label is
/// ellipsised instead of stretching the block across the screen.
const LABEL_MAX_EM: f32 = 12.0;
/// The widest 12-hour times: `0` is the widest figure and `AM`/`PM` differ.
const WIDEST_TIMES: [&str; 2] = ["00:00 AM", "00:00 PM"];

#[derive(Debug, Clone, Copy)]
struct Layout {
    time: f32,
    label: f32,
    /// Cap top to cap top of consecutive rows.
    pitch: f32,
    /// The time's cap top to its baseline.
    cap: f32,
    label_w: f32,
    size: Size,
    blur: f32,
    dy: f32,
}

fn time_size(d: &WorldClockData) -> f32 {
    if d.font_size.is_finite() && d.font_size > 0.0 {
        d.font_size.clamp(8.0, 200.0)
    } else {
        22.0
    }
}

/// Blur radius and drop of the shadow for a time of size `time`: relatively
/// larger than the lock screen's, because small type needs a thicker halo to
/// survive a busy wallpaper.
fn shadow(time: f32) -> (f32, f32) {
    (
        (0.18 * time).clamp(3.0, 14.0),
        (0.06 * time).clamp(1.0, 3.0),
    )
}

fn shadow_colour(t: &Theme) -> Color {
    if t.mode.is_dark() {
        Color::BLACK.with_alpha(0.6)
    } else {
        Color::WHITE.with_alpha(0.7)
    }
}

fn time_run(text: &str, size: f32, t: &Theme, fonts: &mut FontStack) -> TextRun {
    typo::styled(text, size, TIME_WEIGHT, false, fonts).color(t.text_primary)
}

fn label_run(text: &str, size: f32, t: &Theme, fonts: &mut FontStack) -> TextRun {
    typo::styled(text, size, LABEL_WEIGHT, false, fonts).color(t.text_primary)
}

/// Shared by `measure` and `draw_at` so the two cannot disagree.
fn layout(fonts: &mut FontStack, t: &Theme, d: &WorldClockData, scale: f32) -> Layout {
    let time = time_size(d);
    let label = typo::nearest_ladder_step(LABEL_RATIO * time)
        .max(MIN_LABEL)
        .min(time);
    let cap = typo::cap_height(time, Script::Latin);
    let pitch = cap + ROW_GAP * time;
    let (blur, dy) = shadow(time);
    let mut l = Layout {
        time,
        label,
        pitch,
        cap,
        label_w: 0.0,
        size: Size::ZERO,
        blur,
        dy,
    };
    if d.rows.is_empty() {
        return l;
    }

    let mut label_w: f32 = 0.0;
    for r in d.rows {
        let run = label_run(&r.label, label, t, fonts);
        label_w = label_w.max(fonts.measure(&run, scale).width);
    }
    l.label_w = label_w.min(LABEL_MAX_EM * label).ceil();

    let mut time_w: f32 = 0.0;
    let times = WIDEST_TIMES
        .into_iter()
        .chain(d.rows.iter().map(|r| r.time.as_str()));
    for s in times {
        let run = time_run(s, time, t, fonts);
        time_w = time_w.max(fonts.measure(&run, scale).width);
    }

    // The last row's label hangs its descenders below the shared baseline.
    let h = (d.rows.len() - 1) as f32 * pitch + cap + typo::descender(label, Script::Latin);
    l.size = Size::new((l.label_w + COL_GAP * time + time_w).ceil(), h.ceil());
    l
}

/// How big the block is, and how much room its shadow needs around it.
pub fn measure(fonts: &mut FontStack, t: &Theme, d: &WorldClockData, scale: f32) -> WidgetSize {
    let l = layout(fonts, t, d, scale);
    WidgetSize {
        card: l.size,
        // Same reach as `Shadow::bleed`: the blur's visible tail plus the drop.
        bleed: (l.blur * 1.5 + l.dy).ceil(),
    }
}

/// Draw the block centred in whatever room `canvas` provides.
pub fn draw(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &WorldClockData) {
    let size = measure(fonts, t, d, c.scale());
    let rect = size.card_in(c.bounds());
    draw_at(c, fonts, t, d, rect);
}

/// Draw the block with its text box at `card`.
pub fn draw_at(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &WorldClockData, card: Rect) {
    if card.is_empty() || d.rows.is_empty() || !fonts.has_fonts() {
        return;
    }
    let scale = c.scale();
    let l = layout(fonts, t, d, scale);

    let mut runs: Vec<(TextRun, Point)> = Vec::with_capacity(d.rows.len() * 2);
    for (i, row) in d.rows.iter().enumerate() {
        let cap_top = card.y + i as f32 * l.pitch;
        let baseline = cap_top + l.cap;

        // Time: right-aligned to the box, cap top on the row's line.
        let run = time_run(&row.time, l.time, t, fonts);
        let w = fonts.measure(&run, scale).width;
        let y = cap_top - typo::cap_gap(l.time, Script::Latin);
        runs.push((run, Point::new(card.right() - w, y)));

        // Label: left-aligned, sitting on the time's baseline.
        let s = Script::of(&row.label);
        let run = label_run(&row.label, l.label, t, fonts).max_width(l.label_w);
        let y = baseline - typo::cap_height(l.label, s) - typo::cap_gap(l.label, s);
        runs.push((run, Point::new(card.x, y)));
    }

    // One shadow for every row: one blur, no darker patch where they overlap.
    let casts: Vec<(&TextRun, Point)> = runs.iter().map(|(run, at)| (run, *at)).collect();
    c.text_shadow(fonts, &casts, l.blur, l.dy, shadow_colour(t));
    for (run, at) in &runs {
        c.text(fonts, run, *at);
    }
}

/// The world clock as a bare pixmap for the floating overlays: just the text and
/// its shadow on a fully transparent ground, sized to fit.
///
/// `font_size_pt` is `config::WorldClock::font_size_pt`; `scale` is
/// [`crate::widgetkit::scale_for_output`] of the output it floats over. Always
/// drawn in the dark palette (white ink, dark shadow): an overlay sits over
/// arbitrary windows, and the dark palette is the one fitted for the worst-case
/// bright backdrop.
///
/// The pixmap includes the shadow's margin ([`bleed_px`] on each side), so the
/// glyphs sit that far inside it; subtract it from `margin_px` for exact parity
/// with the widget layer, or ignore it (a few pixels).
///
/// The pixels are **premultiplied RGBA**. X11 and Wayland `ARGB8888` on a
/// little-endian machine is **BGRA** in memory, so convert with
/// [`to_bgra_premul`] before handing it to either.
///
/// `None` when there is nothing to draw: no rows, no fonts installed, or a
/// size past the canvas caps. Build `fonts` once with [`FontStack::system`] —
/// it scans the filesystem — and reuse it; it is also the glyph cache.
pub fn render_pixmap(
    fonts: &mut FontStack,
    rows: &[Row],
    font_size_pt: u32,
    scale: f32,
) -> Option<Pixmap> {
    if rows.is_empty() || !fonts.has_fonts() {
        return None;
    }
    let theme = Theme::for_accent(Mode::Dark, crate::config::Accent::Blue);
    let data = WorldClockData {
        rows,
        font_size: font_size_pt as f32,
    };
    let size = measure(fonts, &theme, &data, scale);
    let mut canvas = Canvas::for_logical(size.buffer(), scale).ok()?;
    draw_at(&mut canvas, fonts, &theme, &data, size.card_rect());
    Some(canvas.into_pixmap())
}

/// The shadow margin, in device pixels, that [`render_pixmap`] adds on every side.
pub fn bleed_px(font_size_pt: u32, scale: f32) -> u32 {
    let (blur, dy) = shadow(time_size(&WorldClockData {
        rows: &[],
        font_size: font_size_pt as f32,
    }));
    ((blur * 1.5 + dy).ceil() * scale).ceil() as u32
}

/// `p`'s pixels as premultiplied **BGRA** — the byte order of little-endian
/// `ARGB8888`, which is what both X11 (depth-32 visuals) and `wl_shm` want.
/// tiny-skia stores premultiplied RGBA, so this swaps R and B per pixel.
pub fn to_bgra_premul(p: &Pixmap) -> Vec<u8> {
    let mut out = p.data().to_vec();
    to_bgra_in_place(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row> {
        vec![
            Row {
                label: "Tokyo".into(),
                time: "9:41 PM".into(),
            },
            Row {
                label: "New York".into(),
                time: "8:41 AM".into(),
            },
        ]
    }

    fn alpha_at(p: &Pixmap, x: u32, y: u32) -> u8 {
        p.pixels()[(y * p.width() + x) as usize].alpha()
    }

    #[test]
    fn a_two_row_pixmap_has_text_and_a_transparent_ground() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let p = render_pixmap(&mut f, &rows(), 22, 1.0).expect("two rows must render");
        assert!(p.width() > 0 && p.height() > 0);
        assert!(
            p.pixels().iter().any(|px| px.alpha() > 0),
            "no glyph was drawn"
        );
        let (w, h) = (p.width() - 1, p.height() - 1);
        for (x, y) in [(0, 0), (w, 0), (0, h), (w, h)] {
            assert_eq!(
                alpha_at(&p, x, y),
                0,
                "corner ({x},{y}) must be transparent"
            );
        }
        assert!(render_pixmap(&mut f, &[], 22, 1.0).is_none());
    }

    #[test]
    fn the_box_does_not_follow_the_digits() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = Theme::for_accent(Mode::Dark, crate::config::Accent::Blue);
        let mut at = |time: &str| {
            let rows = vec![Row {
                label: "Tokyo".into(),
                time: time.into(),
            }];
            measure(
                &mut f,
                &t,
                &WorldClockData {
                    rows: &rows,
                    font_size: 22.0,
                },
                1.0,
            )
        };
        let (a, b) = (at("11:11 AM"), at("10:08 PM"));
        assert_eq!(a, b);
    }

    #[test]
    fn bleed_px_matches_what_render_pixmap_adds() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = Theme::for_accent(Mode::Dark, crate::config::Accent::Blue);
        let r = rows();
        let d = WorldClockData {
            rows: &r,
            font_size: 22.0,
        };
        let size = measure(&mut f, &t, &d, 1.0);
        let p = render_pixmap(&mut f, &r, 22, 1.0).unwrap();
        assert_eq!(
            p.width(),
            (size.card.w + 2.0 * bleed_px(22, 1.0) as f32) as u32
        );
    }

    #[test]
    fn bgra_swaps_red_and_blue_only() {
        let mut p = Pixmap::new(1, 1).unwrap();
        p.pixels_mut()[0] = tiny_skia::PremultipliedColorU8::from_rgba(10, 20, 30, 255).unwrap();
        assert_eq!(to_bgra_premul(&p), vec![30, 20, 10, 255]);
    }
}

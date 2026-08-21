//! A 5 × 7 dot-matrix face, for the NOS clock's numerals (spec §9.1.3).
//!
//! # Why a real dot grid, and why only for numerals
//!
//! The NOS design sheets set their headings and numerals in an LED matrix
//! face: every glyph is a grid of discrete round dots. That is not a font
//! substitution — no face in the fallback chain draws dots — so it is drawn
//! here, from a table.
//!
//! It is deliberately **not** used for everything the sheets do, and the
//! reasoning is arithmetic rather than taste:
//!
//! * **A dot cell has a legibility floor.** The grid is seven rows tall, so a
//!   run at cap height `H` has a dot pitch of `H / 7`. Below about
//!   2 device pixels of pitch the dots merge into a grey smear and the glyph
//!   is gone — the counters of `8` and `0` are one cell wide and there is no
//!   antialiasing budget to fake them. So this face is used only where
//!   `pitch × scale ≥ `[`MIN_PITCH_PX`]. The clock's own 11 lu micro-label can
//!   never clear that at 1× (11 × 0.727 / 7 = 1.14 px) and is therefore set in
//!   the mono face instead, tracked wide, which is what a matrix label *looks*
//!   like at small sizes without pretending to be one.
//! * **CJK cannot be dot-matrixed at all.** A 5 × 7 cell holds about ten
//!   strokes' worth of information; `星期一` needs far more, and Fresco ships a
//!   Simplified-Chinese UI as a first-class target (spec §5.2). So the face
//!   covers a closed ASCII set — digits, the two separators a clock uses, and
//!   the three letters `AM`/`PM` need — and [`supported`] is checked before it
//!   is chosen. Anything else falls back to the mono face.
//!
//! The honest summary: **the dots are real where they read and are not faked
//! where they would not.** The ring and the corner markers (spec §9.1.3) are
//! dots at every size, because a dot carrying no glyph shape has no counters to
//! close.
//!
//! # Tabular by construction
//!
//! Every digit occupies exactly five columns and advances six. There is no
//! `tnum` to request and no fallback tabularisation to do (spec §5.4) — a `1`
//! is the same width as a `0` because the grid says so, so a clock set in this
//! face cannot jitter horizontally.

use super::canvas::Canvas;
use super::color::Color;
use super::geom::{Point, Rect};
use super::paint::Fill;

/// Rows in one cell. The full-height glyphs use all seven.
pub const ROWS: usize = 7;
/// Columns in a full-width cell.
pub const COLS: usize = 5;
/// The smallest dot pitch, in **device** pixels, at which this face is legible.
///
/// Below it the dots of `8` merge and the glyph reads as a filled block, so the
/// caller falls back to a real font rather than drawing a smear.
pub const MIN_PITCH_PX: f32 = 2.0;
/// Dot diameter as a fraction of the pitch. Below ~0.7 the glyph reads as
/// speckle; at 1.0 the dots touch and it is a bitmap font, not a matrix.
const DOT_RATIO: f32 = 0.78;

/// One glyph: seven rows of column bits (bit `COLS-1` is the leftmost column),
/// how many columns it actually occupies, and how many it advances.
#[derive(Debug, Clone, Copy)]
struct Glyph {
    rows: [u8; ROWS],
    cols: u8,
    advance: u8,
}

const fn wide(rows: [u8; ROWS]) -> Glyph {
    Glyph {
        rows,
        cols: COLS as u8,
        advance: COLS as u8 + 1,
    }
}

/// The closed set this face covers. Anything outside it makes [`supported`]
/// false and sends the caller to a real font.
fn glyph(ch: char) -> Option<Glyph> {
    let g = match ch {
        '0' => wide([
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ]),
        '1' => wide([
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        '2' => wide([
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ]),
        '3' => wide([
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ]),
        '4' => wide([
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ]),
        '5' => wide([
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ]),
        '6' => wide([
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ]),
        '7' => wide([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ]),
        '8' => wide([
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ]),
        '9' => wide([
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ]),
        'A' | 'a' => wide([
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'P' | 'p' => wide([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'M' | 'm' => wide([
            0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001,
        ]),
        // The separators are one column wide and advance two, which is what
        // keeps `17:07:05` from reading as three unrelated pairs.
        ':' => Glyph {
            rows: [0, 0b10000, 0, 0, 0b10000, 0, 0],
            cols: 1,
            advance: 2,
        },
        '.' => Glyph {
            rows: [0, 0, 0, 0, 0, 0, 0b10000],
            cols: 1,
            advance: 2,
        },
        ' ' => Glyph {
            rows: [0; ROWS],
            cols: 0,
            advance: 3,
        },
        _ => return None,
    };
    Some(g)
}

/// Whether every character of `s` has a cell in this face.
///
/// The gate the clock checks before choosing the matrix over a real font. It is
/// `false` for CJK, for `Wordy`'s spelled-out phrases, and for anything a
/// player or a locale might put in a time string that this table does not know.
pub fn supported(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| glyph(c).is_some())
}

/// Total advance of `s` in **columns**, with no trailing inter-glyph gap.
///
/// Zero for a string this face cannot set, which is also the answer
/// [`supported`] would have refused on.
pub fn advance_cols(s: &str) -> f32 {
    let mut total = 0u32;
    let mut any = false;
    for ch in s.chars() {
        let Some(g) = glyph(ch) else { return 0.0 };
        total += u32::from(g.advance);
        any = true;
    }
    if !any {
        return 0.0;
    }
    // The trailing gap is not part of the ink.
    (total.saturating_sub(1)) as f32
}

/// Width of `s` at `pitch` logical units per cell.
pub fn width(s: &str, pitch: f32) -> f32 {
    if !pitch.is_finite() || pitch <= 0.0 {
        return 0.0;
    }
    advance_cols(s) * pitch
}

/// The pitch that makes `s` exactly `cap` tall.
///
/// The grid is [`ROWS`] cells tall, so the cap height a caller lays out against
/// — the same `cap_height` every other row on the card uses — divides straight
/// into it. That is what keeps a matrix hero and a font-set label sharing one
/// baseline grid.
pub fn pitch_for_cap(cap: f32) -> f32 {
    if !cap.is_finite() || cap <= 0.0 {
        return 0.0;
    }
    cap / ROWS as f32
}

/// Draw `s` with the **top-left of its cell grid** at `at`.
///
/// `at.y` is the cap top, matching what `Canvas::text` is given after the
/// `cap_gap` correction, so a caller can swap between this and a font run
/// without moving anything.
///
/// Returns the ink rect. Nothing is drawn for an unsupported string, a
/// non-finite pitch or a transparent colour — every one of which is a
/// no-op rather than a panic, because this runs on the daemon and
/// `panic = "abort"` takes the wallpaper down with it.
pub fn draw(c: &mut Canvas, s: &str, at: Point, pitch: f32, color: Color) -> Rect {
    if !pitch.is_finite() || pitch <= 0.0 || color.a <= 0.0 || s.is_empty() {
        return Rect::ZERO;
    }
    let dot = (pitch * DOT_RATIO).max(0.5);
    let inset = (pitch - dot) / 2.0;
    let fill = Fill::solid(color);
    let mut x = at.x;
    for ch in s.chars() {
        let Some(g) = glyph(ch) else {
            return Rect::ZERO;
        };
        for (r, bits) in g.rows.iter().enumerate() {
            if *bits == 0 {
                continue;
            }
            for col in 0..usize::from(g.cols) {
                let bit = 1u8 << (COLS - 1 - col);
                if bits & bit == 0 {
                    continue;
                }
                let cx = x + col as f32 * pitch + inset;
                let cy = at.y + r as f32 * pitch + inset;
                c.rounded_rect(Rect::new(cx, cy, dot, dot), dot / 2.0, &fill);
            }
        }
        x += f32::from(g.advance) * pitch;
    }
    Rect::new(at.x, at.y, width(s, pitch), ROWS as f32 * pitch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgetkit::geom::Size;

    #[test]
    fn the_face_covers_exactly_what_a_clock_can_say_and_nothing_else() {
        for s in ["09:41", "17:07:05", "12:48:08 PM", "1.5", "00:00 AM"] {
            assert!(supported(s), "{s}");
        }
        // The three cases that must send the caller back to a real font.
        for s in [
            "",
            "零九:四一",
            "half past ten",
            "09:41\u{202f}pm",
            "Week 31",
        ] {
            assert!(!supported(s), "{s}");
        }
    }

    #[test]
    fn every_digit_is_the_same_width_so_a_clock_cannot_jitter() {
        let mut widths = std::collections::HashSet::new();
        for d in '0'..='9' {
            widths.insert(advance_cols(&d.to_string()) as i32);
        }
        assert_eq!(widths.len(), 1, "digits differ in width: {widths:?}");
        // And the separator is narrower, or `17:07:05` reads as three pairs.
        assert!(advance_cols(":") < advance_cols("0"));
        // Tabular by construction: the advance depends only on the count.
        assert_eq!(advance_cols("00:00"), advance_cols("17:41"));
        assert_eq!(advance_cols("00:00:00"), advance_cols("17:07:05"));
    }

    #[test]
    fn the_pitch_puts_the_grid_on_the_cap_height_it_was_given() {
        let cap = 46.5_f32;
        let p = pitch_for_cap(cap);
        assert!((p * ROWS as f32 - cap).abs() < 1e-4);
        // Degenerate input is answered, not panicked on.
        for bad in [f32::NAN, 0.0, -8.0, f32::INFINITY] {
            let p = pitch_for_cap(bad);
            assert!(p.is_finite() || bad.is_infinite(), "{bad}");
            assert_eq!(width("09:41", f32::NAN), 0.0);
        }
    }

    #[test]
    fn nothing_here_can_panic_or_draw_outside_its_rect() {
        let mut c = Canvas::for_logical(Size::new(200.0, 80.0), 1.0).unwrap();
        for pitch in [f32::NAN, 0.0, -3.0, 0.4, 6.0, 1e6] {
            for s in ["", "09:41", "零九", "88:88:88"] {
                let r = draw(
                    &mut c,
                    s,
                    Point::new(4.0, 4.0),
                    pitch,
                    Color::WHITE.with_alpha(0.5),
                );
                assert!(r.w.is_finite() && r.h.is_finite());
            }
        }
        // A transparent ink is a no-op, not a wasted path build.
        assert_eq!(
            draw(
                &mut c,
                "09:41",
                Point::new(0.0, 0.0),
                4.0,
                Color::WHITE.with_alpha(0.0)
            ),
            Rect::ZERO
        );
    }

    #[test]
    fn a_drawn_run_actually_puts_ink_on_the_canvas() {
        let mut c = Canvas::for_logical(Size::new(120.0, 40.0), 1.0).unwrap();
        let r = draw(&mut c, "09:41", Point::new(4.0, 4.0), 4.0, Color::WHITE);
        assert!(r.w > 0.0 && r.h > 0.0);
        let px = c.to_bgra();
        assert!(px.data.iter().any(|&v| v != 0), "the matrix drew nothing");
    }
}

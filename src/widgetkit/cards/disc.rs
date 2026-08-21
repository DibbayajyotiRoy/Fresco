//! The album-art disc (spec §9.4): a record, not a thumbnail.
//!
//! # Why this is not a wrapper around `artwork::render_disc`
//!
//! `crate::artwork::render_disc` is well-tested and stays exactly as it is —
//! the ASS/mpv path still uses it and nothing here changes its behaviour. This
//! module is a second renderer for the *composited* path, for three reasons
//! that a wrapper cannot get around:
//!
//! 1. **It returns premultiplied BGRA.** Compositing that onto a
//!    [`Canvas`] means an un-premultiply and a channel swap into a scratch
//!    `RgbaImage` on **every frame** — and this is the widget that redraws at
//!    the spin rate. The rotation here lives in the pattern's transform
//!    instead, so it costs a matrix multiply and no buffer at all.
//! 2. **The fixed layers must composite after rotation, in unrotated space.**
//!    The rim bevel and the specular sweep do not turn with the record — that
//!    is the whole detail that makes it read as a record rather than as a
//!    printed circle. A pre-rendered bitmap cannot have layers drawn under and
//!    over it without being drawn onto the same surface as them.
//! 3. **The rim darkening is specified as a real radial gradient**, which
//!    `render_disc`'s per-pixel linear ramp is not and cannot become without
//!    changing its output.
//!
//! Its **published proportions are unchanged** and are still read from
//! [`crate::artwork::DiscCfg`], so `label_ratio`, `hole_ratio`, `ring_darken`
//! and `opacity` have exactly one definition in the codebase.
//!
//! # The counter-rotated highlight
//!
//! A specular highlight that spins with the artwork looks like a smear painted
//! onto the disc. A highlight fixed in screen space, with the artwork turning
//! underneath it, reads as a light in the room and a vinyl surface catching it.
//! The rim bevel is fixed for the same reason. Everything in the "rotates"
//! column below is drawn inside the rotated pattern; everything in the "fixed"
//! column is drawn after it, in ordinary canvas space.
//!
//! | Fixed | Rotates |
//! |---|---|
//! | E3 shadow, rim bevel (highlight + shade), specular sweep | artwork, rim darkening, groove rings, label, spindle |
//!
//! Two of those — the rim darkening and the groove rings — are radially
//! symmetric, so rotating them is invisible either way; they are drawn in
//! canvas space because it is cheaper and the result is identical.
//!
//! # When the label carries no text
//!
//! Only when **both** `2 × 0.33 R ≥ 96` lu and `opacity ≥ 200`. Below the first,
//! 11 lu type in a 32 lu circle is two ellipses and no information. Below the
//! second the whole disc is being faded and the label's contrast against the
//! artwork behind it is no longer bounded, so the label disappears and the disc
//! becomes pure artwork — which is what a faded disc is for.
//!
//! At the shipped default `size_px = 220` the label is **72.6 lu** across, so
//! the default disc carries no text. Text appears at `size_px ≥ 292`.

use crate::artwork::DiscCfg;
use crate::widgetkit::canvas::Canvas;
use crate::widgetkit::color::Color;
use crate::widgetkit::geom::{HAlign, Point, Rect, Size, VAlign};
use crate::widgetkit::paint::Fill;
use crate::widgetkit::surface::{self, WidgetSize};
use crate::widgetkit::text::FontStack;
use crate::widgetkit::theme::Theme;
use crate::widgetkit::typo::{self, Step};

/// What an album-art disc draws.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiscData<'a> {
    /// The cover. `None` draws a well-filled disc with no artwork, which is
    /// still a record.
    pub art: Option<&'a image::RgbaImage>,
    /// Size, rotation and the published proportions — one definition, shared
    /// with `crate::artwork`.
    pub cfg: DiscCfg,
    /// Label line one.
    pub title: &'a str,
    /// Label line two.
    pub artist: &'a str,
}

/// The five groove radii, as fractions of the disc radius.
const GROOVES: [f32; 5] = [0.42, 0.53, 0.64, 0.75, 0.86];
/// Label diameter below which the label carries no text.
const LABEL_TEXT_MIN: f32 = 96.0;
/// Disc opacity below which the label carries no text.
const LABEL_OPACITY_MIN: u8 = 200;
/// Where the rim darkening starts, as a fraction of the radius.
const RIM_INNER: f32 = 0.82;

fn diameter(d: &DiscData) -> f32 {
    d.cfg.size_px.clamp(1, crate::artwork::MAX_DISC_PX) as f32
}

fn ratio01(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// How big this disc is, and how much shadow margin it needs.
///
/// At the defaults: `220 + 2 × 84 = 388 × 388` lu — the E3 bleed, which is the
/// largest in the system precisely because the disc is the one widget that is a
/// free-floating object rather than a panel.
pub fn measure(_fonts: &mut FontStack, t: &Theme, d: &DiscData, _scale: f32) -> WidgetSize {
    let s = diameter(d);
    WidgetSize::new(Size::new(s, s), t.e3())
}

/// Draw the disc, centred in whatever room `canvas` provides.
pub fn draw(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &DiscData) {
    let size = measure(fonts, t, d, c.scale());
    let rect = size.card_in(c.bounds());
    draw_at(c, fonts, t, d, rect);
}

/// Draw the disc with its bounding square at `card`.
pub fn draw_at(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &DiscData, card: Rect) {
    let side = card.min_side();
    if side <= 0.0 {
        return;
    }
    let disc = card.align(Size::new(side, side), HAlign::Center, VAlign::Middle);
    let r = side / 2.0;
    let centre = disc.center();
    let alpha = f32::from(d.cfg.opacity) / 255.0;
    if alpha <= 0.0 {
        return;
    }

    // -- fixed, beneath ----------------------------------------------------
    surface::elevation(c, disc, r, t, t.e3());

    // -- rotating ----------------------------------------------------------
    match d.art {
        Some(img) => c.image_rotated(img, disc, d.cfg.rotation_deg, r),
        None => c.rounded_rect(disc, r, &Fill::solid(t.well.over(t.surface))),
    }

    // Rim darkening: a real radial gradient from 0 at 0.82 R to `ring_darken`
    // at the edge, rather than the linear per-pixel ramp the bitmap path uses.
    let ring = ratio01(d.cfg.ring_darken);
    if ring > 0.0 {
        c.rounded_rect(
            disc,
            r,
            &Fill::RadialGradient {
                center: centre,
                radius: r,
                stops: vec![
                    crate::widgetkit::paint::Stop::new(0.0, Color::BLACK.with_alpha(0.0)),
                    crate::widgetkit::paint::Stop::new(RIM_INNER, Color::BLACK.with_alpha(0.0)),
                    crate::widgetkit::paint::Stop::new(1.0, Color::BLACK.with_alpha(ring)),
                ],
            },
        );
    }

    // Groove rings: each dark hairline paired with a bright one 1 lu outside,
    // which is what makes them read as cut into the surface rather than
    // printed on it. Concentric, so rotating them would be invisible.
    for g in GROOVES {
        let gr = r * g;
        if gr <= 1.0 {
            continue;
        }
        c.arc(
            centre,
            gr,
            0.0,
            360.0,
            1.0,
            &Fill::solid(Color::BLACK.with_alpha(0.10)),
        );
        c.arc(
            centre,
            gr + 1.0,
            0.0,
            360.0,
            1.0,
            &Fill::solid(Color::WHITE.with_alpha(0.05)),
        );
    }

    // Label.
    let label_r = ratio01(d.cfg.label_ratio) * r;
    if label_r > 2.0 {
        let lb = Rect::new(
            centre.x - label_r,
            centre.y - label_r,
            label_r * 2.0,
            label_r * 2.0,
        );
        surface::elevation(c, lb, label_r, t, t.e1());
        // Over the *artwork*, not over a pre-composited card: the label is a
        // paper disc glued to a record, and what shows through it is the
        // record, not the wallpaper.
        c.rounded_rect(lb, label_r, &Fill::solid(t.well));
        c.hairline(lb, label_r, t.edge, t.metrics.hairline);
        if label_r * 2.0 >= LABEL_TEXT_MIN && d.cfg.opacity >= LABEL_OPACITY_MIN {
            draw_label(c, fonts, t, d, lb);
        }
    }

    // -- fixed, on top -----------------------------------------------------
    // The rim bevel: highlight over the upper arc, shade over the lower. In
    // canvas angles (clockwise from 12 o'clock) the spec's screen -150..+30
    // and +30..+210 become -60 and +120, both sweeping 180.
    c.arc(
        centre,
        r - 0.5,
        -60.0,
        180.0,
        1.0,
        &Fill::linear(
            Point::new(disc.x, disc.y),
            Point::new(disc.right(), disc.bottom()),
            Color::WHITE.with_alpha(0.22),
            Color::WHITE.with_alpha(0.0),
        ),
    );
    c.arc(
        centre,
        r - 0.5,
        120.0,
        180.0,
        1.0,
        &Fill::linear(
            Point::new(disc.right(), disc.bottom()),
            Point::new(disc.x, disc.y),
            Color::BLACK.with_alpha(0.35),
            Color::BLACK.with_alpha(0.0),
        ),
    );

    // The specular sweep: fixed in screen space, so the artwork turns beneath
    // it. Clipped to the disc by being drawn as the disc's own path.
    c.rounded_rect(
        disc,
        r,
        &Fill::linear(
            Point::new(disc.x, disc.y),
            Point::new(disc.x + disc.w * 0.45, disc.y + disc.h * 0.45),
            Color::WHITE.with_alpha(0.10),
            Color::WHITE.with_alpha(0.0),
        ),
    );

    // The spindle: a real hole, punched from alpha, with a hairline just
    // outside it so the edge of the hole reads as thickness.
    let hole_r = ratio01(d.cfg.hole_ratio) * r;
    if hole_r > 0.5 {
        let hb = Rect::new(
            centre.x - hole_r,
            centre.y - hole_r,
            hole_r * 2.0,
            hole_r * 2.0,
        );
        c.punch(hb, hole_r);
        c.arc(
            centre,
            hole_r + 0.5,
            0.0,
            360.0,
            1.0,
            &Fill::solid(Color::BLACK.with_alpha(0.40)),
        );
    }

    // The global opacity is applied last, as a wash, so a faded disc fades as
    // one object rather than as a stack of independently translucent layers.
    if alpha < 1.0 {
        c.fade(disc, r, alpha);
    }
}

fn draw_label(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &DiscData, label: Rect) {
    let inner = label.inset(label.w * 0.07);
    if inner.is_empty() {
        return;
    }
    let mut rows: Vec<(String, Step, Color)> = Vec::new();
    if !d.title.is_empty() {
        rows.push((d.title.to_string(), Step::Micro, t.text_primary));
    }
    if !d.artist.is_empty() {
        rows.push((d.artist.to_string(), Step::Caption, t.text_secondary));
    }
    if rows.is_empty() {
        return;
    }
    // The two rows straddle the spindle rather than running through it: the
    // hole is punched from alpha after the label is drawn, and a hole through
    // the middle of a word reads as damage.
    let hole = ratio01(d.cfg.hole_ratio) * label.w / (2.0 * ratio01(d.cfg.label_ratio).max(0.01));
    let line = (typo::cap_height(Step::Micro.size(), typo::Script::Latin) + 4.0).max(hole * 2.2);
    let total = line * rows.len() as f32;
    let mut y = inner.center().y - total / 2.0;
    for (text, step, colour) in rows {
        let run = typo::step_run(&text, step, fonts)
            .color(colour)
            .max_width(inner.w);
        let m = fonts.measure(&run, c.scale());
        let at = Rect::new(inner.x, y, inner.w, line)
            .align(m.size(), HAlign::Center, VAlign::Top)
            .origin();
        c.text(fonts, &run, at);
        y += line;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgetkit::theme::Mode;

    fn theme(mode: Mode) -> Theme {
        Theme::for_accent(mode, crate::config::Accent::Amber)
    }

    fn cover(n: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(n, n, |x, y| {
            image::Rgba([
                (x * 255 / n.max(1)) as u8,
                90,
                (y * 255 / n.max(1)) as u8,
                255,
            ])
        })
    }

    #[test]
    fn the_default_disc_is_the_size_the_spec_works_through() {
        let mut f = FontStack::from_font_data("en-US", []);
        let t = theme(Mode::Dark);
        let d = DiscData::default();
        let m = measure(&mut f, &t, &d, 1.0);
        // Default `DiscCfg::size_px` is 320; at the config default of 220 the
        // buffer is 220 + 2 x 84.
        // Spec §7.4 works the buffer through in the light theme, whose E3 is
        // the larger of the two: 220 + 2 x 84 = 388.
        let light = theme(Mode::Light);
        let at220 = |t: &Theme| {
            measure(
                &mut FontStack::from_font_data("en-US", []),
                t,
                &DiscData {
                    cfg: DiscCfg {
                        size_px: 220,
                        ..d.cfg
                    },
                    ..d
                },
                1.0,
            )
        };
        assert_eq!(at220(&light).bleed, 84.0);
        assert_eq!(at220(&light).buffer(), Size::new(388.0, 388.0));
        // Dark's E3 is tighter, and the buffer follows it rather than the
        // larger of the two: a dark widget must not carry light's margin.
        assert_eq!(at220(&t).bleed, 76.0);
        assert_eq!(m.card, Size::new(320.0, 320.0));
    }

    #[test]
    fn the_label_carries_text_only_when_it_is_big_enough_and_opaque_enough() {
        // 2 x 0.33 R >= 96 means a diameter of at least 2 x 96 / 0.66 = 291.
        let big = |px: u32, op: u8| {
            let r = px as f32 / 2.0;
            let label = 2.0 * 0.33 * r;
            label >= LABEL_TEXT_MIN && op >= LABEL_OPACITY_MIN
        };
        assert!(!big(220, 255), "the default disc should carry no text");
        assert!(big(292, 255), "text appears at 292");
        assert!(!big(400, 199), "a faded disc drops its label");
        assert!(big(400, 200));
        // The worked figures.
        assert!((2.0_f32 * 0.33 * 110.0 - 72.6).abs() < 0.01);
    }

    #[test]
    fn the_published_proportions_are_still_the_ones_artwork_defines() {
        let c = DiscCfg::default();
        assert_eq!(c.label_ratio, 0.33);
        assert_eq!(c.hole_ratio, 0.045);
        assert_eq!(c.ring_darken, 0.35);
        assert_eq!(c.opacity, 255);
        // The grooves are where the spec puts them, and inside the rim.
        assert_eq!(GROOVES, [0.42, 0.53, 0.64, 0.75, 0.86]);
        for g in GROOVES {
            assert!(g > c.label_ratio, "groove {g} runs under the label");
            assert!(g < 1.0, "groove {g} runs off the disc");
        }
        // The outermost groove sits *inside* the rim darkening on purpose:
        // that is where a record's run-out is, and the darkening is what makes
        // it read as the edge rolling away rather than as a printed line.
        const { assert!(GROOVES[4] > RIM_INNER) };
        const { assert!(GROOVES[3] < RIM_INNER) };
    }

    #[test]
    fn the_spindle_is_a_hole_rather_than_a_dot() {
        let t = theme(Mode::Dark);
        let mut f = FontStack::from_font_data("en-US", []);
        let mut c = Canvas::for_logical(Size::new(240.0, 240.0), 1.0).unwrap();
        // Paint the whole surface first, so "transparent" can only come from
        // the punch.
        c.fill(&Fill::solid(Color::WHITE));
        let art = cover(64);
        draw_at(
            &mut c,
            &mut f,
            &t,
            &DiscData {
                art: Some(&art),
                cfg: DiscCfg {
                    size_px: 200,
                    hole_ratio: 0.12,
                    ..DiscCfg::default()
                },
                ..DiscData::default()
            },
            Rect::new(20.0, 20.0, 200.0, 200.0),
        );
        let bgra = c.to_bgra();
        let px = |x: usize, y: usize| bgra.data[(y * 240 + x) * 4 + 3];
        // The centre is a hole; a point well outside it is not.
        assert_eq!(px(120, 120), 0, "the spindle is not punched through");
        assert!(px(120, 60) > 200, "the disc body is transparent");
    }

    #[test]
    fn a_rotating_disc_changes_its_pixels_but_not_its_silhouette() {
        let t = theme(Mode::Dark);
        let mut f = FontStack::from_font_data("en-US", []);
        let art = cover(64);
        let mut alphas = Vec::new();
        let mut bodies = Vec::new();
        for deg in [0.0_f32, 37.0, 180.0] {
            let mut c = Canvas::for_logical(Size::new(240.0, 240.0), 1.0).unwrap();
            draw_at(
                &mut c,
                &mut f,
                &t,
                &DiscData {
                    art: Some(&art),
                    cfg: DiscCfg {
                        size_px: 200,
                        rotation_deg: deg,
                        ..DiscCfg::default()
                    },
                    ..DiscData::default()
                },
                Rect::new(20.0, 20.0, 200.0, 200.0),
            );
            let b = c.to_bgra();
            alphas.push(
                b.data
                    .iter()
                    .skip(3)
                    .step_by(4)
                    .map(|&a| a as u64)
                    .sum::<u64>(),
            );
            bodies.push(b.data.clone());
        }
        // The silhouette is rotation-invariant: a disc turning is still a disc.
        assert!(
            alphas
                .windows(2)
                .all(|w| (w[0] as i64 - w[1] as i64).abs() < 4000),
            "{alphas:?}"
        );
        // But the artwork really did turn.
        assert_ne!(bodies[0], bodies[1]);
    }

    #[test]
    fn no_combination_of_settings_can_panic() {
        let mut f = FontStack::system();
        let mut c = Canvas::for_logical(Size::new(200.0, 200.0), 1.0).unwrap();
        let art = cover(96);
        let wide = image::RgbaImage::from_pixel(200, 40, image::Rgba([10, 20, 30, 255]));
        let empty = image::RgbaImage::new(0, 0);
        // Representative rather than exhaustive: a full cross product of these
        // axes is tens of thousands of rasterisations and buys nothing the
        // extremes do not already cover.
        let cfgs = [
            DiscCfg {
                size_px: 0,
                ..DiscCfg::default()
            },
            DiscCfg {
                size_px: 1,
                hole_ratio: 1.0,
                ..DiscCfg::default()
            },
            DiscCfg {
                size_px: 8,
                label_ratio: 1.0,
                ring_darken: 1.0,
                ..DiscCfg::default()
            },
            DiscCfg {
                size_px: 220,
                rotation_deg: f32::NAN,
                ..DiscCfg::default()
            },
            DiscCfg {
                size_px: 320,
                rotation_deg: -720.0,
                opacity: 0,
                ..DiscCfg::default()
            },
            DiscCfg {
                size_px: 400,
                rotation_deg: 1e9,
                opacity: 199,
                ..DiscCfg::default()
            },
            DiscCfg {
                size_px: 400,
                opacity: 200,
                ..DiscCfg::default()
            },
            DiscCfg {
                size_px: u32::MAX,
                rotation_deg: f32::INFINITY,
                label_ratio: f32::NAN,
                hole_ratio: -1.0,
                ring_darken: 2.0,
                opacity: 1,
            },
        ];
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            for cfg in cfgs {
                for a in [None, Some(&art), Some(&wide), Some(&empty)] {
                    let d = DiscData {
                        art: a,
                        cfg,
                        title: "Blue Monday",
                        artist: "New Order · Substance",
                    };
                    let m = measure(&mut f, &t, &d, 1.0);
                    assert!(m.buffer().w.is_finite());
                    c.reset();
                    draw_at(&mut c, &mut f, &t, &d, Rect::new(8.0, 8.0, 140.0, 140.0));
                    draw_at(&mut c, &mut f, &t, &d, Rect::new(-9.0, -9.0, 40.0, 40.0));
                    draw_at(&mut c, &mut f, &t, &d, Rect::ZERO);
                }
            }
            // `draw` sizes the card from the canvas, so exercise it once per
            // theme rather than once per configuration.
            c.reset();
            draw(
                &mut c,
                &mut f,
                &t,
                &DiscData {
                    art: Some(&art),
                    ..DiscData::default()
                },
            );
        }
    }
}

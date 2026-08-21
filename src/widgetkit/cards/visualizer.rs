//! The audio visualiser (spec §9.3), in three treatments.
//!
//! # Why the default is card-less
//!
//! `config::Visualizer::width_pct` ships at **60**, and a card 60% of the
//! screen wide is a slab. A slab is not what someone turns a live wallpaper on
//! for. So the panel treatment engages below 45% width and above it the card
//! and well are simply not drawn: the bars get a bottom gradient scrim and a
//! per-bar shadow instead, which is the only pair of instruments that works
//! with no surface at all.
//!
//! # Why the panel, and not the skeuomorphic chassis, is the panel default
//!
//! Four reasons, in order of weight.
//!
//! 1. **Fresco's widget layer has an empty input region.** The skeuomorphic
//!    reference is approximately 80% controls — five transport buttons, a knob,
//!    a volume slider, a 2 × 3 grid. Drawing a play button that cannot be
//!    pressed is an affordance that lies, and it generates support load,
//!    because someone *will* click it. Strip the reference of every control it
//!    cannot honour and what is left — an LCD readout, a spectrum, a progress
//!    bar, all sunk into bevelled wells — is the inset panel with an orange
//!    tint. That is exactly what [`VisualizerVariant::Chassis`] draws.
//! 2. **Cost.** This is the only widget that redraws every frame while audio
//!    plays. The panel is one well plus *n* bars, and the card, well, hairlines
//!    and shadow can be rasterised once and only the bar rectangle redrawn.
//!    The chassis is fourteen bevelled sub-surfaces.
//! 3. **Consistency.** The clock, now-playing and disc are all flat glass. One
//!    skeuomorphic slab among three is not a system, it is an accident.
//! 4. **Colour.** The chassis's identity is `#F5A623`, which is not one of
//!    Fresco's six accents and has to opt out of `accent_follow` — precedent
//!    exists (`LyricStylePreset::Karaoke`), but it is a cost.
//!
//! # `opacity` applies to the bars and never to the card
//!
//! Fading the card as well makes the panel vanish and the bars float, which
//! looks like a bug rather than like a setting.

use crate::widgetkit::canvas::Canvas;
use crate::widgetkit::color::Color;
use crate::widgetkit::geom::{Point, Rect, Size};
use crate::widgetkit::paint::Fill;
use crate::widgetkit::surface::{self, BarPaint, BarStyle, WidgetSize};
use crate::widgetkit::text::FontStack;
use crate::widgetkit::theme::{radius_nested, Theme};
use crate::widgetkit::typo::{self, Script, Step};

/// Which treatment to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualizerVariant {
    /// Panel below 45% screen width, bare above it.
    #[default]
    Auto,
    /// A glass card with the spectrum sunk into a well.
    Panel,
    /// Bars straight on the wallpaper, over a bottom gradient scrim.
    Bare,
    /// The opt-in skeuomorphic alternate: an opaque bevelled chassis with LCD
    /// readouts, in a fixed `#F5A623` that opts out of `accent_follow`.
    Chassis,
}

/// What a visualiser draws.
#[derive(Debug, Clone, Copy, Default)]
pub struct VisualizerData<'a> {
    /// Band magnitudes, 0..1. Clamped 8..=160 bands; above 160 the bar is
    /// thinner than the gap and the array reads as noise.
    pub bands: &'a [f32],
    /// Per-band peak positions, 0..1, **owned by the caller** — this widget
    /// redraws every frame and must not allocate. `None` disables peak caps.
    pub peaks: Option<&'a [f32]>,
    /// Box width in logical units.
    pub width: f32,
    /// Box height in logical units. `config::Visualizer::height_px`, min 56.
    pub height: f32,
    /// Width as a percentage of the screen, for [`VisualizerVariant::Auto`].
    pub width_pct: f32,
    /// Bar opacity, 0..1. Applies to the bars only.
    pub opacity: f32,
    /// Round the bar caps.
    pub rounded: bool,
    /// How the bars are coloured.
    pub paint: BarPaint,
    /// Which treatment to draw.
    pub variant: VisualizerVariant,
    /// Chassis status strip, left: the track.
    pub title: &'a str,
    /// Chassis status strip, right: size and time.
    pub status: &'a str,
    /// Chassis LCD, large: elapsed time.
    pub elapsed: &'a str,
    /// Chassis LCD, small: bitrate.
    pub bitrate: &'a str,
    /// Chassis LCD, small: sample rate.
    pub samplerate: &'a str,
    /// Playback position for the chassis progress bar.
    pub position: Option<f32>,
}

/// The lowest a panel is allowed to be before the well plus its bevels eat the
/// bar area.
const MIN_PANEL_H: f32 = 56.0;
/// The card radius for a text-free card. `radius_card` is derived from the
/// largest type size and a spectrum has none, so this is fixed at what the
/// lyric card lands on — which keeps the two neighbours in one family.
const PANEL_RADIUS: f32 = 12.0;
/// Panel padding, from §6.2 at `min(w, h) = 120`.
const PANEL_PAD: f32 = 16.0;
/// The width percentage above which the card is not drawn at all.
const BARE_ABOVE_PCT: f32 = 45.0;

fn variant_for(d: &VisualizerData) -> VisualizerVariant {
    match d.variant {
        VisualizerVariant::Auto => {
            if d.width_pct.is_finite() && d.width_pct > BARE_ABOVE_PCT {
                VisualizerVariant::Bare
            } else {
                VisualizerVariant::Panel
            }
        }
        v => v,
    }
}

fn box_size(d: &VisualizerData) -> Size {
    let w = if d.width.is_finite() && d.width > 0.0 {
        d.width.clamp(48.0, 8192.0)
    } else {
        640.0
    };
    let h = if d.height.is_finite() && d.height > 0.0 {
        d.height.clamp(MIN_PANEL_H, 2048.0)
    } else {
        120.0
    };
    Size::new(w, h)
}

fn bar_style(d: &VisualizerData, variant: VisualizerVariant) -> BarStyle {
    let opacity = if d.opacity.is_finite() {
        d.opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    match variant {
        // Without a well, peak caps read as debris, and the shadow is what
        // keeps a bar visible where it crosses a same-tone region of the photo.
        VisualizerVariant::Bare => BarStyle {
            paint: d.paint,
            rounded: d.rounded,
            baseline: true,
            peaks: false,
            opacity,
            shadow: true,
        },
        _ => BarStyle {
            paint: d.paint,
            rounded: d.rounded,
            baseline: true,
            peaks: d.peaks.is_some(),
            opacity,
            shadow: false,
        },
    }
}

/// How big this visualiser is, and how much margin it needs around it.
pub fn measure(_fonts: &mut FontStack, t: &Theme, d: &VisualizerData, _scale: f32) -> WidgetSize {
    let size = box_size(d);
    match variant_for(d) {
        VisualizerVariant::Chassis => WidgetSize::new(size, t.e3()),
        VisualizerVariant::Bare => {
            // The bare variant has no card, but its gradient scrim reaches
            // 0.35 h above the box and 24 lu past each end, and every bar
            // carries an E1 shadow. The buffer has to hold all of that.
            let mut s = WidgetSize::new(size, t.e1());
            s.bleed = s.bleed.max(24.0).max(size.h * 0.35 + 4.0).ceil();
            s
        }
        _ => WidgetSize::new(size, t.e2()),
    }
}

/// Draw the visualiser, centred in whatever room `canvas` provides.
pub fn draw(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &VisualizerData) {
    let size = measure(fonts, t, d, c.scale());
    let rect = size.card_in(c.bounds());
    draw_at(c, fonts, t, d, rect);
}

/// Draw the visualiser with its box at `card`.
pub fn draw_at(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &VisualizerData, card: Rect) {
    if card.is_empty() {
        return;
    }
    match variant_for(d) {
        VisualizerVariant::Bare => draw_bare(c, t, d, card),
        VisualizerVariant::Chassis => draw_chassis(c, fonts, t, d, card),
        _ => draw_panel(c, t, d, card),
    }
}

fn draw_panel(c: &mut Canvas, t: &Theme, d: &VisualizerData, card: Rect) {
    surface::card(c, card, PANEL_RADIUS, t);
    let bed = card.inset(PANEL_PAD);
    if bed.is_empty() {
        return;
    }
    let r = radius_nested(PANEL_RADIUS, PANEL_PAD);
    surface::well(c, bed, r, t);
    let area = bed.inset(6.0);
    if area.is_empty() {
        return;
    }
    surface::bars(
        c,
        area,
        d.bands,
        d.peaks,
        t,
        bar_style(d, VisualizerVariant::Panel),
    );
}

fn draw_bare(c: &mut Canvas, t: &Theme, d: &VisualizerData, card: Rect) {
    surface::gradient_scrim(c, card, t);
    surface::bars(
        c,
        card,
        d.bands,
        None,
        t,
        BarStyle {
            baseline: false,
            ..bar_style(d, VisualizerVariant::Bare)
        },
    );
    // A stronger baseline than the panel's gridline: with no well behind it,
    // the panel's 0.09 alpha would disappear over a bright wallpaper, and the
    // baseline is what makes a silent spectrum read as silent.
    c.rounded_rect(
        Rect::new(card.x, card.bottom(), card.w, t.metrics.hairline),
        0.0,
        &Fill::solid(t.text_primary.with_alpha(0.22)),
    );
}

/// The opaque bevelled alternate.
///
/// Everything Reference B does that is *readout* survives — the chassis, the
/// bevels, the wells, the LCD glow, the orange. Everything it does that is
/// *control* does not exist here at all.
///
/// The chassis is opaque, so no wallpaper reaches its type and none of the
/// translucent contrast model applies. That is precisely why this theme is easy
/// and also why it is not the default: an opaque slab is not a live wallpaper
/// widget, it is a window without a title bar.
fn draw_chassis(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &VisualizerData, card: Rect) {
    let radius = 20.0_f32.min(card.min_side() / 2.0);
    surface::elevation(c, card, radius, t, t.e3());
    c.rounded_rect(card, radius, &Fill::solid(t.chassis));
    c.top_highlight(card, radius, Color::WHITE.with_alpha(0.14), 1.5);
    c.bottom_highlight(card, radius, Color::BLACK.with_alpha(0.55), 1.5);

    let body = card.inset(16.0);
    if body.is_empty() {
        return;
    }
    let cap = Step::Caption.size();

    // Status strip. Neither ink may be thinned: `#F5A623 @ 0.60` on the chassis
    // is 3.47:1 and white at 0.45 is 4.12:1, both fails.
    let strip_h = typo::cap_height(cap, Script::Latin);
    if !d.title.is_empty() {
        let run = typo::mono_run(d.title, cap, fonts)
            .color(t.lcd)
            .max_width(body.w * 0.6);
        c.text(
            fonts,
            &run,
            Point::new(body.x, body.y - typo::cap_gap(cap, Script::Latin)),
        );
    }
    if !d.status.is_empty() {
        let run = typo::mono_run(d.status, cap, fonts).color(Color::WHITE.with_alpha(0.62));
        let m = fonts.measure(&run, c.scale());
        c.text(
            fonts,
            &run,
            Point::new(
                body.right() - m.width,
                body.y - typo::cap_gap(cap, Script::Latin),
            ),
        );
    }

    let rest = body.inset_ltrb(0.0, strip_h + 10.0, 0.0, 0.0);
    if rest.is_empty() {
        return;
    }
    // Progress claims the bottom; the three bezels share what is left.
    let bar_h = 14.0_f32.min(rest.h * 0.25);
    let (top, bottom) = rest.split_v(rest.h - bar_h - 8.0);
    let cols = top.cols(3, 12.0);
    let (left, mid, right) = match cols.as_slice() {
        [a, b, cc] => (*a, *b, *cc),
        _ => return,
    };

    // Left bezel: the LCD readout plus a miniature spectrum.
    let face = surface::bezel(c, left, t);
    if !face.is_empty() {
        let inner = face.inset(6.0);
        if !inner.is_empty() {
            let size = (Step::HeroS.size()).min(inner.h * 0.45);
            if !d.elapsed.is_empty() {
                let run = typo::mono_run(d.elapsed, size, fonts).color(t.lcd);
                let m = fonts.measure(&run, c.scale());
                let at = Point::new(inner.x, inner.y);
                // The glow: the same colour at 0.22, blurred, drawn beneath.
                c.soft_plate(
                    Rect::at(at, m.size()).inset(-2.0),
                    4.0,
                    6.0,
                    t.lcd.with_alpha(0.22),
                );
                c.text(fonts, &run, at);
            }
            let strip = Rect::new(
                inner.x,
                inner.bottom() - inner.h * 0.34,
                inner.w,
                inner.h * 0.34,
            );
            surface::bars(
                c,
                strip,
                d.bands,
                d.peaks,
                t,
                BarStyle {
                    paint: BarPaint::Fixed(t.lcd),
                    rounded: false,
                    baseline: false,
                    peaks: d.peaks.is_some(),
                    opacity: 1.0,
                    shadow: false,
                },
            );
        }
    }

    // Middle bezel: the spectrum proper.
    let face = surface::bezel(c, mid, t);
    if !face.is_empty() {
        let inner = face.inset(6.0);
        surface::bars(
            c,
            inner,
            d.bands,
            d.peaks,
            t,
            BarStyle {
                paint: BarPaint::Fixed(t.lcd),
                rounded: false,
                baseline: true,
                peaks: d.peaks.is_some(),
                opacity: 1.0,
                shadow: false,
            },
        );
    }

    // Right bezel: the small readouts. 0.70 is the floor — 0.45 gives 2.77:1
    // on the well, and alpha-thinned orange runs out of contrast fast.
    let face = surface::bezel(c, right, t);
    if !face.is_empty() {
        let inner = face.inset(8.0);
        let mut y = inner.y;
        for line in [d.bitrate, d.samplerate] {
            if line.is_empty() || y + typo::cap_height(cap, Script::Latin) > inner.bottom() {
                continue;
            }
            let run = typo::mono_run(line, cap, fonts)
                .color(t.lcd.with_alpha(0.70))
                .max_width(inner.w);
            c.text(
                fonts,
                &run,
                Point::new(inner.x, y - typo::cap_gap(cap, Script::Latin)),
            );
            y += typo::cap_height(cap, Script::Latin) + 6.0;
        }
    }

    // The progress bar stays, because it is real data rather than a control.
    if let Some(p) = d.position {
        let bar = Rect::new(
            bottom.x,
            bottom.bottom() - bar_h.max(4.0),
            bottom.w,
            bar_h.max(4.0),
        );
        c.rounded_rect(bar, bar.h / 2.0, &Fill::solid(t.chassis_well));
        c.top_highlight(
            bar.offset(0.0, 1.0),
            bar.h / 2.0,
            Color::BLACK.with_alpha(0.70),
            1.0,
        );
        let f = if p.is_finite() {
            p.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let w = (bar.w * f).max(bar.h).min(bar.w);
        c.rounded_rect(
            Rect::new(bar.x, bar.y, w, bar.h),
            bar.h / 2.0,
            &Fill::solid(t.lcd),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgetkit::theme::Mode;

    fn theme(mode: Mode) -> Theme {
        Theme::for_accent(mode, crate::config::Accent::Teal)
    }

    fn data() -> VisualizerData<'static> {
        const BANDS: [f32; 32] = [
            0.1, 0.3, 0.6, 0.9, 0.7, 0.5, 0.4, 0.8, 0.95, 0.6, 0.3, 0.2, 0.5, 0.7, 0.9, 0.4, 0.2,
            0.1, 0.35, 0.55, 0.75, 0.85, 0.65, 0.45, 0.25, 0.15, 0.3, 0.5, 0.7, 0.4, 0.2, 0.1,
        ];
        VisualizerData {
            bands: &BANDS,
            peaks: None,
            width: 640.0,
            height: 120.0,
            width_pct: 40.0,
            opacity: 0.86,
            rounded: true,
            paint: BarPaint::Vertical,
            variant: VisualizerVariant::Auto,
            title: "Fatboy Slim — Ya Man",
            status: "4.8 MB · 1:34/3:52",
            elapsed: "01:34",
            bitrate: "278 KBPS",
            samplerate: "44 KHZ",
            position: Some(0.42),
        }
    }

    #[test]
    fn the_default_width_gets_the_card_less_treatment() {
        // The shipped default is 60%, which is a slab as a card.
        let d = VisualizerData {
            width_pct: 60.0,
            ..data()
        };
        assert_eq!(variant_for(&d), VisualizerVariant::Bare);
        assert_eq!(
            variant_for(&VisualizerData {
                width_pct: 45.0,
                ..data()
            }),
            VisualizerVariant::Panel
        );
        assert_eq!(
            variant_for(&VisualizerData {
                width_pct: 45.1,
                ..data()
            }),
            VisualizerVariant::Bare
        );
        // An explicit choice always wins.
        assert_eq!(
            variant_for(&VisualizerData {
                width_pct: 90.0,
                variant: VisualizerVariant::Panel,
                ..data()
            }),
            VisualizerVariant::Panel
        );
    }

    #[test]
    fn the_bare_variant_reserves_room_for_a_scrim_it_has_no_card_to_hold() {
        let mut f = FontStack::from_font_data("en-US", []);
        let t = theme(Mode::Dark);
        let panel = measure(
            &mut f,
            &t,
            &VisualizerData {
                width_pct: 30.0,
                ..data()
            },
            1.0,
        );
        let bare = measure(
            &mut f,
            &t,
            &VisualizerData {
                width_pct: 60.0,
                ..data()
            },
            1.0,
        );
        // The gradient scrim reaches 0.35 h above the box.
        assert!(bare.bleed >= 120.0 * 0.35, "{}", bare.bleed);
        assert!(bare.bleed >= 24.0);
        // The panel's bleed is its E2 shadow, which is larger still.
        assert_eq!(panel.bleed, t.e2().bleed());
        // The chassis is the heaviest of the three.
        let ch = measure(
            &mut f,
            &t,
            &VisualizerData {
                variant: VisualizerVariant::Chassis,
                ..data()
            },
            1.0,
        );
        assert_eq!(ch.bleed, t.e3().bleed());
    }

    #[test]
    fn the_box_is_clamped_rather_than_trusted() {
        for (w, h) in [(f32::NAN, f32::NAN), (0.0, 0.0), (-9.0, -9.0), (1e9, 1e9)] {
            let s = box_size(&VisualizerData {
                width: w,
                height: h,
                ..data()
            });
            assert!(s.w.is_finite() && s.w > 0.0);
            assert!(s.h >= MIN_PANEL_H, "height floor: {}", s.h);
        }
        // The published minimum height is honoured.
        assert_eq!(
            box_size(&VisualizerData {
                height: 10.0,
                ..data()
            })
            .h,
            MIN_PANEL_H
        );
    }

    #[test]
    fn opacity_reaches_the_bars_and_nothing_else() {
        let d = VisualizerData {
            opacity: 0.5,
            ..data()
        };
        assert_eq!(bar_style(&d, VisualizerVariant::Panel).opacity, 0.5);
        for bad in [f32::NAN, -1.0, 5.0] {
            let s = bar_style(
                &VisualizerData {
                    opacity: bad,
                    ..data()
                },
                VisualizerVariant::Panel,
            );
            assert!((0.0..=1.0).contains(&s.opacity));
        }
        // Peak caps are off without a well, where they read as debris.
        let peaks = [0.5_f32; 32];
        let d = VisualizerData {
            peaks: Some(&peaks),
            ..data()
        };
        assert!(!bar_style(&d, VisualizerVariant::Bare).peaks);
        assert!(bar_style(&d, VisualizerVariant::Panel).peaks);
        // And the bare variant is the only one that shadows its bars.
        assert!(bar_style(&d, VisualizerVariant::Bare).shadow);
        assert!(!bar_style(&d, VisualizerVariant::Panel).shadow);
    }

    #[test]
    fn no_combination_of_settings_can_panic() {
        let mut f = FontStack::system();
        let mut c = Canvas::for_logical(Size::new(220.0, 140.0), 1.0).unwrap();
        let long: Vec<f32> = (0..4000).map(|i| (i % 17) as f32 / 16.0).collect();
        let odd = [f32::NAN, f32::INFINITY, -3.0, 9.0];
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            for variant in [
                VisualizerVariant::Auto,
                VisualizerVariant::Panel,
                VisualizerVariant::Bare,
                VisualizerVariant::Chassis,
            ] {
                for bands in [&[][..], &odd[..], &long[..], data().bands] {
                    for (w, h) in [(0.0, 0.0), (f32::NAN, 120.0), (60.0, 20.0), (640.0, 120.0)] {
                        let d = VisualizerData {
                            bands,
                            peaks: Some(&odd),
                            width: w,
                            height: h,
                            variant,
                            opacity: f32::NAN,
                            position: Some(f32::NAN),
                            ..data()
                        };
                        let m = measure(&mut f, &t, &d, 1.0);
                        assert!(m.buffer().w.is_finite() && m.buffer().h.is_finite());
                        c.reset();
                        draw_at(&mut c, &mut f, &t, &d, Rect::new(12.0, 12.0, 190.0, 100.0));
                        draw_at(&mut c, &mut f, &t, &d, Rect::new(-40.0, -40.0, 90.0, 30.0));
                        draw_at(&mut c, &mut f, &t, &d, Rect::ZERO);
                    }
                }
            }
            c.reset();
            draw(&mut c, &mut f, &t, &data());
        }
    }

    #[test]
    fn the_chassis_owns_its_colour_and_draws_no_controls() {
        // The theme's LCD colour is fixed and mode-independent: the look is the
        // colour, so it opts out of `accent_follow` entirely.
        let d = theme(Mode::Dark);
        let l = theme(Mode::Light);
        assert_eq!(d.lcd, l.lcd);
        assert_ne!(d.lcd, d.accent_fill);
        // The chassis is opaque, which is why none of the translucent contrast
        // model applies to it — and also why it is not the default.
        assert_eq!(d.chassis.a, 1.0);
        assert_eq!(d.chassis_well.a, 1.0);
    }
}

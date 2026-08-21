//! The now-playing / synced-lyrics card (spec §9.2).
//!
//! The card with the most missing-data cases in the system, and the reason it
//! is written as a layout pass over an [`Option`]-shaped struct rather than as
//! a fixed template: at any moment it may have no lyrics, no album art, no
//! MPRIS position, no title, or all four.
//!
//! # Two scrims, not one
//!
//! The header block (label, title, artist, progress) and the lyric block get
//! **separate** scrims, with the divider falling in the un-scrimmed gap between
//! them. One scrim spanning both would fill the card, and there would be no
//! glass left anywhere on it. That 40 lu gap is where the wallpaper actually
//! reads through, and it is the whole reason this card looks translucent while
//! the clock card — whose scrim clamps to its inner rect — does not.
//!
//! # The card's width is fixed, its height is not
//!
//! ```text
//! width = clamp(15 · L, 320, 0.9 · screen_width)     → 420 at L = 28
//! ```
//!
//! Not measured from content. A card that changes width every time the track
//! changes is worse than one that is occasionally too wide — and unlike the
//! clock, this card's content changes on someone else's schedule.
//!
//! Height *is* content-driven, because the alternative is an empty band where
//! the lyric goes. A permanent "no lyrics found" apology on the wallpaper is
//! worse than silence, so the block and its divider are removed outright.
//!
//! # What is deliberately absent
//!
//! No play button, no transport, no seek handle. The progress bar is data; the
//! knob on it is a marker and only appears when the bar is thick enough to
//! carry one, which at this card's 4 lu track height it never is.

use crate::widgetkit::canvas::Canvas;
use crate::widgetkit::color::Color;
use crate::widgetkit::geom::{Point, Rect, Size};
use crate::widgetkit::paint::Fill;
use crate::widgetkit::surface::{self, WidgetSize};
use crate::widgetkit::text::FontStack;
use crate::widgetkit::theme::{card_padding, radius_card, Theme};
use crate::widgetkit::typo::{self, Script, Step};

/// What a now-playing card draws.
#[derive(Debug, Clone, Copy, Default)]
pub struct NowPlayingData<'a> {
    /// The micro-label. `"Now playing"` normally; the stream host for a stream
    /// with no metadata.
    pub label: &'a str,
    /// Track title. Ellipsised to one line.
    pub title: &'a str,
    /// Artist. Ellipsised to one line — and `· album` is dropped **before** the
    /// artist is cut, because the artist is the part someone is reading.
    pub artist: &'a str,
    /// Album, appended to the artist when there is room.
    pub album: &'a str,
    /// The current lyric line. Empty removes the whole lyric block *and* the
    /// divider, and the card becomes header-only.
    pub lyric: &'a str,
    /// The next lyric line, when `show_next_line` is on.
    pub next_lyric: &'a str,
    /// True while holding the previous line through an instrumental break: the
    /// row drops to [`Theme::text_tertiary`] instead of blinking out. A row
    /// that appears and disappears every instrumental break is the single most
    /// irritating failure mode of a lyric widget.
    pub lyric_is_stale: bool,
    /// Elapsed time, e.g. `"1:34"`. Tabular figures.
    pub elapsed: &'a str,
    /// Track length, e.g. `"7:29"`.
    pub total: &'a str,
    /// Playback position, 0..1. `None` hides **both** the bar and the readout
    /// and collapses the row — a bar pinned at zero says "this track is stuck",
    /// which is a lie.
    pub position: Option<f32>,
    /// Album art. Non-square sources are centre-cropped, never squashed.
    pub art: Option<&'a image::RgbaImage>,
    /// The source app's icon, for the badge over the art's corner.
    pub badge: Option<&'a image::RgbaImage>,
    /// The source app's name, for the badge fallback when it has no icon.
    pub badge_label: &'a str,
    /// A format chip, e.g. `"FLAC · 44.1"`. Empty draws none.
    pub chip: &'a str,
    /// Lyric size in logical units. `config::Lyrics::font_size_pt`, default 28.
    pub font_size: f32,
    /// Draw the current lyric in [`Theme::accent_ink`].
    pub accent_follow: bool,
    /// Screen width, so the card can clamp to 0.9 of it. Zero means unclamped.
    pub screen_width: f32,
}

#[derive(Debug, Clone, Copy)]
struct Layout {
    lyric: f32,
    title: f32,
    body: f32,
    art: f32,
    pad: f32,
    radius: f32,
    size: Size,
    /// Header block, relative to the card origin.
    header: Rect,
    /// Lyric block, relative to the card origin. Empty when there is none.
    lyrics: Rect,
    divider_y: f32,
    lyric_lines: usize,
    has_next: bool,
    has_progress: bool,
}

/// The default lyric size, and the one every derived size is quoted against.
const DEFAULT_L: f32 = 28.0;
/// Gap between the art and the text column.
const ART_GAP: f32 = 16.0;

fn lyric_size(d: &NowPlayingData) -> f32 {
    if d.font_size.is_finite() && d.font_size > 0.0 {
        d.font_size.clamp(8.0, 200.0)
    } else {
        DEFAULT_L
    }
}

/// The artist row's text: `artist · album`, with the album dropped first when
/// there is not room for both.
fn artist_text(d: &NowPlayingData) -> String {
    match (d.artist.is_empty(), d.album.is_empty()) {
        (true, true) => String::new(),
        (false, true) => d.artist.to_string(),
        (true, false) => d.album.to_string(),
        (false, false) => format!("{} · {}", d.artist, d.album),
    }
}

fn layout(fonts: &mut FontStack, t: &Theme, d: &NowPlayingData, scale: f32) -> Layout {
    let l = lyric_size(d);
    let title = typo::nearest_ladder_step(0.64 * l).max(Step::Micro.size());
    let body = typo::nearest_ladder_step(0.50 * l).max(Step::Micro.size());
    let art = (4.0 * title).max(24.0);
    let radius = radius_card(l);

    let mut width = 15.0 * l;
    let cap = if d.screen_width.is_finite() && d.screen_width > 0.0 {
        0.9 * d.screen_width
    } else {
        f32::INFINITY
    };
    width = width.clamp(320.0_f32.min(cap), cap.max(1.0));

    let has_progress = d.position.is_some();
    let has_lyrics = !d.lyric.is_empty();
    let has_next = has_lyrics && !d.next_lyric.is_empty();

    // Height is built from the blocks that survive, in order.
    let pad = card_padding(width.min(240.0));
    let mut y = pad;
    let header = Rect::new(pad, y, (width - 2.0 * pad).max(1.0), art);
    y += art;

    let lyric_script = Script::of(d.lyric);
    let mut lyric_lines = 0usize;
    let mut lyrics = Rect::ZERO;
    let mut divider_y = 0.0;
    if has_lyrics {
        divider_y = y + t.metrics.gap_xl;
        y = divider_y + 1.0 + t.metrics.gap_xl;
        // Wrap to two lines, then ellipsis. Measured, not guessed: the card
        // grows by exactly one line height when it wraps.
        let run = typo::styled(d.lyric, l, 500, false, fonts)
            .max_width((width - 2.0 * pad).max(1.0))
            .max_lines(2);
        lyric_lines = fonts.measure(&run, scale).lines.max(1);
        let one = typo::cap_height(l, lyric_script);
        let mut h = one + (lyric_lines - 1) as f32 * typo::line_height_ratio(l, lyric_script) * l;
        if has_next {
            h += t.metrics.gap_s + one;
        }
        h += typo::descender(l, lyric_script);
        lyrics = Rect::new(pad, y, (width - 2.0 * pad).max(1.0), h);
        y += h;
    }
    let height = y + pad;

    Layout {
        lyric: l,
        title,
        body,
        art,
        pad,
        radius,
        size: Size::new(width, height),
        header,
        lyrics,
        divider_y,
        lyric_lines,
        has_next,
        has_progress,
    }
}

/// How big this card is, and how much shadow margin it needs.
pub fn measure(fonts: &mut FontStack, t: &Theme, d: &NowPlayingData, scale: f32) -> WidgetSize {
    WidgetSize::new(layout(fonts, t, d, scale).size, t.e2())
}

/// Draw the card, centred in whatever room `canvas` provides.
pub fn draw(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &NowPlayingData) {
    let size = measure(fonts, t, d, c.scale());
    let rect = size.card_in(c.bounds());
    draw_at(c, fonts, t, d, rect);
}

/// Draw the card with its card rect at `card`.
pub fn draw_at(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &NowPlayingData, card: Rect) {
    if card.is_empty() {
        return;
    }
    let l = layout(fonts, t, d, c.scale());
    surface::card(c, card, l.radius, t);

    let header = l.header.offset(card.x, card.y);
    let art = Rect::new(header.x, header.y, l.art, l.art);
    let text_col = header.inset_ltrb(l.art + ART_GAP, 0.0, 0.0, 0.0);

    // Scrim one: the header. Sized from the largest row in it, which is the
    // title, not the lyric.
    //
    // A *zone* scrim, not a floating plate: it runs to the card's inner rect on
    // three sides and takes the card's own rounding on the two corners it
    // shares with it, so its only visible boundary is the straight edge facing
    // the gap. A plate inset on all four sides is what made this card read as
    // two stacked panels.
    let band = if l.lyrics.is_empty() {
        // Header-only: there is no gap to face, so the zone runs to the bottom.
        card.bottom()
    } else {
        card.y + l.divider_y - t.metrics.gap_m
    };
    // With two zones the gap between them gets the waist rather than nothing:
    // a full-strength scrim above it and below it and bare card across it makes
    // the middle of the card a tonal stripe, and a stripe is the one thing that
    // reads as the join between two panels however softly it is feathered. The
    // base and the zones composite to exactly `Theme::scrim` where the text is,
    // so §4 is unchanged; only the waist moves. Header-only has no gap and so
    // takes the plain scrim.
    let zone_ink = if l.lyrics.is_empty() {
        t.scrim
    } else {
        surface::scrim_waist(c, t, card, l.radius)
    };
    surface::zone_scrim_in(
        c,
        t,
        text_col,
        surface::ScrimSpec {
            card,
            radius: l.radius,
            pad: l.pad,
            largest: l.title,
            script: Script::of(d.title),
        },
        surface::ScrimZone::Top { free_edge: band },
        zone_ink,
    );

    draw_art(c, fonts, t, d, &l, art);
    draw_header_text(c, fonts, t, d, &l, text_col, art);

    if !l.lyrics.is_empty() {
        // The divider sits in the un-scrimmed gap, which is the only place on
        // this card the wallpaper reads through unmodified.
        c.rounded_rect(
            Rect::new(
                card.x + l.pad,
                card.y + l.divider_y,
                (card.w - 2.0 * l.pad).max(0.0),
                t.metrics.hairline,
            ),
            0.0,
            &Fill::solid(t.text_primary.with_alpha(0.10)),
        );
        let block = l.lyrics.offset(card.x, card.y);
        // Scrim two: the lyric block, sized from the lyric's own size, and the
        // mirror of the header's — flush with the card's bottom, left and right
        // inner edges, square-cornered on the edge facing the gap. The two free
        // edges sit `gap.m` either side of the divider, so the un-scrimmed band
        // is symmetric about it and reads as one card's waist rather than as
        // the seam between two.
        surface::zone_scrim_in(
            c,
            t,
            block,
            surface::ScrimSpec {
                card,
                radius: l.radius,
                pad: l.pad,
                largest: l.lyric,
                script: Script::of(d.lyric),
            },
            surface::ScrimZone::Bottom {
                free_edge: card.y + l.divider_y + t.metrics.gap_m,
            },
            zone_ink,
        );
        draw_lyrics(c, fonts, t, d, &l, block);
    }
}

fn draw_art(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &NowPlayingData,
    l: &Layout,
    art: Rect,
) {
    if art.is_empty() {
        return;
    }
    let r = (4.0 * (0.18 * l.art / 4.0).round()).clamp(6.0, 20.0);
    surface::elevation(c, art, r, t, t.e1());
    match d.art {
        Some(img) => c.image_cover(img, art, r),
        None => {
            c.rounded_rect(art, r, &Fill::solid(t.well));
            note_glyph(c, art, t);
        }
    }
    c.hairline(art, r, t.edge, t.metrics.hairline);

    // The source badge overlaps the art's bottom-right corner. Its own cutout
    // is a well-filled disc a little larger than the badge, which reads as
    // separation without the bright ring a stroke turns into over a photo.
    let dsz = l.title * 1.55;
    if dsz >= 12.0 {
        let centre = Point::new(art.right() - 6.0, art.bottom() - 6.0);
        let cut = Rect::new(
            centre.x - dsz / 2.0 - 2.0,
            centre.y - dsz / 2.0 - 2.0,
            dsz + 4.0,
            dsz + 4.0,
        );
        c.rounded_rect(cut, (dsz + 4.0) / 2.0, &Fill::solid(t.surface));
        let b = Rect::new(centre.x - dsz / 2.0, centre.y - dsz / 2.0, dsz, dsz);
        surface::badge(c, fonts, t, b, d.badge, d.badge_label);
    }
}

/// A drawn musical note for the missing-art placeholder. Shared with
/// [`super::media`], which has the same hole to fill and must fill it the same
/// way.
///
/// Drawn rather than typed: `♪` is missing from plenty of installed faces, and
/// a tofu box where the album art goes looks like a rendering fault rather than
/// like "no artwork".
pub(crate) fn note_glyph(c: &mut Canvas, art: Rect, t: &Theme) {
    let s = art.min_side() * 0.44;
    if s < 6.0 {
        return;
    }
    let centre = art.center();
    let ink = t.text_tertiary;
    let head = s * 0.34;
    let hx = centre.x - s * 0.30;
    let hy = centre.y + s * 0.30;
    c.rounded_rect(
        Rect::new(hx - head, hy - head * 0.78, head * 2.0, head * 1.56),
        head * 0.78,
        &Fill::solid(ink),
    );
    let stem = (s * 0.10).max(1.0);
    c.rounded_rect(
        Rect::new(hx + head - stem, centre.y - s * 0.62, stem, s * 0.94),
        stem / 2.0,
        &Fill::solid(ink),
    );
    c.rounded_rect(
        Rect::new(hx + head - stem, centre.y - s * 0.62, s * 0.42, stem * 1.6),
        stem,
        &Fill::solid(ink),
    );
}

fn draw_header_text(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &NowPlayingData,
    l: &Layout,
    col: Rect,
    art: Rect,
) {
    if col.is_empty() {
        return;
    }
    let micro = Step::Micro.size();
    // The chip claims the right-hand end of the label row before the label is
    // measured, so a long label ellipsises rather than colliding with it.
    let mut label_w = col.w;
    if !d.chip.is_empty() {
        let probe = typo::step_run(d.chip, Step::Micro, fonts);
        let m = fonts.measure(&probe, c.scale());
        // Two pads, not one: `surface::chip` insets by `height x 0.45` on each
        // side, and reserving only one of them ellipsises a label that fits.
        let cw = m.width + micro * 2.0 * 0.45 * 2.0 + 1.0;
        let at = Point::new(col.right() - cw, col.y);
        surface::chip(c, fonts, t, at, d.chip, false, cw);
        label_w = (col.w - cw - t.metrics.gap_m).max(1.0);
    }

    let mut y = col.y;
    if !d.label.is_empty() {
        let run = typo::step_run(d.label, Step::Micro, fonts)
            .color(t.text_tertiary)
            .max_width(label_w);
        let s = Script::of(d.label);
        c.text(fonts, &run, Point::new(col.x, y - typo::cap_gap(micro, s)));
        y += typo::cap_height(micro, s) + t.metrics.gap_s;
    }
    if !d.title.is_empty() {
        let s = Script::of(d.title);
        let run = typo::styled(d.title, l.title, 600, false, fonts)
            .color(t.text_primary)
            .max_width(col.w);
        c.text(
            fonts,
            &run,
            Point::new(col.x, y - typo::cap_gap(l.title, s)),
        );
        y += typo::cap_height(l.title, s) + 6.0;
    }
    let artist = artist_text(d);
    if !artist.is_empty() {
        let s = Script::of(&artist);
        let run = typo::styled(&artist, l.body, 500, false, fonts)
            .color(t.text_secondary)
            .max_width(col.w);
        c.text(fonts, &run, Point::new(col.x, y - typo::cap_gap(l.body, s)));
    }

    if !l.has_progress {
        return;
    }
    // The progress row is bottom-aligned with the art, which is what keeps the
    // header a rectangle rather than a ragged column.
    let h = surface::progress_height(l.title);
    let readout = format!("{} / {}", d.elapsed, d.total);
    let mut bar_w = col.w;
    if !d.elapsed.is_empty() || !d.total.is_empty() {
        let run = typo::mono_run(&readout, Step::Caption.size(), fonts).color(t.text_tertiary);
        let m = fonts.measure(&run, c.scale());
        bar_w = (col.w - m.width - t.metrics.gap_m).max(h);
        let ty = art.bottom() - typo::cap_height(Step::Caption.size(), Script::Latin);
        c.text(
            fonts,
            &run,
            Point::new(
                col.right() - m.width,
                ty - typo::cap_gap(Step::Caption.size(), Script::Latin),
            ),
        );
    }
    let bar = Rect::new(col.x, art.bottom() - h, bar_w, h);
    surface::progress_bar(c, bar, d.position.unwrap_or(0.0), t);
}

fn draw_lyrics(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &NowPlayingData,
    l: &Layout,
    block: Rect,
) {
    let s = Script::of(d.lyric);
    let colour = if d.lyric_is_stale {
        t.text_tertiary
    } else if d.accent_follow {
        t.accent_ink
    } else {
        t.text_primary
    };
    let run = typo::styled(d.lyric, l.lyric, 500, false, fonts)
        .color(colour)
        .max_width(block.w)
        .max_lines(2);
    let mut y = block.y;
    c.text(
        fonts,
        &run,
        Point::new(block.x, y - typo::cap_gap(l.lyric, s)),
    );
    y += typo::cap_height(l.lyric, s)
        + (l.lyric_lines - 1) as f32 * typo::line_height_ratio(l.lyric, s) * l.lyric;
    if l.has_next {
        y += t.metrics.gap_s;
        let ns = Script::of(d.next_lyric);
        let run = typo::styled(d.next_lyric, l.lyric, 500, false, fonts)
            .color(t.text_tertiary)
            .max_width(block.w);
        c.text(
            fonts,
            &run,
            Point::new(block.x, y - typo::cap_gap(l.lyric, ns)),
        );
    }
}

/// A flat colour for a caller that wants the divider's exact ink.
pub fn divider_colour(t: &Theme) -> Color {
    t.text_primary.with_alpha(0.10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgetkit::theme::Mode;

    fn theme(mode: Mode) -> Theme {
        Theme::for_accent(mode, crate::config::Accent::Coral)
    }

    fn full() -> NowPlayingData<'static> {
        NowPlayingData {
            label: "Now playing",
            title: "Blue Monday",
            artist: "New Order",
            album: "Substance",
            lyric: "I see a ship in the harbour",
            next_lyric: "I can and shall obey",
            lyric_is_stale: false,
            elapsed: "1:34",
            total: "7:29",
            position: Some(0.21),
            art: None,
            badge: None,
            badge_label: "Spotify",
            chip: "FLAC · 44.1",
            font_size: 28.0,
            accent_follow: false,
            screen_width: 1920.0,
        }
    }

    /// Composite the card over the theme's governing worst-case wallpaper and
    /// read the surface back out of the pixels, so the §4 figures are asserted
    /// against what is actually drawn rather than against the tokens alone.
    fn surface_at(
        mode: Mode,
        d: &NowPlayingData,
        fonts: &mut FontStack,
        probes: &[(f32, f32)],
    ) -> Vec<Color> {
        let t = theme(mode);
        let size = measure(fonts, &t, d, 1.0);
        let mut c = Canvas::for_logical(size.buffer(), 1.0).expect("canvas");
        // Dark is fitted against a white wallpaper and light against a black
        // one; each is the case its whole alpha ramp was chosen for.
        let wall = if mode.is_dark() {
            Color::WHITE
        } else {
            Color::BLACK
        };
        let bounds = c.bounds();
        c.rounded_rect(bounds, 0.0, &Fill::solid(wall));
        let card = size.card_rect();
        draw_at(&mut c, fonts, &t, d, card);
        let px = c.to_bgra();
        probes
            .iter()
            .map(|&(x, y)| {
                let ix = (x.round().max(0.0) as u32).min(px.w - 1);
                let iy = (y.round().max(0.0) as u32).min(px.h - 1);
                let o = ((iy * px.w + ix) * 4) as usize;
                // The backdrop is opaque, so premultiplied is straight here.
                Color::rgb8(px.data[o + 2], px.data[o + 1], px.data[o])
            })
            .collect()
    }

    #[test]
    fn the_two_zones_composite_to_one_surface_and_keep_the_ss_4_figures() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let d = full();
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            let l = layout(&mut f, &t, &d, 1.0);
            let card = measure(&mut f, &t, &d, 1.0).card_rect();
            let lyric_y = card.y + l.lyrics.y + 6.0;
            let header_y = card.y + l.pad + 44.0;
            // Left and right of each zone's ink, at the same height. The seam
            // this card used to have was a step *across x*: the plate stopped
            // short of the card's inner rect on both sides, so the ends of
            // every row sat on a different surface from the middle of it.
            let p = surface_at(
                mode,
                &d,
                &mut f,
                &[
                    (card.x + 5.0, lyric_y),
                    (card.right() - 5.0, lyric_y),
                    (card.x + 5.0, header_y),
                    (card.right() - 5.0, header_y),
                ],
            );
            for (a, b) in [(p[0], p[1]), (p[2], p[3])] {
                let step = (a.relative_luminance() - b.relative_luminance()).abs();
                assert!(step < 0.02, "{mode:?}: a seam across the zone: {a:?} {b:?}");
            }
            // And every probe is a scrimmed surface, which is the backdrop the
            // ink was fitted against: AAA for the primary rows, AA for the
            // tertiary micro-label and time readout (§4.1, §4.2).
            for s in &p {
                let primary = s.contrast_ratio(t.text_primary);
                let tertiary = s.contrast_ratio(t.text_tertiary);
                assert!(primary >= 7.0, "{mode:?}: primary {primary} on {s:?}");
                assert!(tertiary >= 4.5, "{mode:?}: tertiary {tertiary} on {s:?}");
            }
        }
    }

    /// The seam the two zones used to leave *across y*.
    ///
    /// Two full scrims with bare card between them put a full-width tonal
    /// stripe across the middle of the card, and a stripe is read as the join
    /// between two panels however far its edges are feathered. Measured on the
    /// light card over black the gap sat 0.134 in relative luminance below the
    /// zones either side of it. The waist (`surface::SCRIM_WAIST`) has to keep
    /// that step small — and has to keep it non-zero, because a gap scrimmed as
    /// hard as the text is is the single scrim §9.2 refuses.
    /// The other half of [`the_gap_between_the_zones_is_a_waist_and_not_a_seam`]'s
    /// bargain: the waist is a *fraction* of the scrim, never all of it, or the
    /// two zones have become the single card-filling scrim §9.2 refuses.
    ///
    /// A `const` item rather than an assertion inside the test, so a bad edit to
    /// the constant fails the build instead of waiting for a test run.
    const _: () = assert!(surface::SCRIM_WAIST > 0.0 && surface::SCRIM_WAIST < 1.0);

    #[test]
    fn the_gap_between_the_zones_is_a_waist_and_not_a_seam() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let d = full();
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            let l = layout(&mut f, &t, &d, 1.0);
            let card = measure(&mut f, &t, &d, 1.0).card_rect();
            // A column clear of every glyph, well, chip and hairline: 5 lu in
            // from the card's right edge.
            let x = card.right() - 5.0;
            let p = surface_at(
                mode,
                &d,
                &mut f,
                &[
                    (x, card.y + l.pad + 44.0),
                    (x, card.y + l.divider_y - 2.0),
                    (x, card.y + l.lyrics.y + 6.0),
                ],
            );
            let waist = p[1].relative_luminance();
            let step = (p[0].relative_luminance() - waist)
                .abs()
                .max((p[2].relative_luminance() - waist).abs());
            assert!(step < 0.08, "{mode:?}: the gap is still a stripe: {step}");
        }
    }

    #[test]
    fn the_worked_geometry_matches_the_spec_at_the_default_size() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let l = layout(&mut f, &t, &full(), 1.0);
        // Spec §9.2: L 28 → title 18, body 14, art 72, pad 20, R 12, width 420.
        assert_eq!(l.title, 18.0);
        assert_eq!(l.body, 14.0);
        assert_eq!(l.art, 72.0);
        assert_eq!(l.pad, 20.0);
        assert_eq!(l.radius, 12.0);
        assert_eq!(l.size.w, 420.0);
        // Height 213 with both lyric rows present.
        assert!((l.size.h - 213.0).abs() < 2.0, "height {}", l.size.h);
    }

    #[test]
    fn losing_the_lyrics_makes_the_card_header_only_rather_than_leaving_a_band() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let both = layout(&mut f, &t, &full(), 1.0);
        let no_next = layout(
            &mut f,
            &t,
            &NowPlayingData {
                next_lyric: "",
                ..full()
            },
            1.0,
        );
        let none = layout(
            &mut f,
            &t,
            &NowPlayingData {
                lyric: "",
                next_lyric: "",
                ..full()
            },
            1.0,
        );
        assert!(no_next.size.h < both.size.h);
        assert!(none.size.h < no_next.size.h);
        // Header-only is 112: pad + art + pad.
        assert!((none.size.h - 112.0).abs() < 1.0, "{}", none.size.h);
        assert!(none.lyrics.is_empty(), "the lyric block survived");
        // Dropping the next line costs about one cap plus the gap.
        let delta = both.size.h - no_next.size.h;
        assert!((delta - 28.4).abs() < 2.0, "delta {delta}");
    }

    #[test]
    fn the_width_is_fixed_and_clamped_to_the_screen_rather_than_measured() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        // A far longer title must not change the width.
        let long = NowPlayingData {
            title: "A title so long that no reasonable card could ever contain it, twice over",
            artist: "An artist with an equally unreasonable name, at length",
            ..full()
        };
        assert_eq!(
            layout(&mut f, &t, &full(), 1.0).size.w,
            layout(&mut f, &t, &long, 1.0).size.w
        );
        // A narrow screen clamps it.
        let narrow = NowPlayingData {
            screen_width: 400.0,
            ..full()
        };
        assert!((layout(&mut f, &t, &narrow, 1.0).size.w - 360.0).abs() < 0.01);
        // And a big lyric size grows it, subject to the 320 floor.
        let small = NowPlayingData {
            font_size: 10.0,
            ..full()
        };
        assert_eq!(layout(&mut f, &t, &small, 1.0).size.w, 320.0);
    }

    #[test]
    fn a_wrapped_lyric_grows_the_card_by_exactly_one_line() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let one = layout(&mut f, &t, &full(), 1.0);
        let two = layout(
            &mut f,
            &t,
            &NowPlayingData {
                lyric: "I see a ship in the harbour and I can and shall obey \
                        every word that follows it here",
                ..full()
            },
            1.0,
        );
        if two.lyric_lines == 2 {
            let delta = two.size.h - one.size.h;
            let want = typo::line_height_ratio(28.0, Script::Latin) * 28.0;
            assert!((delta - want).abs() < 1.5, "delta {delta}, want {want}");
        }
    }

    #[test]
    fn the_album_is_dropped_before_the_artist_is_cut() {
        assert_eq!(artist_text(&full()), "New Order · Substance");
        assert_eq!(
            artist_text(&NowPlayingData {
                album: "",
                ..full()
            }),
            "New Order"
        );
        assert_eq!(
            artist_text(&NowPlayingData {
                artist: "",
                ..full()
            }),
            "Substance"
        );
        assert_eq!(
            artist_text(&NowPlayingData {
                artist: "",
                album: "",
                ..full()
            }),
            ""
        );
    }

    #[test]
    fn a_stale_lyric_dims_rather_than_disappearing() {
        let mut f = FontStack::system();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        // The block survives while the line is stale — the row does not blink
        // in and out on every instrumental break.
        let stale = layout(
            &mut f,
            &t,
            &NowPlayingData {
                lyric_is_stale: true,
                ..full()
            },
            1.0,
        );
        assert!(!stale.lyrics.is_empty());
        assert_eq!(stale.size, layout(&mut f, &t, &full(), 1.0).size);
    }

    #[test]
    fn no_combination_of_settings_can_panic() {
        let mut f = FontStack::system();
        let mut c = Canvas::for_logical(Size::new(220.0, 160.0), 1.0).unwrap();
        let art = image::RgbaImage::from_pixel(300, 120, image::Rgba([200, 60, 90, 255]));
        let tiny = image::RgbaImage::new(0, 0);
        let icon = image::RgbaImage::from_pixel(24, 24, image::Rgba([30, 200, 120, 255]));
        let texts = [
            ("", "", ""),
            ("Blue Monday", "New Order", "I see a ship in the harbour"),
            ("春风十里", "鹿先森乐队", "我在春天等你的消息"),
            (
                "an extremely long title that no card can hold at any size at all",
                "an equally long artist name for good measure",
                "a lyric line so long that it must wrap twice and then be cut off entirely",
            ),
        ];
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            for size in [f32::NAN, 0.0, 28.0, 120.0] {
                for pos in [None, Some(1.0), Some(f32::NAN)] {
                    for (title, artist, lyric) in texts {
                        let d = NowPlayingData {
                            label: "Now playing",
                            title,
                            artist,
                            album: "Substance",
                            lyric,
                            next_lyric: lyric,
                            lyric_is_stale: true,
                            elapsed: "1:34",
                            total: "7:29",
                            position: pos,
                            art: Some(&art),
                            badge: Some(&icon),
                            badge_label: "网易云音乐",
                            chip: "FLAC · 44.1",
                            font_size: size,
                            accent_follow: true,
                            screen_width: 0.0,
                        };
                        let m = measure(&mut f, &t, &d, 1.0);
                        assert!(m.buffer().w.is_finite() && m.buffer().h.is_finite());
                        c.reset();
                        draw_at(&mut c, &mut f, &t, &d, Rect::new(8.0, 8.0, 200.0, 130.0));
                        draw_at(&mut c, &mut f, &t, &d, Rect::new(-30.0, -30.0, 120.0, 60.0));
                        draw_at(&mut c, &mut f, &t, &d, Rect::ZERO);
                    }
                }
            }
            // A zero-sized cover must not reach the rasteriser, and `draw`
            // sizes from the canvas, so both get one pass per theme.
            c.reset();
            draw(
                &mut c,
                &mut f,
                &t,
                &NowPlayingData {
                    art: Some(&tiny),
                    ..full()
                },
            );
        }
        assert_eq!(divider_colour(&theme(Mode::Dark)).a, 0.10);
    }
}

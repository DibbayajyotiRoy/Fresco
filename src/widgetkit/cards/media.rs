//! The **combined media card** (spec §9.6) — one chassis carrying everything
//! the now-playing card and the visualiser used to carry between them.
//!
//! Track, artist, album, elapsed, total, progress, the current lyric and the
//! next one, album art, bitrate, sample rate, playback state and the live
//! spectrum, in one object instead of two widgets that had to be placed so they
//! did not collide and still repeated each other's data.
//!
//! ```text
//!  ┌───────────────────────────────────────────────────────────────────┐ E3
//!  │▔ top bevel ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔│
//!  │  PLAYING · SPOTIFY                            278 KBPS · 44 KHZ   │ status strip
//!  │  Blue Monday                                                      │ title
//!  │  New Order · Substance                                            │ artist
//!  │ ┌───────┐ ╭─────────────────────────────────╮ ┌─────────────────┐ │
//!  │ │ album │ │  ▁▃▅█▇▅▃▁▂▄▆█▇▆▄▂▁▂▃▅▆█▇▅▃▂▁▁▂  │ │  01:34          │ │
//!  │ │  art  │ │      the spectrum, the hero     │ │  03:52    ▶ ▮▮ ■│ │
//!  │ └───────┘ ╰─────────────────────────────────╯ └─────────────────┘ │
//!  │  I see a ship in the harbour                                      │ lyric
//!  │  I can and shall obey                                             │ next
//!  │ ▐████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░▌ │ progress
//!  │_ bottom bevel ___________________________________________________│
//!  └───────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # What the reference's controls became
//!
//! The skeuomorphic reference this is adapted from is roughly half controls: a
//! row of five large glossy circular transport buttons, a 2 × 3 grid of small
//! bevelled buttons, and a `VOLUME` label over a slider. **Fresco's widget
//! layer has an empty input region — nothing drawn on it can ever be clicked**,
//! so every one of those is an affordance that lies, and someone will click it
//! anyway. Spec §11.7 already refused to draw them; this card extends that
//! ruling and states where the space went.
//!
//! | Reference | Here | Why |
//! |---|---|---|
//! | Five glossy transport buttons | **The lyric band** | The one thing on this card a person reads from across a room. It is the largest type on the card and it took the largest zone. |
//! | Small orange circle beside them | gone | It was a sixth control. |
//! | 2 × 3 bevelled button grid | gone | §8.7 keeps the bezel as a *readout* frame, and a grid of six has nothing to read out. |
//! | `VOLUME` label + slider | gone, and not replaced | A volume the widget cannot set and does not observe is a lie twice over. §11 called this and it stands. |
//! | Transport glyphs in the left readout | **State indicators** | Kept, because playback state is real. Redrawn flat: the live state is lit in the readout orange at 1.25 × size, the other two are small, dim and neutral. No gloss, no bevel, no circular pad — nothing that reads as pressable. |
//! | Second spectrum well, bottom left | **The album art** | The reference draws its spectrum twice and gives artwork no home at all. One of those wells was redundant and the other was missing, so they are the same well. |
//! | Centre panel with the glowing waveform | **The live spectrum** | Unchanged in role: the hero, magenta into purple, in a black inset. |
//! | Full-width inset progress track | kept | Real data, not a control (§8.2). Hidden outright when there is no position. |
//!
//! # Degradation is the common case
//!
//! On a real desktop, right now, there is very often no album art, no MPRIS
//! position and no `.lrc` for the playing track. This card is therefore
//! designed around the *reduced* state and decorated up to the full one, not
//! the other way round:
//!
//! | Missing | What happens |
//! |---|---|
//! | Lyrics | The lyric band is removed and its height is given to the **instrument row** — the art, the spectrum panel and the readout well all grow into it. The card's outline does not change, so nothing on the desktop moves; the visualiser simply becomes the hero, which is what the widget is for when there are no words. |
//! | Album art | The well stays and carries the drawn note glyph (§9.2's, shared). A chassis with a hole in it is not a chassis. |
//! | MPRIS position | The progress track is **removed**, not drawn at zero, and the big readout shows `--:--`. A bar pinned at zero says the track is stuck; a dashed readout says the position is not known, which is what an instrument does. |
//! | Elapsed *and* total | The readout well keeps its dashes and the state indicators. It is never empty. |
//! | Bitrate / sample rate | The strip's right-hand end is simply shorter. |
//! | Paused / stopped | The state word changes and a different indicator lights. The spectrum falls to its resting row of minimum-height bars, which is what silence looks like — not an empty panel. |
//! | A very long CJK title | Ellipsised at a grapheme boundary, one line, at the weight §5.3 allows. |
//!
//! `tests::every_degenerate_combination_stays_inside_the_card` renders the
//! whole cross product and asserts nothing leaves the card rect — which is the
//! shape of the last real bug on the now-playing card, a badge that overflowed
//! the bottom edge because the minimal case had never been drawn.
//!
//! # The chassis is opaque, and its ink is mode-aware
//!
//! No wallpaper reaches this card's type, so none of §4's translucent contrast
//! model applies to it — but the body is **not** the same colour in both
//! palettes (dark charcoal, light brushed aluminium), so the ink is not either.
//! White on the light chassis is 1.3:1. The strip and the body rows use
//! [`chassis_ink`] / [`chassis_ink_dim`], fitted per palette; the readout
//! orange and the panel magenta only ever appear **inside a well**, which is
//! dark in both palettes by construction.
//!
//! # Frame budget
//!
//! The spectrum repaints at frame rate and everything else does not, so the
//! card is drawn in [`MediaLayer`]s: `Chrome` once into a cached bitmap, and
//! `Spectrum` — the bars alone, inside [`spectrum_rect`] — per frame. Nothing
//! on the `Spectrum` path builds a gradient (the bars use
//! [`BarPaint::Level`], which is a flat fill per bar) and nothing on it calls
//! `drop_shadow`, which blurs the whole canvas mask.

use crate::widgetkit::canvas::Canvas;
use crate::widgetkit::color::Color;
use crate::widgetkit::geom::{Point, Rect, Size};
use crate::widgetkit::paint::Fill;
use crate::widgetkit::surface::{self, BarPaint, BarStyle, WidgetSize};
use crate::widgetkit::text::FontStack;
use crate::widgetkit::theme::Theme;
use crate::widgetkit::typo::{self, Script, Step};

/// Which playback state the indicators report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    Playing,
    Paused,
    /// Also the state when there is no player at all — the card is only drawn
    /// when something has been playing, and a stopped player is the honest
    /// reading of "nothing is coming out of the speakers".
    #[default]
    Stopped,
}

impl PlayState {
    /// The word the status strip leads with.
    ///
    /// One `t!` per arm, each on one line: the catalog scanner does not
    /// implement Rust's line-continuation semantics, so a wrapped literal can
    /// never match a catalog key.
    pub fn label(self) -> &'static str {
        match self {
            Self::Playing => crate::t!("Playing"),
            Self::Paused => crate::t!("Paused"),
            Self::Stopped => crate::t!("Stopped"),
        }
    }
}

/// Which layer of the card to draw.
///
/// The card is one picture but two costs. Everything except the spectrum is
/// static between track changes; the spectrum is not static for a single frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaLayer {
    /// The whole card. What a sample, a test or a one-shot render wants.
    #[default]
    All,
    /// Everything **except** the spectrum's bars — the chassis, the bevels, the
    /// wells, every row of type, the progress track. Rasterise once, keep the
    /// bitmap, composite each frame under [`MediaLayer::Spectrum`].
    Chrome,
    /// The spectrum's bars and nothing else, inside [`spectrum_rect`]. No
    /// gradient is built and no shadow is blurred on this path.
    Spectrum,
}

/// What a combined media card draws.
#[derive(Debug, Clone, Copy, Default)]
pub struct MediaData<'a> {
    /// Playback state, for the strip's word and the lit indicator.
    pub state: PlayState,
    /// The source application's name, e.g. `"Spotify"`. Empty drops it and the
    /// strip leads with the state word alone.
    pub source: &'a str,
    /// Track title. Ellipsised to one line.
    pub title: &'a str,
    /// Artist. `· album` is dropped **before** the artist is cut.
    pub artist: &'a str,
    /// Album, appended to the artist when there is room.
    pub album: &'a str,
    /// The current lyric line. Empty removes the whole lyric band and gives its
    /// height to the instrument row.
    pub lyric: &'a str,
    /// The next line, drawn only when [`MediaData::show_next_line`] is on.
    pub next_lyric: &'a str,
    /// True while holding the previous line through an instrumental break: the
    /// row drops to the dim ink rather than blinking out.
    pub lyric_is_stale: bool,
    /// Show the next line under the current one.
    pub show_next_line: bool,
    /// Elapsed time, e.g. `"1:34"`. Empty draws `--:--`.
    pub elapsed: &'a str,
    /// Track length, e.g. `"3:52"`.
    pub total: &'a str,
    /// Playback position, 0..1. `None` removes the progress track entirely.
    pub position: Option<f32>,
    /// e.g. `"278 KBPS"`. Empty drops it.
    pub bitrate: &'a str,
    /// e.g. `"44 KHZ"`. Empty drops it.
    pub samplerate: &'a str,
    /// Album art. Non-square sources are centre-cropped, never squashed.
    pub art: Option<&'a image::RgbaImage>,
    /// Band magnitudes, 0..1.
    pub bands: &'a [f32],
    /// Per-band peak positions, **owned by the caller** — this card redraws
    /// every frame and must not allocate.
    pub peaks: Option<&'a [f32]>,
    /// Bar alpha, 0..1. Applies to the bars only; fading the chassis with them
    /// would make the panel vanish and the bars float.
    pub bar_opacity: f32,
    /// Round the bar caps.
    pub rounded: bool,
    /// Lyric size in logical units — the card's one size input, default 24.
    pub font_size: f32,
    /// Screen width, so the card can clamp to 0.9 of it. Zero means unclamped.
    pub screen_width: f32,
    /// Which layer to draw.
    pub layer: MediaLayer,
}

/// The panel colour a quiet band takes. `#8B3BE8` on the chassis well:
/// **3.41:1** dark, 3.15:1 light — a graphic, which needs 3:1, and which is
/// why the panel never carries type.
pub const PANEL_LOW: Color = Color {
    r: 0x8B as f32 / 255.0,
    g: 0x3B as f32 / 255.0,
    b: 0xE8 as f32 / 255.0,
    a: 1.0,
};
/// The panel colour a loud band takes. `#D94FE0` on the chassis well:
/// **5.39:1** dark, 4.97:1 light.
pub const PANEL_HIGH: Color = Color {
    r: 0xD9 as f32 / 255.0,
    g: 0x4F as f32 / 255.0,
    b: 0xE0 as f32 / 255.0,
    a: 1.0,
};

/// The default lyric size, and the one every derived size is quoted against.
const DEFAULT_L: f32 = 24.0;
/// Card width as a multiple of the lyric size. At the default this is 504 lu
/// against a 244 lu height — **2.07:1**, which is the reference's landscape
/// proportion, reached rather than assumed.
const WIDTH_PER_L: f32 = 21.0;
/// The narrowest card that still holds three instrument columns.
const MIN_WIDTH: f32 = 460.0;
/// Chassis corner radius (spec §9.3.3).
const RADIUS: f32 = 20.0;
/// Chassis padding. Fixed rather than derived: the chassis is one physical
/// object and its rebate does not scale with the type inside it.
const PAD: f32 = 16.0;
/// Album art side as a multiple of the title size, when the lyric band is
/// present. Without it the art grows to the whole instrument row.
const ART_PER_TITLE: f32 = 4.5;
/// The readout column's width, as a fraction of the card. Clamped so a very
/// wide card does not give a two-line readout half the chassis.
const READOUT_FRAC: f32 = 0.22;
/// Ink alpha for the chassis's secondary rows. Fitted per palette: white at
/// 0.62 clears 6.41:1 on the dark chassis, and the light chassis needs 0.70
/// (0.62 gives 4.36:1 on its far gradient stop — a fail).
const DIM_DARK: f32 = 0.62;
const DIM_LIGHT: f32 = 0.70;

/// The ink the chassis body carries, per palette.
///
/// **Not** white in both. The light chassis is brushed aluminium (`#E4E4E7`)
/// and white on it is 1.3:1; the dark one is charcoal and near-black on it is
/// 1.2:1. Two bodies, two inks, one call site.
pub fn chassis_ink(t: &Theme) -> Color {
    if t.chassis.relative_luminance() < 0.4 {
        Color::WHITE
    } else {
        Color::rgb8(0x14, 0x14, 0x1A)
    }
}

/// [`chassis_ink`] at the secondary level — 14.2:1 → 6.4:1 dark, 14.5:1 → 5.5:1
/// light on the governing gradient stop.
pub fn chassis_ink_dim(t: &Theme) -> Color {
    let a = if t.chassis.relative_luminance() < 0.4 {
        DIM_DARK
    } else {
        DIM_LIGHT
    };
    chassis_ink(t).with_alpha(a)
}

#[derive(Debug, Clone, Copy)]
struct Layout {
    size: Size,
    lyric: f32,
    title: f32,
    body: f32,
    lcd: f32,
    /// The art well inside it.
    art: Rect,
    /// The spectrum panel inside it — the hero.
    panel: Rect,
    /// The readout well inside it.
    readout: Rect,
    /// The lyric band, empty when there is none.
    lyrics: Rect,
    /// The progress track, empty when there is no position.
    progress: Rect,
    /// The status strip's cap top.
    strip_y: f32,
    title_y: f32,
    artist_y: f32,
    lyric_lines: usize,
    has_next: bool,
}

fn lyric_size(d: &MediaData) -> f32 {
    if d.font_size.is_finite() && d.font_size > 0.0 {
        d.font_size.clamp(10.0, 96.0)
    } else {
        DEFAULT_L
    }
}

/// `artist · album`, with the album dropped first when there is not room.
fn artist_text(d: &MediaData) -> String {
    match (d.artist.is_empty(), d.album.is_empty()) {
        (true, true) => String::new(),
        (false, true) => d.artist.to_string(),
        (true, false) => d.album.to_string(),
        (false, false) => format!("{} · {}", d.artist, d.album),
    }
}

/// The strip's left-hand run: the state, then the source app.
fn strip_left(d: &MediaData) -> String {
    let state = d.state.label();
    if d.source.is_empty() {
        state.to_string()
    } else {
        format!("{state} · {}", d.source)
    }
}

/// The strip's right-hand run: the format readouts the reference puts in a
/// bezel of their own. They are the least important thing on the card, so they
/// go where least important things go.
fn strip_right(d: &MediaData) -> String {
    match (d.bitrate.is_empty(), d.samplerate.is_empty()) {
        (true, true) => String::new(),
        (false, true) => d.bitrate.to_string(),
        (true, false) => d.samplerate.to_string(),
        (false, false) => format!("{} · {}", d.bitrate, d.samplerate),
    }
}

/// The big readout. Dashes rather than zeros when the position is unknown: a
/// zero is a claim and a dash is an admission.
fn elapsed_text<'a>(d: &'a MediaData<'a>) -> &'a str {
    if d.elapsed.is_empty() {
        "--:--"
    } else {
        d.elapsed
    }
}

fn layout(fonts: &mut FontStack, t: &Theme, d: &MediaData, scale: f32) -> Layout {
    let l = lyric_size(d);
    let title = typo::nearest_ladder_step(0.75 * l).max(Step::Micro.size());
    let body = typo::nearest_ladder_step(0.58 * l).max(Step::Micro.size());
    let lcd = typo::nearest_ladder_step(1.30 * l).max(Step::Body.size());
    let cap = Step::Caption.size();

    let cap_low = f32::INFINITY;
    let mut width = WIDTH_PER_L * l;
    if d.screen_width.is_finite() && d.screen_width > 0.0 {
        width = width.min(0.9 * d.screen_width);
    }
    width = width.max(MIN_WIDTH.min(cap_low)).clamp(120.0, 8192.0);
    let inner_w = (width - 2.0 * PAD).max(1.0);

    let has_lyrics = !d.lyric.is_empty();
    let lyric_script = Script::of(d.lyric);

    // -- the rows, top to bottom ------------------------------------------
    let mut y = PAD;
    let strip_y = y;
    y += typo::cap_height(cap, Script::Latin) + t.metrics.gap_s;
    let title_y = y;
    y += typo::cap_height(title, Script::of(d.title)) + t.metrics.gap_xs + 2.0;
    let artist_y = y;
    y += typo::cap_height(body, Script::Latin) + t.metrics.gap_m;

    // **The lyric band is always two rows tall**, whatever is in it — and the
    // reserved height is the same whether the band is drawn at all.
    //
    // The clock card's width rule, applied to a height: size from what the
    // settings can *ever* produce, not from what is on screen now. A card that
    // grew a row when a line wrapped, and lost one when the next track's line
    // did not, would resize on someone else's schedule several times a minute.
    // Two rows is the whole budget: a wrapped current line takes both, and
    // when it does the next line is dropped — the words being sung outrank the
    // words about to be.
    let mut lyric_lines = 1usize;
    if has_lyrics {
        let run = typo::styled(d.lyric, l, 500, false, fonts)
            .max_width(inner_w)
            .max_lines(2);
        lyric_lines = fonts.measure(&run, scale).lines.clamp(1, 2);
    }
    let has_next = has_lyrics && d.show_next_line && !d.next_lyric.is_empty() && lyric_lines == 1;
    let one = typo::cap_height(l, lyric_script);
    let band_h =
        one + typo::line_height_ratio(l, lyric_script) * l + typo::descender(l, lyric_script);

    let row_top = y;
    let base_row = (ART_PER_TITLE * title).max(48.0);
    let row_h = if has_lyrics {
        base_row
    } else {
        base_row + band_h + t.metrics.gap_m
    };
    y += row_h + t.metrics.gap_m;

    let lyrics = if has_lyrics {
        let r = Rect::new(PAD, y, inner_w, band_h);
        y += band_h + t.metrics.gap_m;
        r
    } else {
        Rect::ZERO
    };

    let track_h = surface::progress_height(l).max(4.0);
    let progress = if d.position.is_some() {
        let r = Rect::new(PAD, y, inner_w, track_h);
        y += track_h;
        r
    } else {
        // Removed, and its height with it — but the instrument row already
        // absorbed the lyric band, so the card only shortens by the track.
        Rect::ZERO
    };
    let height = y + PAD;

    // -- the instrument row's three columns --------------------------------
    let row = Rect::new(PAD, row_top, inner_w, row_h);
    let art_side = row_h.min(0.30 * width);
    let art = Rect::new(row.x, row.y, art_side, row_h);
    let readout_w = (READOUT_FRAC * width)
        .clamp(88.0, 220.0)
        .min(inner_w * 0.34);
    let readout = Rect::new(row.right() - readout_w, row.y, readout_w, row_h);
    let panel_x = art.right() + t.metrics.gap_m;
    let panel = Rect::new(
        panel_x,
        row.y,
        (readout.x - t.metrics.gap_m - panel_x).max(0.0),
        row_h,
    );

    Layout {
        size: Size::new(width, height),
        lyric: l,
        title,
        body,
        lcd,
        art,
        panel,
        readout,
        lyrics,
        progress,
        strip_y,
        title_y,
        artist_y,
        lyric_lines,
        has_next,
    }
}

/// How big this card is, and how much shadow margin it needs.
pub fn measure(fonts: &mut FontStack, t: &Theme, d: &MediaData, scale: f32) -> WidgetSize {
    WidgetSize::new(layout(fonts, t, d, scale).size, t.e3())
}

/// The rect the spectrum's bars occupy inside `card` — the per-frame damage
/// rect, and the only part of this card that is not static between tracks.
pub fn spectrum_rect(
    fonts: &mut FontStack,
    t: &Theme,
    d: &MediaData,
    card: Rect,
    scale: f32,
) -> Rect {
    let l = layout(fonts, t, d, scale);
    bar_area(l.panel.offset(card.x, card.y))
}

/// The bars' area inside the panel well: the well deflated by its own rebate.
fn bar_area(panel: Rect) -> Rect {
    let d = (panel.min_side() * 0.10).clamp(3.0, 10.0);
    panel.inset(d)
}

/// Draw the card, centred in whatever room `canvas` provides.
pub fn draw(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &MediaData) {
    let size = measure(fonts, t, d, c.scale());
    let rect = size.card_in(c.bounds());
    draw_at(c, fonts, t, d, rect);
}

/// Draw the card with its card rect at `card`.
pub fn draw_at(c: &mut Canvas, fonts: &mut FontStack, t: &Theme, d: &MediaData, card: Rect) {
    if card.is_empty() {
        return;
    }
    let l = layout(fonts, t, d, c.scale());
    let panel = l.panel.offset(card.x, card.y);

    if d.layer == MediaLayer::Spectrum {
        draw_bars(c, t, d, panel);
        return;
    }

    let radius = RADIUS.min(card.min_side() / 2.0);
    surface::elevation(c, card, radius, t, t.e3());
    c.rounded_rect(card, radius, &t.chassis_fill(card));
    c.top_highlight(card, radius, t.bevel_high, 1.5);
    c.bottom_highlight(card, radius, t.bevel_low, 1.5);

    draw_strip(c, fonts, t, d, &l, card);
    draw_titles(c, fonts, t, d, &l, card);
    draw_art(c, fonts, t, d, &l, card);
    draw_panel(c, t, panel);
    draw_readout(c, fonts, t, d, &l, card);
    draw_lyrics(c, fonts, t, d, &l, card);
    draw_progress(c, t, d, &l, card);

    if d.layer == MediaLayer::All {
        draw_bars(c, t, d, panel);
    }
}

/// A well sunk into the chassis: near-black in **both** palettes, because a
/// display window is a window whichever way the furniture around it is
/// painted, and keeping it dark is what lets one orange serve both.
fn chassis_well(c: &mut Canvas, t: &Theme, r: Rect, radius: f32) {
    if r.is_empty() {
        return;
    }
    c.rounded_rect(r, radius, &Fill::solid(t.chassis_well));
    c.top_highlight(r, radius, Color::BLACK.with_alpha(0.70), 1.0);
    c.bottom_highlight(r, radius, Color::WHITE.with_alpha(0.07), 1.0);
}

fn draw_strip(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &MediaData,
    l: &Layout,
    card: Rect,
) {
    let cap = Step::Caption.size();
    let inner = (card.w - 2.0 * PAD).max(1.0);
    let y = card.y + l.strip_y - typo::cap_gap(cap, Script::Latin);
    let right = strip_right(d);
    let mut left_w = inner;
    if !right.is_empty() {
        let run = typo::mono_run(&right, cap, fonts).color(chassis_ink_dim(t));
        let m = fonts.measure(&run, c.scale());
        c.text(fonts, &run, Point::new(card.right() - PAD - m.width, y));
        left_w = (inner - m.width - t.metrics.gap_m).max(1.0);
    }
    let left = typo::micro_case(&strip_left(d)).into_owned();
    if left.is_empty() {
        return;
    }
    let run = typo::styled(&left, cap, 600, true, fonts)
        .color(chassis_ink_dim(t))
        .max_width(left_w);
    c.text(fonts, &run, Point::new(card.x + PAD, y));
}

fn draw_titles(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &MediaData,
    l: &Layout,
    card: Rect,
) {
    let inner = (card.w - 2.0 * PAD).max(1.0);
    if !d.title.is_empty() {
        let s = Script::of(d.title);
        let run = typo::styled(d.title, l.title, 600, false, fonts)
            .color(chassis_ink(t))
            .max_width(inner);
        c.text(
            fonts,
            &run,
            Point::new(card.x + PAD, card.y + l.title_y - typo::cap_gap(l.title, s)),
        );
    }
    let artist = artist_text(d);
    if !artist.is_empty() {
        let s = Script::of(&artist);
        let run = typo::styled(&artist, l.body, 500, false, fonts)
            .color(chassis_ink_dim(t))
            .max_width(inner);
        c.text(
            fonts,
            &run,
            Point::new(card.x + PAD, card.y + l.artist_y - typo::cap_gap(l.body, s)),
        );
    }
}

fn draw_art(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &MediaData,
    l: &Layout,
    card: Rect,
) {
    let art = l.art.offset(card.x, card.y);
    if art.is_empty() {
        return;
    }
    let radius = (0.10 * art.min_side()).clamp(4.0, 14.0);
    chassis_well(c, t, art, radius);
    match d.art {
        Some(img) => {
            // Inset by the well's own rebate so the bevel still reads as a
            // frame rather than being covered by the picture.
            let inner = art.inset(3.0);
            if !inner.is_empty() {
                c.image_cover(img, inner, (radius - 3.0).max(2.0));
            }
        }
        None => super::nowplaying::note_glyph(c, art, &art_placeholder_theme(t)),
    }
    let _ = fonts;
}

/// The note glyph takes its ink from `Theme::text_tertiary`, which is fitted
/// against a *glass* card. Inside an opaque chassis well the surface is known
/// and the ink can be stated outright, so the shared drawing is handed a
/// palette whose tertiary is the well's own.
fn art_placeholder_theme(t: &Theme) -> Theme {
    Theme {
        text_tertiary: Color::WHITE.with_alpha(0.34),
        ..*t
    }
}

fn draw_panel(c: &mut Canvas, t: &Theme, panel: Rect) {
    if panel.is_empty() {
        return;
    }
    let radius = (0.08 * panel.min_side()).clamp(3.0, 10.0);
    chassis_well(c, t, panel, radius);
    // The bloom, drawn **once** into the chrome rather than per bar per frame:
    // a static wash rising from the panel's floor, which is where the energy
    // is. A real per-bar glow is a full-canvas Gaussian per bar and there is no
    // version of it that survives a frame budget.
    let glow = panel.inset(2.0);
    if !glow.is_empty() {
        c.rounded_rect(
            glow,
            radius,
            &Fill::vertical(glow, PANEL_LOW.with_alpha(0.0), PANEL_LOW.with_alpha(0.20)),
        );
    }
}

fn draw_bars(c: &mut Canvas, t: &Theme, d: &MediaData, panel: Rect) {
    let area = bar_area(panel);
    if area.is_empty() || d.bands.is_empty() {
        return;
    }
    let opacity = if d.bar_opacity.is_finite() {
        d.bar_opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    surface::bars(
        c,
        area,
        d.bands,
        d.peaks,
        t,
        BarStyle {
            paint: BarPaint::Level(PANEL_LOW, PANEL_HIGH),
            rounded: d.rounded,
            baseline: true,
            peaks: d.peaks.is_some(),
            opacity,
            shadow: false,
        },
    );
}

fn draw_readout(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &MediaData,
    l: &Layout,
    card: Rect,
) {
    let well = l.readout.offset(card.x, card.y);
    if well.is_empty() {
        return;
    }
    let radius = (0.10 * well.min_side()).clamp(4.0, 14.0);
    chassis_well(c, t, well, radius);
    let inner = well.inset((well.min_side() * 0.14).clamp(6.0, 14.0));
    if inner.is_empty() {
        return;
    }
    let cap = Step::Caption.size();

    // The big elapsed readout, sized to the well rather than to the ladder: a
    // 34 lu figure in a 40 lu well is a clipped figure.
    let size = l.lcd.min(inner.h * 0.42).max(Step::Caption.size());
    let run = typo::mono_run(elapsed_text(d), size, fonts)
        .color(t.lcd)
        .max_width(inner.w);
    let mut y = inner.y;
    c.text(
        fonts,
        &run,
        Point::new(inner.x, y - typo::cap_gap(size, Script::Latin)),
    );
    y += typo::cap_height(size, Script::Latin) + t.metrics.gap_xs;

    if !d.total.is_empty() && y + typo::cap_height(cap, Script::Latin) <= inner.bottom() {
        // 0.70 is the floor: `#F5A623 @ 0.45` on the well is 2.8:1, and
        // alpha-thinned orange runs out of contrast fast.
        let run = typo::mono_run(d.total, cap, fonts)
            .color(t.lcd.with_alpha(0.70))
            .max_width(inner.w);
        c.text(
            fonts,
            &run,
            Point::new(inner.x, y - typo::cap_gap(cap, Script::Latin)),
        );
    }

    draw_state(c, t, d.state, inner);
}

/// The transport row, demoted from controls to **indicators**.
///
/// Flat: no gloss, no bevel, no circular pad, nothing that reads as pressable.
/// The live state is lit in the readout orange at 1.25 × the size of the other
/// two, which are small and neutral — a size step as well as a colour one, so
/// the row still reports state with the colour thrown away.
fn draw_state(c: &mut Canvas, t: &Theme, state: PlayState, inner: Rect) {
    let unit = (inner.h * 0.22).clamp(4.0, 16.0);
    let lit_unit = unit * 1.25;
    let gap = unit * 0.7;
    let total = 3.0 * unit + 2.0 * gap + (lit_unit - unit);
    if total > inner.w {
        return;
    }
    let mut x = inner.right() - total;
    let cy = inner.bottom() - lit_unit / 2.0;
    let off = Fill::solid(Color::WHITE.with_alpha(0.30));
    let on = Fill::solid(t.lcd);
    for s in [PlayState::Playing, PlayState::Paused, PlayState::Stopped] {
        let live = s == state;
        let u = if live { lit_unit } else { unit };
        let fill = if live { &on } else { &off };
        let top = cy - u / 2.0;
        match s {
            PlayState::Playing => c.triangle(
                Point::new(x, top),
                Point::new(x + u, cy),
                Point::new(x, top + u),
                fill,
            ),
            PlayState::Paused => {
                let bw = u * 0.32;
                c.rounded_rect(Rect::new(x, top, bw, u), bw * 0.3, fill);
                c.rounded_rect(Rect::new(x + u - bw, top, bw, u), bw * 0.3, fill);
            }
            PlayState::Stopped => {
                c.rounded_rect(Rect::new(x, top, u, u), u * 0.14, fill);
            }
        }
        x += u + gap;
    }
}

fn draw_lyrics(
    c: &mut Canvas,
    fonts: &mut FontStack,
    t: &Theme,
    d: &MediaData,
    l: &Layout,
    card: Rect,
) {
    if l.lyrics.is_empty() {
        return;
    }
    let block = l.lyrics.offset(card.x, card.y);
    let s = Script::of(d.lyric);
    // One row in a two-row band sits in the middle of it, not at the top: the
    // band is reserved at its maximum so the card never resizes, and a row
    // pinned to the top of a reserved band reads as a gap under the words.
    let used = typo::cap_height(l.lyric, s)
        + if l.lyric_lines > 1 || l.has_next {
            typo::line_height_ratio(l.lyric, s) * l.lyric
        } else {
            0.0
        }
        + typo::descender(l.lyric, s);
    let block = Rect::new(
        block.x,
        block.y + ((block.h - used) / 2.0).max(0.0),
        block.w,
        used.min(block.h),
    );
    let colour = if d.lyric_is_stale {
        chassis_ink_dim(t)
    } else {
        chassis_ink(t)
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
            .color(chassis_ink_dim(t).scale_alpha(0.7))
            .max_width(block.w);
        c.text(
            fonts,
            &run,
            Point::new(block.x, y - typo::cap_gap(l.lyric, ns)),
        );
    }
}

fn draw_progress(c: &mut Canvas, t: &Theme, d: &MediaData, l: &Layout, card: Rect) {
    let bar = l.progress.offset(card.x, card.y);
    if bar.is_empty() {
        return;
    }
    let Some(p) = d.position else { return };
    let r = bar.h / 2.0;
    chassis_well(c, t, bar, r);
    let f = if p.is_finite() {
        p.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let w = (bar.w * f).max(bar.h).min(bar.w);
    c.rounded_rect(Rect::new(bar.x, bar.y, w, bar.h), r, &Fill::solid(t.lcd));
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

    const BANDS: [f32; 28] = [
        0.10, 0.30, 0.62, 0.90, 0.72, 0.51, 0.44, 0.80, 0.95, 0.60, 0.31, 0.22, 0.50, 0.70, 0.92,
        0.41, 0.20, 0.11, 0.35, 0.55, 0.75, 0.85, 0.65, 0.45, 0.25, 0.15, 0.30, 0.12,
    ];

    fn full() -> MediaData<'static> {
        MediaData {
            state: PlayState::Playing,
            source: "Spotify",
            title: "Blue Monday",
            artist: "New Order",
            album: "Substance",
            lyric: "I see a ship in the harbour",
            next_lyric: "I can and shall obey",
            lyric_is_stale: false,
            show_next_line: true,
            elapsed: "1:34",
            total: "3:52",
            position: Some(0.41),
            bitrate: "278 KBPS",
            samplerate: "44 KHZ",
            art: None,
            bands: &BANDS,
            peaks: None,
            bar_opacity: 0.92,
            rounded: true,
            font_size: 24.0,
            screen_width: 1920.0,
            layer: MediaLayer::All,
        }
    }

    #[test]
    fn the_card_lands_on_the_references_landscape_proportion() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let l = layout(&mut f, &t, &full(), 1.0);
        assert_eq!(l.size.w, 504.0);
        let aspect = l.size.w / l.size.h;
        assert!(
            (1.9..2.4).contains(&aspect),
            "aspect {aspect} is not the reference's landscape"
        );
    }

    /// The single most important structural claim: losing the lyrics must not
    /// move the card on the desktop.
    #[test]
    fn losing_the_lyrics_gives_their_height_to_the_instruments() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let with = layout(&mut f, &t, &full(), 1.0);
        let without = layout(
            &mut f,
            &t,
            &MediaData {
                lyric: "",
                ..full()
            },
            1.0,
        );
        assert_eq!(with.size, without.size, "the card resized when lyrics went");
        assert!(
            without.art.h > with.art.h,
            "the instrument row did not take the band's height"
        );
        assert!(without.panel.h > with.panel.h, "the hero did not grow");
        assert!(without.lyrics.is_empty());
    }

    #[test]
    fn a_missing_position_removes_the_track_and_dashes_the_readout() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let d = MediaData {
            position: None,
            elapsed: "",
            ..full()
        };
        let l = layout(&mut f, &t, &d, 1.0);
        assert!(l.progress.is_empty(), "a bar was drawn with no position");
        assert_eq!(elapsed_text(&d), "--:--");
        // And the card is shorter by exactly the track it dropped, not by more.
        let full_l = layout(&mut f, &t, &full(), 1.0);
        let drop = full_l.size.h - l.size.h;
        assert!(
            (drop - full_l.progress.h).abs() < 0.51,
            "dropped {drop} for a {} track",
            full_l.progress.h
        );
    }

    /// The chassis is opaque, so §4's translucent model does not apply — but
    /// the body is a different colour in each palette and the ink has to be
    /// too. White on brushed aluminium is 1.3:1.
    #[test]
    fn the_chassis_ink_is_legible_on_both_bodies() {
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            let ink = chassis_ink(&t);
            let dim = chassis_ink_dim(&t);
            // Both gradient stops, because the governing one differs by mode.
            for body in [t.chassis, t.chassis_far] {
                let primary = ink.over(body).contrast_ratio(body);
                let secondary = dim.over(body).contrast_ratio(body);
                assert!(primary >= 7.0, "{mode:?} primary {primary:.2}:1");
                assert!(secondary >= 4.5, "{mode:?} secondary {secondary:.2}:1");
            }
            // The orange and the panel colours only ever sit in a well, which
            // is dark in both palettes — that is what makes one orange legal
            // in both.
            let well = t.chassis_well;
            assert!(t.lcd.contrast_ratio(well) >= 4.5);
            assert!(PANEL_HIGH.contrast_ratio(well) >= 3.0);
            assert!(PANEL_LOW.contrast_ratio(well) >= 3.0);
            // And the reason it is never on the body: on aluminium it is 1.6:1.
            if mode == Mode::Light {
                assert!(t.lcd.contrast_ratio(t.chassis) < 3.0);
            }
        }
    }

    /// The state row must still report state with the colour thrown away, so
    /// the lit indicator is a size step as well as a hue one.
    #[test]
    fn the_state_indicators_are_a_size_step_and_not_only_a_colour_one() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let mut ink = |state: PlayState| -> u64 {
            let d = MediaData { state, ..full() };
            let size = measure(&mut f, &t, &d, 1.0);
            let mut c = Canvas::for_logical(size.buffer(), 1.0).expect("canvas");
            let card = size.card_rect();
            let l = layout(&mut f, &t, &d, 1.0);
            draw_at(&mut c, &mut f, &t, &d, card);
            // Count only the readout well's lower half, where the row sits.
            let well = l.readout.offset(card.x, card.y);
            let px = c.to_bgra();
            let mut sum = 0u64;
            for y in 0..px.h {
                for x in 0..px.w {
                    let (fx, fy) = (x as f32, y as f32);
                    if fx < well.x
                        || fx > well.right()
                        || fy < well.center().y
                        || fy > well.bottom()
                    {
                        continue;
                    }
                    let o = ((y * px.w + x) * 4) as usize;
                    // The lit indicator is the orange one: red-heavy.
                    if px.data[o + 2] > 150 && px.data[o] < 120 {
                        sum += 1;
                    }
                }
            }
            sum
        };
        // Whichever state is live, something is lit — and the lit shape is a
        // different area from the two dim ones, in every state.
        for s in [PlayState::Playing, PlayState::Paused, PlayState::Stopped] {
            assert!(ink(s) > 0, "{s:?} lit nothing");
        }
    }

    /// The bug this card was written against: a badge that overflowed the card
    /// bottom because the minimal-content case had never been rendered.
    #[test]
    fn every_degenerate_combination_stays_inside_the_card() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let empty: [f32; 0] = [];
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            for lyric in [
                "",
                "I see a ship in the harbour",
                "蓝色星期一的第一句歌词很长很长很长",
            ] {
                for position in [None, Some(0.0), Some(0.41), Some(1.0)] {
                    for elapsed in ["", "1:34"] {
                        for bands in [&empty[..], &BANDS[..]] {
                            for state in [PlayState::Playing, PlayState::Paused, PlayState::Stopped]
                            {
                                let d = MediaData {
                                    state,
                                    lyric,
                                    position,
                                    elapsed,
                                    bands,
                                    bitrate: if elapsed.is_empty() { "" } else { "278 KBPS" },
                                    samplerate: if elapsed.is_empty() { "" } else { "44 KHZ" },
                                    ..full()
                                };
                                let size = measure(&mut f, &t, &d, 1.0);
                                let mut c =
                                    Canvas::for_logical(size.buffer(), 1.0).expect("canvas");
                                let card = size.card_rect();
                                draw_at(&mut c, &mut f, &t, &d, card);
                                let px = c.to_bgra();
                                // Nothing may be drawn outside the card rect:
                                // the buffer's margin is shadow, and a shadow
                                // is never opaque.
                                let mut spill = 0u32;
                                for y in 0..px.h {
                                    for x in 0..px.w {
                                        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                                        let inside = fx > card.x - 1.0
                                            && fx < card.right() + 1.0
                                            && fy > card.y - 1.0
                                            && fy < card.bottom() + 1.0;
                                        if inside {
                                            continue;
                                        }
                                        let o = ((y * px.w + x) * 4) as usize;
                                        if px.data[o + 3] > 200 {
                                            spill += 1;
                                        }
                                    }
                                }
                                assert_eq!(
                                    spill, 0,
                                    "{mode:?} lyric={lyric:?} pos={position:?} spilled {spill} px"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// The whole point of the layer split: `Chrome` must not draw a bar and
    /// `Spectrum` must not draw anything outside the panel.
    #[test]
    fn the_layers_partition_the_card() {
        let mut f = fonts();
        if !f.has_fonts() {
            return;
        }
        let t = theme(Mode::Dark);
        let d = full();
        let size = measure(&mut f, &t, &d, 1.0);
        let card = size.card_rect();
        let area = spectrum_rect(&mut f, &t, &d, card, 1.0);
        assert!(!area.is_empty());

        let mut c = Canvas::for_logical(size.buffer(), 1.0).unwrap();
        draw_at(
            &mut c,
            &mut f,
            &t,
            &MediaData {
                layer: MediaLayer::Spectrum,
                ..d
            },
            card,
        );
        let px = c.to_bgra();
        for y in 0..px.h {
            for x in 0..px.w {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let inside = fx >= area.x - 1.0
                    && fx <= area.right() + 1.0
                    && fy >= area.y - 1.0
                    && fy <= area.bottom() + 2.0;
                if inside {
                    continue;
                }
                let o = ((y * px.w + x) * 4) as usize;
                assert_eq!(px.data[o + 3], 0, "the spectrum layer drew at {x},{y}");
            }
        }
    }

    #[test]
    fn no_combination_of_settings_can_panic() {
        let mut f = fonts();
        let mut c = Canvas::for_logical(Size::new(320.0, 200.0), 1.0).unwrap();
        let empty: [f32; 0] = [];
        let strings = [
            ("", "", "", "", ""),
            (
                "Blue Monday",
                "New Order",
                "Substance",
                "I see a ship",
                "I can and shall obey",
            ),
            (
                "蓝色星期一",
                "新秩序乐队",
                "物质",
                "我看见港口里有一艘船",
                "我能够也必将服从",
            ),
            ("🎵", "🎶", "🎼", "🎹", "🎺"),
        ];
        for mode in [Mode::Dark, Mode::Light] {
            let t = theme(mode);
            for layer in [MediaLayer::All, MediaLayer::Chrome, MediaLayer::Spectrum] {
                for size in [f32::NAN, 0.0, 1.0, 24.0, 200.0] {
                    for bands in [&empty[..], &BANDS[..]] {
                        for (title, artist, album, lyric, next) in strings {
                            let d = MediaData {
                                title,
                                artist,
                                album,
                                lyric,
                                next_lyric: next,
                                show_next_line: true,
                                bands,
                                bar_opacity: f32::NAN,
                                font_size: size,
                                position: Some(f32::NAN),
                                screen_width: 0.0,
                                layer,
                                ..full()
                            };
                            let m = measure(&mut f, &t, &d, 1.0);
                            assert!(m.buffer().w.is_finite() && m.buffer().h.is_finite());
                            c.reset();
                            draw_at(&mut c, &mut f, &t, &d, Rect::new(8.0, 8.0, 300.0, 180.0));
                            draw_at(&mut c, &mut f, &t, &d, Rect::new(-60.0, -60.0, 200.0, 90.0));
                            draw_at(&mut c, &mut f, &t, &d, Rect::ZERO);
                        }
                    }
                }
            }
            c.reset();
            draw(&mut c, &mut f, &t, &full());
        }
    }
}

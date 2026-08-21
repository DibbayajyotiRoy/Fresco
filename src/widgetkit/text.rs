//! Text: system fonts, shaping, measurement, ellipsis.
//!
//! # Which text stack, and what it cost
//!
//! **cosmic-text**, over `fontdue`, `ab_glyph` and bare `swash`. The deciding
//! requirement is not rendering quality — all four rasterise glyphs fine — it
//! is *finding the right font at all*:
//!
//! * **Real system fonts.** A widget has to look like it belongs to the
//!   desktop, so it must resolve the user's installed families by name.
//!   cosmic-text carries `fontdb`, which enumerates `/usr/share/fonts`,
//!   `~/.local/share/fonts` and the rest, and (with the default `fontconfig`
//!   feature) reads fontconfig's *configuration* to honour user aliases.
//!   `fontdue` and `ab_glyph` take bytes you already loaded; discovering those
//!   bytes would be our problem, and re-implementing font discovery is exactly
//!   the kind of thing that works on the author's machine and nowhere else.
//! * **CJK fallback, which is not optional.** Fresco ships a Simplified-Chinese
//!   UI and has active users in China (see the zh-CN catalogs under `i18n/`).
//!   A stack that renders `♦♦♦` for a Chinese track title is unshippable. Only
//!   cosmic-text does per-script fallback out of the box — see
//!   [`FontStack`]'s docs for the exact chain.
//! * **Kerning and complex shaping.** cosmic-text shapes with HarfBuzz-class
//!   logic (`harfrust`), so GPOS kerning, ligatures and Arabic/Indic joining
//!   all work. `fontdue`/`ab_glyph` advance glyph-by-glyph, which is visibly
//!   loose at the large display sizes a clock uses.
//!
//! **The tradeoff we accepted:** cosmic-text is the heaviest of the four.
//! It pulls a shaper, `skrifa`, `fontdb` and a pile of `unicode-*` tables —
//! roughly 30 extra crates and a noticeably longer cold build than
//! `ab_glyph`'s two. That is real, and we took it, because the alternative is
//! not "a lighter text stack", it is "a text stack that cannot render our
//! Chinese users' track titles". Every one of those crates is pure Rust, which
//! is the constraint that actually cannot bend: no new C or system library
//! enters this build (Fresco already bundles `mpvpaper` because one of its
//! dependencies was not packaged, and we are not repeating that).
//!
//! # A behaviour change worth knowing about
//!
//! The ASS widgets get their fonts from **libass, which uses fontconfig**, so
//! today a missing family is silently substituted by fontconfig's rules (see
//! `crate::lyrics`' notes on font selection). cosmic-text does *not* run
//! fontconfig's substitution engine — it reads the config for aliases and then
//! applies its own matching. So a family name that fontconfig silently
//! rewrites may resolve differently here than in the ASS widgets. That is a
//! deliberate change, not a regression: the fallback chain below is explicit
//! and inspectable, where fontconfig's is neither.
//!
//! # Cost model
//!
//! [`FontStack::system`] **scans the filesystem**: tens to hundreds of
//! milliseconds on a machine with a full font set. The daemon's run loop ticks
//! every 100 ms, so it must never do that on the loop. Build the stack once,
//! off the loop, and hand it to every draw; nothing in this module lazily
//! initialises a font database behind your back.

use cosmic_text::{Attrs, Buffer, Ellipsize, EllipsizeHeightLimit, Metrics, Shaping, SwashCache};

use super::color::Color;
use super::geom::Size;

/// Upper clamp on a type size, in logical units. Well past any plausible
/// widget (a 4K full-height digit is ~1000 logical units) and far short of the
/// values that make a shaper allocate wildly.
const MAX_TYPE_SIZE: f32 = 2048.0;

/// cosmic-text's font database handle, re-exported so a caller can build one
/// (off the daemon loop — see the module docs) without depending on
/// cosmic-text directly.
pub use cosmic_text::FontSystem;

/// Horizontal alignment of a text block **within its `max_width`**.
///
/// Without a `max_width` there is no box to align inside, so every value
/// behaves as [`TextAlign::Start`]. That is cosmic-text's semantic and we do not
/// paper over it: silently inventing a box would misplace text whenever the
/// caller's own layout already positioned it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Flush left (flush right in an RTL paragraph).
    #[default]
    Start,
    /// Centred in `max_width`.
    Center,
    /// Flush right (flush left in an RTL paragraph).
    End,
}

impl TextAlign {
    fn to_cosmic(self) -> cosmic_text::Align {
        match self {
            Self::Start => cosmic_text::Align::Left,
            Self::Center => cosmic_text::Align::Center,
            Self::End => cosmic_text::Align::Right,
        }
    }
}

/// One styled block of text: what to draw, how, and how much room it may take.
///
/// All sizes are **logical** units (see `super::geom`), so one `TextRun`
/// renders correctly at 1080p and at 4K with only the canvas `scale` changing.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    /// The string. May contain `\n`; each is a hard line break.
    pub text: String,
    /// Family name, or `None` for the desktop's default sans-serif. `None` is
    /// the right choice for almost everything: a widget should inherit the
    /// desktop's font, not impose one.
    pub family: Option<String>,
    /// OpenType weight: 400 regular, 500 medium, 600 semibold, 700 bold.
    /// Fresco's reference designs lean on weight rather than colour for
    /// hierarchy, because weight survives any wallpaper behind it.
    pub weight: u16,
    /// Italic rather than roman.
    pub italic: bool,
    /// Type size in logical units.
    pub size: f32,
    /// Baseline-to-baseline distance in logical units. Explicit rather than a
    /// multiplier because a card's vertical rhythm is designed in pixels.
    pub line_height: f32,
    /// Tracking in logical units, positive to open up. All-caps micro-labels
    /// need it; body text does not.
    pub letter_spacing: f32,
    /// Ink colour. Per-glyph colour is not supported — draw two runs.
    pub color: Color,
    /// Width budget in logical units. `None` means unbounded, which also
    /// disables wrapping, alignment and ellipsis.
    pub max_width: Option<f32>,
    /// How many lines may be drawn. `1` (the default) means no wrapping at all;
    /// anything more wraps at `max_width` and ellipsises the last line.
    pub max_lines: usize,
    /// Alignment inside `max_width`.
    pub align: TextAlign,
}

impl TextRun {
    /// A single-line run in the desktop's default sans at `size`, unbounded.
    pub fn new(text: impl Into<String>, size: f32) -> Self {
        Self {
            text: text.into(),
            family: None,
            weight: 400,
            italic: false,
            size,
            // 1.25 × size is a conventional default; every card should override.
            line_height: size * 1.25,
            letter_spacing: 0.0,
            color: Color::WHITE,
            max_width: None,
            max_lines: 1,
            align: TextAlign::Start,
        }
    }

    /// Set the family by name. `None` restores the desktop default.
    pub fn family(mut self, family: Option<impl Into<String>>) -> Self {
        self.family = family.map(Into::into);
        self
    }
    /// Set the OpenType weight.
    pub fn weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }
    /// Italic or roman.
    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
    /// Set the baseline-to-baseline distance in logical units.
    pub fn line_height(mut self, lh: f32) -> Self {
        self.line_height = lh;
        self
    }
    /// Set the tracking in logical units.
    pub fn letter_spacing(mut self, ls: f32) -> Self {
        self.letter_spacing = ls;
        self
    }
    /// Set the ink colour.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
    /// Constrain to `w` logical units, ellipsising what does not fit.
    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }
    /// Allow up to `lines` wrapped lines. Values below 1 are treated as 1.
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = lines.max(1);
        self
    }
    /// Set the alignment within `max_width`.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Lines actually permitted, never zero.
    fn lines(&self) -> usize {
        self.max_lines.max(1)
    }

    /// Type size in logical units, guaranteed finite and usable.
    ///
    /// A size of zero, a negative one, a NaN or an infinity all mean "this run
    /// cannot be drawn", and every entry point turns that into an empty
    /// measurement rather than into a shaping call with a poisoned metric.
    /// The upper clamp is not decoration either: cosmic-text will happily try
    /// to rasterise a 10^9-unit glyph.
    fn drawable_size(&self) -> Option<f32> {
        (self.size.is_finite() && self.size > 0.0).then(|| self.size.min(MAX_TYPE_SIZE))
    }

    /// Baseline-to-baseline distance in logical units, never below the type
    /// size (overlapping lines are always a mistake, never a design).
    fn drawable_line_height(&self) -> f32 {
        let size = self.drawable_size().unwrap_or(1.0);
        if self.line_height.is_finite() && self.line_height > 0.0 {
            self.line_height.min(MAX_TYPE_SIZE * 4.0).max(size)
        } else {
            size * 1.25
        }
    }

    fn attrs(&self) -> Attrs<'_> {
        let mut a = Attrs::new()
            .weight(cosmic_text::Weight(self.weight))
            .style(if self.italic {
                cosmic_text::Style::Italic
            } else {
                cosmic_text::Style::Normal
            });
        if let Some(name) = &self.family {
            a = a.family(cosmic_text::Family::Name(name));
        }
        // cosmic-text expresses tracking as a fraction of the em, which makes
        // it scale-free — exactly what we want, since `size` is already the
        // logical size and the canvas scale multiplies both.
        if self.letter_spacing != 0.0 && self.size > 0.0 {
            a = a.letter_spacing(self.letter_spacing / self.size);
        }
        a
    }
}

/// What a laid-out [`TextRun`] occupies, in **logical** units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// Width of the widest visible line — the *inked* extent, so a centred run
    /// still reports its text width, not its `max_width`.
    pub width: f32,
    /// `lines × line_height`, not the ink height. Card layout stacks boxes, and
    /// a box whose height depended on whether the string had a descender would
    /// make rows jitter as the text changed.
    pub height: f32,
    /// Number of visible lines.
    pub lines: usize,
    /// True when something was dropped to fit — the string is longer than
    /// `max_width`, or wrapped past `max_lines`. Lets a card react (shrink the
    /// size, scroll, choose a shorter label) instead of silently ellipsising.
    pub truncated: bool,
}

impl TextMetrics {
    /// The measured extent as a [`Size`].
    pub fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Nothing at all — what an empty string measures.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
        lines: 0,
        truncated: false,
    };
}

/// The font database, the glyph raster cache and one reusable shaping buffer.
///
/// # Build it once
///
/// [`FontStack::system`] scans the filesystem for fonts. Construct it off the
/// daemon's run loop and keep it for the process's life: it is also the *cache*
/// (rasterised glyphs, shaped runs), so a fresh one per repaint would rebuild
/// every glyph the clock has ever drawn.
///
/// # The fallback chain
///
/// For each word cosmic-text tries, in order:
///
/// 1. the requested [`TextRun::family`], or the desktop's default sans-serif
///    when it is `None`;
/// 2. **script-specific families** — for Han, Hiragana, Katakana and Hangul
///    that is `Noto Sans CJK SC/TC/HK/JP/KR` chosen by the process locale
///    (Simplified Chinese is the default, which is the right default for
///    Fresco's zh-CN users);
/// 3. common families: `Noto Sans`, `DejaVu Sans`, `FreeSans`, then the mono
///    and symbol Notos, then `Noto Color Emoji`;
/// 4. **every other installed font**, in match order.
///
/// Step 4 is why a Chinese title still renders on a machine with, say,
/// WenQuanYi or Source Han Sans but no Noto CJK: the named lists are a
/// preference, not a requirement. It is also why "no glyph at all" only happens
/// when *nothing* installed covers the codepoint.
pub struct FontStack {
    fonts: FontSystem,
    swash: SwashCache,
    /// Reused across every measure and draw so shaping does not allocate a
    /// fresh line buffer per frame. Swapped out via `mem::replace` while in
    /// use, because cosmic-text needs `&mut Buffer` and `&mut FontSystem` at
    /// the same time and they cannot both come from `&mut self`.
    scratch: Buffer,
    /// Resolved once, on first use: `None` until asked, `Some(None)` once the
    /// database has been searched and nothing in the chain was installed.
    latin: Option<Option<&'static str>>,
    mono: Option<Option<&'static str>>,
    cjk: Option<Option<&'static str>>,
}

/// Latin, numerals and symbols, in request order.
///
/// `Inter` is what `crate::clock` and `crate::lyrics` already ask for. The
/// family name is never decorated with a face name (`"Inter SemiBold"`):
/// fontconfig substitutes silently on a missing *family* but resolves a
/// *weight* to the nearest real face inside the family asked for, so asking for
/// the family and setting the weight is the only request that degrades well.
pub const LATIN_FAMILIES: [&str; 4] = ["Inter", "Inter Variable", "Noto Sans", "DejaVu Sans"];

/// Simplified Chinese, in request order.
///
/// `Microsoft YaHei` is last because it is present on dual-boot and Wine-heavy
/// machines, which Fresco's Deepin/China userbase over-indexes on.
pub const CJK_FAMILIES: [&str; 5] = [
    "Noto Sans SC",
    "Source Han Sans SC",
    "Noto Sans CJK SC",
    "WenQuanYi Zen Hei",
    "Microsoft YaHei",
];

/// Monospace, for LCD readouts, bitrate chips and tabular columns.
pub const MONO_FAMILIES: [&str; 3] = ["JetBrains Mono", "DejaVu Sans Mono", "Liberation Mono"];

impl FontStack {
    /// Adopt a caller-built [`FontSystem`]. The way to keep font enumeration
    /// off a latency-sensitive thread: build the `FontSystem` wherever you
    /// like, then hand it over.
    pub fn new(fonts: FontSystem) -> Self {
        Self {
            fonts,
            swash: SwashCache::new(),
            scratch: Buffer::new_empty(Metrics::new(16.0, 20.0)),
            latin: None,
            mono: None,
            cjk: None,
        }
    }

    /// Enumerate the system's fonts and build a stack from them.
    ///
    /// **Slow** — see the type's docs. Do not call this from a draw path.
    pub fn system() -> Self {
        Self::new(FontSystem::new())
    }

    /// Build a stack from font files already in memory, with **no system scan
    /// at all**.
    ///
    /// Note that cosmic-text's own `FontSystem::new_with_fonts` still loads the
    /// system's fonts alongside the ones you pass; this builds the database
    /// from scratch so the result really does contain nothing else. That makes
    /// it the constructor for a deterministic test, and for any future path
    /// that ships a font rather than finding one.
    ///
    /// `locale` is a BCP-47 tag (`"zh-CN"`, `"ja"`, `"en-US"`). It is not
    /// cosmetic: it picks which Han variant the CJK fallback prefers — see the
    /// type's docs.
    pub fn from_font_data(
        locale: impl Into<String>,
        fonts: impl IntoIterator<Item = Vec<u8>>,
    ) -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        for data in fonts {
            db.load_font_source(cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(
                data,
            )));
        }
        Self::new(FontSystem::new_with_locale_and_db(locale.into(), db))
    }

    /// The underlying database, for callers that need to add faces or inspect
    /// what was found.
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.fonts
    }

    /// How many faces the database holds.
    ///
    /// Worth logging once at startup: zero means every widget will be blank,
    /// and a blank widget is much harder to diagnose from a bug report than a
    /// line saying no fonts were found.
    pub fn face_count(&self) -> usize {
        self.fonts.db().len()
    }

    /// Whether anything can be shaped at all.
    ///
    /// **This guard is load-bearing, not a convenience.** cosmic-text's shaper
    /// does `font_iter.next().expect("no default font found")` — it *panics*
    /// when the database is empty — and `panic = "abort"` means that would take
    /// the daemon and the user's wallpaper down rather than losing one label.
    /// A machine with no fonts is not hypothetical: a minimal container image
    /// is exactly that. Every entry point in this module checks here first and
    /// returns an empty measurement instead.
    pub fn has_fonts(&self) -> bool {
        self.face_count() > 0
    }

    /// The first family in `chain` that is actually installed, or `None`.
    ///
    /// Asking for a family that is not present is not free: cosmic-text falls
    /// back per *word*, so a request for an absent "Inter" makes every run take
    /// the fallback path. Resolving once and asking for a family that exists
    /// keeps shaping on the fast route, and returning `None` lets the caller
    /// ask for the desktop's default instead — which is the right answer on a
    /// machine that has neither Inter nor Noto.
    ///
    /// The returned string is `'static`, so it can be held across a `&mut self`
    /// borrow of the stack — which is exactly what a card doing
    /// "pick family, then measure" needs.
    pub fn first_installed(&self, chain: &[&'static str]) -> Option<&'static str> {
        let db = self.fonts.db();
        chain.iter().copied().find(|want| {
            db.faces().any(|f| {
                f.families
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case(want))
            })
        })
    }

    /// The best installed Latin family, resolved once and cached.
    pub fn latin_family(&mut self) -> Option<&'static str> {
        *self
            .latin
            .get_or_insert_with(|| first_installed_in(&self.fonts, &LATIN_FAMILIES))
    }

    /// The best installed Simplified-Chinese family, resolved once and cached.
    pub fn cjk_family(&mut self) -> Option<&'static str> {
        *self
            .cjk
            .get_or_insert_with(|| first_installed_in(&self.fonts, &CJK_FAMILIES))
    }

    /// The best installed monospace family, resolved once and cached.
    pub fn mono_family(&mut self) -> Option<&'static str> {
        *self
            .mono
            .get_or_insert_with(|| first_installed_in(&self.fonts, &MONO_FAMILIES))
    }

    /// Lay `run` out at `scale` and report its extent, **without drawing**.
    ///
    /// This is the measurement card layout is built on: measure, place, then
    /// draw at the placed position. Costs one shaping pass, or two when
    /// `max_width` is set (the second establishes whether anything was
    /// dropped, which is what [`TextMetrics::truncated`] reports). Measure once
    /// and keep the result; do not re-measure per frame.
    pub fn measure(&mut self, run: &TextRun, scale: f32) -> TextMetrics {
        if !self.has_fonts() {
            return TextMetrics::ZERO;
        }
        self.with_buffer(|fonts, buf| measure_in(fonts, buf, run, scale))
    }

    /// Lay `run` out at `scale` and emit its coverage.
    ///
    /// `emit` receives one call per **pixel** — `(x, y, colour)` in device
    /// pixels relative to the block's top-left — with the colour's alpha
    /// already multiplied by the glyph's anti-aliasing coverage and its RGB
    /// straight (not premultiplied). `super::Canvas` is the only caller.
    pub(crate) fn draw(
        &mut self,
        run: &TextRun,
        scale: f32,
        mut emit: impl FnMut(i32, i32, Color),
    ) -> TextMetrics {
        if run.text.is_empty() || run.drawable_size().is_none() || !self.has_fonts() {
            return TextMetrics::ZERO;
        }
        let Self {
            fonts,
            swash,
            scratch,
            ..
        } = self;
        let placeholder = Buffer::new_empty(Metrics::new(16.0, 20.0));
        let mut buf = std::mem::replace(scratch, placeholder);
        configure(fonts, &mut buf, run, scale, true);
        let metrics = read_metrics(&buf, run, scale, false);
        let c = run.color;
        let ink = cosmic_text::Color::rgba(
            (c.r * 255.0 + 0.5) as u8,
            (c.g * 255.0 + 0.5) as u8,
            (c.b * 255.0 + 0.5) as u8,
            (c.a * 255.0 + 0.5) as u8,
        );
        buf.draw(fonts, swash, ink, |x, y, w, h, px| {
            // cosmic-text's legacy callback reports 1×1 spans for glyph
            // coverage, but decorations (underline, strikethrough) arrive as
            // real rectangles, so honour w/h rather than assuming a pixel.
            if px.a() == 0 {
                return;
            }
            let colour = Color::rgba8(px.r(), px.g(), px.b(), f32::from(px.a()) / 255.0);
            for dy in 0..h as i32 {
                for dx in 0..w as i32 {
                    emit(x + dx, y + dy, colour);
                }
            }
        });
        *scratch = buf;
        metrics
    }

    fn with_buffer<R>(&mut self, f: impl FnOnce(&mut FontSystem, &mut Buffer) -> R) -> R {
        let placeholder = Buffer::new_empty(Metrics::new(16.0, 20.0));
        let mut buf = std::mem::replace(&mut self.scratch, placeholder);
        let out = f(&mut self.fonts, &mut buf);
        self.scratch = buf;
        out
    }
}

/// The face *count*, never the database. A derived `Debug` would print every
/// face found on the machine into whatever log line formatted it — the same
/// mistake `Bgra`'s hand-written `Debug` in `crate::artwork` avoids for pixels.
/// The body of [`FontStack::first_installed`], free-standing so the cache
/// accessors can call it while holding a `&mut` borrow of one field.
fn first_installed_in(fonts: &FontSystem, chain: &[&'static str]) -> Option<&'static str> {
    let db = fonts.db();
    chain.iter().copied().find(|want| {
        db.faces().any(|f| {
            f.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(want))
        })
    })
}

impl std::fmt::Debug for FontStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontStack")
            .field("faces", &self.face_count())
            .finish()
    }
}

/// Point the buffer at `run`, in device pixels.
///
/// `limit` is what separates the two passes `measure` makes: `false` lays the
/// text out with no height cap and no ellipsis (its *natural* extent), `true`
/// applies the `max_lines`/`max_width` budget and ellipsises. Everything is
/// multiplied by `scale` here and only here, so the rest of the module thinks
/// in logical units.
fn configure(fonts: &mut FontSystem, buf: &mut Buffer, run: &TextRun, scale: f32, limit: bool) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let size = (run.drawable_size().unwrap_or(1.0) * scale).max(1.0);
    let line_height = (run.drawable_line_height() * scale).max(size);
    let lines = run.lines();
    buf.set_metrics(Metrics::new(size, line_height));
    // Wrapping only exists when more than one line is allowed. A one-line run
    // that wrapped would silently drop everything past the first line, which is
    // the opposite of the ellipsis we want.
    buf.set_wrap(if lines > 1 {
        cosmic_text::Wrap::WordOrGlyph
    } else {
        cosmic_text::Wrap::None
    });
    let width = run.max_width.map(|w| (w * scale).max(0.0));
    let height = if limit {
        Some(line_height * lines as f32)
    } else {
        None
    };
    buf.set_size(width, height);
    buf.set_ellipsize(if limit && width.is_some() {
        Ellipsize::End(EllipsizeHeightLimit::Lines(lines))
    } else {
        Ellipsize::None
    });
    buf.set_text(
        &run.text,
        &run.attrs(),
        Shaping::Advanced,
        Some(run.align.to_cosmic()),
    );
    buf.shape_until_scroll(fonts, false);
}

/// Read the laid-out extent back out of `buf`, converted to logical units.
fn read_metrics(buf: &Buffer, run: &TextRun, scale: f32, truncated: bool) -> TextMetrics {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let mut width = 0.0f32;
    let mut lines = 0usize;
    for r in buf.layout_runs() {
        width = width.max(r.line_w);
        lines += 1;
    }
    if lines == 0 {
        return TextMetrics::ZERO;
    }
    TextMetrics {
        width: width / scale,
        height: run.drawable_line_height() * lines as f32,
        lines,
        truncated,
    }
}

fn measure_in(fonts: &mut FontSystem, buf: &mut Buffer, run: &TextRun, scale: f32) -> TextMetrics {
    if run.text.is_empty() || run.drawable_size().is_none() {
        return TextMetrics::ZERO;
    }
    // A non-finite budget is no budget: treat it as unbounded rather than
    // shaping against a NaN width.
    let max_width = run.max_width.filter(|w| w.is_finite() && *w > 0.0);
    let Some(max_width) = max_width else {
        configure(fonts, buf, run, scale, true);
        return read_metrics(buf, run, scale, false);
    };
    // Natural pass: how big does this want to be?
    configure(fonts, buf, run, scale, false);
    let natural = read_metrics(buf, run, scale, false);
    // Budgeted pass: how big is it allowed to be?
    configure(fonts, buf, run, scale, true);
    // Half a logical unit of slack: shaping at device scale and dividing back
    // can leave a sub-pixel residue, and reporting "truncated" for text that
    // visibly fits would send a card into a pointless shrink loop.
    let truncated = natural.lines > run.lines() || natural.width > max_width + 0.5;
    read_metrics(buf, run, scale, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stack over the machine's real fonts.
    ///
    /// Every assertion below is a *relation* (this is wider than that, this
    /// fits inside that), never an absolute pixel count, precisely so the tests
    /// hold on any font set. They still need at least one face to exist; a
    /// machine with none gets a skip rather than a spurious failure, which is
    /// honest — there is nothing to test there.
    fn stack() -> Option<FontStack> {
        let s = FontStack::system();
        (s.face_count() > 0).then_some(s)
    }

    fn run(text: &str, size: f32) -> TextRun {
        TextRun::new(text, size)
    }

    #[test]
    fn empty_text_measures_to_nothing() {
        let Some(mut s) = stack() else { return };
        let m = s.measure(&run("", 24.0), 1.0);
        assert_eq!(m, TextMetrics::ZERO);
        assert_eq!(m.size(), Size::ZERO);
    }

    #[test]
    fn width_grows_with_size_and_with_length() {
        let Some(mut s) = stack() else { return };
        let small = s.measure(&run("Fresco", 16.0), 1.0);
        let large = s.measure(&run("Fresco", 48.0), 1.0);
        assert!(large.width > small.width * 2.0, "{small:?} {large:?}");
        let short = s.measure(&run("Fr", 24.0), 1.0);
        let long = s.measure(&run("Fresco widgets", 24.0), 1.0);
        assert!(long.width > short.width);
        // Height is line_height × lines, so it does not depend on the glyphs.
        assert!((short.height - long.height).abs() < 1e-3);
        assert_eq!(short.lines, 1);
    }

    #[test]
    fn measurement_is_scale_invariant_in_logical_units() {
        // The whole density story: one card definition, any output resolution.
        let Some(mut s) = stack() else { return };
        let r = run("14:32", 64.0);
        let at_1080 = s.measure(&r, 1.0);
        let at_4k = s.measure(&r, 2.0);
        // Hinting rounds glyph advances to device pixels, so these agree to
        // within a fraction of a logical unit, not exactly.
        assert!(
            (at_1080.width - at_4k.width).abs() < at_1080.width * 0.05,
            "{at_1080:?} vs {at_4k:?}"
        );
        assert!((at_1080.height - at_4k.height).abs() < 1e-3);
    }

    #[test]
    fn letter_spacing_widens_without_changing_the_line_count() {
        let Some(mut s) = stack() else { return };
        let tight = s.measure(&run("NOW PLAYING", 14.0), 1.0);
        let open = s.measure(&run("NOW PLAYING", 14.0).letter_spacing(2.0), 1.0);
        assert!(open.width > tight.width, "{tight:?} {open:?}");
        assert_eq!(open.lines, tight.lines);
    }

    #[test]
    fn weight_is_honoured_when_the_family_has_a_bold() {
        let Some(mut s) = stack() else { return };
        // Not asserted as "wider": some families synthesise, some do not, and a
        // condensed bold can be narrower. What must hold is that asking for a
        // weight never loses the text.
        let bold = s.measure(&run("Fresco", 32.0).weight(700), 1.0);
        assert!(bold.width > 0.0 && bold.lines == 1);
    }

    #[test]
    fn ellipsis_keeps_a_long_line_inside_its_budget_and_says_so() {
        let Some(mut s) = stack() else { return };
        let long = "A very long track title that cannot possibly fit in the space given";
        let natural = s.measure(&run(long, 20.0), 1.0);
        let budget = natural.width / 3.0;
        let clipped = s.measure(&run(long, 20.0).max_width(budget), 1.0);
        assert!(clipped.truncated, "{clipped:?}");
        assert!(clipped.width <= budget + 1.0, "{clipped:?} over {budget}");
        assert_eq!(clipped.lines, 1);
        // A budget the text already fits in is not a truncation.
        let roomy = s.measure(&run(long, 20.0).max_width(natural.width * 2.0), 1.0);
        assert!(!roomy.truncated, "{roomy:?}");
        assert!((roomy.width - natural.width).abs() < 1.0);
    }

    #[test]
    fn multi_line_wraps_at_max_width_and_stacks_by_line_height() {
        let Some(mut s) = stack() else { return };
        let text = "Fresco draws widgets onto the wallpaper";
        let one = s.measure(&run(text, 18.0), 1.0);
        let wrapped = s.measure(
            &run(text, 18.0)
                .line_height(30.0)
                .max_width(one.width / 3.0)
                .max_lines(4),
            1.0,
        );
        assert!(wrapped.lines > 1, "{wrapped:?}");
        assert!(wrapped.lines <= 4);
        assert!((wrapped.height - 30.0 * wrapped.lines as f32).abs() < 1e-3);
        assert!(wrapped.width <= one.width / 3.0 + 1.0);
    }

    #[test]
    fn max_lines_caps_the_block_and_reports_the_overflow() {
        let Some(mut s) = stack() else { return };
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let one = s.measure(&run(text, 16.0), 1.0);
        let capped = s.measure(
            &run(text, 16.0).max_width(one.width / 6.0).max_lines(2),
            1.0,
        );
        assert_eq!(capped.lines, 2, "{capped:?}");
        assert!(capped.truncated);
    }

    #[test]
    fn explicit_newlines_make_lines_without_a_width_budget() {
        let Some(mut s) = stack() else { return };
        let m = s.measure(
            &run("14:32\nMonday", 24.0).line_height(28.0).max_lines(2),
            1.0,
        );
        assert_eq!(m.lines, 2);
        assert!((m.height - 56.0).abs() < 1e-3);
        assert!(!m.truncated);
    }

    #[test]
    fn cjk_text_measures_non_zero_when_any_cjk_capable_font_is_installed() {
        // The requirement that picked this text stack. Skipped rather than
        // failed on a machine with no CJK coverage at all, because that is a
        // property of the machine, not of the code.
        let Some(mut s) = stack() else { return };
        // Skip on *coverage*, not on a measurement. Absent ideographs do not
        // measure zero — they measure a fallback box with some default advance
        // — so the old `width <= 0.0` guard waved a machine with no CJK font
        // straight into the assertion below and failed there instead of
        // skipping: CI reported 76.81 against Latin's 77.83, i.e. ~19 units a
        // glyph where full-width at size 32 is ~32. `cjk_family` asks the font
        // database what is installed, which is the question actually being
        // guarded on.
        let Some(_) = s.cjk_family() else {
            eprintln!("no CJK-capable font installed; skipping coverage assertion");
            return;
        };
        let m = s.measure(&run("桌面壁纸", 32.0), 1.0);
        // CJK ideographs are full-width: four of them at 32 units are far wider
        // than four Latin letters, which is a cheap proof real glyphs were
        // found rather than four notdef boxes of a default advance.
        let latin = s.measure(&run("abcd", 32.0), 1.0);
        assert!(m.width > latin.width, "cjk {m:?} vs latin {latin:?}");
        assert_eq!(m.lines, 1);
    }

    #[test]
    fn degenerate_runs_do_not_panic() {
        // `panic = "abort"` is set for release, so a panic here takes the
        // user's wallpaper down with it. Every one of these must clamp.
        let Some(mut s) = stack() else { return };
        for r in [
            run("x", 0.0),
            run("x", -5.0),
            run("x", f32::NAN),
            run("x", 24.0).line_height(0.0),
            run("x", 24.0).line_height(f32::NAN),
            run("x", 24.0).max_width(0.0),
            run("x", 24.0).max_width(-10.0),
            run("x", 24.0).max_width(f32::NAN),
            run("x", 24.0).max_lines(0),
            run("x", f32::INFINITY),
        ] {
            let m = s.measure(&r, 1.0);
            assert!(
                m.width.is_finite() && m.height.is_finite(),
                "{r:?} -> {m:?}"
            );
        }
        // Non-finite and non-positive scales fall back to 1.0 rather than
        // producing a zero-size font.
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let m = s.measure(&run("x", 24.0), scale);
            assert!(m.width.is_finite(), "scale {scale} -> {m:?}");
        }
    }

    #[test]
    fn a_stack_with_no_fonts_measures_empty_instead_of_panicking() {
        // Not a hypothetical: a container image with no fonts installed.
        let mut none = FontStack::from_font_data("en-US", Vec::new());
        assert_eq!(
            none.face_count(),
            0,
            "the database must be built from scratch"
        );
        assert!(!none.has_fonts());
        // Without the has_fonts() guard this call panics inside cosmic-text.
        let m = none.measure(&run("Fresco", 24.0), 1.0);
        assert!(m.width.is_finite());
        assert_eq!(m.width, 0.0, "no faces means no ink");
        // Debug carries the count and nothing else.
        assert_eq!(format!("{none:?}"), "FontStack { faces: 0 }");
    }

    #[test]
    fn text_run_builder_clamps_max_lines_to_one() {
        let r = TextRun::new("x", 12.0).max_lines(0);
        assert_eq!(r.max_lines, 1);
        assert_eq!(TextRun::new("x", 12.0).max_lines(3).max_lines, 3);
        // family(None) needs a type annotation-free spelling at call sites.
        let r = TextRun::new("x", 12.0)
            .family(Some("Inter"))
            .family(None::<String>);
        assert_eq!(r.family, None);
    }
}

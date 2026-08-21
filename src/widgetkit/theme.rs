//! Design tokens: one palette per mode, derived from the user's accent.
//!
//! # Where these numbers come from
//!
//! `docs/widget-design-spec.md` §3 is the authority. This module is that
//! specification transcribed, and every contrast figure quoted below was
//! recomputed from the token literals in this file — not copied from the
//! document — so a token edit that breaks a claim breaks a test.
//!
//! # Tokens, not constants
//!
//! Every colour a card uses comes from a [`Theme`] field. There are two full
//! palettes here and the only way to keep them honest is to have one place
//! where both are stated and one test that scores both. A card that reaches for
//! a literal `#FFFFFF` is a card that will be unreadable in light mode and
//! nobody will notice until a user reports it.
//!
//! # The compositing model every figure below uses
//!
//! A Fresco widget sits on a **video**. There is no page background to design
//! against and a translucent card lets the wallpaper through by construction,
//! so a token is scored against a declared **worst case**: pure white behind
//! the dark theme, pure black behind the light one.
//!
//! ```text
//! surface = a·C + (1−a)·W          card C at alpha a over wallpaper W
//! ink     = t·I + (1−t)·surface    ink I at alpha t over that
//! ratio   = WCAG(ink, surface)     ← never WCAG(ink_token, card_token)
//! ```
//!
//! The card fill is a **gradient**, so both ends are scored and the worse one
//! governs. Alpha is constant across the gradient (spec §2.2): a gradient that
//! also varied alpha would make contrast a function of position inside the
//! card, which is unverifiable.
//!
//! # The scrim, and why it is not optional in dark mode
//!
//! Solving for the ink alpha that reaches a target on the worst dark surface
//! (`#171B24 @ 0.72` over white, composited luminance **L = 0.1040**):
//!
//! | Target | ink α needed, no scrim | ink α needed, with scrim |
//! |---|---|---|
//! | 4.5:1 (AA body) | **0.725** | **0.497** |
//! | 7.0:1 (AAA body) | **unreachable at any α** (white tops out at 6.82:1) | **0.681** |
//!
//! Without a scrim a dark Fresco card over a bright wallpaper supports exactly
//! **one** legible ink level and no AAA text at all, so the three-tier
//! micro-label / hero / secondary hierarchy is arithmetically impossible. With
//! [`Theme::scrim`] (`#04060A @ 0.50`) the surface drops to **L = 0.0299** and
//! the full ramp clears **13.14 / 7.30 / 4.77:1**.
//!
//! **Therefore: every dark text block carries a scrim.** It is not a style
//! option and it is not conditional on the wallpaper — the renderer never sees
//! the wallpaper. [`Theme::text_backdrop`] encodes this: in dark mode it is the
//! *scrimmed* surface, because that is the only surface dark ink is ever
//! allowed to sit on.
//!
//! ## Dark theme, measured (spec §4.1)
//!
//! | Surface state | Wallpaper | Grad. stop | Surface L | primary 1.00 | secondary 0.70 | tertiary 0.52 |
//! |---|---|---|---|---|---|---|
//! | card only | white | light | 0.1040 | 6.82 | 4.32 ✗ | 3.15 ✗ |
//! | card only | white | dark | 0.0802 | 8.06 | 4.94 | 3.51 ✗ |
//! | card only | black | light | 0.0067 | 18.52 | 9.36 | 5.64 |
//! | **card + scrim** | **white** | **light** | **0.0299** | **13.14** | **7.30** | **4.77** |
//! | card + scrim | white | dark | 0.0241 | 14.17 | 7.74 | 4.99 |
//! | card + scrim | black | dark | 0.0022 | 20.12 | 9.77 | 5.68 |
//!
//! ## Light theme is a separate design, not an inversion (spec §2.4)
//!
//! Two findings force it. **(a)** The ink-alpha floor is nearly independent of
//! everything: solving for `#0D1016` at 4.5:1 across card alphas 0.86–0.92 and
//! gradient ends `#EEF1F6`–`#F6F7FA` over black gives α ∈ **[0.593, 0.618]** —
//! a six-point swing in card opacity moves the required ink alpha by 0.018.
//! Raising a light card's opacity does almost nothing for text contrast,
//! because the sRGB transfer curve is near-linear at the bright end. So light
//! mode's ramp is **compressed to 1.00 / 0.78 / 0.64**: the reference's
//! "secondary at 60%" scores **4.474:1** here, a fail by 0.026 before any
//! wallpaper texture is added.
//!
//! **(b)** The light card is more opaque anyway, for *texture*, not contrast.
//! The wallpaper leak is `1 − a`: dark card 28%, dark card + scrim 14.0%, dark
//! well 12.6%, light card 10%, light card + scrim 4.5%. A busy photo with ±100
//! of local luminance swing shows through a dark card as a ±28 mottle under the
//! type, and a *coloured* mottle under dark ink is worse than a grey one.
//!
//! **(c)** The well **inverts direction between themes**. In dark mode it is
//! darker than the card. In light mode a darker well drives the track toward
//! mid-grey, and mid-grey is the worst backdrop for a saturated accent fill.
//! With `#FFFFFF @ 0.55` instead, all six accents clear **4.78–8.61:1**.
//!
//! | Surface state | Wallpaper | Grad. stop | Surface L | primary 1.00 | secondary 0.78 | tertiary 0.64 |
//! |---|---|---|---|---|---|---|
//! | card only | white | light | 1.0000 | 19.04 | 9.67 | 5.75 |
//! | card only | black | light | 0.7874 | 15.19 | 8.42 | 5.27 |
//! | **card only** | **black** | **dark** | **0.7119** | **13.82** | **7.93** | **5.07** |
//! | card + scrim | black | dark | 0.8633 | 16.56 | 8.89 | 5.46 |
//!
//! Light mode passes AA at every level **with no scrim** (worst 5.07:1), so its
//! scrim is optional for contrast and drawn for texture — which is why
//! [`Theme::text_backdrop`] in light mode is the *bare* card: the governing
//! case, not the flattering one.
//!
//! # The one place this file kept its own argument over the document
//!
//! The previous palette here made an argument the specification does not:
//! **a white hairline on a light card is invisible and the light edge must be
//! dark.** That is correct, and on the spec's own light card it is *stronger*
//! than the version that proved it:
//!
//! | Light edge candidate | on `#FFFFFF @ 0.90` over black | on `#F2F4F8 @ 0.90` over black |
//! |---|---|---|
//! | `#FFFFFF @ 0.10` (a naive inversion of dark's edge) | **1.02:1** | **1.03:1** |
//! | `#000000 @ 0.14` (this file's previous fix) | 1.37:1 | 1.37:1 |
//! | `#0B0E14 @ 0.22` (spec §3.2, shipped) | **1.62:1** | **1.61:1** |
//!
//! So the *reasoning* is kept and credited; the *value* is superseded, because
//! the specification reached the same conclusion and went further. Dark mode's
//! edge stays white (`1.41–1.46:1`) — there the inversion is the correct one.
//!
//! # The accent belongs to the user
//!
//! Fresco lets the user choose an accent ([`crate::config::Accent`]). Raw
//! accents are *background* colours, tuned to sit behind white text; used as
//! ink they fail — Blue `#5E6AD2` scores **2.80:1** on the worst scrimmed dark
//! surface and **1.45:1** on the bare dark card. So the theme carries three
//! derived tokens instead of one:
//!
//! - [`Theme::accent_ink`] — accent **text**. `mix(accent, white, 0.32)` in
//!   dark, `mix(accent_light, black, 0.22)` in light. Worst case **4.78:1**
//!   across all twelve combinations, so it is legal at body size.
//! - [`Theme::accent_fill`] — accent **graphics** (arcs, bars, progress).
//!   `mix(accent, white, 0.14)` in dark; in light it *is* `accent_ink`, because
//!   on a light well a lighter fill fails 3:1.
//! - [`Theme::accent_dim`] — track tints and gradient far ends.
//!
//! **An accent-filled data graphic never sits directly on the card.** On the
//! worst dark card the raw accents give 1.45–2.98:1 and every one of them
//! fails. On a [`Theme::well`] the fills give **3.84:1 minimum**. The well is
//! not decoration; it is what makes the progress bar visible.
//!
//! ## Accent text is scrimmed in **both** themes
//!
//! Spec §4.3 scores accent text on the scrim in both modes ("accent text always
//! sits inside a scrimmed text block") while §4.2 says light mode's scrim is
//! optional. Those two statements are only compatible for the *neutral* ramp.
//! Scored on the **bare** light card, half the accents fail AA:
//!
//! | light `accent.ink` | on the bare card (L 0.7119) | on the scrim (L 0.8633) |
//! |---|---|---|
//! | Blue `#3E4599` | 6.06 | 7.26 |
//! | Teal `#0B6D62` | **4.51** | 5.41 |
//! | Coral `#AD3650` | **4.46 ✗** | 5.34 |
//! | Amber `#885E19` | **4.16 ✗** | 4.99 |
//! | Green `#22783B` | **3.99 ✗** | 4.78 |
//! | Graphite `#474C57` | 6.25 | 7.49 |
//!
//! So the light scrim is optional behind *neutral* ink and **mandatory behind
//! accent-coloured ink**. [`Theme::accent_text_backdrop`] is the scrim in both
//! themes for exactly this reason, and a card that wants accent text must draw
//! the scrim first whichever theme it is in.

use super::color::Color;
use super::geom::{Point, Rect};
use super::paint::Fill;

/// Which of the two palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Translucent near-black cards, white text, white edge light.
    #[default]
    Dark,
    /// Translucent near-white cards, near-black text, dark edge.
    Light,
}

impl Mode {
    /// True for [`Mode::Dark`].
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    /// The other one.
    pub fn flip(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

/// One Gaussian drop shadow: offset, blur and alpha, in logical units.
///
/// `blur` is quoted CSS-style throughout; the rasteriser uses `sigma = blur/2`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Downward offset. The light is above, always.
    pub dy: f32,
    /// CSS blur radius.
    pub blur: f32,
    /// Alpha applied to [`Theme::shadow`].
    pub alpha: f32,
}

impl Shadow {
    /// A shadow with the given geometry.
    pub const fn new(dy: f32, blur: f32, alpha: f32) -> Self {
        Self { dy, blur, alpha }
    }

    /// How far this shadow reaches past the shape that casts it.
    pub fn bleed(&self) -> f32 {
        (self.blur * 1.5 + self.dy.abs()).max(0.0).ceil()
    }
}

/// An elevation level: a key shadow and an optional contact shadow, drawn
/// key-first (spec §7).
///
/// Four levels exist. **E0** is nothing (anything flush inside a card), **E1**
/// is a chip or badge, **E2** is a card on the wallpaper, **E3** is the album
/// disc or a card that overlaps another.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elevation {
    /// The large, soft shadow that carries the separation.
    pub key: Shadow,
    /// The tight shadow that pins the shape to whatever it sits on.
    pub contact: Option<Shadow>,
}

impl Elevation {
    /// No shadow at all — E0.
    pub const NONE: Self = Self {
        key: Shadow::new(0.0, 0.0, 0.0),
        contact: None,
    };

    /// How much margin a buffer needs around the shape so nothing is clipped
    /// (spec §7.4). Shadows clip to the canvas like every other primitive.
    pub fn bleed(&self) -> f32 {
        let c = self.contact.map_or(0.0, |s| s.bleed());
        self.key.bleed().max(c)
    }
}

/// Geometry tokens, in logical units, so cards share a rhythm instead of each
/// inventing its own padding.
///
/// The spacing ladder is spec §6.1, base **b = 4 lu**. The grouping rule that
/// makes it work: the gap *inside* a group is at most half the gap *between*
/// groups.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Corner radius of a generic card. Cards that know their largest type
    /// size should call [`radius_card`] instead.
    pub radius_card: f32,
    /// Corner radius of a panel *inside* a card. See [`radius_nested`].
    pub radius_inner: f32,
    /// Padding from a card's edge to its content. See [`card_padding`].
    pub pad: f32,
    /// Glyph to its label; bar to bar.
    pub gap_xs: f32,
    /// Inside a row group.
    pub gap_s: f32,
    /// Label to value.
    pub gap_m: f32,
    /// Between row groups inside a card.
    pub gap_l: f32,
    /// Between major blocks; column gutter.
    pub gap_xl: f32,
    /// Between two cards.
    pub gap_2xl: f32,
    /// Width of an edge hairline.
    pub hairline: f32,
    /// How far a scrim's edge feathers: 6 lu dark, 8 lu light. Wider in light
    /// because a bright feather against a bright card needs more distance to
    /// disappear.
    pub scrim_feather: f32,
}

impl Metrics {
    /// The dark theme's geometry.
    pub const DARK: Self = Self {
        radius_card: 20.0,
        radius_inner: 12.0,
        pad: 20.0,
        gap_xs: 4.0,
        gap_s: 8.0,
        gap_m: 12.0,
        gap_l: 16.0,
        gap_xl: 24.0,
        gap_2xl: 32.0,
        hairline: 1.0,
        scrim_feather: 6.0,
    };

    /// The light theme's geometry — identical but for the wider scrim feather.
    pub const LIGHT: Self = Self {
        scrim_feather: 8.0,
        ..Self::DARK
    };
}

impl Default for Metrics {
    fn default() -> Self {
        Self::DARK
    }
}

/// Card corner radius from the largest type size on the card (spec §6.3).
///
/// Tying the radius to the type rather than to the box is what makes a wide
/// short clock strip and a tall lyric card look like members of one family.
/// Worked: hero 64 → 28, lyric 27 → 12, title 18 → 12 (clamped).
pub fn radius_card(h_max: f32) -> f32 {
    if !h_max.is_finite() || h_max <= 0.0 {
        return 12.0;
    }
    (4.0 * (0.42 * h_max / 4.0).round()).clamp(12.0, 32.0)
}

/// Uniform card padding from the card's shorter side (spec §6.2).
///
/// Worked: 120 → 16, 200 → 20, 320 → 24, ≥ 400 → 28.
pub fn card_padding(min_side: f32) -> f32 {
    if !min_side.is_finite() || min_side <= 0.0 {
        return 12.0;
    }
    (4.0 * ((0.055 * min_side + 8.0) / 4.0).round()).clamp(12.0, 28.0)
}

/// Radius of a shape inset by `d` inside a parent of radius `r_outer`
/// (spec §6.3, the nesting rule).
///
/// This keeps the two curves **concentric** — the gap between them stays `d`
/// all the way round the corner. Reusing the parent's radius makes the inner
/// corner look too round; an unrelated value makes the gap pinch at 45°.
pub fn radius_nested(r_outer: f32, d: f32) -> f32 {
    if !r_outer.is_finite() || !d.is_finite() {
        return 4.0;
    }
    (r_outer - d).max(4.0)
}

/// A resolved palette. Every field is a straight-alpha colour ready to hand to
/// a [`Fill`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Which palette this is.
    pub mode: Mode,
    /// The card body's **near** gradient stop, and the one every dark-mode
    /// contrast figure is governed by. Translucent — the wallpaper is meant to
    /// read through it. Use [`Theme::card_fill`] to get the real gradient.
    pub surface: Color,
    /// The card body's **far** gradient stop. Same alpha as [`Theme::surface`],
    /// by construction (spec §2.2).
    pub surface_far: Color,
    /// A card stacked on a card — rare, and one step lighter *and* one step
    /// more opaque, because elevation carried by opacity survives any backdrop
    /// and elevation carried by lightness does not.
    pub surface_elevated: Color,
    /// The feathered plate that goes behind a **text block**, over the card.
    ///
    /// Mandatory in dark mode (see the module docs). In light mode it buys
    /// texture suppression rather than contrast and is drawn where the type is
    /// below 18 lu.
    pub scrim: Color,
    /// The inset panel behind **data graphics** — visualiser bed, progress
    /// track, LCD. Darker than the card in dark mode, **lighter** in light.
    pub well: Color,
    /// The hairline around a card. A material cue, not a boundary; the
    /// boundary is the shadow.
    pub edge: Color,
    /// The brighter light along the *top* edge only, fading to nothing at the
    /// horizontal midline. Always white, in both modes: it is a specular
    /// highlight and the light source does not move between themes.
    pub edge_highlight: Color,
    /// The "pressed in" top of a well: a soft inner shade.
    pub edge_well_top: Color,
    /// The lift at the bottom of a well.
    pub edge_well_bottom: Color,
    /// Display and body text. Clears AAA on its intended backdrop in both
    /// themes.
    pub text_primary: Color,
    /// Supporting text: artist lines, secondary rows. Legal **only over a
    /// scrim** in dark mode.
    pub text_secondary: Color,
    /// Micro-labels, captions, axis labels. Also scrim-only in dark mode.
    pub text_tertiary: Color,
    /// Ink to place *inside* an [`Theme::accent_fill`] shape — a chip.
    pub text_on_accent: Color,
    /// The user's accent, untouched.
    ///
    /// **Never draw with this.** It is a background colour: on the worst dark
    /// card the six accents score 1.45–2.98:1, all failing even the 3:1
    /// non-text minimum. It is kept so a card can re-derive from it, and so the
    /// user's choice is legible in a debug dump.
    pub accent: Color,
    /// Accent-coloured **text**. Fitted to body size; worst case 4.78:1.
    pub accent_ink: Color,
    /// Accent-coloured **graphics**: arcs, bars, progress fills. Always drawn
    /// on a [`Theme::well`], where it clears 3.84:1 at worst.
    pub accent_fill: Color,
    /// Track tints, gradient far ends, the unfilled remainder of a gauge.
    pub accent_dim: Color,
    /// The drop shadow's colour at **full alpha**; the per-level alpha lives in
    /// [`Elevation`]. Light mode uses a cool near-black rather than pure black:
    /// a pure-black shadow under a white card over a saturated photo reads as
    /// dirt rather than as shade.
    pub shadow: Color,
    /// Chart baselines and tick marks.
    pub gridline: Color,
    /// The unfilled part of a progress bar, over the well.
    pub track_empty: Color,
    /// The body of the skeuomorphic chassis (spec §9.3.3, §9.5) — its **near**
    /// (top) gradient stop.
    ///
    /// **Opaque**, so no wallpaper reaches its type and none of the translucent
    /// contrast model applies to it. Unlike the glass tokens it is *not*
    /// mode-independent: light mode gets a real brushed-aluminium body rather
    /// than the dark one inverted, and the reasoning is in the module docs.
    pub chassis: Color,
    /// The chassis body's **far** (bottom) gradient stop.
    pub chassis_far: Color,
    /// A well sunk into the chassis — the dark glass window every lit readout
    /// sits behind, in **both** modes. A display window is a window whichever
    /// way the furniture around it is painted, and keeping it dark is what lets
    /// one `lcd` colour serve both palettes at 8:1 or better.
    pub chassis_well: Color,
    /// The lit bevel of a chassis: light from above catching the top edge.
    pub bevel_high: Color,
    /// The shaded bevel of a chassis: the bottom edge falling into shadow.
    pub bevel_low: Color,
    /// The Chassis theme's fixed readout colour. Opts out of `accent_follow`
    /// exactly as `LyricStylePreset::Karaoke`'s amber does.
    pub lcd: Color,
    /// The NOS clock's single chromatic accent (spec §9.1.3).
    ///
    /// That design is greyscale plus **one red**, so the red is a token rather
    /// than an accent derivation: a NOS card drawn in six different accents
    /// would not be the design. `accent_follow` still overrides it, because a
    /// user who asked every widget to match the app accent asked for that too —
    /// but the default, and what the sheets specify, is this.
    pub nos_red: Color,
    /// The NOS ring's **unlit** dot — the remainder of the dotted progress arc.
    ///
    /// A separate token and not [`Theme::text_tertiary`], for an accessibility
    /// reason that is arithmetic rather than taste. `text_tertiary` resolves to
    /// L 0.331 on the dark backdrop and [`Theme::nos_red`] to L 0.257: a ratio
    /// of **1.23:1**, which is to say the two differ in *hue* and in nothing
    /// else. The dotted arc is the signature element of the whole NOS clock, so
    /// hue alone carrying it would mean the ring reads as uniform in greyscale
    /// and to anyone with a red-green deficiency — roughly one man in twelve —
    /// on a wallpaper meant to be read from across a room.
    ///
    /// Fitted instead as a **track**, the role it actually plays (spec §8.3's
    /// remainder arc): dark `#FFFFFF @ 0.22`, light `#0B0E14 @ 0.30`. That puts
    /// it at 2.02:1 / 1.95:1 against its own card — present, not shouting, the
    /// same band [`Theme::track_empty`] occupies — and at **1.93:1 / 2.09:1**
    /// against the red, which is a real luminance step rather than a hue swap.
    /// Size is the third channel and lives in the card, not here
    /// (`cards::nos::ring_dots`).
    pub nos_dim: Color,
    /// The backdrop this palette was fitted against: white for dark, black for
    /// light. Kept as a field so the contrast helpers cannot drift from the
    /// assumption the alphas were chosen under.
    pub worst_backdrop: Color,
    /// Shared geometry.
    pub metrics: Metrics,
}

/// WCAG AA for body text.
pub const AA_TEXT: f32 = 4.5;
/// WCAG AA for large text and non-text indicators.
pub const AA_LARGE: f32 = 3.0;
/// WCAG AAA for body text.
pub const AAA_TEXT: f32 = 7.0;

/// The angle of the card gradient, CSS-style (0° points up, clockwise).
const CARD_GRADIENT_DEG: f32 = 160.0;

/// The Chassis alternate's identity colour. Not one of Fresco's six accents,
/// and deliberately so (spec §9.3.3).
pub const CHASSIS_LCD: Color = Color {
    r: 0xF5 as f32 / 255.0,
    g: 0xA6 as f32 / 255.0,
    b: 0x23 as f32 / 255.0,
    a: 1.0,
};

/// The NOS clock's red, dark palette. Fitted, not picked.
///
/// On the governing dark scrimmed surface (L 0.0299) a graphic needs 3:1.
/// `#E5484D` — the obvious choice, and the one a designer reaches for — manages
/// **3.36:1** there, which passes, and then **2.84:1** on the governing light
/// card, which does not. One red cannot serve both palettes, so there are two,
/// and each is pushed away from its own card until it clears with margin:
/// **3.89:1** here and 4.08:1 in light. The second number that matters is the
/// step to [`Theme::nos_dim`] (1.93:1 here, 1.66:1 for `#E5484D`) — see
/// [`Theme::nos_dim`] for why that one decides the design.
pub const NOS_RED_DARK: Color = Color {
    r: 0xF2 as f32 / 255.0,
    g: 0x55 as f32 / 255.0,
    b: 0x5A as f32 / 255.0,
    a: 1.0,
};

/// The NOS clock's red, light palette. Darker rather than the dark one reused:
/// `#F2555A` on the light card is 1.9:1 and invisible.
pub const NOS_RED_LIGHT: Color = Color {
    r: 0xC4 as f32 / 255.0,
    g: 0x2B as f32 / 255.0,
    b: 0x30 as f32 / 255.0,
    a: 1.0,
};

/// The NOS ring's unlit dot, dark palette. See [`Theme::nos_dim`] for why this
/// is a token of its own rather than the tertiary ink.
pub const NOS_DIM_DARK: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.22,
};

/// The NOS ring's unlit dot, light palette.
pub const NOS_DIM_LIGHT: Color = Color {
    r: 0x0B as f32 / 255.0,
    g: 0x0E as f32 / 255.0,
    b: 0x14 as f32 / 255.0,
    a: 0.30,
};

impl Theme {
    /// The dark palette, derived from the accent's **dark** variant.
    ///
    /// Every value is spec §3.1. See the module docs for the contrast tables
    /// these produce.
    pub fn dark(accent: Color) -> Self {
        let accent = accent.with_alpha(1.0);
        Self {
            mode: Mode::Dark,
            surface: Color::rgba8(0x17, 0x1B, 0x24, 0.72),
            surface_far: Color::rgba8(0x0A, 0x0C, 0x11, 0.72),
            surface_elevated: Color::rgba8(0x1E, 0x24, 0x30, 0.78),
            scrim: Color::rgba8(0x04, 0x06, 0x0A, 0.50),
            well: Color::rgba8(0x04, 0x06, 0x0A, 0.55),
            edge: Color::WHITE.with_alpha(0.14),
            edge_highlight: Color::WHITE.with_alpha(0.34),
            edge_well_top: Color::BLACK.with_alpha(0.55),
            edge_well_bottom: Color::WHITE.with_alpha(0.06),
            text_primary: Color::WHITE,
            text_secondary: Color::WHITE.with_alpha(0.70),
            text_tertiary: Color::WHITE.with_alpha(0.52),
            text_on_accent: Color::rgb8(0x0A, 0x0C, 0x11),
            accent,
            accent_ink: quantized(accent.lerp(Color::WHITE, 0.32)),
            accent_fill: quantized(accent.lerp(Color::WHITE, 0.14)),
            accent_dim: accent.with_alpha(0.28),
            shadow: Color::BLACK,
            gridline: Color::WHITE.with_alpha(0.09),
            track_empty: Color::WHITE.with_alpha(0.11),
            chassis: Color::rgb8(0x2B, 0x2B, 0x2B),
            chassis_far: Color::rgb8(0x20, 0x20, 0x22),
            chassis_well: Color::rgb8(0x14, 0x14, 0x14),
            bevel_high: Color::WHITE.with_alpha(0.14),
            bevel_low: Color::BLACK.with_alpha(0.55),
            lcd: CHASSIS_LCD,
            nos_red: NOS_RED_DARK,
            nos_dim: NOS_DIM_DARK,
            worst_backdrop: Color::WHITE,
            metrics: Metrics::DARK,
        }
    }

    /// The light palette, derived from the accent's **light** variant.
    ///
    /// **Not** an inversion of [`Theme::dark`] — see the module docs for the
    /// three findings that force it to be designed separately.
    pub fn light(accent: Color) -> Self {
        let accent = accent.with_alpha(1.0);
        let ink = Color::rgb8(0x0D, 0x10, 0x16);
        let accent_ink = quantized(accent.lerp(Color::BLACK, 0.22));
        Self {
            mode: Mode::Light,
            surface: Color::WHITE.with_alpha(0.90),
            surface_far: Color::rgba8(0xF2, 0xF4, 0xF8, 0.90),
            surface_elevated: Color::WHITE.with_alpha(0.96),
            scrim: Color::WHITE.with_alpha(0.55),
            well: Color::WHITE.with_alpha(0.55),
            edge: Color::rgba8(0x0B, 0x0E, 0x14, 0.22),
            edge_highlight: Color::WHITE.with_alpha(0.95),
            edge_well_top: Color::rgba8(0x0B, 0x0E, 0x14, 0.18),
            edge_well_bottom: Color::WHITE.with_alpha(0.90),
            text_primary: ink,
            text_secondary: ink.with_alpha(0.78),
            text_tertiary: ink.with_alpha(0.64),
            text_on_accent: Color::WHITE,
            accent,
            accent_ink,
            // One value, two roles: on a light well a *lighter* fill fails 3:1.
            accent_fill: accent_ink,
            accent_dim: accent.with_alpha(0.22),
            shadow: Color::rgb8(0x0B, 0x12, 0x20),
            gridline: Color::rgba8(0x0B, 0x0E, 0x14, 0.14),
            track_empty: Color::rgba8(0x0B, 0x0E, 0x14, 0.13),
            // Brushed aluminium, not the dark chassis inverted: a light
            // instrument is a pale *body* with the same dark display windows
            // sunk into it, which is what real equipment looks like and what
            // keeps `lcd` legal in both palettes without a second colour.
            chassis: Color::rgb8(0xE4, 0xE4, 0xE7),
            chassis_far: Color::rgb8(0xD0, 0xD0, 0xD6),
            chassis_well: Color::rgb8(0x1C, 0x1C, 0x20),
            bevel_high: Color::WHITE.with_alpha(0.95),
            bevel_low: Color::BLACK.with_alpha(0.22),
            lcd: CHASSIS_LCD,
            nos_red: NOS_RED_LIGHT,
            nos_dim: NOS_DIM_LIGHT,
            worst_backdrop: Color::BLACK,
            metrics: Metrics::LIGHT,
        }
    }

    /// The palette for `mode`, taking the accent already resolved for that
    /// mode — see [`accent_color`].
    pub fn new(mode: Mode, accent: Color) -> Self {
        match mode {
            Mode::Dark => Self::dark(accent),
            Mode::Light => Self::light(accent),
        }
    }

    /// The palette for one of Fresco's six accents, with the mode-correct
    /// variant of that accent picked for you. This is the constructor a widget
    /// should use.
    pub fn for_accent(mode: Mode, accent: crate::config::Accent) -> Self {
        Self::new(mode, accent_color(accent, mode))
    }

    /// The palette for `mode`, taking the accent as a hex string.
    ///
    /// The caller is responsible for passing the **mode-appropriate** variant;
    /// `crate::daemon`'s `accent_hex()` only knows the dark ones, which is
    /// exactly why [`Theme::for_accent`] exists. An unparseable string falls
    /// back to Blue rather than costing the user their widget.
    pub fn from_accent_hex(mode: Mode, hex: &str) -> Self {
        match Color::from_hex(hex) {
            Some(c) => Self::new(mode, c),
            None => Self::for_accent(mode, crate::config::Accent::Blue),
        }
    }

    /// The card's gradient fill across `r`: colour varies, alpha does not.
    ///
    /// 160° CSS-style — top-left to bottom-right. A caller that wants a flat
    /// card can use [`Theme::surface`] directly; that is the governing stop, so
    /// doing so never makes contrast worse.
    pub fn card_fill(&self, r: Rect) -> Fill {
        let (from, to) = gradient_line(r, CARD_GRADIENT_DEG);
        Fill::linear(from, to, self.surface, self.surface_far)
    }

    /// The chassis body's vertical gradient across `r`.
    ///
    /// Vertical rather than the card's 160°: a chassis is a physical object lit
    /// from above, and a diagonal ramp on an opaque slab reads as a texture
    /// rather than as light. Opaque at both ends, so nothing about §4 changes.
    pub fn chassis_fill(&self, r: Rect) -> Fill {
        Fill::vertical(r, self.chassis, self.chassis_far)
    }

    /// E1 — chip, badge, bar peak-cap, disc label.
    pub fn e1(&self) -> Elevation {
        Elevation {
            key: Shadow::new(1.0, 3.0, if self.mode.is_dark() { 0.28 } else { 0.20 }),
            contact: None,
        }
    }

    /// E2 — a card on the wallpaper.
    ///
    /// Light's is larger and softer than dark's at the same level: a white card
    /// on a bright photo has no luminance step at its edge, so the shadow is
    /// doing *all* of the separation work a backdrop blur would have shared.
    pub fn e2(&self) -> Elevation {
        if self.mode.is_dark() {
            Elevation {
                key: Shadow::new(10.0, 28.0, 0.46),
                contact: Some(Shadow::new(1.0, 2.0, 0.34)),
            }
        } else {
            Elevation {
                key: Shadow::new(12.0, 32.0, 0.34),
                contact: Some(Shadow::new(1.0, 2.0, 0.26)),
            }
        }
    }

    /// E3 — the album disc, or a card that overlaps another.
    pub fn e3(&self) -> Elevation {
        if self.mode.is_dark() {
            Elevation {
                key: Shadow::new(16.0, 40.0, 0.52),
                contact: Some(Shadow::new(2.0, 4.0, 0.36)),
            }
        } else {
            Elevation {
                key: Shadow::new(18.0, 44.0, 0.40),
                contact: Some(Shadow::new(2.0, 4.0, 0.28)),
            }
        }
    }

    // -- contrast ------------------------------------------------------------

    /// The governing card surface: the worse gradient stop, composited over the
    /// worst-case wallpaper. Dark mode's worst stop is the **light** one; light
    /// mode's is the **dark** one.
    pub fn resolved_surface(&self) -> Color {
        let (a, b) = (
            self.surface.over(self.worst_backdrop),
            self.surface_far.over(self.worst_backdrop),
        );
        worse_of(a, b, self.mode)
    }

    /// [`Theme::surface_elevated`] composited over the worst-case wallpaper.
    pub fn resolved_elevated(&self) -> Color {
        self.surface_elevated.over(self.worst_backdrop)
    }

    /// The scrim composited over the governing card surface.
    pub fn resolved_scrim(&self) -> Color {
        self.scrim.over(self.resolved_surface())
    }

    /// The well composited over the governing card surface.
    pub fn resolved_well(&self) -> Color {
        self.well.over(self.resolved_surface())
    }

    /// **The surface text is actually allowed to sit on**, and the one every
    /// ink token is scored against.
    ///
    /// Dark mode: the *scrimmed* card, because a dark card without a scrim
    /// supports one legible ink level and the design needs three. Light mode:
    /// the *bare* card, because its scrim is drawn for texture and skipping it
    /// behind a large hero is legal — so the bare card is the governing case.
    pub fn text_backdrop(&self) -> Color {
        if self.mode.is_dark() {
            self.resolved_scrim()
        } else {
            self.resolved_surface()
        }
    }

    /// Contrast ratio of `ink` on the bare card, with `ink`'s own alpha taken
    /// into account. Use [`Theme::contrast_on_text`] for anything that is text.
    pub fn contrast_on_surface(&self, ink: Color) -> f32 {
        contrast_over(ink, self.resolved_surface())
    }

    /// Contrast ratio of `ink` on the elevated surface.
    pub fn contrast_on_elevated(&self, ink: Color) -> f32 {
        contrast_over(ink, self.resolved_elevated())
    }

    /// The surface **accent-coloured text** is allowed to sit on: the scrim,
    /// in both themes.
    ///
    /// Unlike the neutral ramp, accent ink does not clear AA on a bare light
    /// card — Green manages only 3.99:1 there. See the module docs for the
    /// table. A card drawing accent text draws a scrim first, always.
    pub fn accent_text_backdrop(&self) -> Color {
        self.resolved_scrim()
    }

    /// Contrast ratio of `ink` on [`Theme::text_backdrop`].
    pub fn contrast_on_text(&self, ink: Color) -> f32 {
        contrast_over(ink, self.text_backdrop())
    }

    /// Contrast ratio of accent-coloured text on [`Theme::accent_text_backdrop`].
    pub fn contrast_on_accent_text(&self, ink: Color) -> f32 {
        contrast_over(ink, self.accent_text_backdrop())
    }

    /// Contrast ratio of a graphic on [`Theme::well`] — the only surface an
    /// accent-filled graphic is allowed on.
    pub fn contrast_on_well(&self, ink: Color) -> f32 {
        contrast_over(ink, self.resolved_well())
    }

    /// True when `ink` clears WCAG AA (4.5:1) as body text **on the backdrop it
    /// is specified to sit on**.
    ///
    /// This is the gate: a token that fails here has no legal use in a card.
    pub fn passes_body_text(&self, ink: Color) -> bool {
        self.contrast_on_text(ink) >= AA_TEXT
    }

    /// True when `ink` clears WCAG AA for large text / non-text (3:1) on its
    /// intended backdrop.
    pub fn passes_large_text(&self, ink: Color) -> bool {
        self.contrast_on_text(ink) >= AA_LARGE
    }
}

/// Fresco's six accents, dark and light variants, exactly as
/// `gui::theme::accent_pair` resolves them.
///
/// Duplicated rather than imported because that function lives behind the
/// `gui` feature and widgetkit is a `daemon` module; the test below is what
/// keeps the two honest.
const ACCENT_TABLE: [(crate::config::Accent, &str, &str); 6] = {
    use crate::config::Accent as A;
    [
        (A::Blue, "#5E6AD2", "#5058C4"),
        (A::Teal, "#2BB6A2", "#0E8C7E"),
        (A::Green, "#46B96B", "#2C9A4C"),
        (A::Amber, "#DBA13C", "#AE7820"),
        (A::Coral, "#F0708A", "#DE4567"),
        (A::Graphite, "#98A1B0", "#5B626F"),
    ]
};

/// Every accent, in declaration order. `crate::config::Accent` does not
/// publish one and widgetkit needs to iterate them to score the palette.
pub const ACCENTS: [crate::config::Accent; 6] = {
    use crate::config::Accent as A;
    [A::Blue, A::Teal, A::Green, A::Amber, A::Coral, A::Graphite]
};

/// The hex for `accent` in `mode`.
///
/// Dark and light are **different colours**, not the same colour at different
/// alphas: `#5E6AD2` is tuned to read over video, `#5058C4` to read on paper.
/// Deriving light mode's palette from the dark hex is the mistake spec §3.2
/// calls out by name.
pub fn accent_hex(accent: crate::config::Accent, mode: Mode) -> &'static str {
    for (a, dark, light) in ACCENT_TABLE {
        if a == accent {
            return if mode.is_dark() { dark } else { light };
        }
    }
    "#5E6AD2"
}

/// The colour for `accent` in `mode`.
pub fn accent_color(accent: crate::config::Accent, mode: Mode) -> Color {
    Color::from_hex(accent_hex(accent, mode)).unwrap_or(Color {
        r: 0.369,
        g: 0.416,
        b: 0.824,
        a: 1.0,
    })
}

/// Snap a derived colour to the 8-bit grid.
///
/// The spec publishes `accent.ink` and `accent.fill` as **hex literals**, and
/// the hex is the token — every ratio in this module was measured from it. A
/// full-precision `mix()` lands up to half a code point away, which is enough
/// to move Teal's light ink from 4.51:1 to 4.50:1 and turn a pass into a fail.
/// Rounding here makes the shipped colour and the published one the same
/// colour.
fn quantized(c: Color) -> Color {
    let q = |v: f32| (v * 255.0).round() / 255.0;
    Color {
        r: q(c.r),
        g: q(c.g),
        b: q(c.b),
        a: c.a,
    }
}

/// Whichever of two composited surfaces makes ink harder to read: the lighter
/// one in dark mode (where ink is white), the darker one in light mode.
fn worse_of(a: Color, b: Color, mode: Mode) -> Color {
    let (la, lb) = (a.relative_luminance(), b.relative_luminance());
    if mode.is_dark() == (la > lb) {
        a
    } else {
        b
    }
}

/// WCAG ratio between `ink` composited onto `bg` and `bg` itself.
fn contrast_over(ink: Color, bg: Color) -> f32 {
    ink.over(bg).contrast_ratio(bg)
}

/// The two endpoints of a CSS-style linear gradient over `r`.
///
/// `deg` is measured clockwise from "up", so 180° runs straight down and 160°
/// runs down and slightly right — the spec's top-left-to-bottom-right diagonal.
pub(crate) fn gradient_line(r: Rect, deg: f32) -> (Point, Point) {
    let rad = deg.to_radians();
    let (dx, dy) = (rad.sin(), -rad.cos());
    let len = (r.w * dx.abs() + r.h * dy.abs()) / 2.0;
    let c = r.center();
    (
        Point::new(c.x - dx * len, c.y - dy * len),
        Point::new(c.x + dx * len, c.y + dy * len),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn themes() -> Vec<Theme> {
        ACCENTS
            .iter()
            .flat_map(|&a| {
                [
                    Theme::for_accent(Mode::Dark, a),
                    Theme::for_accent(Mode::Light, a),
                ]
            })
            .collect()
    }

    /// Assert `got` is `want` to two decimal places, the precision the spec and
    /// the module docs quote.
    #[track_caller]
    fn near(got: f32, want: f32, what: &str) {
        assert!(
            (got - want).abs() < 0.015,
            "{what}: got {got:.4}, documented {want:.2}"
        );
    }

    #[test]
    fn the_dark_contrast_table_in_the_docs_is_what_the_tokens_actually_produce() {
        let t = Theme::for_accent(Mode::Dark, crate::config::Accent::Blue);
        // Bare card, both gradient stops, over the worst-case white wallpaper.
        for (stop, l, p, s, ter) in [
            (t.surface, 0.1040, 6.82, 4.32, 3.15),
            (t.surface_far, 0.0802, 8.06, 4.94, 3.51),
        ] {
            let card = stop.over(Color::WHITE);
            near(card.relative_luminance(), l, "card L");
            near(contrast_over(t.text_primary, card), p, "card primary");
            near(contrast_over(t.text_secondary, card), s, "card secondary");
            near(contrast_over(t.text_tertiary, card), ter, "card tertiary");
        }
        // And with the scrim, which is the row that governs everything.
        for (stop, l, p, s, ter) in [
            (t.surface, 0.0299, 13.14, 7.30, 4.77),
            (t.surface_far, 0.0241, 14.17, 7.74, 4.99),
        ] {
            let sc = t.scrim.over(stop.over(Color::WHITE));
            near(sc.relative_luminance(), l, "scrim L");
            near(contrast_over(t.text_primary, sc), p, "scrim primary");
            near(contrast_over(t.text_secondary, sc), s, "scrim secondary");
            near(contrast_over(t.text_tertiary, sc), ter, "scrim tertiary");
        }
        // The governing surface really is the light stop plus the scrim.
        near(t.text_backdrop().relative_luminance(), 0.0299, "backdrop L");
    }

    #[test]
    fn the_light_contrast_table_in_the_docs_is_what_the_tokens_actually_produce() {
        let t = Theme::for_accent(Mode::Light, crate::config::Accent::Blue);
        for (stop, wp, l, p, s, ter) in [
            (t.surface, Color::WHITE, 1.0000, 19.04, 9.67, 5.75),
            (t.surface, Color::BLACK, 0.7874, 15.19, 8.42, 5.27),
            (t.surface_far, Color::BLACK, 0.7119, 13.82, 7.93, 5.07),
        ] {
            let card = stop.over(wp);
            near(card.relative_luminance(), l, "card L");
            near(contrast_over(t.text_primary, card), p, "card primary");
            near(contrast_over(t.text_secondary, card), s, "card secondary");
            near(contrast_over(t.text_tertiary, card), ter, "card tertiary");
        }
        let sc = t.scrim.over(t.surface_far.over(Color::BLACK));
        near(sc.relative_luminance(), 0.8633, "scrim L");
        near(contrast_over(t.text_primary, sc), 16.56, "scrim primary");
        near(contrast_over(t.text_secondary, sc), 8.89, "scrim secondary");
        near(contrast_over(t.text_tertiary, sc), 5.46, "scrim tertiary");
        // Light's governing case is the bare dark stop, not the flattering one.
        near(t.text_backdrop().relative_luminance(), 0.7119, "backdrop L");
    }

    /// The gate the previous palette could not pass: the tokens must satisfy
    /// the toolkit's own body-text predicate on the backdrop they are specified
    /// to sit on.
    #[test]
    fn every_text_token_clears_aa_on_the_backdrop_it_is_specified_to_sit_on() {
        for t in themes() {
            for (name, ink) in [
                ("primary", t.text_primary),
                ("secondary", t.text_secondary),
                ("tertiary", t.text_tertiary),
            ] {
                let r = t.contrast_on_text(ink);
                assert!(
                    r >= AA_TEXT,
                    "{:?} {name} is {r:.2}:1, below AA {AA_TEXT}",
                    t.mode
                );
                assert!(t.passes_body_text(ink), "{:?} {name}", t.mode);
            }
            // The hero clears AAA in both themes, which is what the scrim was
            // bought for.
            let r = t.contrast_on_text(t.text_primary);
            assert!(r >= AAA_TEXT, "{:?} primary is {r:.2}:1", t.mode);
            // Accent ink is scored on the scrim, which is where it is
            // specified to sit in both themes.
            let a = t.contrast_on_accent_text(t.accent_ink);
            assert!(a >= AA_TEXT, "{:?} accent ink is {a:.2}:1", t.mode);
        }
    }

    #[test]
    fn a_dark_card_without_its_scrim_is_exactly_the_failure_the_scrim_exists_for() {
        let t = Theme::for_accent(Mode::Dark, crate::config::Accent::Blue);
        // Unscrimmed, secondary and tertiary fail AA — the structural result.
        assert!(t.contrast_on_surface(t.text_secondary) < AA_TEXT);
        // Tertiary is worse still: it does not even clear AA-large on the
        // lightest gradient stop, which is where a micro-label would sit.
        near(
            t.contrast_on_surface(t.text_tertiary),
            3.15,
            "bare tertiary",
        );
        assert!(t.contrast_on_surface(t.text_tertiary) < AA_LARGE + 0.2);
        // Primary passes AA but not AAA, so even a hero-only card takes one.
        let p = t.contrast_on_surface(t.text_primary);
        assert!((AA_TEXT..AAA_TEXT).contains(&p), "{p:.2}");
        // 7:1 is unreachable at any alpha without the scrim: pure white, the
        // strongest ink there is, tops out below it.
        near(
            contrast_over(Color::WHITE, t.resolved_surface()),
            6.82,
            "α=1",
        );
    }

    #[test]
    fn light_mode_is_a_separate_design_and_a_mirrored_ramp_would_fail() {
        let l = Theme::for_accent(Mode::Light, crate::config::Accent::Blue);
        let d = Theme::for_accent(Mode::Dark, crate::config::Accent::Blue);
        // The ramp is compressed, not mirrored.
        assert!(l.text_secondary.a > d.text_secondary.a);
        assert!(l.text_tertiary.a > d.text_tertiary.a);
        // The reference's "secondary at 60%" is illegal here, by 0.026.
        let sixty = l.text_primary.with_alpha(0.60);
        near(l.contrast_on_text(sixty), 4.474, "0.60 ink");
        assert!(!l.passes_body_text(sixty));
        // Light is more opaque, for texture rather than contrast.
        assert!(l.surface.a > d.surface.a);
        // The well inverts direction: darker than the card in dark, lighter in
        // light. Structurally identical token, opposite sign.
        assert!(d.resolved_well().relative_luminance() < d.resolved_surface().relative_luminance());
        assert!(l.resolved_well().relative_luminance() > l.resolved_surface().relative_luminance());
    }

    #[test]
    fn the_light_edge_is_dark_because_a_white_one_is_invisible() {
        let l = Theme::for_accent(Mode::Light, crate::config::Accent::Teal);
        let d = Theme::for_accent(Mode::Dark, crate::config::Accent::Teal);
        // The arithmetic that justifies keeping this file's original argument.
        for stop in [l.surface, l.surface_far] {
            let card = stop.over(Color::BLACK);
            let naive = contrast_over(Color::WHITE.with_alpha(0.10), card);
            let previous = contrast_over(Color::BLACK.with_alpha(0.14), card);
            let shipped = contrast_over(l.edge, card);
            assert!(naive < 1.05, "a white light edge scored {naive:.2}:1");
            assert!(previous > naive, "black@14% must beat white@10%");
            assert!(
                shipped > previous,
                "the spec's edge ({shipped:.2}) must beat black@14% ({previous:.2})"
            );
            assert!((1.60..1.63).contains(&shipped), "{shipped:.2}");
        }
        // Dark mode's edge is white and that inversion is the correct one.
        for stop in [d.surface, d.surface_far] {
            let card = stop.over(Color::WHITE);
            let r = contrast_over(d.edge, card);
            assert!((1.40..1.47).contains(&r), "dark edge {r:.2}:1");
        }
        // Both highlights are white: a specular highlight does not invert.
        assert_eq!(d.edge_highlight.with_alpha(1.0), Color::WHITE);
        assert_eq!(l.edge_highlight.with_alpha(1.0), Color::WHITE);
        assert!(l.edge_highlight.a > d.edge_highlight.a * 2.0);
    }

    #[test]
    fn accent_text_clears_body_size_and_accent_graphics_clear_the_well() {
        // Spec §4.3, recomputed from the literals this file derives.
        let mut worst_text = f32::MAX;
        let mut worst_fill = f32::MAX;
        for a in ACCENTS {
            for mode in [Mode::Dark, Mode::Light] {
                let t = Theme::for_accent(mode, a);
                let text = t.contrast_on_accent_text(t.accent_ink);
                let fill = t.contrast_on_well(t.accent_fill);
                assert!(text >= AA_TEXT, "{a:?} {mode:?} accent ink {text:.2}:1");
                assert!(fill >= AA_LARGE, "{a:?} {mode:?} accent fill {fill:.2}:1");
                worst_text = worst_text.min(text);
                worst_fill = worst_fill.min(fill);
            }
        }
        near(worst_text, 4.78, "worst accent ink");
        near(worst_fill, 3.84, "worst accent fill");
    }

    /// The finding that made `accent_text_backdrop` a separate method: light
    /// mode's scrim is optional behind neutral ink and mandatory behind accent
    /// ink, because three of the six accents fail AA on the bare light card.
    #[test]
    fn light_accent_ink_needs_the_scrim_even_though_neutral_light_ink_does_not() {
        use crate::config::Accent as A;
        let mut failed = 0;
        for a in ACCENTS {
            let t = Theme::for_accent(Mode::Light, a);
            let bare = t.contrast_on_surface(t.accent_ink);
            let scrimmed = t.contrast_on_accent_text(t.accent_ink);
            assert!(scrimmed > bare, "{a:?} scrim must help");
            assert!(scrimmed >= AA_TEXT, "{a:?} scrimmed {scrimmed:.2}:1");
            if bare < AA_TEXT {
                failed += 1;
            }
        }
        assert_eq!(failed, 3, "three accents are expected to need the scrim");
        // Teal is the one on the line — 4.511:1 at the published hex. That is
        // why the derived tokens are quantised to 8 bits: at full float
        // precision it lands at 4.4999 and this test flips.
        near(
            Theme::for_accent(Mode::Light, A::Teal)
                .contrast_on_surface(Theme::for_accent(Mode::Light, A::Teal).accent_ink),
            4.511,
            "Teal bare",
        );
        let g = Theme::for_accent(Mode::Light, A::Green);
        near(g.contrast_on_surface(g.accent_ink), 3.99, "Green bare");
        near(
            g.contrast_on_accent_text(g.accent_ink),
            4.78,
            "Green scrimmed",
        );
        // The neutral ramp, by contrast, is fine bare — which is why the two
        // backdrops are different methods rather than one conservative rule.
        assert!(g.passes_body_text(g.text_tertiary));
    }

    #[test]
    fn an_accent_filled_graphic_on_the_bare_card_is_the_thing_the_well_prevents() {
        // Every raw accent fails 3:1 on the worst dark card — which is the
        // whole reason the well is mandatory under a progress bar.
        let mut worst: f32 = 0.0;
        for a in ACCENTS {
            let t = Theme::for_accent(Mode::Dark, a);
            let r = t.contrast_on_surface(t.accent);
            assert!(r < AA_LARGE, "{a:?} raw accent scored {r:.2}:1");
            worst = worst.max(r);
        }
        near(worst, 2.98, "best raw accent");
    }

    #[test]
    fn on_accent_ink_is_legible_inside_the_chip_it_is_specified_for() {
        // `text_on_accent` sits inside an `accent_fill` shape, never on the raw
        // accent — scoring it against the raw accent would be measuring a
        // surface that is never drawn.
        for a in ACCENTS {
            for mode in [Mode::Dark, Mode::Light] {
                let t = Theme::for_accent(mode, a);
                let r = t.text_on_accent.contrast_ratio(t.accent_fill);
                assert!(r >= AA_TEXT, "{a:?} {mode:?} on-accent ink {r:.2}:1");
            }
        }
    }

    #[test]
    fn the_accent_keeps_its_hue_rather_than_being_flattened_to_grey() {
        use crate::config::Accent as A;
        for a in [A::Blue, A::Teal, A::Green, A::Coral] {
            for mode in [Mode::Dark, Mode::Light] {
                let t = Theme::for_accent(mode, a);
                for c in [t.accent_ink, t.accent_fill] {
                    let spread = c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b);
                    assert!(spread > 0.08, "{a:?} {mode:?} lost its hue: {c:?}");
                }
            }
        }
    }

    #[test]
    fn the_accent_table_matches_the_hexes_the_gui_and_daemon_ship() {
        // Literal transcriptions of `gui::theme::accent_pair`, which lives
        // behind a feature widgetkit cannot depend on. If that table moves,
        // this fails rather than a widget quietly changing colour.
        use crate::config::Accent as A;
        for (a, dark, light) in [
            (A::Blue, "#5E6AD2", "#5058C4"),
            (A::Teal, "#2BB6A2", "#0E8C7E"),
            (A::Green, "#46B96B", "#2C9A4C"),
            (A::Amber, "#DBA13C", "#AE7820"),
            (A::Coral, "#F0708A", "#DE4567"),
            (A::Graphite, "#98A1B0", "#5B626F"),
        ] {
            assert_eq!(accent_hex(a, Mode::Dark), dark);
            assert_eq!(accent_hex(a, Mode::Light), light);
            assert_ne!(dark, light, "{a:?} must differ between modes");
        }
    }

    #[test]
    fn the_derived_accent_hexes_are_the_ones_the_spec_publishes() {
        use crate::config::Accent as A;
        let hex = |c: Color| {
            let [r, g, b, _] = c.to_premul_rgba8();
            format!("#{r:02X}{g:02X}{b:02X}")
        };
        for (a, ink, fill) in [
            (A::Blue, "#929AE0", "#757FD8"),
            (A::Teal, "#6FCDC0", "#49C0AF"),
            (A::Green, "#81CF9A", "#60C380"),
            (A::Amber, "#E7BF7A", "#E0AE57"),
            (A::Coral, "#F59EAF", "#F2849A"),
            (A::Graphite, "#B9BFC9", "#A6AEBB"),
        ] {
            let t = Theme::dark(accent_color(a, Mode::Dark));
            assert_eq!(hex(t.accent_ink), ink, "{a:?} dark ink");
            assert_eq!(hex(t.accent_fill), fill, "{a:?} dark fill");
        }
        for (a, ink) in [
            (A::Blue, "#3E4599"),
            (A::Teal, "#0B6D62"),
            (A::Green, "#22783B"),
            (A::Amber, "#885E19"),
            (A::Coral, "#AD3650"),
            (A::Graphite, "#474C57"),
        ] {
            let t = Theme::light(accent_color(a, Mode::Light));
            assert_eq!(hex(t.accent_ink), ink, "{a:?} light ink");
            // Light does not get a separate lighter fill: one value, two roles.
            assert_eq!(t.accent_fill, t.accent_ink);
        }
    }

    #[test]
    fn the_card_gradient_varies_colour_only_and_never_alpha() {
        for t in themes() {
            assert_eq!(
                t.surface.a, t.surface_far.a,
                "{:?} gradient varies alpha, which makes contrast positional",
                t.mode
            );
            assert_ne!(t.surface, t.surface_far, "{:?} gradient is flat", t.mode);
        }
    }

    #[test]
    fn the_card_gradient_runs_top_left_to_bottom_right() {
        let t = Theme::dark(Color::WHITE);
        let r = Rect::new(0.0, 0.0, 200.0, 100.0);
        let Fill::LinearGradient { from, to, stops } = t.card_fill(r) else {
            panic!("card fill is not a gradient");
        };
        // 160° is down and slightly right, so the far stop is below and to the
        // right of the near one.
        assert!(to.y > from.y && to.x > from.x, "{from:?} -> {to:?}");
        assert_eq!(stops.first().map(|s| s.color), Some(t.surface));
        assert_eq!(stops.last().map(|s| s.color), Some(t.surface_far));
        // A degenerate rect must still produce a usable line rather than NaN.
        let (a, b) = gradient_line(Rect::ZERO, 160.0);
        assert!(a.x.is_finite() && b.y.is_finite());
    }

    #[test]
    fn elevation_grows_with_level_and_reports_a_bleed_a_buffer_can_use() {
        for t in themes() {
            let (e1, e2, e3) = (t.e1(), t.e2(), t.e3());
            assert!(
                e1.bleed() < e2.bleed() && e2.bleed() < e3.bleed(),
                "{:?}",
                t.mode
            );
            assert!(e1.contact.is_none() && e2.contact.is_some() && e3.contact.is_some());
            assert_eq!(Elevation::NONE.bleed(), 0.0);
        }
        // The two figures spec §7.4 works through.
        assert_eq!(Theme::dark(Color::WHITE).e2().bleed(), 52.0);
        assert_eq!(Theme::light(Color::WHITE).e3().bleed(), 84.0);
        // Light shadows are larger, softer and cooler than dark's.
        let (d, l) = (Theme::dark(Color::WHITE), Theme::light(Color::WHITE));
        assert!(l.e2().key.blur > d.e2().key.blur);
        assert!(l.e2().key.alpha < d.e2().key.alpha);
        assert_eq!(d.shadow, Color::BLACK);
        assert_ne!(l.shadow, Color::BLACK);
        assert!(l.shadow.b > l.shadow.r, "light's shadow must be cool");
    }

    #[test]
    fn the_geometry_helpers_reproduce_the_worked_examples() {
        // §6.3 radii.
        assert_eq!(radius_card(64.0), 28.0);
        assert_eq!(radius_card(27.0), 12.0);
        assert_eq!(radius_card(18.0), 12.0);
        assert_eq!(radius_card(200.0), 32.0);
        // §6.2 padding.
        assert_eq!(card_padding(120.0), 16.0);
        assert_eq!(card_padding(200.0), 20.0);
        assert_eq!(card_padding(320.0), 24.0);
        assert_eq!(card_padding(400.0), 28.0);
        assert_eq!(card_padding(4000.0), 28.0);
        // §6.3 nesting.
        assert_eq!(radius_nested(12.0, 16.0), 4.0);
        assert_eq!(radius_nested(28.0, 20.0), 8.0);
        // Degenerate input clamps rather than producing NaN geometry.
        for bad in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
            assert!(radius_card(bad).is_finite());
            assert!(card_padding(bad).is_finite());
            assert!(radius_nested(bad, bad).is_finite());
        }
    }

    #[test]
    fn the_scrim_feather_is_wider_in_light_than_in_dark() {
        assert_eq!(Metrics::DARK.scrim_feather, 6.0);
        assert_eq!(Metrics::LIGHT.scrim_feather, 8.0);
        assert_eq!(Metrics::default(), Metrics::DARK);
    }

    #[test]
    fn wallpaper_leak_is_what_the_texture_argument_claims() {
        let leak = |a: f32| ((1.0 - a) * 1000.0).round() / 10.0;
        let (d, l) = (Theme::dark(Color::WHITE), Theme::light(Color::WHITE));
        assert_eq!(leak(d.surface.a), 28.0);
        assert_eq!(leak(1.0 - (1.0 - d.surface.a) * (1.0 - d.scrim.a)), 14.0);
        assert_eq!(leak(1.0 - (1.0 - d.surface.a) * (1.0 - d.well.a)), 12.6);
        assert_eq!(leak(l.surface.a), 10.0);
        assert!((leak(1.0 - (1.0 - l.surface.a) * (1.0 - l.scrim.a)) - 4.5).abs() < 0.05);
    }

    #[test]
    fn a_bad_accent_string_degrades_to_something_legible() {
        for bad in ["", "not a colour", "#12345", "#zzzzzz"] {
            for mode in [Mode::Dark, Mode::Light] {
                let t = Theme::from_accent_hex(mode, bad);
                assert!(t.passes_body_text(t.accent_ink), "{bad} {mode:?}");
                assert_eq!(t.accent, accent_color(crate::config::Accent::Blue, mode));
            }
        }
        let t = Theme::from_accent_hex(Mode::Dark, "#5E6AD2");
        assert_eq!(t.accent, Color::rgb8(0x5E, 0x6A, 0xD2));
        // An accent handed in with alpha is normalised: a translucent accent
        // would silently break every derived token.
        assert_eq!(Theme::dark(Color::WHITE.with_alpha(0.3)).accent.a, 1.0);
    }

    #[test]
    fn elevation_is_carried_by_opacity_so_it_survives_any_backdrop() {
        for t in themes() {
            assert!(t.surface_elevated.a > t.surface.a, "{:?}", t.mode);
            assert!(
                t.contrast_on_elevated(t.text_primary) >= AA_TEXT,
                "{:?}",
                t.mode
            );
        }
    }

    #[test]
    fn the_chassis_reads_as_a_solid_object_and_owns_its_own_colour() {
        for t in themes() {
            // Opaque: no wallpaper reaches its type, which is why none of the
            // translucent contrast tables apply to it.
            assert_eq!(t.chassis.a, 1.0, "{:?} chassis is see-through", t.mode);
            assert_eq!(t.chassis_far.a, 1.0);
            assert_eq!(t.chassis_well.a, 1.0);
            assert!(t.bevel_high.relative_luminance() > t.bevel_low.relative_luminance());
            // The readout colour is mode-independent by design; the *body* is
            // not, because a light instrument is aluminium rather than an
            // inverted dark one.
            assert_eq!(t.lcd, CHASSIS_LCD);
            // Every lit readout sits in a dark window in both palettes, which
            // is the whole reason one `lcd` can serve both.
            assert!(t.chassis_well.relative_luminance() < 0.05, "{:?}", t.mode);
            assert!(
                t.lcd.contrast_ratio(t.chassis_well) >= AA_TEXT,
                "{:?}",
                t.mode
            );
            // The body reads as one object: its two stops are close, and the
            // near stop is the lighter one, so the light falls from the top.
            assert!(t.chassis.relative_luminance() > t.chassis_far.relative_luminance());
        }
        // Light's body really is light, and dark's really is dark — the pair is
        // designed, not derived.
        assert!(Theme::light(Color::WHITE).chassis.relative_luminance() > 0.6);
        assert!(Theme::dark(Color::WHITE).chassis.relative_luminance() < 0.05);
        // Spec §9.3.3's published figures, recomputed.
        let t = Theme::dark(Color::WHITE);
        near(t.lcd.contrast_ratio(t.chassis_well), 9.09, "lcd on well");
        near(t.lcd.contrast_ratio(t.chassis), 6.99, "lcd on chassis");
        near(
            t.lcd
                .with_alpha(0.70)
                .over(t.chassis_well)
                .contrast_ratio(t.chassis_well),
            5.00,
            "lcd secondary",
        );
        // The two thinnings the spec forbids, shown failing.
        assert!(
            t.lcd
                .with_alpha(0.45)
                .over(t.chassis_well)
                .contrast_ratio(t.chassis_well)
                < AA_TEXT
        );
        assert!(
            t.lcd
                .with_alpha(0.60)
                .over(t.chassis)
                .contrast_ratio(t.chassis)
                < AA_TEXT
        );
        // The status strip's right-hand ink.
        near(
            Color::WHITE
                .with_alpha(0.62)
                .over(t.chassis)
                .contrast_ratio(t.chassis),
            6.41,
            "status right",
        );
    }

    /// The NOS clock is greyscale plus one red, and the red is a *graphic* —
    /// the dotted progress arc — so it is held to the 3:1 non-text minimum on
    /// the surface it is actually drawn on, in both palettes.
    #[test]
    fn the_nos_red_clears_the_non_text_minimum_in_both_palettes() {
        for t in themes() {
            let on = contrast_over(t.nos_red, t.text_backdrop());
            assert!(
                on >= AA_LARGE,
                "{:?} nos red is {on:.2}:1 on its own backdrop",
                t.mode
            );
            // And it is a red, not a brown or a pink: the sheets' single
            // chromatic accent has to read as *the* accent next to grey dots.
            assert!(t.nos_red.r > t.nos_red.g + 0.25 && t.nos_red.r > t.nos_red.b + 0.25);
        }
        // The obvious pick, shown failing, so nobody "simplifies" the two
        // reds back into one. `#E5484D` passes on the dark card (3.36:1) and
        // then fails on the light one, which is exactly how a single red gets
        // shipped: it is checked in the palette it was drawn in.
        let obvious = Color::rgb8(0xE5, 0x48, 0x4D);
        let dark = Theme::dark(Color::WHITE);
        let light = Theme::light(Color::WHITE);
        assert!(contrast_over(obvious, dark.text_backdrop()) >= AA_LARGE);
        assert!(contrast_over(obvious, light.text_backdrop()) < AA_LARGE);
    }

    /// **The accessibility gate on the dotted arc**, and the reason
    /// [`Theme::nos_dim`] exists.
    ///
    /// The lit arc and the dots around it must differ by something a greyscale
    /// print, or an eye without functioning L-cones, can still see. Hue is not
    /// that something: the first cut of this palette set the unlit dot to
    /// `text_tertiary` and the two colours landed **1.23:1** apart on dark and
    /// **1.13:1** on light — within 15% of the same luminance, differing in hue
    /// and in nothing else. So the token is fitted as a track instead, and this
    /// asserts the luminance channel it buys. The *size* channel is asserted
    /// where it lives, in `cards::nos`.
    #[test]
    fn the_nos_ring_separates_by_luminance_and_not_only_by_hue() {
        for t in themes() {
            let card = t.text_backdrop();
            let dim = t.nos_dim.over(card);
            // A real luminance step between lit and unlit. 1.4 is the floor the
            // design brief set; the fitted tokens clear it by half again.
            let step = t.nos_red.contrast_ratio(dim);
            assert!(
                step >= 1.8,
                "{:?} nos ring: lit vs unlit is only {step:.2}:1",
                t.mode
            );
            // And the unlit dot is still a dot: present against the card, in
            // the same band `track_empty` occupies, and never shouting — an
            // unlit dot at 3:1 would read as a second value rather than as the
            // remainder.
            let on_card = contrast_over(t.nos_dim, card);
            assert!(
                (1.4..AA_LARGE).contains(&on_card),
                "{:?} nos dim is {on_card:.2}:1 on the card",
                t.mode
            );
        }
        // The colour that used to be here, shown failing, so nobody
        // "simplifies" the ring back onto the tertiary ink.
        for t in themes() {
            let hue_only = t
                .nos_red
                .contrast_ratio(t.text_tertiary.over(t.text_backdrop()));
            assert!(hue_only < 1.4, "{:?} was {hue_only:.2}:1", t.mode);
        }
    }

    #[test]
    fn the_track_and_the_gridline_are_visible_without_shouting() {
        for t in themes() {
            let well = t.resolved_well();
            for (name, c) in [("track", t.track_empty), ("gridline", t.gridline)] {
                let r = contrast_over(c, well);
                assert!(r > 1.2, "{:?} {name} is invisible at {r:.2}:1", t.mode);
                assert!(r < AA_LARGE, "{:?} {name} shouts at {r:.2}:1", t.mode);
            }
            // The accent fill must clear its own track, not just the well.
            let track = t.track_empty.over(well);
            assert!(
                t.accent_fill.contrast_ratio(track) >= 2.0,
                "{:?} accent fill does not separate from its track",
                t.mode
            );
        }
    }

    #[test]
    fn mode_helpers_behave() {
        assert!(Mode::Dark.is_dark());
        assert!(!Mode::Light.is_dark());
        assert_eq!(Mode::Dark.flip(), Mode::Light);
        assert_eq!(Mode::default(), Mode::Dark);
        assert_eq!(Theme::new(Mode::Light, Color::WHITE).mode, Mode::Light);
    }
}

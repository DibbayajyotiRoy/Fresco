//! Colour, alpha and contrast.
//!
//! # Why straight alpha here and premultiplied only at the edges
//!
//! Every colour a card author writes is **straight** (non-premultiplied): a
//! `#1C1C1E` surface at 68% is `rgba8(0x1C, 0x1C, 0x1E, 0.68)`, exactly the
//! numbers a designer hands over. Premultiplication happens in one place only —
//! at the pixel writes in [`super::canvas`] and in
//! [`Canvas::into_bgra`](super::Canvas::into_bgra) — because that is what mpv's
//! `overlay-add` demands (see `crate::artwork`'s module docs for mpv's own
//! wording on the `max(B, G, R) <= A` invariant). Keeping the two conventions
//! in separate layers is the whole defence against the classic failure: dark
//! fringes around every glyph, which is what you get when a straight colour is
//! blended as if it were premultiplied.
//!
//! # Why WCAG luminance lives here
//!
//! Fresco's widgets sit on a *photograph*, not on a known background, so
//! "does this text read" cannot be answered by looking at the token values
//! alone — the surface is translucent and the thing behind it is arbitrary.
//! [`Color::over`] composites a translucent surface onto a stated worst-case
//! backdrop, and [`Color::contrast_ratio`] then scores the result. `super::theme`
//! uses both to *derive* its alphas rather than guess them, and the tests there
//! are what stop a future palette tweak from quietly shipping unreadable text.

/// An sRGB colour with **straight** (non-premultiplied) alpha, all channels in
/// `0.0..=1.0`.
///
/// `f32` rather than `u8` because gradients, contrast fitting and compositing
/// all interpolate, and rounding to bytes between every step visibly banded the
/// 32-step ASS ramp this toolkit exists to replace.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red, `0.0..=1.0`.
    pub r: f32,
    /// Green, `0.0..=1.0`.
    pub g: f32,
    /// Blue, `0.0..=1.0`.
    pub b: f32,
    /// Alpha, `0.0..=1.0`, **straight** — not multiplied into the channels.
    pub a: f32,
}

impl Color {
    /// Fully transparent. Not "transparent black": the channels are zero, so
    /// blending it changes nothing at all.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// From channels in `0.0..=1.0`. Values outside the range are clamped, so a
    /// gradient extrapolation or a lightening step cannot produce a colour that
    /// later violates the premultiplication invariant.
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: clamp01(r),
            g: clamp01(g),
            b: clamp01(b),
            a: clamp01(a),
        }
    }

    /// From 8-bit channels plus a float alpha — the form design tokens are
    /// usually written in (`#1C1C1E` at 68%).
    pub fn rgba8(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self::rgba(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            a,
        )
    }

    /// Opaque, from 8-bit channels.
    pub fn rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::rgba8(r, g, b, 1.0)
    }

    /// Parse `#RRGGBB`, `#RGB`, `#RRGGBBAA` or the same without the `#`.
    ///
    /// Returns `None` rather than erroring on a bad string: the only caller
    /// that matters is the user's accent colour (`accent_hex()` in
    /// `crate::daemon`), and a widget must never fail to draw because a colour
    /// preference was mistyped — it falls back to a token instead.
    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.strip_prefix('#').unwrap_or(s);
        let n = |i: usize, len: usize| -> Option<u8> {
            let part = h.get(i..i + len)?;
            let v = u8::from_str_radix(part, 16).ok()?;
            // "#abc" means "#aabbcc": replicate the nibble, don't shift.
            Some(if len == 1 { v * 17 } else { v })
        };
        match h.len() {
            3 => Some(Self::rgb8(n(0, 1)?, n(1, 1)?, n(2, 1)?)),
            6 => Some(Self::rgb8(n(0, 2)?, n(2, 2)?, n(4, 2)?)),
            8 => Some(Self::rgba8(
                n(0, 2)?,
                n(2, 2)?,
                n(4, 2)?,
                f32::from(n(6, 2)?) / 255.0,
            )),
            _ => None,
        }
    }

    /// The same colour at a different alpha. Tokens are built this way so a
    /// palette reads as "white at 10%" rather than as an opaque grey somebody
    /// has to reverse-engineer.
    pub fn with_alpha(self, a: f32) -> Self {
        Self {
            a: clamp01(a),
            ..self
        }
    }

    /// Multiply the existing alpha (for fading a whole token set out).
    pub fn scale_alpha(self, k: f32) -> Self {
        self.with_alpha(self.a * k)
    }

    /// Linear interpolation in straight-alpha sRGB space, `t` clamped to
    /// `0.0..=1.0`.
    ///
    /// Straight, not premultiplied: this is the reference semantic for
    /// [`super::Fill`] gradient stops, and it is what tiny-skia's gradients do
    /// between stops, so `sample_stops` and the rasteriser agree.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = clamp01(t);
        Self::rgba(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }

    /// Composite `self` **over** `under`, straight-alpha source-over.
    ///
    /// This is the function that makes contrast checking meaningful for
    /// translucent widgets: the token is 68% of `#1C1C1E`, but what the eye
    /// actually judges text against is that token composited onto whatever the
    /// wallpaper is showing. `super::theme` feeds it a deliberate worst case.
    pub fn over(self, under: Self) -> Self {
        let ia = 1.0 - self.a;
        let a = self.a + under.a * ia;
        if a <= f32::EPSILON {
            return Self::TRANSPARENT;
        }
        let ch = |s: f32, u: f32| (s * self.a + u * under.a * ia) / a;
        Self::rgba(
            ch(self.r, under.r),
            ch(self.g, under.g),
            ch(self.b, under.b),
            a,
        )
    }

    /// WCAG 2.x relative luminance of the colour's **channels**, ignoring
    /// alpha.
    ///
    /// Alpha is ignored deliberately rather than forgotten: a relative
    /// luminance is only defined for something you can actually see, so the
    /// caller must resolve translucency with [`Color::over`] first. Feeding a
    /// translucent colour straight in is how you talk yourself into a palette
    /// that fails on a real wallpaper.
    pub fn relative_luminance(self) -> f32 {
        0.2126 * srgb_to_linear(self.r)
            + 0.7152 * srgb_to_linear(self.g)
            + 0.0722 * srgb_to_linear(self.b)
    }

    /// WCAG contrast ratio between two opaque colours, `1.0..=21.0`.
    ///
    /// 4.5:1 is the AA threshold for body text, 3:1 for large text (≥ 24 px, or
    /// ≥ 18.66 px bold) and for non-text indicators like a gauge track.
    pub fn contrast_ratio(self, other: Self) -> f32 {
        let (a, b) = (self.relative_luminance(), other.relative_luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Nudge `self` toward white or black — whichever direction `bg` leaves
    /// room for — until it reaches `target` contrast against `bg`.
    ///
    /// This exists for one reason: **the accent colour is the user's, not
    /// ours.** Fresco lets the user pick from `Accent` in `crate::config`, and
    /// a card that hardcodes "accent text reads fine" is wrong for at least one
    /// of those choices on at least one of the two themes. Rather than clamp
    /// the user's palette, each theme tints its *own* copy of the accent just
    /// far enough to be legible on its own surface and leaves the chrome
    /// (gauge fills, glow) on the untouched original.
    ///
    /// Returns `self` unchanged when it already passes, and returns the best
    /// achievable colour (pure white or pure black) when `target` is
    /// unreachable — never an error, because a widget must always draw.
    pub fn ensure_contrast(self, bg: Self, target: f32) -> Self {
        if self.contrast_ratio(bg) >= target {
            return self;
        }
        let toward = if bg.relative_luminance() < 0.5 {
            Self::WHITE
        } else {
            Self::BLACK
        };
        // Contrast is monotonic in `t` once the direction is fixed (we only
        // ever move away from the background's luminance), so a bisection is
        // exact to within the step count. 16 steps resolves finer than an
        // 8-bit channel can represent, which is the only precision that ships.
        let mut lo = 0.0_f32;
        let mut hi = 1.0_f32;
        let mix = |t: f32| Self {
            a: self.a,
            ..self.lerp(toward, t)
        };
        if mix(1.0).contrast_ratio(bg) < target {
            return mix(1.0);
        }
        for _ in 0..16 {
            let mid = 0.5 * (lo + hi);
            if mix(mid).contrast_ratio(bg) >= target {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        mix(hi)
    }

    /// As tiny-skia's colour type, which is also straight alpha.
    pub(crate) fn to_tiny(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba(self.r, self.g, self.b, self.a)
            .unwrap_or(tiny_skia::Color::TRANSPARENT)
    }

    /// Premultiplied 8-bit `[r, g, b, a]`.
    ///
    /// Rounds, then clamps each colour byte to the alpha byte. The clamp is not
    /// paranoia: rounding `0.6 * 0.6 = 0.36 → 92` against `0.6 → 153` is fine,
    /// but a colour built by lightening can round a channel one unit past its
    /// alpha, and mpv documents that as undefined-per-VO behaviour rather than
    /// as something it will tolerate.
    pub fn to_premul_rgba8(self) -> [u8; 4] {
        let a = round8(self.a);
        [
            round8(self.r * self.a).min(a),
            round8(self.g * self.a).min(a),
            round8(self.b * self.a).min(a),
            a,
        ]
    }

    /// Inverse of [`Color::to_premul_rgba8`]. Fully transparent input yields
    /// [`Color::TRANSPARENT`] — the colour it was carrying is unrecoverable and
    /// also invisible, so inventing one would be worse than dropping it.
    pub fn from_premul_rgba8(p: [u8; 4]) -> Self {
        let a = f32::from(p[3]) / 255.0;
        if a <= 0.0 {
            return Self::TRANSPARENT;
        }
        Self::rgba(
            f32::from(p[0]) / 255.0 / a,
            f32::from(p[1]) / 255.0 / a,
            f32::from(p[2]) / 255.0 / a,
            a,
        )
    }
}

/// sRGB transfer function, gamma-encoded → linear light. WCAG's exact form,
/// including the linear segment near black.
pub fn srgb_to_linear(c: f32) -> f32 {
    let c = clamp01(c);
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Inverse of [`srgb_to_linear`].
pub fn linear_to_srgb(c: f32) -> f32 {
    let c = clamp01(c);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn clamp01(v: f32) -> f32 {
    // `f32::clamp` panics on NaN; a NaN here would come from a divide in a
    // gradient or a contrast fit and must degrade to a drawn pixel, not a
    // crash, for the same reason `crate::clock`'s `du()` saturates.
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

fn round8(v: f32) -> u8 {
    (clamp01(v) * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_every_common_spelling() {
        assert_eq!(Color::from_hex("#1C1C1E"), Some(Color::rgb8(28, 28, 30)));
        assert_eq!(Color::from_hex("1C1C1E"), Some(Color::rgb8(28, 28, 30)));
        // "#abc" is "#aabbcc", not "#0a0b0c".
        assert_eq!(Color::from_hex("#abc"), Some(Color::rgb8(0xAA, 0xBB, 0xCC)));
        let with_alpha = Color::from_hex("#00000080").unwrap();
        assert!((with_alpha.a - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(Color::from_hex("#12345"), None);
        assert_eq!(Color::from_hex("#zzzzzz"), None);
        assert_eq!(Color::from_hex(""), None);
    }

    #[test]
    fn premultiplication_round_trips_within_a_byte() {
        // The invariant every glyph depends on. A break here is the "dark
        // fringe around the text" bug.
        for &(r, g, b, a) in &[
            (1.0, 1.0, 1.0, 1.0),
            (1.0, 1.0, 1.0, 0.5),
            (0.0, 0.0, 0.0, 0.25),
            (0.11, 0.42, 0.93, 0.68),
            (0.96, 0.65, 0.14, 0.87),
        ] {
            let c = Color::rgba(r, g, b, a);
            let p = c.to_premul_rgba8();
            assert!(
                p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3],
                "premultiplied {p:?} violates max(r,g,b) <= a"
            );
            let back = Color::from_premul_rgba8(p);
            // Round-trip error is bounded by the quantisation of the *premultiplied*
            // value divided back out, i.e. it grows as alpha shrinks. One byte of
            // premultiplied slop over `a` is the honest tolerance.
            let tol = 1.5 / 255.0 / a;
            assert!((back.r - c.r).abs() <= tol, "{back:?} vs {c:?}");
            assert!((back.g - c.g).abs() <= tol, "{back:?} vs {c:?}");
            assert!((back.b - c.b).abs() <= tol, "{back:?} vs {c:?}");
            assert!((back.a - c.a).abs() <= 1.0 / 255.0);
        }
        // Fully transparent has no recoverable colour, and says so.
        assert_eq!(Color::from_premul_rgba8([0, 0, 0, 0]), Color::TRANSPARENT);
    }

    #[test]
    fn premultiplied_bytes_never_exceed_alpha_anywhere_in_the_range() {
        // Exhaustive over alpha, worst case channel (white), because this is
        // the rounding boundary the clamp exists for.
        for i in 0..=255u16 {
            let a = f32::from(i) / 255.0;
            let p = Color::WHITE.with_alpha(a).to_premul_rgba8();
            assert!(p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3], "alpha {i}");
        }
    }

    #[test]
    fn source_over_matches_hand_computed_composites() {
        // Opaque over anything is itself.
        let red = Color::rgb8(255, 0, 0);
        assert_eq!(red.over(Color::WHITE), red);
        // Transparent over anything is the thing.
        assert_eq!(Color::TRANSPARENT.over(red), red);
        // 50% black over white is the sRGB *value* 127.5, not the perceptual mid.
        let half = Color::BLACK.with_alpha(0.5).over(Color::WHITE);
        assert!((half.r - 0.5).abs() < 1e-6);
        assert!((half.a - 1.0).abs() < 1e-6);
    }

    #[test]
    fn contrast_ratio_hits_the_known_endpoints() {
        assert!((Color::WHITE.contrast_ratio(Color::BLACK) - 21.0).abs() < 0.01);
        assert!((Color::WHITE.contrast_ratio(Color::WHITE) - 1.0).abs() < 1e-6);
        // Symmetric by construction.
        let a = Color::rgb8(0x1C, 0x1C, 0x1E);
        let b = Color::rgb8(0xF5, 0xA6, 0x23);
        assert!((a.contrast_ratio(b) - b.contrast_ratio(a)).abs() < 1e-6);
        // sRGB mid-grey against white is the textbook ~3.95:1, not 2:1 — the
        // gamma curve is exactly why we cannot eyeball these numbers.
        let mid = Color::rgb8(128, 128, 128);
        let r = mid.contrast_ratio(Color::WHITE);
        assert!((3.9..4.1).contains(&r), "{r}");
    }

    #[test]
    fn ensure_contrast_lifts_a_dark_accent_off_a_dark_surface() {
        let surface = Color::rgb8(0x1C, 0x1C, 0x1E);
        let dark_accent = Color::rgb8(0x1E, 0x3A, 0x8A); // deep blue: unreadable here
        assert!(dark_accent.contrast_ratio(surface) < 4.5);
        let fixed = dark_accent.ensure_contrast(surface, 4.5);
        assert!(fixed.contrast_ratio(surface) >= 4.5, "{fixed:?}");
        // It moved toward white, not to white: hue is preserved as far as it can be.
        assert!(fixed.b > fixed.r, "accent lost its hue: {fixed:?}");
        // Alpha survives the fit.
        let translucent = dark_accent.with_alpha(0.8).ensure_contrast(surface, 4.5);
        assert!((translucent.a - 0.8).abs() < 1e-6);
    }

    #[test]
    fn ensure_contrast_darkens_against_a_light_surface_and_is_a_no_op_when_it_passes() {
        let surface = Color::rgb8(0xF5, 0xF5, 0xF7);
        let bright = Color::rgb8(0xFF, 0xD1, 0x66);
        let fixed = bright.ensure_contrast(surface, 4.5);
        assert!(fixed.contrast_ratio(surface) >= 4.5);
        assert!(fixed.relative_luminance() < bright.relative_luminance());
        // Already-passing colours are returned bit-identical.
        let ok = Color::rgb8(0x10, 0x10, 0x10);
        assert_eq!(ok.ensure_contrast(surface, 4.5), ok);
        // Impossible targets return the best available rather than failing.
        let capped = bright.ensure_contrast(Color::rgb8(0x80, 0x80, 0x80), 21.0);
        assert!(capped.contrast_ratio(Color::rgb8(0x80, 0x80, 0x80)) > 1.0);
    }

    #[test]
    fn transfer_functions_are_inverses() {
        for i in 0..=100i32 {
            let c = i as f32 / 100.0;
            assert!((linear_to_srgb(srgb_to_linear(c)) - c).abs() < 1e-4, "{c}");
        }
    }

    #[test]
    fn nan_and_out_of_range_input_is_clamped_not_propagated() {
        let c = Color::rgba(f32::NAN, 2.0, -1.0, f32::INFINITY);
        assert_eq!(c, Color::rgba(0.0, 1.0, 0.0, 1.0));
        assert!(c.relative_luminance().is_finite());
    }
}

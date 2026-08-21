//! The drawing surface: paths, gradients, soft shadows, arcs, text, images.
//!
//! # What comes out of it
//!
//! [`Canvas::into_bgra`] produces `crate::artwork::Bgra` — the *same* type
//! `render_disc` already returns and the widget engine already knows how to
//! hand to mpv's `overlay-add`. Nothing new has to learn how to ship pixels:
//! a widget that can draw a disc can draw a card.
//!
//! # Premultiplied alpha, end to end
//!
//! tiny-skia stores **premultiplied RGBA**; mpv wants **premultiplied BGRA**.
//! The whole difference is a per-pixel `swap(0, 2)`, and that swap is the one
//! place the invariant `max(B, G, R) <= A` can be broken by a stray write, so
//! [`Canvas::write_bgra`] re-clamps as it swaps. `crate::artwork`'s module docs
//! quote mpv's own wording on why violating it is undefined rather than merely
//! ugly; the test `bgra_output_is_always_premultiplied` here is the local copy
//! of `artwork`'s `colour_never_exceeds_alpha_anywhere`.
//!
//! Text is the other exposure. Glyph coverage arrives from cosmic-text as a
//! **straight** colour whose alpha is already multiplied by coverage, and it is
//! composited by `blend_over` below, which premultiplies before blending.
//! Blending straight values as if they were premultiplied is exactly the bug
//! that shows up as a dark halo around every letter.
//!
//! # Allocation and reuse
//!
//! A `Canvas` owns three buffers and is designed to be **kept alive across
//! frames**:
//!
//! * the `Pixmap` (`w × h × 4` bytes),
//! * a shadow `Mask` (`w × h` bytes), allocated on the first
//!   [`Canvas::drop_shadow`] and never again,
//! * the blur scratch (`w × h` bytes), likewise.
//!
//! [`Canvas::reset`] zeroes the pixmap in place and keeps all three, so the
//! steady-state cost of a repaint is zero allocations. [`Canvas::resize`]
//! reallocates *only* when the pixel size actually changes, which happens on a
//! mode or rotation change, not per frame. A caller that instead built a
//! `Canvas` per frame would allocate ~6 bytes per pixel per frame; with all
//! four widgets on bitmaps that is tens of MB/s of churn while music plays,
//! for no benefit.
//!
//! # Everything clamps
//!
//! `panic = "abort"` is set in `[profile.release]`, so a panic here does not
//! unwind — it kills the daemon and takes the user's wallpaper with it. That is
//! the same bar `crate::artwork` holds itself to ("no combination of settings
//! can panic"). Zero sizes, NaN radii, negative sweeps, radii larger than the
//! rectangle, shadows bigger than the canvas: all clamp or no-op. The only
//! fallible entry points are the constructors, which return `anyhow::Result`
//! because an allocation that big genuinely can fail.

use anyhow::{bail, Context, Result};
use tiny_skia::{
    FillRule, FilterQuality, Mask, Paint, PathBuilder, Pattern, Pixmap, PremultipliedColorU8,
    SpreadMode, Stroke, Transform,
};

use crate::artwork::Bgra;

use super::blur::blur_alpha;
use super::color::Color;
use super::geom::{Point, Rect, Size};
use super::paint::Fill;
use super::text::{FontStack, TextRun};

/// Hard ceiling on either side of a canvas, in device pixels.
///
/// 4096 covers a full-width strip on a 4K output with room to spare. It exists
/// for the same reason `crate::artwork`'s `MAX_DISC_PX` does: a size arrives
/// from a config file and an output's reported mode, and neither is something
/// this code should trust to be sane.
pub const MAX_CANVAS_PX: u32 = 4096;

/// Hard ceiling on a canvas's **area**, in device pixels — 4 megapixels, i.e.
/// 16 MiB of BGRA.
///
/// The per-side cap alone is not enough: 4096 × 4096 would be 64 MiB, and a
/// full-screen 4K widget 33 MiB, *per frame*. No Fresco widget is full-screen —
/// the visualiser is a strip, the cards are cards — so a request that large is
/// a bug in the caller, and failing loudly beats quietly allocating it every
/// tick. [`Canvas::clamp_size`] is there for callers that would rather shrink
/// than fail.
pub const MAX_CANVAS_AREA: u32 = 4 * 1024 * 1024;

/// The `scale` corresponding to Fresco's ASS coordinate space.
///
/// The ASS widgets are laid out in a virtual `RES_X × RES_Y` (1920 × 1080)
/// space and libass rescales to the real output, so every size in the existing
/// widget config means "pixels at 1080p". Bitmaps get no such help — mpv's
/// `overlay-add` is in real output pixels — so the toolkit reproduces it by
/// hand: **`scale = output_height / 1080.0`**. Feed the same numbers as the ASS
/// themes use and they keep their meaning.
pub const REFERENCE_HEIGHT: f32 = 1080.0;

/// The canvas scale for an output `out_h` pixels tall.
///
/// Clamped to a sane band so a compositor reporting a mode it has not brought
/// up yet (height 0) cannot produce a zero or infinite scale.
pub fn scale_for_output(out_h: u32) -> f32 {
    if out_h == 0 {
        return 1.0;
    }
    (out_h as f32 / REFERENCE_HEIGHT).clamp(0.25, 8.0)
}

/// A drawing surface in **logical** units at a fixed device `scale`.
///
/// Every coordinate a caller passes is logical (see `super::geom`); the
/// conversion to device pixels happens inside each primitive, at the moment the
/// path is built. That is deliberately *not* a transform handed to the
/// rasteriser: shaders carry their own transform in tiny-skia, and a gradient
/// whose coordinates were scaled by a different matrix than its shape is a
/// class of bug that only appears at non-unit scales — i.e. on the 4K monitor
/// none of us is testing on. Scaling both by hand, in one place, makes that
/// impossible.
pub struct Canvas {
    pixmap: Pixmap,
    scale: f32,
    /// Reused shadow coverage buffer. `None` until the first shadow.
    shadow_mask: Option<Mask>,
    /// Reused ping-pong buffer for the blur passes.
    blur_scratch: Vec<u8>,
}

/// Geometry only, never pixels — the same rule `Bgra`'s hand-written `Debug` in
/// `crate::artwork` follows, and for the same reason: this ends up inside
/// `{:?}` on a log line.
impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Canvas")
            .field("w", &self.pixmap.width())
            .field("h", &self.pixmap.height())
            .field("scale", &self.scale)
            .finish()
    }
}

impl Canvas {
    /// A transparent canvas `w × h` **device** pixels, drawn in logical units
    /// at `scale`.
    ///
    /// Fails on a zero dimension and on anything past [`MAX_CANVAS_PX`] /
    /// [`MAX_CANVAS_AREA`], rather than clamping silently: a caller that asked
    /// for 4K full-screen wants to know it did.
    pub fn new(w: u32, h: u32, scale: f32) -> Result<Self> {
        if w == 0 || h == 0 {
            bail!("widgetkit canvas must have a non-zero size, got {w}x{h}");
        }
        if w > MAX_CANVAS_PX || h > MAX_CANVAS_PX {
            bail!("widgetkit canvas {w}x{h} exceeds the {MAX_CANVAS_PX}px per-side cap");
        }
        if w.saturating_mul(h) > MAX_CANVAS_AREA {
            bail!(
                "widgetkit canvas {w}x{h} exceeds the {MAX_CANVAS_AREA}px area cap \
                 ({} MiB of BGRA)",
                (w as u64 * h as u64 * 4) / (1024 * 1024)
            );
        }
        let pixmap = Pixmap::new(w, h)
            .with_context(|| format!("allocating a {w}x{h} widget pixmap failed"))?;
        Ok(Self {
            pixmap,
            scale: sane_scale(scale),
            shadow_mask: None,
            blur_scratch: Vec::new(),
        })
    }

    /// A canvas big enough for `size` logical units at `scale`.
    ///
    /// Rounds **up**: a card 100.4 units wide gets 101 device pixels at scale 1
    /// rather than losing its right edge to truncation.
    pub fn for_logical(size: Size, scale: f32) -> Result<Self> {
        let s = sane_scale(scale);
        let w = (size.w * s).ceil().max(1.0) as u32;
        let h = (size.h * s).ceil().max(1.0) as u32;
        Self::new(w, h, s)
    }

    /// The largest size within both caps that keeps `w × h`'s aspect ratio.
    ///
    /// For callers that would rather draw something smaller than draw nothing.
    /// Returns `(1, 1)` for a degenerate request, never `(0, _)`.
    pub fn clamp_size(w: u32, h: u32) -> (u32, u32) {
        let (mut w, mut h) = (w.clamp(1, MAX_CANVAS_PX), h.clamp(1, MAX_CANVAS_PX));
        let area = w as u64 * h as u64;
        if area > u64::from(MAX_CANVAS_AREA) {
            let k = (f64::from(MAX_CANVAS_AREA) / area as f64).sqrt();
            w = ((w as f64 * k).floor() as u32).max(1);
            h = ((h as f64 * k).floor() as u32).max(1);
        }
        (w, h)
    }

    /// Width in device pixels.
    pub fn width_px(&self) -> u32 {
        self.pixmap.width()
    }
    /// Height in device pixels.
    pub fn height_px(&self) -> u32 {
        self.pixmap.height()
    }
    /// Device pixels per logical unit.
    pub fn scale(&self) -> f32 {
        self.scale
    }
    /// The whole surface in logical units, origin at `(0, 0)` — the rectangle
    /// a card lays itself out inside.
    pub fn bounds(&self) -> Rect {
        Rect::new(
            0.0,
            0.0,
            self.pixmap.width() as f32 / self.scale,
            self.pixmap.height() as f32 / self.scale,
        )
    }

    /// Clear to fully transparent, keeping every buffer. The per-frame entry
    /// point: `reset`, redraw, `write_bgra`.
    pub fn reset(&mut self) {
        self.pixmap.fill(tiny_skia::Color::TRANSPARENT);
    }

    /// Point this canvas at a new size and/or scale, reallocating **only** if
    /// the pixel dimensions changed. Always leaves the surface cleared.
    ///
    /// This is the call for a mode or rotation change: a canvas that is already
    /// the right size costs a `memset`, not an allocation.
    pub fn resize(&mut self, w: u32, h: u32, scale: f32) -> Result<()> {
        self.scale = sane_scale(scale);
        if self.pixmap.width() == w && self.pixmap.height() == h {
            self.reset();
            return Ok(());
        }
        let fresh = Self::new(w, h, self.scale)?;
        self.pixmap = fresh.pixmap;
        // The mask and scratch are sized lazily against the pixmap, so simply
        // dropping them is enough; they come back on the next shadow.
        self.shadow_mask = None;
        self.blur_scratch = Vec::new();
        Ok(())
    }

    // -- primitives ---------------------------------------------------------

    /// Paint the entire surface. Mostly for previews and tests — a real widget
    /// is transparent outside its card.
    pub fn fill(&mut self, fill: &Fill) {
        let r = self.bounds();
        self.rounded_rect(r, 0.0, fill);
    }

    /// An anti-aliased rounded rectangle.
    ///
    /// `radius` is clamped to half the shorter side, so `radius = 0` is a plain
    /// rectangle and any radius at or past half gives a stadium (or a circle,
    /// for a square). Fractional radii are exact — the corners are real cubic
    /// arcs, not a stair-step.
    pub fn rounded_rect(&mut self, r: Rect, radius: f32, fill: &Fill) {
        if r.is_empty() || fill.is_invisible() {
            return;
        }
        let Some(path) = rounded_rect_path(r, radius, self.scale) else {
            return;
        };
        let paint = Paint {
            shader: fill.to_shader(self.scale),
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// A **squircle** — the same rectangle with a continuous corner instead of
    /// a circular one.
    ///
    /// The NOS design language is built on these (spec §9.5): near-square cards
    /// with heavy rounding, in iOS-widget proportions. A circular corner at the
    /// radius that language wants — 30% of the side — reads as a bubble, and
    /// the difference between a bubble and a squircle is the whole difference
    /// between "rounded rectangle" and "iOS widget".
    ///
    /// The curve is a **single-cubic superellipse approximation**, not a true
    /// `|x|ⁿ + |y|ⁿ = 1`: the corner is entered [`SQUIRCLE_SPREAD`] × `radius`
    /// back along each edge and the control points are pulled in by
    /// [`SQUIRCLE_KAPPA`] rather than by the circular `KAPPA`, which flattens
    /// the approach to the tangent and tightens the 45° apex — the two things
    /// the eye actually reads as continuity. It is one cubic per corner, the
    /// same cost as [`Canvas::rounded_rect`], and at Fresco's radii the error
    /// against a real superellipse is well under a device pixel.
    ///
    /// `radius` is clamped so the four corners can never overrun each other,
    /// which means a `radius` at or past `min_side / (2 · SQUIRCLE_SPREAD)`
    /// gives the roundest shape this curve has and not a broken path.
    pub fn squircle(&mut self, r: Rect, radius: f32, fill: &Fill) {
        if r.is_empty() || fill.is_invisible() {
            return;
        }
        let Some(path) = squircle_path(r, radius, self.scale) else {
            return;
        };
        let paint = Paint {
            shader: fill.to_shader(self.scale),
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// [`Canvas::hairline`], on a squircle.
    ///
    /// Inside the shape for the same reason the rectangular one is: a card's
    /// edge light must not bleed a pixel past the panel it belongs to.
    pub fn squircle_hairline(&mut self, r: Rect, radius: f32, color: Color, width: f32) {
        if r.is_empty() || color.a <= 0.0 || width <= 0.0 {
            return;
        }
        let half = width / 2.0;
        let inner = r.inset(half);
        if inner.is_empty() {
            return;
        }
        let Some(path) = squircle_path(inner, (radius - half).max(0.0), self.scale) else {
            return;
        };
        self.stroke(&path, &Fill::Solid(color), width, tiny_skia::LineCap::Butt);
    }

    /// A filled triangle.
    ///
    /// The one shape the toolkit needs that is neither a rectangle nor an arc:
    /// a play indicator. Drawn rather than typed, for the same reason the
    /// missing-artwork note is — `▶` is absent from plenty of installed faces
    /// and a tofu box where a state indicator goes reads as a rendering fault.
    ///
    /// Degenerate input (non-finite points, zero area) is a no-op.
    pub fn triangle(&mut self, a: Point, b: Point, c: Point, fill: &Fill) {
        if fill.is_invisible() {
            return;
        }
        let pts = [a, b, c];
        if pts.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return;
        }
        let s = self.scale;
        let mut pb = PathBuilder::new();
        pb.move_to(a.x * s, a.y * s);
        pb.line_to(b.x * s, b.y * s);
        pb.line_to(c.x * s, c.y * s);
        pb.close();
        let Some(path) = pb.finish() else { return };
        let paint = Paint {
            shader: fill.to_shader(self.scale),
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// A hairline edge drawn **inside** `r`.
    ///
    /// Inside, not centred on the boundary, because a card's edge light must
    /// not bleed one pixel past the panel it belongs to — at `scale = 1` a
    /// centred 1-unit stroke would put half a unit of white outside the card,
    /// where there is nothing to sit on.
    pub fn hairline(&mut self, r: Rect, radius: f32, color: Color, width: f32) {
        if r.is_empty() || color.a <= 0.0 || width <= 0.0 {
            return;
        }
        let half = width / 2.0;
        let inner = r.inset(half);
        if inner.is_empty() {
            return;
        }
        let Some(path) = rounded_rect_path(inner, (radius - half).max(0.0), self.scale) else {
            return;
        };
        self.stroke(&path, &Fill::Solid(color), width, tiny_skia::LineCap::Butt);
    }

    /// A brighter light along the **top edge only**, fading out at the corners.
    ///
    /// Not decoration for its own sake: this is the single cue that reads
    /// hardest as "glass". `crate::clock`'s ASS card already relies on it and
    /// says so — a hairline plus a brighter top edge signals a light source
    /// above the surface more strongly than any blur does. Here it can also
    /// *fade*, which ASS could not do, so the highlight dies away into the
    /// corners instead of stopping dead.
    pub fn top_highlight(&mut self, r: Rect, radius: f32, color: Color, width: f32) {
        if r.is_empty() || color.a <= 0.0 || width <= 0.0 {
            return;
        }
        let half = width / 2.0;
        let inner = r.inset(half);
        if inner.is_empty() {
            return;
        }
        let rad = clamp_radius((radius - half).max(0.0), inner);
        let mut pb = PathBuilder::new();
        let s = self.scale;
        let (l, t, right) = (inner.left() * s, inner.top() * s, inner.right() * s);
        let k = rad * s * KAPPA;
        let rad = rad * s;
        pb.move_to(l, t + rad);
        if rad > 0.0 {
            pb.cubic_to(l, t + rad - k, l + rad - k, t, l + rad, t);
        }
        pb.line_to(right - rad, t);
        if rad > 0.0 {
            pb.cubic_to(right - rad + k, t, right, t + rad - k, right, t + rad);
        }
        let Some(path) = pb.finish() else { return };
        // Fade to nothing over the outer sixth at each end, so the highlight
        // has no visible start or stop.
        let fade = Fill::LinearGradient {
            from: Point::new(inner.left(), inner.top()),
            to: Point::new(inner.right(), inner.top()),
            stops: vec![
                super::paint::Stop::new(0.0, color.with_alpha(0.0)),
                super::paint::Stop::new(0.18, color),
                super::paint::Stop::new(0.82, color),
                super::paint::Stop::new(1.0, color.with_alpha(0.0)),
            ],
        };
        self.stroke(&path, &fade, width, tiny_skia::LineCap::Round);
    }

    /// A shaded arc along the **bottom edge only**, fading out at the corners.
    ///
    /// The mirror of [`Canvas::top_highlight`], and the other half of a bevel:
    /// a well is read as *inset* by a dark edge at the top and a light one at
    /// the bottom, and a chassis is read as *raised* by the opposite. Neither
    /// is expressible with one stroke, which is why there are two.
    pub fn bottom_highlight(&mut self, r: Rect, radius: f32, color: Color, width: f32) {
        if r.is_empty() || color.a <= 0.0 || width <= 0.0 {
            return;
        }
        let half = width / 2.0;
        let inner = r.inset(half);
        if inner.is_empty() {
            return;
        }
        let rad = clamp_radius((radius - half).max(0.0), inner);
        let mut pb = PathBuilder::new();
        let s = self.scale;
        let (l, b, right) = (inner.left() * s, inner.bottom() * s, inner.right() * s);
        let k = rad * s * KAPPA;
        let rad = rad * s;
        pb.move_to(l, b - rad);
        if rad > 0.0 {
            pb.cubic_to(l, b - rad + k, l + rad - k, b, l + rad, b);
        }
        pb.line_to(right - rad, b);
        if rad > 0.0 {
            pb.cubic_to(right - rad + k, b, right, b - rad + k, right, b - rad);
        }
        let Some(path) = pb.finish() else { return };
        let fade = Fill::LinearGradient {
            from: Point::new(inner.left(), inner.bottom()),
            to: Point::new(inner.right(), inner.bottom()),
            stops: vec![
                super::paint::Stop::new(0.0, color.with_alpha(0.0)),
                super::paint::Stop::new(0.18, color),
                super::paint::Stop::new(0.82, color),
                super::paint::Stop::new(1.0, color.with_alpha(0.0)),
            ],
        };
        self.stroke(&path, &fade, width, tiny_skia::LineCap::Round);
    }

    /// A rounded rectangle with a **feathered** edge — the scrim primitive.
    ///
    /// Same machinery as [`Canvas::drop_shadow`] with no offset: the shape is
    /// rasterised into the reusable mask, Gaussian-blurred with
    /// `sigma = feather / 2`, and filled with `color`. The interior stays at
    /// `color`'s own alpha and the transition straddles `r`'s boundary, which
    /// is what a scrim needs — a hard-edged plate behind text reads as a second
    /// card, and the whole point of the scrim is that you should not see it.
    ///
    /// `feather = 0` degenerates to a plain [`Canvas::rounded_rect`].
    pub fn soft_plate(&mut self, r: Rect, radius: f32, feather: f32, color: Color) {
        if !(feather.is_finite() && feather > 0.0) {
            self.rounded_rect(r, radius, &Fill::solid(color));
            return;
        }
        self.drop_shadow(r, radius, feather, 0.0, color);
    }

    /// [`Canvas::soft_plate`] with a radius **per corner** — top-left,
    /// top-right, bottom-right, bottom-left.
    ///
    /// A scrim that runs flush with three of a card's edges needs the card's
    /// own radius on the two corners it shares with the card and **none** on
    /// the two it does not. Given one radius for all four it scallops its free
    /// edge, and a plate with four rounded corners floating inside a card is
    /// exactly what reads as a second card — which is the thing a scrim must
    /// never do (spec §9.2).
    ///
    /// Costs one full-canvas blur, the same as [`Canvas::drop_shadow`].
    /// Non-finite radii are treated as zero and every radius is clamped to half
    /// the shorter side.
    pub fn soft_plate_corners(&mut self, r: Rect, corners: [f32; 4], feather: f32, color: Color) {
        if r.is_empty() || color.a <= 0.0 {
            return;
        }
        let Some(path) = rounded_rect_path_corners(r, corners, self.scale) else {
            return;
        };
        self.blur_fill_path(&path, feather, color);
    }

    /// A soft drop shadow under a rounded rectangle.
    ///
    /// `blur` is the CSS `box-shadow` blur radius in logical units; internally
    /// that is a Gaussian of σ = `blur / 2`, approximated by three box passes
    /// (see `super::blur` for why that approximation and not a real kernel).
    /// `dy` offsets the shadow downward — the light is above, always, which is
    /// what makes a stack of cards read as a stack.
    ///
    /// The shadow is clipped to the canvas like everything else, so a card
    /// flush against an edge simply loses the part that fell off. Leave a
    /// margin of about `blur + dy` around the card if you want the whole halo.
    pub fn drop_shadow(&mut self, r: Rect, radius: f32, blur: f32, dy: f32, color: Color) {
        if r.is_empty() || color.a <= 0.0 {
            return;
        }
        let Some(path) = rounded_rect_path(r.offset(0.0, dy), radius, self.scale) else {
            return;
        };
        self.blur_fill_path(&path, blur, color);
    }

    /// Rasterise `path` into the reusable mask, Gaussian-blur that mask and
    /// fill it with `color`.
    ///
    /// The shared tail of [`Canvas::drop_shadow`] and
    /// [`Canvas::soft_plate_corners`]: one full-canvas blur per call, which is
    /// the expensive part of both and the reason a card counts its shadows.
    fn blur_fill_path(&mut self, path: &tiny_skia::Path, blur: f32, color: Color) {
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        // Allocate the mask once, on the first shadow this canvas ever draws.
        if self.shadow_mask.is_none() {
            self.shadow_mask = Mask::new(w, h);
        }
        let Some(mask) = self.shadow_mask.as_mut() else {
            return;
        };
        mask.clear();
        mask.fill_path(path, FillRule::Winding, true, Transform::identity());
        let sigma = if blur.is_finite() && blur > 0.0 {
            blur * self.scale / 2.0
        } else {
            0.0
        };
        blur_alpha(
            mask.data_mut(),
            w as usize,
            h as usize,
            sigma,
            &mut self.blur_scratch,
        );
        let Some(all) = tiny_skia::Rect::from_xywh(0.0, 0.0, w as f32, h as f32) else {
            return;
        };
        let paint = Paint {
            shader: tiny_skia::Shader::SolidColor(color.to_tiny()),
            anti_alias: false,
            ..Default::default()
        };
        self.pixmap
            .fill_rect(all, &paint, Transform::identity(), Some(mask));
    }

    /// A stroked arc — the gauge primitive.
    ///
    /// Angles are **degrees clockwise from 12 o'clock**, which is how a gauge
    /// is described ("sweeps 270° starting at −135°") and avoids every call
    /// site re-deriving the same `-90°` that a maths-convention API would need.
    /// A negative `sweep_deg` runs anticlockwise; `|sweep|` is clamped to 360.
    /// Round caps, because a gauge with square ends looks like a bug.
    pub fn arc(
        &mut self,
        c: Point,
        radius: f32,
        start_deg: f32,
        sweep_deg: f32,
        width: f32,
        fill: &Fill,
    ) {
        if width <= 0.0 || fill.is_invisible() {
            return;
        }
        let Some(path) = arc_path(c, radius, start_deg, sweep_deg, self.scale) else {
            return;
        };
        self.stroke(&path, fill, width, tiny_skia::LineCap::Round);
    }

    /// Draw `run` with its **top-left** at `at`, returning the bounds it
    /// occupied.
    ///
    /// Top-left rather than baseline: cards stack boxes, and a baseline origin
    /// makes every row's position depend on the font's ascent, which changes
    /// with the fallback that happened to be chosen. The returned rect is
    /// `at` plus the measured extent, so it can be fed straight back into
    /// `super::geom` for the next row.
    ///
    /// `fonts` is passed in rather than owned because building a [`FontStack`]
    /// scans the filesystem — see its docs. One stack, shared by every widget,
    /// built off the daemon loop.
    pub fn text(&mut self, fonts: &mut FontStack, run: &TextRun, at: Point) -> Rect {
        if run.text.is_empty() || run.color.a <= 0.0 || run.size <= 0.0 {
            return Rect::new(at.x, at.y, 0.0, 0.0);
        }
        let s = self.scale;
        let (ox, oy) = ((at.x * s).round() as i32, (at.y * s).round() as i32);
        let (w, h) = (self.pixmap.width() as i32, self.pixmap.height() as i32);
        let stride = self.pixmap.width() as usize;
        let pixels = self.pixmap.pixels_mut();
        let metrics = fonts.draw(run, s, |x, y, c| {
            let (px, py) = (ox + x, oy + y);
            if px < 0 || py < 0 || px >= w || py >= h {
                return;
            }
            let i = py as usize * stride + px as usize;
            pixels[i] = blend_over(pixels[i], c);
        });
        Rect::at(at, metrics.size())
    }

    /// Draw `img` into `dst`, scaled to fill it, with rounded corners.
    ///
    /// The source is resampled bilinearly and **not** aspect-corrected: pass a
    /// square crop for a square slot. `crate::artwork::prepare_source` is the
    /// existing way to get a cover down to a sensible size first, and a caller
    /// should use it — a source past [`MAX_CANVAS_PX`] on a side is refused
    /// (with a log line) rather than turned into a second huge allocation.
    pub fn image(&mut self, img: &image::RgbaImage, dst: Rect, radius: f32) {
        if dst.is_empty() {
            return;
        }
        let (sw, sh) = (img.width(), img.height());
        if sw == 0 || sh == 0 {
            return;
        }
        if sw > MAX_CANVAS_PX || sh > MAX_CANVAS_PX {
            log::warn!(
                "widgetkit: refusing to draw a {sw}x{sh} image; downscale it first \
                 (see artwork::prepare_source)"
            );
            return;
        }
        let Some(src) = rgba_to_pixmap(img) else {
            return;
        };
        let Some(path) = rounded_rect_path(dst, radius, self.scale) else {
            return;
        };
        let s = self.scale;
        // Map the source's pixel grid onto the destination rectangle. The
        // pattern's own transform is the only transform in play (the fill uses
        // the identity), so there is no chance of the two disagreeing.
        let ts = Transform::from_row(
            dst.w * s / sw as f32,
            0.0,
            0.0,
            dst.h * s / sh as f32,
            dst.x * s,
            dst.y * s,
        );
        let paint = Paint {
            shader: Pattern::new(
                src.as_ref(),
                SpreadMode::Pad,
                FilterQuality::Bilinear,
                1.0,
                ts,
            ),
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Draw `img` into `dst` as a **centre-cropped cover**: scaled to fill,
    /// aspect preserved, the overflowing axis cropped equally at both ends.
    ///
    /// [`Canvas::image`] stretches, which is correct when the caller already
    /// holds a square crop and wrong the moment it does not — and a squashed
    /// album cover looks broken in a way a cropped one does not. This is the
    /// same rule `crate::artwork::render_disc` already applies, done without
    /// allocating a cropped copy: the crop lives in the pattern's transform,
    /// and the rounded-rect path is what stops the rest of the source painting
    /// outside the slot.
    ///
    /// Run large sources through `crate::artwork::prepare_source` first — this
    /// refuses anything past [`MAX_CANVAS_PX`] on a side, exactly as
    /// [`Canvas::image`] does.
    pub fn image_cover(&mut self, img: &image::RgbaImage, dst: Rect, radius: f32) {
        if dst.is_empty() {
            return;
        }
        let (sw, sh) = (img.width(), img.height());
        if sw == 0 || sh == 0 {
            return;
        }
        if sw > MAX_CANVAS_PX || sh > MAX_CANVAS_PX {
            log::warn!(
                "widgetkit: refusing to draw a {sw}x{sh} cover; downscale it first \
                 (see artwork::prepare_source)"
            );
            return;
        }
        let Some(src) = rgba_to_pixmap(img) else {
            return;
        };
        let Some(path) = rounded_rect_path(dst, radius, self.scale) else {
            return;
        };
        let s = self.scale;
        let (dw, dh) = (dst.w * s, dst.h * s);
        let k = (dw / sw as f32).max(dh / sh as f32);
        if !k.is_finite() || k <= 0.0 {
            return;
        }
        let ts = Transform::from_row(
            k,
            0.0,
            0.0,
            k,
            dst.x * s + (dw - sw as f32 * k) / 2.0,
            dst.y * s + (dh - sh as f32 * k) / 2.0,
        );
        let paint = Paint {
            shader: Pattern::new(
                src.as_ref(),
                SpreadMode::Pad,
                FilterQuality::Bilinear,
                1.0,
                ts,
            ),
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Draw `img` into `dst` as a centre-cropped cover, **rotated** by
    /// `deg` clockwise about `dst`'s centre.
    ///
    /// The rotation lives in the pattern's transform, so it costs one matrix
    /// multiply and no buffer: there is no intermediate rotated bitmap, and
    /// nothing is allocated on a path that runs at the disc's spin rate.
    /// Sampling is inverse-mapped and bilinear by construction — tiny-skia
    /// walks the destination and asks where each pixel came from, which is
    /// what stops a rotated raster developing the lattice of unpainted holes
    /// that forward mapping always leaves.
    ///
    /// A square `dst` with `radius = dst.w / 2` gives a disc.
    pub fn image_rotated(&mut self, img: &image::RgbaImage, dst: Rect, deg: f32, radius: f32) {
        if dst.is_empty() {
            return;
        }
        let (sw, sh) = (img.width(), img.height());
        if sw == 0 || sh == 0 {
            return;
        }
        if sw > MAX_CANVAS_PX || sh > MAX_CANVAS_PX {
            log::warn!(
                "widgetkit: refusing to draw a {sw}x{sh} source; downscale it first \
                 (see artwork::prepare_source)"
            );
            return;
        }
        let Some(src) = rgba_to_pixmap(img) else {
            return;
        };
        let Some(path) = rounded_rect_path(dst, radius, self.scale) else {
            return;
        };
        let s = self.scale;
        let (dw, dh) = (dst.w * s, dst.h * s);
        // A rotating square must cover the *diagonal* of its slot, or the
        // corners sweep in and out of the artwork as it turns.
        let k = (dw / sw as f32).max(dh / sh as f32);
        if !k.is_finite() || k <= 0.0 {
            return;
        }
        let cover = Transform::from_row(
            k,
            0.0,
            0.0,
            k,
            dst.x * s + (dw - sw as f32 * k) / 2.0,
            dst.y * s + (dh - sh as f32 * k) / 2.0,
        );
        let deg = if deg.is_finite() { deg } else { 0.0 };
        let c = dst.center();
        let ts = Transform::from_rotate_at(deg, c.x * s, c.y * s).pre_concat(cover);
        let paint = Paint {
            shader: Pattern::new(
                src.as_ref(),
                SpreadMode::Pad,
                FilterQuality::Bilinear,
                1.0,
                ts,
            ),
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Clear a rounded rectangle back to full transparency, **punching a hole**
    /// through everything already drawn.
    ///
    /// The spindle hole of a record is not a black dot painted on the label —
    /// it is a hole, and on a wallpaper the difference is visible because the
    /// video shows through it. `crate::artwork::render_disc` punches the same
    /// hole out of its alpha channel directly; this is the equivalent for
    /// anything composed on a [`Canvas`].
    pub fn punch(&mut self, r: Rect, radius: f32) {
        if r.is_empty() {
            return;
        }
        let Some(path) = rounded_rect_path(r, radius, self.scale) else {
            return;
        };
        let paint = Paint {
            shader: tiny_skia::Shader::SolidColor(tiny_skia::Color::TRANSPARENT),
            blend_mode: tiny_skia::BlendMode::Clear,
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Multiply the alpha of everything already drawn inside a rounded rect by
    /// `alpha`.
    ///
    /// A widget with a global `opacity` setting has two ways to honour it:
    /// thin every colour it draws, or draw at full strength and fade the
    /// finished object. The first makes a stack of independently translucent
    /// layers, so a faded disc shows its own groove rings *through* its label.
    /// The second fades one object, which is what the setting means. This is
    /// the second.
    pub fn fade(&mut self, r: Rect, radius: f32, alpha: f32) {
        if r.is_empty() || !alpha.is_finite() || alpha >= 1.0 {
            return;
        }
        let a = alpha.max(0.0);
        let Some(path) = rounded_rect_path(r, radius, self.scale) else {
            return;
        };
        let paint = Paint {
            shader: tiny_skia::Shader::SolidColor(Color::BLACK.with_alpha(a).to_tiny()),
            blend_mode: tiny_skia::BlendMode::DestinationIn,
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // -- output -------------------------------------------------------------

    /// Write premultiplied BGRA into `out`, reusing its capacity.
    ///
    /// The zero-allocation output path for a widget repainting every frame:
    /// keep one `Vec<u8>` beside the canvas and call this into it.
    pub fn write_bgra(&self, out: &mut Vec<u8>) {
        out.clear();
        out.extend_from_slice(self.pixmap.data());
        to_bgra_in_place(out);
    }

    /// A copy of the surface as premultiplied BGRA, ready for `overlay-add`.
    pub fn to_bgra(&self) -> Bgra {
        let mut data = Vec::new();
        self.write_bgra(&mut data);
        Bgra {
            w: self.pixmap.width(),
            h: self.pixmap.height(),
            data,
        }
    }

    /// Consume the canvas and hand over its buffer as premultiplied BGRA.
    ///
    /// Cheaper than [`Canvas::to_bgra`] — the pixel `Vec` is moved and swapped
    /// in place, never copied — but it gives up the buffer reuse, so it is for
    /// one-shot renders (a preview, a test) rather than for a live widget.
    pub fn into_bgra(self) -> Bgra {
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let mut data = self.pixmap.take();
        to_bgra_in_place(&mut data);
        Bgra { w, h, data }
    }

    /// Write the surface out as a PNG. For previews and for eyeballing test
    /// output; not on any daemon path.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let path = path.as_ref();
        self.pixmap
            .save_png(path)
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    // -- internals ----------------------------------------------------------

    fn stroke(&mut self, path: &tiny_skia::Path, fill: &Fill, width: f32, cap: tiny_skia::LineCap) {
        let paint = Paint {
            shader: fill.to_shader(self.scale),
            anti_alias: true,
            ..Default::default()
        };
        let stroke = Stroke {
            width: (width * self.scale).max(f32::EPSILON),
            line_cap: cap,
            ..Default::default()
        };
        self.pixmap
            .stroke_path(path, &paint, &stroke, Transform::identity(), None);
    }
}

/// Control-point ratio that makes a cubic Bézier match a quarter circle to
/// within about 0.02% — the standard `4/3 · (√2 − 1)`.
const KAPPA: f32 = 0.552_284_8;

/// How far back along each edge a squircle's corner starts, as a multiple of
/// its radius. Apple's continuous corner runs about 1.5 × its nominal radius;
/// 1.28 is where a single cubic still tracks the superellipse closely.
pub const SQUIRCLE_SPREAD: f32 = 1.28;

/// The control-point ratio inside a squircle's corner, as a fraction of the
/// **spread** rather than of the radius.
///
/// Larger than `KAPPA`, and it has to be. The corner is entered
/// [`SQUIRCLE_SPREAD`] × `radius` back along each edge, so a circular kappa
/// over that longer span pulls the curve *away* from the corner and the shape
/// reads as an octagon — the first cut of this primitive did exactly that. The
/// apex of a single cubic sits `√2 · (4d − 3k) / 8` from the corner; setting
/// `k = 0.72 d` puts it where a circle of radius `radius` would have put it,
/// which is what keeps a squircle as *full* as the rounded rect it replaces
/// while approaching each tangent far more gradually.
pub const SQUIRCLE_KAPPA: f32 = 0.72;

/// The squircle outline in **device** pixels.
///
/// `None` for an empty rectangle. Falls back to the plain rectangle below a
/// hundredth of a pixel of radius, exactly as [`rounded_rect_path`] does.
pub(crate) fn squircle_path(r: Rect, radius: f32, scale: f32) -> Option<tiny_skia::Path> {
    let dev = r.to_tiny(scale)?;
    let rad = if radius.is_nan() {
        0.0
    } else {
        radius.clamp(0.0, r.min_side() / (2.0 * SQUIRCLE_SPREAD))
    };
    let d = rad * SQUIRCLE_SPREAD * scale;
    if d <= 0.01 {
        return PathBuilder::from_rect(dev).into();
    }
    let k = d * SQUIRCLE_KAPPA;
    let (l, t, right, b) = (dev.left(), dev.top(), dev.right(), dev.bottom());
    let mut pb = PathBuilder::new();
    pb.move_to(l + d, t);
    pb.line_to(right - d, t);
    pb.cubic_to(right - d + k, t, right, t + d - k, right, t + d);
    pb.line_to(right, b - d);
    pb.cubic_to(right, b - d + k, right - d + k, b, right - d, b);
    pb.line_to(l + d, b);
    pb.cubic_to(l + d - k, b, l, b - d + k, l, b - d);
    pb.line_to(l, t + d);
    pb.cubic_to(l, t + d - k, l + d - k, t, l + d, t);
    pb.close();
    pb.finish()
}

fn sane_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale.clamp(0.05, 16.0)
    } else {
        1.0
    }
}

/// A corner radius that actually fits `r`: never negative, never more than half
/// the shorter side. NaN becomes zero.
fn clamp_radius(radius: f32, r: Rect) -> f32 {
    if radius.is_nan() {
        return 0.0;
    }
    radius.clamp(0.0, r.min_side() / 2.0)
}

/// The rounded-rectangle outline in **device** pixels.
///
/// `None` for an empty rectangle, which tiny-skia would refuse to fill anyway.
/// Crate-visible so the geometry can be tested without a canvas.
pub(crate) fn rounded_rect_path(r: Rect, radius: f32, scale: f32) -> Option<tiny_skia::Path> {
    let dev = r.to_tiny(scale)?;
    let rad = clamp_radius(radius, r) * scale;
    if rad <= 0.01 {
        // Below a hundredth of a pixel the arc is invisible and the cubics only
        // cost precision, so this really is a plain rectangle.
        return PathBuilder::from_rect(dev).into();
    }
    let (l, t, right, b) = (dev.left(), dev.top(), dev.right(), dev.bottom());
    let k = rad * KAPPA;
    let mut pb = PathBuilder::new();
    pb.move_to(l + rad, t);
    pb.line_to(right - rad, t);
    pb.cubic_to(right - rad + k, t, right, t + rad - k, right, t + rad);
    pb.line_to(right, b - rad);
    pb.cubic_to(right, b - rad + k, right - rad + k, b, right - rad, b);
    pb.line_to(l + rad, b);
    pb.cubic_to(l + rad - k, b, l, b - rad + k, l, b - rad);
    pb.line_to(l, t + rad);
    pb.cubic_to(l, t + rad - k, l + rad - k, t, l + rad, t);
    pb.close();
    pb.finish()
}

/// The rounded-rect outline in **device** pixels with a radius **per corner**,
/// in the order top-left, top-right, bottom-right, bottom-left.
///
/// Every radius is clamped to half the shorter side, so no pair of corners on
/// one edge can ever overrun each other, and a non-finite one is treated as a
/// square corner rather than propagating NaN into the path.
pub(crate) fn rounded_rect_path_corners(
    r: Rect,
    corners: [f32; 4],
    scale: f32,
) -> Option<tiny_skia::Path> {
    let dev = r.to_tiny(scale)?;
    let lim = (dev.width().min(dev.height()) / 2.0).max(0.0);
    let fit = |v: f32| {
        if v.is_finite() {
            (v * scale).clamp(0.0, lim)
        } else {
            0.0
        }
    };
    let [tl, tr, br, bl] = [
        fit(corners[0]),
        fit(corners[1]),
        fit(corners[2]),
        fit(corners[3]),
    ];
    if tl.max(tr).max(br).max(bl) <= 0.01 {
        return PathBuilder::from_rect(dev).into();
    }
    let (l, t, right, b) = (dev.left(), dev.top(), dev.right(), dev.bottom());
    let mut pb = PathBuilder::new();
    pb.move_to(l + tl, t);
    pb.line_to(right - tr, t);
    if tr > 0.01 {
        let k = tr * KAPPA;
        pb.cubic_to(right - tr + k, t, right, t + tr - k, right, t + tr);
    }
    pb.line_to(right, b - br);
    if br > 0.01 {
        let k = br * KAPPA;
        pb.cubic_to(right, b - br + k, right - br + k, b, right - br, b);
    }
    pb.line_to(l + bl, b);
    if bl > 0.01 {
        let k = bl * KAPPA;
        pb.cubic_to(l + bl - k, b, l, b - bl + k, l, b - bl);
    }
    pb.line_to(l, t + tl);
    if tl > 0.01 {
        let k = tl * KAPPA;
        pb.cubic_to(l, t + tl - k, l + tl - k, t, l + tl, t);
    }
    pb.close();
    pb.finish()
}

/// The open arc outline in **device** pixels, `0°` at 12 o'clock and positive
/// sweeps running clockwise.
///
/// `None` when there is nothing to draw (non-finite input, zero radius, zero
/// sweep). Split into segments of at most 90°, because a single cubic's error
/// grows quickly past a quarter turn.
pub(crate) fn arc_path(
    c: Point,
    radius: f32,
    start_deg: f32,
    sweep_deg: f32,
    scale: f32,
) -> Option<tiny_skia::Path> {
    if !c.x.is_finite() || !c.y.is_finite() || !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    if !start_deg.is_finite() || !sweep_deg.is_finite() || sweep_deg == 0.0 {
        return None;
    }
    let sweep = sweep_deg.clamp(-360.0, 360.0);
    let r = radius * scale;
    let (cx, cy) = (c.x * scale, c.y * scale);
    // Screen y grows downward, so the standard parametric circle already turns
    // clockwise; the only correction needed is the quarter turn that moves 0°
    // from 3 o'clock to 12 o'clock.
    let phi0 = (start_deg - 90.0).to_radians();
    let total = sweep.to_radians();
    let segments = (sweep.abs() / 90.0).ceil().max(1.0) as usize;
    let step = total / segments as f32;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let mut pb = PathBuilder::new();
    let at = |phi: f32| (cx + r * phi.cos(), cy + r * phi.sin());
    let (sx, sy) = at(phi0);
    pb.move_to(sx, sy);
    for i in 0..segments {
        let a = phi0 + step * i as f32;
        let b = a + step;
        let (ax, ay) = at(a);
        let (bx, by) = at(b);
        pb.cubic_to(
            ax - r * k * a.sin(),
            ay + r * k * a.cos(),
            bx + r * k * b.sin(),
            by - r * k * b.cos(),
            bx,
            by,
        );
    }
    pb.finish()
}

/// Source-over of a **straight** colour onto a **premultiplied** pixel.
///
/// The one function where the two alpha conventions meet, so it is spelled out
/// rather than inlined into the glyph loop: premultiply the source, blend, then
/// re-clamp each channel to the result's alpha before it becomes bytes again.
fn blend_over(dst: PremultipliedColorU8, src: Color) -> PremultipliedColorU8 {
    let sa = src.a;
    if sa <= 0.0 {
        return dst;
    }
    let inv = 1.0 - sa;
    let d = |v: u8| f32::from(v) / 255.0;
    let out = |s: f32, dv: u8| {
        let v = s * sa + d(dv) * inv;
        (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
    };
    let a = out(1.0, dst.alpha());
    let r = out(src.r, dst.red()).min(a);
    let g = out(src.g, dst.green()).min(a);
    let b = out(src.b, dst.blue()).min(a);
    PremultipliedColorU8::from_rgba(r, g, b, a).unwrap_or(dst)
}

/// Swap R and B in place and re-assert `max(B, G, R) <= A`.
///
/// tiny-skia keeps its own premultiplication invariant, so the clamp is
/// belt-and-braces — but this is the exact byte layout mpv reads, the swap is
/// the only place a stray write could break it, and the cost is three `min`s
/// per pixel.
fn to_bgra_in_place(data: &mut [u8]) {
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2);
        let a = px[3];
        px[0] = px[0].min(a);
        px[1] = px[1].min(a);
        px[2] = px[2].min(a);
    }
}

/// Straight RGBA from `image` to premultiplied tiny-skia RGBA.
fn rgba_to_pixmap(img: &image::RgbaImage) -> Option<Pixmap> {
    let mut pm = Pixmap::new(img.width(), img.height())?;
    for (dst, src) in pm.pixels_mut().iter_mut().zip(img.pixels()) {
        let [r, g, b, a] = src.0;
        let mul = |v: u8| ((u16::from(v) * u16::from(a) + 127) / 255) as u8;
        *dst = PremultipliedColorU8::from_rgba(mul(r), mul(g), mul(b), a)
            .unwrap_or(PremultipliedColorU8::TRANSPARENT);
    }
    Some(pm)
}

#[cfg(test)]
mod tests {
    use super::super::paint::Stop;
    use super::*;

    fn canvas(w: u32, h: u32) -> Canvas {
        Canvas::new(w, h, 1.0).expect("canvas")
    }

    /// One pixel as straight-alpha `(r, g, b, a)` bytes.
    fn px(c: &Canvas, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let p = c.pixmap.pixel(x, y).expect("in bounds");
        (p.red(), p.green(), p.blue(), p.alpha())
    }

    #[test]
    fn constructors_reject_the_sizes_that_would_hurt() {
        assert!(Canvas::new(0, 10, 1.0).is_err());
        assert!(Canvas::new(10, 0, 1.0).is_err());
        assert!(Canvas::new(MAX_CANVAS_PX + 1, 10, 1.0).is_err());
        // Within the per-side cap but far past the area cap: 4096x4096.
        assert!(Canvas::new(MAX_CANVAS_PX, MAX_CANVAS_PX, 1.0).is_err());
        // A realistic full-width 4K strip is fine.
        assert!(Canvas::new(3840, 320, 2.0).is_ok());
        // Non-finite scale degrades to 1.0 rather than poisoning every path.
        for s in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            assert_eq!(canvas_scale(s), 1.0, "scale {s}");
        }
    }

    fn canvas_scale(s: f32) -> f32 {
        Canvas::new(4, 4, s).unwrap().scale()
    }

    #[test]
    fn a_per_corner_plate_squares_the_corners_it_is_told_to() {
        let mut c = canvas(80, 80);
        // Square top corners, rounded bottom ones: the shape a scrim flush with
        // a card's bottom edge needs.
        c.soft_plate_corners(
            Rect::new(10.0, 10.0, 60.0, 60.0),
            [0.0, 0.0, 20.0, 20.0],
            0.0,
            Color::BLACK,
        );
        // The top-left corner pixel is inside the plate...
        assert!(
            px(&c, 11, 11).3 > 200,
            "square corner missing: {:?}",
            px(&c, 11, 11)
        );
        // ...and the bottom-left one is outside it.
        assert!(
            px(&c, 11, 68).3 < 60,
            "round corner missing: {:?}",
            px(&c, 11, 68)
        );
        // Every degenerate radius set is a shape, not a panic and not a NaN.
        for corners in [
            [0.0, 0.0, 0.0, 0.0],
            [f32::NAN, 4.0, -3.0, f32::INFINITY],
            [1e6, 1e6, 1e6, 1e6],
        ] {
            c.soft_plate_corners(Rect::new(4.0, 4.0, 40.0, 20.0), corners, 3.0, Color::WHITE);
        }
        // And a degenerate rect draws nothing rather than faulting.
        for r in [
            Rect::ZERO,
            Rect::new(0.0, 0.0, 0.0, 10.0),
            Rect::new(f32::NAN, 0.0, 10.0, 10.0),
            Rect::new(-1e6, -1e6, 1e7, 1e7),
        ] {
            c.soft_plate_corners(r, [4.0; 4], 6.0, Color::BLACK);
        }
    }

    #[test]
    fn clamp_size_shrinks_to_the_caps_and_keeps_the_aspect() {
        assert_eq!(Canvas::clamp_size(100, 50), (100, 50));
        let (w, h) = Canvas::clamp_size(8000, 4000);
        assert!(w <= MAX_CANVAS_PX && h <= MAX_CANVAS_PX);
        assert!(w * h <= MAX_CANVAS_AREA);
        let (w, h) = Canvas::clamp_size(3840, 2160);
        assert!(w * h <= MAX_CANVAS_AREA);
        assert!((w as f32 / h as f32 - 3840.0 / 2160.0).abs() < 0.05);
        // Degenerate requests still produce a drawable surface.
        assert_eq!(Canvas::clamp_size(0, 0), (1, 1));
        let (w, h) = Canvas::clamp_size(u32::MAX, u32::MAX);
        assert!(Canvas::new(w, h, 1.0).is_ok());
    }

    #[test]
    fn logical_bounds_track_the_scale() {
        let c = Canvas::new(400, 200, 2.0).unwrap();
        assert_eq!(c.bounds(), Rect::new(0.0, 0.0, 200.0, 100.0));
        let c = Canvas::for_logical(Size::new(100.4, 50.2), 2.0).unwrap();
        assert_eq!((c.width_px(), c.height_px()), (201, 101));
    }

    #[test]
    fn a_new_canvas_is_fully_transparent_and_reset_returns_it_there() {
        let mut c = canvas(8, 8);
        assert_eq!(px(&c, 4, 4), (0, 0, 0, 0));
        c.rounded_rect(c.bounds(), 0.0, &Fill::solid(Color::WHITE));
        assert_eq!(px(&c, 4, 4), (255, 255, 255, 255));
        c.reset();
        assert_eq!(px(&c, 4, 4), (0, 0, 0, 0));
    }

    #[test]
    fn resize_only_reallocates_when_the_size_changes() {
        let mut c = canvas(16, 16);
        c.fill(&Fill::solid(Color::WHITE));
        let before = c.pixmap.data().as_ptr();
        c.resize(16, 16, 2.0).unwrap();
        assert_eq!(c.pixmap.data().as_ptr(), before, "same size reallocated");
        assert_eq!(c.scale(), 2.0);
        assert_eq!(px(&c, 8, 8), (0, 0, 0, 0), "resize must clear");
        c.resize(32, 8, 1.0).unwrap();
        assert_eq!((c.width_px(), c.height_px()), (32, 8));
        assert!(c.resize(0, 8, 1.0).is_err());
    }

    // -- geometry -----------------------------------------------------------

    #[test]
    fn rounded_rect_bounds_are_exact_at_radius_zero_and_beyond_half() {
        let r = Rect::new(2.0, 3.0, 40.0, 20.0);
        for radius in [0.0, 0.005, 1.0, 9.999, 10.0, 50.0, 1e6, f32::NAN, -4.0] {
            let p = rounded_rect_path(r, radius, 1.0).expect("path");
            let b = p.bounds();
            assert!((b.left() - 2.0).abs() < 1e-3, "radius {radius}: {b:?}");
            assert!((b.top() - 3.0).abs() < 1e-3, "radius {radius}: {b:?}");
            assert!((b.right() - 42.0).abs() < 1e-3, "radius {radius}: {b:?}");
            assert!((b.bottom() - 23.0).abs() < 1e-3, "radius {radius}: {b:?}");
        }
        // Radius 0 really is a rectangle: four points, no curves.
        let plain = rounded_rect_path(r, 0.0, 1.0).unwrap();
        assert_eq!(plain.len(), 5, "radius 0 should be a bare rect path");
        assert!(rounded_rect_path(r, 4.0, 1.0).unwrap().len() > 5);
        // Scale multiplies the whole thing.
        let big = rounded_rect_path(r, 4.0, 3.0).unwrap();
        assert!((big.bounds().right() - 126.0).abs() < 1e-3);
        // Empty rectangles have no path at all.
        assert!(rounded_rect_path(Rect::ZERO, 4.0, 1.0).is_none());
    }

    #[test]
    fn radius_beyond_half_the_short_side_gives_a_stadium_not_an_overshoot() {
        // A square at a huge radius is a circle: its bounds are still the
        // square's, and the mid-edge points sit on the boundary.
        let sq = Rect::new(0.0, 0.0, 20.0, 20.0);
        let p = rounded_rect_path(sq, 1000.0, 1.0).unwrap();
        let b = p.bounds();
        assert!((b.width() - 20.0).abs() < 1e-3 && (b.height() - 20.0).abs() < 1e-3);
        // And it is genuinely round: filling it must leave the corners empty.
        let mut c = canvas(20, 20);
        c.rounded_rect(sq, 1000.0, &Fill::solid(Color::WHITE));
        assert_eq!(px(&c, 0, 0).3, 0, "corner of a circle should be empty");
        assert_eq!(px(&c, 19, 19).3, 0);
        assert_eq!(px(&c, 10, 10).3, 255, "centre of a circle should be solid");
    }

    #[test]
    fn arc_geometry_is_clockwise_from_twelve_and_rejects_nothing_shapes() {
        // A quarter turn from 12 o'clock ends at 3 o'clock.
        let p = arc_path(Point::new(50.0, 50.0), 40.0, 0.0, 90.0, 1.0).unwrap();
        let b = p.bounds();
        assert!((b.left() - 50.0).abs() < 0.5, "{b:?}");
        assert!((b.top() - 10.0).abs() < 0.5, "{b:?}");
        assert!((b.right() - 90.0).abs() < 0.5, "{b:?}");
        assert!((b.bottom() - 50.0).abs() < 0.5, "{b:?}");
        // A full turn covers the whole circle.
        let full = arc_path(Point::new(50.0, 50.0), 40.0, 0.0, 360.0, 1.0)
            .unwrap()
            .bounds();
        assert!((full.width() - 80.0).abs() < 0.5, "{full:?}");
        // Anticlockwise from 12 o'clock reaches 9 o'clock.
        let ccw = arc_path(Point::new(50.0, 50.0), 40.0, 0.0, -90.0, 1.0)
            .unwrap()
            .bounds();
        assert!((ccw.left() - 10.0).abs() < 0.5, "{ccw:?}");
        assert!((ccw.right() - 50.0).abs() < 0.5, "{ccw:?}");
        // Degenerate input yields no path instead of a panic.
        for (r, start, sweep) in [
            (0.0, 0.0, 90.0),
            (-5.0, 0.0, 90.0),
            (40.0, 0.0, 0.0),
            (f32::NAN, 0.0, 90.0),
            (40.0, f32::NAN, 90.0),
            (40.0, 0.0, f32::NAN),
            (f32::INFINITY, 0.0, 90.0),
        ] {
            assert!(
                arc_path(Point::new(1.0, 1.0), r, start, sweep, 1.0).is_none(),
                "r={r} start={start} sweep={sweep}"
            );
        }
        // Sweeps past a full turn are clamped, not wrapped into a spiral.
        let huge = arc_path(Point::new(50.0, 50.0), 40.0, 0.0, 5000.0, 1.0)
            .unwrap()
            .bounds();
        assert!((huge.width() - 80.0).abs() < 0.5);
    }

    // -- painting -----------------------------------------------------------

    #[test]
    fn a_linear_gradient_actually_ramps_across_the_shape() {
        // The capability ASS does not have, asserted on pixels.
        let mut c = canvas(64, 8);
        let r = Rect::new(0.0, 0.0, 64.0, 8.0);
        c.rounded_rect(
            r,
            0.0,
            &Fill::linear(
                Point::new(0.0, 0.0),
                Point::new(64.0, 0.0),
                Color::BLACK,
                Color::WHITE,
            ),
        );
        let (l, m, rr) = (px(&c, 1, 4).0, px(&c, 32, 4).0, px(&c, 62, 4).0);
        assert!(l < 12, "left end should be near black, got {l}");
        assert!(
            (110..=145).contains(&m),
            "midpoint should be mid grey, got {m}"
        );
        assert!(rr > 243, "right end should be near white, got {rr}");
        // Monotone the whole way across — a stepped ramp would not be.
        let mut prev = 0u8;
        for x in 0..64 {
            let v = px(&c, x, 4).0;
            assert!(v >= prev, "gradient reversed at {x}");
            prev = v;
        }
    }

    #[test]
    fn a_radial_gradient_is_bright_at_the_centre_and_dark_at_the_rim() {
        let mut c = canvas(64, 64);
        c.rounded_rect(
            Rect::new(0.0, 0.0, 64.0, 64.0),
            0.0,
            &Fill::radial(Point::new(32.0, 32.0), 32.0, Color::WHITE, Color::BLACK),
        );
        assert!(px(&c, 32, 32).0 > 245);
        assert!(px(&c, 1, 32).0 < 20);
        // Radially symmetric.
        assert!(px(&c, 32, 10).0.abs_diff(px(&c, 10, 32).0) <= 2);
    }

    #[test]
    fn gradient_alpha_stops_fade_to_genuinely_nothing() {
        let mut c = canvas(64, 8);
        c.rounded_rect(
            Rect::new(0.0, 0.0, 64.0, 8.0),
            0.0,
            &Fill::LinearGradient {
                from: Point::new(0.0, 0.0),
                to: Point::new(64.0, 0.0),
                stops: vec![
                    Stop::new(0.0, Color::WHITE),
                    Stop::new(1.0, Color::WHITE.with_alpha(0.0)),
                ],
            },
        );
        assert!(px(&c, 0, 4).3 > 250);
        assert!(px(&c, 63, 4).3 < 6);
        // Still premultiplied at every step of the fade.
        for x in 0..64 {
            let (r, g, b, a) = px(&c, x, 4);
            assert!(r <= a && g <= a && b <= a, "x={x} {r},{g},{b},{a}");
        }
    }

    #[test]
    fn anti_aliasing_produces_partial_coverage_at_a_fractional_edge() {
        // A rect whose right edge lands mid-pixel must produce a partly covered
        // column, not a hard jump — the property "correct at fractional radii"
        // rests on.
        let mut c = canvas(16, 4);
        c.rounded_rect(
            Rect::new(0.0, 0.0, 8.5, 4.0),
            0.0,
            &Fill::solid(Color::WHITE),
        );
        assert_eq!(px(&c, 7, 2).3, 255);
        let edge = px(&c, 8, 2).3;
        assert!((100..=155).contains(&edge), "half-covered pixel was {edge}");
        assert_eq!(px(&c, 9, 2).3, 0);
    }

    #[test]
    fn hairline_stays_inside_the_rectangle() {
        let mut c = canvas(20, 20);
        let r = Rect::new(2.0, 2.0, 16.0, 16.0);
        c.hairline(r, 4.0, Color::WHITE, 2.0);
        // Nothing outside the rect.
        for x in 0..20 {
            assert_eq!(px(&c, x, 0).3, 0, "leaked above at x={x}");
            assert_eq!(px(&c, x, 19).3, 0, "leaked below at x={x}");
        }
        // The edge is drawn.
        assert!(px(&c, 10, 3).3 > 200, "top edge missing");
        // The middle is not.
        assert_eq!(px(&c, 10, 10).3, 0, "hairline filled the interior");
        // Degenerate parameters are no-ops rather than panics.
        c.hairline(Rect::ZERO, 4.0, Color::WHITE, 2.0);
        c.hairline(r, 4.0, Color::WHITE, 0.0);
        c.hairline(r, 4.0, Color::TRANSPARENT, 2.0);
        c.hairline(r, f32::NAN, Color::WHITE, 2.0);
        c.hairline(r, 4.0, Color::WHITE, 1000.0);
    }

    #[test]
    fn top_highlight_lights_only_the_top_edge_and_fades_at_the_ends() {
        let mut c = canvas(64, 32);
        let r = Rect::new(0.0, 0.0, 64.0, 32.0);
        c.top_highlight(r, 6.0, Color::WHITE, 2.0);
        let mid_top = px(&c, 32, 0).3;
        assert!(mid_top > 150, "top edge should be lit, got {mid_top}");
        // Bottom edge untouched.
        for x in 0..64 {
            assert_eq!(px(&c, x, 31).3, 0, "bottom lit at x={x}");
        }
        // Fades toward the corners.
        assert!(px(&c, 2, 0).3 < mid_top, "highlight did not fade");
    }

    #[test]
    fn drop_shadow_is_soft_offset_and_monotone() {
        let mut c = canvas(96, 96);
        let card = Rect::new(28.0, 28.0, 40.0, 40.0);
        c.drop_shadow(card, 8.0, 16.0, 6.0, Color::BLACK.with_alpha(0.8));

        let below = px(&c, 48, 74).3;
        let above = px(&c, 48, 22).3;
        assert!(
            below > above,
            "shadow is not offset downward: {below} vs {above}"
        );
        // Softness: a band of intermediate alphas, not a hard cut.
        let column: Vec<u8> = (68..90).map(|y| px(&c, 48, y).3).collect();
        assert!(
            column.iter().any(|&v| (20..220).contains(&v)),
            "shadow has no soft band: {column:?}"
        );
        // Monotone falloff away from the shape.
        for w in column.windows(2) {
            assert!(w[1] <= w[0], "shadow ringing: {column:?}");
        }
        // Premultiplied everywhere.
        for y in 0..96 {
            for x in 0..96 {
                let (r, g, b, a) = px(&c, x, y);
                assert!(r <= a && g <= a && b <= a, "({x},{y}) {r},{g},{b},{a}");
            }
        }
    }

    #[test]
    fn drop_shadow_with_no_blur_is_a_hard_shape_and_degenerate_input_is_a_no_op() {
        let mut c = canvas(32, 32);
        c.drop_shadow(Rect::new(8.0, 8.0, 16.0, 16.0), 0.0, 0.0, 0.0, Color::BLACK);
        assert_eq!(px(&c, 16, 16).3, 255);
        assert_eq!(px(&c, 1, 1).3, 0);
        c.reset();
        for (r, radius, blur, dy, col) in [
            (Rect::ZERO, 4.0, 8.0, 2.0, Color::BLACK),
            (
                Rect::new(4.0, 4.0, 8.0, 8.0),
                4.0,
                8.0,
                2.0,
                Color::TRANSPARENT,
            ),
            (
                Rect::new(4.0, 4.0, 8.0, 8.0),
                f32::NAN,
                f32::NAN,
                f32::NAN,
                Color::BLACK,
            ),
            (Rect::new(4.0, 4.0, 8.0, 8.0), 4.0, -8.0, 2.0, Color::BLACK),
            (Rect::new(4.0, 4.0, 8.0, 8.0), 4.0, 1e6, 2.0, Color::BLACK),
        ] {
            c.drop_shadow(r, radius, blur, dy, col);
        }
    }

    #[test]
    fn shadow_buffers_are_allocated_once_and_reused() {
        let mut c = canvas(64, 64);
        let r = Rect::new(16.0, 16.0, 32.0, 32.0);
        c.drop_shadow(r, 8.0, 12.0, 4.0, Color::BLACK.with_alpha(0.5));
        let mask_ptr = c.shadow_mask.as_ref().unwrap().data().as_ptr();
        let scratch_cap = c.blur_scratch.capacity();
        for _ in 0..8 {
            c.reset();
            c.drop_shadow(r, 8.0, 12.0, 4.0, Color::BLACK.with_alpha(0.5));
        }
        assert_eq!(c.shadow_mask.as_ref().unwrap().data().as_ptr(), mask_ptr);
        assert_eq!(c.blur_scratch.capacity(), scratch_cap);
    }

    #[test]
    fn arc_draws_a_ring_of_the_requested_thickness() {
        let mut c = canvas(100, 100);
        c.arc(
            Point::new(50.0, 50.0),
            40.0,
            0.0,
            360.0,
            8.0,
            &Fill::solid(Color::WHITE),
        );
        // On the ring at 12 o'clock.
        assert!(px(&c, 50, 10).3 > 200);
        // Inside the hole and outside the ring: nothing.
        assert_eq!(px(&c, 50, 50).3, 0, "arc filled its centre");
        assert_eq!(px(&c, 50, 2).3, 0, "arc spilled outside its radius");
        // A partial sweep leaves the rest of the circle empty.
        c.reset();
        c.arc(
            Point::new(50.0, 50.0),
            40.0,
            0.0,
            90.0,
            8.0,
            &Fill::solid(Color::WHITE),
        );
        assert!(px(&c, 50, 10).3 > 200, "arc start missing");
        assert_eq!(px(&c, 50, 90).3, 0, "arc drew past its sweep");
        // No-ops rather than panics.
        c.arc(
            Point::new(1.0, 1.0),
            10.0,
            0.0,
            90.0,
            0.0,
            &Fill::solid(Color::WHITE),
        );
        c.arc(
            Point::new(1.0, 1.0),
            10.0,
            0.0,
            90.0,
            4.0,
            &Fill::solid(Color::TRANSPARENT),
        );
        c.arc(
            Point::new(1.0, 1.0),
            f32::NAN,
            0.0,
            90.0,
            4.0,
            &Fill::solid(Color::WHITE),
        );
    }

    #[test]
    fn arc_takes_a_gradient_the_whole_way_round() {
        let mut c = canvas(100, 100);
        c.arc(
            Point::new(50.0, 50.0),
            40.0,
            -135.0,
            270.0,
            10.0,
            &Fill::linear(
                Point::new(10.0, 0.0),
                Point::new(90.0, 0.0),
                Color::BLACK,
                Color::WHITE,
            ),
        );
        let left = px(&c, 14, 50);
        let right = px(&c, 86, 50);
        assert!(
            left.3 > 100 && right.3 > 100,
            "arc missing: {left:?} {right:?}"
        );
        assert!(right.0 > left.0, "gradient did not follow the arc");
    }

    // -- images -------------------------------------------------------------

    #[test]
    fn image_fills_its_slot_and_respects_the_corner_radius() {
        let mut src = image::RgbaImage::new(4, 4);
        for p in src.pixels_mut() {
            *p = image::Rgba([255, 0, 0, 255]);
        }
        let mut c = canvas(32, 32);
        c.image(&src, Rect::new(0.0, 0.0, 32.0, 32.0), 16.0);
        assert_eq!(px(&c, 16, 16), (255, 0, 0, 255));
        assert_eq!(px(&c, 0, 0).3, 0, "corner should be rounded away");
        // Translucent sources stay premultiplied.
        let mut ghost = image::RgbaImage::new(2, 2);
        for p in ghost.pixels_mut() {
            *p = image::Rgba([255, 255, 255, 128]);
        }
        c.reset();
        c.image(&ghost, Rect::new(0.0, 0.0, 32.0, 32.0), 0.0);
        let (r, g, b, a) = px(&c, 16, 16);
        assert!(r <= a && g <= a && b <= a, "{r},{g},{b},{a}");
        assert!(a.abs_diff(128) <= 2);
        // Degenerate sources and slots are no-ops.
        c.image(
            &image::RgbaImage::new(0, 0),
            Rect::new(0.0, 0.0, 8.0, 8.0),
            0.0,
        );
        c.image(&src, Rect::ZERO, 0.0);
    }

    // -- output -------------------------------------------------------------

    #[test]
    fn bgra_output_is_always_premultiplied() {
        // The local copy of artwork.rs's `colour_never_exceeds_alpha_anywhere`:
        // mpv documents a violation as undefined per-VO behaviour, and the
        // R/B swap in `to_bgra_in_place` is where it could happen.
        let mut c = canvas(48, 48);
        c.drop_shadow(
            Rect::new(8.0, 8.0, 32.0, 32.0),
            10.0,
            12.0,
            4.0,
            Color::BLACK.with_alpha(0.7),
        );
        c.rounded_rect(
            Rect::new(8.0, 8.0, 32.0, 32.0),
            10.0,
            &Fill::linear(
                Point::new(8.0, 8.0),
                Point::new(40.0, 40.0),
                Color::rgba8(0x1C, 0x1C, 0x1E, 0.68),
                Color::rgba8(0xF5, 0xA6, 0x23, 0.4),
            ),
        );
        c.hairline(
            Rect::new(8.0, 8.0, 32.0, 32.0),
            10.0,
            Color::WHITE.with_alpha(0.12),
            1.0,
        );
        let bgra = c.to_bgra();
        assert_eq!(bgra.stride(), 48 * 4);
        assert_eq!(bgra.data.len(), 48 * 48 * 4);
        assert!(!bgra.is_empty());
        for (i, p) in bgra.data.chunks_exact(4).enumerate() {
            assert!(
                p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3],
                "pixel {i} is not premultiplied: bgra={p:?}"
            );
        }
    }

    #[test]
    fn bgra_really_is_b_g_r_a_and_matches_into_bgra() {
        let mut c = canvas(4, 4);
        // Opaque pure red: BGRA must be [0, 0, 255, 255].
        c.fill(&Fill::solid(Color::rgb8(255, 0, 0)));
        let copied = c.to_bgra();
        assert_eq!(&copied.data[..4], &[0, 0, 255, 255]);
        let moved = c.into_bgra();
        assert_eq!(copied, moved, "to_bgra and into_bgra must agree");
        assert_eq!((moved.w, moved.h), (4, 4));
    }

    #[test]
    fn write_bgra_reuses_its_buffer() {
        let mut c = canvas(32, 32);
        c.fill(&Fill::solid(Color::WHITE.with_alpha(0.5)));
        let mut buf = Vec::new();
        c.write_bgra(&mut buf);
        let (ptr, cap) = (buf.as_ptr(), buf.capacity());
        for _ in 0..4 {
            c.write_bgra(&mut buf);
        }
        assert_eq!(buf.as_ptr(), ptr);
        assert_eq!(buf.capacity(), cap);
        assert_eq!(buf.len(), 32 * 32 * 4);
    }

    #[test]
    fn blending_a_straight_colour_never_leaves_a_dark_fringe() {
        // The glyph-compositing bug, isolated: white ink at 50% coverage on an
        // empty canvas must be exactly (127, 127, 127, 127) premultiplied. If
        // the source were blended as though it were already premultiplied the
        // channels would come out at half that, and every letter would carry a
        // grey halo.
        let out = blend_over(
            PremultipliedColorU8::TRANSPARENT,
            Color::WHITE.with_alpha(0.5),
        );
        assert_eq!(out.alpha(), 128);
        assert_eq!(out.red(), out.alpha());
        assert_eq!(out.green(), out.alpha());
        assert_eq!(out.blue(), out.alpha());
        // Fully opaque ink replaces whatever was there.
        let over = blend_over(out, Color::rgb8(255, 0, 0));
        assert_eq!(
            (over.red(), over.green(), over.blue(), over.alpha()),
            (255, 0, 0, 255)
        );
        // Zero-alpha ink changes nothing.
        assert_eq!(blend_over(out, Color::TRANSPARENT), out);
        // Repeated half-covers converge upward and stay premultiplied.
        let mut acc = PremultipliedColorU8::TRANSPARENT;
        for _ in 0..8 {
            acc = blend_over(acc, Color::WHITE.with_alpha(0.5));
            assert!(acc.red() <= acc.alpha());
        }
        assert!(acc.alpha() > 250);
    }

    #[test]
    fn scale_for_output_matches_the_ass_coordinate_space() {
        assert_eq!(scale_for_output(1080), 1.0);
        assert_eq!(scale_for_output(2160), 2.0);
        assert!((scale_for_output(1440) - 1.333_333).abs() < 1e-4);
        // A compositor that has not brought a mode up yet must not produce a
        // zero or infinite scale.
        assert_eq!(scale_for_output(0), 1.0);
        assert!(scale_for_output(u32::MAX).is_finite());
    }

    #[test]
    fn the_same_card_at_two_scales_lands_in_the_same_place() {
        // The density guarantee: identical logical drawing, proportional pixels.
        let draw = |scale: f32| {
            let mut c = Canvas::new((64.0 * scale) as u32, (64.0 * scale) as u32, scale).unwrap();
            c.rounded_rect(
                Rect::new(8.0, 8.0, 48.0, 48.0),
                12.0,
                &Fill::solid(Color::WHITE),
            );
            c
        };
        let one = draw(1.0);
        let two = draw(2.0);
        // Corresponding points agree.
        assert_eq!(px(&one, 32, 32).3, px(&two, 64, 64).3);
        assert_eq!(px(&one, 4, 4).3, px(&two, 8, 8).3);
        assert_eq!((two.width_px(), two.height_px()), (128, 128));
        assert_eq!(one.bounds(), two.bounds());
    }
}

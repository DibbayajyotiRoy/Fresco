//! **widgetkit** — a 2D rasteriser for on-wallpaper widgets.
//!
//! # Why this exists
//!
//! Fresco's widgets (clock, lyrics, visualiser, album-art disc) are drawn as
//! **ASS subtitle overlays**, and `crate::clock` documents that substrate's
//! ceiling in its own source: ASS has **no gradients** (the card's "neon" edge
//! is 32 flat-stepped shapes faking one), no soft shadows, and `\blur` only
//! softens the drawn shape's own alpha. The look the project wants next —
//! translucent glass cards, hairline edge lighting, arc gauges, real
//! typographic hierarchy — is not reachable from there. It is not a matter of
//! trying harder.
//!
//! So we rasterise the widgets ourselves and push the pixels through mpv's
//! `overlay-add`, the path `crate::artwork` **already uses** for the album-art
//! disc. That is the whole architectural bet: the widget engine can already
//! ship a [`crate::artwork::Bgra`] to mpv, so anything that produces one is a
//! widget, today, with no new plumbing.
//!
//! ```text
//!   card code (logical units, theme tokens)
//!        │  Canvas::rounded_rect / drop_shadow / arc / text / image
//!        ▼
//!   tiny_skia::Pixmap  (premultiplied RGBA)
//!        │  Canvas::write_bgra / into_bgra   (swap R↔B, re-clamp)
//!        ▼
//!   artwork::Bgra ──► daemon::widgets::BitmapUpdate ──► mpv `overlay-add`
//! ```
//!
//! # Why not a GTK4 layer-shell surface
//!
//! Because `gtk4-layer-shell` is **not packaged in Debian or Ubuntu** — only
//! the GTK3 `libgtk-layer-shell0` is. Fresco already bundles an `mpvpaper`
//! binary for exactly this reason (see `crate::mpvpaper_command`) and doing it
//! twice is not a strategy. Everything here is pure Rust: `tiny-skia` for
//! rasterisation, `cosmic-text` for shaping and fonts. No new C library enters
//! the build, so no new packaging problem enters the release.
//!
//! # Layering
//!
//! Every module here is **pure**. Nothing opens a display, talks to mpv, links
//! GTK or reads the network; the only I/O in the whole toolkit is
//! [`Canvas::save_png`] and the system font scan in [`FontStack::system`].
//! That is what makes the interesting parts — gradient interpolation, shadow
//! falloff, premultiplication, text measurement, contrast fitting, rounded-rect
//! geometry — testable with a plain `cargo test`.
//!
//! | module     | what it owns                                            |
//! |------------|---------------------------------------------------------|
//! | `color`    | straight-alpha colour, premultiplication, WCAG contrast  |
//! | `geom`     | rectangles, alignment, a vertical stack                  |
//! | `paint`    | solid / linear / radial fills and stop interpolation     |
//! | `blur`     | the box-blur cascade behind soft shadows                 |
//! | `text`     | `TextRun`, measurement, ellipsis, system fonts, CJK      |
//! | `typo`     | the modular scale, generated tracking and leading, CJK   |
//! | `theme`    | the dark and light token sets, derived from the accent   |
//! | `canvas`   | the surface everything is drawn on, and BGRA output      |
//! | `surface`  | card, scrim, well, progress, gauge, bar array, chip      |
//! | `dotmatrix`| the 5 x 7 LED face the NOS clock sets its numerals in     |
//! | `cards`    | the widgets Fresco actually has                          |
//!
//! `theme` and `typo` are the two that must be read before anything is drawn:
//! between them they encode why a dark card cannot carry three ink levels
//! without a scrim, and why hierarchy is carried by size, weight and case
//! rather than by opacity. `docs/widget-design-spec.md` is the authority both
//! transcribe.
//!
//! # Units and density
//!
//! Cards are authored in **logical units**, where one unit is one pixel at
//! 1080p, and a [`Canvas`] converts to device pixels with its `scale`. That is
//! the same convention the ASS widgets already use — they are laid out in a
//! virtual `RES_X × RES_Y` (1920 × 1080) space that libass rescales per output
//! — so an existing size value keeps its meaning here. Use
//! [`scale_for_output`] (`output_height / 1080`) and one card definition
//! renders crisply from 1080p to 4K.
//!
//! # Allocation and the frame budget
//!
//! The clock repaints once a minute, but the visualiser runs at `VISUAL_FPS`,
//! so the toolkit is built for a caller that **keeps one [`Canvas`] per widget
//! alive across frames**:
//!
//! ```text
//!   canvas.reset();          // memset, no allocation
//!   draw_the_card(&mut canvas, &theme, &mut fonts);
//!   canvas.write_bgra(&mut buf);   // into a Vec kept beside the canvas
//! ```
//!
//! In that loop the steady-state allocation count is **zero**: the pixmap, the
//! shadow mask, the blur scratch and the shaping buffer are all allocated once
//! and reused, and [`Canvas::resize`] only reallocates when the output's pixel
//! size genuinely changes. Building a `Canvas` per frame instead would churn
//! roughly six bytes per pixel per frame, which with four bitmap widgets is
//! tens of MB/s while music plays, for nothing.
//!
//! [`FontStack`] is the other thing to build once: it scans the filesystem for
//! fonts (tens to hundreds of milliseconds) and then caches every glyph it
//! rasterises. The daemon's run loop ticks every 100 ms, so build it off the
//! loop and share one across all widgets. Check
//! [`FontStack::has_fonts`] once when you build it
//! and log if it is false — a machine with no fonts installed produces blank
//! widgets, and that is worth a line in the journal rather than a bug report.
//!
//! # Nothing here may panic
//!
//! `panic = "abort"` is set in `[profile.release]`: a panic in a widget does
//! not unwind, it kills the daemon and the user's wallpaper with it. Every
//! primitive clamps — zero sizes, NaN radii, negative sweeps, radii larger than
//! their rectangle, shadows wider than the canvas — and the only fallible
//! entry points are the constructors, which return `anyhow::Result` because a
//! large allocation genuinely can fail. This is the bar `crate::artwork` sets
//! for itself and it is not negotiable for anything on the daemon path.
//!
//! # Example
//!
//! ```no_run
//! use fresco::widgetkit::{
//!     cards::clock, Canvas, ClockData, ClockVariant, FontStack, Mode, Size, Theme,
//! };
//!
//! # fn main() -> anyhow::Result<()> {
//! // Once, off the daemon loop: the font scan is slow and the stack is also
//! // the glyph cache.
//! let mut fonts = FontStack::system();
//! let theme = Theme::for_accent(Mode::Dark, fresco::config::Accent::Blue);
//!
//! let data = ClockData {
//!     time: "09:41",
//!     // Sized from the widest string the settings can ever produce, so the
//!     // card does not resize once a second.
//!     widest_time: "00:00",
//!     weekday: "Monday",
//!     date: "28 July",
//!     secondary: "Week 31 · GMT+05:30",
//!     font_size: 64.0,
//!     variant: ClockVariant::Auto,
//!     accent_follow: true,
//!     day_fraction: 0.41,
//! };
//!
//! // Once per output size: measure, then allocate the buffer the shadow needs.
//! let size = clock::measure(&mut fonts, &theme, &data, 1.0);
//! let mut canvas = Canvas::for_logical(size.buffer(), 1.0)?;
//! // Anchor by `size.card_rect()`, never by the buffer edge — the difference
//! // is the shadow bleed, and anchoring to the buffer makes every widget drift
//! // inward as density rises.
//! let anchor_to = size.card_rect();
//!
//! // Per repaint, with no allocation at all.
//! canvas.reset();
//! clock::draw(&mut canvas, &mut fonts, &theme, &data);
//! let bgra = canvas.to_bgra(); // hand to mpv's `overlay-add`
//! # let _ = (bgra, anchor_to, Size::ZERO);
//! # Ok(())
//! # }
//! ```

pub mod blur;
pub mod canvas;
pub mod cards;
pub mod color;
pub mod dotmatrix;
pub mod geom;
pub mod paint;
pub mod surface;
pub mod text;
pub mod theme;
pub mod typo;

pub use blur::blur_alpha;
pub use canvas::{scale_for_output, Canvas, MAX_CANVAS_AREA, MAX_CANVAS_PX, REFERENCE_HEIGHT};
pub use cards::{
    ClockData, ClockVariant, DiscData, MediaData, MediaLayer, NowPlayingData, PlayState,
    VisualizerData, VisualizerVariant,
};
pub use color::{linear_to_srgb, srgb_to_linear, Color};
pub use geom::{HAlign, Point, Rect, Size, Stack, VAlign};
pub use paint::{sample_stops, Fill, Stop};
pub use surface::{BarPaint, BarStyle, ScrimSpec, ScrimZone, WidgetSize};
pub use text::{FontStack, FontSystem, TextAlign, TextMetrics, TextRun};
pub use theme::{Elevation, Metrics, Mode, Shadow, Theme};
pub use typo::{Script, Step};

#[cfg(test)]
mod samples;

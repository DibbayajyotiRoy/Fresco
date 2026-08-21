//! The four widgets Fresco actually has, as reusable renderers.
//!
//! `docs/widget-design-spec.md` §9 is the authority for every dimension.
//!
//! # The shape every renderer has
//!
//! ```text
//! measure(&mut FontStack, &Theme, &Data, scale) -> WidgetSize
//! draw   (&mut Canvas, &mut FontStack, &Theme, &Data)
//! draw_at(&mut Canvas, &mut FontStack, &Theme, &Data, card: Rect)
//! ```
//!
//! `measure` exists so the engine can size its buffer **before** it draws:
//! [`crate::widgetkit::WidgetSize::buffer`] is what to allocate and [`crate::widgetkit::WidgetSize::card_rect`] is
//! what to anchor by. `draw` places the card inside the canvas it is given;
//! `draw_at` puts it where the caller says, which is what composes several
//! widgets into one image.
//!
//! Neither allocates a surface. The canvas, the font stack and the output
//! buffer are all the caller's, reused across frames, which is the
//! `reset()` → draw → `write_bgra()` loop the toolkit is built for.
//!
//! # Nothing here reads Fresco's config types
//!
//! Every renderer takes a plain data struct. `clock.rs`, `lyrics.rs`,
//! `visualizer.rs` and `artwork.rs` compute what to show; these draw it. That
//! separation is not tidiness — it is what makes a card testable without a
//! clock, an MPRIS connection or an audio stream, and it is why the degenerate
//! cases below can be enumerated exhaustively in a unit test.
//!
//! # No controls, ever
//!
//! Fresco's widget layer has an **empty input region**: nothing drawn on it can
//! be clicked. So no play button, no transport, no slider, no knob is drawn
//! anywhere in this module, in any theme, including the skeuomorphic one where
//! the reference is 80% controls. A control that cannot be pressed is an
//! affordance that lies, and someone *will* click it. Progress bars stay
//! because a progress bar is data; the knob on one is a marker, not a handle.

pub mod clock;
pub mod disc;
pub mod media;
pub mod nos;
pub mod nowplaying;
pub mod visualizer;

pub use clock::{ClockData, ClockVariant};
pub use disc::DiscData;
pub use media::{MediaData, MediaLayer, PlayState};
pub use nowplaying::NowPlayingData;
pub use visualizer::{VisualizerData, VisualizerVariant};

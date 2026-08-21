//! Fills: solid, linear gradient, radial gradient.
//!
//! # The headline capability
//!
//! ASS has **no gradients**. `crate::clock`'s card fakes its neon edge with 32
//! separate flat-filled shapes walking the perimeter, and its own module docs
//! call the result "visibly stepped … the honest limit of the substrate". This
//! file is the answer to that: a real multi-stop gradient, interpolated per
//! pixel by tiny-skia, at any angle, with alpha in the stops so a highlight can
//! fade to nothing instead of ending in a hard edge.
//!
//! # Interpolation, and why it is straight-alpha sRGB
//!
//! [`sample_stops`] is the reference semantic and is what the unit tests pin:
//! linear interpolation of straight-alpha sRGB channels between the two
//! bracketing stops. That is deliberately the *same* rule tiny-skia's gradient
//! shader uses, so a card that computes a colour with `sample_stops` (to tint a
//! label, say) matches the pixels beside it rather than drifting a shade off.
//!
//! It is not the perceptually best rule — Oklab would ramp more evenly, and a
//! white→transparent fade in straight sRGB darkens slightly through the middle.
//! We take the match with the rasteriser over the theoretical improvement,
//! because a mismatch between a sampled colour and a drawn one is a bug a card
//! author cannot see coming, while a marginally uneven ramp is a look they can.

use super::color::Color;
use super::geom::Point;

/// One colour stop: a position along the gradient and the colour there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// Position along the ramp, clamped to `0.0..=1.0`.
    pub at: f32,
    /// Colour at that position, straight alpha.
    pub color: Color,
}

impl Stop {
    /// A stop at `at` (clamped to `0.0..=1.0`).
    pub fn new(at: f32, color: Color) -> Self {
        Self {
            at: if at.is_nan() { 0.0 } else { at.clamp(0.0, 1.0) },
            color,
        }
    }
}

/// How a shape is painted.
///
/// Coordinates in the gradient variants are **logical** units in the same space
/// as the shape being filled — `super::Canvas` converts both to device pixels
/// together, so a gradient never drifts away from its shape when `scale`
/// changes.
#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    /// One flat colour.
    Solid(Color),
    /// A ramp along the line `from → to`. Positions are fractions of that line;
    /// beyond either end the terminal stop's colour is held (pad, not repeat),
    /// because every use here is "light falls across this card" and a repeat
    /// would read as a defect.
    LinearGradient {
        /// Where stop `0.0` sits.
        from: Point,
        /// Where stop `1.0` sits.
        to: Point,
        /// At least two stops; fewer degrades to a solid. Order does not
        /// matter — they are sorted on use.
        stops: Vec<Stop>,
    },
    /// A ramp outwards from `center`, stop `0.0` at the centre and `1.0` at
    /// `radius`. Also padded past the end.
    RadialGradient {
        /// Centre of the ramp.
        center: Point,
        /// Distance at which stop `1.0` is reached.
        radius: f32,
        /// At least two stops; fewer degrades to a solid.
        stops: Vec<Stop>,
    },
}

impl Fill {
    /// A flat colour.
    pub fn solid(c: Color) -> Self {
        Self::Solid(c)
    }

    /// A two-stop linear ramp, the common case.
    pub fn linear(from: Point, to: Point, a: Color, b: Color) -> Self {
        Self::LinearGradient {
            from,
            to,
            stops: vec![Stop::new(0.0, a), Stop::new(1.0, b)],
        }
    }

    /// A two-stop radial ramp.
    pub fn radial(center: Point, radius: f32, inner: Color, outer: Color) -> Self {
        Self::RadialGradient {
            center,
            radius,
            stops: vec![Stop::new(0.0, inner), Stop::new(1.0, outer)],
        }
    }

    /// Top-to-bottom ramp across `r`. The glassmorphic surface fill: a card
    /// catches more light at its top edge than at its bottom.
    pub fn vertical(r: super::geom::Rect, top: Color, bottom: Color) -> Self {
        Self::linear(
            Point::new(r.x, r.y),
            Point::new(r.x, r.bottom()),
            top,
            bottom,
        )
    }

    /// The colour at `t` along the ramp, or the flat colour for
    /// [`Fill::Solid`]. Lets a card sample its own gradient — to tint a label
    /// to match the arc under it, for instance — and get exactly the pixel the
    /// rasteriser will draw.
    pub fn sample(&self, t: f32) -> Color {
        match self {
            Self::Solid(c) => *c,
            Self::LinearGradient { stops, .. } | Self::RadialGradient { stops, .. } => {
                sample_stops(stops, t)
            }
        }
    }

    /// True when this fill can never put a pixel on screen, so the caller can
    /// skip the path build entirely.
    pub fn is_invisible(&self) -> bool {
        match self {
            Self::Solid(c) => c.a <= 0.0,
            Self::LinearGradient { stops, .. } | Self::RadialGradient { stops, .. } => {
                stops.iter().all(|s| s.color.a <= 0.0)
            }
        }
    }

    /// Build the tiny-skia shader, converting logical coordinates to device
    /// pixels with `scale`.
    ///
    /// Degenerate geometry (a zero-length line, a zero radius, fewer than two
    /// stops) falls back to a solid fill of the first stop rather than
    /// returning nothing: a card with a bad gradient should look wrong, not
    /// disappear.
    pub(crate) fn to_shader(&self, scale: f32) -> tiny_skia::Shader<'static> {
        let flat = |c: Color| tiny_skia::Shader::SolidColor(c.to_tiny());
        match self {
            Self::Solid(c) => flat(*c),
            Self::LinearGradient { from, to, stops } => {
                let Some(ts) = tiny_stops(stops) else {
                    return flat(self.sample(0.0));
                };
                tiny_skia::LinearGradient::new(
                    dev(*from, scale),
                    dev(*to, scale),
                    ts,
                    tiny_skia::SpreadMode::Pad,
                    tiny_skia::Transform::identity(),
                )
                .unwrap_or_else(|| flat(self.sample(0.0)))
            }
            Self::RadialGradient {
                center,
                radius,
                stops,
            } => {
                let Some(ts) = tiny_stops(stops) else {
                    return flat(self.sample(0.0));
                };
                let p = dev(*center, scale);
                // tiny-skia models a two-point conical gradient; a plain
                // radial is the degenerate case where both circles share a
                // centre and the inner one has zero radius.
                tiny_skia::RadialGradient::new(
                    p,
                    0.0,
                    p,
                    (radius * scale).max(f32::EPSILON),
                    ts,
                    tiny_skia::SpreadMode::Pad,
                    tiny_skia::Transform::identity(),
                )
                .unwrap_or_else(|| flat(self.sample(0.0)))
            }
        }
    }
}

fn dev(p: Point, scale: f32) -> tiny_skia::Point {
    tiny_skia::Point::from_xy(p.x * scale, p.y * scale)
}

/// Sorted, clamped stops for tiny-skia, or `None` when there are too few to
/// make a ramp.
fn tiny_stops(stops: &[Stop]) -> Option<Vec<tiny_skia::GradientStop>> {
    let sorted = normalized(stops);
    if sorted.len() < 2 {
        return None;
    }
    Some(
        sorted
            .into_iter()
            .map(|s| tiny_skia::GradientStop::new(s.at, s.color.to_tiny()))
            .collect(),
    )
}

/// Stops sorted by position, positions already clamped by [`Stop::new`].
///
/// A stable sort, so two stops at the same position keep author order — that is
/// how a *hard* colour break is written (`[(0.5, a), (0.5, b)]`), and reordering
/// it would silently reverse the break.
fn normalized(stops: &[Stop]) -> Vec<Stop> {
    let mut v: Vec<Stop> = stops.iter().map(|s| Stop::new(s.at, s.color)).collect();
    v.sort_by(|a, b| a.at.total_cmp(&b.at));
    v
}

/// The colour at `t` in a stop list.
///
/// The reference implementation of this toolkit's gradient semantics — see the
/// module docs. `t` is clamped, stops are sorted, and the ends are held
/// (padded). An empty list is fully transparent; a single stop is that stop
/// everywhere.
pub fn sample_stops(stops: &[Stop], t: f32) -> Color {
    let v = normalized(stops);
    let Some(first) = v.first() else {
        return Color::TRANSPARENT;
    };
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    if t <= first.at {
        return first.color;
    }
    let last = v[v.len() - 1];
    if t >= last.at {
        return last.color;
    }
    // The *last* stop at or before `t`, not the first. That distinction is the
    // whole of hard-break support: `[(0.5, black), (0.5, white)]` must give
    // white at exactly 0.5, and a first-match scan gives black.
    let i = v.iter().rposition(|s| s.at <= t).unwrap_or(0);
    let a = v[i];
    if (a.at - t).abs() <= 1e-6 || i + 1 >= v.len() {
        return a.color;
    }
    let b = v[i + 1];
    let span = b.at - a.at;
    if span <= f32::EPSILON {
        return b.color;
    }
    a.color.lerp(b.color, (t - a.at) / span)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    #[test]
    fn two_stops_interpolate_linearly_including_alpha() {
        let stops = [
            Stop::new(0.0, Color::rgba(0.0, 0.0, 0.0, 0.0)),
            Stop::new(1.0, Color::rgba(1.0, 1.0, 1.0, 1.0)),
        ];
        approx(sample_stops(&stops, 0.0).r, 0.0);
        approx(sample_stops(&stops, 0.5).r, 0.5);
        approx(sample_stops(&stops, 0.5).a, 0.5);
        approx(sample_stops(&stops, 1.0).r, 1.0);
    }

    #[test]
    fn ends_are_padded_and_t_is_clamped() {
        let stops = [
            Stop::new(0.25, Color::rgb8(255, 0, 0)),
            Stop::new(0.75, Color::rgb8(0, 0, 255)),
        ];
        // Before the first stop and after the last: hold, never extrapolate.
        assert_eq!(sample_stops(&stops, 0.0), Color::rgb8(255, 0, 0));
        assert_eq!(sample_stops(&stops, 0.1), Color::rgb8(255, 0, 0));
        assert_eq!(sample_stops(&stops, 1.0), Color::rgb8(0, 0, 255));
        assert_eq!(sample_stops(&stops, 5.0), Color::rgb8(0, 0, 255));
        assert_eq!(sample_stops(&stops, -5.0), Color::rgb8(255, 0, 0));
        assert_eq!(sample_stops(&stops, f32::NAN), Color::rgb8(255, 0, 0));
        // Midway between the two stops is halfway along, not at t = 0.5 of the
        // whole ramp: 0.5 is exactly halfway between 0.25 and 0.75 here.
        approx(sample_stops(&stops, 0.5).r, 0.5);
    }

    #[test]
    fn three_stops_pick_the_right_segment() {
        let stops = [
            Stop::new(0.0, Color::rgb8(0, 0, 0)),
            Stop::new(0.5, Color::rgb8(255, 0, 0)),
            Stop::new(1.0, Color::rgb8(255, 255, 255)),
        ];
        // In the first segment green stays at zero; in the second it climbs.
        approx(sample_stops(&stops, 0.25).r, 0.5);
        approx(sample_stops(&stops, 0.25).g, 0.0);
        approx(sample_stops(&stops, 0.75).g, 0.5);
        approx(sample_stops(&stops, 0.75).r, 1.0);
    }

    #[test]
    fn unsorted_stops_are_sorted_and_out_of_range_positions_clamped() {
        let stops = [
            Stop::new(1.0, Color::WHITE),
            Stop::new(0.0, Color::BLACK),
            // Clamped into range by Stop::new, so this lands on the far end.
            Stop::new(4.0, Color::WHITE),
        ];
        assert_eq!(sample_stops(&stops, 0.0), Color::BLACK);
        approx(sample_stops(&stops, 0.5).r, 0.5);
    }

    #[test]
    fn coincident_stops_make_a_hard_break_in_author_order() {
        let stops = [
            Stop::new(0.0, Color::BLACK),
            Stop::new(0.5, Color::BLACK),
            Stop::new(0.5, Color::WHITE),
            Stop::new(1.0, Color::WHITE),
        ];
        assert_eq!(sample_stops(&stops, 0.49), Color::BLACK);
        assert_eq!(sample_stops(&stops, 0.5), Color::WHITE);
        assert_eq!(sample_stops(&stops, 0.51), Color::WHITE);
    }

    #[test]
    fn degenerate_stop_lists_do_not_panic() {
        assert_eq!(sample_stops(&[], 0.5), Color::TRANSPARENT);
        let one = [Stop::new(0.3, Color::rgb8(1, 2, 3))];
        assert_eq!(sample_stops(&one, 0.0), Color::rgb8(1, 2, 3));
        assert_eq!(sample_stops(&one, 1.0), Color::rgb8(1, 2, 3));
    }

    #[test]
    fn fill_sample_agrees_with_sample_stops_for_every_variant() {
        let a = Color::rgb8(0xF5, 0xA6, 0x23);
        let b = Color::WHITE;
        let lin = Fill::linear(Point::new(0.0, 0.0), Point::new(10.0, 0.0), a, b);
        let rad = Fill::radial(Point::new(5.0, 5.0), 5.0, a, b);
        approx(lin.sample(0.5).r, (a.r + b.r) / 2.0);
        approx(rad.sample(0.5).g, (a.g + b.g) / 2.0);
        assert_eq!(Fill::solid(a).sample(0.9), a);
    }

    #[test]
    fn invisibility_is_detected_so_the_caller_can_skip_the_path() {
        assert!(Fill::solid(Color::TRANSPARENT).is_invisible());
        assert!(!Fill::solid(Color::WHITE).is_invisible());
        assert!(Fill::linear(
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Color::TRANSPARENT,
            Color::WHITE.with_alpha(0.0)
        )
        .is_invisible());
        // One visible stop is enough to matter.
        assert!(!Fill::linear(
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Color::TRANSPARENT,
            Color::WHITE
        )
        .is_invisible());
    }

    #[test]
    fn degenerate_gradients_fall_back_to_a_solid_shader_rather_than_nothing() {
        // Zero-length line, zero radius, one stop: all must still paint.
        let p = Point::new(3.0, 3.0);
        let one_stop = Fill::LinearGradient {
            from: p,
            to: Point::new(9.0, 9.0),
            stops: vec![Stop::new(0.0, Color::WHITE)],
        };
        assert!(matches!(
            one_stop.to_shader(1.0),
            tiny_skia::Shader::SolidColor(_)
        ));
        let zero_line = Fill::linear(p, p, Color::WHITE, Color::BLACK);
        // tiny-skia rejects a zero-length gradient; we must still get a shader.
        let _ = zero_line.to_shader(1.0);
        let zero_radius = Fill::radial(p, 0.0, Color::WHITE, Color::BLACK);
        let _ = zero_radius.to_shader(2.0);
    }
}

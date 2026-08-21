//! Rectangles, alignment and the smallest useful stack.
//!
//! # Deliberately not a layout engine
//!
//! Everything here is arithmetic on one rectangle. There is no constraint
//! solver, no dirty tracking, no widget tree — a Fresco card is a fixed
//! composition (a title, a value, a gauge, a row of labels), and every attempt
//! to make that generic buys a framework nobody asked for. What card code
//! actually needs is: shrink a rect by its padding, cut a strip off one edge,
//! centre a measured text box inside a slot, and walk down a column. That is
//! this file.
//!
//! # Units
//!
//! Every number is a **logical** unit. One logical unit is one pixel at
//! `scale = 1.0`, which by convention is 1080p — the same convention
//! `crate::lyrics`' `PLAY_RES_X`/`PLAY_RES_Y` space uses for the ASS widgets,
//! so a size that looks right in an ASS theme looks right here. `super::Canvas`
//! multiplies by its `scale` at the moment it builds a path, and nothing above
//! that line ever sees a device pixel. See `super`'s module docs for why the
//! conversion is explicit rather than a transform handed to the rasteriser.

/// A point in logical units. `+y` is **down**, as in every raster coordinate
/// system this code touches (mpv's overlay, tiny-skia, `crate::artwork`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Horizontal position, increasing rightwards.
    pub x: f32,
    /// Vertical position, increasing **downwards**.
    pub y: f32,
}

impl Point {
    /// A point at `(x, y)`.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Translated by `(dx, dy)`.
    pub const fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
}

/// A width/height pair in logical units. Negative extents are clamped to zero
/// on construction, so a measurement mistake yields an empty box rather than a
/// rectangle whose `right` is left of its `left`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    /// Width, never negative.
    pub w: f32,
    /// Height, never negative.
    pub h: f32,
}

impl Size {
    /// A size, with negative and NaN extents clamped to zero.
    pub fn new(w: f32, h: f32) -> Self {
        Self {
            w: non_negative(w),
            h: non_negative(h),
        }
    }

    /// Nothing at all.
    pub const ZERO: Self = Self { w: 0.0, h: 0.0 };

    /// True when either extent is zero, i.e. nothing can be drawn in it.
    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
}

/// Horizontal placement of a child inside a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    /// Flush with the slot's left edge.
    #[default]
    Left,
    /// Centred horizontally.
    Center,
    /// Flush with the slot's right edge.
    Right,
}

/// Vertical placement of a child inside a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    /// Flush with the slot's top edge.
    #[default]
    Top,
    /// Centred vertically.
    Middle,
    /// Flush with the slot's bottom edge.
    Bottom,
}

/// An axis-aligned rectangle in logical units, stored as origin + extent.
///
/// Origin/extent rather than left/top/right/bottom because every operation a
/// card performs (inset, split, align) is naturally expressed on extents, and
/// the alternative form makes it easy to produce an inverted rectangle by
/// accident. Extents here are never negative: [`Rect::new`] clamps, and every
/// method below is written to keep that true.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width, never negative.
    pub w: f32,
    /// Height, never negative.
    pub h: f32,
}

impl Rect {
    /// An empty rectangle at the origin.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };

    /// From an origin and an extent. Negative or NaN extents clamp to zero.
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x: finite(x),
            y: finite(y),
            w: non_negative(w),
            h: non_negative(h),
        }
    }

    /// From two edges per axis, in either order — `ltrb(10, 0, 0, 10)` is the
    /// same rectangle as `ltrb(0, 0, 10, 10)`.
    pub fn ltrb(l: f32, t: f32, r: f32, b: f32) -> Self {
        let (l, r) = if l <= r { (l, r) } else { (r, l) };
        let (t, b) = if t <= b { (t, b) } else { (b, t) };
        Self::new(l, t, r - l, b - t)
    }

    /// A rectangle of `size` with its top-left at `at`.
    pub fn at(at: Point, size: Size) -> Self {
        Self::new(at.x, at.y, size.w, size.h)
    }

    /// Left edge.
    pub fn left(self) -> f32 {
        self.x
    }
    /// Top edge.
    pub fn top(self) -> f32 {
        self.y
    }
    /// Right edge.
    pub fn right(self) -> f32 {
        self.x + self.w
    }
    /// Bottom edge.
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }
    /// Extent.
    pub fn size(self) -> Size {
        Size::new(self.w, self.h)
    }
    /// Top-left corner.
    pub fn origin(self) -> Point {
        Point::new(self.x, self.y)
    }
    /// Geometric centre.
    pub fn center(self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    /// True when nothing can be drawn inside.
    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
    /// The shorter side. The natural ceiling for a corner radius.
    pub fn min_side(self) -> f32 {
        self.w.min(self.h)
    }

    /// Translated by `(dx, dy)`.
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Shrunk by `d` on all four sides — a card's padding, in one call.
    /// Over-insetting collapses to an empty rect *centred in the original*,
    /// rather than inverting, so a too-large padding produces "nothing drawn"
    /// instead of a shape that spills outside its parent.
    pub fn inset(self, d: f32) -> Self {
        self.inset_ltrb(d, d, d, d)
    }

    /// Shrunk by `dx` left/right and `dy` top/bottom.
    pub fn inset_xy(self, dx: f32, dy: f32) -> Self {
        self.inset_ltrb(dx, dy, dx, dy)
    }

    /// Shrunk per side. Negative values grow the rectangle, which is how an
    /// outer glow or a bleed is expressed.
    pub fn inset_ltrb(self, l: f32, t: f32, r: f32, b: f32) -> Self {
        let w = self.w - l - r;
        let h = self.h - t - b;
        if w < 0.0 || h < 0.0 {
            // Collapse about the centre of what is left, so the empty rect is
            // still in a sensible place if a caller ignores `is_empty`.
            let c = self.center();
            return Self::new(
                c.x - w.max(0.0) / 2.0,
                c.y - h.max(0.0) / 2.0,
                w.max(0.0),
                h.max(0.0),
            );
        }
        Self::new(self.x + l, self.y + t, w, h)
    }

    /// Cut a strip `at` units wide off the **left**, returning `(strip, rest)`.
    /// `at` is clamped to the rectangle, so an over-long cut yields the whole
    /// rectangle and an empty remainder.
    pub fn split_h(self, at: f32) -> (Self, Self) {
        let at = clamp_span(at, self.w);
        (
            Self::new(self.x, self.y, at, self.h),
            Self::new(self.x + at, self.y, self.w - at, self.h),
        )
    }

    /// Cut a strip `at` units tall off the **top**, returning `(strip, rest)`.
    pub fn split_v(self, at: f32) -> (Self, Self) {
        let at = clamp_span(at, self.h);
        (
            Self::new(self.x, self.y, self.w, at),
            Self::new(self.x, self.y + at, self.w, self.h - at),
        )
    }

    /// `n` equal columns with `gap` between them. `n == 0` yields nothing;
    /// gaps that do not fit collapse the columns to zero width rather than
    /// producing negative ones.
    pub fn cols(self, n: usize, gap: f32) -> Vec<Self> {
        let Some(step) = even_step(n, self.w, gap) else {
            return Vec::new();
        };
        (0..n)
            .map(|i| Self::new(self.x + (step + gap) * i as f32, self.y, step, self.h))
            .collect()
    }

    /// `n` equal rows with `gap` between them. See [`Rect::cols`].
    pub fn rows(self, n: usize, gap: f32) -> Vec<Self> {
        let Some(step) = even_step(n, self.h, gap) else {
            return Vec::new();
        };
        (0..n)
            .map(|i| Self::new(self.x, self.y + (step + gap) * i as f32, self.w, step))
            .collect()
    }

    /// Place a child of `size` inside `self`.
    ///
    /// The returned rect keeps the *child's* size even when it is bigger than
    /// the slot — clipping is the canvas's job, and silently shrinking a
    /// measured text box here would misalign the baseline it was measured for.
    pub fn align(self, size: Size, h: HAlign, v: VAlign) -> Self {
        let x = match h {
            HAlign::Left => self.x,
            HAlign::Center => self.x + (self.w - size.w) / 2.0,
            HAlign::Right => self.right() - size.w,
        };
        let y = match v {
            VAlign::Top => self.y,
            VAlign::Middle => self.y + (self.h - size.h) / 2.0,
            VAlign::Bottom => self.bottom() - size.h,
        };
        Self::new(x, y, size.w, size.h)
    }

    /// True when `p` is inside, half-open on the right and bottom edges so
    /// tiling rectangles do not both claim a boundary point.
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// The smallest rectangle containing both. An empty operand is ignored
    /// rather than dragging the result to the origin.
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        Self::ltrb(
            self.left().min(other.left()),
            self.top().min(other.top()),
            self.right().max(other.right()),
            self.bottom().max(other.bottom()),
        )
    }

    /// The overlap, or an empty rect at the origin when they do not overlap.
    pub fn intersect(self, other: Self) -> Self {
        let l = self.left().max(other.left());
        let t = self.top().max(other.top());
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        if r <= l || b <= t {
            Self::ZERO
        } else {
            Self::ltrb(l, t, r, b)
        }
    }

    /// As tiny-skia's rect, already converted to device pixels by `scale`.
    ///
    /// `None` for an empty rectangle. tiny-skia's own constructor *accepts* a
    /// zero-extent rect, and then every path built from it has degenerate
    /// bounds that the rasteriser silently declines to fill — so the check is
    /// here, where the caller can see it, rather than three layers down.
    pub(crate) fn to_tiny(self, scale: f32) -> Option<tiny_skia::Rect> {
        if self.is_empty() {
            return None;
        }
        tiny_skia::Rect::from_xywh(
            self.x * scale,
            self.y * scale,
            self.w * scale,
            self.h * scale,
        )
    }
}

/// A top-to-bottom cursor down a column, the one layout aid card code actually
/// repeats.
///
/// Cards are a stack of measured rows — a label, a big value, a gauge, a
/// footnote — where each row's height is only known after the text is measured.
/// `Stack` keeps the running `y` so a card never hand-sums heights and gaps,
/// which is exactly the arithmetic that goes wrong when a row is added.
///
/// The stack never refuses to advance past its area: it keeps handing out rects
/// below the bottom edge and lets [`Stack::overflowed`] report it. A card that
/// silently stopped drawing its last row would be much harder to diagnose than
/// one that visibly runs off its own card.
#[derive(Debug, Clone)]
pub struct Stack {
    area: Rect,
    gap: f32,
    y: f32,
    started: bool,
}

impl Stack {
    /// A stack filling `area`, inserting `gap` between consecutive rows (never
    /// before the first).
    pub fn new(area: Rect, gap: f32) -> Self {
        Self {
            area,
            gap: non_negative(gap),
            y: area.y,
            started: false,
        }
    }

    /// Take the next `h`-tall, full-width row.
    pub fn push(&mut self, h: f32) -> Rect {
        if self.started {
            self.y += self.gap;
        }
        self.started = true;
        let r = Rect::new(self.area.x, self.y, self.area.w, non_negative(h));
        self.y += r.h;
        r
    }

    /// Take the next row and align a child of `size` inside it. The row is as
    /// tall as the child, which is the normal case for text.
    pub fn push_aligned(&mut self, size: Size, h: HAlign) -> Rect {
        self.push(size.h).align(size, h, VAlign::Top)
    }

    /// Advance without producing a row — extra breathing space between groups
    /// that is not the uniform `gap`.
    pub fn skip(&mut self, h: f32) {
        self.y += non_negative(h);
    }

    /// Everything not yet consumed, from the current cursor to the bottom of
    /// the area. Empty once the stack has overflowed.
    pub fn remaining(&self) -> Rect {
        Rect::ltrb(
            self.area.x,
            self.y.min(self.area.bottom()),
            self.area.right(),
            self.area.bottom(),
        )
    }

    /// Total height handed out so far, gaps included.
    pub fn used(&self) -> f32 {
        self.y - self.area.y
    }

    /// True when the rows handed out no longer fit the area — the signal to
    /// drop a row, shrink a size, or ellipsise.
    pub fn overflowed(&self) -> bool {
        self.y > self.area.bottom() + 1e-3
    }
}

fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

fn non_negative(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        0.0
    }
}

fn clamp_span(at: f32, span: f32) -> f32 {
    if at.is_nan() {
        0.0
    } else {
        at.clamp(0.0, span)
    }
}

fn even_step(n: usize, span: f32, gap: f32) -> Option<f32> {
    if n == 0 {
        return None;
    }
    let gaps = gap.max(0.0) * (n - 1) as f32;
    Some(((span - gaps) / n as f32).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    #[test]
    fn construction_never_yields_an_inverted_rectangle() {
        let r = Rect::new(10.0, 10.0, -5.0, -5.0);
        assert!(r.is_empty());
        assert!(r.right() >= r.left() && r.bottom() >= r.top());
        // ltrb accepts its edges in either order.
        assert_eq!(
            Rect::ltrb(10.0, 10.0, 0.0, 0.0),
            Rect::ltrb(0.0, 0.0, 10.0, 10.0)
        );
        // Non-finite input degrades to zero rather than poisoning later maths.
        assert!(Rect::new(f32::NAN, 0.0, f32::INFINITY, 1.0).x.is_finite());
    }

    #[test]
    fn inset_shrinks_and_over_inset_collapses_instead_of_inverting() {
        let r = Rect::new(0.0, 0.0, 100.0, 60.0);
        assert_eq!(r.inset(10.0), Rect::new(10.0, 10.0, 80.0, 40.0));
        assert_eq!(r.inset_xy(10.0, 5.0), Rect::new(10.0, 5.0, 80.0, 50.0));
        assert_eq!(
            r.inset_ltrb(1.0, 2.0, 3.0, 4.0),
            Rect::new(1.0, 2.0, 96.0, 54.0)
        );
        // Negative inset grows (used for glows and bleeds).
        assert_eq!(r.inset(-5.0), Rect::new(-5.0, -5.0, 110.0, 70.0));
        // Padding bigger than the box: empty, still inside the parent.
        let dead = r.inset(80.0);
        assert!(dead.is_empty());
        approx(dead.center().x, r.center().x);
    }

    #[test]
    fn splits_partition_exactly_and_clamp_at_the_edges() {
        let r = Rect::new(4.0, 8.0, 100.0, 60.0);
        let (a, b) = r.split_h(30.0);
        approx(a.w + b.w, r.w);
        approx(a.right(), b.left());
        let (t, u) = r.split_v(25.0);
        approx(t.h + u.h, r.h);
        approx(t.bottom(), u.top());
        // Over-long cut: everything on one side, nothing negative on the other.
        let (all, none) = r.split_h(1000.0);
        assert_eq!(all, r);
        assert!(none.is_empty());
        let (none2, all2) = r.split_h(-5.0);
        assert!(none2.is_empty());
        assert_eq!(all2, r);
    }

    #[test]
    fn cols_and_rows_are_even_and_respect_gaps() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let c = r.cols(3, 5.0);
        assert_eq!(c.len(), 3);
        approx(c[0].w, 30.0);
        approx(c[1].x, 35.0);
        approx(c[2].right(), 100.0);
        let rows = r.rows(2, 10.0);
        approx(rows[0].h, 20.0);
        approx(rows[1].y, 30.0);
        assert!(r.cols(0, 0.0).is_empty());
        // Gaps that cannot fit collapse the cells, never invert them.
        for cell in r.cols(4, 100.0) {
            assert!(cell.w >= 0.0);
        }
    }

    #[test]
    fn align_places_a_measured_box_in_every_corner() {
        let slot = Rect::new(0.0, 0.0, 100.0, 100.0);
        let s = Size::new(20.0, 10.0);
        assert_eq!(
            slot.align(s, HAlign::Left, VAlign::Top).origin(),
            Point::new(0.0, 0.0)
        );
        assert_eq!(
            slot.align(s, HAlign::Right, VAlign::Bottom).origin(),
            Point::new(80.0, 90.0)
        );
        let c = slot.align(s, HAlign::Center, VAlign::Middle);
        approx(c.x, 40.0);
        approx(c.y, 45.0);
        // A child bigger than its slot keeps its own size — clipping is the
        // canvas's job, not the layout's.
        let big = slot.align(Size::new(200.0, 5.0), HAlign::Center, VAlign::Top);
        approx(big.w, 200.0);
        approx(big.x, -50.0);
    }

    #[test]
    fn union_and_intersect_handle_the_empty_cases() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert_eq!(a.union(b), Rect::ltrb(0.0, 0.0, 30.0, 30.0));
        assert_eq!(a.union(Rect::ZERO), a);
        assert_eq!(Rect::ZERO.union(a), a);
        assert!(a.intersect(b).is_empty());
        assert_eq!(
            a.intersect(Rect::new(5.0, 5.0, 20.0, 20.0)),
            Rect::ltrb(5.0, 5.0, 10.0, 10.0)
        );
    }

    #[test]
    fn contains_is_half_open_so_tiles_do_not_overlap() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(Point::new(0.0, 0.0)));
        assert!(!r.contains(Point::new(10.0, 5.0)));
        assert!(!r.contains(Point::new(5.0, 10.0)));
    }

    #[test]
    fn stack_walks_down_with_gaps_only_between_rows() {
        let mut s = Stack::new(Rect::new(0.0, 0.0, 100.0, 100.0), 8.0);
        let a = s.push(20.0);
        let b = s.push(20.0);
        approx(a.y, 0.0);
        approx(b.y, 28.0);
        approx(s.used(), 48.0);
        assert!(!s.overflowed());
        approx(s.remaining().h, 52.0);
        s.skip(10.0);
        approx(s.used(), 58.0);
        // Overflow is reported, not hidden.
        s.push(80.0);
        assert!(s.overflowed());
        assert!(s.remaining().is_empty());
    }

    #[test]
    fn stack_aligns_a_measured_child_inside_its_row() {
        let mut s = Stack::new(Rect::new(0.0, 0.0, 100.0, 100.0), 0.0);
        let r = s.push_aligned(Size::new(40.0, 12.0), HAlign::Right);
        approx(r.right(), 100.0);
        approx(r.h, 12.0);
        approx(s.used(), 12.0);
    }

    #[test]
    fn to_tiny_applies_scale_and_rejects_empty() {
        let r = Rect::new(1.0, 2.0, 10.0, 20.0);
        let t = r.to_tiny(2.0).unwrap();
        approx(t.x(), 2.0);
        approx(t.width(), 20.0);
        assert!(Rect::ZERO.to_tiny(1.0).is_none());
    }
}

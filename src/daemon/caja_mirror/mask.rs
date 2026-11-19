//! Which pixels of the desktop window the mirror shows: the pure part.
//!
//! DDE's desktop buffer is *opaque* (the key PNG has alpha 255), so everything
//! it drew on top of the key colour — anti-aliased icon edges, drop shadows,
//! label halos — has already been blended toward `#010101`. Matching "exactly
//! the key" or "within a couple of units of it" therefore goes wrong in both
//! directions:
//!
//! * a dark pixel that is *part of* an icon (the black screen of a terminal
//!   icon, an eye, a shadowed corner) is within tolerance of the key and
//!   becomes a hole;
//! * a dark blend pixel on the *outside* of an icon survives the test, and
//!   shows as a dark fringe on the video.
//!
//! [`refine_classes`] repairs the first with an enclosed-hole fill and the
//! second with a one-pixel peel. It works on a per-pixel *class* rather than
//! on RGB so the mirror can keep one byte per pixel and re-run it over just
//! the part of the screen that changed ([`refine_area`]).

/// A pixel within tolerance of the key colour: transparent unless enclosed.
pub(super) const CLASS_KEY: u8 = 0;
/// Not the key, but nearly black — a blend of a dark icon edge or a shadow with
/// the key. Kept when it is inside an icon, peeled when it borders the key.
pub(super) const CLASS_DARK: u8 = 1;
/// Everything else: real icon, label and highlight pixels.
pub(super) const CLASS_OTHER: u8 = 2;

/// A non-key pixel whose brightest channel is below this is [`CLASS_DARK`].
const DARK_MAX: u8 = 40;
/// A group of connected non-key pixels gets its enclosed key-coloured holes
/// filled only when its bounding box is at least this many pixels on both
/// sides. That is an icon body, not a letter: the counter of an "o" in a label
/// must stay see-through.
const ICON_MIN_SIDE: usize = 32;
/// ... and at most this on the longer side, so a big outline (a rubber band, a
/// frame around the whole desktop) never turns its interior into a dark slab.
const ICON_MAX_SIDE: usize = 384;
/// How far a change can matter: an icon (and so a hole it encloses) is at most
/// [`ICON_MAX_SIDE`] wide, plus a pixel each for the peel and for the bounding
/// box edge.
const SPAN: usize = ICON_MAX_SIDE + 2;

/// Classify one pixel against the key.
pub(super) fn classify(rgb: [u8; 3], key: [u8; 3], tolerance: u8) -> u8 {
    if rgb
        .iter()
        .zip(key)
        .all(|(&c, k)| c.abs_diff(k) <= tolerance)
    {
        CLASS_KEY
    } else if rgb.iter().all(|&c| c < DARK_MAX) {
        CLASS_DARK
    } else {
        CLASS_OTHER
    }
}

/// A rectangle in pixel indices (not X coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Area {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

/// The visible-pixel mask of a `w × h` bitmap of tightly packed 8-bit RGB
/// (`stride` bytes per row, at least `3 * w`), for the Deepin flavour: pixels
/// within `tolerance` of `key` are transparent, except key-coloured holes an
/// icon encloses; a one-pixel dark fringe against the key is peeled; dark
/// pixels inside an icon stay.
///
/// The live mirror classifies as it reads and calls [`refine_area`] instead;
/// this is the same thing on a plain bitmap, which is what the tests use.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn refine_mask(
    rgb: &[u8],
    stride: usize,
    w: usize,
    h: usize,
    key: [u8; 3],
    tolerance: u8,
) -> Vec<bool> {
    let mut classes = Vec::with_capacity(w * h);
    for y in 0..h {
        let row = rgb.get(y * stride..y * stride + w * 3).unwrap_or(&[]);
        classes.extend(
            row.chunks_exact(3)
                .map(|p| classify([p[0], p[1], p[2]], key, tolerance)),
        );
    }
    classes.resize(w * h, CLASS_KEY);
    refine_classes(&classes, w, h)
}

/// [`refine_mask`] on an already classified `w × h` grid (`classes.len() ==
/// w * h`).
pub(super) fn refine_classes(classes: &[u8], w: usize, h: usize) -> Vec<bool> {
    debug_assert_eq!(classes.len(), w * h);
    let mut filled = vec![false; w * h];
    fill_enclosed_holes(classes, w, h, &mut filled);

    let mut mask: Vec<bool> = classes
        .iter()
        .zip(&filled)
        .map(|(&c, &f)| c != CLASS_KEY || f)
        .collect();
    // A key pixel still transparent after the fill. The peel reads this, never
    // the mask it is writing, so it removes exactly one pixel and cannot eat
    // its way into the icon.
    let open_key = |i: usize| classes[i] == CLASS_KEY && !filled[i];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if classes[i] != CLASS_DARK {
                continue;
            }
            let touches_key = (x > 0 && open_key(i - 1))
                || (x + 1 < w && open_key(i + 1))
                || (y > 0 && open_key(i - w))
                || (y + 1 < h && open_key(i + w));
            if touches_key {
                mask[i] = false;
            }
        }
    }
    mask
}

/// Mark the key-coloured pixels an icon-sized group of connected non-key pixels
/// encloses. Groups are 8-connected and holes 4-connected, so a diagonal gap
/// in an outline neither lets a hole leak nor seals a pocket.
fn fill_enclosed_holes(classes: &[u8], w: usize, h: usize, filled: &mut [bool]) {
    let mut seen = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..w * h {
        if classes[start] == CLASS_KEY || seen[start] {
            continue;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        seen[start] = true;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let j = ny * w + nx;
                    if !seen[j] && classes[j] != CLASS_KEY {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
        if bw.min(bh) < ICON_MIN_SIDE || bw.max(bh) > ICON_MAX_SIDE {
            continue;
        }
        fill_box(
            classes,
            w,
            Area {
                x: x0,
                y: y0,
                w: bw,
                h: bh,
            },
            filled,
        );
    }
}

/// Within `bb`, flood the key colour inward from the box's border; whatever
/// key-coloured pixel the flood does not reach is enclosed.
fn fill_box(classes: &[u8], w: usize, bb: Area, filled: &mut [bool]) {
    let idx = |lx: usize, ly: usize| (bb.y + ly) * w + bb.x + lx;
    let mut reached = vec![false; bb.w * bb.h];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut seed = |lx: usize, ly: usize, reached: &mut Vec<bool>| {
        if classes[idx(lx, ly)] == CLASS_KEY && !reached[ly * bb.w + lx] {
            reached[ly * bb.w + lx] = true;
            stack.push((lx, ly));
        }
    };
    for lx in 0..bb.w {
        seed(lx, 0, &mut reached);
        seed(lx, bb.h - 1, &mut reached);
    }
    for ly in 0..bb.h {
        seed(0, ly, &mut reached);
        seed(bb.w - 1, ly, &mut reached);
    }
    while let Some((lx, ly)) = stack.pop() {
        let mut visit = |nx: usize, ny: usize, reached: &mut Vec<bool>| {
            if classes[idx(nx, ny)] == CLASS_KEY && !reached[ny * bb.w + nx] {
                reached[ny * bb.w + nx] = true;
                stack.push((nx, ny));
            }
        };
        if lx > 0 {
            visit(lx - 1, ly, &mut reached);
        }
        if lx + 1 < bb.w {
            visit(lx + 1, ly, &mut reached);
        }
        if ly > 0 {
            visit(lx, ly - 1, &mut reached);
        }
        if ly + 1 < bb.h {
            visit(lx, ly + 1, &mut reached);
        }
    }
    for ly in 0..bb.h {
        for lx in 0..bb.w {
            if classes[idx(lx, ly)] == CLASS_KEY && !reached[ly * bb.w + lx] {
                filled[idx(lx, ly)] = true;
            }
        }
    }
}

/// Recompute the mask after the classes inside `dirty` changed, from the whole
/// parent's cached `classes` (`width × height`). Returns the area whose mask
/// is now known exactly, and that mask, row-major; the caller writes it back.
///
/// A change at one pixel can alter the mask anywhere within one icon of it: a
/// stroke that closes an outline turns the pocket it closes into a filled hole,
/// and that pocket may be [`ICON_MAX_SIDE`] away. So the area returned is
/// `dirty` grown by [`SPAN`] on every side, and the refine behind it runs on
/// `SPAN` more beyond that. Any icon that matters for a pixel of the returned
/// area then lies wholly inside what was refined, and a group of connected
/// pixels that is cut by its edge was longer than any icon to begin with, so it
/// is skipped by the size rule either way. The result is what a refine of the
/// whole parent gives there — including when `dirty` lies entirely inside an
/// enclosed hole, where nothing in sight says it is one.
pub(super) fn refine_area(
    classes: &[u8],
    width: usize,
    height: usize,
    dirty: Area,
) -> (Area, Vec<bool>) {
    let grow = |by: usize| {
        let x0 = dirty.x.saturating_sub(by);
        let y0 = dirty.y.saturating_sub(by);
        let x1 = (dirty.x + dirty.w + by).min(width);
        let y1 = (dirty.y + dirty.h + by).min(height);
        Area {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    };
    let (keep, work) = (grow(SPAN), grow(2 * SPAN));
    let mut sub = Vec::with_capacity(work.w * work.h);
    for y in work.y..work.y + work.h {
        sub.extend_from_slice(&classes[y * width + work.x..y * width + work.x + work.w]);
    }
    let refined = refine_classes(&sub, work.w, work.h);
    let mut out = Vec::with_capacity(keep.w * keep.h);
    for y in keep.y..keep.y + keep.h {
        let at = (y - work.y) * work.w + (keep.x - work.x);
        out.extend_from_slice(&refined[at..at + keep.w]);
    }
    (keep, out)
}

/// Xfce: the visible pixels of a `w × h` tightly packed RGB grid, written to
/// `mask`. Exact key test, as Caja's. Then every run of identical non-key
/// pixels longer than [`ICON_MAX_SIDE`], along a row or down a column, is a
/// rubber band's fill (or its border) and is taken back out.
///
/// Why runs, not [`classify`]: a band is painted over the key, so its colour is
/// whatever the selection colour blends to. It is not reliably "dark", and a
/// blue tint is not a class at all. What a band does have that no icon has is a
/// flat run far longer than any icon. Icons and labels are at most
/// `ICON_MAX_SIDE` across, and text never repeats one colour that long.
///
/// A flat band with an icon in it is still split only where the icon is: every
/// band pixel lies on a row or a column that the icon does not cross, and so is
/// in a full-length run. A band smaller than `ICON_MAX_SIDE` in both directions
/// is not caught; that is the limit.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn band_mask(rgb: &[u8], w: usize, h: usize, key: [u8; 3], mask: &mut [bool]) {
    let mut flags = vec![0u8; w * h];
    band_update(rgb, w, h, key, &mut flags, mask, Area { x: 0, y: 0, w, h });
}

/// Bit in a band flag: the pixel lies in a long run of one non-key colour along
/// its row, and so is band fill.
const RUN_ROW: u8 = 1;
/// The same, along its column.
const RUN_COL: u8 = 2;

/// Recompute, from the RGB grid `rgb` (`w × h`, tightly packed), the band flags
/// and visible mask of only the rows and columns that cross `dirty`.
///
/// `flags` (one byte per pixel, [`RUN_ROW`] / [`RUN_COL`]) and `mask` must be the
/// state left by the previous call over the same grid. A run can only change if
/// one of its pixels changed, and every changed pixel lies on a dirty row and a
/// dirty column, so this is exact whenever `dirty` covers every changed pixel:
/// rows outside `dirty` keep their row flags, columns outside it keep their
/// column flags, and the mask of a pixel outside both is unchanged. Returns
/// whether any mask bit changed.
///
/// The row pass reads the full row and the column pass the full column, so a
/// damage rect of `d` pixels a side costs about `d · (w + h)` reads rather than
/// `w · h`. The flags are needed because the mask alone does not say whether a
/// pixel's column run is set: a pixel on a dirty row but an undamaged column
/// would otherwise lose it.
// ponytail: the flags are one byte per pixel, the same cost as DDE's classes;
// that is the price of exact stripe updates.
pub(super) fn band_update(
    rgb: &[u8],
    w: usize,
    h: usize,
    key: [u8; 3],
    flags: &mut [u8],
    mask: &mut [bool],
    dirty: Area,
) -> bool {
    debug_assert!(rgb.len() == 3 * w * h && flags.len() == w * h && mask.len() == w * h);
    let (x0, y0) = (dirty.x.min(w), dirty.y.min(h));
    let (x1, y1) = ((dirty.x + dirty.w).min(w), (dirty.y + dirty.h).min(h));
    let mut changed = false;
    for row in y0..y1 {
        let base = row * w;
        for f in &mut flags[base..base + w] {
            *f &= !RUN_ROW;
        }
        mark_long_runs(rgb, key, flags, w, |k| base + k, RUN_ROW);
        for i in base..base + w {
            changed |= settle(rgb, key, flags, mask, i);
        }
    }
    for col in x0..x1 {
        for k in 0..h {
            flags[k * w + col] &= !RUN_COL;
        }
        mark_long_runs(rgb, key, flags, h, |k| k * w + col, RUN_COL);
        for k in 0..h {
            changed |= settle(rgb, key, flags, mask, k * w + col);
        }
    }
    changed
}

/// Set `bit` on every pixel of each run of identical non-key pixels longer than
/// [`ICON_MAX_SIDE`] along one line of `len` pixels; `at(k)` is the index of the
/// k-th. Runs are read from `rgb` only, never from `flags`.
///
/// Why runs, not [`classify`]: a band is painted over the key, so its colour is
/// whatever the selection colour blends to. It is not reliably "dark", and a
/// blue tint is not a class at all. What a band does have that no icon has is a
/// flat run far longer than any icon. Icons and labels are at most
/// `ICON_MAX_SIDE` across, and text never repeats one colour that long.
///
/// A flat band with an icon in it is still split only where the icon is: every
/// band pixel lies on a row or a column that the icon does not cross, and so is
/// in a full-length run. A band smaller than `ICON_MAX_SIDE` in both directions
/// is not caught; that is the limit.
fn mark_long_runs(
    rgb: &[u8],
    key: [u8; 3],
    flags: &mut [u8],
    len: usize,
    at: impl Fn(usize) -> usize,
    bit: u8,
) {
    let px = |i: usize| &rgb[3 * i..3 * i + 3];
    let mut start = 0;
    for k in 1..=len {
        if k == len || px(at(k)) != px(at(start)) {
            if k - start > ICON_MAX_SIDE && px(at(start)) != key {
                for j in start..k {
                    flags[at(j)] |= bit;
                }
            }
            start = k;
        }
    }
}

/// Set pixel `i`'s visible bit from its key test and band flags; true if it
/// changed.
fn settle(rgb: &[u8], key: [u8; 3], flags: &[u8], mask: &mut [bool], i: usize) -> bool {
    let visible = rgb[3 * i..3 * i + 3] != key && flags[i] == 0;
    let changed = mask[i] != visible;
    mask[i] = visible;
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 3] = [1, 1, 1];
    const TOL: u8 = 2;

    /// A `w × h` RGB bitmap of the key colour, with helpers to draw on it.
    struct Bitmap {
        w: usize,
        h: usize,
        px: Vec<[u8; 3]>,
    }

    impl Bitmap {
        fn new(w: usize, h: usize) -> Self {
            Bitmap {
                w,
                h,
                px: vec![KEY; w * h],
            }
        }

        fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
            for yy in y..y + h {
                for xx in x..x + w {
                    self.px[yy * self.w + xx] = c;
                }
            }
        }

        fn rgb(&self) -> Vec<u8> {
            self.px.iter().flatten().copied().collect()
        }

        fn mask(&self) -> Vec<bool> {
            refine_mask(&self.rgb(), self.w * 3, self.w, self.h, KEY, TOL)
        }

        fn at(&self, mask: &[bool], x: usize, y: usize) -> bool {
            mask[y * self.w + x]
        }
    }

    const ICON: [u8; 3] = [200, 120, 40];

    #[test]
    fn classes_separate_key_dark_and_real_pixels() {
        assert_eq!(classify([1, 1, 1], KEY, TOL), CLASS_KEY);
        assert_eq!(classify([0, 0, 0], KEY, TOL), CLASS_KEY);
        assert_eq!(classify([3, 3, 3], KEY, TOL), CLASS_KEY);
        assert_eq!(classify([4, 1, 1], KEY, TOL), CLASS_DARK);
        assert_eq!(classify([39, 39, 39], KEY, TOL), CLASS_DARK);
        assert_eq!(classify([40, 5, 5], KEY, TOL), CLASS_OTHER);
        assert_eq!(classify(ICON, KEY, TOL), CLASS_OTHER);
    }

    #[test]
    fn a_plain_icon_on_the_key_is_exactly_its_own_pixels() {
        let mut b = Bitmap::new(80, 80);
        b.rect(20, 20, 40, 40, ICON);
        let m = b.mask();
        assert!(b.at(&m, 20, 20) && b.at(&m, 59, 59) && b.at(&m, 40, 40));
        assert!(!b.at(&m, 19, 20) && !b.at(&m, 60, 59) && !b.at(&m, 0, 0));
        assert_eq!(m.iter().filter(|&&v| v).count(), 40 * 40);
    }

    #[test]
    fn a_black_square_inside_a_coloured_icon_stays_opaque() {
        // The black screen of a terminal icon: within tolerance of the key, so
        // the old test punched a hole through it.
        let mut b = Bitmap::new(80, 80);
        b.rect(10, 10, 60, 60, ICON);
        b.rect(25, 25, 30, 30, [0, 0, 0]);
        let m = b.mask();
        assert!(b.at(&m, 25, 25) && b.at(&m, 40, 40) && b.at(&m, 54, 54));
        assert_eq!(m.iter().filter(|&&v| v).count(), 60 * 60);
        // Not painted over the outside.
        assert!(!b.at(&m, 5, 5));
    }

    #[test]
    fn an_enclosed_key_hole_is_filled_but_an_open_pocket_is_not() {
        let mut b = Bitmap::new(120, 80);
        // A ring with a key-coloured hole.
        b.rect(10, 10, 50, 50, ICON);
        b.rect(25, 25, 20, 20, KEY);
        // A "C" whose pocket opens to the right.
        b.rect(70, 10, 40, 50, ICON);
        b.rect(80, 25, 30, 20, KEY);
        let m = b.mask();
        assert!(b.at(&m, 30, 30), "enclosed hole");
        assert!(!b.at(&m, 100, 30), "open pocket");
        assert!(!b.at(&m, 85, 30), "open pocket, deep end");
    }

    #[test]
    fn an_outline_that_closes_only_diagonally_still_encloses_its_pocket() {
        // Walls 2 thick; the top wall and the right wall stop one pixel short
        // of each other, so the corner pixels touch only diagonally. Opaque
        // pixels are 8-connected, so the outline is closed: the pocket is a
        // hole, and a 4-connected flood from outside must not leak into it.
        let mut b = Bitmap::new(60, 60);
        b.rect(5, 5, 38, 2, ICON); // top wall, x 5..=42, y 5..=6
        b.rect(43, 7, 2, 38, ICON); // right wall, x 43..=44, y 7..=44
        b.rect(5, 7, 2, 38, ICON); // left wall
        b.rect(7, 43, 36, 2, ICON); // bottom wall
        let m = b.mask();
        assert!(b.at(&m, 42, 7), "pocket pixel at the diagonal corner");
        assert!(b.at(&m, 25, 25), "pocket centre");
        assert!(!b.at(&m, 43, 6), "the corner outside the outline");
        assert!(!b.at(&m, 2, 2));
    }

    #[test]
    fn small_counters_such_as_a_letter_o_stay_see_through() {
        let mut b = Bitmap::new(60, 60);
        // A 14×16 glyph: well under an icon, with a key-coloured counter.
        b.rect(20, 20, 14, 16, [255, 255, 255]);
        b.rect(24, 24, 6, 8, KEY);
        let m = b.mask();
        assert!(!b.at(&m, 26, 28), "counter of the glyph stays transparent");
        assert!(b.at(&m, 21, 21));
    }

    #[test]
    fn an_outline_bigger_than_any_icon_does_not_fill_its_inside() {
        let mut b = Bitmap::new(480, 440);
        // A hollow frame 420 wide: a rubber band, not an icon.
        b.rect(10, 10, 420, 400, ICON);
        b.rect(14, 14, 412, 392, KEY);
        let m = b.mask();
        assert!(!b.at(&m, 200, 200));
        assert!(b.at(&m, 11, 11));
    }

    #[test]
    fn an_antialiased_dark_edge_against_the_key_is_peeled_one_pixel_deep() {
        // A bright icon whose edge ramps down into the key: 255 .. 90, then a
        // dark blend (30), then the key. The 30 is a fringe; the 90 is not.
        let mut b = Bitmap::new(80, 80);
        b.rect(20, 20, 40, 40, [255, 255, 255]);
        b.rect(18, 20, 1, 40, [90, 90, 90]);
        b.rect(17, 20, 1, 40, [30, 30, 30]);
        let m = b.mask();
        assert!(b.at(&m, 18, 40), "mid-grey blend stays");
        assert!(!b.at(&m, 17, 40), "dark fringe against the key is peeled");
        assert!(b.at(&m, 20, 40));
    }

    #[test]
    fn a_shadow_ramp_loses_only_the_pixel_touching_the_key() {
        // Values 40, 30, 20, 10, 4 moving away from the icon, then the key.
        let mut b = Bitmap::new(100, 80);
        b.rect(10, 20, 40, 40, ICON);
        for (i, v) in [40u8, 30, 20, 10, 4].iter().enumerate() {
            b.rect(50 + i, 20, 1, 40, [*v, *v, *v]);
        }
        let m = b.mask();
        for x in 50..54 {
            assert!(b.at(&m, x, 40), "ramp pixel {x} is inside the shadow");
        }
        assert!(!b.at(&m, 54, 40), "the 4 touches the key and goes");
        assert!(!b.at(&m, 55, 40));
    }

    #[test]
    fn interior_dark_pixels_are_kept_even_against_a_filled_hole() {
        let mut b = Bitmap::new(80, 80);
        b.rect(10, 10, 60, 60, ICON);
        // A dark-grey (not key) block next to a black (key-coloured) hole.
        b.rect(30, 30, 10, 10, [20, 20, 20]);
        b.rect(40, 30, 10, 10, [0, 0, 0]);
        let m = b.mask();
        assert!(b.at(&m, 35, 35), "dark pixels inside the icon stay");
        assert!(b.at(&m, 45, 35), "the enclosed black is filled");
    }

    #[test]
    fn a_dark_icon_outline_on_the_key_is_peeled_but_the_body_is_kept() {
        let mut b = Bitmap::new(80, 80);
        b.rect(10, 10, 60, 60, [25, 25, 25]);
        b.rect(11, 11, 58, 58, ICON);
        let m = b.mask();
        assert!(!b.at(&m, 10, 40), "the 1-px dark outline is a fringe");
        assert!(b.at(&m, 11, 40) && b.at(&m, 40, 40));
    }

    #[test]
    fn stride_may_be_wider_than_the_row() {
        let mut b = Bitmap::new(50, 50);
        b.rect(5, 5, 40, 40, ICON);
        // Pad every row with 6 junk bytes.
        let mut padded = Vec::new();
        for y in 0..b.h {
            padded.extend(b.px[y * b.w..(y + 1) * b.w].iter().flatten());
            padded.extend_from_slice(&[255; 6]);
        }
        let m = refine_mask(&padded, 50 * 3 + 6, 50, 50, KEY, TOL);
        assert_eq!(m, b.mask());
    }

    /// The Xfce mask of a [`Bitmap`].
    fn band(b: &Bitmap) -> Vec<bool> {
        let mut m = vec![false; b.w * b.h];
        band_mask(&b.rgb(), b.w, b.h, KEY, &mut m);
        m
    }

    const BAND: [u8; 3] = [18, 36, 60];

    #[test]
    fn a_rubber_band_over_the_key_is_hidden_and_the_icons_in_and_around_it_stay() {
        let mut b = Bitmap::new(720, 520);
        b.rect(20, 20, 600, 400, BAND);
        b.rect(200, 150, 64, 64, ICON); // inside the band
        b.rect(650, 20, 40, 40, ICON); // outside it
        let m = band(&b);
        let visible_in_band = (20..620)
            .flat_map(|x| (20..420).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                m[y * b.w + x] && !((200..264).contains(&x) && (150..214).contains(&y))
            })
            .count();
        assert_eq!(visible_in_band, 0, "no band pixel outside the icon shows");
        assert!(b.at(&m, 200, 150) && b.at(&m, 263, 213), "icon inside");
        assert!(b.at(&m, 650, 20) && b.at(&m, 689, 59), "icon outside");
        assert!(
            !b.at(&m, 0, 0) && !b.at(&m, 700, 500),
            "the key stays see-through"
        );
    }

    #[test]
    fn a_tall_thin_band_is_caught_by_its_columns() {
        // 40 wide: no row run is long enough, but each column is 500 long.
        let mut b = Bitmap::new(100, 520);
        b.rect(30, 10, 40, 500, BAND);
        let m = band(&b);
        assert!(m.iter().all(|&v| !v), "every band pixel is keyed out");
    }

    #[test]
    fn icons_alone_are_exactly_their_own_pixels() {
        let mut b = Bitmap::new(300, 200);
        b.rect(10, 10, 128, 128, ICON);
        b.rect(20, 20, 40, 40, [0, 0, 0]); // a black screen: non-key, kept
        b.rect(150, 40, 380, 20, [255, 255, 255]); // a wide label, 380 < 385
        let m = band(&b);
        let want: Vec<bool> = b.px.iter().map(|&p| p != KEY).collect();
        assert_eq!(m, want);
    }

    #[test]
    fn a_run_of_exactly_the_icon_limit_survives_and_one_more_pixel_does_not() {
        // Row 0: exactly 384 identical pixels. Row 5: 385. Each column is a
        // single pixel of this colour, so only the row run is in play.
        let mut b = Bitmap::new(400, 10);
        b.rect(0, 0, ICON_MAX_SIDE, 1, BAND);
        b.rect(0, 5, ICON_MAX_SIDE + 1, 1, BAND);
        let m = band(&b);
        assert!(
            b.at(&m, 0, 0) && b.at(&m, ICON_MAX_SIDE - 1, 0),
            "384 is an icon"
        );
        assert!(!b.at(&m, 0, 5), "385 is a band");
    }

    /// Bring the stripe state up to date after `b` changed from `prev`: the
    /// changed pixels' bounding box, cut into `tile`-sized damage rects, each
    /// given to `band_update`. `prev` is then set to `b`.
    fn stripe_step(
        b: &Bitmap,
        prev: &mut Vec<[u8; 3]>,
        flags: &mut [u8],
        mask: &mut [bool],
        tile: usize,
    ) {
        let mut bb: Option<(usize, usize, usize, usize)> = None;
        for (i, (p, q)) in b.px.iter().zip(prev.iter()).enumerate() {
            if p != q {
                let (x, y) = (i % b.w, i / b.w);
                bb = Some(match bb {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
        *prev = b.px.clone();
        let Some((x0, y0, x1, y1)) = bb else { return };
        let rgb = b.rgb();
        for ty in (y0..=y1).step_by(tile) {
            for tx in (x0..=x1).step_by(tile) {
                let dirty = Area {
                    x: tx,
                    y: ty,
                    w: (tx + tile).min(x1 + 1) - tx,
                    h: (ty + tile).min(y1 + 1) - ty,
                };
                band_update(&rgb, b.w, b.h, KEY, flags, mask, dirty);
            }
        }
    }

    #[test]
    fn stripe_updates_match_a_full_recompute_as_a_band_is_drawn_grown_and_removed() {
        let (w, h) = (700, 520);
        let mut b = Bitmap::new(w, h);
        let mut prev = b.px.clone();
        let (mut flags, mut mask) = (vec![0u8; w * h], vec![false; w * h]);
        // The stripe state must equal a full recompute of the same picture.
        let check = |b: &Bitmap, flags: &[u8], mask: &[bool]| {
            let (mut full_f, mut full_m) = (vec![0u8; w * h], vec![false; w * h]);
            let rgb = b.rgb();
            band_update(
                &rgb,
                w,
                h,
                KEY,
                &mut full_f,
                &mut full_m,
                Area { x: 0, y: 0, w, h },
            );
            assert!(
                flags == full_f.as_slice(),
                "flags differ from a full recompute"
            );
            assert!(
                mask == full_m.as_slice(),
                "mask differs from a full recompute"
            );
        };

        // Drawn: a band with an icon inside and one outside.
        b.rect(20, 20, 600, 400, BAND);
        b.rect(200, 150, 64, 64, ICON);
        b.rect(650, 20, 40, 40, ICON);
        stripe_step(&b, &mut prev, &mut flags, &mut mask, 64);
        check(&b, &flags, &mask);
        assert!(!b.at(&mask, 30, 30), "the band is keyed out");
        assert!(b.at(&mask, 200, 150) && b.at(&mask, 650, 20));

        // Grown: wider and taller, the icon is drawn over again.
        b.rect(20, 20, 640, 440, BAND);
        b.rect(200, 150, 64, 64, ICON);
        stripe_step(&b, &mut prev, &mut flags, &mut mask, 64);
        check(&b, &flags, &mask);

        // Cut: a horizontal strip of the band goes back to the key, so the band
        // is two pieces and the runs in the cut rows are gone.
        b.rect(20, 300, 640, 140, KEY);
        stripe_step(&b, &mut prev, &mut flags, &mut mask, 64);
        check(&b, &flags, &mask);

        // Removed: the rest of the band goes, the icon stays.
        b.rect(20, 20, 640, 440, KEY);
        b.rect(200, 150, 64, 64, ICON);
        stripe_step(&b, &mut prev, &mut flags, &mut mask, 64);
        check(&b, &flags, &mask);
        assert!(b.at(&mask, 200, 150) && !b.at(&mask, 30, 30));

        // A tall thin band that only its columns can catch, then removed.
        b.rect(100, 60, 40, 440, BAND);
        stripe_step(&b, &mut prev, &mut flags, &mut mask, 64);
        check(&b, &flags, &mask);
        assert!(!b.at(&mask, 110, 200));
        b.rect(100, 60, 40, 440, KEY);
        stripe_step(&b, &mut prev, &mut flags, &mut mask, 64);
        check(&b, &flags, &mask);
    }

    /// A desktop bigger than the refine span, so cutting at its edge is real:
    /// an icon with a black screen, a ring, a shadowed icon, a glyph, a
    /// selection slab, and a hollow frame too big to be an icon.
    fn desktop() -> (Vec<u8>, usize, usize) {
        let (w, h) = (1900, 1500);
        let mut b = Bitmap::new(w, h);
        b.rect(20, 20, 64, 64, ICON);
        b.rect(34, 34, 36, 30, [0, 0, 0]);
        b.rect(200, 30, 70, 70, [90, 200, 120]);
        b.rect(220, 50, 30, 30, KEY);
        b.rect(400, 40, 60, 60, [60, 90, 220]);
        for (i, v) in [38u8, 28, 18, 8, 5].iter().enumerate() {
            b.rect(460 + i, 40, 1, 60, [*v, *v, *v]);
        }
        b.rect(60, 300, 12, 16, [255, 255, 255]);
        b.rect(63, 304, 5, 8, KEY);
        b.rect(300, 300, 150, 120, [30, 30, 30]);
        b.rect(305, 305, 140, 110, [150, 150, 150]);
        // Too big for an icon: its inside must stay see-through, and a cut
        // through it must not make it look small enough to fill.
        b.rect(900, 600, 700, 600, ICON);
        b.rect(906, 606, 688, 588, KEY);
        // An icon with a black screen far from everything else.
        b.rect(1700, 1300, 90, 90, [200, 30, 30]);
        b.rect(1715, 1315, 60, 60, [0, 0, 0]);
        let classes: Vec<u8> = b.px.iter().map(|&p| classify(p, KEY, TOL)).collect();
        (classes, w, h)
    }

    #[test]
    fn refining_a_dirty_area_matches_refining_the_whole_screen() {
        let (classes, w, h) = desktop();
        let full = refine_classes(&classes, w, h);
        let dirties = [
            // Inside the black screen: nothing in sight says it is a hole.
            Area {
                x: 50,
                y: 45,
                w: 3,
                h: 3,
            },
            // Inside the ring's hole, and on its rim.
            Area {
                x: 230,
                y: 60,
                w: 2,
                h: 2,
            },
            Area {
                x: 200,
                y: 50,
                w: 4,
                h: 4,
            },
            // The shadow tail against the key.
            Area {
                x: 458,
                y: 60,
                w: 10,
                h: 3,
            },
            // Inside the hollow frame, far from its walls.
            Area {
                x: 1200,
                y: 900,
                w: 5,
                h: 5,
            },
            // Across the frame's wall.
            Area {
                x: 900,
                y: 880,
                w: 14,
                h: 6,
            },
            // A corner of the parent.
            Area {
                x: 1850,
                y: 1450,
                w: 50,
                h: 50,
            },
            // The selection slab.
            Area {
                x: 310,
                y: 310,
                w: 1,
                h: 1,
            },
            // The far icon's black screen, with the parent's edge close by.
            Area {
                x: 1740,
                y: 1340,
                w: 2,
                h: 2,
            },
        ];
        for d in dirties {
            let (area, sub) = refine_area(&classes, w, h, d);
            assert!(area.x <= d.x && area.y <= d.y, "{d:?} -> {area:?}");
            assert!(area.x + area.w >= d.x + d.w && area.y + area.h >= d.y + d.h);
            for y in 0..area.h {
                for x in 0..area.w {
                    assert_eq!(
                        sub[y * area.w + x],
                        full[(area.y + y) * w + area.x + x],
                        "dirty {d:?}: pixel ({}, {})",
                        area.x + x,
                        area.y + y
                    );
                }
            }
        }
    }

    #[test]
    fn a_dirty_spot_deep_inside_an_enclosed_hole_stays_opaque() {
        let (classes, w, h) = desktop();
        // 3×3 inside the 36×30 black screen of the first icon.
        let (area, sub) = refine_area(
            &classes,
            w,
            h,
            Area {
                x: 50,
                y: 45,
                w: 3,
                h: 3,
            },
        );
        let at = |x: usize, y: usize| sub[(y - area.y) * area.w + (x - area.x)];
        assert!(at(51, 46), "the black screen is a hole, so it is opaque");
        // The result reaches the whole icon, not just the spot.
        assert!(at(35, 35) && at(68, 62));
        // The frame's inside is not a hole however it is looked at.
        let (area, sub) = refine_area(
            &classes,
            w,
            h,
            Area {
                x: 1200,
                y: 900,
                w: 5,
                h: 5,
            },
        );
        let at = |x: usize, y: usize| sub[(y - area.y) * area.w + (x - area.x)];
        assert!(!at(1202, 902));
    }

    #[test]
    fn an_empty_screen_refines_to_an_empty_mask() {
        let b = Bitmap::new(40, 30);
        assert!(b.mask().iter().all(|&v| !v));
        let classes = vec![CLASS_KEY; 40 * 30];
        let (area, sub) = refine_area(
            &classes,
            40,
            30,
            Area {
                x: 5,
                y: 5,
                w: 3,
                h: 3,
            },
        );
        assert_eq!(sub.len(), area.w * area.h);
        assert!(sub.iter().all(|&v| !v));
    }
}

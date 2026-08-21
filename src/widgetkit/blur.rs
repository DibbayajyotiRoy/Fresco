//! Gaussian-approximating blur over an 8-bit alpha mask.
//!
//! # Why this is a real blur and not concentric strokes
//!
//! Soft shadow is the second thing ASS cannot do (after gradients). `\blur` in
//! libass softens the *drawn shape's own* alpha and nothing else, so the
//! existing widgets have no way to lay a diffuse dark halo under a card. The
//! usual fake — a stack of ever-larger, ever-fainter outlines — produces
//! visible contour rings at any radius worth having, and costs a shape per
//! ring. This does the actual convolution instead, once, over one channel.
//!
//! # Three box passes, not a true Gaussian
//!
//! Convolving with a box three times is the standard approximation: by the
//! central limit theorem the result is within a couple of percent of a true
//! Gaussian of the same σ, and it is **O(1) per pixel per pass** regardless of
//! radius, because each pass is a sliding sum. A separable true Gaussian would
//! be O(kernel) per pixel and, at the σ ≈ 12–24 logical units a card shadow
//! wants (× up to 2 for a 4K scale), that is an order of magnitude more work
//! for a difference nobody can see under a translucent panel.
//!
//! The three box widths come from the standard derivation of the widths whose
//! combined variance matches a given σ, which is why they are not simply `3σ`
//! each; the derivation is in `box_radii_for_sigma`.
//!
//! # Edges
//!
//! Samples outside the mask are taken as the nearest edge value (clamp), which
//! is both what a sliding sum does naturally and correct for our use: a shadow
//! mask is drawn with a margin of empty pixels around the shape, so the edge
//! value *is* zero and clamping and zero-padding agree. When a caller does blur
//! a shape that touches the border, clamping smears the edge outward rather
//! than drawing a false dark line along it, which is the friendlier of the two
//! wrong answers.
//!
//! # Allocation
//!
//! [`blur_alpha`] never allocates after the first call: the caller owns both
//! the mask and the scratch buffer and hands them in, so a widget repainting at
//! `VISUAL_FPS` reuses the same two buffers forever. `super::Canvas` does
//! exactly that — see its allocation notes.

/// Blur `mask` (one byte of coverage per pixel, `w * h` of them) in place.
///
/// `scratch` is sized to `w * h` on first use and reused thereafter. `sigma` is
/// in **device pixels**, i.e. the caller has already applied the canvas scale.
///
/// A `sigma` at or below zero, a non-finite one, or a mask whose length
/// disagrees with `w * h` leaves the mask untouched — a shadow that fails to
/// blur is a hard-edged shadow, which is survivable, where a panic is not.
pub fn blur_alpha(mask: &mut [u8], w: usize, h: usize, sigma: f32, scratch: &mut Vec<u8>) {
    if w == 0 || h == 0 || mask.len() != w * h || sigma <= 0.0 || !sigma.is_finite() {
        return;
    }
    let [r0, r1, r2] = box_radii_for_sigma(sigma);
    scratch.clear();
    scratch.resize(w * h, 0);
    // Six passes, ping-ponging between the two buffers so the result lands back
    // in `mask`: three horizontal, then three vertical. Separability is what
    // makes the whole thing linear in pixel count rather than quadratic in
    // radius.
    box_h(mask, scratch, w, h, r0);
    box_h(scratch, mask, w, h, r1);
    box_h(mask, scratch, w, h, r2);
    box_v(scratch, mask, w, h, r0);
    box_v(mask, scratch, w, h, r1);
    box_v(scratch, mask, w, h, r2);
}

/// The three box radii whose triple convolution best matches a Gaussian of
/// `sigma`.
///
/// A box of width `w` has variance `(w² − 1) / 12`; three of them convolve to
/// the sum of their variances. Solving `Σ var = σ²` for a single ideal width
/// gives `w = sqrt(12σ²/3 + 1)`, which is almost never an odd integer, so the
/// passes are split between the two nearest odd widths in whatever proportion
/// lands the total variance closest to `σ²`.
///
/// Crate-visible so the tests can assert the widths directly rather than only
/// inferring them from blurred output.
pub(crate) fn box_radii_for_sigma(sigma: f32) -> [usize; 3] {
    const N: f32 = 3.0;
    let ideal = (12.0 * sigma * sigma / N + 1.0).sqrt();
    // Nearest odd width at or below the ideal, and the next odd width up.
    let mut lower = ideal.floor();
    if (lower as i32) % 2 == 0 {
        lower -= 1.0;
    }
    let lower = lower.max(1.0);
    // How many of the three passes should use the narrower width.
    let narrow = (12.0 * sigma * sigma - N * lower * lower - 4.0 * N * lower - 3.0 * N)
        / (-4.0 * lower - 4.0);
    let narrow = if narrow.is_finite() {
        narrow.round().clamp(0.0, N) as usize
    } else {
        0
    };
    let mut out = [0usize; 3];
    for (i, slot) in out.iter_mut().enumerate() {
        let width = if i < narrow { lower } else { lower + 2.0 };
        *slot = ((width - 1.0) / 2.0).max(0.0) as usize;
    }
    out
}

/// One horizontal box pass, `src` → `dst`: a sliding sum with clamped edges.
fn box_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    if r == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let norm = (2 * r + 1) as u32;
    for y in 0..h {
        let row = &src[y * w..y * w + w];
        let (first, last) = (u32::from(row[0]), u32::from(row[w - 1]));
        // Prime the window for x = 0: r clamped copies of the left edge, the
        // in-range samples, and — when the kernel is wider than the whole row —
        // the clamped right edge for whatever is still missing.
        let mut acc = first * r as u32;
        for &v in &row[..=r.min(w - 1)] {
            acc += u32::from(v);
        }
        if r >= w {
            acc += last * (r + 1 - w) as u32;
        }
        let out = &mut dst[y * w..y * w + w];
        for x in 0..w {
            out[x] = ((acc + norm / 2) / norm) as u8;
            let entering = if x + r + 1 < w {
                row[x + r + 1]
            } else {
                row[w - 1]
            };
            let leaving = if x >= r { row[x - r] } else { row[0] };
            // Add before subtracting: the window always contains `leaving`, so
            // this order cannot underflow the unsigned accumulator.
            acc += u32::from(entering);
            acc -= u32::from(leaving);
        }
    }
}

/// One vertical box pass, `src` → `dst`. The same sliding sum, striding by `w`.
fn box_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    if r == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let norm = (2 * r + 1) as u32;
    for x in 0..w {
        let at = |y: usize| u32::from(src[y * w + x]);
        let (first, last) = (at(0), at(h - 1));
        let mut acc = first * r as u32;
        for y in 0..=r.min(h - 1) {
            acc += at(y);
        }
        if r >= h {
            acc += last * (r + 1 - h) as u32;
        }
        for y in 0..h {
            dst[y * w + x] = ((acc + norm / 2) / norm) as u8;
            let entering = if y + r + 1 < h { at(y + r + 1) } else { last };
            let leaving = if y >= r { at(y - r) } else { first };
            acc += entering;
            acc -= leaving;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(w: usize, h: usize) -> Vec<u8> {
        vec![0u8; w * h]
    }

    #[test]
    fn zero_and_invalid_sigma_leave_the_mask_alone() {
        let mut m = mask(8, 8);
        m[8 * 4 + 4] = 255;
        let before = m.clone();
        let mut scratch = Vec::new();
        blur_alpha(&mut m, 8, 8, 0.0, &mut scratch);
        assert_eq!(m, before);
        blur_alpha(&mut m, 8, 8, -3.0, &mut scratch);
        assert_eq!(m, before);
        blur_alpha(&mut m, 8, 8, f32::NAN, &mut scratch);
        assert_eq!(m, before);
        blur_alpha(&mut m, 8, 8, f32::INFINITY, &mut scratch);
        assert_eq!(m, before);
        // Mismatched dimensions must be refused, not indexed into.
        blur_alpha(&mut m, 9, 9, 4.0, &mut scratch);
        assert_eq!(m, before);
        blur_alpha(&mut m, 0, 0, 4.0, &mut scratch);
    }

    #[test]
    fn a_flat_field_survives_unchanged() {
        // Any correctly normalised blur is the identity on a constant field;
        // this catches normalisation and edge-clamp mistakes in one assertion.
        let mut m = vec![200u8; 32 * 32];
        let mut scratch = Vec::new();
        blur_alpha(&mut m, 32, 32, 5.0, &mut scratch);
        for (i, &v) in m.iter().enumerate() {
            assert!(v.abs_diff(200) <= 1, "pixel {i} drifted to {v}");
        }
    }

    #[test]
    fn a_point_spreads_symmetrically_and_falls_off_monotonically() {
        const N: usize = 65;
        let mut m = mask(N, N);
        m[32 * N + 32] = 255;
        let mut scratch = Vec::new();
        blur_alpha(&mut m, N, N, 6.0, &mut scratch);
        let at = |x: usize, y: usize| u32::from(m[y * N + x]);

        for d in 1..30 {
            // Exact within an axis: the pass sees a symmetric input.
            assert_eq!(at(32 - d, 32), at(32 + d, 32), "h asymmetry at {d}");
            assert_eq!(at(32, 32 - d), at(32, 32 + d), "v asymmetry at {d}");
            // Across axes, only to within a unit: the vertical passes run on
            // values the horizontal passes already rounded to bytes.
            assert!(
                at(32 - d, 32).abs_diff(at(32, 32 - d)) <= 1,
                "axes disagree at {d}"
            );
        }
        // Monotone falloff outward from the peak — the property "soft shadow"
        // actually means, and the one concentric strokes violate.
        let mut prev = at(32, 32);
        for d in 1..32 {
            let v = at(32 + d, 32);
            assert!(v <= prev, "falloff reversed at {d}: {v} > {prev}");
            prev = v;
        }
        // The peak is at the centre and is far below the original impulse,
        // because the energy has been spread over the kernel.
        assert!(at(32, 32) > 0);
        assert!(at(32, 32) < 40, "impulse did not spread: {}", at(32, 32));
        // Nothing has leaked as far as the corner at this sigma.
        assert_eq!(at(0, 0), 0);
    }

    #[test]
    fn a_hard_edge_becomes_a_ramp_without_moving_the_edge() {
        const W: usize = 64;
        const H: usize = 8;
        let mut m = mask(W, H);
        for y in 0..H {
            for x in 0..W / 2 {
                m[y * W + x] = 255;
            }
        }
        let mut scratch = Vec::new();
        let sigma = 4.0;
        blur_alpha(&mut m, W, H, sigma, &mut scratch);
        let row: Vec<u32> = (0..W).map(|x| u32::from(m[4 * W + x])).collect();

        // Monotone decreasing across the whole row.
        for x in 1..W {
            assert!(row[x] <= row[x - 1], "not monotone at {x}: {row:?}");
        }
        // The 50% crossing has not moved: the two pixels straddling the
        // original edge still sum to full coverage.
        assert!(
            (row[W / 2 - 1] + row[W / 2]).abs_diff(255) <= 2,
            "edge moved: {} + {}",
            row[W / 2 - 1],
            row[W / 2]
        );
        assert!(row[W / 2 - 1] >= 128 && row[W / 2] <= 128);
        // Effectively finished within ~3σ of the edge on both sides.
        let reach = (3.0 * sigma).ceil() as usize;
        assert_eq!(row[W / 2 + reach + 2], 0);
        assert_eq!(row[W / 2 - reach - 3], 255);
    }

    #[test]
    fn box_widths_track_sigma_and_stay_close_to_each_other() {
        for &sigma in &[0.5f32, 1.0, 2.0, 6.0, 24.0, 64.0] {
            let r = box_radii_for_sigma(sigma);
            let (min, max) = (r.iter().min().unwrap(), r.iter().max().unwrap());
            assert!(
                max - min <= 1,
                "sigma {sigma} gave wildly different boxes {r:?}"
            );
            // Variance of the triple convolution should land near sigma^2.
            let var: f32 = r
                .iter()
                .map(|&ri| {
                    let w = (2 * ri + 1) as f32;
                    (w * w - 1.0) / 12.0
                })
                .sum();
            let got = var.sqrt();
            assert!(
                (got - sigma).abs() <= 0.5 + sigma * 0.1,
                "sigma {sigma} approximated as {got} (radii {r:?})"
            );
        }
    }

    #[test]
    fn blurring_a_mask_smaller_than_the_kernel_does_not_panic() {
        // A tiny widget with a big shadow radius: the window is wider than the
        // buffer on both axes, which is the case the priming arithmetic must
        // survive.
        let mut m = vec![255u8; 3 * 2];
        let mut scratch = Vec::new();
        blur_alpha(&mut m, 3, 2, 40.0, &mut scratch);
        assert!(m.iter().all(|&v| v.abs_diff(255) <= 1), "{m:?}");
    }

    #[test]
    fn scratch_is_reused_without_regrowing() {
        let mut scratch = Vec::new();
        let mut m = mask(16, 16);
        m[16 * 8 + 8] = 255;
        blur_alpha(&mut m, 16, 16, 3.0, &mut scratch);
        let cap = scratch.capacity();
        for _ in 0..5 {
            blur_alpha(&mut m, 16, 16, 3.0, &mut scratch);
        }
        assert_eq!(scratch.capacity(), cap, "blur reallocated on a hot path");
    }
}

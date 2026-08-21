//! The type system: one modular scale, generated tracking and leading, and the
//! four things CJK does differently.
//!
//! `docs/widget-design-spec.md` §5 is the authority for every number here.
//!
//! # Why tracking and leading are generated rather than tabulated
//!
//! A user can set `Clock::font_size_pt` to any value, so half the sizes this
//! toolkit draws are *not* on the ladder. Tabulated tracking would then either
//! be wrong for those sizes or force them onto the nearest step, and forcing a
//! user-chosen 60 lu clock to 53 or 67 is not a design decision anyone asked
//! for. So the ladder gives the *roles* their sizes, and two continuous
//! functions give **any** size its tracking and leading:
//!
//! ```text
//! tracking_em(s) = clamp(-0.0285 + 0.62 / s, -0.030, +0.140)   (+0.100 for an
//!                                                               UPPERCASE
//!                                                               micro-label)
//! line_height(s) = clamp(1.62 - 0.30 · log2(s / 11), 0.94, 1.62)
//! ```
//!
//! Both encode the same observation, which is Apple's: tracking is
//! size-specific and never one value for all sizes, and leading tracks size
//! inversely. Small type needs to be opened up and given air; a 67 lu hero
//! needs to be closed up and set tight or it reads as a row of separate digits.
//!
//! Weight is **not** generated — it steps, because a font has faces rather than
//! a continuum, and a request for a weight the family does not ship is
//! synthesised by the rasteriser and smears.
//!
//! # Hierarchy is not carried by opacity
//!
//! Spec §2.5, restated because it drives this module's shape: **between any two
//! adjacent rows in a block, at least two of {size, weight, case} must differ.**
//! Opacity is never the only difference. A translucent card can have a bright
//! wallpaper behind it and faded ink is the first thing that destroys — so the
//! ink ramp is a *refinement* of a hierarchy that already reads in black and
//! white, never the hierarchy itself. [`Step`] exists so that a card states
//! which role a row plays and gets the size, weight and case together.
//!
//! # CJK is not "the same layout with different glyphs"
//!
//! Fresco ships a Simplified-Chinese UI, so this is a first-class path, not a
//! fallback. Four concrete deltas, all in [`Script::Cjk`]:
//!
//! | Property | Latin | CJK |
//! |---|---|---|
//! | Weight | 500 / 600 / 650 / 700 | **500 or 700 only** — Noto Sans SC ships 100/300/400/500/700/900, so 600 and 650 are synthesised and smear at ≤ 18 lu |
//! | Micro-label case | `to_uppercase()` | **no case transform** — Han has no case; emphasis comes from weight 700 |
//! | Micro-label tracking | +0.128 em | **+0.040 em** — Han glyphs are already full-width and squared, and +0.128 em breaks a two-character word into two unrelated characters |
//! | Line-height | `line_height(s)` | `× 1.18`, floor **1.20** — taller ascenders and descenders; the Latin 0.94 hero leading collides |
//!
//! # Positioning capitals, not em boxes
//!
//! [`crate::widgetkit::Canvas::text`] takes a **top-left** origin, and the top
//! of an em box is not the top of a capital: Inter's ascent runs about 0.25 em
//! above its capitals. Laying a card out to em boxes gives a 64 lu hero 16 lu of
//! invisible dead band above it and makes the card look bottom-heavy. So every
//! card here places **cap tops** and converts with [`cap_gap`] at the last
//! moment. [`Metrics`] carries the three constants per script.

use super::text::TextRun;

/// Which set of vertical metrics and weight rules a run follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Script {
    /// Latin, Cyrillic, Greek, digits, punctuation.
    #[default]
    Latin,
    /// Han, Kana or Hangul.
    Cjk,
}

impl Script {
    /// The script of `s`, decided by whether it contains **any** ideographic,
    /// kana or Hangul codepoint.
    ///
    /// Any, not most: a title like `Blue Monday — 蓝色星期一` has to take the
    /// CJK line-height or the Han glyphs' descenders collide with the next row.
    /// The spec's per-*run* script split is a refinement of this and is what a
    /// future shaping pass should do; per-string is the conservative version of
    /// the same rule and never produces a collision.
    pub fn of(s: &str) -> Self {
        if s.chars().any(is_cjk) {
            Self::Cjk
        } else {
            Self::Latin
        }
    }

    /// Vertical metrics for this script, as fractions of the em.
    pub fn metrics(self) -> Metrics {
        match self {
            Self::Latin => Metrics {
                cap_gap: 0.250,
                cap_height: 0.727,
                descender: 0.100,
            },
            // An ideographic face: much less dead band above, a taller
            // effective cap, and a shallower descender.
            Self::Cjk => Metrics {
                cap_gap: 0.120,
                cap_height: 0.860,
                descender: 0.060,
            },
        }
    }
}

/// True for a codepoint that forces [`Script::Cjk`].
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2EFF     // CJK radicals
        | 0x3000..=0x303F   // CJK punctuation
        | 0x3040..=0x30FF   // Hiragana, Katakana
        | 0x3100..=0x312F   // Bopomofo
        | 0x3130..=0x318F   // Hangul compatibility jamo
        | 0x31C0..=0x31EF   // CJK strokes
        | 0x3400..=0x4DBF   // CJK Ext A
        | 0x4E00..=0x9FFF   // CJK unified
        | 0xA960..=0xA97F   // Hangul jamo extended A
        | 0xAC00..=0xD7AF   // Hangul syllables
        | 0xF900..=0xFAFF   // CJK compatibility ideographs
        | 0xFF00..=0xFFEF   // halfwidth / fullwidth forms
        | 0x20000..=0x3FFFF // CJK Ext B and beyond
    )
}

/// Vertical metrics as fractions of the em, per script.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Em-box top to cap top. Subtract `cap_gap × size` from a row's cap top to
    /// get the origin [`crate::widgetkit::Canvas::text`] wants.
    pub cap_gap: f32,
    /// Cap top to baseline.
    pub cap_height: f32,
    /// Baseline to the visual bottom of a descender.
    pub descender: f32,
}

/// The modular scale, ratio **1.25** (a major third) anchored at 14 lu.
///
/// A role, not a size: asking for [`Step::Title`] gets the size, the weight and
/// the case together, which is what makes the hierarchy budget in the module
/// docs enforceable at the call site rather than aspirational.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// 11 lu, 600, UPPERCASE, +0.128 em. The micro-label.
    Micro,
    /// 11 lu, 500. Tertiary captions, axis labels, times.
    Caption,
    /// 14 lu, 500. The secondary supporting line.
    Body,
    /// 18 lu, 600. A track title.
    Title,
    /// 22 lu, 500. A lyric line on a small card.
    Lead,
    /// 27 lu, 500. A lyric line on a large card.
    LeadLg,
    /// 34 lu, 600.
    HeroS,
    /// 43 lu, 650.
    HeroM,
    /// 53 lu, 700.
    HeroL,
    /// 67 lu, 700. The default clock hero.
    HeroXl,
    /// 84 lu, 700.
    Hero2Xl,
}

impl Step {
    /// Every step, smallest first.
    pub const ALL: [Self; 11] = [
        Self::Micro,
        Self::Caption,
        Self::Body,
        Self::Title,
        Self::Lead,
        Self::LeadLg,
        Self::HeroS,
        Self::HeroM,
        Self::HeroL,
        Self::HeroXl,
        Self::Hero2Xl,
    ];

    /// Size in logical units at `S = 1`.
    pub fn size(self) -> f32 {
        match self {
            Self::Micro | Self::Caption => 11.0,
            Self::Body => 14.0,
            Self::Title => 18.0,
            Self::Lead => 22.0,
            Self::LeadLg => 27.0,
            Self::HeroS => 34.0,
            Self::HeroM => 43.0,
            Self::HeroL => 53.0,
            Self::HeroXl => 67.0,
            Self::Hero2Xl => 84.0,
        }
    }

    /// OpenType weight for Latin. CJK collapses this to 500 or 700 — see
    /// [`weight_for_script`].
    pub fn weight(self) -> u16 {
        match self {
            Self::Caption | Self::Body | Self::Lead | Self::LeadLg => 500,
            Self::Micro | Self::Title | Self::HeroS => 600,
            Self::HeroM => 650,
            Self::HeroL | Self::HeroXl | Self::Hero2Xl => 700,
        }
    }

    /// True for the one step that is set in capitals.
    pub fn is_micro_label(self) -> bool {
        matches!(self, Self::Micro)
    }
}

/// The size ladder, smallest first, with the duplicate 11 collapsed.
pub const LADDER: [f32; 10] = [11.0, 14.0, 18.0, 22.0, 27.0, 34.0, 43.0, 53.0, 67.0, 84.0];

/// The ladder step nearest `size`, in log space so 20 lands on 18 rather than
/// on 22 (a linear nearest would round toward the coarser end of the scale,
/// which is where the steps are furthest apart and the error is largest).
///
/// A non-finite or non-positive input gets the smallest step; there is no size
/// at which "no type" is the right answer.
pub fn nearest_ladder_step(size: f32) -> f32 {
    if !size.is_finite() || size <= 0.0 {
        return LADDER[0];
    }
    let target = size.ln();
    let mut best = LADDER[0];
    let mut best_err = f32::MAX;
    for s in LADDER {
        let err = (s.ln() - target).abs();
        if err < best_err {
            best_err = err;
            best = s;
        }
    }
    best
}

/// Tracking for `size`, as a fraction of the em (spec §5.1).
///
/// `micro_label` adds the +0.100 em that turns the 11 lu step into the widely
/// letterspaced uppercase micro-label — and for CJK it is replaced outright by
/// a flat +0.040 em, because Han glyphs are square and full-width and the Latin
/// value pulls a two-character word into two unrelated characters.
pub fn tracking_em(size: f32, micro_label: bool, script: Script) -> f32 {
    if script == Script::Cjk {
        return if micro_label { 0.040 } else { 0.0 };
    }
    if !size.is_finite() || size <= 0.0 {
        return 0.0;
    }
    let base = (-0.0285 + 0.62 / size).clamp(-0.030, 0.140);
    if micro_label {
        base + 0.100
    } else {
        base
    }
}

/// Leading for `size`, as a multiple of the size (spec §5.1, §5.3).
pub fn line_height_ratio(size: f32, script: Script) -> f32 {
    if !size.is_finite() || size <= 0.0 {
        return 1.62;
    }
    let base = (1.62 - 0.30 * (size / 11.0).log2()).clamp(0.94, 1.62);
    match script {
        Script::Latin => base,
        Script::Cjk => (base * 1.18).max(1.20),
    }
}

/// Weight for a size the ladder does not name, stepping 500 → 600 → 650 → 700
/// at 14 / 18 / 43 / 53 lu.
pub fn weight_for(size: f32) -> u16 {
    if !size.is_finite() {
        return 500;
    }
    match size {
        s if s >= 53.0 => 700,
        s if s >= 43.0 => 650,
        s if s >= 18.0 => 600,
        _ => 500,
    }
}

/// A Latin weight mapped onto what the script can actually render.
///
/// Noto Sans SC ships 100/300/400/500/700/900. A request for 600 or 650 is
/// synthesised by the rasteriser — emboldening a 500 face by smearing it — and
/// at 11 to 18 lu that closes the counters of a dense Han glyph completely.
/// Mapping both up to 700 asks for a face that exists.
pub fn weight_for_script(weight: u16, script: Script) -> u16 {
    match script {
        Script::Latin => weight,
        Script::Cjk => {
            if weight >= 600 {
                700
            } else {
                500
            }
        }
    }
}

/// The case transform a micro-label takes: uppercase for Latin, **unchanged**
/// for CJK.
///
/// Returns a `Cow` so the overwhelmingly common case — a label that is already
/// the right case, or a Han label that never changes — costs no allocation on
/// the draw path.
pub fn micro_case(text: &str) -> std::borrow::Cow<'_, str> {
    if Script::of(text) == Script::Cjk {
        return std::borrow::Cow::Borrowed(text);
    }
    if text.chars().all(|c| !c.is_lowercase()) {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(text.to_uppercase())
}

/// A [`TextRun`] at `size` with the generated tracking, leading and weight, and
/// the script's adjustments already applied.
///
/// This is the constructor cards use. Building a `TextRun` by hand skips every
/// rule in this module, which is how a card ends up with 600-weight Han at
/// 11 lu.
pub fn run(text: &str, size: f32, fonts: &mut super::text::FontStack) -> TextRun {
    styled(text, size, weight_for(size), false, fonts)
}

/// A [`TextRun`] for a [`Step`], with that step's weight and case.
pub fn step_run(text: &str, step: Step, fonts: &mut super::text::FontStack) -> TextRun {
    let cased = if step.is_micro_label() {
        micro_case(text)
    } else {
        std::borrow::Cow::Borrowed(text)
    };
    styled(
        &cased,
        step.size(),
        step.weight(),
        step.is_micro_label(),
        fonts,
    )
}

/// The general form: any size, any weight, told explicitly whether it is a
/// micro-label (which changes only its tracking — the case transform is the
/// caller's, via [`micro_case`], because it changes the string).
pub fn styled(
    text: &str,
    size: f32,
    weight: u16,
    micro_label: bool,
    fonts: &mut super::text::FontStack,
) -> TextRun {
    let script = Script::of(text);
    let size = if size.is_finite() && size > 0.0 {
        size
    } else {
        LADDER[0]
    };
    let family = match script {
        Script::Latin => fonts.latin_family(),
        // Ask for the CJK family by name when one is installed: cosmic-text
        // would find it by fallback anyway, but a per-word fallback search is
        // the slow path and this run is entirely Han.
        Script::Cjk => fonts.cjk_family().or_else(|| fonts.latin_family()),
    };
    TextRun::new(text, size)
        .family(family)
        .weight(weight_for_script(weight, script))
        .letter_spacing(tracking_em(size, micro_label, script) * size)
        .line_height(line_height_ratio(size, script) * size)
}

/// A monospace run, for LCD readouts and tabular columns.
///
/// Falls back to the Latin stack when no monospace family is installed: a
/// proportional readout is worse than a monospace one and far better than
/// nothing.
pub fn mono_run(text: &str, size: f32, fonts: &mut super::text::FontStack) -> TextRun {
    let family = fonts.mono_family().or_else(|| fonts.latin_family());
    let size = if size.is_finite() && size > 0.0 {
        size
    } else {
        LADDER[0]
    };
    TextRun::new(text, size)
        .family(family)
        .weight(weight_for(size))
        .line_height(line_height_ratio(size, Script::Latin) * size)
}

/// Em-box top for a row whose **cap top** should land at `cap_top`.
///
/// The one conversion every card in this toolkit performs, and the reason the
/// vertical rhythms in the spec are quoted in cap tops.
pub fn cap_gap(size: f32, script: Script) -> f32 {
    if !size.is_finite() || size <= 0.0 {
        return 0.0;
    }
    script.metrics().cap_gap * size
}

/// Cap top to baseline for `size`.
pub fn cap_height(size: f32, script: Script) -> f32 {
    if !size.is_finite() || size <= 0.0 {
        return 0.0;
    }
    script.metrics().cap_height * size
}

/// Baseline to the visual bottom of a descender for `size`.
pub fn descender(size: f32, script: Script) -> f32 {
    if !size.is_finite() || size <= 0.0 {
        return 0.0;
    }
    script.metrics().descender * size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_is_a_major_third_anchored_at_fourteen() {
        // Every neighbouring pair is within a few percent of 1.25 — the scale
        // is a real geometric progression, not a list of round numbers.
        for w in LADDER.windows(2) {
            let ratio = w[1] / w[0];
            assert!(
                (1.20..1.30).contains(&ratio),
                "{:?} -> {:?} is {ratio:.3}",
                w[0],
                w[1]
            );
        }
        assert!(LADDER.contains(&14.0));
        // Every step's size is on the ladder.
        for s in Step::ALL {
            assert!(LADDER.contains(&s.size()), "{s:?} is off the ladder");
        }
    }

    #[test]
    fn nearest_ladder_step_rounds_in_log_space() {
        assert_eq!(nearest_ladder_step(11.0), 11.0);
        // The geometric mean of 18 and 22 is 19.9, so 19 rounds down and 20
        // rounds up — a *linear* nearest would send both to 18 and quietly
        // bias the whole scale toward its coarse end.
        assert_eq!(nearest_ladder_step(19.0), 18.0);
        assert_eq!(nearest_ladder_step(20.0), 22.0);
        assert_eq!(nearest_ladder_step(1000.0), 84.0);
        assert_eq!(nearest_ladder_step(1.0), 11.0);
        // Degenerate input never produces "no type".
        for bad in [f32::NAN, f32::INFINITY, -3.0, 0.0] {
            assert_eq!(nearest_ladder_step(bad), 11.0);
        }
    }

    #[test]
    fn the_generated_tracking_matches_the_published_ladder() {
        // Spec §5.1's table, to three decimals.
        let cases = [
            (11.0, false, 0.028),
            (14.0, false, 0.016),
            (18.0, false, 0.006),
            (22.0, false, 0.000),
            (27.0, false, -0.006),
            (34.0, false, -0.010),
            (43.0, false, -0.014),
            (53.0, false, -0.017),
            (67.0, false, -0.019),
            (84.0, false, -0.021),
            (11.0, true, 0.128),
        ];
        for (size, micro, want) in cases {
            let got = tracking_em(size, micro, Script::Latin);
            assert!(
                (got - want).abs() < 0.0006,
                "{size} lu micro={micro}: got {got:.4}, spec {want:.3}"
            );
        }
        // Tracking is bounded at both ends, so an absurd size cannot produce an
        // absurd advance.
        assert_eq!(tracking_em(0.001, false, Script::Latin), 0.140);
        // The upper clamp binds; the lower one is an asymptote the formula
        // approaches from above and never crosses.
        assert!((tracking_em(1e6, false, Script::Latin) + 0.0285).abs() < 1e-4);
        assert!(tracking_em(1e6, false, Script::Latin) >= -0.030);
    }

    #[test]
    fn the_generated_leading_matches_the_published_ladder() {
        for (size, want) in [
            (11.0, 1.62),
            (14.0, 1.52),
            (18.0, 1.41),
            (22.0, 1.32),
            (27.0, 1.23),
            (34.0, 1.13),
            (43.0, 1.03),
            (53.0, 0.94),
            (67.0, 0.94),
            (84.0, 0.94),
        ] {
            let got = line_height_ratio(size, Script::Latin);
            assert!(
                (got - want).abs() < 0.006,
                "{size} lu: got {got:.3}, spec {want:.2}"
            );
        }
        // Leading is clamped, so it never inverts.
        assert_eq!(line_height_ratio(1e6, Script::Latin), 0.94);
        assert_eq!(line_height_ratio(0.001, Script::Latin), 1.62);
    }

    #[test]
    fn cjk_takes_taller_lines_flatter_tracking_and_a_face_that_exists() {
        for size in LADDER {
            let latin = line_height_ratio(size, Script::Latin);
            let cjk = line_height_ratio(size, Script::Cjk);
            assert!(cjk > latin, "{size} lu: CJK leading must exceed Latin");
            assert!(cjk >= 1.20, "{size} lu: CJK leading floor");
        }
        // The Latin hero's 0.94 leading is exactly the collision the 1.18
        // multiplier and the 1.20 floor exist to prevent.
        assert!(line_height_ratio(67.0, Script::Cjk) >= 1.20);
        // Micro tracking collapses from +0.128 em to +0.040 em.
        assert_eq!(tracking_em(11.0, true, Script::Cjk), 0.040);
        assert_eq!(tracking_em(11.0, false, Script::Cjk), 0.0);
        // 600 and 650 are synthesised weights in Noto Sans SC; both map to 700.
        for w in [600, 650] {
            assert_eq!(weight_for_script(w, Script::Cjk), 700);
        }
        assert_eq!(weight_for_script(500, Script::Cjk), 500);
        assert_eq!(weight_for_script(650, Script::Latin), 650);
    }

    #[test]
    fn script_detection_is_conservative_about_mixed_strings() {
        assert_eq!(Script::of("Blue Monday"), Script::Latin);
        assert_eq!(Script::of(""), Script::Latin);
        assert_eq!(Script::of("蓝色星期一"), Script::Cjk);
        // One Han character in an otherwise Latin line is enough: the line's
        // leading is the maximum across its runs, and the Han run's is taller.
        assert_eq!(Script::of("Blue Monday — 蓝色星期一"), Script::Cjk);
        assert_eq!(Script::of("こんにちは"), Script::Cjk);
        assert_eq!(Script::of("안녕하세요"), Script::Cjk);
        // Cyrillic and Greek are Latin-metric, not CJK.
        assert_eq!(Script::of("Пример"), Script::Latin);
        assert_eq!(Script::of("Παράδειγμα"), Script::Latin);
        // Emoji do not make a string CJK.
        assert_eq!(Script::of("hello 🎵"), Script::Latin);
    }

    #[test]
    fn micro_labels_uppercase_latin_and_leave_han_alone() {
        assert_eq!(micro_case("monday"), "MONDAY");
        assert_eq!(micro_case("NOW PLAYING"), "NOW PLAYING");
        assert_eq!(micro_case("正在播放"), "正在播放");
        // No allocation when nothing changes.
        assert!(matches!(
            micro_case("NOW PLAYING"),
            std::borrow::Cow::Borrowed(_)
        ));
        assert!(matches!(
            micro_case("正在播放"),
            std::borrow::Cow::Borrowed(_)
        ));
        assert_eq!(micro_case(""), "");
    }

    #[test]
    fn the_weight_ladder_steps_where_the_spec_says_it_does() {
        assert_eq!(weight_for(11.0), 500);
        assert_eq!(weight_for(14.0), 500);
        assert_eq!(weight_for(17.9), 500);
        assert_eq!(weight_for(18.0), 600);
        assert_eq!(weight_for(42.0), 600);
        assert_eq!(weight_for(43.0), 650);
        assert_eq!(weight_for(52.0), 650);
        assert_eq!(weight_for(53.0), 700);
        assert_eq!(weight_for(f32::NAN), 500);
    }

    #[test]
    fn the_hierarchy_budget_holds_between_the_rows_a_card_actually_stacks() {
        // Spec §5.1: between adjacent rows, at least two of {size, weight,
        // case} differ. These are the three-row blocks the cards build.
        for block in [
            [Step::Micro, Step::HeroXl, Step::Body],
            [Step::Micro, Step::Title, Step::Body],
            [Step::Micro, Step::HeroS, Step::Caption],
        ] {
            for pair in block.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let differ = u8::from(a.size() != b.size())
                    + u8::from(a.weight() != b.weight())
                    + u8::from(a.is_micro_label() != b.is_micro_label());
                assert!(differ >= 2, "{a:?} vs {b:?} differ in only {differ}");
            }
        }
    }

    #[test]
    fn vertical_metrics_are_per_script_and_never_negative() {
        let l = Script::Latin.metrics();
        let c = Script::Cjk.metrics();
        // The ideographic face has far less dead band above its glyphs and a
        // taller effective cap.
        assert!(c.cap_gap < l.cap_gap);
        assert!(c.cap_height > l.cap_height);
        assert!(c.descender < l.descender);
        for size in [0.0, -1.0, f32::NAN, 11.0, 84.0] {
            for s in [Script::Latin, Script::Cjk] {
                assert!(cap_gap(size, s) >= 0.0 && cap_gap(size, s).is_finite());
                assert!(cap_height(size, s) >= 0.0 && cap_height(size, s).is_finite());
                assert!(descender(size, s) >= 0.0 && descender(size, s).is_finite());
            }
        }
        // The worked example from spec §6.2: a 64 lu hero would acquire 16 lu
        // of invisible band above it if a card laid out to em boxes.
        assert!((cap_gap(64.0, Script::Latin) - 16.0).abs() < 0.01);
    }

    #[test]
    fn a_built_run_carries_the_generated_values() {
        let mut fonts = super::super::text::FontStack::from_font_data("en-US", []);
        let r = step_run("monday · 28 july", Step::Micro, &mut fonts);
        assert_eq!(r.text, "MONDAY · 28 JULY");
        assert_eq!(r.weight, 600);
        assert!((r.letter_spacing / r.size - 0.128).abs() < 0.001);
        assert!((r.line_height / r.size - 1.62).abs() < 0.01);

        let h = step_run("09:41", Step::HeroXl, &mut fonts);
        assert_eq!(h.weight, 700);
        assert!(h.letter_spacing < 0.0, "the hero is set tight");
        assert!((h.line_height / h.size - 0.94).abs() < 0.01);

        // A CJK title takes 700 rather than the synthesised 600.
        let c = step_run("春风十里", Step::Title, &mut fonts);
        assert_eq!(c.weight, 700);
        assert!(c.line_height / c.size >= 1.20);

        // Degenerate sizes produce a drawable run rather than a poisoned one.
        for bad in [0.0, -5.0, f32::NAN] {
            let r = run("x", bad, &mut fonts);
            assert!(r.size > 0.0 && r.line_height.is_finite());
        }
        // An empty string is legal and measures to nothing later.
        assert_eq!(run("", 14.0, &mut fonts).text, "");
    }
}

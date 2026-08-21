//! Synced-lyric engine: `.lrc` parsing and line selection
//! (WIDGETS_ROADMAP W1/W5).
//!
//! Pure functions over lyric text and playback offsets — no I/O, no globals, no
//! desktop — so every rule (multi-timestamp lines, `[offset:]`, hostile file
//! content) is unit-testable, and the module stays platform-neutral. Nothing
//! here touches mpv, D-Bus or the filesystem: the daemon reads the file and owns
//! the clock, this module only answers *which line*.
//!
//! # The two clocks
//!
//! [`next_change_after`] is what makes Smart Sleep possible. `.lrc` timestamps
//! are known ahead of time, so the next visual change is a *known instant*: the
//! daemon waits on an interruptible deadline until it, instead of polling. A
//! 30s instrumental gap must cost one wake, not 300 — so this function being
//! exactly right is a power requirement, not a nicety.
//!
//! # What used to be here
//!
//! This module also generated the widget's ASS payload: an override block per
//! line, an escape pass over every string that reached it, a leading
//! calculation that existed only because ASS has no line-height property, and a
//! `\pos` per line to work around it. The widget rasterises now —
//! `crate::widgetkit::cards::nowplaying` draws it and
//! `crate::daemon::widgets` places it — so all of that is gone, and with it the
//! escape class that made hostile lyric text dangerous in the first place.
//!
//! What survived the move is what was never about ASS: the parser, the line
//! selection, [`Anchor`] (which is still how a user says *where*), and
//! [`PLAY_RES_X`]×[`PLAY_RES_Y`], which is still the coordinate space every
//! widget's logical units are quoted in.
//!
//! # Untrusted input
//!
//! Lyric text comes from third-party `.lrc` files. There is no longer a markup
//! language for it to escape into, but the parser still must never panic or
//! unwrap on file content, and a hostile timestamp must not become a nonsense
//! deadline — a `.lrc` is the one input to this widget that nobody reviewed.

use serde::{Deserialize, Serialize};

/// One timed lyric line: the moment it becomes current, and what to show.
///
/// `at` is in seconds from the start of the track and may be **negative** after
/// a large `[offset:]` — see [`parse_lrc`]. An empty `text` is meaningful and
/// deliberately preserved: `.lrc` files use timed blank lines to mark
/// instrumental gaps, and the daemon should clear the overlay there rather than
/// leave the previous line hanging.
#[derive(Debug, Clone, PartialEq)]
pub struct LrcLine {
    /// Seconds from the start of the track.
    pub at: f64,
    /// The line to display, already stripped of its timestamp tags.
    pub text: String,
}

/// Parse an `.lrc` file body into timed lines, sorted ascending by `at`.
///
/// Handles the format as it exists in the wild rather than as specified:
///
/// - `[mm:ss]`, `[mm:ss.x]`, `[mm:ss.xx]`, `[mm:ss.xxx]`, and the `[mm:ss:xx]`
///   variant some older tools write (LRC has no hours field, so a third
///   numeric component is always a fraction).
/// - Several timestamps on one line — `[00:12.00][01:05.00] chorus` yields two
///   entries sharing the text, which is how repeated choruses are encoded.
/// - Metadata tags (`[ar:]`, `[ti:]`, `[al:]`, `[by:]`, `[length:]`, …) are not
///   lyric lines and are dropped.
/// - A UTF-8 BOM, CRLF, bare-CR and blank lines.
///
/// `[offset:±ms]` shifts every timestamp. **A positive offset makes lyrics
/// appear earlier** (`at -= offset`), which is the de-facto convention the tag
/// was introduced with — "+ shifts the lyrics up". Note this is the *opposite*
/// sign from a user-facing sync slider, where positive naturally reads as
/// "later"; the daemon applies its own correction on top and must not assume
/// the two agree.
///
/// Malformed lines are skipped, never fatal: a single bad timestamp in a
/// stranger's file must not cost the user the other 40 lines.
pub fn parse_lrc(src: &str) -> Vec<LrcLine> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    // `[offset:]` conventionally sits in the header but is not required to, and
    // it applies to every timestamp — so resolve it before shifting anything.
    let offset = find_offset_secs(src);

    let mut out: Vec<LrcLine> = Vec::new();
    for raw in split_lines(src) {
        parse_lrc_line(raw, offset, &mut out);
    }
    // Stable sort: lines sharing a timestamp keep their file order, which is
    // what a two-voice file expects.
    out.sort_by(|a, b| a.at.total_cmp(&b.at));
    out
}

/// Split on LF, CRLF and bare CR. `str::lines` covers the first two; old
/// Mac-style `.lrc` files in the wild still use the third.
fn split_lines(src: &str) -> impl Iterator<Item = &str> {
    src.split(['\n', '\r'])
}

/// Pull the leading `[...]` tags off one line and push its lyric entries.
fn parse_lrc_line(line: &str, offset: f64, out: &mut Vec<LrcLine>) {
    let mut rest = line.trim_start();
    let mut stamps: Vec<f64> = Vec::new();

    // Only *directly adjacent* brackets are consumed as tags. Whitespace ends
    // the run on purpose: `[00:12.00] [Chorus]` is a section marker inside the
    // lyric, and eating it would silently delete words from the display.
    while let Some(after_open) = rest.strip_prefix('[') {
        let Some(close) = after_open.find(']') else {
            // Unterminated bracket — the line is not trustworthy, drop it.
            return;
        };
        if let Some(secs) = parse_timestamp(&after_open[..close]) {
            stamps.push(secs);
        }
        // Anything else in a leading bracket is metadata; ignore it.
        rest = &after_open[close + 1..];
    }

    if stamps.is_empty() {
        return; // Metadata-only line, blank line, or plain prose.
    }
    let text = rest.trim();
    for at in stamps {
        out.push(LrcLine {
            at: at - offset,
            text: text.to_string(),
        });
    }
}

/// `mm:ss`, `mm:ss.xx` or `mm:ss:xx` → seconds. None for anything else, which
/// is also how metadata tags are told apart from timestamps.
fn parse_timestamp(tag: &str) -> Option<f64> {
    let (mm, rest) = tag.split_once(':')?;
    let minutes = parse_digits(mm)?;
    let (ss, frac) = match rest.find(['.', ':']) {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };
    let seconds = parse_digits(ss)?;
    if seconds >= 60 {
        return None; // Minutes may exceed 59 in LRC; seconds may not.
    }
    // checked_*: a 20-digit minute field parses fine as u64 and would overflow.
    let whole = minutes.checked_mul(60)?.checked_add(seconds)?;
    let frac = if frac.is_empty() {
        0.0
    } else {
        parse_fraction(frac)?
    };
    Some(whole as f64 + frac)
}

/// An all-ASCII-digit unsigned field. Rejects signs, spaces and empty input so
/// `parse` cannot accept things the format does not allow.
fn parse_digits(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Digits after the separator, scaled by their count: `.5` = 0.5s, `.05` =
/// 0.05s, `.050` = 0.05s. Capped at 6 digits so a garbage field cannot produce
/// a nonsense divisor.
fn parse_fraction(s: &str) -> Option<f64> {
    if s.len() > 6 {
        return None;
    }
    let v = parse_digits(s)?;
    Some(v as f64 / 10f64.powi(s.len() as i32))
}

/// Seconds to subtract from every timestamp, from the first parsable
/// `[offset:±ms]` tag. Zero when absent or unreadable.
fn find_offset_secs(src: &str) -> f64 {
    for line in split_lines(src) {
        let Some(after_open) = line.trim_start().strip_prefix('[') else {
            continue;
        };
        let Some(close) = after_open.find(']') else {
            continue;
        };
        let Some((key, value)) = after_open[..close].split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("offset") {
            continue;
        }
        // An explicit `+` is idiomatic in this tag but not accepted by `parse`.
        let value = value.trim();
        let value = value.strip_prefix('+').unwrap_or(value);
        if let Ok(ms) = value.parse::<i64>() {
            return ms as f64 / 1000.0;
        }
    }
    0.0
}

/// Index of the line that should be on screen at `t` seconds — the last line
/// whose timestamp is at or before `t`. None before the first line.
///
/// Binary search rather than a scan: this is called from the daemon's tick on
/// every output, and a linear pass over a long file is work we can simply not
/// do. `lines` must come from [`parse_lrc`] (or otherwise be sorted ascending).
pub fn line_at(lines: &[LrcLine], t: f64) -> Option<usize> {
    // partition_point is exact on the boundary: `at == t` counts as reached, so
    // a line becomes current on its own timestamp rather than a tick later.
    lines.partition_point(|l| l.at <= t).checked_sub(1)
}

/// Timestamp of the next line strictly after `t`, or None once `t` is at or
/// past the last line.
///
/// This is the Smart Sleep deadline: the daemon waits until exactly this
/// instant instead of polling. "Strictly after" matters — returning `t` itself
/// when called *on* a line boundary would produce a zero-length sleep and spin
/// the loop, which is the failure this whole design exists to avoid.
pub fn next_change_after(lines: &[LrcLine], t: f64) -> Option<f64> {
    lines
        .get(lines.partition_point(|l| l.at <= t))
        .map(|l| l.at)
}

/// Nine-point placement grid, mirroring `config::LyricAnchor` so the two
/// serialise to the same TOML strings (`"topleft"`, `"midcenter"`, …).
///
/// Anchors rather than coordinates: an anchor stays correct when the
/// resolution, orientation or output changes, where a pixel position quietly
/// ends up off-screen. Each maps to an ASS `\an` value on the numpad layout —
/// `7 8 9` across the top, `1 2 3` across the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    /// `\an7`
    TopLeft,
    /// `\an8`
    TopCenter,
    /// `\an9`
    TopRight,
    /// `\an4`
    MidLeft,
    /// `\an5`
    MidCenter,
    /// `\an6`
    MidRight,
    /// `\an1`
    BottomLeft,
    /// `\an2` — the default: where subtitles have always gone, and the strip of
    /// desktop least likely to be hidden by a window or covered in icons.
    #[default]
    BottomCenter,
    /// `\an3`
    BottomRight,
}

impl Anchor {
    /// Every anchor, in reading order — for populating a GUI grid without
    /// hand-listing the variants a tenth time.
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::TopCenter,
        Anchor::TopRight,
        Anchor::MidLeft,
        Anchor::MidCenter,
        Anchor::MidRight,
        Anchor::BottomLeft,
        Anchor::BottomCenter,
        Anchor::BottomRight,
    ];

    /// The ASS `\an` value: 1–9 on the numpad layout.
    pub const fn an(self) -> u8 {
        match self {
            Anchor::BottomLeft => 1,
            Anchor::BottomCenter => 2,
            Anchor::BottomRight => 3,
            Anchor::MidLeft => 4,
            Anchor::MidCenter => 5,
            Anchor::MidRight => 6,
            Anchor::TopLeft => 7,
            Anchor::TopCenter => 8,
            Anchor::TopRight => 9,
        }
    }
}

/// Width of the reference coordinate space every widget's logical units are
/// quoted in.
///
/// Also what a caller must pass as `res_x` to mpv's `osd-overlay` on the one
/// path that still emits ASS. See the module docs.
pub const PLAY_RES_X: u32 = 1920;

/// Height of the reference coordinate space. Type sizes and margins are
/// therefore "pixels at 1080p", scaling proportionally on any other output —
/// which is what a wallpaper overlay wants, unlike a subtitle. See
/// `crate::widgetkit::scale_for_output`, which resolves the ratio.
pub const PLAY_RES_Y: u32 = 1080;

#[cfg(test)]
mod tests {
    use super::*;

    /// Compact view of a parse result, so assertions read like the file does.
    fn pairs(src: &str) -> Vec<(f64, String)> {
        parse_lrc(src).into_iter().map(|l| (l.at, l.text)).collect()
    }

    #[test]
    fn parses_the_shapes_real_files_use() {
        // Fraction width varies by authoring tool: none, tenths, hundredths and
        // milliseconds all appear, and all must land on the same time scale.
        let got = pairs("[00:01]a\n[00:02.5]b\n[00:03.25]c\n[00:04.125]d\n[01:00.00]e");
        assert_eq!(
            got,
            vec![
                (1.0, "a".into()),
                (2.5, "b".into()),
                (3.25, "c".into()),
                (4.125, "d".into()),
                (60.0, "e".into()),
            ]
        );
        // `[mm:ss:xx]` is the older separator; LRC has no hours field, so the
        // third component can only be a fraction.
        assert_eq!(pairs("[00:03:25]c"), vec![(3.25, "c".into())]);
        assert!(parse_lrc("").is_empty());
    }

    #[test]
    fn multiple_timestamps_share_one_text() {
        // This is how every real file encodes a repeated chorus. Getting it
        // wrong loses the repeat silently — the file still parses.
        let got = pairs("[00:12.00][01:05.00][02:30.50] chorus");
        assert_eq!(
            got,
            vec![
                (12.0, "chorus".into()),
                (65.0, "chorus".into()),
                (150.5, "chorus".into()),
            ]
        );
    }

    #[test]
    fn output_is_sorted_even_when_the_file_is_not() {
        // Multi-timestamp lines interleave by construction, so the parser has
        // to sort; downstream binary search is wrong on unsorted input.
        let got = pairs("[00:30.00][00:10.00] a\n[00:20.00] b");
        assert_eq!(
            got,
            vec![(10.0, "a".into()), (20.0, "b".into()), (30.0, "a".into()),]
        );
    }

    #[test]
    fn metadata_tags_are_not_lyrics() {
        // A header parsed as a lyric would display "Some Artist" at 00:00.
        let src = "[ar:Some Artist]\n[ti:A Song]\n[al:An Album]\n\
                   [by:someone]\n[length:03:21]\n[re:tool]\n[ve:1.0]\n[00:05.00]first";
        assert_eq!(pairs(src), vec![(5.0, "first".into())]);
    }

    #[test]
    fn offset_moves_lyrics_earlier_for_positive_values() {
        // The tag's own convention: "+ shifts the lyrics up", i.e. sooner. This
        // is the opposite of a user-facing sync slider, so it is worth pinning.
        assert_eq!(
            pairs("[offset:+250]\n[00:10.00]a"),
            vec![(9.75, "a".into())]
        );
        assert_eq!(pairs("[offset:250]\n[00:10.00]a"), vec![(9.75, "a".into())]);
        assert_eq!(
            pairs("[offset:-500]\n[00:10.00]a"),
            vec![(10.5, "a".into())]
        );
        // The tag applies wherever it appears, not only in the header.
        assert_eq!(pairs("[00:10.00]a\n[offset:1000]"), vec![(9.0, "a".into())]);
        // A big offset may push a line before zero. Kept rather than clamped:
        // `line_at` already treats it as "current from the start", and
        // clamping would collapse distinct lines onto the same instant.
        assert_eq!(
            pairs("[offset:5000]\n[00:01.00]a"),
            vec![(-4.0, "a".into())]
        );
        // Unreadable offsets must not poison the file.
        assert_eq!(pairs("[offset:abc]\n[00:10.00]a"), vec![(10.0, "a".into())]);
    }

    #[test]
    fn malformed_input_is_skipped_never_fatal() {
        // One bad line in a stranger's file must not cost the user the rest.
        let src = "[00:xx.00]bad minutes\n\
                   [ab:cd]not a time\n\
                   []empty\n\
                   [00:60.00]second overflow\n\
                   [00:12.00 unterminated\n\
                   [-1:00.00]negative\n\
                   [ 00:12.00]padded\n\
                   [99999999999999999999:00.00]overflow\n\
                   [00:12.0000000]absurd fraction\n\
                   plain prose with no timestamp\n\
                   [00:15.00]good";
        assert_eq!(pairs(src), vec![(15.0, "good".into())]);
    }

    #[test]
    fn bom_crlf_and_bare_cr_are_transparent() {
        // Files come from Windows tools and from decade-old Mac ones; the BOM
        // would otherwise attach to the first `[` and break the whole file.
        let got = pairs("\u{feff}[ti:x]\r\n[00:01.00]a\r\n\r\n[00:02.00]b\r");
        assert_eq!(got, vec![(1.0, "a".into()), (2.0, "b".into())]);
        assert_eq!(
            pairs("\u{feff}[00:01.00]a\r[00:02.00]b"),
            vec![(1.0, "a".into()), (2.0, "b".into())]
        );
    }

    #[test]
    fn timed_blank_lines_survive_as_gap_markers() {
        // A timed empty line means "instrumental — clear the overlay". Dropping
        // it would leave the previous lyric on screen through the whole break.
        let got = pairs("[00:01.00]a\n[00:20.00]\n[00:40.00]b");
        assert_eq!(
            got,
            vec![(1.0, "a".into()), (20.0, String::new()), (40.0, "b".into()),]
        );
    }

    #[test]
    fn brackets_inside_the_lyric_are_left_alone() {
        // `[Chorus]` section markers are part of the displayed text. They are
        // only safe because the tag run stops at the first non-bracket char.
        assert_eq!(
            pairs("[00:01.00] [Chorus] sing"),
            vec![(1.0, "[Chorus] sing".into())]
        );
    }

    fn fixture() -> Vec<LrcLine> {
        parse_lrc("[00:10.00]a\n[00:20.00]b\n[00:30.00]c")
    }

    #[test]
    fn line_at_covers_before_first_boundary_and_past_last() {
        let l = fixture();
        assert_eq!(line_at(&l, 0.0), None); // before the first line: show nothing
        assert_eq!(line_at(&l, 9.999), None);
        assert_eq!(line_at(&l, 10.0), Some(0)); // exact boundary is inclusive:
        assert_eq!(line_at(&l, 19.999), Some(0)); // a line is current on its own
        assert_eq!(line_at(&l, 20.0), Some(1)); // timestamp, not a tick later
        assert_eq!(line_at(&l, 30.0), Some(2));
        assert_eq!(line_at(&l, 9_999.0), Some(2)); // last line holds to the end
        assert_eq!(line_at(&[], 5.0), None); // empty file must not panic
    }

    #[test]
    fn next_change_after_is_strict_at_a_boundary() {
        // The one that matters: called *on* a line's timestamp it must return
        // the following line. Returning `t` would make the daemon's
        // wait_timeout zero and turn Smart Sleep into a spin loop.
        let l = fixture();
        assert_eq!(next_change_after(&l, 10.0), Some(20.0));
        assert_eq!(next_change_after(&l, 19.999), Some(20.0));
        assert_eq!(next_change_after(&l, 0.0), Some(10.0)); // before the first
        assert_eq!(next_change_after(&l, 30.0), None); // nothing left to wake for
        assert_eq!(next_change_after(&l, 31.0), None);
        assert_eq!(next_change_after(&[], 0.0), None);
        // Duplicate timestamps must not make it return one of themselves.
        let dup = parse_lrc("[00:10.00]a\n[00:10.00]b\n[00:20.00]c");
        assert_eq!(next_change_after(&dup, 10.0), Some(20.0));
    }

    #[test]
    fn selection_matches_a_linear_scan_over_a_long_file() {
        // The binary search is an optimisation over the obvious loop, so assert
        // it against the obvious loop rather than against hand-picked cases.
        let mut src = String::new();
        for i in 0..600 {
            src.push_str(&format!("[{:02}:{:02}.50]line {i}\n", i / 60, i % 60));
        }
        let lines = parse_lrc(&src);
        assert_eq!(lines.len(), 600);
        for step in 0..1_300 {
            let t = f64::from(step) * 0.5;
            let expect = lines.iter().rposition(|l| l.at <= t);
            assert_eq!(line_at(&lines, t), expect, "line_at at t={t}");
            let expect_next = lines.iter().find(|l| l.at > t).map(|l| l.at);
            assert_eq!(next_change_after(&lines, t), expect_next, "next at t={t}");
        }
    }

    #[test]
    fn anchors_map_to_the_numpad_layout() {
        // `\an` is numpad-ordered, which is not the reading order the variants
        // are written in — the easiest thing in this file to get quietly wrong.
        let table = [
            (Anchor::TopLeft, 7),
            (Anchor::TopCenter, 8),
            (Anchor::TopRight, 9),
            (Anchor::MidLeft, 4),
            (Anchor::MidCenter, 5),
            (Anchor::MidRight, 6),
            (Anchor::BottomLeft, 1),
            (Anchor::BottomCenter, 2),
            (Anchor::BottomRight, 3),
        ];
        for (anchor, an) in table {
            assert_eq!(anchor.an(), an, "{anchor:?}");
        }
        // ALL must stay in step with the enum, and cover 1..=9 exactly once.
        assert_eq!(Anchor::ALL.len(), table.len());
        let mut seen: Vec<u8> = Anchor::ALL.iter().map(|a| a.an()).collect();
        seen.sort_unstable();
        assert_eq!(seen, (1..=9).collect::<Vec<u8>>());
        assert_eq!(Anchor::default(), Anchor::BottomCenter);
    }

    #[test]
    fn end_to_end_over_a_realistic_file() {
        // The daemon's actual sequence: parse, pick the line, and ask when to
        // wake up next.
        let src = "\u{feff}[ti:Test]\r\n[ar:Nobody]\r\n[offset:+100]\r\n\
                   [00:00.00]\r\n[00:12.50]first line\r\n[00:18.00][01:00.00]chorus\r\n\
                   [00:24.00]\r\n[01:30.00]last\r\n";
        let lines = parse_lrc(src);
        assert_eq!(lines.len(), 6);
        let i = line_at(&lines, 19.0).expect("a line is current at 19s");
        assert_eq!(lines[i].text, "chorus");
        assert_eq!(next_change_after(&lines, 19.0), Some(23.9));
        // The gap marker clears the overlay rather than holding "chorus".
        let gap = line_at(&lines, 25.0).expect("the timed blank line");
        assert!(lines[gap].text.is_empty());
        assert_eq!(next_change_after(&lines, 90.0), None);
    }
}

/// Design-review dump: the now-playing card, as PNGs.
///
/// `#[ignore]`d and env-gated, exactly as its ASS ancestor was: a developer
/// harness must never fail on a machine that has no such directory, which is
/// every CI runner. What changed is the artefact — the old one wrote a payload
/// to be rasterised through libass by hand; this writes the picture.
///
/// ```text
/// LYRICS_DUMP=/tmp/lyrics cargo test --all-features -- --ignored lyrics::scratch_dump
/// ```
#[cfg(all(test, feature = "daemon"))]
mod scratch_dump {
    use crate::widgetkit::{cards, Canvas, FontStack, Mode, NowPlayingData, Theme};

    #[test]
    #[ignore]
    fn dump() {
        let dir = std::env::var("LYRICS_DUMP").unwrap_or_default();
        if dir.is_empty() {
            return;
        }
        std::fs::create_dir_all(&dir).expect("$LYRICS_DUMP must be writable");
        let mut fonts = FontStack::system();
        // The four states this card actually has, which is the whole reason it
        // is a layout pass rather than a template: everything, no lyric, no
        // header, and a line long enough to wrap and then ellipsise.
        let cases: [(&str, NowPlayingData); 4] = [
            (
                "full",
                NowPlayingData {
                    label: "Now playing",
                    title: "Nightcall",
                    artist: "Kavinsky",
                    album: "OutRun",
                    lyric: "I'm giving you a night call to tell you how I feel",
                    next_lyric: "I want to drive you through the night",
                    font_size: 28.0,
                    accent_follow: true,
                    screen_width: 1920.0,
                    ..NowPlayingData::default()
                },
            ),
            (
                "header-only",
                NowPlayingData {
                    label: "Now playing",
                    title: "Nightcall",
                    artist: "Kavinsky",
                    font_size: 28.0,
                    screen_width: 1920.0,
                    ..NowPlayingData::default()
                },
            ),
            (
                "lyric-only",
                NowPlayingData {
                    lyric: "I'm giving you a night call",
                    font_size: 28.0,
                    screen_width: 1920.0,
                    ..NowPlayingData::default()
                },
            ),
            (
                "overlong",
                NowPlayingData {
                    label: "Now playing",
                    title: "A title long enough that it has to be cut somewhere",
                    artist: "An artist, a second artist, a third artist",
                    lyric: "A lyric line long enough to wrap onto a second line and then \
                            run past the end of that one too",
                    next_lyric: "and the preview under it",
                    font_size: 28.0,
                    accent_follow: true,
                    screen_width: 1920.0,
                    ..NowPlayingData::default()
                },
            ),
        ];
        for (name, data) in cases {
            for (mode, tag) in [(Mode::Dark, "dark"), (Mode::Light, "light")] {
                for &scale in &[1.0_f32, 2.0] {
                    let t = Theme::for_accent(mode, crate::config::Accent::Blue);
                    let size = cards::nowplaying::measure(&mut fonts, &t, &data, scale);
                    let mut canvas =
                        Canvas::for_logical(size.buffer(), scale).expect("a card-sized canvas");
                    canvas.reset();
                    cards::nowplaying::draw(&mut canvas, &mut fonts, &t, &data);
                    canvas
                        .save_png(format!("{dir}/{name}_{tag}_{}x.png", scale as u32))
                        .expect("png");
                }
            }
        }
    }
}

//! Synced-lyric engine: `.lrc` parsing, line selection and ASS generation
//! (WIDGETS_ROADMAP W1/W5).
//!
//! Pure functions over lyric text and playback offsets — no I/O, no globals, no
//! desktop — so every rule (multi-timestamp lines, `[offset:]`, the untrusted
//! text that ends up inside an ASS override block) is unit-testable, and the
//! module stays platform-neutral. Nothing here touches mpv, D-Bus or the
//! filesystem: the daemon reads the file and owns the clock, this module only
//! answers *which line* and *what markup*.
//!
//! # The two clocks
//!
//! [`next_change_after`] is what makes Smart Sleep possible. `.lrc` timestamps
//! are known ahead of time, so the next visual change is a *known instant*: the
//! daemon waits on an interruptible deadline until it, instead of polling. A
//! 30s instrumental gap must cost one wake, not 300 — so this function being
//! exactly right is a power requirement, not a nicety.
//!
//! # The mpv contract
//!
//! [`render_ass`] emits the `Text` field of ASS dialogue events, which is what
//! mpv's `osd-overlay` command consumes with `format: "ass-events"`. Positions
//! are expressed in a fixed [`PLAY_RES_X`]×[`PLAY_RES_Y`] coordinate space, so
//! **the caller must pass `res_x: PLAY_RES_X` and `res_y: PLAY_RES_Y`** to
//! `osd-overlay`. libass maps that space onto the real output proportionally,
//! which is what we want on a wallpaper: the overlay keeps its proportions on a
//! 4K screen instead of shrinking to a caption.
//!
//! mpv splits the payload on real newlines and turns each piece into a separate
//! event. One line of text is therefore one event with no newline in it, which
//! is what every caller that builds on top of the base block (`render_ass("",
//! …)`) depends on. A *stack* of lines is the deliberate exception — see below.
//!
//! # Leading, and what ASS cannot do
//!
//! **ASS has no line-height property.** There is no `\linespacing` override and
//! no per-event equivalent of mpv's global `sub-line-spacing`. Inside one event
//! the gap between `\N`-separated lines is whatever libass derives from the
//! font metrics of the largest run on each line — for Inter that lands near
//! 1.05–1.10em, which is a tight setting borrowed from body copy and reads
//! cramped on a wallpaper, especially once the type is large.
//!
//! The only way to *choose* the leading is to stop using `\N` and give each
//! line its own event with its own `\pos`. That is what [`render_ass_line`]
//! does: [`leading_px`] turns the type size into a line-box pitch
//! ([`LEADING_PCT`] of the size, ~1.25 — measured against 1.15 and 1.35 over
//! video, and the tightest of the three that still reads as set rather than
//! stacked), and the stack grows away from the anchored edge so a
//! bottom-anchored block still clears the margin.
//!
//! Two things this still cannot control, and callers should not expect:
//!
//! * **The gap is uniform.** It comes from `LyricStyle::size_pt`, not from the
//!   per-run `\fs` of each line, so a stack mixing sizes gets an even rhythm
//!   rather than optically fitted gaps. That is the better default for a
//!   heading/body/caption stack, and it is not a knob.
//! * **Anything still joined by `\N` inside one event keeps libass's spacing.**
//!   A caller that hand-builds a multi-line payload (as the daemon's lyric
//!   runtime does for its header and its dimmed next-line preview) has to move
//!   to [`render_ass_line`] to get leading control; the base block alone cannot
//!   give it to them.
//!
//! # Untrusted input
//!
//! Lyric text comes from third-party `.lrc` files. Inside an ASS event, `{`
//! opens an override block and `\` opens an escape — so unescaped lyric text
//! can move, recolour or hide the overlay, or inject `\N` line breaks. Every
//! string that reaches the payload goes through [`ass_escape`] (text),
//! [`hex_to_ass_colour`] (colours) or the font sanitiser, and parsing never
//! panics or unwraps on file content.

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

/// Make arbitrary text safe to place inside an ASS event.
///
/// Lyric text is untrusted third-party input and lands directly in the overlay
/// payload, so it must not be able to open an override block or an escape:
///
/// - `{` and `}` become `\{` and `\}` (libass renders those as literal braces).
/// - A backslash keeps its glyph but is followed by U+2060 WORD JOINER, a
///   zero-width character that breaks the escape sequence before libass can
///   read `\N`, `\h` or `\{`. This is the same trick mpv uses for its own OSD
///   text; ASS has no `\\` escape, so there is no cleaner option.
/// - Newlines (LF, CRLF, CR) become `\N`, an ASS hard line break — a raw
///   newline would be interpreted by mpv as the start of a *second* event.
pub fn ass_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next(); // CRLF is one break, not two.
                }
                out.push_str("\\N");
            }
            '\n' => out.push_str("\\N"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\\' => {
                out.push('\\');
                out.push('\u{2060}');
            }
            _ => out.push(c),
        }
    }
    out
}

/// `#RRGGBB` → `&HBBGGRR&`, the byte order ASS actually uses.
///
/// ASS colours are little-endian BGR, so the channels come out reversed from
/// every other colour the codebase carries (`theme.rs` and the config both
/// speak `#RRGGBB`). Accepts an optional `#`, three-digit shorthand (`#f80`),
/// surrounding whitespace and either case.
///
/// Returns white on anything unparsable rather than failing: a mistyped colour
/// in a hand-edited config should cost the user their tint, not their lyrics.
pub fn hex_to_ass_colour(hex: &str) -> String {
    match parse_hex_rgb(hex) {
        Some((r, g, b)) => format!("&H{b:02X}{g:02X}{r:02X}&"),
        None => ASS_WHITE.to_string(),
    }
}

/// The fallback [`hex_to_ass_colour`] returns for unusable input.
const ASS_WHITE: &str = "&HFFFFFF&";

/// `#RGB` / `#RRGGBB` → `(r, g, b)`.
fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match h.len() {
        // Shorthand doubles each nibble, so `f` is 0xFF and not 0xF0.
        3 => {
            let v = u16::from_str_radix(h, 16).ok()?;
            let nib = |shift: u32| (((v >> shift) & 0xF) as u8) * 0x11;
            Some((nib(8), nib(4), nib(0)))
        }
        6 => {
            let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
            Some((byte(0)?, byte(2)?, byte(4)?))
        }
        _ => None,
    }
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

/// Width of the ASS coordinate space [`render_ass`] positions text in.
///
/// The caller must pass this as `res_x` to mpv's `osd-overlay`, or the
/// horizontal placement will be wrong. See the module docs.
pub const PLAY_RES_X: u32 = 1920;

/// Height of the ASS coordinate space [`render_ass`] positions text in.
///
/// The caller must pass this as `res_y` to mpv's `osd-overlay`. Type sizes and
/// margins are therefore "pixels at 1080p", scaling proportionally on any other
/// output — which is what a wallpaper overlay wants, unlike a subtitle.
pub const PLAY_RES_Y: u32 = 1080;

/// Smallest type size rendered; below this the outline swallows the glyphs.
const MIN_SIZE_PT: u32 = 8;
/// Largest type size rendered — a guard against a hand-edited config, not a
/// design limit.
const MAX_SIZE_PT: u32 = 400;

/// Line-box pitch as a percentage of the type size — the leading
/// [`render_ass_line`] stacks with. See the module docs for why this is a
/// choice we have to make ourselves.
///
/// 125% is display leading: looser than the ~105–110% libass derives from
/// Inter's own metrics for `\N`, tighter than the 135% at which two lines stop
/// reading as one block. Both neighbours were rendered over video and looked
/// at before this number was picked.
pub const LEADING_PCT: u32 = 125;

/// Outline thickness as a percentage of the type size.
///
/// The outline is the only thing keeping light text readable on a light frame,
/// so it cannot be thin — but it is also what turns large type into a blob,
/// because it grows *into* the counters of the glyphs rather than away from
/// them. 6% is the setting where 90pt display type keeps open counters over a
/// bright sky; the previous 1/12 (8.3%) closed them.
const BORD_PCT: u32 = 6;

/// Drop-shadow offset as a percentage of the type size. Half the outline: it
/// is there to detach the text from a *busy* background of roughly its own
/// brightness, where an outline alone reads as part of the pattern.
const SHAD_PCT: u32 = 3;

/// Thickest outline drawn. Only reachable above ~530pt after the size clamp, so
/// it is a guard rather than a design limit — unlike the ceiling it replaced,
/// which capped at 8px and quietly stopped being proportional at 96pt.
const MAX_BORD_PX: u32 = 32;

/// The OpenType weight [`LyricStyle::bold`] asks for.
///
/// **Not `\b1`.** libass reads `\b0` as normal, `\b1` as bold, and any value
/// from 100 up as an exact weight it passes to fontconfig — verified against
/// libass 0.17.1, where `\b600`, `\b500` and `\b700` render three visibly
/// different faces of Inter and `\b1` is indistinguishable from `\b700`.
///
/// 600 (SemiBold) rather than 700 because 700 over moving video is the weight
/// that reads as blobby: paired with an outline that also thickens with the
/// size, Bold's already-tight counters close up entirely by 90pt.
///
/// Asking for a *weight* is also the safe way to do this. The alternative —
/// naming the face in the family, `\fnInter SemiBold` — depends on that exact
/// family name existing, and `fc-match` **substitutes silently** rather than
/// failing, so a font that is merely absent comes back as some unrelated
/// family at the wrong weight. A weight request has no such cliff: fontconfig
/// resolves it to the nearest real face *within the family the user asked
/// for*. Checked on this machine with `fc-match`: Inter ships Thin through
/// Black, so 600 is a real SemiBold face; DejaVu Sans Mono and Noto Sans ship
/// only Regular and Bold, and there `\b600` renders exactly what `\b1` did.
///
/// Public so a caller writing its own `\b` into a payload built on top of the
/// base block asks for the same weight the block does — two widgets disagreeing
/// about what "bold" means is exactly the drift this constant prevents.
pub const BOLD_WEIGHT: u32 = 600;

/// A resolved lyric look: everything [`render_ass`] needs, with nothing left to
/// look up.
///
/// This is the *output* of preset resolution, not the preset itself — the
/// config stores a named preset and the accent-follow flag, and the caller
/// turns that pair into one of these. Keeping the renderer's input fully
/// resolved is what lets the whole ASS payload be a pure function of a struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricStyle {
    /// Font family name. Resolved by fontconfig, which always substitutes
    /// something, so an unavailable family degrades instead of failing.
    #[serde(default = "default_font")]
    pub font: String,
    /// Type size in [`PLAY_RES_Y`] units, i.e. pixels at 1080p.
    #[serde(default = "default_size_pt")]
    pub size_pt: u32,
    /// Fill colour as `#RRGGBB`; converted on render.
    #[serde(default = "default_primary")]
    pub primary: String,
    /// Outline colour as `#RRGGBB`. The outline is what makes the text legible
    /// over arbitrary video, so this is not decoration.
    #[serde(default = "default_outline")]
    pub outline: String,
    /// Where on the screen the line sits.
    #[serde(default)]
    pub anchor: Anchor,
    /// Distance from the anchored edge(s), in [`PLAY_RES_Y`] units. Keeps the
    /// text off panels, docks and rounded corners, which every desktop places
    /// differently. Ignored on the axis where the anchor is centred.
    #[serde(default = "default_margin_px")]
    pub margin_px: u32,
    /// Bold weight. On by default: thin type over moving video is unreadable
    /// no matter how good the outline is.
    #[serde(default = "default_bold")]
    pub bold: bool,
}

fn default_font() -> String {
    "Inter".to_string()
}

fn default_size_pt() -> u32 {
    28
}

fn default_primary() -> String {
    "#FFFFFF".to_string()
}

fn default_outline() -> String {
    "#000000".to_string()
}

fn default_margin_px() -> u32 {
    48
}

fn default_bold() -> bool {
    true
}

impl Default for LyricStyle {
    fn default() -> Self {
        LyricStyle {
            font: default_font(),
            size_pt: default_size_pt(),
            primary: default_primary(),
            outline: default_outline(),
            anchor: Anchor::default(),
            margin_px: default_margin_px(),
            bold: default_bold(),
        }
    }
}

/// Render lyric text as a complete `ass-events` payload for mpv's
/// `osd-overlay`.
///
/// One line of text is one ASS event — one override block followed by the
/// escaped text, **no newlines**. That is the shape every caller building on
/// top of the base block relies on: `render_ass("", style)` is exactly the
/// override block, with nothing appended and nothing to split.
///
/// Text containing newlines is rendered as a *stack*: one event per line,
/// joined by the real newlines mpv splits on, each positioned by
/// [`render_ass_line`] so the gap between them is [`leading_px`] rather than
/// whatever libass would derive from the font. See the module docs for what
/// that can and cannot control.
///
/// Every visual property is set explicitly rather than inherited, since the OSD
/// style this draws against is mpv's and not ours. Text goes through
/// [`ass_escape`] and colours through [`hex_to_ass_colour`], so no value from a
/// `.lrc` file or a hand-edited config can escape the block.
///
/// Callers must pass `res_x: PLAY_RES_X` / `res_y: PLAY_RES_Y` to
/// `osd-overlay`; see the module docs.
pub fn render_ass(text: &str, style: &LyricStyle) -> String {
    let lines = display_lines(text);
    let count = lines.len();
    if count <= 1 {
        return render_ass_line(text, style, 0, 1);
    }
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| render_ass_line(line, style, i, count))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Split text on the breaks [`ass_escape`] recognises — LF, CRLF and bare CR —
/// so a stack and an escaped `\N` never disagree about where a line ends.
/// Always yields at least one element, so an empty string is one empty line.
fn display_lines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    loop {
        let Some(i) = rest.find(['\n', '\r']) else {
            out.push(rest);
            return out;
        };
        out.push(&rest[..i]);
        // CRLF is one break, not two — otherwise every Windows-authored line
        // would be followed by an empty event.
        let skip = usize::from(rest[i..].starts_with("\r\n")) + 1;
        rest = &rest[i + skip..];
    }
}

/// Render line `index` of a `count`-line stack as one positioned ASS event.
///
/// This is the escape hatch for a payload whose lines do not share one look —
/// a title above a lyric, a date under a clock — where a single event cannot
/// express two sizes and a `\N` between them cannot express a leading. Each
/// call is a self-contained event: join them with `'\n'` and hand the result to
/// `osd-overlay` unchanged.
///
/// The stack grows *away from the anchored edge*, so the line nearest that edge
/// keeps `LyricStyle::margin_px` whatever `count` is: a bottom anchor grows
/// upward, a top anchor downward, a centred one splits the block about the
/// middle. `index >= count` is not rejected — it simply positions past the end
/// of the block, and the result is still clamped inside the coordinate space.
///
/// The leading is [`leading_px`] of `LyricStyle::size_pt`, i.e. the *stack's*
/// size and not the `\fs` a caller may override per line. That is what keeps
/// the rhythm even when the sizes are not.
pub fn render_ass_line(text: &str, style: &LyricStyle, index: usize, count: usize) -> String {
    format!(
        "{{{}}}{}",
        override_tags(style, index, count),
        ass_escape(text)
    )
}

/// Distance between the line boxes of a stack, for a type size in
/// [`PLAY_RES_Y`] units.
///
/// [`LEADING_PCT`] of the size, never zero. Exposed because a caller composing
/// its own stack needs the same number to reason about how tall the block will
/// be — deriving it a second time is how the two drift apart.
pub fn leading_px(size_pt: u32) -> u32 {
    (size_pt.clamp(MIN_SIZE_PT, MAX_SIZE_PT) * LEADING_PCT / 100).max(1)
}

// TODO(W5): karaoke fill. Deliberately not built — the data and the substrate
// are both short, and half of it would be worse than none:
//
//   * `\k`/`\kf` sweep *within* an event, so real karaoke needs per-word
//     timings. Plain `.lrc` is line-level only; word timings would first mean
//     parsing the enhanced `<mm:ss.xx>` inline tags into `LrcLine`, which is a
//     format change, not a rendering one.
//   * mpv renders OSD ASS tracks at time 0 (`ass_render_frame(…, 0, …)` in
//     `sub/osd_libass.c`), so karaoke timing never advances on the W1 overlay
//     path regardless of the markup. A `\kf` payload would render permanently
//     unswept — visibly broken, not merely absent.
//
// It becomes buildable at W2, on a surface Fresco drives itself. Until then a
// progress fill would have to be re-pushed at 4–10Hz, which contradicts the
// roadmap's "redraw only when the line changes" rule.

/// The override block contents (without the enclosing braces) for line `index`
/// of a `count`-line stack.
fn override_tags(style: &LyricStyle, index: usize, count: usize) -> String {
    let size = style.size_pt.clamp(MIN_SIZE_PT, MAX_SIZE_PT);
    let (x, y) = anchor_pos(style.anchor, style.margin_px);
    let y = stack_y(y, style.anchor, index, count, leading_px(size));
    // Outline and shadow track the type size — a fixed 2px border vanishes on a
    // 4K-sized line and drowns a small one. Rounded rather than truncated so
    // the curve is symmetric about each step instead of always landing low.
    let bord = ((size * BORD_PCT + 50) / 100).clamp(2, MAX_BORD_PX);
    let shad = ((size * SHAD_PCT + 50) / 100).max(1);
    format!(
        // `\1a`/`\3a` force full opacity in case mpv's OSD style is translucent;
        // `\4a&H80&` keeps the drop shadow soft. `\4c` is black rather than the
        // outline colour so a light outline still casts a readable shadow.
        "\\an{an}\\pos({x},{y})\
         \\fn{font}\\fs{size}\\b{weight}\
         \\bord{bord}\\shad{shad}\
         \\1c{primary}\\3c{outline}\\4c&H000000&\
         \\1a&H00&\\3a&H00&\\4a&H80&",
        an = style.anchor.an(),
        font = sanitise_font(&style.font),
        weight = if style.bold { BOLD_WEIGHT } else { 0 },
        primary = hex_to_ass_colour(&style.primary),
        outline = hex_to_ass_colour(&style.outline),
    )
}

/// Move the anchor point to line `index` of a `count`-line stack.
///
/// The block grows away from the edge the anchor pins, so the line closest to
/// that edge keeps the margin it was given and the stack extends into the
/// screen — the opposite would push a bottom-anchored block off the bottom as
/// lines were added.
///
/// Arithmetic is in `i64` and the result is clamped into the coordinate space:
/// `count` reaches this from a caller's `Vec::len`, and a stack tall enough to
/// wrap a `u32` should land at the edge, not at the other side of the screen.
fn stack_y(base: u32, anchor: Anchor, index: usize, count: usize, lead: u32) -> u32 {
    // Same derivation as `anchor_pos`, so the row can never disagree with the
    // position it is offsetting.
    let row = (anchor.an() - 1) / 3;
    // Capped before the arithmetic, not after: `count as i64` on a `usize::MAX`
    // wraps to -1 and would push the line the *wrong way* off the screen, which
    // clamping afterwards cannot undo. One line per output pixel is already
    // more than can be seen, so anything past that is the same picture.
    let cap = i64::from(PLAY_RES_Y) + 1;
    let i = i64::try_from(index).unwrap_or(i64::MAX).min(cap);
    let n = i64::try_from(count.max(1)).unwrap_or(i64::MAX).min(cap);
    let lead = i64::from(lead);
    let offset = match row {
        // Bottom row: grow upward, so the last line keeps the margin.
        0 => -((n - 1 - i) * lead),
        // Middle row: split the block about the centre, so it stays centred.
        1 => i * lead - (n - 1) * lead / 2,
        // Top row: grow downward, so the first line keeps the margin.
        _ => i * lead,
    };
    (i64::from(base) + offset).clamp(0, i64::from(PLAY_RES_Y)) as u32
}

/// Anchor point for `\pos`, in the [`PLAY_RES_X`]×[`PLAY_RES_Y`] space.
///
/// ASS has no per-event margin override — `\marginl`/`\marginv` are style
/// fields, not tags — so honouring `margin_px` means positioning explicitly.
/// `\an` still goes in the payload: it decides which corner of the text box
/// `\pos` pins, so the two must agree.
fn anchor_pos(anchor: Anchor, margin: u32) -> (u32, u32) {
    // Derived from `an()` rather than a second match, so a wrong alignment
    // value can never disagree with the position it is paired with.
    let n = anchor.an() - 1;
    // A margin past the centre would flip left and right; clamp instead.
    let mx = margin.min(PLAY_RES_X / 2);
    let my = margin.min(PLAY_RES_Y / 2);
    let x = match n % 3 {
        0 => mx,
        1 => PLAY_RES_X / 2,
        _ => PLAY_RES_X - mx,
    };
    let y = match n / 3 {
        0 => PLAY_RES_Y - my,
        1 => PLAY_RES_Y / 2,
        _ => my,
    };
    (x, y)
}

/// Strip characters that would terminate the override block or start an escape.
/// The font name comes from a config file the user can hand-edit, so it is
/// untrusted for the same reason the lyric text is. Falls back to the default
/// family if nothing usable is left.
fn sanitise_font(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '{' | '}' | '\\' | '\n' | '\r'))
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        default_font()
    } else {
        cleaned.to_string()
    }
}

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
    fn ass_escape_neutralises_override_syntax() {
        // Lyric text is untrusted: an unescaped `{` opens an override block and
        // could move, recolour or hide the overlay.
        assert_eq!(ass_escape("plain"), "plain");
        assert_eq!(
            ass_escape("{\\pos(0,0)}gotcha"),
            "\\{\\\u{2060}pos(0,0)\\}gotcha"
        );
        assert_eq!(ass_escape("a{b}c"), "a\\{b\\}c");
        // A bare backslash keeps its glyph but cannot start an escape.
        assert_eq!(ass_escape("\\"), "\\\u{2060}");
        assert_eq!(ass_escape("\\N"), "\\\u{2060}N");
        assert_eq!(ass_escape("\\h"), "\\\u{2060}h");
        // Real newlines become ASS breaks; a raw one would start a new event.
        assert_eq!(ass_escape("a\nb"), "a\\Nb");
        assert_eq!(ass_escape("a\r\nb"), "a\\Nb"); // CRLF is one break
        assert_eq!(ass_escape("a\rb"), "a\\Nb");
        // Escaping must never leave a raw brace or a live escape behind.
        for evil in ["{\\an7}", "}}}{{{", "\\\\N", "x\\{y"] {
            let out = ass_escape(evil);
            assert!(!out.contains('{') || out.contains("\\{"));
            assert!(out
                .match_indices('\\')
                .all(|(i, _)| out[i + 1..].starts_with('{')
                    || out[i + 1..].starts_with('}')
                    || out[i + 1..].starts_with('\u{2060}')));
        }
    }

    #[test]
    fn hex_to_ass_colour_reverses_the_channels() {
        // ASS is BGR; a straight copy would swap red and blue everywhere.
        assert_eq!(hex_to_ass_colour("#FF8800"), "&H0088FF&");
        assert_eq!(hex_to_ass_colour("FF8800"), "&H0088FF&"); // `#` optional
        assert_eq!(hex_to_ass_colour("#ff8800"), "&H0088FF&"); // case-insensitive
        assert_eq!(hex_to_ass_colour("  #ff8800  "), "&H0088FF&");
        assert_eq!(hex_to_ass_colour("#000000"), "&H000000&");
        assert_eq!(hex_to_ass_colour("#FFFFFF"), "&HFFFFFF&");
        // Shorthand doubles each nibble: `f` is 0xFF, not 0xF0.
        assert_eq!(hex_to_ass_colour("#f80"), "&H0088FF&");
        assert_eq!(hex_to_ass_colour("fff"), "&HFFFFFF&");
        assert_eq!(hex_to_ass_colour("#123"), "&H332211&");
        // Garbage costs the user their tint, not their lyrics.
        for junk in [
            "",
            "#",
            "#12",
            "#12345",
            "#1234567",
            "#gg0000",
            "rgb(1,2,3)",
        ] {
            assert_eq!(hex_to_ass_colour(junk), "&HFFFFFF&", "input {junk:?}");
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
    fn anchor_positions_respect_the_margin_on_the_right_axis() {
        // Margin applies only where the anchor is against an edge; a centred
        // axis must stay centred or the text drifts as the margin is tuned.
        assert_eq!(anchor_pos(Anchor::TopLeft, 48), (48, 48));
        assert_eq!(anchor_pos(Anchor::TopRight, 48), (1872, 48));
        assert_eq!(anchor_pos(Anchor::BottomLeft, 48), (48, 1032));
        assert_eq!(anchor_pos(Anchor::BottomRight, 48), (1872, 1032));
        assert_eq!(anchor_pos(Anchor::BottomCenter, 48), (960, 1032));
        assert_eq!(anchor_pos(Anchor::TopCenter, 48), (960, 48));
        assert_eq!(anchor_pos(Anchor::MidCenter, 48), (960, 540));
        assert_eq!(anchor_pos(Anchor::MidLeft, 48), (48, 540));
        assert_eq!(anchor_pos(Anchor::MidRight, 48), (1872, 540));
        // An absurd margin must not flip the sides or wrap the coordinate.
        assert_eq!(anchor_pos(Anchor::BottomRight, 99_999), (960, 540));
    }

    #[test]
    fn render_ass_is_one_fully_specified_event() {
        // Pinned in full: this string is the actual wire format, and every tag
        // in it is there because the OSD style it draws against is mpv's, not
        // ours — anything left unset inherits something we do not control.
        let got = render_ass("Hello", &LyricStyle::default());
        assert_eq!(
            got,
            "{\\an2\\pos(960,1032)\\fnInter\\fs28\\b600\\bord2\\shad1\
             \\1c&HFFFFFF&\\3c&H000000&\\4c&H000000&\\1a&H00&\\3a&H00&\\4a&H80&}Hello"
        );
        // One line is one event. Every caller that builds a payload on top of
        // the base block depends on this: `render_ass("")` must be the block
        // and nothing else, with nothing for mpv to split.
        let base = render_ass("", &LyricStyle::default());
        assert!(!base.contains('\n'));
        assert!(base.ends_with("&H80&}"));
    }

    #[test]
    fn a_stack_is_one_positioned_event_per_line() {
        // The leading fix. `\N` inside one event takes whatever spacing libass
        // derives from the font; a line per event is the only way to choose it.
        let got = render_ass("first\nsecond", &LyricStyle::default());
        let events: Vec<&str> = got.split('\n').collect();
        assert_eq!(events.len(), 2, "{got}");
        // 28pt at 125% is a 35px pitch, and a bottom anchor grows *upward* so
        // the last line keeps the 48px margin it was given.
        assert!(events[0].starts_with("{\\an2\\pos(960,997)"), "{got}");
        assert!(events[0].ends_with("}first"), "{got}");
        assert!(events[1].starts_with("{\\an2\\pos(960,1032)"), "{got}");
        assert!(events[1].ends_with("}second"), "{got}");
        // Every event is fully specified — a stacked line may not inherit the
        // line above it, because mpv styles each event from its own defaults.
        for e in events {
            assert!(e.contains("\\fnInter\\fs28\\b600\\bord2\\shad1"), "{e}");
        }
        // CRLF and bare CR are the same one break `ass_escape` treats them as,
        // so a Windows-authored file does not gain empty events.
        for src in ["a\r\nb", "a\rb", "a\nb"] {
            assert_eq!(
                render_ass(src, &LyricStyle::default()).split('\n').count(),
                2,
                "{src:?}"
            );
        }
    }

    #[test]
    fn leading_is_proportional_to_the_type_size() {
        // The whole point of a *factor*: the gap has to hold its proportions
        // at 25pt and at 90pt, or one of the two ends up wrong.
        assert_eq!(leading_px(28), 35);
        assert_eq!(leading_px(64), 80);
        assert_eq!(leading_px(90), 112);
        for size in (MIN_SIZE_PT..=MAX_SIZE_PT).step_by(7) {
            let lead = leading_px(size);
            assert_eq!(lead, size * LEADING_PCT / 100);
            // Always more than the type size, or lines touch; never so much
            // that they stop reading as one block.
            assert!(lead > size, "{size}pt leads {lead}px");
            assert!(lead < size * 2, "{size}pt leads {lead}px");
        }
        // Clamped like the size it is derived from, and never zero.
        assert_eq!(leading_px(0), leading_px(MIN_SIZE_PT));
        assert_eq!(leading_px(u32::MAX), leading_px(MAX_SIZE_PT));
        assert!(leading_px(0) >= 1);
    }

    #[test]
    fn a_stack_grows_away_from_the_anchored_edge() {
        // Whichever edge the anchor pins, the line nearest it keeps the
        // margin — otherwise adding a header would push the block off-screen.
        let stack = |anchor| {
            let style = LyricStyle {
                anchor,
                ..Default::default()
            };
            render_ass("one\ntwo\nthree", &style)
                .split('\n')
                .map(|e| {
                    let y = e.split_once(',').unwrap().1;
                    y.split_once(')').unwrap().0.parse::<i32>().unwrap()
                })
                .collect::<Vec<i32>>()
        };
        // Bottom: upward, last line on the margin. 1032 - 35 - 35.
        assert_eq!(stack(Anchor::BottomCenter), vec![962, 997, 1032]);
        assert_eq!(stack(Anchor::BottomLeft), vec![962, 997, 1032]);
        // Top: downward, first line on the margin.
        assert_eq!(stack(Anchor::TopRight), vec![48, 83, 118]);
        // Middle: split about the centre, so the block stays centred.
        assert_eq!(stack(Anchor::MidCenter), vec![505, 540, 575]);
        // A single line is exactly where it was before stacks existed.
        for anchor in Anchor::ALL {
            let style = LyricStyle {
                anchor,
                ..Default::default()
            };
            let (x, y) = anchor_pos(anchor, style.margin_px);
            assert!(
                render_ass("solo", &style).contains(&format!("\\pos({x},{y})")),
                "{anchor:?}"
            );
        }
    }

    #[test]
    fn a_stack_cannot_be_positioned_off_the_coordinate_space() {
        // `count` arrives from a caller's `Vec::len` and the size from a
        // hand-edited config; a tall stack must land on the edge rather than
        // wrap the coordinate or reappear on the far side.
        let style = LyricStyle {
            size_pt: MAX_SIZE_PT,
            ..Default::default()
        };
        for (i, event) in render_ass(&"x\n".repeat(60), &style)
            .split('\n')
            .enumerate()
        {
            let y: i32 = event
                .split_once(',')
                .unwrap()
                .1
                .split_once(')')
                .unwrap()
                .0
                .parse()
                .unwrap();
            assert!(
                (0..=PLAY_RES_Y as i32).contains(&y),
                "line {i} landed at y={y}"
            );
        }
        assert_eq!(stack_y(1032, Anchor::BottomCenter, 0, usize::MAX, 500), 0);
        assert_eq!(stack_y(48, Anchor::TopLeft, usize::MAX, 2, 500), PLAY_RES_Y);
    }

    #[test]
    fn render_ass_cannot_be_escaped_by_its_inputs() {
        // The whole point of the escaping: neither the lyric nor a hand-edited
        // config may close the override block or open one of its own.
        let style = LyricStyle {
            font: "Ev{il}\\Font".into(),
            primary: "not a colour".into(),
            outline: "#zzz".into(),
            ..Default::default()
        };
        let got = render_ass("{\\an7\\fs200}hijack", &style);
        assert!(got.contains("\\fnEvilFont"), "font not sanitised: {got}");
        // Both unusable colours fall back to white rather than emitting junk.
        assert!(got.contains("\\1c&HFFFFFF&\\3c&HFFFFFF&"), "{got}");
        // Exactly one override block: one `{` and one `}` outside the text.
        let body = got.split_once('}').expect("an override block").1;
        assert!(!body.contains('{') || body.contains("\\{"));
        assert!(body.starts_with("\\{\\\u{2060}an7"), "{body}");
    }

    #[test]
    fn style_fields_reach_the_payload() {
        // Cheap guard against a field being added to the struct and quietly
        // never rendered — the failure mode where the GUI slider does nothing.
        let style = LyricStyle {
            font: "DejaVu Sans".into(),
            size_pt: 120,
            primary: "#f80".into(),
            outline: "#102030".into(),
            anchor: Anchor::TopRight,
            margin_px: 100,
            bold: false,
        };
        let got = render_ass("x", &style);
        assert!(got.contains("\\an9"), "{got}");
        assert!(got.contains("\\pos(1820,100)"), "{got}");
        assert!(got.contains("\\fnDejaVu Sans"), "{got}");
        assert!(got.contains("\\fs120"), "{got}");
        assert!(got.contains("\\b0"), "{got}");
        assert!(got.contains("\\1c&H0088FF&"), "{got}");
        assert!(got.contains("\\3c&H302010&"), "{got}");
        // Outline scales with the type size so it neither vanishes nor drowns.
        assert!(got.contains("\\bord7\\shad4"), "{got}");
    }

    #[test]
    fn outline_and_shadow_stay_proportional_to_the_type_size() {
        // The legibility contract: text has to separate from an arbitrary
        // frame of video at every size, without the outline growing into the
        // counters of the glyphs and turning large type into a blob.
        let bord_shad = |size_pt| {
            let got = render_ass(
                "x",
                &LyricStyle {
                    size_pt,
                    ..Default::default()
                },
            );
            let tail = got.split_once("\\bord").expect("an outline").1;
            let (bord, rest) = tail.split_once("\\shad").expect("a shadow");
            let shad = rest.split('\\').next().expect("a shadow value");
            (
                bord.parse::<u32>().expect(bord),
                shad.parse::<u32>().expect(shad),
            )
        };
        assert_eq!(bord_shad(28), (2, 1));
        assert_eq!(bord_shad(64), (4, 2));
        assert_eq!(bord_shad(90), (5, 3));
        assert_eq!(bord_shad(MAX_SIZE_PT), (24, 12));
        // Monotonic and bounded across the whole range, and the shadow always
        // the lighter of the two — a shadow heavier than the outline reads as
        // a second, blurred copy of the text.
        let (mut last_b, mut last_s) = (0, 0);
        for size in MIN_SIZE_PT..=MAX_SIZE_PT {
            let (b, s) = bord_shad(size);
            assert!(b >= last_b && s >= last_s, "{size}pt went backwards");
            assert!((2..=MAX_BORD_PX).contains(&b), "{size}pt → bord{b}");
            assert!(s >= 1 && s <= b, "{size}pt → bord{b} shad{s}");
            // Never so thick that it closes an Inter counter: measured, 6%.
            assert!(b * 100 <= size * 8 + 200, "{size}pt → bord{b} is fat");
            (last_b, last_s) = (b, s);
        }
    }

    #[test]
    fn bold_asks_for_a_weight_rather_than_the_bold_flag() {
        // `\b1` is the coarse flag; libass reads anything from 100 up as an
        // exact weight and hands it to fontconfig, which resolves it to the
        // nearest real face *inside the family the user asked for*. Naming the
        // face instead (`\fnInter SemiBold`) would depend on that family name
        // existing, and fontconfig substitutes silently when it does not.
        let semibold = render_ass("x", &LyricStyle::default());
        assert!(semibold.contains("\\fs28\\b600\\"), "{semibold}");
        assert!(!semibold.contains("\\b1\\"), "{semibold}");
        // The family is never decorated with a face name, so a font that is
        // merely absent degrades within its own family instead of jumping to
        // an unrelated one.
        assert!(semibold.contains("\\fnInter\\"), "{semibold}");
        let regular = render_ass(
            "x",
            &LyricStyle {
                bold: false,
                ..Default::default()
            },
        );
        assert!(regular.contains("\\fs28\\b0\\"), "{regular}");
        const { assert!(BOLD_WEIGHT >= 100, "libass reads <100 as the bold flag") };
    }

    #[test]
    fn extreme_sizes_are_clamped_not_trusted() {
        // config.toml is hand-editable; `\fs0` renders nothing at all and a
        // five-digit size is a hang waiting to happen inside libass.
        let tiny = render_ass(
            "x",
            &LyricStyle {
                size_pt: 0,
                ..Default::default()
            },
        );
        assert!(tiny.contains("\\fs8\\b600\\bord2\\shad1"), "{tiny}");
        let huge = render_ass(
            "x",
            &LyricStyle {
                size_pt: 100_000,
                ..Default::default()
            },
        );
        assert!(huge.contains("\\fs400"), "{huge}");
        assert!(huge.contains("\\bord24\\shad12"), "{huge}");
        // A font that sanitises down to nothing falls back, never to `\fn}`.
        let empty_font = render_ass(
            "x",
            &LyricStyle {
                font: "{}\\".into(),
                ..Default::default()
            },
        );
        assert!(empty_font.contains("\\fnInter"), "{empty_font}");
    }

    #[test]
    fn defaults_match_the_config_defaults() {
        // The config stores these numbers too; if the two drift, the GUI shows
        // one thing and the overlay renders another.
        let s = LyricStyle::default();
        assert_eq!(s.size_pt, 28);
        assert_eq!(s.margin_px, 48);
        assert_eq!(s.anchor, Anchor::BottomCenter);
        assert!(s.bold);
        assert_eq!(hex_to_ass_colour(&s.primary), "&HFFFFFF&");
        assert_eq!(hex_to_ass_colour(&s.outline), "&H000000&");
    }

    #[test]
    fn end_to_end_over_a_realistic_file() {
        // The daemon's actual sequence: parse, pick the line, render it, and
        // ask when to wake up next.
        let src = "\u{feff}[ti:Test]\r\n[ar:Nobody]\r\n[offset:+100]\r\n\
                   [00:00.00]\r\n[00:12.50]first line\r\n[00:18.00][01:00.00]chorus\r\n\
                   [00:24.00]\r\n[01:30.00]last\r\n";
        let lines = parse_lrc(src);
        assert_eq!(lines.len(), 6);
        let i = line_at(&lines, 19.0).expect("a line is current at 19s");
        assert_eq!(lines[i].text, "chorus");
        assert_eq!(next_change_after(&lines, 19.0), Some(23.9));
        assert!(render_ass(&lines[i].text, &LyricStyle::default()).ends_with("}chorus"));
        // The gap marker clears the overlay rather than holding "chorus".
        let gap = line_at(&lines, 25.0).expect("the timed blank line");
        assert!(lines[gap].text.is_empty());
        assert_eq!(next_change_after(&lines, 90.0), None);
    }
}

#[cfg(test)]
mod scratch_dump {
    use super::*;
    use crate::clock::{ClockStyle, ClockTheme};
    use chrono::{Local, TimeZone};

    /// Write a representative payload to `$LYRICS_DUMP`, for rasterising
    /// through libass by hand. Leading and font weight are the two things no
    /// assertion can approve — the tests can prove the offsets are right and
    /// the picture can still look wrong.
    ///
    /// `#[ignore]` **and** env-gated: a developer harness must never fail on a
    /// machine that has no such directory, which is every CI runner.
    #[test]
    #[ignore]
    fn dump() {
        let dir = std::env::var("LYRICS_DUMP").unwrap_or_default();
        if dir.is_empty() {
            return;
        }
        let dir = std::path::Path::new(&dir);
        let now = Local.with_ymd_and_hms(2026, 7, 15, 21, 5, 0).unwrap();
        let mut out = String::new();
        // lyric: 4-line stack the runtime WOULD produce if it adopted render_ass_line
        let st = LyricStyle::default();
        let rows: [(u32, &str, &str); 4] = [
            (21, "\\b0\\alpha&H70&", "Nightcall"),
            (17, "\\b0\\alpha&HA0&", "Kavinsky, Lovefoxxx"),
            (28, "", "I'm giving you a night call to tell you how I feel"),
            (
                21,
                "\\alpha&H80&",
                "I want to drive you through the night, down the hills",
            ),
        ];
        for (i, (fs, extra, text)) in rows.iter().enumerate() {
            out.push_str(&render_ass_line("", &st, i, rows.len()));
            if *fs != st.size_pt || !extra.is_empty() {
                out.push_str(&format!("{{\\fs{fs}{extra}}}"));
            }
            out.push_str(&ass_escape(text));
            out.push('\n');
        }
        // clock: Stacked with its date
        out.push_str(&crate::clock::render_ass(
            now,
            &ClockStyle {
                theme: ClockTheme::Stacked,
                ..Default::default()
            },
            "#3584E4",
        ));
        out.push('\n');
        // large display case straight from render_ass's own stacking
        out.push_str(&render_ass(
            "Large 90pt display\nsecond line",
            &LyricStyle {
                size_pt: 90,
                anchor: Anchor::TopLeft,
                margin_px: 300,
                ..Default::default()
            },
        ));
        std::fs::write(dir.join("p_real.txt"), out).expect("$LYRICS_DUMP must be writable");
    }
}

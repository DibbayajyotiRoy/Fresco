//! Desktop clock widget: the time, themed, drawn over the wallpaper
//! (WIDGETS_ROADMAP W4 — the clock, brought forward onto the W1 substrate).
//!
//! The roadmap parks the clock behind W2 because a *face* — hands, ticks, a
//! bezel — needs a surface Fresco owns. A **digital** clock does not: it is
//! text, and text already has a delivery path. So this module ships now on the
//! same mpv `osd-overlay` channel W0 proved works, and it is deliberately
//! nothing but pure functions over a timestamp — no I/O, no globals, no clock
//! of its own — so every rule in it is unit-testable and the daemon keeps
//! ownership of *when* things happen.
//!
//! # The mpv contract
//!
//! [`render_ass`] emits the `Text` field of a single ASS dialogue event, which
//! is what mpv's `osd-overlay` consumes with `format: "ass-events"`. It shares
//! the coordinate space of the lyric overlay, so **the caller must pass
//! `res_x: lyrics::PLAY_RES_X` and `res_y: lyrics::PLAY_RES_Y`** — W0 found the
//! OSD space otherwise follows the *video's* render area and a rotated
//! wallpaper clips the overlay. Sizes and margins are therefore "pixels at
//! 1080p" and scale proportionally on any output.
//!
//! # Power: this module exists to let the daemon sleep
//!
//! A clock is the one widget whose content changes on a schedule nobody has to
//! discover, so it must never be polled. [`next_change`] returns the *exact
//! instant* the visible string next differs, and the daemon waits on that
//! deadline:
//!
//! ```text
//! render  →  sleep until next_change()  →  wake  →  render  →  …
//! ```
//!
//! Rule 7 of the roadmap's power model sets the budget: **one redraw per minute
//! unless seconds are enabled**, and this module is where that is enforced.
//! `Wordy` is coarser still — its text only changes every five minutes — and
//! [`next_change`] knows it. Getting this function wrong is a power regression,
//! not a cosmetic bug, so it is derived from the *same* helpers
//! ([`shows_seconds`], [`shows_date`]) that decide what [`format_time`] prints:
//! the two cannot disagree without failing a test.
//!
//! [`format_time`] is also deterministic within a bucket — every instant in the
//! same minute (or second, or five-minute span) produces byte-identical output
//! — so a caller that keeps the last rendered string can detect "nothing
//! changed" with one comparison and skip the push entirely.
//!
//! # Untrusted input
//!
//! The visible text is generated here and never comes from a user, but the
//! colours do, from a hand-editable config. Every string that reaches the
//! payload goes through [`lyrics::ass_escape`] (text) or
//! [`lyrics::hex_to_ass_colour`] (colours), which is what keeps a mistyped
//! `colour = "}{\\an7"` from rewriting the overlay instead of tinting it.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::fmt::Write as _;

use chrono::{DateTime, Local, TimeDelta, Timelike};
use serde::{Deserialize, Serialize};

use crate::lyrics::{self, Anchor, LyricStyle, PLAY_RES_X, PLAY_RES_Y};

/// How the clock looks. Each theme fixes face, weight, tracking and layout
/// together, so the user picks a *look* rather than assembling one out of a
/// dozen fields that mostly combine into something ugly — the same bargain
/// `config::LyricStylePreset` makes for lyrics.
///
/// TOML spellings are the variant names lowercased: `"digital"`, `"minimal"`,
/// `"segment"`, `"stacked"`, `"wordy"`, `"card"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClockTheme {
    /// Clean, large, bold `HH:MM`. The default: a clock should read at a
    /// glance from across the room and then be forgotten.
    #[default]
    Digital,
    /// Thin, small, wide-set and lower case. Time only — no date, ever;
    /// "minimal" that still shows a date is just a smaller Digital.
    Minimal,
    /// Seven-segment LED feel: a monospace face, heavy letter spacing and a
    /// blurred outline in the fill colour so the digits appear to emit light.
    Segment,
    /// Big time with the date stacked beneath it in small, wide-tracked caps.
    /// The date is the theme, so it is shown whatever `show_date` says.
    Stacked,
    /// The time spelled out — "half past ten", "quarter to nine". Rounded to
    /// the nearest five minutes, because that is how people actually say it.
    Wordy,
    /// A neon-edged dark card carrying the time, the weekday, the date and a
    /// drawn **analog face** — the only theme that is a picture rather than a
    /// run of text. See [`render_ass`] for what that costs and what it buys.
    ///
    /// Listed last because it is by far the most expensive look to draw: a
    /// thirty-odd ASS events against everything else's one.
    Card,
}

impl ClockTheme {
    /// Every theme, in the order a picker should list them — cheapest look
    /// first — so a GUI does not hand-list the variants a second time.
    pub const ALL: [ClockTheme; 6] = [
        ClockTheme::Digital,
        ClockTheme::Minimal,
        ClockTheme::Segment,
        ClockTheme::Stacked,
        ClockTheme::Wordy,
        ClockTheme::Card,
    ];

    /// Display name for a picker. Kept here rather than in the GUI so the
    /// spelling cannot drift from the variant it names.
    pub const fn label(self) -> &'static str {
        match self {
            ClockTheme::Digital => "Digital",
            ClockTheme::Minimal => "Minimal",
            ClockTheme::Segment => "Segment",
            ClockTheme::Stacked => "Stacked",
            ClockTheme::Wordy => "Wordy",
            ClockTheme::Card => "Card",
        }
    }
}

/// A resolved clock look: everything [`render_ass`] needs except the accent
/// colour, which the caller resolves from the desktop theme.
///
/// Field defaults are wired for serde so the whole struct can be embedded in
/// the config and still survive a `config.toml` written by an older version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockStyle {
    /// Which look to render.
    #[serde(default)]
    pub theme: ClockTheme,
    /// Where on the screen the clock sits.
    #[serde(default = "default_anchor")]
    pub anchor: Anchor,
    /// Size of the *time* line in [`lyrics::PLAY_RES_Y`] units (pixels at
    /// 1080p). Each theme scales it: `Stacked` runs larger, `Minimal` smaller.
    #[serde(default = "default_font_size_pt")]
    pub font_size_pt: u32,
    /// Distance from the anchored edge(s), in the same units. Ignored on the
    /// axis where the anchor is centred.
    #[serde(default = "default_margin_px")]
    pub margin_px: u32,
    /// Show seconds. **Off by default and worth keeping off**: it is the one
    /// switch here that costs power, turning one redraw a minute into sixty.
    #[serde(default)]
    pub show_seconds: bool,
    /// Show the date under the time. Honoured by `Digital`, `Segment` and
    /// `Wordy`; forced on for `Stacked` and off for `Minimal` — see
    /// [`shows_date`].
    #[serde(default)]
    pub show_date: bool,
    /// 24-hour clock. **On by default**, for three reasons that all point the
    /// same way: `HH:MM` is unambiguous without a meridiem suffix, it is a
    /// fixed six-or-fewer glyphs so a right-anchored overlay never shifts as
    /// 09:59 becomes 10:00, and the rest of the codebase already speaks it
    /// (`schedule::parse_hhmm` parses 24-hour `HH:MM`).
    #[serde(default = "default_use_24h")]
    pub use_24h: bool,
    /// Fill colour as `#RRGGBB`. Ignored when `accent_follow` is set.
    #[serde(default = "default_colour")]
    pub colour: String,
    /// Take the fill colour from the desktop accent instead of `colour`.
    #[serde(default)]
    pub accent_follow: bool,
}

/// Top-right: desktop icons start at the *top-left* on every desktop Fresco
/// supports, panels and docks own the bottom edge, and the lyric overlay
/// already defaults to bottom-centre — so this is the corner where both
/// widgets can be on at once without either being covered.
fn default_anchor() -> Anchor {
    Anchor::TopRight
}

/// Big enough to read from across a room at 1080p without dominating the
/// wallpaper; roughly 6% of the screen height.
fn default_font_size_pt() -> u32 {
    64
}

fn default_margin_px() -> u32 {
    56
}

fn default_use_24h() -> bool {
    true
}

fn default_colour() -> String {
    "#FFFFFF".to_string()
}

impl Default for ClockStyle {
    fn default() -> Self {
        ClockStyle {
            theme: ClockTheme::default(),
            anchor: default_anchor(),
            font_size_pt: default_font_size_pt(),
            margin_px: default_margin_px(),
            show_seconds: false,
            show_date: false,
            use_24h: default_use_24h(),
            colour: default_colour(),
            accent_follow: false,
        }
    }
}

/// Smallest type size rendered; below this the outline swallows the glyphs.
/// Mirrors the clamp inside the lyric renderer — the date line is sized here,
/// so it needs its own guard against a hand-edited `font_size_pt`.
const MIN_SIZE_PT: u32 = 8;
/// Largest type size rendered. A guard against a five-digit config value
/// hanging libass, not a design limit.
const MAX_SIZE_PT: u32 = 400;

/// Whether the rendered string carries seconds.
///
/// Not the same question as `ClockStyle::show_seconds`: `Wordy` has no way to
/// say "and seventeen seconds", so it ignores the switch. This is the single
/// source of truth shared by [`format_time`] and [`next_change`] — if the two
/// answered it separately, the daemon could sleep for a minute while the text
/// changed every second, which is the exact bug that shows up as a clock that
/// "sometimes lags".
pub fn shows_seconds(s: &ClockStyle) -> bool {
    s.show_seconds && !matches!(s.theme, ClockTheme::Wordy)
}

/// Whether the rendered string carries a date line.
///
/// Two themes overrule the switch on purpose: `Stacked` *is* "time with the
/// date beneath it", so turning the date off would leave it indistinguishable
/// from `Digital`; `Minimal` is defined as time only. A GUI can call this to
/// grey the switch out rather than letting it silently do nothing.
pub fn shows_date(s: &ClockStyle) -> bool {
    match s.theme {
        ClockTheme::Stacked => true,
        ClockTheme::Minimal => false,
        _ => s.show_date,
    }
}

/// The visible text for `now` under `s` — no markup, no escaping.
///
/// A date, when present, is on a second line separated by a real `'\n'`;
/// [`render_ass`] turns that into an ASS break. Keeping this function free of
/// markup is what makes it testable as *language* rather than as a payload.
///
/// [`ClockTheme::Card`] is the exception to "one or two lines": it stacks the
/// time, the weekday and — when [`shows_date`] agrees — the date, so it can
/// return three. Callers that split on `'\n'` must not assume a limit of two.
///
/// The result is constant across every instant in the same bucket (a second, a
/// minute, or five minutes — see [`next_change`]), so a caller can compare it
/// against the last rendered string to decide whether a push is needed at all.
pub fn format_time(now: DateTime<Local>, s: &ClockStyle) -> String {
    // `Card` is the one theme with three text rows rather than one or two, so
    // it builds its own stack — see [`card_text`].
    if matches!(s.theme, ClockTheme::Card) {
        return card_text(now, s);
    }
    let mut out = match s.theme {
        ClockTheme::Wordy => wordy_time(now.hour(), now.minute()),
        _ => numeric_time(now, s),
    };
    if shows_date(s) {
        out.push('\n');
        out.push_str(&date_text(now, s.theme));
    }
    out
}

/// `HH:MM`, `H:MM AM`, `HH:MM:SS` — the numeric themes.
fn numeric_time(now: DateTime<Local>, s: &ClockStyle) -> String {
    let (h24, m, sec) = (now.hour(), now.minute(), now.second());
    let mut out = if s.use_24h {
        format!("{h24:02}:{m:02}")
    } else if matches!(s.theme, ClockTheme::Segment) {
        // An LED panel has a fixed digit count, so `Segment` pads even in
        // 12-hour mode; every other theme follows the usual "9:05", not "09:05".
        format!("{:02}:{m:02}", hour_12(h24))
    } else {
        format!("{}:{m:02}", hour_12(h24))
    };
    if shows_seconds(s) {
        out.push_str(&format!(":{sec:02}"));
    }
    if !s.use_24h {
        out.push(' ');
        // Lower case for `Minimal` — a quiet clock should not shout "PM".
        out.push_str(match (matches!(s.theme, ClockTheme::Minimal), h24 < 12) {
            (true, true) => "am",
            (true, false) => "pm",
            (false, true) => "AM",
            (false, false) => "PM",
        });
    }
    out
}

/// 0..=23 → the 1..=12 a person reads off a dial. Midnight and noon are both
/// twelve, which is the case a naive `h % 12` gets wrong: it would print
/// "0:00 AM".
fn hour_12(hour24: u32) -> u32 {
    match hour24 % 12 {
        0 => 12,
        h => h,
    }
}

/// The date line. Fixed format, not a locale one: chrono without
/// `unstable-locales` cannot localise, and of the formats available `%a %d %b`
/// is the one that cannot be misread — `15/07` and `07/15` mean opposite things
/// depending on which side of an ocean you learned to write dates on, while
/// "Wed 15 Jul" means the same thing everywhere.
fn date_text(now: DateTime<Local>, theme: ClockTheme) -> String {
    let d = now.format("%a %d %b").to_string();
    if look_for(theme).date_upper {
        d.to_uppercase()
    } else {
        d
    }
}

// ---------------------------------------------------------------------------
// Wordy
// ---------------------------------------------------------------------------

/// The time as English, rounded to the nearest five minutes.
///
/// Two hour rollovers hide in here and both are easy to get backwards:
///
/// * A "to" phrase names the hour it is heading *toward*, so 08:45 is "quarter
///   to nine" — the hour advances by one.
/// * Rounding can cross the hour by itself: 08:58 rounds to 60 minutes, which
///   is "nine o'clock" and not "sixty past eight".
///
/// Always 12-hour wording regardless of `ClockStyle::use_24h`: nobody says
/// "seventeen o'clock", so honouring the switch here would produce a phrase no
/// English speaker uses.
fn wordy_time(hour24: u32, minute: u32) -> String {
    let (five, carry) = round_to_five(minute);
    let (phrase, hour_carry) = match five {
        0 => (None, carry),
        30 => (Some("half past".to_string()), carry),
        // 5, 10, 15, 20, 25 — the first half of the hour, counted forwards.
        1..=29 => (Some(format!("{} past", minute_word(five))), carry),
        // 35, 40, 45, 50, 55 — counted backwards from the *next* hour.
        _ => (Some(format!("{} to", minute_word(60 - five))), carry + 1),
    };
    let hour = hour_word(hour24 + hour_carry);
    match phrase {
        Some(p) => format!("{p} {hour}"),
        None => format!("{hour} o'clock"),
    }
}

/// Round a minute to the nearest five, reporting whether it crossed the hour.
///
/// Returns `(bucket, hours_carried)` with `bucket` in `0..60`. The split is
/// exact on the halfway point because the halfway point is 2.5 and minutes are
/// integers: 0–2 round down, 3–7 round to five, and 58–59 round up to the next
/// hour.
fn round_to_five(minute: u32) -> (u32, u32) {
    let rounded = ((minute + 2) / 5) * 5;
    if rounded >= 60 {
        (0, 1)
    } else {
        (rounded, 0)
    }
}

/// The five-minute bucket as a word. "quarter" rather than "fifteen" —
/// "fifteen past" is technically fine and nobody says it.
fn minute_word(five: u32) -> &'static str {
    debug_assert!(
        matches!(five, 5 | 10 | 15 | 20 | 25),
        "round_to_five cannot produce {five}"
    );
    match five {
        5 => "five",
        10 => "ten",
        15 => "quarter",
        20 => "twenty",
        _ => "twenty-five",
    }
}

/// The hour as a word. Takes a 24-hour value that may already have been carried
/// past 23 by a "to" phrase at 23:45, so it reduces modulo 12 rather than
/// assuming a valid hour.
fn hour_word(hour: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "twelve", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven",
    ];
    NAMES[(hour % 12) as usize]
}

// ---------------------------------------------------------------------------
// Power model
// ---------------------------------------------------------------------------

const NANOS_PER_SEC: i64 = 1_000_000_000;

/// The exact instant the visible string next changes.
///
/// The daemon waits on this deadline instead of polling, so the granularity is
/// per-theme and matches [`format_time`] exactly:
///
/// | when | period | lands on |
/// |---|---|---|
/// | seconds shown | 1s | the next second boundary |
/// | `Wordy` | 5min | the next minute where the words change (`:03`, `:08`, …) |
/// | everything else | 1min | the next minute boundary |
///
/// `Wordy`'s change points are offset by three minutes rather than falling on
/// `:00`, `:05`, … because it rounds to the *nearest* five: at 10:02 it still
/// says "ten o'clock" and only becomes "five past ten" at 10:03. Waking on the
/// obvious `:05` grid would show the wrong words for three minutes out of
/// every five.
///
/// The result is always **strictly after** `now`. Called exactly on a boundary
/// it returns the following one — returning `now` would give the daemon a
/// zero-length sleep and turn Smart Sleep into a spin loop.
///
/// Adding is done in the absolute-time domain, so a DST jump inside the sleep
/// shortens or lengthens the wall-clock gap without the deadline drifting: the
/// wake still lands on a real second boundary, and every real DST offset is a
/// whole multiple of five minutes, so even `Wordy`'s phase survives the jump.
pub fn next_change(now: DateTime<Local>, s: &ClockStyle) -> DateTime<Local> {
    let period = i64::from(tick_secs(s)) * NANOS_PER_SEC;
    // `elapsed` is in `0..period`, so `remaining` is in `1..=period`: strictly
    // positive, which is the whole guarantee this function makes.
    let remaining = period - elapsed_in_period(now, s);
    now.checked_add_signed(TimeDelta::nanoseconds(remaining))
        // Unreachable short of a clock set to the year 262143; a caller that
        // sees a non-future deadline should treat it as "wake now", which is
        // the safe degradation.
        .unwrap_or(now)
}

/// Seconds between visible changes for this style. Named separately from
/// [`next_change`] so the refresh table in the roadmap has one place to be
/// checked against.
pub fn tick_secs(s: &ClockStyle) -> u32 {
    if shows_seconds(s) {
        1
    } else if matches!(s.theme, ClockTheme::Wordy) {
        300
    } else {
        60
    }
}

/// Nanoseconds elapsed since the start of the current display bucket.
fn elapsed_in_period(now: DateTime<Local>, s: &ClockStyle) -> i64 {
    // A leap second reports 1_000_000_000..=1_999_999_999 here. Clamping keeps
    // `elapsed` below the period, so the deadline stays in the future rather
    // than landing a second in the past once every few years.
    let nanos = i64::from(now.nanosecond().min(999_999_999));
    let secs = match tick_secs(s) {
        1 => 0,
        // `Wordy`'s buckets start at :03, :08, … — i.e. minutes where
        // `(minute + 2) % 5 == 0`, which is exactly where `round_to_five`
        // flips. Sixty is a multiple of five, so the pattern repeats cleanly
        // every hour and the hour field never enters the arithmetic.
        300 => i64::from(((now.minute() + 2) % 5) * 60 + now.second()),
        _ => i64::from(now.second()),
    };
    secs * NANOS_PER_SEC + nanos
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// The fixed half of a theme: everything that does not depend on the user's
/// size, colour or anchor. Percentages rather than floats so the arithmetic is
/// exact and the rendered payload is byte-stable across platforms.
struct Look {
    /// Font family. Resolved by fontconfig, which always substitutes
    /// something, so an absent family degrades instead of failing.
    font: &'static str,
    /// Time-line size as a percentage of `ClockStyle::font_size_pt`.
    size_pct: u32,
    bold: bool,
    italic: bool,
    /// Letter spacing (ASS `fsp`) as a percentage of the rendered size, so
    /// tracking stays proportional when the user drags the size slider.
    tracking_pct: u32,
    /// Outline thickness as a percentage of the rendered size. `None` keeps
    /// the lyric renderer's own size-proportional default.
    bord_pct: Option<u32>,
    /// Gaussian edge blur in ASS `blur` units — what turns a fat outline into
    /// a halo instead of a border.
    blur: u32,
    /// Draw the outline in the fill colour rather than black, so the glyphs
    /// look like they are emitting light.
    glow: bool,
    /// Date-line size as a percentage of the *time* line's rendered size.
    date_size_pct: u32,
    date_bold: bool,
    date_tracking_pct: u32,
    /// Upper-case the date line.
    date_upper: bool,
}

/// The look table. This is the whole of "themes" — everything else in the
/// module is shared machinery.
const fn look_for(theme: ClockTheme) -> Look {
    match theme {
        // Big, bold, tight, black-outlined: maximum legibility over arbitrary
        // video, which is what an everyday clock has to survive.
        ClockTheme::Digital => Look {
            font: "Inter",
            size_pct: 100,
            bold: true,
            italic: false,
            tracking_pct: 0,
            bord_pct: None,
            blur: 0,
            glow: false,
            date_size_pct: 34,
            date_bold: false,
            date_tracking_pct: 4,
            date_upper: false,
        },
        // Half the size, unbolded, slightly opened up, and a hairline outline —
        // present without asking for attention.
        ClockTheme::Minimal => Look {
            font: "Inter",
            size_pct: 55,
            bold: false,
            italic: false,
            tracking_pct: 4,
            bord_pct: Some(3),
            blur: 0,
            glow: false,
            date_size_pct: 0,
            date_bold: false,
            date_tracking_pct: 0,
            date_upper: false,
        },
        // Monospace so the digits sit on a fixed grid, heavy tracking so they
        // read as separate cells, and a blurred fill-coloured outline for the
        // bleed a real LED has. DejaVu Sans Mono because it is the monospace
        // family effectively guaranteed present on a Linux desktop.
        ClockTheme::Segment => Look {
            font: "DejaVu Sans Mono",
            size_pct: 95,
            bold: true,
            italic: false,
            tracking_pct: 16,
            bord_pct: Some(9),
            blur: 4,
            glow: true,
            date_size_pct: 26,
            date_bold: false,
            date_tracking_pct: 14,
            date_upper: true,
        },
        // Deliberately the largest, with the date pinned under it in small
        // wide-tracked caps: the weight and tracking contrast between the two
        // lines is the design, not the stacking on its own.
        ClockTheme::Stacked => Look {
            font: "Inter",
            size_pct: 130,
            bold: true,
            italic: false,
            tracking_pct: 0,
            bord_pct: None,
            blur: 0,
            glow: false,
            date_size_pct: 24,
            date_bold: false,
            date_tracking_pct: 22,
            date_upper: true,
        },
        // Prose, so it is set like prose: smaller (the string is five times
        // longer than "21:05"), unbolded and italic.
        ClockTheme::Wordy => Look {
            font: "Inter",
            size_pct: 62,
            bold: false,
            italic: true,
            tracking_pct: 2,
            bord_pct: None,
            blur: 0,
            glow: false,
            date_size_pct: 42,
            date_bold: false,
            date_tracking_pct: 6,
            date_upper: false,
        },
        // `Card` draws its own panel and places every row with its own `\pos`,
        // so almost nothing in this table reaches it — only `size_pct` (the
        // card scales off the rendered time size) and `date_upper` (the card's
        // date row is set in caps). The rest is filled in with the neutral
        // values so a future refactor that *does* read them gets no surprise.
        ClockTheme::Card => Look {
            font: "Inter",
            size_pct: 100,
            bold: true,
            italic: false,
            tracking_pct: 0,
            bord_pct: None,
            blur: 0,
            glow: false,
            date_size_pct: 26,
            date_bold: false,
            date_tracking_pct: 10,
            date_upper: true,
        },
    }
}

/// Render the clock as a complete `ass-events` payload for mpv's
/// `osd-overlay`.
///
/// For the *text* themes the time is one ASS event and a stacked date is a
/// **second** one, joined by the real newline mpv splits on. That is
/// deliberate: ASS has no line-height property, so giving each line its own
/// `\pos` is the only way to choose the gap between them — see
/// [`lyrics::render_ass_line`]. An in-event `\N` break, which is what this used
/// to emit, leaves the spacing to whatever libass derives from the font, and
/// under a 130%-sized `Stacked` time that lands close enough to touching that
/// the date reads as a subscript rather than a second line.
///
/// A separate event inherits nothing, so the theme's own tags are restated on
/// the date line rather than carried across the break.
///
/// [`ClockTheme::Card`] goes further still and emits **several**
/// newline-separated events, because one event cannot put text on top of a
/// drawing: libass gives a `\p1` drawing an advance equal to its bounding-box
/// width and `\fsp` does not apply to it, so anything after a card-sized shape
/// starts to the *right* of the card. Every Card event still specifies its
/// font, size, weight, colours, alphas, border and shadow in full, so none of
/// them inherits mpv's default OSD styling — which is the property the
/// single-event rule was protecting. Events render in payload order, so the
/// panel is emitted before the type that sits on it.
///
/// `accent_hex` is the desktop accent as `#RRGGBB`; it is used only when
/// `ClockStyle::accent_follow` is set, and an unparsable value costs the user
/// their tint rather than their clock.
///
/// Callers must pass `res_x: lyrics::PLAY_RES_X` / `res_y: lyrics::PLAY_RES_Y`
/// to `osd-overlay`; see the module docs.
pub fn render_ass(now: DateTime<Local>, s: &ClockStyle, accent_hex: &str) -> String {
    if matches!(s.theme, ClockTheme::Card) {
        return render_card(now, s, accent_hex);
    }
    let look = look_for(s.theme);
    let text = format_time(now, s);
    let mut lines = text.splitn(2, '\n');
    let time_line = lines.next().unwrap_or_default();
    let date_line = lines.next();

    let fill = if s.accent_follow {
        accent_hex
    } else {
        s.colour.as_str()
    };
    let size = scaled(s.font_size_pt, look.size_pct);

    let base = LyricStyle {
        font: look.font.to_string(),
        size_pt: size,
        primary: fill.to_string(),
        // Black outline everywhere except the glow themes: over arbitrary
        // video the outline is not decoration, it is the only thing keeping
        // light text readable on a light frame.
        outline: if look.glow {
            fill.to_string()
        } else {
            "#000000".to_string()
        },
        anchor: s.anchor,
        margin_px: s.margin_px,
        bold: look.bold,
    };

    // Reuse rather than reimplement: rendering a *line* with empty text yields
    // exactly the shared override block — anchor, position, margin clamping,
    // colour conversion, size-proportional outline and weight, the opacity
    // forcing, and the leading offset for its place in the stack — with nothing
    // appended. The theme's extra tags then go in a *second* consecutive block,
    // which is ordinary ASS and keeps the placement maths in one tested place
    // instead of two.
    let rows = 1 + usize::from(date_line.is_some());
    let extra = extra_tags(&look, size);
    let mut out = lyrics::render_ass_line("", &base, 0, rows);
    if !extra.is_empty() {
        out.push('{');
        out.push_str(&extra);
        out.push('}');
    }
    out.push_str(&lyrics::ass_escape(time_line));

    if let Some(date) = date_line {
        let date_size = scaled(size, look.date_size_pct);
        // A real newline: mpv makes this a second event, which is what buys the
        // date a chosen distance from the time instead of libass's own.
        out.push('\n');
        out.push_str(&lyrics::render_ass_line("", &base, 1, rows));
        // Italic, tracking and blur are the theme speaking, and the date should
        // sound like the same theme — but a separate event inherits none of
        // them, so `extra` is restated here rather than carried over. Size,
        // weight and tracking are then the date's own, and win by coming last.
        // The same weight the base block asks for, so "bold" means one thing
        // across both widgets rather than two.
        let weight = if look.date_bold {
            lyrics::BOLD_WEIGHT
        } else {
            0
        };
        out.push('{');
        out.push_str(&extra);
        out.push_str(&format!(
            "\\fs{date_size}\\b{weight}\\fsp{fsp}",
            fsp = tracking(date_size, look.date_tracking_pct),
        ));
        out.push('}');
        out.push_str(&lyrics::ass_escape(date));
    }
    out
}

/// The theme-specific override block, minus its braces. Empty when the theme
/// needs nothing the shared renderer does not already emit.
fn extra_tags(look: &Look, size: u32) -> String {
    let mut t = String::new();
    if look.italic {
        t.push_str("\\i1");
    }
    if look.tracking_pct > 0 {
        t.push_str(&format!("\\fsp{}", tracking(size, look.tracking_pct)));
    }
    if let Some(pct) = look.bord_pct {
        // At least 1: a zero border with a blur draws no halo at all, and the
        // glow themes are the ones that set this.
        t.push_str(&format!("\\bord{}", (size * pct / 100).clamp(1, 40)));
    }
    if look.blur > 0 {
        t.push_str(&format!("\\blur{}", look.blur));
    }
    t
}

/// `base * pct / 100`, clamped to a size libass can actually draw.
///
/// Saturating because `font_size_pt` comes from a hand-editable config and
/// `u32::MAX * 130` is not a size, it is an overflow.
fn scaled(base: u32, pct: u32) -> u32 {
    (base.saturating_mul(pct) / 100).clamp(MIN_SIZE_PT, MAX_SIZE_PT)
}

/// Letter spacing for a rendered size. Not clamped to a minimum: zero tracking
/// is a legitimate, and common, answer.
fn tracking(size: u32, pct: u32) -> u32 {
    size.saturating_mul(pct) / 100
}

// ---------------------------------------------------------------------------
// Card: the drawn theme
// ---------------------------------------------------------------------------
//
// `Card` is a picture, so it plays by different rules from the five text
// themes, and all of them are here rather than sprinkled through the shared
// machinery above.
//
// # Why several events
//
// mpv splits an `ass-events` payload on newlines into one event each, and this
// theme uses that. It is not a preference: a single event cannot put text on a
// drawing. libass lays a `\p1` drawing out as one glyph whose *advance* is its
// bounding-box width, and `\fsp` — the only tag that can shorten an advance —
// is not applied to drawings, so text following a card-sized shape always
// begins to the right of the card. Each event below therefore places itself
// with its own `\an7\pos`, and every one of them sets font, size, weight,
// colours, alphas, border and shadow explicitly, so none inherits mpv's OSD
// defaults. Payload order is paint order — see `render_card` for the stack.
//
// # Coordinates and the bounding-box pin
//
// Every drawing is authored in **card-local** units — `(0, 0)` at the card's
// top-left, `(w, h)` at its bottom-right — and emitted at `\p1`, where one
// drawing unit is one unit of the `PLAY_RES` space the whole widget layer
// shares. Each drawing opens with two zero-area contours at those two corners
// (`CardPath::pin`, the technique `crate::visualizer` uses for the same
// reason): libass sizes a drawing from the bounding box of its points, so
// without the pin the *hands* would resize the box every minute and the card
// would crawl across the wallpaper. `CardPath::pt` clamps into the same box, so
// a geometry mistake clips a shape instead of moving the widget.
//
// # No float ever reaches the payload
//
// Coordinates enter through `CardPath::pt` and leave as `i32` via `du`, whose
// `as` cast is defined to saturate with `NaN` mapping to zero. libass discards
// a drawing it cannot parse — one `NaN` would cost the whole card, not one
// tick — so the guarantee is carried by the type rather than by review.
//
// # The border is not a gradient
//
// **ASS has no gradients.** The pink-to-cyan edge is `CARD_SEGMENTS` separate
// filled shapes walking the card's perimeter, each a flat step along the ramp
// between [`CARD_NEON_A`] and [`CARD_NEON_B`]. Close up it is visibly stepped,
// and that is the honest limit of the substrate, not a bug to be tuned out.
//
// # The glass is translucency and edge lighting, not frosted glass
//
// The card is styled after glassmorphism, and it is important that a future
// reader knows exactly which half of that effect is here and which is not.
//
// Real glassmorphism is a *backdrop* blur: the surface blurs whatever is behind
// it. **ASS cannot do that, and no amount of tuning will get there.** `\blur`
// is a Gaussian on the drawn shape's own alpha — it softens the shape's edges
// and nothing else; there is no `backdrop-filter` equivalent, no way to sample
// the video underneath, and libass composites the overlay over a frame it never
// gets to read. So the card fakes none of it. What it does instead is the two
// parts of the look that *are* expressible:
//
// * **Translucency.** The body is genuinely see-through
//   ([`CARD_BODY_ALPHA`]), so the wallpaper reads through the card rather than
//   being covered by it. This is the half of glassmorphism that carries the
//   effect.
// * **Edge lighting.** A hairline white edge at low alpha plus a brighter
//   highlight along the *top* edge only ([`card_highlight`]), which reads as a
//   light source above the card. Empirically this signals "glass" harder than
//   the blur does.
//
// The blur that is missing is not only decorative: in real glassmorphism it is
// what keeps type legible when the backdrop is bright and busy. Without it the
// card would fail a white wallpaper, so its job is done instead by
// [`card_scrim`] — a soft, feathered plate behind the *text block only*, which
// is the smallest thing that buys the contrast back. Sizing of both alphas is
// worked out against a worst-case white backdrop in [`CARD_SCRIM_ALPHA`].

/// Hot pink: the ramp's start, at the card's top-left corner.
const CARD_NEON_A: [u8; 3] = [0xFF, 0x2D, 0x95];
/// Cyan: the ramp's far end, at the card's bottom-right corner.
const CARD_NEON_B: [u8; 3] = [0x2A, 0xE0, 0xFF];
/// The card body. Near-black rather than `#000000`: a pure black panel reads as
/// a hole punched in the wallpaper rather than as a surface laid on it, and at
/// these alphas it also crushes whatever is showing through into a flat smear.
const CARD_PANEL: &str = "#0A0C12";
/// The analog dial, a step darker than the body so the face reads as inset.
const CARD_FACE: &str = "#05070C";
/// The glass edge and the top highlight. White, carried entirely by alpha, so
/// the edge stays neutral instead of picking up a tint that fights the neon.
const CARD_EDGE: &str = "#FFFFFF";
/// The date row. A muted blue-grey at *full* opacity rather than white held
/// back by alpha: a translucent card can have a bright wallpaper behind it, and
/// faded white over that is the first thing to become unreadable. Hierarchy is
/// carried by size, weight and hue instead, which do not depend on the backdrop.
const CARD_DATE_INK: &str = "#C2C9D6";

// ASS alpha runs backwards: `0x00` is opaque and `0xFF` is invisible.

/// Body alpha — a little over a third opaque, so the wallpaper genuinely reads
/// through the card. This is the glass, and it is the one number that decides
/// whether the effect lands: much more and it is a dark panel, much less and no
/// scrim can rescue the type over a bright frame.
const CARD_BODY_ALPHA: u8 = 0xA0;
/// Scrim alpha behind the text block, chosen against the worst case rather than
/// by eye. Over a *white* backdrop the body alone leaves a surface at about 64%
/// white, on which white type sits at roughly 2.5:1 — under AA at any size. The
/// scrim over the body takes that surface down to about 31% white, where white
/// type clears 8:1 and [`CARD_DATE_INK`], the weakest ink on the card, still
/// clears 4.5:1.
const CARD_SCRIM_ALPHA: u8 = 0x74;
/// Dial alpha. Darker than the scrim: the dial is where the thinnest ink on the
/// card lives, and hairline minute ticks need the most contrast of anything.
const CARD_FACE_ALPHA: u8 = 0x5A;
/// The hairline glass edge, about a fifth opaque — the CSS `1px solid
/// rgba(255,255,255,0.2)` the look is quoting.
const CARD_EDGE_ALPHA: u8 = 0xCC;
/// The top-edge highlight, brighter than the rest of the edge because it is
/// standing in for a light source above the card.
const CARD_HIGHLIGHT_ALPHA: u8 = 0x82;

/// How many pieces each of the four straight edges is cut into.
const CARD_EDGE_STEPS: usize = 4;

/// How many flat steps the perimeter ramp is cut into: four corner arcs plus
/// [`CARD_EDGE_STEPS`] pieces of each of the four edges.
///
/// Twenty rather than the twelve this started at. Twelve banded visibly along
/// the long edges of a tall card — the jump between adjacent steps is the ramp
/// divided by half the count, so halving the step doubles the smoothness for
/// eight more events, which is cheap against a payload already this size. The
/// count stays even so the halfway piece lands exactly on the bottom-right
/// corner and the ramp folds back on itself without a seam.
const CARD_SEGMENTS: usize = 4 + 4 * CARD_EDGE_STEPS;

/// Bézier circle constant: control points at `r * KAPPA` from the endpoints
/// approximate a quarter turn to within 0.03%.
const KAPPA: f32 = 0.552_284_8;

/// Fraction of the screen the card may fill before it is scaled down to fit.
const CARD_FIT: f32 = 0.94;

// Card metrics, all as multiples of the rendered *time* size, so the whole
// widget is one number wide and nothing in it is a fixed pixel count.

/// Padding inside the card's edge — measured **optically**, to the cap top of
/// the first row and to the bottom of the dial, not to a row's em box. The two
/// differ by [`CARD_CAP_GAP`], which is most of a quarter em and is exactly the
/// dead band that used to sit above the time.
const CARD_PAD: f32 = 0.46;
/// Corner radius. One value, used by the body, the border ring and (reduced by
/// its own inset, so the curves stay concentric) the scrim.
const CARD_RADIUS: f32 = 0.30;
/// Distance from the top of a line's em box — which is where `\an8` hangs it —
/// down to the cap top of the glyphs inside it, as a fraction of the type size.
/// Inter's ascent runs about a quarter em above its capitals, and *that* gap is
/// what an eye reads as space above the number. Every row's `\pos` is pulled up
/// by this so the rhythm below is expressed in cap tops.
const CARD_CAP_GAP: f32 = 0.25;
/// Cap height as a fraction of the em, for the same font. The rhythm measures
/// rows by their capitals, because that is the only part of them anyone sees.
const CARD_CAP_H: f32 = 0.72;
/// Leading from the time's baseline to the weekday's cap top.
const CARD_LEAD_DAY: f32 = 0.28;
/// Leading from the weekday's baseline to the date's cap top. Tighter than
/// [`CARD_LEAD_DAY`], so the three rows read as one block that gets denser
/// rather than as three evenly-spaced strangers.
const CARD_LEAD_DATE: f32 = 0.13;
/// Space between the text block and the dial. Deliberately about double the
/// largest leading inside the block: that ratio is the whole reason the eye
/// groups the three rows together and the face separately.
const CARD_GAP_FACE: f32 = 0.56;
/// Weekday type size.
const CARD_DAY_SIZE: f32 = 0.30;
/// Weekday tracking, as a fraction of the weekday size.
const CARD_DAY_TRACK: f32 = 0.16;
/// Date type size.
const CARD_DATE_SIZE: f32 = 0.26;
/// Date tracking, as a fraction of the date size.
const CARD_DATE_TRACK: f32 = 0.10;
/// Smallest dial radius, for when the text is narrow.
const CARD_FACE_R_MIN: f32 = 1.06;
/// Largest dial radius, for when it is not. The dial otherwise grows to fill
/// the width the type asks for — a fixed radius is what left a card sized for
/// `10:42:07 PM` mostly empty around a small face.
const CARD_FACE_R_MAX: f32 = 1.72;
/// Narrowest the card may be, whatever the text measures.
const CARD_MIN_W: f32 = 2.9;

// Dial furniture, all as fractions of the dial radius.

/// Outer edge of the tick ring. Hour and minute ticks share it, which is what
/// makes them read as one ring set in two weights rather than as two rings.
const CARD_TICK_OUTER: f32 = 0.905;
/// Hour tick length and half-width: twelve of them, long and heavy.
const CARD_TICK_HOUR: (f32, f32) = (0.155, 0.026);
/// Minute tick length and half-width: sixty hairlines, a third the length and a
/// third the weight, so the twelve read as structure and the sixty as texture.
const CARD_TICK_MIN: (f32, f32) = (0.05, 0.0085);
/// Below this dial radius **in drawing units** the sixty minute ticks are no
/// further apart at the rim than they are wide, so they stop being sixty marks
/// and become a grey smudge. Below it they are dropped entirely: twelve clean
/// hour ticks beat sixty smeared ones, and a small clock does not need a minute
/// ring to be read.
const CARD_MINUTE_TICK_MIN_R: f32 = 34.0;
/// Hour hand: a little over half the radius, and by a clear margin the widest
/// thing on the dial.
const CARD_HAND_HOUR: (f32, f32) = (0.55, 0.052);
/// Minute hand: much longer, visibly thinner. The length *and* weight both have
/// to differ or the two hands are indistinguishable when they overlap.
const CARD_HAND_MIN: (f32, f32) = (0.83, 0.031);
/// Second hand: longest and a hairline.
const CARD_HAND_SEC: (f32, f32) = (0.90, 0.0115);
/// Counterweight tail on the second hand. This is the detail that makes a face
/// look designed rather than drawn, and it doubles as the thing that tells the
/// eye which end of a hairline crossing the hub is the pointing end.
const CARD_TAIL_SEC: f32 = 0.23;
/// Stub tail on the hour and minute hands — just enough that they converge on
/// the hub instead of appearing to start at it.
const CARD_TAIL_HAND: f32 = 0.085;
/// The hub disc the hour and minute hands are pinned to.
const CARD_HUB_R: f32 = 0.086;
/// The second hand's own cap, sitting on top of the hub so the stack reads
/// front-to-back: hub, then hands, then cap.
const CARD_SEC_CAP_R: f32 = 0.036;

/// The card's text rows: time, weekday, and the date when [`shows_date`] says
/// so — separated by `'\n'`, like every other theme's [`format_time`].
///
/// The weekday is *not* the date line and is always present: it is the accent
/// row the look is built around, and a card with a hole where it goes is a
/// different design. `show_date` governs the `28 JUL 2025` row under it.
///
/// `%d %b %Y` for the same reason [`date_text`] uses `%a %d %b`: a month spelled
/// with letters cannot be read back-to-front, where `07/28` and `28/07` can.
fn card_text(now: DateTime<Local>, s: &ClockStyle) -> String {
    let mut out = numeric_time(now, s);
    out.push('\n');
    out.push_str(&now.format("%A").to_string().to_uppercase());
    if shows_date(s) {
        out.push('\n');
        out.push_str(&now.format("%d %b %Y").to_string().to_uppercase());
    }
    out
}

/// Where the three hands point, as angles clockwise from twelve o'clock.
///
/// Returned in `(hour, minute, second)` order, in radians.
///
/// The hour hand carries the minute fraction, because a real one does: at half
/// past ten it sits between the ten and the eleven, and a stepped hour hand is
/// the single detail that makes a drawn clock look broken. The minute hand
/// carries the second fraction **only when seconds are shown** — with them off
/// the payload has to be byte-identical for a whole minute, or the daemon's
/// "nothing changed" comparison stops working and the widget quietly starts
/// redrawing sixty times as often. That is also why the hour hand stops at the
/// minute: its second-order term is invisible and would cost the same.
fn card_hands(now: DateTime<Local>, seconds: bool) -> (f32, f32, f32) {
    let second = now.second() as f32;
    let minute = now.minute() as f32 + if seconds { second / 60.0 } else { 0.0 };
    let hour = (now.hour() % 12) as f32 + now.minute() as f32 / 60.0;
    (hour / 12.0 * TAU, minute / 60.0 * TAU, second / 60.0 * TAU)
}

/// Resolved card geometry in drawing units. Built once per render by
/// [`card_layout`], then read by everything that draws.
struct Card {
    /// Rendered time size, after the fit-to-screen clamp.
    size: f32,
    /// Card width.
    w: f32,
    /// Card height.
    h: f32,
    /// Corner radius.
    radius: f32,
    /// `\an8` y for the time row — already pulled up by [`CARD_CAP_GAP`], so
    /// the row's *capitals* land where the rhythm put them.
    time_top: f32,
    /// `\an8` y for the weekday row, corrected the same way at its own size.
    day_top: f32,
    /// `\an8` y for the date row, when there is one.
    date_top: Option<f32>,
    /// Bottom edge of the text block's scrim.
    scrim_bot: f32,
    /// Dial radius.
    face_r: f32,
    /// Dial centre.
    face_cx: f32,
    /// Dial centre.
    face_cy: f32,
}

/// Lay the card out for these three strings at this type size.
///
/// The card is sized to its content rather than to a fixed ratio, because the
/// content changes width by a factor of two: `9:05` against `10:42:07 PM`. The
/// widths come from [`em_width`], a crude per-character estimate — libass owns
/// the real metrics and this module has no access to them, so the card is drawn
/// a little generous and the rows are centred, which turns a bad estimate into
/// slightly uneven padding instead of clipped text.
///
/// Everything is linear in the type size, so the fit-to-screen clamp is exact:
/// the layout is computed once per unit of size and then multiplied by the
/// largest size that still fits [`CARD_FIT`] of the overlay space.
///
/// # The rhythm
///
/// The vertical run is authored in **cap tops**, not in row boxes. A row box
/// includes the font's ascent and descent, and digits use neither, so laying
/// rows out by their boxes puts a quarter em of dead band above the time and
/// another under every row — which is what made the old card read as top-heavy
/// with a hole in the middle. Each row's `\pos` is therefore its cap top pulled
/// back up by [`CARD_CAP_GAP`] at that row's own size.
///
/// On top of that the spacing is deliberately uneven, because even spacing is
/// what stops a stack from grouping: the three text rows sit at
/// [`CARD_LEAD_DAY`]/[`CARD_LEAD_DATE`] — tight, and getting tighter down the
/// stack — and the dial is [`CARD_GAP_FACE`] away, about double the largest of
/// them. Two groups, one gap between them. The padding above the time and below
/// the dial are the same [`CARD_PAD`], so the composition sits optically centred
/// in the card rather than arithmetically centred in a box.
///
/// # Why the dial is sized here
///
/// The card is as wide as its widest row, and that row is the time, which
/// swings by a factor of two between `9:05` and `10:42:07 PM`. A fixed dial
/// radius therefore left the widest cards mostly empty air around a small face.
/// The dial instead grows to fill the width the type asks for, bounded by
/// [`CARD_FACE_R_MIN`] and [`CARD_FACE_R_MAX`], so the face is always a
/// substantial fraction of the card it is on.
fn card_layout(size_pt: u32, time: &str, day: &str, date: Option<&str>) -> Card {
    let row = |text: &str, size: f32, track: f32| {
        (em_width(text) + track * text.chars().count() as f32) * size
    };
    let text_w = [
        row(time, 1.0, 0.0),
        row(day, CARD_DAY_SIZE, CARD_DAY_TRACK),
        date.map_or(0.0, |d| row(d, CARD_DATE_SIZE, CARD_DATE_TRACK)),
    ]
    .into_iter()
    .fold(0.0f32, f32::max);
    let face_r = (text_w / 2.0).clamp(CARD_FACE_R_MIN, CARD_FACE_R_MAX);
    let unit_w = (text_w.max(2.0 * face_r) + 2.0 * CARD_PAD).max(CARD_MIN_W);

    // Cap tops, top down. `y` is always the cap top of the row about to be
    // placed; a row consumes its own cap height and then its leading.
    let mut y = CARD_PAD;
    let time_cap = y;
    y += CARD_CAP_H + CARD_LEAD_DAY;
    let day_cap = y;
    y += CARD_CAP_H * CARD_DAY_SIZE;
    let date_cap = if date.is_some() {
        y += CARD_LEAD_DATE;
        let top = y;
        y += CARD_CAP_H * CARD_DATE_SIZE;
        Some(top)
    } else {
        None
    };
    // The baseline of the last text row: the bottom of the text block, and what
    // the scrim and the face gap are both measured from.
    let text_bot = y;
    y += CARD_GAP_FACE;
    let face_cy = y + face_r;
    let unit_h = y + 2.0 * face_r + CARD_PAD;

    // `unit_w`/`unit_h` are at least CARD_MIN_W and a couple of units, so
    // neither division can be by zero and neither result can be NaN.
    let size = (size_pt as f32)
        .min(PLAY_RES_X as f32 * CARD_FIT / unit_w)
        .min(PLAY_RES_Y as f32 * CARD_FIT / unit_h)
        .max(MIN_SIZE_PT as f32);
    // The scrim gives out half way down the gap between the type and the dial,
    // where its (heavily blurred) edge has the most room to be nothing in
    // particular. Ending it tight under the type put a readable horizontal seam
    // across the middle of the glass.
    let bleed = CARD_GAP_FACE / 2.0;
    Card {
        size,
        w: unit_w * size,
        h: unit_h * size,
        radius: CARD_RADIUS * size,
        time_top: (time_cap - CARD_CAP_GAP) * size,
        day_top: (day_cap - CARD_CAP_GAP * CARD_DAY_SIZE) * size,
        date_top: date_cap.map(|t| (t - CARD_CAP_GAP * CARD_DATE_SIZE) * size),
        scrim_bot: (text_bot + bleed) * size,
        face_r: face_r * size,
        face_cx: unit_w * size / 2.0,
        face_cy: face_cy * size,
    }
}

/// Rough advance width of a string, in ems of its own type size.
///
/// Deliberately crude. The real widths live in a font this module cannot open,
/// and the only decision that depends on them is how wide to draw the panel —
/// so the estimate is tuned to run slightly wide on the faces the theme asks
/// for, and the rows are centred so an error shows as padding, not as clipping.
fn em_width(text: &str) -> f32 {
    text.chars()
        .map(|c| match c {
            '0'..='9' => 0.58,
            ':' => 0.30,
            ' ' => 0.28,
            '.' | ',' | '\'' => 0.26,
            _ => 0.62,
        })
        .sum()
}

/// Top-left corner of the card in the `PLAY_RES` space.
///
/// The nine-point anchor and the margin mean the same thing they do for text —
/// see `lyrics::anchor_pos`, which this mirrors and which a test pins it
/// against — except that the card is a known size, so the anchor point is
/// converted into a corner here instead of being handed to `\an`. Every card
/// event then uses `\an7`, which is what makes the panel, the dial and the type
/// share one coordinate system.
///
/// Clamped onto the screen last: at a hand-edited size the card can be wider
/// than the output, and half a card is better than none of one.
fn card_origin(anchor: Anchor, margin: u32, w: f32, h: f32) -> (f32, f32) {
    let n = anchor.an() - 1;
    let mx = f32::from(u16::try_from(margin.min(PLAY_RES_X / 2)).unwrap_or(u16::MAX));
    let my = f32::from(u16::try_from(margin.min(PLAY_RES_Y / 2)).unwrap_or(u16::MAX));
    let (rx, ry) = (PLAY_RES_X as f32, PLAY_RES_Y as f32);
    let x = match n % 3 {
        0 => mx,
        1 => (rx - w) / 2.0,
        _ => rx - mx - w,
    };
    let y = match n / 3 {
        0 => ry - my - h,
        1 => (ry - h) / 2.0,
        _ => my,
    };
    (
        x.clamp(0.0, (rx - w).max(0.0)),
        y.clamp(0.0, (ry - h).max(0.0)),
    )
}

/// One geometry value as one emittable drawing unit.
///
/// The cast is the guarantee, not a convenience: Rust defines float-to-integer
/// casts as saturating, with `NaN` mapping to zero, so no value of `v` can
/// produce anything but an integer. Every number in a card drawing goes through
/// here.
fn du(v: f32) -> i32 {
    v.round() as i32
}

/// Cartesian point at `angle` on the circle `(cx, cy, r)`. ASS screen angles:
/// y grows downward, so they run clockwise from three o'clock.
fn polar(cx: f32, cy: f32, r: f32, angle: f32) -> (f32, f32) {
    (cx + r * angle.cos(), cy + r * angle.sin())
}

/// A clock angle — clockwise from twelve — as the screen angle [`polar`] wants.
fn dial(angle: f32) -> f32 {
    angle - FRAC_PI_2
}

/// Accumulates one ASS drawing path in card-local units.
///
/// The same two invariants `crate::visualizer`'s path builder carries, for the
/// same two reasons: no float is ever formatted, and nothing is drawn outside
/// the pinned box.
struct CardPath {
    /// Box width as the clamp bound.
    xmax: i32,
    /// Box height as the clamp bound.
    ymax: i32,
    /// The path built so far, space-separated.
    d: String,
}

impl CardPath {
    fn new(w: f32, h: f32) -> Self {
        CardPath {
            xmax: du(w).clamp(1, PLAY_RES_X as i32),
            ymax: du(h).clamp(1, PLAY_RES_Y as i32),
            d: String::new(),
        }
    }

    /// The only float-to-text boundary in the card renderer.
    fn pt(&self, x: f32, y: f32) -> (i32, i32) {
        (du(x).clamp(0, self.xmax), du(y).clamp(0, self.ymax))
    }

    /// Fix the drawing's bounding box to the whole card.
    ///
    /// Two zero-area contours at opposite corners. libass sizes a drawing from
    /// the bounding box of its points, so without these the *hands* would
    /// resize the box every minute and the card would crawl across the
    /// wallpaper.
    ///
    /// Each is a bare `m` with **no segment after it**, and that detail is the
    /// difference between an invisible pin and a visible defect. A contour of
    /// coincident points — `m 0 0 l 0 0 l 0 0`, which this used to emit — is
    /// zero-area and so contributes no *fill*, but it is still an outline, and
    /// `\bord` strokes it into a dot the width of the border. On the twenty
    /// neon border events, whose border is the glow, that dot was a bright
    /// speck sitting just outside each of the card's two rounded corners, in
    /// the ramp colour of whichever segment happened to be painted last. A lone
    /// move contributes its point to the bounding box and produces no outline
    /// at all, so the pin does its whole job and draws nothing.
    fn pin(&mut self) {
        let (x, y) = (self.xmax, self.ymax);
        let _ = write!(self.d, "m 0 0 m {x} {y} ");
    }

    fn move_to(&mut self, p: (f32, f32)) {
        let (x, y) = self.pt(p.0, p.1);
        let _ = write!(self.d, "m {x} {y} ");
    }

    fn line_to(&mut self, p: (f32, f32)) {
        let (x, y) = self.pt(p.0, p.1);
        let _ = write!(self.d, "l {x} {y} ");
    }

    fn curve_to(&mut self, c1: (f32, f32), c2: (f32, f32), p: (f32, f32)) {
        let (x1, y1) = self.pt(c1.0, c1.1);
        let (x2, y2) = self.pt(c2.0, c2.1);
        let (x3, y3) = self.pt(p.0, p.1);
        let _ = write!(self.d, "b {x1} {y1} {x2} {y2} {x3} {y3} ");
    }

    /// One cubic approximating the arc from `a0` to `a1`, assuming the current
    /// point is already the arc's start. Control points sit on the endpoint
    /// tangents at `4/3·tan(θ/4)·r`; exact at both ends, under 0.03% radial
    /// error over a quarter turn.
    fn arc_to(&mut self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let k = 4.0 / 3.0 * ((a1 - a0) / 4.0).tan() * r;
        let p0 = polar(cx, cy, r, a0);
        let p1 = polar(cx, cy, r, a1);
        let c1 = (p0.0 - k * a0.sin(), p0.1 + k * a0.cos());
        let c2 = (p1.0 + k * a1.sin(), p1.1 - k * a1.cos());
        self.curve_to(c1, c2, p1);
    }

    /// A full circle as four cubic arcs.
    fn circle(&mut self, cx: f32, cy: f32, r: f32) {
        self.move_to(polar(cx, cy, r, 0.0));
        for i in 0..4 {
            let a0 = i as f32 * FRAC_PI_2;
            self.arc_to(cx, cy, r, a0, a0 + FRAC_PI_2);
        }
    }

    /// A rounded rectangle, clockwise from the top-left corner.
    fn round_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) {
        let r = r
            .max(0.0)
            .min(((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0));
        if r < 1.0 {
            self.move_to((x0, y0));
            self.line_to((x1, y0));
            self.line_to((x1, y1));
            self.line_to((x0, y1));
            return;
        }
        let k = r * (1.0 - KAPPA);
        self.move_to((x0 + r, y0));
        self.line_to((x1 - r, y0));
        self.curve_to((x1 - k, y0), (x1, y0 + k), (x1, y0 + r));
        self.line_to((x1, y1 - r));
        self.curve_to((x1, y1 - k), (x1 - k, y1), (x1 - r, y1));
        self.line_to((x0 + r, y1));
        self.curve_to((x0 + k, y1), (x0, y1 - k), (x0, y1 - r));
        self.line_to((x0, y0 + r));
        self.curve_to((x0, y0 + k), (x0 + k, y0), (x0 + r, y0));
    }

    /// A straight bar of half-width `hw` from `a` to `b`, its ends extended by
    /// `hw` so consecutive bars butt together instead of leaving a notch.
    ///
    /// A zero-length bar is skipped rather than normalised: dividing by its
    /// length is where a `NaN` would come from, and one `NaN` costs the drawing.
    ///
    /// Wound the *same way round* as [`CardPath::circle`] and
    /// [`CardPath::round_rect`], and that is load-bearing rather than tidy.
    /// libass fills a drawing by an even-odd rule across all of its contours, so
    /// two overlapping shapes wound in opposite directions cancel each other out
    /// in the overlap instead of merging. Wound the other way, the hub disc and
    /// the hands crossing it punched a hole through the centre of the dial —
    /// which is exactly what the undersized, hollow-looking hub was.
    fn bar(&mut self, a: (f32, f32), b: (f32, f32), hw: f32, extend: bool) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy);
        if !len.is_finite() || len <= f32::EPSILON || !hw.is_finite() {
            return;
        }
        let (ux, uy) = (dx / len, dy / len);
        let e = if extend { hw } else { 0.0 };
        let (a, b) = ((a.0 - ux * e, a.1 - uy * e), (b.0 + ux * e, b.1 + uy * e));
        let (px, py) = (-uy * hw, ux * hw);
        self.move_to((a.0 - px, a.1 - py));
        self.line_to((b.0 - px, b.1 - py));
        self.line_to((b.0 + px, b.1 + py));
        self.line_to((a.0 + px, a.1 + py));
    }

    /// A rectangle whose *top* two corners are rounded to `r` and whose bottom
    /// edge is straight, from `(x0, y0)` to `(x1, y1)`.
    ///
    /// The scrim's shape. Its top and sides trace the card's own outline
    /// exactly, so those three edges are invisible — only the straight bottom
    /// edge shows, and blur turns that into a falloff rather than a border.
    fn hood(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) {
        let r = r
            .max(0.0)
            .min(((x1 - x0) / 2.0).min((y1 - y0).max(0.0)).max(0.0));
        if r < 1.0 {
            self.move_to((x0, y0));
            self.line_to((x1, y0));
            self.line_to((x1, y1));
            self.line_to((x0, y1));
            return;
        }
        let k = r * (1.0 - KAPPA);
        self.move_to((x0 + r, y0));
        self.line_to((x1 - r, y0));
        self.curve_to((x1 - k, y0), (x1, y0 + k), (x1, y0 + r));
        self.line_to((x1, y1));
        self.line_to((x0, y1));
        self.line_to((x0, y0 + r));
        self.curve_to((x0, y0 + k), (x0 + k, y0), (x0 + r, y0));
    }

    /// A sector of an annulus: out along the outer radius, back along the
    /// inner one. Used for the card's rounded corners, where the border has to
    /// turn without the join opening up.
    fn ring_arc(&mut self, cx: f32, cy: f32, r_in: f32, r_out: f32, a0: f32, a1: f32) {
        self.move_to(polar(cx, cy, r_out, a0));
        self.arc_to(cx, cy, r_out, a0, a1);
        self.line_to(polar(cx, cy, r_in, a1));
        self.arc_to(cx, cy, r_in, a1, a0);
    }

    /// The finished path, with the trailing separator removed.
    fn finish(self) -> String {
        let mut d = self.d;
        while d.ends_with(' ') {
            d.pop();
        }
        d
    }
}

/// Everything an ASS event needs to know about ink, minus the shape.
struct Pen {
    /// Fill colour as `#RRGGBB`.
    fill: String,
    /// Fill alpha; ASS runs backwards, `0x00` opaque.
    alpha: u8,
    /// Border colour as `#RRGGBB`.
    edge: String,
    /// Border alpha, on the same backwards scale as `alpha`. This is what lets
    /// the glass edge be a hairline of *nearly transparent white* rather than a
    /// solid stroke — the difference between a lit edge and a drawn one.
    edge_alpha: u8,
    /// Border width in drawing units.
    bord: u32,
    /// Gaussian edge blur — what turns a border into a glow. Note this softens
    /// the shape's *own* edge; ASS has nothing that blurs the backdrop.
    blur: u32,
}

/// One drawing event, placed by its top-left corner.
fn draw_event(at: (f32, f32), pen: &Pen, body: &str) -> String {
    format!(
        // `\fscx\fscy\frz` are set because libass scales and rotates drawings
        // by them exactly as it does glyphs, and this event inherits nothing.
        "{{\\an7\\pos({x},{y})\\fscx100\\fscy100\\frz0\\bord{bord}\\shad0\\blur{blur}\
         \\1c{fill}\\3c{edge}\\1a&H{alpha:02X}&\\3a&H{edge_alpha:02X}&\\4a&HFF&\\p1}}{body}{{\\p0}}",
        x = du(at.0),
        y = du(at.1),
        bord = pen.bord,
        blur = pen.blur,
        fill = lyrics::hex_to_ass_colour(&pen.fill),
        edge = lyrics::hex_to_ass_colour(&pen.edge),
        alpha = pen.alpha,
        edge_alpha = pen.edge_alpha,
    )
}

/// How one text row is set.
struct RowStyle {
    /// Type size in drawing units.
    size: u32,
    /// Bold weight.
    bold: bool,
    /// Letter spacing in drawing units.
    fsp: u32,
    /// Fill colour as `#RRGGBB`.
    colour: String,
    /// Fill alpha; ASS runs backwards, `0x00` opaque.
    alpha: u8,
}

/// One text event, centred horizontally on `at` and hung from its top edge.
///
/// The outline is a hairline — one drawing unit, or a fortieth of the type size
/// on a large card — where every other theme carries a heavy black one. That is
/// the whole point of a card: the panel underneath is a known dark surface, so
/// the outline is no longer doing the work of keeping white type off a white
/// video frame and can go back to being what an outline is for, which is
/// keeping the edges crisp. A heavy one here would only thicken the letters.
fn text_event(at: (f32, f32), row: &RowStyle, text: &str) -> String {
    format!(
        "{{\\an8\\pos({x},{y})\\fnInter\\fs{size}\\b{bold}\\fsp{fsp}\\i0\
         \\bord{bord}\\shad0\\blur0\\1c{colour}\\3c&H000000&\\1a&H{alpha:02X}&\\3a&H00&\\4a&HFF&}}{body}",
        x = du(at.0),
        y = du(at.1),
        size = row.size,
        bold = u8::from(row.bold),
        fsp = row.fsp,
        // Integer division on purpose, so the outline simply does not exist
        // below forty units of type. A one-unit black outline is a hairline on
        // the time and a tenth of the em on the date row, where it stops
        // sharpening the glyphs and starts filling them in with grit. The
        // contrast it used to be defending is now [`card_scrim`]'s job, so the
        // small rows can go without it.
        bord = row.size / 40,
        colour = lyrics::hex_to_ass_colour(&row.colour),
        alpha = row.alpha,
        body = lyrics::ass_escape(text),
    )
}

/// `#RRGGBB` for a channel triple.
fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// A flat step `t` of the way from [`CARD_NEON_A`] to [`CARD_NEON_B`].
///
/// A *step*, not a gradient: ASS cannot interpolate colour across a shape, so
/// the card's edge is a run of separately-coloured pieces and this picks the
/// colour of one of them.
fn ramp(t: f32) -> String {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let mix = |i: usize| {
        let (a, b) = (f32::from(CARD_NEON_A[i]), f32::from(CARD_NEON_B[i]));
        (a + (b - a) * t).clamp(0.0, 255.0) as u8
    };
    hex([mix(0), mix(1), mix(2)])
}

/// One piece of the card's perimeter.
enum Seg {
    /// A corner: a quarter turn of the border's centre line.
    Arc {
        /// Centre of the corner's radius.
        c: (f32, f32),
        /// Start angle, in screen radians.
        a0: f32,
        /// End angle, in screen radians.
        a1: f32,
    },
    /// A straight run along one edge.
    Line {
        /// Where the run starts.
        a: (f32, f32),
        /// Where it ends.
        b: (f32, f32),
    },
}

/// The perimeter cut into [`CARD_SEGMENTS`] pieces, clockwise from the
/// top-left corner: corner, half an edge, half an edge, corner, …
///
/// The order is what makes the ramp read as a diagonal: piece 0 sits at the
/// top-left and piece `CARD_SEGMENTS / 2` at the bottom-right, so a parameter
/// that rises to the halfway piece and falls back arrives at both ends of the
/// loop on the same colour — no seam where the ramp wraps.
fn card_perimeter(c: &Card, hw: f32) -> Vec<Seg> {
    let (x0, y0) = (hw, hw);
    let (x1, y1) = (c.w - hw, c.h - hw);
    let r = (c.radius - hw).max(1.0);
    // One edge, cut into `CARD_EDGE_STEPS` equal runs. Splitting evenly (rather
    // than by length) keeps the ramp's rate of change the same on the short
    // edges as on the long ones, which is what stops a tall card from banding
    // down its sides while its top looks smooth.
    let steps = |a: (f32, f32), b: (f32, f32)| {
        let point = move |k: usize| {
            let t = k as f32 / CARD_EDGE_STEPS as f32;
            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
        };
        (0..CARD_EDGE_STEPS).map(move |i| Seg::Line {
            a: point(i),
            b: point(i + 1),
        })
    };
    let mut out = Vec::with_capacity(CARD_SEGMENTS);
    // Top-left corner, then the top edge left to right.
    out.push(Seg::Arc {
        c: (x0 + r, y0 + r),
        a0: PI,
        a1: 1.5 * PI,
    });
    out.extend(steps((x0 + r, y0), (x1 - r, y0)));
    out.push(Seg::Arc {
        c: (x1 - r, y0 + r),
        a0: 1.5 * PI,
        a1: TAU,
    });
    out.extend(steps((x1, y0 + r), (x1, y1 - r)));
    out.push(Seg::Arc {
        c: (x1 - r, y1 - r),
        a0: 0.0,
        a1: FRAC_PI_2,
    });
    out.extend(steps((x1 - r, y1), (x0 + r, y1)));
    out.push(Seg::Arc {
        c: (x0 + r, y1 - r),
        a0: FRAC_PI_2,
        a1: PI,
    });
    out.extend(steps((x0, y1 - r), (x0, y0 + r)));
    out
}

/// Render the `Card` theme: several newline-separated ASS events, painted in
/// the order they appear.
fn render_card(now: DateTime<Local>, s: &ClockStyle, accent_hex: &str) -> String {
    let text = card_text(now, s);
    let mut rows = text.split('\n');
    let time = rows.next().unwrap_or_default();
    let day = rows.next().unwrap_or_default();
    let date = rows.next();

    let size_pt = scaled(s.font_size_pt, look_for(ClockTheme::Card).size_pct);
    let c = card_layout(size_pt, time, day, date);
    let at = card_origin(s.anchor, s.margin_px, c.w, c.h);
    let fill = if s.accent_follow {
        accent_hex
    } else {
        s.colour.as_str()
    };

    // Payload order is paint order, and this is the depth stack: the glass
    // body, the scrim shading its upper half, the lit top edge, the neon rim
    // over both, then the dial and its furniture back to front, then the type.
    // The scrim goes *under* the border on purpose — its blur spreads past the
    // card's own sides, and the border is what covers that fringe.
    let mut out: Vec<String> = Vec::with_capacity(CARD_SEGMENTS + 10);
    out.push(card_panel(&c, at));
    out.push(card_scrim(&c, at));
    out.push(card_highlight(&c, at));
    out.extend(card_border(&c, at));
    out.push(card_dial(&c, at));
    out.extend(card_ticks(&c, at, fill));
    out.extend(card_hand_events(&c, at, now, s, fill));
    out.extend(card_rows(&c, at, time, day, date, fill));
    out.join("\n")
}

/// The glass body: one rounded rectangle, translucent, with a hairline lit
/// edge.
///
/// The border here is a *hairline at low alpha*, not the wide blurred halo in
/// the ramp's midpoint colour this used to carry. That halo was the muddy ring
/// the design review called out, and it was muddy for a structural reason: the
/// midpoint of a pink→cyan ramp is a desaturated lavender that belongs to
/// neither end, so a single averaged glow could only ever fight the border it
/// was sitting under. The glow is now the border's own job — each of
/// [`card_border`]'s segments carries a blur in *its* colour, so the halo
/// matches the edge everywhere along it instead of nowhere.
fn card_panel(c: &Card, at: (f32, f32)) -> String {
    let mut p = CardPath::new(c.w, c.h);
    p.pin();
    p.round_rect(0.0, 0.0, c.w, c.h, c.radius);
    draw_event(
        at,
        &Pen {
            fill: CARD_PANEL.to_string(),
            alpha: CARD_BODY_ALPHA,
            edge: CARD_EDGE.to_string(),
            edge_alpha: CARD_EDGE_ALPHA,
            bord: hairline(c),
            blur: 1,
        },
        &p.finish(),
    )
}

/// The top-edge highlight: a soft bright bar just inside the upper edge.
///
/// One stroke, on one edge, because that is what a light source above the card
/// would actually produce — lighting all four edges equally reads as an outline
/// and tells the eye nothing. Blurred rather than crisp so it fades out at both
/// ends instead of stopping, and inset by the border thickness so it sits *on
/// the glass* rather than on the neon.
fn card_highlight(c: &Card, at: (f32, f32)) -> String {
    let inset = c.radius * 0.5 + border_hw(c) * 2.0;
    let y = inset.min(c.h / 4.0);
    let mut p = CardPath::new(c.w, c.h);
    p.pin();
    p.bar(
        (c.radius, y),
        (c.w - c.radius, y),
        (c.size * 0.02).max(0.8),
        false,
    );
    draw_event(
        at,
        &Pen {
            fill: CARD_EDGE.to_string(),
            alpha: CARD_HIGHLIGHT_ALPHA,
            edge: CARD_EDGE.to_string(),
            edge_alpha: 0xFF,
            bord: 0,
            blur: (c.size * 0.13).clamp(4.0, 30.0) as u32,
        },
        &p.finish(),
    )
}

/// The scrim the text block sits on.
///
/// This is the stand-in for the backdrop blur ASS cannot do. Real glassmorphism
/// blurs what is behind the surface, and the reason that matters is not that it
/// looks nice — it is that a blurred backdrop has no high-frequency detail and
/// no local extremes, so type stays legible over it. A merely *translucent*
/// card has neither property: over a bright wallpaper the body alone leaves
/// white type at about 1.3:1, which is not legible at any size.
///
/// So the contrast is bought back the only way the substrate allows, with a
/// second darker plate — but only under the type, and feathered hard enough by
/// [`Pen::blur`] that it reads as the type being shaded rather than as a box.
/// Its corners are the card's radius less its own inset, which is what keeps
/// two nested rounded rectangles looking concentric instead of merely both
/// rounded.
fn card_scrim(c: &Card, at: (f32, f32)) -> String {
    let mut p = CardPath::new(c.w, c.h);
    p.pin();
    p.hood(0.0, 0.0, c.w, c.scrim_bot, c.radius);
    draw_event(
        at,
        &Pen {
            fill: CARD_PANEL.to_string(),
            alpha: CARD_SCRIM_ALPHA,
            edge: CARD_PANEL.to_string(),
            edge_alpha: 0xFF,
            bord: 0,
            blur: (c.size * 0.22).clamp(6.0, 48.0) as u32,
        },
        &p.finish(),
    )
}

/// Half the neon border's thickness, in drawing units.
fn border_hw(c: &Card) -> f32 {
    (c.size * 0.042).clamp(1.5, 9.0) / 2.0
}

/// A hairline, in drawing units: one unit at the sizes where one unit is a
/// hairline, and proportional once the card is big enough that it would not be.
fn hairline(c: &Card) -> u32 {
    (c.size * 0.014).clamp(1.0, 3.0) as u32
}

/// The neon edge: [`CARD_SEGMENTS`] flat steps along the ramp, one event each.
fn card_border(c: &Card, at: (f32, f32)) -> Vec<String> {
    let hw = border_hw(c);
    let glow = (c.size * 0.045).clamp(2.0, 10.0) as u32;
    card_perimeter(c, hw)
        .into_iter()
        .enumerate()
        .map(|(i, seg)| {
            let mut p = CardPath::new(c.w, c.h);
            p.pin();
            match seg {
                Seg::Arc { c: o, a0, a1 } => {
                    // Overlap the neighbours by half a thickness of arc so the
                    // steps meet without a hairline of wallpaper between them.
                    let r = (c.radius - hw).max(1.0);
                    let pad = hw / r;
                    p.ring_arc(o.0, o.1, r - hw, r + hw, a0 - pad, a1 + pad);
                }
                // Collinear runs butt exactly, so they need no overlap — and
                // must not have one: two blurred ends laid on top of each other
                // sum to a bright bead, which is what beaded every edge once
                // the segment count went up.
                Seg::Line { a, b } => p.bar(a, b, hw, false),
            }
            // Rises to the halfway piece and falls back, so the loop closes on
            // one colour instead of butting pink against cyan at the corner.
            let u = i as f32 / (CARD_SEGMENTS as f32 / 2.0);
            let colour = ramp(if u <= 1.0 { u } else { 2.0 - u });
            draw_event(
                at,
                &Pen {
                    fill: colour.clone(),
                    alpha: 0x00,
                    // The glow is this piece's own colour, so the halo agrees
                    // with the edge everywhere round the card. One averaged
                    // colour for the whole perimeter is what read as mud.
                    edge: colour,
                    edge_alpha: 0x00,
                    bord: glow,
                    blur: glow,
                },
                &p.finish(),
            )
        })
        .collect()
}

/// The dial: a darker disc, inset from the card, with a hairline lit rim.
///
/// The rim is the same nearly-transparent white as the card's own edge, not the
/// ramp's midpoint colour it used to be. A lavender ring round the face was
/// competing with a pink-to-cyan border eight units away and losing — two
/// saturated rings on one small card is one too many. A neutral hairline still
/// reads as a bezel and stops arguing with the neon.
fn card_dial(c: &Card, at: (f32, f32)) -> String {
    let mut p = CardPath::new(c.w, c.h);
    p.pin();
    p.circle(c.face_cx, c.face_cy, c.face_r);
    draw_event(
        at,
        &Pen {
            fill: CARD_FACE.to_string(),
            alpha: CARD_FACE_ALPHA,
            edge: CARD_EDGE.to_string(),
            edge_alpha: CARD_EDGE_ALPHA,
            bord: hairline(c),
            blur: 0,
        },
        &p.finish(),
    )
}

/// The rim marks: twelve hour ticks and sixty minute ticks, as two events.
///
/// Two events rather than one is the whole point. Ticks are the part of a face
/// that decides whether it looks made or drawn, and what makes them look made
/// is that the twelve and the sixty are obviously not the same mark: the hour
/// ticks are three times the length and three times the width of the minute
/// ticks, at a higher alpha, so the twelve read as structure and the sixty as
/// texture behind them. Sharing an outer radius ([`CARD_TICK_OUTER`]) is what
/// keeps them one ring rather than two.
///
/// Both widths are derived from the dial radius rather than the type size, so a
/// tick keeps its proportion to the circle it is on at every size — the old
/// marks drifted toward a common chunky width as the card shrank, which is why
/// they read as uneven blocks.
///
/// Below [`CARD_MINUTE_TICK_MIN_R`] the minute ring is **not drawn at all**.
/// Sixty marks round a small dial land closer together than a hairline is wide,
/// and libass resolves that into a grey smear that looks like dirt. Omitting
/// them is the design decision; drawing them badly is not.
fn card_ticks(c: &Card, at: (f32, f32), fill: &str) -> Vec<String> {
    let r = c.face_r;
    let mut bands: Vec<(bool, u8, (f32, f32), f32)> = vec![(true, 0x0Cu8, CARD_TICK_HOUR, 1.0)];
    if r >= CARD_MINUTE_TICK_MIN_R {
        bands.push((false, 0x8Eu8, CARD_TICK_MIN, 0.5));
    }
    bands
        .into_iter()
        .map(|(hour, alpha, (len, hw), floor)| {
            let mut p = CardPath::new(c.w, c.h);
            p.pin();
            let r1 = r * CARD_TICK_OUTER;
            let r0 = r1 - r * len;
            let hw = (r * hw).max(floor);
            for i in 0..60 {
                if (i % 5 == 0) != hour {
                    continue;
                }
                let a = dial(i as f32 / 60.0 * TAU);
                p.bar(
                    polar(c.face_cx, c.face_cy, r0, a),
                    polar(c.face_cx, c.face_cy, r1, a),
                    hw,
                    false,
                );
            }
            draw_event(
                at,
                &Pen {
                    fill: fill.to_string(),
                    alpha,
                    edge: "#000000".to_string(),
                    edge_alpha: 0x00,
                    bord: 0,
                    blur: 0,
                },
                &p.finish(),
            )
        })
        .collect()
}

/// The hands. Hour and minute share one event because they share a colour; the
/// second hand gets its own so it can be the contrasting one, and it is drawn
/// **only when seconds are shown** — a second hand frozen for a minute at a
/// time is worse than no second hand, and pushing one per second to avoid that
/// is the power regression this module exists to prevent.
///
/// # The hierarchy is the design
///
/// The hands used to run 0.50r and 0.78r at widths of 0.055r and 0.038r —
/// close enough on both axes that at 3:16, where they nearly coincide, the face
/// showed one fat white blob and told you nothing. Watchmaking convention exists
/// because it solves exactly that, so the proportions now follow it:
///
/// | hand | length | width |
/// |---|---|---|
/// | hour | [`CARD_HAND_HOUR`] — a little over half the radius | by far the widest |
/// | minute | [`CARD_HAND_MIN`] — reaches the tick ring | visibly thinner |
/// | second | [`CARD_HAND_SEC`] — longest | a hairline, and a different colour |
///
/// Length and weight both separate, in the same direction, so overlapping hands
/// still resolve: the short fat one is the hour, whatever it is lying on top of.
///
/// The stack at the centre is three deep and is drawn back to front — hub disc,
/// then the hands over it, then the second hand's own smaller cap over those.
/// The second hand also carries a counterweight tail past the hub, which is both
/// the detail that makes a face look designed and the thing that says which end
/// of a hairline through the centre is the pointing end.
fn card_hand_events(
    c: &Card,
    at: (f32, f32),
    now: DateTime<Local>,
    s: &ClockStyle,
    fill: &str,
) -> Vec<String> {
    let seconds = shows_seconds(s);
    let (h, m, sec) = card_hands(now, seconds);
    let r = c.face_r;
    let hub = (c.face_cx, c.face_cy);
    let hand = |p: &mut CardPath, angle: f32, tip: f32, tail: f32, hw: f32| {
        let a = dial(angle);
        p.bar(
            polar(hub.0, hub.1, -tail, a),
            polar(hub.0, hub.1, tip, a),
            hw.max(1.0),
            false,
        );
    };

    // The hub disc goes down first so the hands lie *on* it; the shapes are in
    // one event and one colour, so where they overlap they simply merge.
    let mut p = CardPath::new(c.w, c.h);
    p.pin();
    p.circle(hub.0, hub.1, (r * CARD_HUB_R).max(2.0));
    let tail = r * CARD_TAIL_HAND;
    hand(&mut p, h, r * CARD_HAND_HOUR.0, tail, r * CARD_HAND_HOUR.1);
    hand(&mut p, m, r * CARD_HAND_MIN.0, tail, r * CARD_HAND_MIN.1);
    let mut out = vec![draw_event(
        at,
        &Pen {
            fill: fill.to_string(),
            alpha: 0x00,
            // A dark rim keeps a white hand legible where it crosses a tick.
            edge: "#000000".to_string(),
            edge_alpha: 0x40,
            bord: (c.size * 0.010).clamp(1.0, 3.0) as u32,
            blur: 0,
        },
        &p.finish(),
    )];

    if seconds {
        let mut p = CardPath::new(c.w, c.h);
        p.pin();
        hand(
            &mut p,
            sec,
            r * CARD_HAND_SEC.0,
            r * CARD_TAIL_SEC,
            r * CARD_HAND_SEC.1,
        );
        // On top of the hands, and smaller than the hub under them, so the
        // centre reads as three distinct layers rather than one lump.
        p.circle(hub.0, hub.1, (r * CARD_SEC_CAP_R).max(1.5));
        out.push(draw_event(
            at,
            &Pen {
                fill: hex(CARD_NEON_A),
                alpha: 0x00,
                // A much lighter rim than the white hands carry. A saturated
                // hairline on a near-black dial needs almost no separation of
                // its own, and at the weight the white hands use it was biting
                // visible notches out of them wherever the two crossed.
                edge: "#000000".to_string(),
                edge_alpha: 0x90,
                bord: (c.size * 0.007).clamp(1.0, 2.0) as u32,
                blur: 0,
            },
            &p.finish(),
        ));
    }
    out
}

/// The three text rows, each centred on the card and hung from its own top.
fn card_rows(
    c: &Card,
    at: (f32, f32),
    time: &str,
    day: &str,
    date: Option<&str>,
    fill: &str,
) -> Vec<String> {
    let cx = at.0 + c.w / 2.0;
    let size = |f: f32| (c.size * f).clamp(MIN_SIZE_PT as f32, MAX_SIZE_PT as f32) as u32;
    let day_size = size(CARD_DAY_SIZE);
    let date_size = size(CARD_DATE_SIZE);
    let mut out = vec![
        text_event(
            (cx, at.1 + c.time_top),
            &RowStyle {
                size: size(1.0),
                bold: true,
                fsp: 0,
                colour: fill.to_string(),
                alpha: 0x00,
            },
            time,
        ),
        text_event(
            (cx, at.1 + c.day_top),
            &RowStyle {
                size: day_size,
                bold: true,
                // Wide-tracked caps: the weekday is a label, not a word, and
                // tracking is what makes a short line of caps read as one.
                fsp: (day_size as f32 * CARD_DAY_TRACK) as u32,
                colour: ramp(1.0),
                alpha: 0x00,
            },
            day,
        ),
    ];
    if let (Some(date), Some(top)) = (date, c.date_top) {
        out.push(text_event(
            (cx, at.1 + top),
            &RowStyle {
                size: date_size,
                bold: false,
                fsp: (date_size as f32 * CARD_DATE_TRACK) as u32,
                // A dimmer *ink*, not a faded white. Held back in the hierarchy
                // by colour, size and weight — three things a bright wallpaper
                // behind a translucent card cannot take away, where alpha is
                // the one thing it can. At the old `\1a&H55&` this row simply
                // vanished on a small card.
                colour: CARD_DATE_INK.to_string(),
                alpha: 0x00,
            },
            date,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone};

    /// A local timestamp on a fixed date.
    ///
    /// 2026-07-15 is deliberate: mid-July is far from every DST transition in
    /// the IANA database, so every wall-clock time on that date exists exactly
    /// once in every zone and these tests give the same answers whatever `TZ`
    /// the machine running them is set to. `Local::now()` never appears in a
    /// test — a clock widget tested against the wall clock tests nothing.
    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 7, 15, h, m, s)
            .earliest()
            .expect("2026-07-15 has no DST gap in any timezone")
    }

    fn style(theme: ClockTheme) -> ClockStyle {
        ClockStyle {
            theme,
            ..Default::default()
        }
    }

    // -- defaults ----------------------------------------------------------

    #[test]
    fn defaults_are_the_cheap_ones() {
        // Every default here is also a power decision: seconds off is 1 redraw
        // a minute instead of 60, and 24h is one glyph count instead of two.
        let s = ClockStyle::default();
        assert_eq!(s.theme, ClockTheme::Digital);
        assert_eq!(s.anchor, Anchor::TopRight);
        assert!(!s.show_seconds);
        assert!(!s.show_date);
        assert!(s.use_24h);
        assert!(!s.accent_follow);
        assert_eq!(tick_secs(&s), 60);
        // ALL must stay in step with the enum and start at the default.
        assert_eq!(ClockTheme::ALL.len(), 6);
        assert_eq!(ClockTheme::ALL[0], ClockTheme::default());
        let mut labels: Vec<&str> = ClockTheme::ALL.iter().map(|t| t.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), ClockTheme::ALL.len());
    }

    // -- text --------------------------------------------------------------

    #[test]
    fn every_theme_says_the_time() {
        // The floor: no theme may render blank, and each must contain a form
        // of 21:05 a person would recognise.
        let now = at(21, 5, 7);
        let expect = [
            (ClockTheme::Digital, "21:05"),
            (ClockTheme::Minimal, "21:05"),
            (ClockTheme::Segment, "21:05"),
            (ClockTheme::Stacked, "21:05"),
            (ClockTheme::Wordy, "five past nine"),
        ];
        for (theme, want) in expect {
            let got = format_time(now, &style(theme));
            assert!(!got.is_empty(), "{theme:?} rendered nothing");
            assert!(got.contains(want), "{theme:?}: {got:?} lacks {want:?}");
            assert!(
                !render_ass(now, &style(theme), "#3584E4").is_empty(),
                "{theme:?}"
            );
        }
    }

    #[test]
    fn twenty_four_hour_pads_and_never_wraps() {
        let s = style(ClockTheme::Digital);
        assert_eq!(format_time(at(0, 0, 0), &s), "00:00");
        assert_eq!(format_time(at(9, 5, 0), &s), "09:05");
        assert_eq!(format_time(at(12, 0, 0), &s), "12:00");
        assert_eq!(format_time(at(13, 45, 0), &s), "13:45");
        assert_eq!(format_time(at(23, 59, 0), &s), "23:59");
    }

    #[test]
    fn twelve_hour_gets_midnight_and_noon_right() {
        // The classic off-by-twelve: `h % 12` alone prints "0:00 AM" at
        // midnight and "0:00 PM" at noon, and both are wrong.
        let s = ClockStyle {
            use_24h: false,
            ..style(ClockTheme::Digital)
        };
        assert_eq!(format_time(at(0, 0, 0), &s), "12:00 AM");
        assert_eq!(format_time(at(0, 30, 0), &s), "12:30 AM");
        assert_eq!(format_time(at(11, 59, 0), &s), "11:59 AM");
        assert_eq!(format_time(at(12, 0, 0), &s), "12:00 PM");
        assert_eq!(format_time(at(12, 30, 0), &s), "12:30 PM");
        assert_eq!(format_time(at(13, 0, 0), &s), "1:00 PM");
        assert_eq!(format_time(at(23, 59, 0), &s), "11:59 PM");
        // Nothing in twelve-hour mode may ever print a zero or a 13+ hour.
        for h in 0..24 {
            let got = format_time(at(h, 0, 0), &s);
            let hour: u32 = got.split(':').next().unwrap().parse().unwrap();
            assert!((1..=12).contains(&hour), "hour {h} rendered as {got:?}");
        }
    }

    #[test]
    fn themes_style_the_numerals_differently() {
        let twelve = |theme| {
            format_time(
                at(9, 5, 0),
                &ClockStyle {
                    use_24h: false,
                    ..style(theme)
                },
            )
        };
        assert_eq!(twelve(ClockTheme::Digital), "9:05 AM");
        // Minimal keeps the meridiem quiet.
        assert_eq!(twelve(ClockTheme::Minimal), "9:05 am");
        // Segment pads, because an LED panel has a fixed digit count.
        assert_eq!(twelve(ClockTheme::Segment), "09:05 AM");
    }

    #[test]
    fn seconds_change_the_text_and_the_tick() {
        let off = ClockStyle {
            show_seconds: false,
            ..style(ClockTheme::Digital)
        };
        let on = ClockStyle {
            show_seconds: true,
            ..style(ClockTheme::Digital)
        };
        assert_eq!(format_time(at(21, 5, 7), &off), "21:05");
        assert_eq!(format_time(at(21, 5, 7), &on), "21:05:07");
        assert_eq!(tick_secs(&off), 60);
        assert_eq!(tick_secs(&on), 1);
        // …and in twelve-hour mode the meridiem stays last.
        let on12 = ClockStyle {
            use_24h: false,
            ..on.clone()
        };
        assert_eq!(format_time(at(21, 5, 7), &on12), "9:05:07 PM");
        // Wordy has no way to say seconds, so the switch must not reach it —
        // if it silently did, `next_change` would sleep a minute at a time
        // while the text changed every second.
        let wordy = ClockStyle {
            show_seconds: true,
            ..style(ClockTheme::Wordy)
        };
        assert!(!shows_seconds(&wordy));
        assert_eq!(format_time(at(21, 5, 7), &wordy), "five past nine");
        assert_eq!(tick_secs(&wordy), 300);
    }

    #[test]
    fn date_appears_only_when_asked() {
        for theme in [ClockTheme::Digital, ClockTheme::Segment, ClockTheme::Wordy] {
            let off = style(theme);
            let on = ClockStyle {
                show_date: true,
                ..style(theme)
            };
            assert!(!shows_date(&off), "{theme:?}");
            assert!(!format_time(at(21, 5, 0), &off).contains('\n'), "{theme:?}");
            assert!(shows_date(&on), "{theme:?}");
            let got = format_time(at(21, 5, 0), &on);
            let (_, date) = got.split_once('\n').expect("a date line");
            // `%a %d %b` — unambiguous on either side of an ocean.
            assert!(date.eq_ignore_ascii_case("Wed 15 Jul"), "{theme:?}: {date}");
        }
    }

    #[test]
    fn two_themes_overrule_the_date_switch() {
        // Stacked without a date would be Digital; Minimal with one would not
        // be minimal. Both are documented behaviour, so both are pinned.
        let stacked_off = style(ClockTheme::Stacked);
        assert!(shows_date(&stacked_off));
        assert_eq!(format_time(at(21, 5, 0), &stacked_off), "21:05\nWED 15 JUL");
        let minimal_on = ClockStyle {
            show_date: true,
            ..style(ClockTheme::Minimal)
        };
        assert!(!shows_date(&minimal_on));
        assert_eq!(format_time(at(21, 5, 0), &minimal_on), "21:05");
    }

    // -- Wordy -------------------------------------------------------------

    #[test]
    fn wordy_covers_every_minute_of_an_hour() {
        // The whole hour in one table: rounding, the past/to flip at half
        // past, and the hour the phrase names. Ten o'clock is chosen so the
        // "to" phrases have to say "eleven".
        let s = style(ClockTheme::Wordy);
        let cases: [(u32, &str); 60] = [
            (0, "ten o'clock"),
            (1, "ten o'clock"),
            (2, "ten o'clock"),
            (3, "five past ten"),
            (4, "five past ten"),
            (5, "five past ten"),
            (6, "five past ten"),
            (7, "five past ten"),
            (8, "ten past ten"),
            (9, "ten past ten"),
            (10, "ten past ten"),
            (11, "ten past ten"),
            (12, "ten past ten"),
            (13, "quarter past ten"),
            (14, "quarter past ten"),
            (15, "quarter past ten"),
            (16, "quarter past ten"),
            (17, "quarter past ten"),
            (18, "twenty past ten"),
            (19, "twenty past ten"),
            (20, "twenty past ten"),
            (21, "twenty past ten"),
            (22, "twenty past ten"),
            (23, "twenty-five past ten"),
            (24, "twenty-five past ten"),
            (25, "twenty-five past ten"),
            (26, "twenty-five past ten"),
            (27, "twenty-five past ten"),
            (28, "half past ten"),
            (29, "half past ten"),
            (30, "half past ten"),
            (31, "half past ten"),
            (32, "half past ten"),
            (33, "twenty-five to eleven"),
            (34, "twenty-five to eleven"),
            (35, "twenty-five to eleven"),
            (36, "twenty-five to eleven"),
            (37, "twenty-five to eleven"),
            (38, "twenty to eleven"),
            (39, "twenty to eleven"),
            (40, "twenty to eleven"),
            (41, "twenty to eleven"),
            (42, "twenty to eleven"),
            (43, "quarter to eleven"),
            (44, "quarter to eleven"),
            (45, "quarter to eleven"),
            (46, "quarter to eleven"),
            (47, "quarter to eleven"),
            (48, "ten to eleven"),
            (49, "ten to eleven"),
            (50, "ten to eleven"),
            (51, "ten to eleven"),
            (52, "ten to eleven"),
            (53, "five to eleven"),
            (54, "five to eleven"),
            (55, "five to eleven"),
            (56, "five to eleven"),
            (57, "five to eleven"),
            (58, "eleven o'clock"),
            (59, "eleven o'clock"),
        ];
        for (minute, want) in cases {
            assert_eq!(format_time(at(10, minute, 0), &s), want, "10:{minute:02}");
        }
    }

    #[test]
    fn wordy_rolls_the_hour_in_both_directions() {
        let s = style(ClockTheme::Wordy);
        // The headline case from the brief: quarter to nine is 8:45.
        assert_eq!(format_time(at(8, 45, 0), &s), "quarter to nine");
        // Rounding alone can cross the hour without a "to" phrase.
        assert_eq!(format_time(at(8, 57, 0), &s), "five to nine");
        assert_eq!(format_time(at(8, 58, 0), &s), "nine o'clock");
        assert_eq!(format_time(at(8, 59, 0), &s), "nine o'clock");
        assert_eq!(format_time(at(9, 0, 0), &s), "nine o'clock");
        // 12 → 1, both ways round the dial.
        assert_eq!(format_time(at(12, 45, 0), &s), "quarter to one");
        assert_eq!(format_time(at(12, 58, 0), &s), "one o'clock");
        assert_eq!(format_time(at(0, 45, 0), &s), "quarter to one");
        assert_eq!(format_time(at(0, 5, 0), &s), "five past twelve");
        // Midnight and noon are both "twelve", never "zero".
        assert_eq!(format_time(at(0, 0, 0), &s), "twelve o'clock");
        assert_eq!(format_time(at(12, 0, 0), &s), "twelve o'clock");
        // The wrap past 23:00 is the one that indexes out of a naive table.
        assert_eq!(format_time(at(23, 45, 0), &s), "quarter to twelve");
        assert_eq!(format_time(at(23, 58, 0), &s), "twelve o'clock");
    }

    #[test]
    fn wordy_is_always_twelve_hour_wording() {
        // Nobody says "seventeen o'clock", so the 24h switch must not reach it.
        let h24 = style(ClockTheme::Wordy);
        let h12 = ClockStyle {
            use_24h: false,
            ..style(ClockTheme::Wordy)
        };
        for h in 0..24 {
            for m in [0, 7, 30, 46] {
                assert_eq!(
                    format_time(at(h, m, 0), &h24),
                    format_time(at(h, m, 0), &h12)
                );
            }
        }
        assert_eq!(format_time(at(17, 30, 0), &h24), "half past five");
    }

    #[test]
    fn wordy_never_names_a_minute_it_cannot_round_to() {
        // Every minute of every hour must produce one of a small closed set of
        // phrases — a guard against a rounding change quietly inventing
        // "thirteen past".
        let s = style(ClockTheme::Wordy);
        for h in 0..24 {
            for m in 0..60 {
                let got = format_time(at(h, m, 0), &s);
                let head = got.split(" past ").next().unwrap();
                let head = head.split(" to ").next().unwrap();
                assert!(
                    got.ends_with("o'clock")
                        || matches!(
                            head,
                            "five" | "ten" | "quarter" | "twenty" | "twenty-five" | "half"
                        ),
                    "{h}:{m:02} → {got:?}"
                );
            }
        }
    }

    // -- next_change -------------------------------------------------------

    #[test]
    fn next_change_lands_on_the_next_minute() {
        let s = style(ClockTheme::Digital);
        let got = next_change(at(21, 5, 7), &s);
        assert_eq!((got.hour(), got.minute(), got.second()), (21, 6, 0));
        assert_eq!(got.nanosecond(), 0);
        assert_eq!((got - at(21, 5, 7)).num_seconds(), 53);
        // Across an hour and across midnight.
        let hour = next_change(at(21, 59, 30), &s);
        assert_eq!((hour.hour(), hour.minute(), hour.second()), (22, 0, 0));
        let midnight = next_change(at(23, 59, 30), &s);
        assert_eq!((midnight.hour(), midnight.minute()), (0, 0));
        assert_eq!(midnight.day(), 16);
    }

    #[test]
    fn next_change_lands_on_the_next_second_when_seconds_are_shown() {
        let s = ClockStyle {
            show_seconds: true,
            ..style(ClockTheme::Digital)
        };
        let now = at(21, 5, 7).with_nanosecond(250_000_000).unwrap();
        let got = next_change(now, &s);
        assert_eq!((got.hour(), got.minute(), got.second()), (21, 5, 8));
        assert_eq!(got.nanosecond(), 0);
        assert_eq!((got - now).num_milliseconds(), 750);
        // Granularity really is per-style, not per-theme.
        assert_eq!(
            (next_change(at(21, 5, 7), &s) - at(21, 5, 7)).num_seconds(),
            1
        );
    }

    #[test]
    fn next_change_on_a_boundary_moves_a_whole_period_forward() {
        // The spin-loop guard: returning `now` gives the daemon a zero-length
        // sleep, and it wakes forever instead of once a minute.
        let minute = style(ClockTheme::Digital);
        let on_boundary = at(21, 5, 0);
        assert_eq!(
            (next_change(on_boundary, &minute) - on_boundary).num_seconds(),
            60
        );
        let seconds = ClockStyle {
            show_seconds: true,
            ..style(ClockTheme::Digital)
        };
        assert_eq!(
            (next_change(on_boundary, &seconds) - on_boundary).num_nanoseconds(),
            Some(NANOS_PER_SEC)
        );
        // Wordy's boundaries are the :03/:08/… ones, not :00.
        let wordy = style(ClockTheme::Wordy);
        assert_eq!(
            (next_change(at(21, 3, 0), &wordy) - at(21, 3, 0)).num_seconds(),
            300
        );
    }

    #[test]
    fn next_change_is_strictly_in_the_future_all_day() {
        // Swept rather than sampled: a phase error in one theme shows up as a
        // deadline in the past exactly once per period, which spot checks miss.
        for theme in ClockTheme::ALL {
            for seconds in [false, true] {
                let s = ClockStyle {
                    show_seconds: seconds,
                    ..style(theme)
                };
                let period = i64::from(tick_secs(&s));
                for total in (0..86_400).step_by(7) {
                    let now = at(total / 3600, (total / 60) % 60, total % 60);
                    let next = next_change(now, &s);
                    let delta = (next - now).num_seconds();
                    assert!(delta > 0, "{theme:?} seconds={seconds} at {now}: {delta}s");
                    assert!(delta <= period, "{theme:?} at {now}: {delta}s > {period}s");
                    // A deadline is only useful if the text differs when it
                    // fires — and identical the instant before.
                    assert_ne!(
                        format_time(now, &s),
                        format_time(next, &s),
                        "{theme:?} seconds={seconds} at {now}"
                    );
                }
            }
        }
    }

    #[test]
    fn next_change_for_wordy_lands_where_the_words_change() {
        // Wordy rounds to the *nearest* five, so its text flips at :03, :08,
        // …, not on the :00/:05 grid. Waking on the obvious grid would show
        // the wrong words for three minutes out of every five.
        let s = style(ClockTheme::Wordy);
        for m in 0..60 {
            let now = at(10, m, 30);
            let next = next_change(now, &s);
            assert_eq!(next.second(), 0, "10:{m:02} → {next}");
            assert_eq!((next.minute() + 2) % 5, 0, "10:{m:02} → {next}");
            assert!(next > now);
            assert!((next - now).num_seconds() <= 300);
        }
        assert_eq!(next_change(at(10, 2, 59), &s).minute(), 3);
        assert_eq!(next_change(at(10, 3, 0), &s).minute(), 8);
        assert_eq!(next_change(at(10, 57, 0), &s).minute(), 58);
        let over_the_hour = next_change(at(10, 58, 0), &s);
        assert_eq!((over_the_hour.hour(), over_the_hour.minute()), (11, 3));
    }

    #[test]
    fn text_is_constant_inside_its_bucket() {
        // The "nothing changed" fast path: if the string wobbled inside a
        // bucket, comparing against the last render would not be enough and
        // the daemon would have to poll.
        let minute = style(ClockTheme::Digital);
        let wordy = style(ClockTheme::Wordy);
        for sec in 0..60 {
            assert_eq!(format_time(at(21, 5, sec), &minute), "21:05");
        }
        for m in 3..8 {
            for sec in [0, 17, 59] {
                assert_eq!(format_time(at(10, m, sec), &wordy), "five past ten");
            }
        }
        // With seconds on, the bucket is a second — every one differs.
        let seconds = ClockStyle {
            show_seconds: true,
            ..minute
        };
        let mut seen: Vec<String> = (0..60)
            .map(|s| format_time(at(21, 5, s), &seconds))
            .collect();
        seen.dedup();
        assert_eq!(seen.len(), 60);
    }

    // -- ASS ---------------------------------------------------------------

    #[test]
    fn render_is_one_fully_specified_event() {
        // Pinned in full: this string is the actual wire format. mpv turns
        // every newline in the payload into a separate event, so a clock that
        // emitted one would render its date with mpv's styling, not ours.
        let got = render_ass(at(21, 5, 0), &ClockStyle::default(), "#3584E4");
        assert_eq!(
            got,
            "{\\an9\\pos(1864,56)\\fnInter\\fs64\\b600\\bord4\\shad2\
             \\1c&HFFFFFF&\\3c&H000000&\\4c&H000000&\\1a&H00&\\3a&H00&\\4a&H80&}21:05"
        );
        for theme in ClockTheme::ALL {
            for date in [false, true] {
                let s = ClockStyle {
                    show_date: date,
                    ..style(theme)
                };
                let out = render_ass(at(21, 5, 0), &s, "#3584E4");
                if matches!(theme, ClockTheme::Card) {
                    // Card is a drawing with type on it, not a stack of lines:
                    // it pins its own anchor and its own event count. See
                    // `render_card` for the rules it keeps instead.
                    continue;
                }
                assert!(out.starts_with("{\\an9\\pos("), "{theme:?}: {out}");
                // Every event is a *whole* look. mpv styles each event from
                // its own defaults, so a line leaning on the one above it
                // would render with mpv's OSD styling instead of ours.
                for event in out.split('\n') {
                    assert!(event.starts_with("{\\an9\\pos("), "{theme:?}: {event}");
                    assert!(event.contains("\\fs"), "{theme:?}: {event}");
                }
                // A date is a second *event*, not a `\N` break — which is the
                // only way to choose the distance between the two lines.
                assert!(!out.contains("\\N"), "{theme:?}: {out}");
                assert_eq!(
                    out.split('\n').count(),
                    1 + usize::from(shows_date(&s)),
                    "{theme:?}: {out}"
                );
            }
        }
    }

    #[test]
    fn the_themes_are_visibly_different() {
        // A "themes" feature whose payloads differ only in font size is not a
        // themes feature. Each of these tags is one theme's whole identity.
        let now = at(21, 5, 0);
        let payload = |t| render_ass(now, &style(t), "#3584E4");
        let digital = payload(ClockTheme::Digital);
        let minimal = payload(ClockTheme::Minimal);
        let segment = payload(ClockTheme::Segment);
        let stacked = payload(ClockTheme::Stacked);
        let wordy = payload(ClockTheme::Wordy);

        assert!(digital.contains("\\fs64\\b600"), "{digital}");
        assert!(!digital.contains("\\fsp"), "{digital}");
        // Minimal: unbolded, smaller, opened up, hairline outline.
        assert!(minimal.contains("\\b0"), "{minimal}");
        assert!(minimal.contains("\\fs35"), "{minimal}");
        assert!(minimal.contains("\\fsp1"), "{minimal}");
        // Segment: monospace, heavy tracking, blurred fill-coloured halo.
        assert!(segment.contains("\\fnDejaVu Sans Mono"), "{segment}");
        assert!(segment.contains("\\fsp9"), "{segment}");
        assert!(segment.contains("\\blur4"), "{segment}");
        assert!(segment.contains("\\3c&HFFFFFF&"), "{segment}");
        // Stacked: largest, and two lines with different sizes and weights.
        assert!(stacked.contains("\\fs83"), "{stacked}");
        // The date is its own event, one leading below the time — 125% of the
        // 83pt time line is 103px, so 56 + 103. A `\N` break instead would put
        // it wherever libass reads off Inter's metrics, which at this size is
        // close enough to touching that it reads as a subscript.
        assert_eq!(stacked.matches("\\N").count(), 0, "{stacked}");
        let (time_ev, date_ev) = stacked.split_once('\n').expect("a date event");
        assert!(time_ev.starts_with("{\\an9\\pos(1864,56)"), "{stacked}");
        assert!(time_ev.ends_with("}21:05"), "{stacked}");
        assert!(date_ev.starts_with("{\\an9\\pos(1864,159)"), "{stacked}");
        assert!(
            date_ev.ends_with("{\\fs19\\b0\\fsp4}WED 15 JUL"),
            "{stacked}"
        );
        // Wordy: italic prose.
        assert!(wordy.contains("\\i1"), "{wordy}");
        assert!(wordy.ends_with("five past nine"), "{wordy}");

        let all = [&digital, &minimal, &segment, &stacked, &wordy];
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                assert_ne!(a, b, "two themes render identically");
            }
        }
    }

    /// The `y` of an event's `\pos`.
    fn pos_y(event: &str) -> u32 {
        event
            .split_once(',')
            .and_then(|(_, rest)| rest.split_once(')'))
            .and_then(|(y, _)| y.parse().ok())
            .expect("every event carries a \\pos")
    }

    #[test]
    fn the_date_sits_one_leading_below_the_time() {
        // The `\N` break this replaced had no say in the gap at all — libass
        // took it off the font metrics. Now it is a chosen fraction of the type
        // size, so it holds its proportions as the user drags the size slider
        // instead of reading cramped at one end of the range and loose at the
        // other.
        for font_size_pt in [24, 64, 120, 200] {
            let plain = ClockStyle {
                font_size_pt,
                ..style(ClockTheme::Digital)
            };
            let dated = ClockStyle {
                show_date: true,
                ..plain.clone()
            };
            let out = render_ass(at(21, 5, 0), &dated, "#3584E4");
            let (time_ev, date_ev) = out.split_once('\n').expect("a date event");
            let size = scaled(font_size_pt, look_for(ClockTheme::Digital).size_pct);
            assert_eq!(
                pos_y(date_ev) - pos_y(time_ev),
                lyrics::leading_px(size),
                "{font_size_pt}pt"
            );
            // The clock is top-anchored by default, so the stack grows down:
            // the *time* keeps its margin and the date is what moves. Adding a
            // date must never shift the line that was already on screen.
            let solo = render_ass(at(21, 5, 0), &plain, "#3584E4");
            assert_eq!(pos_y(time_ev), pos_y(&solo), "{font_size_pt}pt");
        }
        // Against the bottom edge it is the other way round: the date is the
        // line nearest the edge, so it keeps the margin and the time rises.
        let bottom = ClockStyle {
            anchor: Anchor::BottomCenter,
            show_date: true,
            ..style(ClockTheme::Digital)
        };
        let out = render_ass(at(21, 5, 0), &bottom, "#3584E4");
        let (time_ev, date_ev) = out.split_once('\n').expect("a date event");
        assert_eq!(pos_y(date_ev), lyrics::PLAY_RES_Y - bottom.margin_px);
        assert!(pos_y(time_ev) < pos_y(date_ev));
    }

    #[test]
    fn style_fields_reach_the_payload() {
        // Cheap guard against a field being added and quietly never rendered —
        // the failure mode where the GUI slider does nothing.
        let s = ClockStyle {
            theme: ClockTheme::Digital,
            anchor: Anchor::BottomLeft,
            font_size_pt: 120,
            margin_px: 100,
            show_seconds: true,
            show_date: true,
            use_24h: false,
            colour: "#f80".into(),
            accent_follow: false,
        };
        let got = render_ass(at(21, 5, 7), &s, "#3584E4");
        assert!(got.contains("\\an1"), "{got}");
        assert!(got.contains("\\pos(100,980)"), "{got}");
        assert!(got.contains("\\fs120"), "{got}");
        assert!(got.contains("\\1c&H0088FF&"), "{got}");
        assert!(got.ends_with("Wed 15 Jul"), "{got}");
        assert!(got.contains("9:05:07 PM"), "{got}");
    }

    #[test]
    fn accent_follow_swaps_the_fill_and_the_glow() {
        let plain = ClockStyle {
            colour: "#FF0000".into(),
            ..style(ClockTheme::Digital)
        };
        let followed = ClockStyle {
            accent_follow: true,
            ..plain.clone()
        };
        assert!(render_ass(at(21, 5, 0), &plain, "#3584E4").contains("\\1c&H0000FF&"));
        assert!(render_ass(at(21, 5, 0), &followed, "#3584E4").contains("\\1c&HE48435&"));
        // On a glow theme the accent tints the halo too, so the whole widget
        // follows the desktop rather than half of it.
        let glow = ClockStyle {
            accent_follow: true,
            ..style(ClockTheme::Segment)
        };
        let out = render_ass(at(21, 5, 0), &glow, "#3584E4");
        assert!(out.contains("\\1c&HE48435&\\3c&HE48435&"), "{out}");
    }

    #[test]
    fn hostile_colours_cannot_escape_the_override_block() {
        // `colour` and the accent both come from files a user can hand-edit,
        // and both land inside an override block. An unescaped one could move,
        // recolour or hide the overlay instead of tinting it.
        for theme in ClockTheme::ALL {
            let s = ClockStyle {
                colour: "}{\\an7\\fs200\\pos(0,0)}".into(),
                show_date: true,
                ..style(theme)
            };
            for accent in ["#3584E4", "}{\\fscx400}", "", "rgb(1,2,3)"] {
                for follow in [false, true] {
                    let out = render_ass(
                        at(21, 5, 0),
                        &ClockStyle {
                            accent_follow: follow,
                            ..s.clone()
                        },
                        accent,
                    );
                    // Unusable colours fall back to white, never to raw junk.
                    // Asserted against the *injected* values rather than
                    // against the tag names: a theme may legitimately emit an
                    // `\an7` or an `\fscx` of its own, and pinning the names
                    // would make this test a list of which ones currently do.
                    assert!(!out.contains("\\fs200"), "{theme:?}: {out}");
                    assert!(!out.contains("\\pos(0,0)"), "{theme:?}: {out}");
                    assert!(!out.contains("fscx400"), "{theme:?}: {out}");
                    // Braces stay balanced in every event: mpv splits the
                    // payload on newlines, so a block left open by one line
                    // cannot be closed by the next.
                    for event in out.split('\n') {
                        assert_eq!(
                            event.matches('{').count(),
                            event.matches('}').count(),
                            "{theme:?}: {event}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn visible_text_is_escaped_even_though_we_wrote_it() {
        // Nothing user-supplied reaches the text today, but the escaping is
        // what keeps that true if a custom format is ever added — so assert it
        // structurally rather than trusting the current inputs.
        let out = render_ass(
            at(21, 5, 0),
            &ClockStyle {
                show_date: true,
                ..style(ClockTheme::Stacked)
            },
            "#3584E4",
        );
        // The visible text sits after the last override block and carries no
        // brace or bare backslash of its own.
        let tail = out.rsplit('}').next().expect("text after the last block");
        assert_eq!(tail, "WED 15 JUL");
        assert_eq!(lyrics::ass_escape("{\\an7}"), "\\{\\\u{2060}an7\\}");
        // An apostrophe is ordinary text in ASS and must survive untouched.
        assert!(
            render_ass(at(21, 0, 0), &style(ClockTheme::Wordy), "#3584E4")
                .ends_with("nine o'clock")
        );
    }

    #[test]
    fn extreme_sizes_are_clamped_not_trusted() {
        // `config.toml` is hand-editable; `\fs0` renders nothing at all and a
        // ten-digit size would multiply into an overflow before libass ever
        // saw it.
        let tiny = render_ass(
            at(21, 5, 0),
            &ClockStyle {
                font_size_pt: 0,
                ..style(ClockTheme::Stacked)
            },
            "#3584E4",
        );
        assert!(tiny.contains("\\fs8"), "{tiny}");
        // The date line clamps on its own account, and is still a second event.
        assert!(tiny.contains("\n{"), "{tiny}");
        assert!(tiny.ends_with("{\\fs8\\b0\\fsp1}WED 15 JUL"), "{tiny}");
        let huge = render_ass(
            at(21, 5, 0),
            &ClockStyle {
                font_size_pt: u32::MAX,
                margin_px: u32::MAX,
                ..style(ClockTheme::Stacked)
            },
            "#3584E4",
        );
        assert!(huge.contains("\\fs400"), "{huge}");
        assert!(huge.contains("\n{"), "{huge}");
        assert!(huge.contains("{\\fs96"), "{huge}");
        // The lyric renderer clamps the margin to the centre of the screen, so
        // an absurd one must not flip the anchor to the other side.
        assert!(huge.contains("\\pos(960,540)"), "{huge}");
    }

    #[test]
    fn serde_round_trips_and_tolerates_an_older_config() {
        // The lead embeds this in `config.toml`; a file written before a field
        // existed must still load, and a saved file must reload identically.
        let s = ClockStyle {
            theme: ClockTheme::Segment,
            show_seconds: true,
            ..Default::default()
        };
        let text = toml_like(&s);
        assert!(text.contains("\"theme\":\"segment\""), "{text}");
        let back: ClockStyle = serde_json::from_str(&text).expect("round trip");
        assert_eq!(back, s);
        let sparse: ClockStyle = serde_json::from_str("{}").expect("an empty table");
        assert_eq!(sparse, ClockStyle::default());
        for theme in ClockTheme::ALL {
            let name = format!("\"{}\"", theme.label().to_lowercase());
            let back: ClockTheme = serde_json::from_str(&name).expect(&name);
            assert_eq!(back, theme);
        }
    }

    /// Serde is exercised through JSON rather than TOML so the test needs no
    /// dependency the crate does not already have on every feature set.
    fn toml_like(s: &ClockStyle) -> String {
        serde_json::to_string(s).expect("serialisable")
    }

    // -- Card ---------------------------------------------------------------
    //
    // `Card` is the one theme that is a picture, so it is the one theme whose
    // tests have to read geometry rather than text. The helpers below take a
    // payload apart the way libass does — events, then drawings, then contours,
    // then points — so an assertion can be about the *shape* that will appear on
    // screen instead of about a substring that happens to be in the string.

    /// One drawing taken apart: its contours, each a run of points.
    type Contours = Vec<Vec<(i32, i32)>>;

    fn card(size: u32, seconds: bool, date: bool) -> ClockStyle {
        ClockStyle {
            theme: ClockTheme::Card,
            font_size_pt: size,
            show_seconds: seconds,
            show_date: date,
            anchor: Anchor::MidCenter,
            ..Default::default()
        }
    }

    /// The layout `render_ass` will actually use for this instant and style.
    /// Mirrors `render_card`'s own three lines, so a test can never lay out a
    /// differently sized card than the one it is making assertions about.
    fn card_geom(now: DateTime<Local>, s: &ClockStyle) -> Card {
        let text = card_text(now, s);
        let mut rows = text.split('\n');
        let time = rows.next().unwrap_or_default();
        let day = rows.next().unwrap_or_default();
        let date = rows.next();
        let size = scaled(s.font_size_pt, look_for(ClockTheme::Card).size_pct);
        card_layout(size, time, day, date)
    }

    /// Every `\p1 … \p0` drawing body in a payload, in paint order.
    fn drawings(payload: &str) -> Vec<String> {
        payload
            .split('\n')
            .filter_map(|e| {
                let (_, rest) = e.split_once("\\p1}")?;
                let (body, _) = rest.split_once("{\\p0}")?;
                Some(body.to_string())
            })
            .collect()
    }

    /// One drawing body as its contours, each a run of points. `m` opens a
    /// contour, `l` adds a point, `b` adds a cubic's three.
    fn contours(body: &str) -> Contours {
        let mut out: Contours = Vec::new();
        let mut it = body.split_whitespace();
        while let Some(op) = it.next() {
            let n = match op {
                "m" => {
                    out.push(Vec::new());
                    1
                }
                "l" => 1,
                "b" => 3,
                other => panic!("unknown drawing op {other:?} in {body:?}"),
            };
            for _ in 0..n {
                let x: i32 = it.next().expect("an x").parse().expect("integer x");
                let y: i32 = it.next().expect("a y").parse().expect("integer y");
                out.last_mut().expect("a contour").push((x, y));
            }
        }
        out
    }

    /// The two zero-area pin contours every card drawing opens with.
    fn pin_of(body: &str) -> Contours {
        contours(body).into_iter().take(2).collect()
    }

    /// The narrowest side of a four-point bar — its width, since the bars this
    /// module draws are always longer than they are wide.
    fn bar_width(quad: &[(i32, i32)]) -> f32 {
        assert_eq!(quad.len(), 4, "not a bar: {quad:?}");
        (0..4)
            .map(|i| {
                let (a, b) = (quad[i], quad[(i + 1) % 4]);
                f64::from(a.0 - b.0).hypot(f64::from(a.1 - b.1)) as f32
            })
            .fold(f32::INFINITY, f32::min)
    }

    /// The longest side of a four-point bar — its length.
    fn bar_length(quad: &[(i32, i32)]) -> f32 {
        assert_eq!(quad.len(), 4, "not a bar: {quad:?}");
        (0..4)
            .map(|i| {
                let (a, b) = (quad[i], quad[(i + 1) % 4]);
                f64::from(a.0 - b.0).hypot(f64::from(a.1 - b.1)) as f32
            })
            .fold(0.0f32, f32::max)
    }

    /// The hour-tick and (when drawn) minute-tick drawings, found by their
    /// contour counts rather than by their position in the payload, so
    /// reordering the paint stack does not silently retarget the test.
    ///
    /// Twelve hour marks and **forty-eight** minute marks, not sixty: the dial
    /// has sixty divisions, and the twelve that fall on an hour are drawn as
    /// hour marks. Stacking a hairline under each heavy mark would draw ink
    /// nobody can see and fatten twelve of the sixty by a rounding error.
    fn tick_drawings(payload: &str) -> (Contours, Option<Contours>) {
        let mut hour = None;
        let mut minute = None;
        for d in drawings(payload) {
            let c = contours(&d);
            // Two pin contours plus the marks themselves.
            match c.len() {
                14 => hour = Some(c[2..].to_vec()),
                50 => minute = Some(c[2..].to_vec()),
                _ => {}
            }
        }
        (hour.expect("twelve hour ticks"), minute)
    }

    #[test]
    fn card_hands_point_where_a_clock_points() {
        // Angles are clockwise from twelve, in radians, and every one of these
        // is a number a person can check on a real dial.
        let quarter = TAU / 4.0;
        let cases = [
            // (h, m, s) -> (hour, minute, second)
            ((12, 0, 0), (0.0, 0.0, 0.0)),
            ((3, 0, 0), (quarter, 0.0, 0.0)),
            ((6, 0, 0), (2.0 * quarter, 0.0, 0.0)),
            ((9, 0, 0), (3.0 * quarter, 0.0, 0.0)),
            ((0, 0, 0), (0.0, 0.0, 0.0)),
            // Noon and midnight are the same picture, and 15:00 is 3 o'clock.
            ((15, 0, 0), (quarter, 0.0, 0.0)),
        ];
        for ((h, m, s), want) in cases {
            let got = card_hands(at(h, m, s), true);
            for (g, w) in [(got.0, want.0), (got.1, want.1), (got.2, want.2)] {
                assert!((g - w).abs() < 1e-5, "{h}:{m:02}:{s:02} → {got:?}");
            }
        }

        // The screenshot the design pass started from. At half past three the
        // hour hand is *half way between* the three and the four — a stepped
        // hour hand is the single detail that makes a drawn clock look broken.
        let (h, m, s) = card_hands(at(15, 30, 32), true);
        assert!((h - TAU * 3.5 / 12.0).abs() < 1e-5, "hour {h}");
        // With seconds shown the minute hand carries the second fraction.
        assert!(
            (m - TAU * (30.0 + 32.0 / 60.0) / 60.0).abs() < 1e-5,
            "min {m}"
        );
        assert!((s - TAU * 32.0 / 60.0).abs() < 1e-5, "sec {s}");

        // With seconds off it does not, because the payload has to be
        // byte-identical for a whole minute or the daemon starts redrawing
        // sixty times as often as it budgeted for.
        let (_, m_off, _) = card_hands(at(15, 30, 32), false);
        assert!((m_off - TAU * 30.0 / 60.0).abs() < 1e-5, "min {m_off}");
    }

    #[test]
    fn card_hour_hand_advances_between_the_hours() {
        // Strictly increasing across an hour, and exactly one twelfth of a turn
        // from one o'clock to the next.
        let mut last = card_hands(at(3, 0, 0), false).0;
        for m in 1..60 {
            let now = card_hands(at(3, m, 0), false).0;
            assert!(now > last, "3:{m:02} did not advance: {now} <= {last}");
            last = now;
        }
        let three = card_hands(at(3, 0, 0), false).0;
        let four = card_hands(at(4, 0, 0), false).0;
        assert!((four - three - TAU / 12.0).abs() < 1e-5);
        // And it wraps rather than running past a full turn at eleven.
        assert!(card_hands(at(11, 59, 0), false).0 < TAU);
    }

    #[test]
    fn card_ticks_are_twelve_heavy_and_forty_eight_hairline() {
        let s = card(120, true, true);
        let payload = render_ass(at(15, 30, 32), &s, "#3584E4");
        let (hour, minute) = tick_drawings(&payload);
        let minute = minute.expect("a minute ring at 120pt");
        assert_eq!(hour.len(), 12);
        assert_eq!(minute.len(), 48);

        // The whole point of two bands: an hour tick has to be obviously not a
        // minute tick, on *both* axes, or the ring reads as one smear of
        // uneven blocks. Half again as long and half again as wide is the floor.
        let hw: Vec<f32> = hour.iter().map(|q| bar_width(q)).collect();
        let mw: Vec<f32> = minute.iter().map(|q| bar_width(q)).collect();
        let hl: Vec<f32> = hour.iter().map(|q| bar_length(q)).collect();
        let ml: Vec<f32> = minute.iter().map(|q| bar_length(q)).collect();
        let min = |v: &[f32]| v.iter().copied().fold(f32::INFINITY, f32::min);
        let max = |v: &[f32]| v.iter().copied().fold(0.0f32, f32::max);
        assert!(min(&hw) > max(&mw) * 1.5, "widths {:?} vs {:?}", hw, mw);
        assert!(min(&hl) > max(&ml) * 1.5, "lengths {:?} vs {:?}", hl, ml);

        // Within a band every mark is the same size. Uneven ticks were the
        // loudest complaint about the old face, and they were uneven because
        // the widths came out of the type size rather than the dial radius.
        // A one-unit-and-a-bit spread is integer rounding, not drift: a bar
        // rotated to an arbitrary angle has its corners snapped to the drawing
        // grid, so a true 3.0-unit hairline measures 2.83 to 4.12 depending on
        // where round the dial it sits. Anything wider than that is the fault
        // the old ticks had — widths coming out of the type size instead of the
        // dial radius, so they drifted toward a common chunk as the card shrank.
        assert!(max(&hw) - min(&hw) <= 1.5, "hour widths vary: {hw:?}");
        assert!(max(&mw) - min(&mw) <= 1.5, "minute widths vary: {mw:?}");
        // Lengths get two units of slack rather than one and a half: a length
        // is measured between two rounded endpoints, so it carries a rounding
        // unit at each end where a width carries one across.
        assert!(max(&hl) - min(&hl) <= 2.0, "hour lengths vary: {hl:?}");
        assert!(max(&ml) - min(&ml) <= 2.0, "minute lengths vary: {ml:?}");

        // The two bands share an outer radius, which is what makes them read as
        // one ring rather than two. Compare the farthest point of each band
        // from the dial centre.
        let c = card_geom(at(15, 30, 32), &s);
        let far = |band: &[Vec<(i32, i32)>]| {
            band.iter()
                .flatten()
                .map(|&(x, y)| {
                    f64::from(x as f32 - c.face_cx).hypot(f64::from(y as f32 - c.face_cy)) as f32
                })
                .fold(0.0f32, f32::max)
        };
        assert!((far(&hour) - far(&minute)).abs() <= 2.0);
        // Two events, not one: the alphas differ, so the twelve read as
        // structure and the forty-eight as texture behind them.
        assert_ne!(
            payload.matches("\\1a&H0C&").count(),
            0,
            "hour ticks lost their alpha"
        );
        assert_ne!(payload.matches("\\1a&H8E&").count(), 0);
    }

    #[test]
    fn card_drops_the_minute_ring_before_it_turns_to_mush() {
        // Sixty marks round a small dial land closer together than a hairline
        // is wide. Below the threshold they are not drawn at all — twelve clean
        // ticks beat sixty smeared ones, and this is the assertion that keeps
        // someone from "fixing" the gap by drawing them anyway.
        let tiny = render_ass(at(15, 30, 32), &card(8, false, false), "#3584E4");
        let (hour, minute) = tick_drawings(&tiny);
        assert_eq!(hour.len(), 12, "the hour ring is never dropped");
        assert!(minute.is_none(), "a minute ring at 8pt is a smudge");
        // …and it comes back once there is room for it.
        let big = render_ass(at(15, 30, 32), &card(64, false, false), "#3584E4");
        assert!(tick_drawings(&big).1.is_some());
    }

    #[test]
    fn card_hand_hierarchy_survives_an_overlap() {
        // 3:16 and 12:00 are where a weak hierarchy shows worst: the hands
        // nearly or exactly coincide, and if they are similar in length *and*
        // weight the face becomes one white blob that tells you nothing.
        for (h, m, s) in [(3, 16, 0), (12, 0, 0), (15, 30, 32)] {
            let payload = render_ass(at(h, m, s), &card(120, true, true), "#3584E4");
            let ds = drawings(&payload);
            // The hour+minute event is the one with the hub disc and two bars.
            let hands = ds
                .iter()
                .map(|d| contours(d))
                .find(|c| c.len() == 5 && c[3].len() == 4 && c[4].len() == 4)
                .expect("an hour+minute drawing");
            let (hour, minute) = (&hands[3], &hands[4]);
            let (hl, ml) = (bar_length(hour), bar_length(minute));
            let (hwid, mwid) = (bar_width(hour), bar_width(minute));
            // Convention: the hour hand is the short fat one, the minute hand
            // the long thin one. Both separations, in the same direction.
            assert!(ml > hl * 1.35, "{h}:{m:02} lengths {hl} vs {ml}");
            assert!(hwid > mwid * 1.4, "{h}:{m:02} widths {hwid} vs {mwid}");
            // The second hand is longer still and thinner than both.
            let sec = ds
                .iter()
                .map(|d| contours(d))
                .find(|c| c.len() == 4 && c[2].len() == 4)
                .expect("a second-hand drawing");
            assert!(bar_length(&sec[2]) > ml, "{h}:{m:02} second hand too short");
            assert!(bar_width(&sec[2]) < mwid, "{h}:{m:02} second hand too fat");
        }
    }

    #[test]
    fn card_centre_is_a_three_layer_stack() {
        // Hub disc under the hands, the hands over it, the second hand's own
        // smaller cap over those — so the centre reads front-to-back instead of
        // as one lump, and the hands visibly converge instead of crossing.
        let s = card(120, true, true);
        let payload = render_ass(at(15, 30, 32), &s, "#3584E4");
        let c = card_geom(at(15, 30, 32), &s);
        let radius = |disc: &[(i32, i32)]| {
            disc.iter()
                .map(|&(x, y)| {
                    f64::from(x as f32 - c.face_cx).hypot(f64::from(y as f32 - c.face_cy)) as f32
                })
                .fold(0.0f32, f32::max)
        };
        let ds: Vec<Contours> = drawings(&payload).iter().map(|d| contours(d)).collect();
        let hub = ds
            .iter()
            .find(|c| c.len() == 5 && c[3].len() == 4)
            .map(|c| radius(&c[2]))
            .expect("a hub disc");
        let cap = ds
            .iter()
            .find(|c| c.len() == 4 && c[2].len() == 4)
            .map(|c| radius(&c[3]))
            .expect("a second-hand cap");
        assert!(
            cap < hub,
            "the cap ({cap}) must be smaller than the hub ({hub})"
        );
        // A hub that does not clear the hour hand is not a hub, it is a bulge.
        let hand_hw = c.face_r * CARD_HAND_HOUR.1;
        assert!(
            hub > hand_hw * 1.3,
            "hub {hub} vs hand half-width {hand_hw}"
        );
        // The second hand carries a counterweight past the hub, so its bar is
        // longer than its reach. Tip plus tail, within a unit of rounding.
        let sec = ds
            .iter()
            .find(|c| c.len() == 4 && c[2].len() == 4)
            .map(|c| bar_length(&c[2]))
            .expect("a second hand");
        let want = c.face_r * (CARD_HAND_SEC.0 + CARD_TAIL_SEC);
        assert!(
            (sec - want).abs() <= 2.0,
            "second hand {sec}, wanted {want}"
        );
    }

    #[test]
    fn card_rhythm_is_optical_not_arithmetic() {
        for size in [40, 64, 120] {
            for date in [false, true] {
                let d = if date { Some("28 JUL 2025") } else { None };
                let c = card_layout(size, "15:30:32", "MONDAY", d);
                let pad = CARD_PAD * c.size;
                // The gap above the time's *capitals* — not above its em box —
                // matches the gap below the dial. Optical centring: the em box
                // carries a quarter em of ascent that no digit ever uses, and
                // measuring to it is what left dead air at the top of the card.
                let above = c.time_top + CARD_CAP_GAP * c.size;
                let below = c.h - (c.face_cy + c.face_r);
                assert!((above - pad).abs() < 0.01, "{size}pt date={date}: {above}");
                assert!((below - pad).abs() < 0.01, "{size}pt date={date}: {below}");
                assert!(
                    (above - below).abs() < 0.02,
                    "{size}pt date={date}: {above} above, {below} below"
                );

                // Two groups, one gap — and it has to be measured between the
                // rows' *cap boxes*, because the three rows are set at three
                // sizes and their `\an8` origins each sit a different distance
                // above their capitals.
                let caps = |top: f32, row: f32| {
                    let t = top + CARD_CAP_GAP * row * c.size;
                    (t, t + CARD_CAP_H * row * c.size)
                };
                let (_, time_bot) = caps(c.time_top, 1.0);
                let (day_top, day_bot) = caps(c.day_top, CARD_DAY_SIZE);
                let lead_day = day_top - time_bot;
                let text_bot = match c.date_top {
                    Some(t) => {
                        let (date_top, date_bot) = caps(t, CARD_DATE_SIZE);
                        // Tightening down the stack, so the three rows cohere
                        // into one block instead of reading as three strangers.
                        assert!(
                            date_top - day_bot < lead_day,
                            "{size}pt: {} vs {lead_day}",
                            date_top - day_bot
                        );
                        date_bot
                    }
                    None => day_bot,
                };
                let face_gap = (c.face_cy - c.face_r) - text_bot;
                assert!(
                    face_gap > lead_day * 1.5,
                    "{size}pt date={date}: {face_gap} to the dial vs {lead_day} leading"
                );
            }
        }
    }

    #[test]
    fn card_face_is_a_substantial_share_of_the_card() {
        // The face used to be a fixed radius on a card whose width swings by a
        // factor of two, so `10:42:07 PM` left it marooned in empty air. It now
        // grows with the content it shares the card with.
        for (size, seconds) in [(40, false), (64, false), (120, true), (120, false)] {
            let s = card(size, seconds, true);
            let text = card_text(at(22, 42, 7), &s);
            let mut rows = text.split('\n');
            let (t, d) = (rows.next().unwrap(), rows.next().unwrap());
            let c = card_layout(size, t, d, rows.next());
            let content = c.w - 2.0 * CARD_PAD * c.size;
            let share = 2.0 * c.face_r / content;
            assert!(
                share > 0.68,
                "{size}pt seconds={seconds}: dial is {share} of the content width"
            );
            assert!(
                share <= 1.001,
                "{size}pt: dial wider than the card's content"
            );
        }
    }

    #[test]
    fn card_drawings_are_pinned_to_one_box_at_every_time() {
        // The pin is what stops the hands from resizing the drawing's bounding
        // box and walking the widget across the wallpaper. Same style, same
        // string lengths, wildly different hand positions: every drawing must
        // open with the same two contours, and every event must sit at the same
        // `\pos`.
        let s = card(120, true, true);
        let base = render_ass(at(12, 0, 0), &s, "#3584E4");
        let want: Vec<Contours> = drawings(&base).iter().map(|d| pin_of(d)).collect();
        let positions = |p: &str| -> Vec<String> {
            p.split('\n')
                .map(|e| e.split_once(')').expect("a \\pos").0.to_string())
                .collect()
        };
        let want_pos = positions(&base);
        for (h, m, sec) in [(3, 16, 44), (6, 45, 1), (9, 59, 59), (23, 30, 30)] {
            let other = render_ass(at(h, m, sec), &s, "#3584E4");
            let got: Vec<Contours> = drawings(&other).iter().map(|d| pin_of(d)).collect();
            assert_eq!(got, want, "{h}:{m:02}:{sec:02} moved the bounding box");
            assert_eq!(positions(&other), want_pos, "{h}:{m:02}:{sec:02} moved");
            // And the pin really is two zero-area contours at opposite corners.
            for p in &got {
                assert_eq!(p.len(), 2);
                assert_eq!(p[0], vec![(0, 0)]);
                assert_eq!(p[1].len(), 1);
                assert!(p[1][0].0 > 0 && p[1][0].1 > 0);
            }
        }
    }

    #[test]
    fn card_payload_is_structurally_sound() {
        // Everything libass would refuse to parse, asserted across the whole
        // matrix rather than on one lucky timestamp. A drawing it cannot read is
        // discarded whole, so one bad number costs the card and not one tick.
        for size in [8, 40, 64, 120, 400] {
            for seconds in [false, true] {
                for date in [false, true] {
                    for (h, m, sec) in [(0, 0, 0), (12, 0, 0), (3, 16, 0), (15, 30, 32)] {
                        let s = card(size, seconds, date);
                        let out = render_ass(at(h, m, sec), &s, "#3584E4");
                        let ctx = format!("{size}pt s={seconds} d={date} {h}:{m:02}:{sec:02}");
                        assert!(!out.is_empty(), "{ctx}");
                        // No float ever reaches the payload, so none of these
                        // spellings can appear however the arithmetic went.
                        for bad in ["NaN", "nan", "inf", "Inf", "."] {
                            assert!(!out.contains(bad), "{ctx}: {bad} in {out}");
                        }
                        for event in out.split('\n') {
                            assert_eq!(
                                event.matches('{').count(),
                                event.matches('}').count(),
                                "{ctx}: unbalanced braces in {event}"
                            );
                            // A drawing is opened and closed exactly once, or
                            // libass keeps treating the *text* as coordinates.
                            let p1 = event.matches("\\p1").count();
                            assert_eq!(p1, event.matches("\\p0").count(), "{ctx}: {event}");
                            assert!(p1 <= 1, "{ctx}: {event}");
                            assert!(event.starts_with("{\\an"), "{ctx}: {event}");
                        }
                        // Every coordinate is an integer inside the pinned box.
                        for d in drawings(&out) {
                            let cs = contours(&d);
                            let (mx, my) = cs[1][0];
                            for (x, y) in cs.into_iter().flatten() {
                                assert!((0..=mx).contains(&x), "{ctx}: x {x} outside {mx}");
                                assert!((0..=my).contains(&y), "{ctx}: y {y} outside {my}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn card_stays_on_screen_at_any_size_or_anchor() {
        // The fit clamp and the origin clamp, together: a hand-edited size can
        // ask for a card larger than the output, and half a card is better than
        // none of one — but no part of it may be placed off the overlay.
        for size in [0, 1, 8, 200, 400, u32::MAX] {
            for anchor in [
                Anchor::TopLeft,
                Anchor::TopRight,
                Anchor::MidCenter,
                Anchor::BottomLeft,
                Anchor::BottomRight,
            ] {
                let s = ClockStyle {
                    anchor,
                    margin_px: u32::MAX,
                    ..card(size, true, true)
                };
                let out = render_ass(at(15, 30, 32), &s, "#3584E4");
                assert!(!out.is_empty(), "{size}pt {anchor:?}");
                for event in out.split('\n') {
                    let pos = event
                        .split_once("\\pos(")
                        .and_then(|(_, r)| r.split_once(')'))
                        .expect("a \\pos")
                        .0;
                    let (x, y) = pos.split_once(',').expect("an x,y");
                    let x: i32 = x.parse().expect("integer x");
                    let y: i32 = y.parse().expect("integer y");
                    assert!((0..=PLAY_RES_X as i32).contains(&x), "{size}pt: {x}");
                    assert!((0..=PLAY_RES_Y as i32).contains(&y), "{size}pt: {y}");
                }
            }
        }
    }

    #[test]
    fn card_never_makes_the_daemon_wake_more_often() {
        // The face is the one part of this module that could quietly cost 60x
        // the power: a second hand has to move every second, so if `Card` drew
        // one unconditionally the payload would differ every second and the
        // "nothing changed" comparison would stop suppressing pushes.
        //
        // It does not. Without seconds there is no second hand at all, and the
        // whole payload — hands included — is byte-identical for the full
        // minute, so the cadence stays exactly what `tick_secs` promises.
        let off = card(120, false, true);
        assert_eq!(tick_secs(&off), 60);
        let want = render_ass(at(15, 30, 0), &off, "#3584E4");
        for sec in 0..60 {
            assert_eq!(render_ass(at(15, 30, sec), &off, "#3584E4"), want, "{sec}s");
        }
        // The minute *does* change it, or the clock would be wrong.
        assert_ne!(render_ass(at(15, 31, 0), &off, "#3584E4"), want);
        // And the second hand appears only when the cadence pays for it.
        let on = card(120, true, true);
        assert_eq!(tick_secs(&on), 1);
        let seconds_drawn = |p: &str| drawings(p).len();
        assert_eq!(
            seconds_drawn(&render_ass(at(15, 30, 32), &on, "#3584E4")),
            seconds_drawn(&want) + 1,
            "seconds should add exactly one drawing, the second hand"
        );
        for sec in 0..60 {
            let a = render_ass(at(15, 30, sec), &on, "#3584E4");
            let b = render_ass(at(15, 30, (sec + 1) % 60), &on, "#3584E4");
            assert_ne!(a, b, "the second hand did not move at {sec}s");
        }
        // The deadline itself is untouched by any of the drawing above.
        for (theme, seconds, want) in [
            (ClockTheme::Card, false, 60),
            (ClockTheme::Card, true, 1),
            (ClockTheme::Digital, false, 60),
            (ClockTheme::Wordy, true, 300),
        ] {
            let s = ClockStyle {
                theme,
                show_seconds: seconds,
                ..Default::default()
            };
            assert_eq!(tick_secs(&s), want, "{theme:?} seconds={seconds}");
        }
    }

    #[test]
    fn card_glass_is_translucent_and_lit_from_above() {
        // The look is translucency plus edge lighting, not frosted glass — ASS
        // has no backdrop blur. These are the three tags that carry it, and a
        // change to any of them is a change to the design rather than a tidy-up.
        let out = render_ass(at(15, 30, 32), &card(120, true, true), "#3584E4");
        let body = out.split('\n').next().expect("the body event");
        // Genuinely see-through, and nowhere near opaque.
        assert!(
            body.contains(&format!("\\1a&H{CARD_BODY_ALPHA:02X}&")),
            "{body}"
        );
        const { assert!(CARD_BODY_ALPHA > 0x60, "the body stopped being glass") };
        const { assert!(CARD_BODY_ALPHA < 0xD0, "the body stopped being readable") };
        // A hairline lit edge rather than a solid stroke.
        assert!(
            body.contains(&format!("\\3a&H{CARD_EDGE_ALPHA:02X}&")),
            "{body}"
        );
        // The scrim is darker than the body: it is the contrast the missing
        // backdrop blur would otherwise have provided.
        const { assert!(CARD_SCRIM_ALPHA < CARD_BODY_ALPHA) };
        assert!(
            out.contains(&format!("\\1a&H{CARD_SCRIM_ALPHA:02X}&")),
            "{out}"
        );
        // The highlight sits in the *top* quarter of the card. A light source
        // above the card lights one edge; lighting all four is an outline.
        let c = card_geom(at(15, 30, 32), &card(120, true, true));
        let highlight = drawings(&out)
            .iter()
            .map(|d| contours(d))
            .find(|d| d.len() == 3 && d[2].len() == 4)
            .expect("a highlight bar");
        let y = highlight[2].iter().map(|p| p.1).max().expect("a point");
        assert!(f32::from(u16::try_from(y).unwrap()) < c.h / 4.0, "at y={y}");
        // Nothing on the card is pure black — a black panel reads as a hole
        // punched in the wallpaper rather than a surface laid on it.
        assert_ne!(CARD_PANEL, "#000000");
        assert_ne!(CARD_FACE, "#000000");
        // And the date row is an ink, not a faded white: over a bright frame
        // behind a translucent card, alpha is the first thing to fail.
        assert!(
            out.contains(&lyrics::hex_to_ass_colour(CARD_DATE_INK)),
            "{out}"
        );
        assert!(
            !out.contains("\\1a&H55&"),
            "the date row went back to alpha"
        );
    }

    #[test]
    fn card_border_is_a_smooth_closed_ramp() {
        // No gradients in ASS, so the edge is flat steps — but they have to walk
        // the ramp out and back so the loop closes on one colour instead of
        // butting pink against cyan, and there have to be enough of them that a
        // long edge does not band.
        let c = card_geom(at(15, 30, 32), &card(120, true, true));
        let segs = card_perimeter(&c, 3.0);
        assert_eq!(segs.len(), CARD_SEGMENTS);
        const { assert!(CARD_SEGMENTS >= 20, "fewer steps than this bands visibly") };
        const {
            assert!(
                CARD_SEGMENTS.is_multiple_of(2),
                "the ramp must fold on a piece"
            )
        };
        // Four corners, the rest straight runs.
        let arcs = segs.iter().filter(|s| matches!(s, Seg::Arc { .. })).count();
        assert_eq!(arcs, 4);
        // The ramp: piece 0 and the last piece are near-identical colours, and
        // the halfway piece is the far end. That is what closes the loop.
        let colour = |i: usize| {
            let u = i as f32 / (CARD_SEGMENTS as f32 / 2.0);
            ramp(if u <= 1.0 { u } else { 2.0 - u })
        };
        assert_eq!(colour(0), hex(CARD_NEON_A));
        assert_eq!(colour(CARD_SEGMENTS / 2), hex(CARD_NEON_B));
        // Adjacent steps never jump more than a tenth of the ramp, at any
        // position round the loop including the wrap.
        for i in 0..CARD_SEGMENTS {
            let (a, b) = (colour(i), colour((i + 1) % CARD_SEGMENTS));
            let bytes = |h: &str| {
                (0..3)
                    .map(|k| i32::from_str_radix(&h[1 + 2 * k..3 + 2 * k], 16).expect("hex"))
                    .collect::<Vec<_>>()
            };
            let (a, b) = (bytes(&a), bytes(&b));
            let step: i32 = (0..3)
                .map(|k| (a[k] - b[k]).abs())
                .max()
                .expect("a channel");
            assert!(step <= 48, "step {i} jumps {step}");
        }
        // Every corner turns on the same radius, so no corner is rounder than
        // another and the ring's thickness is constant round the turn.
        for s in &segs {
            if let Seg::Arc { a0, a1, .. } = s {
                assert!((a1 - a0 - FRAC_PI_2).abs() < 1e-5, "not a quarter turn");
            }
        }
    }

    #[test]
    fn card_text_rows_are_placed_and_escaped() {
        // The rows are the only part of the card that is type rather than
        // shape, and they still have to be whole events with no inheritance.
        let out = render_ass(at(15, 30, 32), &card(120, true, true), "#3584E4");
        let rows: Vec<&str> = out.split('\n').filter(|e| !e.contains("\\p1")).collect();
        assert_eq!(rows.len(), 3, "time, weekday, date");
        for r in &rows {
            assert!(r.starts_with("{\\an8\\pos("), "{r}");
            for tag in ["\\fn", "\\fs", "\\b", "\\fsp", "\\1c", "\\3c", "\\1a"] {
                assert!(r.contains(tag), "{r} is missing {tag}");
            }
        }
        assert!(rows[0].ends_with("}15:30:32"), "{}", rows[0]);
        assert!(rows[1].ends_with("}WEDNESDAY"), "{}", rows[1]);
        assert!(rows[2].ends_with("}15 JUL 2026"), "{}", rows[2]);
        // Without a date there are two rows and no stray third `\pos`.
        let no_date = render_ass(at(15, 30, 32), &card(120, true, false), "#3584E4");
        assert_eq!(
            no_date.split('\n').filter(|e| !e.contains("\\p1")).count(),
            2
        );
    }
}

#[cfg(test)]
mod scratch_dump {
    use super::*;
    use chrono::TimeZone;

    #[test]
    #[ignore]
    fn dump() {
        let dir = std::env::var("CARD_DUMP").unwrap_or_default();
        if dir.is_empty() {
            return;
        }
        // The matrix the design pass is judged against: both ends of the size
        // range, seconds and the date each way, and three times — noon and
        // 15:16, where the hands nearly coincide and a weak hierarchy is
        // unreadable, plus the 15:30:32 from the maintainer's screenshot.
        for &(h, m, sec) in &[(12u32, 0u32, 0u32), (15, 16, 0), (15, 30, 32)] {
            let now = Local
                .with_ymd_and_hms(2025, 7, 28, h, m, sec)
                .earliest()
                .unwrap();
            for &size in &[40u32, 64, 120] {
                for &seconds in &[false, true] {
                    for &date in &[false, true] {
                        let s = ClockStyle {
                            theme: ClockTheme::Card,
                            anchor: Anchor::MidCenter,
                            font_size_pt: size,
                            show_seconds: seconds,
                            show_date: date,
                            ..Default::default()
                        };
                        let name = format!(
                            "{h:02}{m:02}{sec:02}_{size}_{}_{}",
                            if seconds { "sec" } else { "nosec" },
                            if date { "date" } else { "nodate" },
                        );
                        let payload = render_ass(now, &s, "#3584E4");
                        std::fs::write(format!("{dir}/{name}.txt"), payload).unwrap();
                    }
                }
            }
        }
    }
}

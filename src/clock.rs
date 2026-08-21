//! Desktop clock widget: the time, themed, drawn over the wallpaper
//! (WIDGETS_ROADMAP W4 — the clock, brought forward onto the W1 substrate).
//!
//! Nothing but **pure functions over a timestamp** — no I/O, no globals, no
//! clock of its own — so every rule in here is unit-testable and the daemon
//! keeps ownership of *when* things happen.
//!
//! # This module decides what to say; it does not draw
//!
//! The clock used to emit an ASS payload from here, and a `\p1` vector card
//! with drawn hands on top of that. Both are gone: the widget is rasterised now
//! by [`crate::widgetkit::cards::clock`], which takes a
//! [`ClockData`](crate::widgetkit::ClockData) of plain strings and owns every
//! decision about face, weight, tracking, scrims and shadow. What is left here
//! is the language — [`format_time`], [`time_line`], [`weekday`],
//! [`date_line`], [`secondary_line`] — plus the one design decision a
//! rasteriser cannot make for us, the per-theme [`hero_size`].
//!
//! [`widest_time`] is the other half of that contract and the easiest to skip:
//! a card sized from what the clock says *now* resizes once a second with
//! seconds on, and at noon in 12-hour mode. The card is sized from the widest
//! string the current settings can ever produce instead.
//!
//! # Power: this module exists to let the daemon sleep
//!
//! A clock is the one widget whose content changes on a schedule nobody has to
//! discover, so it must never be polled. [`next_change`] returns the *exact
//! instant* the visible string next differs, and the daemon waits on that
//! deadline:
//!
//! ```text
//! render  ->  sleep until next_change()  ->  wake  ->  render  ->  ...
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
//! — so it is exactly the right thing to derive the widget's content key from,
//! and the daemon does.
//!
//! # Untrusted input
//!
//! Nothing visible here comes from a user: the strings are generated from a
//! timestamp and a `chrono` format. [`ClockStyle::colour`] *is* hand-editable,
//! and it reaches no layout decision at all — the card's palette comes from
//! [`crate::widgetkit::Theme`] and the only thing an unparseable colour can
//! cost is the tint.

use chrono::{DateTime, Local, TimeDelta, Timelike};
use serde::{Deserialize, Serialize};

use crate::lyrics::Anchor;

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
    /// run of text. On the rasterised substrate that is
    /// [`crate::widgetkit::ClockVariant::Expanded`]: the full three-row card
    /// plus the day-progress arc gauge in a second column. The drawn analog
    /// face went with ASS — it was thirty-odd `\p1` vector events faking one
    /// dial — and the gauge is the honest replacement.
    ///
    /// Listed last because it is still the most expensive look to draw.
    Card,
    /// **NOS** — the author's own design language rather than a reference's: a
    /// near-square squircle with a dot-matrix hero, a ring of discrete dots
    /// carrying the day's progress, and monochrome plus one red. Maps to
    /// [`crate::widgetkit::ClockVariant::Nos`], which owns every dimension.
    ///
    /// A *theme* and not a palette. `config::WidgetTheme` decides light or
    /// dark for the whole widget layer; NOS renders in both, and putting a form
    /// language into the palette key would have made "dark" and "NOS" mutually
    /// exclusive, which they are not.
    Nos,
}

impl ClockTheme {
    /// Every theme, in the order a picker should list them — cheapest look
    /// first — so a GUI does not hand-list the variants a second time.
    pub const ALL: [ClockTheme; 7] = [
        ClockTheme::Digital,
        ClockTheme::Minimal,
        ClockTheme::Segment,
        ClockTheme::Stacked,
        ClockTheme::Wordy,
        ClockTheme::Card,
        ClockTheme::Nos,
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
            ClockTheme::Nos => "NOS",
        }
    }
}

/// A resolved clock look: everything the daemon needs to build a
/// [`crate::widgetkit::ClockData`] except the accent colour and the palette,
/// which it resolves from the config.
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
    /// Size of the *time* line in logical units (pixels at 1080p). Each theme
    /// scales it — see [`hero_size`]: `Stacked` runs larger, `Minimal` smaller.
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
    /// Fill colour as `#RRGGBB`. Ignored when `accent_follow` is set, and
    /// ignored by the card renderer, which takes its ink from the palette.
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
/// A date, when present, is on a second line separated by a real `'\n'`. The
/// card does not consume this — it wants the rows separately, from
/// [`time_line`], [`weekday`] and [`date_line`] — but the daemon derives the
/// widget's **content key** from it, because it is the one string that is
/// byte-identical for exactly as long as the visible clock is unchanged.
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
    // it builds its own stack out of the same rows the card is handed.
    if matches!(s.theme, ClockTheme::Card) {
        let mut out = time_line(now, s);
        out.push('\n');
        out.push_str(&weekday(now, s.theme));
        if shows_date(s) {
            out.push('\n');
            out.push_str(&date_line(now, s));
        }
        return out;
    }
    let mut out = time_line(now, s);
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
    if date_upper(theme) {
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
// What a clock card draws
// ---------------------------------------------------------------------------

/// Time-line size as a percentage of [`ClockStyle::font_size_pt`].
///
/// The whole of what is left of the old ASS "look" table, and the only part of
/// it that survives the move to a rasterised card: face, weight, tracking,
/// outline and blur are all decided by `widgetkit`'s type ladder and contrast
/// model now, but the *relative size* of the themes is a design decision this
/// module still owns — it is what keeps `Minimal` quiet and `Stacked` loud at
/// one shared `font_size_pt`.
const fn size_pct(theme: ClockTheme) -> u32 {
    match theme {
        ClockTheme::Digital => 100,
        ClockTheme::Minimal => 55,
        ClockTheme::Segment => 95,
        ClockTheme::Stacked => 130,
        ClockTheme::Wordy => 62,
        ClockTheme::Card => 100,
        // NOS sizes its whole card from `font_size_pt` and then solves the
        // hero against the ring's inner chord, so the setting arrives
        // unscaled and `cards::nos` does the rest.
        ClockTheme::Nos => 100,
    }
}

/// Whether this theme's date row is set in caps.
const fn date_upper(theme: ClockTheme) -> bool {
    matches!(
        theme,
        ClockTheme::Segment | ClockTheme::Stacked | ClockTheme::Card | ClockTheme::Nos
    )
}

/// The rendered size of the time line, in logical units (pixels at 1080p).
///
/// `font_size_pt` is what the user set; this is what is actually drawn, after
/// the theme's own scaling and the clamp against a hand-edited config.
pub fn hero_size(s: &ClockStyle) -> u32 {
    scaled(s.font_size_pt, size_pct(s.theme))
}

/// The time, with no date and no markup — the hero row of the card.
///
/// Split out of [`format_time`] because a card places the time, the weekday and
/// the date in three separate rows and cannot use a newline-joined string.
pub fn time_line(now: DateTime<Local>, s: &ClockStyle) -> String {
    match s.theme {
        ClockTheme::Wordy => wordy_time(now.hour(), now.minute()),
        _ => numeric_time(now, s),
    }
}

/// The widest string [`time_line`] can **ever** produce under these settings.
///
/// The card sizes its width from this rather than from what the clock says
/// right now, because otherwise `show_seconds` resizes the card once a second
/// and 12-hour time resizes it at noon — which looks like a rendering fault
/// rather than like a clock.
///
/// Numeric themes have a closed form (`00:00`, `00:00:00`, `+ " PM"`); `Wordy`
/// does not, so it is brute-forced over the 144 phrases it can say. That is
/// 144 small `format!`s once a minute at the very most — the widget only
/// re-renders when its text changes — against a card that visibly jumps every
/// time the phrase length changes.
pub fn widest_time(s: &ClockStyle) -> String {
    if matches!(s.theme, ClockTheme::Wordy) {
        let mut widest = String::new();
        for h in 0..12u32 {
            for five in 0..12u32 {
                let candidate = wordy_time(h, five * 5);
                if candidate.chars().count() > widest.chars().count() {
                    widest = candidate;
                }
            }
        }
        return widest;
    }
    // 00 is the widest hour in both clocks: 12-hour never prints a leading
    // zero, so the widest it reaches is a two-digit hour, and 24-hour always
    // pads to two.
    let mut out = "00:00".to_string();
    if shows_seconds(s) {
        out.push_str(":00");
    }
    if !s.use_24h {
        out.push(' ');
        out.push_str(if matches!(s.theme, ClockTheme::Minimal) {
            "am"
        } else {
            "AM"
        });
    }
    out
}

/// The weekday for the card's micro-label, e.g. `"Monday"`.
///
/// Never empty and never governed by `show_date`: the micro row is the one the
/// card's layout is built around, and a card with a hole where a row goes is a
/// different design. `%A` rather than `%a` because there is room for it and an
/// abbreviated weekday is a saving nobody asked for.
pub fn weekday(now: DateTime<Local>, theme: ClockTheme) -> String {
    let d = now.format("%A").to_string();
    if date_upper(theme) {
        d.to_uppercase()
    } else {
        d
    }
}

/// The date for the card's micro-label, e.g. `"28 July"`, or empty when
/// [`shows_date`] says no.
///
/// Day-then-month with the month spelled out, for the same reason the
/// secondary row uses `%a %d %b`: `15/07` and `07/15` mean opposite things
/// depending on where you learned to write dates, and "15 July" does not.
pub fn date_line(now: DateTime<Local>, s: &ClockStyle) -> String {
    if !shows_date(s) {
        return String::new();
    }
    let d = now.format("%-d %B").to_string();
    if date_upper(s.theme) {
        d.to_uppercase()
    } else {
        d
    }
}

/// The card's secondary row — how much of the local day is left — or empty.
///
/// # What this used to say, and why it does not any more
///
/// It said `Week 34 · GMT+05:30`. Both halves are developer trivia: an ISO
/// week number is a value almost nobody can act on and most people cannot
/// place within a fortnight, and a UTC offset is a fact about the machine's
/// configuration, not about the day — it is already correct, or the clock
/// beside it is wrong. Neither ever changes anything anyone does. On a card
/// whose whole argument is that a widget should earn the desktop it covers,
/// that row was the weakest thing on it.
///
/// It now says how much of the day is left, which is a number people already
/// keep in their heads and check against — and, on [`ClockTheme::Nos`], it is
/// the number the dotted ring draws, so the row is also the arc's label
/// (`cards::nos` binds the two with a legend dot). One string, two jobs.
///
/// Costs nothing: it changes once a minute, and the clock already repaints at
/// least that often — [`next_change`] is unaffected.
///
/// Tied to [`shows_date`], except on NOS where the ring needs it whatever the
/// switch says. Empty removes the row and the card's height recomputes; it
/// never renders an empty band.
pub fn secondary_line(now: DateTime<Local>, s: &ClockStyle) -> String {
    if !shows_date(s) && !matches!(s.theme, ClockTheme::Nos) {
        return String::new();
    }
    day_remaining(now, tick_secs(s))
}

/// `"9h 28m left today"`, `"47m left today"`, `"3h left today"`.
///
/// **Quantised to the caller's own repaint bucket**, which is a cadence
/// decision rather than a rounding one and is the whole reason `step_secs` is
/// a parameter.
///
/// `second()` is not read at all: a value computed from the second changes at
/// some arbitrary point *inside* every minute, so it would either disagree
/// with the time above it for most of each minute or force the clock awake
/// once a second to keep them in step. And a minute is not fine enough either
/// — [`ClockTheme::Wordy`] repaints every five, so a per-minute row inside it
/// would be wrong for up to four minutes at a time or would drag the theme's
/// cadence down to a per-minute one. So the minute-of-day is snapped to the
/// nearest multiple of the caller's own tick before the subtraction, exactly
/// as `Wordy` snaps the time it prints. `crate::clock`'s whole power model is
/// "repaint when the picture changes"; this row is free only because it
/// changes precisely when the hero does.
///
/// Three forms rather than one, because `0h 47m` is how a machine says it. The
/// hour is dropped when it is zero and the minute when *it* is. Never zero and
/// never negative: at 23:59 there is one minute left, and at midnight 1440.
pub fn day_remaining(now: DateTime<Local>, step_secs: u32) -> String {
    let step = i64::from(step_secs.max(1) / 60).max(1);
    let mins = i64::from(now.hour() * 60 + now.minute());
    // Nearest, not floor: `Wordy` says "five past ten" at 10:03, and a row
    // beside it that had already floored to ten o'clock would contradict it.
    let snapped = ((mins as f64 / step as f64).round() as i64) * step;
    let left = (1440 - snapped).clamp(1, 1440);
    let (h, m) = (left / 60, left % 60);
    if h == 0 {
        return crate::tf!("{m}m left today", "m" => m.to_string());
    }
    if m == 0 {
        return crate::tf!("{h}h left today", "h" => h.to_string());
    }
    crate::tf!("{h}h {m}m left today", "h" => h.to_string(), "m" => m.to_string())
}

/// How much of the local day has elapsed, in `0.0..1.0` — the arc gauge's
/// value on the expanded card.
///
/// Deliberately **not** part of the widget's content key: it moves
/// continuously, and a continuously moving quantity in a content key redraws at
/// the loop rate. It does not need to be, either — the gauge advances by
/// 1/1440 a minute and the time string it sits beside changes at least that
/// often, so every repaint the gauge could want it already gets.
pub fn day_fraction(now: DateTime<Local>) -> f32 {
    let secs = now.hour() * 3600 + now.minute() * 60 + now.second();
    (secs as f32 / 86_400.0).clamp(0.0, 1.0)
}

/// Every string one clock card draws, owned.
///
/// The card renderer takes borrowed `&str`s and this is what they borrow from:
/// the rows are built once per repaint, hashed into the widget's content key,
/// and then handed to the rasteriser. Owning them in one struct rather than
/// passing five `String`s around is what makes "the key and the pixels came
/// from the same instant" true by construction instead of by review.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ClockText {
    /// The hero row.
    pub time: String,
    /// The widest string the hero can ever be under these settings.
    pub widest: String,
    /// The micro-label's weekday. Never empty.
    pub weekday: String,
    /// The micro-label's date, or empty.
    pub date: String,
    /// The secondary row, or empty.
    pub secondary: String,
}

impl ClockText {
    /// Everything the card draws at `now` under `s`.
    pub fn of(now: DateTime<Local>, s: &ClockStyle) -> Self {
        ClockText {
            time: time_line(now, s),
            widest: widest_time(s),
            weekday: weekday(now, s.theme),
            date: date_line(now, s),
            secondary: secondary_line(now, s),
        }
    }
}

/// Which density the rasteriser draws this theme at.
///
/// Two of the six pin a variant instead of letting the size choose:
/// [`ClockTheme::Minimal`] is defined as "present without asking for
/// attention", which is exactly [`crate::widgetkit::ClockVariant::Bare`] — no
/// card at all — and [`ClockTheme::Card`] is the one theme that is a picture
/// rather than a run of text, so it takes the expanded card with the gauge. The
/// rest let the size decide, which is what
/// [`crate::widgetkit::ClockVariant::Auto`] is for.
#[cfg(feature = "daemon")]
pub fn card_variant(theme: ClockTheme) -> crate::widgetkit::ClockVariant {
    use crate::widgetkit::ClockVariant as V;
    match theme {
        ClockTheme::Minimal => V::Bare,
        ClockTheme::Stacked => V::Standard,
        ClockTheme::Card => V::Expanded,
        ClockTheme::Nos => V::Nos,
        _ => V::Auto,
    }
}

#[cfg(feature = "daemon")]
impl ClockText {
    /// Borrow these rows as the rasteriser's data struct.
    ///
    /// `day_fraction` is passed rather than recomputed so the caller can keep it
    /// out of the content key — see [`day_fraction`].
    pub fn card_data<'a>(
        &'a self,
        s: &ClockStyle,
        day_fraction: f32,
    ) -> crate::widgetkit::ClockData<'a> {
        crate::widgetkit::ClockData {
            time: &self.time,
            widest_time: &self.widest,
            weekday: &self.weekday,
            date: &self.date,
            secondary: &self.secondary,
            font_size: hero_size(s) as f32,
            variant: card_variant(s.theme),
            accent_follow: s.accent_follow,
            day_fraction,
        }
    }
}

/// `base * pct / 100`, clamped to a size that can actually be drawn.
///
/// Saturating because `font_size_pt` comes from a hand-editable config and
/// `u32::MAX * 130` is not a size, it is an overflow.
fn scaled(base: u32, pct: u32) -> u32 {
    (base.saturating_mul(pct) / 100).clamp(MIN_SIZE_PT, MAX_SIZE_PT)
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
        assert_eq!(ClockTheme::ALL.len(), 7);
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
                ClockText::of(now, &style(theme)).time.contains(want),
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
        // **The content-key contract**, and it matters more now than it did
        // when the widget was a string: the daemon hashes [`ClockText`] and
        // rasterises whenever the hash moves. A row that wobbled inside its
        // bucket would not merely defeat a string comparison — it would
        // re-measure, re-draw and re-write a card to disk on every one of the
        // ten ticks a second the loop makes.
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

        // Every row of every theme, not just the hero: the key is hashed from
        // the whole struct, so one field that wobbles is one redraw per tick.
        // 10:03:00 is a bucket boundary for the coarsest theme as well as for
        // the minute ones, so the whole bucket is walked from its start.
        for theme in ClockTheme::ALL {
            for &show_date in &[false, true] {
                let s = ClockStyle {
                    show_date,
                    ..style(theme)
                };
                let start = at(10, 3, 0);
                let first = ClockText::of(start, &s);
                for sec in 1..i64::from(tick_secs(&s)) {
                    let now = start + TimeDelta::seconds(sec);
                    assert_eq!(
                        ClockText::of(now, &s),
                        first,
                        "{theme:?} (date={show_date}) moved {sec}s into its bucket"
                    );
                }
            }
        }

        // With seconds on, the bucket is a second — every one differs, and the
        // key differs with it.
        let seconds = ClockStyle {
            show_seconds: true,
            ..minute
        };
        let mut seen: Vec<ClockText> = (0..60)
            .map(|s| ClockText::of(at(21, 5, s), &seconds))
            .collect();
        seen.dedup();
        assert_eq!(seen.len(), 60);
    }

    // -- what the card is told ----------------------------------------------
    //
    // The clock no longer emits a payload, so there is no payload to parse back
    // out. What replaced those assertions is the contract the rasteriser
    // actually reads: five strings, a size, and a gauge fraction.

    /// Every row the card is handed, for one instant and one style.
    fn rows(now: DateTime<Local>, s: &ClockStyle) -> (String, String, String, String) {
        (
            time_line(now, s),
            weekday(now, s.theme),
            date_line(now, s),
            secondary_line(now, s),
        )
    }

    #[test]
    fn every_theme_hands_the_card_a_full_set_of_rows() {
        // The card's layout has no "hole where a row goes" state: the hero and
        // the micro-label always have content, and the two optional rows are
        // *absent* rather than empty when they are off.
        for theme in ClockTheme::ALL {
            for date in [false, true] {
                let s = ClockStyle {
                    show_date: date,
                    ..style(theme)
                };
                let (time, day, d, sec) = rows(at(21, 5, 0), &s);
                assert!(!time.is_empty(), "{theme:?}: no hero");
                assert!(!day.is_empty(), "{theme:?}: no micro-label");
                assert_eq!(
                    d.is_empty(),
                    !shows_date(&s),
                    "{theme:?}: the date row disagrees with shows_date"
                );
                // The secondary row comes and goes with the date — except on
                // NOS, where it is the dotted ring's label rather than a third
                // row of extras, and a ring with no caption is the unlabelled
                // arc §8.3 exists to forbid.
                let want_secondary = shows_date(&s) || theme == ClockTheme::Nos;
                assert_eq!(
                    sec.is_empty(),
                    !want_secondary,
                    "{theme:?}: the secondary row disagrees with its rule"
                );
                // No markup ever reaches the card, in either direction.
                for row in [&time, &day, &d, &sec] {
                    assert!(!row.contains('\n'), "{theme:?}: {row:?} carries a break");
                    assert!(!row.contains('{'), "{theme:?}: {row:?} carries markup");
                }
            }
        }
    }

    #[test]
    fn the_card_is_sized_from_the_widest_string_the_settings_can_produce() {
        // The width-stability contract (`ClockData::widest_time`). If the card
        // were sized from the current string it would resize once a second with
        // seconds on, and again at noon in 12-hour mode — which reads as a
        // rendering fault, not as a clock.
        for theme in ClockTheme::ALL {
            for use_24h in [false, true] {
                for seconds in [false, true] {
                    let s = ClockStyle {
                        use_24h,
                        show_seconds: seconds,
                        ..style(theme)
                    };
                    let widest = widest_time(&s);
                    assert!(!widest.is_empty(), "{theme:?}");
                    for h in 0..24 {
                        for m in [0u32, 5, 9, 11, 25, 30, 45, 58, 59] {
                            let now = time_line(at(h, m, 59), &s);
                            assert!(
                                now.chars().count() <= widest.chars().count(),
                                "{theme:?} {use_24h} {seconds}: {now:?} is wider than {widest:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_themes_are_visibly_different() {
        // A "themes" feature whose cards differ in nothing is not a themes
        // feature. Face, weight and tracking belong to the rasteriser now, so
        // what this module still owns — and what this pins — is the relative
        // size of the hero and the casing of the dated rows.
        let sizes: Vec<u32> = ClockTheme::ALL
            .iter()
            .map(|&t| hero_size(&style(t)))
            .collect();
        assert_eq!(hero_size(&style(ClockTheme::Digital)), 64);
        // Minimal is the quiet one and Stacked the loud one; that ordering is
        // the design, not an accident of the table.
        assert!(
            sizes.iter().min() == Some(&hero_size(&style(ClockTheme::Minimal))),
            "{sizes:?}"
        );
        assert!(
            sizes.iter().max() == Some(&hero_size(&style(ClockTheme::Stacked))),
            "{sizes:?}"
        );
        // Segment, Stacked and Card set their dated rows in caps; the rest do
        // not, and that is visible at a glance on the card.
        let now = at(21, 5, 0);
        for theme in ClockTheme::ALL {
            let s = ClockStyle {
                show_date: true,
                ..style(theme)
            };
            let day = weekday(now, theme);
            if !shows_date(&s) {
                continue;
            }
            assert_eq!(
                day == day.to_uppercase(),
                date_upper(theme),
                "{theme:?}: {day:?}"
            );
            let d = date_line(now, &s);
            assert_eq!(d == d.to_uppercase(), date_upper(theme), "{theme:?}: {d:?}");
        }
    }

    #[test]
    fn extreme_sizes_are_clamped_not_trusted() {
        // `config.toml` is hand-editable. A zero size draws nothing at all and
        // a ten-digit one would overflow the theme's percentage before it ever
        // reached a font.
        for theme in ClockTheme::ALL {
            let tiny = hero_size(&ClockStyle {
                font_size_pt: 0,
                ..style(theme)
            });
            assert_eq!(tiny, MIN_SIZE_PT, "{theme:?}");
            let huge = hero_size(&ClockStyle {
                font_size_pt: u32::MAX,
                ..style(theme)
            });
            assert_eq!(huge, MAX_SIZE_PT, "{theme:?}");
        }
    }

    #[test]
    fn a_hand_edited_colour_cannot_reach_the_layout() {
        // The ASS ancestor of this test — `hostile_colours_cannot_escape_the
        // _override_block` — checked that a `colour = "}{\\an7"` could not close
        // the override block and re-anchor the widget. There is no override
        // block to escape any more, and the escape class went with it.
        //
        // The *intent* survives and is now stronger: the colour key reaches no
        // layout decision at all. Whatever is in it, every row the card is
        // handed and the size it is drawn at are byte-identical.
        let now = at(21, 5, 0);
        let sane = ClockStyle {
            show_date: true,
            colour: "#3584E4".to_string(),
            ..style(ClockTheme::Digital)
        };
        for hostile in [
            "}{\\an7\\pos(0,0)",
            "#FF0000\\N\\N\\N",
            "\n\n{\\fs400}",
            "",
            "not a colour at all",
            "#GGGGGG",
        ] {
            let s = ClockStyle {
                colour: hostile.to_string(),
                ..sane.clone()
            };
            assert_eq!(rows(now, &s), rows(now, &sane), "colour = {hostile:?}");
            assert_eq!(hero_size(&s), hero_size(&sane), "colour = {hostile:?}");
            // And it cannot smuggle itself into the content key either, which
            // is what would otherwise turn a mistyped colour into a redraw
            // every tick. `ClockText` *is* the key, so this is the whole of it
            // rather than the hero row alone.
            assert_eq!(
                ClockText::of(now, &s),
                ClockText::of(now, &sane),
                "colour = {hostile:?}"
            );
        }
    }

    #[test]
    fn the_day_gauge_is_a_fraction_and_never_leaves_its_range() {
        // The Expanded card's arc gauge. Out of range it would sweep past its
        // own track, and `arc_gauge` would clamp it silently — better to be
        // right here, where it can be tested.
        assert!(day_fraction(at(0, 0, 0)).abs() < 1e-6);
        assert!((day_fraction(at(12, 0, 0)) - 0.5).abs() < 1e-6);
        for h in 0..24 {
            for m in [0u32, 30, 59] {
                let f = day_fraction(at(h, m, 59));
                assert!((0.0..1.0).contains(&f), "{h}:{m} gave {f}");
            }
        }
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

    #[test]
    fn the_card_never_makes_the_daemon_wake_more_often() {
        // The ASS `Card` theme could quietly cost 60x the power: a drawn second
        // hand moves every second, so the payload differed every second even
        // with seconds off, and the daemon's "nothing changed" comparison
        // stopped suppressing pushes.
        //
        // The hands are gone, but the trap is not: the same thing happens if
        // *any* continuously moving quantity reaches the widget's content key.
        // `format_time` is that key, so it is pinned here — for every theme, at
        // every second of a minute.
        for theme in ClockTheme::ALL {
            let off = ClockStyle {
                show_seconds: false,
                show_date: true,
                font_size_pt: 120,
                ..style(theme)
            };
            assert_eq!(
                tick_secs(&off),
                if theme == ClockTheme::Wordy { 300 } else { 60 }
            );
            let want = format_time(at(15, 30, 0), &off);
            for sec in 0..60 {
                assert_eq!(format_time(at(15, 30, sec), &off), want, "{theme:?} {sec}s");
            }
            // The rows the card draws move on exactly the same schedule; a row
            // outside the key would repaint without the key noticing.
            let want_rows = rows(at(15, 30, 0), &off);
            for sec in 0..60 {
                assert_eq!(rows(at(15, 30, sec), &off), want_rows, "{theme:?} {sec}s");
            }
        }
        // And the deadline is what it always was.
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
}

/// Design-review dump: every clock card, as a PNG.
///
/// `#[ignore]`d and gated on `CARD_DUMP` because it writes files and is here to
/// be *looked at*, not to pass. The ASS ancestor wrote `.txt` payloads for the
/// same purpose; on a rasterised substrate the reviewable artefact is the
/// image, so it renders each card onto a real [`Canvas`](crate::widgetkit::Canvas)
/// — at 1x and at 2x, because bleed, hairlines and scrim feather are the three
/// things that only go wrong at density — and saves it.
///
/// ```text
/// CARD_DUMP=/tmp/cards cargo test --all-features -- --ignored clock::scratch_dump
/// ```
#[cfg(all(test, feature = "daemon"))]
mod scratch_dump {
    use super::*;
    use crate::widgetkit::{cards, Canvas, FontStack, Mode, Theme};
    use chrono::TimeZone;

    #[test]
    #[ignore]
    fn dump() {
        let dir = std::env::var("CARD_DUMP").unwrap_or_default();
        if dir.is_empty() {
            return;
        }
        std::fs::create_dir_all(&dir).expect("the dump directory");
        let mut fonts = FontStack::system();
        // The matrix the design pass is judged against: every theme, both ends
        // of the size range, the date each way, both palettes and both
        // densities. 15:30:32 is the maintainer's screenshot; noon is where the
        // day gauge is exactly half.
        for &(h, m, sec) in &[(12u32, 0u32, 0u32), (15, 30, 32)] {
            let now = Local
                .with_ymd_and_hms(2025, 7, 28, h, m, sec)
                .earliest()
                .unwrap();
            for theme in ClockTheme::ALL {
                for &size in &[40u32, 64, 120] {
                    for &date in &[false, true] {
                        for (mode, tag) in [(Mode::Dark, "dark"), (Mode::Light, "light")] {
                            for &scale in &[1.0_f32, 2.0] {
                                let s = ClockStyle {
                                    theme,
                                    font_size_pt: size,
                                    show_date: date,
                                    show_seconds: sec != 0,
                                    ..Default::default()
                                };
                                let t = Theme::for_accent(mode, crate::config::Accent::Blue);
                                let text = ClockText::of(now, &s);
                                let data = text.card_data(&s, day_fraction(now));
                                let size_of = cards::clock::measure(&mut fonts, &t, &data, scale);
                                let mut canvas = Canvas::for_logical(size_of.buffer(), scale)
                                    .expect("a card-sized canvas");
                                canvas.reset();
                                cards::clock::draw(&mut canvas, &mut fonts, &t, &data);
                                let name = format!(
                                    "{h:02}{m:02}{sec:02}_{}_{size}_{}_{tag}_{}x.png",
                                    theme.label().to_lowercase(),
                                    if date { "date" } else { "nodate" },
                                    scale as u32,
                                );
                                canvas.save_png(format!("{dir}/{name}")).expect("png");
                            }
                        }
                    }
                }
            }
        }
    }
}

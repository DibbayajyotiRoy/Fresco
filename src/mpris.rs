//! MPRIS now-playing source for the lyrics widget (WIDGETS_ROADMAP W1).
//!
//! No DBus crate: like `src/daemon/dde.rs`, we shell out to `gdbus` and parse
//! its output. `gdbus` is the only D-Bus CLI present in **both** the
//! `org.freedesktop.Platform` and `org.gnome.Platform` Flatpak runtimes
//! (`busctl` and `dbus-monitor` are absent from both), so it is also the
//! Flathub-safe choice. `zbus` would add 66 crates and an async runtime to a
//! deliberately synchronous codebase.
//!
//! # Layering
//!
//! Everything here is either a **pure function** (parsing, player selection,
//! the position clock, the degradation detector) or a **short blocking query**
//! (`list_players`, `get_all`, `get_position_us`, `get_status`). This module
//! owns no threads and starts none: the daemon's 100ms loop must never call a
//! query inline. The intended shape is a caller-owned worker thread that
//! publishes a snapshot the loop reads:
//!
//! ```text
//!   worker thread                                  daemon tick (100ms)
//!   ─────────────                                  ───────────────────
//!   gdbus monitor  ──► PropertiesChanged/Seeked  ──► clock.predicted_us(now)
//!   1s Position poll while Playing ──► clock.resync()   (no I/O, no lock wait)
//!   nothing at all while Paused/Stopped
//! ```
//!
//! # Power
//!
//! Zero CPU when nothing plays: with no player on the bus there is no poll and
//! no subprocess, and [`PositionClock`] freezes whenever the status is not
//! `Playing`, so a paused player must never be polled. A parked
//! `gdbus monitor` costs 0 CPU ticks over 10s; a `gdbus call` costs ~3.1ms.
//!
//! # Parsing hazards
//!
//! `gdbus` prints GVariant *text*, which is hostile in three specific ways —
//! all handled by [`parse_gvariant`] and covered by tests against captured
//! output. `dde.rs`'s `parse_first_string` must **not** be reused for
//! `Metadata`; see [`GVal`].

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// D-Bus well-known-name prefix every MPRIS player registers under. Matched as
/// a **prefix**: real names look like `org.mpris.MediaPlayer2.vlc` and
/// `org.mpris.MediaPlayer2.firefox.instance_1_1234`.
pub const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

/// The single object path every MPRIS player exposes.
pub const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";

/// The MPRIS interface carrying playback state and metadata.
pub const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";

/// `gdbus --timeout` in seconds. Short on purpose: a wedged player must not
/// hold a worker thread for the default 25s. Two seconds is ~600x the measured
/// 3.1ms round trip.
const CALL_TIMEOUT_SECS: &str = "2";

/// The MPRIS-blessed "no track" object path. Players emit it between tracks;
/// it is not a usable track identity.
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";

// ---------------------------------------------------------------------------
// GVariant text parsing
// ---------------------------------------------------------------------------

/// A parsed GVariant text value.
///
/// `gdbus` prints GLib's GVariant text form, which cannot be handled by
/// slicing or a regex:
///
/// * **The quote character depends on the content.** `g_variant_print` uses
///   `'` normally but switches to `"` when the string contains an apostrophe,
///   and then escapes only `"` and `\` — so `` `<"Don't Stop">` `` has an
///   unescaped apostrophe inside double quotes, while
///   `` `<'Album "Quoted"'>` `` has unescaped double quotes inside single ones.
/// * **`xesam:artist` is an array whose elements contain commas** —
///   `` `<['A, Band', 'Guest']>` ``. Splitting on `,` silently corrupts artist
///   names.
/// * **Scalar type prefixes are inconsistent** — `` `<int64 245000000>` ``,
///   `` `<objectpath '/x'>` ``, `` `<@as []>` `` and `` `<true>` `` carry a
///   prefix, but `` `<'string'>` ``, `` `<254>` `` and `` `<1.0>` `` do not.
///
/// Variant wrappers (`` `<...>` ``) are unwrapped transparently — there is no
/// `Variant` case, because for our purposes `` `<int64 5>` `` and `5` are the
/// same number.
#[derive(Debug, Clone, PartialEq)]
pub enum GVal {
    /// A string, object path, signature or bytestring, unescaped.
    Str(String),
    /// Any signed/unsigned integer type, including `byte 0x41` hex literals.
    Int(i64),
    /// A double (`1.0`, `-1.5e3`, `inf`, `nan`).
    Float(f64),
    /// `true` / `false`.
    Bool(bool),
    /// An array — `` `['a', 'b']` `` or `` `@as []` ``.
    Arr(Vec<GVal>),
    /// A dictionary, in printed order — `` `{'k': <v>}` ``.
    Dict(Vec<(GVal, GVal)>),
    /// A tuple — every `gdbus call` reply is one, e.g. `` `(<int64 5>,)` ``.
    Tuple(Vec<GVal>),
}

impl GVal {
    /// The string content, if this is a [`GVal::Str`].
    pub fn as_str(&self) -> Option<&str> {
        match self {
            GVal::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The integer content, if this is a [`GVal::Int`]. Strict: a double-typed
    /// number is not accepted here, see [`GVal::to_i64`].
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            GVal::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// The double content, if this is a [`GVal::Float`].
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            GVal::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// The boolean content, if this is a [`GVal::Bool`].
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            GVal::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The elements, if this is a [`GVal::Arr`].
    pub fn as_array(&self) -> Option<&[GVal]> {
        match self {
            GVal::Arr(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// The fields, if this is a [`GVal::Tuple`].
    pub fn as_tuple(&self) -> Option<&[GVal]> {
        match self {
            GVal::Tuple(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// The entries in printed order, if this is a [`GVal::Dict`].
    pub fn as_dict(&self) -> Option<&[(GVal, GVal)]> {
        match self {
            GVal::Dict(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// An integer, tolerating a double-typed number by truncation. Some players
    /// report `mpris:length` as a double rather than the spec's `int64`.
    pub fn to_i64(&self) -> Option<i64> {
        match self {
            GVal::Int(i) => Some(*i),
            GVal::Float(f) if f.is_finite() => Some(*f as i64),
            _ => None,
        }
    }

    /// Look up a string key in a [`GVal::Dict`]. First match wins.
    pub fn dict_get(&self, key: &str) -> Option<&GVal> {
        self.as_dict()?
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }

    /// Index into a [`GVal::Arr`] or [`GVal::Tuple`].
    pub fn at(&self, i: usize) -> Option<&GVal> {
        match self {
            GVal::Arr(v) | GVal::Tuple(v) => v.get(i),
            _ => None,
        }
    }

    /// Strings out of an array of strings. A bare string yields a one-element
    /// list, because some players type `xesam:artist` as `s` rather than the
    /// spec's `as`. Non-string elements are skipped.
    pub fn strings(&self) -> Vec<String> {
        match self {
            GVal::Str(s) => vec![s.clone()],
            GVal::Arr(v) => v
                .iter()
                .filter_map(|e| e.as_str())
                .map(String::from)
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Nesting limit. GVariant text is untrusted input from another process, and
/// the scanner is recursive; a pathological `[[[[...` must fail, not blow the
/// stack. No real MPRIS payload nests more than four deep.
const MAX_DEPTH: u32 = 64;

/// Correction applied to the *whole* input: parse one value, then require that
/// only whitespace follows. Returns `None` for anything malformed rather than
/// guessing — a half-parsed metadata dictionary is worse than none.
///
/// ```text
/// parse_gvariant("(<int64 42123456>,)\n")  ==  Tuple([Int(42123456)])
/// ```
pub fn parse_gvariant(s: &str) -> Option<GVal> {
    let mut sc = Scanner {
        b: s.as_bytes(),
        i: 0,
    };
    let v = sc.value(0)?;
    sc.skip_ws();
    if sc.i != sc.b.len() {
        return None;
    }
    Some(v)
}

/// Byte cursor over GVariant text. All structural characters are ASCII, so
/// byte-wise scanning is safe on UTF-8 input; multi-byte content is copied
/// through verbatim.
struct Scanner<'a> {
    b: &'a [u8],
    i: usize,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    /// One value of any kind. `depth` guards the recursion.
    fn value(&mut self, depth: u32) -> Option<GVal> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.skip_ws();
        match self.peek()? {
            // Variant wrapper: transparent, its inner value is the result.
            b'<' => {
                self.i += 1;
                let v = self.value(depth + 1)?;
                self.skip_ws();
                if self.peek()? != b'>' {
                    return None;
                }
                self.i += 1;
                Some(v)
            }
            b'[' => {
                self.i += 1;
                self.items(b']', depth).map(GVal::Arr)
            }
            b'(' => {
                self.i += 1;
                self.items(b')', depth).map(GVal::Tuple)
            }
            b'{' => self.dict(depth),
            b'\'' | b'"' => self.string().map(GVal::Str),
            // Explicit type annotation: `@as []`, `@a{sv} {}`, `@mv nothing`.
            b'@' => {
                self.i += 1;
                while let Some(c) = self.peek() {
                    if c.is_ascii_whitespace() {
                        break;
                    }
                    self.i += 1;
                }
                self.value(depth + 1)
            }
            c if c.is_ascii_alphabetic() || c == b'_' => self.word(depth),
            c if c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.' => self.number(),
            _ => None,
        }
    }

    /// Comma-separated values up to `close`. A trailing comma is allowed, which
    /// is how GLib prints one-element tuples: `` `(<int64 5>,)` ``.
    fn items(&mut self, close: u8, depth: u32) -> Option<Vec<GVal>> {
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            if self.peek()? == close {
                self.i += 1;
                return Some(out);
            }
            out.push(self.value(depth + 1)?);
            self.skip_ws();
            let c = self.peek()?;
            if c == b',' {
                self.i += 1;
            } else if c == close {
                self.i += 1;
                return Some(out);
            } else {
                return None;
            }
        }
    }

    /// `{key: value, ...}`. GLib prints dictionaries and standalone dict
    /// entries with a colon between key and value.
    fn dict(&mut self, depth: u32) -> Option<GVal> {
        self.i += 1; // '{'
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            if self.peek()? == b'}' {
                self.i += 1;
                return Some(GVal::Dict(out));
            }
            let k = self.value(depth + 1)?;
            self.skip_ws();
            if self.peek()? != b':' {
                return None;
            }
            self.i += 1;
            let v = self.value(depth + 1)?;
            out.push((k, v));
            self.skip_ws();
            let c = self.peek()?;
            if c == b',' {
                self.i += 1;
            } else if c == b'}' {
                self.i += 1;
                return Some(GVal::Dict(out));
            } else {
                return None;
            }
        }
    }

    /// A quoted string. The opening quote decides the closing quote, so a
    /// double-quoted string may contain bare apostrophes and vice versa —
    /// exactly what `g_variant_print` emits.
    fn string(&mut self) -> Option<String> {
        let quote = self.peek()?;
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek()?;
            self.i += 1;
            if c == quote {
                return String::from_utf8(out).ok();
            }
            if c != b'\\' {
                out.push(c);
                continue;
            }
            let e = self.peek()?;
            self.i += 1;
            match e {
                b'a' => out.push(0x07),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0c),
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'v' => out.push(0x0b),
                b'u' => push_char(&mut out, self.hex(4)?),
                b'U' => push_char(&mut out, self.hex(8)?),
                // `\\`, `\'`, `\"` and, leniently, anything else.
                other => out.push(other),
            }
        }
    }

    /// `n` hex digits as a code point.
    fn hex(&mut self, n: usize) -> Option<u32> {
        let end = self.i.checked_add(n)?;
        let s = std::str::from_utf8(self.b.get(self.i..end)?).ok()?;
        let v = u32::from_str_radix(s, 16).ok()?;
        self.i = end;
        Some(v)
    }

    /// A bare word: a literal (`true`), or a type prefix followed by its value
    /// (`int64 5`), or a bytestring introducer (`b'...'`).
    fn word(&mut self, depth: u32) -> Option<GVal> {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'_' {
                self.i += 1;
            } else {
                break;
            }
        }
        let w = std::str::from_utf8(self.b.get(start..self.i)?).ok()?;
        // Bytestring: `b'/some/bytes'`.
        if w == "b" && matches!(self.peek(), Some(b'\'') | Some(b'"')) {
            return self.string().map(GVal::Str);
        }
        match w {
            "true" => Some(GVal::Bool(true)),
            "false" => Some(GVal::Bool(false)),
            "nan" => Some(GVal::Float(f64::NAN)),
            "inf" => Some(GVal::Float(f64::INFINITY)),
            // Every type prefix GLib can print before a scalar. `just` is a
            // maybe-type wrapper: D-Bus has no maybe types so it cannot reach
            // us over the bus, but it costs nothing to accept.
            "byte" | "int16" | "uint16" | "int32" | "uint32" | "int64" | "uint64" | "handle"
            | "double" | "string" | "objectpath" | "signature" | "just" => self.value(depth + 1),
            _ => None,
        }
    }

    /// A numeric literal, integer or double, decimal or `0x` hex.
    fn number(&mut self) -> Option<GVal> {
        let start = self.i;
        if matches!(self.peek(), Some(b'-') | Some(b'+')) {
            self.i += 1;
        }
        // Alphanumerics are in the run so `0x41`, `1e-5` and `inf` scan as one
        // token; no delimiter (`,]}>):` or whitespace) can be swallowed.
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'.' || c == b'+' || c == b'-' {
                self.i += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(self.b.get(start..self.i)?).ok()?;
        if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            return i64::from_str_radix(h, 16).ok().map(GVal::Int);
        }
        if let Some(h) = s
            .strip_prefix("-0x")
            .or_else(|| s.strip_prefix("-0X"))
            .and_then(|h| i64::from_str_radix(h, 16).ok())
        {
            return Some(GVal::Int(-h));
        }
        if let Ok(v) = s.parse::<i64>() {
            return Some(GVal::Int(v));
        }
        // `uint64` above i64::MAX cannot be represented; saturate rather than
        // discard the value. No MPRIS field can legitimately reach here.
        if let Ok(v) = s.parse::<u64>() {
            return Some(GVal::Int(i64::try_from(v).unwrap_or(i64::MAX)));
        }
        s.parse::<f64>().ok().map(GVal::Float)
    }
}

/// Append a code point as UTF-8, substituting U+FFFD for a lone surrogate.
fn push_char(out: &mut Vec<u8>, cp: u32) {
    let ch = char::from_u32(cp).unwrap_or(char::REPLACEMENT_CHARACTER);
    let mut buf = [0u8; 4];
    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
}

/// Unwrap the one-element reply tuple `gdbus call` always prints, or return the
/// value unchanged if it is not a tuple.
fn reply_value(v: &GVal) -> &GVal {
    match v {
        GVal::Tuple(items) if items.len() == 1 => &items[0],
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// MPRIS `PlaybackStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackStatus {
    /// A track is advancing.
    Playing,
    /// A track is loaded and held at a position.
    Paused,
    /// Nothing is playing; MPRIS defines the position as 0.
    #[default]
    Stopped,
}

impl PlaybackStatus {
    /// Parse the MPRIS wire string. Case-insensitive and whitespace-tolerant;
    /// unknown values yield `None` rather than a wrong guess.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "playing" => Some(PlaybackStatus::Playing),
            "paused" => Some(PlaybackStatus::Paused),
            "stopped" => Some(PlaybackStatus::Stopped),
            _ => None,
        }
    }

    /// The MPRIS wire string.
    pub fn as_str(self) -> &'static str {
        match self {
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
            PlaybackStatus::Stopped => "Stopped",
        }
    }
}

/// One snapshot of a player's track and state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NowPlaying {
    /// The bus name this came from, e.g. `org.mpris.MediaPlayer2.vlc`.
    pub player: String,
    /// `xesam:title`, empty when the player published none.
    pub title: String,
    /// `xesam:artist`, in published order.
    pub artists: Vec<String>,
    /// `xesam:album`, empty when the player published none.
    pub album: String,
    /// `mpris:artUrl`. May be `http(s)://`, `file://` **or** `data:`, and may
    /// point into another app's sandbox that Fresco cannot read.
    pub art_url: Option<String>,
    /// `mpris:trackid`. The only sound track identity — titles repeat, and
    /// repeat-one would never retrigger if identity were keyed on the title.
    pub track_id: Option<String>,
    /// `mpris:length` in microseconds, when published and positive.
    pub length_us: Option<i64>,
    /// `PlaybackStatus`.
    pub status: PlaybackStatus,
}

impl NowPlaying {
    /// Artists joined for display, e.g. `"A, Band, Guest"`. Empty when unknown.
    pub fn artist_line(&self) -> String {
        self.artists.join(", ")
    }

    /// Whether this snapshot names a track.
    ///
    /// **The single definition of "usable"** — [`PlayerScan::of`] and the
    /// daemon's now-playing worker both defer to it, so selection and the
    /// per-poll check can never disagree about the same player. A title of only
    /// whitespace counts as none: it is exactly as useless for a lyrics lookup
    /// as an empty one, and players do pad.
    pub fn has_title(&self) -> bool {
        !self.title.trim().is_empty()
    }

    /// Whether `other` is the same track as `self`.
    ///
    /// Both tests must agree, and each covers what the other cannot:
    ///
    /// * **The metadata triple** — title, album, artists. The only signal that
    ///   survives a player whose `mpris:trackid` is a constant.
    /// * **`mpris:trackid`**, when *both* sides publish one. The only field that
    ///   moves on repeat-one, where the triple stays identical; keying on the
    ///   triple alone would never retrigger the lyrics there.
    ///
    /// Requiring both is the fix for an automatic track advance being invisible.
    /// Firefox publishes a literal `mpris:trackid` of
    /// `/org/mpris/MediaPlayer2/firefox` — the player's own object path, with no
    /// track component — and never varies it, so trusting the id alone made
    /// every song Firefox ever played "the same track" and the lyric overlay
    /// latched on the first one until the widget was toggled off and on. The
    /// same shape shows up in any player that hardcodes the field to satisfy the
    /// spec's "required" without having a track list to key it on.
    ///
    /// What this deliberately does **not** look at is `mpris:artUrl`, `Position`
    /// or `mpris:length`: those are what chatty players revise mid-song, and
    /// treating a late album art as a new track would drop the overlay through
    /// `Clear` several times a song.
    ///
    /// One case stays out of reach: repeat-one *on a constant-id player*. Both
    /// tests see identical input because the player published nothing that
    /// differs, and no amount of work on this side can invent the difference.
    pub fn same_track(&self, other: &NowPlaying) -> bool {
        let meta_same =
            self.title == other.title && self.album == other.album && self.artists == other.artists;
        let id_same = match (&self.track_id, &other.track_id) {
            (Some(a), Some(b)) => a == b,
            // At most one side names an id: it cannot testify either way, so
            // the triple decides alone.
            _ => true,
        };
        meta_same && id_same
    }
}

// ---------------------------------------------------------------------------
// Pure parsers for the reply shapes
// ---------------------------------------------------------------------------

/// MPRIS bus names out of a `ListNames` reply, in bus order, deduplicated.
///
/// Prefix match on [`MPRIS_PREFIX`]: an exact-name match would miss
/// `org.mpris.MediaPlayer2.firefox.instance_1_1234`, which is what browsers
/// actually register.
pub fn parse_list_names(out: &str) -> Vec<String> {
    let Some(v) = parse_gvariant(out) else {
        return Vec::new();
    };
    let mut names: Vec<String> = Vec::new();
    for n in reply_value(&v).strings() {
        // The prefix carries the trailing dot, so the bare interface name
        // `org.mpris.MediaPlayer2` (which no player owns) cannot match.
        if n.starts_with(MPRIS_PREFIX) && n.len() > MPRIS_PREFIX.len() && !names.contains(&n) {
            names.push(n);
        }
    }
    names
}

/// Build a [`NowPlaying`] from a `Properties.GetAll` reply on
/// [`PLAYER_IFACE`]. `None` when the reply is not a property dictionary;
/// individual missing properties degrade to empty/`None` rather than failing
/// the whole snapshot.
pub fn parse_get_all(player: &str, out: &str) -> Option<NowPlaying> {
    let v = parse_gvariant(out)?;
    let props = reply_value(&v);
    props.as_dict()?;
    let mut np = NowPlaying {
        player: player.to_string(),
        status: props
            .dict_get("PlaybackStatus")
            .and_then(GVal::as_str)
            .and_then(PlaybackStatus::parse)
            .unwrap_or_default(),
        ..Default::default()
    };
    if let Some(meta) = props.dict_get("Metadata") {
        apply_metadata(&mut np, meta);
    }
    Some(np)
}

/// Overwrite the track fields of `np` from an MPRIS `Metadata` dictionary.
///
/// Separate from [`parse_get_all`] because the event plane receives `Metadata`
/// on its own inside `PropertiesChanged` and must apply it the same way.
/// Non-dictionary input clears nothing and does nothing.
pub fn apply_metadata(np: &mut NowPlaying, meta: &GVal) {
    if meta.as_dict().is_none() {
        return;
    }
    np.title = meta
        .dict_get("xesam:title")
        .and_then(GVal::as_str)
        .unwrap_or_default()
        .to_string();
    np.album = meta
        .dict_get("xesam:album")
        .and_then(GVal::as_str)
        .unwrap_or_default()
        .to_string();
    // Some players publish only `xesam:albumArtist`.
    np.artists = meta
        .dict_get("xesam:artist")
        .map(GVal::strings)
        .filter(|v| !v.is_empty())
        .or_else(|| meta.dict_get("xesam:albumArtist").map(GVal::strings))
        .unwrap_or_default();
    np.art_url = meta
        .dict_get("mpris:artUrl")
        .and_then(GVal::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from);
    // The "no track" path is published between tracks and is not an identity.
    np.track_id = meta
        .dict_get("mpris:trackid")
        .and_then(GVal::as_str)
        .filter(|s| !s.is_empty() && *s != NO_TRACK)
        .map(String::from);
    // A zero or negative length means "unknown", not "zero-length track".
    np.length_us = meta
        .dict_get("mpris:length")
        .and_then(GVal::to_i64)
        .filter(|n| *n > 0);
}

/// Position in microseconds out of a `Properties.Get` reply.
///
/// The hot path: called once a second while playing, so it first tries a cheap
/// digit scan and only falls back to [`parse_gvariant`] for the rare
/// double-typed reply.
pub fn parse_position(out: &str) -> Option<i64> {
    if let Some(n) = scan_first_integer(out) {
        return Some(n);
    }
    parse_gvariant(out).and_then(|v| reply_value(&v).to_i64())
}

/// The first integer literal in `s` that is not part of a longer token.
///
/// The trap this exists to avoid: the type prefix in `` `(<int64 42123456>,)` ``
/// itself contains the digits `64`. A run only counts when the character before
/// it is not alphanumeric, and a run followed by `.` or more letters (a double,
/// a hex literal) is skipped entirely rather than half-read.
fn scan_first_integer(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if !((c.is_ascii_digit() || c == b'-') && (i == 0 || !b[i - 1].is_ascii_alphanumeric())) {
            i += 1;
            continue;
        }
        let start = i;
        if c == b'-' {
            i += 1;
        }
        let digits_at = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == digits_at {
            continue; // a lone '-', already stepped over
        }
        if i < b.len() && (b[i] == b'.' || b[i].is_ascii_alphanumeric()) {
            // Part of a double or a hex literal — skip the whole token.
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.') {
                i += 1;
            }
            continue;
        }
        return std::str::from_utf8(&b[start..i]).ok()?.parse().ok();
    }
    None
}

/// [`PlaybackStatus`] out of a `Properties.Get` reply, e.g. `` `(<'Playing'>,)` ``.
pub fn parse_status(out: &str) -> Option<PlaybackStatus> {
    let v = parse_gvariant(out)?;
    reply_value(&v).as_str().and_then(PlaybackStatus::parse)
}

// ---------------------------------------------------------------------------
// Queries (blocking — never call these from the daemon loop)
// ---------------------------------------------------------------------------

/// True when the `gdbus` binary this module drives is on `PATH`.
///
/// Exists so a caller can tell "no player is running" (the overwhelmingly
/// common reason [`list_players`] is empty, and not worth a word) from "the
/// tool we ask with is not installed" (a packaging problem the user can fix in
/// five seconds, but only if somebody names it). `gdbus_call` itself must
/// stay quiet — it fails constantly and by design as players come and go — so
/// the check belongs at the point a feature turns on, not in the hot path.
pub fn gdbus_available() -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join("gdbus").is_file()))
        .unwrap_or(false)
}

/// Run `gdbus call --session` with a short timeout; stdout on success.
///
/// `--timeout` is the guard against a wedged player: without it `gdbus` waits
/// the D-Bus default of 25s. Blocking is still blocking, so every caller runs
/// on a worker thread.
fn gdbus_call(dest: &str, path: &str, iface_method: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("gdbus");
    cmd.args(["call", "--session", "--timeout", CALL_TIMEOUT_SECS])
        .args(["--dest", dest, "--object-path", path])
        .args(["--method", iface_method])
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    let out = cmd.output().ok()?;
    if !out.status.success() {
        // Expected constantly: players come and go mid-poll. Never warn.
        log::debug!("mpris: {iface_method} on {dest} failed");
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every MPRIS player currently on the session bus, in bus order.
///
/// Empty when `gdbus` is missing, no session bus exists, or nothing is running
/// — all of which are normal, none of which are errors.
pub fn list_players() -> Vec<String> {
    let Some(out) = gdbus_call(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus.ListNames",
        &[],
    ) else {
        return Vec::new();
    };
    parse_list_names(&out)
}

/// One `Properties.GetAll` round trip: status and full metadata together.
///
/// Call on track change, not per tick — the hot path is
/// [`get_position_us`].
pub fn get_all(player: &str) -> Option<NowPlaying> {
    let out = gdbus_call(
        player,
        MPRIS_PATH,
        "org.freedesktop.DBus.Properties.GetAll",
        &[PLAYER_IFACE],
    )?;
    parse_get_all(player, &out)
}

/// The player's `Position` in microseconds. The hot path — one property, one
/// integer, no metadata.
pub fn get_position_us(player: &str) -> Option<i64> {
    let out = gdbus_call(
        player,
        MPRIS_PATH,
        "org.freedesktop.DBus.Properties.Get",
        &[PLAYER_IFACE, "Position"],
    )?;
    parse_position(&out)
}

/// The player's `PlaybackStatus`, without fetching metadata.
pub fn get_status(player: &str) -> Option<PlaybackStatus> {
    let out = gdbus_call(
        player,
        MPRIS_PATH,
        "org.freedesktop.DBus.Properties.Get",
        &[PLAYER_IFACE, "PlaybackStatus"],
    )?;
    parse_status(&out)
}

/// `PlaybackStatus` for each of `players`, skipping any that failed to answer.
/// Feeds [`pick_player`]. One round trip per player, so call it when the set of
/// players changes, not on every tick.
///
/// Prefer [`scan_players`] when the answer is going to be fed to a selection:
/// it costs the same one round trip per player and additionally answers whether
/// the player can drive lyrics at all.
pub fn statuses(players: &[String]) -> Vec<(String, PlaybackStatus)> {
    players
        .iter()
        .filter_map(|p| get_status(p).map(|s| (p.clone(), s)))
        .collect()
}

/// One `Properties.GetAll` per player — status *and* metadata in the same round
/// trip, so learning whether a player is **usable** costs nothing over learning
/// its status. Players that failed to answer are skipped, as in [`statuses`].
pub fn scan_players(players: &[String]) -> Vec<PlayerScan> {
    players
        .iter()
        .filter_map(|p| get_all(p).map(|np| PlayerScan::of(&np)))
        .collect()
}

// ---------------------------------------------------------------------------
// Player selection (pure)
// ---------------------------------------------------------------------------

/// One player as a bus scan saw it: what it is doing **and** whether it carries
/// anything the lyric pipeline can work with.
///
/// `has_title` is the fact a `PlaybackStatus` ladder cannot supply, and without
/// it the ladder confidently picks players that can never produce a lyric.
/// The case this type exists for, captured verbatim from a live session:
///
/// ```text
/// org.mpris.MediaPlayer2.brave.instance6389
///   PlaybackStatus: (<'Stopped'>,)
///   Metadata:       (<{'mpris:artUrl': <'file:///tmp/.org.chromium.Chromium.1J5tKq'>,
///                      'mpris:length': <int64 0>}>,)
///   Position:       (<int64 0>,)
/// ```
///
/// Chromium-family browsers (Chrome, Brave, Edge, Vivaldi, Opera) claim
/// `org.mpris.MediaPlayer2.<brand>.instance<PID>` — one name per **browser
/// process**, aggregating every tab, suffixed with the main process id — the
/// first time any page plays media, and then **never release it**: there is no
/// `ReleaseOwnership` anywhere in `SystemMediaControlsLinux`, so the name sits
/// on the bus until the browser exits (crbug 40703847, open for years).
///
/// When the media session ends, Chromium's `ClearMetadata()` resets every
/// `xesam:` field to "unset" and re-publishes — and its `Metadata` dictionary
/// omits any field that is unset, so the keys vanish rather than going empty.
/// The artwork survives because the two are on **separate debounce timers**:
/// clearing stops the metadata timer but not the icon one, and the temp-file
/// PNG write completes afterwards and repopulates `mpris:artUrl` on its own.
/// (That `file:///tmp/.org.chromium.Chromium.XXXXXX` is a PNG Chromium
/// rasterised itself, usually the browser's own product logo when the page
/// supplied no artwork — not the page's cover art, and frequently already
/// deleted by the time anyone reads it.)
///
/// The consequence that makes this a **sound** test rather than a heuristic:
/// while a session is genuinely live, Chromium falls `xesam:title` back to the
/// tab title, so it is essentially never absent for real playback. A missing
/// title therefore means "no live media session", not "a live session we could
/// not read". Such a player must lose to any player that does publish a title,
/// at every rung of the ladder. See [`pick_usable_player`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerScan {
    /// The bus name, e.g. `org.mpris.MediaPlayer2.brave.instance6389`.
    pub name: String,
    /// `PlaybackStatus`, defaulting to [`PlaybackStatus::Stopped`] when the
    /// player published none.
    pub status: PlaybackStatus,
    /// The player published a non-empty `xesam:title`. **The usability test**:
    /// false means this player cannot drive lyrics no matter what it is doing.
    pub has_title: bool,
}

impl PlayerScan {
    /// Summarise a polled snapshot. Usability is [`NowPlaying::has_title`] and
    /// nothing else, so this can never disagree with a per-poll check.
    pub fn of(np: &NowPlaying) -> Self {
        PlayerScan {
            name: np.player.clone(),
            status: np.status,
            has_title: np.has_title(),
        }
    }

    /// Whether this player can drive the lyric widget. Currently exactly
    /// [`PlayerScan::has_title`], named for the question rather than the field
    /// so the ladder reads as intent.
    pub fn is_usable(&self) -> bool {
        self.has_title
    }
}

/// Pick the player the overlay should follow, ignoring players that cannot
/// drive it.
///
/// This is [`pick_player_sticky`]'s ladder run over the **usable** players
/// only — those with a title. Usability is not a rung, it is the filter the
/// whole ladder runs inside, which is what makes a title-less player lose at
/// *every* rung rather than only at the bottom one:
///
/// * it cannot win rung 2 by being the only thing `Playing`;
/// * it cannot win rung 4 by being the only thing `Paused`;
/// * and it cannot be **stickily retained** (rungs 1, 3, 5) over a player that
///   does have a title — the incumbent only counts as an incumbent while it is
///   still usable itself.
///
/// `None` when no player on the bus is usable, including when the bus is empty.
/// That is a real answer, not a failure: with nothing to look up, following a
/// player and sitting there doing nothing is strictly worse than following
/// nobody and rescanning. The caller is expected to keep scanning on its idle
/// cadence — see the worker in `daemon::widgets`.
pub fn pick_usable_player(scans: &[PlayerScan], current: Option<&str>) -> Option<String> {
    let usable: Vec<String> = scans
        .iter()
        .filter(|s| s.is_usable())
        .map(|s| s.name.clone())
        .collect();
    if usable.is_empty() {
        return None;
    }
    let statuses: Vec<(String, PlaybackStatus)> = scans
        .iter()
        .filter(|s| s.is_usable())
        .map(|s| (s.name.clone(), s.status))
        .collect();
    // An incumbent that has gone title-less has nothing left to be sticky
    // about; dropping it here is what lets a usable player take the overlay.
    let incumbent = current.filter(|c| usable.iter().any(|n| n == c));
    pick_player_sticky(&usable, &statuses, incumbent)
}

/// Pick the player the overlay should follow, with no incumbent.
///
/// See [`pick_player_sticky`] for the ladder; this is that function with
/// `current: None`.
pub fn pick_player(players: &[String], statuses: &[(String, PlaybackStatus)]) -> Option<String> {
    pick_player_sticky(players, statuses, None)
}

/// Pick the player the overlay should follow, preferring the incumbent.
///
/// `players` is in caller-preferred order; the caller should keep it
/// most-recently-active first (the order `NameOwnerChanged` and
/// `PropertiesChanged` arrive in), because that order *is* the "most recently
/// active" tier. `statuses` need not cover every player; an unknown status is
/// treated as [`PlaybackStatus::Stopped`].
///
/// The ladder, highest first:
///
/// 1. **Sticky play** — `current` is still present and still `Playing`. This
///    rung is the whole point of stickiness: without it, a background browser
///    tab that starts playing would steal the overlay mid-song.
/// 2. **Any `Playing`** — first in `players` order. An actively playing player
///    beats our paused incumbent; that is a real user intent.
/// 3. **Sticky pause** — `current` is still present and `Paused`.
/// 4. **A single `Paused`** — unambiguous, so follow it.
/// 5. **Sticky anything** — `current` is still present. Never drop a player
///    that merely stopped between tracks.
/// 6. **Most recently active** — the head of `players`.
///
/// `None` only when `players` is empty.
///
/// **This ladder ranks on `PlaybackStatus` alone.** It will happily hand back a
/// player that publishes no track metadata, because it is never told about any.
/// Callers driving the lyric widget want [`pick_usable_player`], which runs this
/// same ladder over players that can actually produce a lookup.
pub fn pick_player_sticky(
    players: &[String],
    statuses: &[(String, PlaybackStatus)],
    current: Option<&str>,
) -> Option<String> {
    if players.is_empty() {
        return None;
    }
    let status_of = |name: &str| -> PlaybackStatus {
        statuses
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| *s)
            .unwrap_or_default()
    };
    // The incumbent only counts while it is still on the bus.
    let incumbent = current.filter(|c| players.iter().any(|p| p == c));

    // 1. Sticky play.
    if let Some(c) = incumbent {
        if status_of(c) == PlaybackStatus::Playing {
            return Some(c.to_string());
        }
    }
    // 2. Any playing.
    if let Some(p) = players
        .iter()
        .find(|p| status_of(p) == PlaybackStatus::Playing)
    {
        return Some(p.clone());
    }
    // 3. Sticky pause.
    if let Some(c) = incumbent {
        if status_of(c) == PlaybackStatus::Paused {
            return Some(c.to_string());
        }
    }
    // 4. Exactly one paused.
    let mut paused = players
        .iter()
        .filter(|p| status_of(p) == PlaybackStatus::Paused);
    if let (Some(p), None) = (paused.next(), paused.next()) {
        return Some(p.clone());
    }
    // 5. Sticky anything, then 6. most recently active.
    incumbent
        .map(String::from)
        .or_else(|| players.first().cloned())
}

// ---------------------------------------------------------------------------
// Position clock (pure)
// ---------------------------------------------------------------------------

/// Resync error at or beyond which the clock hard-snaps instead of slewing.
///
/// Below this, snapping every second is *worse* than being wrong: a lyric line
/// that jumps 80ms once a second reads as jitter, while an 80ms offset is
/// imperceptible.
pub const SNAP_THRESHOLD_US: i64 = 300_000;

/// Window over which a slewed error is absorbed. Matched to
/// [`RESYNC_INTERVAL`] so a small error is gone by the next poll.
pub const SLEW_WINDOW: Duration = Duration::from_secs(1);

/// How often to poll `Position` **while playing**. Never poll while paused.
pub const RESYNC_INTERVAL: Duration = Duration::from_secs(1);

/// The local tick that advances the prediction between resyncs. Matches the
/// daemon loops' existing 100ms cadence — no new thread, no new timer.
pub const TICK: Duration = Duration::from_millis(100);

/// Largest share of real time a slew may add or remove, so the correction can
/// never stall or reverse the clock (lyrics must never scroll backwards).
const MAX_SLEW_FRACTION: f64 = 0.5;

/// What [`PositionClock::resync`] did, for logging and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resync {
    /// Error absorbed smoothly over [`SLEW_WINDOW`]; the predicted position did
    /// not jump.
    Slewed {
        /// polled − predicted, in microseconds.
        error_us: i64,
    },
    /// Error was at or beyond [`SNAP_THRESHOLD_US`] (or playback is not
    /// running), so the anchor was set to the polled value outright.
    Snapped {
        /// polled − predicted, in microseconds.
        error_us: i64,
    },
}

/// A monotonic prediction of the current playback position.
///
/// The design (WIDGETS_ROADMAP W1): an anchor of `(position, Instant, rate)`
/// advanced locally, corrected by a 1s `Position` poll while playing and by
/// `Seeked` signals, frozen while paused.
///
/// [`Instant`] and not `SystemTime`: the clock must be immune to NTP steps,
/// manual clock changes and suspend/resume, all of which would otherwise
/// teleport the lyrics.
///
/// **No I/O and no hidden clock.** `now` is a parameter on every method, so the
/// whole algorithm is unit-testable without sleeping.
#[derive(Debug, Clone)]
pub struct PositionClock {
    anchor_us: i64,
    anchor_at: Instant,
    rate: f64,
    status: PlaybackStatus,
    /// Correction still being spread across [`SLEW_WINDOW`] from `anchor_at`.
    slew_us: i64,
}

impl PositionClock {
    /// A stopped clock at position 0, rate 1.0.
    pub fn new(now: Instant) -> Self {
        Self {
            anchor_us: 0,
            anchor_at: now,
            rate: 1.0,
            status: PlaybackStatus::Stopped,
            slew_us: 0,
        }
    }

    /// The predicted position at `now`, in microseconds, never negative.
    ///
    /// Frozen while not `Playing` — a paused player's position does not move,
    /// and pretending otherwise is what makes lyrics run away after a pause.
    pub fn predicted_us(&self, now: Instant) -> i64 {
        if self.status != PlaybackStatus::Playing {
            return self.anchor_us.max(0);
        }
        let dt_us = now.saturating_duration_since(self.anchor_at).as_micros() as f64;
        let mut pos = self.anchor_us as f64 + dt_us * self.rate;
        if self.slew_us != 0 {
            let window = SLEW_WINDOW.as_micros() as f64;
            let progress = if window > 0.0 {
                (dt_us / window).clamp(0.0, 1.0)
            } else {
                1.0
            };
            pos += self.slew_us as f64 * progress;
        }
        pos.max(0.0) as i64
    }

    /// Fold everything accrued so far into the anchor without changing the
    /// predicted value.
    fn reanchor(&mut self, now: Instant) {
        self.anchor_us = self.predicted_us(now);
        self.anchor_at = now;
        self.slew_us = 0;
    }

    /// Correct the prediction with a freshly polled `Position`.
    ///
    /// Under [`SNAP_THRESHOLD_US`] the error is **slewed**: the prediction is
    /// pinned where it already is and the clock runs slightly fast or slow for
    /// [`SLEW_WINDOW`], so nothing on screen jumps. At or beyond the threshold
    /// the player really is somewhere else (a seek we missed, a stall) and the
    /// clock snaps.
    ///
    /// While not `Playing` the clock is frozen, so a resync simply re-anchors
    /// exactly and reports [`Resync::Snapped`].
    pub fn resync(&mut self, polled_us: i64, now: Instant) -> Resync {
        let predicted = self.predicted_us(now);
        let error_us = polled_us.saturating_sub(predicted);
        if self.status != PlaybackStatus::Playing || error_us.saturating_abs() >= SNAP_THRESHOLD_US
        {
            self.anchor_us = polled_us.max(0);
            self.anchor_at = now;
            self.slew_us = 0;
            return Resync::Snapped { error_us };
        }
        self.anchor_us = predicted;
        self.anchor_at = now;
        // Cap so the effective rate stays positive: at rate 0.25 a −300ms
        // correction over 1s would otherwise run the clock backwards.
        let cap = (SLEW_WINDOW.as_micros() as f64 * self.rate.abs() * MAX_SLEW_FRACTION) as i64;
        self.slew_us = error_us.clamp(-cap, cap);
        Resync::Slewed { error_us }
    }

    /// Apply a `Seeked` signal: the player told us exactly where it is, so
    /// there is nothing to slew.
    pub fn seeked(&mut self, pos_us: i64, now: Instant) {
        self.anchor_us = pos_us.max(0);
        self.anchor_at = now;
        self.slew_us = 0;
    }

    /// Change playback status.
    ///
    /// Pausing freezes at the currently predicted position, and resuming
    /// continues from there, so a pause/resume round trip loses nothing.
    /// `Stopped` resets the position to 0, as MPRIS defines.
    pub fn set_status(&mut self, status: PlaybackStatus, now: Instant) {
        if status == self.status {
            return;
        }
        self.reanchor(now);
        self.status = status;
        if status == PlaybackStatus::Stopped {
            self.anchor_us = 0;
        }
    }

    /// Change the playback rate, keeping the current predicted position.
    /// Non-finite and negative rates are ignored — MPRIS allows negative rates
    /// in principle, no player implements them, and honouring one would run the
    /// lyrics backwards.
    pub fn set_rate(&mut self, rate: f64, now: Instant) {
        if !rate.is_finite() || rate < 0.0 || (rate - self.rate).abs() <= f64::EPSILON {
            return;
        }
        self.reanchor(now);
        self.rate = rate;
    }

    /// A new track started: position returns to 0. Status and rate are
    /// unchanged, because a track change does not pause anything.
    pub fn track_changed(&mut self, now: Instant) {
        self.anchor_us = 0;
        self.anchor_at = now;
        self.slew_us = 0;
    }

    /// The current status.
    pub fn status(&self) -> PlaybackStatus {
        self.status
    }

    /// The current rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Whether the position is actually advancing. The caller's gate for
    /// scheduling a wake, polling `Position`, or animating anything: when this
    /// is false there is nothing to do until an event arrives.
    pub fn is_running(&self) -> bool {
        self.status == PlaybackStatus::Playing && self.rate > 0.0
    }
}

// ---------------------------------------------------------------------------
// Degraded-player detection (pure)
// ---------------------------------------------------------------------------

/// Minimum spacing between the zero polls that count towards a verdict. Wide
/// enough that a healthy player must have moved between them.
pub const DEGRADED_MIN_GAP: Duration = Duration::from_secs(3);

/// Consecutive spaced-out zero polls needed for a verdict.
pub const DEGRADED_ZERO_POLLS: u32 = 3;

/// Detects players whose `Position` is useless.
///
/// Spotify's native Linux client returns `Position: 0` forever and never emits
/// `Seeked` — reported since 2018, still unfixed, identical across native,
/// Flatpak and snap. QQ Music and some Electron clients fail the same way.
/// (Spotify *in a browser* reports correct positions, which is one more reason
/// a bus-name blocklist would be wrong.)
///
/// The detection is **behavioural**, never a name list: a blocklist would miss
/// every player not on it and would rot as players are fixed or broken.
///
/// Once [`PositionReliability::is_unreliable`] is true, the caller stops
/// calling [`PositionClock::resync`] and lets the clock free-run from the last
/// track change. That is wrong by however much the track was already into
/// playback when we attached, and right for everything after — much better than
/// pinning every lyric to 0:00.
#[derive(Debug, Clone, Default)]
pub struct PositionReliability {
    zeros: u32,
    last_counted: Option<Instant>,
    unreliable: bool,
    logged: bool,
}

impl PositionReliability {
    /// A detector with no observations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one polled position. Returns the current verdict.
    ///
    /// Only polls taken while `Playing` count, and only when at least
    /// [`DEGRADED_MIN_GAP`] after the last counted one — so the legitimate
    /// zeros at the very start of a track can never accumulate a verdict
    /// (reaching it needs more than 6s of playback still reading 0).
    ///
    /// Any non-zero position while playing clears the verdict: the player is
    /// demonstrably working, whatever it did before.
    pub fn observe(&mut self, polled_us: i64, status: PlaybackStatus, now: Instant) -> bool {
        if status != PlaybackStatus::Playing {
            // A paused player sitting at 0 says nothing about its reporting.
            return self.unreliable;
        }
        if polled_us != 0 {
            self.zeros = 0;
            self.last_counted = None;
            self.unreliable = false;
            return false;
        }
        let counts = match self.last_counted {
            None => true,
            Some(prev) => now.saturating_duration_since(prev) >= DEGRADED_MIN_GAP,
        };
        if counts {
            self.zeros = self.zeros.saturating_add(1);
            self.last_counted = Some(now);
        }
        if self.zeros >= DEGRADED_ZERO_POLLS && !self.unreliable {
            self.unreliable = true;
            if !self.logged {
                self.logged = true;
                log::warn!(
                    "mpris: player reports Position 0 while playing — treating its \
                     position as unreliable and free-running the lyric clock \
                     (known in Spotify's native Linux client and some Electron players)"
                );
            }
        }
        self.unreliable
    }

    /// The current verdict.
    pub fn is_unreliable(&self) -> bool {
        self.unreliable
    }

    /// A new track started on the **same** player: drop the streak (a fresh
    /// track really is at 0 for a moment) but keep the verdict — a player that
    /// cannot report a position cannot report it for the next track either.
    pub fn track_changed(&mut self) {
        self.zeros = 0;
        self.last_counted = None;
    }

    /// The selected player changed: forget everything, including the verdict
    /// and the one-shot log, since it described a different program.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verbatim `gdbus call ... Properties.GetAll org.mpris.MediaPlayer2.Player`
    /// output captured from a live session. Every quirk this module exists for
    /// is in this one line.
    const GET_ALL: &str = "({'PlaybackStatus': <'Playing'>, 'Metadata': <{'mpris:trackid': <objectpath '/com/fake/track/1'>, 'mpris:length': <int64 245000000>, 'mpris:artUrl': <'https://i.example.com/ab'>, 'xesam:title': <\"Don't Stop 'Til You (Remix)\">, 'xesam:artist': <['A, Band', 'Guest']>, 'xesam:album': <'Album \"Quoted\"'>}>, 'Position': <int64 42123456>, 'CanSeek': <true>, 'Rate': <1.0>},)";

    /// Verbatim `Properties.Get ... Position`.
    const POSITION: &str = "(<int64 42123456>,)";

    /// Verbatim `Properties.Get ... PlaybackStatus`.
    const STATUS: &str = "(<'Playing'>,)";

    // -- GVariant scanner ---------------------------------------------------

    /// The whole captured reply must parse, structure and all. If this breaks,
    /// nothing downstream is trustworthy.
    #[test]
    fn parses_the_captured_get_all_reply() {
        let v = parse_gvariant(GET_ALL).expect("captured reply must parse");
        let props = reply_value(&v);
        assert_eq!(props.as_dict().map(<[_]>::len), Some(5));
        assert_eq!(
            props.dict_get("PlaybackStatus").and_then(GVal::as_str),
            Some("Playing")
        );
        assert_eq!(
            props.dict_get("Position").and_then(GVal::as_i64),
            Some(42_123_456)
        );
        assert_eq!(
            props.dict_get("CanSeek").and_then(GVal::as_bool),
            Some(true)
        );
        assert_eq!(props.dict_get("Rate").and_then(GVal::as_f64), Some(1.0));
        assert!(props.dict_get("Metadata").unwrap().as_dict().is_some());
        // Unknown keys are absent, not defaulted.
        assert!(props.dict_get("Nope").is_none());
    }

    /// GLib picks the quote character from the content and only escapes the
    /// quote it actually used. A parser that assumes `'` (or that unescapes
    /// eagerly) mangles both of these real titles.
    #[test]
    fn quote_style_switches_with_content() {
        // Contains an apostrophe ⇒ printed double-quoted, apostrophes bare.
        assert_eq!(
            parse_gvariant("<\"Don't Stop 'Til You (Remix)\">"),
            Some(GVal::Str("Don't Stop 'Til You (Remix)".into()))
        );
        // No apostrophe ⇒ printed single-quoted, double quotes bare.
        assert_eq!(
            parse_gvariant("<'Album \"Quoted\"'>"),
            Some(GVal::Str("Album \"Quoted\"".into()))
        );
        // Both kinds present ⇒ double-quoted with the double quotes escaped.
        assert_eq!(
            parse_gvariant(r#"<"it's a \"test\"">"#),
            Some(GVal::Str("it's a \"test\"".into()))
        );
    }

    /// The single most damaging shortcut available here: splitting an artist
    /// array on commas. "A, Band" is one artist.
    #[test]
    fn array_elements_may_contain_commas() {
        let v = parse_gvariant("<['A, Band', 'Guest']>").unwrap();
        assert_eq!(v.strings(), vec!["A, Band".to_string(), "Guest".into()]);
        assert_eq!(v.as_array().map(<[_]>::len), Some(2));
        // Brackets and colons inside strings must not be structural either.
        let v = parse_gvariant("<['a]b', 'c}d', 'e:f', 'g,h']>").unwrap();
        assert_eq!(v.strings(), vec!["a]b", "c}d", "e:f", "g,h"]);
    }

    /// `g_variant_print` escapes the active quote, the backslash, the C escapes
    /// and non-printables as `\uXXXX` / `\UXXXXXXXX`. All must round-trip.
    #[test]
    fn string_escape_sequences() {
        assert_eq!(
            parse_gvariant(r"<'a\\b'>"),
            Some(GVal::Str("a\\b".into())),
            "escaped backslash"
        );
        assert_eq!(
            parse_gvariant(r"<'it\'s'>"),
            Some(GVal::Str("it's".into())),
            "escaped active quote"
        );
        assert_eq!(
            parse_gvariant(r"<'a\nb\tc\rd'>"),
            Some(GVal::Str("a\nb\tc\rd".into()))
        );
        assert_eq!(parse_gvariant(r"<'é'>"), Some(GVal::Str("é".into())));
        assert_eq!(
            parse_gvariant(r"<'\U0001f600'>"),
            Some(GVal::Str("\u{1f600}".into()))
        );
        // A lone surrogate is not a char; substitute rather than fail.
        assert_eq!(
            parse_gvariant(r"<'\ud800'>"),
            Some(GVal::Str("\u{fffd}".into()))
        );
        // Multi-byte content passes through untouched.
        assert_eq!(
            parse_gvariant("<'日本語 — ok'>"),
            Some(GVal::Str("日本語 — ok".into()))
        );
    }

    /// Every scalar spelling GLib can emit. `<254>` and `<-1>` are captured
    /// from a live `GetAll`; the rest are the documented prefixes.
    #[test]
    fn scalar_type_prefixes() {
        let cases: [(&str, GVal); 14] = [
            ("<int64 245000000>", GVal::Int(245_000_000)),
            ("<int32 -5>", GVal::Int(-5)),
            ("<uint32 7>", GVal::Int(7)),
            ("<int16 -3>", GVal::Int(-3)),
            ("<uint16 3>", GVal::Int(3)),
            ("<uint64 12>", GVal::Int(12)),
            ("<handle 4>", GVal::Int(4)),
            ("<byte 0x41>", GVal::Int(0x41)),
            ("<254>", GVal::Int(254)),          // bare int32, captured live
            ("<-1>", GVal::Int(-1)),            // captured live
            ("<double 2.5>", GVal::Float(2.5)), // explicit prefix
            ("<1.0>", GVal::Float(1.0)),        // bare double
            (
                "<objectpath '/com/fake/track/1'>",
                GVal::Str("/com/fake/track/1".into()),
            ),
            ("<signature 'a{sv}'>", GVal::Str("a{sv}".into())),
        ];
        for (input, want) in cases {
            assert_eq!(parse_gvariant(input).as_ref(), Some(&want), "input {input}");
        }
        assert_eq!(parse_gvariant("<true>"), Some(GVal::Bool(true)));
        assert_eq!(parse_gvariant("<false>"), Some(GVal::Bool(false)));
        assert_eq!(parse_gvariant("<string 'x'>"), Some(GVal::Str("x".into())));
        assert_eq!(
            parse_gvariant("<b'bytes'>"),
            Some(GVal::Str("bytes".into()))
        );
        assert_eq!(parse_gvariant("<1e-3>"), Some(GVal::Float(1e-3)));
        assert_eq!(
            parse_gvariant("<-inf>"),
            Some(GVal::Float(f64::NEG_INFINITY))
        );
        assert!(parse_gvariant("<nan>").unwrap().as_f64().unwrap().is_nan());
    }

    /// Empty containers are printed with an explicit type annotation — this is
    /// `<@as []>`, captured from a live `org.freedesktop.DBus` `GetAll`. The
    /// `@`-prefix path had no other coverage.
    #[test]
    fn empty_and_annotated_containers() {
        assert_eq!(parse_gvariant("<@as []>"), Some(GVal::Arr(vec![])));
        assert_eq!(parse_gvariant("<@a{sv} {}>"), Some(GVal::Dict(vec![])));
        assert_eq!(parse_gvariant("()"), Some(GVal::Tuple(vec![])));
        assert_eq!(parse_gvariant("[]"), Some(GVal::Arr(vec![])));
        assert_eq!(parse_gvariant("{}"), Some(GVal::Dict(vec![])));
        // The trailing comma GLib prints for one-element tuples.
        assert_eq!(
            parse_gvariant(POSITION),
            Some(GVal::Tuple(vec![GVal::Int(42_123_456)]))
        );
        assert_eq!(
            parse_gvariant("<@as ['x']>"),
            Some(GVal::Arr(vec![GVal::Str("x".into())]))
        );
        // A multi-field tuple, as `g_variant_print` emits it: the type prefix
        // appears per field, not once for the tuple.
        assert_eq!(
            parse_gvariant("(int64 5, true)"),
            Some(GVal::Tuple(vec![GVal::Int(5), GVal::Bool(true)]))
        );
    }

    /// Metadata is a dict inside a variant inside a dict inside a tuple, and
    /// nested arrays of dicts show up in `xesam:` extensions.
    #[test]
    fn nested_containers() {
        let v = parse_gvariant("({'a': <{'b': <[{'c': <int64 1>}]>}>},)").unwrap();
        let inner = reply_value(&v)
            .dict_get("a")
            .and_then(|x| x.dict_get("b"))
            .and_then(|x| x.at(0))
            .and_then(|x| x.dict_get("c"))
            .and_then(GVal::as_i64);
        assert_eq!(inner, Some(1));
    }

    /// Malformed input must yield `None`, never a partial value: half a
    /// metadata dictionary would show a wrong title with total confidence.
    #[test]
    fn malformed_input_is_none() {
        let bad = [
            "",
            "   ",
            "(",
            ")",
            "(<int64 5>",            // unterminated tuple
            "<int64 5",              // unterminated variant
            "'unterminated",         // unterminated string
            "\"mismatched'",         // wrong closing quote
            "{'k' 5}",               // dict entry without a colon
            "{'k': }",               // dict entry without a value
            "['a' 'b']",             // missing separator
            "(<int64 5>,) trailing", // junk after the value
            "garbage",               // unknown bare word
            "int64",                 // prefix with nothing after it
            "[1, ]extra",
            r"'\u00'", // truncated escape
        ];
        for input in bad {
            assert_eq!(parse_gvariant(input), None, "must reject {input:?}");
        }
    }

    /// The scanner recurses, and the input comes from another process. Deep
    /// nesting must be rejected rather than overflow the stack.
    #[test]
    fn absurd_nesting_is_rejected() {
        let deep = format!("{}{}", "[".repeat(500), "]".repeat(500));
        assert_eq!(parse_gvariant(&deep), None);
        // Just inside the limit still parses.
        let ok = format!("{}{}", "[".repeat(20), "]".repeat(20));
        assert!(parse_gvariant(&ok).is_some());
    }

    /// A `uint64` above `i64::MAX` cannot be represented; saturating beats
    /// dropping the whole reply.
    #[test]
    fn huge_unsigned_saturates() {
        assert_eq!(
            parse_gvariant("<uint64 18446744073709551615>"),
            Some(GVal::Int(i64::MAX))
        );
    }

    /// Accessors are strict about type except `to_i64`, which players force us
    /// to be lenient about.
    #[test]
    fn accessors_are_type_strict() {
        let v = parse_gvariant(GET_ALL).unwrap();
        let props = reply_value(&v);
        assert!(props.dict_get("Rate").unwrap().as_i64().is_none());
        assert_eq!(props.dict_get("Rate").unwrap().to_i64(), Some(1));
        assert!(props.dict_get("Position").unwrap().as_str().is_none());
        assert!(props.as_array().is_none());
        assert!(props.at(0).is_none());
        assert!(GVal::Float(f64::NAN).to_i64().is_none());
        assert_eq!(GVal::Str("x".into()).strings(), vec!["x".to_string()]);
        assert!(GVal::Bool(true).strings().is_empty());
    }

    // -- NowPlaying ---------------------------------------------------------

    /// End-to-end extraction from the captured reply — the one test that says
    /// the module does its job.
    #[test]
    fn now_playing_from_the_captured_reply() {
        let np = parse_get_all("org.mpris.MediaPlayer2.fake", GET_ALL).unwrap();
        assert_eq!(np.player, "org.mpris.MediaPlayer2.fake");
        assert_eq!(np.status, PlaybackStatus::Playing);
        assert_eq!(np.title, "Don't Stop 'Til You (Remix)");
        assert_eq!(np.artists, vec!["A, Band".to_string(), "Guest".into()]);
        assert_eq!(np.artist_line(), "A, Band, Guest");
        assert_eq!(np.album, "Album \"Quoted\"");
        assert_eq!(np.art_url.as_deref(), Some("https://i.example.com/ab"));
        assert_eq!(np.track_id.as_deref(), Some("/com/fake/track/1"));
        assert_eq!(np.length_us, Some(245_000_000));
    }

    /// A player that publishes almost nothing must still produce a snapshot —
    /// the widget shows what it has instead of vanishing.
    #[test]
    fn sparse_metadata_degrades_to_empty() {
        let np = parse_get_all("p", "({'PlaybackStatus': <'Paused'>},)").unwrap();
        assert_eq!(np.status, PlaybackStatus::Paused);
        assert_eq!(np.title, "");
        assert!(np.artists.is_empty());
        assert_eq!(np.artist_line(), "");
        assert_eq!(np.length_us, None);
        assert_eq!(np.track_id, None);
        // An unknown status word is not a wrong guess: fall back to the default.
        let np = parse_get_all("p", "({'PlaybackStatus': <'Buffering'>},)").unwrap();
        assert_eq!(np.status, PlaybackStatus::Stopped);
        // Not a property dictionary at all ⇒ no snapshot.
        assert!(parse_get_all("p", "(<int64 5>,)").is_none());
        assert!(parse_get_all("p", "nonsense").is_none());
    }

    /// Real players publish placeholders that mean "unknown". Treating them as
    /// data produces a track identity that never changes and a 0:00 duration.
    #[test]
    fn placeholder_metadata_reads_as_absent() {
        let out = "({'Metadata': <{'mpris:trackid': <objectpath \
                   '/org/mpris/MediaPlayer2/TrackList/NoTrack'>, 'mpris:length': <int64 0>, \
                   'mpris:artUrl': <''>}>},)";
        let np = parse_get_all("p", out).unwrap();
        assert_eq!(np.track_id, None, "NoTrack is not an identity");
        assert_eq!(np.length_us, None, "length 0 means unknown");
        assert_eq!(np.art_url, None, "empty artUrl is not a URL");
    }

    /// Two deviations seen in the wild: `xesam:artist` typed as a plain string,
    /// and only `xesam:albumArtist` published.
    #[test]
    fn artist_field_deviations() {
        let np = parse_get_all("p", "({'Metadata': <{'xesam:artist': <'Solo, Act'>}>},)").unwrap();
        assert_eq!(np.artists, vec!["Solo, Act".to_string()]);
        let np = parse_get_all(
            "p",
            "({'Metadata': <{'xesam:albumArtist': <['Fallback']>}>},)",
        )
        .unwrap();
        assert_eq!(np.artists, vec!["Fallback".to_string()]);
        // An empty artist array still falls through to albumArtist.
        let np = parse_get_all(
            "p",
            "({'Metadata': <{'xesam:artist': <@as []>, 'xesam:albumArtist': <['B']>}>},)",
        )
        .unwrap();
        assert_eq!(np.artists, vec!["B".to_string()]);
    }

    /// Repeat-one replays the same title, album and artist; only the track id
    /// changes. Keying identity on the title would never retrigger the lyrics.
    #[test]
    fn track_identity_keys_on_trackid() {
        let a = parse_get_all("p", GET_ALL).unwrap();
        let mut b = a.clone();
        assert!(a.same_track(&b));
        b.track_id = Some("/com/fake/track/2".into());
        assert!(!a.same_track(&b), "different trackid ⇒ different track");
        // Same everything but a new trackid, i.e. repeat-one.
        let mut repeat = a.clone();
        repeat.track_id = Some("/com/fake/track/1/2".into());
        assert!(!a.same_track(&repeat));
        // Without ids, fall back to the metadata triple.
        let mut c = a.clone();
        let mut d = a.clone();
        c.track_id = None;
        d.track_id = None;
        assert!(c.same_track(&d));
        d.title = "Other".into();
        assert!(!c.same_track(&d));
    }

    /// Firefox's `Properties.GetAll` across a real automatic track advance,
    /// captured verbatim from a live session (Spotify Web, one song ending and
    /// the next starting on its own). Note `mpris:trackid`: it is Firefox's own
    /// object path, with no track component, and it is **the same string in both
    /// replies**. Note also `mpris:length`, which Firefox had not yet revised —
    /// more reason identity must not depend on it.
    const FF_TRACK_1: &str = "({'PlaybackStatus': <'Playing'>, 'Rate': <1.0>, 'Metadata': \
        <{'mpris:trackid': <objectpath '/org/mpris/MediaPlayer2/firefox'>, 'xesam:title': \
        <'Counting Stars'>, 'xesam:album': <'Native'>, 'xesam:artist': <['OneRepublic']>, \
        'xesam:url': <'https://open.spotify.com/lyrics'>, 'mpris:length': <int64 257000000>}>, \
        'Volume': <1.0>, 'Position': <int64 221000000>, 'CanPlay': <true>},)";
    const FF_TRACK_2: &str = "({'PlaybackStatus': <'Playing'>, 'Rate': <1.0>, 'Metadata': \
        <{'mpris:trackid': <objectpath '/org/mpris/MediaPlayer2/firefox'>, 'xesam:title': \
        <'Dil Nu'>, 'xesam:album': <'Two Hearts Never Break The Same'>, 'xesam:artist': \
        <['AP Dhillon, Shinda Kahlon']>, 'xesam:url': <'https://open.spotify.com/lyrics'>, \
        'mpris:length': <int64 257000000>}>, \
        'Volume': <1.0>, 'Position': <int64 1000000>, 'CanPlay': <true>},)";

    /// Regression: a player that hardcodes `mpris:trackid` must not make every
    /// song it ever plays "the same track".
    ///
    /// Before the fix `same_track` returned as soon as both sides had an id, so
    /// this returned `true` for two audibly different songs — the worker never
    /// bumped `Track::seq`, the lyric overlay latched on whatever was playing
    /// when the widget was switched on, and only toggling it off and on (which
    /// goes through `set_config` + `invalidate`) ever recovered.
    #[test]
    fn a_constant_trackid_does_not_hide_a_track_advance() {
        let one = parse_get_all("org.mpris.MediaPlayer2.firefox.instance_1_1802", FF_TRACK_1)
            .expect("Firefox GetAll parses");
        let two = parse_get_all("org.mpris.MediaPlayer2.firefox.instance_1_1802", FF_TRACK_2)
            .expect("Firefox GetAll parses");
        assert_eq!(
            one.track_id, two.track_id,
            "the premise: Firefox reuses one id across the advance"
        );
        assert!(
            !one.same_track(&two),
            "an automatic advance must be visible even when the id never moves"
        );
        assert!(!two.same_track(&one), "and symmetrically");
    }

    /// The other half of the same coin, and the reason the id test cannot simply
    /// be dropped: Firefox re-emits *byte-identical* `Metadata` around twenty
    /// times per song. Every one of those must still read as the same track, or
    /// the overlay is torn down and rebuilt repeatedly mid-song.
    #[test]
    fn a_byte_identical_re_announcement_is_still_the_same_track() {
        let first = parse_get_all("p", FF_TRACK_2).unwrap();
        let again = parse_get_all("p", FF_TRACK_2).unwrap();
        assert!(first.same_track(&again));

        // Late album art is the classic chatty-player revision, and it is not a
        // new track either.
        let mut with_art = again.clone();
        with_art.art_url = Some("https://i.scdn.co/image/abc".into());
        assert!(
            first.same_track(&with_art),
            "late album art is not a new track"
        );

        // Nor is a length correction, which Firefox does issue mid-stream.
        let mut relength = again.clone();
        relength.length_us = Some(198_000_000);
        assert!(
            first.same_track(&relength),
            "a length revision is not a new track"
        );
    }

    // -- Reply shapes -------------------------------------------------------

    /// The fast path must not read the `64` out of the `int64` type prefix.
    #[test]
    fn position_fast_path_skips_the_type_prefix() {
        assert_eq!(parse_position(POSITION), Some(42_123_456));
        assert_eq!(parse_position("(<int64 0>,)"), Some(0));
        assert_eq!(parse_position("(<uint64 900>,)"), Some(900));
        assert_eq!(parse_position("(<int64 -1>,)"), Some(-1));
        assert_eq!(parse_position("(<int32 12>,)"), Some(12));
        assert_eq!(parse_position("(<7>,)"), Some(7));
        // The `Seeked` signal payload has no variant wrapper at all; the event
        // plane reuses this parser for it.
        assert_eq!(parse_position("(int64 61000000,)"), Some(61_000_000));
        // Same answer as the full parser, always.
        assert_eq!(
            parse_position(POSITION),
            parse_gvariant(POSITION).and_then(|v| reply_value(&v).to_i64())
        );
    }

    /// A player that types `Position` as a double must not be half-read as the
    /// digits after the decimal point.
    #[test]
    fn position_falls_back_for_double_replies() {
        assert_eq!(parse_position("(<42123456.0>,)"), Some(42_123_456));
        assert_eq!(parse_position("(<double 1500000.75>,)"), Some(1_500_000));
    }

    /// A reply that carries no position at all is `None`, not 0 — 0 is a
    /// meaningful position and must never be invented.
    #[test]
    fn position_of_a_non_numeric_reply_is_none() {
        assert_eq!(parse_position(STATUS), None);
        assert_eq!(parse_position("()"), None);
        assert_eq!(parse_position(""), None);
        assert_eq!(parse_position("Error: GDBus.Error:...ServiceUnknown"), None);
    }

    /// The trivial status reply, plus the case-tolerance we accept.
    #[test]
    fn status_reply_parsing() {
        assert_eq!(parse_status(STATUS), Some(PlaybackStatus::Playing));
        assert_eq!(parse_status("(<'Paused'>,)"), Some(PlaybackStatus::Paused));
        assert_eq!(
            parse_status("(<'Stopped'>,)"),
            Some(PlaybackStatus::Stopped)
        );
        assert_eq!(
            parse_status("(<'playing'>,)"),
            Some(PlaybackStatus::Playing)
        );
        assert_eq!(parse_status("(<'Wat'>,)"), None);
        assert_eq!(parse_status("(<int64 1>,)"), None);
        assert_eq!(PlaybackStatus::Playing.as_str(), "Playing");
        assert_eq!(PlaybackStatus::default(), PlaybackStatus::Stopped);
    }

    /// Prefix match, not equality: browsers register instance-suffixed names,
    /// and the bare interface name belongs to nobody.
    #[test]
    fn list_names_prefix_filter() {
        let out = "(['org.freedesktop.DBus', ':1.42', 'org.mpris.MediaPlayer2.vlc', \
                   'org.mpris.MediaPlayer2.firefox.instance_1_1234', \
                   'org.mpris.MediaPlayer2', 'org.mpris.MediaPlayer2.vlc', \
                   'org.gnome.Shell', 'org.mpris.MediaPlayer2.spotify'],)";
        assert_eq!(
            parse_list_names(out),
            vec![
                "org.mpris.MediaPlayer2.vlc".to_string(),
                "org.mpris.MediaPlayer2.firefox.instance_1_1234".into(),
                "org.mpris.MediaPlayer2.spotify".into(),
            ]
        );
        assert!(parse_list_names("garbage").is_empty());
        assert!(parse_list_names("([],)").is_empty());
    }

    // -- Selection ladder ---------------------------------------------------

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }
    fn st(v: &[(&str, PlaybackStatus)]) -> Vec<(String, PlaybackStatus)> {
        v.iter().map(|(n, s)| ((*n).to_string(), *s)).collect()
    }

    /// Rung 2: something is actually playing, so follow it regardless of order.
    #[test]
    fn ladder_prefers_a_playing_player() {
        use PlaybackStatus::*;
        let p = names(&["a", "b", "c"]);
        let s = st(&[("a", Paused), ("b", Playing), ("c", Stopped)]);
        assert_eq!(pick_player(&p, &s).as_deref(), Some("b"));
    }

    /// Rung 1, the reason stickiness exists: our player is mid-song and a
    /// background browser tab starts playing. The overlay must not follow it.
    #[test]
    fn ladder_is_sticky_against_a_background_tab() {
        use PlaybackStatus::*;
        let p = names(&["firefox", "vlc"]);
        let s = st(&[("firefox", Playing), ("vlc", Playing)]);
        assert_eq!(
            pick_player_sticky(&p, &s, Some("vlc")).as_deref(),
            Some("vlc"),
            "the incumbent keeps the overlay while it is still playing"
        );
        // With no incumbent, order decides — and order is caller-supplied
        // most-recently-active-first.
        assert_eq!(pick_player(&p, &s).as_deref(), Some("firefox"));
    }

    /// Rung 2 outranks rung 3: a player that is actually playing beats our
    /// paused incumbent, because the user pressed play on it.
    #[test]
    fn ladder_playing_beats_a_paused_incumbent() {
        use PlaybackStatus::*;
        let p = names(&["a", "b"]);
        let s = st(&[("a", Playing), ("b", Paused)]);
        assert_eq!(pick_player_sticky(&p, &s, Some("b")).as_deref(), Some("a"));
    }

    /// Rungs 3 and 4: one paused player is unambiguous; several are not, so the
    /// incumbent (if any) wins and otherwise order decides.
    #[test]
    fn ladder_paused_rungs() {
        use PlaybackStatus::*;
        let p = names(&["a", "b", "c"]);
        // Exactly one paused, nothing playing.
        let one = st(&[("a", Stopped), ("b", Paused), ("c", Stopped)]);
        assert_eq!(pick_player(&p, &one).as_deref(), Some("b"));
        // Several paused, no incumbent ⇒ most recently active (head of list).
        let many = st(&[("a", Paused), ("b", Paused), ("c", Stopped)]);
        assert_eq!(pick_player(&p, &many).as_deref(), Some("a"));
        // Several paused with an incumbent ⇒ keep the incumbent.
        assert_eq!(
            pick_player_sticky(&p, &many, Some("b")).as_deref(),
            Some("b")
        );
    }

    /// Rung 5: a player that stopped between tracks is still ours. Dropping it
    /// would make the overlay flicker to another player every gap.
    #[test]
    fn ladder_keeps_a_stopped_incumbent() {
        use PlaybackStatus::*;
        let p = names(&["a", "b"]);
        let s = st(&[("a", Stopped), ("b", Stopped)]);
        assert_eq!(pick_player_sticky(&p, &s, Some("b")).as_deref(), Some("b"));
    }

    /// An incumbent that left the bus is not viable; selection restarts.
    /// Missing statuses default to `Stopped` rather than failing.
    #[test]
    fn ladder_drops_a_vanished_incumbent() {
        use PlaybackStatus::*;
        let p = names(&["a", "b"]);
        let s = st(&[("b", Playing)]);
        assert_eq!(
            pick_player_sticky(&p, &s, Some("gone")).as_deref(),
            Some("b")
        );
        // No statuses at all: everything is Stopped, so rung 6 answers.
        assert_eq!(pick_player(&p, &[]).as_deref(), Some("a"));
        // Nothing on the bus.
        assert_eq!(pick_player(&[], &s), None);
        assert_eq!(pick_player_sticky(&[], &s, Some("a")), None);
    }

    // -- Usability filter ---------------------------------------------------

    /// Bus name of the live Brave session this whole filter exists for.
    const BRAVE_BUS: &str = "org.mpris.MediaPlayer2.brave.instance6389";

    /// Verbatim `gdbus call ... Properties.Get ... PlaybackStatus` from that
    /// session, while the user believed Spotify Web was playing.
    const BRAVE_STATUS: &str = "(<'Stopped'>,)";

    /// Verbatim `Properties.Get ... Metadata` from the same session. Artwork
    /// (Chromium's favicon, spilled to a temp file) and a zero length — and no
    /// `xesam:title`, no `xesam:artist`, no `mpris:trackid`.
    const BRAVE_METADATA: &str = "(<{'mpris:artUrl': <'file:///tmp/.org.chromium.Chromium.1J5tKq'>, 'mpris:length': <int64 0>}>,)";

    /// Verbatim `Properties.Get ... Position`. Never advanced.
    const BRAVE_POSITION: &str = "(<int64 0>,)";

    /// The same session as one `GetAll`, which is what [`scan_players`] issues.
    const BRAVE_GET_ALL: &str = "({'PlaybackStatus': <'Stopped'>, 'Metadata': <{'mpris:artUrl': <'file:///tmp/.org.chromium.Chromium.1J5tKq'>, 'mpris:length': <int64 0>}>, 'Position': <int64 0>, 'CanSeek': <false>, 'Rate': <1.0>},)";

    fn scan(name: &str, status: PlaybackStatus, has_title: bool) -> PlayerScan {
        PlayerScan {
            name: name.to_string(),
            status,
            has_title,
        }
    }

    /// The captured Brave payload, end to end: it parses, it yields artwork and
    /// nothing else, and it is classified unusable — including as an incumbent,
    /// which is the state Fresco used to latch into and never leave.
    #[test]
    fn the_captured_brave_session_is_classified_unusable() {
        assert_eq!(parse_status(BRAVE_STATUS), Some(PlaybackStatus::Stopped));
        assert_eq!(parse_position(BRAVE_POSITION), Some(0));

        // Through the property-by-property path the event plane uses…
        let meta = parse_gvariant(BRAVE_METADATA).expect("captured metadata must parse");
        let mut np = NowPlaying {
            player: BRAVE_BUS.to_string(),
            status: PlaybackStatus::Stopped,
            ..Default::default()
        };
        apply_metadata(&mut np, reply_value(&meta));

        // …and through the single `GetAll` the scan issues: same verdict.
        let from_get_all = parse_get_all(BRAVE_BUS, BRAVE_GET_ALL).expect("captured GetAll parses");
        assert_eq!(
            from_get_all, np,
            "both paths must read the session the same"
        );

        assert_eq!(
            np.art_url.as_deref(),
            Some("file:///tmp/.org.chromium.Chromium.1J5tKq"),
            "artwork is the one thing this session does publish"
        );
        assert_eq!(np.title, "", "no xesam:title is the whole problem");
        assert!(np.artists.is_empty());
        assert_eq!(np.track_id, None);
        assert_eq!(np.length_us, None, "mpris:length 0 means unknown");

        let brave = PlayerScan::of(&np);
        assert!(!brave.is_usable());
        assert_eq!(brave.status, PlaybackStatus::Stopped);

        // Alone on the bus it is not selected, with or without stickiness.
        assert_eq!(pick_usable_player(std::slice::from_ref(&brave), None), None);
        assert_eq!(
            pick_usable_player(std::slice::from_ref(&brave), Some(BRAVE_BUS)),
            None,
            "an incumbent with no title has nothing to be sticky about"
        );
        // And an empty scan is the same answer, not a panic.
        assert_eq!(pick_usable_player(&[], None), None);
        assert_eq!(pick_usable_player(&[], Some(BRAVE_BUS)), None);
    }

    /// A title-less player loses at **every** rung, not just the bottom one: it
    /// cannot win by playing, by being the only paused player, by being first
    /// in most-recently-active order, or by being the incumbent.
    #[test]
    fn a_title_less_player_loses_at_every_rung() {
        use PlaybackStatus::*;
        let brave = |s| scan(BRAVE_BUS, s, false);
        let vlc = |s| scan("org.mpris.MediaPlayer2.vlc", s, true);

        // Rung 2: Brave is the only thing Playing, VLC merely paused — and the
        // useless player still loses, because it cannot produce a lookup.
        assert_eq!(
            pick_usable_player(&[brave(Playing), vlc(Paused)], None).as_deref(),
            Some("org.mpris.MediaPlayer2.vlc")
        );
        // Rung 4: Brave is paused too, so "exactly one paused" must count VLC.
        assert_eq!(
            pick_usable_player(&[brave(Paused), vlc(Paused)], None).as_deref(),
            Some("org.mpris.MediaPlayer2.vlc")
        );
        // Rung 6: nothing is playing anywhere, and Brave heads the list.
        assert_eq!(
            pick_usable_player(&[brave(Stopped), vlc(Stopped)], None).as_deref(),
            Some("org.mpris.MediaPlayer2.vlc")
        );
        // Rungs 1, 3 and 5: Brave is the incumbent. Stickiness must not save it.
        for (b, v) in [
            (Playing, Playing),
            (Playing, Paused),
            (Playing, Stopped),
            (Paused, Paused),
            (Stopped, Stopped),
        ] {
            assert_eq!(
                pick_usable_player(&[brave(b), vlc(v)], Some(BRAVE_BUS)).as_deref(),
                Some("org.mpris.MediaPlayer2.vlc"),
                "sticky {b:?} incumbent with no title beat a usable {v:?} player"
            );
        }
    }

    /// Stickiness is untouched **between usable players** — the background-tab
    /// case rung 1 exists for still holds.
    #[test]
    fn stickiness_survives_between_two_usable_players() {
        use PlaybackStatus::*;
        let ff = scan("org.mpris.MediaPlayer2.firefox.instance_1_1", Playing, true);
        let vlc = scan("org.mpris.MediaPlayer2.vlc", Playing, true);
        let both = [ff.clone(), vlc.clone()];
        assert_eq!(
            pick_usable_player(&both, Some("org.mpris.MediaPlayer2.vlc")).as_deref(),
            Some("org.mpris.MediaPlayer2.vlc"),
            "a background tab must not steal the overlay mid-song"
        );
        // With no incumbent, caller order (most recently active first) decides.
        assert_eq!(
            pick_usable_player(&both, None).as_deref(),
            Some("org.mpris.MediaPlayer2.firefox.instance_1_1")
        );
        // Rung 5: an incumbent that merely stopped between tracks is kept.
        let stopped = [
            scan("org.mpris.MediaPlayer2.firefox.instance_1_1", Stopped, true),
            scan("org.mpris.MediaPlayer2.vlc", Stopped, true),
        ];
        assert_eq!(
            pick_usable_player(&stopped, Some("org.mpris.MediaPlayer2.vlc")).as_deref(),
            Some("org.mpris.MediaPlayer2.vlc")
        );
        // And a usable incumbent still loses rung 2 to something actually
        // playing — that ordering is unchanged by the filter.
        let mixed = [vlc, scan("org.mpris.MediaPlayer2.mpv", Paused, true)];
        assert_eq!(
            pick_usable_player(&mixed, Some("org.mpris.MediaPlayer2.mpv")).as_deref(),
            Some("org.mpris.MediaPlayer2.vlc")
        );
    }

    /// A bus with nothing usable on it yields no selection at all — several
    /// Chromium windows, a PWA and a dead session are the realistic shape of it.
    #[test]
    fn an_all_useless_bus_yields_no_selection() {
        use PlaybackStatus::*;
        let bus = [
            scan(BRAVE_BUS, Stopped, false),
            scan("org.mpris.MediaPlayer2.chromium.instance42", Playing, false),
            scan("org.mpris.MediaPlayer2.brave.instance6390", Paused, false),
        ];
        assert_eq!(pick_usable_player(&bus, None), None);
        for incumbent in [
            BRAVE_BUS,
            "org.mpris.MediaPlayer2.chromium.instance42",
            "org.mpris.MediaPlayer2.gone",
        ] {
            assert_eq!(pick_usable_player(&bus, Some(incumbent)), None);
        }
    }

    /// The transition that matters in practice: the user hits play on the
    /// Chromium tab, the page finally sets `navigator.mediaSession.metadata`,
    /// and the very same bus name becomes selectable.
    #[test]
    fn a_useless_player_that_gains_a_title_is_picked_up() {
        use PlaybackStatus::*;
        let before = [scan(BRAVE_BUS, Stopped, false)];
        assert_eq!(pick_usable_player(&before, None), None);

        let after = [scan(BRAVE_BUS, Playing, true)];
        assert_eq!(pick_usable_player(&after, None).as_deref(), Some(BRAVE_BUS));
        // Having nothing selected in between must not stop it being picked.
        assert_eq!(
            pick_usable_player(&after, None).as_deref(),
            pick_usable_player(&after, Some(BRAVE_BUS)).as_deref()
        );
        // And the reverse transition gives the overlay back up.
        assert_eq!(pick_usable_player(&before, Some(BRAVE_BUS)), None);
    }

    /// [`PlayerScan::of`] reads the fields it claims to, and a padded title is
    /// no title.
    #[test]
    fn player_scan_summarises_a_snapshot() {
        let np = parse_get_all("org.mpris.MediaPlayer2.fake", GET_ALL).unwrap();
        let s = PlayerScan::of(&np);
        assert_eq!(s.name, "org.mpris.MediaPlayer2.fake");
        assert_eq!(s.status, PlaybackStatus::Playing);
        assert!(s.is_usable());

        let mut blank = np.clone();
        blank.title = "   \t ".to_string();
        assert!(
            !PlayerScan::of(&blank).is_usable(),
            "a whitespace title is as useless for a lookup as an empty one"
        );
        // Artists alone are not enough: the lookup is keyed on the title.
        let mut artist_only = np;
        artist_only.title = String::new();
        assert!(!PlayerScan::of(&artist_only).is_usable());
    }

    // -- Position clock -----------------------------------------------------

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    /// The base case: while playing, the position advances with real time at
    /// the playback rate, with no I/O.
    #[test]
    fn clock_predicts_linearly_while_playing() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(10_000_000, t0);
        assert_eq!(c.predicted_us(t0), 10_000_000);
        assert_eq!(c.predicted_us(at(t0, 100)), 10_100_000);
        assert_eq!(c.predicted_us(at(t0, 2_500)), 12_500_000);
        assert!(c.is_running());
    }

    /// Freezing while paused is rule 6 of the power model *and* correctness:
    /// a clock that keeps running through a pause desynchronises every lyric
    /// after it.
    #[test]
    fn clock_freezes_while_paused_and_resumes_in_place() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(5_000_000, t0);
        c.set_status(PlaybackStatus::Paused, at(t0, 1_000));
        assert_eq!(c.predicted_us(at(t0, 1_000)), 6_000_000);
        assert_eq!(
            c.predicted_us(at(t0, 60_000)),
            6_000_000,
            "a minute of pause must not advance the position"
        );
        assert!(!c.is_running());
        c.set_status(PlaybackStatus::Playing, at(t0, 60_000));
        assert_eq!(c.predicted_us(at(t0, 60_000)), 6_000_000);
        assert_eq!(c.predicted_us(at(t0, 60_500)), 6_500_000);
    }

    /// Small errors must not produce a visible jump. The prediction at the
    /// resync instant is unchanged; the correction is spread over the window.
    #[test]
    fn clock_slews_small_errors_without_a_jump() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(0, t0);
        // At +1s we predict 1.000s; the player says 1.200s (200ms fast).
        let r = c.resync(1_200_000, at(t0, 1_000));
        assert_eq!(r, Resync::Slewed { error_us: 200_000 });
        assert_eq!(
            c.predicted_us(at(t0, 1_000)),
            1_000_000,
            "no jump at the moment of the resync"
        );
        // Halfway through the window: half the correction applied.
        assert_eq!(c.predicted_us(at(t0, 1_500)), 1_600_000);
        // By the end of the window the error is fully absorbed.
        assert_eq!(c.predicted_us(at(t0, 2_000)), 2_200_000);
        // And it stays absorbed, not re-applied.
        assert_eq!(c.predicted_us(at(t0, 3_000)), 3_200_000);
    }

    /// Beyond the threshold the player really is elsewhere (a seek we missed,
    /// a stall); pretending otherwise would take seconds to converge.
    #[test]
    fn clock_snaps_past_the_threshold() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(0, t0);
        let r = c.resync(1_000_000 + SNAP_THRESHOLD_US, at(t0, 1_000));
        assert_eq!(
            r,
            Resync::Snapped {
                error_us: SNAP_THRESHOLD_US
            }
        );
        assert_eq!(c.predicted_us(at(t0, 1_000)), 1_000_000 + SNAP_THRESHOLD_US);
        // Just inside the threshold still slews — the boundary is exact.
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(0, t0);
        let r = c.resync(1_000_000 + SNAP_THRESHOLD_US - 1, at(t0, 1_000));
        assert!(matches!(r, Resync::Slewed { .. }));
        // Backwards errors snap too.
        let r = c.resync(0, at(t0, 2_000));
        assert!(matches!(r, Resync::Snapped { .. }));
        assert_eq!(c.predicted_us(at(t0, 2_000)), 0);
    }

    /// A resync while paused is an exact re-anchor, not a slew: there is no
    /// rate to absorb the error with.
    #[test]
    fn clock_resync_while_paused_is_exact() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Paused, t0);
        let r = c.resync(7_000_000, t0);
        assert_eq!(
            r,
            Resync::Snapped {
                error_us: 7_000_000
            }
        );
        assert_eq!(c.predicted_us(at(t0, 10_000)), 7_000_000);
    }

    /// `Seeked` is authoritative — the player is telling us where it is, so
    /// there is nothing to smooth away.
    #[test]
    fn clock_seek_hard_anchors() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(0, t0);
        c.resync(1_100_000, at(t0, 1_000)); // leave a slew in flight
        c.seeked(30_000_000, at(t0, 1_200));
        assert_eq!(c.predicted_us(at(t0, 1_200)), 30_000_000);
        assert_eq!(
            c.predicted_us(at(t0, 2_200)),
            31_000_000,
            "the pending slew must be discarded by a seek"
        );
        // Negative positions are impossible; clamp rather than propagate.
        c.seeked(-5, at(t0, 3_000));
        assert_eq!(c.predicted_us(at(t0, 3_000)), 0);
    }

    /// A rate change keeps the current position and changes only the slope.
    #[test]
    fn clock_rate_change_preserves_position() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(0, t0);
        c.set_rate(2.0, at(t0, 1_000));
        assert_eq!(c.predicted_us(at(t0, 1_000)), 1_000_000);
        assert_eq!(c.predicted_us(at(t0, 2_000)), 3_000_000);
        assert!((c.rate() - 2.0).abs() < f64::EPSILON);
        // Nonsense rates are ignored, not honoured.
        c.set_rate(f64::NAN, at(t0, 2_000));
        c.set_rate(-1.0, at(t0, 2_000));
        assert!((c.rate() - 2.0).abs() < f64::EPSILON);
        // Rate 0 means "not advancing", and is_running must say so.
        c.set_rate(0.0, at(t0, 2_000));
        assert!(!c.is_running());
        assert_eq!(c.predicted_us(at(t0, 9_000)), 3_000_000);
    }

    /// The slew must never stall or reverse the clock. At rate 0.25 an
    /// uncapped −300ms correction over 1s would run the lyrics backwards.
    #[test]
    fn clock_slew_never_runs_backwards() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(10_000_000, t0);
        c.set_rate(0.25, t0);
        // At +1s we predict 10.250s; claim the player is at 10.000s.
        c.resync(10_000_000, at(t0, 1_000));
        let mut prev = c.predicted_us(at(t0, 1_000));
        for ms in (1_000..=2_000).step_by(50) {
            let now = c.predicted_us(at(t0, ms));
            assert!(
                now >= prev,
                "position went backwards at {ms}ms: {prev} → {now}"
            );
            prev = now;
        }
    }

    /// A new track restarts at 0 without touching status or rate — a track
    /// change is not a pause.
    #[test]
    fn clock_track_change_resets_to_zero() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(200_000_000, t0);
        c.track_changed(at(t0, 500));
        assert_eq!(c.predicted_us(at(t0, 500)), 0);
        assert_eq!(c.predicted_us(at(t0, 1_500)), 1_000_000);
        assert_eq!(c.status(), PlaybackStatus::Playing);
    }

    /// MPRIS defines `Stopped` as position 0; a stopped clock that keeps its
    /// old position would show the last lyric of the previous track.
    #[test]
    fn clock_stopped_resets_to_zero() {
        let t0 = Instant::now();
        let mut c = PositionClock::new(t0);
        c.set_status(PlaybackStatus::Playing, t0);
        c.seeked(120_000_000, t0);
        c.set_status(PlaybackStatus::Stopped, at(t0, 1_000));
        assert_eq!(c.predicted_us(at(t0, 5_000)), 0);
        assert!(!c.is_running());
        // A fresh clock is stopped at 0 and stays there.
        let c = PositionClock::new(t0);
        assert_eq!(c.status(), PlaybackStatus::Stopped);
        assert_eq!(c.predicted_us(at(t0, 10_000)), 0);
    }

    /// The clock is sampled with whatever `Instant` the caller has; an `Instant`
    /// from before the anchor (a stale sample) must not go negative.
    #[test]
    fn clock_tolerates_a_stale_now() {
        let t0 = Instant::now();
        let t1 = at(t0, 5_000);
        let mut c = PositionClock::new(t1);
        c.set_status(PlaybackStatus::Playing, t1);
        c.seeked(1_000_000, t1);
        assert_eq!(c.predicted_us(t0), 1_000_000);
    }

    // -- Degraded-player detection ------------------------------------------

    /// The Spotify pattern: `Playing`, position pinned at 0, forever. Three
    /// spaced polls is the verdict.
    #[test]
    fn spotify_pattern_marks_position_unreliable() {
        let t0 = Instant::now();
        let mut d = PositionReliability::new();
        assert!(!d.observe(0, PlaybackStatus::Playing, t0));
        assert!(!d.observe(0, PlaybackStatus::Playing, at(t0, 3_000)));
        assert!(
            d.observe(0, PlaybackStatus::Playing, at(t0, 6_000)),
            "three spaced zero polls while playing ⇒ unreliable"
        );
        assert!(d.is_unreliable());
    }

    /// The spacing is what makes the rule safe: a healthy player is legitimately
    /// at 0 for the first instants of a track, and a burst of fast polls there
    /// must not convict it.
    #[test]
    fn rapid_zero_polls_do_not_convict() {
        let t0 = Instant::now();
        let mut d = PositionReliability::new();
        for ms in [0, 100, 200, 300, 400, 500, 2_900] {
            assert!(!d.observe(0, PlaybackStatus::Playing, at(t0, ms)));
        }
        assert!(!d.is_unreliable(), "6s of playback has not elapsed");
        // Only the spaced ones count, so the verdict needs the full 6s.
        assert!(!d.observe(0, PlaybackStatus::Playing, at(t0, 3_000)));
        assert!(d.observe(0, PlaybackStatus::Playing, at(t0, 6_000)));
    }

    /// Any working position clears the verdict. The detector describes present
    /// behaviour, not a permanent label — and this is why a bus-name blocklist
    /// is the wrong shape (Spotify in a browser reports correctly).
    #[test]
    fn a_working_position_clears_the_verdict() {
        let t0 = Instant::now();
        let mut d = PositionReliability::new();
        for i in 0..3 {
            d.observe(0, PlaybackStatus::Playing, at(t0, i * 3_000));
        }
        assert!(d.is_unreliable());
        assert!(!d.observe(1_500_000, PlaybackStatus::Playing, at(t0, 9_000)));
        assert!(!d.is_unreliable());
    }

    /// A paused player sitting at 0 is normal and says nothing. Counting those
    /// polls would convict every player left paused at the start of a track —
    /// and we do not poll while paused anyway.
    #[test]
    fn zeros_while_not_playing_are_ignored() {
        let t0 = Instant::now();
        let mut d = PositionReliability::new();
        for i in 0..10 {
            assert!(!d.observe(0, PlaybackStatus::Paused, at(t0, i * 3_000)));
            assert!(!d.observe(0, PlaybackStatus::Stopped, at(t0, i * 3_000)));
        }
        assert!(!d.is_unreliable());
    }

    /// A broken player is broken on every track, so the verdict survives a
    /// track change; a *different* player deserves a clean slate.
    #[test]
    fn verdict_survives_a_track_change_but_not_a_player_change() {
        let t0 = Instant::now();
        let mut d = PositionReliability::new();
        for i in 0..3 {
            d.observe(0, PlaybackStatus::Playing, at(t0, i * 3_000));
        }
        assert!(d.is_unreliable());
        d.track_changed();
        assert!(d.is_unreliable(), "same player, still broken");
        // The streak was dropped, so the next track's legitimate zeros start
        // counting again from scratch.
        d.reset();
        assert!(!d.is_unreliable(), "a different player starts trusted");
        assert!(!d.observe(0, PlaybackStatus::Playing, at(t0, 20_000)));
    }
}

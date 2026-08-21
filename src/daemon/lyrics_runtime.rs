//! Lyrics runtime: turns "what is playing" into "what should be on screen
//! right now" (WIDGETS_ROADMAP W1).
//!
//! [`crate::lyrics`] answers *which line* and *what markup*; [`crate::mpris`]
//! answers *where the playhead is*. Neither remembers anything. This module is
//! the memory between them: it owns the loaded `.lrc`, the resolved style, and
//! — the whole reason it exists — **what is currently on screen**.
//!
//! # Why the memory is the point
//!
//! The roadmap's power model, rule 1: *never redraw unless content changed*. On
//! the W1 OSD path we control only when we push, not how libass draws, so the
//! single lever we have is pushing less. A lyric line is up for 2–8 seconds
//! while the daemon ticks ten times a second — so ~99% of ticks must produce
//! nothing at all. [`LyricsRuntime::tick`] therefore returns [`Action::Idle`]
//! unless the rendered string *actually differs* from the one already pushed,
//! and it reaches that answer without building a string in the common case.
//!
//! [`LyricsRuntime::next_deadline_us`] is the other half: `.lrc` timestamps are
//! known ahead of time, so between lines there is nothing to poll for. The
//! daemon waits on an interruptible deadline until the next line instead of
//! ticking — a 30s instrumental gap costs one wake, not 300.
//!
//! # Where the I/O is
//!
//! [`load_lyrics`] is the only function here that touches the filesystem, and
//! it takes the paths rather than computing them. [`lrc_candidates`] is pure and
//! decides *what to look for*; everything after it is a pure state machine over
//! a position and a status. That split is what makes the interesting rules —
//! the offset sign, the idle guarantee, the pause freeze — unit-testable with no
//! player, no D-Bus and no files.

use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use crate::config::{self, LyricAnchor};
use crate::lyrics::{self, Anchor, LrcLine};
use crate::mpris::{NowPlaying, PlaybackStatus};

// ---------------------------------------------------------------------------
// Finding the file
// ---------------------------------------------------------------------------

/// Where this track's `.lrc` might live, best candidate first.
///
/// Pure: it builds paths and never touches the filesystem, so the whole naming
/// policy is testable without a music library. `url` is the track's `xesam:url`
/// (which [`NowPlaying`] does not carry — it is metadata the daemon fetches on
/// track change), and `folder` is [`config::Lyrics::folder`].
///
/// The order is the order of confidence:
///
/// 1. **The sidecar beside the audio file.** Unbeatable when it exists: it is
///    the file the user put next to *this* recording, so it cannot be the wrong
///    take, the wrong live version or a different mix.
/// 2. **`{artist} - {title}.lrc`** in the lyrics folder — the layout every
///    lyric downloader writes, and specific enough to survive a shared title.
/// 3. **`{title}.lrc`** — the fallback for hand-saved files, and the one most
///    likely to collide, so it deliberately loses to the artist form.
/// 4. **`{album}/{title}.lrc`** — for libraries mirrored per album. Last
///    because it is the least common layout, not because it is less trusted.
///
/// Each tier is then repeated with an uppercase `.LRC` extension. Ext4 is
/// case-sensitive and a lot of these files were authored on Windows, so a
/// `Song.LRC` sitting right there would otherwise be invisible; a handful of
/// extra `stat` calls once per track is not a cost worth optimising.
///
/// Metadata is untrusted text from a stranger's tags: components are stripped
/// of `/` and control characters (NUL included) so a title can only ever name a
/// file *inside* the folder, and a component that sanitises down to nothing,
/// `.` or `..` drops its pattern instead of producing `/lyrics/.lrc`.
pub fn lrc_candidates(np: &NowPlaying, url: Option<&str>, folder: Option<&Path>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    if let Some(side) = url.and_then(file_url_to_path).as_deref().and_then(sidecar) {
        push_unique(&mut out, side);
    }

    if let Some(dir) = folder {
        // Sanitised once: `artist_line()` allocates, and so does every filter.
        let title = sanitise(&np.title);
        let artist = sanitise(&np.artist_line());
        let album = sanitise(&np.album);
        // No title means no name to build a pattern out of — `" - .lrc"` is not
        // a file anyone has, and searching for it is pure syscall waste.
        if let Some(title) = title {
            if let Some(artist) = artist {
                push_unique(&mut out, dir.join(format!("{artist} - {title}.lrc")));
            }
            push_unique(&mut out, dir.join(format!("{title}.lrc")));
            if let Some(album) = album {
                push_unique(&mut out, dir.join(album).join(format!("{title}.lrc")));
            }
        }
    }

    // The uppercase tier goes after *all* of the lowercase ones: a correctly
    // named file two tiers down still beats a shouting one at the top.
    let uppercase: Vec<PathBuf> = out.iter().map(|p| p.with_extension("LRC")).collect();
    for upper in uppercase {
        push_unique(&mut out, upper);
    }
    out
}

/// Append `p` unless it is already queued. The list is at most a handful of
/// entries, so a linear scan is cheaper than a set — and it keeps the order,
/// which is the entire meaning of the return value.
fn push_unique(out: &mut Vec<PathBuf>, p: PathBuf) {
    if !out.contains(&p) {
        out.push(p);
    }
}

/// The `.lrc` sitting beside an audio file: same directory, same stem.
fn sidecar(audio: &Path) -> Option<PathBuf> {
    // A path with no file name (`/`, `..`) would turn into a bare `.lrc` in
    // some parent directory, which is not what "beside the audio file" means.
    audio.file_name()?;
    Some(audio.with_extension("lrc"))
}

/// A `file://` URL → the local path it names. `None` for every other scheme.
///
/// Players publish `xesam:url` as `http(s)://`, `file://` or a custom scheme;
/// only the local one can have a sidecar, and streaming URLs must fall through
/// to the folder patterns rather than producing a nonsense path.
///
/// A percent-decoder rather than a URL crate: this is the only URL parsing in
/// the daemon and it is four lines, where a dependency would be 40 crates.
fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    // `file:///path` leaves `/path`; `file://localhost/path` leaves
    // `localhost/path`. A host we cannot mount is not a path we can read, so
    // take everything from the first `/` either way and let the open fail.
    let path = match rest.find('/') {
        Some(i) => &rest[i..],
        None => return None,
    };
    let bytes = percent_decode(path);
    // Paths are bytes on Unix, not UTF-8 — a lossy decode would corrupt a
    // legitimately non-UTF-8 filename into one that does not exist. A NUL
    // cannot appear in a real path and makes every syscall fail, so drop it
    // here rather than at the open.
    if bytes.is_empty() || bytes.contains(&0) {
        return None;
    }
    Some(PathBuf::from(OsString::from_vec(bytes)))
}

/// `%XX` → the byte it names, everything else verbatim.
///
/// A stray `%` that is not followed by two hex digits is kept as a literal `%`
/// rather than dropped: players do emit under-encoded URLs, and a filename with
/// a real `%` in it is far more likely than a truncated escape.
fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(hi), Some(lo)) = (hex_nibble(b[i + 1]), hex_nibble(b[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// One hex digit's value, either case.
fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// One metadata field → one safe path component.
///
/// `None` when nothing usable is left, which drops the pattern rather than
/// building a path out of a hole. `/` is removed instead of replaced so
/// `"AC/DC"` becomes `ACDC` — the shape a downloader writes — and `.`/`..` are
/// rejected outright so a crafted title cannot name the folder's parent.
fn sanitise(s: &str) -> Option<String> {
    let cleaned: String = s
        .chars()
        // `is_control` covers NUL, and also the newlines and escapes that would
        // otherwise make a filename impossible to type or read in a log line.
        .filter(|c| *c != '/' && !c.is_control())
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return None;
    }
    Some(cleaned.to_string())
}

/// Largest `.lrc` we will read. Synced lyrics are a few kilobytes; this exists
/// only so a lyrics folder mistakenly pointed at a video library cannot make
/// the daemon pull a gigabyte into memory on a track change.
const MAX_LRC_BYTES: u64 = 1 << 20;

/// The first candidate that exists and parses to at least one timed line.
///
/// The only I/O in this module, deliberately kept to one short function so
/// everything else stays a pure state machine.
///
/// A file that exists but yields no lines does **not** stop the search: an
/// empty file, a stub with only `[ar:]`/`[ti:]` headers, or an *unsynced* lyric
/// dump with no timestamps are all common, and all of them should lose to a
/// real file further down the list rather than silently ending it.
pub fn load_lyrics(candidates: &[PathBuf]) -> Option<Vec<LrcLine>> {
    for path in candidates {
        let Ok(file) = File::open(path) else {
            continue;
        };
        let mut buf = Vec::new();
        if file.take(MAX_LRC_BYTES).read_to_end(&mut buf).is_err() {
            continue;
        }
        // Lossy rather than strict UTF-8. Plenty of `.lrc` files in circulation
        // are Shift-JIS, GBK or Latin-1, and the timestamps are ASCII in every
        // one of them — so a lossy decode still gives correctly *timed* lines
        // with some replacement characters, which beats showing nothing.
        let lines = lyrics::parse_lrc(&String::from_utf8_lossy(&buf));
        if !lines.is_empty() {
            log::debug!(
                "lyrics: loaded {} lines from {}",
                lines.len(),
                path.display()
            );
            return Some(lines);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The state machine
// ---------------------------------------------------------------------------

/// What the daemon should do with the overlay after a [`LyricsRuntime::tick`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Draw this. Returned **only** when the words differ from the ones
    /// already on screen — see [`LyricFrame`], which deliberately carries
    /// content and no markup at all.
    Show(LyricFrame),
    /// Remove the overlay. Returned once per transition into "nothing to show",
    /// never repeatedly.
    Clear,
    /// Nothing changed — do not touch mpv. The answer to the overwhelming
    /// majority of ticks, and the reason this type exists at all.
    Idle,
}

/// What we believe is currently on screen.
///
/// `Unknown` is not the same as `Clear`: at startup, and after
/// [`LyricsRuntime::clear`], we have not pushed anything and cannot assume the
/// overlay is empty — a previous daemon run, or an mpv respawn carrying an old
/// overlay, both leave state we do not own. `Unknown` guarantees exactly one
/// push to establish the truth; `Clear` then suppresses every push after it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    Unknown,
    Clear,
    Text(LyricFrame),
}

/// The lyric overlay's memory: loaded lines, resolved config, and what has
/// already been painted.
///
/// One instance per daemon, not per output — the overlay is a single logical
/// widget (see [`config::Widgets::monitor`]), and duplicating the state machine
/// per screen would duplicate the decisions too.
pub struct LyricsRuntime {
    /// Owned copy of the config block. Owned rather than borrowed because the
    /// daemon reloads `config.toml` under us and the runtime outlives any
    /// borrow of the old value.
    cfg: config::Lyrics,
    /// The current track's lines. Empty means "no lyrics for this track", which
    /// is a normal state, not an error.
    lines: Vec<LrcLine>,
    /// The track the lines belong to, kept only to recognise a re-announcement
    /// of the same track (see [`LyricsRuntime::track_changed`]).
    track: Option<NowPlaying>,
    /// What we last pushed.
    screen: Screen,
    /// Line index behind `screen`. Comparing indices is how a tick decides it
    /// has nothing to do without building a frame.
    idx: Option<usize>,
    /// Something *other than the clock* changed — config, track, teardown — so
    /// the next tick must recompute even if the index and accent look
    /// unchanged, and even while playback is paused.
    dirty: bool,
}

impl LyricsRuntime {
    /// A runtime with no track loaded. The first [`tick`](Self::tick) after
    /// this establishes the overlay state with exactly one push.
    pub fn new(cfg: &config::Lyrics) -> Self {
        LyricsRuntime {
            cfg: cfg.clone(),
            lines: Vec::new(),
            track: None,
            screen: Screen::Unknown,
            idx: None,
            dirty: true,
        }
    }

    /// Adopt a new config block.
    ///
    /// Style, anchor, size, margin, offset and the track-info switch all change
    /// what the *current* line looks like, so this forces the next tick to
    /// rebuild even though the line index has not moved — otherwise a preset
    /// change would appear to do nothing until the song reached its next line,
    /// which reads as a bug. The comparison is over the whole block, so a knob
    /// added to [`config::Lyrics`] is picked up here without being named.
    ///
    /// An identical block is ignored on purpose. The GUI rewrites the whole of
    /// `config.toml` for unrelated edits and the daemon re-reads it wholesale;
    /// repainting the lyric because the user picked a new wallpaper would be a
    /// redraw with no content change, which is exactly rule 1's failure mode.
    pub fn set_config(&mut self, cfg: &config::Lyrics) {
        if self.cfg == *cfg {
            return;
        }
        self.cfg = cfg.clone();
        self.dirty = true;
    }

    /// Point the runtime at a new track and its lyrics (`None` when no `.lrc`
    /// was found — a normal outcome, and the state that clears the overlay).
    ///
    /// A re-announcement of the *same* track with no new lyrics is ignored.
    /// MPRIS players emit `PropertiesChanged` on `Metadata` for things that are
    /// not a new track — art arriving late, a rating edit, some clients on every
    /// volume change — and resetting here on each would drop the overlay back
    /// through `Clear` several times a song on a chatty player.
    pub fn track_changed(&mut self, np: &NowPlaying, lines: Option<Vec<LrcLine>>) {
        let same = matches!(&self.track, Some(prev) if prev.same_track(np));
        if same && lines.is_none() && !self.lines.is_empty() {
            return;
        }
        self.track = Some(np.clone());
        self.lines = lines.unwrap_or_default();
        self.idx = None;
        self.dirty = true;
    }

    /// Forget the track and the lyrics: the player went away, the overlay is
    /// being torn down, or the wallpaper is being swapped out from under it.
    ///
    /// The screen state goes back to `Unknown` rather than `Clear`, so the next
    /// tick pushes exactly one [`Action::Clear`] instead of assuming the
    /// overlay is already gone. Re-establishing a line afterwards is a
    /// [`track_changed`](Self::track_changed), which is what the daemon already
    /// does on an output respawn.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.track = None;
        self.idx = None;
        self.screen = Screen::Unknown;
        self.dirty = true;
    }

    /// Advance to `position_us` and say what the overlay should do.
    ///
    /// The accent is deliberately **not** an input any more. It is a property
    /// of the palette, the palette belongs to the rasteriser, and a change to
    /// it reaches the widget through the engine's dirty flag — not by making
    /// this state machine believe the words changed.
    ///
    /// Two guarantees, in order of importance:
    ///
    /// * **[`Action::Show`] only when the words actually differ.** Not when the
    ///   line index changes — when the *content* changes. A repeated chorus and
    ///   a re-announced track are both free.
    /// * **Paused and stopped freeze.** Not clear — the user is looking at a
    ///   paused song and expects its lyric to stay put — and not advance
    ///   either: players report a slightly different position on every poll
    ///   while paused (and a few report `0`), which would otherwise walk the
    ///   overlay through lines nobody is listening to.
    ///
    /// The freeze yields to `dirty`. A config or track change while paused is a
    /// deliberate user action, and a style preview that does nothing until you
    /// press play is a bug report.
    pub fn tick(&mut self, position_us: i64, status: PlaybackStatus) -> Action {
        let idx = self.index_at(position_us);

        // Rule 1's fast path: no string built, no style resolved, no allocation
        // — this is what ~99% of ticks execute.
        if !self.dirty && self.screen != Screen::Unknown {
            if status != PlaybackStatus::Playing {
                return Action::Idle;
            }
            if idx == self.idx {
                return Action::Idle;
            }
        }

        let desired = self.render(idx);
        self.idx = idx;
        self.dirty = false;

        match desired {
            Some(frame) => {
                if matches!(&self.screen, Screen::Text(shown) if *shown == frame) {
                    // The index moved but the words did not: a duplicated line
                    // or a repeated chorus. Still not a redraw.
                    Action::Idle
                } else {
                    self.screen = Screen::Text(frame.clone());
                    Action::Show(frame)
                }
            }
            None => {
                if self.screen == Screen::Clear {
                    Action::Idle
                } else {
                    self.screen = Screen::Clear;
                    Action::Clear
                }
            }
        }
    }

    /// Microseconds of *playback* until the overlay next changes, or `None`
    /// when nothing further will happen on this track.
    ///
    /// This is Smart Sleep. `.lrc` timestamps are known ahead of time, so
    /// between lines there is nothing to poll for: the daemon waits on an
    /// interruptible deadline of this length and ticks once when it expires.
    ///
    /// Two things the caller owns, because this function cannot:
    ///
    /// * **The wait must be interruptible.** Pause, seek, track change and
    ///   player exit all invalidate the deadline; a bare `thread::sleep` would
    ///   leave lyrics running after a pause, which is worse than polling.
    /// * **Do not arm it while paused.** The unit is playback time, and a
    ///   paused clock never reaches it.
    ///
    /// Always at least 1µs, so a deadline can never degenerate into a spin.
    pub fn next_deadline_us(&self, position_us: i64) -> Option<i64> {
        if !self.cfg.enabled || self.lines.is_empty() {
            return None;
        }
        let at = lyrics::next_change_after(&self.lines, self.lyric_time(position_us))?;
        // Back out of lyric time into playback time — the same correction
        // `lyric_time` applies, in the other direction. Saturating throughout:
        // a hand-edited `[99999999:00]` must produce a very long sleep, not a
        // wrapped negative one. (Rust's float→int casts saturate too.)
        let target_us = (at * 1e6).round() as i64;
        let delta = target_us
            .saturating_add(i64::from(self.cfg.offset_ms) * 1_000)
            .saturating_sub(position_us);
        // `next_change_after` is strict, so this is positive in exact
        // arithmetic; the floor only guards the float rounding at a boundary.
        Some(delta.max(1))
    }

    /// Which line is current at `position_us`, or `None` when there is nothing
    /// to show (disabled, no lyrics, or the position is before the first line).
    fn index_at(&self, position_us: i64) -> Option<usize> {
        if !self.cfg.enabled || self.lines.is_empty() {
            return None;
        }
        lyrics::line_at(&self.lines, self.lyric_time(position_us))
    }

    /// Playback position → the time a line is looked up at. **The sign trap.**
    ///
    /// There are two offsets in this feature and they point opposite ways:
    ///
    /// * The `.lrc` `[offset:]` tag, which [`lyrics::parse_lrc`] has already
    ///   folded into every `at`, uses the format's own convention — *positive
    ///   shifts the lyrics earlier*. That one is the file author's correction
    ///   and is not visible from here.
    /// * [`config::Lyrics::offset_ms`] is the user's sync slider and is
    ///   documented the way a slider has to read — *positive = show each line
    ///   later*.
    ///
    /// So a line stamped `at` must become current at `at + offset`. `line_at`
    /// answers "which line is current at `t`", which means the correction goes
    /// on the *position* and is **subtracted**:
    ///
    /// ```text
    /// at + offset <= position   ⟺   at <= position - offset
    /// ```
    ///
    /// Adding it here instead would make the slider run backwards — invisible
    /// in review, obvious on screen, hence the test that pins the direction.
    fn lyric_time(&self, position_us: i64) -> f64 {
        position_us as f64 / 1e6 - f64::from(self.cfg.offset_ms) / 1e3
    }

    /// The frame for line `idx`, or `None` when the overlay should be empty.
    ///
    /// **Two independent things can be on screen**: the current lyric line, and
    /// — when [`config::Lyrics::show_track_info`] is on — a title/artist header
    /// above it. Either one alone is a legitimate overlay, which is the whole
    /// behavioural change the switch brings: a track with no `.lrc` at all used
    /// to mean "nothing to draw", and now means "draw what we do know", because
    /// a now-playing readout that disappears on exactly the tracks LRCLIB has
    /// never heard of is the setting appearing broken where it is wanted most.
    ///
    /// # Content, never markup
    ///
    /// This used to return the ASS payload, which meant the *look* — colours,
    /// sizes, alphas, the accent — was folded into the value the state machine
    /// compared against. On a rasterised card that is exactly wrong twice over:
    /// the palette is not this module's business, and a value that carries the
    /// look changes when the look changes, which would key a megabyte of pixels
    /// off a colour the renderer resolves for itself. So the frame is the
    /// **words**, and nothing else. The engine hashes it into a
    /// `ContentKey`, and everything that is a style change reaches the
    /// rasteriser through the widget's dirty flag instead.
    fn render(&self, idx: Option<usize>) -> Option<LyricFrame> {
        // The master switch used to be enforced entirely by `index_at`
        // returning `None`. The header does not come from a line index, so
        // "no index" no longer implies "nothing to show" and the switch has to
        // be read here too — otherwise turning lyrics off would leave the
        // title sitting on the wallpaper.
        if !self.cfg.enabled {
            return None;
        }
        // A timed blank line is an instrumental gap marker, not a line with no
        // words — `.lrc` files use it to say "clear the overlay here". Holding
        // the previous lyric through a 40-second break is the bug it prevents.
        // With a header up, the gap leaves the header standing on its own,
        // which is exactly the state it exists for.
        let current = idx
            .and_then(|i| self.lines.get(i).map(|line| (i, line)))
            .filter(|(_, line)| !line.text.trim().is_empty());
        let info = self.track_info();
        if current.is_none() && info.is_none() {
            return None;
        }

        let mut frame = LyricFrame::default();
        if let Some((title, artist)) = info {
            // The card's micro-label. Only ever drawn with a title under it, so
            // it is set here rather than unconditionally — a "Now playing" with
            // nothing playing is furniture, not information.
            frame.label = NOW_PLAYING_LABEL.to_string();
            frame.title = title;
            frame.artist = artist.unwrap_or_default();
        }
        if let Some((i, line)) = current {
            frame.lyric = line.text.clone();
            // Only the immediately following line, and only if it has words: a
            // gap marker is the next thing that happens, and previewing past it
            // would show a lyric that is two changes away.
            if self.cfg.show_next_line {
                if let Some(next) = self.lines.get(i + 1).filter(|n| !n.text.trim().is_empty()) {
                    frame.next_lyric = next.text.clone();
                }
            }
        }
        Some(frame)
    }

    /// The header's contents: the title, and the artist under it when there is
    /// one. `None` whenever there is nothing honest to draw.
    ///
    /// Keyed on the **title**, mirroring [`NowPlaying::has_title`]: a player
    /// that published no title has not identified anything, and an artist name
    /// floating over a lyric with no song attached reads as a stray caption
    /// rather than a now-playing display. A missing artist is the opposite
    /// case — the title alone is complete, so it renders alone, with no dash
    /// and no empty second line.
    fn track_info(&self) -> Option<(String, Option<String>)> {
        if !self.cfg.show_track_info {
            return None;
        }
        let np = self.track.as_ref()?;
        let title = fit_for_header(&np.title);
        if title.is_empty() {
            return None;
        }
        let artist = Some(fit_for_header(&np.artist_line())).filter(|a| !a.is_empty());
        Some((title, artist))
    }
}

/// Everything one now-playing card draws, and nothing else.
///
/// Deliberately **content only**: no colour, no size, no anchor, no accent. The
/// look is [`crate::widgetkit`]'s and the placement is the engine's; what this
/// carries is the words, so that "did anything visibly change" is a comparison
/// of what a person would read rather than of a rendered payload.
///
/// `Hash` because the engine turns exactly this into the widget's
/// `ContentKey`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LyricFrame {
    /// The card's micro-label. Empty when there is no header.
    pub label: String,
    /// Track title, already fitted to [`MAX_HEADER_CHARS`]. Empty with no
    /// header.
    pub title: String,
    /// Artist, or empty. `artist · album` is already joined by
    /// [`NowPlaying::artist_line`].
    pub artist: String,
    /// The current lyric line. Empty removes the whole lyric block *and* its
    /// divider, and the card becomes header-only.
    pub lyric: String,
    /// The upcoming line, when [`config::Lyrics::show_next_line`] is on and
    /// there is one with words in it.
    pub next_lyric: String,
}

/// The card's micro-label above the title.
///
/// Not translated here: `crate::i18n` is a GUI concern and the daemon has no
/// locale of its own to consult. The GUI owns the widget controls, and if this
/// is ever localised it is localised there and handed down.
const NOW_PLAYING_LABEL: &str = "Now playing";

/// [`config::LyricAnchor`] → [`lyrics::Anchor`].
///
/// Two enums for one idea, joined here and nowhere else. `config` is the
/// on-disk schema and must stay stable for files written by older Frescos;
/// `lyrics` is the renderer's own vocabulary and is free to grow an anchor the
/// config does not expose. They serialise to the same TOML strings on purpose,
/// so this is a rename-safe mapping and not a translation.
const fn map_anchor(a: LyricAnchor) -> Anchor {
    match a {
        LyricAnchor::TopLeft => Anchor::TopLeft,
        LyricAnchor::TopCenter => Anchor::TopCenter,
        LyricAnchor::TopRight => Anchor::TopRight,
        LyricAnchor::MidLeft => Anchor::MidLeft,
        LyricAnchor::MidCenter => Anchor::MidCenter,
        LyricAnchor::MidRight => Anchor::MidRight,
        LyricAnchor::BottomLeft => Anchor::BottomLeft,
        LyricAnchor::BottomCenter => Anchor::BottomCenter,
        LyricAnchor::BottomRight => Anchor::BottomRight,
    }
}

/// Longest title or artist the header will draw, in characters.
///
/// Not a taste decision. `xesam:title` is whatever the player was handed, and a
/// web radio stream or a DJ set publishes a title with the station name, the
/// bitrate and a URL in it. The card ellipsises to one line on its own, but it
/// measures what it is given first, and measuring a four-kilobyte title on
/// every track change is work nobody asked for. An ellipsis says "there was
/// more" where a silent clip would look like bad metadata.
pub const MAX_HEADER_CHARS: usize = 56;

/// One metadata field → the string the header actually draws: trimmed, and
/// shortened to [`MAX_HEADER_CHARS`] with an ellipsis. Empty when there is
/// nothing left, which is how the caller tells "no artist" from "an artist".
///
/// Counted in `chars` and not bytes: a byte slice would panic on a multibyte
/// boundary, and every non-Latin title in a library is multibyte.
fn fit_for_header(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= MAX_HEADER_CHARS {
        return trimmed.to_string();
    }
    let kept: String = trimmed.chars().take(MAX_HEADER_CHARS - 1).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpris::PlaybackStatus::{Paused, Playing, Stopped};

    // -- helpers ------------------------------------------------------------

    /// Seconds → the microsecond clock the daemon actually carries.
    fn us(secs: f64) -> i64 {
        (secs * 1e6).round() as i64
    }

    fn np(title: &str) -> NowPlaying {
        NowPlaying {
            title: title.to_string(),
            ..Default::default()
        }
    }

    fn np_full(title: &str, artists: &[&str], album: &str) -> NowPlaying {
        NowPlaying {
            title: title.to_string(),
            artists: artists.iter().map(|a| (*a).to_string()).collect(),
            album: album.to_string(),
            ..Default::default()
        }
    }

    /// Three lines at 10s, 20s and 30s — the shape every timing assertion uses.
    fn fixture() -> Vec<LrcLine> {
        lyrics::parse_lrc("[00:10.00]a\n[00:20.00]b\n[00:30.00]c")
    }

    fn enabled_cfg() -> config::Lyrics {
        config::Lyrics {
            enabled: true,
            ..Default::default()
        }
    }

    /// A runtime with `fixture()`-style lines already loaded.
    fn runtime_with(lines: Vec<LrcLine>, tweak: impl FnOnce(&mut config::Lyrics)) -> LyricsRuntime {
        let mut cfg = enabled_cfg();
        tweak(&mut cfg);
        let mut rt = LyricsRuntime::new(&cfg);
        rt.track_changed(&np("song"), Some(lines));
        rt
    }

    fn runtime() -> LyricsRuntime {
        runtime_with(fixture(), |_| {})
    }

    /// The rendered lyric out of a `Show`, or a failure naming what came back.
    fn shown(a: Action) -> LyricFrame {
        match a {
            Action::Show(f) => f,
            other => panic!("expected a Show, got {other:?}"),
        }
    }

    /// Just the lyric line off a [`Action::Show`], which is what most of the
    /// assertions below are actually about.
    fn lyric(a: Action) -> String {
        shown(a).lyric
    }

    // -- lrc_candidates -----------------------------------------------------

    #[test]
    fn a_file_url_yields_the_sidecar_beside_the_audio() {
        // Percent-encoding is the norm, not the exception: every player encodes
        // the spaces in "My Music", and a decoder that missed them would make
        // the sidecar — the best candidate we have — permanently unfindable.
        let got = lrc_candidates(
            &np_full("Song", &["Artist"], "Album"),
            Some("file:///home/u/My%20Music/Artist%20-%20Song.mp3"),
            None,
        );
        assert_eq!(
            got,
            vec![
                PathBuf::from("/home/u/My Music/Artist - Song.lrc"),
                PathBuf::from("/home/u/My Music/Artist - Song.LRC"),
            ]
        );
    }

    #[test]
    fn file_url_edge_cases_do_not_produce_nonsense_paths() {
        let n = np("Song");
        let side = |url: &str| lrc_candidates(&n, Some(url), None);
        // `file://localhost/...` is legal and some players emit it.
        assert_eq!(
            side("file://localhost/m/a.flac")[0],
            PathBuf::from("/m/a.lrc")
        );
        // Scheme case is not significant in a URL.
        assert_eq!(side("FILE:///m/a.opus")[0], PathBuf::from("/m/a.lrc"));
        // A file with no extension still gets one.
        assert_eq!(side("file:///m/track")[0], PathBuf::from("/m/track.lrc"));
        // Non-UTF-8 bytes are a legal filename on Linux and must survive.
        assert_eq!(
            side("file:///m/%FF.mp3")[0],
            PathBuf::from(OsString::from_vec(b"/m/\xff.lrc".to_vec()))
        );
        // An under-encoded `%` is a literal, not a truncated escape.
        assert_eq!(side("file:///m/50%.mp3")[0], PathBuf::from("/m/50%.lrc"));
        // Streams have no sidecar; they must fall through, not invent a path.
        for junk in [
            "https://open.spotify.com/track/x",
            "spotify:track:x",
            "file://",
            "file:///",
            "not a url",
            "",
        ] {
            assert!(side(junk).is_empty(), "input {junk:?} produced a candidate");
        }
        // A NUL cannot be in a real path and makes every syscall fail.
        assert!(side("file:///m/a%00b.mp3").is_empty());
    }

    #[test]
    fn folder_patterns_run_specific_to_general() {
        // Order is the contract: `{title}.lrc` collides across artists, so it
        // must lose to the artist form even though both usually exist.
        let got = lrc_candidates(
            &np_full("Song", &["A", "B"], "Album"),
            None,
            Some(Path::new("/l")),
        );
        assert_eq!(
            got,
            vec![
                PathBuf::from("/l/A, B - Song.lrc"),
                PathBuf::from("/l/Song.lrc"),
                PathBuf::from("/l/Album/Song.lrc"),
                PathBuf::from("/l/A, B - Song.LRC"),
                PathBuf::from("/l/Song.LRC"),
                PathBuf::from("/l/Album/Song.LRC"),
            ]
        );
        // The sidecar still outranks every folder pattern.
        let with_url = lrc_candidates(
            &np_full("Song", &["A"], "Album"),
            Some("file:///m/x.mp3"),
            Some(Path::new("/l")),
        );
        assert_eq!(with_url[0], PathBuf::from("/m/x.lrc"));
    }

    #[test]
    fn missing_metadata_drops_patterns_instead_of_building_holes() {
        // Nothing at all: no title means no name to search for, and a stat of
        // "/l/.lrc" is pure waste.
        assert!(lrc_candidates(&NowPlaying::default(), None, Some(Path::new("/l"))).is_empty());
        // Title only — no artist and no album pattern.
        assert_eq!(
            lrc_candidates(&np("Song"), None, Some(Path::new("/l"))),
            vec![PathBuf::from("/l/Song.lrc"), PathBuf::from("/l/Song.LRC")]
        );
        // No folder configured and no local file: nothing to look at.
        assert!(lrc_candidates(&np_full("Song", &["A"], "Album"), None, None).is_empty());
        // Whitespace-only tags are the same as absent.
        assert!(lrc_candidates(&np("   "), None, Some(Path::new("/l"))).is_empty());
    }

    #[test]
    fn metadata_can_only_ever_name_a_file_inside_the_folder() {
        // Tags are untrusted text from a stranger's library. A `/` in a title
        // is common and innocent ("AC/DC"); `..` in one is not.
        let got = lrc_candidates(
            &np_full("A/B\u{0}C", &["AC/DC"], "../etc"),
            None,
            Some(Path::new("/l")),
        );
        assert_eq!(
            got[..3],
            [
                PathBuf::from("/l/ACDC - ABC.lrc"),
                PathBuf::from("/l/ABC.lrc"),
                PathBuf::from("/l/..etc/ABC.lrc"),
            ]
        );
        for p in &got {
            assert!(p.starts_with("/l"), "{p:?} escaped the folder");
            assert!(
                !p.components()
                    .any(|c| matches!(c, std::path::Component::ParentDir)),
                "{p:?} contains a traversal"
            );
        }
        // A component that sanitises down to exactly `..` drops its pattern
        // rather than becoming an empty or parent-relative one.
        let dotdot = lrc_candidates(&np_full("Song", &[], ".."), None, Some(Path::new("/l")));
        assert_eq!(
            dotdot,
            vec![PathBuf::from("/l/Song.lrc"), PathBuf::from("/l/Song.LRC")]
        );
    }

    // -- load_lyrics --------------------------------------------------------

    #[test]
    fn load_lyrics_takes_the_first_candidate_that_actually_parses() {
        // Files that exist but yield nothing (empty, headers-only, unsynced
        // dumps) are common enough that stopping at the first *existing* file
        // would lose real lyrics sitting one candidate further down.
        let dir = std::env::temp_dir().join(format!("fresco-lyrics-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let empty = dir.join("empty.lrc");
        let headers = dir.join("headers.lrc");
        let real = dir.join("real.lrc");
        std::fs::write(&empty, "").expect("write");
        std::fs::write(&headers, "[ar:Nobody]\n[ti:Untimed]\nplain prose\n").expect("write");
        std::fs::write(&real, "[00:05.00]hello\n").expect("write");

        let missing = dir.join("nope.lrc");
        let got = load_lyrics(&[missing.clone(), empty, headers, real.clone()]);
        assert_eq!(got.as_ref().map(Vec::len), Some(1));
        assert_eq!(got.expect("lines")[0].text, "hello");
        // Nothing readable at all is `None`, not a panic and not an empty vec.
        assert!(load_lyrics(&[missing]).is_none());
        assert!(load_lyrics(&[]).is_none());
        // A directory is openable but not readable as a file — must not abort
        // the search either.
        assert!(load_lyrics(&[dir.clone(), real]).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- the power guarantee ------------------------------------------------

    #[test]
    fn an_unchanged_line_is_painted_exactly_once() {
        // Power-model rule 1, and the reason this module exists. The daemon
        // ticks ten times a second; a lyric is up for seconds at a time.
        let mut rt = runtime();
        // Before the first line there is nothing to show — one Clear, then
        // silence, not a Clear every tick.
        assert_eq!(rt.tick(us(0.0), Playing), Action::Clear);
        for step in 0..100 {
            let t = us(f64::from(step) * 0.1);
            assert_eq!(rt.tick(t, Playing), Action::Idle, "at {t}us");
        }
        // The line lands once...
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
        // ...and every one of the 99 ticks across the rest of its life is free.
        for step in 1..100 {
            let t = us(10.0 + f64::from(step) * 0.1);
            assert_eq!(rt.tick(t, Playing), Action::Idle, "at {t}us");
        }
    }

    #[test]
    fn a_line_change_is_exactly_one_show_carrying_the_new_text() {
        let mut rt = runtime();
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
        assert_eq!(lyric(rt.tick(us(20.0), Playing)), "b");
        assert_eq!(rt.tick(us(20.1), Playing), Action::Idle);
        assert_eq!(lyric(rt.tick(us(30.0), Playing)), "c");
    }

    #[test]
    fn a_repeated_chorus_does_not_repaint() {
        // Two entries, same words: the index moves but the pixels do not.
        // Comparing the *string* and not the index is what catches this.
        let lines = lyrics::parse_lrc("[00:10.00]chorus\n[00:20.00]chorus\n[00:30.00]verse");
        let mut rt = runtime_with(lines, |_| {});
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "chorus");
        assert_eq!(rt.tick(us(20.0), Playing), Action::Idle);
        assert_eq!(lyric(rt.tick(us(30.0), Playing)), "verse");
    }

    #[test]
    fn pause_freezes_the_line_and_resume_does_not_repaint_it() {
        let mut rt = runtime();
        assert_eq!(lyric(rt.tick(us(20.0), Playing)), "b");
        // Paused players report a drifting position — some report 0 — and none
        // of it may move the overlay. Note the tick at 30s: playing, that is a
        // new line; paused, it must be ignored entirely.
        for t in [us(20.0), us(20.4), us(30.0), 0] {
            assert_eq!(rt.tick(t, Paused), Action::Idle, "paused at {t}us");
        }
        assert_eq!(rt.tick(us(20.0), Stopped), Action::Idle);
        // Resuming where we left off must not re-push what is already there.
        assert_eq!(rt.tick(us(20.0), Playing), Action::Idle);
        // And the line the pause hid still arrives once playback resumes.
        assert_eq!(lyric(rt.tick(us(30.0), Playing)), "c");
    }

    #[test]
    fn before_the_first_line_clears_and_past_the_last_holds() {
        let mut rt = runtime();
        assert_eq!(rt.tick(us(0.0), Playing), Action::Clear);
        assert_eq!(rt.tick(us(9.999), Playing), Action::Idle);
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
        // The last line holds to the end of the track rather than clearing —
        // an outro is not an instrumental gap unless the file says so.
        assert_eq!(lyric(rt.tick(us(30.0), Playing)), "c");
        assert_eq!(rt.tick(us(600.0), Playing), Action::Idle);
        // Seeking back before the first line clears once, then stays quiet.
        assert_eq!(rt.tick(us(1.0), Playing), Action::Clear);
        assert_eq!(rt.tick(us(2.0), Playing), Action::Idle);
    }

    #[test]
    fn a_timed_blank_line_clears_the_overlay() {
        // `.lrc` files mark instrumental breaks with a timed empty line.
        // Holding the previous lyric through a 20-second gap is the failure.
        let lines = lyrics::parse_lrc("[00:10.00]a\n[00:20.00]\n[00:30.00]c");
        let mut rt = runtime_with(lines, |_| {});
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
        assert_eq!(rt.tick(us(20.0), Playing), Action::Clear);
        assert_eq!(rt.tick(us(25.0), Playing), Action::Idle);
        assert_eq!(lyric(rt.tick(us(30.0), Playing)), "c");
    }

    #[test]
    fn no_lyrics_and_disabled_both_clear_once() {
        let mut rt = LyricsRuntime::new(&enabled_cfg());
        rt.track_changed(&np("song"), None);
        assert_eq!(rt.tick(us(10.0), Playing), Action::Clear);
        assert_eq!(rt.tick(us(11.0), Playing), Action::Idle);

        // Turning the feature off mid-song must take the overlay down, once.
        let mut rt = runtime();
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
        let mut off = enabled_cfg();
        off.enabled = false;
        rt.set_config(&off);
        assert_eq!(rt.tick(us(10.0), Playing), Action::Clear);
        assert_eq!(rt.tick(us(20.0), Playing), Action::Idle);
        assert_eq!(rt.next_deadline_us(us(10.0)), None);
    }

    #[test]
    fn clear_takes_the_overlay_down_exactly_once() {
        let mut rt = runtime();
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
        rt.clear();
        assert_eq!(rt.tick(us(10.0), Playing), Action::Clear);
        assert_eq!(rt.tick(us(20.0), Playing), Action::Idle);
        assert_eq!(rt.next_deadline_us(us(10.0)), None);
        // And a fresh track brings it back.
        rt.track_changed(&np("next song"), Some(fixture()));
        assert_eq!(lyric(rt.tick(us(20.0), Playing)), "b");
    }

    #[test]
    fn a_re_announced_track_does_not_reset_the_line() {
        // Players emit PropertiesChanged on Metadata for late album art and for
        // volume; each one would otherwise drop the overlay back to Clear.
        let mut rt = runtime();
        assert_eq!(lyric(rt.tick(us(20.0), Playing)), "b");
        rt.track_changed(&np("song"), None);
        assert_eq!(rt.tick(us(20.0), Playing), Action::Idle);
        // A genuinely different track does reset, lyrics or not.
        rt.track_changed(&np("another song"), None);
        assert_eq!(rt.tick(us(20.0), Playing), Action::Clear);
    }

    /// Regression, the second half of the double guard.
    ///
    /// The worker decides a track changed with [`NowPlaying::same_track`] and
    /// bumps `Track::seq`; the engine adopts on the new `seq`; and then *this*
    /// runtime applies `same_track` a second time to drop re-announcements. Two
    /// independent verdicts, and when a player hardcodes `mpris:trackid` — as
    /// Firefox does, to its own object path — the second one used to veto the
    /// first: a bumped `seq` arrived with metadata the runtime called unchanged,
    /// so the new track's lyrics were dropped on the floor and the old line
    /// stayed on screen until the widget was toggled off and on.
    #[test]
    fn an_advance_on_a_constant_trackid_player_still_reloads() {
        const FIREFOX_ID: &str = "/org/mpris/MediaPlayer2/firefox";

        let mut counting_stars = np_full("Counting Stars", &["OneRepublic"], "Native");
        counting_stars.track_id = Some(FIREFOX_ID.into());
        let mut dil_nu = np_full(
            "Dil Nu",
            &["AP Dhillon, Shinda Kahlon"],
            "Two Hearts Never Break The Same",
        );
        dil_nu.track_id = Some(FIREFOX_ID.into());

        let mut rt = LyricsRuntime::new(&enabled_cfg());
        rt.track_changed(&counting_stars, Some(fixture()));
        assert_eq!(lyric(rt.tick(us(20.0), Playing)), "b");

        // Firefox re-emits byte-identical Metadata ~20x a song. Still nothing.
        rt.track_changed(&counting_stars, None);
        assert_eq!(rt.tick(us(20.0), Playing), Action::Idle);

        // The song ends and the next one starts on its own. Same track id, new
        // everything else — this must take the old lyrics down…
        rt.track_changed(&dil_nu, None);
        assert_eq!(
            rt.tick(us(20.0), Playing),
            Action::Clear,
            "the old track's lyric must not survive an automatic advance"
        );

        // …and a later advance that does find lyrics must show them, from the
        // new track's own position rather than the old one's.
        let mut third = np_full("Third", &["Someone"], "Album");
        third.track_id = Some(FIREFOX_ID.into());
        rt.track_changed(&third, Some(fixture()));
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
    }

    // -- offset -------------------------------------------------------------

    #[test]
    fn offset_ms_shifts_lines_in_the_direction_the_slider_promises() {
        // The sign trap. `config::Lyrics::offset_ms` is documented "Positive =
        // show each line later", which is the OPPOSITE convention from the
        // `.lrc` `[offset:]` tag parse_lrc has already applied. Composing them
        // backwards is invisible in review and immediately wrong on screen.
        let mut late = runtime_with(fixture(), |c| c.offset_ms = 1_000);
        // The line is stamped 10s. With +1s it must NOT be up at 10s...
        assert_eq!(late.tick(us(10.0), Playing), Action::Clear);
        assert_eq!(late.tick(us(10.9), Playing), Action::Idle);
        // ...and must arrive a full second late.
        assert_eq!(lyric(late.tick(us(11.0), Playing)), "a");

        // Negative pulls every line earlier, for a file timed against a master
        // with a longer intro.
        let mut early = runtime_with(fixture(), |c| c.offset_ms = -1_000);
        assert_eq!(lyric(early.tick(us(9.0), Playing)), "a");

        // The `[offset:]` tag composes on top in its own direction: +250ms in
        // the file pulls the line 0.25s earlier, the user's +1s pushes it 1s
        // later, so it lands at 10.75s.
        let tagged = lyrics::parse_lrc("[offset:+250]\n[00:10.00]a");
        let mut rt = runtime_with(tagged, |c| c.offset_ms = 1_000);
        assert_eq!(rt.tick(us(10.74), Playing), Action::Clear);
        assert_eq!(lyric(rt.tick(us(10.75), Playing)), "a");
    }

    // -- Smart Sleep --------------------------------------------------------

    #[test]
    fn next_deadline_is_the_wait_until_the_next_line() {
        let rt = runtime();
        assert_eq!(rt.next_deadline_us(us(0.0)), Some(us(10.0)));
        assert_eq!(rt.next_deadline_us(us(15.0)), Some(us(5.0)));
        // On a boundary it must return the line *after* it. Returning 0 here
        // would turn the daemon's wait_timeout into a spin loop, which is the
        // exact failure Smart Sleep exists to avoid.
        assert_eq!(rt.next_deadline_us(us(20.0)), Some(us(10.0)));
        // One microsecond short of a boundary is a one-microsecond wait, not a
        // negative one and not a skipped line.
        assert_eq!(rt.next_deadline_us(us(19.999_999)), Some(1));
        // Past the last line there is nothing left to wake for.
        assert_eq!(rt.next_deadline_us(us(30.0)), None);
        assert_eq!(rt.next_deadline_us(us(600.0)), None);
        // Neither is there with no track loaded.
        assert_eq!(LyricsRuntime::new(&enabled_cfg()).next_deadline_us(0), None);
    }

    #[test]
    fn next_deadline_carries_the_user_offset() {
        // The deadline is in playback time, so it has to undo the same
        // correction the lookup applies — in the other direction.
        let late = runtime_with(fixture(), |c| c.offset_ms = 1_000);
        assert_eq!(late.next_deadline_us(us(0.0)), Some(us(11.0)));
        assert_eq!(late.next_deadline_us(us(11.0)), Some(us(10.0)));
        let early = runtime_with(fixture(), |c| c.offset_ms = -1_000);
        assert_eq!(early.next_deadline_us(us(0.0)), Some(us(9.0)));
        // Never zero, never negative, whatever the float arithmetic does.
        for pos in [0, us(9.999_999), us(10.0), us(29.999_999)] {
            if let Some(d) = runtime().next_deadline_us(pos) {
                assert!(d >= 1, "deadline {d} at {pos}us");
            }
        }
    }

    #[test]
    fn a_gap_costs_one_wake_not_three_hundred() {
        // The roadmap's own number: a 30s instrumental must be one deadline,
        // not 300 polls. Walk the file the way the daemon would and count.
        let lines = lyrics::parse_lrc("[00:10.00]a\n[00:40.00]b");
        let mut rt = runtime_with(lines, |_| {});
        let mut pos = us(10.0);
        let mut wakes = 0;
        assert_eq!(lyric(rt.tick(pos, Playing)), "a");
        while let Some(d) = rt.next_deadline_us(pos) {
            pos += d;
            wakes += 1;
            assert!(matches!(rt.tick(pos, Playing), Action::Show(_)));
            assert!(wakes < 5, "far too many wakes");
        }
        assert_eq!(wakes, 1);
    }

    // -- what changes the frame ---------------------------------------------
    //
    // The frame is **content**, so what repaints it is a content change and
    // nothing else. A style change — preset, colour, anchor, size — no longer
    // moves it, and must not: the look belongs to the palette and the placement
    // to the engine, and both reach the widget through its dirty flag. The
    // tests that used to pin those looks are gone with the payload they read;
    // what is left is the half that is still this module's job.

    #[test]
    fn a_content_change_repaints_the_current_line_immediately() {
        // A settings row that only takes effect at the next lyric reads as
        // broken.
        let mut rt = runtime();
        let before = shown(rt.tick(us(10.0), Playing));
        assert_eq!(before.lyric, "a");
        assert!(before.next_lyric.is_empty());

        let mut cfg = enabled_cfg();
        cfg.show_next_line = true;
        rt.set_config(&cfg);
        let after = shown(rt.tick(us(10.0), Playing));
        assert_ne!(before, after);
        assert_eq!(after.next_lyric, "b");

        // Re-applying the same block is not a change and must not repaint...
        rt.set_config(&cfg);
        assert_eq!(rt.tick(us(10.0), Playing), Action::Idle);
        // ...and neither does a change that cannot alter the content. The GUI
        // rewrites the whole file for unrelated edits.
        cfg.folder = Some(PathBuf::from("/l"));
        rt.set_config(&cfg);
        assert_eq!(rt.tick(us(10.0), Playing), Action::Idle);
        // A *style* edit is the interesting one, and the answer changed with
        // the substrate: it no longer reaches this state machine at all, so the
        // frame is unchanged and the engine's dirty flag is what repaints the
        // pixels. Asserting `Idle` here is asserting that the accent and the
        // size are not smuggled into the content key.
        cfg.anchor = LyricAnchor::TopLeft;
        cfg.font_size_pt = 96;
        cfg.accent_follow = !cfg.accent_follow;
        rt.set_config(&cfg);
        assert_eq!(rt.tick(us(10.0), Playing), Action::Idle);
    }

    #[test]
    fn a_content_change_lands_even_while_paused() {
        // The freeze is about the clock, not about the user. Someone turning
        // the track readout on during a paused song must see it appear.
        let mut rt = runtime();
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
        let mut cfg = enabled_cfg();
        cfg.show_next_line = true;
        rt.set_config(&cfg);
        assert_eq!(shown(rt.tick(us(10.0), Paused)).next_lyric, "b");
        assert_eq!(rt.tick(us(10.0), Paused), Action::Idle);
    }

    #[test]
    fn show_next_line_carries_the_upcoming_line() {
        let mut rt = runtime_with(fixture(), |c| c.show_next_line = true);
        let got = shown(rt.tick(us(10.0), Playing));
        assert_eq!((got.lyric.as_str(), got.next_lyric.as_str()), ("a", "b"));
        // The last line has nothing to preview.
        let last = shown(rt.tick(us(30.0), Playing));
        assert_eq!(last.lyric, "c");
        assert!(last.next_lyric.is_empty(), "{last:?}");
        // Neither does a line followed by a gap marker — previewing past it
        // would show a lyric that is two changes away.
        let mut gapped = runtime_with(
            lyrics::parse_lrc("[00:10.00]a\n[00:20.00]\n[00:30.00]c"),
            |c| c.show_next_line = true,
        );
        let got = shown(gapped.tick(us(10.0), Playing));
        assert_eq!(got.lyric, "a");
        assert!(got.next_lyric.is_empty(), "{got:?}");
        // The preview changes at the same instants as the line itself, so the
        // deadline does not move.
        assert_eq!(rt.next_deadline_us(us(10.0)), Some(us(10.0)));
    }

    #[test]
    fn untrusted_lyric_text_is_carried_verbatim_and_reaches_no_decision() {
        // Its ASS ancestor checked that `{`, `}` and `\` in a `.lrc` file could
        // not open an override block and re-anchor the overlay. That escape
        // class does not exist on a rasterised card — there is no markup to
        // escape into — so what is worth pinning is the property underneath it:
        // third-party text is **data**. It arrives at the renderer exactly as
        // the file wrote it, and nothing in it changes what the widget does.
        let hostile = "[00:10.00]{\\an7}first\n[00:20.00]{\\fs900}\u{2060}second\n\
                       [00:30.00]}}}{{{";
        let mut rt = runtime_with(lyrics::parse_lrc(hostile), |c| c.show_next_line = true);
        let got = shown(rt.tick(us(10.0), Playing));
        assert_eq!(got.lyric, "{\\an7}first");
        assert_eq!(got.next_lyric, "{\\fs900}\u{2060}second");
        // Same shape of frame as a harmless line: the words differ and nothing
        // else does.
        let mut plain = runtime_with(fixture(), |c| c.show_next_line = true);
        let benign = shown(plain.tick(us(10.0), Playing));
        assert_eq!(got.label, benign.label);
        assert_eq!(got.title, benign.title);
        assert_eq!(got.artist, benign.artist);
        // And it still advances on the file's own schedule.
        assert_eq!(lyric(rt.tick(us(30.0), Playing)), "}}}{{{");
    }

    // -- the track-info header ----------------------------------------------

    /// A runtime for one named track with the header switched on.
    fn info_runtime(track: &NowPlaying, lines: Option<Vec<LrcLine>>) -> LyricsRuntime {
        let mut cfg = enabled_cfg();
        cfg.show_track_info = true;
        let mut rt = LyricsRuntime::new(&cfg);
        rt.track_changed(track, lines);
        rt
    }

    fn nightcall() -> NowPlaying {
        np_full("Nightcall", &["Kavinsky"], "OutRun")
    }

    #[test]
    fn the_header_off_costs_nothing_and_shows_nothing() {
        // `show_track_info` is off by default, and off must not put a single
        // character of metadata on the wallpaper.
        let mut rt = LyricsRuntime::new(&enabled_cfg());
        rt.track_changed(&nightcall(), Some(fixture()));
        let rich = shown(rt.tick(us(10.0), Playing));
        assert_eq!(rich.lyric, "a");
        assert!(rich.label.is_empty(), "{rich:?}");
        assert!(rich.title.is_empty(), "{rich:?}");
        assert!(rich.artist.is_empty(), "{rich:?}");
        // A track with full metadata gives the same frame as one with none, so
        // the switch is the only thing that can put a title on screen.
        let mut bare = LyricsRuntime::new(&enabled_cfg());
        bare.track_changed(&np("song"), Some(fixture()));
        assert_eq!(shown(bare.tick(us(10.0), Playing)), rich);
    }

    #[test]
    fn the_header_puts_title_and_artist_beside_the_lyric() {
        let mut rt = info_runtime(
            &np_full("Nightcall", &["Kavinsky", "Lovefoxxx"], "OutRun"),
            Some(fixture()),
        );
        let got = shown(rt.tick(us(10.0), Playing));
        assert_eq!(got.title, "Nightcall");
        assert_eq!(got.artist, "Kavinsky, Lovefoxxx");
        assert_eq!(got.lyric, "a");
        // The micro-label only ever appears with a title under it: a "Now
        // playing" over nothing is furniture, not information.
        assert_eq!(got.label, NOW_PLAYING_LABEL);
    }

    #[test]
    fn a_track_with_no_lyrics_still_shows_the_header() {
        // The point of the whole setting. Streaming something LRCLIB has never
        // heard of is exactly when a now-playing readout is wanted, and it used
        // to be the one case that showed nothing at all — so the switch would
        // look broken precisely where it earns its keep.
        let mut rt = info_runtime(&np_full("Unknown Track", &["Some Band"], ""), None);
        let got = shown(rt.tick(us(10.0), Playing));
        assert_eq!(got.title, "Unknown Track");
        assert_eq!(got.artist, "Some Band");
        // No lyric under it: the card drops the whole lyric block and its
        // divider rather than drawing an empty band.
        assert!(got.lyric.is_empty(), "{got:?}");
        assert!(got.next_lyric.is_empty(), "{got:?}");
        // There is no clock to follow, so every tick after the first is free.
        for step in 0..100 {
            let t = us(f64::from(step) * 0.7);
            assert_eq!(rt.tick(t, Playing), Action::Idle, "at {t}us");
        }
        // With the switch off the same track is still nothing to draw.
        let mut off = LyricsRuntime::new(&enabled_cfg());
        off.track_changed(&np_full("Unknown Track", &["Some Band"], ""), None);
        assert_eq!(off.tick(us(10.0), Playing), Action::Clear);
        // Turning lyrics off entirely takes the header down with them — the
        // master switch is the master switch.
        let mut cfg = enabled_cfg();
        cfg.show_track_info = true;
        let mut rt = info_runtime(&nightcall(), None);
        cfg.enabled = false;
        rt.set_config(&cfg);
        assert_eq!(rt.tick(us(10.0), Playing), Action::Clear);
        assert_eq!(rt.tick(us(11.0), Playing), Action::Idle);
    }

    #[test]
    fn the_header_stands_alone_before_the_first_line_and_through_a_gap() {
        // The states where there is no lyric but the song is still playing are
        // most of an instrumental track; the header is what makes the widget
        // present rather than blinking in and out.
        let lines = lyrics::parse_lrc("[00:10.00]a\n[00:20.00]\n[00:30.00]c");
        let mut rt = info_runtime(&nightcall(), Some(lines));
        let intro = shown(rt.tick(us(0.0), Playing));
        assert_eq!(intro.artist, "Kavinsky");
        assert!(intro.lyric.is_empty(), "{intro:?}");
        assert_eq!(lyric(rt.tick(us(10.0), Playing)), "a");
        // The gap marker drops the lyric and leaves the header exactly where it
        // was, rather than clearing the overlay outright.
        assert_eq!(shown(rt.tick(us(20.0), Playing)), intro);
        assert_eq!(rt.tick(us(25.0), Playing), Action::Idle);
        assert_eq!(lyric(rt.tick(us(30.0), Playing)), "c");
    }

    #[test]
    fn a_track_with_no_title_draws_no_header_at_all() {
        // An artist with no song attached is a caption, not a now-playing
        // display.
        let mut rt = info_runtime(&np_full("", &["Some Band"], "An Album"), Some(fixture()));
        let got = shown(rt.tick(us(10.0), Playing));
        assert!(got.title.is_empty(), "{got:?}");
        assert!(got.artist.is_empty(), "{got:?}");
        assert!(got.label.is_empty(), "{got:?}");
        assert_eq!(got.lyric, "a");
        // A whitespace-only title is the same as none — players do pad.
        let mut padded = info_runtime(&np_full("   ", &["Some Band"], ""), None);
        assert_eq!(padded.tick(us(10.0), Playing), Action::Clear);
        // A title with no artist renders alone, with no empty second row.
        let mut solo = info_runtime(&np("Solo"), Some(fixture()));
        let got = shown(solo.tick(us(10.0), Playing));
        assert_eq!(got.title, "Solo");
        assert!(got.artist.is_empty(), "{got:?}");
        // No track at all is nothing to show, switch or no switch.
        let mut cfg = enabled_cfg();
        cfg.show_track_info = true;
        assert_eq!(
            LyricsRuntime::new(&cfg).tick(us(10.0), Playing),
            Action::Clear
        );
    }

    #[test]
    fn toggling_the_header_repaints_the_line_that_is_already_up() {
        // A settings row that does nothing until the next lyric arrives — or
        // until you press play — is a bug report either way.
        let mut cfg = enabled_cfg();
        let mut rt = LyricsRuntime::new(&cfg);
        rt.track_changed(&nightcall(), Some(fixture()));
        let before = shown(rt.tick(us(10.0), Playing));
        assert!(before.title.is_empty(), "{before:?}");

        cfg.show_track_info = true;
        rt.set_config(&cfg);
        let after = shown(rt.tick(us(10.0), Paused));
        assert_eq!(after.title, "Nightcall");
        assert_eq!(rt.tick(us(10.0), Paused), Action::Idle);

        // And back off takes it down again, once, returning the exact frame it
        // started from.
        cfg.show_track_info = false;
        rt.set_config(&cfg);
        assert_eq!(shown(rt.tick(us(10.0), Paused)), before);
        assert_eq!(rt.tick(us(10.0), Paused), Action::Idle);
    }

    #[test]
    fn an_unchanged_header_over_an_unchanged_line_is_painted_exactly_once() {
        // Rule 1 again, and the reason the header is part of the same frame
        // instead of tracked beside it: the frame is the unit of comparison, so
        // an unchanged title over an unchanged lyric is not a redraw however
        // many times the daemon asks.
        let mut rt = info_runtime(&nightcall(), Some(fixture()));
        // The header lands once during the intro...
        assert_eq!(shown(rt.tick(us(0.0), Playing)).artist, "Kavinsky");
        for step in 0..100 {
            let t = us(f64::from(step) * 0.09);
            assert_eq!(rt.tick(t, Playing), Action::Idle, "at {t}us");
        }
        // ...the first lyric costs one push...
        let first = shown(rt.tick(us(10.0), Playing));
        assert_eq!(
            (first.title.as_str(), first.lyric.as_str()),
            ("Nightcall", "a")
        );
        for step in 1..100 {
            let t = us(10.0 + f64::from(step) * 0.09);
            assert_eq!(rt.tick(t, Playing), Action::Idle, "at {t}us");
        }
        // ...and so does the next, header and all.
        assert_eq!(lyric(rt.tick(us(20.0), Playing)), "b");
        for step in 1..100 {
            let t = us(20.0 + f64::from(step) * 0.09);
            assert_eq!(rt.tick(t, Playing), Action::Idle, "at {t}us");
        }
        // A re-announcement of the same track is not new content either.
        rt.track_changed(&nightcall(), None);
        assert_eq!(rt.tick(us(20.0), Playing), Action::Idle);
    }

    #[test]
    fn untrusted_track_metadata_is_data_and_only_data() {
        // Tags are third-party text out of another process. On the ASS path a
        // `{` in a title opened an override block and could move, recolour or
        // hide the whole overlay; a raw newline split the payload into a second
        // event mpv styled itself. Neither exists here — but the property that
        // made those bugs matter does, so it is pinned: metadata is carried
        // through untouched except for the length bound, and it changes nothing
        // about the frame's shape.
        let evil = np_full(
            "{\\pos(0,0)\\fs900}gotcha\nsecond",
            &["a\\Nb", "{\\an7}"],
            "",
        );
        let mut rt = info_runtime(&evil, Some(fixture()));
        let got = shown(rt.tick(us(10.0), Playing));
        assert_eq!(got.title, "{\\pos(0,0)\\fs900}gotcha\nsecond");
        assert_eq!(got.artist, "a\\Nb, {\\an7}");
        assert_eq!(got.lyric, "a");
        assert_eq!(got.label, NOW_PLAYING_LABEL);
    }

    #[test]
    fn an_absurd_title_is_clipped_rather_than_measured_in_full() {
        // Web radio and DJ sets publish a title with the station name, the
        // bitrate and a URL in it. The card ellipsises to one line itself, but
        // it measures what it is given first, and shaping a four-kilobyte title
        // on every track change is work nobody asked for.
        let long = "A".repeat(400);
        let mut rt = info_runtime(&np_full(&long, &["B".repeat(400).as_str()], ""), None);
        let got = shown(rt.tick(us(10.0), Playing));
        assert_eq!(got.title, format!("{}…", "A".repeat(MAX_HEADER_CHARS - 1)));
        assert_eq!(got.artist, format!("{}…", "B".repeat(MAX_HEADER_CHARS - 1)));
        // Counted in chars and not bytes: a byte slice would panic here, and
        // every non-Latin title in a real library is multibyte.
        let mut cjk = info_runtime(&np_full(&"宇".repeat(200), &[], ""), None);
        assert_eq!(
            shown(cjk.tick(us(10.0), Playing)).title,
            format!("{}…", "宇".repeat(MAX_HEADER_CHARS - 1))
        );
        // A title at exactly the limit keeps every character and gains nothing.
        let exact = "x".repeat(MAX_HEADER_CHARS);
        let mut rt = info_runtime(&np(&exact), None);
        let got = shown(rt.tick(us(10.0), Playing));
        assert_eq!(got.title, exact);
        assert!(!got.title.contains('…'), "{got:?}");
    }

    #[test]
    fn every_config_anchor_maps_to_its_renderer_twin() {
        // Two enums for one idea; they only stay in step because this is the
        // single place they are joined.
        let table = [
            (LyricAnchor::TopLeft, Anchor::TopLeft),
            (LyricAnchor::TopCenter, Anchor::TopCenter),
            (LyricAnchor::TopRight, Anchor::TopRight),
            (LyricAnchor::MidLeft, Anchor::MidLeft),
            (LyricAnchor::MidCenter, Anchor::MidCenter),
            (LyricAnchor::MidRight, Anchor::MidRight),
            (LyricAnchor::BottomLeft, Anchor::BottomLeft),
            (LyricAnchor::BottomCenter, Anchor::BottomCenter),
            (LyricAnchor::BottomRight, Anchor::BottomRight),
        ];
        for (cfg_anchor, want) in table {
            assert_eq!(map_anchor(cfg_anchor), want, "{cfg_anchor:?}");
        }
    }
}

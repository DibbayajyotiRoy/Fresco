//! The widget engine: the one object the daemon's run loops talk to about
//! on-wallpaper widgets (WIDGETS_ROADMAP W1).
//!
//! # Why this is an object and not code in the loops
//!
//! `daemon/mod.rs` has **three** run loops — [`Daemon::run`](super::Daemon)
//! (X11), `run_wayland_layershell` (mpvpaper) and `run_gnome_static`. Widget
//! logic written inline would be written three times and would then drift, which
//! is the failure this codebase already has a name for: `raise_demuxer_cache`
//! silently no-ops on one backend, and the roadmap's own risk list calls that
//! out as the thing not to repeat. So every rule lives here, and each loop makes
//! the same two or three calls:
//!
//! ```text
//! let mut widgets = WidgetEngine::new(config.widgets.as_ref(), accent);  // at start
//!
//! for u in widgets.tick() {                                     // every loop tick
//!     match &u.bitmap {
//!         None => player.set_overlay(u.overlay_id, &u.ass, RES_X, RES_Y),
//!         Some(BitmapUpdate::Draw(b)) =>
//!             player.overlay_add(u.overlay_id, b.x, b.y, &b.path_str(), b.w, b.h, b.stride),
//!         Some(BitmapUpdate::Remove) => player.overlay_remove(u.overlay_id),
//!     }
//! }
//! let wake = widgets.next_deadline();      // clamp the loop's own wait to this
//!
//! for u in widgets.clear_all() { … }       // wallpaper swap / teardown
//! widgets.invalidate();                    // renderer respawn / rotation change
//! ```
//!
//! # The power model is the design
//!
//! Rules 1, 6, 7, 8 and Smart Sleep of the roadmap's power model apply to this
//! module (2–5, 9 and 10 are W2+, on a surface we own — on the OSD path we
//! control only *when we push*). They collapse into two guarantees:
//!
//! * **[`WidgetEngine::tick`] returns an empty `Vec` unless something visible
//!   actually changed.** A lyric held for 8 seconds is one update out of 80
//!   ticks; a clock reading `14:32` produces nothing at all until 14:33. The
//!   comparison is against the *rendered string*, not against the line index or
//!   the minute, so a repeated chorus and an accent the preset ignores are both
//!   free.
//! * **[`WidgetEngine::next_deadline`] tells the loop when to wake.** Two of the
//!   four sources of change here are schedules known in advance — `.lrc`
//!   timestamps and the next minute boundary — so between them there is nothing
//!   to poll for. A 30s instrumental gap costs one wake, not 300. The other two
//!   are continuous by nature (a spectrum follows the music, a record turns), so
//!   they are **rate-capped** instead and publish the cap as their deadline:
//!   [`VISUAL_FPS`] and [`DISC_FPS`], falling to nothing at all the moment the
//!   audio goes quiet or playback pauses.
//!
//! Nothing exists until something is enabled: [`WidgetEngine::new`] with widgets
//! off starts no thread, opens no D-Bus connection, records no audio and
//! allocates nothing beyond the accent string.
//!
//! ## Idle cost, per widget
//!
//! | widget | idle state | what runs |
//! |---|---|---|
//! | lyrics | no player / paused | the worker's condvar sleep (see *Threads*) |
//! | clock | between minutes | nothing — not even a render |
//! | visualiser | silence | one FFT every [`VISUAL_SILENT_PERIOD`] (~13µs), **zero** pushes |
//! | disc | paused | nothing — rotation speed 0 ⇒ [`artwork::should_redraw`] says no |
//!
//! # Threads
//!
//! At most **three**, none of which exist for a widget that is switched off:
//!
//! * The **now-playing worker**, while the lyric *or* the disc widget is
//!   enabled. Every [`crate::mpris`] query shells out to `gdbus` and **blocks**,
//!   as does fetching and decoding cover art, so none of them may run on a 100ms
//!   daemon loop. The worker owns all of it and publishes an immutable
//!   [`Snapshot`] the loop reads with one uncontended mutex acquisition and no
//!   allocation. It is stopped and **joined** when both widgets are disabled or
//!   the engine is dropped — a leaked thread shelling out to `gdbus` forever
//!   would be a real bug.
//! * The **audio capture child process and its reader thread**, while the
//!   visualiser is enabled *and* a capture tool is installed. Both are owned by
//!   [`AudioCapture`], whose `Drop` kills the child and joins the thread.
//!
//! The clock needs no thread at all: its input is the wall clock.
//!
//! # What the lead must connect
//!
//! * Register the module — `#[allow(dead_code)] pub mod widgets;` — matching
//!   `lyrics_runtime` above it.
//! * Push each [`WidgetUpdate`], which now carries an optional
//!   [`bitmap`](WidgetUpdate::bitmap): text widgets go through
//!   `PlayerHandle::set_overlay(overlay_id, &ass, `[`RES_X`]`, `[`RES_Y`]`)`
//!   (an empty `ass` means *remove this overlay*), and the bitmap-bearing disc
//!   goes through `PlayerHandle::overlay_add` / `overlay_remove`. See
//!   [`WidgetUpdate`] for the three-arm `match` and [`BitmapOverlay`] for the
//!   geometry. A call site that ignores `bitmap` still drives all three text
//!   widgets correctly. Passing the resolution explicitly on
//!   the ASS path is the W0 spike's one hard constraint: the OSD coordinate
//!   space otherwise follows the video's render area, and a rotated wallpaper
//!   clips the overlay.
//! * Call [`WidgetEngine::set_output_size`] with the pixel size of the output
//!   the widget layer is on, and again whenever it changes. `overlay-add` is in
//!   **real output pixels**, not the ASS coordinate space, so this is what
//!   places the disc; without it the engine assumes 1920×1080.
//! * Call [`WidgetEngine::set_config`] wherever `Config::load()` is re-read
//!   (`Request::Apply` on all three loops).
//! * Call [`WidgetEngine::clear_all`] on wallpaper swap, renderer teardown and
//!   before an output respawn; [`WidgetEngine::invalidate`] after a respawn or a
//!   rotation change, since a fresh mpv carries no overlays.
//! * The clock, the visualiser and the disc take their settings through small
//!   local structs rather than `config` types — see [`ClockCfg`], [`VisualCfg`]
//!   and [`DiscWidgetCfg`] with [`WidgetEngine::set_clock`],
//!   [`WidgetEngine::set_visualizer`] and [`WidgetEngine::set_disc`]. A loop
//!   that never calls one of those setters gets that widget switched off and
//!   pays nothing for it.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, TimeDelta};
use image::RgbaImage;

use crate::artwork::{self, Bgra, DiscCfg};
use crate::audio_capture::AudioCapture;
use crate::clock::{self, ClockStyle};
use crate::config;
use crate::dsp::{SpectrumAnalyzer, SpectrumConfig};
use crate::lyrics::{self, Anchor, LrcLine};
use crate::mpris::{self, NowPlaying, PlaybackStatus, PositionClock, PositionReliability};
use crate::visualizer::{self, VisualStyleCfg};

use super::lyrics_runtime::{self, Action, LyricsRuntime};

// ---------------------------------------------------------------------------
// The wire between the engine and the loops
// ---------------------------------------------------------------------------

/// Overlay id of the lyric widget.
///
/// mpv's `osd-overlay` is keyed by id, so every widget needs one that nothing
/// else uses. They live here rather than in `daemon/mod.rs` so the engine and
/// the call sites cannot disagree about which id is which.
pub const LYRICS_OVERLAY: u32 = 1;

/// Overlay id of the clock widget. See [`LYRICS_OVERLAY`].
pub const CLOCK_OVERLAY: u32 = 2;

/// Overlay id of the audio visualiser. See [`LYRICS_OVERLAY`].
pub const VISUALIZER_OVERLAY: u32 = 3;

/// Overlay id of the album-art disc. See [`LYRICS_OVERLAY`].
///
/// This one is a **bitmap** overlay (`overlay-add`), not an ASS one. mpv keeps
/// the two in separate id namespaces, so the collision would be harmless — the
/// id is distinct anyway, because a reader tracing "which widget owns overlay
/// 2" should never have to know that.
pub const DISC_OVERLAY: u32 = 4;

/// `res_x` every `osd-overlay` call must carry. Re-exported from
/// [`crate::lyrics`] so a call site never has to remember which coordinate
/// space the payload was built in.
pub const RES_X: u32 = lyrics::PLAY_RES_X;

/// `res_y` every `osd-overlay` call must carry. See [`RES_X`].
pub const RES_Y: u32 = lyrics::PLAY_RES_Y;

/// One overlay's new content.
///
/// **Two payload kinds, because mpv has two overlay commands and they are not
/// interchangeable.** `osd-overlay` draws ASS text and is cleared by pushing an
/// empty event list; `overlay-add` draws raw pixels read from a file and is
/// cleared by `overlay-remove`. Every text widget takes the first path; the
/// album-art disc is the only thing that can take the second, because ASS has
/// no bitmap support at all.
///
/// `bitmap` is what distinguishes them, and it is deliberately an *added field*
/// rather than a rewrite of this type into an enum: a call site that only knows
/// about `overlay_id`/`ass` keeps compiling and keeps behaving correctly for
/// all three text widgets, and gains the disc when it grows the third arm.
///
/// The whole of the lead's dispatch:
///
/// ```text
/// for u in widgets.tick() {
///     match &u.bitmap {
///         None => player.set_overlay(u.overlay_id, &u.ass, RES_X, RES_Y),
///         Some(BitmapUpdate::Draw(b)) =>
///             player.overlay_add(u.overlay_id, b.x, b.y, &b.path_str(), b.w, b.h, b.stride),
///         Some(BitmapUpdate::Remove) => player.overlay_remove(u.overlay_id),
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetUpdate {
    /// [`LYRICS_OVERLAY`], [`CLOCK_OVERLAY`], [`VISUALIZER_OVERLAY`] or
    /// [`DISC_OVERLAY`].
    pub overlay_id: u32,
    /// The `ass-events` payload, or empty to clear. Always empty when `bitmap`
    /// is `Some` — the two payloads are alternatives, not layers.
    pub ass: String,
    /// `None` for the ASS path. `Some` when this update is about pixels; see
    /// [`BitmapUpdate`].
    pub bitmap: Option<BitmapUpdate>,
}

/// The bitmap half of a [`WidgetUpdate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitmapUpdate {
    /// Show these pixels: `overlay_add(id, x, y, &path_str(), w, h, stride)`.
    Draw(BitmapOverlay),
    /// Take the overlay down: `overlay_remove(id)`. The bitmap counterpart of
    /// an empty `ass`, and **not** interchangeable with it.
    Remove,
}

impl WidgetUpdate {
    /// Whether this update takes the overlay down rather than drawing on it.
    pub fn is_clear(&self) -> bool {
        match &self.bitmap {
            None => self.ass.is_empty(),
            Some(BitmapUpdate::Draw(_)) => false,
            Some(BitmapUpdate::Remove) => true,
        }
    }

    /// The pixels this update carries, if it carries any.
    pub fn frame(&self) -> Option<&BitmapOverlay> {
        match &self.bitmap {
            Some(BitmapUpdate::Draw(b)) => Some(b),
            _ => None,
        }
    }

    /// An ASS update for `overlay_id`.
    fn ass(overlay_id: u32, ass: String) -> Self {
        WidgetUpdate {
            overlay_id,
            ass,
            bitmap: None,
        }
    }

    /// An update that blanks a **text** `overlay_id`.
    fn clear(overlay_id: u32) -> Self {
        WidgetUpdate::ass(overlay_id, String::new())
    }

    /// An update that draws `frame` on `overlay_id`.
    fn draw(overlay_id: u32, frame: BitmapOverlay) -> Self {
        WidgetUpdate {
            overlay_id,
            ass: String::new(),
            bitmap: Some(BitmapUpdate::Draw(frame)),
        }
    }

    /// An update that blanks a **bitmap** `overlay_id`.
    fn remove(overlay_id: u32) -> Self {
        WidgetUpdate {
            overlay_id,
            ass: String::new(),
            bitmap: Some(BitmapUpdate::Remove),
        }
    }
}

/// Where a rendered bitmap frame is and how to read it — everything
/// `overlay-add` needs and nothing else.
///
/// `path` is a file the engine **keeps and rewrites in place**, one per engine,
/// because mpv `mmap`s it: a fresh temporary file per frame would be a create,
/// a write, an unlink and a fresh mapping ten times a second, and a file that
/// vanished under mpv would be a SIGBUS rather than a missing frame. The engine
/// owns its lifetime; the caller only ever reads it.
///
/// `x`/`y` are **real output pixels**, not the [`RES_X`]×[`RES_Y`] ASS space —
/// see [`WidgetEngine::set_output_size`], which is what the engine resolves the
/// widget's anchor against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitmapOverlay {
    /// Left edge, in output pixels.
    pub x: i32,
    /// Top edge, in output pixels.
    pub y: i32,
    /// File holding `h * stride` bytes of premultiplied BGRA.
    pub path: PathBuf,
    /// Frame width in pixels.
    pub w: u32,
    /// Frame height in pixels.
    pub h: u32,
    /// Bytes per row — always `w * 4`, since the buffer is tightly packed.
    pub stride: u32,
}

impl BitmapOverlay {
    /// `path` as the `&str` `overlay-add` wants.
    ///
    /// Lossy, because the mpv command line is text and a path is bytes. Not a
    /// practical risk: the engine chose this path itself, under
    /// `$XDG_RUNTIME_DIR`.
    pub fn path_str(&self) -> std::borrow::Cow<'_, str> {
        self.path.to_string_lossy()
    }
}

/// The clock widget's settings — **the one thing the lead must bridge.**
///
/// [`WidgetEngine::set_config`] takes `config::Widgets` and reads the lyric
/// block out of it. It deliberately does *not* read the clock block, because
/// `config::Clock` is a **mirror** of [`ClockStyle`] rather than that type
/// re-exported (its own doc explains why: `config` is the hand-audited shape of
/// `config.toml` and must not inherit renames from a renderer that is free to
/// change), and it says in as many words that *the daemon owns the one small
/// mapping between the two*. This is that seam. Writing it here rather than
/// against a config type that was landing while this module was written is what
/// keeps the engine's API fixed regardless of what that block ends up looking
/// like.
///
/// **To connect** — beside every [`WidgetEngine::set_config`] call site:
///
/// ```text
/// engine.set_config(cfg, accent);
/// engine.set_clock(cfg.map(|w| clock_cfg(&w.clock)).as_ref());
/// ```
///
/// where `clock_cfg` maps the config block onto [`ClockStyle`] field for field —
/// `theme` and `anchor` through small `match`es, `colour` fixed at `#FFFFFF`
/// (the config has no colour key on purpose; accent-follow is the theming
/// path). Nothing in this module changes when that lands.
///
/// A loop that never calls [`WidgetEngine::set_clock`] gets no clock and pays
/// nothing for it, which is also exactly what a user who never enables one
/// should get.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClockCfg {
    /// Master switch. **False by default**: an overlay nobody asked for is a bug
    /// report, and the power budget only holds because nothing is created until
    /// this is true.
    pub enabled: bool,
    /// Look, placement and granularity. See [`ClockStyle`] — note
    /// [`ClockStyle::show_seconds`] is the one field that costs power, turning
    /// one redraw a minute into sixty.
    pub style: ClockStyle,
}

/// The audio visualiser's settings.
///
/// Local for the same reason [`ClockCfg`] is: `config::Visualizer` is the
/// hand-audited shape of `config.toml` and is free to diverge from the
/// renderer's, so the daemon owns the one small mapping between the two and the
/// engine's API stays fixed whatever that block ends up looking like.
///
/// **To connect** — beside every [`WidgetEngine::set_config`] call site:
///
/// ```text
/// engine.set_visualizer(cfg.map(|w| visual_cfg(&w.visualizer)).as_ref());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct VisualCfg {
    /// Master switch. **False by default.** This is the one setting in Fresco
    /// that opens a stream carrying everything the user hears, so it must never
    /// be on because a default said so — see [`AudioCapture::start`]'s own
    /// privacy note.
    pub enabled: bool,
    /// Look and placement. See [`VisualStyleCfg`].
    pub style: VisualStyleCfg,
    /// Number of bars. Feeds [`SpectrumConfig::bands`].
    pub bands: usize,
    /// FFT window length. Feeds [`SpectrumConfig::fft_size`]; 1024 at 44.1 kHz
    /// is a 23 ms window, measured at ~13µs per frame.
    pub fft_size: usize,
    /// Capture sample rate in Hz.
    pub sample_rate: u32,
    /// Redraw cap, in frames per second, while there **is** audio. Clamped to
    /// `1..=60`; see [`VISUAL_FPS`] for why the default is nowhere near 60.
    pub fps: u32,
}

impl Default for VisualCfg {
    fn default() -> Self {
        VisualCfg {
            enabled: false,
            style: VisualStyleCfg::default(),
            bands: 32,
            fft_size: 1024,
            sample_rate: 44_100,
            fps: VISUAL_FPS,
        }
    }
}

/// The album-art disc's settings. Local for the same reason [`VisualCfg`] is.
///
/// **To connect** — beside every [`WidgetEngine::set_config`] call site:
///
/// ```text
/// engine.set_disc(cfg.map(|w| disc_cfg(&w.disc)).as_ref());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscWidgetCfg {
    /// Master switch. **False by default**, and the switch that decides whether
    /// the now-playing worker fetches cover art at all.
    pub enabled: bool,
    /// Where on the output the disc sits.
    pub anchor: Anchor,
    /// Disc diameter in output pixels. Clamped to `1..=`[`artwork::MAX_DISC_PX`]
    /// by [`artwork::render_disc`].
    pub size_px: u32,
    /// Distance from the anchored edge(s), in output pixels. Ignored on
    /// whichever axis the anchor is centred, exactly as in the text widgets.
    pub margin_px: u32,
    /// Turn the disc while the track plays. Off pins it at 0°, which costs one
    /// render per track instead of [`DISC_FPS`] per second.
    pub spin: bool,
    /// 0 (invisible) to 255 (solid).
    pub opacity: u8,
}

impl Default for DiscWidgetCfg {
    fn default() -> Self {
        DiscWidgetCfg {
            enabled: false,
            // Bottom right by default: the lyric widget owns the bottom centre,
            // and the disc must not land on top of it.
            anchor: Anchor::BottomRight,
            size_px: 320,
            margin_px: 48,
            spin: true,
            opacity: 255,
        }
    }
}

// ---------------------------------------------------------------------------
// What the worker publishes
// ---------------------------------------------------------------------------

/// One resolved track: what is playing and the lyrics we found for it.
///
/// Behind an `Arc` in [`Snapshot`] so the loop's per-tick read is a refcount
/// bump rather than a deep copy of the metadata and every `.lrc` line.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// Metadata and status as last polled.
    pub now_playing: NowPlaying,
    /// Lines from the resolved `.lrc`, or `None` when the track has no lyric
    /// file. `None` is an ordinary outcome, not an error.
    pub lyrics: Option<Vec<LrcLine>>,
    /// Cover art, already through [`artwork::prepare_source`] and ready to be
    /// handed to [`artwork::render_disc`] every frame without the 4× cost a
    /// 3000×3000 source would add.
    ///
    /// `None` only when the disc widget is switched off, so the worker never
    /// touched the network for a picture nobody asked for. When it *is* on this
    /// is always `Some`: an unreadable, missing or undecodable cover falls back
    /// to [`artwork::placeholder_art`] rather than to nothing, because a track
    /// change that silently removed the disc reads as a bug (W4).
    ///
    /// Behind an `Arc` for the same reason the whole [`Track`] is: the engine
    /// takes a handle per track change, not a copy of a megabyte of pixels.
    pub art: Option<Arc<RgbaImage>>,
    /// Bumped once per **distinct** track — [`NowPlaying::same_track`] decides,
    /// on the metadata triple *and* `mpris:trackid` together, so repeat-one
    /// retriggers, an advance on a player with a hardcoded track id is still
    /// seen, and a late album-art update is not mistaken for either — and once
    /// more whenever the same track's `.lrc` is
    /// re-resolved after a lyrics-folder change. The engine reloads the lyric
    /// runtime only when this changes, which is what keeps a track change to one
    /// `Vec` clone instead of one per tick.
    pub seq: u64,
}

/// An immutable view of what is playing, published by the worker and read by
/// the loop.
///
/// Cloning is deliberately allocation-free — an `Arc` bump plus a few scalars —
/// because the loop clones one every tick to avoid holding the lock while it
/// renders.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The current track, or `None` when no player is selected.
    pub track: Option<Arc<Track>>,
    /// The playhead. [`PositionClock::predicted_us`] extrapolates it to any
    /// instant, so the loop gets a fresh position every tick without any I/O and
    /// the worker only has to resync once a second.
    pub clock: PositionClock,
    /// The selected player's `Position` is stuck at 0 (Spotify's native Linux
    /// client and some Electron players), so `clock` is free-running from the
    /// last track change instead of being resynced. Diagnostic only — the
    /// engine reads the clock either way.
    pub position_unreliable: bool,
}

impl Snapshot {
    /// The "nothing is playing" snapshot a freshly started worker publishes.
    fn idle(now: Instant) -> Self {
        Snapshot {
            track: None,
            clock: PositionClock::new(now),
            position_unreliable: false,
        }
    }
}

// ---------------------------------------------------------------------------
// The now-playing worker
// ---------------------------------------------------------------------------

/// How often the full player scan (`ListNames` + one `PlaybackStatus` per
/// player) runs while the selected player is **playing**.
///
/// Long, because it is nearly always pointless then: [`mpris::pick_player_sticky`]'s
/// top rung keeps a playing incumbent whatever else appears, and a player that
/// exits is noticed immediately by its next query failing. It runs at all only
/// so a second player that starts playing while the first is between tracks is
/// eventually seen.
const SCAN_PLAYING: Duration = Duration::from_secs(15);

/// How often the full player scan runs while a selected player is paused or
/// stopped.
///
/// This is the rung-2 latency: pressing play in a *different* app can only be
/// noticed by a scan, so it bounds how long the overlay keeps following the app
/// you just left. Five seconds is the compromise between that and the idle cost.
const SCAN_IDLE: Duration = Duration::from_secs(5);

/// How often the player scan runs when **nothing** is on the bus.
///
/// The idle desktop, and the state the roadmap's power AC actually measures
/// ("zero daemon CPU when no player runs — measured, not asserted"). There is no
/// rung-2 latency to protect here because there is no second player to switch
/// to; the only cost of a longer interval is how long after launching a music
/// app the lyrics appear, which is the one moment nobody is looking at the
/// wallpaper.
///
/// Note this is the *empty bus*, not "no player selected": a bus carrying only
/// unusable sessions scans at [`SCAN_IDLE`] instead, because there really is a
/// second player to switch to — see [`scan_interval`].
const SCAN_EMPTY: Duration = Duration::from_secs(15);

/// How often `GetAll` runs while playing — the track-change detector.
///
/// Also the worst-case lag before a new song's lyrics appear. It is not
/// *shorter* because polling metadata is a whole subprocess; it is not longer
/// because two seconds of the previous song's lyrics over the new one is the
/// point at which it looks broken. The roadmap's event plane (a parked
/// `gdbus monitor` on `PropertiesChanged`) removes this poll entirely and is the
/// follow-up that makes this number irrelevant.
const META_PLAYING: Duration = Duration::from_secs(2);

/// How often `GetAll` runs on the selected player while it is not playing. One
/// call, and it answers both "did it start playing" and "did the track change".
const META_IDLE: Duration = Duration::from_secs(5);

/// Floor on the worker's sleep, so a mis-set deadline can never spin.
const MIN_WAIT: Duration = Duration::from_millis(50);

/// `gdbus --timeout`, in seconds, for the one query this module makes itself.
/// Mirrors `mpris`'s own (private) constant: a wedged player must not hold the
/// worker for D-Bus's 25s default.
const CALL_TIMEOUT_SECS: &str = "2";

/// Mutable state shared with the worker thread.
struct WorkerState {
    /// Set by [`Worker::stop`]; the worker checks it at the top of every cycle
    /// and between the two halves of a wait.
    stop: bool,
    /// [`config::Lyrics::folder`], read fresh each cycle so a config change does
    /// not need the thread restarted.
    folder: Option<PathBuf>,
    /// The folder changed: re-resolve the *current* track's `.lrc` instead of
    /// waiting for the next song, which is what makes the GUI's folder picker
    /// appear to work.
    reload: bool,
    /// `Some(px)` while the disc widget wants cover art, carrying the disc size
    /// so [`artwork::prepare_source`] can pick its downscale target. `None`
    /// means the worker must not fetch art at all — the disc is off, and
    /// fetching a picture for a widget nobody enabled would be a network round
    /// trip per track for nothing.
    art_size: Option<u32>,
    /// The disc was just switched on: load art for the *current* track instead
    /// of waiting for the next song. The artwork twin of `reload`.
    art_reload: bool,
    /// What the loop reads.
    snapshot: Snapshot,
}

/// The condvar-guarded channel between the loop and its worker.
struct Shared {
    state: Mutex<WorkerState>,
    /// Signalled on stop and on a config change, so the worker's long idle sleep
    /// is interruptible — the same requirement Smart Sleep puts on the loop.
    wake: Condvar,
}

/// Lock helper that survives a panicked worker rather than propagating it. A
/// poisoned snapshot is at worst stale; taking the daemon down with it is not an
/// improvement.
fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, WorkerState> {
    shared.state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Owns the now-playing thread and joins it on drop.
struct Worker {
    shared: Arc<Shared>,
    join: Option<JoinHandle<()>>,
}

impl Worker {
    /// Start the thread. It publishes its first snapshot within one `gdbus`
    /// round trip; until then the engine sees [`Snapshot::idle`], which renders
    /// as "no lyrics" rather than as stale ones.
    fn start(folder: Option<PathBuf>, art_size: Option<u32>) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(WorkerState {
                stop: false,
                folder,
                reload: false,
                art_size,
                art_reload: art_size.is_some(),
                snapshot: Snapshot::idle(Instant::now()),
            }),
            wake: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let join = std::thread::Builder::new()
            .name("fresco-nowplaying".to_string())
            .spawn(move || run_worker(&worker_shared))
            .map_err(|e| log::warn!("widgets: could not start the now-playing thread: {e}"))
            .ok();
        Worker { shared, join }
    }

    /// The latest published snapshot. One uncontended lock, one `Arc` bump.
    fn snapshot(&self) -> Snapshot {
        lock(&self.shared).snapshot.clone()
    }

    /// Point the worker at a new lyrics folder and have it re-resolve the
    /// current track. A no-op when the folder is unchanged, because the daemon
    /// re-reads the whole of `config.toml` for edits that have nothing to do
    /// with lyrics.
    fn set_folder(&self, folder: Option<PathBuf>) {
        let mut st = lock(&self.shared);
        if st.folder == folder {
            return;
        }
        st.folder = folder;
        st.reload = true;
        drop(st);
        self.shared.wake.notify_all();
    }

    /// Tell the worker whether the disc widget wants cover art, and at what
    /// size. Turning it on re-resolves the *current* track, so enabling the
    /// widget mid-song shows a record rather than waiting for the next one.
    ///
    /// A no-op when unchanged, for the same reason [`Worker::set_folder`] is:
    /// the daemon re-reads the whole of `config.toml` for edits that have
    /// nothing to do with widgets.
    fn set_art(&self, art_size: Option<u32>) {
        let mut st = lock(&self.shared);
        if st.art_size == art_size {
            return;
        }
        st.art_size = art_size;
        st.art_reload = art_size.is_some();
        drop(st);
        self.shared.wake.notify_all();
    }

    /// Ask the thread to finish and wait for it.
    ///
    /// The join blocks for at most one in-flight `gdbus` call, which is itself
    /// bounded by `--timeout 2`. That bound is the price of never leaking a
    /// thread that keeps spawning subprocesses after the daemon has moved on.
    fn stop(&mut self) {
        {
            let mut st = lock(&self.shared);
            st.stop = true;
        }
        self.shared.wake.notify_all();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// How long the worker sleeps before its next cycle: until the nearest deadline
/// that is actually pending.
///
/// Pure, and separated out because it *is* the idle-cost story — and because of
/// the trap it exists to make visible. A deadline belonging to work the worker
/// cannot do (the metadata poll when there is no player, the position poll when
/// nothing is playing) sits permanently in the past, and feeding one in here
/// would floor every sleep at [`MIN_WAIT`] — turning a 15-second idle poll into
/// a 20Hz loop spawning `gdbus` forever. Hence `Option`, and hence the test.
fn cycle_wait(now: Instant, due: [Option<Instant>; 3]) -> Duration {
    due.iter()
        .flatten()
        .map(|d| d.saturating_duration_since(now))
        .min()
        .unwrap_or(MIN_WAIT)
        .max(MIN_WAIT)
}

/// The worker's whole life.
///
/// Cost, by state — this is the "nothing when idle" claim, stated so it can be
/// checked against `powertop` rather than believed:
///
/// | state | what runs | subprocesses |
/// |---|---|---|
/// | no player on the bus | one `ListNames` every 15s | ~0.07/s |
/// | sessions on the bus, none usable | one scan every 5s | ~0.4/s |
/// | a player, not playing | one `GetAll` and one scan every 5s | ~0.5/s |
/// | playing | one `Position` every 1s, one `GetAll` every 2s | ~1.5/s |
///
/// The "none usable" row is the Chromium-family case: a browser holds an MPRIS
/// bus name open for any page that ever played audio, and publishes no
/// `xesam:title` for pages that never set `navigator.mediaSession.metadata`.
/// Fresco used to select such a player and then poll it forever without ever
/// showing anything (the row below it, and the same cost); it now selects
/// nobody and runs the scan alone, which is strictly cheaper and actually
/// recovers when a usable session appears.
///
/// At the measured 3.1ms of CPU per `gdbus` call that is ~0.02% of one core on
/// an idle desktop with no player, and nothing at all between cycles: the thread
/// is parked on a condvar, not spinning. It is **not zero**, and the honest way
/// to reach zero is the roadmap's event plane — a parked `gdbus monitor`, which
/// costs 0 CPU ticks over 10s — replacing the polls above. This function is
/// deliberately shaped so that lands as "stop polling when the monitor is
/// healthy" rather than as a rewrite.
fn run_worker(shared: &Arc<Shared>) {
    let start = Instant::now();
    let mut selected: Option<String> = None;
    let mut clock = PositionClock::new(start);
    let mut reliability = PositionReliability::new();
    let mut track: Option<Arc<Track>> = None;
    let mut seq: u64 = 0;
    // A track whose lyrics came back empty is retried a few times. Players
    // publish Metadata in stages — Firefox commonly reports a title before the
    // artist — so the first lookup after a track change can run against
    // half-filled metadata, miss, and otherwise latch until the user toggles
    // the widget off and on again.
    let mut lyric_retry: Option<Instant> = None;
    let mut lyric_tries: u8 = 0;
    // One `log::info!` per unusable bus name, so the Brave case explains itself.
    let mut skipped = SkipLog::default();
    // All three due immediately, so the first cycle establishes everything.
    let (mut next_scan, mut next_meta, mut next_pos) = (start, start, start);

    loop {
        let (folder, reload, art_size, art_reload) = {
            let mut st = lock(shared);
            if st.stop {
                return;
            }
            (
                st.folder.clone(),
                std::mem::take(&mut st.reload),
                st.art_size,
                std::mem::take(&mut st.art_reload),
            )
        };

        let now = Instant::now();

        // -- who are we following? ------------------------------------------
        // Deliberately only the deadline, never "…or we have no player": with
        // nothing on the bus that condition is true every cycle, which is a
        // `ListNames` per cycle for as long as the desktop is idle.
        if now >= next_scan {
            let players = mpris::list_players();
            // One `GetAll` per player rather than one `PlaybackStatus` — the
            // same single round trip, and it also answers the question the
            // status ladder cannot: whether this player publishes a track title
            // at all. A session with none can never produce a lyric, so it must
            // not be selected over one that can — see `mpris::PlayerScan` for
            // why every Chromium-family browser leaves such sessions lying
            // around on the bus.
            let scans = mpris::scan_players(&players);
            skipped.retain(&players);
            for s in scans.iter().filter(|s| !s.is_usable()) {
                skipped.note(&s.name);
            }
            // "A session exists but we cannot use it" is a different state from
            // "no music app is running", and they want different cadences.
            let sessions_seen = !scans.is_empty();
            let picked = mpris::pick_usable_player(&scans, selected.as_deref());
            if picked != selected {
                // A different program owns the overlay now, so nothing we
                // learned about the old one carries over — not its position,
                // not its track, and not the verdict on whether it can report a
                // position at all.
                clock = PositionClock::new(now);
                reliability.reset();
                track = None;
                next_meta = now;
                if let Some(name) = &picked {
                    log::debug!("widgets: following {name}");
                }
                selected = picked;
            }
            if let Some(name) = selected.as_deref() {
                if let Some(s) = scans.iter().find(|s| s.name == name) {
                    clock.set_status(s.status, now);
                }
            }
            next_scan = now + scan_interval(selected.as_deref(), sessions_seen, &clock);
        }

        // -- what is it doing? ----------------------------------------------
        if now >= next_meta || reload || art_reload {
            if let Some(name) = selected.clone() {
                match mpris::get_all(&name) {
                    // The session is still there but has stopped naming a
                    // track. Either the browser finished with it and left the
                    // bus name behind (see `mpris::PlayerScan`) or it is a real
                    // player between tracks. Both are dead ends for lyrics, and
                    // the old code sat on them indefinitely, so give the
                    // overlay up and let the scan find someone who can.
                    //
                    // Deliberately *not* a new retry timer: this reuses the
                    // scan cadence, the same way `LYRIC_RETRY_BACKOFF` reuses
                    // the metadata poll. And deliberately not `next_scan = now`
                    // — a player that keeps answering without a title would
                    // then be re-picked and re-dropped every cycle, which is
                    // the `MIN_WAIT` spin `cycle_wait`'s own doc warns about.
                    Some(np) if !np.has_title() => {
                        skipped.note(&name);
                        selected = None;
                        track = None;
                        clock = PositionClock::new(now);
                        reliability.reset();
                        next_scan = now + SCAN_IDLE;
                    }
                    Some(np) => {
                        clock.set_status(np.status, now);
                        let changed = track
                            .as_ref()
                            .is_none_or(|t| !t.now_playing.same_track(&np));
                        if changed || reload || art_reload {
                            if changed {
                                // A new track starts at 0. Doing this here and
                                // not on the next resync is what makes a
                                // position-less player (Spotify) free-run from
                                // the right place.
                                clock.track_changed(now);
                                reliability.track_changed();
                            }
                            // Bumped on a re-resolve as well as on a new track:
                            // the engine keys the reload on this, so a folder
                            // change that finds a file would otherwise be found
                            // here and then ignored there.
                            seq = seq.wrapping_add(1);
                            // A disc-only wake must not re-run the lyrics
                            // lookup: that one can reach the network, and
                            // enabling one widget has no business costing a
                            // request on behalf of another.
                            let lines = if changed || reload {
                                let lines = resolve_lyrics(&name, &np, folder.as_deref());
                                lyric_tries = 0;
                                lyric_retry = lines.is_none().then(|| now + LYRIC_RETRY_BACKOFF[0]);
                                lines
                            } else {
                                track.as_ref().and_then(|t| t.lyrics.clone())
                            };
                            // …and symmetrically, a lyrics-only wake keeps the
                            // art it already has rather than re-fetching it.
                            let art = match art_size {
                                None => None,
                                Some(px)
                                    if changed
                                        || art_reload
                                        || track.as_ref().is_none_or(|t| t.art.is_none()) =>
                                {
                                    Some(resolve_art(&np, px))
                                }
                                Some(_) => track.as_ref().and_then(|t| t.art.clone()),
                            };
                            track = Some(Arc::new(Track {
                                now_playing: np,
                                lyrics: lines,
                                art,
                                seq,
                            }));
                        } else if lyric_retry.is_some_and(|at| now >= at)
                            && track.as_ref().is_some_and(|t| t.lyrics.is_none())
                        {
                            // Same track, still no lyrics: try again with the
                            // metadata as it stands now, which may have filled in.
                            if let Some(lines) = resolve_lyrics(&name, &np, folder.as_deref()) {
                                seq = seq.wrapping_add(1);
                                lyric_retry = None;
                                let art = track.as_ref().and_then(|t| t.art.clone());
                                track = Some(Arc::new(Track {
                                    now_playing: np,
                                    lyrics: Some(lines),
                                    art,
                                    seq,
                                }));
                            } else {
                                lyric_tries = lyric_tries.saturating_add(1);
                                lyric_retry = LYRIC_RETRY_BACKOFF
                                    .get(lyric_tries as usize)
                                    .map(|d| now + *d);
                            }
                        }
                        next_meta = now + meta_interval(&clock);
                    }
                    None => {
                        // Gone from the bus mid-poll — normal, players come and
                        // go. Drop everything and rescan on the next cycle.
                        selected = None;
                        track = None;
                        clock = PositionClock::new(now);
                        reliability.reset();
                        next_scan = now;
                    }
                }
            }
        }

        // -- where is the playhead? -----------------------------------------
        if clock.is_running() && now >= next_pos {
            if let Some(name) = selected.clone() {
                match mpris::get_position_us(&name) {
                    Some(polled) => {
                        let unusable = reliability.observe(polled, clock.status(), now);
                        if !unusable {
                            clock.resync(polled, now);
                        }
                    }
                    None => {
                        // Same disappearance as above, and it must clear the
                        // clock too: a snapshot left "playing" would have the
                        // engine advancing lyrics for a player that is gone.
                        selected = None;
                        track = None;
                        clock = PositionClock::new(now);
                        reliability.reset();
                        next_scan = now;
                    }
                }
                next_pos = now + mpris::RESYNC_INTERVAL;
            }
        }

        // -- publish ---------------------------------------------------------
        let snapshot = Snapshot {
            track: track.clone(),
            clock: clock.clone(),
            position_unreliable: reliability.is_unreliable(),
        };
        let mut st = lock(shared);
        st.snapshot = snapshot;
        if st.stop {
            return;
        }
        // Only the deadlines that belong to work this state can actually do.
        let following = selected.is_some();
        let wait = cycle_wait(
            Instant::now(),
            [
                Some(next_scan),
                following.then_some(next_meta),
                (following && clock.is_running()).then_some(next_pos),
            ],
        );
        let (st, _) = shared
            .wake
            .wait_timeout(st, wait)
            .unwrap_or_else(PoisonError::into_inner);
        if st.stop {
            return;
        }
        drop(st);
    }
}

/// Scan cadence for what we are currently following. See [`SCAN_EMPTY`].
///
/// `sessions_seen` is whether the last scan found any media session at all,
/// usable or not. It only matters when nothing is selected, and it is the
/// difference between the two "no player" states:
///
/// * **Nothing on the bus.** Only starting a music app can change that, and a
///   15s latency there is invisible. [`SCAN_EMPTY`].
/// * **Sessions exist, none of them usable.** A Chromium window sitting on a
///   page that never set `navigator.mediaSession.metadata`. The user pressing
///   play makes one of those sessions publish a title, and noticing that is
///   exactly as urgent as noticing a *different* app start playing — so it gets
///   the same [`SCAN_IDLE`] cadence, not the idle-desktop one.
///
/// This costs no more than the old behaviour in that state: Fresco used to
/// *select* the title-less player and then poll it (a scan plus a `GetAll` every
/// five seconds) forever without ever showing anything. Now it selects nobody
/// and runs the scan alone.
fn scan_interval(selected: Option<&str>, sessions_seen: bool, clock: &PositionClock) -> Duration {
    match selected {
        Some(_) if clock.status() == PlaybackStatus::Playing => SCAN_PLAYING,
        Some(_) => SCAN_IDLE,
        None if sessions_seen => SCAN_IDLE,
        None => SCAN_EMPTY,
    }
}

/// Cap on the bus names [`SkipLog`] remembers.
///
/// Bounded because the names churn: every browser restart mints a fresh
/// `instanceNNNN`, so an unbounded set grows for as long as the daemon runs.
/// Far above any real desktop's player count, and the only consequence of an
/// eviction is one repeated log line.
const SKIP_LOG_CAP: usize = 32;

/// One-shot diagnostics for players that cannot drive the lyric widget.
///
/// Without this the Brave case is indistinguishable from Fresco being broken:
/// a media session is on the bus, the user is looking at a page that is
/// playing, and the overlay stays empty with nothing in the log to explain it.
/// One line per bus name — **not** per poll, which at the scan cadence would be
/// twelve lines a minute forever — naming the player so the next person can go
/// and look at it.
///
/// Names are forgotten when they leave the bus ([`SkipLog::retain`]), so
/// restarting the browser reports again rather than staying silent about a
/// genuinely new session.
#[derive(Debug, Default)]
struct SkipLog {
    seen: Vec<String>,
}

impl SkipLog {
    /// Report `name` as unusable, at most once. Returns whether this call was
    /// the one that logged.
    fn note(&mut self, name: &str) -> bool {
        if self.seen.iter().any(|n| n == name) {
            return false;
        }
        if self.seen.len() >= SKIP_LOG_CAP {
            self.seen.remove(0);
        }
        self.seen.push(name.to_string());
        log::info!(
            "widgets: ignoring {name}: media session has no track title \
             (the page may not set Media Session metadata, or the browser is \
             holding a finished session open)"
        );
        true
    }

    /// Forget every name that is no longer on the bus.
    fn retain(&mut self, present: &[String]) {
        self.seen.retain(|n| present.iter().any(|p| p == n));
    }
}

/// Metadata cadence for the current playback state. See [`META_PLAYING`].
fn meta_interval(clock: &PositionClock) -> Duration {
    if clock.status() == PlaybackStatus::Playing {
        META_PLAYING
    } else {
        META_IDLE
    }
}

/// How long to wait before re-attempting a lyrics lookup that came back empty,
/// per attempt. Spaced out deliberately: a miss is usually "this player has not
/// finished publishing its metadata yet", which resolves in a second or two,
/// but it can also be "this track genuinely has no lyrics" — and that must not
/// turn into a poll against a free community service. Four tries, then stop.
const LYRIC_RETRY_BACKOFF: [Duration; 4] = [
    Duration::from_millis(1500),
    Duration::from_secs(4),
    Duration::from_secs(10),
    Duration::from_secs(30),
];

/// Find and load this track's `.lrc`. Runs on the worker: it stats several
/// paths and reads a file.
fn resolve_lyrics(
    player: &str,
    np: &NowPlaying,
    folder: Option<&std::path::Path>,
) -> Option<Vec<LrcLine>> {
    let url = track_url(player);
    let candidates = lyrics_runtime::lrc_candidates(np, url.as_deref(), folder);
    if let Some(lines) = lyrics_runtime::load_lyrics(&candidates) {
        return Some(lines);
    }
    // No local .lrc. Almost nobody streaming from Spotify or a browser has one,
    // so a local-only lookup means the widget shows nothing for most people.
    // Fall back to the online database — cache-first, so a repeat play of the
    // same track never touches the network. Blocking, which is fine: this runs
    // on the worker, once per track change, never on a render tick.
    let q = crate::lyrics_fetch::query_from(np)?;
    match crate::lyrics_fetch::fetch_cached(&q) {
        Ok(Some(lrc)) => {
            let lines = crate::lyrics::parse_lrc(&lrc);
            (!lines.is_empty()).then_some(lines)
        }
        Ok(None) => None,
        Err(e) => {
            // Never retried here: a transport failure must not become a hot
            // loop against a free community service (see its rate-limit rules).
            log::debug!("lyrics lookup failed: {e}");
            None
        }
    }
}

/// Fetch, decode and pre-scale this track's cover art. Runs on the worker,
/// once per track: [`artwork::load_bytes`] reads a file or **blocks on the
/// network for up to ten seconds**, and [`artwork::decode_art`] can spend
/// milliseconds on a 3000×3000 PNG. Neither may happen on a render tick.
///
/// Never fails. Every step here fails routinely — the player published no
/// `mpris:artUrl`, the URL names a path inside another application's sandbox
/// (the long-standing Firefox-Flatpak behaviour), the host is down, the bytes
/// are not an image — and W4 is explicit that none of them may take the widget
/// down. All of them land on [`artwork::placeholder_art`], which
/// [`artwork::render_disc`] turns into an unlabelled record.
///
/// [`artwork::prepare_source`] is applied **here and only here**, which is the
/// point of doing this on the worker at all: it is what makes every subsequent
/// frame cheap, and skipping it costs ~4× per frame forever after.
fn resolve_art(np: &NowPlaying, size_px: u32) -> Arc<RgbaImage> {
    let decoded = np
        .art_url
        .as_deref()
        .and_then(artwork::parse_art_url)
        .and_then(|src| {
            artwork::load_bytes(&src)
                .map_err(|e| log::debug!("widgets: no cover art: {e}"))
                .ok()
        })
        .and_then(|bytes| {
            artwork::decode_art(&bytes)
                .map_err(|e| log::debug!("widgets: cover art will not decode: {e}"))
                .ok()
        });
    match decoded {
        Some(art) => Arc::new(artwork::prepare_source(&art, size_px).into_owned()),
        None => Arc::new(artwork::placeholder_art(size_px)),
    }
}

/// The playing file's `xesam:url`, when it has one.
///
/// The one query this module makes for itself, and it exists for one reason:
/// `xesam:url` is what makes the **sidecar** tier of
/// [`lyrics_runtime::lrc_candidates`] reachable — the `.lrc` sitting next to the
/// audio file, which that function documents as the best candidate there is and
/// the only one that cannot be the wrong take of the right song. [`NowPlaying`]
/// does not carry the field and `mpris`'s own `gdbus` helper is private, so the
/// alternatives were to pass `None` (and lose local-library lyrics entirely
/// unless a folder happens to be configured) or to spend fifteen lines here.
///
/// **Worth removing:** add `url: Option<String>` to [`NowPlaying`] in
/// `mpris::apply_metadata` and this function deletes itself.
///
/// Once per track change, on the worker thread, never on the loop.
fn track_url(player: &str) -> Option<String> {
    let out = Command::new("gdbus")
        .args(["call", "--session", "--timeout", CALL_TIMEOUT_SECS])
        .args(["--dest", player, "--object-path", mpris::MPRIS_PATH])
        .args(["--method", "org.freedesktop.DBus.Properties.Get"])
        .args([mpris::PLAYER_IFACE, "Metadata"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let reply = mpris::parse_gvariant(&text)?;
    // `gdbus call` always replies with a one-element tuple; `parse_gvariant`
    // unwraps the variant inside it, so the dictionary is at index 0.
    let meta = reply.at(0).unwrap_or(&reply);
    meta.dict_get("xesam:url")
        .and_then(mpris::GVal::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

// ---------------------------------------------------------------------------
// The visualiser
// ---------------------------------------------------------------------------

/// Default redraw cap for the visualiser, in frames per second.
///
/// A spectrum has no schedule to sleep until — it follows the music — so the
/// only lever is how often we agree to look. 24 is well inside the range where
/// bars read as continuous motion (the analyser's own ~250 ms release means a
/// falling bar is smooth at far less), and it is **half** the rate a naive
/// "render every tick" implementation would produce against a 20 Hz loop with
/// the ~13µs FFT plus an ASS re-render and an IPC round trip behind each one.
/// Going to 60 would triple that cost for motion nobody can see on a wallpaper.
pub const VISUAL_FPS: u32 = 24;

/// How often the capture is examined while the audio is **silent**.
///
/// This is the whole idle story for the visualiser. Silence pushes *nothing* —
/// one clear on the way in and then not another byte of IPC — but something
/// still has to notice when the music comes back, and there is no event for
/// that. Four looks a second is ~50µs of CPU per second of silence, and the
/// worst case is that a track starts up to 250 ms before its first bar appears,
/// which is under the latency of the player's own metadata poll.
pub const VISUAL_SILENT_PERIOD: Duration = Duration::from_millis(250);

/// Band level at or below which the spectrum counts as nothing worth drawing,
/// on top of [`SpectrumAnalyzer::is_silent`]'s dBFS test.
///
/// Belt and braces, and it earns its place: the analyser's silence test is an
/// RMS one over the whole window, so a lone sub-audible band can keep it false
/// while every bar is under a pixel tall on a 120px box. Redrawing for that is
/// exactly the "content did not change" case rule 1 forbids.
const VISUAL_BAND_FLOOR: f32 = 0.01;

/// One visualiser frame period. Clamped, so a hand-edited `fps = 0` cannot turn
/// [`WidgetEngine::next_deadline`] into a spin.
fn visual_period(fps: u32) -> Duration {
    Duration::from_micros(1_000_000 / u64::from(fps.clamp(1, 60)))
}

/// The visualiser's whole runtime: the capture, the analyser, and what is on
/// screen.
///
/// Exists exactly while the widget is enabled. `capture` is separately optional
/// because "enabled" and "able to run" are different states: a box with neither
/// `pw-cat` nor `parec` installed can enable the widget in the GUI, and the
/// honest response is one log line and then the cost profile of a widget that
/// is off — never a retry loop against a tool that is not going to appear.
struct VisualState {
    /// Preallocated; [`SpectrumAnalyzer::process`] never allocates.
    analyzer: SpectrumAnalyzer,
    /// One frame of audio, sized to roughly the samples that arrive between
    /// frames, so `process` integrates its envelope over the right time step.
    scratch: Vec<f32>,
    /// The capture child and its reader thread. `None` when no capture tool is
    /// installed or the stream would not start.
    capture: Option<AudioCapture>,
    /// Whether anything can produce samples. False makes the widget cost
    /// exactly what a disabled one costs — no reads, no FFT, no deadline.
    live: bool,
    /// What we last pushed. `None` = we believe the overlay is blank.
    ass: Option<String>,
    /// Earliest instant the next frame may be examined. Also what
    /// [`WidgetEngine::next_deadline`] publishes for this widget.
    next_frame: Instant,
}

impl VisualState {
    /// Build the runtime for `cfg`, with `capture` already opened (or not).
    fn new(cfg: &VisualCfg, capture: Option<AudioCapture>, now: Instant) -> Self {
        let rate = capture
            .as_ref()
            .map_or(cfg.sample_rate, AudioCapture::sample_rate);
        let analyzer = SpectrumAnalyzer::new(SpectrumConfig {
            sample_rate: rate,
            fft_size: cfg.fft_size,
            bands: cfg.bands,
            ..SpectrumConfig::default()
        });
        // The audio that arrives between two frames. `read_latest` hands back
        // the most recent N samples, so asking for exactly one frame's worth is
        // what keeps `process`'s time step honest — asking for the whole FFT
        // window every frame would re-integrate the same audio and make the
        // envelope depend on the frame rate.
        let chunk = (rate as usize / cfg.fps.clamp(1, 60) as usize).clamp(64, 8192);
        VisualState {
            analyzer,
            scratch: vec![0.0; chunk],
            live: capture.is_some(),
            capture,
            ass: None,
            next_frame: now,
        }
    }

    /// Copy this frame's audio into `scratch`, returning how many samples
    /// arrived. Zero before the first audio lands, and zero forever without a
    /// capture.
    fn fill(&mut self) -> usize {
        match &self.capture {
            Some(c) => c.read_latest(&mut self.scratch),
            None => 0,
        }
    }

    /// Advance one frame from the `n` samples sitting in `scratch`, pushing at
    /// most one update.
    ///
    /// The tested seam for this widget, in the same spirit as
    /// [`WidgetEngine::tick_at`]: every decision — the rate cap, the silence
    /// verdict, whether the payload actually changed — is a pure function of
    /// the samples and the clock over this struct's own memory, so it can be
    /// driven from a synthesised tone with no audio device anywhere.
    ///
    /// **Rule 1, twice over.** Silence emits one clear and then nothing at all;
    /// audio that renders to a byte-identical payload emits nothing either.
    fn frame(
        &mut self,
        n: usize,
        cfg: &VisualCfg,
        accent: &str,
        now: Instant,
        force: bool,
        out: &mut Vec<WidgetUpdate>,
    ) {
        // Disjoint field borrows: the analyser is written, the scratch read.
        self.analyzer.process(&self.scratch[..n]);
        let silent = self.analyzer.is_silent()
            || visualizer::is_silent(self.analyzer.bands(), VISUAL_BAND_FLOOR);
        if silent {
            // Back off to the silent cadence *before* the early return, so a
            // quiet desktop settles at four looks a second and stays there.
            self.next_frame = now + VISUAL_SILENT_PERIOD;
            if self.ass.take().is_some() {
                out.push(WidgetUpdate::clear(VISUALIZER_OVERLAY));
            }
            return;
        }
        self.next_frame = now + visual_period(cfg.fps);
        let ass = visualizer::render_ass(self.analyzer.bands(), &cfg.style, accent);
        if ass.is_empty() {
            // `render_ass` returns empty for an empty spectrum, which it
            // documents as "clear the overlay" rather than as a `{}` block
            // libass would still have to re-render.
            if self.ass.take().is_some() {
                out.push(WidgetUpdate::clear(VISUALIZER_OVERLAY));
            }
            return;
        }
        if force || self.ass.as_deref() != Some(ass.as_str()) {
            out.push(WidgetUpdate::ass(VISUALIZER_OVERLAY, ass.clone()));
            self.ass = Some(ass);
        }
    }
}

/// Open the audio capture for `cfg`, or `None` with **one** actionable log line.
///
/// Never retried. The two ways this fails are "no capture tool is installed"
/// and "the audio server refused the stream", and neither is fixed by asking
/// again in a second — the first needs a package installed and the second needs
/// the user to look. A retry timer here would be a subprocess spawn every few
/// seconds for the lifetime of the daemon, which is precisely the shape of bug
/// this module's power model exists to prevent. Toggling the widget off and on
/// (or reloading the config) tries again, because that is a person asking.
///
/// Under `cfg(test)` this always returns `None`: CI has no audio server, and a
/// unit test must never open a stream carrying the developer's speakers. That
/// is the same path as "no tool installed", so the degradation is exercised for
/// free — and the render path is driven through
/// [`WidgetEngine::feed_audio`] instead.
#[cfg(not(test))]
fn open_capture(cfg: &VisualCfg) -> Option<AudioCapture> {
    if crate::audio_capture::detect_tool().is_none() {
        log::warn!(
            "widgets: the audio visualiser is enabled but no capture tool is installed — \
             install pipewire-bin (PipeWire) or pulseaudio-utils (PulseAudio / pipewire-pulse); \
             the widget stays off until then"
        );
        return None;
    }
    // Informational only — capture works without it (see its own docs), but a
    // named source is what makes "which device is Fresco listening to?"
    // answerable from the log.
    if let Some(src) = crate::audio_capture::default_monitor_source() {
        log::debug!("widgets: visualiser will follow {src}");
    }
    match AudioCapture::start(cfg.sample_rate, 2) {
        Ok(c) => Some(c),
        Err(e) => {
            log::warn!("widgets: the audio visualiser could not start: {e:#}");
            None
        }
    }
}

#[cfg(test)]
fn open_capture(_cfg: &VisualCfg) -> Option<AudioCapture> {
    None
}

// ---------------------------------------------------------------------------
// The album-art disc
// ---------------------------------------------------------------------------

/// Redraw cap for the spinning disc, in frames per second.
///
/// Deliberately far below the visualiser's, because a disc frame is nothing
/// like an ASS payload: `artwork.rs` measures ~2.8 ms to render a 320px disc,
/// and each frame is then a ~410 KB file write plus an IPC round trip. At 60
/// fps that is 24 MB/s of writes and ~17% of a core to animate a picture whose
/// only content is its angle. At 12 the motion is still smooth — a 33⅓ rpm
/// record turns 16.7° between frames — for a fifth of the cost.
pub const DISC_FPS: u32 = 12;

/// One disc frame period.
const DISC_PERIOD: Duration = Duration::from_millis(1000 / DISC_FPS as u64);

/// How long to wait before trying again after a failed frame write. Long, so a
/// full or read-only runtime directory costs one attempt every two seconds
/// rather than twelve.
const DISC_RETRY: Duration = Duration::from_secs(2);

/// The disc's runtime: the art, where it has got to, and what is on screen.
struct DiscState {
    /// The one file mpv reads pixels from, rewritten in place per frame. See
    /// [`BitmapOverlay::path`] for why it is one file and not a fresh one.
    path: PathBuf,
    /// Prepared source art for the current track, from [`Track::art`].
    art: Option<Arc<RgbaImage>>,
    /// [`Track::seq`] the art belongs to.
    seq: Option<u64>,
    /// Accumulated **playing** time — the input to [`artwork::rotation_for`].
    /// Advanced only while the player is actually playing, which is how rule 6
    /// ("paused ⇒ rotation speed 0 ⇒ no redraw") holds with no special case:
    /// a paused disc computes the same angle it already drew, and
    /// [`artwork::should_redraw`] says no.
    elapsed: Duration,
    /// When `elapsed` was last advanced.
    last: Option<Instant>,
    /// Angle of the frame currently on screen.
    angle: f32,
    /// The frame currently on screen. `None` = we believe nothing is up.
    shown: Option<BitmapOverlay>,
    /// Earliest instant the next frame may be drawn.
    next_frame: Instant,
    /// Something about the art or the geometry changed and the next frame must
    /// be drawn whatever the angle says.
    dirty: bool,
    /// One log line per failing state, not one per frame.
    warned: bool,
}

impl DiscState {
    fn new(now: Instant) -> Self {
        DiscState {
            path: disc_frame_path(),
            art: None,
            seq: None,
            elapsed: Duration::ZERO,
            last: None,
            angle: 0.0,
            shown: None,
            next_frame: now,
            dirty: true,
            warned: false,
        }
    }
}

/// Where the disc's BGRA frame lives.
///
/// Under `$XDG_RUNTIME_DIR/fresco/` beside the control socket: a tmpfs, so the
/// per-frame write never reaches a disk, and per-user, so two people on the
/// same machine cannot collide. [`crate::ipc::socket_dir`] already carries the
/// `/tmp` fallback for a session with no runtime directory.
fn disc_frame_path() -> PathBuf {
    crate::ipc::socket_dir().join("widget-disc.bgra")
}

/// Write one frame to the file mpv reads.
///
/// **Not** `fs::write`, which truncates first: mpv `mmap`s this file for
/// `overlay-add`, and truncating a live mapping out from under it is a SIGBUS
/// in the renderer rather than a dropped frame. The bytes are overwritten in
/// place, and the file is only shortened when the disc has actually been
/// resized — the one case where the old mapping is going away regardless.
fn write_frame(path: &Path, frame: &Bgra) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    f.write_all(&frame.data)?;
    f.set_len(frame.data.len() as u64)?;
    f.flush()
}

/// Resolve an anchor to the top-left corner of a `size`×`size` box on a
/// `out_w`×`out_h` output, in **output pixels**.
///
/// The bitmap twin of the ASS `\an` placement the text widgets get for free.
/// `overlay-add` takes a corner, not an anchor, so somebody has to do this —
/// and doing it here rather than in the three run loops is the whole reason
/// this module exists.
///
/// Saturating throughout: a margin or a disc larger than the output pins the
/// box to the edge instead of wrapping to a nonsense coordinate.
fn anchor_xy(anchor: Anchor, size: u32, margin: u32, out_w: u32, out_h: u32) -> (i32, i32) {
    let far = |extent: u32| -> u32 { extent.saturating_sub(size).saturating_sub(margin) };
    let centre = |extent: u32| -> u32 { extent.saturating_sub(size) / 2 };
    let x = match anchor {
        Anchor::TopLeft | Anchor::MidLeft | Anchor::BottomLeft => margin.min(far(out_w)),
        Anchor::TopCenter | Anchor::MidCenter | Anchor::BottomCenter => centre(out_w),
        Anchor::TopRight | Anchor::MidRight | Anchor::BottomRight => far(out_w),
    };
    let y = match anchor {
        Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => margin.min(far(out_h)),
        Anchor::MidLeft | Anchor::MidCenter | Anchor::MidRight => centre(out_h),
        Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => far(out_h),
    };
    // Both fit in i32 for any output a compositor will ever report.
    (x as i32, y as i32)
}

// ---------------------------------------------------------------------------
// The engine
// ---------------------------------------------------------------------------

/// Everything the daemon knows about on-wallpaper widgets.
///
/// One instance per daemon, not per output: the overlay is a single logical
/// widget layer (see [`config::Widgets::monitor`]), and duplicating the state
/// machine per screen would duplicate the decisions with it. Which renderer the
/// updates are pushed to is the loop's business — [`WidgetEngine::monitor`]
/// answers "which one".
pub struct WidgetEngine {
    /// Owned copy of the lyric config; the daemon reloads `config.toml` under us.
    lyrics_cfg: config::Lyrics,
    clock_cfg: ClockCfg,
    visual_cfg: VisualCfg,
    disc_cfg: DiscWidgetCfg,
    monitor: Option<String>,
    /// Size of the output the widget layer is on, in pixels. Only the disc
    /// needs it — `overlay-add` is not in the ASS coordinate space — and it is
    /// assumed rather than required, so a loop that never calls
    /// [`WidgetEngine::set_output_size`] still gets a disc, just placed against
    /// 1080p.
    out_w: u32,
    out_h: u32,
    /// `#RRGGBB` from the app theme, used by whichever widget has
    /// `accent_follow` set.
    accent: String,
    /// Line selection, offsets, presets and the "unchanged ⇒ [`Action::Idle`]"
    /// fast path all live in here; this module never re-implements any of it.
    lyrics: LyricsRuntime,
    /// `Some` exactly while the lyric widget is enabled.
    worker: Option<Worker>,
    /// [`Track::seq`] of the track currently loaded into `lyrics`.
    track_seq: Option<u64>,
    /// What we last pushed to each overlay. `None` = we believe it is blank.
    /// This is what makes an unchanged tick free, and what [`WidgetEngine::invalidate`]
    /// re-pushes.
    lyrics_ass: Option<String>,
    clock_ass: Option<String>,
    /// `Some` exactly while the visualiser is enabled. Holds the capture, so
    /// dropping it stops recording.
    visual: Option<VisualState>,
    /// `Some` exactly while the disc is enabled.
    disc: Option<DiscState>,
    /// Wall-clock instant at which the clock's *text* next differs. Between now
    /// and then, the clock branch does not render, allocate or compare.
    clock_due: Option<DateTime<Local>>,
    /// Re-push everything on the next tick, whatever we believe is on screen.
    force: bool,
}

impl WidgetEngine {
    /// Build an engine for this config.
    ///
    /// `cfg` is [`config::Config::widgets`] — `None` when the user has never
    /// touched a widget, which is the overwhelmingly common case and must cost
    /// nothing: no thread, no D-Bus, no timer, and no allocation beyond the
    /// accent string.
    ///
    /// `accent_hex` is the app accent as `#RRGGBB`. It is stored rather than
    /// borrowed because the engine outlives any borrow of the config the daemon
    /// is about to replace.
    pub fn new(cfg: Option<&config::Widgets>, accent_hex: &str) -> Self {
        let mut engine = WidgetEngine {
            lyrics_cfg: config::Lyrics::default(),
            clock_cfg: ClockCfg::default(),
            visual_cfg: VisualCfg::default(),
            disc_cfg: DiscWidgetCfg::default(),
            monitor: None,
            out_w: 1920,
            out_h: 1080,
            accent: String::new(),
            lyrics: LyricsRuntime::new(&config::Lyrics::default()),
            worker: None,
            track_seq: None,
            lyrics_ass: None,
            clock_ass: None,
            visual: None,
            disc: None,
            clock_due: None,
            force: false,
        };
        engine.set_config(cfg, accent_hex);
        engine
    }

    /// Adopt a freshly loaded config.
    ///
    /// Call this wherever the daemon re-reads `config.toml` (`Request::Apply` on
    /// every loop). Three things happen here and nowhere else:
    ///
    /// * **The worker starts and stops as `enabled` flips.** Enabling starts the
    ///   thread; disabling stops and joins it, and the next [`tick`](Self::tick)
    ///   takes the overlay down. Nothing is left running for a widget that is
    ///   off.
    /// * **A style change forces a re-render even though the line index has not
    ///   moved.** A preset that only takes effect at the next lyric — or at the
    ///   next minute — reads as broken. (The push still only happens if the
    ///   rendered string actually differs; see [`tick`](Self::tick).)
    /// * **An identical config is ignored.** The GUI rewrites the whole file for
    ///   unrelated edits, and repainting because the user picked a new wallpaper
    ///   is exactly the redraw rule 1 exists to prevent.
    pub fn set_config(&mut self, cfg: Option<&config::Widgets>, accent_hex: &str) {
        let lyrics_cfg = cfg.map(|w| w.lyrics.clone()).unwrap_or_default();
        self.monitor = cfg.and_then(|w| w.monitor.clone());

        if self.accent != accent_hex {
            self.accent = accent_hex.to_string();
            // The lyric runtime takes the accent per tick and notices for
            // itself; the clock is only rendered when due, so it needs telling.
            self.clock_due = None;
        }

        // `LyricsRuntime::set_config` owns the "did anything visible change"
        // decision for lyrics, including ignoring a no-op edit.
        self.lyrics.set_config(&lyrics_cfg);
        self.lyrics_cfg = lyrics_cfg;
        self.sync_worker();
    }

    /// Start, update or stop the now-playing worker to match what is enabled.
    ///
    /// One thread serves both widgets that need a player: the lyric widget
    /// wants the metadata and the playhead, the disc wants the metadata and the
    /// cover art, and running two threads to shell out to the same `gdbus`
    /// would double the idle cost for nothing.
    fn sync_worker(&mut self) {
        let folder = self.lyrics_cfg.folder.clone();
        // `None` here is what stops the worker touching the network for a
        // picture nobody asked for.
        let art = self.disc_cfg.enabled.then_some(self.disc_cfg.size_px);
        if self.lyrics_cfg.enabled || self.disc_cfg.enabled {
            match &self.worker {
                Some(worker) => {
                    worker.set_folder(folder);
                    worker.set_art(art);
                }
                None => self.worker = Some(Worker::start(folder, art)),
            }
        } else if let Some(mut worker) = self.worker.take() {
            // Explicit rather than relying on the field drop, so the join is
            // visible at the place that decided to stop.
            worker.stop();
            self.track_seq = None;
        }
    }

    /// Tell the engine how big the output carrying the widget layer is.
    ///
    /// Only the disc reads this, and it must: `overlay-add` places a bitmap in
    /// **real output pixels**, where the ASS widgets are laid out in the fixed
    /// [`RES_X`]×[`RES_Y`] space and are resolution-independent for free. Call
    /// it once the loop knows its output's mode, and again on a mode or
    /// rotation change — the disc is re-placed and re-pushed on the next tick.
    ///
    /// Ignored when either dimension is zero (a compositor reporting a mode it
    /// has not brought up yet), because a disc placed against a 0×0 output
    /// would land in the corner and stay there.
    pub fn set_output_size(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 || (self.out_w == w && self.out_h == h) {
            return;
        }
        self.out_w = w;
        self.out_h = h;
        if let Some(disc) = &mut self.disc {
            disc.dirty = true;
        }
    }

    /// Adopt the clock's settings.
    ///
    /// Separate from [`set_config`](Self::set_config) only because the clock has
    /// no config block yet — see [`ClockCfg`]. `None` disables it, which is also
    /// the state a never-called engine is in, so a loop that ignores this method
    /// entirely gets no clock and pays nothing for it.
    pub fn set_clock(&mut self, cfg: Option<&ClockCfg>) {
        let cfg = cfg.cloned().unwrap_or_default();
        if cfg == self.clock_cfg {
            return;
        }
        self.clock_cfg = cfg;
        // Anything here can change the text or the look, and a change that only
        // took effect at the next minute boundary would read as a dead switch.
        self.clock_due = None;
    }

    /// Adopt the audio visualiser's settings. `None` disables it.
    ///
    /// **This is where recording starts and stops.** Enabling opens a capture
    /// of the system's audio output; disabling drops it, which kills the child
    /// process and joins its reader thread. Nothing is left listening for a
    /// widget that is off, and an identical config is ignored rather than
    /// restarting the stream — the GUI rewrites the whole file for unrelated
    /// edits, and a capture that respawned every time the user picked a
    /// wallpaper would show up as a gap in the bars.
    ///
    /// If no capture tool is installed, or the stream will not start, this logs
    /// once with the actionable reason and the widget behaves as disabled. It
    /// is **not** retried on a timer; the next config change tries again.
    pub fn set_visualizer(&mut self, cfg: Option<&VisualCfg>) {
        let cfg = cfg.cloned().unwrap_or_default();
        if cfg == self.visual_cfg {
            return;
        }
        // Anything that changes the analysis (rate, size, band count, frame
        // rate) needs a fresh runtime, and so does anything that left us
        // without a capture — including a previous *disable*, which drops the
        // capture the moment it is asked to and would otherwise leave the
        // widget switched on and permanently deaf. A style-only edit keeps
        // both, because restarting the stream would show up as a gap in the
        // bars.
        let restart = self.visual.as_ref().is_none_or(|v| v.capture.is_none())
            || cfg.sample_rate != self.visual_cfg.sample_rate
            || cfg.fft_size != self.visual_cfg.fft_size
            || cfg.bands != self.visual_cfg.bands
            || cfg.fps != self.visual_cfg.fps;
        self.visual_cfg = cfg;
        if !self.visual_cfg.enabled {
            if let Some(v) = &mut self.visual {
                // Stop recording *now*, not at the next tick: dropping the
                // capture kills the child and joins its reader. The rest of the
                // state outlives it by exactly one tick, which is what takes
                // the overlay down before the runtime is thrown away.
                v.capture = None;
                v.live = false;
            }
            return;
        }
        if restart {
            let now = Instant::now();
            let capture = self
                .visual
                .take()
                .and_then(|v| v.capture)
                .or_else(|| open_capture(&self.visual_cfg));
            self.visual = Some(VisualState::new(&self.visual_cfg, capture, now));
        } else if let Some(v) = &mut self.visual {
            // Style-only edit: repaint at the next frame rather than waiting
            // for the spectrum to happen to differ.
            v.ass = None;
        }
    }

    /// Adopt the album-art disc's settings. `None` disables it.
    ///
    /// Enabling starts the now-playing worker if it is not already running, and
    /// asks it to fetch cover art for the **current** track rather than waiting
    /// for the next one. Disabling stops the art fetching, takes the overlay
    /// down on the next tick, and stops the worker too if the lyric widget is
    /// not also using it.
    pub fn set_disc(&mut self, cfg: Option<&DiscWidgetCfg>) {
        let cfg = cfg.copied().unwrap_or_default();
        if cfg == self.disc_cfg {
            return;
        }
        let now = Instant::now();
        self.disc_cfg = cfg;
        if self.disc_cfg.enabled {
            match &mut self.disc {
                // Size, anchor, margin, opacity and spin all change the frame,
                // and a change that only took effect at the next track would
                // read as a dead switch.
                Some(disc) => disc.dirty = true,
                None => self.disc = Some(DiscState::new(now)),
            }
        }
        self.sync_worker();
    }

    /// Whether any widget is enabled. `false` means every other method is a
    /// no-op and the loop can skip its widget block entirely.
    pub fn is_active(&self) -> bool {
        self.lyrics_cfg.enabled
            || self.clock_cfg.enabled
            || self.visual_cfg.enabled
            || self.disc_cfg.enabled
    }

    /// Which output the widget layer belongs on — [`config::Widgets::monitor`],
    /// i.e. a connector name like `"DP-1"`, or `None` for the primary (or first)
    /// output. The loops already know each renderer's connector.
    pub fn monitor(&self) -> Option<&str> {
        self.monitor.as_deref()
    }

    /// The latest now-playing snapshot, or `None` when neither the lyric widget
    /// nor the disc is on and no worker exists.
    ///
    /// Exposed for status/logging; [`tick`](Self::tick) does not need the caller
    /// to fetch it.
    pub fn now_playing(&self) -> Option<Snapshot> {
        self.worker.as_ref().map(Worker::snapshot)
    }

    /// Advance every widget and return **only** the overlays whose content
    /// changed.
    ///
    /// An unchanged frame returns an empty `Vec`, which does not allocate. That
    /// is the whole power story: a lyric line held for eight seconds is one
    /// update out of eighty ticks, and a clock reading `14:32` produces nothing
    /// at all until 14:33 — the clock is not even *rendered* in between, so
    /// there is no string built and nothing compared.
    ///
    /// The one push that is not a content change: the first tick after the
    /// engine starts (or after [`clear_all`](Self::clear_all)) blanks the lyric
    /// overlay once, because we cannot assume an overlay left by a previous
    /// daemon run is gone. Exactly one, then silence.
    pub fn tick(&mut self) -> Vec<WidgetUpdate> {
        if !self.lyrics_live() && !self.clock_live() && !self.visual_live() && !self.disc_live() {
            return Vec::new();
        }
        let snapshot = if self.lyrics_live() || self.disc_live() {
            self.now_playing()
        } else {
            None
        };
        // Read a clock only for a widget that is switched on.
        let wall = self.clock_live().then(Local::now);
        self.tick_at(snapshot.as_ref(), Instant::now(), wall)
    }

    /// The earliest instant anything this engine owns will change, or `None`
    /// when nothing is pending.
    ///
    /// **Smart Sleep.** Both sources of change are schedules known in advance —
    /// `.lrc` timestamps and the next minute boundary — so the loop can wait on
    /// this deadline (`recv_timeout`, or its sleep clamped to it) instead of
    /// ticking. A 30s instrumental gap costs one wake, not 300.
    ///
    /// Two things the caller owns, because this function cannot:
    ///
    /// * **The wait must be interruptible.** All three loops already wait on
    ///   their command channel, which is exactly the right primitive: clamp that
    ///   timeout to this deadline and an IPC request still lands immediately.
    /// * **The result is advisory.** A pause, a seek or a track change
    ///   invalidates it, and none of those go through this engine — which is why
    ///   the loop should take `min(its own tick, this)` rather than sleeping
    ///   here unconditionally. The lyric deadline is only armed while playback
    ///   is actually running, so a paused song contributes nothing.
    pub fn next_deadline(&self) -> Option<Instant> {
        let snapshot = if self.lyrics_cfg.enabled || self.disc_cfg.enabled {
            self.now_playing()
        } else {
            None
        };
        let wall = self.clock_cfg.enabled.then(Local::now);
        self.next_deadline_at(snapshot.as_ref(), Instant::now(), wall)
    }

    /// Updates that blank every overlay this engine owns.
    ///
    /// The loop calls this on wallpaper swap, renderer teardown and before an
    /// output respawn, so an overlay can never leak onto the next wallpaper.
    ///
    /// Overlays are blanked **unconditionally**, including ones we believe are
    /// already empty: the reason to call this is precisely that our belief about
    /// what the renderer is showing may be wrong. The lyric runtime is reset
    /// with it, so the next [`tick`](Self::tick) re-establishes the current line
    /// from the worker's snapshot rather than staying dark until the next song.
    pub fn clear_all(&mut self) -> Vec<WidgetUpdate> {
        let mut out = Vec::new();
        if self.lyrics_cfg.enabled || self.lyrics_ass.is_some() {
            out.push(WidgetUpdate::clear(LYRICS_OVERLAY));
        }
        if self.clock_cfg.enabled || self.clock_ass.is_some() {
            out.push(WidgetUpdate::clear(CLOCK_OVERLAY));
        }
        if self.visual_cfg.enabled || self.visual.as_ref().is_some_and(|v| v.ass.is_some()) {
            out.push(WidgetUpdate::clear(VISUALIZER_OVERLAY));
        }
        if self.disc_cfg.enabled || self.disc.as_ref().is_some_and(|d| d.shown.is_some()) {
            // `overlay-remove`, not an empty ASS payload: a bitmap overlay is a
            // different mpv command and an empty `osd-overlay` would leave the
            // record sitting on the next wallpaper.
            out.push(WidgetUpdate::remove(DISC_OVERLAY));
        }
        self.lyrics_ass = None;
        self.clock_ass = None;
        self.clock_due = None;
        if let Some(v) = &mut self.visual {
            v.ass = None;
        }
        if let Some(d) = &mut self.disc {
            d.shown = None;
            // Back to "we have pushed nothing": the next tick must draw the
            // disc again rather than decide the angle has not moved enough.
            d.dirty = true;
        }
        // Back to "we have pushed nothing", not to "the overlay is empty": the
        // next tick must re-adopt the track rather than assume it still holds.
        self.lyrics.clear();
        self.track_seq = None;
        self.force = false;
        out
    }

    /// Force a full re-push on the next [`tick`](Self::tick).
    ///
    /// For the cases where the renderer lost our overlays without us clearing
    /// them: a respawned mpv (which starts with none), and a rotation change
    /// (W0: the OSD coordinate space follows the video's render area, so the
    /// payload must be pushed again against the new one).
    ///
    /// Only overlays that *have* content are re-pushed — re-sending a blank to a
    /// renderer that never had the overlay is a wasted IPC round trip, and after
    /// [`clear_all`](Self::clear_all) there is by definition nothing to restore.
    pub fn invalidate(&mut self) {
        self.force = true;
        // Make the clock render on the next tick rather than at its next
        // minute boundary; the push itself is still gated on `force`.
        self.clock_due = None;
        // Same for the two rate-capped widgets: a respawned mpv must not wait
        // out a frame period before the bars and the record come back.
        let now = Instant::now();
        if let Some(v) = &mut self.visual {
            v.next_frame = now;
        }
        if let Some(d) = &mut self.disc {
            d.next_frame = now;
            d.dirty = true;
        }
    }

    // -- the tested seam ----------------------------------------------------

    /// Drive one visualiser frame from `samples` instead of from a capture.
    ///
    /// The audio counterpart of handing `tick_at` a [`Snapshot`]: it is the
    /// only way to exercise the render, silence and rate-cap decisions on a
    /// machine with no audio server, which is every CI machine and — by
    /// [`open_capture`]'s deliberate `cfg(test)` arm — every test run.
    ///
    /// Marking the runtime `live` is part of the fake: it is what a real
    /// capture would have set, and without it [`Self::next_deadline_at`] would
    /// correctly report that this widget has nothing to wake for.
    #[cfg(test)]
    fn feed_audio(&mut self, samples: &[f32], now: Instant) -> Vec<WidgetUpdate> {
        let mut out = Vec::new();
        let force = self.force;
        let Some(v) = &mut self.visual else {
            return out;
        };
        v.live = true;
        if !force && now < v.next_frame {
            return out;
        }
        let n = samples.len().min(v.scratch.len());
        v.scratch[..n].copy_from_slice(&samples[..n]);
        v.frame(
            n,
            &self.visual_cfg,
            self.accent.as_str(),
            now,
            force,
            &mut out,
        );
        self.force = false;
        out
    }

    /// [`tick`](Self::tick) with its inputs handed in.
    ///
    /// Every decision this engine makes is in here, as a pure function of
    /// (snapshot, monotonic now, wall clock) over the engine's own memory — so
    /// the whole thing is testable with no D-Bus, no desktop, no player and no
    /// sleeping. `wall` is `None` when the caller knows the clock widget is off.
    fn tick_at(
        &mut self,
        snapshot: Option<&Snapshot>,
        now: Instant,
        wall: Option<DateTime<Local>>,
    ) -> Vec<WidgetUpdate> {
        let mut out = Vec::new();

        if self.lyrics_live() {
            self.adopt_track(snapshot);
            let (position_us, status) = match snapshot {
                Some(s) => (s.clock.predicted_us(now), s.clock.status()),
                // No worker and no snapshot is the same situation as a stopped
                // player, and the runtime already freezes on that.
                None => (0, PlaybackStatus::Stopped),
            };
            match self.lyrics.tick(position_us, status, self.accent.as_str()) {
                Action::Show(ass) => {
                    self.lyrics_ass = Some(ass.clone());
                    out.push(WidgetUpdate::ass(LYRICS_OVERLAY, ass));
                }
                Action::Clear => {
                    self.lyrics_ass = None;
                    out.push(WidgetUpdate::clear(LYRICS_OVERLAY));
                }
                Action::Idle => {
                    if self.force {
                        if let Some(ass) = &self.lyrics_ass {
                            out.push(WidgetUpdate::ass(LYRICS_OVERLAY, ass.clone()));
                        }
                    }
                }
            }
        }

        if let Some(wall) = wall {
            if self.clock_cfg.enabled {
                let period = clock::tick_secs(&self.clock_cfg.style);
                if self.force || clock_is_due(self.clock_due, wall, period) {
                    let ass = clock::render_ass(wall, &self.clock_cfg.style, self.accent.as_str());
                    self.clock_due = Some(clock::next_change(wall, &self.clock_cfg.style));
                    if self.force || self.clock_ass.as_deref() != Some(ass.as_str()) {
                        out.push(WidgetUpdate::ass(CLOCK_OVERLAY, ass.clone()));
                        self.clock_ass = Some(ass);
                    }
                }
            } else if self.clock_ass.take().is_some() {
                // Switched off while it was on screen: take it down, once.
                self.clock_due = None;
                out.push(WidgetUpdate::clear(CLOCK_OVERLAY));
            }
        }

        self.visual_tick(now, &mut out);
        self.disc_tick(snapshot, now, &mut out);

        self.force = false;
        out
    }

    /// Advance the visualiser. See [`VisualState::frame`] for the decisions.
    ///
    /// Three ways to cost nothing, in order: switched off (no state at all), no
    /// capture behind it (one `bool` test), and not yet due (one `Instant`
    /// comparison — no read, no FFT, no allocation). Only past all three does
    /// this touch the audio.
    fn visual_tick(&mut self, now: Instant, out: &mut Vec<WidgetUpdate>) {
        let force = self.force;
        let Some(v) = &mut self.visual else { return };
        if !self.visual_cfg.enabled {
            // Switched off while it was on screen: take it down, once, and only
            // then throw the runtime away.
            if v.ass.take().is_some() {
                out.push(WidgetUpdate::clear(VISUALIZER_OVERLAY));
            }
            self.visual = None;
            return;
        }
        if !v.live {
            // Enabled, but there is no capture tool on this machine. Already
            // logged once by `open_capture`; from here it costs what a disabled
            // widget costs.
            return;
        }
        // A capture that died (the sink was switched, PipeWire restarted) is
        // reported once and then left alone — see `open_capture` on why this is
        // not a retry loop.
        if v.capture.as_ref().is_some_and(|c| !c.is_alive()) {
            let why = v
                .capture
                .as_ref()
                .and_then(AudioCapture::last_error)
                .unwrap_or_else(|| "the capture process exited".to_string());
            log::warn!("widgets: the audio visualiser stopped: {why}");
            v.capture = None;
            v.live = false;
            if v.ass.take().is_some() {
                out.push(WidgetUpdate::clear(VISUALIZER_OVERLAY));
            }
            return;
        }
        if !force && now < v.next_frame {
            return;
        }
        let n = v.fill();
        v.frame(n, &self.visual_cfg, self.accent.as_str(), now, force, out);
    }

    /// Advance the album-art disc.
    ///
    /// The redraw gate is four questions, cheapest first, and all four have to
    /// say no for the frame to be free: is anything forced or dirty, has the
    /// geometry moved, is the frame period up, and has the record actually
    /// turned far enough to see ([`artwork::should_redraw`])? A paused player
    /// fails the last one forever, because `elapsed` stops advancing and the
    /// angle it computes is the angle already on screen.
    fn disc_tick(
        &mut self,
        snapshot: Option<&Snapshot>,
        now: Instant,
        out: &mut Vec<WidgetUpdate>,
    ) {
        let force = self.force;
        let cfg = self.disc_cfg;
        let (out_w, out_h) = (self.out_w, self.out_h);
        let Some(d) = &mut self.disc else { return };
        if !cfg.enabled {
            if d.shown.take().is_some() {
                out.push(WidgetUpdate::remove(DISC_OVERLAY));
            }
            self.disc = None;
            return;
        }

        // -- adopt this track's art, once per track -------------------------
        match snapshot.and_then(|s| s.track.as_ref()) {
            Some(track) => {
                if d.seq != Some(track.seq) {
                    d.seq = Some(track.seq);
                    d.art = track.art.clone();
                    // A new record goes on at the top rather than continuing
                    // the last one's angle. `last` goes with it: the time since
                    // the previous tick belongs to the previous track, and
                    // carrying it over would spin the new one forward by a
                    // whole tick's worth on its first frame.
                    d.elapsed = Duration::ZERO;
                    d.last = None;
                    d.angle = 0.0;
                    d.dirty = true;
                }
            }
            None => {
                if d.seq.take().is_some() {
                    d.art = None;
                }
            }
        }

        // -- how far has it turned? -----------------------------------------
        let status = snapshot.map_or(PlaybackStatus::Stopped, |s| s.clock.status());
        let spinning = cfg.spin && status == PlaybackStatus::Playing;
        if let Some(last) = d.last {
            if spinning {
                d.elapsed += now.saturating_duration_since(last);
            }
        }
        d.last = Some(now);

        let Some(art) = d.art.clone() else {
            // No player, so no record. Down it comes, once.
            if d.shown.take().is_some() {
                out.push(WidgetUpdate::remove(DISC_OVERLAY));
            }
            return;
        };

        let size = cfg.size_px.clamp(1, artwork::MAX_DISC_PX);
        let (x, y) = anchor_xy(cfg.anchor, size, cfg.margin_px, out_w, out_h);
        let angle = if cfg.spin {
            artwork::rotation_for(d.elapsed, artwork::VINYL_RPM)
        } else {
            0.0
        };
        let moved = now >= d.next_frame
            && artwork::should_redraw(d.angle, angle, artwork::DEFAULT_MIN_STEP_DEG);
        let misplaced = d
            .shown
            .as_ref()
            .is_none_or(|b| (b.x, b.y, b.w) != (x, y, size));
        if !(force || d.dirty || misplaced || moved) {
            return;
        }

        let frame = artwork::render_disc(
            &art,
            &DiscCfg {
                size_px: size,
                rotation_deg: angle,
                opacity: cfg.opacity,
                ..DiscCfg::default()
            },
        );
        if let Err(e) = write_frame(&d.path, &frame) {
            if !d.warned {
                d.warned = true;
                log::warn!(
                    "widgets: cannot write the album-art frame to {}: {e} — the disc stays hidden",
                    d.path.display()
                );
            }
            d.next_frame = now + DISC_RETRY;
            return;
        }
        d.warned = false;
        d.angle = angle;
        d.dirty = false;
        d.next_frame = now + DISC_PERIOD;
        let bitmap = BitmapOverlay {
            x,
            y,
            path: d.path.clone(),
            w: frame.w,
            h: frame.h,
            stride: frame.stride(),
        };
        d.shown = Some(bitmap.clone());
        out.push(WidgetUpdate::draw(DISC_OVERLAY, bitmap));
    }

    /// [`next_deadline`](Self::next_deadline) with its inputs handed in. See
    /// `tick_at` for why the seam exists.
    fn next_deadline_at(
        &self,
        snapshot: Option<&Snapshot>,
        now: Instant,
        wall: Option<DateTime<Local>>,
    ) -> Option<Instant> {
        let mut best: Option<Instant> = None;

        if self.lyrics_cfg.enabled {
            // Only while the playhead is actually moving: the unit of
            // `next_deadline_us` is playback time, and a paused clock never
            // reaches it. A pause is an event the loop hears about anyway.
            if let Some(s) = snapshot.filter(|s| s.clock.is_running()) {
                let position_us = s.clock.predicted_us(now);
                if let Some(playback_us) = self.lyrics.next_deadline_us(position_us) {
                    // Playback microseconds are not real ones at rate != 1.0.
                    // `is_running` guarantees a positive rate.
                    let real_us = (playback_us as f64 / s.clock.rate()).clamp(1.0, MAX_WAIT_US);
                    best = min_instant(best, now + Duration::from_micros(real_us as u64));
                }
            }
        }

        if self.clock_cfg.enabled {
            if let Some(wall) = wall {
                let period = clock::tick_secs(&self.clock_cfg.style);
                let wait = match self.clock_due {
                    Some(due) if !clock_is_due(Some(due), wall, period) => due
                        .signed_duration_since(wall)
                        .to_std()
                        .unwrap_or(Duration::ZERO),
                    // Never rendered, or already overdue: the next tick changes it.
                    _ => Duration::ZERO,
                };
                best = min_instant(best, now + wait);
            }
        }

        // The visualiser has no schedule to sleep until, so it publishes its
        // own rate cap — 24 Hz with audio, 4 Hz in silence, and *nothing* when
        // it is off or has no capture behind it, which is what keeps an
        // unusable visualiser exactly as cheap as a disabled one.
        if self.visual_cfg.enabled {
            if let Some(v) = self.visual.as_ref().filter(|v| v.live) {
                best = min_instant(best, v.next_frame);
            }
        }

        // The disc publishes its cap only while it is actually turning. Paused
        // is rule 6: rotation speed 0, no next frame, nothing to wake for. The
        // one exception is a disc that has art but has never been drawn, which
        // must not wait for a play event that may never come.
        if self.disc_cfg.enabled {
            if let Some(d) = self.disc.as_ref().filter(|d| d.art.is_some()) {
                let spinning = self.disc_cfg.spin
                    && snapshot.map_or(PlaybackStatus::Stopped, |s| s.clock.status())
                        == PlaybackStatus::Playing;
                if spinning || d.shown.is_none() {
                    best = min_instant(best, d.next_frame.max(now));
                }
            }
        }

        best
    }

    /// Whether the lyric branch has anything to do: it is enabled, or it still
    /// has an overlay of ours to take down.
    fn lyrics_live(&self) -> bool {
        self.lyrics_cfg.enabled || self.lyrics_ass.is_some()
    }

    /// The same question for the clock.
    fn clock_live(&self) -> bool {
        self.clock_cfg.enabled || self.clock_ass.is_some()
    }

    /// The same question for the visualiser.
    fn visual_live(&self) -> bool {
        self.visual_cfg.enabled || self.visual.is_some()
    }

    /// The same question for the disc.
    fn disc_live(&self) -> bool {
        self.disc_cfg.enabled || self.disc.is_some()
    }

    /// Hand the worker's track to the lyric runtime, once per track.
    ///
    /// Keyed on [`Track::seq`] rather than on the metadata, so the `Vec<LrcLine>`
    /// clone happens once per song instead of once per tick — and so a player
    /// re-announcing the same track (late album art, a rating edit, some clients
    /// on every volume change) costs nothing.
    fn adopt_track(&mut self, snapshot: Option<&Snapshot>) {
        match snapshot.and_then(|s| s.track.as_ref()) {
            Some(track) => {
                if self.track_seq != Some(track.seq) {
                    self.track_seq = Some(track.seq);
                    self.lyrics
                        .track_changed(&track.now_playing, track.lyrics.clone());
                }
            }
            None => {
                if self.track_seq.is_some() {
                    self.track_seq = None;
                    self.lyrics.clear();
                }
            }
        }
    }
}

/// Ceiling on a computed wait, in microseconds — about a day. Guards the
/// float→integer conversion against a hand-edited `[99999999:00]` timestamp
/// turning into a nonsense [`Duration`].
const MAX_WAIT_US: f64 = 86_400e6;

/// The smaller of two optional instants.
fn min_instant(a: Option<Instant>, b: Instant) -> Option<Instant> {
    Some(match a {
        Some(a) => a.min(b),
        None => b,
    })
}

/// Whether the clock's text needs re-rendering at `wall`.
///
/// `None` means "never rendered", which is always due. The second arm is the
/// wall-clock-went-backwards guard: `next_change` is at most one period ahead,
/// so a deadline further away than that means the system clock moved (NTP step,
/// a manual change, a resume) and the displayed time is now wrong in a way
/// waiting will not fix.
fn clock_is_due(due: Option<DateTime<Local>>, wall: DateTime<Local>, period_secs: u32) -> bool {
    let Some(due) = due else {
        return true;
    };
    let ahead = due.signed_duration_since(wall);
    ahead <= TimeDelta::zero() || ahead > TimeDelta::seconds(i64::from(period_secs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const ACCENT: &str = "#3584E4";

    // -- fixtures -----------------------------------------------------------

    /// A wall-clock instant on a fixed date. Mid-July 2026 is far from every DST
    /// transition in the IANA database, so these tests give the same answers
    /// whatever `TZ` the machine running them is set to (the same reasoning, and
    /// the same date, as `clock`'s own tests).
    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 7, 15, h, m, s)
            .earliest()
            .expect("2026-07-15 has no DST gap in any timezone")
    }

    /// A widgets block with only the lyric switch set.
    ///
    /// `..Default::default()` rather than a full literal on purpose: this block
    /// is growing a key per widget, and a test that has to be edited every time
    /// one lands is a test that will be edited carelessly.
    fn widgets(enabled: bool) -> config::Widgets {
        config::Widgets {
            lyrics: config::Lyrics {
                enabled,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn clock_cfg() -> ClockCfg {
        ClockCfg {
            enabled: true,
            style: ClockStyle::default(),
        }
    }

    /// Two lines far apart, so a hundred ticks can pass inside one of them.
    fn fixture() -> Vec<LrcLine> {
        lyrics::parse_lrc("[00:10.00]a\n[01:00.00]b")
    }

    /// A snapshot of a player at `position_us`, anchored at `now`.
    ///
    /// Built through the real [`PositionClock`] rather than by hand: an
    /// out-of-threshold resync snaps, so the predicted position at `now` is
    /// exactly `position_us` and the tests stay honest about the clock they are
    /// driving.
    ///
    /// The title is derived from `seq` because the two must agree: the worker
    /// bumps [`Track::seq`] only when [`NowPlaying::same_track`] says the track
    /// really changed, and [`LyricsRuntime::track_changed`] applies that same
    /// test again to ignore a re-announcement. A fixture with a moving `seq` and
    /// a fixed title would be testing a state the worker cannot produce — and
    /// would quietly pass while the runtime ignored every "new" track.
    fn snapshot_at(
        now: Instant,
        lines: Option<Vec<LrcLine>>,
        position_us: i64,
        status: PlaybackStatus,
        seq: u64,
    ) -> Snapshot {
        snapshot_full(now, lines, None, position_us, status, seq)
    }

    /// [`snapshot_at`] with cover art, for the disc.
    ///
    /// The art is [`artwork::placeholder_art`], which is generated rather than
    /// loaded — so every disc test runs with no network, no files and no
    /// player, exactly as the worker would have handed it over after a failed
    /// art load.
    fn snapshot_with_art(
        now: Instant,
        position_us: i64,
        status: PlaybackStatus,
        seq: u64,
    ) -> Snapshot {
        let art = Arc::new(artwork::placeholder_art(32));
        snapshot_full(now, None, Some(art), position_us, status, seq)
    }

    fn snapshot_full(
        now: Instant,
        lines: Option<Vec<LrcLine>>,
        art: Option<Arc<RgbaImage>>,
        position_us: i64,
        status: PlaybackStatus,
        seq: u64,
    ) -> Snapshot {
        let mut clock = PositionClock::new(now);
        clock.set_status(status, now);
        clock.resync(position_us, now);
        Snapshot {
            track: Some(Arc::new(Track {
                now_playing: NowPlaying {
                    title: format!("song {seq}"),
                    ..Default::default()
                },
                lyrics: lines,
                art,
                seq,
            })),
            clock,
            position_unreliable: false,
        }
    }

    fn us(secs: f64) -> i64 {
        (secs * 1e6).round() as i64
    }

    /// The payload pushed to `overlay`, or a failure naming what did come back.
    fn payload(updates: &[WidgetUpdate], overlay: u32) -> &str {
        updates
            .iter()
            .find(|u| u.overlay_id == overlay)
            .map(|u| u.ass.as_str())
            .unwrap_or_else(|| panic!("no update for overlay {overlay} in {updates:?}"))
    }

    // -- nothing enabled ----------------------------------------------------

    #[test]
    fn a_disabled_engine_does_nothing_and_starts_nothing() {
        // The default state of every Fresco install: `widgets` is absent from
        // config.toml entirely. It must cost nothing at all — no thread, no
        // D-Bus, no timer, no updates.
        for cfg in [None, Some(&widgets(false))] {
            let mut engine = WidgetEngine::new(cfg, ACCENT);
            assert!(!engine.is_active());
            assert!(engine.worker.is_none(), "a thread was started");
            assert!(engine.now_playing().is_none());
            assert_eq!(engine.next_deadline(), None);
            for _ in 0..100 {
                assert!(engine.tick().is_empty());
            }
            // And nothing to take down either.
            assert!(engine.clear_all().is_empty());
            // Even asked to re-push, there is nothing to re-push.
            engine.invalidate();
            assert!(engine.tick().is_empty());
        }
    }

    #[test]
    fn the_monitor_comes_straight_from_the_config() {
        let mut cfg = widgets(true);
        assert_eq!(WidgetEngine::new(Some(&cfg), ACCENT).monitor(), None);
        cfg.monitor = Some("DP-1".to_string());
        assert_eq!(
            WidgetEngine::new(Some(&cfg), ACCENT).monitor(),
            Some("DP-1")
        );
    }

    // -- rule 1: never redraw unless content changed -------------------------

    #[test]
    fn an_unchanged_lyric_line_is_pushed_exactly_once() {
        // The whole power story in one test. The daemon ticks ten times a
        // second; a lyric is up for seconds at a time.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let t0 = Instant::now();

        // The establishing push: we cannot assume an overlay left by a previous
        // daemon run is gone, so exactly one blank goes out first.
        let first = engine.tick_at(None, t0, None);
        assert_eq!(first, vec![WidgetUpdate::clear(LYRICS_OVERLAY)]);
        assert!(engine.tick_at(None, t0, None).is_empty());

        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let shown = engine.tick_at(Some(&snap), t0, None);
        assert_eq!(shown.len(), 1);
        assert!(payload(&shown, LYRICS_OVERLAY).ends_with("}a"));

        // A hundred consecutive ticks across the rest of the line's life, at the
        // daemon's real 100ms cadence: every one of them free.
        for step in 1..=100 {
            let now = t0 + Duration::from_millis(100 * step);
            let updates = engine.tick_at(Some(&snap), now, None);
            assert!(updates.is_empty(), "tick {step} pushed {updates:?}");
        }

        // The next line lands once, and is then silent in its turn.
        let now = t0 + Duration::from_secs(50);
        let next = engine.tick_at(Some(&snap), now, None);
        assert!(payload(&next, LYRICS_OVERLAY).ends_with("}b"));
        assert!(engine
            .tick_at(Some(&snap), now + Duration::from_millis(100), None)
            .is_empty());
    }

    #[test]
    fn the_clock_pushes_once_a_minute_and_not_once_a_tick() {
        // Rule 7's budget: one redraw per minute unless seconds are enabled.
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();

        let first = engine.tick_at(None, t0, Some(at(14, 32, 0)));
        assert_eq!(first.len(), 1);
        assert!(payload(&first, CLOCK_OVERLAY).ends_with("14:32"));

        // Every tick for the rest of the minute — a hundred of them, plus the
        // last instant before the boundary — produces nothing.
        for step in 0..100 {
            let wall = at(14, 32, 0) + TimeDelta::milliseconds(step * 590);
            assert!(
                engine.tick_at(None, t0, Some(wall)).is_empty(),
                "pushed at {wall}"
            );
        }
        assert!(engine.tick_at(None, t0, Some(at(14, 32, 59))).is_empty());

        // And exactly one at the boundary.
        let next = engine.tick_at(None, t0, Some(at(14, 33, 0)));
        assert_eq!(next.len(), 1);
        assert!(payload(&next, CLOCK_OVERLAY).ends_with("14:33"));
        assert!(engine.tick_at(None, t0, Some(at(14, 33, 1))).is_empty());
    }

    #[test]
    fn the_clock_recovers_from_a_backwards_wall_clock() {
        // An NTP step or a resume can leave the deadline further ahead than a
        // whole period, at which point waiting for it would show the wrong time
        // for as long as the jump was.
        assert!(clock_is_due(None, at(14, 32, 0), 60));
        assert!(clock_is_due(Some(at(14, 32, 0)), at(14, 32, 0), 60));
        assert!(!clock_is_due(Some(at(14, 33, 0)), at(14, 32, 30), 60));
        assert!(clock_is_due(Some(at(14, 33, 0)), at(12, 0, 0), 60));

        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        assert_eq!(engine.tick_at(None, t0, Some(at(14, 32, 30))).len(), 1);
        let back = engine.tick_at(None, t0, Some(at(12, 0, 0)));
        assert!(payload(&back, CLOCK_OVERLAY).ends_with("12:00"));
    }

    #[test]
    fn a_style_change_repaints_without_waiting_for_the_next_change() {
        // A preset that only takes effect at the next lyric line, or the next
        // minute, reads as a dead switch.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let wall = Some(at(14, 32, 30));
        let first = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(first.len(), 2);
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());

        // Lyrics: a new anchor moves the line.
        let mut cfg = widgets(true);
        cfg.lyrics.anchor = config::LyricAnchor::TopLeft;
        engine.set_config(Some(&cfg), ACCENT);
        let moved = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(moved.len(), 1);
        assert!(payload(&moved, LYRICS_OVERLAY).contains("\\an7"));

        // Clock: a new theme changes the payload mid-minute.
        engine.set_clock(Some(&ClockCfg {
            enabled: true,
            style: ClockStyle {
                theme: crate::clock::ClockTheme::Wordy,
                ..Default::default()
            },
        }));
        let themed = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(themed.len(), 1);
        assert!(payload(&themed, CLOCK_OVERLAY).ends_with("half past two"));

        // Re-applying the same config is not a change and must not repaint.
        engine.set_config(Some(&cfg), ACCENT);
        engine.set_clock(Some(&ClockCfg {
            enabled: true,
            style: ClockStyle {
                theme: crate::clock::ClockTheme::Wordy,
                ..Default::default()
            },
        }));
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());
    }

    #[test]
    fn an_accent_change_repaints_both_widgets() {
        let mut cfg = widgets(true);
        cfg.lyrics.accent_follow = true;
        let mut engine = WidgetEngine::new(Some(&cfg), "#FF8800");
        engine.set_clock(Some(&ClockCfg {
            enabled: true,
            style: ClockStyle {
                accent_follow: true,
                ..Default::default()
            },
        }));
        let t0 = Instant::now();
        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let wall = Some(at(14, 32, 30));
        let first = engine.tick_at(Some(&snap), t0, wall);
        assert!(payload(&first, LYRICS_OVERLAY).contains("\\1c&H0088FF&"));
        assert!(payload(&first, CLOCK_OVERLAY).contains("\\1c&H0088FF&"));

        engine.set_config(Some(&cfg), ACCENT);
        let retinted = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(retinted.len(), 2);
        assert!(payload(&retinted, LYRICS_OVERLAY).contains("\\1c&HE48435&"));
        assert!(payload(&retinted, CLOCK_OVERLAY).contains("\\1c&HE48435&"));
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());
    }

    // -- tracks -------------------------------------------------------------

    #[test]
    fn a_new_track_reloads_the_lyrics_and_a_re_announcement_does_not() {
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let t0 = Instant::now();
        let first = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        assert!(payload(&engine.tick_at(Some(&first), t0, None), LYRICS_OVERLAY).ends_with("}a"));

        // Same seq, same everything: the worker is telling us nothing new.
        assert!(engine.tick_at(Some(&first), t0, None).is_empty());

        // A different track with no lyrics takes the overlay down, once.
        let second = snapshot_at(t0, None, us(10.0), PlaybackStatus::Playing, 2);
        assert_eq!(
            engine.tick_at(Some(&second), t0, None),
            vec![WidgetUpdate::clear(LYRICS_OVERLAY)]
        );
        assert!(engine.tick_at(Some(&second), t0, None).is_empty());

        // The player going away clears too, and only once.
        let third = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 3);
        assert!(payload(&engine.tick_at(Some(&third), t0, None), LYRICS_OVERLAY).ends_with("}a"));
        let gone = Snapshot {
            track: None,
            ..third.clone()
        };
        assert_eq!(
            engine.tick_at(Some(&gone), t0, None),
            vec![WidgetUpdate::clear(LYRICS_OVERLAY)]
        );
        assert!(engine.tick_at(Some(&gone), t0, None).is_empty());
    }

    #[test]
    fn a_paused_player_freezes_the_lyric_and_arms_no_deadline() {
        // Rule 6: no playback, nothing to animate, nothing to wake for.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let t0 = Instant::now();
        let playing = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        assert!(payload(&engine.tick_at(Some(&playing), t0, None), LYRICS_OVERLAY).ends_with("}a"));

        let paused = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Paused, 1);
        for step in 0..600 {
            let now = t0 + Duration::from_millis(100 * step);
            assert!(engine.tick_at(Some(&paused), now, None).is_empty());
        }
        assert_eq!(engine.next_deadline_at(Some(&paused), t0, None), None);
    }

    // -- clear_all / invalidate ---------------------------------------------

    #[test]
    fn clear_all_blanks_every_enabled_overlay_and_nothing_else() {
        // Lyrics only.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        assert_eq!(
            engine.clear_all(),
            vec![WidgetUpdate::clear(LYRICS_OVERLAY)]
        );

        // Both.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        assert_eq!(
            engine.tick_at(Some(&snap), t0, Some(at(14, 32, 30))).len(),
            2
        );

        let cleared = engine.clear_all();
        assert_eq!(cleared.len(), 2);
        assert!(cleared.iter().all(WidgetUpdate::is_clear));
        assert_eq!(cleared[0].overlay_id, LYRICS_OVERLAY);
        assert_eq!(cleared[1].overlay_id, CLOCK_OVERLAY);

        // And the widgets come straight back on the next tick — a wallpaper swap
        // must not leave the desktop without its lyric until the next song.
        let restored = engine.tick_at(Some(&snap), t0, Some(at(14, 32, 30)));
        assert_eq!(restored.len(), 2);
        assert!(payload(&restored, LYRICS_OVERLAY).ends_with("}a"));
        assert!(payload(&restored, CLOCK_OVERLAY).ends_with("14:32"));

        // The clock alone: nothing is pushed for a widget that is off.
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        assert_eq!(engine.clear_all(), vec![WidgetUpdate::clear(CLOCK_OVERLAY)]);
    }

    #[test]
    fn invalidate_re_pushes_exactly_what_is_on_screen() {
        // A respawned mpv has no overlays, and nothing about our state changed
        // — so this is the one case where an unchanged frame must still push.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let wall = Some(at(14, 32, 30));
        let first = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(first.len(), 2);
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());

        engine.invalidate();
        let again = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(again.len(), 2);
        assert_eq!(
            payload(&again, LYRICS_OVERLAY),
            payload(&first, LYRICS_OVERLAY)
        );
        assert_eq!(
            payload(&again, CLOCK_OVERLAY),
            payload(&first, CLOCK_OVERLAY)
        );
        // One re-push, not a permanent one.
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());

        // With nothing on screen there is nothing to restore: a blank overlay on
        // a fresh renderer is already blank.
        engine.clear_all();
        engine.set_clock(None);
        engine.invalidate();
        let empty = engine.tick_at(None, t0, wall);
        assert_eq!(empty, vec![WidgetUpdate::clear(LYRICS_OVERLAY)]);
    }

    // -- Smart Sleep --------------------------------------------------------

    #[test]
    fn next_deadline_is_the_earliest_of_everything_pending() {
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);

        // Line at 10s, next at 60s ⇒ 50s away. Clock at 14:32:30 ⇒ 30s away.
        engine.tick_at(Some(&snap), t0, Some(at(14, 32, 30)));
        let deadline = engine
            .next_deadline_at(Some(&snap), t0, Some(at(14, 32, 30)))
            .expect("something is pending");
        assert_eq!(deadline - t0, Duration::from_secs(30));

        // Move the playhead so the lyric is the nearer of the two.
        let late = snapshot_at(t0, Some(fixture()), us(59.5), PlaybackStatus::Playing, 1);
        engine.tick_at(Some(&late), t0, Some(at(14, 32, 30)));
        let deadline = engine
            .next_deadline_at(Some(&late), t0, Some(at(14, 32, 30)))
            .expect("something is pending");
        assert_eq!(deadline - t0, Duration::from_millis(500));

        // Each on its own gives its own answer.
        engine.set_clock(None);
        assert_eq!(
            engine
                .next_deadline_at(Some(&late), t0, None)
                .map(|d| d - t0),
            Some(Duration::from_millis(500))
        );
        engine.set_clock(Some(&clock_cfg()));
        let mut clock_only = WidgetEngine::new(None, ACCENT);
        clock_only.set_clock(Some(&clock_cfg()));
        clock_only.tick_at(None, t0, Some(at(14, 32, 30)));
        assert_eq!(
            clock_only
                .next_deadline_at(None, t0, Some(at(14, 32, 30)))
                .map(|d| d - t0),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn next_deadline_is_none_when_nothing_is_pending() {
        let t0 = Instant::now();
        // Nothing enabled.
        let engine = WidgetEngine::new(None, ACCENT);
        assert_eq!(
            engine.next_deadline_at(None, t0, Some(at(14, 32, 30))),
            None
        );

        // Lyrics enabled but no player at all.
        let engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        assert_eq!(engine.next_deadline_at(None, t0, None), None);

        // A player, but past the last line: nothing further happens on this
        // track, so there is nothing to wake for.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let snap = snapshot_at(t0, Some(fixture()), us(120.0), PlaybackStatus::Playing, 1);
        engine.tick_at(Some(&snap), t0, None);
        assert_eq!(engine.next_deadline_at(Some(&snap), t0, None), None);

        // A track with no lyrics at all is the same.
        let none = snapshot_at(t0, None, us(10.0), PlaybackStatus::Playing, 2);
        engine.tick_at(Some(&none), t0, None);
        assert_eq!(engine.next_deadline_at(Some(&none), t0, None), None);
    }

    #[test]
    fn a_gap_between_lines_costs_one_wake() {
        // The roadmap's own number, walked the way a loop would walk it.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let t0 = Instant::now();
        let mut now = t0;
        let mut wakes = 0;
        let snap = |now: Instant, base: Instant| {
            let position = us(10.0) + now.duration_since(base).as_micros() as i64;
            snapshot_at(base, Some(fixture()), position, PlaybackStatus::Playing, 1)
        };
        engine.tick_at(Some(&snap(now, t0)), now, None);
        loop {
            let pending = snap(now, t0);
            // Deliberately not a `while let`: the scrutinee's borrow of `engine`
            // would outlive the condition and collide with the tick below.
            let Some(deadline) = engine.next_deadline_at(Some(&pending), now, None) else {
                break;
            };
            now = deadline;
            wakes += 1;
            let woken = snap(now, t0);
            assert!(
                !engine.tick_at(Some(&woken), now, None).is_empty(),
                "woke at {wakes} for nothing"
            );
            assert!(wakes < 5, "far too many wakes");
        }
        assert_eq!(wakes, 1, "a 50s gap must cost one wake, not 500");
    }

    #[test]
    fn a_deadline_is_never_zero_or_negative() {
        // A zero-length wait turns Smart Sleep into a spin loop, which is worse
        // than the polling it replaces.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let t0 = Instant::now();
        for position in [0.0, 9.999_999, 10.0, 30.0, 59.999_999] {
            let snap = snapshot_at(
                t0,
                Some(fixture()),
                us(position),
                PlaybackStatus::Playing,
                1,
            );
            engine.tick_at(Some(&snap), t0, None);
            if let Some(deadline) = engine.next_deadline_at(Some(&snap), t0, None) {
                assert!(
                    deadline > t0,
                    "deadline at {position}s was not in the future"
                );
            }
        }
    }

    // -- config lifecycle ---------------------------------------------------

    #[test]
    fn enabling_and_disabling_starts_and_stops_the_worker() {
        let mut engine = WidgetEngine::new(None, ACCENT);
        assert!(engine.worker.is_none());
        assert!(!engine.is_active());

        engine.set_config(Some(&widgets(true)), ACCENT);
        assert!(engine.is_active());
        assert!(engine.worker.is_some(), "enabling must start the worker");
        // The worker publishes something readable immediately, even before its
        // first D-Bus round trip completes.
        assert!(engine.now_playing().is_some());

        // Re-applying the same config must not churn the thread.
        engine.set_config(Some(&widgets(true)), ACCENT);
        assert!(engine.worker.is_some());

        // Turning it off stops and joins it, and the overlay comes down once.
        let t0 = Instant::now();
        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        engine.tick_at(Some(&snap), t0, None);
        engine.set_config(Some(&widgets(false)), ACCENT);
        assert!(!engine.is_active());
        assert!(engine.worker.is_none(), "disabling must stop the worker");
        assert!(engine.now_playing().is_none());
        assert_eq!(
            engine.tick_at(None, t0, None),
            vec![WidgetUpdate::clear(LYRICS_OVERLAY)]
        );
        assert!(engine.tick_at(None, t0, None).is_empty());

        // Dropping an engine with a live worker must not hang or leak: the
        // thread is joined in `Worker::drop`.
        let engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        drop(engine);
    }

    #[test]
    fn disabling_the_clock_takes_it_down_once() {
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        let wall = Some(at(14, 32, 30));
        assert_eq!(engine.tick_at(None, t0, wall).len(), 1);
        engine.set_clock(None);
        assert!(!engine.is_active());
        assert_eq!(
            engine.tick_at(None, t0, wall),
            vec![WidgetUpdate::clear(CLOCK_OVERLAY)]
        );
        assert!(engine.tick_at(None, t0, wall).is_empty());
        // And the public tick, which decides for itself whether to read a clock,
        // agrees that there is nothing left to do.
        assert!(engine.tick().is_empty());
    }

    #[test]
    fn a_folder_change_reaches_the_worker_without_restarting_it() {
        let mut cfg = widgets(true);
        let mut engine = WidgetEngine::new(Some(&cfg), ACCENT);
        let worker = engine.worker.as_ref().expect("a worker");
        assert!(!lock(&worker.shared).reload);

        cfg.lyrics.folder = Some(PathBuf::from("/music/lyrics"));
        engine.set_config(Some(&cfg), ACCENT);
        let worker = engine.worker.as_ref().expect("the same worker");
        let state = lock(&worker.shared);
        assert_eq!(state.folder, Some(PathBuf::from("/music/lyrics")));
        // The current track is re-resolved rather than the change applying only
        // to the next song.
        assert!(state.reload);
    }

    // -- worker cadence -----------------------------------------------------

    #[test]
    fn the_worker_sleeps_rather_than_polls() {
        // The idle-cost claim, as arithmetic. The wait is the nearest *pending*
        // deadline, floored so it can never become a spin.
        let now = Instant::now();
        let in_secs = |s: u64| Some(now + Duration::from_secs(s));
        assert_eq!(cycle_wait(now, [in_secs(15), in_secs(5), None]), SCAN_IDLE);
        assert_eq!(cycle_wait(now, [None, None, None]), MIN_WAIT);
        assert_eq!(cycle_wait(now, [Some(now), None, None]), MIN_WAIT);

        // The trap. A metadata deadline belongs to a player; with none selected
        // it sits permanently in the past, and counting it would floor every
        // sleep at MIN_WAIT — an idle desktop spawning `gdbus` twenty times a
        // second, forever. Which is exactly what a `powertop` run would show.
        let stale = Some(now - Duration::from_secs(600));
        assert_eq!(
            cycle_wait(now, [Some(now + SCAN_EMPTY), None, None]),
            SCAN_EMPTY
        );
        assert_eq!(
            cycle_wait(now, [Some(now + SCAN_EMPTY), stale, None]),
            MIN_WAIT
        );

        // Cadence per state: nothing on the bus is the cheapest, and playing is
        // the only state that costs a query per second.
        let mut clock = PositionClock::new(now);
        assert_eq!(scan_interval(None, false, &clock), SCAN_EMPTY);
        assert_eq!(scan_interval(Some("org.mpris.x"), false, &clock), SCAN_IDLE);
        assert_eq!(meta_interval(&clock), META_IDLE);
        clock.set_status(PlaybackStatus::Playing, now);
        assert_eq!(
            scan_interval(Some("org.mpris.x"), false, &clock),
            SCAN_PLAYING
        );
        assert_eq!(meta_interval(&clock), META_PLAYING);
        // …and a playing status on a player we no longer have does not revive it.
        assert_eq!(scan_interval(None, false, &clock), SCAN_EMPTY);
        clock.set_status(PlaybackStatus::Paused, now);
        assert_eq!(meta_interval(&clock), META_IDLE);
    }

    #[test]
    fn an_unusable_session_scans_on_the_idle_cadence_not_the_empty_one() {
        // The Brave case. Nothing is selected, but a media session is sitting
        // right there — and pressing play in that tab is what makes it publish
        // a title. Noticing that is exactly as urgent as noticing a different
        // app start playing, so it gets the same latency, not the idle
        // desktop's fifteen seconds.
        let now = Instant::now();
        let mut clock = PositionClock::new(now);
        assert_eq!(scan_interval(None, true, &clock), SCAN_IDLE);
        assert_eq!(scan_interval(None, false, &clock), SCAN_EMPTY);
        // A stale "Playing" from a player we have since given up must not pin
        // the scan at the 15s playing cadence — there is nothing to be sticky
        // about once nothing is selected.
        clock.set_status(PlaybackStatus::Playing, now);
        assert_eq!(scan_interval(None, true, &clock), SCAN_IDLE);
        assert_eq!(scan_interval(None, false, &clock), SCAN_EMPTY);
        // Selecting somebody puts the usual cadences back in charge.
        assert_eq!(
            scan_interval(Some("org.mpris.x"), true, &clock),
            SCAN_PLAYING
        );
        clock.set_status(PlaybackStatus::Paused, now);
        assert_eq!(scan_interval(Some("org.mpris.x"), true, &clock), SCAN_IDLE);
        // Idle-cost sanity: the unusable-session cadence is still a multi-second
        // sleep, not a poll.
        assert!(SCAN_IDLE >= Duration::from_secs(5));
    }

    // -- diagnostics --------------------------------------------------------

    #[test]
    fn a_skipped_player_is_logged_once_per_bus_name() {
        // The line that explains the Brave case instead of it looking like
        // Fresco is broken — and the reason it is one line and not twelve a
        // minute.
        let brave = "org.mpris.MediaPlayer2.brave.instance6389";
        let mut log = SkipLog::default();
        assert!(log.note(brave), "the first sighting reports");
        for _ in 0..1000 {
            assert!(!log.note(brave), "every later poll is silent");
        }
        // A second, genuinely different session gets its own line.
        let chromium = "org.mpris.MediaPlayer2.chromium.instance42";
        assert!(log.note(chromium));
        assert!(!log.note(chromium));

        // Leaving the bus is forgotten, so restarting the browser — which mints
        // a fresh `instanceNNNN` — reports again rather than staying silent.
        log.retain(&[chromium.to_string()]);
        assert!(!log.note(chromium), "still present, still silent");
        assert!(log.note(brave), "came back, so report it again");

        // And the set cannot grow without bound as instances churn.
        let mut log = SkipLog::default();
        for i in 0..SKIP_LOG_CAP * 4 {
            assert!(log.note(&format!("org.mpris.MediaPlayer2.brave.instance{i}")));
        }
        assert!(log.seen.len() <= SKIP_LOG_CAP);
        log.retain(&[]);
        assert!(log.seen.is_empty());
    }

    #[test]
    fn selection_ignores_a_title_less_session_at_every_stage() {
        // The worker's two decision points share one definition of "usable", so
        // the scan and the per-poll check can never disagree and bounce a
        // player between selected and dropped.
        use crate::mpris::PlayerScan;
        let brave = NowPlaying {
            player: "org.mpris.MediaPlayer2.brave.instance6389".to_string(),
            art_url: Some("file:///tmp/.org.chromium.Chromium.1J5tKq".to_string()),
            status: PlaybackStatus::Stopped,
            ..Default::default()
        };
        assert!(!brave.has_title());
        assert!(!PlayerScan::of(&brave).is_usable());
        assert_eq!(
            mpris::pick_usable_player(&[PlayerScan::of(&brave)], None),
            None
        );

        // The same session once the page sets Media Session metadata.
        let playing = NowPlaying {
            title: "Some Song".to_string(),
            status: PlaybackStatus::Playing,
            ..brave.clone()
        };
        assert!(playing.has_title());
        assert_eq!(
            mpris::pick_usable_player(&[PlayerScan::of(&playing)], None).as_deref(),
            Some(brave.player.as_str()),
        );
    }

    #[test]
    fn overlay_ids_and_resolution_are_the_ones_the_loops_use() {
        // Pinned because both sides of the push depend on them and a silent
        // change would put the clock on top of the lyric — or, in the case of
        // the resolution, clip the overlay on a rotated wallpaper (W0).
        let ids = [
            LYRICS_OVERLAY,
            CLOCK_OVERLAY,
            VISUALIZER_OVERLAY,
            DISC_OVERLAY,
        ];
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                assert_ne!(a, b, "two widgets share an overlay id");
            }
        }
        assert_eq!((RES_X, RES_Y), (lyrics::PLAY_RES_X, lyrics::PLAY_RES_Y));
        assert!(WidgetUpdate::clear(LYRICS_OVERLAY).is_clear());
        assert!(!WidgetUpdate::ass(LYRICS_OVERLAY, "{\\an2}x".to_string()).is_clear());

        // A bitmap overlay is cleared by `overlay-remove`, not by an empty ASS
        // payload: mpv's two overlay commands do not see each other, so the
        // wrong one would leave the record on the next wallpaper.
        let remove = WidgetUpdate::remove(DISC_OVERLAY);
        assert!(remove.is_clear());
        assert_eq!(remove.bitmap, Some(BitmapUpdate::Remove));
        assert!(remove.frame().is_none());
        assert!(remove.ass.is_empty());

        let draw = WidgetUpdate::draw(
            DISC_OVERLAY,
            BitmapOverlay {
                x: 1,
                y: 2,
                path: PathBuf::from("/run/user/1000/fresco/widget-disc.bgra"),
                w: 8,
                h: 8,
                stride: 32,
            },
        );
        assert!(!draw.is_clear());
        let frame = draw.frame().expect("a bitmap update carries pixels");
        // stride is bytes per row, which is the one field a caller cannot
        // guess: BGRA is four bytes per pixel and the buffer is unpadded.
        assert_eq!(frame.stride, frame.w * 4);
        assert!(frame.path_str().ends_with("widget-disc.bgra"));
        // The ASS field stays empty on the bitmap path, so a call site that has
        // not grown its third arm yet pushes a harmless no-op rather than
        // garbage.
        assert!(draw.ass.is_empty());
    }

    // -- the visualiser ------------------------------------------------------

    fn visual_cfg(enabled: bool) -> VisualCfg {
        VisualCfg {
            enabled,
            // Small and cheap: these tests care about *when* a frame is pushed,
            // never about what the bars look like — `visualizer` and `dsp` own
            // that and test it themselves.
            bands: 8,
            fft_size: 256,
            sample_rate: 8_000,
            ..Default::default()
        }
    }

    /// A loud tone, long enough to fill one frame of `visual_cfg`'s buffer.
    fn tone(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 * 0.35).sin() * 0.9)
            .collect::<Vec<_>>()
    }

    /// Digital silence.
    fn quiet(n: usize) -> Vec<f32> {
        vec![0.0; n]
    }

    /// Drive `steps` frames of audio, one per frame period, and count the
    /// updates. The loop a daemon would run, compressed.
    fn drive_audio(
        engine: &mut WidgetEngine,
        samples: &[f32],
        t0: Instant,
        steps: u32,
    ) -> Vec<WidgetUpdate> {
        let mut out = Vec::new();
        for step in 1..=steps {
            let now = t0 + Duration::from_millis(50 * u64::from(step));
            out.extend(engine.feed_audio(samples, now));
        }
        out
    }

    #[test]
    fn a_disabled_visualizer_records_nothing_and_pushes_nothing() {
        // The privacy requirement stated as a test: the one feature in Fresco
        // that can listen to the user must not exist until it is asked for.
        for cfg in [None, Some(&visual_cfg(false))] {
            let mut engine = WidgetEngine::new(None, ACCENT);
            engine.set_visualizer(cfg);
            assert!(engine.visual.is_none(), "a capture was created");
            assert!(!engine.is_active());
            let t0 = Instant::now();
            for step in 0..100 {
                let now = t0 + Duration::from_millis(50 * step);
                assert!(engine.tick_at(None, now, None).is_empty());
            }
            assert_eq!(engine.next_deadline_at(None, t0, None), None);
            assert!(engine.clear_all().is_empty());
        }
    }

    #[test]
    fn silence_emits_one_clear_and_then_nothing() {
        // The hard power requirement: no audio must mean no redraw and no IPC,
        // not a stream of identical empty frames.
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_visualizer(Some(&visual_cfg(true)));
        let t0 = Instant::now();
        let n = engine.visual.as_ref().expect("a runtime").scratch.len();

        // Audio first, so there is something on screen to take down.
        let loud = drive_audio(&mut engine, &tone(n), t0, 4);
        assert!(!loud.is_empty(), "a loud tone must draw something");
        assert!(loud.iter().all(|u| u.overlay_id == VISUALIZER_OVERLAY));

        // Then a minute of silence. The bars fall rather than vanish — that is
        // the analyser's ~250ms release, and it is bounded — but the fall ends
        // in **exactly one** clear, and after that nothing is pushed again for
        // as long as the room stays quiet.
        let t1 = t0 + Duration::from_secs(1);
        let silent = drive_audio(&mut engine, &quiet(n), t1, 1200);
        let clears: Vec<_> = silent
            .iter()
            .enumerate()
            .filter(|(_, u)| u.is_clear())
            .collect();
        assert_eq!(clears.len(), 1, "silence pushed {} clears", clears.len());
        assert_eq!(clears[0].0, silent.len() - 1, "the clear must come last");
        assert_eq!(clears[0].1.overlay_id, VISUALIZER_OVERLAY);
        assert!(
            silent.len() < VISUAL_FPS as usize * 3,
            "the fade-out is bounded by the release time, not by the silence: \
             {} frames",
            silent.len()
        );

        // …and the widget is now sleeping on the silent cadence, not the
        // 24 Hz one: four looks a second, no pushes at all.
        let end = t1 + Duration::from_millis(50 * 1200);
        let deadline = engine
            .next_deadline_at(None, end, None)
            .expect("silence still has to notice the music coming back");
        assert!(
            deadline.saturating_duration_since(end) <= VISUAL_SILENT_PERIOD,
            "the silent cadence must still wake, just rarely"
        );
        assert!(
            deadline.saturating_duration_since(end) > visual_period(VISUAL_FPS),
            "silence must back off from the active frame rate"
        );
    }

    #[test]
    fn an_active_visualizer_is_rate_capped_and_never_pushes_per_tick() {
        // Rule 1 has no fixed point for a spectrum — it genuinely does change
        // every frame — so the budget is the frame rate instead, and
        // `next_deadline` has to publish it or the loop sleeps through it.
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_visualizer(Some(&visual_cfg(true)));
        let t0 = Instant::now();
        let n = engine.visual.as_ref().expect("a runtime").scratch.len();
        let loud = tone(n);

        // One second of a 100 Hz daemon loop.
        let mut pushes = 0;
        for step in 1..=100u32 {
            let now = t0 + Duration::from_millis(10 * u64::from(step));
            pushes += engine.feed_audio(&loud, now).len();
        }
        assert!(pushes > 0, "an active visualiser must draw");
        assert!(
            pushes <= VISUAL_FPS as usize + 1,
            "{pushes} pushes in a second, cap is {VISUAL_FPS}"
        );

        // And the deadline is one frame away, not zero and not never.
        let now = t0 + Duration::from_secs(1);
        let wait = engine
            .next_deadline_at(None, now, None)
            .expect("an active visualiser has a next frame")
            .saturating_duration_since(now);
        assert!(wait <= visual_period(VISUAL_FPS), "{wait:?} is too long");
    }

    #[test]
    fn a_visualizer_with_no_capture_costs_what_a_disabled_one_costs() {
        // Neither `pw-cat` nor `parec` installed — and, by `open_capture`'s
        // `cfg(test)` arm, every test run. It must degrade to "off with a log
        // line", never to a retry loop and never to a panic.
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_visualizer(Some(&visual_cfg(true)));
        let v = engine.visual.as_ref().expect("the widget is still enabled");
        assert!(v.capture.is_none(), "there is no audio device in a test");
        assert!(
            !v.live,
            "a captureless visualiser must not claim to be live"
        );

        // Enabled, so `is_active` is honest about it — and completely inert.
        assert!(engine.is_active());
        let t0 = Instant::now();
        for step in 0..500 {
            let now = t0 + Duration::from_millis(20 * step);
            assert!(engine.tick_at(None, now, None).is_empty());
            assert_eq!(engine.next_deadline_at(None, now, None), None);
        }
    }

    #[test]
    fn disabling_the_visualizer_stops_recording_and_takes_it_down_once() {
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_visualizer(Some(&visual_cfg(true)));
        let t0 = Instant::now();
        let n = engine.visual.as_ref().expect("a runtime").scratch.len();
        assert!(!drive_audio(&mut engine, &tone(n), t0, 4).is_empty());

        engine.set_visualizer(None);
        assert!(!engine.is_active());
        // The capture is dropped at the setter, not at the next tick: the child
        // process must stop recording the moment the user says so.
        assert!(engine.visual.as_ref().is_some_and(|v| v.capture.is_none()));

        let down = engine.tick_at(None, t0, None);
        assert_eq!(down, vec![WidgetUpdate::clear(VISUALIZER_OVERLAY)]);
        assert!(engine.visual.is_none(), "the runtime must be thrown away");
        assert!(engine.tick_at(None, t0, None).is_empty());
        assert!(engine.tick().is_empty());

        // Switching it back on must build a fresh runtime and try the capture
        // again. Re-using the disabled one — which had its capture dropped at
        // the setter — would leave the widget on and permanently deaf.
        engine.set_visualizer(Some(&visual_cfg(true)));
        let v = engine.visual.as_ref().expect("re-enabled");
        assert!(v.ass.is_none());
        // Off and on again *before* the tick that takes it down is the same
        // story, and must not leave a half-disabled runtime behind either.
        engine.set_visualizer(None);
        engine.set_visualizer(Some(&visual_cfg(true)));
        assert!(engine.visual.is_some());
        assert!(engine.is_active());
    }

    // -- the album-art disc --------------------------------------------------

    /// A disc widget small enough that `render_disc` costs microseconds, with
    /// its frame file in the test's own temporary directory rather than in the
    /// developer's runtime directory.
    fn with_disc(engine: &mut WidgetEngine, cfg: DiscWidgetCfg, tag: &str) {
        engine.set_disc(Some(&cfg));
        let disc = engine.disc.as_mut().expect("the disc is enabled");
        disc.path = std::env::temp_dir().join(format!("fresco-test-disc-{tag}.bgra"));
    }

    fn disc_cfg() -> DiscWidgetCfg {
        DiscWidgetCfg {
            enabled: true,
            size_px: 32,
            ..Default::default()
        }
    }

    #[test]
    fn a_disabled_disc_fetches_no_art_and_pushes_nothing() {
        for cfg in [None, Some(&DiscWidgetCfg::default())] {
            let mut engine = WidgetEngine::new(None, ACCENT);
            engine.set_disc(cfg);
            assert!(engine.disc.is_none());
            // No worker either: fetching cover art is the disc's reason for
            // needing one, and nothing else here wants a player.
            assert!(engine.worker.is_none());
            assert!(!engine.is_active());
            let t0 = Instant::now();
            let snap = snapshot_with_art(t0, us(0.0), PlaybackStatus::Playing, 1);
            for step in 0..100 {
                let now = t0 + Duration::from_millis(50 * step);
                assert!(engine.tick_at(Some(&snap), now, None).is_empty());
            }
            assert_eq!(engine.next_deadline_at(Some(&snap), t0, None), None);
        }
    }

    #[test]
    fn a_paused_disc_draws_once_and_then_stops_entirely() {
        // Rule 6, and the reason `elapsed` is playing time rather than wall
        // time: a paused record computes the angle it is already at, and
        // `should_redraw` refuses it. Ten minutes of pause is one frame.
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "paused");
        let t0 = Instant::now();
        let paused = snapshot_with_art(t0, us(30.0), PlaybackStatus::Paused, 1);

        let first = engine.tick_at(Some(&paused), t0, None);
        assert_eq!(first.len(), 1, "the record must go on screen");
        assert_eq!(first[0].overlay_id, DISC_OVERLAY);
        assert!(first[0].frame().is_some(), "and it must carry pixels");

        for step in 1..=6000 {
            let now = t0 + Duration::from_millis(100 * step);
            let updates = engine.tick_at(Some(&paused), now, None);
            assert!(updates.is_empty(), "paused tick {step} pushed {updates:?}");
        }
        // Nothing is going to change, so there is nothing to wake for.
        let end = t0 + Duration::from_secs(600);
        assert_eq!(engine.next_deadline_at(Some(&paused), end, None), None);
    }

    #[test]
    fn a_playing_disc_is_rate_capped_and_skips_imperceptible_steps() {
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "playing");
        let t0 = Instant::now();
        let playing = snapshot_with_art(t0, us(0.0), PlaybackStatus::Playing, 1);

        // One second of a 100 Hz daemon loop.
        let mut pushes = 0;
        for step in 1..=100u32 {
            let now = t0 + Duration::from_millis(10 * u64::from(step));
            pushes += engine.tick_at(Some(&playing), now, None).len();
        }
        // The establishing frame plus the cap.
        assert!(pushes > 1, "a playing record must turn");
        assert!(
            pushes <= DISC_FPS as usize + 2,
            "{pushes} frames in a second, cap is {DISC_FPS}"
        );

        // The step gate, on its own: a frame period that has moved the disc by
        // less than half a degree is not worth ~2.8ms and a 410KB write.
        assert!(!artwork::should_redraw(
            10.0,
            10.1,
            artwork::DEFAULT_MIN_STEP_DEG
        ));
        assert!(artwork::should_redraw(
            10.0,
            26.7,
            artwork::DEFAULT_MIN_STEP_DEG
        ));

        // `spin = false` pins the angle, so after the first frame there is
        // nothing to redraw however long it plays.
        let mut still = WidgetEngine::new(None, ACCENT);
        with_disc(
            &mut still,
            DiscWidgetCfg {
                spin: false,
                ..disc_cfg()
            },
            "still",
        );
        assert_eq!(still.tick_at(Some(&playing), t0, None).len(), 1);
        for step in 1..=200 {
            let now = t0 + Duration::from_millis(50 * step);
            assert!(still.tick_at(Some(&playing), now, None).is_empty());
        }
    }

    #[test]
    fn the_disc_follows_the_track_and_comes_down_with_the_player() {
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "track");
        let t0 = Instant::now();
        let first = snapshot_with_art(t0, us(0.0), PlaybackStatus::Playing, 1);
        assert_eq!(engine.tick_at(Some(&first), t0, None).len(), 1);

        // A new track puts a new record on, at the top rather than continuing
        // the old one's angle.
        let t1 = t0 + Duration::from_secs(1);
        let second = snapshot_with_art(t1, us(0.0), PlaybackStatus::Playing, 2);
        let swapped = engine.tick_at(Some(&second), t1, None);
        assert_eq!(swapped.len(), 1);
        assert!(swapped[0].frame().is_some());
        assert_eq!(
            engine.disc.as_ref().expect("a disc").elapsed,
            Duration::ZERO
        );

        // The player going away takes it down, once — and with `overlay-remove`
        // rather than an empty ASS payload.
        let gone = Snapshot {
            track: None,
            ..second.clone()
        };
        let t2 = t1 + Duration::from_secs(1);
        assert_eq!(
            engine.tick_at(Some(&gone), t2, None),
            vec![WidgetUpdate::remove(DISC_OVERLAY)]
        );
        assert!(engine.tick_at(Some(&gone), t2, None).is_empty());
        assert_eq!(engine.next_deadline_at(Some(&gone), t2, None), None);
    }

    #[test]
    fn the_disc_is_placed_against_the_output_not_the_ass_grid() {
        // `overlay-add` takes a corner in real output pixels, where every text
        // widget gets `\an` placement for free. Getting this wrong puts the
        // record off the edge of a 4K screen and nowhere near it on a 720p one.
        let (w, h) = (3840, 2160);
        assert_eq!(anchor_xy(Anchor::TopLeft, 320, 48, w, h), (48, 48));
        assert_eq!(
            anchor_xy(Anchor::BottomRight, 320, 48, w, h),
            ((w - 320 - 48) as i32, (h - 320 - 48) as i32)
        );
        assert_eq!(
            anchor_xy(Anchor::MidCenter, 320, 48, w, h),
            (((w - 320) / 2) as i32, ((h - 320) / 2) as i32)
        );
        // A disc bigger than the output pins to the edge instead of wrapping
        // around to a nonsense coordinate.
        assert_eq!(anchor_xy(Anchor::BottomRight, 4000, 48, w, h), (0, 0));
        assert_eq!(anchor_xy(Anchor::TopLeft, 4000, 48, w, h), (0, 0));

        // And a size change re-places and re-pushes rather than waiting for the
        // next track.
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "place");
        engine.set_output_size(w, h);
        let t0 = Instant::now();
        let snap = snapshot_with_art(t0, us(0.0), PlaybackStatus::Paused, 1);
        let placed = engine.tick_at(Some(&snap), t0, None);
        let frame = placed[0].frame().expect("pixels");
        assert_eq!(
            (frame.x, frame.y),
            anchor_xy(Anchor::BottomRight, 32, 48, w, h)
        );
        assert_eq!((frame.w, frame.h), (32, 32));
        assert_eq!(frame.stride, 32 * 4);

        engine.set_output_size(1280, 720);
        let moved = engine.tick_at(Some(&snap), t0, None);
        let frame = moved[0].frame().expect("pixels");
        assert_eq!(
            (frame.x, frame.y),
            anchor_xy(Anchor::BottomRight, 32, 48, 1280, 720)
        );
        // A zero-sized mode report is ignored rather than parking the disc in
        // the corner.
        engine.set_output_size(0, 0);
        assert_eq!((engine.out_w, engine.out_h), (1280, 720));
    }

    // -- all four together ---------------------------------------------------

    #[test]
    fn clear_all_blanks_every_enabled_overlay_including_the_bitmap_one() {
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        engine.set_visualizer(Some(&visual_cfg(true)));
        with_disc(&mut engine, disc_cfg(), "clearall");

        let cleared = engine.clear_all();
        assert_eq!(cleared.len(), 4, "one per widget: {cleared:?}");
        assert!(cleared.iter().all(WidgetUpdate::is_clear));
        let ids: Vec<u32> = cleared.iter().map(|u| u.overlay_id).collect();
        assert_eq!(
            ids,
            vec![
                LYRICS_OVERLAY,
                CLOCK_OVERLAY,
                VISUALIZER_OVERLAY,
                DISC_OVERLAY
            ]
        );
        // The disc's clear is an `overlay-remove`; everybody else's is an empty
        // ASS payload. Sending the wrong one leaves the widget on screen.
        assert_eq!(
            cleared[3].bitmap,
            Some(BitmapUpdate::Remove),
            "a bitmap overlay is not cleared by an empty ASS payload"
        );
        assert!(cleared[..3].iter().all(|u| u.bitmap.is_none()));

        // And everything comes straight back: a wallpaper swap must not leave
        // the desktop bare until the next song.
        let t0 = Instant::now();
        let snap = snapshot_with_art(t0, us(10.0), PlaybackStatus::Playing, 1);
        let back = engine.tick_at(Some(&snap), t0, Some(at(14, 32, 30)));
        let ids: Vec<u32> = back.iter().map(|u| u.overlay_id).collect();
        assert!(ids.contains(&CLOCK_OVERLAY), "{back:?}");
        assert!(ids.contains(&DISC_OVERLAY), "{back:?}");
    }

    #[test]
    fn invalidate_re_pushes_the_bitmap_widget_too() {
        // A respawned mpv has no overlays *of either kind*, and the bitmap one
        // will not come back on its own: a paused record is exactly the case
        // where nothing would otherwise redraw, ever.
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "invalidate");
        let t0 = Instant::now();
        let paused = snapshot_with_art(t0, us(0.0), PlaybackStatus::Paused, 1);
        assert_eq!(engine.tick_at(Some(&paused), t0, None).len(), 1);
        assert!(engine.tick_at(Some(&paused), t0, None).is_empty());

        engine.invalidate();
        let again = engine.tick_at(Some(&paused), t0, None);
        assert_eq!(again.len(), 1);
        assert!(again[0].frame().is_some());
        // One re-push, not a permanent one.
        assert!(engine.tick_at(Some(&paused), t0, None).is_empty());
    }

    #[test]
    fn next_deadline_is_the_minimum_across_all_four_widgets() {
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        // A lyric line 50s away and a clock 30s away — the two schedule-driven
        // widgets, established first so their deadlines are armed.
        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let wall = Some(at(14, 32, 30));
        engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(
            engine
                .next_deadline_at(Some(&snap), t0, wall)
                .map(|d| d - t0),
            Some(Duration::from_secs(30))
        );

        // The disc undercuts both of them: a turning record is the nearest
        // thing that changes.
        with_disc(&mut engine, disc_cfg(), "deadline");
        let art = snapshot_with_art(t0, us(10.0), PlaybackStatus::Playing, 1);
        engine.tick_at(Some(&art), t0, wall);
        let wait = engine
            .next_deadline_at(Some(&art), t0, wall)
            .expect("something is pending")
            - t0;
        assert!(wait <= DISC_PERIOD, "{wait:?} should be one disc frame");

        // …and the visualiser undercuts the disc in turn, being the fastest of
        // the four.
        engine.set_visualizer(Some(&visual_cfg(true)));
        let n = engine.visual.as_ref().expect("a runtime").scratch.len();
        engine.feed_audio(&tone(n), t0);
        let wait = engine
            .next_deadline_at(Some(&art), t0, wall)
            .expect("something is pending")
            - t0;
        assert!(
            wait <= visual_period(VISUAL_FPS),
            "{wait:?} should be one visualiser frame"
        );

        // Turning the two fast widgets off puts the schedule-driven answer back
        // — the minimum is over what is *enabled*, not over what once was.
        engine.set_visualizer(None);
        engine.set_disc(None);
        engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(
            engine
                .next_deadline_at(Some(&snap), t0, wall)
                .map(|d| d - t0),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn the_worker_serves_both_widgets_that_need_a_player() {
        // One thread, not two: the lyric widget and the disc want the same
        // `gdbus` queries, and running a thread each would double the idle cost
        // this module's whole design is about.
        let mut engine = WidgetEngine::new(None, ACCENT);
        assert!(engine.worker.is_none());

        // The disc alone starts it, and asks for art at its own size.
        //
        // `art_size` and not `art_reload`: the reload flag is deliberately
        // one-shot and the worker takes it at the top of its very first cycle,
        // so reading it from here is a race with a thread that is already
        // running. What it *does* is asserted below, where it is observable.
        with_disc(&mut engine, disc_cfg(), "worker");
        let worker = engine.worker.as_ref().expect("the disc needs a player");
        assert_eq!(lock(&worker.shared).art_size, Some(32));

        // Adding lyrics reuses it.
        engine.set_config(Some(&widgets(true)), ACCENT);
        assert!(engine.worker.is_some());

        // Dropping the disc leaves it running for the lyrics, and stops it
        // fetching art nobody is going to draw.
        engine.set_disc(None);
        let worker = engine.worker.as_ref().expect("the lyrics still need it");
        assert_eq!(lock(&worker.shared).art_size, None);

        // Dropping the lyrics too stops and joins it.
        engine.set_config(Some(&widgets(false)), ACCENT);
        assert!(engine.worker.is_none());
    }

    #[test]
    fn art_is_only_ever_fetched_for_a_disc_that_is_switched_on() {
        // `art_size` is the switch that decides whether the worker touches the
        // network for a picture: `None` means it must not, at any size. This is
        // the whole matrix, and it is race-free — the worker reads this field
        // and never writes it.
        for lyrics_on in [false, true] {
            for disc_on in [false, true] {
                let mut engine = WidgetEngine::new(None, ACCENT);
                engine.set_config(Some(&widgets(lyrics_on)), ACCENT);
                if disc_on {
                    with_disc(
                        &mut engine,
                        DiscWidgetCfg {
                            size_px: 96,
                            ..disc_cfg()
                        },
                        "matrix",
                    );
                }
                match engine.worker.as_ref() {
                    Some(w) => {
                        assert!(lyrics_on || disc_on, "a worker nobody needs");
                        assert_eq!(
                            lock(&w.shared).art_size,
                            disc_on.then_some(96),
                            "lyrics={lyrics_on} disc={disc_on}"
                        );
                    }
                    None => assert!(
                        !lyrics_on && !disc_on,
                        "lyrics={lyrics_on} disc={disc_on} needs a player"
                    ),
                }
            }
        }
    }

    #[test]
    fn a_frame_write_failure_is_reported_once_and_backs_off() {
        // A read-only or full runtime directory must not become a render and a
        // log line twelve times a second forever.
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "unwritable");
        // A path whose parent is a *file*, so `create_dir_all` cannot succeed.
        let blocker = std::env::temp_dir().join("fresco-test-disc-blocker");
        std::fs::write(&blocker, b"not a directory").expect("temp dir is writable");
        engine.disc.as_mut().expect("a disc").path = blocker.join("frame.bgra");

        let t0 = Instant::now();
        let playing = snapshot_with_art(t0, us(0.0), PlaybackStatus::Playing, 1);
        let mut attempts = 0;
        for step in 0..600u32 {
            let now = t0 + Duration::from_millis(50 * u64::from(step));
            assert!(
                engine.tick_at(Some(&playing), now, None).is_empty(),
                "a frame that was never written must not be announced"
            );
            if engine.disc.as_ref().expect("a disc").warned {
                attempts += 1;
            }
        }
        assert!(attempts > 0, "the failure must be noticed");
        // 30 seconds at the retry cadence, not at the frame cadence.
        let retries = 30 / DISC_RETRY.as_secs();
        assert!(
            engine.disc.as_ref().expect("a disc").warned,
            "the failure is latched so it is logged once"
        );
        assert!(retries <= 15, "the back-off must be seconds, not frames");
        let _ = std::fs::remove_file(&blocker);
    }

    #[test]
    fn the_frame_file_is_rewritten_in_place_and_never_truncated_away() {
        // mpv `mmap`s this file. Truncating it under a live mapping is a SIGBUS
        // in the renderer, so the write must overwrite rather than recreate.
        let path = std::env::temp_dir().join("fresco-test-disc-write.bgra");
        let _ = std::fs::remove_file(&path);
        let art = artwork::placeholder_art(16);
        let frame = artwork::render_disc(
            &art,
            &DiscCfg {
                size_px: 16,
                ..Default::default()
            },
        );
        write_frame(&path, &frame).expect("the temp dir is writable");
        let first = std::fs::metadata(&path).expect("written").len();
        assert_eq!(first, u64::from(frame.w * frame.h * 4));
        assert_eq!(first, u64::from(frame.h * frame.stride()));

        // A second frame of the same size reuses the file byte for byte.
        write_frame(&path, &frame).expect("rewritable");
        assert_eq!(std::fs::metadata(&path).expect("written").len(), first);

        // A resize shortens it, which is the one case where the old mapping was
        // going away anyway.
        let small = artwork::render_disc(
            &art,
            &DiscCfg {
                size_px: 8,
                ..Default::default()
            },
        );
        write_frame(&path, &small).expect("rewritable");
        assert_eq!(
            std::fs::metadata(&path).expect("written").len(),
            u64::from(small.w * small.h * 4)
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_frame_rates_are_the_ones_the_power_budget_assumes() {
        // Pinned: these are the numbers the module docs quote, and a silent
        // bump to 60 would be a 5x cost increase nobody would notice in review.
        assert!((10..=30).contains(&VISUAL_FPS));
        assert!((8..=20).contains(&DISC_FPS));
        assert!(VISUAL_SILENT_PERIOD >= Duration::from_millis(100));
        // A hand-edited `fps = 0` must not turn Smart Sleep into a spin.
        assert_eq!(visual_period(0), visual_period(1));
        assert!(visual_period(0) >= Duration::from_millis(16));
        assert!(visual_period(10_000) >= Duration::from_millis(16));
    }
}

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
//! widgets.set_outputs(&geoms);             // every tick, BEFORE tick()
//! for u in widgets.tick() {                                     // every loop tick
//!     for target in &targets {
//!         if !u.is_for(&target.connector) { continue; }   // per-output pixels
//!         match &u.bitmap {
//!             None => target.player.set_overlay(u.overlay_id, &u.ass, RES_X, RES_Y),
//!             Some(BitmapUpdate::Draw(b)) => target.player
//!                 .overlay_add(u.overlay_id, b.x, b.y, &b.path_str(), b.w, b.h, b.stride),
//!             Some(BitmapUpdate::Remove) => target.player.overlay_remove(u.overlay_id),
//!         }
//!     }
//! }
//! let wake = widgets.next_deadline();      // clamp the loop's own wait to this
//!
//! for u in widgets.clear_all() { … }       // wallpaper swap / teardown
//! widgets.invalidate();                    // renderer respawn / rotation change
//! ```
//!
//! # Bitmaps are first-class, and that costs three things ASS did not
//!
//! Three of the four widgets are ASS overlays and one — the album-art disc — is
//! a BGRA bitmap pushed through `overlay-add`. ASS gets three properties for
//! free that pixels do not, and the engine has to supply each of them itself:
//!
//! * **Clearing.** An empty `osd-overlay` blanks ASS; `overlay-remove` blanks a
//!   bitmap; neither does the other's job. So the engine records the
//!   [`OverlayKind`] it actually pushed to each id and clears by *that* — never
//!   by which widget it is, because a widget can change substrate at runtime.
//! * **Resolution independence.** libass rescales [`RES_X`]×[`RES_Y`] per
//!   output; `overlay-add` takes real pixels. So the engine is told every
//!   output ([`WidgetEngine::set_outputs`]) and tags each bitmap update with
//!   the one it belongs on ([`WidgetUpdate::target`]). The state machine stays
//!   **single** — one lyric line, one angle, one clock reading — and only
//!   rasterisation and placement are per output.
//! * **Change detection.** The ASS widgets compare the rendered string, which
//!   is free. Pixels are megabytes. So a bitmap widget hands over a
//!   [`ContentKey`] derived from its content instead, and
//!   the engine rasterises only when the key moves.
//!
//! [`MAX_WIDGET_AREA_PX`] caps what may be rasterised at all, and
//! `write_frame` documents the one rule a bitmap widget must not break: the
//! frame file is grown and rewritten in place, never shortened.
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
//! * Call [`WidgetEngine::set_outputs`] with every output the widget layer is
//!   on, **before** [`WidgetEngine::tick`] and on every tick. `overlay-add` is
//!   in **real output pixels**, not the ASS coordinate space, so this is what
//!   places and sizes every bitmap widget; without it the engine assumes one
//!   1920×1080 output. Then route each update by [`WidgetUpdate::is_for`]. A
//!   loop driving exactly one output can use
//!   [`WidgetEngine::set_output_size`] and skip the routing entirely.
//! * Clamp the loop's own wait with [`WidgetEngine::next_deadline`]. It may
//!   only ever *shorten* it: the loops have their own reasons to wake
//!   (animation frames, monitor hotplug) and a widget must not be able to
//!   starve them.
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
use crate::widgetkit::{
    self, cards, BarPaint, Canvas, Color, FontStack, Mode, Size, Theme, WidgetSize,
};

use super::lyrics_runtime::{self, Action, LyricsRuntime};
use super::widget_anchor;

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
    /// Which output this update belongs on.
    ///
    /// `None` — the overwhelmingly common case — means *every* target the loop
    /// is driving: an ASS payload is resolution-independent (libass rescales
    /// [`RES_X`]×[`RES_Y`] per output for us) and a clear is a clear whatever
    /// is behind it.
    ///
    /// `Some(connector)` means **these pixels were rasterised and placed
    /// against that one output's mode** and are wrong anywhere else. That is
    /// the whole of the per-output story: one state machine deciding *what* to
    /// show, N geometries deciding *how big and where*, so a mixed-DPI desktop
    /// gets a correctly sized widget on both screens instead of one sized for
    /// whichever monitor the loop happened to look at first.
    ///
    /// The loop's filter is one line — see [`WidgetUpdate::is_for`].
    pub target: Option<String>,
}

/// Which of mpv's two overlay commands currently owns an overlay id.
///
/// The reason this is tracked at all: clearing an overlay is **not** one
/// command. An ASS overlay goes down by pushing an empty `osd-overlay`; a
/// bitmap overlay goes down by `overlay-remove`, and the wrong one of the two
/// is a silent no-op that leaves the widget burned onto the next wallpaper.
///
/// It is deliberately recorded from what the engine **actually emitted**, not
/// inferred from config: a widget that falls back from a bitmap to ASS (or the
/// other way) changes kind at runtime, and the only trustworthy answer to
/// "what is on screen right now" is "what we last pushed".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    /// Drawn by `osd-overlay`; cleared by an empty ASS payload.
    Ass,
    /// Drawn by `overlay-add`; cleared by `overlay-remove`.
    Bitmap,
}

/// One output the widget layer is being drawn on, in **real pixels**.
///
/// The engine keeps a list of these rather than a single size (see
/// [`WidgetEngine::set_outputs`]) because `overlay-add` takes real pixels: a
/// bitmap sized for a 4K screen lands at a quarter of the intended size, in the
/// wrong place, on the 1080p one beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputGeom {
    /// Connector name (`"DP-1"`). Empty means "the one unnamed output", which
    /// is what [`WidgetEngine::set_output_size`] installs and what makes every
    /// update it produces a broadcast.
    pub connector: String,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

impl OutputGeom {
    /// The [`WidgetUpdate::target`] for this output: `None` for the unnamed
    /// one, since there is then nothing to tell apart.
    fn target(&self) -> Option<String> {
        (!self.connector.is_empty()).then(|| self.connector.clone())
    }
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

    /// Whether this update belongs on the output called `connector`.
    ///
    /// The loop's whole per-output filter: `if u.is_for(&r.connector) { … }`.
    /// An untargeted update — every ASS payload, every clear — is for all of
    /// them.
    pub fn is_for(&self, connector: &str) -> bool {
        self.target.as_deref().is_none_or(|t| t == connector)
    }

    /// Which overlay command this update uses, or `None` when it takes the
    /// overlay down rather than drawing on it.
    fn kind(&self) -> Option<OverlayKind> {
        match &self.bitmap {
            None if self.ass.is_empty() => None,
            None => Some(OverlayKind::Ass),
            Some(BitmapUpdate::Draw(_)) => Some(OverlayKind::Bitmap),
            Some(BitmapUpdate::Remove) => None,
        }
    }

    /// An ASS update for `overlay_id`.
    fn ass(overlay_id: u32, ass: String) -> Self {
        WidgetUpdate {
            overlay_id,
            ass,
            bitmap: None,
            target: None,
        }
    }

    /// An update that blanks a **text** `overlay_id`.
    fn clear(overlay_id: u32) -> Self {
        WidgetUpdate::ass(overlay_id, String::new())
    }

    /// An update that draws `frame` on `overlay_id`, on every target.
    #[cfg(test)]
    fn draw(overlay_id: u32, frame: BitmapOverlay) -> Self {
        WidgetUpdate::draw_on(overlay_id, None, frame)
    }

    /// An update that draws `frame` on `overlay_id`, on one target (or on all
    /// of them when `target` is `None`).
    fn draw_on(overlay_id: u32, target: Option<String>, frame: BitmapOverlay) -> Self {
        WidgetUpdate {
            overlay_id,
            ass: String::new(),
            bitmap: Some(BitmapUpdate::Draw(frame)),
            target,
        }
    }

    /// An update that blanks a **bitmap** `overlay_id`.
    fn remove(overlay_id: u32) -> Self {
        WidgetUpdate {
            overlay_id,
            ass: String::new(),
            bitmap: Some(BitmapUpdate::Remove),
            target: None,
        }
    }

    /// The update that takes `overlay_id` down given what kind of overlay is
    /// sitting on it. **This is the whole of defect 1**: picking by kind rather
    /// than by widget is what keeps a clear working when a widget is ported
    /// from ASS to pixels.
    fn blank(overlay_id: u32, kind: OverlayKind) -> Self {
        match kind {
            OverlayKind::Ass => WidgetUpdate::clear(overlay_id),
            OverlayKind::Bitmap => WidgetUpdate::remove(overlay_id),
        }
    }
}

/// Where a rendered bitmap frame is and how to read it — everything
/// `overlay-add` needs and nothing else.
///
/// `path` is a file the engine **keeps and rewrites in place**, one per widget
/// per output geometry, because mpv `mmap`s it: a fresh temporary file per
/// frame would be a create, a write, an unlink and a fresh mapping ten times a
/// second, and a file that vanished under mpv would be a SIGBUS rather than a
/// missing frame. The engine owns its lifetime; the caller only ever reads it.
/// It is also **never shortened** — see `write_frame` for why that is a
/// correctness requirement and not a micro-optimisation.
///
/// `x`/`y` are **real output pixels**, not the [`RES_X`]×[`RES_Y`] ASS space,
/// and they are resolved against **one** output — the one named by
/// [`WidgetUpdate::target`]. See [`WidgetEngine::set_outputs`].
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

/// Say once, when an MPRIS-driven widget turns on, that the tool every MPRIS
/// query needs is missing.
///
/// Same contract as [`open_capture`]'s warning, and for the same reason: with
/// no `gdbus`, [`mpris::list_players`] returns empty forever, so the lyrics,
/// album-art disc and track-synced widgets are enabled in the config and then
/// simply never draw. That is indistinguishable from "nothing is playing" — the
/// difference between a bug report and a one-line fix is naming the binary and
/// the package that ships it.
///
/// Logged at `warn` and only from [`Worker::start`] (once per enable, not per
/// tick): [`mpris::gdbus_call`] deliberately stays at `debug` because it fails
/// routinely whenever a player exits mid-poll.
fn warn_if_no_gdbus() {
    if mpris::gdbus_available() {
        return;
    }
    log::warn!(
        "widgets: a now-playing widget (lyrics / album art / track-synced clock) is enabled but \
         `gdbus` is not installed — install libglib2.0-bin (Debian/Ubuntu), glib2 (Arch/Fedora) \
         or glib2-tools (openSUSE); these widgets stay blank until then"
    );
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
        warn_if_no_gdbus();
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

/// Back-off after a failed visualiser frame write.
///
/// The longest of the four, and deliberately: this is the widget that would
/// otherwise retry twenty-four times a second against a full or read-only
/// runtime directory.
const VISUAL_RETRY: Duration = Duration::from_secs(3);

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

/// The width percentage above which `widgetkit` draws no card at all.
///
/// A copy of `widgetkit::cards::visualizer`'s own `BARE_ABOVE_PCT`, and the one
/// number in this module that is duplicated from there. It is duplicated on
/// purpose: the engine resolves the variant itself and passes a **concrete**
/// one to the card, because the chrome cache below is only valid for the panel
/// treatment and "let the card decide" would mean the two could disagree about
/// which treatment is on screen. Passing the resolved answer makes that
/// impossible; keeping `Auto` would not.
const BARE_ABOVE_PCT: f32 = 45.0;

/// Which treatment this configuration draws.
fn visual_variant(cfg: &VisualStyleCfg) -> cards::VisualizerVariant {
    if cfg.width_pct.is_finite() && cfg.width_pct > BARE_ABOVE_PCT {
        cards::VisualizerVariant::Bare
    } else {
        cards::VisualizerVariant::Panel
    }
}

/// How the bars are coloured, from the three config keys that decide it.
///
/// `accent_follow` wins over `colour`, exactly as it does everywhere else in
/// the widget layer, and the gradient mode then decides whether the resulting
/// colour is flat or one end of a ramp — so the two settings compose instead of
/// contradicting.
fn bar_paint(cfg: &VisualStyleCfg, t: &Theme) -> BarPaint {
    let hex = |s: &str, fallback: Color| Color::from_hex(s).unwrap_or(fallback);
    match cfg.gradient {
        // The panel default (spec §8.4): a per-bar vertical ramp, which is what
        // makes the caps read as the data and the bases as the floor. Only
        // reachable while the accent is in force — a user-picked colour has no
        // second stop to ramp to.
        visualizer::Gradient::None if cfg.accent_follow => BarPaint::Vertical,
        visualizer::Gradient::None => BarPaint::Fixed(hex(&cfg.colour, t.accent_fill)),
        visualizer::Gradient::Linear => BarPaint::Across(hex(&cfg.colour_end, t.accent_dim)),
        visualizer::Gradient::Spectrum => BarPaint::Spectrum,
    }
}

/// The palette with every **chrome** token removed, for the per-frame pass.
///
/// This is how the visualiser affords to redraw at [`VISUAL_FPS`].
/// `Canvas::drop_shadow` blurs the *whole canvas* mask on every call, so a
/// straight full redraw of the panel is two full-canvas blurs a frame — and of
/// the bare treatment, where every bar carries its own E1 shadow, it is two per
/// **bar**: ninety-six full-canvas blurs a frame at the default band count,
/// which is not a frame budget, it is a fan.
///
/// So the card body, its hairline, its edge light and every shadow are drawn
/// **once** into [`ChromeLayer`] and composited under each frame, and the
/// per-frame pass runs against this palette instead — where `elevation` returns
/// immediately (`color.a <= 0.0`) and the card's own fills are no-ops. What is
/// left for it to draw is the well, the bars and the peak caps.
///
/// [`Theme::well`] is deliberately **kept**: it is not only the well's fill, it
/// is also the base of `BarPaint::Vertical`'s ramp, and zeroing it would change
/// the colour of every bar. The well is therefore drawn on the frame pass and
/// left out of the chrome layer, which costs one rounded rect and four
/// hairline strokes — no blur — per frame.
///
/// # The one thing this gives up
///
/// The bare treatment's **per-bar shadow** (spec §9.3.1). It is the instrument
/// that keeps a bar visible where it crosses a same-tone region of a photo, and
/// there is no version of it that survives a full-canvas blur per bar per
/// frame. The gradient scrim — the other, larger instrument for that treatment
/// — is untouched, and it is the one doing most of the work.
fn bars_only(t: &Theme) -> Theme {
    Theme {
        shadow: Color::TRANSPARENT,
        surface: Color::TRANSPARENT,
        surface_far: Color::TRANSPARENT,
        edge: Color::TRANSPARENT,
        edge_highlight: Color::TRANSPARENT,
        ..*t
    }
}

/// The palette with the **well** removed, for the cached chrome pass.
///
/// The mirror of [`bars_only`]: between them every layer of the card is drawn
/// exactly once. Anything added to one must be removed from the other.
fn chrome_only(t: &Theme) -> Theme {
    Theme {
        well: Color::TRANSPARENT,
        edge_well_top: Color::TRANSPARENT,
        edge_well_bottom: Color::TRANSPARENT,
        ..*t
    }
}

/// The card body, its hairline, its edge light and its shadow — rasterised once
/// and composited under every frame.
///
/// Premultiplied BGRA, the same layout the frame files are in, so compositing a
/// frame over it is a source-over in premultiplied space: `out = src + dst ·
/// (1 − src.a)`, four bytes at a time, with an exact fast path at both ends of
/// the alpha range. That is a handful of arithmetic per pixel against the
/// several full-canvas Gaussian passes it replaces.
struct ChromeLayer {
    w: u32,
    h: u32,
    scale: f32,
    /// Premultiplied BGRA, `w · h · 4` bytes.
    data: Vec<u8>,
}

/// Composite `src` over `dst`, both premultiplied BGRA of the same length.
///
/// The Porter-Duff "over" in premultiplied space, which is the whole reason the
/// chrome is cached in that form: no un-premultiply, no channel swap, no
/// second `Pixmap`. Most of a bar layer is fully transparent and the caps are
/// fully opaque, so the two exact branches carry the overwhelming majority of
/// the pixels and the arithmetic runs on the edges only.
fn over_in_place(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let a = s[3];
        if a == 0 {
            continue;
        }
        if a == 255 {
            d.copy_from_slice(s);
            continue;
        }
        let inv = 255 - u32::from(a);
        for i in 0..4 {
            // `+ 127` then the classic /255 approximation: exact for every
            // input, and no division.
            let under = u32::from(d[i]) * inv + 127;
            let under = (under + (under >> 8)) >> 8;
            d[i] = (u32::from(s[i]) + under).min(255) as u8;
        }
    }
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
    /// The bar and peak-cap envelopes (spec §10). Caller-owned, because the
    /// renderer never allocates and this is the widget that redraws every
    /// frame while audio plays.
    motion: visualizer::Motion,
    /// Whether we believe a spectrum is on screen. Silence takes it down.
    shown: bool,
    /// When [`VisualState::motion`] was last advanced, so the envelopes run on
    /// real elapsed time rather than on a frame count.
    stepped_at: Option<Instant>,
    /// The card body, its hairline, its edge light and its shadow — see
    /// [`ChromeLayer`]. `None` for the bare treatment, which has no card, and
    /// until the first frame.
    chrome: Option<ChromeLayer>,
    /// The per-frame surface: well, bars, peak caps. See [`Surface`].
    canvas: Surface,
    /// Scratch for the bar layer's pixels, so the per-frame path allocates the
    /// output buffer and nothing else.
    layer: Vec<u8>,
    /// Files, per-output placement, the redraw gate and the write back-off.
    bmp: BitmapState,
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
            motion: visualizer::Motion::new(cfg.bands),
            shown: false,
            stepped_at: None,
            chrome: None,
            canvas: Surface::default(),
            layer: Vec::new(),
            bmp: BitmapState::new("widget-visualizer", now),
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
    /// most one update per output.
    ///
    /// The tested seam for this widget, in the same spirit as
    /// [`WidgetEngine::tick_at`]: every decision — the rate cap, the silence
    /// verdict, the envelopes — is a pure function of the samples and the clock
    /// over this struct's own memory, so it can be driven from a synthesised
    /// tone with no audio device anywhere.
    ///
    /// **Rule 1.** Silence emits one `overlay-remove` and then nothing at all.
    #[allow(clippy::too_many_arguments)]
    fn frame(
        &mut self,
        n: usize,
        cfg: &VisualCfg,
        theme: &Theme,
        // Neither treatment this engine selects draws text — the one that does
        // is the opt-in `Chassis`, which no config key reaches — but the card
        // signatures take a stack because that one needs it.
        fonts: &mut FontStack,
        geoms: &[OutputGeom],
        repush: bool,
        now: Instant,
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
            // Drop the envelopes with it: coming back from a minute of silence
            // must not resume a half-fallen peak cap from before it.
            self.motion.reset();
            self.stepped_at = None;
            if self.shown {
                self.shown = false;
                self.bmp.forget();
                out.push(WidgetUpdate::remove(VISUALIZER_OVERLAY));
            }
            return;
        }
        let period = visual_period(cfg.fps);
        self.next_frame = now + period;

        let style = &cfg.style;
        let variant = visual_variant(style);
        let opacity = f32::from(style.opacity) / 255.0;
        let paint = bar_paint(style, theme);
        let bars_theme = bars_only(theme);
        let chrome_theme = chrome_only(theme);
        let anchor = style.anchor;
        let margin = style.margin_px;

        // Real elapsed time, so the envelopes look the same at 24 Hz and at the
        // 4 Hz the widget drops to when it is nearly quiet.
        let dt = self
            .stepped_at
            .map_or(period, |t| now.saturating_duration_since(t));
        self.stepped_at = Some(now);

        // Disjoint borrows again: `push` takes the bitmap state, the closure
        // takes everything that draws.
        let VisualState {
            analyzer,
            motion,
            chrome,
            canvas,
            layer,
            bmp,
            ..
        } = self;
        let bands = analyzer.bands();

        // What the picture is *of*, and nothing that moves: the spectrum itself
        // is the animation, and it is `stepped`. A band magnitude in here would
        // rasterise at the loop rate and defeat the whole rate cap.
        let key = ContentKey::of((bands.len(), variant == cards::VisualizerVariant::Bare));
        // The chrome is only valid until something that is not a band changes.
        let stale = bmp.dirty;

        let drawn = bmp.push(
            Push {
                overlay_id: VISUALIZER_OVERLAY,
                key,
                repush,
                stepped: true,
                now,
                // `VisualState::next_frame` is the single rate authority for
                // this widget — it also gates the capture read and the FFT, and
                // two caps would only ever disagree.
                period: Duration::ZERO,
                retry: VISUAL_RETRY,
            },
            geoms,
            |geom| {
                let scale = widgetkit::scale_for_output(geom.h);
                let screen_w = geom.w as f32 / scale;
                let (w, h) = visualizer::box_size(style, screen_w);
                // The envelopes need the bar area's height to turn a fall
                // quoted in logical units into one in the renderer's 0..1. The
                // box height is within a few units of it and does not depend on
                // the card's private padding.
                motion.advance(bands, dt, period, h);

                let data = cards::VisualizerData {
                    bands: motion.levels(),
                    // Caller-owned, never allocated by the renderer.
                    peaks: Some(motion.peaks()),
                    width: w,
                    height: h,
                    width_pct: style.width_pct,
                    opacity,
                    rounded: style.rounded,
                    paint,
                    variant,
                    ..cards::VisualizerData::default()
                };
                let size = cards::visualizer::measure(&mut *fonts, theme, &data, scale);
                let buffer = size.buffer();
                let card = size.card_rect();
                let px_w = (buffer.w * scale).ceil().max(1.0) as u32;
                let px_h = (buffer.h * scale).ceil().max(1.0) as u32;

                // -- the cached chrome, for the treatment that has a card -----
                let want_chrome = variant == cards::VisualizerVariant::Panel;
                if !want_chrome {
                    *chrome = None;
                } else {
                    let fits = chrome
                        .as_ref()
                        .is_some_and(|c| c.w == px_w && c.h == px_h && c.scale == scale);
                    if stale || !fits {
                        let empty = cards::VisualizerData {
                            bands: &[],
                            peaks: None,
                            ..data
                        };
                        let cv = canvas.get(buffer, scale)?;
                        cards::visualizer::draw_at(cv, &mut *fonts, &chrome_theme, &empty, card);
                        let mut data = Vec::new();
                        cv.write_bgra(&mut data);
                        *chrome = Some(ChromeLayer {
                            w: px_w,
                            h: px_h,
                            scale,
                            data,
                        });
                    }
                }

                // -- the bars, every frame ------------------------------------
                let cv = canvas.get(buffer, scale)?;
                cards::visualizer::draw_at(cv, &mut *fonts, &bars_theme, &data, card);
                let bgra = match chrome.as_ref() {
                    Some(chrome) if chrome.data.len() == (px_w as usize * px_h as usize * 4) => {
                        cv.write_bgra(layer);
                        let mut data = chrome.data.clone();
                        over_in_place(&mut data, layer);
                        Bgra {
                            w: px_w,
                            h: px_h,
                            data,
                        }
                    }
                    // The bare treatment, and the one frame after a failed
                    // chrome build: the bar layer is the whole picture.
                    _ => cv.to_bgra(),
                };
                let (x, y) = place(&size, anchor, margin, scale, geom);
                Some(BitmapFrame { bgra, x, y })
            },
            out,
        );
        if drawn > 0 {
            self.shown = true;
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
// The lyric / now-playing card
// ---------------------------------------------------------------------------

/// Shortest gap between two lyric frames. See [`CLOCK_PERIOD`] — this is a
/// floor on bursts, not a frame rate: the lyric changes on `.lrc` timestamps
/// and nothing else.
const LYRICS_PERIOD: Duration = Duration::from_millis(200);

/// Back-off after a failed lyric frame write. See [`DISC_RETRY`].
const LYRICS_RETRY: Duration = Duration::from_secs(2);

/// The lyric widget's pixels: what is on screen, and the surface it is drawn on.
///
/// The *state machine* is [`LyricsRuntime`]'s and is untouched by the move to a
/// bitmap — line selection, offsets, the pause freeze and the "unchanged ⇒
/// [`Action::Idle`]" fast path all still live there. This is only the part that
/// is about pixels.
struct LyricsView {
    /// The words currently on screen. `None` = the overlay should be empty.
    frame: Option<lyrics_runtime::LyricFrame>,
    /// Reused across frames; see [`Surface`].
    canvas: Surface,
    /// Files, per-output placement, the redraw gate and the rate cap.
    bmp: BitmapState,
}

impl LyricsView {
    fn new(now: Instant) -> Self {
        LyricsView {
            frame: None,
            canvas: Surface::default(),
            bmp: BitmapState::new("widget-lyrics", now),
        }
    }
}

/// The lyric type size the card is drawn at, in logical units.
///
/// The preset survives the move to a card only as far as size. Face, fill,
/// outline and the fake "panel" that `LyricStylePreset::Card` was built out of
/// are all the palette's business now, and the palette does them properly — a
/// real translucent card with a real scrim, rather than near-black text inside a
/// heavy near-white outline. What is left that a card *can* honour is
/// `Karaoke`'s extra quarter of a size, which was always the loudest thing
/// about it.
fn lyric_font_size(cfg: &config::Lyrics) -> f32 {
    let size = match cfg.style {
        config::LyricStylePreset::Karaoke => cfg.font_size_pt.saturating_mul(5) / 4,
        _ => cfg.font_size_pt,
    };
    size.clamp(8, 200) as f32
}

// ---------------------------------------------------------------------------
// The clock
// ---------------------------------------------------------------------------

/// Shortest gap between two clock frames.
///
/// Not a frame rate — the clock is not animated, and its redraw gate is the
/// content key, which moves once a minute (or once a second with seconds on).
/// This only stops a burst of config edits rasterising four times in a tick.
const CLOCK_PERIOD: Duration = Duration::from_millis(200);

/// Back-off after a failed clock frame write. See [`DISC_RETRY`].
const CLOCK_RETRY: Duration = Duration::from_secs(2);

/// The clock's runtime: what it currently says, and where those pixels are.
///
/// Everything substrate-shaped is [`BitmapState`]'s. What is left here is the
/// two things that are about a clock: the rows it is showing, and the canvas
/// they are drawn on.
struct ClockState {
    /// The rows the card is drawing. `None` until the first render; refreshed
    /// only when `clock_due` says the visible text has changed, which is what
    /// makes an idle minute cost no allocation at all.
    text: Option<clock::ClockText>,
    /// Reused across frames; see [`Surface`].
    canvas: Surface,
    /// Files, per-output placement, the redraw gate and the rate cap.
    bmp: BitmapState,
}

impl ClockState {
    fn new(now: Instant) -> Self {
        ClockState {
            text: None,
            canvas: Surface::default(),
            bmp: BitmapState::new("widget-clock", now),
        }
    }
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
    /// Prepared source art for the current track, from [`Track::art`].
    art: Option<Arc<RgbaImage>>,
    /// [`Track::seq`] the art belongs to.
    seq: Option<u64>,
    /// The label's two lines. Drawn only on a disc large enough to carry type
    /// and opaque enough for its contrast to be bounded — see
    /// [`cards::disc`] — so on the shipped default size they are computed and
    /// not used, which costs one `clone` per track.
    title: String,
    artist: String,
    /// The per-frame surface; see [`Surface`].
    canvas: Surface,
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
    /// Files, per-output placement and the redraw gate. Everything in here is
    /// widget-agnostic; the disc contributes only the four fields above.
    bmp: BitmapState,
}

impl DiscState {
    fn new(now: Instant) -> Self {
        DiscState {
            art: None,
            seq: None,
            title: String::new(),
            artist: String::new(),
            canvas: Surface::default(),
            elapsed: Duration::ZERO,
            last: None,
            angle: 0.0,
            bmp: BitmapState::new("widget-disc", now),
        }
    }
}

// ---------------------------------------------------------------------------
// The bitmap substrate: what every bitmap widget shares
// ---------------------------------------------------------------------------

/// Largest bitmap a widget may rasterise, in pixels of **area**.
///
/// The engine-side twin of [`artwork::MAX_DISC_PX`], which caps the disc at
/// 2048 per side for exactly this reason. Square widgets are the special case;
/// a lyric bar or a visualiser is a rectangle, so the cap has to be on area or
/// it caps nothing useful. 2048² is 16 MB of premultiplied BGRA — already far
/// more than any widget should want, and the point at which a full-screen
/// visualiser at 4K (8.3 Mpx, 33 MB **per frame**) is refused instead of
/// quietly writing 400 MB/s to tmpfs.
///
/// A frame over the cap is dropped with one log line and the widget stays
/// hidden, which is the same degradation a failed write gets: visible, cheap,
/// and never a partial draw.
///
/// It is deliberately the same number as `widgetkit`'s own `MAX_CANVAS_AREA`.
/// The two guard different things — that one refuses to *allocate* a surface,
/// this one refuses to *write and push* one — and a widget built any other way
/// still hits this. If they ever have to differ, this is the outer one.
pub const MAX_WIDGET_AREA_PX: u64 = 2048 * 2048;

/// The shared font stack: **one per daemon, built once, off the tick.**
///
/// [`FontStack::system`] scans the filesystem for fonts, which is tens to
/// hundreds of milliseconds, and the stack is also the glyph cache — so a
/// per-widget or per-frame one would pay that scan repeatedly *and* throw away
/// every rasterised glyph between frames. The daemon's run loops tick every
/// 100 ms; this must never be built inside one.
///
/// It is built lazily rather than in [`WidgetEngine::new`] because the
/// overwhelmingly common case is a user who has never turned a widget on, and
/// that user must pay nothing at all. The build is primed from the **config
/// setters** — [`WidgetEngine::set_clock`] and friends, which run on
/// `Request::Apply` and at startup, not in a loop — so by the time a tick wants
/// a glyph the scan has already happened.
struct Fonts {
    stack: Option<FontStack>,
}

impl Fonts {
    fn new() -> Self {
        Fonts { stack: None }
    }

    /// Build the stack now if it is not built. Call from a config setter,
    /// never from a tick.
    fn prime(&mut self) {
        let _ = self.get();
    }

    /// The stack, building it if this is the first widget to ask.
    ///
    /// No `unwrap`: `panic = "abort"` means a panic here takes the user's
    /// wallpaper down with the daemon, and `get_or_insert_with` gives the same
    /// answer without one.
    fn get(&mut self) -> &mut FontStack {
        self.stack.get_or_insert_with(|| {
            let t = Instant::now();
            let stack = FontStack::system();
            if stack.has_fonts() {
                log::debug!(
                    "widgets: font stack ready, {} faces in {:?}",
                    stack.face_count(),
                    t.elapsed()
                );
            } else {
                // Worth a line in the journal rather than a bug report: a
                // machine with no fonts installed draws blank cards, and
                // nothing downstream can tell that from a layout bug.
                log::warn!(
                    "widgets: no system fonts were found — widget cards will draw \
                     their surfaces but no text"
                );
            }
            stack
        })
    }
}

/// One reusable rasterisation surface.
///
/// `widgetkit` is built for a caller that keeps a [`Canvas`] alive across
/// frames: `reset()` is a memset, and in that loop the steady-state allocation
/// count is zero. Building one per frame would churn roughly six bytes per
/// pixel per frame — with the visualiser at [`VISUAL_FPS`] that is tens of MB/s
/// while music plays, for nothing.
///
/// One canvas per **widget**, not per output. A mixed-DPI desktop makes it
/// resize between outputs on each frame, which reallocates; that is the rare
/// case, and the alternative (a canvas per output per widget) holds four idle
/// buffers per screen forever.
#[derive(Default)]
struct Surface {
    canvas: Option<Canvas>,
    /// One log line per failing size, not one per frame.
    warned: bool,
}

impl Surface {
    /// A cleared canvas of exactly `size` logical units at `scale`, reusing the
    /// buffer whenever the device size has not moved.
    ///
    /// `None` when the surface is larger than `widgetkit` will allocate — which
    /// is a refusal, not a clamp: a widget that asked for a full-screen 4K
    /// buffer wants to know it did, and `BitmapState`'s own area cap would
    /// refuse the frame a moment later anyway.
    fn get(&mut self, size: Size, scale: f32) -> Option<&mut Canvas> {
        let w = (size.w * scale).ceil().max(1.0) as u32;
        let h = (size.h * scale).ceil().max(1.0) as u32;
        let fits = self
            .canvas
            .as_ref()
            .is_some_and(|c| c.width_px() == w && c.height_px() == h && c.scale() == scale);
        if !fits {
            let resized = match &mut self.canvas {
                Some(c) => c.resize(w, h, scale).is_ok(),
                None => false,
            };
            if !resized {
                self.canvas = None;
                match Canvas::new(w, h, scale) {
                    Ok(c) => self.canvas = Some(c),
                    Err(e) => {
                        if !self.warned {
                            self.warned = true;
                            log::warn!("widgets: cannot rasterise a {w}x{h} widget: {e:#}");
                        }
                        return None;
                    }
                }
            }
        }
        self.warned = false;
        let canvas = self.canvas.as_mut()?;
        canvas.reset();
        Some(canvas)
    }
}

/// Top-left corner, in **output pixels**, of the buffer holding a measured card.
///
/// The one subtlety of the whole bitmap path, and the one that is invisible at
/// 1x: `margin_px` is measured to the **card**, never to the buffer edge. The
/// buffer is the card plus its shadow bleed on all four sides — 52 logical
/// units in dark mode and 84 in light, which at 4K is 104 and 168 *device*
/// pixels — so anchoring the buffer instead makes every widget drift inward as
/// density rises, by an amount nobody can explain from the config.
///
/// So the card is anchored, and the buffer's corner is the card's corner minus
/// the bleed.
fn place(
    size: &WidgetSize,
    anchor: Anchor,
    margin_px: u32,
    scale: f32,
    geom: &OutputGeom,
) -> (i32, i32) {
    let card = size.card_rect();
    let cw = (card.w * scale).round().max(1.0) as u32;
    let ch = (card.h * scale).round().max(1.0) as u32;
    let margin = (margin_px as f32 * scale)
        .round()
        .clamp(0.0, f32::from(u16::MAX)) as u32;
    let (x, y) = anchor_xy(anchor, cw, ch, margin, geom.w, geom.h);
    let bleed = (size.bleed * scale).round() as i32;
    (x - bleed, y - bleed)
}

/// A cheap stand-in for a bitmap widget's pixels.
///
/// **Why this type exists.** The ASS widgets detect "did anything visibly
/// change" by comparing the *rendered string*, which is free because the string
/// is what gets pushed anyway. That trick dies with bitmaps: the payload is
/// megabytes, comparing it costs more than redrawing, and it does not even
/// exist until after the expensive step. So a bitmap widget instead hands the
/// engine a small key derived from its **content**, and the engine rasterises
/// only when the key moves.
///
/// A good key is whatever the renderer reads and nothing else — the clock's is
/// its formatted time (which it already computes), a lyric's is the line plus
/// its highlight, the disc's is the track and the geometry it was drawn at.
///
/// The key must **not** include a continuously animating quantity. The disc's
/// angle changes every tick; folding it in here would defeat [`DISC_FPS`] and
/// redraw at the loop rate. Animation is the separate, rate-capped `stepped`
/// input to `BitmapState::push`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentKey(u64);

impl ContentKey {
    /// Hash anything hashable into a key.
    pub fn of<T: std::hash::Hash>(value: T) -> Self {
        use std::hash::Hasher as _;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut h);
        ContentKey(h.finish())
    }
}

/// One rasterised frame, ready to be written and placed.
///
/// What a bitmap widget's render closure hands back: the pixels, and where on
/// *this* output their top-left corner goes. Placement is the widget's own
/// because only the widget knows how big the box turned out — a lyric line's
/// width is not known until it has been laid out.
pub struct BitmapFrame {
    /// Premultiplied BGRA, tightly packed.
    pub bgra: Bgra,
    /// Left edge in output pixels.
    pub x: i32,
    /// Top edge in output pixels.
    pub y: i32,
}

/// One output's frame file and what we believe is on that output.
struct BitmapSlot {
    /// The output this slot draws on.
    geom: OutputGeom,
    /// The file mpv reads pixels from for this output.
    path: PathBuf,
    /// What the pixels in `path` are of. `None` = nothing written yet.
    key: Option<ContentKey>,
    /// The frame currently on screen here. `None` = we believe nothing is up.
    shown: Option<BitmapOverlay>,
}

/// The reusable runtime of a **bitmap** widget: its files, its per-output
/// placement, its redraw gate and its rate cap.
///
/// One of these per bitmap widget, holding everything that is not about what
/// the widget draws. The disc had all of it inline; four widgets sharing one
/// copy is the point of the type.
///
/// # The redraw gate
///
/// Five questions, cheapest first, and all five have to say no for the frame to
/// be free:
///
/// 1. `repush` — the renderer lost our overlays. Re-issues `overlay-add` from
///    the file already on disk: **no rasterising at all**, see
///    [`WidgetEngine::invalidate`].
/// 2. `dirty` — config, output geometry or anything else that moves the box.
///    This is what the disc's old `misplaced` check was really testing.
/// 3. `shown.is_none()` — we have pushed nothing to this output yet.
/// 4. the [`ContentKey`] moved — the picture is of something else now.
/// 5. `stepped` — a rate-capped animation says the picture moved.
struct BitmapState {
    /// Base path; slot *n*'s file is `{stem}-{n}.bgra`.
    stem: PathBuf,
    /// One per output currently driven, in [`WidgetEngine::outputs`] order.
    slots: Vec<BitmapSlot>,
    /// Earliest instant the next frame may be drawn.
    next_frame: Instant,
    /// Something about the content or the geometry changed and the next frame
    /// must be drawn whatever the rate cap says.
    dirty: bool,
    /// One log line per failing state, not one per frame.
    warned: bool,
    /// Set while a failed write is backing off; nothing rasterises until then.
    retry_at: Option<Instant>,
}

/// Everything [`BitmapState::push`] needs that is not about *what* the widget
/// draws: which overlay, what the pixels are of, and when it is allowed to.
///
/// A struct rather than seven arguments because the four bitmap widgets will
/// each fill it in, and a positional `bool, bool, Instant, Duration, Duration`
/// is a transposition waiting to happen.
struct Push {
    /// mpv overlay id.
    overlay_id: u32,
    /// What the pixels are of. See [`ContentKey`].
    key: ContentKey,
    /// Re-issue `overlay-add` from the file already on disk, without
    /// rasterising. See [`WidgetEngine::invalidate`].
    repush: bool,
    /// A rate-capped animation says the picture moved. Kept out of `key` on
    /// purpose — see [`ContentKey`].
    stepped: bool,
    /// Monotonic now.
    now: Instant,
    /// Shortest gap between two frames of this widget.
    period: Duration,
    /// Back-off after a write that failed, which is much longer.
    retry: Duration,
}

impl BitmapState {
    fn new(name: &str, now: Instant) -> Self {
        BitmapState {
            stem: frame_stem(name),
            slots: Vec::new(),
            next_frame: now,
            dirty: true,
            warned: false,
            retry_at: None,
        }
    }

    /// Point this widget's frame files somewhere else, dropping what we had.
    ///
    /// Only the tests need it, and they need it badly: without redirecting the
    /// stem, a `cargo test` run writes frames into the developer's live
    /// `$XDG_RUNTIME_DIR` beside the daemon's own.
    #[cfg(test)]
    fn set_stem(&mut self, stem: PathBuf) {
        self.stem = stem;
        self.slots.clear();
        self.dirty = true;
    }

    /// Whether anything of ours is believed to be on screen anywhere.
    fn is_shown(&self) -> bool {
        self.slots.iter().any(|s| s.shown.is_some())
    }

    /// Bring the slot list in line with the outputs the loop is driving.
    ///
    /// An output that went away takes its overlay with it (its renderer is
    /// gone), so a dropped slot needs no `overlay-remove`. An output whose mode
    /// changed is a new geometry and therefore a redraw.
    fn sync(&mut self, outputs: &[OutputGeom]) {
        if self.slots.len() == outputs.len()
            && self.slots.iter().zip(outputs).all(|(s, g)| &s.geom == g)
        {
            return;
        }
        let mut old: Vec<BitmapSlot> = std::mem::take(&mut self.slots);
        // Keep a slot whose geometry is unchanged: its file already holds the
        // right pixels, so a hotplug elsewhere costs it nothing.
        let mut kept: Vec<Option<BitmapSlot>> = outputs
            .iter()
            .map(|g| old.iter().position(|s| s.geom == *g).map(|i| old.remove(i)))
            .collect();
        // A kept slot brings its file with it, and that file's index is now
        // spoken for: handing the same name to a new slot would have two
        // outputs writing each other's pixels.
        let used: Vec<PathBuf> = kept.iter().flatten().map(|s| s.path.clone()).collect();
        let mut n = 0usize;
        for (i, entry) in kept.iter_mut().enumerate() {
            if entry.is_some() {
                continue;
            }
            while used.contains(&slot_path(&self.stem, n)) {
                n += 1;
            }
            *entry = Some(BitmapSlot {
                geom: outputs[i].clone(),
                path: slot_path(&self.stem, n),
                key: None,
                shown: None,
            });
            n += 1;
        }
        self.slots = kept.into_iter().flatten().collect();
    }

    /// Forget what is on screen without emitting anything.
    ///
    /// For [`WidgetEngine::clear_all`], which has already emitted the blank
    /// itself — by kind, which is a decision only the engine can make.
    fn forget(&mut self) {
        for s in &mut self.slots {
            s.shown = None;
        }
        // Back to "we have pushed nothing": the next tick must draw again
        // rather than decide nothing has changed.
        self.dirty = true;
    }

    /// Take every overlay of ours down and forget the widget is drawable.
    fn retire(&mut self, overlay_id: u32, out: &mut Vec<WidgetUpdate>) {
        if self.is_shown() {
            out.push(WidgetUpdate::remove(overlay_id));
        }
        self.slots.clear();
    }

    /// Rasterise and push this widget wherever it needs it.
    ///
    /// `render` is called **at most once per output**, and only for the outputs
    /// the gate above says need it — that is the whole reason it is a closure
    /// and not a pre-built frame.
    ///
    /// `render` returning `None` means "nothing to draw here"; the overlay comes
    /// down on that output, once.
    ///
    /// Returns the number of outputs actually rasterised, so the caller can
    /// tell an animation step ("arm the rate cap") from a no-op.
    fn push<F>(
        &mut self,
        args: Push,
        outputs: &[OutputGeom],
        mut render: F,
        out: &mut Vec<WidgetUpdate>,
    ) -> usize
    where
        F: FnMut(&OutputGeom) -> Option<BitmapFrame>,
    {
        let Push {
            overlay_id,
            key,
            repush,
            stepped,
            now,
            period,
            retry,
        } = args;
        self.sync(outputs);
        let dirty = self.dirty;
        // A widget whose last write failed is not asked to rasterise again
        // until the back-off is up. Without this the retry cadence is the
        // *loop's*, and a read-only runtime directory costs a full render ten
        // times a second forever.
        let blocked = self.retry_at.is_some_and(|t| now < t);
        let mut drawn = 0usize;
        let mut failed = false;
        let mut removed = false;
        for slot in &mut self.slots {
            // `shown` is deliberately not in this gate: a slot that drew
            // nothing for the current key recorded the key anyway, so "we have
            // pushed nothing here" is `key == None`, which a fresh slot has.
            let changed = !blocked && (dirty || slot.key != Some(key) || stepped);
            if !changed {
                // Nothing moved. The file on disk is still exactly right, so a
                // renderer that lost its overlays gets the command again and
                // not the rasteriser.
                if repush {
                    if let Some(b) = &slot.shown {
                        out.push(WidgetUpdate::draw_on(
                            overlay_id,
                            slot.geom.target(),
                            b.clone(),
                        ));
                    }
                }
                continue;
            }
            let Some(frame) = render(&slot.geom) else {
                // Nothing to draw here. `overlay-remove` is not per output, so
                // one is enough however many slots have just gone empty.
                if slot.shown.take().is_some() && !removed {
                    removed = true;
                    out.push(WidgetUpdate::remove(overlay_id));
                }
                // Record the key anyway: "there is nothing to draw for this
                // content" is an answer, and asking again every tick until the
                // content changes is the whole cost the key exists to avoid.
                slot.key = Some(key);
                continue;
            };
            let area = u64::from(frame.bgra.w) * u64::from(frame.bgra.h);
            if area > MAX_WIDGET_AREA_PX {
                if !self.warned {
                    self.warned = true;
                    log::warn!(
                        "widgets: overlay {overlay_id} wanted a {}x{} bitmap ({area} px), over the \
                         {MAX_WIDGET_AREA_PX} px cap — the widget stays hidden",
                        frame.bgra.w,
                        frame.bgra.h
                    );
                }
                failed = true;
                continue;
            }
            if let Err(e) = write_frame(&slot.path, &frame.bgra) {
                if !self.warned {
                    self.warned = true;
                    log::warn!(
                        "widgets: cannot write the frame for overlay {overlay_id} to {}: {e} — \
                         the widget stays hidden",
                        slot.path.display()
                    );
                }
                failed = true;
                continue;
            }
            slot.key = Some(key);
            let bitmap = BitmapOverlay {
                x: frame.x,
                y: frame.y,
                path: slot.path.clone(),
                w: frame.bgra.w,
                h: frame.bgra.h,
                stride: frame.bgra.stride(),
            };
            slot.shown = Some(bitmap.clone());
            out.push(WidgetUpdate::draw_on(
                overlay_id,
                slot.geom.target(),
                bitmap,
            ));
            drawn += 1;
        }
        if failed {
            // Long backoff, so a full or read-only runtime directory costs one
            // attempt every couple of seconds rather than one every frame.
            self.retry_at = Some(now + retry);
            self.next_frame = now + retry;
            return drawn;
        }
        if !blocked {
            self.retry_at = None;
            self.warned = false;
            self.dirty = false;
        }
        if drawn > 0 {
            self.next_frame = now + period;
        }
        drawn
    }
}

/// Where a bitmap widget's frames live.
///
/// Under `$XDG_RUNTIME_DIR/fresco/` beside the control socket: a tmpfs, so the
/// per-frame write never reaches a disk, and per-user, so two people on the
/// same machine cannot collide. [`crate::ipc::socket_dir`] already carries the
/// `/tmp` fallback for a session with no runtime directory.
///
/// **Under `cfg(test)` this is redirected**, and it has to be. Four of the four
/// widgets rasterise now, so a `cargo test` run that used the real answer would
/// write frames into the developer's live `$XDG_RUNTIME_DIR/fresco/` beside the
/// running daemon's own — and, because the harness runs tests in parallel in one
/// process, two tests exercising the same widget would be writing each other's
/// pixels into one file. The counter is what keeps every [`BitmapState`] in a
/// run on its own files; the directory is stable across runs so the files are
/// overwritten rather than accumulated.
fn frame_stem(name: &str) -> PathBuf {
    #[cfg(test)]
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join("fresco-test-frames")
            .join(format!("{name}-{n}"))
    }
    #[cfg(not(test))]
    {
        crate::ipc::socket_dir().join(name)
    }
}

/// The file for one output's frames: `{stem}-{n}.bgra`.
///
/// One file per output and not one per widget, because two outputs of different
/// sizes hold genuinely different pixels — sharing would mean the second
/// `overlay-add` read the first output's frame.
fn slot_path(stem: &Path, n: usize) -> PathBuf {
    let mut p = stem.to_path_buf().into_os_string();
    p.push(format!("-{n}.bgra"));
    PathBuf::from(p)
}

/// Write one frame to the file mpv reads.
///
/// **Not** `fs::write`, which truncates first: mpv `mmap`s this file for
/// `overlay-add`, and truncating a live mapping out from under it is a SIGBUS
/// in the renderer rather than a dropped frame. The bytes are overwritten in
/// place.
///
/// # Why the file is never shortened
///
/// The old version of this function called `set_len` unconditionally, which is
/// the one path here that can shorten a live mapping. It was safe only because
/// the disc resizes on a config change and nowhere else. It is **not** safe for
/// a widget whose bitmap changes size as its content does — a lyric line is a
/// different width on every line — and the failure is the worst kind: a SIGBUS
/// inside mpv, on a machine other than the developer's, at a rate proportional
/// to how much the user likes lyrics.
///
/// The fix is to grow and never shrink. mpv maps the file when it handles
/// `overlay-add` and keeps that mapping until the next `overlay-add` on the id
/// (or `overlay-remove`), so between our write and mpv's next command there is
/// always a mapping of the *previous* frame's length still live. Growing leaves
/// it fully backed; shrinking unbacks its tail. The trailing bytes of a frame
/// smaller than its predecessor are simply never read — mpv reads `h * stride`
/// bytes, which we pass it explicitly.
///
/// The cost is that the file settles at the largest frame the widget has ever
/// drawn. That is bounded, and bounded tightly, by [`MAX_WIDGET_AREA_PX`].
///
/// The alternative — confirming that mpv re-`mmap`s on every `overlay-add` and
/// shrinking anyway — buys back only tmpfs pages we have already capped, and
/// stakes a renderer crash on an implementation detail of whichever mpv the
/// user's distribution ships. Not worth it.
fn write_frame(path: &Path, frame: &Bgra) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    let want = frame.data.len() as u64;
    // Grow first, so the write never lands past the end of a mapping mpv is
    // still holding; never shrink (see above).
    if f.metadata()?.len() < want {
        f.set_len(want)?;
    }
    f.write_all(&frame.data)?;
    f.flush()
}

/// Resolve an anchor to the top-left corner of a `w`×`h` box on a
/// `out_w`×`out_h` output, in **output pixels**.
///
/// The bitmap twin of the ASS `\an` placement the text widgets get for free.
/// `overlay-add` takes a corner, not an anchor, so somebody has to do this —
/// and doing it here rather than in the three run loops is the whole reason
/// this module exists.
///
/// `w` and `h` are separate because only the disc is square: a clock, a lyric
/// line and a spectrum are all wider than they are tall, and resolving both
/// axes against one number puts every one of them in the wrong place.
///
/// Saturating throughout: a margin or a box larger than the output pins it to
/// the edge instead of wrapping to a nonsense coordinate.
fn anchor_xy(anchor: Anchor, w: u32, h: u32, margin: u32, out_w: u32, out_h: u32) -> (i32, i32) {
    let far =
        |extent: u32, size: u32| -> u32 { extent.saturating_sub(size).saturating_sub(margin) };
    let centre = |extent: u32, size: u32| -> u32 { extent.saturating_sub(size) / 2 };
    let x = match anchor {
        Anchor::TopLeft | Anchor::MidLeft | Anchor::BottomLeft => margin.min(far(out_w, w)),
        Anchor::TopCenter | Anchor::MidCenter | Anchor::BottomCenter => centre(out_w, w),
        Anchor::TopRight | Anchor::MidRight | Anchor::BottomRight => far(out_w, w),
    };
    let y = match anchor {
        Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => margin.min(far(out_h, h)),
        Anchor::MidLeft | Anchor::MidCenter | Anchor::MidRight => centre(out_h, h),
        Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => far(out_h, h),
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
    /// Every output the widget layer is being drawn on, in the order the loop
    /// handed them over. **Never empty**: an engine nobody has told about a
    /// display assumes one unnamed 1080p output, so a loop that never calls
    /// [`WidgetEngine::set_outputs`] still gets its widgets.
    ///
    /// Only the bitmap widgets read it — `overlay-add` is in real pixels, where
    /// ASS is laid out in the fixed [`RES_X`]×[`RES_Y`] space and is
    /// resolution-independent for free — but they *must*, and per output: this
    /// is what stops a bitmap sized for the 4K screen landing at the wrong size
    /// on the 1080p one beside it.
    outputs: Vec<OutputGeom>,
    /// The app accent, as the enum rather than a hex string: the dark and
    /// light palettes use **different** colours for the same accent, and
    /// [`Theme::for_accent`] is the only thing that knows both tables.
    accent: config::Accent,
    /// Which palette the cards are drawn in, straight from the config.
    theme_cfg: config::WidgetTheme,
    /// The resolved palette. Recomputed whenever the accent or the mode moves,
    /// never per frame — it is a table of thirty-odd colours and every one of
    /// them is a fixed function of those two inputs.
    theme: Theme,
    /// The shared font stack and glyph cache. See [`Fonts`].
    fonts: Fonts,
    /// Line selection, offsets, presets and the "unchanged ⇒ [`Action::Idle`]"
    /// fast path all live in here; this module never re-implements any of it.
    lyrics: LyricsRuntime,
    /// `Some` exactly while the lyric widget is enabled.
    worker: Option<Worker>,
    /// [`Track::seq`] of the track currently loaded into `lyrics`.
    track_seq: Option<u64>,
    /// `Some` exactly while the lyric widget is enabled.
    lyrics_view: Option<LyricsView>,
    /// `Some` exactly while the clock is enabled.
    clock: Option<ClockState>,
    /// `Some` exactly while the visualiser is enabled. Holds the capture, so
    /// dropping it stops recording.
    visual: Option<VisualState>,
    /// `Some` exactly while the disc is enabled.
    disc: Option<DiscState>,
    /// Wall-clock instant at which the clock's *text* next differs. Between now
    /// and then, the clock branch does not render, allocate or compare.
    clock_due: Option<DateTime<Local>>,
    /// Re-*push* everything on the next tick, whatever we believe is on screen
    /// — see [`WidgetEngine::invalidate`]. Deliberately not "re-render": the
    /// pixels on disk are still valid, and re-rasterising four bitmap widgets
    /// because an output's respawn counter flapped is a visible hitch.
    repush: bool,
    /// Which mpv command owns each overlay id right now, indexed by
    /// [`overlay_slot`]. Recorded from what was actually emitted, because a
    /// widget can change kind at runtime and only the emitted command knows.
    /// [`clear_all`](WidgetEngine::clear_all) reads it to pick between an empty
    /// ASS payload and `overlay-remove`.
    on_screen: [Option<OverlayKind>; OVERLAY_SLOTS],
}

/// How many overlay ids this engine owns. See [`overlay_slot`].
const OVERLAY_SLOTS: usize = 4;

/// Index of `overlay_id` in the engine's per-overlay arrays, or `None` for an
/// id this engine does not own.
///
/// Spelled out rather than computed from the id, so adding a fifth widget is a
/// compile-time decision here instead of an off-by-one somewhere else.
fn overlay_slot(overlay_id: u32) -> Option<usize> {
    match overlay_id {
        LYRICS_OVERLAY => Some(0),
        CLOCK_OVERLAY => Some(1),
        VISUALIZER_OVERLAY => Some(2),
        DISC_OVERLAY => Some(3),
        _ => None,
    }
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
    pub fn new(cfg: Option<&config::Widgets>, accent: config::Accent) -> Self {
        let mut engine = WidgetEngine {
            lyrics_cfg: config::Lyrics::default(),
            clock_cfg: ClockCfg::default(),
            visual_cfg: VisualCfg::default(),
            disc_cfg: DiscWidgetCfg::default(),
            monitor: None,
            outputs: vec![OutputGeom {
                connector: String::new(),
                w: 1920,
                h: 1080,
            }],
            accent: config::Accent::default(),
            theme_cfg: config::WidgetTheme::default(),
            theme: Theme::for_accent(Mode::Dark, config::Accent::default()),
            fonts: Fonts::new(),
            lyrics: LyricsRuntime::new(&config::Lyrics::default()),
            worker: None,
            track_seq: None,
            lyrics_view: None,
            clock: None,
            visual: None,
            disc: None,
            clock_due: None,
            repush: false,
            on_screen: [None; OVERLAY_SLOTS],
        };
        engine.set_config(cfg, accent);
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
    pub fn set_config(&mut self, cfg: Option<&config::Widgets>, accent: config::Accent) {
        let lyrics_cfg = cfg.map(|w| w.lyrics.clone()).unwrap_or_default();
        self.monitor = cfg.and_then(|w| w.monitor.clone());
        let theme_cfg = cfg.map_or_else(config::WidgetTheme::default, |w| w.theme);

        if self.accent != accent || self.theme_cfg != theme_cfg {
            self.accent = accent;
            self.theme_cfg = theme_cfg;
            // A palette is a fixed function of (mode, accent), so it is
            // resolved here and never in a frame.
            self.theme = Theme::for_accent(widget_mode(theme_cfg), accent);
            // Every card on screen is now drawn in the wrong colours, and a
            // content key cannot see a palette change.
            self.mark_bitmaps_dirty();
        }

        // `LyricsRuntime::set_config` owns the "did anything visible change"
        // decision for lyrics, including ignoring a no-op edit.
        let lyrics_changed = lyrics_cfg != self.lyrics_cfg;
        self.lyrics.set_config(&lyrics_cfg);
        self.lyrics_cfg = lyrics_cfg;
        if self.lyrics_cfg.enabled {
            if self.lyrics_view.is_none() {
                self.lyrics_view = Some(LyricsView::new(Instant::now()));
            }
            // Off the tick: this is a config setter, not a loop.
            self.fonts.prime();
        }
        if lyrics_changed {
            // A style change a content key cannot see, so the lyric widget has
            // to be told to repaint. **Only the lyric widget**: the accent and
            // the theme are handled above, and dirtying all four here would
            // make a lyric anchor edit re-rasterise the clock, the spectrum and
            // the record as well — a rule-1 violation with nothing on the other
            // side of it.
            if let Some(l) = &mut self.lyrics_view {
                l.bmp.dirty = true;
            }
        }
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

    /// Tell the engine every output the widget layer is being drawn on, in the
    /// order the loop will dispatch to them.
    ///
    /// **Call this before [`tick`](Self::tick), every tick.** Only the bitmap
    /// widgets read it, and they must: `overlay-add` places pixels in **real
    /// output pixels**, where the ASS widgets are laid out in the fixed
    /// [`RES_X`]×[`RES_Y`] space and are resolution-independent for free. A
    /// mixed-DPI desktop given one monitor's size gets a widget at the wrong
    /// size and in the wrong place on the other.
    ///
    /// The engine stays **one state machine**: which lyric line, what angle,
    /// what the clock reads are decided once. Only rasterisation and placement
    /// are per output, and the updates come back tagged with
    /// [`WidgetUpdate::target`] so the loop can route them.
    ///
    /// Outputs with a zero dimension are dropped (a compositor reporting a mode
    /// it has not brought up yet); if that leaves nothing, the previous list is
    /// kept, because a widget placed against a 0×0 output would land in the
    /// corner and stay there.
    pub fn set_outputs(&mut self, outputs: &[OutputGeom]) {
        let want: Vec<OutputGeom> = outputs
            .iter()
            .filter(|g| g.w != 0 && g.h != 0)
            .cloned()
            .collect();
        if want.is_empty() || want == self.outputs {
            return;
        }
        self.outputs = want;
        self.mark_bitmaps_dirty();
    }

    /// [`set_outputs`](Self::set_outputs) for a loop that drives exactly one
    /// output and has no name for it.
    ///
    /// Kept because it is the smallest thing a call site can do to place a
    /// bitmap correctly, and because updates for an unnamed output carry no
    /// [`WidgetUpdate::target`] — so a loop that only ever calls this needs no
    /// routing at all.
    pub fn set_output_size(&mut self, w: u32, h: u32) {
        self.set_outputs(&[OutputGeom {
            connector: String::new(),
            w,
            h,
        }]);
    }

    /// The geometry every bitmap widget is currently placed against. Never
    /// empty.
    fn out_geoms(&self) -> &[OutputGeom] {
        &self.outputs
    }

    /// Force every bitmap widget to re-rasterise on the next tick.
    ///
    /// The generalisation of the disc's old `dirty = true`: anything that moves
    /// a box — a config edit, a new output list, a mode change — goes through
    /// here, and adding a bitmap widget means adding one line to it.
    fn mark_bitmaps_dirty(&mut self) {
        if let Some(l) = &mut self.lyrics_view {
            l.bmp.dirty = true;
        }
        if let Some(c) = &mut self.clock {
            c.bmp.dirty = true;
        }
        if let Some(v) = &mut self.visual {
            v.bmp.dirty = true;
        }
        if let Some(d) = &mut self.disc {
            d.bmp.dirty = true;
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
        if self.clock_cfg.enabled {
            match &mut self.clock {
                // The frame on disk is now of the wrong thing — a content key
                // cannot see a style change. **Only this widget's frame**:
                // dirtying all four here would make a clock edit re-rasterise
                // the lyric card, the spectrum and the record for nothing.
                Some(c) => c.bmp.dirty = true,
                None => self.clock = Some(ClockState::new(Instant::now())),
            }
            // Off the tick: this is a config setter, and the font scan is tens
            // to hundreds of milliseconds.
            self.fonts.prime();
        }
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
            // for the spectrum to happen to differ. The chrome cache goes with
            // it — a colour change a content key cannot see is exactly what it
            // would otherwise hold on to.
            v.bmp.dirty = true;
        }
        if self.visual_cfg.enabled {
            self.fonts.prime();
        }
        // No `mark_bitmaps_dirty` here: both arms above have already dirtied
        // *this* widget (a restart builds a fresh `BitmapState`, which starts
        // dirty), and a visualiser edit is not a reason to redraw the other
        // three.
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
                Some(disc) => disc.bmp.dirty = true,
                None => self.disc = Some(DiscState::new(now)),
            }
            // Off the tick: this is a config setter, not a loop.
            self.fonts.prime();
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
    /// **Nothing is pushed for a widget that has never drawn.** The ASS engine
    /// opened with one blank per text overlay, on the grounds that an overlay
    /// left by a previous daemon run might still be up. A bitmap widget cannot
    /// honestly say that: `BitmapState` tracks what it actually put on each
    /// output, and an `overlay-remove` for a frame we never drew is an IPC round
    /// trip that changes nothing. The case the blank was defending against is
    /// [`clear_all`](Self::clear_all)'s, which still blanks **unconditionally**
    /// — and which is what the loop calls on renderer teardown and on wallpaper
    /// swap, i.e. at every point where our belief could be wrong.
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
        let live = [
            (LYRICS_OVERLAY, self.lyrics_cfg.enabled),
            (CLOCK_OVERLAY, self.clock_cfg.enabled),
            (VISUALIZER_OVERLAY, self.visual_cfg.enabled),
            (DISC_OVERLAY, self.disc_cfg.enabled),
        ];
        for (id, enabled) in live {
            let Some(slot) = overlay_slot(id) else {
                continue;
            };
            let on = self.on_screen[slot];
            if !enabled && on.is_none() {
                continue;
            }
            // **Defect 1.** The command is chosen by what is *on the overlay*,
            // never by which widget it is: an empty `osd-overlay` does not take
            // a bitmap down and `overlay-remove` does not take ASS down, so
            // guessing leaves a stale widget burned onto the next wallpaper.
            // Falling back to `substrate` covers the one case where there is
            // nothing to read — an overlay we believe is already blank, which
            // is cleared anyway precisely because that belief may be wrong.
            out.push(WidgetUpdate::blank(id, on.unwrap_or(self.substrate(id))));
        }
        if let Some(l) = &mut self.lyrics_view {
            l.bmp.forget();
        }
        self.clock_due = None;
        if let Some(c) = &mut self.clock {
            c.bmp.forget();
        }
        if let Some(v) = &mut self.visual {
            v.bmp.forget();
            v.shown = false;
        }
        if let Some(d) = &mut self.disc {
            d.bmp.forget();
        }
        // Back to "we have pushed nothing", not to "the overlay is empty": the
        // next tick must re-adopt the track rather than assume it still holds.
        self.lyrics.clear();
        self.track_seq = None;
        self.repush = false;
        self.on_screen = [None; OVERLAY_SLOTS];
        out
    }

    /// Which overlay kind a widget rasterises into when it has content.
    ///
    /// **The seam a widget port flips.** All four widgets rasterise today;
    /// moving one back to text — or adding a fifth that is text — means
    /// changing its renderer and this one arm. Everything else — clearing,
    /// per-output placement, deadlines — already reads the kind that was
    /// actually pushed (`on_screen`), so a widget that falls back from one to
    /// the other at runtime stays correct without touching this at all.
    fn substrate(&self, overlay_id: u32) -> OverlayKind {
        match overlay_id {
            LYRICS_OVERLAY | CLOCK_OVERLAY | VISUALIZER_OVERLAY | DISC_OVERLAY => {
                OverlayKind::Bitmap
            }
            _ => OverlayKind::Ass,
        }
    }

    /// Record what a batch of updates leaves on each overlay, and insert the
    /// blank that a **kind change** needs.
    ///
    /// A widget that switches substrate mid-run (a bitmap renderer failing back
    /// to ASS, say) would otherwise leave the old overlay up underneath the new
    /// one: mpv keeps `osd-overlay` and `overlay-add` in separate namespaces,
    /// so pushing one never displaces the other. One blank of the outgoing kind
    /// goes out first, and then everything downstream just works.
    fn note_pushed(&mut self, out: &mut Vec<WidgetUpdate>) {
        // Verdict per overlay: `None` = untouched, `Some(k)` = what this batch
        // leaves on it. A batch can carry one update per output, so a single
        // draw anywhere outweighs a remove elsewhere.
        let mut verdict: [Option<Option<OverlayKind>>; OVERLAY_SLOTS] = [None; OVERLAY_SLOTS];
        for u in out.iter() {
            let Some(slot) = overlay_slot(u.overlay_id) else {
                continue;
            };
            let kind = u.kind();
            verdict[slot] = Some(match (verdict[slot].flatten(), kind) {
                (Some(prev), None) => Some(prev),
                (_, k) => k,
            });
        }
        for (slot, want) in verdict.iter().enumerate() {
            let Some(want) = *want else { continue };
            let (Some(prev), Some(want)) = (self.on_screen[slot], want) else {
                self.on_screen[slot] = want;
                continue;
            };
            if prev != want {
                let id = out
                    .iter()
                    .find(|u| overlay_slot(u.overlay_id) == Some(slot))
                    .map(|u| u.overlay_id)
                    .expect("the verdict came from an update");
                let at = out
                    .iter()
                    .position(|u| u.overlay_id == id)
                    .expect("same update");
                out.insert(at, WidgetUpdate::blank(id, prev));
            }
            self.on_screen[slot] = Some(want);
        }
    }

    /// Re-**push** what is on screen on the next [`tick`](Self::tick).
    ///
    /// For the cases where the renderer lost our overlays without us clearing
    /// them: a respawned mpv (which starts with none), and a rotation change
    /// (W0: the OSD coordinate space follows the video's render area, so the
    /// payload must be pushed again against the new one).
    ///
    /// **This re-pushes; it does not re-render.** The distinction is invisible
    /// while every widget is a string, and expensive once they are pixels: the
    /// Wayland loop calls this whenever any output's respawn generation moves,
    /// which can flap, and re-rasterising four bitmap widgets on each flap is a
    /// visible hitch for no gain. The files on disk are still valid frames of
    /// exactly the right thing, so what goes out is the `overlay-add` command
    /// again — not the rasteriser. Same for the ASS widgets: the stored payload
    /// is re-sent, and the clock is not re-formatted.
    ///
    /// Anything that genuinely invalidates the *pixels* — a config edit, a new
    /// output list — already marks the widgets dirty at the setter that
    /// caused it, and is not this.
    ///
    /// Only overlays that *have* content are re-pushed — re-sending a blank to a
    /// renderer that never had the overlay is a wasted IPC round trip, and after
    /// [`clear_all`](Self::clear_all) there is by definition nothing to restore.
    pub fn invalidate(&mut self) {
        self.repush = true;
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
        let repush = self.repush;
        let theme = self.theme;
        let geoms = self.outputs.clone();
        let fonts = &mut self.fonts;
        let Some(v) = &mut self.visual else {
            return out;
        };
        v.live = true;
        if now >= v.next_frame {
            let n = samples.len().min(v.scratch.len());
            v.scratch[..n].copy_from_slice(&samples[..n]);
            v.frame(
                n,
                &self.visual_cfg,
                &theme,
                fonts.get(),
                &geoms,
                repush,
                now,
                &mut out,
            );
        }
        self.repush = false;
        self.note_pushed(&mut out);
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
            self.lyrics_tick(position_us, status, now, &mut out);
        }

        if let Some(wall) = wall {
            self.clock_tick(wall, now, &mut out);
        }

        self.visual_tick(now, &mut out);
        self.disc_tick(snapshot, now, &mut out);

        self.repush = false;
        self.note_pushed(&mut out);
        out
    }

    /// Advance the lyric / now-playing card.
    ///
    /// The state machine's answer is [`LyricsRuntime::tick`]'s and nothing here
    /// second-guesses it: `Show` means the words changed, `Clear` means there
    /// is nothing to say, and `Idle` — the answer to ~99% of ticks — means the
    /// picture on disk is still of exactly the right thing.
    ///
    /// **What is deliberately not on this card.** `NowPlayingData` can carry a
    /// progress bar, an elapsed/total readout and the album art, and all three
    /// are left empty. The first two move every second, which would turn a
    /// widget that wakes on `.lrc` timestamps — one wake per line, one per
    /// 30-second instrumental gap — into a permanent 1 Hz rasterise-and-write
    /// for as long as anything is playing. That is the power model this whole
    /// module is built around, and it is not worth a readout the player already
    /// shows. The art is absent for a different reason: the worker only fetches
    /// cover art when the *disc* widget is on, and making the lyric widget pull
    /// art would put a network fetch behind a switch that never promised one.
    fn lyrics_tick(
        &mut self,
        position_us: i64,
        status: PlaybackStatus,
        now: Instant,
        out: &mut Vec<WidgetUpdate>,
    ) {
        let repush = self.repush;
        let cfg = self.lyrics_cfg.clone();
        let geoms = self.outputs.clone();
        let theme = self.theme;
        let action = self.lyrics.tick(position_us, status);

        let fonts = &mut self.fonts;
        let Some(state) = &mut self.lyrics_view else {
            return;
        };
        if !cfg.enabled {
            // Switched off while it was on screen: take it down, once.
            state.bmp.retire(LYRICS_OVERLAY, out);
            self.lyrics_view = None;
            return;
        }
        match action {
            Action::Show(frame) => state.frame = Some(frame),
            Action::Clear => state.frame = None,
            Action::Idle => {}
        }

        let LyricsView { frame, canvas, bmp } = state;
        // `None` is a content state, not an absence of one: it is what makes
        // "there is nothing to draw for this track" cost one `overlay-remove`
        // rather than one question per tick.
        let key = ContentKey::of(&*frame);
        let font_size = lyric_font_size(&cfg);
        let anchor = widget_anchor(cfg.anchor);
        bmp.push(
            Push {
                overlay_id: LYRICS_OVERLAY,
                key,
                repush,
                stepped: false,
                now,
                period: LYRICS_PERIOD,
                retry: LYRICS_RETRY,
            },
            &geoms,
            |geom| {
                let frame = frame.as_ref()?;
                let scale = widgetkit::scale_for_output(geom.h);
                let data = cards::NowPlayingData {
                    label: &frame.label,
                    title: &frame.title,
                    artist: &frame.artist,
                    lyric: &frame.lyric,
                    next_lyric: &frame.next_lyric,
                    font_size,
                    accent_follow: cfg.accent_follow,
                    // So the card can clamp itself to 0.9 of the screen. In
                    // logical units, like everything else it is handed.
                    screen_width: geom.w as f32 / scale,
                    ..cards::NowPlayingData::default()
                };
                let fonts = fonts.get();
                let size = cards::nowplaying::measure(fonts, &theme, &data, scale);
                let canvas = canvas.get(size.buffer(), scale)?;
                cards::nowplaying::draw_at(canvas, fonts, &theme, &data, size.card_rect());
                let (x, y) = place(&size, anchor, cfg.margin_px, scale, geom);
                Some(BitmapFrame {
                    bgra: canvas.to_bgra(),
                    x,
                    y,
                })
            },
            out,
        );
    }

    /// Advance the clock.
    ///
    /// The redraw discipline is unchanged from the ASS version and is the whole
    /// power story for this widget: between `clock_due` and now, **nothing
    /// happens** — no format, no measure, no rasterise, no compare. A clock
    /// reading `14:32` costs one `Instant` comparison per tick until 14:33.
    ///
    /// What changed is only what happens when it *is* due: the rows go into a
    /// [`clock::ClockText`], the content key is hashed from that struct, and
    /// [`BitmapState`] decides per output whether the pixels on disk are still
    /// of the right thing.
    fn clock_tick(&mut self, wall: DateTime<Local>, now: Instant, out: &mut Vec<WidgetUpdate>) {
        let repush = self.repush;
        let cfg = self.clock_cfg.clone();
        let geoms = self.outputs.clone();
        let theme = self.theme;
        let due = clock_is_due(self.clock_due, wall, clock::tick_secs(&cfg.style));
        let next_due = clock::next_change(wall, &cfg.style);
        // The gauge's value, deliberately computed here and deliberately *not*
        // in the content key — see `clock::day_fraction`.
        let day = clock::day_fraction(wall);

        let fonts = &mut self.fonts;
        let Some(state) = &mut self.clock else { return };
        if !cfg.enabled {
            // Switched off while it was on screen: take it down, once, and only
            // then throw the runtime away.
            state.bmp.retire(CLOCK_OVERLAY, out);
            self.clock = None;
            self.clock_due = None;
            return;
        }
        if due {
            state.text = Some(clock::ClockText::of(wall, &cfg.style));
            self.clock_due = Some(next_due);
        }
        // Disjoint field borrows: `push` takes the bitmap state, the render
        // closure takes the canvas and the rows.
        let ClockState { text, canvas, bmp } = state;
        let Some(text) = text.as_ref() else { return };
        // Everything the card draws, and nothing that moves on its own.
        let key = ContentKey::of(text);
        bmp.push(
            Push {
                overlay_id: CLOCK_OVERLAY,
                key,
                repush,
                stepped: false,
                now,
                period: CLOCK_PERIOD,
                retry: CLOCK_RETRY,
            },
            &geoms,
            |geom| {
                let scale = widgetkit::scale_for_output(geom.h);
                let data = text.card_data(&cfg.style, day);
                let fonts = fonts.get();
                let size = cards::clock::measure(fonts, &theme, &data, scale);
                let canvas = canvas.get(size.buffer(), scale)?;
                cards::clock::draw_at(canvas, fonts, &theme, &data, size.card_rect());
                let (x, y) = place(&size, cfg.style.anchor, cfg.style.margin_px, scale, geom);
                Some(BitmapFrame {
                    bgra: canvas.to_bgra(),
                    x,
                    y,
                })
            },
            out,
        );
    }

    /// Advance the visualiser. See [`VisualState::frame`] for the decisions.
    ///
    /// Three ways to cost nothing, in order: switched off (no state at all), no
    /// capture behind it (one `bool` test), and not yet due (one `Instant`
    /// comparison — no read, no FFT, no allocation). Only past all three does
    /// this touch the audio.
    fn visual_tick(&mut self, now: Instant, out: &mut Vec<WidgetUpdate>) {
        let repush = self.repush;
        let theme = self.theme;
        let geoms = self.outputs.clone();
        let fonts = &mut self.fonts;
        let Some(v) = &mut self.visual else { return };
        if !self.visual_cfg.enabled {
            // Switched off while it was on screen: take it down, once, and only
            // then throw the runtime away.
            v.bmp.retire(VISUALIZER_OVERLAY, out);
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
            v.bmp.retire(VISUALIZER_OVERLAY, out);
            v.shown = false;
            return;
        }
        if now < v.next_frame {
            // The bars we last sent are still the bars that are up; a fresh mpv
            // just needs telling. No capture read, no FFT, no render — the file
            // on disk is still a frame of exactly the right thing.
            if repush {
                v.bmp.push(
                    Push {
                        overlay_id: VISUALIZER_OVERLAY,
                        // Unreachable: `repush` never rasterises, so the key is
                        // only ever compared, and an unchanged one is what makes
                        // this the re-push path rather than the redraw one.
                        key: ContentKey::of(()),
                        repush: true,
                        stepped: false,
                        now,
                        period: Duration::ZERO,
                        retry: VISUAL_RETRY,
                    },
                    &geoms,
                    |_| None,
                    out,
                );
            }
            return;
        }
        let n = v.fill();
        v.frame(
            n,
            &self.visual_cfg,
            &theme,
            fonts.get(),
            &geoms,
            repush,
            now,
            out,
        );
    }

    /// Advance the album-art disc.
    ///
    /// Everything substrate-shaped — the files, the per-output placement, the
    /// redraw gate, the rate cap, the write failure path — is
    /// [`BitmapState::push`]'s. What is left here is the four things that are
    /// actually about a spinning record: which art, how far it has turned, how
    /// big, and where.
    fn disc_tick(
        &mut self,
        snapshot: Option<&Snapshot>,
        now: Instant,
        out: &mut Vec<WidgetUpdate>,
    ) {
        let repush = self.repush;
        let cfg = self.disc_cfg;
        let geoms = self.outputs.clone();
        let Some(d) = &mut self.disc else { return };
        if !cfg.enabled {
            d.bmp.retire(DISC_OVERLAY, out);
            self.disc = None;
            return;
        }

        // -- adopt this track's art, once per track -------------------------
        match snapshot.and_then(|s| s.track.as_ref()) {
            Some(track) => {
                if d.seq != Some(track.seq) {
                    d.seq = Some(track.seq);
                    d.art = track.art.clone();
                    d.title = track.now_playing.title.clone();
                    d.artist = track.now_playing.artist_line();
                    // A new record goes on at the top rather than continuing
                    // the last one's angle. `last` goes with it: the time since
                    // the previous tick belongs to the previous track, and
                    // carrying it over would spin the new one forward by a
                    // whole tick's worth on its first frame.
                    d.elapsed = Duration::ZERO;
                    d.last = None;
                    d.angle = 0.0;
                    d.bmp.dirty = true;
                }
            }
            None => {
                if d.seq.take().is_some() {
                    d.art = None;
                    d.title.clear();
                    d.artist.clear();
                    d.bmp.dirty = true;
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

        let art = d.art.clone();
        let size = cfg.size_px.clamp(1, artwork::MAX_DISC_PX);
        let angle = if cfg.spin {
            artwork::rotation_for(d.elapsed, artwork::VINYL_RPM)
        } else {
            0.0
        };
        // The animation input, kept out of the content key on purpose: an angle
        // in the key would move on every tick and defeat `DISC_FPS`.
        let stepped = art.is_some()
            && now >= d.bmp.next_frame
            && artwork::should_redraw(d.angle, angle, artwork::DEFAULT_MIN_STEP_DEG);
        // What the picture is *of*: which record, at what size and opacity.
        // Anchor and margin are not in here because they cannot move without
        // `set_disc` marking the widget dirty.
        let key = ContentKey::of((d.seq, size, cfg.opacity, art.is_some()));
        // `d` is re-borrowed field-by-field below, so everything read off it
        // whole has to happen first.

        let theme = self.theme;
        let fonts = &mut self.fonts;
        let DiscState {
            title,
            artist,
            canvas,
            bmp,
            ..
        } = d;
        let drawn = bmp.push(
            Push {
                overlay_id: DISC_OVERLAY,
                key,
                repush,
                stepped,
                now,
                period: DISC_PERIOD,
                retry: DISC_RETRY,
            },
            &geoms,
            |geom| {
                let art = art.as_ref()?;
                // `widgetkit::cards::disc` rather than `artwork::render_disc`:
                // the rim bevel and the specular sweep are *fixed* layers that
                // composite over the artwork in unrotated space, which is what
                // makes it read as a record catching a light in the room rather
                // than as a printed circle with a smear painted on it. A
                // pre-rendered bitmap cannot have layers drawn under and over
                // it. `render_disc` keeps its own callers and its own tests;
                // the published proportions are shared, so `label_ratio`,
                // `hole_ratio` and `ring_darken` still have one definition.
                let scale = widgetkit::scale_for_output(geom.h);
                let disc = DiscCfg {
                    size_px: size,
                    rotation_deg: angle,
                    opacity: cfg.opacity,
                    ..DiscCfg::default()
                };
                let data = cards::DiscData {
                    art: Some(art),
                    cfg: disc,
                    title,
                    artist,
                };
                let fonts = fonts.get();
                let wsize = cards::disc::measure(fonts, &theme, &data, scale);
                let canvas = canvas.get(wsize.buffer(), scale)?;
                cards::disc::draw_at(canvas, fonts, &theme, &data, wsize.card_rect());
                let (x, y) = place(&wsize, cfg.anchor, cfg.margin_px, scale, geom);
                Some(BitmapFrame {
                    bgra: canvas.to_bgra(),
                    x,
                    y,
                })
            },
            out,
        );
        if drawn > 0 {
            d.angle = angle;
        }
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
                if spinning || !d.bmp.is_shown() {
                    best = min_instant(best, d.bmp.next_frame.max(now));
                }
            }
        }

        best
    }

    /// Whether the lyric branch has anything to do: it is enabled, or it still
    /// has an overlay of ours to take down.
    fn lyrics_live(&self) -> bool {
        self.lyrics_cfg.enabled || self.lyrics_view.is_some()
    }

    /// The same question for the clock.
    fn clock_live(&self) -> bool {
        self.clock_cfg.enabled || self.clock.is_some()
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

/// Resolve [`config::WidgetTheme`] into the palette the cards are drawn in.
///
/// **`Auto` is dark**, and that is a decision rather than a placeholder — see
/// [`config::WidgetTheme`] for the reasoning. The short version: a widget is
/// drawn on someone's wallpaper, `Config::theme_mode` describes the app's own
/// chrome, and the desktop's light/dark preference says nothing about what is
/// behind the card. The dark palette is the one every alpha in the spec was
/// fitted for, against a white worst-case backdrop, and it is also the cheaper
/// of the two (52 lu of shadow bleed against 84).
fn widget_mode(theme: config::WidgetTheme) -> Mode {
    match theme {
        config::WidgetTheme::Auto | config::WidgetTheme::Dark => Mode::Dark,
        config::WidgetTheme::Light => Mode::Light,
    }
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

    /// The accent every fixture uses. An enum now and not a hex string: the
    /// dark and light palettes use different colours for the same accent, and
    /// only `Theme::for_accent` knows both tables.
    const ACCENT: config::Accent = config::Accent::Blue;

    /// A second accent, for the "a palette change repaints" tests.
    const OTHER_ACCENT: config::Accent = config::Accent::Coral;

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

    // -- reading a batch of updates -----------------------------------------
    //
    // These replace the old `payload()`, which read `WidgetUpdate::ass` and is
    // now always `""`: every widget rasterises, and the two payload kinds are
    // alternatives rather than layers. What a test asserts against instead is
    // the update itself — which command it is, which output it is for, and
    // what picture it carries.

    /// The update pushed to `overlay`, or a failure naming what did come back.
    fn update_for(updates: &[WidgetUpdate], overlay: u32) -> &WidgetUpdate {
        updates
            .iter()
            .find(|u| u.overlay_id == overlay)
            .unwrap_or_else(|| panic!("no update for overlay {overlay} in {updates:?}"))
    }

    /// The pixels pushed to `overlay`.
    fn frame_for(updates: &[WidgetUpdate], overlay: u32) -> &BitmapOverlay {
        update_for(updates, overlay)
            .frame()
            .unwrap_or_else(|| panic!("overlay {overlay} drew nothing in {updates:?}"))
    }

    /// The overlay ids in a batch, in the order they were emitted.
    fn ids(updates: &[WidgetUpdate]) -> Vec<u32> {
        updates.iter().map(|u| u.overlay_id).collect()
    }

    /// Everything a viewer of this overlay could tell apart: where it is, how
    /// big it is, and a hash of the `h * stride` bytes mpv would read from its
    /// file.
    ///
    /// **The bitmap answer to "is this the same picture as last time".** The
    /// [`BitmapOverlay`] alone is not enough — the path, the corner and the
    /// size are all identical between two frames of the same widget while the
    /// file underneath has been completely rewritten — and comparing megabytes
    /// of pixels in every assertion is precisely the cost [`ContentKey`] exists
    /// to avoid, so the bytes are read once and hashed.
    ///
    /// Read it **immediately**: the widget rewrites the same file in place, so
    /// a hash taken after the next push is a hash of the next frame.
    fn picture(b: &BitmapOverlay) -> (i32, i32, u32, u32, u64) {
        let want = b.h as usize * b.stride as usize;
        let bytes = std::fs::read(&b.path)
            .unwrap_or_else(|e| panic!("the frame file {} is unreadable: {e}", b.path.display()));
        assert!(
            bytes.len() >= want,
            "{} holds {} bytes, mpv would read {want}",
            b.path.display(),
            bytes.len()
        );
        (b.x, b.y, b.w, b.h, ContentKey::of(&bytes[..want]).0)
    }

    /// [`picture`] of whatever `overlay` was pushed in this batch.
    fn picture_for(updates: &[WidgetUpdate], overlay: u32) -> (i32, i32, u32, u32, u64) {
        picture(frame_for(updates, overlay))
    }

    /// The gap between a frame's right/bottom edges and its output's, in
    /// **logical** units.
    ///
    /// The one comparison that is meaningful across two outputs of different
    /// densities: a widget anchored the same way on both must sit the same
    /// distance from the corner *as the user sees it*, which is logical units —
    /// and a different number of device pixels on each.
    fn corner_gap_lu(b: &BitmapOverlay, geom: &OutputGeom) -> (f32, f32) {
        let scale = widgetkit::scale_for_output(geom.h);
        (
            (geom.w as f32 - (b.x + b.w as i32) as f32) / scale,
            (geom.h as f32 - (b.y + b.h as i32) as f32) / scale,
        )
    }

    /// The gap between a frame's left/top edges and its output's, in **logical**
    /// units. The near-edge twin of [`corner_gap_lu`].
    ///
    /// Both can be slightly **negative**, and that is not a placement fault:
    /// the buffer is the card *plus its shadow bleed on all four sides*, and
    /// `margin_px` is measured to the card. A card 48 units from the edge with
    /// a 76-unit bleed hangs 28 units of (nearly transparent) shadow off it.
    fn near_gap_lu(b: &BitmapOverlay, geom: &OutputGeom) -> (f32, f32) {
        let scale = widgetkit::scale_for_output(geom.h);
        (b.x as f32 / scale, b.y as f32 / scale)
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

        // Nothing playing, and nothing of ours has ever been on this renderer:
        // the ASS engine opened with one establishing blank here, and the
        // bitmap one has nothing honest to say. See `tick`.
        assert!(engine.tick_at(None, t0, None).is_empty());
        assert!(engine.tick_at(None, t0, None).is_empty());

        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let shown = engine.tick_at(Some(&snap), t0, None);
        assert_eq!(ids(&shown), vec![LYRICS_OVERLAY]);
        let line_a = picture_for(&shown, LYRICS_OVERLAY);

        // A hundred consecutive ticks across the rest of the line's life, at the
        // daemon's real 100ms cadence: every one of them free.
        for step in 1..=100 {
            let now = t0 + Duration::from_millis(100 * step);
            let updates = engine.tick_at(Some(&snap), now, None);
            assert!(updates.is_empty(), "tick {step} pushed {updates:?}");
        }

        // The next line lands once, and is then silent in its turn. It is a
        // genuinely different picture: a redraw budget that spent its one push
        // on the same pixels would be worse than no budget at all.
        let now = t0 + Duration::from_secs(50);
        let next = engine.tick_at(Some(&snap), now, None);
        assert_eq!(ids(&next), vec![LYRICS_OVERLAY]);
        let line_b = picture_for(&next, LYRICS_OVERLAY);
        assert_ne!(
            line_a, line_b,
            "the second line drew the first line's pixels"
        );
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
        assert_eq!(ids(&first), vec![CLOCK_OVERLAY]);
        let at_32 = picture_for(&first, CLOCK_OVERLAY);
        // What the pixels are of, asserted at the source the content key is
        // hashed from rather than by reading the picture back.
        assert_eq!(clock_reads(&engine), "14:32");

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

        // And exactly one at the boundary, of a different minute.
        let next = engine.tick_at(None, t0, Some(at(14, 33, 0)));
        assert_eq!(ids(&next), vec![CLOCK_OVERLAY]);
        assert_eq!(clock_reads(&engine), "14:33");
        assert_ne!(at_32, picture_for(&next, CLOCK_OVERLAY));
        assert!(engine.tick_at(None, t0, Some(at(14, 33, 1))).is_empty());
    }

    /// The hero row the clock widget currently holds — the string its content
    /// key is hashed from, and so the honest answer to "what does it say".
    fn clock_reads(engine: &WidgetEngine) -> &str {
        engine
            .clock
            .as_ref()
            .and_then(|c| c.text.as_ref())
            .map(|t| t.time.as_str())
            .expect("the clock has rendered")
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
        let first = engine.tick_at(None, t0, Some(at(14, 32, 30)));
        assert_eq!(ids(&first), vec![CLOCK_OVERLAY]);
        let at_1432 = picture_for(&first, CLOCK_OVERLAY);
        let back = engine.tick_at(None, t0, Some(at(12, 0, 0)));
        assert_eq!(ids(&back), vec![CLOCK_OVERLAY]);
        assert_eq!(clock_reads(&engine), "12:00");
        assert_ne!(at_1432, picture_for(&back, CLOCK_OVERLAY));
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
        assert_eq!(ids(&first), vec![LYRICS_OVERLAY, CLOCK_OVERLAY]);
        let lyric_before = picture_for(&first, LYRICS_OVERLAY);
        let clock_before = picture_for(&first, CLOCK_OVERLAY);
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());

        // Lyrics: a new anchor moves the line — and moves *only* the line. A
        // lyric edit that also re-rasterised the clock would be spending three
        // widgets' worth of work on one widget's change.
        let mut cfg = widgets(true);
        cfg.lyrics.anchor = config::LyricAnchor::TopLeft;
        engine.set_config(Some(&cfg), ACCENT);
        let moved = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(ids(&moved), vec![LYRICS_OVERLAY]);
        let lyric_after = picture_for(&moved, LYRICS_OVERLAY);
        assert_ne!(lyric_before, lyric_after);
        // Top-left rather than the default corner: the anchor is the thing that
        // changed, so it is the thing to check. The card sits its margin in
        // from both near edges (less its shadow bleed — see `near_gap_lu`).
        let geom = engine.out_geoms()[0].clone();
        let moved_frame = frame_for(&moved, LYRICS_OVERLAY);
        let (nx, ny) = near_gap_lu(moved_frame, &geom);
        let (fx, fy) = corner_gap_lu(moved_frame, &geom);
        assert!(
            nx < fx && ny < fy,
            "an anchor of TopLeft put the card at {moved_frame:?} on {geom:?}"
        );

        // Clock: a new theme changes what it says mid-minute.
        engine.set_clock(Some(&ClockCfg {
            enabled: true,
            style: ClockStyle {
                theme: crate::clock::ClockTheme::Wordy,
                ..Default::default()
            },
        }));
        let themed = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(ids(&themed), vec![CLOCK_OVERLAY]);
        assert_eq!(clock_reads(&engine), "half past two");
        assert_ne!(clock_before, picture_for(&themed, CLOCK_OVERLAY));

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
        let mut engine = WidgetEngine::new(Some(&cfg), config::Accent::Amber);
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
        assert_eq!(ids(&first), vec![LYRICS_OVERLAY, CLOCK_OVERLAY]);
        let amber_lyric = picture_for(&first, LYRICS_OVERLAY);
        let amber_clock = picture_for(&first, CLOCK_OVERLAY);
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());

        // The palette is what changed, and a content key cannot see a palette:
        // nothing either widget *says* is different, so the only thing that can
        // prove the repaint happened is the pixels.
        engine.set_config(Some(&cfg), ACCENT);
        let retinted = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(ids(&retinted), vec![LYRICS_OVERLAY, CLOCK_OVERLAY]);
        let blue_lyric = picture_for(&retinted, LYRICS_OVERLAY);
        let blue_clock = picture_for(&retinted, CLOCK_OVERLAY);
        assert_ne!(
            amber_lyric, blue_lyric,
            "the lyric card kept its old accent"
        );
        assert_ne!(
            amber_clock, blue_clock,
            "the clock card kept its old accent"
        );
        // The card did not move or resize: an accent is a colour, and a widget
        // that jumped a pixel on a theme change would be a layout bug.
        let placement = |p: (i32, i32, u32, u32, u64)| (p.0, p.1, p.2, p.3);
        assert_eq!(placement(amber_lyric), placement(blue_lyric));
        assert_eq!(placement(amber_clock), placement(blue_clock));
        // And one repaint, not a permanent one.
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());
    }

    // -- tracks -------------------------------------------------------------

    #[test]
    fn a_new_track_reloads_the_lyrics_and_a_re_announcement_does_not() {
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let t0 = Instant::now();
        let first = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let shown = engine.tick_at(Some(&first), t0, None);
        assert_eq!(ids(&shown), vec![LYRICS_OVERLAY]);
        let song_one = picture_for(&shown, LYRICS_OVERLAY);

        // Same seq, same everything: the worker is telling us nothing new.
        assert!(engine.tick_at(Some(&first), t0, None).is_empty());

        // A different track with no lyrics takes the overlay down, once — and
        // by `overlay-remove`, which is the only command that takes a bitmap
        // overlay down. An empty ASS payload here would leave the last song's
        // card on the wallpaper.
        let second = snapshot_at(t0, None, us(10.0), PlaybackStatus::Playing, 2);
        assert_eq!(
            engine.tick_at(Some(&second), t0, None),
            vec![WidgetUpdate::remove(LYRICS_OVERLAY)]
        );
        assert!(engine.tick_at(Some(&second), t0, None).is_empty());

        // A third track draws again, and draws its own words rather than
        // re-pushing the file the first song left on disk.
        let other_words = lyrics::parse_lrc("[00:10.00]c\n[01:00.00]d");
        let third = snapshot_at(t0, Some(other_words), us(10.0), PlaybackStatus::Playing, 3);
        let again = engine.tick_at(Some(&third), t0, None);
        assert_eq!(ids(&again), vec![LYRICS_OVERLAY]);
        assert_ne!(song_one, picture_for(&again, LYRICS_OVERLAY));

        // The player going away clears too, and only once.
        let gone = Snapshot {
            track: None,
            ..third.clone()
        };
        assert_eq!(
            engine.tick_at(Some(&gone), t0, None),
            vec![WidgetUpdate::remove(LYRICS_OVERLAY)]
        );
        assert!(engine.tick_at(Some(&gone), t0, None).is_empty());
    }

    #[test]
    fn a_paused_player_freezes_the_lyric_and_arms_no_deadline() {
        // Rule 6: no playback, nothing to animate, nothing to wake for.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let t0 = Instant::now();
        let playing = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let shown = engine.tick_at(Some(&playing), t0, None);
        assert_eq!(ids(&shown), vec![LYRICS_OVERLAY]);
        assert!(frame_for(&shown, LYRICS_OVERLAY).w > 0);

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
        // Lyrics only, and never ticked: blanked anyway, because the reason to
        // call this is that our belief about the renderer may be wrong.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        assert_eq!(
            engine.clear_all(),
            vec![WidgetUpdate::remove(LYRICS_OVERLAY)]
        );

        // Both.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        let snap = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let up = engine.tick_at(Some(&snap), t0, Some(at(14, 32, 30)));
        assert_eq!(ids(&up), vec![LYRICS_OVERLAY, CLOCK_OVERLAY]);
        let lyric_up = picture_for(&up, LYRICS_OVERLAY);
        let clock_up = picture_for(&up, CLOCK_OVERLAY);

        let cleared = engine.clear_all();
        assert_eq!(ids(&cleared), vec![LYRICS_OVERLAY, CLOCK_OVERLAY]);
        assert!(cleared.iter().all(WidgetUpdate::is_clear));

        // And the widgets come straight back on the next tick — a wallpaper swap
        // must not leave the desktop without its lyric until the next song —
        // showing exactly what was up before it.
        let restored = engine.tick_at(Some(&snap), t0, Some(at(14, 32, 30)));
        assert_eq!(ids(&restored), vec![LYRICS_OVERLAY, CLOCK_OVERLAY]);
        assert_eq!(lyric_up, picture_for(&restored, LYRICS_OVERLAY));
        assert_eq!(clock_up, picture_for(&restored, CLOCK_OVERLAY));
        // Once each, not once per tick.
        assert!(engine
            .tick_at(Some(&snap), t0, Some(at(14, 32, 30)))
            .is_empty());

        // The clock alone: nothing is pushed for a widget that is off.
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        assert_eq!(
            engine.clear_all(),
            vec![WidgetUpdate::remove(CLOCK_OVERLAY)]
        );
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
        assert_eq!(ids(&first), vec![LYRICS_OVERLAY, CLOCK_OVERLAY]);
        let lyric = frame_for(&first, LYRICS_OVERLAY).clone();
        let clock = frame_for(&first, CLOCK_OVERLAY).clone();
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());

        engine.invalidate();
        let again = engine.tick_at(Some(&snap), t0, wall);
        assert_eq!(ids(&again), vec![LYRICS_OVERLAY, CLOCK_OVERLAY]);
        // Byte for byte the same `overlay-add`: same file, same corner, same
        // size. Anything else and this would be a re-render, which is the whole
        // thing `invalidate` promises not to be.
        assert_eq!(frame_for(&again, LYRICS_OVERLAY), &lyric);
        assert_eq!(frame_for(&again, CLOCK_OVERLAY), &clock);
        // One re-push, not a permanent one.
        assert!(engine.tick_at(Some(&snap), t0, wall).is_empty());

        // With nothing on screen there is nothing to restore: `clear_all` has
        // already emitted the blanks and forgotten what was up, so a re-push
        // has nothing to re-push and the disabled clock has nothing to retire.
        engine.clear_all();
        engine.set_clock(None);
        engine.invalidate();
        assert!(engine.tick_at(None, t0, wall).is_empty());
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
            vec![WidgetUpdate::remove(LYRICS_OVERLAY)]
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
        assert_eq!(ids(&engine.tick_at(None, t0, wall)), vec![CLOCK_OVERLAY]);
        engine.set_clock(None);
        assert!(!engine.is_active());
        assert_eq!(
            engine.tick_at(None, t0, wall),
            vec![WidgetUpdate::remove(CLOCK_OVERLAY)]
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
        assert_eq!(down, vec![WidgetUpdate::remove(VISUALIZER_OVERLAY)]);
        assert!(engine.visual.is_none(), "the runtime must be thrown away");
        assert!(engine.tick_at(None, t0, None).is_empty());
        assert!(engine.tick().is_empty());

        // Switching it back on must build a fresh runtime and try the capture
        // again. Re-using the disabled one — which had its capture dropped at
        // the setter — would leave the widget on and permanently deaf.
        engine.set_visualizer(Some(&visual_cfg(true)));
        let v = engine.visual.as_ref().expect("re-enabled");
        // Nothing on screen yet, asserted on both halves of "on screen": the
        // runtime's own flag *and* the per-output slots the substrate keeps.
        // The ASS version read `v.ass.is_none()`, which was one field for both.
        assert!(!v.shown);
        assert!(!v.bmp.is_shown());
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
        disc.bmp
            .set_stem(std::env::temp_dir().join(format!("fresco-test-disc-{tag}")));
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
        assert_eq!(anchor_xy(Anchor::TopLeft, 320, 320, 48, w, h), (48, 48));
        assert_eq!(
            anchor_xy(Anchor::BottomRight, 320, 320, 48, w, h),
            ((w - 320 - 48) as i32, (h - 320 - 48) as i32)
        );
        assert_eq!(
            anchor_xy(Anchor::MidCenter, 320, 320, 48, w, h),
            (((w - 320) / 2) as i32, ((h - 320) / 2) as i32)
        );
        // A disc bigger than the output pins to the edge instead of wrapping
        // around to a nonsense coordinate.
        assert_eq!(anchor_xy(Anchor::BottomRight, 4000, 4000, 48, w, h), (0, 0));
        assert_eq!(anchor_xy(Anchor::TopLeft, 4000, 4000, 48, w, h), (0, 0));

        // And the widget itself is placed against the output it is drawn on.
        //
        // The record is no longer a bare 32x32 bitmap: it is a card, measured
        // by the rasteriser, and its buffer is the card *plus its shadow bleed
        // on all four sides*. So the corner is not `anchor_xy` of the frame,
        // and asserting that it were would just be re-running `place` inside
        // the test.
        //
        // What is checkable without re-deriving the bleed is the symmetry the
        // placement cannot break if it used this output's mode: the **same**
        // widget anchored at opposite corners is a mirror image about the
        // output's centre, so `left + right + width == the output's width`. Get
        // the geometry from the wrong screen and that identity misses by the
        // difference between the two screens.
        let t0 = Instant::now();
        let snap = snapshot_with_art(t0, us(0.0), PlaybackStatus::Paused, 1);
        let corner = |anchor: Anchor, out: (u32, u32), tag: &str| -> BitmapOverlay {
            let mut engine = WidgetEngine::new(None, ACCENT);
            with_disc(
                &mut engine,
                DiscWidgetCfg {
                    anchor,
                    ..disc_cfg()
                },
                tag,
            );
            engine.set_output_size(out.0, out.1);
            let placed = engine.tick_at(Some(&snap), t0, None);
            frame_for(&placed, DISC_OVERLAY).clone()
        };

        let br = corner(Anchor::BottomRight, (w, h), "place-br");
        let tl = corner(Anchor::TopLeft, (w, h), "place-tl");
        assert_eq!((tl.w, tl.h), (br.w, br.h), "the anchor resized the card");
        assert_eq!(
            tl.x + br.x + tl.w as i32,
            w as i32,
            "the two corners are not mirrored about the output's centre"
        );
        assert_eq!(tl.y + br.y + tl.h as i32, h as i32);
        // The buffer holds at least the record itself, and is tightly packed.
        assert!(br.w >= 32 && br.h >= 32, "{br:?}");
        assert_eq!(br.stride, br.w * 4);

        // The same widget on a 720p screen: the identity holds against *that*
        // output's mode, and against nothing else.
        let small_br = corner(Anchor::BottomRight, (1280, 720), "place-small");
        let small_tl = corner(Anchor::TopLeft, (1280, 720), "place-small-tl");
        assert_eq!(small_tl.x + small_br.x + small_tl.w as i32, 1280);
        assert_eq!(small_tl.y + small_br.y + small_tl.h as i32, 720);
        assert!(small_br.w < br.w, "{small_br:?} vs {br:?}");

        // A mode change re-places and re-pushes rather than waiting for the
        // next track.
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "place");
        engine.set_output_size(w, h);
        let placed = engine.tick_at(Some(&snap), t0, None);
        let frame = frame_for(&placed, DISC_OVERLAY).clone();
        assert!(engine.tick_at(Some(&snap), t0, None).is_empty());
        engine.set_output_size(1280, 720);
        let moved = engine.tick_at(Some(&snap), t0, None);
        let shrunk = frame_for(&moved, DISC_OVERLAY).clone();
        assert_ne!((frame.x, frame.y), (shrunk.x, shrunk.y));
        assert!(shrunk.w < frame.w && shrunk.h < frame.h, "{shrunk:?}");

        // A zero-sized mode report is ignored rather than parking the disc in
        // the corner.
        engine.set_output_size(0, 0);
        assert_eq!(engine.out_geoms()[0].w, 1280);
        assert_eq!(engine.out_geoms()[0].h, 720);
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
        // Every one of the four is a bitmap overlay now, so every one of them
        // comes down by `overlay-remove`. An empty `osd-overlay` is a silent
        // no-op against `overlay-add`, and the widget rides onto the next
        // wallpaper.
        assert!(
            cleared
                .iter()
                .all(|u| u.bitmap == Some(BitmapUpdate::Remove)),
            "a bitmap overlay is not cleared by an empty ASS payload: {cleared:?}"
        );

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
        // `VisualState::new` arms its first frame at the real `Instant::now()`,
        // which by here is some way past the synthetic `t0` this test drives
        // everything else from — rasterising three widgets is not free in a
        // debug build. Re-base it, or the widget is simply not due yet and this
        // measures the wall clock rather than the rate cap.
        engine.visual.as_mut().expect("a runtime").next_frame = t0;
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
        engine
            .disc
            .as_mut()
            .expect("a disc")
            .bmp
            .set_stem(blocker.join("frame"));

        let t0 = Instant::now();
        let playing = snapshot_with_art(t0, us(0.0), PlaybackStatus::Playing, 1);
        let mut attempts = 0;
        for step in 0..600u32 {
            let now = t0 + Duration::from_millis(50 * u64::from(step));
            assert!(
                engine.tick_at(Some(&playing), now, None).is_empty(),
                "a frame that was never written must not be announced"
            );
            if engine.disc.as_ref().expect("a disc").bmp.warned {
                attempts += 1;
            }
        }
        assert!(attempts > 0, "the failure must be noticed");
        // 30 seconds at the retry cadence, not at the frame cadence.
        let retries = 30 / DISC_RETRY.as_secs();
        assert!(
            engine.disc.as_ref().expect("a disc").bmp.warned,
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

        // A **smaller** frame must not shorten it. `set_len` is the one call
        // here that can unback a page mpv still has mapped, and a widget whose
        // bitmap changes size with its content (a lyric line is a different
        // width on every line) would hit that on every line. Grow only; the
        // tail of a smaller frame is never read, because `overlay-add` is told
        // `h * stride` explicitly.
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
            first,
            "a shrinking frame must never shorten a live mapping"
        );
        // The bytes the smaller frame does own are its own, at offset 0.
        let on_disk = std::fs::read(&path).expect("readable");
        assert_eq!(&on_disk[..small.data.len()], &small.data[..]);

        // Growing past the high-water mark does extend it: the old mapping
        // stays fully backed, it simply stops covering the new tail.
        let big = artwork::render_disc(
            &art,
            &DiscCfg {
                size_px: 32,
                ..Default::default()
            },
        );
        write_frame(&path, &big).expect("rewritable");
        assert_eq!(
            std::fs::metadata(&path).expect("written").len(),
            u64::from(big.h * big.stride())
        );
        let _ = std::fs::remove_file(&path);
    }

    // -- defect 1: clearing follows what is on screen, not what a widget is ---

    /// Pretend `overlay_id` is currently occupied by `kind`, the way a tick
    /// that pushed such an update would have left it. This is the only honest
    /// way to test the clear path against a widget that has *changed*
    /// substrate, which is precisely the case the engine must survive.
    fn pretend_on_screen(engine: &mut WidgetEngine, overlay_id: u32, kind: OverlayKind) {
        let mut batch = vec![match kind {
            OverlayKind::Ass => WidgetUpdate::ass(overlay_id, "x".into()),
            OverlayKind::Bitmap => WidgetUpdate::draw(
                overlay_id,
                BitmapOverlay {
                    x: 0,
                    y: 0,
                    path: PathBuf::from("/dev/null"),
                    w: 4,
                    h: 4,
                    stride: 16,
                },
            ),
        }];
        engine.note_pushed(&mut batch);
    }

    #[test]
    fn a_clear_uses_the_command_the_overlay_on_screen_needs() {
        // The highest-risk defect. An empty `osd-overlay` does not take a
        // bitmap overlay down and `overlay-remove` does not take ASS down, so a
        // clear that guesses from the widget's name silently no-ops the moment
        // that widget is ported — and the stale widget rides onto the next
        // wallpaper.
        let mut engine = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        engine.set_visualizer(Some(&visual_cfg(true)));
        with_disc(&mut engine, disc_cfg(), "clearkind");

        // As things stand today: four bitmap widgets, so four `overlay-remove`s.
        let cleared = engine.clear_all();
        assert_eq!(cleared.len(), 4);
        for u in &cleared {
            assert!(u.is_clear(), "{u:?}");
            assert_eq!(
                u.bitmap,
                Some(BitmapUpdate::Remove),
                "overlay {} got the wrong command: {u:?}",
                u.overlay_id
            );
        }

        // Now demote two of them to ASS behind the engine's back — a bitmap
        // renderer failing back to text is exactly the move this has to survive
        // — without touching any config. What is on screen decides, not what
        // the widget is nominally made of.
        pretend_on_screen(&mut engine, LYRICS_OVERLAY, OverlayKind::Ass);
        pretend_on_screen(&mut engine, DISC_OVERLAY, OverlayKind::Ass);
        let cleared = engine.clear_all();
        assert_eq!(update_for(&cleared, LYRICS_OVERLAY).bitmap, None);
        assert!(update_for(&cleared, LYRICS_OVERLAY).ass.is_empty());
        assert_eq!(update_for(&cleared, DISC_OVERLAY).bitmap, None);
        assert_eq!(
            update_for(&cleared, CLOCK_OVERLAY).bitmap,
            Some(BitmapUpdate::Remove),
            "an untouched widget must keep its own command"
        );
        // Still one per widget, and every one of them a clear.
        assert_eq!(cleared.len(), 4);
        assert!(cleared.iter().all(WidgetUpdate::is_clear));

        // And a second clear with nothing believed to be up falls back to the
        // widget's own substrate rather than to whatever was last seen.
        let again = engine.clear_all();
        assert_eq!(again.len(), 4);
        for u in &again {
            assert_eq!(
                u.bitmap,
                Some(BitmapUpdate::Remove),
                "overlay {} did not fall back to its own substrate: {u:?}",
                u.overlay_id
            );
        }
    }

    #[test]
    fn changing_substrate_blanks_the_overlay_it_is_leaving() {
        // mpv keeps `osd-overlay` and `overlay-add` in separate namespaces, so
        // drawing a bitmap over an id that still holds ASS leaves both up.
        let mut engine = WidgetEngine::new(None, ACCENT);
        pretend_on_screen(&mut engine, CLOCK_OVERLAY, OverlayKind::Ass);

        let mut batch = vec![WidgetUpdate::draw(
            CLOCK_OVERLAY,
            BitmapOverlay {
                x: 1,
                y: 2,
                path: PathBuf::from("/dev/null"),
                w: 4,
                h: 4,
                stride: 16,
            },
        )];
        engine.note_pushed(&mut batch);
        assert_eq!(batch.len(), 2, "the outgoing ASS overlay must be blanked");
        assert!(batch[0].is_clear() && batch[0].bitmap.is_none());
        assert!(batch[1].frame().is_some());

        // Going back the other way blanks the bitmap.
        let mut batch = vec![WidgetUpdate::ass(CLOCK_OVERLAY, "back to text".into())];
        engine.note_pushed(&mut batch);
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].bitmap, Some(BitmapUpdate::Remove));
        assert_eq!(batch[1].ass, "back to text");

        // Staying on the same substrate inserts nothing.
        let mut batch = vec![WidgetUpdate::ass(CLOCK_OVERLAY, "still text".into())];
        engine.note_pushed(&mut batch);
        assert_eq!(batch.len(), 1);
    }

    // -- defect 2: one state machine, N geometries ---------------------------

    #[test]
    fn two_outputs_of_different_sizes_get_their_own_geometry_from_one_tick() {
        // `overlay-add` is in real pixels. Sizing every output's frame against
        // whichever monitor the loop looked at first puts the widget at the
        // wrong size and in the wrong place on the other one.
        let uhd = OutputGeom {
            connector: "DP-1".into(),
            w: 3840,
            h: 2160,
        };
        let hd = OutputGeom {
            connector: "HDMI-1".into(),
            w: 1280,
            h: 720,
        };
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "twoup");
        engine.set_outputs(&[uhd.clone(), hd.clone()]);
        let t0 = Instant::now();
        let snap = snapshot_with_art(t0, us(0.0), PlaybackStatus::Paused, 1);
        let drawn = engine.tick_at(Some(&snap), t0, None);
        assert_eq!(drawn.len(), 2, "one frame per output: {drawn:?}");

        let big = by_target(&drawn, "DP-1").frame().expect("pixels").clone();
        let small = by_target(&drawn, "HDMI-1").frame().expect("pixels").clone();
        // Each frame is sized and placed against *its own* output. The bug this
        // guards is one geometry serving both, which shows up as a widget of
        // the wrong size anchored to the wrong screen's corner.
        assert!(
            (big.w, big.h) != (small.w, small.h),
            "both outputs got one size: {big:?} / {small:?}"
        );
        assert!(big.w > small.w && big.h > small.h);
        assert_ne!((big.x, big.y), (small.x, small.y));
        // Same anchor on both, so the same gap from the corner — measured in
        // logical units, which is the only frame of reference two screens of
        // different densities share. Placed against one geometry, the 720p
        // frame's gap would be off by the difference between the two screens.
        let (bx, by) = corner_gap_lu(&big, &uhd);
        let (sx, sy) = corner_gap_lu(&small, &hd);
        assert!(
            (bx - sx).abs() < 2.0 && (by - sy).abs() < 2.0,
            "{bx},{by} vs {sx},{sy}"
        );
        // Separate files, because two outputs of different sizes hold genuinely
        // different pixels the moment a widget sizes itself against its screen.
        assert_ne!(big.path, small.path);
        // And the two files really do hold different pictures, not one written
        // twice.
        assert_ne!(picture(&big).4, picture(&small).4);
        // And each update is routed to exactly one of them.
        assert!(by_target(&drawn, "DP-1").is_for("DP-1"));
        assert!(!by_target(&drawn, "DP-1").is_for("HDMI-1"));

        // The state machine stayed single: one record, one angle, one decision.
        assert_eq!(engine.disc.as_ref().expect("a disc").seq, Some(1));
        assert!(engine.tick_at(Some(&snap), t0, None).is_empty());

        // A loop that drives one unnamed output gets untargeted updates and so
        // needs no routing at all — the other half of `WidgetUpdate::target`.
        let mut one_up = WidgetEngine::new(Some(&widgets(true)), ACCENT);
        let lyric = snapshot_at(t0, Some(fixture()), us(10.0), PlaybackStatus::Playing, 1);
        let first = one_up.tick_at(Some(&lyric), t0, None);
        assert_eq!(ids(&first), vec![LYRICS_OVERLAY]);
        assert!(first.iter().all(|u| u.target.is_none()));
        assert!(first[0].is_for("anything at all"));
    }

    /// The update aimed at `connector`.
    fn by_target<'a>(updates: &'a [WidgetUpdate], connector: &str) -> &'a WidgetUpdate {
        updates
            .iter()
            .find(|u| u.target.as_deref() == Some(connector))
            .unwrap_or_else(|| panic!("nothing for {connector} in {updates:?}"))
    }

    #[test]
    fn a_widget_that_sizes_itself_against_the_output_gets_a_size_per_output() {
        // The disc is absolute-sized, so only its corner moves. A clock, a
        // lyric bar or a spectrum is sized *relative to its screen*, and that
        // is the case the substrate has to carry — one rasterise per geometry,
        // into that geometry's own file.
        let mut st = BitmapState::new("unused", Instant::now());
        st.set_stem(std::env::temp_dir().join("fresco-test-perout"));
        let outputs = [
            OutputGeom {
                connector: "DP-1".into(),
                w: 3840,
                h: 2160,
            },
            OutputGeom {
                connector: "HDMI-1".into(),
                w: 1280,
                h: 720,
            },
        ];
        let mut out = Vec::new();
        let now = Instant::now();
        let pace = |key: ContentKey| Push {
            overlay_id: CLOCK_OVERLAY,
            key,
            repush: false,
            stepped: false,
            now,
            period: Duration::from_millis(100),
            retry: Duration::from_secs(2),
        };
        let drawn = st.push(
            pace(ContentKey::of("14:32")),
            &outputs,
            |geom| {
                // A bar a tenth of the screen high, full width — the shape an
                // ASS widget gets for free and a bitmap one does not.
                let (w, h) = (geom.w / 4, geom.h / 10);
                Some(BitmapFrame {
                    bgra: Bgra {
                        w,
                        h,
                        data: vec![0u8; (w as usize) * (h as usize) * 4],
                    },
                    x: anchor_xy(Anchor::TopCenter, w, h, 16, geom.w, geom.h).0,
                    y: 16,
                })
            },
            &mut out,
        );
        assert_eq!(drawn, 2);
        let big = by_target(&out, "DP-1").frame().expect("pixels");
        let small = by_target(&out, "HDMI-1").frame().expect("pixels");
        assert_eq!((big.w, big.h), (960, 216));
        assert_eq!((small.w, small.h), (320, 72));
        assert_ne!(big.path, small.path);
        assert_eq!(big.stride, big.w * 4);
        // The content key holds both still on the next pass.
        out.clear();
        st.push(
            pace(ContentKey::of("14:32")),
            &outputs,
            |_| panic!("an unchanged key must not rasterise"),
            &mut out,
        );
        assert!(out.is_empty());
        // A new key redraws both.
        let drawn = st.push(
            pace(ContentKey::of("14:33")),
            &outputs,
            |geom| {
                let (w, h) = (geom.w / 4, geom.h / 10);
                Some(BitmapFrame {
                    bgra: Bgra {
                        w,
                        h,
                        data: vec![0u8; (w as usize) * (h as usize) * 4],
                    },
                    x: 0,
                    y: 0,
                })
            },
            &mut out,
        );
        assert_eq!(drawn, 2);
        for n in 0..2 {
            let _ = std::fs::remove_file(slot_path(
                &std::env::temp_dir().join("fresco-test-perout"),
                n,
            ));
        }
    }

    #[test]
    fn a_failed_write_backs_off_the_rasteriser_and_not_just_the_push() {
        // The retry cadence has to gate the *render*, not only the write.
        // Otherwise a read-only runtime directory costs a full rasterise ten
        // times a second forever, which is the cost this whole module exists to
        // keep off an idle desktop.
        let blocker = std::env::temp_dir().join("fresco-test-bmp-blocker");
        std::fs::write(&blocker, b"not a directory").expect("temp dir is writable");
        let t0 = Instant::now();
        let mut st = BitmapState::new("unused", t0);
        st.set_stem(blocker.join("frame"));
        let outputs = [OutputGeom {
            connector: String::new(),
            w: 1920,
            h: 1080,
        }];
        let retry = Duration::from_secs(2);
        let mut renders = 0u32;
        let mut out = Vec::new();
        // 30 seconds of a 100ms loop.
        for step in 0..300u32 {
            let now = t0 + Duration::from_millis(100 * u64::from(step));
            st.push(
                Push {
                    overlay_id: DISC_OVERLAY,
                    key: ContentKey::of(1u8),
                    repush: false,
                    stepped: false,
                    now,
                    period: Duration::from_millis(100),
                    retry,
                },
                &outputs,
                |_| {
                    renders += 1;
                    Some(BitmapFrame {
                        bgra: Bgra {
                            w: 2,
                            h: 2,
                            data: vec![0u8; 16],
                        },
                        x: 0,
                        y: 0,
                    })
                },
                &mut out,
            );
        }
        assert!(
            out.is_empty(),
            "a frame that was never written must not be announced"
        );
        assert!(renders > 0, "the failure must actually be attempted");
        assert!(
            renders <= 30 / retry.as_secs() as u32 + 2,
            "{renders} renders in 30s is the frame cadence, not the retry cadence"
        );
        assert!(st.warned, "and it is logged once, not once per attempt");
        let _ = std::fs::remove_file(&blocker);
    }

    #[test]
    fn a_hotplug_never_makes_two_outputs_share_one_frame_file() {
        // Slots are rebuilt when the output list changes, and a slot that
        // survives keeps its file. Numbering the new ones from scratch would
        // hand a surviving slot's name to a new one — two outputs writing each
        // other's pixels, which looks like a corrupted frame and nothing else.
        let mut st = BitmapState::new("unused", Instant::now());
        let stem = std::env::temp_dir().join("fresco-test-hotplug");
        st.set_stem(stem.clone());
        let geom = |c: &str, w: u32, h: u32| OutputGeom {
            connector: c.into(),
            w,
            h,
        };
        st.sync(&[geom("A", 1920, 1080), geom("B", 3840, 2160)]);
        let b_path = st.slots[1].path.clone();
        // A goes away, C arrives: B survives at a new index.
        st.sync(&[geom("B", 3840, 2160), geom("C", 1280, 720)]);
        assert_eq!(st.slots[0].geom.connector, "B");
        assert_eq!(st.slots[0].path, b_path, "a surviving slot keeps its file");
        assert_ne!(st.slots[0].path, st.slots[1].path);
        let paths: Vec<_> = st.slots.iter().map(|s| s.path.clone()).collect();
        assert_eq!(
            paths.len(),
            paths.iter().collect::<std::collections::HashSet<_>>().len(),
            "{paths:?} must be distinct"
        );
        // And the survivor kept its pixels, while the newcomer has none.
        assert!(st.slots[1].key.is_none());
        let _ = std::fs::remove_file(&stem);
    }

    #[test]
    fn an_oversized_surface_is_refused_rather_than_rasterised() {
        // A full-screen visualiser at 4K is 33 MB a frame. `artwork` caps the
        // disc at 2048 per side for exactly this reason; the engine needs the
        // same guard for a widget that is not square.
        let mut st = BitmapState::new("unused", Instant::now());
        st.set_stem(std::env::temp_dir().join("fresco-test-huge"));
        let outputs = [OutputGeom {
            connector: String::new(),
            w: 3840,
            h: 2160,
        }];
        let mut out = Vec::new();
        let drawn = st.push(
            Push {
                overlay_id: VISUALIZER_OVERLAY,
                key: ContentKey::of(0u8),
                repush: false,
                stepped: false,
                now: Instant::now(),
                period: Duration::from_millis(100),
                retry: Duration::from_secs(2),
            },
            &outputs,
            |geom| {
                // Deliberately over the cap, and *not* allocated: the guard
                // reads `w`/`h`, so a real renderer would be asked to stop
                // before it allocated 33 MB.
                Some(BitmapFrame {
                    bgra: Bgra {
                        w: geom.w,
                        h: geom.h,
                        data: Vec::new(),
                    },
                    x: 0,
                    y: 0,
                })
            },
            &mut out,
        );
        assert_eq!(drawn, 0);
        assert!(out.is_empty(), "a refused frame must not be announced");
        assert!(st.warned, "and it must say so, once");
        assert!(u64::from(3840u32) * u64::from(2160u32) > MAX_WIDGET_AREA_PX);
    }

    // -- defect 3: anchors on a rectangle ------------------------------------

    #[test]
    fn anchor_xy_resolves_both_axes_independently() {
        // The disc is square, so one `size` was enough. A clock, a lyric line
        // and a spectrum are not, and resolving `y` against the width puts
        // every one of them somewhere it was not asked to be.
        let (ow, oh) = (1920u32, 1080u32);
        let (w, h, m) = (600u32, 80u32, 40u32);
        let left = m as i32;
        let right = (ow - w - m) as i32;
        let cx = ((ow - w) / 2) as i32;
        let top = m as i32;
        let bottom = (oh - h - m) as i32;
        let cy = ((oh - h) / 2) as i32;
        for (anchor, want) in [
            (Anchor::TopLeft, (left, top)),
            (Anchor::TopCenter, (cx, top)),
            (Anchor::TopRight, (right, top)),
            (Anchor::MidLeft, (left, cy)),
            (Anchor::MidCenter, (cx, cy)),
            (Anchor::MidRight, (right, cy)),
            (Anchor::BottomLeft, (left, bottom)),
            (Anchor::BottomCenter, (cx, bottom)),
            (Anchor::BottomRight, (right, bottom)),
        ] {
            assert_eq!(anchor_xy(anchor, w, h, m, ow, oh), want, "{anchor:?}");
        }

        // A box wider than the output but shorter than it pins only the axis
        // that overflows — the saturating behaviour, now per axis.
        assert_eq!(anchor_xy(Anchor::TopLeft, 4000, h, m, ow, oh), (0, top));
        assert_eq!(
            anchor_xy(Anchor::BottomRight, 4000, h, m, ow, oh),
            (0, bottom)
        );
        assert_eq!(anchor_xy(Anchor::MidCenter, 4000, h, m, ow, oh), (0, cy));
        // Larger than the output on both axes still pins to the corner.
        for anchor in [
            Anchor::TopLeft,
            Anchor::MidCenter,
            Anchor::BottomRight,
            Anchor::TopRight,
            Anchor::BottomLeft,
        ] {
            assert_eq!(
                anchor_xy(anchor, 4000, 4000, m, ow, oh),
                (0, 0),
                "{anchor:?}"
            );
        }
        // And the square case the disc relies on is unchanged.
        assert_eq!(
            anchor_xy(Anchor::TopLeft, 320, 320, 48, 3840, 2160),
            (48, 48)
        );
    }

    // -- invalidate re-pushes without re-rendering ---------------------------

    #[test]
    fn invalidate_re_pushes_the_file_on_disk_without_rasterising_again() {
        // The Wayland loop calls `invalidate` whenever any output's respawn
        // generation moves, which can flap. Re-rasterising four bitmap widgets
        // on each flap is a visible hitch, and unnecessary: the frame on disk
        // is still a frame of exactly the right thing.
        let mut engine = WidgetEngine::new(None, ACCENT);
        with_disc(&mut engine, disc_cfg(), "repush");
        let t0 = Instant::now();
        let paused = snapshot_with_art(t0, us(0.0), PlaybackStatus::Paused, 1);
        let first = engine.tick_at(Some(&paused), t0, None);
        let path = first[0].frame().expect("pixels").path.clone();
        assert!(path.exists());

        // Delete the file. A re-*render* would put it back; a re-*push* must
        // hand mpv the same command for the same path and touch nothing.
        std::fs::remove_file(&path).expect("our own temp file");
        engine.invalidate();
        let again = engine.tick_at(Some(&paused), t0, None);
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].frame().expect("pixels").path, path);
        assert!(!path.exists(), "invalidate must not rasterise");
        // One re-push, not a permanent one.
        assert!(engine.tick_at(Some(&paused), t0, None).is_empty());
    }

    #[test]
    fn invalidate_re_pushes_the_clock_without_re_formatting_it() {
        // The same rule for the widget with a *schedule*, where it is also what
        // keeps `clock_due` meaningful: a re-push that reset the schedule would
        // turn a flapping respawn counter into a render per flap.
        let mut engine = WidgetEngine::new(None, ACCENT);
        engine.set_clock(Some(&clock_cfg()));
        let t0 = Instant::now();
        let wall = at(14, 32, 30);
        let first = engine.tick_at(None, t0, Some(wall));
        let frame = frame_for(&first, CLOCK_OVERLAY).clone();
        let due = engine.clock_due.expect("the clock armed its schedule");

        // Delete the file underneath it. A re-render would put it back; a
        // re-push hands mpv the same command for the same path and touches
        // nothing at all.
        std::fs::remove_file(&frame.path).expect("our own temp file");
        engine.invalidate();
        let again = engine.tick_at(None, t0, Some(wall));
        assert_eq!(frame_for(&again, CLOCK_OVERLAY), &frame);
        assert!(!frame.path.exists(), "invalidate must not re-rasterise");
        assert_eq!(
            engine.clock_due,
            Some(due),
            "the render schedule must survive a re-push"
        );
        assert!(engine.tick_at(None, t0, Some(wall)).is_empty());
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

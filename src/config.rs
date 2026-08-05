use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Video,
    Playlist,
    Image,
    Slideshow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Cover,
    Contain,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scaling {
    #[default]
    Balanced,
    High,
}

/// How hard Fresco works to cut GPU load, trading image sharpness for battery
/// and heat.
///
/// Two earlier designs were measured and abandoned. The 1.1.32 frame-rate cap
/// was actively harmful: `fps` is a *software* filter, so putting it in a
/// VA-API pipeline forced every frame back off the GPU and roughly DOUBLED
/// video-engine load. The 1.1.33 decoder-level skipping (`vd-lavc-skipframe`)
/// was merely useless: for a hardware-decoded wallpaper the load is
/// **Render/3D (~99%)**, not decode (~17%), so skipping frames inside
/// libavcodec saved nothing measurable.
///
/// What actually works is reducing the per-frame *scaler* cost — the thing the
/// render engine is busy with (see [`video_scalers`]). Confirmed with
/// `turbostat` on Alder Lake-N (Intel N150): at 4K, GPU power fell 2.77 W ->
/// 1.60 W on Reduced (-42%) and -> 0.99 W on Minimum (-65%); at 1080p, Reduced
/// halved GPU power (1.37 W -> 0.63 W) and Minimum added nothing further.
/// No frame is ever dropped and hardware decoding is untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PowerSaving {
    /// Quality scalers (spline36 / lanczos, dithering, linear-light
    /// downscaling) — the sharpest image and the most GPU work.
    Full,
    /// Cheap bilinear scaling with dithering kept. The default: at 1080p it
    /// captures nearly all the available saving, and the softening is hard to
    /// see on a wallpaper sitting behind windows.
    #[default]
    Reduced,
    /// Bilinear everywhere with no dithering — the largest saving, and worth it
    /// mainly for 4K sources, where it roughly halves GPU power again.
    Minimum,
}

/// The GPU scaler configuration applied to a video wallpaper. All fields are
/// mpv option values; [`VideoScalers::to_options`] turns them into `(key,
/// value)` pairs the daemon sets either as spawn options or live properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoScalers {
    pub scale: &'static str,
    pub cscale: &'static str,
    pub dscale: &'static str,
    pub dither: bool,
    pub correct_downscaling: bool,
    pub linear_downscaling: bool,
}

impl VideoScalers {
    /// mpv `(option, value)` pairs for these scalers, in a stable order.
    pub fn to_options(self) -> [(&'static str, &'static str); 6] {
        let yn = |b| if b { "yes" } else { "no" };
        [
            ("scale", self.scale),
            ("cscale", self.cscale),
            ("dscale", self.dscale),
            ("dither-depth", if self.dither { "auto" } else { "no" }),
            ("correct-downscaling", yn(self.correct_downscaling)),
            ("linear-downscaling", yn(self.linear_downscaling)),
        ]
    }
}

/// Choose the video scalers for a wallpaper.
///
/// Power saving trades image sharpness for GPU-render load — the *right* lever,
/// because the load that pegs weak Intel GPUs for a video wallpaper is
/// Render/3D (per-frame shading), not decode. Our quality defaults (spline36 /
/// lanczos with linear-light downscaling + dithering) are several texture
/// samples and extra passes per pixel; `bilinear` with no extra passes is a
/// fraction of that. It can only reduce or match GPU work, never increase it —
/// unlike the 1.1.32 `fps` filter, which forced frames off the GPU.
///
/// Rotation safety: a *custom* chroma scaler on rotated video corrupts chroma
/// into a green cast (see the note in `mpv/player.rs`). `cscale` is therefore
/// only ever non-`bilinear` in `Full` on unrotated video; every cheaper level,
/// and all rotated video, uses `bilinear` chroma — mpv's default, which is safe.
pub fn video_scalers(scaling: Scaling, power: PowerSaving, rotated: bool) -> VideoScalers {
    match power {
        PowerSaving::Full => {
            let hi = matches!(scaling, Scaling::High);
            let luma = if hi { "lanczos" } else { "spline36" };
            VideoScalers {
                scale: luma,
                cscale: if rotated { "bilinear" } else { luma },
                dscale: if hi { "lanczos" } else { "mitchell" },
                dither: true,
                correct_downscaling: true,
                linear_downscaling: true,
            }
        }
        // Cheap luma, drop the expensive linear-light downscaling passes; keep
        // a decent downscale filter and dithering so it isn't ugly.
        PowerSaving::Reduced => VideoScalers {
            scale: "bilinear",
            cscale: "bilinear",
            dscale: "mitchell",
            dither: true,
            correct_downscaling: false,
            linear_downscaling: false,
        },
        // Cheapest path mpv offers: bilinear everywhere, no dither pass.
        PowerSaving::Minimum => VideoScalers {
            scale: "bilinear",
            cscale: "bilinear",
            dscale: "bilinear",
            dither: false,
            correct_downscaling: false,
            linear_downscaling: false,
        },
    }
}

/// How Fresco deals with Deepin DDE's covering desktop window (issue #2).
/// `Auto` probes the desktop window's visual depth and picks for itself;
/// `Transparent` forces the DBus transparent-wallpaper strategy; `Restack`
/// forces stacking our windows above DDE's desktop (icons may be hidden).
/// The `FRESCO_DDE_MODE` env var overrides this key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DdeMode {
    #[default]
    Auto,
    Transparent,
    Restack,
}

/// Light/dark preference. `System` follows the desktop's color scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Accent color applied across the UI (works in both light and dark).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    #[default]
    Blue,
    Teal,
    Green,
    Amber,
    Coral,
    Graphite,
}

/// Normalized crop rectangle (all values in 0.0..=1.0, relative to source).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Crop {
    /// Convert this crop rect to mpv `(video-zoom, video-pan-x, video-pan-y)`.
    /// Uses VO-side zoom/pan so hardware decode stays zero-copy (never `vf=crop`).
    /// The daemon sets these as mpv properties.
    pub fn to_mpv_zoom_pan(&self) -> (f64, f64, f64) {
        // video-zoom = log2(1/w): zoom so crop.w of the source fills the screen width.
        let zoom = (1.0_f64 / self.w).log2();
        let cx = self.x + self.w / 2.0;
        let cy = self.y + self.h / 2.0;
        // mpv pan is in post-zoom display units: (0.5 - center) / size.
        let pan_x = (0.5 - cx) / self.w;
        let pan_y = (0.5 - cy) / self.h;
        (zoom, pan_x, pan_y)
    }

    /// Clamp to sane bounds; returns None if the rect is degenerate.
    pub fn sanitized(self) -> Option<Crop> {
        let w = self.w.clamp(0.01, 1.0);
        let h = self.h.clamp(0.01, 1.0);
        let x = self.x.clamp(0.0, 1.0 - w);
        let y = self.y.clamp(0.0, 1.0 - h);
        if w < 1.0 || h < 1.0 {
            Some(Crop { x, y, w, h })
        } else {
            None // full-frame crop == no crop
        }
    }
}

/// Transition effect played when a slideshow advances to the next image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Transition {
    #[default]
    None,
    Crossfade,
    Fade,
    Slide,
    KenBurns,
}

/// A set of images cycled on a timer. Either a `folder` (all images inside) or
/// an explicit `paths` list of hand-picked images.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slideshow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<PathBuf>,
    #[serde(default = "default_interval")]
    pub interval_s: u64,
    #[serde(default)]
    pub transition: Transition,
}

fn default_interval() -> u64 {
    30
}

/// Time-of-day wallpaper schedule (ROADMAP 3.3). Evaluated by the daemon (the
/// always-running process); the engine itself is a pure function in
/// `crate::schedule` so it stays unit-testable and platform-neutral.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    #[serde(default)]
    pub mode: ScheduleMode,
    /// daynight/solar: what plays during the day / night.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<Wallpaper>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub night: Option<Wallpaper>,
    /// daynight: manual switch times, "HH:MM" 24h local.
    #[serde(default = "default_day_start")]
    pub day_start: String,
    #[serde(default = "default_night_start")]
    pub night_start: String,
    /// solar: manual coordinates (no geoclue — privacy + dependency weight).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    /// times: arbitrary slots; the latest slot at or before now wins (wrapping
    /// past midnight to the previous day's last slot).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub at: Vec<TimeSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleMode {
    #[default]
    Daynight,
    Times,
    Solar,
}

/// One "from this local time, show this wallpaper" rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeSlot {
    /// "HH:MM", 24h local wall clock.
    pub time: String,
    pub wallpaper: Wallpaper,
}

fn default_day_start() -> String {
    "07:00".into()
}

fn default_night_start() -> String {
    "19:00".into()
}

/// Things drawn *on top of* the wallpaper (WIDGETS_ROADMAP W1). Absent from
/// `config.toml` until the user turns something on — the same shape as
/// [`Config::browser_wallpaper`] and [`Config::schedule`], so every config
/// written by an earlier Fresco keeps parsing untouched and no one who never
/// asks for widgets ever sees the key.
///
/// A block rather than loose `lyrics_*` keys on [`Config`] because lyrics is
/// the first widget, not the only planned one; the clock and visualiser land
/// beside it here rather than spraying another dozen top-level keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Widgets {
    /// Now-playing / synced-lyrics overlay — the only widget in v1.
    #[serde(default)]
    pub lyrics: Lyrics,
    /// Wall-clock overlay; see [`Clock`].
    #[serde(default)]
    pub clock: Clock,
    /// Audio spectrum overlay; see [`Visualizer`]. The only widget that listens
    /// to the machine's sound, and the only one that redraws continuously.
    #[serde(default)]
    pub visualizer: Visualizer,
    /// Spinning album-art disc; see [`Disc`].
    #[serde(default)]
    pub disc: Disc,
    /// Which output the widgets are drawn on, by RandR/wl-output connector name
    /// (e.g. "DP-1"). `None` = **every** output.
    ///
    /// This lives on the widgets block and *not* on [`Wallpaper`] deliberately:
    /// a [`Wallpaper`] describes a piece of media and is reused verbatim as a
    /// per-monitor override value in [`Config::monitors`] — it is already keyed
    /// *by* connector there, so a `monitor` field inside it would be either
    /// redundant or contradictory. "Which screen" is a property of the widget
    /// layer, which has exactly one instance.
    ///
    /// Every output by default. The first cut drew on one screen, reasoning that
    /// a mirrored lyric is triple the libass work for no extra information —
    /// but the wallpaper itself is on every screen, so a widget on one reads as
    /// half-broken rather than as a saving. Naming a connector here is the way
    /// back to a single screen, and to that saving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<String>,
}

/// Synced-lyric overlay settings.
///
/// Lyrics are looked up **locally first** — a `.lrc` sidecar next to the audio
/// file, or a match inside [`Lyrics::folder`] — and only then online, from
/// LRCLIB, cached per-user under the XDG cache dir. Local-only was the original
/// design because it carries zero licensing exposure, but almost nobody
/// streaming from a browser has `.lrc` files on disk, so it showed nothing for
/// most people. There is deliberately no key for the online source: enabling
/// the widget enables the lookup, and a cache miss sends the track title and
/// artist to a third party. Fresco neither hosts nor licenses lyric content —
/// it fetches on demand and never bundles or redistributes a corpus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lyrics {
    /// Master switch. **False by default**, and that is a product decision, not
    /// an oversight: an overlay that appears unasked on someone's desktop is a
    /// bug report, and the roadmap's power budget only holds because nothing is
    /// created — no MPRIS watcher, no overlay, no wakeups — until this is true.
    #[serde(default)]
    pub enabled: bool,
    /// Visual preset. Presets rather than a pile of font/colour/shadow knobs:
    /// a small opinionated set is what makes this feel designed instead of
    /// configurable.
    #[serde(default)]
    pub style: LyricStylePreset,
    /// Where on the screen the lyric sits (9-point grid).
    #[serde(default)]
    pub anchor: LyricAnchor,
    /// Distance in pixels from the anchored screen edge(s). Keeps the text off
    /// panels, docks and rounded corners, which every desktop places differently.
    #[serde(default = "default_lyric_margin")]
    pub margin_px: u32,
    /// Lyric type size in points. Sized to be readable from across a room while
    /// still fitting a long line on a 1080p screen.
    #[serde(default = "default_lyric_font_size")]
    pub font_size_pt: u32,
    /// Tint the lyric with the app accent ([`Config::accent`]) so the overlay
    /// matches the rest of Fresco. Set false to let [`Lyrics::colour`], or the
    /// preset's own colour, stand — some wallpapers fight an accent.
    ///
    /// **Wins over [`Lyrics::colour`]**: while this is on the accent is the
    /// fill and the colour key is ignored, which is what the GUI says by
    /// grinding the colour row out when the switch is on.
    #[serde(default = "default_true")]
    pub accent_follow: bool,
    /// Lyric fill colour as `#RRGGBB`, used only when `accent_follow` is off.
    ///
    /// Absent by default, and absent means *the preset chooses* — Karaoke's
    /// amber, Card's near-black ink, white for the rest. That is the difference
    /// between an option and a regression: a plain `String` default would have
    /// repainted every existing accent-free Karaoke lyric white the moment this
    /// key shipped. Set it and it overrides the preset's fill; clear it and the
    /// preset has its colour back. The outline is not exposed: it is what keeps
    /// the text legible over arbitrary video, not decoration.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_opt_hex_colour"
    )]
    pub colour: Option<String>,
    /// Also show the upcoming line, dimmed. Off by default: two lines is twice
    /// the desktop covered, and it only helps if you are singing along.
    #[serde(default)]
    pub show_next_line: bool,
    /// Show the track title and artist alongside the lyric line.
    ///
    /// Off by default, and deliberately a choice rather than something Fresco
    /// decides: the lyric line is what people turn this widget on for, and a
    /// title/artist block is a second, permanent piece of furniture on the
    /// wallpaper — it is there between songs and during instrumentals, where
    /// the lyric line is not. Plenty of people already have that readout in a
    /// panel applet or on the player itself and do not want a third copy of it.
    /// Plenty of others want the wallpaper to be the now-playing display. Both
    /// are right, so it is a switch, and the quieter of the two is the default.
    ///
    /// Costs no extra wakeups: the title changes when the track does, which is
    /// an MPRIS event the widget is already listening for.
    #[serde(default)]
    pub show_track_info: bool,
    /// Global sync correction in milliseconds, added to every `.lrc` timestamp.
    /// Positive = show each line later.
    ///
    /// Every lyrics tool needs this knob because the error is in the *data*:
    /// `.lrc` files are hand-timed by strangers, some against a different
    /// master or a version with a longer intro, so a file can sit a second off
    /// no matter how exact our own clock is. Player and pipeline latency add to
    /// it. Cheaper to expose one slider than to pretend the timestamps are true.
    #[serde(default)]
    pub offset_ms: i32,
    /// Optional folder searched for `.lrc` sidecars when none sits next to the
    /// audio file — for libraries where lyrics are kept apart from the music,
    /// or where the music lives somewhere Fresco cannot write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
}

fn default_lyric_margin() -> u32 {
    48
}

fn default_lyric_font_size() -> u32 {
    28
}

impl Default for Lyrics {
    fn default() -> Self {
        Lyrics {
            enabled: false,
            style: LyricStylePreset::default(),
            anchor: LyricAnchor::default(),
            margin_px: default_lyric_margin(),
            font_size_pt: default_lyric_font_size(),
            accent_follow: true,
            colour: None,
            show_next_line: false,
            show_track_info: false,
            offset_ms: 0,
            folder: None,
        }
    }
}

/// The look of the lyric overlay. Each preset fixes font weight, colour,
/// outline and emphasis together, so the user picks a *feeling* instead of
/// assembling one out of a dozen fields that mostly combine into something ugly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LyricStylePreset {
    /// One quiet line, no decoration. The default, because a wallpaper widget
    /// should be noticed only when you look for it.
    #[default]
    Minimal,
    /// Bold and high-contrast, sized for singing along.
    Karaoke,
    /// Outlined film-subtitle look — the most legible over busy video.
    Subtitle,
    /// The line inside a soft rounded panel, so it stays readable even where
    /// the wallpaper behind it is bright.
    Card,
}

/// Nine-point placement grid for the lyric overlay. Anchors rather than raw
/// coordinates: an anchor stays correct when the resolution, orientation or
/// output changes, where a pixel position quietly ends up off-screen.
///
/// TOML spellings are the variant names lowercased with no separator —
/// `"topleft"`, `"midcenter"`, `"bottomcenter"` — matching how
/// [`Transition::KenBurns`] already serialises as `"kenburns"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LyricAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    MidLeft,
    MidCenter,
    MidRight,
    BottomLeft,
    /// The default: where subtitles have always gone, and the strip of desktop
    /// least likely to be hidden by a window or covered in icons.
    #[default]
    BottomCenter,
    BottomRight,
}

/// Wall-clock overlay settings.
///
/// Deliberately a *mirror* of [`crate::clock::ClockStyle`] rather than that type
/// re-exported: `config` is the stable, hand-audited public shape of
/// `config.toml` and must not inherit fields — or renames — from a renderer that
/// is free to change. The daemon owns the one small mapping between the two, and
/// pays for it by being the only place that has to change when the renderer
/// grows a knob we do not want to promise users forever. Two fields of
/// `ClockStyle` are intentionally absent here: `colour` (an exact hex is a
/// theming decision Fresco makes from [`Config::accent`], not a text field to
/// mistype) and anything not listed below.
///
/// The clock does *not* need MPRIS, a network, or anything playing — unlike
/// [`Lyrics`] it always has something to draw. That makes the power discipline
/// below the only thing standing between this widget and a permanently busy
/// desktop, which is why `show_seconds` gets a paragraph of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clock {
    /// Master switch. **False by default**, for the same reason [`Lyrics`] is:
    /// a widget that appears on someone's wallpaper without being asked for is
    /// a bug report, not a feature announcement. Nothing is created — no timer,
    /// no overlay, no wakeups — until this is true.
    #[serde(default)]
    pub enabled: bool,
    /// Which look to draw. Presets rather than a pile of font/weight/tracking
    /// knobs, on the same bargain [`LyricStylePreset`] makes.
    #[serde(default)]
    pub theme: ClockThemeCfg,
    /// Where on the screen the clock sits (the same 9-point grid as the lyric
    /// overlay — one placement vocabulary for every widget, so "top right"
    /// means the same thing and is spelled the same way everywhere).
    ///
    /// Defaults to [`LyricAnchor::TopRight`]: desktop icons start at the
    /// *top-left* on every desktop Fresco supports, panels and docks own the
    /// bottom edge, and lyrics already default to bottom-centre — so this is
    /// the corner where both widgets can be on at once without either being
    /// covered.
    #[serde(default = "default_clock_anchor")]
    pub anchor: LyricAnchor,
    /// Size of the time line in points. Much larger than the lyric default: a
    /// clock is read at a glance from across the room, where a lyric is read
    /// deliberately. Roughly 6% of the height of a 1080p screen.
    #[serde(default = "default_clock_font_size")]
    pub font_size_pt: u32,
    /// Distance in pixels from the anchored screen edge(s). Keeps the digits
    /// off panels, docks and rounded corners, which every desktop places
    /// differently.
    #[serde(default = "default_clock_margin")]
    pub margin_px: u32,
    /// Show seconds. **Off by default, and that is a power decision.**
    ///
    /// A clock without seconds changes its text once a minute, so the widget
    /// layer sleeps until the top of the next minute and the machine records no
    /// wakeups attributable to Fresco. Turning seconds on makes the text change
    /// every second: one redraw a minute becomes sixty, a **60× increase in
    /// wakeups and composite passes** for the whole widget layer, permanently,
    /// on an idle desktop (WIDGETS_ROADMAP "Power model": the clock is budgeted
    /// at 1/minute, and 1 Hz *only if seconds are enabled*). That is a real
    /// cost on a fanless laptop and it buys a digit pair few people look at, so
    /// it is opt-in — never a default, and never enabled on the user's behalf.
    ///
    /// [`ClockThemeCfg::Wordy`] ignores this: there is no way to say "and
    /// seventeen seconds", so the renderer drops it and keeps the cheap tick.
    #[serde(default)]
    pub show_seconds: bool,
    /// Show the date under the time. Off by default — the date is a second line
    /// of desktop covered for something most people can already see in their
    /// panel clock.
    ///
    /// Two themes overrule this on purpose: [`ClockThemeCfg::Stacked`] *is*
    /// "time with the date beneath it" and always shows it, and
    /// [`ClockThemeCfg::Minimal`] is defined as time only and never does. Both
    /// are free — the date changes at midnight either way.
    #[serde(default)]
    pub show_date: bool,
    /// 24-hour clock. **On by default**, for three reasons that all point the
    /// same way: `HH:MM` is unambiguous without a meridiem suffix, it is a
    /// fixed width so a right-anchored overlay does not shift as 09:59 becomes
    /// 10:00, and the rest of the config already speaks it ([`Schedule::day_start`]
    /// is 24-hour `HH:MM`).
    #[serde(default = "default_true")]
    pub use_24h: bool,
    /// Draw the clock in the app accent ([`Config::accent`]) so the overlay
    /// matches the rest of Fresco. On by default, matching [`Lyrics::accent_follow`];
    /// set false to get plain white, which some wallpapers need.
    #[serde(default = "default_true")]
    pub accent_follow: bool,
}

fn default_clock_anchor() -> LyricAnchor {
    LyricAnchor::TopRight
}

fn default_clock_font_size() -> u32 {
    64
}

fn default_clock_margin() -> u32 {
    56
}

impl Default for Clock {
    fn default() -> Self {
        Clock {
            enabled: false,
            theme: ClockThemeCfg::default(),
            anchor: default_clock_anchor(),
            font_size_pt: default_clock_font_size(),
            margin_px: default_clock_margin(),
            show_seconds: false,
            show_date: false,
            use_24h: true,
            accent_follow: true,
        }
    }
}

/// The look of the clock overlay — the config-file spelling of
/// [`crate::clock::ClockTheme`], variant for variant.
///
/// TOML spellings are the variant names lowercased: `"digital"`, `"minimal"`,
/// `"segment"`, `"stacked"`, `"wordy"`, matching how [`LyricStylePreset`]
/// already serialises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClockThemeCfg {
    /// Clean, large, bold `HH:MM`. The default: a clock should read at a glance
    /// and then be forgotten.
    #[default]
    Digital,
    /// Thin, small, wide-set and lower case. Time only — no date, ever.
    Minimal,
    /// Seven-segment LED feel: monospace, heavy tracking, and a glow.
    Segment,
    /// Big time with the date stacked beneath it. The date is the theme, so it
    /// is shown whatever [`Clock::show_date`] says.
    Stacked,
    /// The time spelled out — "half past ten". Rounded to the nearest five
    /// minutes, because that is how people say it, and consequently the one
    /// theme that cannot show seconds.
    Wordy,
    /// A rounded card carrying the digital time, weekday and date, with an
    /// analog face drawn beneath them. The most decorative theme and the most
    /// drawing work per repaint — it is a vector card, ticks and hands, not
    /// just text — so it costs more than the others at the same cadence.
    Card,
}

/// Audio-spectrum overlay settings.
///
/// Two costs are stated here rather than buried, because both are real and
/// neither is a matter of taste.
///
/// **It captures your system audio output.** The visualiser has no way to know
/// what the music looks like without listening to it: the daemon opens a
/// monitor source on PipeWire or PulseAudio and reads the mix your speakers are
/// playing — every application's sound, not just the music player's. That is a
/// genuine privacy surface, and it is a different one from [`Lyrics`], which
/// only ever asks MPRIS for a track title. Nothing is analysed, stored or sent
/// anywhere — the samples become bar heights and are dropped — but a capture
/// stream is a capture stream, and some desktops will show a recording
/// indicator while one is open. So this is [`Visualizer::enabled`]-gated,
/// **false by default**, and nothing opens a capture device until a person
/// deliberately turns it on. It is never enabled on the user's behalf, and no
/// migration will ever turn it on for an existing config.
///
/// **It is the one widget that legitimately redraws continuously.** Lyrics
/// repaint when the line changes — a handful of times a minute — and the clock
/// repaints once a minute unless [`Clock::show_seconds`] is set. The visualiser
/// has to repaint every frame while sound is playing, because a spectrum that
/// updates at 1 Hz is not a spectrum. That is tens of composite passes a second
/// for as long as the music runs, and it is the most expensive thing the widget
/// layer can be asked to do. It costs meaningfully more power than the other
/// widgets; on a fanless laptop it is the difference between an idle machine
/// and a warm one. The daemon stops pushing frames when the audio is silent, so
/// the bill is only paid while something is actually playing — but while it is
/// playing, it is paid in full.
///
/// The fields below mirror `crate::visualizer::VisualStyleCfg` rather than
/// re-exporting it, on exactly the bargain [`Clock`] documents: this module is
/// the hand-audited public shape of `config.toml` and must not inherit renames
/// from a renderer that is free to change. One of that struct's fields is
/// deliberately absent — `gap_px`, because the spacing between bars is part of
/// what makes each style look like itself and nothing good comes of letting it
/// be set to zero.
///
/// # Colour
///
/// Three keys, and one rule between them. [`Visualizer::accent_follow`] wins:
/// while it is on, the spectrum is drawn in [`Config::accent`] and
/// [`Visualizer::colour`] is ignored (the GUI greys it out to say so). With it
/// off, `colour` is the fill. [`Visualizer::gradient`] then decides whether
/// that fill is flat or the near end of a ramp to
/// [`Visualizer::colour_end`] — see [`GradientMode`], and note that the ramp's
/// near end is whichever of accent/`colour` is in force, so the two settings
/// compose instead of contradicting.
///
/// An unreadable hex in any of them falls back to white on the way in rather
/// than reaching the renderer: `config.toml` is hand-editable, and a malformed
/// colour must cost the tint and never the overlay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Visualizer {
    /// Master switch. **False by default** — see the type docs: turning this on
    /// is what opens an audio capture stream and starts the continuous redraw.
    /// Neither happens until it is true.
    #[serde(default)]
    pub enabled: bool,
    /// Which silhouette to draw.
    #[serde(default)]
    pub style: VisualizerStyleCfg,
    /// Where on the screen the visualiser box sits (the same 9-point grid as
    /// every other widget — one placement vocabulary, spelled one way).
    ///
    /// Defaults to [`LyricAnchor::BottomCenter`], matching the renderer's own
    /// default: a spectrum reads as a floor the sound stands on, which only
    /// works along the bottom edge.
    #[serde(default)]
    pub anchor: LyricAnchor,
    /// Box width as a percentage of the screen width. A percentage and not
    /// pixels because this is the axis people think about proportionally
    /// ("about half the screen"), and because it stays right on an ultrawide.
    #[serde(default = "default_visualizer_width_pct")]
    pub width_pct: u32,
    /// Box height in pixels at 1080p, scaled with the screen.
    #[serde(default = "default_visualizer_height_px")]
    pub height_px: u32,
    /// Distance in pixels from the anchored screen edge(s). Ignored on
    /// whichever axis the anchor is centred, exactly as in the lyric widget.
    #[serde(default = "default_visualizer_margin_px")]
    pub margin_px: u32,
    /// How many frequency bands the spectrum is split into — the number of bars,
    /// dots or wave control points.
    ///
    /// More bands is more detail and slightly more work per frame, but the real
    /// limit is optical: past roughly two hundred the bars are thinner than the
    /// gaps between them and the whole thing reads as noise, so the renderer
    /// folds anything larger down before drawing.
    #[serde(default = "default_visualizer_bands")]
    pub bands: u32,
    /// Draw the spectrum in the app accent ([`Config::accent`]) so the overlay
    /// matches the rest of Fresco. On by default, matching [`Lyrics::accent_follow`]
    /// and [`Clock::accent_follow`]. **Wins over [`Visualizer::colour`]** — see
    /// the type docs.
    #[serde(default = "default_true")]
    pub accent_follow: bool,
    /// Fill colour as `#RRGGBB`, used when `accent_follow` is off, and the near
    /// end of the ramp when a gradient is on.
    ///
    /// Defaults to white, which is what the renderer has always drawn with the
    /// accent switched off. Not a theming decision Fresco can make on the
    /// user's behalf: a spectrum sits on the wallpaper, and the colour that
    /// works over a given wallpaper is not the colour of the app's buttons.
    #[serde(default = "default_widget_colour", deserialize_with = "de_hex_colour")]
    pub colour: String,
    /// Whether the colour varies across the bars, and how. See [`GradientMode`].
    #[serde(default)]
    pub gradient: GradientMode,
    /// The far end of the ramp as `#RRGGBB`. Used by [`GradientMode::Linear`]
    /// only; the other modes ignore it.
    #[serde(default = "default_widget_colour", deserialize_with = "de_hex_colour")]
    pub colour_end: String,
    /// 0 (invisible) to 255 (solid). Below full opacity by default: a spectrum
    /// is motion, and motion at full strength over a wallpaper pulls the eye
    /// away from everything else on the desktop.
    #[serde(default = "default_visualizer_opacity")]
    pub opacity: u8,
    /// Round the ends of the shapes — rounded bar caps, circular dots, curved
    /// wave segments. Off gives the same layout with hard edges.
    #[serde(default = "default_true")]
    pub rounded: bool,
}

fn default_visualizer_width_pct() -> u32 {
    60
}

fn default_visualizer_height_px() -> u32 {
    120
}

fn default_visualizer_margin_px() -> u32 {
    48
}

fn default_visualizer_bands() -> u32 {
    32
}

fn default_visualizer_opacity() -> u8 {
    220
}

impl Default for Visualizer {
    fn default() -> Self {
        Visualizer {
            enabled: false,
            style: VisualizerStyleCfg::default(),
            anchor: LyricAnchor::BottomCenter,
            width_pct: default_visualizer_width_pct(),
            height_px: default_visualizer_height_px(),
            margin_px: default_visualizer_margin_px(),
            bands: default_visualizer_bands(),
            accent_follow: true,
            colour: default_widget_colour(),
            gradient: GradientMode::default(),
            colour_end: default_widget_colour(),
            opacity: default_visualizer_opacity(),
            rounded: true,
        }
    }
}

/// The colour every widget falls back to: plain white, which is what the
/// renderers draw with no accent and no preference.
fn default_widget_colour() -> String {
    "#FFFFFF".to_string()
}

/// `#RGB` / `#RRGGBB` → a normalised `#RRGGBB`, or `None`.
///
/// The same rule `crate::lyrics` parses colours with, applied here so a
/// malformed value is caught at the edge of the program instead of somewhere
/// inside an ASS payload. Written out rather than imported because this module
/// is the hand-audited shape of `config.toml` and does not take its validation
/// from a renderer that is free to change.
fn normalise_hex(raw: &str) -> Option<String> {
    let h = raw.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match h.len() {
        // Shorthand doubles each nibble, so `f` is 0xFF and not 0xF0.
        3 => {
            let mut out = String::with_capacity(7);
            out.push('#');
            for c in h.chars() {
                out.push(c.to_ascii_uppercase());
                out.push(c.to_ascii_uppercase());
            }
            Some(out)
        }
        6 => Some(format!("#{}", h.to_ascii_uppercase())),
        _ => None,
    }
}

/// Read a `#RRGGBB` colour, falling back to white on anything unusable.
///
/// Deserialising rather than validating later is the point: every consumer of
/// these fields — renderer, GUI colour button, daemon — then gets a value it
/// can trust, and the one place that has to know what a colour looks like is
/// this one.
fn de_hex_colour<'de, D>(d: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = String::deserialize(d)?;
    Ok(normalise_hex(&raw).unwrap_or_else(default_widget_colour))
}

/// Read an optional `#RRGGBB` colour. Unusable input becomes `None` — for
/// [`Lyrics::colour`] that means "keep the preset's own colour", which is a
/// better answer to a typo than a white lyric.
fn de_opt_hex_colour<'de, D>(d: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(d)?;
    Ok(raw.as_deref().and_then(normalise_hex))
}

/// How the visualiser's colour varies across the bars — the config-file
/// spelling of `crate::visualizer::Gradient`, variant for variant.
///
/// ASS has no gradient primitive: a ramp is drawn by giving each bar (or each
/// small run of bars) its own flat fill, stepping the colour along. That is why
/// this is a small closed set of modes rather than a list of colour stops —
/// every extra stop is more work in every frame, forever, on the one widget
/// that already redraws continuously.
///
/// TOML spellings are the variant names lowercased: `"none"`, `"linear"`,
/// `"spectrum"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GradientMode {
    /// One flat colour across the whole widget. The default: a spectrum is
    /// already busy, and the colour should be something the user asks for.
    #[default]
    None,
    /// Ramp from the fill colour (or the accent) to [`Visualizer::colour_end`].
    Linear,
    /// A fixed hue sweep, red round to violet. Ignores both colour keys —
    /// which is the point of having it: it is the classic visualiser look and
    /// it needs no colour picking at all.
    Spectrum,
}

/// The look of the visualiser — the config-file spelling of
/// [`crate::visualizer::VisualStyle`], variant for variant.
///
/// Named `VisualizerStyleCfg` and not `VisualStyleCfg` on purpose: the
/// renderer already has a `VisualStyleCfg`, and it is a *struct* holding a
/// fully resolved look. Two types one word apart, one an enum and one a struct,
/// both in scope wherever the daemon maps between them, is a trap; the extra
/// syllable buys a name that cannot be confused for it.
///
/// TOML spellings are the variant names lowercased: `"bars"`, `"mirror"`,
/// `"wave"`, `"dots"`, `"ring"`, matching how [`ClockThemeCfg`] already
/// serialises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VisualizerStyleCfg {
    /// Classic spectrum bars rising from a floor. The default, because it is
    /// the shape everyone already reads as "this is the music".
    #[default]
    Bars,
    /// Bars mirrored about a centre line — the symmetric "equaliser" look.
    Mirror,
    /// One continuous filled silhouette instead of discrete bars.
    Wave,
    /// A row of dots that ride up and down and grow with their band.
    Dots,
    /// Bars radiating outward from a hub ring.
    Ring,
}

/// Album-art disc overlay settings.
///
/// The cover of whatever is playing, drawn as a record that turns while the
/// track does. Like [`Lyrics`] it needs a media player reporting over MPRIS,
/// and like [`Lyrics`] it draws nothing at all when nothing is playing — but
/// unlike lyrics it reads no files of the user's and opens no capture device.
/// The artwork comes from the player's own metadata.
///
/// **Spinning is not free.** A still disc is drawn once per track and then
/// costs nothing; with [`Disc::spin`] on it is re-rendered continuously for as
/// long as playback runs, which puts it in the same power bracket as the
/// [`Visualizer`] rather than the same one as [`Lyrics`] and [`Clock`] (both of
/// which repaint only when their content changes). It is still on by default —
/// a record that does not turn is a circle, and the motion is the entire point
/// of the widget — but the switch is there, and turning it off makes this the
/// cheapest widget Fresco has. Rotation stops with playback either way: a
/// paused track holds its angle instead of spinning on.
///
/// The geometry knobs of `crate::artwork::DiscCfg` — label size, spindle-hole
/// size, rim darkening — are deliberately *not* here. They are what make the
/// thing read as a record at all, and there is no setting of them a user wants
/// that the renderer's proportions do not already give.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disc {
    /// Master switch. **False by default**, for the same reason every other
    /// widget is: an overlay that appears on someone's wallpaper unasked is a
    /// bug report, not a feature announcement. No MPRIS watcher, no artwork
    /// fetch, no overlay and no wakeups exist until this is true.
    #[serde(default)]
    pub enabled: bool,
    /// Where on the screen the disc sits (the same 9-point grid as every other
    /// widget).
    ///
    /// Defaults to [`LyricAnchor::BottomRight`], the last corner left: desktop
    /// icons start top-left on every desktop Fresco supports, [`Clock`] defaults
    /// to top-right, and lyrics and the visualiser both default to
    /// bottom-centre. This is the one anchor at which every widget can be on at
    /// once without any of them landing on another.
    #[serde(default = "default_disc_anchor")]
    pub anchor: LyricAnchor,
    /// Diameter of the disc in pixels at 1080p, scaled with the screen. Sized
    /// to be recognisable as the album you are listening to without becoming a
    /// poster.
    #[serde(default = "default_disc_size_px")]
    pub size_px: u32,
    /// Distance in pixels from the anchored screen edge(s). Keeps the disc off
    /// panels, docks and rounded corners, which every desktop places
    /// differently.
    #[serde(default = "default_disc_margin_px")]
    pub margin_px: u32,
    /// Turn the disc while the track plays, at 33⅓ rpm — the speed of the LP
    /// the widget is imitating.
    ///
    /// **On by default, and the one power knob of this widget.** Spinning means
    /// the disc is re-rendered continuously during playback instead of once per
    /// track; see the type docs. Off keeps the artwork and the record styling
    /// and pays for neither, which is the right setting on a laptop running on
    /// battery.
    #[serde(default = "default_true")]
    pub spin: bool,
    /// 0 (invisible) to 255 (solid). Full by default — this widget *is* the
    /// artwork, and a faded cover looks like a rendering fault rather than a
    /// choice.
    #[serde(default = "default_disc_opacity")]
    pub opacity: u8,
}

fn default_disc_anchor() -> LyricAnchor {
    LyricAnchor::BottomRight
}

fn default_disc_size_px() -> u32 {
    220
}

fn default_disc_margin_px() -> u32 {
    48
}

fn default_disc_opacity() -> u8 {
    255
}

impl Default for Disc {
    fn default() -> Self {
        Disc {
            enabled: false,
            anchor: default_disc_anchor(),
            size_px: default_disc_size_px(),
            margin_px: default_disc_margin_px(),
            spin: true,
            opacity: default_disc_opacity(),
        }
    }
}

fn default_volume() -> u8 {
    50
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wallpaper {
    #[serde(default)]
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub fit: Fit,
    /// Clockwise rotation in degrees: 0, 90, 180, or 270.
    #[serde(default)]
    pub rotation: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    #[serde(default = "default_true")]
    pub mute: bool,
    #[serde(default = "default_volume")]
    pub volume: u8,
    /// Per-wallpaper power-saving override. `None` inherits the global
    /// [`Config::power_saving`]; `Some(_)` overrides it for this wallpaper —
    /// e.g. leave one showpiece clip on Full while everything else saves power.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_saving: Option<PowerSaving>,
    /// Deprecated 1.1.32 per-wallpaper frame-rate cap; parsed for backward
    /// compatibility and migrated on load. Never applied. See
    /// [`Config::power_saving`].
    #[serde(default, skip_serializing)]
    pub framerate: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slideshow: Option<Slideshow>,
}

impl Wallpaper {
    /// The single media path to load for video/image/playlist-of-one.
    /// Returns None for slideshows (the daemon drives those frame by frame).
    pub fn effective_path(&self) -> Option<&std::path::Path> {
        self.path
            .as_deref()
            .or_else(|| self.paths.first().map(|p| p.as_path()))
    }

    /// Power-saving level actually applied to this wallpaper: the per-wallpaper
    /// override if set, otherwise the `global` default.
    pub fn effective_power_saving(&self, global: PowerSaving) -> PowerSaving {
        self.power_saving.unwrap_or(global)
    }
}

impl Default for Wallpaper {
    fn default() -> Self {
        Wallpaper {
            kind: Kind::default(),
            path: None,
            paths: Vec::new(),
            shuffle: false,
            fit: Fit::default(),
            rotation: 0,
            crop: None,
            mute: true,
            volume: default_volume(),
            power_saving: None,
            framerate: None,
            slideshow: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    /// Restore wallpaper on login (autostart entry present).
    #[serde(default = "default_true")]
    pub autostart: bool,
    /// False after the user hits Stop — autostart must not resurrect it.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub pause_on_battery: bool,
    #[serde(default)]
    pub scaling: Scaling,
    /// Global decode-load reduction; see [`PowerSaving`].
    #[serde(default)]
    pub power_saving: PowerSaving,
    /// Deprecated 1.1.32 frame-rate cap, in fps. Retained only so configs
    /// written by 1.1.32 still parse; the value is migrated to
    /// [`Config::power_saving`] on load (see `Config::migrate`) and never
    /// applied to mpv — the filter it drove made decode load worse, not better.
    #[serde(default, skip_serializing)]
    pub framerate: u16,
    /// Deepin DDE strategy (auto | transparent | restack); see [`DdeMode`].
    #[serde(default)]
    pub dde_mode: DdeMode,
    /// Light/dark preference (System follows the desktop).
    #[serde(default)]
    pub theme_mode: ThemeMode,
    /// UI language (System follows `LC_ALL`/`LC_MESSAGES`/`LANG`).
    ///
    /// A sibling of [`Config::theme_mode`], and explicit for the same reason it
    /// is: the desktop's own setting is the right default but the wrong
    /// mandate. Running an English locale while wanting a Chinese UI is a
    /// common, deliberate setup, so the inference needs an override.
    #[serde(default)]
    pub language: crate::i18n::Language,
    /// UI accent color.
    #[serde(default)]
    pub accent: Accent,
    #[serde(default)]
    pub wallpaper: Wallpaper,
    /// Last app version whose "What's new" notes the user has already seen.
    #[serde(default)]
    pub last_seen_version: String,
    /// Unix epoch (seconds) of first run; drives the one-time feedback prompt.
    #[serde(default)]
    pub first_run_epoch: u64,
    /// True once the (one-time, opt-in) feedback prompt has been shown.
    #[serde(default)]
    pub feedback_prompted: bool,
    /// Periodic desktop reminder to send feedback (every 5 hours until the
    /// user submits once). Set false in config.toml to silence it.
    #[serde(default = "default_true")]
    pub feedback_reminders: bool,
    /// Full anonymous usage telemetry (daily ping with a random install id,
    /// feature counts, error kinds). Opt-out via the Settings switch or
    /// config.toml. False does NOT mean total silence: see
    /// [`Config::telemetry_consent_version`].
    #[serde(default = "default_true")]
    pub telemetry: bool,
    /// Whether the telemetry consent dialog was answered. Nothing is ever sent
    /// before this is true — consent-first, like a cookie banner but honest
    /// (no dark patterns, both buttons equal weight).
    #[serde(default)]
    pub telemetry_prompted: bool,
    /// Which revision of the consent terms the answer above was given under.
    ///
    /// This exists because declining is no longer total silence: declining the
    /// optional statistics still sends a daily install id + country check-in
    /// (see [`crate::telemetry::minimal_heartbeat`]). Someone who declined
    /// under an earlier revision agreed to something different, so re-asking
    /// them once is the only honest way to change what declining means. Bump
    /// [`crate::telemetry::CONSENT_VERSION`] whenever the terms change again,
    /// and every install below it is asked exactly once more.
    #[serde(default)]
    pub telemetry_consent_version: u32,
    /// Whether the user has agreed, in the one-time dialog, to let the audio
    /// visualiser listen to the computer's sound output.
    ///
    /// A sibling of [`Config::telemetry_prompted`] and for the same reason.
    /// [`Visualizer`] is the only thing Fresco ships that opens a capture
    /// stream on the system mix — every application's sound, not just the
    /// music player's — and a capture stream that starts because a switch was
    /// flipped in a settings dialog is not consent. So the flag is separate
    /// from [`Visualizer::enabled`]: enabling the widget says what you want,
    /// this says you were told what it costs.
    ///
    /// **False by default, and enforced on load**, not merely in the GUI:
    /// [`Config::load_from`] switches the visualiser back off if this is not
    /// set, so hand-editing `enabled = true` into `config.toml` cannot start a
    /// capture the user was never asked about. Every process reads its config
    /// through that function, the daemon included.
    #[serde(default)]
    pub audio_capture_consented: bool,
    /// Local browser bridge (127.0.0.1 only): lets the Fresco browser
    /// extension mirror the wallpaper on new tabs. Off by default — nothing
    /// listens on any port unless the user opts in.
    #[serde(default)]
    pub browser_bridge: bool,
    /// Optional wallpaper shown ONLY in the browser (new-tab extension),
    /// independent of the desktop. None = mirror the desktop wallpaper.
    /// Follows the per-monitor override pattern: absent from config.toml
    /// unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_wallpaper: Option<Wallpaper>,
    /// Successful wallpaper applies so far — the star nudge stays silent until
    /// the user has visibly gotten value (3+ applies).
    #[serde(default)]
    pub apply_count: u32,
    /// Unix epoch (seconds) of the last "star Fresco on GitHub" nudge, so it
    /// repeats at most once every 2 days.
    #[serde(default)]
    pub last_star_nudge: u64,
    /// Whether the one-time "What can Fresco do?" feature tour was shown.
    #[serde(default)]
    pub tour_shown: bool,
    /// Highest onboarding revision this install has been walked through.
    /// Versioned rather than boolean so a release that introduces a flow worth
    /// teaching can re-show it to *existing* users (who already have
    /// `tour_shown = true` and would otherwise never see it) by bumping
    /// `ONBOARDING_VERSION`. 0 means "never shown".
    #[serde(default)]
    pub onboarding_version: u32,
    /// IDs of admin notifications already shown, so each appears only once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen_notifications: Vec<String>,
    /// Per-monitor overrides keyed by RandR connector name (e.g. "HDMI-1").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub monitors: BTreeMap<String, Wallpaper>,
    /// Unix epoch (seconds) of the last GitHub Releases check, so the client
    /// self-throttles to roughly once every 24h.
    #[serde(default)]
    pub last_update_check: u64,
    /// The latest version the user chose "Later" for, so the banner doesn't
    /// re-appear for that same version on the next check.
    #[serde(default)]
    pub update_skipped_version: String,
    /// Optional time-of-day schedule for the default wallpaper (v1: does not
    /// apply to per-monitor overrides). Absent = no scheduling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
    /// Temporarily suspend the schedule WITHOUT deleting it — the quick
    /// on/off switch in the menu flips this, so users don't lose their
    /// configured day/night setup just to pause it.
    #[serde(default)]
    pub schedule_paused: bool,
    /// Optional overlay widgets drawn on top of the wallpaper; see [`Widgets`].
    /// Absent = no widgets, which is what every config written before this
    /// feature existed says, so they all keep their current behaviour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widgets: Option<Widgets>,
}

fn default_version() -> u32 {
    1
}

/// Translate a deprecated 1.1.32 frame-rate cap into a power-saving level.
/// Any cap meant "I want less load", which is what `Reduced` delivers — this
/// time without making decode worse.
fn power_saving_from_legacy_framerate(fps: u16) -> Option<PowerSaving> {
    (fps > 0).then_some(PowerSaving::Reduced)
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            autostart: true,
            enabled: true,
            pause_on_battery: false,
            scaling: Scaling::default(),
            power_saving: PowerSaving::default(),
            framerate: 0,
            dde_mode: DdeMode::default(),
            theme_mode: ThemeMode::default(),
            language: crate::i18n::Language::default(),
            accent: Accent::default(),
            wallpaper: Wallpaper::default(),
            last_seen_version: String::new(),
            first_run_epoch: 0,
            feedback_prompted: false,
            feedback_reminders: true,
            telemetry: true,
            telemetry_prompted: false,
            telemetry_consent_version: 0,
            audio_capture_consented: false,
            browser_bridge: false,
            browser_wallpaper: None,
            apply_count: 0,
            last_star_nudge: 0,
            tour_shown: false,
            onboarding_version: 0,
            seen_notifications: Vec::new(),
            monitors: BTreeMap::new(),
            last_update_check: 0,
            update_skipped_version: String::new(),
            schedule: None,
            schedule_paused: false,
            widgets: None,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("fresco")
            .join("config.toml")
    }

    pub fn load() -> Result<Config> {
        Self::load_from(&Self::path())
    }

    pub fn load_from(path: &std::path::Path) -> Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        cfg.migrate();
        cfg.enforce_audio_consent();
        Ok(cfg)
    }

    /// Refuse to run the visualiser until the audio-capture dialog has been
    /// answered — see [`Config::audio_capture_consented`].
    ///
    /// Here rather than in the GUI because the GUI is not the only writer of
    /// `config.toml` and not the only reader of it. This runs inside
    /// [`Config::load_from`], which is how every process in the program —
    /// `fresco`, `frescod`, the CLI — obtains its configuration, so there is
    /// no path by which a capture device opens without the flag. The daemon
    /// needs no check of its own; if one is ever added it must be exactly this
    /// one.
    ///
    /// It clears `enabled` rather than remembering it: a stored "on, but not
    /// really" is a state the GUI would then have to render, and the switch is
    /// one click.
    fn enforce_audio_consent(&mut self) {
        if self.audio_capture_consented {
            return;
        }
        if let Some(w) = self.widgets.as_mut() {
            w.visualizer.enabled = false;
        }
    }

    /// Fold deprecated keys into their replacements. Runs on every load and is
    /// idempotent; the migrated value is persisted on the next save (the old
    /// keys are `skip_serializing`, so they disappear then).
    fn migrate(&mut self) {
        // 1.1.32's frame-rate cap -> power saving. Only fills a value left at
        // the default, so an explicit `power_saving` in the file always wins.
        // (Now largely moot: 1.1.32 configs have no `power_saving` key at all,
        // so serde already defaults them to Reduced — the same target.)
        if self.power_saving == PowerSaving::default() {
            if let Some(p) = power_saving_from_legacy_framerate(self.framerate) {
                self.power_saving = p;
            }
        }
        self.framerate = 0;
        if self.wallpaper.power_saving.is_none() {
            self.wallpaper.power_saving = self
                .wallpaper
                .framerate
                .and_then(power_saving_from_legacy_framerate);
        }
        self.wallpaper.framerate = None;
        // `widgets` needs nothing here: it has never shipped under another
        // name, so no released config can contain a deprecated spelling of it.
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path())
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self)?;
        // Write-then-rename so a crash mid-write can't corrupt the config.
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Effective wallpaper for a connector, honoring per-monitor overrides.
    pub fn wallpaper_for(&self, connector: &str) -> &Wallpaper {
        self.monitors.get(connector).unwrap_or(&self.wallpaper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_from_empty_toml() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg, Config::default());
        assert!(cfg.autostart);
        assert!(cfg.enabled);
        assert!(cfg.wallpaper.mute);
        assert_eq!(cfg.wallpaper.volume, 50);
        // Absent power_saving key → Reduced: measured to capture nearly all the
        // available GPU saving at 1080p for a barely-visible softening.
        assert_eq!(cfg.power_saving, PowerSaving::Reduced);
    }

    #[test]
    fn power_saving_reduces_scaler_cost() {
        // Full = quality scalers; cheaper levels drop to bilinear + fewer passes.
        let full = video_scalers(Scaling::Balanced, PowerSaving::Full, false);
        assert_eq!(full.scale, "spline36");
        assert!(full.correct_downscaling && full.linear_downscaling && full.dither);

        let reduced = video_scalers(Scaling::Balanced, PowerSaving::Reduced, false);
        assert_eq!(reduced.scale, "bilinear");
        assert!(!reduced.correct_downscaling && !reduced.linear_downscaling);

        let min = video_scalers(Scaling::High, PowerSaving::Minimum, false);
        assert_eq!(min.scale, "bilinear");
        assert_eq!(min.dscale, "bilinear");
        assert!(!min.dither);
        // High quality is overridden downward by Minimum — power saving wins.
        assert_ne!(min.scale, "lanczos");
    }

    #[test]
    fn chroma_scaler_is_bilinear_whenever_rotated_or_saving() {
        // The green-cast bug: a custom cscale on ROTATED video corrupts chroma.
        // cscale may only be non-bilinear in Full + unrotated.
        assert_ne!(
            video_scalers(Scaling::High, PowerSaving::Full, false).cscale,
            "bilinear"
        );
        assert_eq!(
            video_scalers(Scaling::High, PowerSaving::Full, true).cscale,
            "bilinear"
        );
        for power in [PowerSaving::Reduced, PowerSaving::Minimum] {
            for rotated in [false, true] {
                assert_eq!(
                    video_scalers(Scaling::High, power, rotated).cscale,
                    "bilinear"
                );
            }
        }
    }

    #[test]
    fn legacy_framerate_migrates_to_power_saving() {
        // A config written by 1.1.32 must still load, and its frame-rate cap
        // becomes the equivalent intent: reduce load.
        let cfg: Config = toml::from_str("framerate = 30").unwrap();
        let mut cfg = cfg;
        cfg.migrate();
        assert_eq!(cfg.power_saving, PowerSaving::Reduced);
        assert_eq!(cfg.framerate, 0, "legacy key must not survive migration");

        // framerate = 0 meant "original rate". There is no power_saving key to
        // preserve, so such a config lands on the current default like any
        // other — the legacy key must simply not push it further.
        let mut untouched: Config = toml::from_str("framerate = 0").unwrap();
        untouched.migrate();
        assert_eq!(untouched.power_saving, PowerSaving::default());

        // An explicit power_saving always wins over the legacy key.
        let mut explicit: Config =
            toml::from_str("framerate = 30\npower_saving = \"minimum\"").unwrap();
        explicit.migrate();
        assert_eq!(explicit.power_saving, PowerSaving::Minimum);
    }

    #[test]
    fn legacy_per_wallpaper_framerate_migrates() {
        let mut cfg: Config = toml::from_str("[wallpaper]\nframerate = 24").unwrap();
        cfg.migrate();
        assert_eq!(cfg.wallpaper.power_saving, Some(PowerSaving::Reduced));
        assert_eq!(cfg.wallpaper.framerate, None);
    }

    #[test]
    fn effective_power_saving_prefers_override_then_global() {
        let mut w = Wallpaper::default();
        // No override → inherit the global default.
        assert_eq!(
            w.effective_power_saving(PowerSaving::Full),
            PowerSaving::Full
        );
        assert_eq!(
            w.effective_power_saving(PowerSaving::Reduced),
            PowerSaving::Reduced
        );
        // Override wins, including forcing Full back on under a saving default.
        w.power_saving = Some(PowerSaving::Minimum);
        assert_eq!(
            w.effective_power_saving(PowerSaving::Full),
            PowerSaving::Minimum
        );
        w.power_saving = Some(PowerSaving::Full);
        assert_eq!(
            w.effective_power_saving(PowerSaving::Minimum),
            PowerSaving::Full
        );
    }

    #[test]
    fn power_saving_roundtrips_through_toml() {
        let cfg = Config {
            power_saving: PowerSaving::Reduced,
            ..Config::default()
        };
        let s = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.power_saving, PowerSaving::Reduced);
    }

    #[test]
    fn roundtrip() {
        let mut cfg = Config::default();
        cfg.wallpaper.kind = Kind::Playlist;
        cfg.wallpaper.paths = vec!["/a.mp4".into(), "/b.webm".into()];
        cfg.wallpaper.crop = Some(Crop {
            x: 0.1,
            y: 0.2,
            w: 0.5,
            h: 0.5,
        });
        cfg.pause_on_battery = true;
        cfg.monitors.insert(
            "HDMI-1".into(),
            Wallpaper {
                kind: Kind::Image,
                path: Some("/p.png".into()),
                ..Default::default()
            },
        );
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn save_load_file() {
        let dir = std::env::temp_dir().join(format!("fresco-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let cfg = Config {
            enabled: false,
            ..Default::default()
        };
        cfg.save_to(&path).unwrap();
        let back = Config::load_from(&path).unwrap();
        assert_eq!(cfg, back);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn crop_to_mpv_zoom_pan() {
        // Full frame: no zoom, no pan.
        let (z, px, py) = (Crop {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        })
        .to_mpv_zoom_pan();
        assert!(z.abs() < 1e-9 && px.abs() < 1e-9 && py.abs() < 1e-9);
        // Center 50%: zoom 1 stop, no pan.
        let (z, px, py) = (Crop {
            x: 0.25,
            y: 0.25,
            w: 0.5,
            h: 0.5,
        })
        .to_mpv_zoom_pan();
        assert!((z - 1.0).abs() < 1e-9 && px.abs() < 1e-9 && py.abs() < 1e-9);
        // Top-left quarter: zoom 1 stop, pan right+down by 0.5.
        let (z, px, py) = (Crop {
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 0.5,
        })
        .to_mpv_zoom_pan();
        assert!((z - 1.0).abs() < 1e-9 && (px - 0.5).abs() < 1e-9 && (py - 0.5).abs() < 1e-9);
    }

    #[test]
    fn crop_sanitize() {
        // Out-of-bounds rect gets clamped.
        let c = Crop {
            x: 0.9,
            y: -0.5,
            w: 0.5,
            h: 0.5,
        }
        .sanitized()
        .unwrap();
        assert!((c.x + c.w) <= 1.0 + f64::EPSILON);
        assert!(c.y >= 0.0);
        // Full-frame crop collapses to None.
        assert!(Crop {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0
        }
        .sanitized()
        .is_none());
    }

    #[test]
    fn per_monitor_override() {
        let mut cfg = Config::default();
        cfg.wallpaper.path = Some("/default.mp4".into());
        cfg.monitors.insert(
            "DP-2".into(),
            Wallpaper {
                path: Some("/other.mp4".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            cfg.wallpaper_for("DP-2").path.as_deref().unwrap().to_str(),
            Some("/other.mp4")
        );
        assert_eq!(
            cfg.wallpaper_for("eDP-1").path.as_deref().unwrap().to_str(),
            Some("/default.mp4")
        );
    }

    #[test]
    fn widgets_absent_unless_configured() {
        // Every config.toml written before widgets existed must keep working
        // and must not acquire a widget layer by accident.
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.widgets, None);

        let legacy: Config = toml::from_str(
            "version = 1\nenabled = true\n\n[wallpaper]\nkind = \"video\"\npath = \"/a.mp4\"\n",
        )
        .unwrap();
        assert_eq!(legacy.widgets, None);
    }

    #[test]
    fn empty_widgets_table_leaves_lyrics_disabled() {
        // Product guarantee, not an implementation detail: asking for the
        // widgets block never by itself puts text on someone's desktop.
        let cfg: Config = toml::from_str("[widgets]\n").unwrap();
        let w = cfg.widgets.expect("[widgets] table must deserialize");
        assert!(!w.lyrics.enabled, "lyrics must default to OFF");
        assert_eq!(w, Widgets::default());
        assert_eq!(w.monitor, None, "None = primary output only");
        // The rest of the defaults come along even though the table was empty.
        assert_eq!(w.lyrics.style, LyricStylePreset::Minimal);
        assert_eq!(w.lyrics.anchor, LyricAnchor::BottomCenter);
        assert_eq!(w.lyrics.margin_px, 48);
        assert_eq!(w.lyrics.font_size_pt, 28);
        assert!(w.lyrics.accent_follow);
        assert!(!w.lyrics.show_next_line);
        assert!(
            !w.lyrics.show_track_info,
            "the now-playing readout is opt-in, not a second widget by default"
        );
        assert_eq!(w.lyrics.offset_ms, 0);
        assert_eq!(w.lyrics.folder, None);
    }

    #[test]
    fn widgets_roundtrip_through_toml() {
        let cfg = Config {
            widgets: Some(Widgets {
                lyrics: Lyrics {
                    enabled: true,
                    style: LyricStylePreset::Card,
                    anchor: LyricAnchor::TopRight,
                    margin_px: 96,
                    font_size_pt: 42,
                    accent_follow: false,
                    colour: Some("#FF00AA".into()),
                    show_next_line: true,
                    show_track_info: true,
                    // Negative offsets matter: a .lrc timed against a version
                    // with a longer intro needs the lines pulled earlier.
                    offset_ms: -350,
                    folder: Some("/home/u/Music/lyrics".into()),
                },
                clock: Clock {
                    enabled: true,
                    theme: ClockThemeCfg::Segment,
                    anchor: LyricAnchor::TopLeft,
                    font_size_pt: 96,
                    margin_px: 24,
                    show_seconds: true,
                    show_date: true,
                    use_24h: false,
                    accent_follow: false,
                },
                visualizer: Visualizer {
                    enabled: true,
                    style: VisualizerStyleCfg::Ring,
                    anchor: LyricAnchor::MidRight,
                    width_pct: 35,
                    height_px: 300,
                    margin_px: 12,
                    bands: 64,
                    accent_follow: false,
                    colour: "#FF0000".into(),
                    gradient: GradientMode::Spectrum,
                    colour_end: "#00FF00".into(),
                    opacity: 140,
                    rounded: false,
                },
                disc: Disc {
                    enabled: true,
                    anchor: LyricAnchor::BottomLeft,
                    size_px: 400,
                    margin_px: 8,
                    spin: false,
                    opacity: 90,
                },
                monitor: Some("DP-1".into()),
            }),
            ..Config::default()
        };
        let s = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back, cfg, "every widgets field must survive a round trip");
        assert_eq!(back.widgets, cfg.widgets);
    }

    #[test]
    fn fully_populated_lyrics_parses() {
        let cfg: Config = toml::from_str(
            r#"
[widgets]
monitor = "DP-1"

[widgets.lyrics]
enabled = true
style = "karaoke"
anchor = "midleft"
margin_px = 24
font_size_pt = 36
accent_follow = false
show_next_line = true
show_track_info = true
offset_ms = 250
folder = "/srv/lyrics"
"#,
        )
        .unwrap();
        let w = cfg.widgets.unwrap();
        assert_eq!(w.monitor.as_deref(), Some("DP-1"));
        assert!(w.lyrics.enabled);
        assert_eq!(w.lyrics.style, LyricStylePreset::Karaoke);
        assert_eq!(w.lyrics.anchor, LyricAnchor::MidLeft);
        assert_eq!(w.lyrics.margin_px, 24);
        assert_eq!(w.lyrics.font_size_pt, 36);
        assert!(!w.lyrics.accent_follow);
        assert!(w.lyrics.show_next_line);
        assert!(w.lyrics.show_track_info);
        assert_eq!(w.lyrics.offset_ms, 250);
        assert_eq!(
            w.lyrics.folder.as_deref(),
            Some(std::path::Path::new("/srv/lyrics"))
        );
    }

    #[test]
    fn lyric_enum_spellings_are_stable() {
        // These strings are the config file's public surface; renaming a
        // variant must not silently invalidate everyone's config.toml.
        for (text, want) in [
            ("minimal", LyricStylePreset::Minimal),
            ("karaoke", LyricStylePreset::Karaoke),
            ("subtitle", LyricStylePreset::Subtitle),
            ("card", LyricStylePreset::Card),
        ] {
            let cfg: Config =
                toml::from_str(&format!("[widgets.lyrics]\nstyle = \"{text}\"")).unwrap();
            assert_eq!(cfg.widgets.unwrap().lyrics.style, want, "style {text}");
        }
        // Nine-point grid: multi-word variants lowercase with no separator,
        // exactly as Transition::KenBurns serialises as "kenburns".
        for (text, want) in [
            ("topleft", LyricAnchor::TopLeft),
            ("topcenter", LyricAnchor::TopCenter),
            ("topright", LyricAnchor::TopRight),
            ("midleft", LyricAnchor::MidLeft),
            ("midcenter", LyricAnchor::MidCenter),
            ("midright", LyricAnchor::MidRight),
            ("bottomleft", LyricAnchor::BottomLeft),
            ("bottomcenter", LyricAnchor::BottomCenter),
            ("bottomright", LyricAnchor::BottomRight),
        ] {
            let cfg: Config =
                toml::from_str(&format!("[widgets.lyrics]\nanchor = \"{text}\"")).unwrap();
            assert_eq!(cfg.widgets.unwrap().lyrics.anchor, want, "anchor {text}");
            // ...and we write back what we read. Serialised from the lyric
            // block alone: [widgets.clock] carries an `anchor` key of its own,
            // so a whole-Widgets dump could satisfy this from the wrong table.
            let l = Lyrics {
                anchor: want,
                ..Lyrics::default()
            };
            assert!(
                toml::to_string(&l)
                    .unwrap()
                    .contains(&format!("anchor = \"{text}\"")),
                "anchor {text} must serialize back to the same spelling"
            );
        }
    }

    #[test]
    fn track_info_is_opt_in_and_survives_a_round_trip() {
        // A config written before this key existed — and every config written
        // by someone who does not want a now-playing block — must keep the
        // lyric line alone.
        let old: Config = toml::from_str("[widgets.lyrics]\nenabled = true\n").unwrap();
        assert!(!old.widgets.unwrap().lyrics.show_track_info);

        // ...and setting it is remembered, on its own, without dragging any
        // other lyric field off its default.
        let l = Lyrics {
            show_track_info: true,
            ..Lyrics::default()
        };
        let text = toml::to_string(&l).unwrap();
        assert!(text.contains("show_track_info = true"), "got:\n{text}");
        let back: Lyrics = toml::from_str(&text).unwrap();
        assert_eq!(back, l);
        assert!(!back.show_next_line, "an unrelated switch must stay off");
    }

    #[test]
    fn empty_widgets_table_leaves_clock_disabled() {
        // Same product guarantee as the lyric case: naming the block, or
        // configuring the *other* widget, never puts a clock on the desktop.
        let cfg: Config = toml::from_str("[widgets]\n").unwrap();
        let w = cfg.widgets.expect("[widgets] table must deserialize");
        assert!(!w.clock.enabled, "clock must default to OFF");
        assert_eq!(w.clock, Clock::default());

        let lyrics_only: Config = toml::from_str("[widgets.lyrics]\nenabled = true\n").unwrap();
        let w = lyrics_only.widgets.unwrap();
        assert!(w.lyrics.enabled);
        assert!(!w.clock.enabled, "absent [widgets.clock] must stay off");
        assert_eq!(w.clock, Clock::default(), "and take every other default");
    }

    #[test]
    fn clock_defaults_are_the_cheap_ones() {
        // The defaults are the power contract, so they are pinned by value and
        // not merely "whatever Default says".
        let c = Clock::default();
        assert!(!c.enabled);
        assert_eq!(c.theme, ClockThemeCfg::Digital);
        assert_eq!(c.anchor, LyricAnchor::TopRight, "clear of icons and docks");
        assert_eq!(c.font_size_pt, 64);
        assert_eq!(c.margin_px, 56);
        assert!(
            !c.show_seconds,
            "seconds redraw 60x more often; never a default"
        );
        assert!(!c.show_date);
        assert!(
            c.use_24h,
            "fixed-width HH:MM, and the rest of the config agrees"
        );
        assert!(c.accent_follow, "matches Lyrics::accent_follow");
        // A [widgets.clock] table with nothing in it must produce exactly that.
        let cfg: Config = toml::from_str("[widgets.clock]\n").unwrap();
        assert_eq!(cfg.widgets.unwrap().clock, c);
    }

    #[test]
    fn fully_populated_clock_parses() {
        let cfg: Config = toml::from_str(
            r#"
[widgets.clock]
enabled = true
theme = "stacked"
anchor = "bottomright"
font_size_pt = 120
margin_px = 12
show_seconds = true
show_date = true
use_24h = false
accent_follow = false
"#,
        )
        .unwrap();
        let c = cfg.widgets.unwrap().clock;
        assert!(c.enabled);
        assert_eq!(c.theme, ClockThemeCfg::Stacked);
        assert_eq!(c.anchor, LyricAnchor::BottomRight);
        assert_eq!(c.font_size_pt, 120);
        assert_eq!(c.margin_px, 12);
        assert!(c.show_seconds);
        assert!(c.show_date);
        assert!(!c.use_24h);
        assert!(!c.accent_follow);
    }

    #[test]
    fn clock_roundtrip_through_toml() {
        // Every field set away from its default, so a dropped #[serde] attribute
        // shows up as a mismatch rather than as a coincidence.
        let c = Clock {
            enabled: true,
            theme: ClockThemeCfg::Wordy,
            anchor: LyricAnchor::MidCenter,
            font_size_pt: 33,
            margin_px: 7,
            show_seconds: true,
            show_date: true,
            use_24h: false,
            accent_follow: false,
        };
        let cfg = Config {
            widgets: Some(Widgets {
                clock: c.clone(),
                ..Widgets::default()
            }),
            ..Config::default()
        };
        let s = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back, cfg, "every clock field must survive a round trip");
        assert_eq!(back.widgets.unwrap().clock, c);
    }

    #[test]
    fn clock_enum_spellings_are_stable() {
        // These strings are the config file's public surface; renaming a
        // variant must not silently invalidate everyone's config.toml.
        for (text, want) in [
            ("digital", ClockThemeCfg::Digital),
            ("minimal", ClockThemeCfg::Minimal),
            ("segment", ClockThemeCfg::Segment),
            ("stacked", ClockThemeCfg::Stacked),
            ("wordy", ClockThemeCfg::Wordy),
        ] {
            let cfg: Config =
                toml::from_str(&format!("[widgets.clock]\ntheme = \"{text}\"")).unwrap();
            assert_eq!(cfg.widgets.unwrap().clock.theme, want, "theme {text}");
            let c = Clock {
                theme: want,
                ..Clock::default()
            };
            assert!(
                toml::to_string(&c)
                    .unwrap()
                    .contains(&format!("theme = \"{text}\"")),
                "theme {text} must serialize back to the same spelling"
            );
        }
        // The clock shares the lyric anchor enum, so it must also share every
        // spelling — one placement vocabulary, not two that can drift apart.
        for (text, want) in [
            ("topleft", LyricAnchor::TopLeft),
            ("topcenter", LyricAnchor::TopCenter),
            ("topright", LyricAnchor::TopRight),
            ("midleft", LyricAnchor::MidLeft),
            ("midcenter", LyricAnchor::MidCenter),
            ("midright", LyricAnchor::MidRight),
            ("bottomleft", LyricAnchor::BottomLeft),
            ("bottomcenter", LyricAnchor::BottomCenter),
            ("bottomright", LyricAnchor::BottomRight),
        ] {
            let cfg: Config =
                toml::from_str(&format!("[widgets.clock]\nanchor = \"{text}\"")).unwrap();
            assert_eq!(cfg.widgets.unwrap().clock.anchor, want, "anchor {text}");
            let c = Clock {
                anchor: want,
                ..Clock::default()
            };
            assert!(
                toml::to_string(&c)
                    .unwrap()
                    .contains(&format!("anchor = \"{text}\"")),
                "anchor {text} must serialize back to the same spelling"
            );
        }
    }

    #[test]
    fn empty_widgets_table_leaves_visualizer_and_disc_disabled() {
        // The same product guarantee the lyric and clock cases make, and for
        // the visualiser it is stronger than cosmetic: `enabled = false` is
        // what keeps Fresco from ever opening an audio capture stream.
        let cfg: Config = toml::from_str("[widgets]\n").unwrap();
        let w = cfg.widgets.expect("[widgets] table must deserialize");
        assert!(
            !w.visualizer.enabled,
            "the visualiser listens to system audio; it must default to OFF"
        );
        assert!(!w.disc.enabled, "album art must default to OFF");
        assert_eq!(w, Widgets::default());

        // Configuring one widget must not switch on another.
        let others: Config =
            toml::from_str("[widgets.lyrics]\nenabled = true\n[widgets.clock]\nenabled = true\n")
                .unwrap();
        let w = others.widgets.unwrap();
        assert!(w.lyrics.enabled && w.clock.enabled);
        assert!(!w.visualizer.enabled, "absent table must stay off");
        assert!(!w.disc.enabled, "absent table must stay off");
        assert_eq!(w.visualizer, Visualizer::default());
        assert_eq!(w.disc, Disc::default());
    }

    #[test]
    fn visualizer_defaults_are_pinned() {
        // Pinned by value, not by "whatever Default says": `enabled` is a
        // privacy promise and the rest is the shape people first see.
        let v = Visualizer::default();
        assert!(!v.enabled, "no audio capture without an explicit opt-in");
        assert_eq!(v.style, VisualizerStyleCfg::Bars);
        assert_eq!(v.anchor, LyricAnchor::BottomCenter, "a spectrum is a floor");
        assert_eq!(v.width_pct, 60);
        assert_eq!(v.height_px, 120);
        assert_eq!(v.margin_px, 48);
        assert_eq!(v.bands, 32);
        assert!(v.accent_follow, "matches Lyrics and Clock");
        assert_eq!(v.colour, "#FFFFFF", "white is what the renderer draws");
        assert_eq!(v.gradient, GradientMode::None, "a ramp is asked for");
        assert_eq!(v.colour_end, "#FFFFFF", "equal ends, i.e. no ramp at all");
        assert_eq!(v.opacity, 220, "motion at full strength pulls the eye");
        assert!(v.rounded);
        // An empty [widgets.visualizer] table must produce exactly that.
        let cfg: Config = toml::from_str("[widgets.visualizer]\n").unwrap();
        assert_eq!(cfg.widgets.unwrap().visualizer, v);
    }

    #[test]
    fn disc_defaults_are_pinned() {
        let d = Disc::default();
        assert!(!d.enabled);
        assert_eq!(
            d.anchor,
            LyricAnchor::BottomRight,
            "the one corner no other widget defaults to"
        );
        assert_eq!(d.size_px, 220);
        assert_eq!(d.margin_px, 48);
        assert!(d.spin, "a record that does not turn is a circle");
        assert_eq!(d.opacity, 255, "a faded cover reads as a rendering fault");
        let cfg: Config = toml::from_str("[widgets.disc]\n").unwrap();
        assert_eq!(cfg.widgets.unwrap().disc, d);

        // Every widget's default anchor is distinct, so switching all four on
        // without touching anything else cannot stack two in one corner.
        let w = Widgets::default();
        let corners = [w.clock.anchor, w.disc.anchor];
        assert_ne!(corners[0], corners[1]);
        assert_ne!(w.disc.anchor, w.lyrics.anchor);
        assert_ne!(w.disc.anchor, w.visualizer.anchor);
    }

    #[test]
    fn fully_populated_visualizer_and_disc_parse() {
        let cfg: Config = toml::from_str(
            r#"
[widgets.visualizer]
enabled = true
style = "mirror"
anchor = "topcenter"
width_pct = 80
height_px = 200
margin_px = 16
bands = 96
accent_follow = false
opacity = 128
rounded = false

[widgets.disc]
enabled = true
anchor = "midleft"
size_px = 512
margin_px = 4
spin = false
opacity = 200
"#,
        )
        .unwrap();
        let w = cfg.widgets.unwrap();
        let v = w.visualizer;
        assert!(v.enabled);
        assert_eq!(v.style, VisualizerStyleCfg::Mirror);
        assert_eq!(v.anchor, LyricAnchor::TopCenter);
        assert_eq!(v.width_pct, 80);
        assert_eq!(v.height_px, 200);
        assert_eq!(v.margin_px, 16);
        assert_eq!(v.bands, 96);
        assert!(!v.accent_follow);
        assert_eq!(v.opacity, 128);
        assert!(!v.rounded);

        let d = w.disc;
        assert!(d.enabled);
        assert_eq!(d.anchor, LyricAnchor::MidLeft);
        assert_eq!(d.size_px, 512);
        assert_eq!(d.margin_px, 4);
        assert!(!d.spin);
        assert_eq!(d.opacity, 200);
    }

    #[test]
    fn visualizer_and_disc_roundtrip_through_toml() {
        // Every field set away from its default, so a dropped #[serde]
        // attribute shows up as a mismatch rather than as a coincidence.
        let v = Visualizer {
            enabled: true,
            style: VisualizerStyleCfg::Dots,
            anchor: LyricAnchor::TopLeft,
            width_pct: 25,
            height_px: 66,
            margin_px: 3,
            bands: 7,
            accent_follow: false,
            colour: "#123456".into(),
            gradient: GradientMode::Linear,
            colour_end: "#ABCDEF".into(),
            opacity: 11,
            rounded: false,
        };
        let d = Disc {
            enabled: true,
            anchor: LyricAnchor::MidCenter,
            size_px: 999,
            margin_px: 1,
            spin: false,
            opacity: 2,
        };
        let cfg = Config {
            widgets: Some(Widgets {
                visualizer: v.clone(),
                disc: d.clone(),
                ..Widgets::default()
            }),
            ..Config::default()
        };
        let s = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back, cfg, "every field must survive a round trip");
        let w = back.widgets.unwrap();
        assert_eq!(w.visualizer, v);
        assert_eq!(w.disc, d);
    }

    #[test]
    fn visualizer_enum_spellings_are_stable() {
        // These strings are the config file's public surface; renaming a
        // variant must not silently invalidate everyone's config.toml.
        for (text, want) in [
            ("bars", VisualizerStyleCfg::Bars),
            ("mirror", VisualizerStyleCfg::Mirror),
            ("wave", VisualizerStyleCfg::Wave),
            ("dots", VisualizerStyleCfg::Dots),
            ("ring", VisualizerStyleCfg::Ring),
        ] {
            let cfg: Config =
                toml::from_str(&format!("[widgets.visualizer]\nstyle = \"{text}\"")).unwrap();
            assert_eq!(cfg.widgets.unwrap().visualizer.style, want, "style {text}");
            let v = Visualizer {
                style: want,
                ..Visualizer::default()
            };
            assert!(
                toml::to_string(&v)
                    .unwrap()
                    .contains(&format!("style = \"{text}\"")),
                "style {text} must serialize back to the same spelling"
            );
        }
    }

    #[test]
    fn gradient_spellings_are_stable() {
        // As with every other enum here: these strings are the config file's
        // public surface, and a rename would invalidate configs in the field.
        for (text, want) in [
            ("none", GradientMode::None),
            ("linear", GradientMode::Linear),
            ("spectrum", GradientMode::Spectrum),
        ] {
            let cfg: Config =
                toml::from_str(&format!("[widgets.visualizer]\ngradient = \"{text}\"")).unwrap();
            assert_eq!(
                cfg.widgets.unwrap().visualizer.gradient,
                want,
                "gradient {text}"
            );
            let v = Visualizer {
                gradient: want,
                ..Visualizer::default()
            };
            assert!(
                toml::to_string(&v)
                    .unwrap()
                    .contains(&format!("gradient = \"{text}\"")),
                "gradient {text} must serialize back to the same spelling"
            );
        }
        assert_eq!(GradientMode::default(), GradientMode::None);
    }

    #[test]
    fn widget_colours_are_normalised_and_bad_ones_fall_back() {
        // config.toml is hand-editable and the value ends up inside an ASS
        // payload, where a malformed colour costs the whole overlay and not
        // just the tint. So it is caught here, at the edge, once.
        for (raw, want) in [
            ("#3584e4", "#3584E4"),
            ("3584E4", "#3584E4"),
            ("#abc", "#AABBCC"),
            ("  #FFF  ", "#FFFFFF"),
            // Everything unusable becomes plain white rather than a parse error
            // that would cost the user every other setting in the file.
            ("", "#FFFFFF"),
            ("#12345", "#FFFFFF"),
            ("rgb(1,2,3)", "#FFFFFF"),
            ("#GGGGGG", "#FFFFFF"),
            ("cornflowerblue", "#FFFFFF"),
        ] {
            let cfg: Config = toml::from_str(&format!(
                "[widgets.visualizer]\ncolour = \"{raw}\"\ncolour_end = \"{raw}\""
            ))
            .expect("a bad colour must never fail the whole parse");
            let v = cfg.widgets.expect("widgets").visualizer;
            assert_eq!(v.colour, want, "colour {raw:?}");
            assert_eq!(v.colour_end, want, "colour_end {raw:?}");
        }
    }

    #[test]
    fn a_lyric_colour_is_absent_until_it_is_chosen() {
        // Absent means "the preset picks", which is what every existing config
        // already gets: adding this key must not repaint anybody's lyrics.
        assert_eq!(Lyrics::default().colour, None);
        let cfg: Config = toml::from_str("[widgets.lyrics]\n").unwrap();
        assert_eq!(cfg.widgets.unwrap().lyrics.colour, None);
        // Unusable input lands in the same place, for the same reason: a typo
        // should leave the preset alone rather than force the lyric white.
        for raw in ["", "#12345", "not a colour"] {
            let cfg: Config =
                toml::from_str(&format!("[widgets.lyrics]\ncolour = \"{raw}\"")).unwrap();
            assert_eq!(cfg.widgets.unwrap().lyrics.colour, None, "colour {raw:?}");
        }
        let cfg: Config = toml::from_str("[widgets.lyrics]\ncolour = \"#ff8800\"").unwrap();
        assert_eq!(
            cfg.widgets.unwrap().lyrics.colour.as_deref(),
            Some("#FF8800")
        );
        // And an unset colour writes no key at all, so a config file stays as
        // small as the choices actually made in it.
        let text = toml::to_string(&Lyrics::default()).unwrap();
        assert!(!text.contains("colour"), "{text}");
    }

    #[test]
    fn audio_capture_needs_consent_before_the_visualiser_can_run() {
        // The one feature in Fresco that opens a capture stream on everything
        // the machine plays. Consent-first, exactly like telemetry, and
        // enforced where every process reads its config rather than only in the
        // dialog that asks the question.
        assert!(
            !Config::default().audio_capture_consented,
            "nobody has agreed to anything yet"
        );

        // Same throwaway-directory idiom as `save_load_file`, so this test
        // needs no dependency the crate does not already have.
        let dir = std::env::temp_dir().join(format!("fresco-consent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        // A hand-edited config that switches the visualiser on without the
        // flag must come back off. Otherwise the dialog is decoration.
        std::fs::write(
            &path,
            "[widgets.visualizer]\nenabled = true\nstyle = \"ring\"\n",
        )
        .unwrap();
        let cfg = Config::load_from(&path).unwrap();
        let v = cfg.widgets.expect("widgets").visualizer;
        assert!(!v.enabled, "capture started without consent");
        assert_eq!(v.style, VisualizerStyleCfg::Ring, "only `enabled` is reset");

        // With the flag set it stays on, and the flag itself round-trips.
        std::fs::write(
            &path,
            "audio_capture_consented = true\n\n[widgets.visualizer]\nenabled = true\n",
        )
        .unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert!(cfg.audio_capture_consented);
        assert!(cfg.widgets.as_ref().unwrap().visualizer.enabled);

        // Consent persists across a save/load cycle: it is asked once.
        let saved = dir.join("saved.toml");
        cfg.save_to(&saved).unwrap();
        let back = Config::load_from(&saved).unwrap();
        assert!(back.audio_capture_consented);
        assert!(back.widgets.as_ref().unwrap().visualizer.enabled);
        assert_eq!(back.widgets, cfg.widgets);

        // Withdrawing consent by hand switches the widget off again on the next
        // load, so the file remains the source of truth.
        let text = std::fs::read_to_string(&saved).unwrap().replace(
            "audio_capture_consented = true",
            "audio_capture_consented = false",
        );
        std::fs::write(&saved, text).unwrap();
        let back = Config::load_from(&saved).unwrap();
        assert!(!back.widgets.unwrap().visualizer.enabled);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn visualizer_and_disc_share_the_anchor_spellings() {
        // Four widgets, one placement vocabulary. Serialised from each widget's
        // own struct, not from a whole-Widgets dump: three other tables carry an
        // `anchor` key, so a dump could satisfy the assertion from the wrong one.
        for (text, want) in [
            ("topleft", LyricAnchor::TopLeft),
            ("topcenter", LyricAnchor::TopCenter),
            ("topright", LyricAnchor::TopRight),
            ("midleft", LyricAnchor::MidLeft),
            ("midcenter", LyricAnchor::MidCenter),
            ("midright", LyricAnchor::MidRight),
            ("bottomleft", LyricAnchor::BottomLeft),
            ("bottomcenter", LyricAnchor::BottomCenter),
            ("bottomright", LyricAnchor::BottomRight),
        ] {
            let cfg: Config =
                toml::from_str(&format!("[widgets.visualizer]\nanchor = \"{text}\"")).unwrap();
            assert_eq!(cfg.widgets.unwrap().visualizer.anchor, want, "vis {text}");
            let v = Visualizer {
                anchor: want,
                ..Visualizer::default()
            };
            assert!(
                toml::to_string(&v)
                    .unwrap()
                    .contains(&format!("anchor = \"{text}\"")),
                "visualizer anchor {text} must serialize back the same"
            );

            let cfg: Config =
                toml::from_str(&format!("[widgets.disc]\nanchor = \"{text}\"")).unwrap();
            assert_eq!(cfg.widgets.unwrap().disc.anchor, want, "disc {text}");
            let d = Disc {
                anchor: want,
                ..Disc::default()
            };
            assert!(
                toml::to_string(&d)
                    .unwrap()
                    .contains(&format!("anchor = \"{text}\"")),
                "disc anchor {text} must serialize back the same"
            );
        }
    }

    #[test]
    fn no_widgets_key_when_unset() {
        // What skip_serializing_if buys: a user who never touches widgets
        // never gets the key written into their config.toml.
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        assert!(
            !text.contains("widgets"),
            "widgets: None must emit no key, got:\n{text}"
        );
        // ...and it does appear once configured.
        let cfg = Config {
            widgets: Some(Widgets::default()),
            ..Config::default()
        };
        let text = toml::to_string_pretty(&cfg).unwrap();
        assert!(text.contains("[widgets"), "got:\n{text}");
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), cfg);
    }
}

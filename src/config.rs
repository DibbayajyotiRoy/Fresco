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
    /// Light/dark preference (System follows the desktop).
    #[serde(default)]
    pub theme_mode: ThemeMode,
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
    /// Anonymous usage telemetry (daily ping, feature counts, error kinds).
    /// Opt-out via the Settings switch or config.toml.
    #[serde(default = "default_true")]
    pub telemetry: bool,
    /// Whether the one-time telemetry consent dialog was answered. Nothing is
    /// ever sent before this is true — consent-first, like a cookie banner
    /// but honest (no dark patterns, both buttons equal weight).
    #[serde(default)]
    pub telemetry_prompted: bool,
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
}

fn default_version() -> u32 {
    1
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            autostart: true,
            enabled: true,
            pause_on_battery: false,
            scaling: Scaling::default(),
            theme_mode: ThemeMode::default(),
            accent: Accent::default(),
            wallpaper: Wallpaper::default(),
            last_seen_version: String::new(),
            first_run_epoch: 0,
            feedback_prompted: false,
            feedback_reminders: true,
            telemetry: true,
            telemetry_prompted: false,
            browser_bridge: false,
            browser_wallpaper: None,
            apply_count: 0,
            last_star_nudge: 0,
            tour_shown: false,
            seen_notifications: Vec::new(),
            monitors: BTreeMap::new(),
            last_update_check: 0,
            update_skipped_version: String::new(),
            schedule: None,
            schedule_paused: false,
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
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
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
}

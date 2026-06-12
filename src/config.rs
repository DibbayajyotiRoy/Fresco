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

/// Normalized crop rectangle (all values in 0.0..=1.0, relative to source).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Crop {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slideshow {
    pub folder: PathBuf,
    #[serde(default = "default_interval")]
    pub interval_s: u64,
}

fn default_interval() -> u64 {
    600
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    #[serde(default = "default_true")]
    pub mute: bool,
    #[serde(default = "default_volume")]
    pub volume: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slideshow: Option<Slideshow>,
}

impl Default for Wallpaper {
    fn default() -> Self {
        Wallpaper {
            kind: Kind::default(),
            path: None,
            paths: Vec::new(),
            shuffle: false,
            fit: Fit::default(),
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
    #[serde(default)]
    pub wallpaper: Wallpaper,
    /// Per-monitor overrides keyed by RandR connector name (e.g. "HDMI-1").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub monitors: BTreeMap<String, Wallpaper>,
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
            wallpaper: Wallpaper::default(),
            monitors: BTreeMap::new(),
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
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
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
        cfg.wallpaper.crop = Some(Crop { x: 0.1, y: 0.2, w: 0.5, h: 0.5 });
        cfg.pause_on_battery = true;
        cfg.monitors.insert(
            "HDMI-1".into(),
            Wallpaper { kind: Kind::Image, path: Some("/p.png".into()), ..Default::default() },
        );
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn save_load_file() {
        let dir = std::env::temp_dir().join(format!("fresco-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let mut cfg = Config::default();
        cfg.enabled = false;
        cfg.save_to(&path).unwrap();
        let back = Config::load_from(&path).unwrap();
        assert_eq!(cfg, back);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn crop_sanitize() {
        // Out-of-bounds rect gets clamped.
        let c = Crop { x: 0.9, y: -0.5, w: 0.5, h: 0.5 }.sanitized().unwrap();
        assert!((c.x + c.w) <= 1.0 + f64::EPSILON);
        assert!(c.y >= 0.0);
        // Full-frame crop collapses to None.
        assert!(Crop { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }.sanitized().is_none());
    }

    #[test]
    fn per_monitor_override() {
        let mut cfg = Config::default();
        cfg.wallpaper.path = Some("/default.mp4".into());
        cfg.monitors.insert(
            "DP-2".into(),
            Wallpaper { path: Some("/other.mp4".into()), ..Default::default() },
        );
        assert_eq!(cfg.wallpaper_for("DP-2").path.as_deref().unwrap().to_str(), Some("/other.mp4"));
        assert_eq!(
            cfg.wallpaper_for("eDP-1").path.as_deref().unwrap().to_str(),
            Some("/default.mp4")
        );
    }
}

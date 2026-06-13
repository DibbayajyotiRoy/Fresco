use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{Fit, Kind, Slideshow, Wallpaper};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    /// Primary path (single video/image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Playlist items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<PathBuf>,
    /// Slideshow source folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
    /// Cached thumbnail path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PathBuf>,
    /// Unix timestamp of last activation.
    #[serde(default)]
    pub last_used: u64,
    /// True when the source was missing on last health check.
    #[serde(default)]
    pub broken: bool,
    /// Load-failure message from the daemon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl LibraryEntry {
    pub fn new_video(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            id: make_id(),
            name,
            kind: Kind::Video,
            path: Some(path),
            paths: vec![],
            folder: None,
            thumbnail: None,
            last_used: 0,
            broken: false,
            error: None,
        }
    }

    pub fn new_image(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            id: make_id(),
            name,
            kind: Kind::Image,
            path: Some(path),
            paths: vec![],
            folder: None,
            thumbnail: None,
            last_used: 0,
            broken: false,
            error: None,
        }
    }

    pub fn new_playlist(paths: Vec<PathBuf>) -> Self {
        let name = paths
            .first()
            .and_then(|p| p.file_stem())
            .map(|s| {
                format!(
                    "{} (+{})",
                    s.to_string_lossy(),
                    paths.len().saturating_sub(1)
                )
            })
            .unwrap_or_else(|| "Playlist".to_string());
        Self {
            id: make_id(),
            name,
            kind: Kind::Playlist,
            path: None,
            paths,
            folder: None,
            thumbnail: None,
            last_used: 0,
            broken: false,
            error: None,
        }
    }

    pub fn new_slideshow(folder: PathBuf) -> Self {
        let name = folder
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Slideshow".to_string());
        Self {
            id: make_id(),
            name,
            kind: Kind::Slideshow,
            path: None,
            paths: vec![],
            folder: Some(folder),
            thumbnail: None,
            last_used: 0,
            broken: false,
            error: None,
        }
    }

    pub fn check_health(&mut self) {
        self.broken = match self.kind {
            Kind::Video | Kind::Image => self.path.as_ref().is_none_or(|p| !p.exists()),
            Kind::Playlist => self.paths.is_empty() || !self.paths.iter().any(|p| p.exists()),
            Kind::Slideshow => self.folder.as_ref().is_none_or(|f| !f.exists()),
        };
    }

    pub fn touch(&mut self) {
        self.last_used = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }

    pub fn expected_thumbnail(&self) -> PathBuf {
        library_dir()
            .join("thumbs")
            .join(format!("{}.png", self.id))
    }

    /// Generate thumbnail via ffmpegthumbnailer (silently skips if not available).
    pub fn generate_thumbnail(&mut self) {
        let source = match self.kind {
            Kind::Video | Kind::Playlist => {
                self.path.clone().or_else(|| self.paths.first().cloned())
            }
            Kind::Image => self.path.clone(),
            Kind::Slideshow => self.folder.as_ref().and_then(|f| {
                fs::read_dir(f).ok()?.flatten().find_map(|e| {
                    let p = e.path();
                    is_image(&p).then_some(p)
                })
            }),
        };
        let Some(src) = source else { return };
        if !src.exists() {
            return;
        }
        let out = self.expected_thumbnail();
        if let Some(dir) = out.parent() {
            fs::create_dir_all(dir).ok();
        }
        let ok = std::process::Command::new("ffmpegthumbnailer")
            .args([
                "-i",
                &src.to_string_lossy(),
                "-o",
                &out.to_string_lossy(),
                "-s",
                "256",
                "-q",
                "8",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            self.thumbnail = Some(out);
        }
    }

    pub fn to_wallpaper(&self) -> Wallpaper {
        Wallpaper {
            kind: self.kind,
            path: self.path.clone(),
            paths: self.paths.clone(),
            shuffle: false,
            fit: Fit::Cover,
            crop: None,
            mute: true,
            volume: 50,
            slideshow: self.folder.as_ref().map(|f| Slideshow {
                folder: f.clone(),
                interval_s: 600,
            }),
        }
    }
}

// ─── Library store ────────────────────────────────────────────────────────────

pub fn library_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fresco")
        .join("library")
}

fn entries_path() -> PathBuf {
    library_dir().join("entries.json")
}

fn make_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn load_entries() -> Result<Vec<LibraryEntry>> {
    let path = entries_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let entries: Vec<LibraryEntry> = serde_json::from_str(&text)?;
    Ok(entries)
}

pub fn save_entries(entries: &[LibraryEntry]) -> Result<()> {
    let dir = library_dir();
    fs::create_dir_all(&dir)?;
    let path = entries_path();
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(entries)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Up to `limit` entries sorted by most recently used.
pub fn recent_entries(entries: &[LibraryEntry], limit: usize) -> Vec<&LibraryEntry> {
    let mut sorted: Vec<&LibraryEntry> = entries.iter().filter(|e| e.last_used > 0).collect();
    sorted.sort_by_key(|e| std::cmp::Reverse(e.last_used));
    sorted.truncate(limit);
    sorted
}

pub fn is_video(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .as_deref(),
        Some("mp4" | "webm" | "mkv" | "avi" | "mov" | "flv" | "gif")
    )
}

pub fn is_image(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "bmp" | "tiff")
    )
}

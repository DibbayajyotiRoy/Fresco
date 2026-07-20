use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{Fit, Kind, Slideshow, Transition, Wallpaper};

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
    /// Slideshow cycle interval in seconds; only meaningful for Kind::Slideshow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_s: Option<u64>,
    /// Slideshow transition effect; only meaningful for Kind::Slideshow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<Transition>,
    /// Remembered audio + orientation (video/playlist), so setting from the
    /// gallery keeps what you chose in the editor. None = sensible default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<u16>,
    /// Catalog item this entry was installed from (ROADMAP 3.1), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<String>,
    /// Probed media metadata (ffprobe; see `probe_media`). All optional so
    /// pre-1.2 entries.json files load unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// User-starred entry: shown in the Favorites section first.
    #[serde(default)]
    pub favorite: bool,
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
            interval_s: None,
            transition: None,
            mute: None,
            volume: None,
            rotation: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
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
            interval_s: None,
            transition: None,
            mute: None,
            volume: None,
            rotation: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
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
            interval_s: None,
            transition: None,
            mute: None,
            volume: None,
            rotation: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
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
            interval_s: Some(30),
            transition: Some(Transition::Crossfade),
            mute: None,
            volume: None,
            rotation: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
        }
    }

    /// A slideshow built from hand-picked image files (no folder).
    pub fn new_image_set(paths: Vec<PathBuf>) -> Self {
        let name = format!("Slideshow ({} images)", paths.len());
        Self {
            id: make_id(),
            name,
            kind: Kind::Slideshow,
            path: None,
            paths,
            folder: None,
            thumbnail: None,
            last_used: 0,
            broken: false,
            error: None,
            interval_s: Some(30),
            transition: Some(Transition::Crossfade),
            mute: None,
            volume: None,
            rotation: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
        }
    }

    pub fn check_health(&mut self) {
        self.broken = match self.kind {
            Kind::Video | Kind::Image => self.path.as_ref().is_none_or(|p| !p.exists()),
            Kind::Playlist => self.paths.is_empty() || !self.paths.iter().any(|p| p.exists()),
            Kind::Slideshow => {
                if !self.paths.is_empty() {
                    !self.paths.iter().any(|p| p.exists())
                } else {
                    self.folder.as_ref().is_none_or(|f| !f.exists())
                }
            }
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
            Kind::Slideshow => self.paths.first().cloned().or_else(|| {
                self.folder.as_ref().and_then(|f| {
                    fs::read_dir(f).ok()?.flatten().find_map(|e| {
                        let p = e.path();
                        is_image(&p).then_some(p)
                    })
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
        // The thumbnail must show the entry's ROTATION, or the card keeps the
        // old orientation after an edit. ffmpegthumbnailer can't rotate, so
        // rotated entries go through ffmpeg (fall through to the unrotated
        // thumbnailer if that fails).
        let rotation = self.rotation.unwrap_or(0) % 360;
        if rotation != 0 {
            let transpose = match rotation {
                90 => "transpose=1",
                180 => "transpose=1,transpose=1",
                270 => "transpose=2",
                _ => "null",
            };
            let ok = std::process::Command::new("ffmpeg")
                // -nostdin + null stdio: see overview.rs — a terminal-launched
                // app must never let ffmpeg read the TTY (SIGTTIN stops us).
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .args([
                    "-nostdin",
                    "-y",
                    "-loglevel",
                    "error",
                    "-i",
                    &src.to_string_lossy(),
                    "-frames:v",
                    "1",
                    "-vf",
                    &format!("{transpose},scale=256:-2"),
                    &out.to_string_lossy(),
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                self.thumbnail = Some(out);
                return;
            }
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

    /// The single media file that best represents this entry (used for
    /// metadata probing; mirrors the thumbnail source choice).
    pub fn probe_source(&self) -> Option<PathBuf> {
        match self.kind {
            Kind::Video | Kind::Image => self.path.clone(),
            Kind::Playlist | Kind::Slideshow => {
                self.paths.first().cloned().or_else(|| self.path.clone())
            }
        }
    }

    /// True when this entry still needs a metadata probe. `size_bytes` doubles
    /// as the "probed" marker: it is always fillable from the filesystem, so a
    /// probed entry keeps it even when ffprobe is absent — the batch prober
    /// never re-probes the same files at every launch.
    pub fn needs_probe(&self) -> bool {
        self.size_bytes.is_none() && !self.broken && self.probe_source().is_some_and(|p| p.exists())
    }

    /// Whether the source resolution qualifies for the "4K" badge (≥2160p).
    pub fn is_4k(&self) -> bool {
        self.width.unwrap_or(0) >= 3840 || self.height.unwrap_or(0) >= 2160
    }

    /// The dim second scrim line: "4K · 60fps · 32 MB". Parts are omitted when
    /// unknown; images/slideshows never show fps. `None` when nothing is known.
    pub fn meta_line(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let (Some(w), Some(h)) = (self.width, self.height) {
            parts.push(res_label(w, h));
        }
        if matches!(self.kind, Kind::Video | Kind::Playlist) {
            if let Some(fps) = self.fps.filter(|f| *f > 0.0) {
                parts.push(format!("{}fps", fps.round() as u32));
            }
        }
        if let Some(b) = self.size_bytes.filter(|b| *b > 0) {
            parts.push(human_size(b));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    pub fn to_wallpaper(&self) -> Wallpaper {
        Wallpaper {
            kind: self.kind,
            path: self.path.clone(),
            paths: self.paths.clone(),
            shuffle: false,
            fit: Fit::Cover,
            rotation: self.rotation.unwrap_or(0),
            crop: None,
            mute: self.mute.unwrap_or(true),
            volume: self.volume.unwrap_or(50),
            slideshow: if self.kind == Kind::Slideshow {
                Some(Slideshow {
                    folder: self.folder.clone(),
                    paths: self.paths.clone(),
                    interval_s: self.interval_s.unwrap_or(30),
                    transition: self.transition.unwrap_or_default(),
                })
            } else {
                None
            },
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

// ─── Media metadata ───────────────────────────────────────────────────────────

/// Probed media facts. Everything optional: an absent ffprobe just means no
/// resolution/fps; size still comes from the filesystem.
#[derive(Debug, Clone, Copy, Default)]
pub struct MediaMeta {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f32>,
    pub size_bytes: Option<u64>,
}

/// Probe one media file. Size comes from `fs::metadata`; resolution + fps come
/// from `ffprobe` when it is installed (it ships with the recommended ffmpeg
/// dependency). ffprobe being missing or failing is never an error — the
/// affected fields just stay `None`.
pub fn probe_media(path: &Path) -> MediaMeta {
    let mut meta = MediaMeta {
        size_bytes: fs::metadata(path).ok().map(|m| m.len()),
        ..MediaMeta::default()
    };
    let output = std::process::Command::new("ffprobe")
        .stdin(std::process::Stdio::null())
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            &path.to_string_lossy(),
        ])
        .output();
    let Ok(out) = output else { return meta };
    if !out.status.success() {
        return meta;
    }
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return meta;
    };
    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        if let Some(v) = streams
            .iter()
            .find(|s| s.get("codec_type").and_then(|t| t.as_str()) == Some("video"))
        {
            meta.width = v.get("width").and_then(|w| w.as_u64()).map(|w| w as u32);
            meta.height = v.get("height").and_then(|h| h.as_u64()).map(|h| h as u32);
            meta.fps = v
                .get("avg_frame_rate")
                .or_else(|| v.get("r_frame_rate"))
                .and_then(|r| r.as_str())
                .and_then(parse_frame_rate);
        }
    }
    meta
}

/// Parse ffprobe's "num/den" frame-rate fraction ("60/1", "30000/1001", "0/0").
fn parse_frame_rate(s: &str) -> Option<f32> {
    let (num, den) = s.split_once('/')?;
    let (num, den) = (
        num.trim().parse::<f32>().ok()?,
        den.trim().parse::<f32>().ok()?,
    );
    if den == 0.0 || num <= 0.0 {
        return None;
    }
    Some(num / den)
}

/// Friendly resolution label: 3840-wide → "4K", 2560 → "1440p", 1920 → "1080p",
/// anything else → "WxH".
pub fn res_label(w: u32, h: u32) -> String {
    if w >= 3840 {
        "4K".to_string()
    } else if w >= 2560 {
        "1440p".to_string()
    } else if w >= 1920 {
        "1080p".to_string()
    } else {
        format!("{w}x{h}")
    }
}

/// Humanized byte size ("824 KB", "32 MB", "1.5 GB").
pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A pre-1.2 entries.json entry (no metadata/favorite fields) must load,
    /// with defaults filled in.
    #[test]
    fn old_entries_json_still_loads() {
        let json = r#"[{
            "id": "123-0",
            "name": "CAR",
            "kind": "video",
            "path": "/videos/car.mp4",
            "last_used": 5
        }]"#;
        let entries: Vec<LibraryEntry> = serde_json::from_str(json).expect("old entry loads");
        let e = &entries[0];
        assert_eq!(e.name, "CAR");
        assert_eq!(e.width, None);
        assert_eq!(e.height, None);
        assert_eq!(e.fps, None);
        assert_eq!(e.size_bytes, None);
        assert!(!e.favorite);
    }

    #[test]
    fn metadata_and_favorite_round_trip() {
        let mut e = LibraryEntry::new_video(PathBuf::from("/videos/car.mp4"));
        e.width = Some(3840);
        e.height = Some(2160);
        e.fps = Some(60.0);
        e.size_bytes = Some(32 * 1024 * 1024);
        e.favorite = true;
        let json = serde_json::to_string(&e).unwrap();
        let back: LibraryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.width, Some(3840));
        assert_eq!(back.height, Some(2160));
        assert_eq!(back.fps, Some(60.0));
        assert_eq!(back.size_bytes, Some(32 * 1024 * 1024));
        assert!(back.favorite);
        assert!(back.is_4k());
        assert_eq!(back.meta_line().as_deref(), Some("4K · 60fps · 32 MB"));
    }

    #[test]
    fn unprobed_serializes_without_metadata_keys() {
        let e = LibraryEntry::new_image(PathBuf::from("/pics/a.png"));
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("width"));
        assert!(!json.contains("size_bytes"));
        assert!(json.contains("\"favorite\":false"));
    }

    #[test]
    fn image_meta_line_has_no_fps() {
        let mut e = LibraryEntry::new_image(PathBuf::from("/pics/a.png"));
        e.width = Some(2560);
        e.height = Some(1440);
        e.fps = Some(25.0); // ffprobe reports one for stills; must be ignored
        e.size_bytes = Some(900 * 1024);
        assert_eq!(e.meta_line().as_deref(), Some("1440p · 900 KB"));
    }

    #[test]
    fn res_and_size_labels() {
        assert_eq!(res_label(3840, 2160), "4K");
        assert_eq!(res_label(2560, 1440), "1440p");
        assert_eq!(res_label(1920, 1080), "1080p");
        assert_eq!(res_label(1280, 720), "1280x720");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(900 * 1024), "900 KB");
        assert_eq!(human_size(32 * 1024 * 1024), "32 MB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    }

    #[test]
    fn frame_rate_fraction_parses() {
        assert_eq!(parse_frame_rate("60/1"), Some(60.0));
        assert!((parse_frame_rate("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert_eq!(parse_frame_rate("0/0"), None);
        assert_eq!(parse_frame_rate("garbage"), None);
    }
}

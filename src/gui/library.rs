use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{Fit, Kind, PowerSaving, Slideshow, Transition, Wallpaper};
use crate::{t, tf};

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
    /// Per-wallpaper power-saving override; `None` inherits the global default.
    /// Remembered so a later gallery set keeps the chosen level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_saving: Option<PowerSaving>,
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
    /// The [`Collection`] this entry belongs to, by id; `None` = uncategorized.
    ///
    /// **Single membership on purpose.** The request that drove this asked for
    /// "folders" (SCI-FI, Nature, Space…), and a folder is somewhere a thing
    /// *is*, not a label it *has*. One id keeps every derived question cheap
    /// and unambiguous: which section a card renders in, what "move up" means,
    /// and what happens to an entry when its collection is deleted. Multi
    /// membership would need per-collection ordering (an entry cannot have one
    /// `order` in two lists) and a UI for resolving "which folder am I looking
    /// at this card in" — a tagging model, which is a different feature. If it
    /// is ever wanted, this becomes `Vec<String>` plus a side table of orders;
    /// nothing else here assumes the id is unique per entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    /// Manual position **within this entry's group** — its collection, or the
    /// uncategorized pool when [`LibraryEntry::collection`] is `None`. Only
    /// [`SortMode::Manual`] reads it. Kept compact (0..n) by [`renumber`],
    /// which every mutation below calls, so pre-1.2 entries (all `0`) settle
    /// into their existing display order the first time anything is moved.
    #[serde(default)]
    pub order: i64,
    /// Unix timestamp of when the entry was added to the library, for
    /// [`SortMode::RecentlyAdded`]. Pre-1.2 entries.json files have no such
    /// record and deserialize as `0`, which sorts as "oldest" — the truthful
    /// answer, since every one of them predates the entries that do carry a
    /// timestamp.
    #[serde(default)]
    pub added: u64,
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
            power_saving: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
            collection: None,
            order: 0,
            added: now_secs(),
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
            power_saving: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
            collection: None,
            order: 0,
            added: now_secs(),
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
            .unwrap_or_else(|| t!("Playlist").to_string());
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
            power_saving: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
            collection: None,
            order: 0,
            added: now_secs(),
        }
    }

    pub fn new_slideshow(folder: PathBuf) -> Self {
        let name = folder
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| t!("Slideshow").to_string());
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
            power_saving: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
            collection: None,
            order: 0,
            added: now_secs(),
        }
    }

    /// A slideshow built from hand-picked image files (no folder).
    pub fn new_image_set(paths: Vec<PathBuf>) -> Self {
        let name = tf!("Slideshow ({count} images)", "count" => paths.len().to_string());
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
            power_saving: None,
            catalog_id: None,
            width: None,
            height: None,
            fps: None,
            size_bytes: None,
            favorite: false,
            collection: None,
            order: 0,
            added: now_secs(),
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
        self.last_used = now_secs();
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

    // ─── Playlist / slideshow item editing ────────────────────────────────
    //
    // A multi-file entry used to be write-once: the only way to change what
    // was in it was to delete it and re-pick every file. These three methods
    // are the model side of making that list editable.
    //
    // All three keep `paths`, `kind` and `folder` mutually consistent, re-run
    // `check_health`, and **invalidate `thumbnail` when the item that the
    // thumbnail was rendered from changes** (that is `paths[0]`; see
    // `generate_thumbnail`). Invalidating means setting it to `None` — the
    // stale PNG stays on disk under the entry's id and is overwritten in
    // place on the next render. Regenerating is deliberately the caller's
    // job: it shells out to ffmpeg and must not happen inside a `borrow_mut`
    // on the UI thread. The contract is "if `thumbnail` came back `None`,
    // call `generate_thumbnail()` before you redraw the card".

    /// Append media files to a multi-file entry, skipping unsupported
    /// extensions and files the entry already contains.
    ///
    /// Two conversions happen here, both of them because the alternative is a
    /// state the daemon cannot play:
    ///
    /// * A single-file `Video`/`Image` grows into a `Playlist` (or a
    ///   `Slideshow`, when every item is a still) with its old `path` first —
    ///   otherwise `path` and `paths` would both be set and only one of them
    ///   would ever be honoured.
    /// * A **folder-backed** slideshow is materialised first: the folder is
    ///   scanned into `paths` and `folder` is cleared. `slideshow_images` in
    ///   the daemon prefers explicit `paths` over `folder`, so appending
    ///   without materialising would silently *replace* the folder's contents
    ///   with the one file just added. Materialising is also what the user
    ///   asking for this actually wanted: "add a wallpaper to that playlist"
    ///   means add it to what is playing now. The trade-off is that the entry
    ///   stops tracking later additions to the folder — a hand-edited list is
    ///   a hand-edited list.
    pub fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let incoming: Vec<PathBuf> = paths
            .into_iter()
            .filter(|p| is_video(p) || is_image(p))
            .collect();
        if incoming.is_empty() {
            return;
        }
        if self.paths.is_empty() {
            if let Some(folder) = self.folder.take() {
                self.paths = folder_media(&folder, false);
            } else if let Some(single) = self.path.take() {
                self.paths = vec![single];
            }
        }
        let was_empty = self.paths.is_empty();
        let mut seen: Vec<PathBuf> = self.paths.iter().map(|p| canonical(p)).collect();
        for p in incoming {
            let key = canonical(&p);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            self.paths.push(p);
        }
        self.path = None;
        self.retype_for_paths();
        if was_empty {
            self.thumbnail = None;
        }
        self.check_health();
    }

    /// Drop the item at `index`, returning it. Out-of-range is a no-op
    /// (`None`) rather than a panic — the index comes from a list row that may
    /// have been re-rendered since the click.
    ///
    /// Removing the *last* item leaves `paths` empty, which `check_health`
    /// marks broken; the caller should either add something back or delete the
    /// entry. Collapsing a one-item playlist back into a plain `Video` entry is
    /// deliberately not done here — it would change the entry's kind (and so
    /// its home section) as a side effect of a delete the user thinks of as
    /// undoing one row.
    pub fn remove_path_at(&mut self, index: usize) -> Option<PathBuf> {
        if index >= self.paths.len() {
            return None;
        }
        let removed = self.paths.remove(index);
        if index == 0 {
            self.thumbnail = None;
        }
        self.retype_for_paths();
        self.check_health();
        Some(removed)
    }

    /// Reorder one item, moving it from `from` to `to` (both indices into
    /// `paths` as it is *before* the move). Returns false for an out-of-range
    /// or no-op move.
    pub fn move_path(&mut self, from: usize, to: usize) -> bool {
        let len = self.paths.len();
        if from >= len || to >= len || from == to {
            return false;
        }
        let item = self.paths.remove(from);
        self.paths.insert(to, item);
        // Only position 0 feeds the thumbnail, so only a move that touches it
        // invalidates one.
        if from == 0 || to == 0 {
            self.thumbnail = None;
        }
        true
    }

    /// Re-derive `kind` after `paths` changed: an all-stills list is a
    /// slideshow (images in a playlist would flash past at video pace), a list
    /// with any video in it is a playlist. Mirrors the choice `add_media_paths`
    /// makes when the entry is first created, so an entry does not end up in a
    /// kind its contents cannot sustain. An emptied list keeps its old kind —
    /// there is nothing left to infer from.
    fn retype_for_paths(&mut self) {
        if self.paths.is_empty() {
            return;
        }
        self.kind = if self.paths.iter().all(|p| is_image(p)) {
            Kind::Slideshow
        } else {
            Kind::Playlist
        };
        if self.kind == Kind::Slideshow {
            // A hand-picked slideshow still needs a cadence; keep whatever the
            // entry already had rather than resetting a tuned interval.
            self.interval_s.get_or_insert(30);
            self.transition.get_or_insert(Transition::Crossfade);
        }
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
            power_saving: self.power_saving,
            framerate: None, // deprecated; see Config::migrate
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

/// Seconds since the epoch, or `0` if the clock is before it. Shared by
/// `LibraryEntry::touch` and the `added` stamp so both use one definition of
/// "now" — and one definition of the sentinel that means "unknown".
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

// ─── Collections ──────────────────────────────────────────────────────────────

/// A user-created folder — "SCI-FI", "Nature", "Cityscapes" — grouping library
/// entries.
///
/// Collections live in their own `collections.json` rather than as a key inside
/// `entries.json`. The entries file is rewritten on every activation, rename,
/// health check and metadata probe; hanging a second schema off it means every
/// one of those writes can take the folder list down with it, and means a
/// pre-1.2 Fresco reading the file would drop the folders on its next save.
/// A separate file makes the failure modes independent: a missing or corrupt
/// `collections.json` costs the user their folders, never their wallpapers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    /// Display order among sibling collections. Kept compact (0..n) by
    /// [`renumber_collections`].
    #[serde(default)]
    pub position: u32,
}

impl Collection {
    pub fn new(name: impl Into<String>, position: u32) -> Self {
        Self {
            id: make_id(),
            name: name.into(),
            position,
        }
    }
}

fn collections_path() -> PathBuf {
    library_dir().join("collections.json")
}

/// Read the folder list. **A missing file is not an error** — it is what every
/// install that has never made a folder looks like, which is all of them before
/// this feature shipped.
pub fn load_collections() -> Result<Vec<Collection>> {
    let path = collections_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let collections: Vec<Collection> =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(collections)
}

/// Write the folder list, atomically (write to `.tmp`, rename over the top) so
/// an interrupted save cannot leave a half-written file behind — the same
/// pattern [`save_entries`] uses, for the same reason.
pub fn save_collections(collections: &[Collection]) -> Result<()> {
    let dir = library_dir();
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = collections_path();
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(collections)?)
        .with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

/// Add a folder at the end of the list; returns its id, which is what the
/// caller needs to drop entries into it. An all-whitespace name would render as
/// an unclickable blank row, so it falls back to a placeholder the user can
/// rename.
pub fn create_collection(collections: &mut Vec<Collection>, name: &str) -> String {
    let name = name.trim();
    let name = if name.is_empty() {
        t!("New Folder").to_string()
    } else {
        name.to_string()
    };
    let c = Collection::new(name, collections.len() as u32);
    let id = c.id.clone();
    collections.push(c);
    renumber_collections(collections);
    id
}

/// Rename a folder in place. Returns false when the id is unknown or the new
/// name is blank — both are "nothing happened", not errors worth a dialog.
pub fn rename_collection(collections: &mut [Collection], id: &str, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    match collections.iter_mut().find(|c| c.id == id) {
        Some(c) => {
            c.name = name.to_string();
            true
        }
        None => false,
    }
}

/// Delete a folder and hand its contents back to the uncategorized pool.
///
/// **Deleting a folder never deletes wallpapers.** Members are appended to the
/// end of the uncategorized list in the relative order they had inside the
/// folder, so the user can see where everything went and, if the delete was a
/// mistake, re-select the block and drop it into a new folder. Leaving the
/// members pointing at a dead id would strand them: they belong to no folder
/// the UI draws, so they would simply vanish from the library.
pub fn delete_collection(
    collections: &mut Vec<Collection>,
    entries: &mut [LibraryEntry],
    id: &str,
) -> bool {
    if !collections.iter().any(|c| c.id == id) {
        return false;
    }
    let mut members: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.collection.as_deref() == Some(id))
        .map(|(i, _)| i)
        .collect();
    members.sort_by_key(|&i| (entries[i].order, i));
    let base = next_order(entries, None);
    for (offset, i) in members.into_iter().enumerate() {
        entries[i].collection = None;
        entries[i].order = base + offset as i64;
    }
    collections.retain(|c| c.id != id);
    renumber_collections(collections);
    renumber(entries, None);
    true
}

/// Move entries into a folder (`Some(id)`) or back out of every folder
/// (`None`). Unknown ids are skipped.
///
/// Moved entries land at the *end* of the destination, which is the only
/// placement that needs no further input from the user; they can then be
/// nudged with [`move_entry_up`] / [`move_entry_down`]. Both the destination
/// and every folder the entries left are renumbered, so no group is left with
/// gaps or duplicate `order` values.
pub fn assign(entries: &mut [LibraryEntry], entry_ids: &[String], collection_id: Option<&str>) {
    let mut touched: Vec<Option<String>> = vec![collection_id.map(str::to_string)];
    let mut next = next_order(entries, collection_id);
    for id in entry_ids {
        let Some(e) = entries.iter_mut().find(|e| &e.id == id) else {
            continue;
        };
        if e.collection.as_deref() == collection_id {
            continue;
        }
        let from = e.collection.clone();
        if !touched.contains(&from) {
            touched.push(from);
        }
        e.collection = collection_id.map(str::to_string);
        e.order = next;
        next += 1;
    }
    for group in touched {
        renumber(entries, group.as_deref());
    }
}

/// The entries in one folder, in manual order.
pub fn entries_in<'a>(entries: &'a [LibraryEntry], collection_id: &str) -> Vec<&'a LibraryEntry> {
    let mut v: Vec<&LibraryEntry> = entries
        .iter()
        .filter(|e| e.collection.as_deref() == Some(collection_id))
        .collect();
    sort_entries(&mut v, SortMode::Manual);
    v
}

/// The entries that are in no folder, in manual order. This is the default
/// library view, and the home of everything that existed before folders did.
pub fn uncategorized(entries: &[LibraryEntry]) -> Vec<&LibraryEntry> {
    let mut v: Vec<&LibraryEntry> = entries.iter().filter(|e| e.collection.is_none()).collect();
    sort_entries(&mut v, SortMode::Manual);
    v
}

/// Look up a folder's display name.
pub fn collection_name<'a>(collections: &'a [Collection], id: &str) -> Option<&'a str> {
    collections
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name.as_str())
}

/// The `order` a newly added entry should take in `collection_id` — one past
/// the current last item.
pub fn next_order(entries: &[LibraryEntry], collection_id: Option<&str>) -> i64 {
    entries
        .iter()
        .filter(|e| e.collection.as_deref() == collection_id)
        .map(|e| e.order)
        .max()
        .map_or(0, |m| m.saturating_add(1))
}

/// Append a freshly built entry to the library with a manual position that puts
/// it at the end of the uncategorized list, rather than tied at `order == 0`
/// with everything imported before folders existed.
pub fn push_entry(entries: &mut Vec<LibraryEntry>, mut entry: LibraryEntry) {
    entry.order = next_order(entries, entry.collection.as_deref());
    entries.push(entry);
}

/// Renumber one group's `order` values to a compact 0..n, preserving the order
/// they currently sort in.
///
/// Every mutation that can leave holes (a move, a removal, a reassignment)
/// calls this, which is what lets the rest of the model treat `order` as a
/// dense index. It is also the migration path: entries written before 1.2 all
/// carry `order == 0`, and the tie-break on vector position means their first
/// renumber freezes the order they were already being displayed in instead of
/// scrambling them.
pub fn renumber(entries: &mut [LibraryEntry], collection_id: Option<&str>) {
    let mut idx: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.collection.as_deref() == collection_id)
        .map(|(i, _)| i)
        .collect();
    idx.sort_by_key(|&i| (entries[i].order, i));
    for (rank, i) in idx.into_iter().enumerate() {
        entries[i].order = rank as i64;
    }
}

/// Compact the folder list's `position` values to 0..n, preserving order.
pub fn renumber_collections(collections: &mut [Collection]) {
    let mut idx: Vec<usize> = (0..collections.len()).collect();
    idx.sort_by_key(|&i| (collections[i].position, i));
    for (rank, i) in idx.into_iter().enumerate() {
        collections[i].position = rank as u32;
    }
}

/// Move an entry one place earlier within its own group. Returns false when it
/// is already first (or the id is unknown), so the caller can skip the save.
pub fn move_entry_up(entries: &mut [LibraryEntry], entry_id: &str) -> bool {
    shift_entry(entries, entry_id, -1)
}

/// Move an entry one place later within its own group.
pub fn move_entry_down(entries: &mut [LibraryEntry], entry_id: &str) -> bool {
    shift_entry(entries, entry_id, 1)
}

/// Shared body of [`move_entry_up`] / [`move_entry_down`].
///
/// The group is renumbered *first*: a swap of two `order` values is a no-op
/// when both are the same, and "both are the same" is exactly the state a
/// library upgraded from 1.1 is in (every entry at `0`). After renumbering the
/// values are distinct, so swapping the mover with its neighbour is enough and
/// nothing else in the group has to move.
fn shift_entry(entries: &mut [LibraryEntry], entry_id: &str, delta: i64) -> bool {
    let Some(pos) = entries.iter().position(|e| e.id == entry_id) else {
        return false;
    };
    let group = entries[pos].collection.clone();
    renumber(entries, group.as_deref());
    let rank = entries[pos].order;
    let target = rank + delta;
    if target < 0 {
        return false;
    }
    let Some(other) = entries
        .iter()
        .position(|e| e.collection == group && e.order == target)
    else {
        return false;
    };
    entries[pos].order = target;
    entries[other].order = rank;
    true
}

// ─── Sorting ──────────────────────────────────────────────────────────────────

/// How the library grid is ordered.
///
/// Serialized with explicit names rather than the Rust spelling of the variant:
/// this value is persisted in `view.json`, and a later rename of a variant must
/// not silently reset everyone's saved preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortMode {
    /// The user's own arrangement, from [`LibraryEntry::order`]. The default,
    /// because it is the only mode that a drag or a "move up" can write back
    /// to — every other mode is derived and would silently discard the move.
    #[default]
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "name-asc")]
    NameAsc,
    #[serde(rename = "name-desc")]
    NameDesc,
    #[serde(rename = "recently-used")]
    RecentlyUsed,
    #[serde(rename = "recently-added")]
    RecentlyAdded,
    #[serde(rename = "largest")]
    LargestFirst,
    #[serde(rename = "smallest")]
    SmallestFirst,
    #[serde(rename = "resolution")]
    Resolution,
}

impl SortMode {
    /// Every mode, in menu order — so a GUI need not hand-list the variants a
    /// second time.
    pub const ALL: [SortMode; 8] = [
        SortMode::Manual,
        SortMode::NameAsc,
        SortMode::NameDesc,
        SortMode::RecentlyUsed,
        SortMode::RecentlyAdded,
        SortMode::LargestFirst,
        SortMode::SmallestFirst,
        SortMode::Resolution,
    ];

    /// The name shown in the sort menu.
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Manual => t!("My order"),
            SortMode::NameAsc => t!("Name (A–Z)"),
            SortMode::NameDesc => t!("Name (Z–A)"),
            SortMode::RecentlyUsed => t!("Recently used"),
            SortMode::RecentlyAdded => t!("Recently added"),
            SortMode::LargestFirst => t!("Largest file"),
            SortMode::SmallestFirst => t!("Smallest file"),
            SortMode::Resolution => t!("Resolution"),
        }
    }

    /// True when this mode reflects a hand-made arrangement, i.e. when reorder
    /// controls should be offered. Under any other mode a "move up" would be
    /// written to `order` and then immediately overruled by the sort, so the
    /// UI must hide (or disable) those controls instead of lying about them.
    pub fn is_manual(self) -> bool {
        self == SortMode::Manual
    }
}

/// Order `entries` in place.
///
/// Every mode is a **total** order: the primary key is followed by a name-then-
/// id tie-break, so the same library always renders in the same sequence no
/// matter what order the entries happened to be collected in.
///
/// Entries with unknown metadata sort **last** in every mode that reads it, in
/// both directions. Sorting by "smallest file" must not open with a wall of
/// entries whose size simply has not been probed yet — an unprobed entry is not
/// a zero-byte entry, and the same goes for `last_used == 0` (never used) and
/// `added == 0` (imported before Fresco recorded the date).
pub fn sort_entries(entries: &mut [&LibraryEntry], mode: SortMode) {
    entries.sort_by(|a, b| primary_key(a, b, mode).then_with(|| tie_break(a, b)));
}

fn primary_key(a: &LibraryEntry, b: &LibraryEntry, mode: SortMode) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match mode {
        SortMode::Manual => a.order.cmp(&b.order),
        SortMode::NameAsc => name_key(a).cmp(&name_key(b)),
        SortMode::NameDesc => name_key(b).cmp(&name_key(a)),
        // Descending, so the "never" sentinel of 0 falls to the bottom on its
        // own — no special case needed.
        SortMode::RecentlyUsed => b.last_used.cmp(&a.last_used),
        SortMode::RecentlyAdded => b.added.cmp(&a.added),
        SortMode::LargestFirst => b.size_bytes.unwrap_or(0).cmp(&a.size_bytes.unwrap_or(0)),
        // Ascending, so unknown has to be pushed to the end explicitly.
        SortMode::SmallestFirst => a
            .size_bytes
            .unwrap_or(u64::MAX)
            .cmp(&b.size_bytes.unwrap_or(u64::MAX)),
        SortMode::Resolution => match (pixels(a), pixels(b)) {
            (Some(x), Some(y)) => y.cmp(&x),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
    }
}

/// Total pixel count, or `None` when the entry has not been probed.
fn pixels(e: &LibraryEntry) -> Option<u64> {
    match (e.width, e.height) {
        (Some(w), Some(h)) => Some(u64::from(w) * u64::from(h)),
        _ => None,
    }
}

/// Case-insensitive sort key. Names come from file stems, so "Aurora" and
/// "aurora" must not end up in different halves of the grid.
fn name_key(e: &LibraryEntry) -> String {
    e.name.to_lowercase()
}

/// Final tie-break, applied under every mode. The id is unique and stable, so
/// this makes the result independent of the input order — sorting an already
/// sorted list never reshuffles equal entries.
fn tie_break(a: &LibraryEntry, b: &LibraryEntry) -> std::cmp::Ordering {
    name_key(a).cmp(&name_key(b)).then_with(|| a.id.cmp(&b.id))
}

// ─── View preferences ─────────────────────────────────────────────────────────

/// Sticky library-screen state: how the grid is sorted and which folder is
/// open.
///
/// Kept here rather than in `Config` on purpose. `config.toml` is written by
/// the daemon as well as the GUI, and `Config` is compiled for both features;
/// a GUI-only enum in it would either need feature gates or risk a daemon-side
/// save dropping a key it was not built to understand. This is a view
/// preference, not configuration — it belongs next to the data it describes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryView {
    pub sort: SortMode,
    /// Folder shown on open; `None` = the whole library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
}

fn view_path() -> PathBuf {
    library_dir().join("view.json")
}

/// Read the saved view preference, falling back to the default.
///
/// Infallible by design, and the only loader here that is: a preference file
/// that is missing (every install until the first sort), unreadable, or written
/// by a future version has one obviously correct answer — show the default view
/// — and no answer at all that is worth interrupting a launch with.
pub fn load_view() -> LibraryView {
    fs::read_to_string(view_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Persist the view preference, atomically, like the other two stores.
pub fn save_view(view: &LibraryView) -> Result<()> {
    let dir = library_dir();
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = view_path();
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(view)?)
        .with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

// ─── Duplicate detection ──────────────────────────────────────────────────────

/// Resolve a path for comparison.
///
/// `canonicalize` is what makes `~/Videos/a.mp4`, `/home/me/Videos/../Videos/a.mp4`
/// and a symlink to the same file compare equal, which is most of what
/// "accidentally added it twice" actually looks like. It fails on paths that do
/// not exist — precisely the broken entries whose file has been moved away —
/// and there the raw path is still the best key available, so the failure falls
/// back rather than propagating.
fn canonical(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The existing entry that already points at `candidate`, if any.
///
/// Checks both the single-file `path` and playlist/slideshow `paths`, so
/// re-adding a file that is already an item of some playlist is reported too —
/// the user asked to stop accidentally adding duplicates, and a file being
/// buried inside a playlist is exactly the case they cannot see by eye.
/// Folder-backed slideshows are deliberately not expanded: a folder's contents
/// change under Fresco, and one file being inside one is not the same claim as
/// the library already holding it as a wallpaper.
pub fn duplicate_of<'a>(entries: &'a [LibraryEntry], candidate: &Path) -> Option<&'a LibraryEntry> {
    let target = canonical(candidate);
    entries.iter().find(|e| {
        e.path.as_deref().map(canonical).as_ref() == Some(&target)
            || e.paths.iter().any(|p| canonical(p) == target)
    })
}

/// Split a batch of picked files into `(fresh, duplicates)` so the caller can
/// import the first list and report the second ("Added 7 · skipped 3
/// duplicates").
///
/// Duplicates *within the batch itself* count as duplicates too — a file
/// selected twice, or reachable by two paths in the same drop, must not become
/// two entries. Both lists keep the caller's original order.
pub fn partition_new(
    entries: &[LibraryEntry],
    candidates: Vec<PathBuf>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for e in entries {
        if let Some(p) = e.path.as_deref() {
            seen.insert(canonical(p));
        }
        seen.extend(e.paths.iter().map(|p| canonical(p)));
    }
    let mut fresh = Vec::new();
    let mut dupes = Vec::new();
    for c in candidates {
        if seen.insert(canonical(&c)) {
            fresh.push(c);
        } else {
            dupes.push(c);
        }
    }
    (fresh, dupes)
}

// ─── Batch import ─────────────────────────────────────────────────────────────

/// Turn a multi-file selection into **one entry per file**.
///
/// This is the other half of the request behind collections: someone who
/// downloads a batch of wallpapers and keeps each one up for days wants ten
/// wallpapers, not one ten-item playlist. [`LibraryEntry::new_playlist`] and
/// [`LibraryEntry::new_image_set`] still exist and still do the grouping thing
/// — the picker offers both, because "play these in sequence" is a real and
/// different intent.
///
/// Unsupported extensions are dropped rather than imported as broken entries;
/// pair with [`partition_new`] first if the caller wants to report skips.
/// Thumbnails and metadata are *not* generated here (both shell out to ffmpeg,
/// which must not happen on the UI thread) — the caller runs them in the
/// background as it already does for single imports.
pub fn entries_for_each(paths: Vec<PathBuf>) -> Vec<LibraryEntry> {
    paths
        .into_iter()
        .filter_map(|p| {
            if is_video(&p) {
                Some(LibraryEntry::new_video(p))
            } else if is_image(&p) {
                Some(LibraryEntry::new_image(p))
            } else {
                None
            }
        })
        .collect()
}

/// How deep [`folder_media`] descends when asked to recurse. A bound rather
/// than a full walk: `is_dir` follows symlinks, so an unbounded descent can
/// loop forever on a self-referential link, and a wallpaper folder nested more
/// than a few levels deep is not what "add this folder" means.
const MAX_SCAN_DEPTH: usize = 4;

/// Every supported media file in `dir`, sorted by file name.
///
/// This is what lets "Add folder" offer *"as individual wallpapers"* next to
/// its existing "as a timed slideshow" — the user complained that the folder
/// picker only ever produced a slideshow. Non-recursive by default; the
/// recursive walk is depth-bounded (see [`MAX_SCAN_DEPTH`]).
pub fn folder_media(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    scan_media(dir, if recursive { MAX_SCAN_DEPTH } else { 0 }, &mut out);
    // By file name, with the full path as tie-break so a recursive scan that
    // finds "01.jpg" in two subfolders is still deterministic.
    out.sort_by(|a, b| {
        file_name_key(a)
            .cmp(&file_name_key(b))
            .then_with(|| a.cmp(b))
    });
    out
}

fn scan_media(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth > 0 {
                scan_media(&p, depth - 1, out);
            }
        } else if is_video(&p) || is_image(&p) {
            out.push(p);
        }
    }
}

fn file_name_key(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default()
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
        // …including the 1.2 folder/ordering fields: no folder, position 0,
        // and no record of when it was added (which sorts as "oldest").
        assert_eq!(e.collection, None);
        assert_eq!(e.order, 0);
        assert_eq!(e.added, 0);
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

    // ─── Collections ──────────────────────────────────────────────────────

    /// Build `n` entries with predictable names/ids for the ordering tests.
    fn fixtures(n: usize) -> Vec<LibraryEntry> {
        (0..n)
            .map(|i| {
                let mut e = LibraryEntry::new_image(PathBuf::from(format!("/pics/{i}.png")));
                e.id = format!("e{i}");
                e.name = format!("{i}");
                e
            })
            .collect()
    }

    fn ids(entries: &[&LibraryEntry]) -> Vec<String> {
        entries.iter().map(|e| e.id.clone()).collect()
    }

    #[test]
    fn collections_round_trip() {
        let mut cs = Vec::new();
        let sci = create_collection(&mut cs, "SCI-FI");
        let nature = create_collection(&mut cs, "  Nature  ");
        let json = serde_json::to_string(&cs).unwrap();
        let back: Vec<Collection> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].id, sci);
        assert_eq!(back[0].name, "SCI-FI");
        assert_eq!(back[0].position, 0);
        assert_eq!(back[1].id, nature);
        assert_eq!(back[1].name, "Nature"); // trimmed
        assert_eq!(back[1].position, 1);
    }

    /// `collections.json` predates nothing, but a hand-written one with only
    /// the required keys must still load — `position` is additive.
    #[test]
    fn collections_json_without_position_loads() {
        let cs: Vec<Collection> =
            serde_json::from_str(r#"[{"id":"c1","name":"Space"}]"#).expect("loads");
        assert_eq!(cs[0].position, 0);
    }

    #[test]
    fn rename_rejects_blank_and_unknown() {
        let mut cs = Vec::new();
        let id = create_collection(&mut cs, "Space");
        assert!(!rename_collection(&mut cs, &id, "   "));
        assert!(!rename_collection(&mut cs, "nope", "Cities"));
        assert!(rename_collection(&mut cs, &id, " Cityscapes "));
        assert_eq!(cs[0].name, "Cityscapes");
    }

    /// Deleting a folder must hand its wallpapers back, never orphan them.
    #[test]
    fn delete_collection_reassigns_members() {
        let mut entries = fixtures(4);
        let mut cs = Vec::new();
        let sci = create_collection(&mut cs, "SCI-FI");
        assign(&mut entries, &["e1".into(), "e2".into()], Some(&sci));
        assert_eq!(ids(&entries_in(&entries, &sci)), ["e1", "e2"]);
        assert_eq!(ids(&uncategorized(&entries)), ["e0", "e3"]);

        assert!(delete_collection(&mut cs, &mut entries, &sci));
        assert!(cs.is_empty());
        assert!(entries.iter().all(|e| e.collection.is_none()));
        // Members land at the end, keeping their in-folder order.
        assert_eq!(ids(&uncategorized(&entries)), ["e0", "e3", "e1", "e2"]);
        // …and the pool is compact again.
        let mut orders: Vec<i64> = entries.iter().map(|e| e.order).collect();
        orders.sort_unstable();
        assert_eq!(orders, [0, 1, 2, 3]);
        assert!(!delete_collection(&mut cs, &mut entries, &sci));
    }

    #[test]
    fn assign_moves_between_folders_and_back_out() {
        let mut entries = fixtures(3);
        let mut cs = Vec::new();
        let a = create_collection(&mut cs, "A");
        let b = create_collection(&mut cs, "B");
        assign(&mut entries, &["e0".into(), "e1".into()], Some(&a));
        assign(&mut entries, &["e0".into()], Some(&b));
        assert_eq!(ids(&entries_in(&entries, &a)), ["e1"]);
        assert_eq!(ids(&entries_in(&entries, &b)), ["e0"]);
        // The vacated folder was renumbered, not left with a hole at 0.
        assert_eq!(entries.iter().find(|e| e.id == "e1").unwrap().order, 0);
        assign(&mut entries, &["e0".into()], None);
        assert_eq!(ids(&uncategorized(&entries)), ["e2", "e0"]);
    }

    #[test]
    fn move_up_down_within_group() {
        let mut entries = fixtures(3);
        assert!(move_entry_down(&mut entries, "e0"));
        assert_eq!(ids(&uncategorized(&entries)), ["e1", "e0", "e2"]);
        assert!(move_entry_up(&mut entries, "e2"));
        assert_eq!(ids(&uncategorized(&entries)), ["e1", "e2", "e0"]);
        // Ends of the list, and unknown ids, are no-ops rather than panics.
        assert!(!move_entry_up(&mut entries, "e1"));
        assert!(!move_entry_down(&mut entries, "e0"));
        assert!(!move_entry_up(&mut entries, "ghost"));
        assert_eq!(ids(&uncategorized(&entries)), ["e1", "e2", "e0"]);
    }

    /// A library upgraded from 1.1 has every entry at `order == 0`; the first
    /// move must freeze the existing display order rather than scramble it.
    #[test]
    fn first_move_on_legacy_orders_keeps_sequence() {
        let mut entries = fixtures(4);
        assert!(entries.iter().all(|e| e.order == 0));
        assert!(move_entry_down(&mut entries, "e2"));
        assert_eq!(ids(&uncategorized(&entries)), ["e0", "e1", "e3", "e2"]);
    }

    #[test]
    fn moves_do_not_cross_folder_boundaries() {
        let mut entries = fixtures(3);
        let mut cs = Vec::new();
        let a = create_collection(&mut cs, "A");
        assign(&mut entries, &["e1".into()], Some(&a));
        // e1 is alone in its folder: nowhere to go, and it must not swap with
        // an uncategorized neighbour.
        assert!(!move_entry_up(&mut entries, "e1"));
        assert!(!move_entry_down(&mut entries, "e1"));
        assert_eq!(ids(&uncategorized(&entries)), ["e0", "e2"]);
    }

    #[test]
    fn push_entry_lands_at_the_end() {
        let mut entries = fixtures(2);
        renumber(&mut entries, None);
        push_entry(&mut entries, {
            let mut e = LibraryEntry::new_image(PathBuf::from("/pics/new.png"));
            e.id = "new".into();
            e.name = "new".into();
            e
        });
        assert_eq!(ids(&uncategorized(&entries)), ["e0", "e1", "new"]);
    }

    // ─── Sorting ──────────────────────────────────────────────────────────

    #[test]
    fn sort_modes_order_as_documented() {
        let mut entries = fixtures(3);
        entries[0].name = "Bravo".into();
        entries[1].name = "alpha".into();
        entries[2].name = "Charlie".into();
        entries[0].last_used = 10;
        entries[1].last_used = 30;
        entries[2].last_used = 20;
        entries[0].added = 300;
        entries[1].added = 100;
        entries[2].added = 200;
        entries[0].size_bytes = Some(50);
        entries[1].size_bytes = Some(10);
        entries[2].size_bytes = Some(90);
        entries[0].width = Some(1920);
        entries[0].height = Some(1080);
        entries[1].width = Some(3840);
        entries[1].height = Some(2160);
        entries[2].width = Some(1280);
        entries[2].height = Some(720);

        let sorted = |mode| {
            let mut v: Vec<&LibraryEntry> = entries.iter().collect();
            sort_entries(&mut v, mode);
            ids(&v)
        };
        // Case-insensitive: "alpha" beats "Bravo".
        assert_eq!(sorted(SortMode::NameAsc), ["e1", "e0", "e2"]);
        assert_eq!(sorted(SortMode::NameDesc), ["e2", "e0", "e1"]);
        assert_eq!(sorted(SortMode::RecentlyUsed), ["e1", "e2", "e0"]);
        assert_eq!(sorted(SortMode::RecentlyAdded), ["e0", "e2", "e1"]);
        assert_eq!(sorted(SortMode::LargestFirst), ["e2", "e0", "e1"]);
        assert_eq!(sorted(SortMode::SmallestFirst), ["e1", "e0", "e2"]);
        assert_eq!(sorted(SortMode::Resolution), ["e1", "e0", "e2"]);
    }

    /// Unknown metadata sorts last in *both* directions — an unprobed entry is
    /// not a zero-byte entry, and a `0` timestamp is "no record", not "1970".
    #[test]
    fn unknown_metadata_sorts_last() {
        let mut entries = fixtures(3);
        entries[0].size_bytes = Some(90);
        entries[1].size_bytes = None; // unprobed
        entries[2].size_bytes = Some(10);
        entries[0].added = 200;
        entries[1].added = 0; // pre-1.2 entry, no record
        entries[2].added = 100;
        entries[0].last_used = 5;
        entries[1].last_used = 0; // never used
        entries[2].last_used = 9;
        entries[0].width = Some(1920);
        entries[0].height = Some(1080);
        entries[2].width = Some(1280);
        entries[2].height = Some(720);

        let sorted = |mode| {
            let mut v: Vec<&LibraryEntry> = entries.iter().collect();
            sort_entries(&mut v, mode);
            ids(&v)
        };
        assert_eq!(sorted(SortMode::LargestFirst), ["e0", "e2", "e1"]);
        assert_eq!(sorted(SortMode::SmallestFirst), ["e2", "e0", "e1"]);
        assert_eq!(sorted(SortMode::RecentlyAdded), ["e0", "e2", "e1"]);
        assert_eq!(sorted(SortMode::RecentlyUsed), ["e2", "e0", "e1"]);
        assert_eq!(sorted(SortMode::Resolution), ["e0", "e2", "e1"]);
    }

    /// Every mode is a total order: sorting twice, from two different input
    /// orders, must give the same answer.
    #[test]
    fn sorting_is_total_and_stable() {
        let entries = fixtures(4); // identical metadata, differing only in id
        for mode in SortMode::ALL {
            let mut forward: Vec<&LibraryEntry> = entries.iter().collect();
            let mut backward: Vec<&LibraryEntry> = entries.iter().rev().collect();
            sort_entries(&mut forward, mode);
            sort_entries(&mut backward, mode);
            assert_eq!(ids(&forward), ids(&backward), "{mode:?} is not total");
        }
    }

    #[test]
    fn sort_mode_serializes_by_explicit_name() {
        assert_eq!(
            serde_json::to_string(&SortMode::RecentlyAdded).unwrap(),
            "\"recently-added\""
        );
        assert_eq!(
            serde_json::from_str::<SortMode>("\"manual\"").unwrap(),
            SortMode::Manual
        );
        assert_eq!(SortMode::default(), SortMode::Manual);
        assert!(SortMode::Manual.is_manual());
        assert!(!SortMode::NameAsc.is_manual());
        assert_eq!(SortMode::ALL.len(), 8);
    }

    // ─── View preferences ─────────────────────────────────────────────────

    #[test]
    fn view_round_trips_and_defaults() {
        let v = LibraryView {
            sort: SortMode::Resolution,
            collection: Some("c1".into()),
        };
        let back: LibraryView = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(back, v);
        // An empty object — or a file written by a future version that only
        // knows other keys — must fall back to the defaults, not fail.
        let empty: LibraryView = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, LibraryView::default());
        assert_eq!(empty.sort, SortMode::Manual);
        assert!(empty.collection.is_none());
    }

    /// `load_view` never fails: a missing or corrupt file is a default view.
    /// (`library_dir()` is untouched here — the test only asserts the parse
    /// side of that contract, which is the part that can go wrong.)
    #[test]
    fn malformed_view_parses_to_default() {
        let parsed: LibraryView = serde_json::from_str("not json").unwrap_or_default();
        assert_eq!(parsed, LibraryView::default());
    }

    // ─── Duplicate detection ──────────────────────────────────────────────

    #[test]
    fn duplicate_detected_via_path_and_paths() {
        let mut entries = vec![
            LibraryEntry::new_video(PathBuf::from("/videos/car.mp4")),
            LibraryEntry::new_playlist(vec![
                PathBuf::from("/videos/a.mp4"),
                PathBuf::from("/videos/b.mp4"),
            ]),
        ];
        entries[0].id = "single".into();
        entries[1].id = "list".into();
        assert_eq!(
            duplicate_of(&entries, Path::new("/videos/car.mp4")).map(|e| e.id.as_str()),
            Some("single")
        );
        assert_eq!(
            duplicate_of(&entries, Path::new("/videos/b.mp4")).map(|e| e.id.as_str()),
            Some("list")
        );
        assert!(duplicate_of(&entries, Path::new("/videos/new.mp4")).is_none());
    }

    #[test]
    fn partition_new_splits_and_dedupes_within_the_batch() {
        let entries = vec![LibraryEntry::new_video(PathBuf::from("/videos/car.mp4"))];
        let (fresh, dupes) = partition_new(
            &entries,
            vec![
                PathBuf::from("/videos/new.mp4"),
                PathBuf::from("/videos/car.mp4"),
                PathBuf::from("/videos/new.mp4"), // same file twice in one pick
                PathBuf::from("/videos/other.mp4"),
            ],
        );
        assert_eq!(
            fresh,
            vec![
                PathBuf::from("/videos/new.mp4"),
                PathBuf::from("/videos/other.mp4")
            ]
        );
        assert_eq!(
            dupes,
            vec![
                PathBuf::from("/videos/car.mp4"),
                PathBuf::from("/videos/new.mp4")
            ]
        );
    }

    // ─── Batch import ─────────────────────────────────────────────────────

    #[test]
    fn entries_for_each_makes_one_entry_per_file_and_skips_junk() {
        let made = entries_for_each(vec![
            PathBuf::from("/m/a.mp4"),
            PathBuf::from("/m/b.png"),
            PathBuf::from("/m/readme.txt"),
            PathBuf::from("/m/no_extension"),
        ]);
        assert_eq!(made.len(), 2);
        assert_eq!(made[0].kind, Kind::Video);
        assert_eq!(made[0].name, "a");
        assert_eq!(made[1].kind, Kind::Image);
        assert_eq!(made[1].name, "b");
        // Every one is independently usable — no shared playlist.
        assert!(made.iter().all(|e| e.path.is_some() && e.paths.is_empty()));
        assert!(made.iter().all(|e| e.added > 0));
    }

    #[test]
    fn folder_media_lists_sorted_and_respects_recursion() {
        let dir = std::env::temp_dir().join(format!("fresco-folder-media-{}", make_id()));
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        for f in ["b.png", "a.mp4", "notes.txt"] {
            fs::write(dir.join(f), b"x").unwrap();
        }
        fs::write(sub.join("c.png"), b"x").unwrap();

        let flat = folder_media(&dir, false);
        assert_eq!(
            flat.iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["a.mp4", "b.png"]
        );
        let deep = folder_media(&dir, true);
        assert_eq!(deep.len(), 3);
        assert!(deep.iter().any(|p| p.ends_with("sub/c.png")));
        assert!(folder_media(&dir.join("missing"), false).is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    // ─── Playlist item editing ────────────────────────────────────────────

    #[test]
    fn playlist_items_can_be_added_removed_and_reordered() {
        let mut e =
            LibraryEntry::new_playlist(vec![PathBuf::from("/v/a.mp4"), PathBuf::from("/v/b.mp4")]);
        e.thumbnail = Some(PathBuf::from("/thumbs/x.png"));

        e.add_paths(vec![
            PathBuf::from("/v/c.mp4"),
            PathBuf::from("/v/a.mp4"),     // already in the list
            PathBuf::from("/v/notes.txt"), // unsupported
        ]);
        assert_eq!(e.paths.len(), 3);
        assert!(e.paths.ends_with(&[PathBuf::from("/v/c.mp4")]));
        // Nothing changed at position 0, so the thumbnail is still valid.
        assert!(e.thumbnail.is_some());

        assert!(e.move_path(2, 0));
        assert_eq!(e.paths[0], PathBuf::from("/v/c.mp4"));
        assert!(e.thumbnail.is_none(), "moving into slot 0 invalidates it");
        assert!(!e.move_path(0, 0));
        assert!(!e.move_path(0, 9));

        e.thumbnail = Some(PathBuf::from("/thumbs/x.png"));
        assert_eq!(e.remove_path_at(1), Some(PathBuf::from("/v/a.mp4")));
        assert!(e.thumbnail.is_some());
        assert_eq!(e.remove_path_at(9), None);
        assert_eq!(e.remove_path_at(0), Some(PathBuf::from("/v/c.mp4")));
        assert!(e.thumbnail.is_none());
        assert_eq!(e.paths, vec![PathBuf::from("/v/b.mp4")]);
    }

    /// Adding a still to a single-image entry promotes it to a slideshow with
    /// the original file first; adding a video makes it a playlist instead.
    #[test]
    fn add_paths_promotes_single_file_entries() {
        let mut img = LibraryEntry::new_image(PathBuf::from("/p/a.png"));
        img.add_paths(vec![PathBuf::from("/p/b.png")]);
        assert_eq!(img.kind, Kind::Slideshow);
        assert!(img.path.is_none());
        assert_eq!(
            img.paths,
            vec![PathBuf::from("/p/a.png"), PathBuf::from("/p/b.png")]
        );
        assert_eq!(img.interval_s, Some(30));

        let mut vid = LibraryEntry::new_image(PathBuf::from("/p/a.png"));
        vid.add_paths(vec![PathBuf::from("/p/clip.mp4")]);
        assert_eq!(vid.kind, Kind::Playlist);
    }

    #[test]
    fn add_paths_ignores_an_empty_or_unsupported_batch() {
        let mut e = LibraryEntry::new_image(PathBuf::from("/p/a.png"));
        e.add_paths(vec![]);
        e.add_paths(vec![PathBuf::from("/p/readme.txt")]);
        assert_eq!(e.kind, Kind::Image);
        assert_eq!(e.path, Some(PathBuf::from("/p/a.png")));
        assert!(e.paths.is_empty());
    }

    /// A folder-backed slideshow is materialised before the append, or the
    /// daemon's "explicit paths win over folder" rule would quietly replace the
    /// whole folder with the one file just added.
    #[test]
    fn add_paths_materializes_a_folder_slideshow() {
        let dir = std::env::temp_dir().join(format!("fresco-materialize-{}", make_id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("1.png"), b"x").unwrap();
        fs::write(dir.join("2.png"), b"x").unwrap();

        let mut e = LibraryEntry::new_slideshow(dir.clone());
        e.add_paths(vec![PathBuf::from("/p/extra.png")]);
        assert!(e.folder.is_none());
        assert_eq!(e.paths.len(), 3);
        assert!(e.paths[0].ends_with("1.png"));
        assert_eq!(e.paths[2], PathBuf::from("/p/extra.png"));
        assert_eq!(e.kind, Kind::Slideshow);
        fs::remove_dir_all(&dir).ok();
    }
}

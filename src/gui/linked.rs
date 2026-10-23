//! Linked folders: a folder of wallpapers that stays where it is and is shown
//! in place, instead of being imported file by file.
//!
//! A linked folder is a [`Collection`](super::library::Collection) carrying a
//! [`Link`]; that list is the only thing stored about the folder itself. Its
//! files become ordinary [`LibraryEntry`] values built from a scan on every
//! launch, so everything that works on an entry (apply, editor, schedules,
//! `fresco next`) works on them unchanged. They are never written to
//! `entries.json`: a 4,000-file folder would otherwise bloat a file that is
//! rewritten on every activation. What the user changes on one (favourite,
//! name, rotation, sound, last used) is kept as a [`Tweaks`] keyed by path in
//! `linked.json`, and only for files that actually differ from the default.
//!
//! An entry's id is a hash of its path, so thumbnails, hover previews and
//! tweaks find their file again after a restart.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::library::{self, Collection, Link, LinkStatus, LibraryEntry};
use crate::config::{PowerSaving, Transition};
use crate::media;

const ID_PREFIX: &str = "l-";

/// Directories watched per linked folder. inotify watches are a per-user
/// budget shared with every other program; a folder tree larger than this
/// still shows everything, but only the first directories notice changes on
/// their own (Rescan covers the rest).
pub const MAX_WATCHED_DIRS: usize = 64;

/// Whether `id` names a file of a linked folder (real ids are `{pid}-{n}`).
pub fn is_linked_id(id: &str) -> bool {
    id.starts_with(ID_PREFIX)
}

/// Id of the linked file at `path`: FNV-1a over its bytes. Unlike
/// `DefaultHasher`, the output is specified, so ids survive a Rust upgrade.
pub fn file_id(path: &Path) -> String {
    let hash = path
        .as_os_str()
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("{ID_PREFIX}{hash:016x}")
}

// ─── Scan ────────────────────────────────────────────────────────────────────

/// A media file found in a linked folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    pub size: u64,
    /// Seconds since the epoch.
    pub mtime: u64,
}

#[derive(Debug, Default)]
pub struct Scan {
    pub files: Vec<Found>,
    /// Every directory that was read, the root first: what to watch.
    pub dirs: Vec<PathBuf>,
}

fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.as_bytes().first() == Some(&b'.')
}

/// Scan a linked folder. `None` when the folder itself cannot be read (gone,
/// unmounted); an unreadable subfolder is skipped, like the import scan does.
///
/// Accepts exactly the files the import path accepts ([`media::is_supported`])
/// minus anything hidden, in hidden folders too, so `.Trash` and thumbnail
/// caches stay out. Paths that are not valid UTF-8 are skipped: they could not
/// be stored in `config.toml` once applied.
pub fn scan(link: &Link) -> Option<Scan> {
    let depth = if link.recursive {
        media::MAX_SCAN_DEPTH
    } else {
        0
    };
    let mut out = Scan::default();
    walk(&link.path, depth, &mut out).ok()?;
    Some(out)
}

fn walk(dir: &Path, depth: usize, out: &mut Scan) -> std::io::Result<()> {
    let rd = fs::read_dir(dir)?;
    out.dirs.push(dir.to_path_buf());
    for e in rd.flatten() {
        if is_hidden(&e.file_name()) {
            continue;
        }
        let path = e.path();
        // Follows symlinks, as the import scan's `is_dir` does; the depth bound
        // keeps a link loop finite.
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            if depth > 0 {
                walk(&path, depth - 1, out).ok();
            }
        } else if media::is_supported(&path) && path.to_str().is_some() {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            out.files.push(Found {
                path,
                size: meta.len(),
                mtime,
            });
        }
    }
    Ok(())
}

/// Whether linking `path` would show files an existing link already shows
/// (two sections can't share a file: they would share its id).
pub fn overlaps(existing: &Link, path: &Path, recursive: bool) -> bool {
    existing.path == path
        || (existing.recursive && path.starts_with(&existing.path))
        || (recursive && existing.path.starts_with(path))
}

// ─── Entries ─────────────────────────────────────────────────────────────────

/// A fresh entry for a scanned file: what a plain import would make, minus the
/// probe (`size_bytes` is known from the scan, which also marks it probed, so
/// a 4,000-file folder never costs 4,000 ffprobe runs).
fn entry_for(source: &str, f: &Found) -> LibraryEntry {
    let mut e = if library::is_video(&f.path) {
        LibraryEntry::new_video(f.path.clone())
    } else {
        LibraryEntry::new_image(f.path.clone())
    };
    e.id = file_id(&f.path);
    e.collection = Some(source.to_string());
    e.size_bytes = Some(f.size);
    e.added = f.mtime;
    // A thumbnail older than its file belongs to what the file used to be.
    let thumb = e.expected_thumbnail();
    let fresh = fs::metadata(&thumb)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .is_some_and(|t| t.as_secs() >= f.mtime);
    if fresh {
        e.thumbnail = Some(thumb);
    }
    e
}

/// The entries of one linked folder, in file-name order (`order` follows it:
/// a linked section is not hand-reorderable, it mirrors the folder).
pub fn build(source: &str, mut files: Vec<Found>, tweaks: &TweakMap) -> Vec<LibraryEntry> {
    files.sort_by(|a, b| {
        library::file_name_key(&a.path)
            .cmp(&library::file_name_key(&b.path))
            .then_with(|| a.path.cmp(&b.path))
    });
    files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut e = entry_for(source, f);
            e.order = i as i64;
            if let Some(t) = tweaks.get(&f.path) {
                t.apply(&mut e);
            }
            e
        })
        .collect()
}

/// Scan `link` and build its entries, with the directories to watch. `None`
/// when the folder cannot be read. Blocking: run it off the UI thread.
pub fn load_source(source: &str, link: &Link) -> Option<(Vec<LibraryEntry>, Vec<PathBuf>)> {
    let scan = scan(link)?;
    Some((build(source, scan.files, &load_tweaks()), scan.dirs))
}

/// Add the files of every reachable linked folder to `entries`, for callers
/// with no window to scan for them (`fresco next`).
pub fn append_all(entries: &mut Vec<LibraryEntry>, collections: &[Collection]) {
    for c in collections {
        if let Some((fresh, _)) = c.link.as_ref().and_then(|l| load_source(&c.id, l)) {
            entries.extend(fresh);
        }
    }
}

/// Make `entries` agree with a fresh scan of `source`; true when anything
/// changed. Files still there keep their in-memory entry (so an edit made
/// while the scan ran is not undone), new files are added, vanished ones are
/// dropped. A rename is a vanish plus an add. A file whose modification time
/// moved also gets its thumbnail and size refreshed.
pub fn sync_source(entries: &mut Vec<LibraryEntry>, source: &str, fresh: Vec<LibraryEntry>) -> bool {
    let mine = |e: &LibraryEntry| e.collection.as_deref() == Some(source);
    {
        let have: HashMap<&str, &LibraryEntry> = entries
            .iter()
            .filter(|e| mine(e))
            .map(|e| (e.id.as_str(), e))
            .collect();
        let same = have.len() == fresh.len()
            && fresh
                .iter()
                .all(|f| have.get(f.id.as_str()).is_some_and(|k| k.added == f.added));
        if same {
            return false;
        }
    }
    let (old, mut rest): (Vec<_>, Vec<_>) = std::mem::take(entries)
        .into_iter()
        .partition(|e| mine(e));
    let mut old: HashMap<String, LibraryEntry> =
        old.into_iter().map(|e| (e.id.clone(), e)).collect();
    rest.extend(fresh.into_iter().map(|f| match old.remove(&f.id) {
        Some(mut kept) => {
            kept.order = f.order;
            if kept.added != f.added {
                kept.added = f.added;
                kept.size_bytes = f.size_bytes;
                kept.thumbnail = f.thumbnail;
            }
            kept
        }
        None => f,
    }));
    *entries = rest;
    true
}

// ─── Change detection ────────────────────────────────────────────────────────

/// Whether a change at `path` can alter what `link` shows, so a rescan is due.
/// `path` is as GIO reports it, which for a deleted directory can no longer be
/// told apart from a file: in a recursive link anything below the root counts.
pub fn event_matters(link: &Link, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(&link.path) else {
        return false;
    };
    // The root itself: deleted, renamed or unmounted.
    if rel.as_os_str().is_empty() {
        return true;
    }
    if rel.components().any(|c| is_hidden(c.as_os_str())) {
        return false;
    }
    let max_depth = if link.recursive {
        1 + media::MAX_SCAN_DEPTH
    } else {
        1
    };
    rel.components().count() <= max_depth && (link.recursive || media::is_supported(path))
}

/// Whether a folder's reachability differs from what its section last showed.
pub fn reachability_changed(link: &Link, is_dir: bool) -> bool {
    match link.status {
        LinkStatus::Scanning => false,
        LinkStatus::Ready => !is_dir,
        LinkStatus::Missing => is_dir,
    }
}

// ─── Tweaks ──────────────────────────────────────────────────────────────────

/// What the user changed on one linked file. Everything else about it is
/// rebuilt from the file on every scan.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tweaks {
    /// Only when renamed; the default is the file stem.
    name: Option<String>,
    favorite: bool,
    last_used: u64,
    mute: Option<bool>,
    volume: Option<u8>,
    rotation: Option<u16>,
    power_saving: Option<PowerSaving>,
    transition: Option<Transition>,
}

impl Tweaks {
    fn of(e: &LibraryEntry) -> Self {
        let stem_is_name = e
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .is_some_and(|s| s.to_string_lossy() == e.name);
        Self {
            name: (!stem_is_name).then(|| e.name.clone()),
            favorite: e.favorite,
            last_used: e.last_used,
            mute: e.mute,
            volume: e.volume,
            rotation: e.rotation,
            power_saving: e.power_saving,
            transition: e.transition,
        }
    }

    fn apply(&self, e: &mut LibraryEntry) {
        if let Some(name) = &self.name {
            e.name.clone_from(name);
        }
        e.favorite = self.favorite;
        e.last_used = self.last_used;
        e.mute = self.mute;
        e.volume = self.volume;
        e.rotation = self.rotation;
        e.power_saving = self.power_saving;
        e.transition = self.transition;
    }
}

/// Tweaks by file path. Only files that differ from the default are present.
pub type TweakMap = BTreeMap<PathBuf, Tweaks>;

fn tweaks_path() -> PathBuf {
    library::library_dir().join("linked.json")
}

/// A missing or unreadable file is no tweaks: the files just look untouched.
pub fn load_tweaks() -> TweakMap {
    let path = tweaks_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return TweakMap::new();
    };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        log::warn!("ignoring {}: {e}", path.display());
        TweakMap::new()
    })
}

/// Fold the linked entries of `entries` into `map`; true when `map` changed.
/// Files not in `entries` (an unplugged drive) keep what they had.
fn merge_tweaks(map: &mut TweakMap, entries: &[LibraryEntry]) -> bool {
    let mut changed = false;
    for e in entries.iter().filter(|e| is_linked_id(&e.id)) {
        let Some(path) = &e.path else { continue };
        let t = Tweaks::of(e);
        changed |= if t == Tweaks::default() {
            map.remove(path).is_some()
        } else {
            map.insert(path.clone(), t.clone()) != Some(t)
        };
    }
    changed
}

fn write_tweaks(map: &TweakMap) -> Result<()> {
    let path = tweaks_path();
    let dir = library::library_dir();
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(map)?)
        .with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

/// Persist the tweaks of the linked entries in `entries`. Touches nothing on
/// disk when there are none, so a library that never links a folder never
/// grows a `linked.json`.
pub fn save_tweaks(entries: &[LibraryEntry]) -> Result<()> {
    if !entries.iter().any(|e| is_linked_id(&e.id)) {
        return Ok(());
    }
    let mut map = load_tweaks();
    if merge_tweaks(&mut map, entries) {
        write_tweaks(&map)?;
    }
    Ok(())
}

/// Drop the tweaks of everything under `root`, when its link is removed.
// ponytail: a file deleted from a still-linked folder keeps its tweaks (a
// re-added file gets its favourite back); only unlinking clears them.
pub fn forget_tweaks(root: &Path) {
    let mut map = load_tweaks();
    let before = map.len();
    map.retain(|p, _| !p.starts_with(root));
    if map.len() != before {
        write_tweaks(&map).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tree(tag: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fresco-linked-{tag}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        for f in files {
            let p = dir.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"x").unwrap();
        }
        dir
    }

    fn link(path: &Path, recursive: bool) -> Link {
        Link {
            path: path.to_path_buf(),
            recursive,
            status: LinkStatus::Ready,
        }
    }

    fn names(scan: &Scan, root: &Path) -> Vec<String> {
        let mut v: Vec<String> = scan
            .files
            .iter()
            .map(|f| f.path.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn scan_keeps_what_import_accepts_and_skips_hidden() {
        let dir = tree(
            "filter",
            &[
                "a.jpg", "b.PNG", "c.webp", "d.gif", "e.mp4", "f.webm", "g.mkv", "h.mov",
                "i.avi", "notes.txt", "noext", ".hidden.jpg", ".cache/x.png", "sub/s.jpg",
            ],
        );
        let flat = scan(&link(&dir, false)).unwrap();
        assert_eq!(
            names(&flat, &dir),
            [
                "a.jpg", "b.PNG", "c.webp", "d.gif", "e.mp4", "f.webm", "g.mkv", "h.mov",
                "i.avi"
            ]
        );
        assert_eq!(flat.dirs, [dir.clone()], "a flat scan reads only the root");

        let deep = scan(&link(&dir, true)).unwrap();
        let got = names(&deep, &dir);
        assert!(got.contains(&"sub/s.jpg".to_string()));
        assert!(
            !got.iter().any(|n| n.contains(".cache") || n.contains("hidden")),
            "hidden files and folders stay out: {got:?}"
        );
        assert_eq!(deep.dirs.len(), 2, "root and sub");
        // Every accepted file is one the import path would accept.
        assert!(deep.files.iter().all(|f| media::is_supported(&f.path)));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_depth_is_bounded_and_a_missing_root_is_none() {
        let dir = tree("depth", &["1/2/3/4/5/deep.jpg", "1/2/ok.jpg"]);
        let got = names(&scan(&link(&dir, true)).unwrap(), &dir);
        assert_eq!(got, ["1/2/ok.jpg"], "{} levels at most", media::MAX_SCAN_DEPTH);
        assert!(scan(&link(&dir.join("missing"), true)).is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ids_come_from_the_path_alone() {
        let a = file_id(Path::new("/pics/a.jpg"));
        assert_eq!(a, file_id(Path::new("/pics/a.jpg")));
        assert_ne!(a, file_id(Path::new("/pics/b.jpg")));
        assert!(is_linked_id(&a) && a.len() == 18);
        // Pinned: a changed hash would orphan every thumbnail and tweak.
        assert_eq!(file_id(Path::new("")), "l-cbf29ce484222325");
        // Real ids are `{pid}-{n}`, never linked.
        assert!(!is_linked_id("1234-0"));
    }

    fn found(path: &str, mtime: u64) -> Found {
        Found {
            path: PathBuf::from(path),
            size: 10,
            mtime,
        }
    }

    #[test]
    fn built_entries_are_sorted_stable_and_probed() {
        let files = vec![found("/p/b.mp4", 1), found("/p/A.png", 2), found("/p/c.gif", 3)];
        let built = build("src", files.clone(), &TweakMap::new());
        let names: Vec<&str> = built.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["A", "b", "c"]);
        assert_eq!(built.iter().map(|e| e.order).collect::<Vec<_>>(), [0, 1, 2]);
        assert!(built.iter().all(|e| e.collection.as_deref() == Some("src")));
        assert!(built.iter().all(|e| !e.needs_probe()), "size is known from the scan");
        assert_eq!(built[0].added, 2);
        // Same files in another order give the same ids.
        let mut shuffled = files;
        shuffled.reverse();
        let again = build("src", shuffled, &TweakMap::new());
        assert_eq!(
            built.iter().map(|e| &e.id).collect::<Vec<_>>(),
            again.iter().map(|e| &e.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn tweaks_merge_onto_the_default_entry() {
        let files = vec![found("/p/a.png", 1), found("/p/b.png", 1)];
        let mut tweaks = TweakMap::new();
        tweaks.insert(
            PathBuf::from("/p/b.png"),
            Tweaks {
                name: Some("Beach".into()),
                favorite: true,
                rotation: Some(90),
                mute: Some(false),
                ..Tweaks::default()
            },
        );
        let built = build("s", files, &tweaks);
        assert_eq!(built[0].name, "a");
        assert!(!built[0].favorite && built[0].rotation.is_none());
        assert_eq!(built[1].name, "Beach");
        assert!(built[1].favorite);
        assert_eq!(built[1].rotation, Some(90));
        assert_eq!(built[1].mute, Some(false));
    }

    #[test]
    fn only_changed_files_are_stored_and_reverting_forgets_them() {
        let mut built = build("s", vec![found("/p/a.png", 1), found("/p/b.png", 1)], &TweakMap::new());
        let mut map = TweakMap::new();
        assert!(!merge_tweaks(&mut map, &built), "untouched files store nothing");
        assert!(map.is_empty());

        built[1].favorite = true;
        built[1].name = "Beach".into();
        assert!(merge_tweaks(&mut map, &built));
        assert_eq!(map.len(), 1);
        assert!(!merge_tweaks(&mut map, &built), "same state is not a change");

        // A regular entry is never stored here.
        let own = LibraryEntry::new_video(PathBuf::from("/own.mp4"));
        assert!(!merge_tweaks(&mut map, std::slice::from_ref(&own)));

        // Files missing from the slice (an unplugged drive) keep their tweaks.
        assert!(!merge_tweaks(&mut map, &built[..1]));
        assert_eq!(map.len(), 1);

        built[1].favorite = false;
        built[1].name = "b".into();
        assert!(merge_tweaks(&mut map, &built));
        assert!(map.is_empty());
    }

    #[test]
    fn tweaks_round_trip_through_json_keyed_by_path() {
        let mut map = TweakMap::new();
        map.insert(
            PathBuf::from("/p/a.png"),
            Tweaks {
                favorite: true,
                last_used: 7,
                transition: Some(Transition::Fade),
                ..Tweaks::default()
            },
        );
        let json = serde_json::to_string_pretty(&map).unwrap();
        assert!(json.contains("\"/p/a.png\""));
        let back: TweakMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back, map);
        // A future field or a hand-trimmed entry still loads.
        let sparse: TweakMap = serde_json::from_str(r#"{"/x.png":{"favorite":true,"new":1}}"#).unwrap();
        assert!(sparse[Path::new("/x.png")].favorite);
    }

    fn entries_of(source: &str, files: &[(&str, u64)]) -> Vec<LibraryEntry> {
        build(
            source,
            files.iter().map(|(p, m)| found(p, *m)).collect(),
            &TweakMap::new(),
        )
    }

    fn ids_of(entries: &[LibraryEntry], source: &str) -> Vec<String> {
        entries
            .iter()
            .filter(|e| e.collection.as_deref() == Some(source))
            .map(|e| e.name.clone())
            .collect()
    }

    #[test]
    fn sync_adds_removes_and_renames() {
        let own = LibraryEntry::new_video(PathBuf::from("/own.mp4"));
        let other = entries_of("other", &[("/o/x.png", 1)]);
        let mut entries = vec![own.clone()];
        entries.extend(other.clone());
        entries.extend(entries_of("s", &[("/p/a.png", 1), ("/p/b.png", 1)]));

        // Unchanged scan: nothing to do, nothing moves.
        let before: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
        assert!(!sync_source(&mut entries, "s", entries_of("s", &[("/p/a.png", 1), ("/p/b.png", 1)])));
        assert_eq!(before, entries.iter().map(|e| e.id.clone()).collect::<Vec<_>>());

        // Added.
        assert!(sync_source(
            &mut entries,
            "s",
            entries_of("s", &[("/p/a.png", 1), ("/p/b.png", 1), ("/p/c.png", 1)])
        ));
        assert_eq!(ids_of(&entries, "s"), ["a", "b", "c"]);

        // Removed.
        assert!(sync_source(&mut entries, "s", entries_of("s", &[("/p/a.png", 1), ("/p/c.png", 1)])));
        assert_eq!(ids_of(&entries, "s"), ["a", "c"]);

        // Renamed: the old name goes, the new one appears.
        assert!(sync_source(&mut entries, "s", entries_of("s", &[("/p/a.png", 1), ("/p/z.png", 1)])));
        assert_eq!(ids_of(&entries, "s"), ["a", "z"]);

        // Other sections and imported entries are untouched.
        assert!(entries.iter().any(|e| e.id == own.id));
        assert_eq!(ids_of(&entries, "other"), ["x"]);

        // The folder going away empties its section.
        assert!(sync_source(&mut entries, "s", Vec::new()));
        assert!(ids_of(&entries, "s").is_empty());
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn sync_keeps_in_memory_edits_and_refreshes_changed_files() {
        let mut entries = entries_of("s", &[("/p/a.png", 1), ("/p/b.png", 1)]);
        entries[0].favorite = true;
        entries[0].thumbnail = Some(PathBuf::from("/thumbs/a.png"));
        entries[1].thumbnail = Some(PathBuf::from("/thumbs/b.png"));

        // `b` was replaced on disk (newer mtime), and `c` appeared.
        let mut fresh = entries_of("s", &[("/p/a.png", 1), ("/p/b.png", 9), ("/p/c.png", 1)]);
        fresh[1].size_bytes = Some(99);
        assert!(sync_source(&mut entries, "s", fresh));

        let a = entries.iter().find(|e| e.name == "a").unwrap();
        assert!(a.favorite, "the edit survives the rescan");
        assert!(a.thumbnail.is_some(), "an unchanged file keeps its thumbnail");
        let b = entries.iter().find(|e| e.name == "b").unwrap();
        assert!(b.thumbnail.is_none(), "a replaced file is re-thumbnailed");
        assert_eq!((b.added, b.size_bytes), (9, Some(99)));
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn events_matter_only_for_media_inside_the_folder() {
        let flat = link(Path::new("/pics"), false);
        let deep = link(Path::new("/pics"), true);
        assert!(event_matters(&flat, Path::new("/pics/new.jpg")));
        assert!(event_matters(&flat, Path::new("/pics/clip.MP4")));
        assert!(!event_matters(&flat, Path::new("/pics/notes.txt")));
        assert!(!event_matters(&flat, Path::new("/pics/.hidden.jpg")));
        assert!(!event_matters(&flat, Path::new("/pics/sub/deep.jpg")));
        assert!(!event_matters(&flat, Path::new("/elsewhere/a.jpg")));
        assert!(!event_matters(&flat, Path::new("/pics-other/a.jpg")));
        // The root itself: gone, renamed, unmounted.
        assert!(event_matters(&flat, Path::new("/pics")));

        // A recursive link also cares about folders (their extension says nothing).
        assert!(event_matters(&deep, Path::new("/pics/sub")));
        assert!(event_matters(&deep, Path::new("/pics/sub/deep.jpg")));
        assert!(!event_matters(&deep, Path::new("/pics/.Trash/x.jpg")));
        assert!(!event_matters(&deep, Path::new("/pics/a/b/c/d/e/f.jpg")));
    }

    #[test]
    fn links_overlap_when_they_would_share_files() {
        let flat = link(Path::new("/pics"), false);
        let deep = link(Path::new("/pics"), true);
        assert!(overlaps(&flat, Path::new("/pics"), false));
        assert!(!overlaps(&flat, Path::new("/pics/sub"), false), "flat link does not see sub");
        assert!(overlaps(&deep, Path::new("/pics/sub"), false));
        assert!(overlaps(&flat, Path::new("/"), true), "a recursive parent takes it in");
        assert!(!overlaps(&flat, Path::new("/pics2"), true));
    }

    #[test]
    fn reachability_only_changes_after_a_first_scan() {
        let mut l = link(Path::new("/d"), false);
        l.status = LinkStatus::Scanning;
        assert!(!reachability_changed(&l, true) && !reachability_changed(&l, false));
        l.status = LinkStatus::Ready;
        assert!(reachability_changed(&l, false) && !reachability_changed(&l, true));
        l.status = LinkStatus::Missing;
        assert!(reachability_changed(&l, true) && !reachability_changed(&l, false));
    }
}

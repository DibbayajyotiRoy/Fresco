use std::collections::HashMap;
use std::path::Path;

use crate::config::{Config, CycleSource, Kind, Wallpaper};
use crate::gui::library::{self, LibraryEntry};
use crate::gui::window;
use crate::media;

/// Pick the next wallpaper from the daemon-side auto-cycle pool.
///
/// Returns `None` when auto-cycling is disabled, the interval is zero, or the
/// pool is empty / contains only the current item (in which case there is
/// nothing to switch to).
pub fn next_wallpaper(config: &Config, current: &Wallpaper) -> Option<Wallpaper> {
    if !config.auto_cycle.enabled || config.auto_cycle.interval_min == 0 {
        return None;
    }
    let pool = pool(&config.auto_cycle.source);
    let current = pool.iter().position(|w| w.same_media(current));
    let slot = pick(
        pool.len(),
        current,
        config.auto_cycle.shuffle,
        random_usize(),
    )?;
    Some(pool[slot].clone())
}

/// Every wallpaper a source offers, in the order it cycles: the library in the
/// gallery's display order (broken entries left out), a collection in manual
/// order, a folder sorted by path (subfolders too when its linked section is
/// recursive).
pub fn pool(source: &CycleSource) -> Vec<Wallpaper> {
    match source {
        CycleSource::Library => library_pool(),
        CycleSource::Collection(id) => collection_pool(id),
        CycleSource::Folder(path) => folder_pool(path),
    }
}

/// [`pool`] narrowed to what a day/night slot can show: single image or video
/// files (the scheduler applies a path, not a playlist or slideshow).
pub fn slot_pool(source: &CycleSource) -> Vec<Wallpaper> {
    pool(source)
        .into_iter()
        .filter(|w| matches!(w.kind, Kind::Image | Kind::Video))
        .collect()
}

/// A random wallpaper from a day/night slot's pool, or None when it is empty.
pub fn random_from(source: &CycleSource) -> Option<Wallpaper> {
    let pool = slot_pool(source);
    if pool.is_empty() {
        return None;
    }
    let i = random_usize() % pool.len();
    pool.into_iter().nth(i)
}

fn library_pool() -> Vec<Wallpaper> {
    let Ok(mut entries) = library::load_entries() else {
        return Vec::new();
    };
    for e in &mut entries {
        e.check_health();
    }
    let collections = library::load_collections().unwrap_or_default();
    crate::gui::linked::append_all(&mut entries, &collections);

    let index: HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    window::display_order(&entries, &collections, library::load_view())
        .iter()
        .filter_map(|id| index.get(id.as_str()).copied())
        .filter(|&i| !entries[i].broken)
        .map(|i| entries[i].to_wallpaper())
        .collect()
}

fn collection_pool(id: &str) -> Vec<Wallpaper> {
    let Ok(mut entries) = library::load_entries() else {
        return Vec::new();
    };
    for e in &mut entries {
        e.check_health();
    }
    let mut refs: Vec<&LibraryEntry> = entries
        .iter()
        .filter(|e| e.collection.as_deref() == Some(id) && !e.broken)
        .collect();
    library::sort_entries(&mut refs, library::SortMode::Manual);
    refs.into_iter().map(LibraryEntry::to_wallpaper).collect()
}

fn folder_pool(folder: &Path) -> Vec<Wallpaper> {
    // A linked section set to "include subfolders" shows its subfolders too,
    // so the cycle pool must match. The GUI hands us the link's exact path.
    let recursive = library::load_collections()
        .unwrap_or_default()
        .iter()
        .any(|c| {
            c.link
                .as_ref()
                .is_some_and(|l| l.recursive && l.path == folder)
        });
    let mut files = media::scan(folder, recursive, &media::is_supported);
    files.sort();
    files.iter().map(|p| wallpaper_from_file(p)).collect()
}

fn wallpaper_from_file(path: &Path) -> Wallpaper {
    let kind = if media::is_video(path) {
        Kind::Video
    } else {
        Kind::Image
    };
    Wallpaper {
        kind,
        path: Some(path.to_path_buf()),
        ..Wallpaper::default()
    }
}

/// Choose the next slot. `rand` is only used in shuffle mode; passing 0 is
/// fine for deterministic sequential advancement.
fn pick(len: usize, current: Option<usize>, shuffle: bool, rand: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(if shuffle {
        if len == 1 {
            0
        } else {
            // Draw from the other `len - 1` slots, skipping the current one.
            let c = current.unwrap_or(0);
            let r = rand % (len - 1);
            if r >= c {
                r + 1
            } else {
                r
            }
        }
    } else {
        match current {
            Some(c) => (c + 1) % len,
            None => 0,
        }
    })
}

fn random_usize() -> usize {
    use std::hash::BuildHasher;
    std::collections::hash_map::RandomState::new().hash_one(0u8) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auto_cycle(source: CycleSource, shuffle: bool) -> Config {
        use crate::config::AutoCycle;
        Config {
            auto_cycle: AutoCycle {
                enabled: true,
                interval_min: 1,
                shuffle,
                source,
            },
            ..Config::default()
        }
    }

    #[test]
    fn disabled_or_zero_interval_returns_none() {
        let mut c = Config::default();
        c.auto_cycle.enabled = false;
        assert!(next_wallpaper(&c, &Wallpaper::default()).is_none());
        c.auto_cycle.enabled = true;
        c.auto_cycle.interval_min = 0;
        assert!(next_wallpaper(&c, &Wallpaper::default()).is_none());
    }

    #[test]
    fn pick_next_wraps_around_and_starts_at_zero() {
        assert_eq!(pick(3, Some(0), false, 0), Some(1));
        assert_eq!(pick(3, Some(2), false, 0), Some(0));
        assert_eq!(pick(5, None, false, 0), Some(0));
    }

    #[test]
    fn pick_shuffle_never_repeats_current_when_possible() {
        for len in 2..6 {
            for current in 0..len {
                for rand in 0..(len * 4) {
                    let got = pick(len, Some(current), true, rand).unwrap();
                    assert_ne!(got, current, "len {len} current {current} rand {rand}");
                    assert!(got < len);
                }
            }
        }
    }

    #[test]
    fn pick_shuffle_single_item_stays_put() {
        assert_eq!(pick(1, Some(0), true, 0), Some(0));
        assert_eq!(pick(1, None, true, 7), Some(0));
    }

    #[test]
    fn folder_source_cycles_sorted_files() {
        let dir = std::env::temp_dir().join(format!("fresco-cycle-folder-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.png"), b"x").unwrap();
        std::fs::write(dir.join("a.mp4"), b"x").unwrap();

        let cfg = auto_cycle(CycleSource::Folder(dir.clone()), false);
        let first = next_wallpaper(&cfg, &Wallpaper::default()).unwrap();
        assert_eq!(first.effective_path(), Some(dir.join("a.mp4").as_path()));
        assert_eq!(first.kind, Kind::Video);

        let second = next_wallpaper(&cfg, &first).unwrap();
        assert_eq!(second.effective_path(), Some(dir.join("b.png").as_path()));
        assert_eq!(second.kind, Kind::Image);

        let wrap = next_wallpaper(&cfg, &second).unwrap();
        assert_eq!(wrap.effective_path(), first.effective_path());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn folder_shuffle_picks_a_different_file() {
        let dir = std::env::temp_dir().join(format!("fresco-cycle-shuffle-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.mp4"), b"x").unwrap();
        std::fs::write(dir.join("b.png"), b"x").unwrap();

        let cfg = auto_cycle(CycleSource::Folder(dir.clone()), true);
        let a = wallpaper_from_file(&dir.join("a.mp4"));
        let got = next_wallpaper(&cfg, &a).unwrap();
        assert_eq!(got.effective_path(), Some(dir.join("b.png").as_path()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn linked_recursive_folder_cycles_subfolders_too() {
        temp_library(|dir| {
            let lib = dir.join("fresco/library");
            std::fs::create_dir_all(&lib).unwrap();
            let folder = dir.join("pics");
            std::fs::create_dir_all(folder.join("sub")).unwrap();
            std::fs::write(folder.join("a.png"), b"x").unwrap();
            std::fs::write(folder.join("sub/b.png"), b"x").unwrap();
            std::fs::write(lib.join("entries.json"), "[]").unwrap();
            std::fs::write(lib.join("view.json"), "{}").unwrap();

            let link = |recursive| {
                vec![library::Collection {
                    id: "c1".into(),
                    name: "Pics".into(),
                    position: 0,
                    link: Some(library::Link {
                        path: folder.clone(),
                        recursive,
                        status: Default::default(),
                    }),
                }]
            };
            let cfg = auto_cycle(CycleSource::Folder(folder.clone()), false);

            // Not recursive: only the top-level file.
            std::fs::write(
                lib.join("collections.json"),
                serde_json::to_string(&link(false)).unwrap(),
            )
            .unwrap();
            let only = next_wallpaper(&cfg, &Wallpaper::default()).unwrap();
            assert_eq!(only.effective_path(), Some(folder.join("a.png").as_path()));
            assert_eq!(
                next_wallpaper(&cfg, &only).unwrap().effective_path(),
                only.effective_path()
            );

            // Recursive: both files, in sorted path order.
            std::fs::write(
                lib.join("collections.json"),
                serde_json::to_string(&link(true)).unwrap(),
            )
            .unwrap();
            let first = next_wallpaper(&cfg, &Wallpaper::default()).unwrap();
            assert_eq!(first.effective_path(), Some(folder.join("a.png").as_path()));
            let second = next_wallpaper(&cfg, &first).unwrap();
            assert_eq!(
                second.effective_path(),
                Some(folder.join("sub/b.png").as_path())
            );
        });
    }

    #[test]
    fn folder_source_ignores_unsupported_files() {
        let dir = std::env::temp_dir().join(format!("fresco-cycle-skip-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();

        let cfg = auto_cycle(CycleSource::Folder(dir.clone()), false);
        assert!(next_wallpaper(&cfg, &Wallpaper::default()).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    static LIB_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn temp_library<F: FnOnce(&Path)>(f: F) {
        let _guard = LIB_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("fresco-cycle-lib-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_DATA_HOME", &dir);
        f(&dir);
        std::env::remove_var("XDG_DATA_HOME");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn collection_source_filters_by_folder_and_cycles_in_manual_order() {
        temp_library(|dir| {
            let lib = dir.join("fresco/library");
            std::fs::create_dir_all(&lib).unwrap();
            let media = dir.join("media");
            std::fs::create_dir_all(&media).unwrap();
            let a_path = media.join("a.mp4");
            let b_path = media.join("b.mp4");
            let other_path = media.join("other.mp4");
            std::fs::write(&a_path, b"x").unwrap();
            std::fs::write(&b_path, b"x").unwrap();
            std::fs::write(&other_path, b"x").unwrap();

            let col_id = "col-1";
            let mut a = LibraryEntry::new_video(a_path.clone());
            a.collection = Some(col_id.to_string());
            a.order = 1;
            let mut b = LibraryEntry::new_video(b_path.clone());
            b.collection = Some(col_id.to_string());
            b.order = 0;
            let other = LibraryEntry::new_video(other_path);

            std::fs::write(
                lib.join("entries.json"),
                serde_json::to_string_pretty(&vec![a.clone(), b.clone(), other]).unwrap(),
            )
            .unwrap();
            std::fs::write(lib.join("collections.json"), "[]").unwrap();
            std::fs::write(lib.join("view.json"), "{}").unwrap();

            let cfg = auto_cycle(CycleSource::Collection(col_id.to_string()), false);
            let current = b.to_wallpaper();
            let next = next_wallpaper(&cfg, &current).unwrap();
            assert_eq!(next.effective_path(), Some(a_path.as_path()));
            let wrap = next_wallpaper(&cfg, &next).unwrap();
            assert_eq!(wrap.effective_path(), Some(b_path.as_path()));
        });
    }

    #[test]
    fn library_source_wraps_through_the_whole_library() {
        temp_library(|dir| {
            let lib = dir.join("fresco/library");
            std::fs::create_dir_all(&lib).unwrap();
            let media = dir.join("media");
            std::fs::create_dir_all(&media).unwrap();
            let a_path = media.join("a.mp4");
            let b_path = media.join("b.png");
            std::fs::write(&a_path, b"x").unwrap();
            std::fs::write(&b_path, b"x").unwrap();

            let a = LibraryEntry::new_video(a_path.clone());
            let b = LibraryEntry::new_image(b_path.clone());
            std::fs::write(
                lib.join("entries.json"),
                serde_json::to_string_pretty(&vec![a.clone(), b.clone()]).unwrap(),
            )
            .unwrap();
            std::fs::write(lib.join("collections.json"), "[]").unwrap();
            std::fs::write(lib.join("view.json"), "{}").unwrap();

            let cfg = auto_cycle(CycleSource::Library, false);
            // With no collections the library is grouped Images -> Videos -> GIFs.
            let first = next_wallpaper(&cfg, &Wallpaper::default()).unwrap();
            assert_eq!(first.effective_path(), Some(b_path.as_path()));
            let second = next_wallpaper(&cfg, &first).unwrap();
            assert_eq!(second.effective_path(), Some(a_path.as_path()));
            let wrap = next_wallpaper(&cfg, &second).unwrap();
            assert_eq!(wrap.effective_path(), Some(b_path.as_path()));
        });
    }
}

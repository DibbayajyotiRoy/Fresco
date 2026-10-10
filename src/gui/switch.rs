//! `fresco next` / `prev` / `random`: switch the wallpaper from a keybinding.
//!
//! Runs from `cli::dispatch`, before GTK exists, so a hotkey costs a config
//! read and one daemon round trip. It applies a library entry exactly like
//! the gallery's "Set as wallpaper" (`apply_entry_by_idx`): `to_wallpaper`,
//! `enabled = true`, save, then `daemon_ctl::apply_blocking`, which starts
//! `frescod` when it is not running.

use std::hash::BuildHasher;

use anyhow::{bail, Result};

use super::{daemon_ctl, library, window};
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Next,
    Prev,
    Random,
}

/// Which of `len` pool slots to switch to. `current` is the slot now playing,
/// if it is in the pool; `rand` is any random number. Next/prev wrap around;
/// random never repeats `current` unless it is the only choice.
fn pick(len: usize, current: Option<usize>, step: Step, rand: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match (step, current) {
        (Step::Next, Some(c)) => (c + 1) % len,
        (Step::Next, None) => 0,
        (Step::Prev, Some(c)) => (c + len - 1) % len,
        (Step::Prev, None) => len - 1,
        // Draw from the other `len - 1` slots, skipping over `c`.
        (Step::Random, Some(c)) if len > 1 => {
            let r = rand % (len - 1);
            if r >= c {
                r + 1
            } else {
                r
            }
        }
        (Step::Random, _) => rand % len,
    })
}

/// The view to cycle through. Applying an entry stamps its `last_used`, so
/// under "Recently used" it would jump to the top and `next` would ping-pong
/// between the top two forever; use the stable manual order instead (the open
/// folder still scopes the pool).
fn cycle_view(mut view: library::LibraryView) -> library::LibraryView {
    if view.sort == library::SortMode::RecentlyUsed {
        view.sort = library::SortMode::Manual;
    }
    view
}

/// Entry point for the CLI: exit code 0 and the applied name on stdout, or 1
/// and a message on stderr. A library with nothing else to switch to is not an
/// error: exit 0 with a note on stderr.
pub fn run(step: Step) -> i32 {
    match switch(step) {
        Ok(Some(name)) => {
            println!("Applied “{name}”");
            0
        }
        Ok(None) => {
            eprintln!("note: your library has only this one wallpaper, nothing to switch to");
            0
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    }
}

/// `Ok(None)`: the only candidate is already playing, so nothing was touched.
fn switch(step: Step) -> Result<Option<String>> {
    // Never fall back to defaults here: saving them would clobber a config
    // that merely failed to parse.
    let mut config = Config::load()?;
    let mut entries = library::load_entries()?;
    let collections = library::load_collections().unwrap_or_default();
    // `broken` is stored from whenever the GUI last ran; the GUI re-checks on
    // every start, so do the same or a file deleted since then gets picked.
    for e in &mut entries {
        e.check_health();
    }

    // The library as the gallery lists it, minus entries whose file is gone.
    let pool: Vec<usize> =
        window::display_order(&entries, &collections, cycle_view(library::load_view()))
            .iter()
            .filter_map(|id| entries.iter().position(|e| &e.id == id))
            .filter(|&i| !entries[i].broken)
            .collect();
    let current = pool
        .iter()
        .position(|&i| window::entry_matches_wallpaper(&entries[i], &config.wallpaper));
    let rand = std::collections::hash_map::RandomState::new().hash_one(0u8) as usize;
    let Some(slot) = pick(pool.len(), current, step, rand) else {
        bail!("no wallpapers to switch between — add some in the Fresco app first");
    };
    // Only a one-item pool picks the current item. Re-applying it would
    // restart the renderer for nothing; if it is stopped or the daemon is
    // down, falling through starts it.
    if current == Some(slot) && config.enabled && crate::ipc::daemon_alive() {
        return Ok(None);
    }

    let entry = &mut entries[pool[slot]];
    entry.touch();
    config.wallpaper = entry.to_wallpaper();
    config.enabled = true;
    let name = entry.name.clone();
    config.save()?;
    library::save_entries(&entries).ok();
    daemon_ctl::apply_blocking()?;
    Ok(Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_and_prev_wrap_around() {
        assert_eq!(pick(3, Some(0), Step::Next, 0), Some(1));
        assert_eq!(pick(3, Some(2), Step::Next, 0), Some(0));
        assert_eq!(pick(3, Some(1), Step::Prev, 0), Some(0));
        assert_eq!(pick(3, Some(0), Step::Prev, 0), Some(2));
    }

    #[test]
    fn without_a_current_item_next_starts_and_prev_ends() {
        assert_eq!(pick(4, None, Step::Next, 0), Some(0));
        assert_eq!(pick(4, None, Step::Prev, 0), Some(3));
    }

    #[test]
    fn random_never_repeats_the_current_item() {
        for len in 2..6 {
            for current in 0..len {
                for rand in 0..(len * 3) {
                    let got = pick(len, Some(current), Step::Random, rand).unwrap();
                    assert!(got < len && got != current, "len {len} cur {current}");
                }
            }
        }
    }

    #[test]
    fn random_reaches_every_other_item() {
        let seen: std::collections::HashSet<_> = (0..4)
            .map(|r| pick(4, Some(2), Step::Random, r).unwrap())
            .collect();
        assert_eq!(seen, [0, 1, 3].into());
    }

    /// Under "Recently used", applying `b` must not reorder the cycle, or
    /// `next` would bounce between the top two entries.
    #[test]
    fn recently_used_sort_does_not_reorder_the_cycle() {
        use std::path::PathBuf;
        let mut a = library::LibraryEntry::new_video(PathBuf::from("/a.mp4"));
        let mut b = library::LibraryEntry::new_video(PathBuf::from("/b.mp4"));
        a.last_used = 5;
        b.last_used = 1;
        let view = library::LibraryView {
            sort: library::SortMode::RecentlyUsed,
            collection: None,
        };
        let order =
            |e: &[library::LibraryEntry]| window::display_order(e, &[], cycle_view(view.clone()));
        let before = order(&[a.clone(), b.clone()]);
        assert_eq!(before, vec![a.id.clone(), b.id.clone()]);
        b.touch(); // what applying `b` does
        assert_eq!(order(&[a.clone(), b.clone()]), before);
        // The raw view would have flipped it, which is the bug.
        let raw = window::display_order(&[a.clone(), b.clone()], &[], view.clone());
        assert_eq!(raw, vec![b.id, a.id]);
    }

    #[test]
    fn single_item_and_empty_pools() {
        for step in [Step::Next, Step::Prev, Step::Random] {
            assert_eq!(pick(1, Some(0), step, 7), Some(0));
            assert_eq!(pick(0, None, step, 7), None);
        }
        assert!(pick(3, None, Step::Random, 8).unwrap() < 3);
    }
}

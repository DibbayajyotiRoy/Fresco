//! Background import queue: thumbnails + metadata for freshly added entries.
//!
//! A 200-image folder used to be one thread doing every thumbnail, then one
//! doing every ffprobe, each reporting back exactly once — so the gallery sat
//! on placeholder slivers until the very end, and the metadata (which lands
//! first) painted "1080p · 2 MB" on cards that still had no picture. Here a
//! small worker pool finishes entries one at a time and the UI fills each card
//! in place as its result arrives, with a progress row and a Cancel button.
//!
//! An entry is *pending* from the moment it is queued until its result has
//! been applied (or the run was cancelled). Pending cards are inert — see
//! [`is_pending`], which the card handlers check at event time so a card can
//! come alive without being rebuilt.
//!
//! Everything GTK-facing lives in `thread_local`s: it is all main-thread
//! state, and it keeps `AppState` (and the window construction code) out of
//! this feature's way.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use gtk4::{gio, glib, prelude::*};

use super::library::{self, save_entries, LibraryEntry, MediaMeta};
use super::window::AppState;
use crate::config::Kind;
use crate::{t, tf};

/// Save `entries.json` after this many finished items, so a crash mid-import
/// loses at most a couple dozen thumbnails' bookkeeping (the PNGs stay on disk).
const CHECKPOINT_EVERY: usize = 25;

/// ...but a long run checkpoints at most this many times. Each checkpoint
/// rewrites the whole of `entries.json` on the UI thread, so a fixed cadence made
/// a 4,000-item import write the (growing) file 160 times.
const MAX_CHECKPOINTS: usize = 40;

/// Only the first this-many items of a run get an eager hover-preview proxy
/// (see `preview_proxy::prefetch`). `prefetch` caps how many are *waiting*, but
/// the worker drains that queue as fast as it fills, so without a cap on the
/// run a 4,000-video import kept ffmpeg transcoding for hours. The rest are
/// made on first hover instead.
const PREFETCH_FIRST: usize = 24;

// ─── Pure logic ──────────────────────────────────────────────────────────────

/// In which order queued entries are worked on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// The order given (folder listing / picker order). The default: cards
    /// fill in top-to-bottom, the way they sit in the grid, which reads as
    /// steady progress rather than random pop-in.
    AsGiven,
    /// Smallest file first. File size is only a weak proxy for decode cost
    /// (a tiny PNG can be huge in pixels), so this is opt-in.
    #[allow(dead_code)]
    SmallestFirst,
}

/// Work order for `(id, size_bytes)` items: duplicates dropped (first
/// occurrence wins), then ordered per `order`. Stable — equal sizes keep the
/// order they were given in.
pub fn plan(items: &[(String, u64)], order: Order) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut unique: Vec<&(String, u64)> = items
        .iter()
        .filter(|(id, _)| seen.insert(id.as_str()))
        .collect();
    if order == Order::SmallestFirst {
        unique.sort_by_key(|(_, size)| *size);
    }
    unique.into_iter().map(|(id, _)| id.clone()).collect()
}

/// Worker threads for the pool: one core stays free for the UI, at most four
/// (more just thrash the disk and the little RAM a low-end box has).
pub fn worker_count(parallelism: usize) -> usize {
    parallelism.saturating_sub(1).clamp(1, 4)
}

/// Whether the item that just finished (the `finished`-th of its run) should
/// queue an eager hover-preview proxy.
pub fn should_prefetch(finished: usize) -> bool {
    finished <= PREFETCH_FIRST
}

/// Items between checkpoints for a run of `total`: [`CHECKPOINT_EVERY`], spread
/// out for a run long enough to exceed [`MAX_CHECKPOINTS`].
pub fn checkpoint_every(total: usize) -> usize {
    (total / MAX_CHECKPOINTS).max(CHECKPOINT_EVERY)
}

/// Whether a run that has finished `finished` items should checkpoint now.
pub fn should_checkpoint(finished: usize, every: usize) -> bool {
    every > 0 && finished > 0 && finished.is_multiple_of(every)
}

/// A card takes clicks, hover actions and menus unless its entry is pending.
pub fn card_interactive(pending: &HashSet<String>, id: &str) -> bool {
    !pending.contains(id)
}

/// How far a run has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    /// Items that produced what they needed.
    pub done: usize,
    /// Items that finished without a thumbnail (unreadable / no decoder).
    pub failed: usize,
    pub total: usize,
}

impl Progress {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            ..Self::default()
        }
    }

    pub fn record(&mut self, ok: bool) {
        if ok {
            self.done += 1;
        } else {
            self.failed += 1;
        }
    }

    /// Items handled so far, successful or not.
    pub fn finished(&self) -> usize {
        self.done + self.failed
    }

    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        (self.finished() as f64 / self.total as f64).min(1.0)
    }

    pub fn is_finished(&self) -> bool {
        self.finished() >= self.total
    }
}

// ─── Pending set + in-place card updates ─────────────────────────────────────

/// The widgets of a still-loading card that a finished item updates in place.
struct CardParts {
    overlay: glib::WeakRef<gtk4::Overlay>,
    pic: glib::WeakRef<gtk4::Picture>,
    placeholder: glib::WeakRef<gtk4::Box>,
    meta: glib::WeakRef<gtk4::Label>,
}

struct Ui {
    row: gtk4::Box,
    label: gtk4::Label,
    bar: gtk4::ProgressBar,
}

struct ActiveRun {
    cancel: Arc<AtomicBool>,
    ids: Vec<String>,
}

struct Queued {
    ids: Vec<String>,
    on_finish: Box<dyn FnOnce(bool)>,
}

thread_local! {
    static PENDING: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    static CARDS: RefCell<HashMap<String, CardParts>> = RefCell::new(HashMap::new());
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
    static RUN: RefCell<Option<ActiveRun>> = const { RefCell::new(None) };
    static BACKLOG: RefCell<VecDeque<Queued>> = const { RefCell::new(VecDeque::new()) };
}

/// True while `id`'s import work is outstanding. Checked at event time by the
/// card handlers, so finishing an item makes its card live without a rebuild.
pub fn is_pending(id: &str) -> bool {
    PENDING.with(|p| !card_interactive(&p.borrow(), id))
}

/// Called by `build_library_card` for a card that is pending, so a finished
/// item can swap in its thumbnail and meta line without rebuilding the grid.
pub fn register_card(
    id: &str,
    overlay: &gtk4::Overlay,
    pic: &gtk4::Picture,
    placeholder: &gtk4::Box,
    meta: &gtk4::Label,
) {
    CARDS.with(|c| {
        c.borrow_mut().insert(
            id.to_string(),
            CardParts {
                overlay: overlay.downgrade(),
                pic: pic.downgrade(),
                placeholder: placeholder.downgrade(),
                meta: meta.downgrade(),
            },
        );
    });
}

fn unpend(id: &str) {
    PENDING.with(|p| p.borrow_mut().remove(id));
}

/// Fill a registered card from a finished item (no-op when the card was since
/// rebuilt or destroyed — the rebuilt one reads the updated entry).
fn update_card(id: &str, thumb: Option<&std::path::Path>, meta_line: Option<&str>) {
    let Some(parts) = CARDS.with(|c| c.borrow_mut().remove(id)) else {
        return;
    };
    if let Some(overlay) = parts.overlay.upgrade() {
        overlay.remove_css_class("wp-loading");
    }
    if let (Some(thumb), Some(pic)) = (thumb, parts.pic.upgrade()) {
        pic.add_css_class("wp-thumb");
        // Same fade-in as a card built with its thumbnail (see build_library_card).
        pic.add_css_class("thumb-loading");
        pic.set_file(Some(&gio::File::for_path(thumb)));
        let pic2 = pic.clone();
        glib::idle_add_local_once(move || pic2.remove_css_class("thumb-loading"));
        if let Some(ph) = parts.placeholder.upgrade() {
            ph.set_visible(false);
        }
    }
    if let (Some(line), Some(label)) = (meta_line, parts.meta.upgrade()) {
        label.set_text(line);
        label.set_visible(true);
    }
}

// ─── Progress row ────────────────────────────────────────────────────────────

/// Build the (initially hidden) progress row into `slot`. Called once, from
/// the library view, above the scrolling grid.
pub fn install_progress_row(slot: &gtk4::Box) {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    row.set_margin_start(16);
    row.set_margin_end(16);
    row.set_margin_top(6);
    row.set_margin_bottom(2);
    row.set_visible(false);
    let label = gtk4::Label::new(None);
    label.add_css_class("dialog-sub");
    label.set_xalign(0.0);
    let bar = gtk4::ProgressBar::new();
    bar.set_hexpand(true);
    bar.set_valign(gtk4::Align::Center);
    let cancel = gtk4::Button::with_label(t!("Cancel"));
    cancel.add_css_class("flat");
    cancel.connect_clicked(|_| cancel_all());
    row.append(&label);
    row.append(&bar);
    row.append(&cancel);
    slot.append(&row);
    UI.with(|u| *u.borrow_mut() = Some(Ui { row, label, bar }));
}

fn show_progress(p: &Progress) {
    UI.with(|u| {
        if let Some(ui) = u.borrow().as_ref() {
            ui.label.set_text(&tf!(
                "Importing {done} of {total}",
                "done" => p.finished().to_string(),
                "total" => p.total.to_string()
            ));
            ui.bar.set_fraction(p.fraction());
            ui.row.set_visible(true);
        }
    });
}

fn hide_progress() {
    UI.with(|u| {
        if let Some(ui) = u.borrow().as_ref() {
            ui.row.set_visible(false);
        }
    });
}

// ─── Runtime ─────────────────────────────────────────────────────────────────

/// One finished item, sent from a worker as soon as it is ready.
struct Done {
    id: String,
    thumbnail: Option<PathBuf>,
    baked_rotation: Option<u16>,
    meta: Option<MediaMeta>,
    /// Everything this item needed was produced.
    ok: bool,
}

/// Run the calling thread, and every ffmpeg / ffprobe it spawns, at nice 10.
///
/// A big import keeps up to four workers decoding video (ffmpegthumbnailer
/// threads one decoder per core), which on a low-core machine starves the UI and
/// the rest of the desktop for the length of the run. Linux keeps the nice value
/// per thread and children inherit it, so lowering it here covers the helpers
/// without wrapping each command in `nice`.
fn lower_priority() {
    extern "C" {
        fn setpriority(which: i32, who: u32, prio: i32) -> i32;
    }
    // SAFETY: plain syscall wrapper, no pointers. PRIO_PROCESS (0) with id 0 is
    // the calling thread on Linux; failure just leaves the priority unchanged.
    unsafe {
        setpriority(0, 0, 10);
    }
}

/// Thumbnail + metadata for one entry. Still images are decoded once, in
/// process, and yield their size for free; videos/GIFs keep ffmpegthumbnailer
/// and ffprobe.
fn import_one(mut e: LibraryEntry) -> Done {
    let need_thumb = e.thumbnail.is_none();
    let need_probe = e.needs_probe();
    let dims = if need_thumb {
        e.generate_thumbnail_dims()
    } else {
        None
    };
    let meta = if need_probe {
        e.probe_source().map(|src| match dims {
            Some((w, h)) if e.kind == Kind::Image => MediaMeta {
                width: Some(w),
                height: Some(h),
                fps: None,
                size_bytes: std::fs::metadata(&src).ok().map(|m| m.len()),
            },
            _ => library::probe_media(&src),
        })
    } else {
        None
    };
    Done {
        ok: !need_thumb || e.thumbnail.is_some(),
        id: e.id,
        thumbnail: e.thumbnail,
        baked_rotation: e.thumbnail_rotation,
        meta,
    }
}

fn needs_work(e: &LibraryEntry) -> bool {
    e.thumbnail.is_none() || e.needs_probe()
}

/// Queue the given entries. Entries with nothing left to do are skipped;
/// `on_finish(cancelled)` runs when the queued work is over (immediately if
/// there was none), e.g. to show the import toast. If a run is already going
/// this one waits its turn.
pub fn start(
    state: &Rc<RefCell<AppState>>,
    ids: Vec<String>,
    on_finish: impl FnOnce(bool) + 'static,
) {
    let ids = {
        let s = state.borrow();
        let by_id: HashMap<&str, &LibraryEntry> =
            s.entries.iter().map(|e| (e.id.as_str(), e)).collect();
        let wanted: Vec<(String, u64)> = ids
            .into_iter()
            .filter(|id| by_id.get(id.as_str()).is_some_and(|e| needs_work(e)))
            .map(|id| (id, 0))
            .collect();
        plan(&wanted, Order::AsGiven)
    };
    if ids.is_empty() {
        on_finish(false);
        return;
    }
    PENDING.with(|p| p.borrow_mut().extend(ids.iter().cloned()));
    let queued = Queued {
        ids,
        on_finish: Box::new(on_finish),
    };
    if RUN.with(|r| r.borrow().is_some()) {
        BACKLOG.with(|b| b.borrow_mut().push_back(queued));
    } else {
        launch(state, queued);
    }
}

/// Stop taking new items. In-flight ones finish; everything unfinished stays
/// as a placeholder card but becomes interactive (the editor thumbnails it
/// lazily when opened).
fn cancel_all() {
    RUN.with(|r| {
        if let Some(run) = r.borrow().as_ref() {
            run.cancel.store(true, Ordering::Relaxed);
            run.ids.iter().for_each(|id| unpend(id));
        }
    });
    let waiting: Vec<Queued> = BACKLOG.with(|b| b.borrow_mut().drain(..).collect());
    for q in waiting {
        q.ids.iter().for_each(|id| unpend(id));
        (q.on_finish)(true);
    }
    hide_progress();
}

fn launch(state: &Rc<RefCell<AppState>>, queued: Queued) {
    let Queued { ids, on_finish } = queued;
    let jobs: Vec<LibraryEntry> = {
        let s = state.borrow();
        let by_id: HashMap<&str, &LibraryEntry> =
            s.entries.iter().map(|e| (e.id.as_str(), e)).collect();
        ids.iter()
            .filter_map(|id| by_id.get(id.as_str()).map(|e| (*e).clone()))
            .collect()
    };
    let cancel = Arc::new(AtomicBool::new(false));
    RUN.with(|r| {
        *r.borrow_mut() = Some(ActiveRun {
            cancel: cancel.clone(),
            ids: ids.clone(),
        })
    });
    let mut progress = Progress::new(jobs.len());
    show_progress(&progress);

    let (tx, rx) = async_channel::unbounded::<Done>();
    let jobs = Arc::new(jobs);
    let next = Arc::new(AtomicUsize::new(0));
    let workers = worker_count(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2),
    );
    for n in 0..workers.min(jobs.len()) {
        let (tx, jobs, next, cancel) = (tx.clone(), jobs.clone(), next.clone(), cancel.clone());
        let spawned = std::thread::Builder::new()
            .name(format!("fresco-import-{n}"))
            .spawn(move || {
                lower_priority();
                while !cancel.load(Ordering::Relaxed) {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    if tx.send_blocking(import_one(job.clone())).is_err() {
                        break;
                    }
                }
            });
        if let Err(e) = spawned {
            log::warn!("import worker {n} failed to start: {e}");
        }
    }
    drop(tx);

    let state = state.clone();
    glib::spawn_future_local(async move {
        while let Ok(done) = rx.recv().await {
            apply(&state, done, &mut progress);
            if !cancel.load(Ordering::Relaxed) {
                show_progress(&progress);
            }
        }
        // Channel closed: every worker is finished (or was stopped).
        let cancelled = cancel.load(Ordering::Relaxed);
        if !cancelled && !progress.is_finished() {
            log::warn!("import queue ended early: {progress:?}");
        }
        ids.iter().for_each(|id| unpend(id));
        RUN.with(|r| *r.borrow_mut() = None);
        checkpoint(&state);
        hide_progress();
        let refresh = state.borrow().refresh.clone();
        if let Some(r) = refresh {
            r();
        }
        on_finish(cancelled);
        if let Some(next) = BACKLOG.with(|b| b.borrow_mut().pop_front()) {
            launch(&state, next);
        }
    });
}

/// Fold one finished item into the entries and its card; checkpoint every
/// [`CHECKPOINT_EVERY`] items.
fn apply(state: &Rc<RefCell<AppState>>, done: Done, progress: &mut Progress) {
    unpend(&done.id);
    progress.record(done.ok);
    let previews_on = state.borrow().config.hover_previews;
    let meta_line = {
        let mut s = state.borrow_mut();
        match s.entries.iter_mut().find(|e| e.id == done.id) {
            Some(e) => {
                if done.thumbnail.is_some() {
                    e.thumbnail = done.thumbnail.clone();
                    e.thumbnail_rotation = done.baked_rotation;
                }
                if let Some(m) = done.meta {
                    e.width = m.width;
                    e.height = m.height;
                    e.fps = m.fps;
                    e.size_bytes = m.size_bytes;
                }
                // A hover preview of a large video plays a small proxy clip;
                // make it now, at low priority, so the first hover is already
                // alive. Only for the first few items of a run (`PREFETCH_FIRST`),
                // so a big folder gets the rest on first hover instead.
                if previews_on && should_prefetch(progress.finished()) {
                    if let Some(request) = super::preview_proxy::request_for(e) {
                        super::preview_proxy::prefetch(request);
                    }
                }
                e.meta_line()
            }
            None => {
                // Removed while it was queued: don't leave its PNG behind.
                if let Some(t) = &done.thumbnail {
                    std::fs::remove_file(t).ok();
                }
                super::preview_proxy::remove_for(&done.id);
                None
            }
        }
    };
    update_card(&done.id, done.thumbnail.as_deref(), meta_line.as_deref());
    if should_checkpoint(progress.finished(), checkpoint_every(progress.total)) {
        checkpoint(state);
    }
}

/// The one place the queue writes `entries.json`.
fn checkpoint(state: &Rc<RefCell<AppState>>) {
    save_entries(&state.borrow().entries).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(v: &[(&str, u64)]) -> Vec<(String, u64)> {
        v.iter().map(|(i, s)| (i.to_string(), *s)).collect()
    }

    #[test]
    fn plan_keeps_given_order_and_drops_duplicates() {
        let got = plan(
            &items(&[("b", 9), ("a", 1), ("b", 9), ("c", 5)]),
            Order::AsGiven,
        );
        assert_eq!(got, ["b", "a", "c"]);
    }

    #[test]
    fn plan_smallest_first_is_stable() {
        let got = plan(
            &items(&[("big", 100), ("x", 5), ("y", 5), ("mid", 50), ("x", 5)]),
            Order::SmallestFirst,
        );
        assert_eq!(got, ["x", "y", "mid", "big"]);
    }

    #[test]
    fn plan_of_nothing_is_empty() {
        assert!(plan(&[], Order::AsGiven).is_empty());
    }

    #[test]
    fn progress_tracks_done_and_failed() {
        let mut p = Progress::new(4);
        assert!(!p.is_finished());
        assert_eq!(p.fraction(), 0.0);
        p.record(true);
        p.record(false);
        assert_eq!((p.done, p.failed, p.finished()), (1, 1, 2));
        assert_eq!(p.fraction(), 0.5);
        p.record(true);
        p.record(true);
        assert!(p.is_finished());
        assert_eq!(p.fraction(), 1.0);
    }

    #[test]
    fn empty_progress_is_finished_and_full() {
        let p = Progress::new(0);
        assert!(p.is_finished());
        assert_eq!(p.fraction(), 1.0);
    }

    #[test]
    fn worker_count_leaves_a_core_and_caps_at_four() {
        assert_eq!(worker_count(0), 1);
        assert_eq!(worker_count(1), 1);
        assert_eq!(worker_count(2), 1);
        assert_eq!(worker_count(4), 3);
        assert_eq!(worker_count(5), 4);
        assert_eq!(worker_count(64), 4);
    }

    #[test]
    fn only_the_first_items_of_a_run_prefetch_a_proxy() {
        assert!(should_prefetch(1));
        assert!(should_prefetch(PREFETCH_FIRST));
        assert!(!should_prefetch(PREFETCH_FIRST + 1));
        assert!(!should_prefetch(4000));
    }

    #[test]
    fn long_runs_checkpoint_less_often() {
        assert_eq!(checkpoint_every(0), CHECKPOINT_EVERY);
        assert_eq!(checkpoint_every(200), CHECKPOINT_EVERY);
        assert_eq!(checkpoint_every(1000), CHECKPOINT_EVERY);
        assert_eq!(checkpoint_every(4000), 100);
        // However long the run, no more than MAX_CHECKPOINTS writes.
        for total in [1000, 4000, 25_000] {
            assert!(total / checkpoint_every(total) <= MAX_CHECKPOINTS);
        }
    }

    #[test]
    fn checkpoints_every_n_items_but_not_at_zero() {
        assert!(!should_checkpoint(0, 25));
        assert!(!should_checkpoint(24, 25));
        assert!(should_checkpoint(25, 25));
        assert!(should_checkpoint(50, 25));
        assert!(!should_checkpoint(10, 0));
    }

    #[test]
    fn pending_cards_are_not_interactive() {
        let pending: HashSet<String> = ["a".to_string()].into();
        assert!(!card_interactive(&pending, "a"));
        assert!(card_interactive(&pending, "b"));
        assert!(card_interactive(&HashSet::new(), "a"));
    }
}

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
//! A linked folder's files are not queued here: their cards ask for a thumbnail
//! themselves, when the lazy grid builds them ([`request_thumbnail`]), so linking
//! a 4,000-file folder costs the cards on screen, not 4,000 thumbnails.
//!
//! Everything GTK-facing lives in `thread_local`s: it is all main-thread
//! state, and it keeps `AppState` (and the window construction code) out of
//! this feature's way.

use std::cell::{Cell, RefCell};
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

/// Thumbnails made at once for linked-folder cards. They trail the scroll
/// position rather than finish a batch, and each video one is an ffmpeg decode
/// (the eager run peaked at 458 MB with four), so two is plenty.
const LAZY_WORKERS: usize = 2;

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

/// Worker threads for this machine.
fn available_workers() -> usize {
    worker_count(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2),
    )
}

/// Whether a card should ask for its own thumbnail as it is built: a linked
/// folder's file with none yet. (A plain import is thumbnailed up front by the
/// queue, and a card still waiting on that must not be done twice.)
pub fn wants_lazy_thumbnail(id: &str, has_thumbnail: bool, pending: bool) -> bool {
    !has_thumbnail && !pending && super::linked::is_linked_id(id)
}

/// Thumbnails requested by cards as they are built: which are waiting, which
/// are being made, which failed.
#[derive(Debug)]
struct LazyQueue {
    limit: usize,
    queued: VecDeque<String>,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
}

impl LazyQueue {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            queued: VecDeque::new(),
            in_flight: HashSet::new(),
            failed: HashSet::new(),
        }
    }

    /// Queue `id`; false when it is already queued or running, or failed
    /// before (a card rebuilt on every scroll must not retry a broken file).
    // ponytail: a failure is remembered for the session; a file replaced since
    // gets its thumbnail after a restart.
    fn request(&mut self, id: &str) -> bool {
        let known = self.failed.contains(id)
            || self.in_flight.contains(id)
            || self.queued.iter().any(|q| q == id);
        if !known {
            self.queued.push_back(id.to_string());
        }
        !known
    }

    /// The next id to start, oldest first. Ids whose card is gone (`alive`
    /// says no: scrolled away) are dropped unmade; they are requested again
    /// if the card comes back. `None` at the limit, or when nothing is left.
    // ponytail: oldest first fills a fresh view top to bottom; after a long
    // scroll the newest cards wait behind older ones that are still alive.
    fn next(&mut self, alive: impl Fn(&str) -> bool) -> Option<String> {
        while self.in_flight.len() < self.limit {
            let id = self.queued.pop_front()?;
            if alive(&id) {
                self.in_flight.insert(id.clone());
                return Some(id);
            }
        }
        None
    }

    fn finish(&mut self, id: &str, ok: bool) {
        self.in_flight.remove(id);
        if !ok {
            self.failed.insert(id.to_string());
        }
    }
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
    static LAZY: RefCell<LazyQueue> =
        RefCell::new(LazyQueue::new(available_workers().min(LAZY_WORKERS)));
    static LAZY_SCHEDULED: Cell<bool> = const { Cell::new(false) };
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
    for n in 0..available_workers().min(jobs.len()) {
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
    let previews_on = state.borrow().config.effective_hover_previews();
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

// ─── Lazy thumbnails (linked folders) ────────────────────────────────────────

/// Whether the card registered for `id` is still in the widget tree. One that
/// scrolled away is unparented; its [`CARDS`] entry is dropped here, since
/// nothing will fill it.
fn card_alive(id: &str) -> bool {
    CARDS.with(|c| {
        let mut c = c.borrow_mut();
        let alive = c
            .get(id)
            .and_then(|p| p.overlay.upgrade())
            .is_some_and(|o| o.root().is_some());
        if !alive {
            c.remove(id);
        }
        alive
    })
}

/// Make the thumbnail of a card that was just built without one (see
/// [`wants_lazy_thumbnail`]) and fill the card in place when it is ready. The
/// card must be [`register_card`]ed first. Work starts from an idle callback,
/// once the card has been added to its slot.
pub fn request_thumbnail(state: &Rc<RefCell<AppState>>, id: &str) {
    if LAZY.with(|l| l.borrow_mut().request(id)) && !LAZY_SCHEDULED.replace(true) {
        let state = state.clone();
        glib::idle_add_local_once(move || {
            LAZY_SCHEDULED.set(false);
            pump_lazy(&state);
        });
    }
}

/// Start requested thumbnails until the concurrency limit is reached.
fn pump_lazy(state: &Rc<RefCell<AppState>>) {
    while let Some(id) = LAZY.with(|l| l.borrow_mut().next(card_alive)) {
        let entry = state.borrow().entries.iter().find(|e| e.id == id).cloned();
        let Some(mut entry) = entry else {
            // Unlinked since: nothing to make, nothing to retry.
            LAZY.with(|l| l.borrow_mut().finish(&id, true));
            continue;
        };
        if entry.thumbnail.as_deref().is_some_and(|p| p.exists()) {
            LAZY.with(|l| l.borrow_mut().finish(&id, true));
            continue;
        }
        // `import_one` leaves alone an entry that names a thumbnail, and this
        // one's file is gone.
        entry.thumbnail = None;
        let (tx, rx) = async_channel::bounded::<Done>(1);
        let spawned = std::thread::Builder::new()
            .name("fresco-thumb".into())
            .spawn(move || {
                lower_priority();
                let _ = tx.send_blocking(import_one(entry));
            });
        if let Err(e) = spawned {
            log::warn!("thumbnail worker failed to start: {e}");
            LAZY.with(|l| l.borrow_mut().finish(&id, false));
            continue;
        }
        let state = state.clone();
        glib::spawn_future_local(async move {
            match rx.recv().await {
                Ok(done) => apply_lazy(&state, done),
                // The worker panicked.
                Err(_) => LAZY.with(|l| l.borrow_mut().finish(&id, false)),
            }
            pump_lazy(&state);
        });
    }
}

/// Fold one lazily made thumbnail into its entry and card. Linked entries are
/// never saved, so there is nothing to checkpoint: the next launch finds the
/// PNG beside the others.
fn apply_lazy(state: &Rc<RefCell<AppState>>, done: Done) {
    LAZY.with(|l| l.borrow_mut().finish(&done.id, done.ok));
    let found = {
        let mut s = state.borrow_mut();
        s.entries
            .iter_mut()
            .find(|e| e.id == done.id)
            .map(|e| {
                if done.thumbnail.is_some() {
                    e.thumbnail.clone_from(&done.thumbnail);
                    e.thumbnail_rotation = done.baked_rotation;
                }
            })
            .is_some()
    };
    if !found {
        // Unlinked while it ran: don't leave its PNG behind.
        if let Some(t) = &done.thumbnail {
            std::fs::remove_file(t).ok();
        }
        return;
    }
    update_card(&done.id, done.thumbnail.as_deref(), None);
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

    #[test]
    fn only_linked_cards_without_a_thumbnail_ask_for_one() {
        let linked = super::super::linked::file_id(std::path::Path::new("/p/a.mp4"));
        assert!(wants_lazy_thumbnail(&linked, false, false));
        assert!(!wants_lazy_thumbnail(&linked, true, false), "has one");
        assert!(!wants_lazy_thumbnail(&linked, false, true), "queue has it");
        assert!(!wants_lazy_thumbnail("1234-0", false, false), "an import");
    }

    #[test]
    fn a_thumbnail_is_requested_once_while_queued_or_running() {
        let mut q = LazyQueue::new(2);
        assert!(q.request("a"));
        assert!(!q.request("a"), "already queued");
        assert_eq!(q.next(|_| true).as_deref(), Some("a"));
        assert!(!q.request("a"), "already running");
        q.finish("a", true);
        assert!(q.request("a"), "a finished one may be asked for again");
    }

    #[test]
    fn at_most_limit_thumbnails_run_at_once_oldest_first() {
        let mut q = LazyQueue::new(2);
        for id in ["a", "b", "c"] {
            q.request(id);
        }
        assert_eq!(q.next(|_| true).as_deref(), Some("a"));
        assert_eq!(q.next(|_| true).as_deref(), Some("b"));
        assert_eq!(q.next(|_| true), None, "limit reached, c waits");
        q.finish("a", true);
        assert_eq!(q.next(|_| true).as_deref(), Some("c"));
        assert_eq!(q.next(|_| true), None);
    }

    #[test]
    fn cards_that_scrolled_away_are_skipped_and_can_come_back() {
        let mut q = LazyQueue::new(1);
        for id in ["gone", "here"] {
            q.request(id);
        }
        assert_eq!(q.next(|id| id != "gone").as_deref(), Some("here"));
        q.finish("here", true);
        assert_eq!(q.next(|_| true), None, "the skipped one was forgotten");
        assert!(q.request("gone"), "its card came back");
    }

    #[test]
    fn a_failed_thumbnail_is_not_retried() {
        let mut q = LazyQueue::new(1);
        q.request("bad");
        assert_eq!(q.next(|_| true).as_deref(), Some("bad"));
        q.finish("bad", false);
        assert!(!q.request("bad"));
        assert!(q.request("other"), "others are unaffected");
    }
}

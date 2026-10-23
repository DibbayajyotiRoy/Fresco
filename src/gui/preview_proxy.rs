//! Tiny stand-in clips for the library's hover preview.
//!
//! `gtk4::MediaFile` decodes at the source's full resolution and converts every
//! frame to a system-memory RGB `GdkMemoryTexture` before it is uploaded to the
//! GPU. For a 4K clip that is 25–33 MB per frame, 30 frames a second, to paint
//! a card about 300 px wide. Under Vulkan the churn exhausts device memory
//! (`VK_ERROR_OUT_OF_DEVICE_MEMORY`) and GSK then dereferences the failed
//! allocation: the whole GUI segfaults on hover. Measured on this repo's
//! reference box, hovering through a folder of 4K clips grew the GUI to 2.8 GB
//! of RSS and ~1.4 GB of GPU memory that was never handed back.
//!
//! We cannot fix GTK, but we can stop feeding it frames that size. A preview
//! never needs more than a postage stamp, so the first few seconds of each
//! large video are transcoded once into a 480 px, 15 fps, ~4 s H.264 clip under
//! `library/previews/`, and *that* is what the card plays. The same churn on
//! such clips measured flat: 350–430 MB RSS and ~127 MB of GPU memory.
//!
//! Everything GTK-free lives at the top of this file (dimensions, the ffmpeg
//! command line, freshness stamps, the job queue) so it is testable without a
//! display. The bottom half is the machinery that runs the jobs:
//!
//! * One background thread, one job at a time, at `nice 19` — a 200-clip folder
//!   must not make the machine feel slow for the minutes it takes to digest.
//! * Hover requests jump the queue; eager requests (right after an import) are
//!   capped, because a folder of 200 clips is not worth 200 transcodes nobody
//!   asked for. The rest are produced the first time someone hovers them.
//! * No `ffmpeg` means no proxy, and a large video then simply keeps its
//!   thumbnail on hover (logged once). Nothing about the app depends on it.
//!
//! Small sources (longest side ≤ [`DIRECT_MAX_SIDE`]) are not transcoded at
//! all: a GIF or a 480p clip costs GTK next to nothing, so it plays directly.

use std::collections::{BTreeSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

use gtk4::glib;

use super::library::{library_dir, LibraryEntry};
use crate::config::Kind;

/// The proxy fits inside a square this many pixels on a side.
pub const PROXY_BOX: u32 = 480;
/// Sources whose longest side is at most this play directly, no proxy.
pub const DIRECT_MAX_SIDE: u32 = 640;
/// How much of the source the proxy keeps, from the start.
pub const CLIP_SECONDS: u32 = 4;
/// Proxy frame rate.
pub const CLIP_FPS: u32 = 15;
/// Eager (post-import) jobs are refused once this many are already waiting.
const EAGER_QUEUE_CAP: usize = 24;
/// A transcode that has not finished in this long is stuck (a stalled network
/// mount, a pathological file) and is killed rather than holding the queue.
const JOB_TIMEOUT: Duration = Duration::from_secs(120);

// ---------------------------------------------------------------------------
// Pure helpers: dimensions, command line, freshness. No GTK, no I/O.
// ---------------------------------------------------------------------------

/// Round down to an even number, at least 2 (`yuv420p` needs even dimensions).
fn even(n: u32) -> u32 {
    (n & !1).max(2)
}

/// Proxy size for a `w`×`h` source: fit inside [`PROXY_BOX`], aspect kept,
/// both sides even, never larger than the source. `None` when either side is
/// unknown (0) or too small to hold a frame.
pub fn proxy_dims(w: u32, h: u32) -> Option<(u32, u32)> {
    if w < 2 || h < 2 {
        return None;
    }
    let long = w.max(h);
    let (w, h) = if long <= PROXY_BOX {
        (w, h)
    } else {
        let scale = |side: u32| (u64::from(side) * u64::from(PROXY_BOX) / u64::from(long)) as u32;
        if w >= h {
            (PROXY_BOX, scale(h))
        } else {
            (scale(w), PROXY_BOX)
        }
    };
    Some((even(w), even(h)))
}

/// Whether a `w`×`h` source needs a proxy. Only a source we *know* to be small
/// plays directly: an unknown size (0 — the metadata probe has not landed, or
/// ffprobe is missing) is treated as large, because guessing wrong the other
/// way is the crash this module exists to prevent.
pub fn needs_proxy(w: u32, h: u32) -> bool {
    w == 0 || h == 0 || w.max(h) > DIRECT_MAX_SIDE
}

/// The size a player shows a `w`×`h` stream stored with `rotation` degrees of
/// display rotation (phone footage is often stored landscape and flagged).
fn display_dims(w: u32, h: u32, rotation: i64) -> (u32, u32) {
    if rotation.rem_euclid(180) == 90 {
        (h, w)
    } else {
        (w, h)
    }
}

/// Parse `ffprobe -of json` output for the first video stream's *displayed*
/// size. ffmpeg autorotates before filtering, so the scale in [`ffmpeg_args`]
/// has to be computed for the rotated frame or the clip comes out squashed.
fn parse_probe(json: &str) -> Option<(u32, u32)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let stream = v.get("streams")?.as_array()?.first()?;
    let w = u32::try_from(stream.get("width")?.as_u64()?).ok()?;
    let h = u32::try_from(stream.get("height")?.as_u64()?).ok()?;
    let rotation = stream
        .get("side_data_list")
        .and_then(|l| l.as_array())
        .and_then(|l| l.iter().find_map(|d| d.get("rotation")?.as_f64()))
        .or_else(|| {
            stream
                .get("tags")?
                .get("rotate")?
                .as_str()?
                .trim()
                .parse::<f64>()
                .ok()
        })
        .unwrap_or(0.0);
    Some(display_dims(w, h, rotation.round() as i64))
}

/// The ffmpeg arguments that turn `src` into a proxy at `out`.
///
/// `-map 0:v:0` pins the first video stream (so cover art, timecode or data
/// tracks cannot trip the mp4 muxer), `-an -sn` drops audio and subtitles, and
/// `+faststart` puts the index first so GStreamer can start immediately.
pub fn ffmpeg_args(src: &Path, out: &Path, (w, h): (u32, u32)) -> Vec<String> {
    let s = |v: &str| v.to_string();
    vec![
        s("-nostdin"),
        s("-loglevel"),
        s("error"),
        s("-y"),
        s("-ss"),
        s("0"),
        s("-t"),
        CLIP_SECONDS.to_string(),
        s("-i"),
        src.to_string_lossy().into_owned(),
        s("-map"),
        s("0:v:0"),
        s("-an"),
        s("-sn"),
        s("-vf"),
        format!("fps={CLIP_FPS},scale={w}:{h}"),
        s("-c:v"),
        s("libx264"),
        s("-preset"),
        s("veryfast"),
        s("-crf"),
        s("32"),
        s("-pix_fmt"),
        s("yuv420p"),
        s("-movflags"),
        s("+faststart"),
        out.to_string_lossy().into_owned(),
    ]
}

/// The program and arguments to actually spawn: ffmpeg behind `nice -n 19` when
/// `nice` exists, bare otherwise.
fn command_line(nice: bool, ffmpeg: &[String]) -> (String, Vec<String>) {
    if nice {
        let mut argv = vec!["-n".to_string(), "19".to_string(), "ffmpeg".to_string()];
        argv.extend_from_slice(ffmpeg);
        ("nice".to_string(), argv)
    } else {
        ("ffmpeg".to_string(), ffmpeg.to_vec())
    }
}

/// What a proxy was made from. The proxy is reusable exactly while this string
/// is unchanged: it covers the recipe (a new box size or frame rate must not
/// leave old clips behind), the source path (a playlist whose first item
/// changed), and the source's mtime and size (the file was replaced or edited).
fn make_stamp(path: &str, mtime_secs: u64, mtime_nanos: u32, size: u64) -> String {
    format!("p1-{PROXY_BOX}-{CLIP_FPS}-{CLIP_SECONDS} {size} {mtime_secs}.{mtime_nanos:09} {path}")
}

/// [`make_stamp`] for the file as it is on disk right now.
fn stamp(src: &Path) -> Option<String> {
    let md = std::fs::metadata(src).ok()?;
    let mtime = md.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(make_stamp(
        &src.to_string_lossy(),
        mtime.as_secs(),
        mtime.subsec_nanos(),
        md.len(),
    ))
}

/// Whether the stamp stored next to a proxy still describes its source.
fn is_fresh(on_disk: Option<&str>, current: &str) -> bool {
    on_disk.is_some_and(|s| s.trim_end() == current)
}

/// A file-name-safe form of an entry id. Ids are generated (`pid-counter`), but
/// a path component built from anything else must not be able to leave the
/// previews directory.
fn safe_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Where proxies live.
pub fn previews_dir() -> PathBuf {
    library_dir().join("previews")
}

fn proxy_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{}.mp4", safe_id(id)))
}

fn stamp_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{}.stamp", safe_id(id)))
}

/// In-progress output. Ends in `.mp4` because ffmpeg picks the muxer from the
/// extension; the rename to [`proxy_file`] is what publishes it.
fn part_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{}.part.mp4", safe_id(id)))
}

/// Whether a file in the previews directory belongs to no known entry. The
/// entry id is everything before the first dot (`<id>.mp4`, `<id>.stamp`,
/// `<id>.part.mp4`). Files of linked folders are never orphans here: the
/// library is pruned before their folders have been scanned.
fn is_orphan(file_name: &str, known: &BTreeSet<String>) -> bool {
    let stem = file_name.split('.').next().unwrap_or(file_name);
    !known.contains(stem) && !super::linked::is_linked_id(stem)
}

// ---------------------------------------------------------------------------
// The queue: dedupe, hover-first ordering, an eager cap. Still no I/O.
// ---------------------------------------------------------------------------

/// Everything a job needs to know about the entry it serves, copied out of the
/// `LibraryEntry` so it can cross to the worker thread and outlive the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The entry id: names the proxy files.
    pub id: String,
    /// The video to preview.
    pub src: PathBuf,
    /// Stored size from the metadata probe, when it has run.
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// The `Request` for `entry`, or `None` if it has nothing to preview: images and
/// slideshows have no motion, and a rotated entry keeps its (rotated) thumbnail
/// because GTK's `MediaFile` cannot rotate and motion in the wrong orientation
/// reads as a bug.
pub fn request_for(entry: &LibraryEntry) -> Option<Request> {
    if !entry.rotation.unwrap_or(0).is_multiple_of(360) {
        return None;
    }
    let src = match entry.kind {
        Kind::Video => entry.path.clone()?,
        Kind::Playlist => entry.paths.first().cloned()?,
        _ => return None,
    };
    Some(Request {
        id: entry.id.clone(),
        src,
        width: entry.width,
        height: entry.height,
    })
}

/// How urgently a job is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Someone is hovering the card right now.
    Hover,
    /// Speculative: an import just finished and the clip will probably be
    /// wanted eventually.
    Prefetch,
}

#[derive(Default)]
struct JobQueue {
    jobs: VecDeque<Request>,
}

impl JobQueue {
    const fn new() -> Self {
        JobQueue {
            jobs: VecDeque::new(),
        }
    }

    /// Queue `req`. Returns whether it is (now) waiting: `false` means an eager
    /// request was turned away because the queue is already long.
    ///
    /// A hover jumps to the front — and drags an already-queued eager job there
    /// with it — so the card the user is looking at is never stuck behind a
    /// folder's worth of speculative work.
    fn push(&mut self, req: Request, priority: Priority) -> bool {
        let queued = self.jobs.iter().position(|j| j.id == req.id);
        match (priority, queued) {
            (Priority::Hover, Some(i)) => {
                let job = self.jobs.remove(i).expect("position just found");
                self.jobs.push_front(job);
                true
            }
            (Priority::Hover, None) => {
                self.jobs.push_front(req);
                true
            }
            (Priority::Prefetch, Some(_)) => true,
            (Priority::Prefetch, None) if self.jobs.len() >= EAGER_QUEUE_CAP => false,
            (Priority::Prefetch, None) => {
                self.jobs.push_back(req);
                true
            }
        }
    }

    fn pop(&mut self) -> Option<Request> {
        self.jobs.pop_front()
    }

    fn remove(&mut self, id: &str) {
        self.jobs.retain(|j| j.id != id);
    }
}

// ---------------------------------------------------------------------------
// What a card asks for, and the shared state behind it.
// ---------------------------------------------------------------------------

/// What a card should play when hovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSource {
    /// Small enough to play as it is.
    Direct(PathBuf),
    /// A fresh proxy clip, ready now.
    Proxy(PathBuf),
    /// A proxy is needed and does not exist yet: [`enqueue`] it and keep
    /// showing the thumbnail until it lands.
    Build(Request),
    /// Nothing to play — the source is gone, `ffmpeg` is missing, or the proxy
    /// already failed once this session. The card keeps its thumbnail.
    None,
}

struct Inner {
    queue: JobQueue,
    /// The id being transcoded right now.
    current: Option<String>,
    /// The entry behind `current` was removed mid-transcode: discard the result.
    cancel_current: bool,
    /// Ids whose transcode failed this session. Not retried on every hover — a
    /// corrupt file would otherwise burn a transcode per mouse-over.
    failed: BTreeSet<String>,
}

static STATE: Mutex<Inner> = Mutex::new(Inner {
    queue: JobQueue::new(),
    current: None,
    cancel_current: false,
    failed: BTreeSet::new(),
});
static WAKE: Condvar = Condvar::new();

fn lock() -> MutexGuard<'static, Inner> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn in_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|dir| dir.join(name).is_file()))
}

/// Whether `ffmpeg` is installed. Logs once when it is not.
fn ffmpeg_available() -> bool {
    static FOUND: OnceLock<bool> = OnceLock::new();
    *FOUND.get_or_init(|| {
        let found = in_path("ffmpeg");
        if !found {
            log::warn!(
                "hover previews: ffmpeg not found; large videos keep their thumbnail on hover \
                 (install ffmpeg to enable previews of them)"
            );
        }
        found
    })
}

fn nice_available() -> bool {
    static FOUND: OnceLock<bool> = OnceLock::new();
    *FOUND.get_or_init(|| in_path("nice"))
}

fn fresh_proxy(dir: &Path, req: &Request) -> Option<PathBuf> {
    let proxy = proxy_file(dir, &req.id);
    if !proxy.is_file() {
        return None;
    }
    let current = stamp(&req.src)?;
    let on_disk = std::fs::read_to_string(stamp_file(dir, &req.id)).ok();
    is_fresh(on_disk.as_deref(), &current).then_some(proxy)
}

/// Decide what `req` plays right now. Cheap — a few `stat`s — and meant to be
/// called at hover time, not when the card is built.
pub fn select(req: &Request) -> PreviewSource {
    if !req.src.is_file() {
        return PreviewSource::None;
    }
    if !needs_proxy(req.width.unwrap_or(0), req.height.unwrap_or(0)) {
        return PreviewSource::Direct(req.src.clone());
    }
    if let Some(proxy) = fresh_proxy(&previews_dir(), req) {
        return PreviewSource::Proxy(proxy);
    }
    if !ffmpeg_available() || lock().failed.contains(&req.id) {
        return PreviewSource::None;
    }
    PreviewSource::Build(req.clone())
}

/// Queue a proxy for `req` right after an import, if one is worth having. Does
/// nothing for small sources, for entries that already have a fresh proxy, and
/// once the eager queue is full — those are made on first hover instead.
pub fn prefetch(req: Request) {
    if matches!(select(&req), PreviewSource::Build(_)) {
        enqueue(req, Priority::Prefetch, None);
    }
}

/// Callbacks waiting for a job, by entry id. Main thread only: the worker
/// reports over a channel and [`finish`] runs them from the GLib loop.
type DoneCallback = Box<dyn FnOnce(bool)>;

thread_local! {
    static WAITERS: std::cell::RefCell<std::collections::HashMap<String, Vec<DoneCallback>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    static WORKER_STARTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct Done {
    id: String,
    ok: bool,
}

/// Ask for a proxy for `req`. `on_done(true)` runs on the main thread once the
/// clip exists; `on_done(false)` if the job failed or was discarded. Asking for
/// an id that is already queued or running only adds the callback.
pub fn enqueue(req: Request, priority: Priority, on_done: Option<DoneCallback>) {
    if !ffmpeg_available() {
        return;
    }
    ensure_worker();
    let id = req.id.clone();
    let accepted = {
        let mut g = lock();
        if g.failed.contains(&id) {
            false
        } else if g.current.as_deref() == Some(id.as_str()) {
            true
        } else {
            g.queue.push(req, priority)
        }
    };
    if !accepted {
        return;
    }
    if let Some(cb) = on_done {
        WAITERS.with(|w| w.borrow_mut().entry(id).or_default().push(cb));
    }
    WAKE.notify_one();
}

/// Forget everything about `id`: queued work, an in-flight transcode's result,
/// and the files. Called wherever the entry's thumbnail is deleted.
pub fn remove_for(id: &str) {
    {
        let mut g = lock();
        g.queue.remove(id);
        if g.current.as_deref() == Some(id) {
            g.cancel_current = true;
        }
        g.failed.remove(id);
    }
    let dir = previews_dir();
    for f in [
        proxy_file(&dir, id),
        stamp_file(&dir, id),
        part_file(&dir, id),
    ] {
        std::fs::remove_file(f).ok();
    }
}

/// Delete proxy files whose entry no longer exists — what a crash between a
/// removal and its cleanup, or a transcode finishing for a just-removed entry,
/// leaves behind. `known` holds the ids of every entry in the library.
pub fn prune_orphans(known: impl IntoIterator<Item = String>) {
    let known: BTreeSet<String> = known.into_iter().map(|id| safe_id(&id)).collect();
    let dir = previews_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let in_flight = lock().current.clone().map(|id| safe_id(&id));
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let stem = name.split('.').next().unwrap_or(&name);
        if in_flight.as_deref() == Some(stem) {
            continue;
        }
        if is_orphan(&name, &known) {
            std::fs::remove_file(e.path()).ok();
        }
    }
}

// ---------------------------------------------------------------------------
// The worker.
// ---------------------------------------------------------------------------

fn ensure_worker() {
    if WORKER_STARTED.with(|s| s.replace(true)) {
        return;
    }
    let (tx, rx) = async_channel::unbounded::<Done>();
    let spawned = std::thread::Builder::new()
        .name("fresco-preview-proxy".into())
        .spawn(move || worker_loop(tx));
    if let Err(e) = spawned {
        log::warn!("hover previews: could not start the proxy worker: {e}");
        return;
    }
    glib::spawn_future_local(async move {
        while let Ok(done) = rx.recv().await {
            finish(done);
        }
    });
}

/// Run the callbacks waiting on a finished job. Main thread.
fn finish(done: Done) {
    let waiting = WAITERS.with(|w| w.borrow_mut().remove(&done.id));
    for cb in waiting.into_iter().flatten() {
        cb(done.ok);
    }
}

/// Why a job produced no proxy.
enum Failure {
    /// Not an error: the entry was removed, or the source changed under us.
    /// Nothing is remembered, so the next hover tries again.
    Skipped,
    Failed(String),
}

fn worker_loop(tx: async_channel::Sender<Done>) {
    static WARNED: AtomicBool = AtomicBool::new(false);
    loop {
        let req = {
            let mut g = lock();
            loop {
                if let Some(req) = g.queue.pop() {
                    g.current = Some(req.id.clone());
                    g.cancel_current = false;
                    break req;
                }
                g = WAKE.wait(g).unwrap_or_else(|e| e.into_inner());
            }
        };
        let result = generate(&previews_dir(), &req, &|| lock().cancel_current);
        {
            let mut g = lock();
            g.current = None;
            g.cancel_current = false;
            if matches!(result, Err(Failure::Failed(_))) {
                g.failed.insert(req.id.clone());
            }
        }
        if let Err(Failure::Failed(why)) = &result {
            // The first failure is worth a warning (it usually means ffmpeg
            // lacks libx264); the rest are the same news.
            if WARNED.swap(true, Ordering::Relaxed) {
                log::debug!("hover previews: no proxy for {}: {why}", req.src.display());
            } else {
                log::warn!("hover previews: no proxy for {}: {why}", req.src.display());
            }
        }
        if tx
            .send_blocking(Done {
                id: req.id,
                ok: result.is_ok(),
            })
            .is_err()
        {
            return; // the main loop is gone
        }
    }
}

/// The displayed size of `src`'s first video stream, via ffprobe.
fn probe_display_dims(src: &Path) -> Option<(u32, u32)> {
    let out = Command::new("ffprobe")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height:stream_side_data=rotation:stream_tags=rotate",
            "-of",
            "json",
        ])
        .arg(src)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_probe(&String::from_utf8_lossy(&out.stdout))
}

/// Keep only the last bytes of a stream while draining all of it, so a chatty
/// failing ffmpeg can neither block on a full pipe nor fill our memory.
fn drain_tail(mut r: impl Read) -> String {
    const KEEP: usize = 400;
    let mut tail: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    while let Ok(n) = r.read(&mut buf) {
        if n == 0 {
            break;
        }
        tail.extend_from_slice(&buf[..n]);
        if tail.len() > KEEP {
            tail.drain(..tail.len() - KEEP);
        }
    }
    String::from_utf8_lossy(&tail).trim().to_string()
}

fn run_ffmpeg(args: &[String]) -> Result<(), String> {
    let (program, argv) = command_line(nice_available(), args);
    let mut child = Command::new(&program)
        .args(&argv)
        // -nostdin is in the args, but a terminal-launched app must also never
        // hand ffmpeg the TTY (SIGTTIN would stop us) — see overview.rs.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawning {program}: {e}"))?;
    let stderr = child
        .stderr
        .take()
        .map(|r| std::thread::spawn(move || drain_tail(r)));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > JOB_TIMEOUT => {
                child.kill().ok();
                child.wait().ok();
                return Err(format!("timed out after {}s", JOB_TIMEOUT.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("waiting for ffmpeg: {e}")),
        }
    };
    let tail = stderr
        .and_then(|h| h.join().ok())
        .filter(|t| !t.is_empty())
        .unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg {status}: {tail}"))
    }
}

/// Transcode `req.src` into `dir`, publishing atomically: the clip is written
/// as `<id>.part.mp4`, renamed to `<id>.mp4`, and only then is the stamp that
/// declares it fresh written (also via rename). A crash anywhere in between
/// leaves a clip with no matching stamp, which reads as stale and is rebuilt.
fn generate(dir: &Path, req: &Request, cancelled: &dyn Fn() -> bool) -> Result<(), Failure> {
    let fail = |why: String| Failure::Failed(why);
    std::fs::create_dir_all(dir).map_err(|e| fail(format!("creating {}: {e}", dir.display())))?;
    let before = stamp(&req.src).ok_or_else(|| fail("source unreadable".into()))?;
    let dims = probe_display_dims(&req.src)
        .or_else(|| req.width.zip(req.height))
        .and_then(|(w, h)| proxy_dims(w, h))
        .ok_or_else(|| fail("source dimensions unknown".into()))?;

    let part = part_file(dir, &req.id);
    let outcome = run_ffmpeg(&ffmpeg_args(&req.src, &part, dims));
    let discard = |part: &Path| {
        std::fs::remove_file(part).ok();
    };
    if cancelled() {
        discard(&part);
        return Err(Failure::Skipped);
    }
    if let Err(why) = outcome {
        discard(&part);
        return Err(fail(why));
    }
    if std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0) == 0 {
        discard(&part);
        return Err(fail("ffmpeg wrote an empty clip".into()));
    }
    if stamp(&req.src).as_deref() != Some(before.as_str()) {
        discard(&part);
        return Err(Failure::Skipped);
    }

    let publish = || -> std::io::Result<()> {
        std::fs::rename(&part, proxy_file(dir, &req.id))?;
        let stamp_part = dir.join(format!("{}.stamp.part", safe_id(&req.id)));
        std::fs::write(&stamp_part, &before)?;
        std::fs::rename(&stamp_part, stamp_file(dir, &req.id))
    };
    publish().map_err(|e| {
        discard(&part);
        fail(format!("publishing the clip: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: &str) -> Request {
        Request {
            id: id.into(),
            src: PathBuf::from(format!("/videos/{id}.mp4")),
            width: Some(3840),
            height: Some(2160),
        }
    }

    #[test]
    fn proxy_dims_fit_the_box_with_even_sides() {
        assert_eq!(proxy_dims(3840, 2160), Some((480, 270)));
        assert_eq!(proxy_dims(1920, 1080), Some((480, 270)));
        assert_eq!(proxy_dims(7680, 4320), Some((480, 270)));
        // Portrait fits the same box, rotated.
        assert_eq!(proxy_dims(2160, 3840), Some((270, 480)));
        assert_eq!(proxy_dims(1080, 1920), Some((270, 480)));
        assert_eq!(proxy_dims(2000, 2000), Some((480, 480)));
        // 1001x1000 would be 480x479: odd heights are rounded down to even.
        assert_eq!(proxy_dims(1001, 1000), Some((480, 478)));
        // A sliver still gets a legal (>= 2) side.
        assert_eq!(proxy_dims(4000, 3), Some((480, 2)));
    }

    #[test]
    fn proxy_dims_never_upscale() {
        // Already inside the box: kept (rounded down to even), never enlarged.
        assert_eq!(proxy_dims(320, 180), Some((320, 180)));
        assert_eq!(proxy_dims(480, 480), Some((480, 480)));
        assert_eq!(proxy_dims(479, 301), Some((478, 300)));
        for (w, h) in [(2, 2), (3, 3), (100, 7), (640, 360), (481, 3), (5000, 9)] {
            let (pw, ph) = proxy_dims(w, h).expect("legal source");
            assert!(pw <= w.max(2) && ph <= h.max(2), "{w}x{h} -> {pw}x{ph}");
            assert!(pw % 2 == 0 && ph % 2 == 0, "{w}x{h} -> odd {pw}x{ph}");
            assert!(pw <= PROXY_BOX && ph <= PROXY_BOX);
        }
    }

    #[test]
    fn proxy_dims_refuse_unknown_sizes() {
        assert_eq!(proxy_dims(0, 0), None);
        assert_eq!(proxy_dims(1920, 0), None);
        assert_eq!(proxy_dims(0, 1080), None);
        assert_eq!(proxy_dims(1, 1), None);
    }

    #[test]
    fn only_known_small_sources_play_directly() {
        assert!(!needs_proxy(640, 360));
        assert!(!needs_proxy(360, 640), "portrait: the longest side counts");
        assert!(!needs_proxy(320, 240));
        assert!(needs_proxy(641, 360));
        assert!(needs_proxy(1920, 1080));
        assert!(needs_proxy(3840, 2160));
        // The metadata probe has not run (or ffprobe is absent): assume large.
        assert!(needs_proxy(0, 0));
        assert!(needs_proxy(0, 360));
        assert!(needs_proxy(640, 0));
    }

    #[test]
    fn ffmpeg_command_line_is_stable() {
        let args = ffmpeg_args(
            Path::new("/v/a b.mp4"),
            Path::new("/p/1-0.part.mp4"),
            (480, 270),
        );
        assert_eq!(
            args.join(" "),
            "-nostdin -loglevel error -y -ss 0 -t 4 -i /v/a b.mp4 -map 0:v:0 -an -sn \
             -vf fps=15,scale=480:270 -c:v libx264 -preset veryfast -crf 32 \
             -pix_fmt yuv420p -movflags +faststart /p/1-0.part.mp4"
        );
        // The path is one argv element, never re-split by a shell.
        assert!(args.contains(&"/v/a b.mp4".to_string()));
    }

    #[test]
    fn low_priority_wrapper_is_used_only_when_nice_exists() {
        let args = vec!["-y".to_string(), "out.mp4".to_string()];
        let (prog, argv) = command_line(true, &args);
        assert_eq!(prog, "nice");
        assert_eq!(argv, ["-n", "19", "ffmpeg", "-y", "out.mp4"]);
        let (prog, argv) = command_line(false, &args);
        assert_eq!(prog, "ffmpeg");
        assert_eq!(argv, ["-y", "out.mp4"]);
    }

    #[test]
    fn a_proxy_is_fresh_only_for_the_source_it_was_made_from() {
        let now = make_stamp("/v/a.mp4", 1_700_000_000, 5, 1000);
        // Newline-terminated, as a hand-edited sidecar might be.
        assert!(is_fresh(Some(&format!("{now}\n")), &now));
        assert!(is_fresh(Some(&now), &now));
        assert!(!is_fresh(None, &now), "no sidecar means stale");
        // Edited in place: mtime moves.
        assert!(!is_fresh(
            Some(&make_stamp("/v/a.mp4", 1_700_000_001, 5, 1000)),
            &now
        ));
        assert!(!is_fresh(
            Some(&make_stamp("/v/a.mp4", 1_700_000_000, 6, 1000)),
            &now
        ));
        // Replaced by a file of another size.
        assert!(!is_fresh(
            Some(&make_stamp("/v/a.mp4", 1_700_000_000, 5, 1001)),
            &now
        ));
        // A playlist whose first item changed.
        assert!(!is_fresh(
            Some(&make_stamp("/v/b.mp4", 1_700_000_000, 5, 1000)),
            &now
        ));
        assert!(!is_fresh(Some(""), &now));
    }

    #[test]
    fn stamps_track_the_real_file() {
        let dir = std::env::temp_dir().join(format!("fresco-stamp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("clip.bin");
        std::fs::write(&f, b"one").unwrap();
        let a = stamp(&f).expect("stamp of a real file");
        assert_eq!(stamp(&f).as_deref(), Some(a.as_str()), "stable");
        std::fs::write(&f, b"three").unwrap();
        let b = stamp(&f).unwrap();
        assert_ne!(a, b, "a size change must invalidate");
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(stamp(&f), None, "a missing source has no stamp");
    }

    #[test]
    fn ids_cannot_escape_the_previews_directory() {
        assert_eq!(safe_id("1234-7"), "1234-7");
        assert_eq!(safe_id("../../etc/passwd"), "______etc_passwd");
        let dir = Path::new("/lib/previews");
        assert_eq!(proxy_file(dir, "9-1"), Path::new("/lib/previews/9-1.mp4"));
        assert_eq!(stamp_file(dir, "9-1"), Path::new("/lib/previews/9-1.stamp"));
        assert_eq!(
            part_file(dir, "9-1"),
            Path::new("/lib/previews/9-1.part.mp4")
        );
        assert_eq!(
            proxy_file(dir, "../x").parent(),
            Some(dir),
            "a hostile id left the directory"
        );
    }

    #[test]
    fn probe_output_gives_the_displayed_size() {
        let plain = r#"{"streams":[{"width":3840,"height":2160}]}"#;
        assert_eq!(parse_probe(plain), Some((3840, 2160)));
        // Display-matrix rotation (modern phones), either sign.
        let matrix = r#"{"streams":[{"width":3840,"height":2160,
            "side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]}]}"#;
        assert_eq!(parse_probe(matrix), Some((2160, 3840)));
        let matrix270 = r#"{"streams":[{"width":1920,"height":1080,
            "side_data_list":[{"rotation":90}]}]}"#;
        assert_eq!(parse_probe(matrix270), Some((1080, 1920)));
        // Old-style rotate tag.
        let tag = r#"{"streams":[{"width":1920,"height":1080,"tags":{"rotate":"270"}}]}"#;
        assert_eq!(parse_probe(tag), Some((1080, 1920)));
        // 180 flips the picture but not the shape.
        let upside = r#"{"streams":[{"width":1920,"height":1080,
            "side_data_list":[{"rotation":180}]}]}"#;
        assert_eq!(parse_probe(upside), Some((1920, 1080)));
        assert_eq!(parse_probe(r#"{"streams":[]}"#), None);
        assert_eq!(parse_probe("not json"), None);
    }

    #[test]
    fn entries_without_motion_or_in_a_rotated_orientation_get_no_request() {
        let mut v = LibraryEntry::new_video(PathBuf::from("/v/a.mp4"));
        let r = request_for(&v).expect("a plain video previews");
        assert_eq!(r.src, Path::new("/v/a.mp4"));
        assert_eq!(r.id, v.id);

        v.rotation = Some(90);
        assert_eq!(
            request_for(&v),
            None,
            "rotated entries keep their thumbnail"
        );
        v.rotation = Some(360);
        assert!(request_for(&v).is_some(), "a full turn is no rotation");

        let img = LibraryEntry::new_image(PathBuf::from("/v/a.png"));
        assert_eq!(request_for(&img), None);

        let mut list = LibraryEntry::new_video(PathBuf::from("/v/first.mp4"));
        list.kind = Kind::Playlist;
        list.path = None;
        assert_eq!(
            request_for(&list),
            None,
            "an empty playlist has no first item"
        );
        list.paths = vec![
            PathBuf::from("/v/first.mp4"),
            PathBuf::from("/v/second.mp4"),
        ];
        assert_eq!(
            request_for(&list).map(|r| r.src),
            Some(PathBuf::from("/v/first.mp4"))
        );
    }

    #[test]
    fn a_hover_jumps_ahead_of_queued_work() {
        let mut q = JobQueue::default();
        assert!(q.push(req("a"), Priority::Prefetch));
        assert!(q.push(req("b"), Priority::Prefetch));
        assert!(q.push(req("c"), Priority::Hover));
        assert_eq!(q.pop().map(|r| r.id), Some("c".into()));
        assert_eq!(q.pop().map(|r| r.id), Some("a".into()));

        // Hovering something already queued drags it to the front, once.
        assert!(q.push(req("d"), Priority::Prefetch));
        assert!(q.push(req("d"), Priority::Hover));
        assert_eq!(q.jobs.len(), 2, "the same id was queued twice");
        assert_eq!(q.pop().map(|r| r.id), Some("d".into()));
        assert_eq!(q.pop().map(|r| r.id), Some("b".into()));
        assert!(q.pop().is_none());
    }

    #[test]
    fn eager_work_is_capped_but_a_hover_never_is() {
        let mut q = JobQueue::default();
        for i in 0..EAGER_QUEUE_CAP {
            assert!(q.push(req(&format!("p{i}")), Priority::Prefetch));
        }
        assert!(
            !q.push(req("one-too-many"), Priority::Prefetch),
            "a 200-clip import queued 200 speculative transcodes"
        );
        assert!(
            q.push(req("p0"), Priority::Prefetch),
            "already queued is fine"
        );
        assert!(q.push(req("hovered"), Priority::Hover));
        assert_eq!(q.pop().map(|r| r.id), Some("hovered".into()));
    }

    #[test]
    fn removing_an_entry_drops_its_queued_job() {
        let mut q = JobQueue::default();
        q.push(req("a"), Priority::Prefetch);
        q.push(req("b"), Priority::Prefetch);
        q.remove("a");
        q.remove("never-queued");
        assert_eq!(q.pop().map(|r| r.id), Some("b".into()));
        assert!(q.pop().is_none());
    }

    #[test]
    fn only_files_of_unknown_entries_are_orphans() {
        let known: BTreeSet<String> = ["10-1", "10-2"].iter().map(|s| s.to_string()).collect();
        for kept in ["10-1.mp4", "10-1.stamp", "10-2.part.mp4", "10-2.stamp.part"] {
            assert!(!is_orphan(kept, &known), "{kept}");
        }
        for gone in ["10-3.mp4", "10-3.stamp", "stray.mp4", "10-12.mp4", ""] {
            assert!(is_orphan(gone, &known), "{gone}");
        }
    }

    #[test]
    fn ffmpeg_chatter_is_bounded_to_its_tail() {
        let noise = "x".repeat(100_000) + "\nConversion failed!";
        let tail = drain_tail(noise.as_bytes());
        assert!(tail.len() <= 400);
        assert!(tail.ends_with("Conversion failed!"));
        assert_eq!(drain_tail(&b""[..]), "");
    }

    /// The real thing, end to end: a generated 1280x720 clip becomes a
    /// 480x270 H.264 proxy with a matching stamp, and the original is
    /// untouched. Skipped where ffmpeg (or its libx264) is not installed.
    #[test]
    fn generate_transcodes_a_real_clip_and_publishes_it() {
        let has_x264 = Command::new("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("libx264"))
            .unwrap_or(false);
        if !has_x264 || !in_path("ffprobe") {
            eprintln!("skipping: ffmpeg with libx264 and ffprobe are required");
            return;
        }
        let dir = std::env::temp_dir().join(format!("fresco-proxy-e2e-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.mp4");
        let make_src = || {
            Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=1280x720:rate=30:duration=6",
                    "-pix_fmt",
                    "yuv420p",
                ])
                .arg(&src)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        };
        assert!(make_src(), "could not synthesise a test clip");

        let previews = dir.join("previews");
        let r = Request {
            id: "t-1".into(),
            src: src.clone(),
            width: None,
            height: None,
        };
        assert!(generate(&previews, &r, &|| false).is_ok());

        let proxy = proxy_file(&previews, "t-1");
        assert!(proxy.is_file());
        assert!(
            !part_file(&previews, "t-1").exists(),
            "the part file leaked"
        );
        assert_eq!(probe_display_dims(&proxy), Some((480, 270)));
        assert!(
            fresh_proxy(&previews, &r).is_some(),
            "a just-built proxy reads as stale"
        );

        // Replacing the source invalidates it.
        std::fs::write(&src, b"replaced").unwrap();
        assert!(fresh_proxy(&previews, &r).is_none());

        // A job whose entry was removed mid-flight publishes nothing and is
        // not recorded as a failure.
        std::fs::remove_file(proxy_file(&previews, "t-1")).ok();
        std::fs::remove_file(stamp_file(&previews, "t-1")).ok();
        assert!(make_src());
        assert!(matches!(
            generate(&previews, &r, &|| true),
            Err(Failure::Skipped)
        ));
        assert!(!part_file(&previews, "t-1").exists());
        assert!(!proxy_file(&previews, "t-1").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}

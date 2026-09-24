use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::{
    config::Config,
    ipc::{self, Request, Response},
};
use anyhow::Result;
use gtk4::glib;

/// Spawn frescod detached from the GUI process so it outlives it.
/// The daemon binary is responsible for its own daemonization.
pub fn spawn_daemon() -> Result<()> {
    let frescod = frescod_path();
    Command::new(&frescod)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

fn frescod_path() -> PathBuf {
    // Look next to the current executable first (installed .deb), then PATH.
    if let Ok(mut path) = std::env::current_exe() {
        path.set_file_name("frescod");
        if path.exists() {
            return path;
        }
    }
    PathBuf::from("frescod")
}

/// How long a fresh daemon gets to bind its socket before we give up, polled
/// in small steps instead of one fixed sleep so a fast daemon (the common
/// case) doesn't cost more than it has to.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(3);
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// `Apply` can mean "rebuild renderers + redecode a full-size overview frame"
/// on the daemon side, which is legitimately slow on weak hardware — this
/// runs off the GTK thread, so a generous timeout costs nothing but time on a
/// background worker.
const APPLY_TIMEOUT: Duration = Duration::from_secs(30);

/// Apply config, starting the daemon first if not running. Blocking — callers
/// MUST run this off the GTK main thread; see `apply_async`.
fn apply_blocking() -> Result<()> {
    if ipc::daemon_alive() {
        let resp = ipc::request_with_timeout(&Request::Apply, APPLY_TIMEOUT)?;
        if let Response::Err { message } = resp {
            return Err(anyhow::anyhow!("daemon error: {message}"));
        }
    } else {
        spawn_daemon()?;
        // Poll instead of one fixed sleep: most daemons bind well under a
        // second, and this thread isn't the GTK thread so polling costs
        // nothing but wall time.
        let start = std::time::Instant::now();
        while !ipc::daemon_alive() {
            if start.elapsed() >= STARTUP_TIMEOUT {
                return Err(anyhow::anyhow!(
                    "daemon failed to start — check ~/.local/state/fresco/frescod.log"
                ));
            }
            std::thread::sleep(STARTUP_POLL_INTERVAL);
        }
        let resp = ipc::request_with_timeout(&Request::Apply, APPLY_TIMEOUT)?;
        if let Response::Err { message } = resp {
            return Err(anyhow::anyhow!("daemon error: {message}"));
        }
    }
    Ok(())
}

/// Blocking `Stop`, off the GTK main thread; see `stop_async`.
fn stop_blocking() -> Result<()> {
    if ipc::daemon_alive() {
        let resp = ipc::request_with_timeout(&Request::Stop, APPLY_TIMEOUT)?;
        if let Response::Err { message } = resp {
            return Err(anyhow::anyhow!("daemon error: {message}"));
        }
    }
    Ok(())
}

/// What an async apply/stop resolves to. `superseded` is true for every
/// callback in a coalesced batch except the last — those requests were
/// overtaken by a later one before their own worker ever ran, so their
/// result describes stale state and callers should skip toast/telemetry for
/// them.
pub struct ApplyOutcome {
    pub result: Result<(), String>,
    pub superseded: bool,
}

/// Coalesces rapid callers of the same blocking operation into at most one
/// in-flight worker plus one queued follow-up, so ten clicks in the time it
/// takes the daemon to answer one produce exactly one daemon round trip in
/// flight (plus one more once it's back) rather than ten queued IPC calls.
///
/// Deliberately dumb and GTK-free: `request`/`finish` are the only state
/// transitions, so the coalescing logic can be unit-tested without a display.
/// The batch of callbacks currently *running* is NOT stored here — see the
/// `RUNNING`/`STOP_RUNNING` thread_locals below — `waiting` only ever holds
/// callbacks for the *next* run.
struct ApplyQueue<Cb> {
    in_flight: bool,
    waiting: Vec<Cb>,
}

impl<Cb> ApplyQueue<Cb> {
    const fn new() -> Self {
        Self {
            in_flight: false,
            waiting: Vec::new(),
        }
    }

    /// Queue `cb`. Returns true when nothing is running and the caller must
    /// start a worker now — in which case the caller is expected to drain
    /// `waiting` itself immediately (it holds exactly `cb`) into its own
    /// "running batch" storage before any other request can arrive.
    fn request(&mut self, cb: Cb) -> bool {
        let was_idle = !self.in_flight;
        self.in_flight = true;
        self.waiting.push(cb);
        was_idle
    }

    /// Called when a worker finishes. If more requests queued up while it
    /// ran, returns them as the next batch to run (the queue stays marked
    /// in-flight for that run); otherwise the queue goes idle.
    fn finish(&mut self) -> Option<Vec<Cb>> {
        if self.waiting.is_empty() {
            self.in_flight = false;
            None
        } else {
            Some(std::mem::take(&mut self.waiting))
        }
    }
}

type ApplyCb = Box<dyn FnOnce(ApplyOutcome)>;

thread_local! {
    static APPLY_QUEUE: RefCell<ApplyQueue<ApplyCb>> = RefCell::new(ApplyQueue::new());
    /// Callbacks for the batch whose worker is currently running.
    static APPLY_RUNNING: RefCell<Vec<ApplyCb>> = const { RefCell::new(Vec::new()) };
    static STOP_QUEUE: RefCell<ApplyQueue<ApplyCb>> = RefCell::new(ApplyQueue::new());
    static STOP_RUNNING: RefCell<Vec<ApplyCb>> = const { RefCell::new(Vec::new()) };
}

/// True for every index except the last in a batch of length `len` — the
/// pure rule behind "only the last request in a coalesced batch is not
/// superseded", factored out so it's testable without the GTK/thread-local
/// machinery around it.
fn is_superseded(index: usize, len: usize) -> bool {
    index + 1 != len
}

/// Deliver a finished batch's result to every queued callback, marking all
/// but the last `superseded`, then start one more worker if anything queued
/// up while this one ran.
fn deliver_apply_batch(result: Result<(), String>) {
    let batch = APPLY_RUNNING.with(|r| std::mem::take(&mut *r.borrow_mut()));
    let len = batch.len();
    for (i, cb) in batch.into_iter().enumerate() {
        cb(ApplyOutcome {
            result: result.clone(),
            superseded: is_superseded(i, len),
        });
    }
    let next = APPLY_QUEUE.with(|q| q.borrow_mut().finish());
    if let Some(next_batch) = next {
        APPLY_RUNNING.with(|r| *r.borrow_mut() = next_batch);
        spawn_apply_worker();
    }
}

fn deliver_stop_batch(result: Result<(), String>) {
    let batch = STOP_RUNNING.with(|r| std::mem::take(&mut *r.borrow_mut()));
    let len = batch.len();
    for (i, cb) in batch.into_iter().enumerate() {
        cb(ApplyOutcome {
            result: result.clone(),
            superseded: is_superseded(i, len),
        });
    }
    let next = STOP_QUEUE.with(|q| q.borrow_mut().finish());
    if let Some(next_batch) = next {
        STOP_RUNNING.with(|r| *r.borrow_mut() = next_batch);
        spawn_stop_worker();
    }
}

/// Run `apply_blocking` on a worker thread and hand the result back to
/// `deliver_apply_batch` on the GTK thread. Mirrors the thread +
/// `async_channel` + `glib::spawn_future_local` pattern used by
/// `status::poll_once` / `spawn_thumbnail_batch`.
fn spawn_apply_worker() {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = apply_blocking().map_err(|e| format!("{e:#}"));
        let _ = tx.send_blocking(result);
    });
    glib::spawn_future_local(async move {
        let result = rx
            .recv()
            .await
            .unwrap_or_else(|_| Err("apply worker channel closed".to_string()));
        deliver_apply_batch(result);
    });
}

fn spawn_stop_worker() {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = stop_blocking().map_err(|e| format!("{e:#}"));
        let _ = tx.send_blocking(result);
    });
    glib::spawn_future_local(async move {
        let result = rx
            .recv()
            .await
            .unwrap_or_else(|_| Err("stop worker channel closed".to_string()));
        deliver_stop_batch(result);
    });
}

/// Apply `config` without blocking the GTK main thread.
///
/// `config.save()` runs synchronously here — it's a small local write, and
/// keeping "newest state always on disk" means that if several callers race,
/// the daemon's next `Apply` always picks up the last one written, regardless
/// of which worker happens to run last. On a save error `on_done` is still
/// called, but only via `glib::idle_add_local_once`, never inline: calling it
/// synchronously here would re-enter a `state.borrow_mut()` a caller may
/// still be holding a few frames up the stack (see the note on `edit_widgets`
/// in window.rs) and panic.
///
/// Rapid callers coalesce: at most one apply is in flight and one more
/// queued behind it; everyone still gets exactly one callback.
pub fn apply_async(config: &Config, on_done: impl FnOnce(ApplyOutcome) + 'static) {
    if let Err(e) = config.save() {
        let msg = format!("{e:#}");
        glib::idle_add_local_once(move || {
            on_done(ApplyOutcome {
                result: Err(msg),
                superseded: false,
            });
        });
        return;
    }
    let cb: ApplyCb = Box::new(on_done);
    let start = APPLY_QUEUE.with(|q| q.borrow_mut().request(cb));
    if start {
        let batch = APPLY_QUEUE.with(|q| std::mem::take(&mut q.borrow_mut().waiting));
        APPLY_RUNNING.with(|r| *r.borrow_mut() = batch);
        spawn_apply_worker();
    }
}

/// Stop the wallpaper without blocking the GTK main thread. Deliberately its
/// own queue rather than joining the apply queue: Stop and Apply are
/// different daemon requests and rarely race in practice (Stop tears the
/// wallpaper down; a Stop-then-Apply-again pattern is unusual UI flow), so
/// keeping them separate avoids a Stop waiting behind an unrelated Apply.
pub fn stop_async(on_done: impl FnOnce(ApplyOutcome) + 'static) {
    let cb: ApplyCb = Box::new(on_done);
    let start = STOP_QUEUE.with(|q| q.borrow_mut().request(cb));
    if start {
        let batch = STOP_QUEUE.with(|q| std::mem::take(&mut q.borrow_mut().waiting));
        STOP_RUNNING.with(|r| *r.borrow_mut() = batch);
        spawn_stop_worker();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_superseded_marks_all_but_last() {
        assert!(!is_superseded(0, 1)); // batch of one: not superseded
        assert!(is_superseded(0, 3));
        assert!(is_superseded(1, 3));
        assert!(!is_superseded(2, 3)); // last one wins
    }

    /// Mirrors how `apply_async` drives an `ApplyQueue`: the first request
    /// starts a worker and the caller immediately drains `waiting` into its
    /// own "running batch"; everything that arrives before that worker
    /// reports back queues up instead of starting a second one.
    #[test]
    fn rapid_requests_start_one_worker_and_batch_the_rest() {
        let mut q: ApplyQueue<u32> = ApplyQueue::new();

        assert!(q.request(0));
        let running: Vec<u32> = std::mem::take(&mut q.waiting);
        assert_eq!(running, vec![0]);

        for i in 1..10 {
            assert!(!q.request(i), "request {i} should not start a new worker");
        }

        // The worker for `running` finishes: the 9 queued requests become
        // the next batch, and the queue stays marked in-flight for it.
        let next = q.finish().expect("9 requests queued while the worker ran");
        assert_eq!(next, (1..10).collect::<Vec<_>>());

        // That batch's worker finishes too, and nothing else queued up.
        assert!(q.finish().is_none());
    }

    #[test]
    fn a_single_request_with_nothing_queued_goes_idle_after_finishing() {
        let mut q: ApplyQueue<u32> = ApplyQueue::new();
        assert!(q.request(0));
        let _running = std::mem::take(&mut q.waiting);
        assert!(q.finish().is_none());
    }
}

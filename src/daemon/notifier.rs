//! Event-driven admin notifications + update prompts for the daemon.
//!
//! Instead of polling, the always-running daemon holds **one** Supabase Realtime
//! websocket open and stays idle until a row is pushed. On each pushed
//! `notifications` row it raises a native desktop notification (freedesktop /
//! D-Bus via `notify-rust`), so it reaches users who have Fresco running even
//! with no window open.
//!
//! Flow per connection:
//!   1. **Catch-up read** — one HTTPS GET of recent published rows, to reconcile
//!      anything inserted while we were offline (Realtime only pushes events that
//!      happen *after* you subscribe; it never replays history). This is a single
//!      reconciliation read on connect, not a poll.
//!   2. **Subscribe** — open the websocket, join the channel, then block on reads.
//!   3. On disconnect/error, back off and reconnect (which re-runs the catch-up).
//!      A 429 from the handshake backs off far longer — see [`Backoff`].
//!
//! Dedup is kept in the daemon's own state file so it never races the GUI's
//! config writes. `update` rows are semver-gated against the running version.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::StatusCode;
use tungstenite::{HandshakeError, Message};

use crate::{t, tf};

/// Project host + publishable (anon) key — same as the GUI client. Safe to ship:
/// Row-Level Security (supabase/schema.sql) is what protects the data, letting
/// the anon role read only published notifications.
const HOST: &str = "mmoxgmvrpiaflfnsrynx.supabase.co";
const ANON_KEY: &str = "sb_publishable_eWKJzAuME5rstSxGyCBoHA_8hrTwkQM";

/// The version this binary was built as — the floor for update prompts.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where to send users when we can't install for them (e.g. Flatpak).
/// Follows the host this copy was installed from — see [`crate::update::Origin`].
fn releases_url() -> &'static str {
    crate::update::Origin::current().releases_page()
}

/// Send a Phoenix heartbeat a little more often than the server's idle timeout.
const HEARTBEAT: Duration = Duration::from_secs(25);

/// An admin-pushed notification row.
#[derive(Debug, Clone, Deserialize)]
struct Notification {
    id: String,
    title: String,
    body: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    version: Option<String>,
}

fn default_kind() -> String {
    "info".to_string()
}

/// Spawn the background notifier thread. Fire-and-forget: it owns its own
/// connection, reconnects on failure, and never touches the wallpaper loop.
pub fn spawn() {
    std::thread::Builder::new()
        .name("fresco-notifier".into())
        .spawn(run_forever)
        .ok();
}

/// Wait this long (plus jitter) before the first connect. `spawn()` runs at the
/// top of the daemon, before the single-instance lock and the `enabled = false`
/// early exit, so without this every aborted start (a second `frescod`, a
/// disabled wallpaper) still opened a connection to the backend and then died.
/// It also spreads logins instead of stampeding the backend at session start.
const STARTUP_DELAY: Duration = Duration::from_secs(15);

/// A session must stay up this long before earlier failures are forgiven.
/// Connecting is not health: a server that accepts the socket and then drops it
/// would otherwise reset the schedule on every attempt.
const BACKOFF_RESET_AFTER: Duration = Duration::from_secs(5 * 60);

/// Ladder for ordinary drops: 5 s doubling up to 5 min.
const DROP_BACKOFF: (Duration, Duration) = (Duration::from_secs(5), Duration::from_secs(5 * 60));
/// Ladder after a 429: 1 min doubling up to 30 min. A rate limit is the
/// backend telling us to go away, so the ordinary ladder is far too eager.
const LIMITED_BACKOFF: (Duration, Duration) =
    (Duration::from_secs(60), Duration::from_secs(30 * 60));
/// Longest `Retry-After` we will honour.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60 * 60);

/// The handshake was answered `429 Too Many Requests`; carries the server's
/// `Retry-After` when it sent one in seconds.
#[derive(Debug, Clone, Copy)]
struct RateLimited(Option<Duration>);

impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HTTP error: 429 Too Many Requests")
    }
}

impl std::error::Error for RateLimited {}

/// `Retry-After` as a delay in seconds. The HTTP-date form is not parsed: it
/// falls back to our own ladder, which is already at least a minute.
fn retry_after_secs(v: &str) -> Option<Duration> {
    v.trim().parse().ok().map(Duration::from_secs)
}

/// Reconnect pacing. Pure apart from the jitter the caller applies, so the whole
/// schedule is unit-tested without a socket.
#[derive(Default)]
struct Backoff {
    failures: u32,
}

impl Backoff {
    /// Delay (before jitter) to wait after a session that ran for `ran` and
    /// ended with `limited` set if it was a 429. Counts consecutive failures; a
    /// session that stayed up for [`BACKOFF_RESET_AFTER`] starts the ladder over.
    ///
    /// A `Retry-After` is a floor on top of the ladder, never a shortcut under
    /// it: waiting longer than asked is always allowed, and with thousands of
    /// clients sharing one backend "retry in 1 s" must not mean "all at once".
    fn next(&mut self, ran: Duration, limited: Option<RateLimited>) -> Duration {
        if ran >= BACKOFF_RESET_AFTER {
            self.failures = 0;
        }
        let n = self.failures.min(16);
        self.failures = self.failures.saturating_add(1);
        let (base, cap) = if limited.is_some() {
            LIMITED_BACKOFF
        } else {
            DROP_BACKOFF
        };
        let step = base.saturating_mul(1 << n).min(cap);
        match limited {
            Some(RateLimited(Some(ra))) => step.max(ra.min(MAX_RETRY_AFTER)),
            _ => step,
        }
    }
}

/// Stretch `d` by up to +50% (`j` in `[0, 1)`) so clients that dropped together
/// do not return together. Never shortens it: a `Retry-After` stays a floor.
fn with_jitter(d: Duration, j: f64) -> Duration {
    d.mul_f64(1.0 + 0.5 * j.clamp(0.0, 1.0))
}

/// Uniform-ish value in `[0, 1)` from std's per-process random hasher keys (no
/// `rand` dependency for one number).
fn jitter() -> f64 {
    use std::hash::BuildHasher;
    let r = std::collections::hash_map::RandomState::new().hash_one(0u8);
    (r >> 11) as f64 / (1u64 << 53) as f64
}

/// Reconnect loop. Ordinary drops retry on a short ladder; a 429 backs off for
/// much longer (see [`Backoff`]). The first 429 of each backoff step is logged
/// at `warn`; a repeat of the same step (the ladder has hit its cap) at `debug`,
/// so a long outage does not fill the log.
fn run_forever() {
    std::thread::sleep(with_jitter(STARTUP_DELAY, jitter()));
    let mut backoff = Backoff::default();
    let mut seen = Seen::load();
    let mut last_limited = None;
    loop {
        let started = Instant::now();
        let limited = match run_session(&mut seen) {
            Err(e) => match e.downcast_ref::<RateLimited>() {
                Some(r) => Some(*r),
                None => {
                    log::warn!("notifier: session ended: {e:#}");
                    None
                }
            },
            Ok(()) => None,
        };
        let step = backoff.next(started.elapsed(), limited);
        let wait = with_jitter(step, jitter());
        if limited.is_some() {
            let msg = format!(
                "notifier: rate limited (429); retrying in {}s",
                wait.as_secs()
            );
            if last_limited == Some(step) {
                log::debug!("{msg}");
            } else {
                log::warn!("{msg}");
            }
            last_limited = Some(step);
        }
        std::thread::sleep(wait);
    }
}

/// One connection: catch up on missed rows, then subscribe and read until the
/// socket errors or closes.
fn run_session(seen: &mut Seen) -> anyhow::Result<()> {
    catch_up(seen);

    let url = format!("wss://{HOST}/realtime/v1/websocket?apikey={ANON_KEY}&vsn=1.0.0");
    let request = url.into_client_request()?;

    // Connect the TCP stream ourselves so we can set a read timeout *before* the
    // TLS/websocket handshake. With a timeout in place, an idle `read()` returns
    // WouldBlock/TimedOut, which we use as the cue to send the next heartbeat.
    let tcp = TcpStream::connect((HOST, 443))?;
    tcp.set_read_timeout(Some(HEARTBEAT))?;
    let (mut socket, _resp) = match tungstenite::client_tls(request, tcp) {
        Err(HandshakeError::Failure(tungstenite::Error::Http(resp)))
            if resp.status() == StatusCode::TOO_MANY_REQUESTS =>
        {
            let wait = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(retry_after_secs);
            return Err(RateLimited(wait).into());
        }
        other => other?,
    };

    // Join the channel and ask for INSERTs on public.notifications.
    let join = serde_json::json!({
        "topic": "realtime:fresco-notifications",
        "event": "phx_join",
        "payload": {
            "config": {
                "postgres_changes": [
                    { "event": "INSERT", "schema": "public", "table": "notifications" }
                ]
            }
        },
        "ref": "1"
    });
    socket.send(Message::Text(join.to_string()))?;
    log::info!("notifier: subscribed to Realtime notifications");

    let mut hb_ref = 0u64;
    let mut last_hb = Instant::now();
    loop {
        match socket.read() {
            Ok(Message::Text(text)) => handle_message(&text, seen),
            Ok(Message::Ping(p)) => {
                socket.send(Message::Pong(p)).ok();
            }
            Ok(Message::Close(_)) => {
                anyhow::bail!("server closed the connection");
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut =>
            {
                // Idle read timeout — heartbeat cue, not an error.
            }
            Err(e) => return Err(e.into()),
        }

        if last_hb.elapsed() >= HEARTBEAT {
            hb_ref += 1;
            let hb = serde_json::json!({
                "topic": "phoenix",
                "event": "heartbeat",
                "payload": {},
                "ref": hb_ref.to_string()
            });
            socket.send(Message::Text(hb.to_string()))?;
            last_hb = Instant::now();
        }
    }
}

/// Reconcile rows inserted while we were offline. Shows at most the newest unseen
/// `update` and the newest unseen `info`; marks every fetched row seen so the
/// backlog never resurfaces.
fn catch_up(seen: &mut Seen) {
    let list = match fetch_recent() {
        Ok(list) => list,
        Err(e) => {
            log::warn!("notifier: catch-up fetch failed: {e:#}");
            return;
        }
    };

    let mut shown_update = false;
    let mut shown_info = false;
    for n in &list {
        if seen.contains(&n.id) {
            continue;
        }
        let is_update = n.kind == "update";
        if is_update && !shown_update {
            shown_update = true;
            handle(n);
        } else if !is_update && !shown_info {
            shown_info = true;
            handle(n);
        }
    }
    // Suppress the rest of the backlog.
    for n in &list {
        seen.insert(&n.id);
    }
}

/// One HTTPS GET of recent published notifications (newest first).
fn fetch_recent() -> anyhow::Result<Vec<Notification>> {
    let url = format!(
        "https://{HOST}/rest/v1/notifications\
         ?select=id,title,body,url,kind,version&published=eq.true&order=created_at.desc&limit=20"
    );
    let resp = ureq::get(&url)
        .set("apikey", ANON_KEY)
        .set("Authorization", &format!("Bearer {ANON_KEY}"))
        .call()?;
    Ok(resp.into_json()?)
}

/// Parse one websocket frame; act only on Postgres INSERT events.
fn handle_message(text: &str, seen: &mut Seen) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    if value.get("event").and_then(|e| e.as_str()) != Some("postgres_changes") {
        return; // phx_reply, presence, system frames — ignore.
    }
    let record = &value["payload"]["data"]["record"];
    let Ok(n) = serde_json::from_value::<Notification>(record.clone()) else {
        return;
    };
    if seen.contains(&n.id) {
        return;
    }
    seen.insert(&n.id);
    handle(&n);
}

/// What clicking the notification's action button does.
enum Click {
    OpenUrl(String),
    /// Run the bundled updater script (download + install the latest .deb) via pkexec.
    Update,
    /// Launch the Fresco GUI (the feedback dialog lives in its menu).
    OpenApp,
    /// Launch the GUI plainly. No flag is needed to land on the conversation:
    /// the window's launch check opens the support dialog by itself whenever
    /// there are unread replies, so this deliberately adds no CLI surface.
    OpenSupport,
}

/// How often to check whether the maintainer has replied to this user's
/// support thread. Long on purpose: a reply is a human writing back, so
/// minutes of latency cost nothing, and this must never look like polling
/// telemetry to someone watching their network.
const SUPPORT_POLL: Duration = Duration::from_secs(30 * 60);

/// How often to nudge for feedback, and how often the loop wakes to check.
///
/// Weekly. It was every five hours, which on a machine left running is up to
/// four prompts a day asking the same favour — frequent enough to read as
/// nagging from software that is otherwise silent, and the surest way to have
/// the reminder switched off before it is ever acted on. Once a week is still
/// present without being a recurring interruption.
const FEEDBACK_REMINDER_INTERVAL: u64 = 7 * 24 * 60 * 60; // seconds
const FEEDBACK_REMINDER_POLL: Duration = Duration::from_secs(10 * 60);

/// Spawn the periodic feedback reminder: a desktop notification once a week
/// until the user submits feedback once (then it stops for good), or turns
/// reminders off in Settings.
pub fn spawn_feedback_reminder() {
    std::thread::Builder::new()
        .name("fresco-feedback-nudge".into())
        .spawn(feedback_reminder_loop)
        .ok();
}

/// Watch this install's support thread for a maintainer reply, and raise a
/// desktop notification when one lands.
///
/// Only runs at all once the user has written to the maintainer: no thread
/// means no ticket file, and the loop exits immediately rather than polling for
/// something that cannot exist. Nothing here depends on the telemetry consent
/// tier, because a support thread is not telemetry — the user opened it
/// deliberately and is waiting for an answer.
pub fn spawn_support_watcher() {
    std::thread::Builder::new()
        .name("fresco-support-watch".into())
        .spawn(support_watch_loop)
        .ok();
}

fn support_watch_loop() {
    let mut announced = crate::support::seen_count();
    loop {
        std::thread::sleep(SUPPORT_POLL);
        if !crate::support::has_thread() {
            continue;
        }
        let Ok(messages) = crate::support::poll() else {
            continue;
        };
        // Compare against the GUI's seen count too: if the user read the reply
        // in the app between polls, there is nothing to announce.
        let seen = crate::support::seen_count().max(announced);
        if messages.len() <= seen {
            announced = seen;
            continue;
        }
        // Only a maintainer reply is worth interrupting someone for; the
        // user's own messages also grow the thread.
        let Some(last) = messages.last() else {
            continue;
        };
        if !last.is_maintainer() {
            announced = messages.len();
            continue;
        }
        announced = messages.len();
        notify(
            t!("The Fresco maintainer replied"),
            &last.body,
            Some((t!("Open"), Click::OpenSupport)),
        );
    }
}

fn feedback_reminder_loop() {
    let path = reminder_state_path();
    // First run: start the clock now so the first nudge lands a week from
    // install, not at first launch.
    if read_epoch(&path).is_none() {
        write_epoch(&path, epoch_now());
    }
    loop {
        std::thread::sleep(FEEDBACK_REMINDER_POLL);
        if crate::supabase::feedback_sent_marker().exists() {
            log::info!("feedback reminder: feedback already sent; stopping");
            return;
        }
        // Re-read config each cycle so a toggle takes effect without a restart.
        let enabled = crate::config::Config::load()
            .map(|c| c.feedback_reminders)
            .unwrap_or(true);
        if !enabled {
            continue;
        }
        let due = read_epoch(&path)
            .map(|t| epoch_now().saturating_sub(t) >= FEEDBACK_REMINDER_INTERVAL)
            .unwrap_or(true);
        if due {
            write_epoch(&path, epoch_now());
            notify(
                t!("Enjoying Fresco?"),
                t!("Tell us what's working and what isn't — it takes ten seconds and shapes the next release."),
                Some((t!("Send feedback"), Click::OpenApp)),
            );
        }
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_epoch(path: &PathBuf) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn write_epoch(path: &PathBuf, epoch: u64) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(path, epoch.to_string()).ok();
}

fn reminder_state_path() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("feedback-reminder-epoch")
}

/// Launch the Fresco GUI straight into the feedback dialog. `--feedback` is
/// forwarded to a running instance over D-Bus, so this works whether the app
/// is open or not.
fn open_app() {
    launch_gui(&["--feedback"]);
}

/// Start the GUI, handling the Flatpak indirection. Extra args are forwarded to
/// a running instance over D-Bus, so this works whether the app is open or not.
fn launch_gui(args: &[&str]) {
    let result = if crate::is_flatpak() {
        let mut cmd = std::process::Command::new("flatpak");
        cmd.args(["run", "io.github.dibbayajyotiroy.Fresco"]);
        cmd.args(args);
        cmd.spawn()
    } else {
        std::process::Command::new("fresco").args(args).spawn()
    };
    if let Err(e) = result {
        log::warn!("notifier: launching the GUI failed: {e}");
    }
}

/// Decide what to do with one notification. An `update` row is semver-gated; on a
/// normal (apt/.deb) install it offers an "Update now" button that runs the
/// updater script under the hood, while a Flatpak install (read-only, can't
/// apt-install) just links to the release page. An `info` row is a plain
/// announcement that opens its url if it has one.
fn handle(n: &Notification) {
    if n.kind == "update" {
        if let Some(version) = &n.version {
            if !is_newer(version) {
                log::info!("notifier: update {version} not newer than {CURRENT_VERSION}; ignoring");
                return;
            }
        }
        if crate::is_flatpak() {
            let url = n.url.clone().unwrap_or_else(|| releases_url().to_string());
            notify(&n.title, &n.body, Some((t!("Open"), Click::OpenUrl(url))));
        } else {
            notify(&n.title, &n.body, Some((t!("Update now"), Click::Update)));
        }
    } else if let Some(url) = n.url.clone() {
        notify(&n.title, &n.body, Some((t!("Open"), Click::OpenUrl(url))));
    } else {
        notify(&n.title, &n.body, None);
    }
}

/// True if `candidate` is a strictly newer semver than the running version.
fn is_newer(candidate: &str) -> bool {
    crate::update::is_newer(candidate, CURRENT_VERSION)
}

/// Tell the user a Wayland output gave up on live playback and fell back to a
/// paused static frame, instead of leaving them looking at a wallpaper that
/// silently stopped animating with no clue why. `hint` is the same one-line,
/// user-facing explanation `Status.error` already carries (`EarlyExit::hint`
/// or `spawn_fail_hint`), so the notification and the pill never disagree
/// about the cause. Called at most once per output per daemon run — see
/// `WlOutput::supervise`'s `giveup_reported` latch.
pub(crate) fn renderer_gave_up(connector: &str, hint: &str) {
    notify(
        &tf!("The {connector} wallpaper stopped playing", "connector" => connector),
        &tf!(
            "{hint} — showing a still frame instead. Fresco will try live playback again in a few minutes.",
            "hint" => hint
        ),
        Some((t!("Open Fresco"), Click::OpenSupport)),
    );
}

/// Raise a native desktop notification, optionally with one action button. The
/// show + click-wait run in a detached thread so nothing blocks the read loop.
fn notify(title: &str, body: &str, action: Option<(&str, Click)>) {
    let title = title.to_string();
    let body = body.to_string();
    let action = action.map(|(label, click)| (label.to_string(), click));
    std::thread::spawn(move || {
        let mut builder = notify_rust::Notification::new();
        builder
            .summary(&title)
            .body(&body)
            .appname("Fresco")
            .icon("io.github.dibbayajyotiroy.Fresco");
        if let Some((label, _)) = &action {
            builder.action("act", label);
        }
        match builder.show() {
            Ok(handle) => {
                if let Some((_, click)) = action {
                    handle.wait_for_action(|invoked| {
                        if invoked == "act" {
                            match click {
                                Click::OpenUrl(url) => open_url(&url),
                                Click::Update => run_updater(),
                                Click::OpenApp => open_app(),
                                Click::OpenSupport => launch_gui(&[]),
                            }
                        }
                    });
                }
            }
            Err(e) => log::warn!("notifier: desktop notification failed: {e}"),
        }
    });
}

/// Open a URL in the user's default browser.
fn open_url(url: &str) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(url).spawn() {
        log::warn!("notifier: xdg-open failed: {e}");
    }
}

/// Download + install the latest .deb by running the bundled updater script as
/// root via pkexec (the desktop's polkit agent prompts once). On success, prompt
/// the user to restart so the new binary takes over.
fn run_updater() {
    let Some(script) = crate::update::updater_script() else {
        log::warn!("notifier: updater script not found; opening releases page");
        open_url(releases_url());
        return;
    };
    log::info!(
        "notifier: launching updater via pkexec: {}",
        script.display()
    );
    match crate::update::run_updater_blocking() {
        crate::update::UpdateOutcome::Success => {
            notify(
                t!("Fresco updated"),
                t!("The latest version was installed. Restart Fresco to apply it."),
                None,
            );
        }
        crate::update::UpdateOutcome::AlreadyUpToDate => {
            log::info!("notifier: already on the latest version; nothing to install");
        }
        crate::update::UpdateOutcome::Unsupported => {
            log::warn!("notifier: unsupported install for auto-update; opening releases page");
            open_url(releases_url());
        }
        crate::update::UpdateOutcome::Failed(e) => log::warn!("notifier: {e}"),
    }
}

/// Fire-and-forget entry point for `Request::Update`: runs the updater on a
/// background thread and reports success/failure via the same desktop
/// notification path as the Supabase-pushed "Update now" action, without
/// blocking the daemon's main loop.
pub(crate) fn run_updater_async() {
    std::thread::spawn(run_updater);
}

/// Persistent set of notification ids already shown. Lives in the daemon's state
/// dir (next to frescod.log) so it never races the GUI's config.toml writes.
struct Seen {
    ids: HashSet<String>,
    path: PathBuf,
    dirty: bool,
}

impl Seen {
    fn load() -> Self {
        let path = state_path();
        let ids = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .map(|v| v.into_iter().collect())
            .unwrap_or_default();
        Self {
            ids,
            path,
            dirty: false,
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    fn insert(&mut self, id: &str) {
        if self.ids.insert(id.to_string()) {
            self.dirty = true;
            self.save();
        }
    }

    /// Persist, keeping the file from growing without bound.
    fn save(&mut self) {
        if !self.dirty {
            return;
        }
        let mut ids: Vec<&String> = self.ids.iter().collect();
        // Keep the most recent ~500; order isn't meaningful for a set, so this is
        // just a bound, accepting that the trimmed ones could re-show once.
        if ids.len() > 500 {
            ids.truncate(500);
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string(&ids) {
            std::fs::write(&self.path, json).ok();
        }
        self.dirty = false;
    }
}

fn state_path() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("seen-notifications.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn limited(retry_after: Option<u64>) -> Option<RateLimited> {
        Some(RateLimited(retry_after.map(s)))
    }

    #[test]
    fn limited_without_retry_after_starts_at_a_minute_and_doubles_to_the_cap() {
        let mut b = Backoff::default();
        let got: Vec<u64> = (0..8)
            .map(|_| b.next(s(1), limited(None)).as_secs())
            .collect();
        assert_eq!(got, [60, 120, 240, 480, 960, 1800, 1800, 1800]);
    }

    #[test]
    fn ordinary_drops_keep_the_short_ladder() {
        let mut b = Backoff::default();
        let got: Vec<u64> = (0..8).map(|_| b.next(s(1), None).as_secs()).collect();
        assert_eq!(got, [5, 10, 20, 40, 80, 160, 300, 300]);
    }

    #[test]
    fn retry_after_is_a_floor_clamped_to_an_hour() {
        let mut b = Backoff::default();
        assert_eq!(b.next(s(1), limited(Some(600))), s(600));
        // A short Retry-After never undercuts our own ladder (second step = 120 s).
        assert_eq!(b.next(s(1), limited(Some(1))), s(120));
        assert_eq!(b.next(s(1), limited(Some(0))), s(240));
        assert_eq!(b.next(s(1), limited(Some(86_400))), MAX_RETRY_AFTER);
    }

    #[test]
    fn never_overflows_however_long_the_outage() {
        let mut b = Backoff::default();
        for _ in 0..10_000 {
            assert!(b.next(s(1), limited(None)) <= LIMITED_BACKOFF.1);
        }
    }

    #[test]
    fn only_a_healthy_session_resets_the_ladder() {
        let mut b = Backoff::default();
        for _ in 0..4 {
            b.next(s(1), limited(None));
        }
        // Up for just under the threshold: not healthy, keeps climbing.
        assert_eq!(b.next(BACKOFF_RESET_AFTER - s(1), limited(None)), s(960));
        // Healthy: forgiven, so the next failure is a plain first step.
        assert_eq!(b.next(BACKOFF_RESET_AFTER, None), s(5));
        assert_eq!(b.next(s(1), limited(None)), s(120));
    }

    #[test]
    fn jitter_only_lengthens_and_stays_within_half() {
        let d = s(60);
        assert_eq!(with_jitter(d, 0.0), d);
        assert_eq!(with_jitter(d, 1.0), s(90));
        assert!(with_jitter(d, 0.5) > d && with_jitter(d, 0.5) < s(90));
        // Out-of-range input is clamped: never shorter, never past +50%.
        assert_eq!(with_jitter(d, -3.0), d);
        assert_eq!(with_jitter(d, 9.0), s(90));
        for _ in 0..1000 {
            let j = jitter();
            assert!((0.0..1.0).contains(&j), "jitter {j} out of range");
        }
    }

    #[test]
    fn retry_after_header_parsing() {
        assert_eq!(retry_after_secs("30"), Some(s(30)));
        assert_eq!(retry_after_secs(" 45 "), Some(s(45)));
        assert_eq!(retry_after_secs("Wed, 21 Oct 2015 07:28:00 GMT"), None);
        assert_eq!(retry_after_secs("-5"), None);
        assert_eq!(retry_after_secs(""), None);
    }
}

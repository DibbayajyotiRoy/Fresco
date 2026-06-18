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
use tungstenite::Message;

/// Project host + publishable (anon) key — same as the GUI client. Safe to ship:
/// Row-Level Security (supabase/schema.sql) is what protects the data, letting
/// the anon role read only published notifications.
const HOST: &str = "mmoxgmvrpiaflfnsrynx.supabase.co";
const ANON_KEY: &str = "sb_publishable_eWKJzAuME5rstSxGyCBoHA_8hrTwkQM";

/// The version this binary was built as — the floor for update prompts.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where to send users when we can't install for them (e.g. Flatpak).
const RELEASES_URL: &str = "https://github.com/DibbayajyotiRoy/fresco/releases/latest";

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

/// Reconnect loop with exponential backoff. A session that stays up for a while
/// resets the backoff so transient drops don't ramp it up forever.
fn run_forever() {
    let mut backoff = Duration::from_secs(5);
    let mut seen = Seen::load();
    loop {
        let started = Instant::now();
        if let Err(e) = run_session(&mut seen) {
            log::warn!("notifier: session ended: {e:#}");
        }
        if started.elapsed() > Duration::from_secs(120) {
            backoff = Duration::from_secs(5);
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(Duration::from_secs(300));
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
    let (mut socket, _resp) = tungstenite::client_tls(request, tcp)?;

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
            let url = n.url.clone().unwrap_or_else(|| RELEASES_URL.to_string());
            notify(&n.title, &n.body, Some(("Open", Click::OpenUrl(url))));
        } else {
            notify(&n.title, &n.body, Some(("Update now", Click::Update)));
        }
    } else if let Some(url) = n.url.clone() {
        notify(&n.title, &n.body, Some(("Open", Click::OpenUrl(url))));
    } else {
        notify(&n.title, &n.body, None);
    }
}

/// True if `candidate` is a strictly newer semver than the running version.
fn is_newer(candidate: &str) -> bool {
    let strip = |v: &str| v.trim().trim_start_matches('v').to_string();
    match (
        semver::Version::parse(&strip(candidate)),
        semver::Version::parse(&strip(CURRENT_VERSION)),
    ) {
        (Ok(c), Ok(cur)) => c > cur,
        _ => false,
    }
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
    let Some(script) = updater_script() else {
        log::warn!("notifier: updater script not found; opening releases page");
        open_url(RELEASES_URL);
        return;
    };
    log::info!(
        "notifier: launching updater via pkexec: {}",
        script.display()
    );
    match std::process::Command::new("pkexec").arg(&script).status() {
        Ok(status) if status.success() => {
            notify(
                "Fresco updated",
                "The latest version was installed. Restart Fresco to apply it.",
                None,
            );
        }
        Ok(status) => log::warn!("notifier: updater exited with {status}"),
        Err(e) => log::warn!("notifier: failed to launch pkexec: {e}"),
    }
}

/// Locate the bundled updater script: beside our binary (dev tree), then the
/// prefix-relative libexec dir, then the absolute .deb install path.
fn updater_script() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("fresco-update.sh"));
            candidates.push(dir.join("../lib/fresco/fresco-update.sh"));
        }
    }
    candidates.push(PathBuf::from("/usr/lib/fresco/fresco-update.sh"));
    candidates.into_iter().find(|p| p.is_file())
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

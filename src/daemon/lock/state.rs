//! Lock-state monitor: merges every source the daemon can observe into one
//! debounced Locked/Unlocked answer.
//!
//! # Sources, and why there are so many
//!
//! No single desktop exposes one reliable "is the session locked" signal, so
//! this module watches all of them and lets [`LockState`] arbitrate:
//!
//! * **COSMIC's lockfile** — `cosmic-greeter` creates
//!   `$XDG_RUNTIME_DIR/cosmic-greeter-$XDG_SESSION_ID.lock` when it starts
//!   locking (found by pattern: an autostarted daemon has no
//!   `$XDG_SESSION_ID`) and removes it on unlock, and — this is the reason it exists at
//!   all in this list — **cosmic-greeter never emits logind's `Unlock`
//!   signal**, so on COSMIC the lockfile is the only authoritative unlock
//!   source. Polled at ≤2 Hz ([`LOCKFILE_POLL`]); no `inotify` (no new
//!   crates, and a plain `stat` every half second is a rounding error next to
//!   the mpv IPC traffic this daemon already generates).
//! * **logind, system bus** — one supervised `gdbus monitor --system --dest
//!   org.freedesktop.login1` child. `Session.Lock`/`Session.Unlock` on *our*
//!   session's object path (resolved via `Manager.GetSession`, falling back to
//!   `Manager.GetSessionByPID`, then to the user's `Display` session; retried
//!   every [`SESSION_RETRY`] while unresolved) and `Manager.PrepareForSleep (true,)`
//!   (system-wide, not session-scoped) are the three signals this module
//!   acts on.
//! * **The session-bus screensaver interfaces** — one supervised `gdbus
//!   monitor --session --dest <interface>` child, where `<interface>` is
//!   whichever of `org.freedesktop.ScreenSaver` / `org.gnome.ScreenSaver` /
//!   `org.cinnamon.ScreenSaver` / `org.mate.ScreenSaver` / `org.xfce.ScreenSaver`
//!   matches the detected [`super::hosts::HostKind`] — see
//!   [`screensaver_interface`]. Not spawned at all on a host with no known
//!   interface (COSMIC, a wlroots compositor, a bare X11 WM), so a session
//!   with nothing to watch there opens no extra bus connection.
//! * **IPC and process facts** — `Request::LockNotify` (an out-of-process
//!   renderer reporting its own start/stop), the exit of a host child the
//!   daemon itself spawned (`hosts::RunningHost::child`), and every lock
//!   socket disconnecting. These never arrive through this module's own
//!   children; the daemon's run loops feed them in directly — see
//!   [`LockMonitor::on_lock_notify`], [`on_host_child_exited`](LockMonitor::on_host_child_exited)
//!   and [`on_sockets_disconnected`](LockMonitor::on_sockets_disconnected).
//!
//! # Authoritative-source rules
//!
//! Every source may vote **Locked** — believing "locked" a little too eagerly
//! is the fail-closed direction (a spurious lock vote costs one extra widget
//! swap; a spurious unlock vote could show desktop-only content over a real
//! lock screen). **Unlocked** is stricter: [`LockState`] only accepts it from
//! a source that is actually authoritative for the *current* host —
//! `LockState::vote_for` is the one function that encodes this table:
//!
//! | Host | Authoritative unlock source(s) |
//! |---|---|
//! | COSMIC | lockfile absent — **only** this; see below |
//! | KDE / GNOME / Cinnamon / MATE / Xfce | that host's `ActiveChanged(false)`, or logind `Unlock` |
//! | everything else | logind `Unlock` only |
//!
//! `logind Unlock` is trusted on every host **except** COSMIC. On every other
//! host, this daemon's only way to see the lock's real on-screen state is a
//! bus signal, and logind's own `Unlock` is as good as any of them. COSMIC is
//! the deliberate exception: `cosmic-greeter` paints directly into the same
//! mpvpaper surface this daemon owns
//! (`hosts::LockTargets::Desktop`), so believing a *wrong* unlock there does
//! not just mis-schedule a redraw — it can put desktop-only content
//! (lyrics, an unmasked greeting) on screen while `cosmic-greeter`'s panel is
//! still genuinely up, which is exactly the "desktop-only content over a real
//! lock screen" failure this whole table exists to prevent. `cosmic-greeter`
//! itself never emits `Session.Unlock` — the lockfile is what actually
//! carries this signal in practice — but *something else* calling
//! `loginctl unlock-session` while the greeter is still on screen is exactly
//! the scenario this exception guards against, not a case assumed away.
//!
//! # Debounce, and the one signal that skips it
//!
//! Every vote that disagrees with the current state starts (or refreshes) a
//! [`DEBOUNCE`] timer; the vote only commits once that timer has held for the
//! whole window with no newer, conflicting vote — "no flapping" from a burst
//! of near-simultaneous signals during a real transition. The one exception
//! is `PrepareForSleep(true)`: the plan's own requirement is to "swap widgets
//! immediately so nothing desktop-only is on screen at resume", and a machine
//! can finish suspending within milliseconds of that signal, so it commits
//! **immediately**, bypassing the debounce entirely.
//!
//! That immediate "locked" is a placeholder, not a lock: the session is only
//! *about to* sleep, and nothing votes "unlocked" for a session that was never
//! locked. So `PrepareForSleep(false)` (the resume) schedules a fall back to
//! whatever the session was before it slept, after [`WAKE_GRACE`] for the
//! other sources to report a lock the greeter raised on resume. A session that
//! really was locked stays locked until its own unlock source fires; one that
//! was not goes back to unlocked, instead of keeping its desktop widgets
//! cleared until the daemon restarted.
//!
//! # Process supervision
//!
//! [`MonitorChild`] owns one `gdbus monitor` child: a reader thread streams
//! its stdout lines into a channel, [`MonitorChild::poll_lines`] drains
//! whatever has arrived without blocking, and a dead child (exited, or never
//! spawned because `gdbus` is missing) is restarted with exponential backoff
//! the next time it is polled — never inline, never blocking the caller. Every
//! failure degrades: a machine with no `gdbus` still gets the COSMIC lockfile
//! and IPC sources, just not the bus ones.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use super::hosts::HostKind;

/// How long a losing vote must hold uncontested before it commits — see the
/// module docs' "Debounce" section. `PrepareForSleep(true)` is the one
/// exception and bypasses this entirely.
const DEBOUNCE: Duration = Duration::from_millis(250);

/// How long after `PrepareForSleep(false)` the other lock sources get to
/// report before the machine falls back to its pre-sleep state — see
/// [`StateMachine::apply`]'s wake branch. A greeter that locks on resume writes
/// its lockfile within the next poll or two; waiting this long keeps the desktop
/// widgets from flashing over a lock screen that is about to appear.
const WAKE_GRACE: Duration = Duration::from_secs(2);

/// How long to wait before asking logind for our session again after a lookup
/// failed. The lookup forks `gdbus`, so repeating it on every run-loop tick
/// (about ten a second) cost a process spawn per tick for the whole session on
/// any daemon logind cannot tie to a session — which is every systemd user
/// service, i.e. every autostarted daemon.
const SESSION_RETRY: Duration = Duration::from_secs(30);

/// Ceiling on how often the COSMIC lockfile is `stat`-ed — "≤2 Hz" per the
/// design brief.
const LOCKFILE_POLL: Duration = Duration::from_millis(500);

/// Initial, and per-attempt-doubled, backoff before a dead `gdbus monitor`
/// child is restarted.
const RESTART_BACKOFF_BASE: Duration = Duration::from_millis(500);
/// Ceiling the backoff above doubles up to — a `gdbus` that is simply not
/// installed must not be retried more than twice a minute forever.
const RESTART_BACKOFF_MAX: Duration = Duration::from_secs(30);

/// `gdbus --timeout` equivalent isn't available on `monitor` (it's a
/// long-lived subscription, not a call), so there is nothing to bound here —
/// the process itself is what gets superseded on restart.
/// Merged Locked/Unlocked answer, plus the one variant that exists purely so
/// a caller can log *why* it went to the lock arrangement — see the module
/// docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// `Locked::Locked` reads clearly in every call site that matters
// (`state.current() == Locked::Locked`, `Some(Locked::Locked)`); renaming it
// to dodge the lint would make those call sites worse to read for the lint's
// sake alone.
#[allow(clippy::enum_variant_names)]
pub enum Locked {
    Unlocked,
    Locked,
    /// `Manager.PrepareForSleep (true,)` fired: the system is about to
    /// suspend. Treated identically to [`Locked::Locked`] by
    /// [`Locked::is_locked`] — the only thing that differs is *why*.
    SleepImminent,
}

impl Locked {
    /// Whether the lock engine should be showing lock-mode content right now.
    /// `true` for both [`Locked::Locked`] and [`Locked::SleepImminent`].
    pub fn is_locked(self) -> bool {
        !matches!(self, Locked::Unlocked)
    }
}

/// One raw fact this module can observe or be told about. Private: callers
/// reach the state machine only through [`LockMonitor`]'s typed methods, so a
/// future ninth event can't be constructed anywhere it wasn't accounted for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    CosmicLockfile(bool),
    LogindLock,
    LogindUnlock,
    /// `Manager.PrepareForSleep`'s one bool argument.
    LogindPrepareForSleep(bool),
    ScreensaverActiveChanged(bool),
    IpcLockNotify(bool),
    HostChildExited,
    AllSocketsDisconnected,
}

/// The pure debounced state machine — no I/O, no threads, no clock of its
/// own (every method takes `now` explicitly), so the whole authoritative-
/// source/debounce/immediacy design is unit-testable with synthetic event
/// sequences and no real D-Bus, filesystem, or process.
#[derive(Debug)]
struct StateMachine {
    host: HostKind,
    locked: Locked,
    pending: Option<Locked>,
    pending_since: Instant,
    /// What the session was (or was about to become) when `PrepareForSleep(true)`
    /// arrived — where a resume falls back to. [`Locked::SleepImminent`] is a
    /// pessimistic placeholder, not a lock: nothing ever votes "unlocked" for a
    /// session that was never locked, so without this a suspend left the
    /// machine "locked" until the daemon restarted.
    before_sleep: Locked,
}

impl StateMachine {
    fn new(host: HostKind, now: Instant) -> Self {
        StateMachine {
            host,
            locked: Locked::Unlocked,
            pending: None,
            pending_since: now,
            before_sleep: Locked::Unlocked,
        }
    }

    fn current(&self) -> Locked {
        self.locked
    }

    /// What `event` votes for, or `None` when it is not a vote at all (e.g.
    /// `PrepareForSleep(false)`, which only means "the machine is awake
    /// again" and is never itself an unlock — the real `Unlock` still has to
    /// arrive on its own authoritative source), or when it votes Unlocked on
    /// a host where its source is not authoritative for that — see the module
    /// docs' table.
    fn vote_for(&self, event: &Event) -> Option<Locked> {
        match *event {
            Event::CosmicLockfile(true) => Some(Locked::Locked),
            Event::CosmicLockfile(false) => {
                matches!(self.host, HostKind::Cosmic { .. }).then_some(Locked::Unlocked)
            }
            Event::LogindLock => Some(Locked::Locked),
            // Trusted everywhere except COSMIC — see the module docs'
            // "Authoritative-source rules" for why COSMIC is the deliberate
            // exception rather than an oversight.
            Event::LogindUnlock => {
                (!matches!(self.host, HostKind::Cosmic { .. })).then_some(Locked::Unlocked)
            }
            Event::LogindPrepareForSleep(true) => Some(Locked::SleepImminent),
            Event::LogindPrepareForSleep(false) => None,
            // Locking is trusted from anywhere; unlocking only from a host
            // that actually has this interface — see [`screensaver_interface`]
            // and the module docs' table. Without this check a stray (or
            // mis-routed) `ActiveChanged(false)` could unlock a COSMIC session
            // that has no screensaver bus name at all.
            Event::ScreensaverActiveChanged(active) => {
                if active {
                    Some(Locked::Locked)
                } else if screensaver_interface(self.host).is_some() {
                    Some(Locked::Unlocked)
                } else {
                    None
                }
            }
            Event::IpcLockNotify(locked) => Some(if locked {
                Locked::Locked
            } else {
                Locked::Unlocked
            }),
            // A daemon-spawned locker going away (its child exited, or every
            // socket it exposed disconnected) is the daemon's own first-party
            // evidence the lock ended — authoritative on every host, since
            // only the wlroots/X11 hosts ever produce it in the first place.
            Event::HostChildExited | Event::AllSocketsDisconnected => Some(Locked::Unlocked),
        }
    }

    /// Feed one event and settle immediately. Returns the new state only when
    /// it actually changed.
    fn apply(&mut self, event: Event, now: Instant) -> Option<Locked> {
        // Waking is not a vote, but it ends the placeholder: fall back to what
        // the session was before it slept, once the other sources have had
        // [`WAKE_GRACE`] to say otherwise. A real lock stays locked (its own
        // unlock source still has to fire); a session that was never locked
        // goes back to unlocked instead of staying "locked" until a restart.
        if matches!(event, Event::LogindPrepareForSleep(false)) {
            if self.locked == Locked::SleepImminent && self.pending.is_none() {
                self.pending = Some(self.before_sleep);
                self.pending_since = now + WAKE_GRACE;
            }
            return None;
        }
        let vote = self.vote_for(&event)?;
        // `PrepareForSleep(true)` bypasses the debounce — see the module
        // docs' "Debounce, and the one signal that skips it".
        if matches!(event, Event::LogindPrepareForSleep(true)) {
            if self.locked != Locked::SleepImminent {
                // A vote still settling counts: a lock a few milliseconds from
                // committing must survive the sleep.
                self.before_sleep = self.pending.unwrap_or(self.locked);
            }
            self.pending = None;
            return self.commit(vote);
        }
        if vote == self.locked {
            // Confirms the current state: cancel any pending flip toward the
            // *other* direction rather than let a now-stale one commit later.
            self.pending = None;
            return None;
        }
        if self.pending != Some(vote) {
            self.pending = Some(vote);
            self.pending_since = now;
        }
        self.settle(now)
    }

    /// Advance time with no new event, so a debounced transition can commit
    /// even if nothing else happens to call [`Self::apply`] right at the
    /// [`DEBOUNCE`] boundary. Returns the new state only when it changed.
    fn tick(&mut self, now: Instant) -> Option<Locked> {
        self.settle(now)
    }

    fn settle(&mut self, now: Instant) -> Option<Locked> {
        match self.pending {
            // Saturating: a wake fallback is scheduled `WAKE_GRACE` ahead, so
            // `pending_since` is briefly in the future.
            Some(v) if now.saturating_duration_since(self.pending_since) >= DEBOUNCE => {
                self.pending = None;
                self.commit(v)
            }
            _ => None,
        }
    }

    fn commit(&mut self, v: Locked) -> Option<Locked> {
        if self.locked == v {
            return None;
        }
        self.locked = v;
        Some(v)
    }
}

// ---------------------------------------------------------------------------
// gdbus monitor line parsing
// ---------------------------------------------------------------------------

/// The five session-bus screensaver interfaces every host in the plan's §4
/// table might expose `ActiveChanged` under. Checked in [`screensaver_event`]
/// regardless of which one this session actually spawned a monitor for
/// (see [`screensaver_interface`]) — cheap, and it means a desktop that
/// happens to also register the plain freedesktop name alongside its own
/// still gets recognised.
const SCREENSAVER_INTERFACES: [&str; 5] = [
    "org.freedesktop.ScreenSaver",
    "org.gnome.ScreenSaver",
    "org.cinnamon.ScreenSaver",
    "org.mate.ScreenSaver",
    "org.xfce.ScreenSaver",
];

/// Which session-bus screensaver interface to monitor for `host`, or `None`
/// when this host has no known one — COSMIC (the lockfile already covers it),
/// a wlroots compositor or bare X11 WM (no desktop screensaver daemon to
/// watch), Deepin (`dde-lock` exposes no such interface — see
/// `docs/plan-lock-screen.md` §10.5) and the unsupported catch-all.
fn screensaver_interface(host: HostKind) -> Option<&'static str> {
    match host {
        HostKind::Kde => Some("org.freedesktop.ScreenSaver"),
        HostKind::Gnome => Some("org.gnome.ScreenSaver"),
        HostKind::Cinnamon => Some("org.cinnamon.ScreenSaver"),
        HostKind::Mate => Some("org.mate.ScreenSaver"),
        HostKind::Xfce => Some("org.xfce.ScreenSaver"),
        HostKind::Cosmic { .. }
        | HostKind::Deepin
        | HostKind::Wlroots
        | HostKind::X11Wm
        | HostKind::Unsupported => None,
    }
}

/// One parsed `gdbus monitor` output line: `<path>: <interface>.<member>
/// (<args>)`. Verified live against this exact `gdbus` (2026-09-28, session
/// bus, `org.freedesktop.DBus`'s own `NameOwnerChanged`):
///
/// ```text
/// /org/freedesktop/DBus: org.freedesktop.DBus.NameOwnerChanged (':1.1411', '', ':1.1411')
/// ```
///
/// The two banner lines `gdbus monitor` prints once at start —
/// `"Monitoring signals from all objects owned by <name>"` and `"The name
/// <name> is owned by <unique-name>"` — do not contain `": "` followed by a
/// `" ("`-delimited member the way every signal line does, so they parse to
/// `None` here rather than needing a separate skip step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorLine<'a> {
    path: &'a str,
    /// `"<interface>.<member>"`, e.g. `"org.freedesktop.login1.Session.Lock"`.
    member: &'a str,
    /// The text between the outer parens, e.g. `"true,"` or empty.
    args: &'a str,
}

fn parse_monitor_line(line: &str) -> Option<MonitorLine<'_>> {
    let (path, rest) = line.split_once(": ")?;
    let (member, rest) = rest.split_once(" (")?;
    let args = rest.strip_suffix(')')?;
    if path.is_empty() || member.is_empty() {
        return None;
    }
    Some(MonitorLine { path, member, args })
}

/// The one boolean argument out of a one-arg tuple's printed text, e.g.
/// `"true,"`. This is **not** [`crate::userinfo`]'s `parse_gvariant_string`
/// shape (`<'...'>`) — that quoting is how `gdbus` prints a `v` (variant); a
/// plain, non-variant `b` argument in a signal's own signature (every signal
/// this module cares about: `PrepareForSleep(b)`, `ActiveChanged(b)`) prints
/// bare, with no angle brackets at all.
fn parse_bool_arg(args: &str) -> Option<bool> {
    match args.trim().trim_end_matches(',').trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Map one already-parsed system-bus line to an [`Event`], when it is one of
/// the three logind signals this module acts on and — for the two
/// session-scoped ones — its path is *our* session.
///
/// `Session.Lock`/`Session.Unlock` carry no arguments (confirmed live via
/// `gdbus introspect` against a real logind session, 2026-09-28); the fixture
/// lines below construct them from that verified grammar, since triggering a
/// real lock/suspend to capture one live is exactly what this feature must
/// never do as a side effect of its own tests.
fn login1_event(line: &MonitorLine, our_session: Option<&str>) -> Option<Event> {
    match line.member {
        "org.freedesktop.login1.Session.Lock" if Some(line.path) == our_session => {
            Some(Event::LogindLock)
        }
        "org.freedesktop.login1.Session.Unlock" if Some(line.path) == our_session => {
            Some(Event::LogindUnlock)
        }
        "org.freedesktop.login1.Manager.PrepareForSleep" => {
            parse_bool_arg(line.args).map(Event::LogindPrepareForSleep)
        }
        _ => None,
    }
}

/// Map one already-parsed session-bus line to an [`Event`] when its member is
/// `<one of [`SCREENSAVER_INTERFACES`]>.ActiveChanged`.
fn screensaver_event(line: &MonitorLine) -> Option<Event> {
    for iface in SCREENSAVER_INTERFACES {
        if let Some(".ActiveChanged") = line.member.strip_prefix(iface) {
            return parse_bool_arg(line.args).map(Event::ScreensaverActiveChanged);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Our own logind session path
// ---------------------------------------------------------------------------

/// Pull the one `objectpath '...'` out of a `gdbus call` reply shaped like
/// `(objectpath '/org/freedesktop/login1/session/_34',)` — the exact reply
/// `Manager.GetSession`/`GetSessionByPID` produce, confirmed live
/// (2026-09-28) against this machine's own session.
fn parse_object_path_reply(out: &str) -> Option<String> {
    let s = out.trim();
    let s = s.strip_prefix('(')?;
    let s = s.strip_prefix("objectpath ")?;
    let s = s.trim_start();
    let s = s.strip_prefix('\'')?;
    let end = s.find('\'')?;
    Some(s[..end].to_string())
}

/// `gdbus --timeout`, seconds — short enough that a wedged/absent logind
/// cannot hold up daemon startup. Matches `userinfo.rs`'s own constant.
const CALL_TIMEOUT_SECS: &str = "2";

fn gdbus_call_session_path(method: &str, arg: &str) -> Option<String> {
    let out = Command::new("gdbus")
        .args(["call", "--system", "--timeout", CALL_TIMEOUT_SECS])
        .args([
            "--dest",
            "org.freedesktop.login1",
            "--object-path",
            "/org/freedesktop/login1",
        ])
        .args([
            "--method",
            &format!("org.freedesktop.login1.Manager.{method}"),
            arg,
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_object_path_reply(&String::from_utf8_lossy(&out.stdout))
}

/// The session path inside a `Properties.Get ... Display` reply such as
/// `(<('4', objectpath '/org/freedesktop/login1/session/_34')>,)`.
fn parse_display_session_reply(out: &str) -> Option<String> {
    let rest = out.split_once("objectpath '")?.1;
    let path = &rest[..rest.find('\'')?];
    // logind answers "/" when the user has no graphical session.
    (path.starts_with('/') && path.len() > 1).then(|| path.to_string())
}

/// The user's graphical session, from logind's `User.Display` property.
///
/// The fallback for a daemon logind cannot tie to a session: an autostarted one
/// runs as a systemd *user service*, outside every session scope, so
/// `GetSessionByPID` answers `NoSessionForPID` and `$XDG_SESSION_ID` is not in
/// its environment either.
fn resolve_display_session_path() -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self").ok()?.uid().to_string();
    let user = gdbus_call_session_path("GetUser", &uid)?;
    let out = Command::new("gdbus")
        .args(["call", "--system", "--timeout", CALL_TIMEOUT_SECS])
        .args(["--dest", "org.freedesktop.login1", "--object-path", &user])
        .args([
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.freedesktop.login1.User",
            "Display",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_display_session_reply(&String::from_utf8_lossy(&out.stdout))
}

/// Resolve our own logind session object path: `Manager.GetSession` on
/// `$XDG_SESSION_ID` first (set by every login manager this daemon runs
/// under), then `Manager.GetSessionByPID` on our own pid, then the user's
/// graphical session ([`resolve_display_session_path`]) for a daemon that
/// belongs to no session scope. `None` — logind unreachable, or genuinely no
/// session — degrades to "logind's `Lock`/`Unlock` are never matched", not a
/// panic; the COSMIC lockfile and IPC sources still work.
fn resolve_our_session_path() -> Option<String> {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        if !id.is_empty() {
            if let Some(p) = gdbus_call_session_path("GetSession", &id) {
                return Some(p);
            }
        }
    }
    let pid = std::process::id().to_string();
    gdbus_call_session_path("GetSessionByPID", &pid).or_else(resolve_display_session_path)
}

// ---------------------------------------------------------------------------
// Supervised gdbus monitor child
// ---------------------------------------------------------------------------

/// One supervised child process: a reader thread streams its stdout lines
/// into a channel, and a dead child (exited, or never spawned) is restarted
/// with backoff the next time [`MonitorChild::poll_lines`] is called — never
/// inline, never blocking. `program`/`args` are parameters (not hardcoded
/// `"gdbus"`) purely so the tests can point this at a trivial `sh`/`printf`
/// stand-in instead of needing a real D-Bus.
struct MonitorChild {
    program: &'static str,
    args: Vec<&'static str>,
    child: Option<Child>,
    lines: Option<Receiver<String>>,
    next_attempt: Instant,
    backoff: Duration,
    warned: bool,
}

impl MonitorChild {
    fn new(program: &'static str, args: Vec<&'static str>, now: Instant) -> Self {
        MonitorChild {
            program,
            args,
            child: None,
            lines: None,
            next_attempt: now,
            backoff: RESTART_BACKOFF_BASE,
            warned: false,
        }
    }

    fn ensure_running(&mut self, now: Instant) {
        if self.child.is_some() || now < self.next_attempt {
            return;
        }
        match Command::new(self.program)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                // `stdout(Stdio::piped())` just above guarantees this.
                let stdout = child.stdout.take().expect("piped stdout");
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        if tx.send(line).is_err() {
                            break; // nobody polling any more; stop reading.
                        }
                    }
                    // EOF: the child exited (or closed stdout). Dropping `tx`
                    // here is what turns the next `poll_lines` into a
                    // `Disconnected`, which is this module's only way to
                    // learn the child is gone without a blocking `wait`.
                });
                self.child = Some(child);
                self.lines = Some(rx);
                self.backoff = RESTART_BACKOFF_BASE;
                self.warned = false;
            }
            Err(e) => {
                if !self.warned {
                    self.warned = true;
                    log::warn!(
                        "lock state: failed to spawn `{} {}`: {e} — this event source is \
                         unavailable; retrying with backoff",
                        self.program,
                        self.args.join(" ")
                    );
                }
                self.schedule_retry(now);
            }
        }
    }

    fn schedule_retry(&mut self, now: Instant) {
        self.next_attempt = now + self.backoff;
        self.backoff = (self.backoff * 2).min(RESTART_BACKOFF_MAX);
    }

    /// Drain whatever has arrived since the last call, restarting the child
    /// with backoff first if it isn't running. Never blocks: a `gdbus` that
    /// is merely slow to print the next line just means an empty `Vec` this
    /// time, not a wait.
    fn poll_lines(&mut self, now: Instant) -> Vec<String> {
        self.ensure_running(now);
        let mut lines = Vec::new();
        let mut dead = false;
        if let Some(rx) = &self.lines {
            loop {
                match rx.try_recv() {
                    Ok(line) => lines.push(line),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        dead = true;
                        break;
                    }
                }
            }
        }
        if !dead {
            if let Some(child) = &mut self.child {
                // Catches the process having exited even if its reader
                // thread's sender hasn't dropped (and so hasn't disconnected
                // the channel) quite yet.
                if matches!(child.try_wait(), Ok(Some(_))) {
                    dead = true;
                }
            }
        }
        if dead {
            self.child = None;
            self.lines = None;
            self.schedule_retry(now);
        }
        lines
    }
}

impl Drop for MonitorChild {
    fn drop(&mut self) {
        // Best-effort: a `gdbus monitor` left running as an orphan after the
        // daemon exits would be a small, permanent leak on every restart.
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ---------------------------------------------------------------------------
// The public monitor
// ---------------------------------------------------------------------------

/// Everything the daemon's run loops need: one merged, debounced
/// Locked/Unlocked answer, fed by every source in the module docs.
///
/// Owns up to two `gdbus monitor` children (system bus for logind; session
/// bus for the host's screensaver interface, when it has one) plus the
/// COSMIC lockfile poll. [`LockMonitor::poll`] is the one method every run
/// loop calls on its own tick; the three IPC/process sources have their own
/// typed entry points because the daemon observes them directly rather than
/// through a spawned child.
pub struct LockMonitor {
    machine: StateMachine,
    /// `$XDG_RUNTIME_DIR` — where `cosmic-greeter` drops its lockfile. `Some`
    /// on COSMIC only.
    lockfile_dir: Option<PathBuf>,
    lockfile_present: bool,
    last_lockfile_check: Instant,
    logind: MonitorChild,
    screensaver: Option<MonitorChild>,
    our_session: Option<String>,
    /// Earliest instant the session lookup may run again after a failure; see
    /// [`SESSION_RETRY`].
    next_session_try: Instant,
}

impl LockMonitor {
    /// Build a monitor for `host`, spawning nothing yet — the children start
    /// on the first [`LockMonitor::poll`] via [`MonitorChild::ensure_running`],
    /// exactly like every other lazily-started thing in this daemon (see
    /// `widgets::WidgetEngine::new`'s own "nothing exists until something is
    /// enabled" rule).
    pub fn new(host: HostKind, now: Instant) -> Self {
        let lockfile_dir = matches!(host, HostKind::Cosmic { .. })
            .then(dirs::runtime_dir)
            .flatten();
        let screensaver = screensaver_interface(host).map(|iface| {
            MonitorChild::new("gdbus", vec!["monitor", "--session", "--dest", iface], now)
        });
        LockMonitor {
            machine: StateMachine::new(host, now),
            lockfile_dir,
            lockfile_present: false,
            last_lockfile_check: now - LOCKFILE_POLL,
            logind: MonitorChild::new(
                "gdbus",
                vec!["monitor", "--system", "--dest", "org.freedesktop.login1"],
                now,
            ),
            screensaver,
            our_session: None,
            next_session_try: now,
        }
    }

    /// Current merged state, with no side effects.
    pub fn current(&self) -> Locked {
        self.machine.current()
    }

    /// Poll every source this module owns directly (the lockfile and both
    /// `gdbus monitor` children) and settle the debounce. Call this on every
    /// run-loop tick; it never blocks. Returns the new state only on an
    /// actual change, so a caller can do "swap widgets" work exactly once per
    /// transition rather than on every tick.
    pub fn poll(&mut self, now: Instant) -> Option<Locked> {
        let mut changed = None;

        if matches!(self.machine.host, HostKind::Cosmic { .. }) {
            if let Some(dir) = &self.lockfile_dir {
                if now.duration_since(self.last_lockfile_check) >= LOCKFILE_POLL {
                    self.last_lockfile_check = now;
                    let present = cosmic_lockfile_present(dir);
                    if present != self.lockfile_present {
                        self.lockfile_present = present;
                        if let Some(v) = self.machine.apply(Event::CosmicLockfile(present), now) {
                            changed = Some(v);
                        }
                    }
                }
            }
        }

        if self.our_session.is_none() && now >= self.next_session_try {
            // Best-effort, retried every `SESSION_RETRY` until it succeeds
            // (logind may not be reachable for a moment right at daemon
            // startup); a failed attempt is not logged — `login1_event`
            // degrading to "no match" is silently correct, not an error,
            // until this resolves.
            self.our_session = resolve_our_session_path();
            self.next_session_try = now + SESSION_RETRY;
        }
        for line in self.logind.poll_lines(now) {
            if let Some(parsed) = parse_monitor_line(&line) {
                if let Some(event) = login1_event(&parsed, self.our_session.as_deref()) {
                    if let Some(v) = self.machine.apply(event, now) {
                        changed = Some(v);
                    }
                }
            }
        }

        if let Some(ss) = &mut self.screensaver {
            for line in ss.poll_lines(now) {
                if let Some(parsed) = parse_monitor_line(&line) {
                    if let Some(event) = screensaver_event(&parsed) {
                        if let Some(v) = self.machine.apply(event, now) {
                            changed = Some(v);
                        }
                    }
                }
            }
        }

        if let Some(v) = self.machine.tick(now) {
            changed = Some(v);
        }
        changed
    }

    /// Feed `Request::LockNotify` — an out-of-process renderer (the X11 saver,
    /// once wave 2b lands) reporting that it started or stopped.
    pub fn on_lock_notify(&mut self, locked: bool, now: Instant) -> Option<Locked> {
        self.machine.apply(Event::IpcLockNotify(locked), now)
    }

    /// A host child the daemon itself spawned (`hosts::RunningHost::child`)
    /// has exited — the daemon's own first-party evidence the lock it started
    /// has ended.
    pub fn on_host_child_exited(&mut self, now: Instant) -> Option<Locked> {
        self.machine.apply(Event::HostChildExited, now)
    }

    /// Every lock socket (`hosts::LockTargets::Sockets`) this daemon was
    /// driving has disconnected.
    pub fn on_sockets_disconnected(&mut self, now: Instant) -> Option<Locked> {
        self.machine.apply(Event::AllSocketsDisconnected, now)
    }
}

/// Whether `cosmic-greeter` currently holds a lock for this user:
/// `dir/cosmic-greeter-<session>.lock` exists.
///
/// Matched by pattern rather than built from `$XDG_SESSION_ID`: an autostarted
/// daemon is a systemd user service, which does not inherit that variable, and
/// the lockfile was then never found — so on COSMIC a lock could never be seen
/// to end. `$XDG_RUNTIME_DIR` is per user, so any such file is ours.
fn cosmic_lockfile_present(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("cosmic-greeter-") && name.ends_with(".lock")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    // -- parse_monitor_line ----------------------------------------------

    #[test]
    fn parses_the_live_captured_name_owner_changed_line() {
        // Captured verbatim, 2026-09-28, `gdbus monitor --session --dest
        // org.freedesktop.DBus` while another `gdbus call` connected and
        // disconnected — the read-only capture this module's own docs cite
        // for the general `<path>: <iface>.<member> (<args>)` grammar.
        let line =
            "/org/freedesktop/DBus: org.freedesktop.DBus.NameOwnerChanged (':1.1411', '', ':1.1411')";
        let p = parse_monitor_line(line).unwrap();
        assert_eq!(p.path, "/org/freedesktop/DBus");
        assert_eq!(p.member, "org.freedesktop.DBus.NameOwnerChanged");
        assert_eq!(p.args, "':1.1411', '', ':1.1411'");
    }

    #[test]
    fn parses_zero_arg_and_one_bool_arg_lines() {
        let p = parse_monitor_line(
            "/org/freedesktop/login1/session/_34: org.freedesktop.login1.Session.Lock ()",
        )
        .unwrap();
        assert_eq!(p.member, "org.freedesktop.login1.Session.Lock");
        assert_eq!(p.args, "");

        let p = parse_monitor_line(
            "/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (true,)",
        )
        .unwrap();
        assert_eq!(p.args, "true,");
    }

    #[test]
    fn banner_lines_and_garbage_do_not_parse() {
        assert_eq!(
            parse_monitor_line("Monitoring signals from all objects owned by org.freedesktop.DBus"),
            None
        );
        assert_eq!(
            parse_monitor_line("The name org.freedesktop.DBus is owned by org.freedesktop.DBus"),
            None
        );
        assert_eq!(parse_monitor_line(""), None);
        assert_eq!(parse_monitor_line("no colon here at all"), None);
        assert_eq!(parse_monitor_line("/path: no paren here"), None);
    }

    #[test]
    fn parse_bool_arg_covers_every_shape_it_must() {
        assert_eq!(parse_bool_arg("true,"), Some(true));
        assert_eq!(parse_bool_arg("false,"), Some(false));
        assert_eq!(parse_bool_arg(" true , "), Some(true));
        assert_eq!(parse_bool_arg(""), None);
        assert_eq!(parse_bool_arg("':1.1411', '', ':1.1411'"), None);
    }

    // -- login1_event / screensaver_event ---------------------------------

    #[test]
    fn login1_event_matches_only_our_session() {
        let our = "/org/freedesktop/login1/session/_34";
        let other = "/org/freedesktop/login1/session/_1";

        let lock_ours_line = format!("{our}: org.freedesktop.login1.Session.Lock ()");
        let lock_ours = parse_monitor_line(&lock_ours_line).unwrap();
        assert_eq!(login1_event(&lock_ours, Some(our)), Some(Event::LogindLock));

        let lock_other_line = format!("{other}: org.freedesktop.login1.Session.Lock ()");
        let lock_other = parse_monitor_line(&lock_other_line).unwrap();
        assert_eq!(login1_event(&lock_other, Some(our)), None);

        // No resolved session yet: nothing session-scoped matches, ever.
        assert_eq!(login1_event(&lock_ours, None), None);

        let unlock_ours_line = format!("{our}: org.freedesktop.login1.Session.Unlock ()");
        let unlock_ours = parse_monitor_line(&unlock_ours_line).unwrap();
        assert_eq!(
            login1_event(&unlock_ours, Some(our)),
            Some(Event::LogindUnlock)
        );
    }

    #[test]
    fn login1_event_prepare_for_sleep_is_session_independent() {
        let line = parse_monitor_line(
            "/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (true,)",
        )
        .unwrap();
        assert_eq!(
            login1_event(&line, Some("/whatever")),
            Some(Event::LogindPrepareForSleep(true))
        );
        assert_eq!(
            login1_event(&line, None),
            Some(Event::LogindPrepareForSleep(true))
        );

        let false_line = parse_monitor_line(
            "/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (false,)",
        )
        .unwrap();
        assert_eq!(
            login1_event(&false_line, None),
            Some(Event::LogindPrepareForSleep(false))
        );
    }

    #[test]
    fn screensaver_event_matches_any_of_the_five_interfaces() {
        for iface in SCREENSAVER_INTERFACES {
            let text = format!("/{}: {iface}.ActiveChanged (true,)", "ScreenSaver");
            let line = parse_monitor_line(&text).unwrap();
            assert_eq!(
                screensaver_event(&line),
                Some(Event::ScreensaverActiveChanged(true)),
                "{iface}"
            );
        }
        let unrelated = parse_monitor_line(
            "/org/freedesktop/ScreenSaver: org.freedesktop.ScreenSaver.Inhibit (1,)",
        )
        .unwrap();
        assert_eq!(screensaver_event(&unrelated), None);
    }

    #[test]
    fn screensaver_interface_is_host_specific_and_absent_elsewhere() {
        assert_eq!(
            screensaver_interface(HostKind::Kde),
            Some("org.freedesktop.ScreenSaver")
        );
        assert_eq!(
            screensaver_interface(HostKind::Gnome),
            Some("org.gnome.ScreenSaver")
        );
        assert_eq!(
            screensaver_interface(HostKind::Cinnamon),
            Some("org.cinnamon.ScreenSaver")
        );
        assert_eq!(
            screensaver_interface(HostKind::Mate),
            Some("org.mate.ScreenSaver")
        );
        assert_eq!(
            screensaver_interface(HostKind::Xfce),
            Some("org.xfce.ScreenSaver")
        );
        for host in [
            HostKind::Cosmic { live: true },
            HostKind::Cosmic { live: false },
            HostKind::Deepin,
            HostKind::Wlroots,
            HostKind::X11Wm,
            HostKind::Unsupported,
        ] {
            assert_eq!(screensaver_interface(host), None, "{host:?}");
        }
    }

    // -- parse_object_path_reply -------------------------------------------

    #[test]
    fn parses_the_live_captured_get_session_reply() {
        // Captured verbatim, 2026-09-28: `gdbus call --system --dest
        // org.freedesktop.login1 --object-path /org/freedesktop/login1
        // --method org.freedesktop.login1.Manager.GetSession "4"` on this
        // machine's own COSMIC session.
        assert_eq!(
            parse_object_path_reply("(objectpath '/org/freedesktop/login1/session/_34',)"),
            Some("/org/freedesktop/login1/session/_34".to_string())
        );
    }

    #[test]
    fn object_path_reply_garbage_is_none() {
        assert_eq!(parse_object_path_reply(""), None);
        assert_eq!(
            parse_object_path_reply(
                "Error: GDBus.Error:org.freedesktop.login1.NoSuchSession: No session '99' known"
            ),
            None
        );
        assert_eq!(parse_object_path_reply("('/plain/string',)"), None);
    }

    // -- StateMachine: authoritative unlock per host -----------------------

    #[test]
    fn cosmic_only_the_lockfile_unlocks() {
        let now = t0();
        let mut m = StateMachine::new(HostKind::Cosmic { live: true }, now);
        assert_eq!(
            m.apply(Event::CosmicLockfile(true), now),
            None,
            "vote alone must not commit before the debounce"
        );
        assert_eq!(
            m.tick(at(now, 260)),
            Some(Locked::Locked),
            "settles once the debounce has elapsed with no new vote"
        );

        // Logind Unlock and ActiveChanged(false) must NOT unlock a COSMIC
        // session — cosmic-greeter never emits the former, and this asserts
        // the latter is not mistakenly trusted there either.
        assert_eq!(m.apply(Event::LogindUnlock, at(now, 300)), None);
        assert_eq!(
            m.tick(at(now, 600)),
            None,
            "an untrusted vote must never commit, however long it waits"
        );
        assert_eq!(
            m.apply(Event::ScreensaverActiveChanged(false), at(now, 700)),
            None
        );
        assert_eq!(m.tick(at(now, 1000)), None);
        assert!(m.current().is_locked());

        // The lockfile disappearing does unlock it.
        assert_eq!(m.apply(Event::CosmicLockfile(false), at(now, 1000)), None);
        assert_eq!(m.tick(at(now, 1260)), Some(Locked::Unlocked));
    }

    #[test]
    fn kde_active_changed_false_and_logind_unlock_both_unlock() {
        let now = t0();
        for unlock in [Event::ScreensaverActiveChanged(false), Event::LogindUnlock] {
            let mut m = StateMachine::new(HostKind::Kde, now);
            m.apply(Event::LogindLock, now);
            m.tick(at(now, 260));
            assert!(m.current().is_locked());
            m.apply(unlock.clone(), at(now, 300));
            assert_eq!(m.tick(at(now, 560)), Some(Locked::Unlocked), "{unlock:?}");
        }
    }

    #[test]
    fn non_screensaver_hosts_ignore_active_changed_but_accept_logind_unlock() {
        let now = t0();
        for host in [HostKind::Wlroots, HostKind::X11Wm, HostKind::Deepin] {
            let mut m = StateMachine::new(host, now);
            m.apply(Event::LogindLock, now);
            m.tick(at(now, 260));
            assert!(m.current().is_locked(), "{host:?}");

            // No screensaver interface exists for these hosts, so even if a
            // stray ActiveChanged(false) somehow arrived it must not unlock.
            m.apply(Event::ScreensaverActiveChanged(false), at(now, 300));
            assert_eq!(m.tick(at(now, 600)), None, "{host:?}");

            m.apply(Event::LogindUnlock, at(now, 600));
            assert_eq!(m.tick(at(now, 860)), Some(Locked::Unlocked), "{host:?}");
        }
    }

    #[test]
    fn host_child_exit_and_socket_disconnect_always_unlock() {
        let now = t0();
        for event in [Event::HostChildExited, Event::AllSocketsDisconnected] {
            let mut m = StateMachine::new(HostKind::Wlroots, now);
            m.apply(Event::IpcLockNotify(true), now);
            m.tick(at(now, 260));
            assert!(m.current().is_locked());
            m.apply(event.clone(), at(now, 300));
            assert_eq!(m.tick(at(now, 560)), Some(Locked::Unlocked), "{event:?}");
        }
    }

    // -- StateMachine: debounce / no flapping --------------------------------

    #[test]
    fn a_burst_of_conflicting_votes_settles_on_the_last_one_only() {
        let now = t0();
        let mut m = StateMachine::new(HostKind::Gnome, now);
        // Lock, unlock, lock again, all within the debounce window: only the
        // final vote should ever commit, and only once — never a Locked ->
        // Unlocked -> Locked flicker the caller would see as two swaps.
        assert_eq!(m.apply(Event::LogindLock, at(now, 0)), None);
        assert_eq!(m.apply(Event::LogindUnlock, at(now, 50)), None);
        assert_eq!(m.apply(Event::LogindLock, at(now, 100)), None);
        assert_eq!(m.tick(at(now, 200)), None, "debounce has not elapsed yet");
        assert_eq!(m.tick(at(now, 360)), Some(Locked::Locked));
        assert_eq!(m.tick(at(now, 1000)), None, "commits exactly once");
    }

    #[test]
    fn a_vote_that_confirms_the_current_state_cancels_a_pending_flip() {
        let now = t0();
        let mut m = StateMachine::new(HostKind::Gnome, now);
        assert_eq!(m.apply(Event::LogindLock, now), None);
        assert_eq!(m.tick(at(now, 260)), Some(Locked::Locked));

        // Start an unlock vote, then reconfirm Locked before it settles.
        m.apply(Event::LogindUnlock, at(now, 300));
        m.apply(Event::LogindLock, at(now, 350));
        assert_eq!(
            m.tick(at(now, 700)),
            None,
            "the reconfirming vote must cancel the pending unlock"
        );
        assert!(m.current().is_locked());
    }

    #[test]
    fn repeated_identical_votes_never_commit_twice() {
        let now = t0();
        let mut m = StateMachine::new(HostKind::Gnome, now);
        assert_eq!(m.apply(Event::LogindLock, now), None);
        assert_eq!(m.tick(at(now, 260)), Some(Locked::Locked));
        // More Lock votes after settling: already the current state, so
        // never a "change".
        for ms in [300, 400, 500] {
            assert_eq!(m.apply(Event::LogindLock, at(now, ms)), None);
        }
    }

    // -- StateMachine: PrepareForSleep is immediate, not debounced ----------

    #[test]
    fn prepare_for_sleep_true_commits_immediately_even_while_unlocked() {
        let now = t0();
        let mut m = StateMachine::new(HostKind::Kde, now);
        assert_eq!(
            m.apply(Event::LogindPrepareForSleep(true), now),
            Some(Locked::SleepImminent),
            "must not wait out the debounce"
        );
        assert!(m.current().is_locked());
    }

    #[test]
    fn prepare_for_sleep_false_never_unlocks_a_session_that_was_locked() {
        // The lock was real before the machine slept, so resuming must leave it
        // exactly as it was: only the host's own unlock source ends it.
        let now = t0();
        let mut m = StateMachine::new(HostKind::Kde, now);
        m.apply(Event::LogindLock, now);
        m.tick(at(now, 260));
        assert_eq!(m.current(), Locked::Locked);
        m.apply(Event::LogindPrepareForSleep(true), at(now, 300));
        assert_eq!(m.current(), Locked::SleepImminent);
        assert_eq!(
            m.apply(Event::LogindPrepareForSleep(false), at(now, 400)),
            None
        );
        // Past the wake grace, back to the real lock — never to Unlocked.
        let later = at(now, 400) + WAKE_GRACE + DEBOUNCE;
        assert_eq!(m.tick(later), Some(Locked::Locked));
        assert!(m.current().is_locked());
        assert_eq!(m.tick(later + Duration::from_secs(60)), None);
        // And the real unlock still unlocks it.
        m.apply(Event::LogindUnlock, later + Duration::from_secs(61));
        assert_eq!(
            m.tick(later + Duration::from_secs(62)),
            Some(Locked::Unlocked)
        );
    }

    #[test]
    fn a_suspend_without_a_lock_does_not_leave_the_session_locked() {
        // The reported bug: nothing ever votes "unlocked" for a session that was
        // never locked, so the sleep placeholder stuck for the whole session —
        // on COSMIC it cleared the desktop widgets and never brought them back.
        let now = t0();
        let mut m = StateMachine::new(HostKind::Cosmic { live: true }, now);
        assert_eq!(
            m.apply(Event::LogindPrepareForSleep(true), now),
            Some(Locked::SleepImminent)
        );
        m.apply(Event::LogindPrepareForSleep(false), at(now, 5_000));
        assert_eq!(
            m.tick(at(now, 5_000) + DEBOUNCE),
            None,
            "the wake grace has not elapsed: a lock raised on resume still has \
             time to be seen"
        );
        assert_eq!(
            m.tick(at(now, 5_000) + WAKE_GRACE + DEBOUNCE),
            Some(Locked::Unlocked)
        );
        assert!(!m.current().is_locked());
    }

    #[test]
    fn a_lock_raised_on_resume_wins_over_the_wake_fallback() {
        // The greeter locks as the machine wakes: its lockfile shows up inside
        // the grace window, so the desktop widgets never come back over it.
        let now = t0();
        let mut m = StateMachine::new(HostKind::Cosmic { live: true }, now);
        m.apply(Event::LogindPrepareForSleep(true), now);
        m.apply(Event::LogindPrepareForSleep(false), at(now, 1_000));
        m.apply(Event::CosmicLockfile(true), at(now, 1_500));
        assert_eq!(m.tick(at(now, 1_800)), Some(Locked::Locked));
        assert_eq!(
            m.tick(at(now, 1_000) + WAKE_GRACE + DEBOUNCE + DEBOUNCE),
            None,
            "the cancelled fallback must not unlock a real lock later"
        );
        assert_eq!(m.current(), Locked::Locked);
    }

    #[test]
    fn a_lock_still_settling_when_the_machine_sleeps_survives_the_sleep() {
        let now = t0();
        let mut m = StateMachine::new(HostKind::Gnome, now);
        m.apply(Event::LogindLock, now); // pending, debounce not yet elapsed
        m.apply(Event::LogindPrepareForSleep(true), at(now, 100));
        m.apply(Event::LogindPrepareForSleep(false), at(now, 200));
        assert_eq!(
            m.tick(at(now, 200) + WAKE_GRACE + DEBOUNCE),
            Some(Locked::Locked),
            "falls back to the lock that was about to commit, not to Unlocked"
        );
    }

    #[test]
    fn waking_when_not_asleep_is_a_no_op() {
        // A stray PrepareForSleep(false) (the signal is system-wide and the
        // monitor can start mid-stream) must not disturb a settled state.
        let now = t0();
        let mut m = StateMachine::new(HostKind::Kde, now);
        m.apply(Event::LogindLock, now);
        m.tick(at(now, 260));
        m.apply(Event::LogindPrepareForSleep(false), at(now, 300));
        assert_eq!(m.tick(at(now, 300) + WAKE_GRACE + DEBOUNCE), None);
        assert_eq!(m.current(), Locked::Locked);
    }

    #[test]
    fn the_cosmic_lockfile_is_found_without_a_session_id() {
        let dir = std::env::temp_dir().join(format!("fresco-lockfile-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!cosmic_lockfile_present(&dir));
        // Unrelated files in a runtime dir are not a lock.
        std::fs::write(dir.join("wayland-1.lock"), b"").unwrap();
        std::fs::write(dir.join("cosmic-greeter-4.txt"), b"").unwrap();
        assert!(!cosmic_lockfile_present(&dir));
        std::fs::write(dir.join("cosmic-greeter-4.lock"), b"").unwrap();
        assert!(cosmic_lockfile_present(&dir));
        std::fs::remove_file(dir.join("cosmic-greeter-4.lock")).unwrap();
        assert!(!cosmic_lockfile_present(&dir));
        assert!(!cosmic_lockfile_present(&dir.join("does-not-exist")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn display_session_is_read_from_the_user_property_reply() {
        // Captured from `gdbus call --system … Properties.Get
        // org.freedesktop.login1.User Display` on the affected machine.
        assert_eq!(
            parse_display_session_reply(
                "(<('4', objectpath '/org/freedesktop/login1/session/_34')>,)"
            ),
            Some("/org/freedesktop/login1/session/_34".to_string())
        );
        assert_eq!(
            parse_display_session_reply("(<('', objectpath '/')>,)"),
            None,
            "'/' is logind's \"no graphical session\""
        );
        assert_eq!(parse_display_session_reply("Error: no such property"), None);
        assert_eq!(parse_display_session_reply(""), None);
    }

    #[test]
    fn prepare_for_sleep_interrupts_a_pending_unlock() {
        let now = t0();
        let mut m = StateMachine::new(HostKind::Kde, now);
        m.apply(Event::LogindLock, now);
        m.tick(at(now, 260));
        // An unlock vote is in flight (not yet settled) when sleep begins.
        m.apply(Event::LogindUnlock, at(now, 300));
        assert_eq!(
            m.apply(Event::LogindPrepareForSleep(true), at(now, 320)),
            Some(Locked::SleepImminent)
        );
        // The interrupted unlock vote must not resurrect itself later.
        assert_eq!(m.tick(at(now, 1000)), None);
        assert!(m.current().is_locked());
    }

    // -- LockMonitor: IPC-facing entry points --------------------------------

    #[test]
    fn lock_monitor_ipc_entry_points_feed_the_machine() {
        let now = t0();
        let mut mon = LockMonitor::new(HostKind::Wlroots, now);
        assert_eq!(mon.current(), Locked::Unlocked);
        assert_eq!(mon.on_lock_notify(true, now), None, "debounced");
        // Advance via the same event so the debounce sees no new conflicting
        // vote — `poll` isn't called here since it would spawn a real
        // `gdbus`, which these unit tests must not depend on.
        assert_eq!(mon.on_lock_notify(true, at(now, 260)), Some(Locked::Locked));
        assert_eq!(mon.on_host_child_exited(at(now, 300)), None, "debounced");
        assert_eq!(
            mon.on_host_child_exited(at(now, 600)),
            Some(Locked::Unlocked)
        );
    }

    #[test]
    fn lock_monitor_sockets_disconnected_unlocks() {
        let now = t0();
        let mut mon = LockMonitor::new(HostKind::X11Wm, now);
        mon.on_lock_notify(true, now);
        mon.on_lock_notify(true, at(now, 260));
        assert!(mon.current().is_locked());
        mon.on_sockets_disconnected(at(now, 300));
        assert_eq!(
            mon.on_sockets_disconnected(at(now, 600)),
            Some(Locked::Unlocked)
        );
    }

    // -- MonitorChild: process supervision, against a fake command ----------

    #[test]
    fn monitor_child_streams_lines_from_a_real_child_process() {
        let now = Instant::now();
        // `sh -c` in place of `gdbus`: two lines, a short pause, so the test
        // has to poll more than once — proving lines really stream rather
        // than only arriving once at exit.
        let mut mc = MonitorChild::new(
            "/bin/sh",
            vec!["-c", "echo one; /bin/sleep 0.05; echo two"],
            now,
        );
        let mut got = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while got.len() < 2 && Instant::now() < deadline {
            got.extend(mc.poll_lines(Instant::now()));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(got, vec!["one".to_string(), "two".to_string()]);
    }

    #[test]
    fn monitor_child_restarts_a_dead_child_with_backoff() {
        let now = Instant::now();
        let mut mc = MonitorChild::new("/bin/sh", vec!["-c", "echo hi"], now);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_hi = false;
        while Instant::now() < deadline {
            if mc.poll_lines(Instant::now()).iter().any(|l| l == "hi") {
                saw_hi = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(saw_hi, "first run must produce a line");
        // The process exits right after printing; give the reader thread a
        // moment to observe EOF, then poll again — this call must detect the
        // death and schedule (not perform) a retry rather than hang.
        std::thread::sleep(Duration::from_millis(50));
        let before = Instant::now();
        let _ = mc.poll_lines(before);
        assert!(
            mc.next_attempt > before,
            "a dead child must schedule a future retry, not restart inline"
        );
    }

    #[test]
    fn monitor_child_missing_program_never_panics_and_backs_off() {
        let now = Instant::now();
        let mut mc = MonitorChild::new("this-program-does-not-exist-fresco-lock-test", vec![], now);
        assert_eq!(mc.poll_lines(now), Vec::<String>::new());
        assert!(mc.next_attempt > now, "a spawn failure must back off");
        // Polling again before the backoff elapses must not spawn again or
        // panic; it's still just an empty result.
        assert_eq!(mc.poll_lines(now), Vec::<String>::new());
    }

    // -- Locked::is_locked ----------------------------------------------------

    #[test]
    fn is_locked_truth_table() {
        assert!(!Locked::Unlocked.is_locked());
        assert!(Locked::Locked.is_locked());
        assert!(Locked::SleepImminent.is_locked());
    }
}

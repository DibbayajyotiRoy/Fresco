use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

/// Requests the GUI (or CLI) sends to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum Request {
    /// Re-read config.toml and apply it (swap wallpaper in place).
    Apply,
    /// Tear down wallpaper windows and exit the daemon.
    Stop,
    Pause,
    Resume,
    Status,
    /// Download and install the latest release in the background (fire-and-forget).
    Update,
    /// Lock the session right now, through this desktop's own lock host (see
    /// `daemon::lock::hosts`) — `loginctl lock-session`, swaylock-plugin,
    /// xsecurelock, or whatever else that host already trusts. Fresco never
    /// authenticates; this only asks the existing, already-audited locker to
    /// engage, then arranges for Fresco's wallpaper/widgets to show through
    /// it wherever the host allows (see `docs/plan-lock-screen.md` §5's
    /// "fail closed" invariant).
    Lock,
    /// Render a lock-screen preview PNG at `width`x`height` and reply with
    /// its path. **Never locks anything** — the preview path must stay
    /// incapable of reaching any locking primitive, by construction (see
    /// `docs/plan-lock-screen.md` §6).
    LockPreview {
        width: u32,
        height: u32,
    },
    /// An out-of-process lock renderer — today, only the X11 saver `fresco
    /// lock` spawns under `xsecurelock` — reports that it started or
    /// stopped, and which mpv IPC sockets (one per output) it now exposes so
    /// the daemon can drive their wallpaper/widgets the same way it drives
    /// its own mpvpaper children.
    LockNotify {
        locked: bool,
        #[serde(default)]
        sockets: Vec<LockSocket>,
    },
    /// Install this desktop's lock-host integration (e.g. the KDE Plasma
    /// wallpaper plugin plus a `kscreenlockerrc` edit), backing up whatever
    /// was there first so `LockUndo` can restore it exactly.
    LockSetup,
    /// Undo `LockSetup`: restore this desktop's own lock-screen
    /// configuration exactly as it was before Fresco touched it.
    LockUndo,
}

/// One mpv IPC socket an out-of-process lock renderer exposes for the daemon
/// to drive, reported via [`Request::LockNotify`]. `connector` matches
/// [`MonitorInfo::connector`] / `Config.monitors` keys, so the daemon can
/// pair each socket with the right per-output wallpaper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockSocket {
    pub connector: String,
    pub path: String,
}

/// One connected display, as the daemon sees it (RandR on X11, `wl_output`
/// on Wayland). Connector names match the keys `Config.monitors` accepts, so
/// the GUI can offer per-monitor assignment without guessing names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub connector: String,
    pub width: u16,
    pub height: u16,
    pub x: i16,
    pub y: i16,
}

/// Where a host's one-click lock-screen integration stands — the GUI's
/// per-host status row (`docs/plan-lock-screen.md` §6) reads this to decide
/// between offering "Set up", "Remove", a copyable snippet, or nothing at
/// all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LockSetupState {
    /// Nothing to install — `Request::Lock` alone is already enough on this
    /// host (e.g. COSMIC, or any `loginctl`-only host).
    NotNeeded,
    /// Setup is available and has not been run yet.
    Needed,
    /// Setup has been run and is currently installed.
    Done,
    /// No setup path exists on this host yet (e.g. wlroots/X11 before wave
    /// 2b's adapters land).
    Unavailable,
}

/// Lock-screen feature status for the GUI's per-host status row, carried
/// inside [`StatusReply`] so it arrives on the same poll as everything else
/// rather than a separate round trip. See `docs/plan-lock-screen.md` §6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockStatus {
    /// `[lockscreen].enabled` from config.toml.
    pub enabled: bool,
    /// Stable host id: `"cosmic"` | `"kde"` | `"gnome"` | `"cinnamon"` |
    /// `"mate"` | `"xfce"` | `"deepin"` | `"wlroots"` | `"x11"` |
    /// `"unsupported"`. See `daemon::lock::hosts::HostKind::id`.
    pub host: String,
    /// Whether this host can show the wallpaper as live video while locked
    /// — false means no live surface (GNOME, Cinnamon, MATE, Xfce, Deepin);
    /// [`LockStatus::still_frame`] says whether a still is shown instead.
    pub live_video: bool,
    /// Whether this host can show Fresco's own widgets while locked.
    pub widgets: bool,
    /// Whether the real lock screen shows at least a still frame of Fresco's
    /// wallpaper on this host. Implied by `live_video`; what separates "still
    /// frame only" from "nothing reaches the lock screen yet" for a host with
    /// neither live video nor widgets (the Lock Screen page's support
    /// summary). `None` from a daemon that predates the field — the GUI then
    /// says nothing rather than guess.
    #[serde(default)]
    pub still_frame: Option<bool>,
    /// Whether the session is locked right now.
    pub locked: bool,
    pub setup: LockSetupState,
    /// Short, already-localized lines for the GUI status row — e.g. an
    /// unanswered spike gate or a host-specific limit. Empty when there is
    /// nothing to say.
    #[serde(default)]
    pub notes: Vec<String>,
}

/// Why the wallpaper is held paused, for `fresco status` and the GUI pill —
/// "it stopped moving and I don't know why" was the bug report. See
/// [`PauseSources::reason`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
    /// The user pressed Pause (GUI, `fresco pause`, hotkey).
    User,
    /// "Pause on battery" and the machine is discharging.
    Battery,
    /// The session is locked and the lock screen's policy holds the video.
    Locked,
    /// A fullscreen window covers an output.
    Fullscreen,
    /// A large window (fullscreen or not) covers most of an output.
    Covered,
}

/// The pause sources the daemon folds into one decision; the input of
/// [`PauseSources::reason`], kept as named fields so call sites cannot swap
/// two positional bools.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PauseSources {
    pub user: bool,
    pub battery: bool,
    pub locked: bool,
    pub fullscreen: bool,
    pub covered: bool,
}

impl PauseSources {
    /// The one reason to show. Order = what the user can act on first (their
    /// own pause, then the battery setting), then the session, then what is
    /// merely covering the screen.
    // ponytail: a single reason, not a list — when two sources hold the pause,
    // clearing the first reveals the second on the next poll. Make this a
    // Vec if "both at once" ever needs to be visible.
    pub fn reason(self) -> Option<PauseReason> {
        [
            (self.user, PauseReason::User),
            (self.battery, PauseReason::Battery),
            (self.locked, PauseReason::Locked),
            (self.fullscreen, PauseReason::Fullscreen),
            (self.covered, PauseReason::Covered),
        ]
        .into_iter()
        .find_map(|(on, reason)| on.then_some(reason))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StatusReply {
    pub running: bool,
    pub paused: bool,
    /// Why playback is held right now, `None` when nothing holds it. Unlike
    /// `paused` (the user/battery toggle the pill's button flips) this also
    /// covers a fullscreen/maximized window or the lock screen holding it, so
    /// it can be `Some` while `paused` is false. `#[serde(default)]` keeps an
    /// older daemon/GUI pair compatible in both directions.
    #[serde(default)]
    pub paused_reason: Option<PauseReason>,
    /// Active mpv hwdec per monitor, e.g. "vaapi" / "nvdec" / "no" (software).
    pub hwdec: Option<String>,
    /// Human-readable description of what's playing.
    pub wallpaper: Option<String>,
    /// CPU of the daemon + renderer children since the previous status poll,
    /// as a percentage of ONE core (`top` semantics — may exceed 100 on
    /// multicore). 0.0 on the first poll (no baseline yet).
    pub cpu_percent: f32,
    /// Resident memory of the daemon + renderer children (mpvpaper), MB.
    pub rss_mb: u64,
    pub monitors: Vec<String>,
    /// Last media load failure, if any (file path + reason).
    pub error: Option<String>,
    /// True when the primary renderer has an audio track selected (mpv `aid`
    /// != no). False means mpv dropped/skipped audio — e.g. muted entries load
    /// with `aid=no`, and mpv deselects the track permanently when no audio
    /// server was reachable at load time. None = unknown / not applicable.
    #[serde(default)]
    pub audio_track: Option<bool>,
    #[serde(default)]
    pub mute: Option<bool>,
    #[serde(default)]
    pub volume: Option<u8>,
    /// Decode honesty (primary renderer): source dimensions, bit depth, and
    /// decoder frame drops — so "quality looks off" is diagnosable instead of
    /// silent (e.g. 8K on a GPU without 8K decode support).
    #[serde(default)]
    pub source_w: Option<u32>,
    #[serde(default)]
    pub source_h: Option<u32>,
    #[serde(default)]
    pub bit_depth: Option<u8>,
    #[serde(default)]
    pub dropped_frames: Option<u64>,
    /// ALL connected displays with geometry — unlike `monitors`, which only
    /// lists outputs that currently have a wallpaper.
    #[serde(default)]
    pub monitors_info: Vec<MonitorInfo>,
    /// Connectors (Wayland only) whose renderer gave up on live playback and
    /// is holding a paused static frame instead — see `WlOutput::supervise`'s
    /// give-up arm. `#[serde(default)]` so an older daemon/GUI pair (neither
    /// of which knows this field) still round-trips a `Status` reply fine:
    /// an old GUI just ignores it, and a new GUI reading an old daemon's
    /// reply sees an empty list rather than failing to deserialize.
    #[serde(default)]
    pub gave_up: Vec<String>,
    /// Lock-screen feature status, for the GUI's Lock Screen page.
    /// `#[serde(default)]` so a reply from a pre-lock-screen daemon still
    /// parses as `None`; a current daemon always sets `Some(..)`.
    #[serde(default)]
    pub lockscreen: Option<LockStatus>,
    /// Media file the DEFAULT wallpaper is showing right now, from the
    /// daemon's in-memory config — which a day/night boundary swap updates
    /// without ever saving, so unlike `config.toml` it says what is actually
    /// on screen. `None` for playlists/slideshows (no single file) and from
    /// an older daemon (`#[serde(default)]`), in which case the GUI falls
    /// back to its own config.
    #[serde(default)]
    pub wallpaper_path: Option<PathBuf>,
}

/// Outcome of a [`Request::Lock`] attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockReply {
    /// Stable host id — see [`LockStatus::host`].
    pub host: String,
    pub ok: bool,
    #[serde(default)]
    pub message: Option<String>,
}

// `StatusReply` is a fat but short-lived value (one per request, never stored
// in bulk); boxing it would ripple through every `Response::Status(..)`
// pattern in the daemon, CLI and GUI for no real saving.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum Response {
    Ok,
    Status(StatusReply),
    Err {
        message: String,
    },
    /// Reply to [`Request::Lock`].
    Lock(LockReply),
    /// Reply to [`Request::LockPreview`]: path to the rendered PNG.
    LockPreview {
        path: String,
    },
}

/// `($XDG_RUNTIME_DIR or the `/tmp` fallback, used_tmp_fallback)`. Split out
/// of [`socket_dir`] so callers that only need to harden the risky case (see
/// [`ensure_safe_socket_dir`]'s doc comment) can branch on the same "was
/// `XDG_RUNTIME_DIR` set" fact [`socket_dir`] itself already computed,
/// instead of re-deriving it.
fn socket_base_dir() -> (PathBuf, bool) {
    match dirs::runtime_dir() {
        Some(dir) => (dir, false),
        None => (
            PathBuf::from(format!("/tmp/fresco-{}", libc_getuid())),
            true,
        ),
    }
}

pub fn socket_dir() -> PathBuf {
    socket_base_dir().0.join("fresco")
}

pub fn socket_path() -> PathBuf {
    socket_dir().join("control.sock")
}

// Avoid pulling in the libc crate for one call.
fn libc_getuid() -> u32 {
    std::fs::metadata("/proc/self")
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
        .unwrap_or(0)
}

/// Create `dir` as a private (`0700`) directory this process can trust to
/// hold the control socket, or verify that an already-existing `dir` is
/// still safe to trust — never silently accept whatever
/// `std::fs::create_dir_all` would have accepted.
///
/// `create_dir_all` treats "the path already exists and is a directory" as
/// success with no check at all of who owns it, what its mode is, or whether
/// it is actually a symlink to a directory elsewhere. Under the
/// `/tmp/fresco-<uid>` fallback (`XDG_RUNTIME_DIR` unset — see
/// [`socket_dir`]), `/tmp` is world-writable, so another local user can
/// pre-create that exact path — as a symlink, or as a directory they own —
/// before this user's daemon ever runs, then host their own listener on
/// `control.sock` inside it and answer every `Request::Lock` with a forged
/// `Response::Lock { ok: true }` while the real session sits unlocked. This
/// is the fail-closed gate for that: create with `DirBuilder` (never
/// `create_dir_all`, and never a mode wider than `0o700`), and on
/// `AlreadyExists`, `lstat` the path (see `verify_safe_dir`) and accept it
/// only if it is a real directory (not a symlink), owned by our own uid, with
/// no group/other permission bits set at all.
///
/// One exception: a real directory we own whose *only* problem is wide mode
/// bits is tightened to `0o700` in place and then accepted. Releases before
/// 1.1.46 created this directory with `create_dir_all` (umask `022` →
/// `0755`), so every upgrading user has exactly such a directory; refusing
/// it would brick the daemon on update with "refusing to trust it". A
/// directory owned by someone else, a symlink, or a non-directory is still
/// refused — tightening those would hand an attacker-owned path to the
/// socket, which is the attack this gate exists to stop.
///
/// Even under `$XDG_RUNTIME_DIR` (created `0700` by systemd-logind, and so
/// already unreachable by any other uid) this same check runs on the `fresco`
/// subdirectory this function creates inside it — belt and suspenders costs
/// nothing here, and it means this function has exactly one contract
/// regardless of which base directory called it with. The parent directory
/// itself (`$XDG_RUNTIME_DIR`, or `/tmp`) is created/left alone the ordinary
/// way; it is the leaf directory's own `0700` ownership that actually
/// protects the socket inside it from every other uid, regardless of who
/// owns anything above it.
pub fn ensure_safe_socket_dir(dir: &Path) -> Result<()> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => verify_safe_dir(dir),
        Err(e) => Err(e).with_context(|| format!("creating {}", dir.display())),
    }
}

/// Pure judgement: does `(is_symlink, is_dir, owner_uid, mode)` describe a
/// directory this process can trust? Split out of [`verify_safe_dir`] purely
/// so the uid-mismatch branch — otherwise untestable in a single-user CI
/// environment, since there is no other uid to create a real file as — is a
/// plain unit test like every other branch here, with a fake `our_uid`
/// standing in for [`libc_getuid`].
fn dir_is_trustworthy(
    is_symlink: bool,
    is_dir: bool,
    owner_uid: u32,
    mode: u32,
    our_uid: u32,
) -> Result<(), &'static str> {
    if is_symlink {
        return Err("is a symlink");
    }
    if !is_dir {
        return Err("is not a directory");
    }
    if owner_uid != our_uid {
        return Err("is not owned by us");
    }
    if mode & 0o077 != 0 {
        return Err("is readable/writable/traversable by other users");
    }
    Ok(())
}

/// Real-filesystem half of [`ensure_safe_socket_dir`]'s `AlreadyExists` path:
/// `lstat`s `dir` (never following a symlink — that is exactly the case this
/// must catch) and runs [`dir_is_trustworthy`] against what it finds, with
/// one self-heal first: a real directory owned by us whose only fault is
/// wide mode bits (what pre-1.1.46 releases left behind — see
/// [`ensure_safe_socket_dir`]) is chmodded to `0o700` and re-checked rather
/// than refused. Anything else untrustworthy (symlink, non-directory, another
/// uid's directory) is an error, as is a chmod that fails or still leaves
/// the directory untrustworthy.
fn verify_safe_dir(dir: &Path) -> Result<()> {
    let meta =
        std::fs::symlink_metadata(dir).with_context(|| format!("checking {}", dir.display()))?;
    if !meta.file_type().is_symlink()
        && meta.is_dir()
        && meta.uid() == libc_getuid()
        && meta.mode() & 0o077 != 0
    {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("tightening {}", dir.display()))?;
        let meta = std::fs::symlink_metadata(dir)
            .with_context(|| format!("re-checking {}", dir.display()))?;
        return dir_is_trustworthy(
            meta.file_type().is_symlink(),
            meta.is_dir(),
            meta.uid(),
            meta.mode(),
            libc_getuid(),
        )
        .map_err(|reason| anyhow!("{} {reason} — refusing to trust it", dir.display()));
    }
    dir_is_trustworthy(
        meta.file_type().is_symlink(),
        meta.is_dir(),
        meta.uid(),
        meta.mode(),
        libc_getuid(),
    )
    .map_err(|reason| anyhow!("{} {reason} — refusing to trust it", dir.display()))
}

/// [`socket_path`], but refuses to hand back a path under the `/tmp`
/// fallback (see [`socket_dir`]) unless [`ensure_safe_socket_dir`] accepts
/// its directory. An `Err` here means "do not connect", not "the daemon is
/// down" — but every existing caller of [`request`]/[`request_with_timeout`]
/// already treats their `Err` as "daemon not reachable" and falls through to
/// its own fallback chain (see `fn request`'s own doc comment), so a
/// distinct error variant would buy nothing. `$XDG_RUNTIME_DIR` itself is
/// trusted as-is (systemd-logind's own `0700`), so this check is skipped
/// there — the daemon side still runs it unconditionally before binding (see
/// `daemon::control::start_server`), and that is what actually creates the
/// directory this function trusts once a real daemon has started at least
/// once.
fn checked_socket_path() -> Result<PathBuf> {
    let (base, used_tmp_fallback) = socket_base_dir();
    let dir = base.join("fresco");
    if used_tmp_fallback {
        ensure_safe_socket_dir(&dir)
            .context("refusing to trust the /tmp control-socket directory")?;
    }
    Ok(dir.join("control.sock"))
}

/// Blocking request to the daemon. Returns Err if the daemon isn't running
/// (connection refused / socket missing) — callers treat that as "not
/// running" — and likewise if the control socket's directory exists but
/// cannot be trusted (see `checked_socket_path`): both are "do not use this
/// socket", and every caller already handles them identically.
pub fn request(req: &Request) -> Result<Response> {
    request_at(&checked_socket_path()?, req, DEFAULT_TIMEOUT)
}

/// Default read/write timeout for `request`. `Apply` on a slow machine can
/// legitimately take longer than this (rebuild + overview refresh), so
/// off-thread callers that expect that use `request_with_timeout` instead.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Like `request`, but with an explicit timeout instead of the 5s default.
/// Used off the GTK main thread, where a slow daemon should time out
/// gracefully rather than hang the worker indefinitely.
pub fn request_with_timeout(req: &Request, timeout: Duration) -> Result<Response> {
    request_at(&checked_socket_path()?, req, timeout)
}

/// Send `req` to the daemon listening at `path`. Split out from `request` so
/// tests can target an isolated (guaranteed-absent) socket deterministically.
fn request_at(path: &std::path::Path, req: &Request, timeout: Duration) -> Result<Response> {
    let mut stream = UnixStream::connect(path)
        .with_context(|| format!("daemon not reachable at {}", path.display()))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    let mut reader = BufReader::new(stream);
    let mut reply = String::new();
    reader
        .read_line(&mut reply)
        .context("reading daemon reply")?;
    let resp: Response = serde_json::from_str(reply.trim()).context("parsing daemon reply")?;
    Ok(resp)
}

/// True if a daemon is up and answering.
pub fn daemon_alive() -> bool {
    matches!(request(&Request::Status), Ok(Response::Status(s)) if s.running)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_json_shape() {
        assert_eq!(
            serde_json::to_string(&Request::Apply).unwrap(),
            r#"{"cmd":"apply"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::Status).unwrap(),
            r#"{"cmd":"status"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::Update).unwrap(),
            r#"{"cmd":"update"}"#
        );
    }

    #[test]
    fn response_roundtrip() {
        let r = Response::Status(StatusReply {
            running: true,
            hwdec: Some("vaapi".into()),
            cpu_percent: 1.5,
            rss_mb: 120,
            monitors: vec!["eDP-1".into()],
            ..Default::default()
        });
        let s = serde_json::to_string(&r).unwrap();
        let back: Response = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    /// Replies from an older daemon (without the audio fields, or —
    /// separately, see below — without `lockscreen`) must still parse.
    #[test]
    fn status_reply_backcompat_without_audio_fields() {
        let old = r#"{"result":"status","running":true,"paused":false,"hwdec":null,
                      "wallpaper":null,"cpu_percent":0.0,"rss_mb":10,"monitors":[],"error":null}"#;
        let r: Response = serde_json::from_str(old).unwrap();
        match r {
            Response::Status(s) => {
                assert_eq!(s.audio_track, None);
                assert_eq!(s.mute, None);
                assert_eq!(s.volume, None);
                assert_eq!(s.source_w, None);
                assert_eq!(s.source_h, None);
                assert_eq!(s.bit_depth, None);
                assert_eq!(s.dropped_frames, None);
                assert!(s.monitors_info.is_empty());
                // Same pre-lock-screen JSON also has no `lockscreen` key at
                // all — a daemon built before this feature existed — and
                // that must default to `None` rather than fail to parse.
                assert_eq!(s.lockscreen, None);
            }
            other => panic!("expected Status, got {other:?}"),
        }
    }

    // -- lock screen: Request/Response exact JSON --------------------------

    #[test]
    fn lock_request_json_shapes() {
        assert_eq!(
            serde_json::to_string(&Request::Lock).unwrap(),
            r#"{"cmd":"lock"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::LockPreview {
                width: 1920,
                height: 1080
            })
            .unwrap(),
            r#"{"cmd":"lockpreview","width":1920,"height":1080}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::LockNotify {
                locked: true,
                sockets: vec![LockSocket {
                    connector: "eDP-1".into(),
                    path: "/run/user/1000/fresco/lock-eDP-1.sock".into(),
                }],
            })
            .unwrap(),
            r#"{"cmd":"locknotify","locked":true,"sockets":[{"connector":"eDP-1","path":"/run/user/1000/fresco/lock-eDP-1.sock"}]}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::LockSetup).unwrap(),
            r#"{"cmd":"locksetup"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::LockUndo).unwrap(),
            r#"{"cmd":"lockundo"}"#
        );
    }

    /// `LockNotify.sockets` is `#[serde(default)]` so the X11 saver's own
    /// future wire format can add sockets later without breaking an older
    /// daemon, and so a bare `{"locked":true}` (no `sockets` key at all)
    /// still parses today — mirroring how every other backward-compat field
    /// in this file is tested.
    #[test]
    fn lock_notify_sockets_default_when_absent() {
        let r: Request = serde_json::from_str(r#"{"cmd":"locknotify","locked":true}"#).unwrap();
        assert_eq!(
            r,
            Request::LockNotify {
                locked: true,
                sockets: Vec::new(),
            }
        );
    }

    #[test]
    fn lock_socket_json_shape() {
        let s = LockSocket {
            connector: "HDMI-A-1".into(),
            path: "/run/user/1000/fresco/lock-HDMI-A-1.sock".into(),
        };
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            r#"{"connector":"HDMI-A-1","path":"/run/user/1000/fresco/lock-HDMI-A-1.sock"}"#
        );
    }

    #[test]
    fn lock_response_json_shapes() {
        assert_eq!(
            serde_json::to_string(&Response::Lock(LockReply {
                host: "cosmic".into(),
                ok: true,
                message: None,
            }))
            .unwrap(),
            r#"{"result":"lock","host":"cosmic","ok":true,"message":null}"#
        );
        assert_eq!(
            serde_json::to_string(&Response::Lock(LockReply {
                host: "wlroots".into(),
                ok: false,
                message: Some("swaylock-plugin not found".into()),
            }))
            .unwrap(),
            r#"{"result":"lock","host":"wlroots","ok":false,"message":"swaylock-plugin not found"}"#
        );
        assert_eq!(
            serde_json::to_string(&Response::LockPreview {
                path: "/tmp/fresco-lock-preview.png".into()
            })
            .unwrap(),
            r#"{"result":"lockpreview","path":"/tmp/fresco-lock-preview.png"}"#
        );
    }

    #[test]
    fn lock_setup_state_json_shapes() {
        assert_eq!(
            serde_json::to_string(&LockSetupState::NotNeeded).unwrap(),
            r#""notneeded""#
        );
        assert_eq!(
            serde_json::to_string(&LockSetupState::Needed).unwrap(),
            r#""needed""#
        );
        assert_eq!(
            serde_json::to_string(&LockSetupState::Done).unwrap(),
            r#""done""#
        );
        assert_eq!(
            serde_json::to_string(&LockSetupState::Unavailable).unwrap(),
            r#""unavailable""#
        );
    }

    #[test]
    fn lock_status_json_shape() {
        let status = LockStatus {
            enabled: true,
            host: "cosmic".into(),
            live_video: true,
            widgets: true,
            still_frame: Some(true),
            locked: false,
            setup: LockSetupState::NotNeeded,
            notes: vec!["cosmic-greeter's own panel stays put for now".into()],
        };
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            r#"{"enabled":true,"host":"cosmic","live_video":true,"widgets":true,"still_frame":true,"locked":false,"setup":"notneeded","notes":["cosmic-greeter's own panel stays put for now"]}"#
        );
    }

    /// A status from a daemon that predates `still_frame` still parses, and
    /// reads as "unknown" (`None`) rather than guessing either way — the GUI
    /// says nothing about the still frame then.
    #[test]
    fn lock_status_without_still_frame_parses_as_unknown() {
        let old = r#"{"enabled":true,"host":"deepin","live_video":false,"widgets":false,"locked":false,"setup":"notneeded","notes":[]}"#;
        let status: LockStatus = serde_json::from_str(old).unwrap();
        assert_eq!(status.still_frame, None);
    }

    // -- lock screen: round-trips -------------------------------------------

    #[test]
    fn lock_requests_roundtrip() {
        let reqs = [
            Request::Lock,
            Request::LockPreview {
                width: 640,
                height: 480,
            },
            Request::LockNotify {
                locked: false,
                sockets: Vec::new(),
            },
            Request::LockNotify {
                locked: true,
                sockets: vec![LockSocket {
                    connector: "eDP-1".into(),
                    path: "/tmp/lock.sock".into(),
                }],
            },
            Request::LockSetup,
            Request::LockUndo,
        ];
        for req in reqs {
            let s = serde_json::to_string(&req).unwrap();
            let back: Request = serde_json::from_str(&s).unwrap();
            assert_eq!(req, back, "roundtrip of {s}");
        }
    }

    #[test]
    fn lock_responses_roundtrip() {
        let resps = [
            Response::Lock(LockReply {
                host: "kde".into(),
                ok: true,
                message: None,
            }),
            Response::Lock(LockReply {
                host: "x11".into(),
                ok: false,
                message: Some("xsecurelock not installed".into()),
            }),
            Response::LockPreview {
                path: "/tmp/preview.png".into(),
            },
        ];
        for resp in resps {
            let s = serde_json::to_string(&resp).unwrap();
            let back: Response = serde_json::from_str(&s).unwrap();
            assert_eq!(resp, back, "roundtrip of {s}");
        }
    }

    /// `StatusReply.lockscreen` round-trips like any other field once a
    /// current daemon actually sets it (as opposed to the backcompat case
    /// above, where it is absent entirely).
    #[test]
    fn status_reply_lockscreen_roundtrips() {
        let r = Response::Status(StatusReply {
            running: true,
            lockscreen: Some(LockStatus {
                enabled: true,
                host: "wlroots".into(),
                live_video: false,
                widgets: false,
                still_frame: Some(false),
                locked: true,
                setup: LockSetupState::Unavailable,
                notes: Vec::new(),
            }),
            ..Default::default()
        });
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains(r#""lockscreen":{"#), "{s}");
        let back: Response = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    /// `wallpaper_path` arrived after the other status fields: a reply from an
    /// older daemon (no such key) must parse as `None`, and a current one must
    /// carry the path through unchanged.
    #[test]
    fn status_reply_wallpaper_path_backcompat_and_roundtrip() {
        let old = r#"{"result":"status","running":true,"paused":false,"hwdec":null,
                      "wallpaper":null,"cpu_percent":0.0,"rss_mb":10,"monitors":[],"error":null}"#;
        match serde_json::from_str::<Response>(old).unwrap() {
            Response::Status(s) => assert_eq!(s.wallpaper_path, None),
            other => panic!("expected a status reply, got {other:?}"),
        }

        let r = Response::Status(StatusReply {
            running: true,
            wallpaper_path: Some(PathBuf::from("/walls/night.mp4")),
            ..Default::default()
        });
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains(r#""wallpaper_path":"/walls/night.mp4""#), "{s}");
        let back: Response = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    /// `paused_reason` arrived late: an older daemon's reply (no key) must
    /// parse as `None`, and a current one round-trips it as a snake_case tag.
    #[test]
    fn status_reply_paused_reason_backcompat_and_roundtrip() {
        let old = r#"{"result":"status","running":true,"paused":true,"hwdec":null,
                      "wallpaper":null,"cpu_percent":0.0,"rss_mb":10,"monitors":[],"error":null}"#;
        match serde_json::from_str::<Response>(old).unwrap() {
            Response::Status(s) => assert_eq!(s.paused_reason, None),
            other => panic!("expected a status reply, got {other:?}"),
        }

        let r = Response::Status(StatusReply {
            running: true,
            paused: true,
            paused_reason: Some(PauseReason::Battery),
            ..Default::default()
        });
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains(r#""paused_reason":"battery""#), "{s}");
        let back: Response = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn pause_reason_none_when_nothing_holds_the_pause() {
        assert_eq!(PauseSources::default().reason(), None);
    }

    #[test]
    fn pause_reason_maps_each_source() {
        let off = PauseSources::default();
        for (sources, want) in [
            (PauseSources { user: true, ..off }, PauseReason::User),
            (
                PauseSources {
                    battery: true,
                    ..off
                },
                PauseReason::Battery,
            ),
            (
                PauseSources {
                    locked: true,
                    ..off
                },
                PauseReason::Locked,
            ),
            (
                PauseSources {
                    fullscreen: true,
                    ..off
                },
                PauseReason::Fullscreen,
            ),
            (
                PauseSources {
                    covered: true,
                    ..off
                },
                PauseReason::Covered,
            ),
        ] {
            assert_eq!(sources.reason(), Some(want), "{sources:?}");
        }
    }

    /// With several sources active the actionable one wins: the user's own
    /// pause, then battery, then the session, then a covering window — and
    /// fullscreen outranks a merely large window.
    #[test]
    fn pause_reason_priority_when_sources_overlap() {
        let mut s = PauseSources {
            user: true,
            battery: true,
            locked: true,
            fullscreen: true,
            covered: true,
        };
        assert_eq!(s.reason(), Some(PauseReason::User));
        s.user = false;
        assert_eq!(s.reason(), Some(PauseReason::Battery));
        s.battery = false;
        assert_eq!(s.reason(), Some(PauseReason::Locked));
        s.locked = false;
        assert_eq!(s.reason(), Some(PauseReason::Fullscreen));
        s.fullscreen = false;
        assert_eq!(s.reason(), Some(PauseReason::Covered));
    }

    #[test]
    fn unreachable_daemon_errors() {
        // Target a socket path with no listener so the result is deterministic
        // even when a real frescod is running on this machine.
        let path = std::env::temp_dir().join(format!("fresco-absent-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert!(request_at(&path, &Request::Status, DEFAULT_TIMEOUT).is_err());
    }

    /// A daemon that accepts the connection but never replies must still time
    /// out promptly, not hang the caller forever — this is what protects a
    /// worker thread from a wedged daemon.
    #[test]
    fn slow_daemon_times_out() {
        use std::os::unix::net::UnixListener;

        let dir = std::env::temp_dir().join(format!("fresco-ipc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("control.sock");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();

        // Accept the connection but never write a reply, then hold the thread
        // open so the socket stays alive for the duration of the test.
        let handle = std::thread::spawn(move || {
            if let Ok((_stream, _addr)) = listener.accept() {
                std::thread::sleep(Duration::from_secs(2));
            }
        });

        let start = std::time::Instant::now();
        let result = request_at(&path, &Request::Status, Duration::from_millis(200));
        assert!(result.is_err());
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "request_at should time out near the requested 200ms, took {:?}",
            start.elapsed()
        );

        drop(handle); // detach; the test process exit reaps it
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    // -- dir_is_trustworthy: pure judgement, every branch including the
    //    uid-mismatch one no single-user test environment can otherwise hit --

    #[test]
    fn dir_is_trustworthy_accepts_a_private_dir_we_own() {
        assert_eq!(dir_is_trustworthy(false, true, 1000, 0o700, 1000), Ok(()));
    }

    #[test]
    fn dir_is_trustworthy_rejects_a_symlink() {
        assert_eq!(
            dir_is_trustworthy(true, true, 1000, 0o700, 1000),
            Err("is a symlink")
        );
    }

    #[test]
    fn dir_is_trustworthy_rejects_a_non_directory() {
        assert_eq!(
            dir_is_trustworthy(false, false, 1000, 0o700, 1000),
            Err("is not a directory")
        );
    }

    #[test]
    fn dir_is_trustworthy_rejects_a_directory_owned_by_someone_else() {
        assert_eq!(
            dir_is_trustworthy(false, true, 1001, 0o700, 1000),
            Err("is not owned by us")
        );
    }

    #[test]
    fn dir_is_trustworthy_rejects_group_or_other_accessible_modes() {
        assert_eq!(
            dir_is_trustworthy(false, true, 1000, 0o750, 1000),
            Err("is readable/writable/traversable by other users")
        );
        assert_eq!(
            dir_is_trustworthy(false, true, 1000, 0o705, 1000),
            Err("is readable/writable/traversable by other users")
        );
        // 0o700 exactly is the only mode this accepts among the "owner-only"
        // family — 0o600 (no execute/traverse bit) is a directory nobody,
        // including us, could actually list, which is a different bug this
        // gate does not need to also catch.
        assert_eq!(dir_is_trustworthy(false, true, 1000, 0o700, 1000), Ok(()));
    }

    // -- ensure_safe_socket_dir / verify_safe_dir: real temp-dir fixtures ----

    fn ipc_tempdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fresco-ipc-secdir-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ensure_safe_socket_dir_creates_a_fresh_private_dir() {
        let parent = ipc_tempdir("happy-path");
        let target = parent.join("fresco");
        assert!(!target.exists());

        ensure_safe_socket_dir(&target).expect("fresh directory must be accepted");

        let meta = std::fs::symlink_metadata(&target).unwrap();
        assert!(meta.is_dir());
        assert!(!meta.file_type().is_symlink());
        assert_eq!(meta.mode() & 0o777, 0o700);

        std::fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn ensure_safe_socket_dir_accepts_its_own_dir_on_a_second_call() {
        let parent = ipc_tempdir("idempotent");
        let target = parent.join("fresco");

        ensure_safe_socket_dir(&target).unwrap();
        // The daemon calls this every time it starts, not just once — a
        // second call against the same, still-safe directory must succeed
        // exactly like the first, not treat `AlreadyExists` as a problem.
        ensure_safe_socket_dir(&target).expect("re-checking our own safe dir must succeed");

        std::fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn ensure_safe_socket_dir_rejects_a_pre_existing_symlink() {
        let parent = ipc_tempdir("symlink");
        let real_elsewhere = parent.join("attacker-owned-elsewhere");
        std::fs::create_dir_all(&real_elsewhere).unwrap();
        let target = parent.join("fresco");
        std::os::unix::fs::symlink(&real_elsewhere, &target).unwrap();

        let err = ensure_safe_socket_dir(&target).unwrap_err();
        assert!(format!("{err:#}").contains("symlink"), "{err:#}");

        std::fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn ensure_safe_socket_dir_repairs_a_pre_existing_dir_with_a_wide_mode() {
        // Pre-1.1.46 releases created this directory with `create_dir_all`
        // (umask 022 → 0755), so every upgrading user has one: it must be
        // tightened to 0700 and accepted, not refused (which bricked the
        // daemon on update with "refusing to trust it").
        let parent = ipc_tempdir("wide-mode");
        let target = parent.join("fresco");
        std::fs::create_dir_all(&target).unwrap();
        // World-readable/traversable: exactly what the old releases left.
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();

        ensure_safe_socket_dir(&target)
            .expect("a 0755 dir we own must be tightened and accepted, not refused");

        let meta = std::fs::symlink_metadata(&target).unwrap();
        assert_eq!(meta.mode() & 0o777, 0o700);

        std::fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn ensure_safe_socket_dir_accepts_a_pre_existing_dir_with_exactly_0700() {
        let parent = ipc_tempdir("pre-existing-ok");
        let target = parent.join("fresco");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700)).unwrap();

        ensure_safe_socket_dir(&target)
            .expect("a pre-existing 0700 dir we own must be accepted, not just one we created");

        std::fs::remove_dir_all(&parent).ok();
    }
}

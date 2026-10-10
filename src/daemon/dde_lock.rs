//! Deepin lock-screen background sync (GitHub issue #37) — the same problem
//! `overview` solves for GNOME and `cosmic_bg` for COSMIC, against DDE's own
//! store.
//!
//! Fresco's live wallpaper is a window DDE's lock screen (`dde-lock`, and the
//! `lightdm` greeter before login) never draws: both paint the per-user
//! *greeter background* instead. Left alone that is Deepin's stock
//! `default_lock_background.jpg`, so with "Show Fresco on the lock screen" on
//! the user still saw Deepin's wallpaper, not even a still of their own. This
//! module keeps that background pointed at a still frame of Fresco's wallpaper,
//! and puts the user's original back when Fresco stops.
//!
//! HARD RULE (feature-wide): Fresco never touches authentication. This module
//! only ever asks DDE to use a different picture, through the same calls its
//! own control center and file manager make. `dde-lock` still owns the
//! password prompt.
//!
//! # What DDE does (read from upstream `master`, 2026-10-10)
//!
//! * **Setting.** `org.deepin.dde.Appearance1` (session bus; legacy
//!   `com.deepin.daemon.Appearance`) has no dedicated method — the greeter
//!   background goes through `Set("greeterbackground", uri)`
//!   (`linuxdeepin/dde-appearance`: `dbus/org.deepin.dde.Appearance1.xml`;
//!   `TYPEGREETERBACKGROUND` in `src/service/modules/common/commondefine.h`;
//!   `doSetByType` → `doSetGreeterBackground` in
//!   `src/service/impl/appearancemanager.cpp`). Control center's "Lock screen"
//!   option and the file manager's wallpaper dialog make the same call. It is
//!   queued and returns nothing, so success cannot be read off the call.
//! * **Store of record.** `doSetGreeterBackground` forwards to
//!   `org.deepin.dde.Accounts1.User.SetGreeterBackground` on the *system* bus,
//!   which validates the file (gif/jpeg/png/bmp/tiff by content) and stores the
//!   `GreeterBackground` property on `/org/deepin/dde/Accounts1/User<uid>`
//!   (`linuxdeepin/dde-daemon`: `accounts1/user_ifc.go`). That property is also
//!   the only place the current value can be read back, which is how a set is
//!   verified here. Side effect of going through Appearance: it flags
//!   `isCustomLockBackground` in its dconfig, so a later light/dark theme
//!   switch keeps the lock background instead of resetting it — see
//!   [`restore`] for what that means for the user's original.
//! * **Reading, and why 1.1.48 changed nothing.** `dde-lock` reads that
//!   property and follows its `GreeterBackgroundChanged` signal
//!   (`linuxdeepin/dde-session-shell`: `src/session-widgets/userinfo.cpp`). It
//!   then does not draw the picture itself: `FullScreenBackground::
//!   updateBlurBackground` (`src/widgets/fullscreenbackground.cpp`) asks the
//!   *system* bus `org.deepin.dde.ImageEffect1.Get("", path)` (2 s timeout) for
//!   a processed copy and draws that. When the call fails, or returns nothing
//!   readable, it silently draws `/usr/share/backgrounds/default_background.jpg`
//!   instead — Deepin's own wallpaper, while the Accounts property (what the
//!   old code read back) still named our frame. The service behind
//!   `ImageEffect1` is, on current Deepin 25, the wallpaper-cache plugin of
//!   `linuxdeepin/dde-services` (`src/plugin-qt/wallpapercache/`). It runs as
//!   `deepin-daemon` under `PrivateTmp=yes` and `PrivateUsers=yes`
//!   (`misc/deepin-service-plugin@org.deepin.dde.WallpaperCache.service.d/
//!   override.conf`), and `WallpaperCacheService::Get` answers `QString()` for a
//!   path it cannot open. So a frame in `/var/tmp` or `/tmp` does not exist for
//!   it, and neither does one under a home directory that is not
//!   world-searchable: the old frame directories (see [`pick_frame_dir`]) were
//!   exactly the places it cannot see.
//! * **Where Deepin keeps pictures it must share.** Control center and
//!   `dde-appearance` (`Backgrounds::prepare`) copy every user picture through
//!   the root helper `org.deepin.dde.Daemon1.SaveCustomWallPaper`
//!   (`linuxdeepin/dde-daemon`: `bin/dde-system-daemon/wallpaper.go`) into
//!   `/var/cache/wallpapers/custom-wallpapers/<user>/<md5>.<ext>` (mode 0644,
//!   directories 0755), which every service can read. This module does the same
//!   ([`Greeter::store`]) and points the greeter background at the copy. The
//!   helper's signature changed on 2026-06-30 (`eae6bb95`): `(s user, h fd,
//!   s type)` since, `(s user, s file)` before (the helper then read the file as
//!   the user, and `solid::` in front of the path chose the solid dir). It is
//!   read from the service's introspection so both work ([`save_args`]). The
//!   `h` argument is a real D-Bus file descriptor, which `gdbus` sends from its
//!   own stdin (`@h 0`); `busctl` rejects the type. Frames go to the
//!   `custom-solidwallpapers` dir ("solid"), not the custom one, so they do not
//!   push the user's own pictures out of the 20-per-directory cap; the helper
//!   evicts the oldest file past that cap, so even a crash cannot grow it.
//!   One copy is kept at a time: the previous one is deleted
//!   (`DeleteCustomWallPaper`, no polkit for one's own files) when a new frame
//!   is installed, and on restore. Without the helper (older DDE) the frame is
//!   used where it was rendered, as 1.1.48 did.
//! * **Processing and blur.** `Get("")` is the `pixmix` effect
//!   (`imageeffectprocessor.cpp`): a darkened, saturated average colour laid
//!   over the picture at 90 % opacity. Deepin always does that to the lock
//!   background, so blurring the frame first only costs time; the frame carries
//!   the user's dim, not their blur. The service caches by `md5(path)` and
//!   returns the cached image while its mtime is not older than the source's,
//!   without comparing contents. A copy's name is the md5 of its content, so
//!   changed pixels are a new path; the frame itself gets a fresh timestamped
//!   name for the same reason.
//! * **Verifying the blur step.** After a set, this module asks
//!   `ImageEffect1.Get` itself, which is what `dde-lock` will do next. An empty
//!   answer is logged as the cause of "Deepin still shows its own background".
//!   The call also has the service build the processed image now instead of
//!   inside `dde-lock`'s 2 s window.
//! * **Known limitation.** `dde-lock` also swaps the background for the primary
//!   monitor's current-workspace *desktop* wallpaper whenever the workspace
//!   changes while the screen is locked (`LockContent::currentWorkspaceChanged`
//!   in `src/session-widgets/lockcontent.cpp`). That is not touched here:
//!   setting Deepin's desktop wallpaper would hand Fresco the user's desktop,
//!   and DDE's X11 strategies already use it for their own purposes (see
//!   `dde`).
//! * **Widgets.** Not in the frame: the greeter background is one frozen
//!   picture, so a clock in it would be wrong a minute later.
//!
//! # Backup and restore
//!
//! The first time a frame is installed, the user's current greeter background
//! is saved to `$XDG_STATE_HOME/fresco/dde-saved-lock-background.json`, with the
//! copies this module has put in Deepin's wallpaper store. An existing state
//! file is **never** overwritten — after a crash it holds the true original, and
//! "current" is by then our own frame. [`restore`] puts the original back on
//! Stop/SIGTERM and again at startup when the lock feature is off (crash
//! recovery), then deletes the copies, but only while the lock screen still
//! shows one of our frames or copies: if the user chose another picture in the
//! meantime, theirs wins and the state is just dropped. Restoring through
//! Appearance leaves `isCustomLockBackground` set (see above); nothing here can
//! clear it.
//!
//! Only runs on Deepin and only while `[lockscreen].enabled`; everywhere else
//! every entry point returns before touching anything.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::dde::{self, Bus};
use super::overview::{encode_file_uri, gvariant_string_literal};
use crate::config::Config;

/// Prefix of every frame this module writes; what [`is_our_frame`] keys on.
pub(super) const FRAME_PREFIX: &str = "dde-lock-";

/// State file name (under `dde::state_dir()`).
const SAVED_FILE: &str = "dde-saved-lock-background.json";

/// `Appearance1.Set`'s type name for the greeter background.
const TYPE_GREETER_BACKGROUND: &str = "greeterbackground";

/// The per-user Accounts object that stores the greeter background:
/// (service, object-path prefix — the uid is appended, interface). Deepin 25
/// first, then the pre-25 names.
const ACCOUNTS: [(&str, &str, &str); 2] = [
    (
        "org.deepin.dde.Accounts1",
        "/org/deepin/dde/Accounts1/User",
        "org.deepin.dde.Accounts1.User",
    ),
    (
        "com.deepin.daemon.Accounts",
        "/com/deepin/daemon/Accounts/User",
        "com.deepin.daemon.Accounts.User",
    ),
];

/// Deepin's root helper (system bus): (service, object path, interface). Its
/// `SaveCustomWallPaper` copies a picture into the store every Deepin service
/// can read; `DeleteCustomWallPaper` removes our copy again.
const DAEMON: (&str, &str, &str) = (
    "org.deepin.dde.Daemon1",
    "/org/deepin/dde/Daemon1",
    "org.deepin.dde.Daemon1",
);

/// The service `dde-lock` asks for the processed lock background (system bus):
/// (service, object path, interface).
const IMAGE_EFFECT: (&str, &str, &str) = (
    "org.deepin.dde.ImageEffect1",
    "/org/deepin/dde/ImageEffect1",
    "org.deepin.dde.ImageEffect1",
);

/// `SaveCustomWallPaper`'s picture kind for our frames: the solid-colour
/// directory, so they never push the user's own custom pictures out of the
/// per-directory cap.
const WALLPAPER_KIND: &str = "solid";

/// What the pre-`h` `SaveCustomWallPaper(user, file)` reads in front of the
/// path to pick the solid directory.
const LEGACY_SOLID_PREFIX: &str = "solid::";

/// How long a set is given to show up in the Accounts property before it is
/// reported as not accepted. The set is queued twice (Appearance, then
/// Accounts); on a healthy session the first readback already matches.
const PATIENCE: Patience = Patience {
    tries: 15,
    step: Duration::from_millis(100),
};

/// Frames the first failure of a run is logged at `warn`; the same failure on
/// every later apply (a rotating schedule) drops to `debug`.
static FAILURE_LOGGED: AtomicBool = AtomicBool::new(false);

// -- entry points ------------------------------------------------------------

/// What [`apply`] does for a given session and config.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    /// Not Deepin: nothing here concerns this session.
    Nothing,
    /// Deepin with the lock feature on: show a still of the wallpaper.
    Sync,
    /// Deepin with the lock feature off: give back anything a previous run
    /// (or a toggle just now) left behind.
    Restore,
}

fn decide(deepin: bool, lock_enabled: bool) -> Action {
    match (deepin, lock_enabled) {
        (false, _) => Action::Nothing,
        (true, true) => Action::Sync,
        (true, false) => Action::Restore,
    }
}

/// Keep Deepin's lock-screen background on a still frame of `config`'s
/// wallpaper — or, with the lock feature off, put the user's own back. No-op
/// off Deepin.
pub fn apply(config: &Config) {
    let lock_enabled = config.lockscreen.as_ref().is_some_and(|l| l.enabled);
    match decide(crate::capability::is_deepin_dde(), lock_enabled) {
        Action::Nothing => {}
        Action::Restore => restore(),
        Action::Sync => sync(config),
    }
}

/// Put the user's original lock-screen background back and forget the saved
/// state. Safe to call at any time: with nothing saved (or off Deepin) it does
/// nothing, and it keeps the state when DDE cannot be reached so a later run
/// can still finish the job.
pub fn restore() {
    if !crate::capability::is_deepin_dde() {
        return;
    }
    let Some(uid) = crate::userinfo::current_uid() else {
        return;
    };
    let outcome = restore_in(&Dde { uid }, &saved_path(), &frame_dirs(uid), PATIENCE);
    match outcome {
        Restored::NothingSaved => {}
        Restored::Done => log::info!("DDE lock screen: original background restored"),
        Restored::Discarded => log::info!(
            "DDE lock screen: background was changed since Fresco set it; leaving the new one"
        ),
        Restored::Corrupt => {
            log::warn!("DDE lock screen: dropped unreadable saved-background state")
        }
        Restored::Pending => log_failure(
            "DDE lock screen: could not put the original background back yet (DDE not \
             reachable, or it refused the picture); the saved state is kept for the next run",
        ),
    }
}

fn sync(config: &Config) {
    let Some(uid) = crate::userinfo::current_uid() else {
        return;
    };
    let dir = pick_frame_dir(uid);
    let Some(frame) = render_frame(config, &dir) else {
        log::debug!("DDE lock screen: no still available for the wallpaper; leaving it as-is");
        return;
    };
    match sync_in(
        &Dde { uid },
        &saved_path(),
        &frame_dirs(uid),
        &frame,
        PATIENCE,
    ) {
        Synced::Applied {
            installed,
            blur_reads,
        } => {
            FAILURE_LOGGED.store(false, Ordering::Relaxed);
            log::info!("DDE lock screen: background set to {}", installed.display());
            if blur_reads == Some(false) {
                log_failure(&format!(
                    "DDE lock screen: Deepin's blur service (org.deepin.dde.ImageEffect1) cannot \
                     read {}, so the lock screen will show Deepin's default background instead",
                    installed.display()
                ));
            }
            if let Some(blocked) = first_unsearchable_ancestor(&installed) {
                static PRIVATE_WARNED: std::sync::Once = std::sync::Once::new();
                PRIVATE_WARNED.call_once(|| {
                    log::warn!(
                        "DDE lock screen: {} is not searchable by other users, so Deepin's \
                         blur service and greeter may not be able to read the frame and \
                         will fall back to the default picture",
                        blocked.display()
                    );
                });
            }
        }
        Synced::Unreadable => log_failure(
            "DDE lock screen: could not read the current lock-screen background from \
             Deepin's Accounts service, so it could not be restored later; leaving it alone",
        ),
        Synced::StateNotSaved => log_failure(
            "DDE lock screen: could not save the original lock-screen background; leaving it alone",
        ),
        Synced::NotRequested => log_failure(
            "DDE lock screen: Deepin's Appearance service refused the lock-screen background \
             request (is org.deepin.dde.Appearance1 running?)",
        ),
        Synced::NotAccepted => log_failure(
            "DDE lock screen: Deepin did not switch to the new lock-screen background \
             (picture rejected, or the wallpaper is locked by an administrator)",
        ),
    }
}

/// `warn` the first time per run, `debug` after.
fn log_failure(msg: &str) {
    if FAILURE_LOGGED.swap(true, Ordering::Relaxed) {
        log::debug!("{msg}");
    } else {
        log::warn!("{msg}");
    }
}

// -- DDE access --------------------------------------------------------------

/// The operations on DDE's lock-screen background, so the save/restore logic
/// can be exercised without a Deepin session.
trait Greeter {
    /// The current greeter background exactly as DDE reports it (a
    /// `file://` URI or a path); `None` when it cannot be read or is empty.
    fn current(&self) -> Option<String>;
    /// Ask DDE to use `uri`. True when a service took the request — not that
    /// it has been applied; check with [`Greeter::current`].
    fn set(&self, uri: &str) -> bool;
    /// Copy `frame` into Deepin's own wallpaper store, where every Deepin
    /// service can read it; the copy's absolute path. `None` when Deepin has no
    /// such helper (older DDE) or refused the picture.
    fn store(&self, frame: &Path) -> Option<String>;
    /// Delete a copy [`Greeter::store`] returned. Best effort.
    fn forget(&self, copy: &str);
    /// Whether Deepin's blur service, which `dde-lock` asks for the picture it
    /// draws, can read `picture`: `Some(false)` when it answers with nothing,
    /// `None` when it could not be asked (not there, or too slow).
    fn blur_reads(&self, picture: &Path) -> Option<bool>;
}

/// The real thing: `gdbus` against the Accounts (read) and Appearance (write)
/// services.
struct Dde {
    uid: u32,
}

/// Object path of `uid`'s Accounts object under `prefix` (one of [`ACCOUNTS`]).
fn accounts_user_path(prefix: &str, uid: u32) -> String {
    format!("{prefix}{uid}")
}

impl Greeter for Dde {
    fn current(&self) -> Option<String> {
        for (dest, prefix, iface) in ACCOUNTS {
            let path = accounts_user_path(prefix, self.uid);
            let out = dde::gdbus_call_on(
                Bus::System,
                dest,
                &path,
                "org.freedesktop.DBus.Properties.Get",
                &[
                    &gvariant_string_literal(iface),
                    &gvariant_string_literal("GreeterBackground"),
                ],
            );
            if let Some(value) = out.as_deref().and_then(dde::parse_first_string) {
                return (!value.is_empty()).then_some(value);
            }
        }
        None
    }

    fn set(&self, uri: &str) -> bool {
        for (dest, path, iface) in dde::SERVICES {
            let accepted = dde::gdbus_call_on(
                Bus::Session,
                dest,
                path,
                &format!("{iface}.Set"),
                &[
                    &gvariant_string_literal(TYPE_GREETER_BACKGROUND),
                    &gvariant_string_literal(uri),
                ],
            )
            .is_some();
            if accepted {
                return true;
            }
        }
        false
    }

    fn store(&self, frame: &Path) -> Option<String> {
        let login = crate::userinfo::current_login()?;
        let (dest, path, iface) = DAEMON;
        let xml = dde::gdbus_call_on(
            Bus::System,
            dest,
            path,
            "org.freedesktop.DBus.Introspectable.Introspect",
            &[],
        )
        .as_deref()
        .and_then(dde::parse_first_string)?;
        let inputs = introspected_inputs(&xml, "SaveCustomWallPaper")?;
        let call = save_args(&inputs, &login, frame)?;
        let args: Vec<&str> = call.args.iter().map(String::as_str).collect();
        let method = format!("{iface}.SaveCustomWallPaper");
        let out = if call.send_fd {
            let file = std::fs::File::open(frame).ok()?;
            dde::gdbus_call_on_with_fd0(Bus::System, dest, path, &method, &args, file)
        } else {
            dde::gdbus_call_on(Bus::System, dest, path, &method, &args)
        }?;
        dde::parse_first_string(&out).filter(|p| Path::new(p).is_absolute())
    }

    fn forget(&self, copy: &str) {
        let Some(login) = crate::userinfo::current_login() else {
            return;
        };
        let (dest, path, iface) = DAEMON;
        let gone = dde::gdbus_call_on(
            Bus::System,
            dest,
            path,
            &format!("{iface}.DeleteCustomWallPaper"),
            &[
                &gvariant_string_literal(&login),
                &gvariant_string_literal(copy),
            ],
        );
        if gone.is_none() {
            log::debug!("DDE lock screen: could not delete {copy} from Deepin's wallpaper store");
        }
    }

    fn blur_reads(&self, picture: &Path) -> Option<bool> {
        let (dest, path, iface) = IMAGE_EFFECT;
        let out = dde::gdbus_call_on(
            Bus::System,
            dest,
            path,
            &format!("{iface}.Get"),
            &[
                &gvariant_string_literal(""),
                &gvariant_string_literal(&picture.to_string_lossy()),
            ],
        )?;
        Some(dde::parse_first_string(&out).is_some_and(|p| !p.is_empty()))
    }
}

/// The types of the `in` arguments of `method` in a D-Bus introspection
/// document, in order. `None` when the method is not described.
fn introspected_inputs(xml: &str, method: &str) -> Option<Vec<String>> {
    let start = xml.find(&format!("<method name=\"{method}\""))?;
    let body = &xml[start..];
    let body = &body[..body.find("</method>").unwrap_or(body.len())];
    let mut inputs = Vec::new();
    for tag in body.split("<arg ").skip(1) {
        let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
        if !tag.contains("direction=\"in\"") {
            continue;
        }
        let ty = tag.split("type=\"").nth(1)?;
        inputs.push(ty[..ty.find('"')?].to_string());
    }
    Some(inputs)
}

/// One `SaveCustomWallPaper` call, ready for `gdbus`.
#[derive(Debug, PartialEq, Eq)]
struct SaveCall {
    /// Arguments in GVariant text form.
    args: Vec<String>,
    /// The picture travels as a file descriptor (`@h 0`, the child's stdin).
    send_fd: bool,
}

/// The call for a helper whose `in` arguments are `inputs`: `(s user, h file,
/// s kind)` since dde-daemon `eae6bb95` (2026-06-30), `(s user, s file)`
/// before, which takes the path and reads it as the user. Anything else is a
/// helper this module does not know how to call.
fn save_args(inputs: &[String], login: &str, frame: &Path) -> Option<SaveCall> {
    let user = gvariant_string_literal(login);
    match inputs.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["s", "h", "s"] => Some(SaveCall {
            args: vec![
                user,
                "@h 0".to_string(),
                gvariant_string_literal(WALLPAPER_KIND),
            ],
            send_fd: true,
        }),
        ["s", "s"] => Some(SaveCall {
            args: vec![
                user,
                gvariant_string_literal(&format!("{LEGACY_SOLID_PREFIX}{}", frame.display())),
            ],
            send_fd: false,
        }),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct Patience {
    tries: u32,
    step: Duration,
}

/// Wait for DDE to report `want` as the greeter background.
fn shows<G: Greeter>(g: &G, want: &str, patience: Patience) -> bool {
    for attempt in 0..patience.tries {
        if attempt > 0 {
            std::thread::sleep(patience.step);
        }
        if g.current().is_some_and(|c| same_picture(&c, want)) {
            return true;
        }
    }
    false
}

// -- URIs --------------------------------------------------------------------

/// Undo `%XX` escapes. Anything that is not a valid escape is kept as written
/// (DDE stores `file://` plus the *decoded* path, so a stray `%` is data).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &bytes[i + 1..i + 3];
            // `from_str_radix` alone would also take a leading `+`.
            if hex.iter().all(u8::is_ascii_hexdigit) {
                if let Some(b) = std::str::from_utf8(hex)
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                {
                    out.push(b);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The local path a greeter-background value names: `file:///a%20b.png` and
/// `/a b.png` both give `/a b.png`. `None` for anything that is not an
/// absolute local path.
pub(super) fn uri_to_path(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    let raw = value.strip_prefix("file://").unwrap_or(value);
    raw.starts_with('/')
        .then(|| PathBuf::from(percent_decode(raw)))
}

/// Whether two greeter-background values name the same file, however each is
/// spelled.
fn same_picture(a: &str, b: &str) -> bool {
    matches!((uri_to_path(a), uri_to_path(b)), (Some(x), Some(y)) if x == y)
}

/// Whether `value` names a frame this module wrote: our prefix, a `.png`, and
/// directly inside one of `dirs`.
fn is_our_frame(value: &str, dirs: &[PathBuf]) -> bool {
    let Some(path) = uri_to_path(value) else {
        return false;
    };
    let named = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(FRAME_PREFIX) && n.ends_with(".png"));
    named && path.parent().is_some_and(|p| dirs.iter().any(|d| d == p))
}

// -- save / restore decisions -------------------------------------------------

/// What to do about the user's original before installing a frame.
#[derive(Debug, PartialEq, Eq)]
enum SaveStep {
    /// A state file exists: it holds the original; never overwrite it.
    AlreadySaved,
    /// Record this value as the original.
    Save(String),
    /// No state, yet the lock screen already shows one of our frames (the
    /// state was lost): nothing genuine to record. Carry on; there will be
    /// nothing to restore.
    OwnFrameNoState,
    /// The current value cannot be read, so it could not be given back:
    /// touch nothing.
    Unreadable,
}

fn save_step(current: Option<&str>, state_exists: bool, dirs: &[PathBuf]) -> SaveStep {
    if state_exists {
        return SaveStep::AlreadySaved;
    }
    match current {
        None => SaveStep::Unreadable,
        Some(c) if is_our_frame(c, dirs) => SaveStep::OwnFrameNoState,
        Some(c) => SaveStep::Save(c.to_string()),
    }
}

/// What to do about a saved original when giving the lock screen back.
#[derive(Debug, PartialEq, Eq)]
enum RestoreStep {
    /// The lock screen still shows our frame: put this back.
    Write(String),
    /// It shows something else (the user picked another picture, or our set
    /// never took): leave it, drop the state.
    Discard,
    /// Cannot tell what it shows: keep the state and try later.
    Keep,
}

/// `copies` are the pictures this module put in Deepin's wallpaper store, as
/// the state file lists them; those are ours as much as the frames in `dirs`.
fn restore_step(
    saved: &str,
    current: Option<&str>,
    dirs: &[PathBuf],
    copies: &[String],
) -> RestoreStep {
    match current {
        None => RestoreStep::Keep,
        Some(c) if is_our_frame(c, dirs) || copies.iter().any(|x| same_picture(x, c)) => {
            RestoreStep::Write(saved.to_string())
        }
        Some(_) => RestoreStep::Discard,
    }
}

// -- the persisted original ----------------------------------------------------

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Saved {
    /// The user's greeter background, exactly as DDE reported it.
    uri: String,
    /// Copies of our frames in Deepin's wallpaper store that are not deleted
    /// yet, as [`Greeter::store`] returned them. Absent in state files from
    /// before the copies existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    copies: Vec<String>,
}

fn saved_path() -> PathBuf {
    dde::state_dir().join(SAVED_FILE)
}

enum Load {
    Absent,
    Valid(Saved),
    Corrupt,
}

fn load_saved(path: &Path) -> Load {
    match std::fs::read(path) {
        Err(_) => Load::Absent,
        Ok(bytes) => match serde_json::from_slice::<Saved>(&bytes) {
            Ok(s) if !s.uri.is_empty() => Load::Valid(s),
            _ => Load::Corrupt,
        },
    }
}

fn write_saved(path: &Path, saved: &Saved) -> bool {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    serde_json::to_vec_pretty(saved)
        .ok()
        .is_some_and(|b| std::fs::write(path, b).is_ok())
}

/// The copies the state file lists; none without a valid one.
fn tracked_copies(path: &Path) -> Vec<String> {
    match load_saved(path) {
        Load::Valid(s) => s.copies,
        _ => Vec::new(),
    }
}

/// Replace the state file's list of copies. Does nothing without a valid state
/// file: the original is then lost, there is nothing to restore, and so nothing
/// to clean up either (Deepin itself caps the directory at 20 files).
fn set_tracked_copies(path: &Path, copies: Vec<String>) {
    if let Load::Valid(mut saved) = load_saved(path) {
        saved.copies = copies;
        write_saved(path, &saved);
    }
}

/// The copies of our frames now in Deepin's wallpaper store, as paths. They sit
/// beside the user's own wallpapers there, so `dde::user_wallpaper` skips them.
pub(super) fn our_copies() -> Vec<PathBuf> {
    tracked_copies(&saved_path())
        .iter()
        .filter_map(|c| uri_to_path(c))
        .collect()
}

// -- sync / restore flows ------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Synced {
    Applied {
        /// The picture the greeter background now names: the copy in Deepin's
        /// wallpaper store, or the frame itself when there is no such store.
        installed: PathBuf,
        /// Whether Deepin's blur service can read it (see
        /// [`Greeter::blur_reads`]).
        blur_reads: Option<bool>,
    },
    /// The current value could not be read; nothing was changed.
    Unreadable,
    /// The original could not be written to disk; nothing was changed.
    StateNotSaved,
    /// No Appearance service took the request.
    NotRequested,
    /// The request was taken but Deepin never reported the new picture.
    NotAccepted,
}

/// Install `frame` (already rendered, named with [`FRAME_PREFIX`], in one of
/// `dirs`) as the greeter background, saving the original into `state` first.
/// The frame goes through [`Greeter::store`] when Deepin has one; the previous
/// copy is deleted once the new one shows, so one slot is ever used.
fn sync_in<G: Greeter>(
    g: &G,
    state: &Path,
    dirs: &[PathBuf],
    frame: &Path,
    patience: Patience,
) -> Synced {
    let current = g.current();
    match save_step(current.as_deref(), state.exists(), dirs) {
        SaveStep::AlreadySaved | SaveStep::OwnFrameNoState => {}
        SaveStep::Save(original) => {
            let saved = Saved {
                uri: original,
                copies: Vec::new(),
            };
            if !write_saved(state, &saved) {
                std::fs::remove_file(frame).ok();
                return Synced::StateNotSaved;
            }
        }
        SaveStep::Unreadable => {
            std::fs::remove_file(frame).ok();
            return Synced::Unreadable;
        }
    }

    let previous = tracked_copies(state);
    let copy = g.store(frame);
    let installed = match &copy {
        Some(c) => {
            std::fs::remove_file(frame).ok();
            // Listed before the set, so a request that lands late is still
            // cleaned up by a restore.
            let mut listed = previous.clone();
            if !listed.contains(c) {
                listed.push(c.clone());
            }
            set_tracked_copies(state, listed);
            PathBuf::from(c)
        }
        None => {
            log::debug!(
                "DDE lock screen: no wallpaper store to copy the frame into; using it in place"
            );
            frame.to_path_buf()
        }
    };

    let uri = encode_file_uri(&installed);
    if !g.set(&uri) {
        match &copy {
            // A copy an earlier sync made is not ours to delete here: it may
            // be what the lock screen shows.
            Some(c) if !previous.contains(c) => g.forget(c),
            Some(_) => {}
            None => {
                std::fs::remove_file(frame).ok();
            }
        }
        return Synced::NotRequested;
    }
    if !shows(g, &uri, patience) {
        // Kept on purpose: the request may still land, and then the lock
        // screen needs the file. The next sync or restore sweeps it.
        return Synced::NotAccepted;
    }
    for old in previous.iter().filter(|p| Some(*p) != copy.as_ref()) {
        g.forget(old);
    }
    set_tracked_copies(state, copy.iter().cloned().collect());
    remove_frames(dirs, copy.is_none().then_some(frame));
    let blur_reads = g.blur_reads(&installed);
    Synced::Applied {
        installed,
        blur_reads,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Restored {
    NothingSaved,
    Done,
    Discarded,
    Corrupt,
    /// DDE could not be read or refused the original; state kept.
    Pending,
}

/// Forget the state, and with it every picture of ours: the copies in Deepin's
/// wallpaper store and the frames on disk.
fn clean_up<G: Greeter>(g: &G, state: &Path, dirs: &[PathBuf], copies: &[String]) {
    for copy in copies {
        g.forget(copy);
    }
    std::fs::remove_file(state).ok();
    remove_frames(dirs, None);
}

fn restore_in<G: Greeter>(g: &G, state: &Path, dirs: &[PathBuf], patience: Patience) -> Restored {
    let saved = match load_saved(state) {
        Load::Absent => return Restored::NothingSaved,
        Load::Corrupt => {
            std::fs::remove_file(state).ok();
            return Restored::Corrupt;
        }
        Load::Valid(s) => s,
    };
    match restore_step(&saved.uri, g.current().as_deref(), dirs, &saved.copies) {
        RestoreStep::Keep => Restored::Pending,
        RestoreStep::Discard => {
            clean_up(g, state, dirs, &saved.copies);
            Restored::Discarded
        }
        RestoreStep::Write(uri) => {
            if g.set(&uri) && shows(g, &uri, patience) {
                clean_up(g, state, dirs, &saved.copies);
                Restored::Done
            } else {
                Restored::Pending
            }
        }
    }
}

/// Delete our frames from `dirs`, except `keep`.
fn remove_frames(dirs: &[PathBuf], keep: Option<&Path>) {
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let ours = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(FRAME_PREFIX));
            if ours && Some(path.as_path()) != keep {
                std::fs::remove_file(path).ok();
            }
        }
    }
}

// -- where frames live ---------------------------------------------------------

fn cache_frame_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("fresco")
}

fn shared_frame_dir(uid: u32) -> PathBuf {
    let base = Path::new("/var/tmp");
    let base = if base.is_dir() {
        base
    } else {
        Path::new("/tmp")
    };
    base.join(format!("fresco-{uid}"))
}

/// Every directory a frame may be in.
fn frame_dirs(uid: u32) -> Vec<PathBuf> {
    vec![cache_frame_dir(), shared_frame_dir(uid)]
}

/// The first directory above `path` (closest first) that other users cannot
/// search (`o+x` clear). A file below one is invisible to Deepin's blur
/// service (`deepin-daemon`) and greeter (`lightdm`). Only existing ancestors
/// count.
fn first_unsearchable_ancestor(path: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    path.ancestors()
        .skip(1)
        .filter(|a| !a.as_os_str().is_empty())
        .find(|a| {
            std::fs::metadata(a).is_ok_and(|m| m.is_dir() && m.permissions().mode() & 0o001 == 0)
        })
        .map(Path::to_path_buf)
}

/// A `0755` directory `dir` owned by `uid`, created if missing — or `None`
/// when what is there cannot be trusted. `/var/tmp` is shared and the name is
/// predictable, so an existing entry is accepted only if it is a real directory
/// (not a symlink), ours, and not writable by anyone else; otherwise another
/// user could have planted it to read or swap our frames.
fn secure_shared_dir(dir: &Path, uid: u32) -> Option<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    match std::fs::DirBuilder::new().mode(0o755).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return None,
    }
    let md = std::fs::symlink_metadata(dir).ok()?;
    let trusted = md.is_dir() && md.uid() == uid && md.mode() & 0o022 == 0;
    if !trusted {
        return None;
    }
    if md.mode() & 0o755 != 0o755 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).ok()?;
    }
    Some(dir.to_path_buf())
}

/// Choose between the user's cache directory and the shared fallback.
fn choose_frame_dir(cache: PathBuf, cache_searchable: bool, shared: Option<PathBuf>) -> PathBuf {
    if cache_searchable {
        cache
    } else {
        shared.unwrap_or(cache)
    }
}

/// The directory to write this frame into: `~/.cache/fresco` when Deepin's
/// services can reach it, else `/var/tmp/fresco-<uid>`. Logged when it is the
/// fallback, since that is the case a bug report needs to know about.
fn pick_frame_dir(uid: u32) -> PathBuf {
    let cache = cache_frame_dir();
    std::fs::create_dir_all(&cache).ok();
    // Probe a file inside it so the directory's own mode is part of the check.
    let probe = cache.join(format!("{FRAME_PREFIX}probe.png"));
    let searchable = first_unsearchable_ancestor(&probe).is_none();
    let shared = if searchable {
        None
    } else {
        secure_shared_dir(&shared_frame_dir(uid), uid)
    };
    let dir = choose_frame_dir(cache.clone(), searchable, shared);
    if dir != cache {
        log::info!(
            "DDE lock screen: {} is not searchable by other users; keeping frames in {}",
            cache.display(),
            dir.display()
        );
    }
    dir
}

/// Bake the lock screen's `blur` radius and `dim` (as `lockscreen::resolve`
/// gives them) into the still at `path`, in place. A no-op at 0 and 0, so the
/// frame is then exactly the plain still. If the picture cannot be read or
/// redrawn the plain still stays: an ungraded lock background beats the
/// default one.
fn grade_still(path: &Path, blur: f32, dim: f32) {
    if blur <= 0.0 && dim <= 0.0 {
        return;
    }
    let graded = image::open(path).ok().and_then(|img| {
        let img = img.to_rgba8();
        let size = crate::widgetkit::geom::Size::new(img.width() as f32, img.height() as f32);
        crate::widgetkit::lockscene::compose_backdrop(&img, size, blur, dim)
    });
    let written = graded.is_some_and(|bgra| {
        let rgba = super::lock::engine::bgra_to_rgba_image(&bgra);
        super::lock::engine::write_png_atomic(path, &rgba).is_ok()
    });
    if !written {
        log::warn!("DDE lock screen: could not apply dim/blur to the frame; using it plain");
    }
}

/// Render a still of the wallpaper (the global one, else the first per-monitor
/// one — the greeter background is a single picture per user), with the user's
/// lock-screen dim applied (not the blur: Deepin processes the picture itself,
/// see the module doc), into `dir` under a fresh name, readable by everyone.
fn render_frame(config: &Config, dir: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let rendered = super::overview::render_still(config.lock_source(None)).or_else(|| {
        config
            .monitors
            .values()
            .find_map(super::overview::render_still)
    })?;
    if let Some(lock) = &config.lockscreen {
        let resolved = crate::lockscreen::resolve(lock);
        grade_still(&rendered, 0.0, resolved.dim);
    }
    std::fs::create_dir_all(dir).ok()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = dir.join(format!("{FRAME_PREFIX}{stamp}.png"));
    // `render_still` writes into ~/.cache/fresco; `dir` may be another
    // filesystem, where `rename` fails and a copy is needed.
    if std::fs::rename(&rendered, &dest).is_err() {
        std::fs::copy(&rendered, &dest).ok()?;
        std::fs::remove_file(&rendered).ok();
    }
    // Deepin's services read this as other users; a restrictive umask must not
    // make the frame private.
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o644)).ok()?;
    Some(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::os::unix::fs::PermissionsExt;

    const NO_WAIT: Patience = Patience {
        tries: 2,
        step: Duration::ZERO,
    };

    fn tempdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fresco-dde-lock-test-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A fake Deepin: holds the greeter background, and can be told to refuse
    /// requests or to take them without ever applying them. It has no
    /// wallpaper store until `stores` is switched on.
    struct Fake {
        value: RefCell<Option<String>>,
        takes_requests: Cell<bool>,
        applies: Cell<bool>,
        requests: RefCell<Vec<String>>,
        stores: Cell<bool>,
        forgotten: RefCell<Vec<String>>,
        blur: Cell<Option<bool>>,
    }

    impl Fake {
        fn showing(value: Option<&str>) -> Fake {
            Fake {
                value: RefCell::new(value.map(str::to_string)),
                takes_requests: Cell::new(true),
                applies: Cell::new(true),
                requests: RefCell::new(Vec::new()),
                stores: Cell::new(false),
                forgotten: RefCell::new(Vec::new()),
                blur: Cell::new(Some(true)),
            }
        }
        fn now(&self) -> Option<String> {
            self.value.borrow().clone()
        }
    }

    impl Greeter for Fake {
        fn current(&self) -> Option<String> {
            self.now()
        }
        fn set(&self, uri: &str) -> bool {
            self.requests.borrow_mut().push(uri.to_string());
            if !self.takes_requests.get() {
                return false;
            }
            if self.applies.get() {
                // Appearance1 stores `file://` + the decoded path.
                let raw = uri.strip_prefix("file://").unwrap_or(uri);
                *self.value.borrow_mut() = Some(format!("file://{}", percent_decode(raw)));
            }
            true
        }
        fn store(&self, frame: &Path) -> Option<String> {
            // The helper names a copy after its content; the fake after the frame.
            if !self.stores.get() {
                return None;
            }
            let name = frame.file_name()?.to_str()?.replace(FRAME_PREFIX, "copy-");
            Some(format!("{STORE}/{name}"))
        }
        fn forget(&self, copy: &str) {
            self.forgotten.borrow_mut().push(copy.to_string());
        }
        fn blur_reads(&self, _picture: &Path) -> Option<bool> {
            self.blur.get()
        }
    }

    /// Where the fake store keeps its copies.
    const STORE: &str = "/var/cache/wallpapers/custom-solidwallpapers/u";

    const ORIGINAL: &str = "file:///usr/share/backgrounds/default_lock_background.jpg";

    /// A frame file in `dir`, as `render_frame` leaves one.
    fn frame_in(dir: &Path, stamp: u32) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let p = dir.join(format!("{FRAME_PREFIX}{stamp}.png"));
        std::fs::write(&p, b"png").unwrap();
        p
    }

    // -- dim and blur ------------------------------------------------------------

    #[test]
    fn grading_bakes_in_dim_and_blur_and_leaves_a_plain_frame_untouched() {
        let dir = tempdir("grade");
        let path = dir.join("frame.png");
        let save = |img: &image::RgbaImage| img.save(&path).unwrap();
        let open = || image::open(&path).unwrap().to_rgba8();

        let grey = image::RgbaImage::from_pixel(64, 36, image::Rgba([200, 200, 200, 255]));
        save(&grey);
        let before = std::fs::read(&path).unwrap();
        grade_still(&path, 0.0, 0.0);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "0/0 must not rewrite"
        );

        grade_still(&path, 0.0, 0.5);
        let out = open();
        assert_eq!(out.dimensions(), (64, 36));
        let v = out.get_pixel(32, 18).0[0];
        assert!((96..=104).contains(&v), "half dim of 200 gave {v}");

        // A hard black|white edge: blur softens the seam, dim is not involved.
        let edge = image::RgbaImage::from_fn(64, 36, |x, _| {
            let c = if x < 32 { 0 } else { 255 };
            image::Rgba([c, c, c, 255])
        });
        save(&edge);
        grade_still(&path, 0.1, 0.0);
        let seam = open().get_pixel(32, 18).0[0];
        assert!((40..=215).contains(&seam), "seam still hard: {seam}");
    }

    // -- decisions -------------------------------------------------------------

    #[test]
    fn acts_only_on_deepin_and_only_with_the_lock_feature_on() {
        assert_eq!(decide(false, true), Action::Nothing);
        assert_eq!(decide(false, false), Action::Nothing);
        assert_eq!(decide(true, true), Action::Sync);
        // Feature off on Deepin: give back whatever a previous run left.
        assert_eq!(decide(true, false), Action::Restore);
    }

    #[test]
    fn accounts_object_path_carries_the_uid() {
        assert_eq!(
            accounts_user_path(ACCOUNTS[0].1, 1000),
            "/org/deepin/dde/Accounts1/User1000"
        );
        assert_eq!(
            accounts_user_path(ACCOUNTS[1].1, 0),
            "/com/deepin/daemon/Accounts/User0"
        );
    }

    #[test]
    fn percent_decoding_keeps_what_is_not_an_escape() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("%E6%9F%92%E7%8E%96"), "柒玖");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz%4"), "%zz%4");
        assert_eq!(percent_decode(""), "");
    }

    #[test]
    fn uri_and_path_spellings_name_the_same_picture() {
        assert_eq!(
            uri_to_path("file:///home/u/.cache/a%20b.png"),
            Some(PathBuf::from("/home/u/.cache/a b.png"))
        );
        assert_eq!(
            uri_to_path(" /home/u/a.png "),
            Some(PathBuf::from("/home/u/a.png"))
        );
        assert_eq!(uri_to_path(""), None);
        assert_eq!(uri_to_path("file://"), None);
        assert_eq!(uri_to_path("relative/a.png"), None);
        assert!(same_picture("file:///a%20b.png", "file:///a b.png"));
        assert!(same_picture("/a.png", "file:///a.png"));
        assert!(!same_picture("file:///a.png", "file:///b.png"));
        assert!(!same_picture("", ""));
    }

    #[test]
    fn our_frames_are_recognised_by_directory_prefix_and_extension() {
        let dirs = vec![PathBuf::from("/home/u/.cache/fresco")];
        assert!(is_our_frame(
            "file:///home/u/.cache/fresco/dde-lock-123.png",
            &dirs
        ));
        assert!(is_our_frame(
            "/home/u/.cache/fresco/dde-lock-123.png",
            &dirs
        ));
        // Same name, wrong place; right place, wrong name or type.
        assert!(!is_our_frame("file:///tmp/dde-lock-123.png", &dirs));
        assert!(!is_our_frame(
            "file:///home/u/.cache/fresco/overview-123.png",
            &dirs
        ));
        assert!(!is_our_frame(
            "file:///home/u/.cache/fresco/dde-lock-123.jpg",
            &dirs
        ));
        assert!(!is_our_frame(ORIGINAL, &dirs));
        // Not directly inside.
        assert!(!is_our_frame(
            "file:///home/u/.cache/fresco/sub/dde-lock-1.png",
            &dirs
        ));
    }

    #[test]
    fn save_step_never_overwrites_and_never_records_our_own_frame() {
        let dirs = vec![PathBuf::from("/c")];
        // An existing state file always wins, whatever is showing.
        assert_eq!(
            save_step(Some("file:///c/dde-lock-1.png"), true, &dirs),
            SaveStep::AlreadySaved
        );
        assert_eq!(save_step(None, true, &dirs), SaveStep::AlreadySaved);
        // First time: record the user's own picture.
        assert_eq!(
            save_step(Some(ORIGINAL), false, &dirs),
            SaveStep::Save(ORIGINAL.to_string())
        );
        // State lost while our frame is up: nothing genuine to record.
        assert_eq!(
            save_step(Some("file:///c/dde-lock-1.png"), false, &dirs),
            SaveStep::OwnFrameNoState
        );
        // Cannot read it: cannot give it back, so do not touch it.
        assert_eq!(save_step(None, false, &dirs), SaveStep::Unreadable);
    }

    #[test]
    fn restore_step_only_overwrites_our_own_frame() {
        let dirs = vec![PathBuf::from("/c")];
        assert_eq!(
            restore_step(ORIGINAL, Some("file:///c/dde-lock-9.png"), &dirs, &[]),
            RestoreStep::Write(ORIGINAL.to_string())
        );
        // A copy in Deepin's wallpaper store counts, as the state lists it.
        let copy = format!("{STORE}/copy-9.png");
        assert_eq!(
            restore_step(
                ORIGINAL,
                Some(&format!("file://{copy}")),
                &dirs,
                std::slice::from_ref(&copy)
            ),
            RestoreStep::Write(ORIGINAL.to_string())
        );
        // The same directory is not enough: another solid picture is the user's.
        assert_eq!(
            restore_step(
                ORIGINAL,
                Some(&format!("file://{STORE}/theirs.png")),
                &dirs,
                &[copy]
            ),
            RestoreStep::Discard
        );
        // The user picked something else meanwhile: theirs wins.
        assert_eq!(
            restore_step(ORIGINAL, Some("file:///home/u/mine.jpg"), &dirs, &[]),
            RestoreStep::Discard
        );
        // Already the original (our set never took).
        assert_eq!(
            restore_step(ORIGINAL, Some(ORIGINAL), &dirs, &[]),
            RestoreStep::Discard
        );
        assert_eq!(restore_step(ORIGINAL, None, &dirs, &[]), RestoreStep::Keep);
    }

    // -- the flows ----------------------------------------------------------------

    #[test]
    fn first_sync_saves_the_original_and_shows_the_frame() {
        let root = tempdir("first");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));

        assert!(matches!(
            sync_in(&g, &state, &dirs, &frame, NO_WAIT),
            Synced::Applied { .. }
        ));

        assert!(same_picture(&g.now().unwrap(), &frame.to_string_lossy()));
        let saved: Saved = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
        assert_eq!(saved.uri, ORIGINAL);
        assert!(frame.exists());
    }

    #[test]
    fn later_syncs_keep_the_first_original_and_sweep_the_old_frame() {
        let root = tempdir("again");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let g = Fake::showing(Some(ORIGINAL));
        let first = frame_in(&cache, 1);
        sync_in(&g, &state, &dirs, &first, NO_WAIT);

        let second = frame_in(&cache, 2);
        assert!(matches!(
            sync_in(&g, &state, &dirs, &second, NO_WAIT),
            Synced::Applied { .. }
        ));

        // The state still names the user's picture, not our first frame.
        let saved: Saved = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
        assert_eq!(saved.uri, ORIGINAL);
        assert!(!first.exists(), "the superseded frame is swept");
        assert!(second.exists());
    }

    #[test]
    fn an_unreadable_background_is_left_completely_alone() {
        let root = tempdir("unreadable");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(None);

        assert_eq!(
            sync_in(&g, &state, &[cache], &frame, NO_WAIT),
            Synced::Unreadable
        );
        assert!(g.requests.borrow().is_empty(), "no request may be sent");
        assert!(!state.exists());
        assert!(!frame.exists(), "an unused frame is not left behind");
    }

    #[test]
    fn a_refused_request_leaves_the_original_in_place_and_restorable_as_a_no_op() {
        let root = tempdir("refused");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));
        g.takes_requests.set(false);

        assert_eq!(
            sync_in(&g, &state, &dirs, &frame, NO_WAIT),
            Synced::NotRequested
        );
        assert_eq!(g.now().as_deref(), Some(ORIGINAL));
        assert!(!frame.exists());
        // The state written on the way is harmless: nothing of ours is showing.
        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Discarded);
        assert!(!state.exists());
        assert_eq!(g.now().as_deref(), Some(ORIGINAL));
    }

    #[test]
    fn a_request_deepin_never_applies_is_reported_and_keeps_its_file() {
        let root = tempdir("never");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));
        g.applies.set(false);

        assert_eq!(
            sync_in(&g, &state, &[cache], &frame, NO_WAIT),
            Synced::NotAccepted
        );
        assert!(frame.exists(), "the request may still land");
    }

    #[test]
    fn an_unwritable_state_directory_means_no_change_at_all() {
        let root = tempdir("nostate");
        let cache = root.join("cache");
        let frame = frame_in(&cache, 1);
        // The state path's parent is a regular file, so it can never be created.
        let blocker = root.join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        let g = Fake::showing(Some(ORIGINAL));

        assert_eq!(
            sync_in(&g, &blocker.join("state.json"), &[cache], &frame, NO_WAIT),
            Synced::StateNotSaved
        );
        assert!(g.requests.borrow().is_empty());
    }

    #[test]
    fn restore_puts_the_original_back_and_clears_everything() {
        let root = tempdir("restore");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));
        sync_in(&g, &state, &dirs, &frame, NO_WAIT);

        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Done);

        assert_eq!(g.now().as_deref(), Some(ORIGINAL));
        assert!(!state.exists());
        assert!(!frame.exists());
        // And it is idempotent.
        assert_eq!(
            restore_in(&g, &state, &dirs, NO_WAIT),
            Restored::NothingSaved
        );
    }

    #[test]
    fn restore_after_a_crash_recovers_the_original_not_our_frame() {
        // A previous run installed a frame and died: the state file holds the
        // original, DDE shows the frame. A new run syncs again (new frame),
        // then stops.
        let root = tempdir("crash");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let old = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));
        sync_in(&g, &state, &dirs, &old, NO_WAIT);
        // -- crash: the Fake and the files persist, the process does not --

        let new = frame_in(&cache, 2);
        assert!(matches!(
            sync_in(&g, &state, &dirs, &new, NO_WAIT),
            Synced::Applied { .. }
        ));
        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Done);
        assert_eq!(g.now().as_deref(), Some(ORIGINAL));
    }

    #[test]
    fn restore_at_startup_with_the_feature_off_recovers_a_crashed_run() {
        let root = tempdir("startup");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));
        sync_in(&g, &state, &dirs, &frame, NO_WAIT);

        // A fresh process: only the disk and DDE remember anything.
        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Done);
        assert_eq!(g.now().as_deref(), Some(ORIGINAL));
    }

    #[test]
    fn restore_does_not_clobber_a_picture_the_user_chose_meanwhile() {
        let root = tempdir("userchose");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));
        sync_in(&g, &state, &dirs, &frame, NO_WAIT);
        *g.value.borrow_mut() = Some("file:///home/u/mine.jpg".to_string());
        g.requests.borrow_mut().clear();

        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Discarded);

        assert!(g.requests.borrow().is_empty(), "nothing may be written");
        assert_eq!(g.now().as_deref(), Some("file:///home/u/mine.jpg"));
        assert!(!state.exists());
        assert!(!frame.exists());
    }

    #[test]
    fn restore_keeps_the_state_when_deepin_cannot_be_asked_or_refuses() {
        let root = tempdir("pending");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));
        sync_in(&g, &state, &dirs, &frame, NO_WAIT);

        // Cannot read the current value.
        let shown = g.now();
        *g.value.borrow_mut() = None;
        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Pending);
        assert!(state.exists());

        // Can read it, but the original is refused.
        *g.value.borrow_mut() = shown;
        g.takes_requests.set(false);
        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Pending);
        assert!(state.exists());
        assert!(frame.exists(), "still on screen, so still needed");
    }

    #[test]
    fn a_corrupt_state_file_is_dropped_not_obeyed() {
        let root = tempdir("corrupt");
        let state = root.join("state.json");
        let g = Fake::showing(Some("file:///c/dde-lock-1.png"));
        for junk in ["", "not json", r#"{"uri":""}"#, r#"{"nope":1}"#] {
            std::fs::write(&state, junk).unwrap();
            assert_eq!(
                restore_in(&g, &state, &[PathBuf::from("/c")], NO_WAIT),
                Restored::Corrupt,
                "{junk:?}"
            );
            assert!(!state.exists());
        }
        assert!(g.requests.borrow().is_empty());
    }

    #[test]
    fn a_lost_state_file_with_our_frame_showing_still_updates_the_frame() {
        let root = tempdir("lost");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let old = frame_in(&cache, 1);
        let g = Fake::showing(Some(&format!("file://{}", old.display())));
        let new = frame_in(&cache, 2);

        assert!(matches!(
            sync_in(&g, &state, &dirs, &new, NO_WAIT),
            Synced::Applied { .. }
        ));

        // Never records our own frame as the "original".
        assert!(!state.exists());
        assert!(same_picture(&g.now().unwrap(), &new.to_string_lossy()));
    }

    #[test]
    fn frames_with_awkward_names_round_trip_through_deepins_decoding() {
        let root = tempdir("名 前");
        let (state, cache) = (root.join("state.json"), root.join("c a'che"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));

        assert!(matches!(
            sync_in(&g, &state, &dirs, &frame, NO_WAIT),
            Synced::Applied { .. }
        ));
        assert!(is_our_frame(&g.now().unwrap(), &dirs));
    }

    // -- where frames live ----------------------------------------------------------

    #[test]
    fn the_cache_dir_is_used_when_searchable_else_the_shared_one() {
        let cache = PathBuf::from("/home/u/.cache/fresco");
        let shared = PathBuf::from("/var/tmp/fresco-1000");
        assert_eq!(
            choose_frame_dir(cache.clone(), true, Some(shared.clone())),
            cache
        );
        assert_eq!(
            choose_frame_dir(cache.clone(), false, Some(shared.clone())),
            shared
        );
        // No trustworthy shared dir: the cache dir is still the best there is.
        assert_eq!(choose_frame_dir(cache.clone(), false, None), cache);
    }

    #[test]
    fn a_private_ancestor_is_found_and_a_searchable_chain_is_not() {
        let root = tempdir("perm");
        let inner = root.join("a").join("b");
        std::fs::create_dir_all(&inner).unwrap();
        let file = inner.join("x.png");
        for d in [&root, &root.join("a"), &inner] {
            std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // Nothing inside the test tree blocks (what is above it is the
        // machine's own business).
        match first_unsearchable_ancestor(&file) {
            None => {}
            Some(p) => assert!(!p.starts_with(&root), "{p:?}"),
        }

        std::fs::set_permissions(root.join("a"), std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(first_unsearchable_ancestor(&file), Some(root.join("a")));
        // Restore so the temp tree can be inspected/cleaned by the user.
        std::fs::set_permissions(root.join("a"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn the_shared_dir_is_created_0755_and_reused() {
        let uid = crate::userinfo::current_uid().unwrap();
        let dir = tempdir("shared").join(format!("fresco-{uid}"));

        assert_eq!(secure_shared_dir(&dir, uid), Some(dir.clone()));
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
        // A second call accepts what it created.
        assert_eq!(secure_shared_dir(&dir, uid), Some(dir));
    }

    #[test]
    fn the_shared_dir_is_refused_when_it_cannot_be_trusted() {
        let uid = crate::userinfo::current_uid().unwrap();
        let root = tempdir("untrusted");

        // A symlink planted at the predictable name.
        let target = root.join("elsewhere");
        std::fs::create_dir_all(&target).unwrap();
        let link = root.join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(secure_shared_dir(&link, uid), None);

        // A directory writable by others.
        let open = root.join("open");
        std::fs::create_dir_all(&open).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(secure_shared_dir(&open, uid), None);

        // A plain file in the way.
        let file = root.join("file");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(secure_shared_dir(&file, uid), None);

        // Owned by someone else (as far as this call can tell).
        let mine = root.join("mine");
        std::fs::create_dir_all(&mine).unwrap();
        assert_eq!(secure_shared_dir(&mine, uid.wrapping_add(1)), None);
    }

    #[test]
    fn a_mode_stripped_shared_dir_is_repaired() {
        let uid = crate::userinfo::current_uid().unwrap();
        let dir = tempdir("repair");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(secure_shared_dir(&dir, uid), Some(dir.clone()));
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[test]
    fn sweeping_removes_only_our_frames_and_honours_keep() {
        let root = tempdir("sweep");
        let a = frame_in(&root, 1);
        let b = frame_in(&root, 2);
        let other = root.join("overview-1.png");
        std::fs::write(&other, b"x").unwrap();
        // An unrelated directory in the list is tolerated.
        let dirs = vec![root.clone(), root.join("missing")];

        remove_frames(&dirs, Some(&b));
        assert!(!a.exists() && b.exists() && other.exists());

        remove_frames(&dirs, None);
        assert!(!b.exists() && other.exists());
    }

    // -- the copy in Deepin's wallpaper store -------------------------------------

    /// `Introspect` as Deepin's Go daemon writes it (the `h` form), and the
    /// pre-2026-06-30 `s` form, each with an unrelated method around it.
    const XML_FD: &str = r#"<node><interface name="org.deepin.dde.Daemon1">
<method name="DeleteCustomWallPaper"><arg name="username" type="s" direction="in"></arg><arg name="file" type="s" direction="in"></arg></method>
<method name="SaveCustomWallPaper">
  <arg name="username" type="s" direction="in"></arg>
  <arg name="fd" type="h" direction="in"></arg>
  <arg name="wallpaperType" type="s" direction="in"></arg>
  <arg name="result" type="s" direction="out"></arg>
</method>
</interface></node>"#;
    const XML_PATH: &str = r#"<node><method name="SaveCustomWallPaper"><arg name="username" type="s" direction="in"></arg><arg name="file" type="s" direction="in"></arg><arg type="s" direction="out"></arg></method></node>"#;

    fn types(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_helper_signature_is_read_from_its_introspection() {
        assert_eq!(
            introspected_inputs(XML_FD, "SaveCustomWallPaper"),
            Some(types(&["s", "h", "s"]))
        );
        assert_eq!(
            introspected_inputs(XML_FD, "DeleteCustomWallPaper"),
            Some(types(&["s", "s"]))
        );
        assert_eq!(
            introspected_inputs(XML_PATH, "SaveCustomWallPaper"),
            Some(types(&["s", "s"]))
        );
        // Self-closing tags, `direction` first: how Qt services and the bus
        // daemon itself write it.
        assert_eq!(
            introspected_inputs(
                r#"<node><method name="M"><arg direction="in" type="s"/><arg direction="out" type="u"/><arg direction="in" type="h"/></method></node>"#,
                "M"
            ),
            Some(types(&["s", "h"]))
        );
        assert_eq!(introspected_inputs(XML_PATH, "Nope"), None);
        assert_eq!(introspected_inputs("", "SaveCustomWallPaper"), None);
        // Through gdbus's `('…',)` quoting, newlines escaped as it prints them.
        let quoted = format!("('{}',)", XML_FD.replace('\n', "\\n"));
        let xml = dde::parse_first_string(&quoted).unwrap();
        assert_eq!(
            introspected_inputs(&xml, "SaveCustomWallPaper"),
            Some(types(&["s", "h", "s"]))
        );
    }

    #[test]
    fn the_save_call_matches_the_helper_signature() {
        let frame = Path::new("/home/u/.cache/fresco/dde-lock-1.png");
        assert_eq!(
            save_args(&types(&["s", "h", "s"]), "u", frame),
            Some(SaveCall {
                args: vec!["'u'".into(), "@h 0".into(), "'solid'".into()],
                send_fd: true,
            })
        );
        assert_eq!(
            save_args(&types(&["s", "s"]), "u", frame),
            Some(SaveCall {
                args: vec![
                    "'u'".into(),
                    "'solid::/home/u/.cache/fresco/dde-lock-1.png'".into()
                ],
                send_fd: false,
            })
        );
        // A helper this module cannot call is not guessed at.
        assert_eq!(save_args(&types(&["s", "h"]), "u", frame), None);
        assert_eq!(save_args(&types(&["s", "s", "s"]), "u", frame), None);
        assert_eq!(save_args(&[], "u", frame), None);
    }

    fn copy_of(stamp: u32) -> String {
        format!("{STORE}/copy-{stamp}.png")
    }

    #[test]
    fn a_copy_in_the_store_is_what_the_greeter_names() {
        let root = tempdir("copy");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));
        g.stores.set(true);

        assert_eq!(
            sync_in(&g, &state, &dirs, &frame, NO_WAIT),
            Synced::Applied {
                installed: PathBuf::from(copy_of(1)),
                blur_reads: Some(true),
            }
        );

        assert!(same_picture(&g.now().unwrap(), &copy_of(1)));
        assert!(!frame.exists(), "the local frame is only the source");
        let saved: Saved = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
        assert_eq!(saved.uri, ORIGINAL);
        assert_eq!(saved.copies, vec![copy_of(1)]);
    }

    #[test]
    fn a_new_copy_replaces_the_old_one_but_never_deletes_itself() {
        let root = tempdir("slot");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let g = Fake::showing(Some(ORIGINAL));
        g.stores.set(true);
        sync_in(&g, &state, &dirs, &frame_in(&cache, 1), NO_WAIT);

        sync_in(&g, &state, &dirs, &frame_in(&cache, 2), NO_WAIT);
        assert_eq!(*g.forgotten.borrow(), vec![copy_of(1)]);
        assert!(same_picture(&g.now().unwrap(), &copy_of(2)));
        assert_eq!(tracked_copies(&state), vec![copy_of(2)]);

        // Same pixels again: the helper hands back the same file, which is
        // what is showing and must stay.
        g.forgotten.borrow_mut().clear();
        sync_in(&g, &state, &dirs, &frame_in(&cache, 2), NO_WAIT);
        assert!(g.forgotten.borrow().is_empty());
        assert_eq!(tracked_copies(&state), vec![copy_of(2)]);
    }

    #[test]
    fn without_a_store_the_frame_is_installed_in_place_as_before() {
        let root = tempdir("nostore");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let frame = frame_in(&cache, 1);
        let g = Fake::showing(Some(ORIGINAL));

        assert_eq!(
            sync_in(&g, &state, &dirs, &frame, NO_WAIT),
            Synced::Applied {
                installed: frame.clone(),
                blur_reads: Some(true),
            }
        );
        assert!(frame.exists());
        assert!(tracked_copies(&state).is_empty());
        assert!(g.forgotten.borrow().is_empty());
    }

    #[test]
    fn falling_back_to_the_frame_drops_copies_an_earlier_run_left() {
        let root = tempdir("fallback");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let g = Fake::showing(Some(ORIGINAL));
        g.stores.set(true);
        sync_in(&g, &state, &dirs, &frame_in(&cache, 1), NO_WAIT);

        g.stores.set(false);
        sync_in(&g, &state, &dirs, &frame_in(&cache, 2), NO_WAIT);

        assert_eq!(*g.forgotten.borrow(), vec![copy_of(1)]);
        assert!(tracked_copies(&state).is_empty());
    }

    #[test]
    fn an_unreadable_picture_is_reported_with_the_service_that_cannot_read_it() {
        let root = tempdir("blur");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let g = Fake::showing(Some(ORIGINAL));
        g.blur.set(Some(false));

        let synced = sync_in(
            &g,
            &state,
            std::slice::from_ref(&cache),
            &frame_in(&cache, 1),
            NO_WAIT,
        );

        assert!(matches!(
            synced,
            Synced::Applied {
                blur_reads: Some(false),
                ..
            }
        ));
    }

    #[test]
    fn a_refused_request_deletes_a_fresh_copy_but_not_one_already_showing() {
        let root = tempdir("refusedcopy");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let g = Fake::showing(Some(ORIGINAL));
        g.stores.set(true);
        sync_in(&g, &state, &dirs, &frame_in(&cache, 1), NO_WAIT);
        g.takes_requests.set(false);

        // A different picture: its copy is useless once refused.
        assert_eq!(
            sync_in(&g, &state, &dirs, &frame_in(&cache, 2), NO_WAIT),
            Synced::NotRequested
        );
        assert_eq!(*g.forgotten.borrow(), vec![copy_of(2)]);

        // The picture that is showing: leave its copy alone.
        g.forgotten.borrow_mut().clear();
        assert_eq!(
            sync_in(&g, &state, &dirs, &frame_in(&cache, 1), NO_WAIT),
            Synced::NotRequested
        );
        assert!(g.forgotten.borrow().is_empty());
    }

    #[test]
    fn restore_gives_the_original_back_and_deletes_the_copy() {
        let root = tempdir("restorecopy");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let g = Fake::showing(Some(ORIGINAL));
        g.stores.set(true);
        sync_in(&g, &state, &dirs, &frame_in(&cache, 1), NO_WAIT);

        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Done);

        assert_eq!(g.now().as_deref(), Some(ORIGINAL));
        assert_eq!(*g.forgotten.borrow(), vec![copy_of(1)]);
        assert!(!state.exists());
    }

    #[test]
    fn restore_keeps_the_copy_while_it_is_still_on_screen() {
        let root = tempdir("keepcopy");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let g = Fake::showing(Some(ORIGINAL));
        g.stores.set(true);
        sync_in(&g, &state, &dirs, &frame_in(&cache, 1), NO_WAIT);
        g.takes_requests.set(false);

        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Pending);

        assert!(g.forgotten.borrow().is_empty());
        assert_eq!(tracked_copies(&state), vec![copy_of(1)]);
    }

    #[test]
    fn restore_deletes_the_copy_even_when_the_user_chose_another_picture() {
        let root = tempdir("userchosecopy");
        let (state, cache) = (root.join("state.json"), root.join("cache"));
        let dirs = vec![cache.clone()];
        let g = Fake::showing(Some(ORIGINAL));
        g.stores.set(true);
        sync_in(&g, &state, &dirs, &frame_in(&cache, 1), NO_WAIT);
        *g.value.borrow_mut() = Some("file:///home/u/mine.jpg".to_string());

        assert_eq!(restore_in(&g, &state, &dirs, NO_WAIT), Restored::Discarded);

        assert_eq!(g.now().as_deref(), Some("file:///home/u/mine.jpg"));
        assert_eq!(*g.forgotten.borrow(), vec![copy_of(1)]);
    }

    #[test]
    fn a_state_file_from_before_copies_still_loads() {
        let root = tempdir("oldstate");
        let state = root.join("state.json");
        std::fs::write(&state, format!(r#"{{"uri":"{ORIGINAL}"}}"#)).unwrap();
        assert!(matches!(
            load_saved(&state),
            Load::Valid(Saved { copies, .. }) if copies.is_empty()
        ));

        // And an empty list is not written back out.
        assert!(write_saved(
            &state,
            &Saved {
                uri: ORIGINAL.to_string(),
                copies: Vec::new()
            }
        ));
        assert!(!std::fs::read_to_string(&state).unwrap().contains("copies"));
    }
}

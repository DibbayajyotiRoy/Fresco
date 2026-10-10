//! KDE Plasma 6 lock host — installs/configures/removes the Plasma wallpaper
//! plugin (`packaging/kde/`) that draws Fresco's wallpaper and widgets on the
//! real KDE lock screen.
//!
//! `lock()`/`targets_while_locked` were already real before this file: KDE's
//! own `kscreenlocker` answers `loginctl lock-session` like any other
//! systemd-logind session, and the widget engine always paints into
//! `$XDG_RUNTIME_DIR/fresco/lock` regardless of whether the plugin reading it
//! is actually selected. What this file adds is everything gated on that
//! selection actually happening: [`KdeHost::setup`] installs the plugin
//! package if needed and points `kscreenlockerrc` at it; [`KdeHost::undo`]
//! restores exactly what was there before; [`refresh_config`] updates the
//! four keys that can change between locks (video/still path, play-video
//! policy, dim) without touching anything else.
//!
//! Every `kscreenlockerrc` edit here follows `packaging/kde/README.md`
//! *exactly* — that file is the verified contract (group paths, key names,
//! `kwriteconfig6`/`kreadconfig6` flag behaviour, all checked against
//! `kscreenlocker`'s and `kconfig`'s own source) between this daemon and the
//! plugin package; nothing below invents a key or a group path the README
//! doesn't already specify.
//!
//! # Backup/restore is type-blind on purpose
//!
//! [`setup_with`]/[`undo_with`] never interpret *what kind* of value a key
//! holds (string, bool, double, int) — they only ever read and write plain
//! text. That is safe because `kwriteconfig6`'s plain-string mode
//! (`cfgGroup.writeEntry(key, value)`, confirmed from `kwriteconfig.cpp`,
//! fetched from `invent.kde.org/frameworks/kconfig`) stores exactly the bytes
//! given, and `kreadconfig6`'s own plain-string mode reads them back
//! byte-for-byte (`cfgGroup.readEntry(key, dflt)`, from `kreadconfig.cpp`,
//! same repository) — so writing back whatever text was backed up reproduces
//! the original file exactly, whether that text happens to spell a bool
//! (`"true"`), a double (`"0.2"`), or a plugin id, with no need to know
//! which. `--type bool` only matters when *choosing a new value* to write
//! (see `setup_with`'s own calls) — never for round-tripping one that
//! already exists.
//!
//! `kreadconfig6` has no separate way to report "this key does not exist" —
//! with no `--type`, it just prints `--default`'s value (defaulting to `""`)
//! for both a missing key and one that is genuinely present-but-empty (see
//! `kreadconfig.cpp`: `cfgGroup.readEntry(key, dflt)`, no distinct "not
//! found" case at all). [`ABSENT_SENTINEL`] is how this module tells the two
//! apart.
//!
//! # Crash safety: atomic backup writes, corrupt-backup recovery, and setup rollback
//!
//! [`write_backup`] never truncates the real backup file in place — it writes
//! to a temp file in the same directory, `fsync`s it, then `rename`s over the
//! real path, so a crash mid-write can only ever leave the OLD backup (or
//! nothing, if this is the very first write) at that path, never a
//! half-written one `serde_json` can't parse back. [`setup_with`]
//! additionally treats an existing backup file it cannot parse as no backup
//! at all: it quarantines the corrupt file (renamed aside with a
//! `.corrupt-<timestamp>` suffix, so it survives on disk for forensics) and
//! takes a fresh one, rather than either failing `undo` forever or — far
//! worse — discarding the user's real original values by treating the
//! corrupt file as "already backed up, nothing to do".
//!
//! And because the nine `kscreenlockerrc` writes below can't be applied as
//! one atomic group (`kwriteconfig6` runs once per key, with no transaction
//! across calls), [`setup_with`] orders them so `[Greeter] WallpaperPlugin`
//! — the one key that actually switches KDE's greeter onto this plugin — is
//! written **last**, and rolls every key already written back to its backed-
//! up value the moment any write in the sequence fails, then still returns
//! that original error. A failure partway through can then never leave
//! kscreenlockerrc pointing at this plugin with its own `General`/`LnF` keys
//! half-applied — the greeter would have no sane way to render that.

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{loginctl_lock_session, HostCtx, HostKind, LockHost, LockTargets, RunningHost};
use crate::config::{Config, Kind, Wallpaper};
use crate::ipc::LockSetupState;

/// This plugin's KPackage id — matches the directory name under
/// `packaging/kde/` and `metadata.json`'s own `KPlugin.Id`.
const PLUGIN_ID: &str = "io.github.dibbayajyotiroy.fresco.lockscreen";

const KSCREENLOCKERRC: &str = "kscreenlockerrc";

/// `[Greeter] WallpaperPlugin=`.
const GREETER_GROUP: &[&str] = &["Greeter"];
/// `[Greeter][Wallpaper][<id>][General]`.
const GENERAL_GROUP: &[&str] = &["Greeter", "Wallpaper", PLUGIN_ID, "General"];
/// `[Greeter][LnF][General]`.
const LNF_GROUP: &[&str] = &["Greeter", "LnF", "General"];

/// `contents/config/main.xml`'s own documented default, mirrored here rather
/// than invented: how often the plugin re-checks the widget-layer PNGs.
const DEFAULT_REFRESH_MS: u32 = 1000;

/// A value no real `kscreenlockerrc` entry will ever equal, passed as
/// `kreadconfig6 --default` so the backup step in [`setup_with`] can tell a
/// genuinely absent key (the sentinel comes back unchanged) from one that is
/// present but empty — see the module docs for why `kreadconfig6` itself has
/// no other way to say "not found".
const ABSENT_SENTINEL: &str = "__fresco_kde_lock_absent_sentinel__";

/// One `kscreenlockerrc` entry this host manages: its nested `--group` path
/// and key name. A flat list, not nested match arms, so [`setup_with`]'s
/// backup pass and [`undo_with`]'s restore pass iterate the exact same set
/// and can never drift onto a different key list.
struct ManagedKey {
    groups: &'static [&'static str],
    key: &'static str,
}

const MANAGED_KEYS: &[ManagedKey] = &[
    ManagedKey {
        groups: GREETER_GROUP,
        key: "WallpaperPlugin",
    },
    ManagedKey {
        groups: GENERAL_GROUP,
        key: "VideoPath",
    },
    ManagedKey {
        groups: GENERAL_GROUP,
        key: "StillPath",
    },
    ManagedKey {
        groups: GENERAL_GROUP,
        key: "PlayVideo",
    },
    ManagedKey {
        groups: GENERAL_GROUP,
        key: "Dim",
    },
    ManagedKey {
        groups: GENERAL_GROUP,
        key: "LayerDir",
    },
    ManagedKey {
        groups: GENERAL_GROUP,
        key: "RefreshMs",
    },
    ManagedKey {
        groups: LNF_GROUP,
        key: "alwaysShowClock",
    },
    ManagedKey {
        groups: LNF_GROUP,
        key: "showMediaControls",
    },
];

/// Stable identifier for one [`ManagedKey`], used as the backup file's JSON
/// map key. Not meant to be pretty — just unambiguous and stable across a
/// setup/undo pair.
fn entry_id(entry: &ManagedKey) -> String {
    format!("{}.{}", entry.groups.join("."), entry.key)
}

pub struct KdeHost;

impl LockHost for KdeHost {
    fn kind(&self) -> HostKind {
        HostKind::Kde
    }

    fn lock(&self, _ctx: &HostCtx) -> Result<RunningHost, String> {
        loginctl_lock_session()
    }

    fn targets_while_locked(&self, ctx: &HostCtx) -> LockTargets {
        LockTargets::LayerFiles(ctx.runtime_dir.join("lock"))
    }

    fn setup(&self, ctx: &HostCtx) -> Result<(), String> {
        let paths = KdePaths::real()?;
        setup_with(&ProcessRunner, ctx, &paths)
    }

    fn undo(&self, _ctx: &HostCtx) -> Result<(), String> {
        let paths = KdePaths::real()?;
        undo_with(&ProcessRunner, &paths)
    }

    fn setup_state(&self, _ctx: &HostCtx) -> LockSetupState {
        setup_state_with(&ProcessRunner)
    }

    fn notes(&self, _ctx: &HostCtx) -> Vec<String> {
        if ProcessRunner.available() {
            Vec::new()
        } else {
            vec![crate::t!(
                "Install kconfig (kreadconfig6/kwriteconfig6) to let Fresco set up the KDE lock screen"
            )
            .to_string()]
        }
    }
}

/// Rewrite only the plugin's `VideoPath`/`StillPath`/`PlayVideo`/`Dim` keys —
/// the ones that can legitimately change between locks — leaving
/// `WallpaperPlugin`, `LayerDir`, `RefreshMs` and the `[Greeter][LnF]` keys
/// untouched. A no-op when [`KdeHost::setup_state`] is not
/// [`LockSetupState::Done`]: there is nothing to refresh if the plugin was
/// never selected, and calling this must never be what *turns it on* — that
/// is `setup()`'s job, one explicit click away in the GUI. The integrator
/// wires this to the GUI's own Apply action.
pub fn refresh_config(ctx: &HostCtx) -> Result<(), String> {
    refresh_config_with(&ProcessRunner, ctx)
}

fn refresh_config_with<R: KConfigRunner>(runner: &R, ctx: &HostCtx) -> Result<(), String> {
    if setup_state_with(runner) != LockSetupState::Done {
        return Ok(());
    }
    let resolved = crate::lockscreen::resolve(&ctx.config.lockscreen.clone().unwrap_or_default());
    let live = wants_live(ctx.config);
    let (video_path, still_path) = wallpaper_paths(ctx.config.lock_source(None));
    runner.write(GENERAL_GROUP, "VideoPath", &video_path, false)?;
    runner.write(GENERAL_GROUP, "StillPath", &still_path, false)?;
    runner.write(GENERAL_GROUP, "PlayVideo", bool_str(live), true)?;
    runner.write(GENERAL_GROUP, "Dim", &resolved.dim.to_string(), false)?;
    Ok(())
}

fn setup_with<R: KConfigRunner>(runner: &R, ctx: &HostCtx, paths: &KdePaths) -> Result<(), String> {
    if !runner.available() {
        return Err(UNAVAILABLE_MSG.to_string());
    }
    ensure_plugin_installed(paths)?;

    // Create-if-absent, never overwrite: a `setup()` retried after a partial
    // failure (a write call errored partway through the sequence below) must
    // back up the state from *before Fresco ever touched this file*, not the
    // half-modified state its own previous attempt left behind. But "the
    // file is there" alone isn't enough to call it a valid backup — a crash
    // mid-write (before atomic writes landed here) or outside interference
    // could leave it unparseable, and treating THAT as "already backed up"
    // would silently adopt garbage as the values `undo()` restores.
    if paths.backup_file.exists() {
        match read_backup(&paths.backup_file) {
            Ok(_) => {
                log::info!(
                    "kde lock setup: a backup already exists at {} — reusing it rather than overwriting",
                    paths.backup_file.display()
                );
            }
            Err(e) => {
                let quarantined = quarantine_path(&paths.backup_file);
                log::warn!(
                    "kde lock setup: existing backup at {} is corrupt ({e}) — moving it to {} \
                     and taking a fresh backup so the user's real original values aren't lost",
                    paths.backup_file.display(),
                    quarantined.display()
                );
                std::fs::rename(&paths.backup_file, &quarantined).map_err(|e| {
                    format!(
                        "failed to move corrupt backup {} aside to {}: {e}",
                        paths.backup_file.display(),
                        quarantined.display()
                    )
                })?;
                take_fresh_backup(runner, paths)?;
            }
        }
    } else {
        take_fresh_backup(runner, paths)?;
    }

    let resolved = crate::lockscreen::resolve(&ctx.config.lockscreen.clone().unwrap_or_default());
    let live = wants_live(ctx.config);
    let (video_path, still_path) = wallpaper_paths(ctx.config.lock_source(None));
    let layer_dir = ctx.runtime_dir.join("lock").to_string_lossy().into_owned();

    if let Err(e) = apply_setup_writes(
        runner,
        &video_path,
        &still_path,
        live,
        resolved.dim,
        &layer_dir,
    ) {
        // A backup was guaranteed to exist (and to parse) by the block
        // above, so this should always succeed — but if it somehow doesn't
        // (the file vanished under us, a race with another process), log
        // that separately and still surface the ORIGINAL write failure
        // below rather than masking it with an unrelated rollback error.
        match read_backup(&paths.backup_file) {
            Ok(backup) => {
                if let Err(rollback_err) = restore_from_backup(runner, &backup) {
                    log::error!(
                        "kde lock setup: rollback after a failed write also failed: \
                         {rollback_err} — kscreenlockerrc may be left half-applied; re-run \
                         setup or restore {} by hand",
                        paths.backup_file.display()
                    );
                }
            }
            Err(read_err) => {
                log::error!(
                    "kde lock setup: could not read {} to roll back a failed write ({read_err}) \
                     — kscreenlockerrc may be left half-applied",
                    paths.backup_file.display()
                );
            }
        }
        return Err(e);
    }
    Ok(())
}

/// Snapshot every [`ManagedKey`]'s current value into a fresh [`KdeBackup`]
/// and persist it — the "nothing has backed this up yet" case for
/// [`setup_with`], shared by the no-backup-file-at-all path and the
/// corrupt-backup-quarantined path, since both need exactly the same fresh
/// snapshot of whatever `kscreenlockerrc` holds right now.
fn take_fresh_backup<R: KConfigRunner>(runner: &R, paths: &KdePaths) -> Result<(), String> {
    let mut backup = KdeBackup::default();
    for entry in MANAGED_KEYS {
        let value = runner.read(entry.groups, entry.key, ABSENT_SENTINEL)?;
        let original = (value != ABSENT_SENTINEL).then_some(value);
        backup.entries.insert(entry_id(entry), original);
    }
    write_backup(&paths.backup_file, &backup)
}

/// Where to move a backup file that failed to parse, so [`setup_with`] can
/// take a fresh backup at the normal path without destroying the corrupt
/// one — kept on disk (never overwriting a previous quarantine, in the
/// ordinary case) purely so a person debugging a lost-settings report has
/// something to inspect.
fn quarantine_path(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    path.with_extension(format!("corrupt-{stamp}"))
}

/// The nine `kscreenlockerrc` writes [`setup_with`] applies once a valid
/// backup is safely on disk — `General`/`LnF` keys first, `[Greeter]
/// WallpaperPlugin` **last**. Order matters: `WallpaperPlugin` is the one key
/// [`setup_state_with`] (and KDE's own greeter) reads to decide whether this
/// plugin is selected at all, so writing it last means a failure anywhere in
/// this sequence leaves `kscreenlockerrc` pointing at whatever plugin was
/// selected before `setup()` ran — never at Fresco's plugin with its
/// `General`/`LnF` keys half-written, which the greeter has no sane way to
/// render. [`setup_with`] rolls back everything already written here the
/// moment any one call returns `Err`.
fn apply_setup_writes<R: KConfigRunner>(
    runner: &R,
    video_path: &str,
    still_path: &str,
    live: bool,
    dim: f32,
    layer_dir: &str,
) -> Result<(), String> {
    runner.write(GENERAL_GROUP, "VideoPath", video_path, false)?;
    runner.write(GENERAL_GROUP, "StillPath", still_path, false)?;
    runner.write(GENERAL_GROUP, "PlayVideo", bool_str(live), true)?;
    runner.write(GENERAL_GROUP, "Dim", &dim.to_string(), false)?;
    runner.write(GENERAL_GROUP, "LayerDir", layer_dir, false)?;
    runner.write(
        GENERAL_GROUP,
        "RefreshMs",
        &DEFAULT_REFRESH_MS.to_string(),
        false,
    )?;
    runner.write(LNF_GROUP, "alwaysShowClock", "false", true)?;
    runner.write(LNF_GROUP, "showMediaControls", "false", true)?;
    runner.write(GREETER_GROUP, "WallpaperPlugin", PLUGIN_ID, false)?;
    Ok(())
}

/// Restore every [`ManagedKey`] to the value recorded in `backup` — write
/// back the original text if it had one, delete the key if it did not.
/// Shared by [`undo_with`] (a user-requested "remove Fresco from my lock
/// screen") and by [`setup_with`]'s own failure path (a write partway
/// through [`apply_setup_writes`] failed, and no key it already wrote may
/// survive as a half-applied state). Restoring a key this attempt never
/// actually touched is a harmless no-op — it already holds that exact
/// value — so applying this to every managed key is equivalent to applying
/// it to just the ones written so far, without either caller needing to
/// track which those were.
fn restore_from_backup<R: KConfigRunner>(runner: &R, backup: &KdeBackup) -> Result<(), String> {
    for entry in MANAGED_KEYS {
        match backup.entries.get(&entry_id(entry)) {
            Some(Some(value)) => runner.write(entry.groups, entry.key, value, false)?,
            Some(None) => runner.delete(entry.groups, entry.key)?,
            None => {} // backup predates a key this version added; leave it
        }
    }
    Ok(())
}

fn undo_with<R: KConfigRunner>(runner: &R, paths: &KdePaths) -> Result<(), String> {
    if !runner.available() {
        return Err(UNAVAILABLE_MSG.to_string());
    }
    let backup = read_backup(&paths.backup_file)?;
    restore_from_backup(runner, &backup)?;
    let _ = std::fs::remove_file(&paths.backup_file);
    Ok(())
}

fn setup_state_with<R: KConfigRunner>(runner: &R) -> LockSetupState {
    if !runner.available() {
        return LockSetupState::Unavailable;
    }
    match runner.read(GREETER_GROUP, "WallpaperPlugin", ABSENT_SENTINEL) {
        Ok(v) if v == PLUGIN_ID => LockSetupState::Done,
        Ok(_) => LockSetupState::Needed,
        // A read that fails outright (the binary vanished between the
        // `available()` check and now, the config file is unreadable, …)
        // means the GUI cannot trust what it would show either way — treat
        // it the same as "no setup path", rather than guess.
        Err(_) => LockSetupState::Unavailable,
    }
}

const UNAVAILABLE_MSG: &str =
    "kreadconfig6/kwriteconfig6 not found — install KDE Frameworks' kconfig tools to use Fresco \
     on the KDE lock screen";

/// `true` if `config`'s current wallpaper should play as live video on the
/// KDE lock screen right now — the lock screen's own policy
/// (`config.lockscreen.live_video`), the same one `daemon::saver` applies for
/// the X11 host. Duplicated rather than shared: neither module owns
/// `src/lockscreen.rs`/`src/battery.rs` in this wave, and both call sites are
/// one line wrapping already-public, already-tested functions
/// (`LiveVideo::plays`, `battery::on_battery`), so there is nothing here that
/// could drift without both copies visibly disagreeing with those functions'
/// own contracts.
fn wants_live(config: &Config) -> bool {
    config
        .lockscreen
        .as_ref()
        .map(|l| l.live_video)
        .unwrap_or_default()
        .plays(crate::battery::on_battery())
}

fn bool_str(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

/// `(VideoPath, StillPath)` for `w` — `VideoPath` only for a wallpaper kind
/// QtMultimedia can actually play (`Video`/`Playlist`, via `effective_path`'s
/// "first file" rule for a playlist — this plugin has one `VideoPath` slot,
/// not a list); empty for `Image`/`Slideshow`, where there is no single video
/// to point at. `StillPath` always goes through
/// `crate::daemon::overview::render_still`, which already produces the right
/// answer for every kind (a slideshow's first frame, a video's poster frame,
/// or the image itself) — empty only when that returns `None` (no source
/// file yet).
fn wallpaper_paths(w: &Wallpaper) -> (String, String) {
    let video = matches!(w.kind, Kind::Video | Kind::Playlist)
        .then(|| w.effective_path())
        .flatten()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let still = crate::daemon::overview::render_still(w)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    (video, still)
}

// ---------------------------------------------------------------------------
// Filesystem locations — bundled so tests can redirect every one of them
// under a temp directory and never touch the real ~/.local/share or
// ~/.local/state this feature's own safety rule forbids a test run from
// writing to.
// ---------------------------------------------------------------------------

struct KdePaths {
    system_plugin_dir: PathBuf,
    user_plugin_dir: PathBuf,
    seed_plugin_dir: PathBuf,
    backup_file: PathBuf,
}

impl KdePaths {
    fn real() -> Result<KdePaths, String> {
        let user_data = dirs::data_local_dir()
            .ok_or_else(|| "no XDG data directory available ($HOME unset?)".to_string())?;
        Ok(KdePaths {
            system_plugin_dir: PathBuf::from("/usr/share/plasma/wallpapers").join(PLUGIN_ID),
            user_plugin_dir: user_data.join("plasma/wallpapers").join(PLUGIN_ID),
            // The copy this package's own packaging installs specifically for
            // this self-heal path (`packaging/debian`/`packaging/aur`/
            // `install.sh`) — independent of whether the system-wide Plasma
            // path above was also populated by that same packaging, so this
            // still works if it wasn't (a non-standard install, or a distro
            // packaging path that only ships the seed copy).
            seed_plugin_dir: PathBuf::from("/usr/share/fresco/kde").join(PLUGIN_ID),
            backup_file: state_dir().join("kde-lock-backup.json"),
        })
    }

    #[cfg(test)]
    fn fake_in(root: &Path) -> KdePaths {
        KdePaths {
            system_plugin_dir: root.join("system-plasma").join(PLUGIN_ID),
            user_plugin_dir: root.join("user-plasma").join(PLUGIN_ID),
            seed_plugin_dir: root.join("seed").join(PLUGIN_ID),
            backup_file: root.join("backup.json"),
        }
    }
}

/// Same fallback chain as `daemon::dde`'s own local `state_dir()`.
fn state_dir() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
}

/// Make sure the plugin package is discoverable somewhere Plasma will find
/// it: the system path, the per-user path, or — if neither has it yet — copy
/// it into the per-user path from this package's own seed copy.
fn ensure_plugin_installed(paths: &KdePaths) -> Result<(), String> {
    if paths.system_plugin_dir.join("metadata.json").is_file() {
        return Ok(());
    }
    if paths.user_plugin_dir.join("metadata.json").is_file() {
        return Ok(());
    }
    if !paths.seed_plugin_dir.join("metadata.json").is_file() {
        return Err(format!(
            "Fresco's KDE lock-screen plugin files were not found in any of {}, {}, or {} — \
             reinstall Fresco",
            paths.system_plugin_dir.display(),
            paths.user_plugin_dir.display(),
            paths.seed_plugin_dir.display(),
        ));
    }
    copy_dir_recursive(&paths.seed_plugin_dir, &paths.user_plugin_dir).map_err(|e| {
        format!(
            "failed to install the KDE lock-screen plugin to {}: {e}",
            paths.user_plugin_dir.display()
        )
    })
}

/// Recursively copy `src` to `dst`, creating directories as needed. No
/// symlink handling: the package tree this copies (`packaging/kde/`) is a
/// handful of plain QML/JSON/XML files, never a symlink, and skipping them
/// rather than following or copying-as-link avoids ever escaping `src`.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

/// Write `backup` to `path` atomically. A crash or power loss mid-write must
/// never leave `path` holding truncated/invalid JSON: [`setup_with`] treats
/// "this file exists" as its cue to skip taking a fresh backup, and
/// [`undo_with`] (via [`read_backup`]) needs to parse it to restore anything
/// at all — a half-written file breaks both. Mirrors `Config::save_to`
/// (`src/config.rs`): write the new bytes to a temp file in the *same*
/// directory (so the final rename stays on one filesystem and is therefore
/// atomic), `fsync` it so those bytes are actually on disk before anything
/// can observe them, then `rename` over the real path — `rename(2)` never
/// yields a reader a half-written file, whatever happens in between.
fn write_backup(path: &Path, backup: &KdeBackup) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("backup path {} has no parent directory", path.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let json = serde_json::to_string_pretty(backup).map_err(|e| e.to_string())?;

    let tmp = path.with_extension("json.tmp");
    let mut file =
        std::fs::File::create(&tmp).map_err(|e| format!("creating {}: {e}", tmp.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("writing {}: {e}", tmp.display()))?;
    // Force the data itself to disk before the rename below can make it
    // visible at `path` — the whole point of the temp-file dance is
    // pointless if the bytes it holds might still only exist in a page
    // cache when the rename (or the write, or the process) is what
    // ultimately survives the crash.
    file.sync_all()
        .map_err(|e| format!("fsyncing {}: {e}", tmp.display()))?;
    drop(file);

    std::fs::rename(&tmp, path)
        .map_err(|e| format!("renaming {} to {}: {e}", tmp.display(), path.display()))
}

fn read_backup(path: &Path) -> Result<KdeBackup, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {} (nothing to undo?): {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))
}

/// The `kscreenlockerrc` values [`setup_with`] is about to overwrite, one
/// entry per [`ManagedKey`] — restored byte-for-byte by [`undo_with`].
/// Persisted to disk (not just kept in memory): `setup()`/`undo()` are two
/// separate [`LockHost`] calls, possibly separated by a daemon restart.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
struct KdeBackup {
    /// [`entry_id`] → the key's original value, or `None` if it did not
    /// exist before `setup()` ran (in which case [`undo_with`] deletes it).
    entries: BTreeMap<String, Option<String>>,
}

// ---------------------------------------------------------------------------
// kreadconfig6 / kwriteconfig6 — abstracted so tests inject a fake
// ---------------------------------------------------------------------------

/// Runs `kreadconfig6`/`kwriteconfig6` against `kscreenlockerrc` — abstracted
/// so every test below can inject [`FakeKConfig`] and never touch the real
/// `~/.config` (this feature's own hard rule, extended to its tests).
trait KConfigRunner {
    /// `kreadconfig6 --file kscreenlockerrc <repeated --group>... --key <key>
    /// --default <default>`, plain-string mode always — see the module docs
    /// for why backup/restore needs no type awareness at all.
    fn read(&self, groups: &[&str], key: &str, default: &str) -> Result<String, String>;
    /// `kwriteconfig6 --file kscreenlockerrc <repeated --group>... --key <key>
    /// [--type bool] <value>`.
    fn write(&self, groups: &[&str], key: &str, value: &str, as_bool: bool) -> Result<(), String>;
    /// `kwriteconfig6 --file kscreenlockerrc <repeated --group>... --key <key>
    /// --delete`.
    fn delete(&self, groups: &[&str], key: &str) -> Result<(), String>;
    /// Whether both binaries this trait shells out to actually exist.
    fn available(&self) -> bool;
}

/// Longest this module waits for `kreadconfig6`/`kwriteconfig6` to exit —
/// mirrors `hosts::mod`'s own `LOGINCTL_TIMEOUT` and its reasoning exactly:
/// both are near-instant ini-file operations, so this bound is generous, not
/// tight, and exists only so a wedged or missing binary fails loudly instead
/// of hanging the caller.
const KCONFIG_TIMEOUT: Duration = Duration::from_secs(5);

/// Spawn `cmd`, wait up to `timeout` via a bounded `try_wait` poll — never a
/// plain blocking `wait`, same shape as `hosts::mod::loginctl_lock_session`'s
/// own bounded wait — and return `(exit success, stdout, stderr)`. Draining
/// the pipes only *after* `try_wait` confirms the process already exited is
/// safe here specifically because both tools' entire output is one short
/// line, far under a pipe buffer; draining before exit (to avoid ever
/// blocking on a full pipe) is not needed for output this small.
fn run_bounded(cmd: &mut Command, timeout: Duration) -> Result<(bool, String, String), String> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut s) = child.stdout.take() {
                    let _ = s.read_to_string(&mut stdout);
                }
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut stderr);
                }
                return Ok((status.success(), stdout, stderr));
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{program} timed out after {timeout:?}"));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(format!("failed to wait on {program}: {e}")),
        }
    }
}

struct ProcessRunner;

impl ProcessRunner {
    fn command(program: &str, groups: &[&str], key: &str) -> Command {
        let mut cmd = Command::new(program);
        cmd.arg("--file").arg(KSCREENLOCKERRC);
        for group in groups {
            cmd.arg("--group").arg(group);
        }
        cmd.arg("--key").arg(key);
        cmd
    }
}

impl KConfigRunner for ProcessRunner {
    fn read(&self, groups: &[&str], key: &str, default: &str) -> Result<String, String> {
        let mut cmd = Self::command("kreadconfig6", groups, key);
        cmd.arg("--default").arg(default);
        let (ok, stdout, stderr) = run_bounded(&mut cmd, KCONFIG_TIMEOUT)?;
        if !ok {
            return Err(format!("kreadconfig6 {key} failed: {}", stderr.trim()));
        }
        Ok(stdout.trim_end_matches('\n').to_string())
    }

    fn write(&self, groups: &[&str], key: &str, value: &str, as_bool: bool) -> Result<(), String> {
        let mut cmd = Self::command("kwriteconfig6", groups, key);
        if as_bool {
            cmd.arg("--type").arg("bool");
        }
        cmd.arg(value);
        let (ok, _stdout, stderr) = run_bounded(&mut cmd, KCONFIG_TIMEOUT)?;
        if !ok {
            return Err(format!("kwriteconfig6 {key} failed: {}", stderr.trim()));
        }
        Ok(())
    }

    fn delete(&self, groups: &[&str], key: &str) -> Result<(), String> {
        let mut cmd = Self::command("kwriteconfig6", groups, key);
        cmd.arg("--delete");
        let (ok, _stdout, stderr) = run_bounded(&mut cmd, KCONFIG_TIMEOUT)?;
        if !ok {
            return Err(format!(
                "kwriteconfig6 --delete {key} failed: {}",
                stderr.trim()
            ));
        }
        Ok(())
    }

    fn available(&self) -> bool {
        on_path("kreadconfig6") && on_path("kwriteconfig6")
    }
}

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| is_executable(&dir.join(name)))
    })
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LockScreen;
    use std::cell::{Cell, RefCell};

    // -- FakeKConfig: an in-memory kscreenlockerrc stand-in --------------------

    struct FakeKConfig {
        entries: RefCell<BTreeMap<(Vec<String>, String), String>>,
        available: Cell<bool>,
        /// When `Some(n)`, the n-th call to `write` (1-indexed, counting a
        /// failing call itself) returns `Err` instead of applying — lets a
        /// test make `setup_with`'s nine-write sequence die at any specific
        /// point without needing nine near-identical test functions.
        fail_write_at: Cell<Option<usize>>,
        write_count: Cell<usize>,
        /// Every key successfully written, in call order — lets a test
        /// assert *which* key was written last without hard-coding
        /// assumptions about the other eight's relative order.
        order_log: RefCell<Vec<String>>,
    }

    impl FakeKConfig {
        fn new() -> Self {
            FakeKConfig {
                entries: RefCell::new(BTreeMap::new()),
                available: Cell::new(true),
                fail_write_at: Cell::new(None),
                write_count: Cell::new(0),
                order_log: RefCell::new(Vec::new()),
            }
        }

        fn seed(&self, groups: &[&str], key: &str, value: &str) {
            self.entries
                .borrow_mut()
                .insert(fake_key(groups, key), value.to_string());
        }

        fn get(&self, groups: &[&str], key: &str) -> Option<String> {
            self.entries.borrow().get(&fake_key(groups, key)).cloned()
        }
    }

    fn fake_key(groups: &[&str], key: &str) -> (Vec<String>, String) {
        (
            groups.iter().map(|s| s.to_string()).collect(),
            key.to_string(),
        )
    }

    impl KConfigRunner for FakeKConfig {
        fn read(&self, groups: &[&str], key: &str, default: &str) -> Result<String, String> {
            Ok(self.get(groups, key).unwrap_or_else(|| default.to_string()))
        }

        fn write(
            &self,
            groups: &[&str],
            key: &str,
            value: &str,
            _as_bool: bool,
        ) -> Result<(), String> {
            let n = self.write_count.get() + 1;
            self.write_count.set(n);
            if self.fail_write_at.get() == Some(n) {
                return Err(format!("simulated failure on write #{n} ({key})"));
            }
            self.entries
                .borrow_mut()
                .insert(fake_key(groups, key), value.to_string());
            self.order_log.borrow_mut().push(key.to_string());
            Ok(())
        }

        fn delete(&self, groups: &[&str], key: &str) -> Result<(), String> {
            self.entries.borrow_mut().remove(&fake_key(groups, key));
            Ok(())
        }

        fn available(&self) -> bool {
            self.available.get()
        }
    }

    fn test_tmp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fresco-kdehost-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed_plugin_files(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("metadata.json"), b"{}").unwrap();
        std::fs::create_dir_all(dir.join("contents/ui")).unwrap();
        std::fs::write(dir.join("contents/ui/main.qml"), b"// stub").unwrap();
    }

    fn ctx(config: &Config) -> HostCtx<'_> {
        HostCtx {
            config,
            outputs: &[],
            runtime_dir: PathBuf::from("/run/user/1000/fresco"),
        }
    }

    // -- kind / targets_while_locked -------------------------------------------

    #[test]
    fn kind_and_targets_while_locked() {
        let config = Config::default();
        let host = KdeHost;
        assert_eq!(host.kind(), HostKind::Kde);
        assert_eq!(
            host.targets_while_locked(&ctx(&config)),
            LockTargets::LayerFiles(PathBuf::from("/run/user/1000/fresco/lock"))
        );
    }

    // -- setup_state_with -----------------------------------------------------

    #[test]
    fn setup_state_unavailable_when_tools_missing() {
        let fake = FakeKConfig::new();
        fake.available.set(false);
        assert_eq!(setup_state_with(&fake), LockSetupState::Unavailable);
    }

    #[test]
    fn setup_state_needed_then_done_tracks_the_selected_plugin() {
        let fake = FakeKConfig::new();
        assert_eq!(setup_state_with(&fake), LockSetupState::Needed);

        fake.seed(GREETER_GROUP, "WallpaperPlugin", PLUGIN_ID);
        assert_eq!(setup_state_with(&fake), LockSetupState::Done);

        fake.seed(GREETER_GROUP, "WallpaperPlugin", "org.kde.image");
        assert_eq!(setup_state_with(&fake), LockSetupState::Needed);
    }

    // -- ensure_plugin_installed ------------------------------------------------

    #[test]
    fn ensure_plugin_installed_skips_the_copy_when_already_installed_system_wide() {
        let tmp = test_tmp_dir("already-system");
        let paths = KdePaths::fake_in(&tmp);
        seed_plugin_files(&paths.system_plugin_dir);
        // No seed dir at all — must not be needed when the system copy exists.
        ensure_plugin_installed(&paths).unwrap();
        assert!(!paths.user_plugin_dir.join("metadata.json").is_file());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn ensure_plugin_installed_skips_the_copy_when_already_installed_per_user() {
        let tmp = test_tmp_dir("already-user");
        let paths = KdePaths::fake_in(&tmp);
        seed_plugin_files(&paths.user_plugin_dir);
        ensure_plugin_installed(&paths).unwrap();
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn ensure_plugin_installed_copies_from_the_seed_when_missing_everywhere_else() {
        let tmp = test_tmp_dir("copy-from-seed");
        let paths = KdePaths::fake_in(&tmp);
        seed_plugin_files(&paths.seed_plugin_dir);

        ensure_plugin_installed(&paths).unwrap();

        assert!(paths.user_plugin_dir.join("metadata.json").is_file());
        assert!(paths.user_plugin_dir.join("contents/ui/main.qml").is_file());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn ensure_plugin_installed_errs_clearly_when_found_nowhere() {
        let tmp = test_tmp_dir("nothing-found");
        let paths = KdePaths::fake_in(&tmp);
        let err = ensure_plugin_installed(&paths).unwrap_err();
        assert!(err.contains("reinstall Fresco"), "{err}");
        std::fs::remove_dir_all(&tmp).ok();
    }

    // -- setup_with / undo_with: the full round trip --------------------------

    #[test]
    fn setup_then_undo_restores_the_exact_original_state() {
        let fake = FakeKConfig::new();
        // A plausible "before Fresco touched anything" state: the stock
        // plugin selected, one unrelated pre-existing leftover value, and
        // every other managed key genuinely absent.
        fake.seed(GREETER_GROUP, "WallpaperPlugin", "org.kde.image");
        fake.seed(GENERAL_GROUP, "Dim", "0.3");

        let tmp = test_tmp_dir("setup-undo-roundtrip");
        let paths = KdePaths::fake_in(&tmp);
        seed_plugin_files(&paths.seed_plugin_dir);

        let mut config = Config::default();
        config.wallpaper.kind = Kind::Video;
        config.wallpaper.path = Some(PathBuf::from("/videos/wp.mp4"));
        config.lockscreen = Some(LockScreen {
            enabled: true,
            dim: 0.5,
            ..LockScreen::default()
        });
        let c = ctx(&config);

        setup_with(&fake, &c, &paths).unwrap();

        assert_eq!(
            fake.get(GREETER_GROUP, "WallpaperPlugin").as_deref(),
            Some(PLUGIN_ID)
        );
        assert_eq!(
            fake.get(GENERAL_GROUP, "PlayVideo").as_deref(),
            Some("true")
        );
        assert_eq!(fake.get(GENERAL_GROUP, "Dim").as_deref(), Some("0.5"));
        assert!(fake
            .get(GENERAL_GROUP, "LayerDir")
            .unwrap()
            .ends_with("/lock"));
        assert_eq!(
            fake.get(LNF_GROUP, "alwaysShowClock").as_deref(),
            Some("false")
        );
        assert!(paths.user_plugin_dir.join("metadata.json").is_file());
        assert!(paths.backup_file.exists());

        undo_with(&fake, &paths).unwrap();

        // The pre-existing values come back exactly...
        assert_eq!(
            fake.get(GREETER_GROUP, "WallpaperPlugin").as_deref(),
            Some("org.kde.image")
        );
        assert_eq!(fake.get(GENERAL_GROUP, "Dim").as_deref(), Some("0.3"));
        // ...and every key that was genuinely absent before is absent again.
        for key in [
            "VideoPath",
            "StillPath",
            "PlayVideo",
            "LayerDir",
            "RefreshMs",
        ] {
            assert_eq!(fake.get(GENERAL_GROUP, key), None, "{key}");
        }
        for key in ["alwaysShowClock", "showMediaControls"] {
            assert_eq!(fake.get(LNF_GROUP, key), None, "{key}");
        }
        assert!(!paths.backup_file.exists());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn setup_is_idempotent_against_a_partial_failure_retry() {
        // Simulates: setup() ran once (backup written, some values changed),
        // then something failed and the caller retries setup() from
        // scratch — the SECOND run must not clobber the backup with the
        // now-half-modified state the first run left behind.
        let fake = FakeKConfig::new();
        fake.seed(GREETER_GROUP, "WallpaperPlugin", "org.kde.image");

        let tmp = test_tmp_dir("idempotent-retry");
        let paths = KdePaths::fake_in(&tmp);
        seed_plugin_files(&paths.seed_plugin_dir);
        let config = Config::default();
        let c = ctx(&config);

        setup_with(&fake, &c, &paths).unwrap();
        // Pretend the plugin selection changed again after setup (as if a
        // partial failure or a manual edit happened).
        fake.seed(GREETER_GROUP, "WallpaperPlugin", "something-else");

        setup_with(&fake, &c, &paths).unwrap();
        undo_with(&fake, &paths).unwrap();

        // The ORIGINAL value survives, not "something-else".
        assert_eq!(
            fake.get(GREETER_GROUP, "WallpaperPlugin").as_deref(),
            Some("org.kde.image")
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn setup_and_undo_err_clearly_when_tools_are_unavailable() {
        let fake = FakeKConfig::new();
        fake.available.set(false);
        let tmp = test_tmp_dir("unavailable");
        let paths = KdePaths::fake_in(&tmp);
        let config = Config::default();
        assert!(setup_with(&fake, &ctx(&config), &paths).is_err());
        assert!(undo_with(&fake, &paths).is_err());
        std::fs::remove_dir_all(&tmp).ok();
    }

    // -- setup_with: write ordering and rollback on a partial failure ---------

    #[test]
    fn setup_writes_the_greeter_plugin_key_last() {
        let fake = FakeKConfig::new();
        let tmp = test_tmp_dir("write-order");
        let paths = KdePaths::fake_in(&tmp);
        seed_plugin_files(&paths.seed_plugin_dir);
        let config = Config::default();

        setup_with(&fake, &ctx(&config), &paths).unwrap();

        let order = fake.order_log.borrow();
        assert_eq!(
            order.last().map(String::as_str),
            Some("WallpaperPlugin"),
            "General/LnF keys must all land before the key that actually \
             switches KDE onto this plugin: {order:?}"
        );
        assert_eq!(order.len(), 9, "all nine managed writes ran: {order:?}");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn setup_rolls_back_every_write_so_far_when_any_write_fails_partway() {
        // For EVERY possible failure point in the nine-write sequence, the
        // fake config must end up exactly as it started — never half-applied
        // (e.g. `WallpaperPlugin` pointing at Fresco without its keys).
        for fail_at in 1..=9usize {
            let fake = FakeKConfig::new();
            // A plausible pre-Fresco state: the stock plugin selected, one
            // unrelated pre-existing value, everything else genuinely absent.
            fake.seed(GREETER_GROUP, "WallpaperPlugin", "org.kde.image");
            fake.seed(GENERAL_GROUP, "Dim", "0.3");
            let original = fake.entries.borrow().clone();

            let tmp = test_tmp_dir(&format!("rollback-fail-{fail_at}"));
            let paths = KdePaths::fake_in(&tmp);
            seed_plugin_files(&paths.seed_plugin_dir);

            let mut config = Config::default();
            config.wallpaper.kind = Kind::Video;
            config.wallpaper.path = Some(PathBuf::from("/videos/wp.mp4"));
            config.lockscreen = Some(LockScreen {
                enabled: true,
                dim: 0.5,
                ..LockScreen::default()
            });
            let c = ctx(&config);

            fake.fail_write_at.set(Some(fail_at));
            let err = setup_with(&fake, &c, &paths).unwrap_err();
            assert!(!err.is_empty(), "fail_at={fail_at}");

            assert_eq!(
                *fake.entries.borrow(),
                original,
                "fail_at={fail_at}: config must roll back to exactly its original state"
            );

            std::fs::remove_dir_all(&tmp).ok();
        }
    }

    // -- setup_with: recovering from a corrupt backup file ---------------------

    #[test]
    fn setup_recovers_from_a_truncated_backup_file_by_quarantining_it_and_taking_a_fresh_one() {
        let fake = FakeKConfig::new();
        fake.seed(GREETER_GROUP, "WallpaperPlugin", "org.kde.image");
        fake.seed(GENERAL_GROUP, "Dim", "0.42");

        let tmp = test_tmp_dir("corrupt-backup-recovery");
        let paths = KdePaths::fake_in(&tmp);
        seed_plugin_files(&paths.seed_plugin_dir);

        // Simulate what a crash mid-write (before this fix) could leave
        // behind: a truncated, unparseable JSON file at the backup path.
        if let Some(dir) = paths.backup_file.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(&paths.backup_file, b"{\"entries\":{\"Greeter.Wallpaper").unwrap();

        let config = Config::default();
        setup_with(&fake, &ctx(&config), &paths).unwrap();

        // The corrupt file was moved aside for forensics, not left in place
        // and not silently treated as a valid (empty) backup.
        let dir_entries: Vec<String> = std::fs::read_dir(&tmp)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            dir_entries.iter().any(|n| n.starts_with("backup.corrupt-")),
            "expected a quarantined copy of the corrupt backup, found {dir_entries:?}"
        );

        // A FRESH backup of the CURRENT (pre-setup) values now lives at the
        // normal path, not the corrupt file — so undo can still restore them.
        let backup = read_backup(&paths.backup_file).unwrap();
        assert_eq!(
            backup.entries.get(&entry_id(&MANAGED_KEYS[0])), // WallpaperPlugin
            Some(&Some("org.kde.image".to_string()))
        );
        assert_eq!(
            backup.entries.get(&entry_id(&MANAGED_KEYS[4])), // Dim
            Some(&Some("0.42".to_string()))
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    // -- write_backup: atomic write never corrupts the previous file ----------

    #[test]
    fn write_backup_leaves_the_old_file_intact_when_the_write_fails() {
        use std::os::unix::fs::PermissionsExt as _;

        let tmp = test_tmp_dir("atomic-write-failure");
        let backup_path = tmp.join("backup.json");
        let original = KdeBackup {
            entries: BTreeMap::from([("a.b".to_string(), Some("original-value".to_string()))]),
        };
        write_backup(&backup_path, &original).unwrap();

        // Make the directory read-only so creating the NEXT write's temp
        // file fails partway through — `backup_path` itself must never be
        // touched by a write that can't even get as far as `rename`.
        let dir_perms = std::fs::metadata(&tmp).unwrap().permissions();
        let mut readonly = dir_perms.clone();
        readonly.set_mode(0o555);
        std::fs::set_permissions(&tmp, readonly).unwrap();

        let attempted = KdeBackup {
            entries: BTreeMap::from([("a.b".to_string(), Some("corrupted-attempt".to_string()))]),
        };
        let result = write_backup(&backup_path, &attempted);

        // Restore permissions before any assertion can panic and skip cleanup.
        std::fs::set_permissions(&tmp, dir_perms).unwrap();

        assert!(result.is_err(), "a write into a read-only dir must fail");
        let survived = read_backup(&backup_path).unwrap();
        assert_eq!(
            survived, original,
            "the original backup file must be untouched by a failed write"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    // -- refresh_config_with ---------------------------------------------------

    #[test]
    fn refresh_config_is_a_noop_unless_setup_is_done() {
        let fake = FakeKConfig::new();
        let config = Config::default();
        refresh_config_with(&fake, &ctx(&config)).unwrap();
        assert_eq!(fake.get(GENERAL_GROUP, "VideoPath"), None);
    }

    #[test]
    fn refresh_config_rewrites_only_its_four_keys_when_done() {
        let fake = FakeKConfig::new();
        fake.seed(GREETER_GROUP, "WallpaperPlugin", PLUGIN_ID);
        fake.seed(GENERAL_GROUP, "LayerDir", "/original/lock");
        fake.seed(GENERAL_GROUP, "RefreshMs", "1000");

        let mut config = Config::default();
        config.wallpaper.kind = Kind::Video;
        config.wallpaper.path = Some(PathBuf::from("/videos/new.mp4"));
        config.lockscreen = Some(LockScreen {
            dim: 0.6,
            ..LockScreen::default()
        });

        refresh_config_with(&fake, &ctx(&config)).unwrap();

        assert_eq!(
            fake.get(GENERAL_GROUP, "VideoPath").as_deref(),
            Some("/videos/new.mp4")
        );
        assert_eq!(fake.get(GENERAL_GROUP, "Dim").as_deref(), Some("0.6"));
        // Untouched.
        assert_eq!(
            fake.get(GENERAL_GROUP, "LayerDir").as_deref(),
            Some("/original/lock")
        );
        assert_eq!(
            fake.get(GENERAL_GROUP, "RefreshMs").as_deref(),
            Some("1000")
        );
    }

    // -- wallpaper_paths / bool_str / entry_id (pure) ---------------------------

    #[test]
    fn wallpaper_paths_video_kind_has_a_video_path_and_no_still_for_a_missing_source() {
        let w = Wallpaper {
            kind: Kind::Video,
            path: Some(PathBuf::from("/tmp/fresco-kde-test-does-not-exist.mp4")),
            ..Wallpaper::default()
        };
        let (video, still) = wallpaper_paths(&w);
        assert_eq!(video, "/tmp/fresco-kde-test-does-not-exist.mp4");
        assert_eq!(still, "", "render_still returns None for a missing source");
    }

    #[test]
    fn wallpaper_paths_image_kind_has_no_video_path() {
        let w = Wallpaper {
            kind: Kind::Image,
            path: Some(PathBuf::from("/tmp/fresco-kde-test-does-not-exist.png")),
            ..Wallpaper::default()
        };
        let (video, _still) = wallpaper_paths(&w);
        assert_eq!(video, "");
    }

    #[test]
    fn bool_str_pins_the_literal_text() {
        assert_eq!(bool_str(true), "true");
        assert_eq!(bool_str(false), "false");
    }

    #[test]
    fn entry_id_is_stable_and_distinct_per_key() {
        let ids: Vec<String> = MANAGED_KEYS.iter().map(entry_id).collect();
        let unique: std::collections::BTreeSet<&String> = ids.iter().collect();
        assert_eq!(
            unique.len(),
            ids.len(),
            "no two managed keys collide: {ids:?}"
        );
    }
}

//! KDE Plasma desktop wallpaper — GitHub issue #44.
//!
//! plasmashell paints the desktop (wallpaper *and* icons) into one opaque
//! window — KWin's desktop layer on X11, a layer-shell background surface on
//! Wayland, window colour black. A window of ours underneath it is never
//! visible (it shows up only in KWin's Overview / Desktop Grid thumbnails);
//! one above it hides the icons and widgets. So on Plasma we create no window
//! at all: the Plasma wallpaper plugin Fresco already ships for the lock
//! screen (`packaging/kde/`) is selected as the *desktop* wallpaper of every
//! screen through plasmashell's own scripting DBus API — the call
//! `plasma-apply-wallpaperimage` makes — and plasmashell plays the video
//! inside its own desktop, icons included.
//!
//! The user's previous wallpaper plugin per desktop is saved once and put
//! back on Stop. That plugin keeps its own config group (an image keeps its
//! `Image=` path), so restoring is just selecting it again.
//!
//! Trade-offs against the mpv backends, accepted for Plasma: QtMultimedia
//! plays the video (muted, no hwdec tuning, crop/rotation/scaling or
//! transitions), a playlist plays its first file, a slideshow shows its first
//! frame, and one wallpaper is used for every screen.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::lock::hosts::kde::{self, PLUGIN_ID};
use super::{dde, overview};
use crate::config::{Config, Wallpaper};

const DEST: &str = "org.kde.plasmashell";
const OBJECT: &str = "/PlasmaShell";
const METHOD: &str = "org.kde.PlasmaShell.evaluateScript";

/// What Plasma shows when a desktop's saved plugin is unknown.
const FALLBACK_PLUGIN: &str = "org.kde.image";

/// How long one `evaluateScript` call may take.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(10);
/// Autostart can run before plasmashell has claimed its bus name; wait this
/// long for it (checked every second) before giving up on an apply.
const SHELL_WAIT: Duration = Duration::from_secs(90);

/// Why the plasmashell path is (not) in use. Decided once per daemon: nothing
/// it looks at changes while we run.
fn decision() -> &'static Result<(), String> {
    static DECISION: OnceLock<Result<(), String>> = OnceLock::new();
    DECISION.get_or_init(|| {
        if !crate::capability::is_kde() {
            return Err("not a KDE Plasma session".into());
        }
        if std::env::var("FRESCO_KDE_DESKTOP").is_ok_and(|v| v.trim() == "0") {
            return Err("FRESCO_KDE_DESKTOP=0".into());
        }
        if !kde::on_path("plasmashell") {
            return Err("plasmashell is not installed".into());
        }
        kde::ensure_plugin_installed_real()?;
        log::info!(
            "KDE Plasma: the wallpaper is applied through plasmashell's wallpaper plugin \
             ({PLUGIN_ID}), not a window of ours — the desktop icons stay visible"
        );
        Ok(())
    })
}

/// Is the wallpaper applied through plasmashell on this session? When true the
/// daemons create no wallpaper window / mpvpaper surface.
pub fn enabled() -> bool {
    decision().is_ok()
}

/// One line for `frescod --check`: how this Plasma session gets its wallpaper.
pub fn report() -> String {
    match decision() {
        Ok(()) => {
            let desktops = match list_desktops() {
                Ok(d) if d.is_empty() => "none reported".to_string(),
                Ok(d) => d
                    .iter()
                    .map(|(id, plugin)| format!("{id}={plugin}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                Err(e) => format!("plasmashell not reachable: {e}"),
            };
            format!("wallpaper plugin {PLUGIN_ID} (desktop plugins: {desktops})")
        }
        Err(why) => format!("window backend — the plugin path is off ({why})"),
    }
}

// ── apply / restore ──────────────────────────────────────────────────────────

/// Bumped by every apply and restore. A queued apply that finds it changed
/// has been superseded and drops out.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Serialises plasmashell calls: applies from the loops, and the final restore.
static BUSY: Mutex<()> = Mutex::new(());

/// Show `wallpaper` as every Plasma desktop's wallpaper. No-op off Plasma.
/// Runs on a detached thread: waiting for plasmashell at login, rendering the
/// poster frame and the DBus call can take seconds, none of which may stall a
/// daemon loop. Idempotent.
pub fn apply(config: &Config) {
    if !enabled() {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let wallpaper = config.wallpaper.clone();
    let pause_mode = kde_pause_mode(config);
    std::thread::spawn(move || {
        let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = apply_now(&wallpaper, pause_mode, generation) {
            log::warn!("KDE Plasma: could not apply the wallpaper through plasmashell: {e}");
        }
    });
}

fn apply_now(wallpaper: &Wallpaper, pause_mode: i32, generation: u64) -> Result<(), String> {
    if !wait_for_shell(generation) {
        return Ok(()); // superseded
    }
    let (video, still) = kde::wallpaper_paths(wallpaper);
    if GENERATION.load(Ordering::SeqCst) != generation {
        return Ok(());
    }
    if video.is_empty() && still.is_empty() {
        return Err("the wallpaper has no video or image to show".into());
    }
    // Not fatal: without a record, Stop falls back to Plasma's default image.
    if let Err(e) = save_original() {
        log::warn!("KDE Plasma: could not record the previous wallpaper: {e}");
    }
    let uri = |p: &str| {
        if p.is_empty() {
            String::new()
        } else {
            overview::encode_file_uri(std::path::Path::new(p))
        }
    };
    evaluate(&apply_script(&uri(&video), &uri(&still), pause_mode))?;
    log::info!(
        "KDE Plasma: applied via plasmashell wallpaper plugin ({})",
        if video.is_empty() { &still } else { &video }
    );
    Ok(())
}

/// Put every desktop that still shows our plugin back on what it showed before
/// Fresco (or on Plasma's default image). Blocks, bounded — it runs on the way
/// out. No-op off Plasma and when Fresco never changed anything.
pub fn restore() {
    if !enabled() {
        return;
    }
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    let Some(saved) = std::fs::read(saved_path())
        .ok()
        .and_then(|b| serde_json::from_slice::<Saved>(&b).ok())
    else {
        return;
    };
    if !shell_running() {
        log::info!(
            "KDE Plasma: plasmashell is not running; leaving the wallpaper restore for next time"
        );
        return;
    }
    match evaluate(&restore_script(&saved)) {
        Ok(_) => {
            std::fs::remove_file(saved_path()).ok();
            log::info!("KDE Plasma: restored the previous desktop wallpaper");
        }
        Err(e) => log::warn!("KDE Plasma: could not restore the previous wallpaper: {e}"),
    }
}

// ── saved state ──────────────────────────────────────────────────────────────

/// The wallpaper plugin each desktop showed before Fresco took over.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Saved {
    /// Containment id → plugin id. Desktops already on our plugin are absent.
    desktops: BTreeMap<String, String>,
}

fn saved_path() -> PathBuf {
    dde::state_dir().join("kde-desktop-saved.json")
}

/// Record what each desktop shows now, once. A state file left by an earlier
/// run holds the true original — the desktops may already be on our plugin.
fn save_original() -> Result<(), String> {
    let path = saved_path();
    if path.exists() {
        return Ok(());
    }
    let mut saved = Saved::default();
    match list_desktops() {
        Ok(listing) => {
            for (id, plugin) in listing {
                if plugin != PLUGIN_ID {
                    saved.desktops.insert(id.to_string(), plugin);
                }
            }
        }
        // Still apply: restore then falls back to Plasma's default image.
        Err(e) => log::warn!("KDE Plasma: could not read the current desktop wallpapers ({e}); a restore will use {FALLBACK_PLUGIN}"),
    }
    std::fs::create_dir_all(dde::state_dir()).ok();
    let json = serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// `(containment id, wallpaper plugin)` of every desktop.
fn list_desktops() -> Result<Vec<(u64, String)>, String> {
    let out = evaluate(LIST_SCRIPT)?;
    let json = dde::parse_first_string(&out).ok_or_else(|| format!("unexpected reply {out:?}"))?;
    serde_json::from_str(json.trim()).map_err(|e| format!("unexpected reply {json:?}: {e}"))
}

// ── plasmashell scripting ────────────────────────────────────────────────────

const LIST_SCRIPT: &str =
    "print(JSON.stringify(desktops().map(function(d){return [d.id, d.wallpaperPlugin];})));";

/// A JS string literal for `s`. JSON is JS except for U+2028/2029, which older
/// JS engines treat as line breaks inside a literal. Every value interpolated
/// into a script goes through here: paths and plugin ids are the injection
/// boundary of `evaluateScript`.
fn js_str(s: &str) -> String {
    serde_json::to_string(s)
        .unwrap_or_else(|_| "\"\"".into())
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// The plugin's `PauseMode` (`contents/config/main.xml`): 0 pauses the video
/// while a window is fullscreen, 1 also while one is maximized, 2 never.
fn kde_pause_mode(_config: &Config) -> i32 {
    // wire to config.pause_on_maximized once feat/pause-on-maximized lands
    0
}

/// The script `plasma-apply-wallpaperimage` sends, with our plugin and its
/// config keys (`contents/config/main.xml`). No veil, no widget layer on the
/// desktop.
fn apply_script(video_uri: &str, still_uri: &str, pause_mode: i32) -> String {
    let id = js_str(PLUGIN_ID);
    format!(
        "desktops().forEach(function(d){{\
         d.wallpaperPlugin={id};\
         d.currentConfigGroup=['Wallpaper',{id},'General'];\
         d.writeConfig('VideoPath',{video});\
         d.writeConfig('StillPath',{still});\
         d.writeConfig('PlayVideo',{play});\
         d.writeConfig('PauseMode',{pause_mode});\
         d.writeConfig('Dim',0);\
         }});",
        video = js_str(video_uri),
        still = js_str(still_uri),
        play = !video_uri.is_empty(),
    )
}

/// Reselect the saved plugin on every desktop still showing ours.
fn restore_script(saved: &Saved) -> String {
    let map = serde_json::to_string(&saved.desktops)
        .unwrap_or_else(|_| "{}".into())
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    format!(
        "var m={map};desktops().forEach(function(d){{\
         if(d.wallpaperPlugin=={id}){{d.wallpaperPlugin=m[d.id]||{fallback};}}\
         }});",
        id = js_str(PLUGIN_ID),
        fallback = js_str(FALLBACK_PLUGIN),
    )
}

/// Run `script` in plasmashell; returns what it `print()`ed.
fn evaluate(script: &str) -> Result<String, String> {
    let mut cmd = Command::new("gdbus");
    cmd.args(["call", "--session", "--timeout", "10"])
        .args(["--dest", DEST, "--object-path", OBJECT, "--method", METHOD])
        .arg(overview::gvariant_string_literal(script));
    let (ok, stdout, stderr) = kde::run_bounded(&mut cmd, SCRIPT_TIMEOUT + Duration::from_secs(2))?;
    if ok {
        Ok(stdout)
    } else {
        Err(stderr.trim().to_string())
    }
}

fn shell_running() -> bool {
    let mut cmd = Command::new("gdbus");
    cmd.args(["call", "--session", "--timeout", "3"])
        .args(["--dest", "org.freedesktop.DBus"])
        .args(["--object-path", "/org/freedesktop/DBus"])
        .args(["--method", "org.freedesktop.DBus.NameHasOwner", DEST]);
    kde::run_bounded(&mut cmd, Duration::from_secs(5))
        .is_ok_and(|(ok, out, _)| ok && out.contains("true"))
}

/// Wait for plasmashell to own its bus name. False when this apply was
/// superseded meanwhile; on timeout it returns true and the call itself then
/// reports the failure.
fn wait_for_shell(generation: u64) -> bool {
    let start = Instant::now();
    while !shell_running() {
        if GENERATION.load(Ordering::SeqCst) != generation {
            return false;
        }
        if start.elapsed() >= SHELL_WAIT {
            log::warn!(
                "KDE Plasma: plasmashell is not on the session bus after {}s",
                SHELL_WAIT.as_secs()
            );
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    GENERATION.load(Ordering::SeqCst) == generation
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script with every JS string literal collapsed to `S`, so what is
    /// left is the code skeleton the interpolated values must not alter.
    fn skeleton(script: &str) -> String {
        let mut out = String::new();
        let mut chars = script.chars();
        while let Some(c) = chars.next() {
            if c == '"' {
                loop {
                    match chars.next() {
                        Some('\\') => {
                            chars.next();
                        }
                        Some('"') | None => break,
                        Some(_) => {}
                    }
                }
                out.push('S');
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn js_str_quotes_and_escapes() {
        assert_eq!(js_str("a"), r#""a""#);
        assert_eq!(js_str(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(js_str("a\nb\u{2028}c\u{2029}"), "\"a\\nb\\u2028c\\u2029\"");
        assert_eq!(js_str("é'`$"), "\"é'`$\"");
    }

    #[test]
    fn hostile_paths_stay_inside_their_string_literal() {
        let benign = apply_script("file:///a.mp4", "file:///a.png", 0);
        for evil in [
            "x');d.wallpaperPlugin=('evil",
            "x\");evil();(\"",
            "x\\\");evil();(\"",
            "x\n});evil();desktops().forEach(function(d){",
            "x\u{2028}evil();//",
            "x</script>",
        ] {
            let s = apply_script(evil, evil, 0);
            assert_eq!(skeleton(&s), skeleton(&benign), "{evil:?}");
        }
    }

    #[test]
    fn apply_script_selects_our_plugin_and_writes_the_plugin_keys() {
        let s = apply_script("file:///v.mp4", "file:///s.png", 1);
        assert!(s.starts_with("desktops().forEach(function(d){d.wallpaperPlugin=\"io.github."));
        for key in ["VideoPath", "StillPath", "PlayVideo", "PauseMode", "Dim"] {
            assert!(s.contains(&format!("writeConfig('{key}'")), "{key}");
        }
        assert!(s.contains("writeConfig('PlayVideo',true)"));
        assert!(s.contains("writeConfig('PauseMode',1)"));
        assert!(
            s.contains("['Wallpaper',\"io.github.dibbayajyotiroy.fresco.lockscreen\",'General']")
        );
        // An image wallpaper has nothing to play.
        assert!(apply_script("", "file:///s.png", 0).contains("writeConfig('PlayVideo',false)"));
    }

    #[test]
    fn restore_script_only_touches_desktops_on_our_plugin() {
        let mut saved = Saved::default();
        saved
            .desktops
            .insert("7".into(), "org.kde.slideshow".into());
        saved.desktops.insert("9".into(), "ev\"il".into());
        let s = restore_script(&saved);
        assert!(s.starts_with(r#"var m={"7":"org.kde.slideshow","9":"ev\"il"};"#));
        assert!(s.contains("if(d.wallpaperPlugin==\"io.github."));
        assert!(s.contains("||\"org.kde.image\""));
        let empty = restore_script(&Saved::default());
        assert!(empty.starts_with("var m={};"));
    }

    #[test]
    fn script_survives_gdbus_argument_quoting() {
        // gdbus parses the argument as a GVariant string: quotes and
        // backslashes in the script must round-trip through the literal.
        let script = apply_script("file:///it's.mp4", "", 0);
        let lit = overview::gvariant_string_literal(&script);
        let inner = &lit[1..lit.len() - 1];
        let mut un = String::new();
        let mut it = inner.chars();
        while let Some(c) = it.next() {
            un.push(if c == '\\' { it.next().unwrap() } else { c });
        }
        assert_eq!(un, script);
    }

    #[test]
    fn saved_state_round_trips() {
        let mut s = Saved::default();
        s.desktops.insert("12".into(), "org.kde.image".into());
        let back: Saved = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}

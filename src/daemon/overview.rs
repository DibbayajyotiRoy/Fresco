//! GNOME overview fallback.
//!
//! Our live wallpaper is an X11 window the GNOME Activities overview, workspace
//! switcher, and lock screen can't see — they draw `org.gnome.desktop.background`
//! instead. To keep those surfaces consistent, we extract a still frame from the
//! active wallpaper and set it as the desktop background, saving the user's
//! original first and restoring it on Stop. No-op on non-GNOME desktops.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{Kind, Wallpaper};

const SCHEMA: &str = "org.gnome.desktop.background";

/// Set a still frame of `wallpaper` as the desktop background.
pub fn apply(wallpaper: &Wallpaper) {
    if !gnome_available() {
        return;
    }
    let Some(frame) = render_still(wallpaper) else {
        return;
    };
    save_original_once();
    // GVariant string literal: 'file:///path'. Our frame path is a safe cache
    // location (no spaces/quotes), so simple single-quoting is sufficient.
    let gv = format!("'file://{}'", frame.display());
    gset("picture-uri", &gv);
    gset("picture-uri-dark", &gv);
    log::info!("overview background set to {}", frame.display());
}

/// Restore the user's original background (called on Stop / shutdown).
pub fn restore() {
    let sf = state_file();
    let Ok(text) = std::fs::read_to_string(&sf) else {
        return;
    };
    let mut lines = text.lines();
    if let Some(v) = lines.next() {
        gset("picture-uri", v);
    }
    if let Some(v) = lines.next() {
        gset("picture-uri-dark", v);
    }
    std::fs::remove_file(&sf).ok();
    log::info!("overview background restored");
}

/// Produce a full-size still PNG for the active wallpaper. Uses a fresh
/// timestamped filename each call so GNOME reliably reloads the new image.
fn render_still(w: &Wallpaper) -> Option<PathBuf> {
    let src = match w.kind {
        Kind::Slideshow => {
            let s = w.slideshow.as_ref()?;
            super::slideshow_images(s).into_iter().next()?
        }
        _ => w.effective_path()?.to_path_buf(),
    };
    if !src.exists() {
        return None;
    }
    let dir = cache_dir();
    std::fs::create_dir_all(&dir).ok();
    // Drop previous frames so the cache doesn't grow.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let is_frame = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("overview-"));
            if is_frame {
                std::fs::remove_file(p).ok();
            }
        }
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out = dir.join(format!("overview-{stamp}.png"));
    // The still must match what's on screen — INCLUDING the user's rotation,
    // or the workspace switcher / overview shows the unrotated frame.
    // ffmpegthumbnailer can't rotate, so rotated wallpapers go through ffmpeg
    // when available; without ffmpeg we fall back to the unrotated frame
    // (better than none) and say so in the log.
    let rotation = w.rotation % 360;
    if rotation != 0 {
        let transpose = match rotation {
            90 => "transpose=1", // mpv video-rotate is clockwise
            180 => "transpose=1,transpose=1",
            270 => "transpose=2",
            _ => "null",
        };
        let ok = Command::new("ffmpeg")
            // -nostdin + null stdio: ffmpeg reads the terminal by default, and
            // from a shell-launched daemon that SIGTTIN-stops the WHOLE
            // process group — daemon suspended, wallpaper frozen.
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .args([
                "-nostdin",
                "-y",
                "-loglevel",
                "error",
                "-i",
                &src.to_string_lossy(),
                "-frames:v",
                "1",
                "-vf",
                transpose,
                &out.to_string_lossy(),
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(out);
        }
        log::warn!("ffmpeg unavailable/failed; overview frame will not be rotated");
    }
    // ffmpegthumbnailer handles both video frames and images; -s 0 = full size.
    let ok = Command::new("ffmpegthumbnailer")
        .args([
            "-i",
            &src.to_string_lossy(),
            "-o",
            &out.to_string_lossy(),
            "-s",
            "0",
            "-q",
            "10",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then_some(out)
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("fresco")
}

fn state_file() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fresco")
        .join("saved-background")
}

/// Save the user's current background once, so Stop can restore it. Guarded by
/// the state file's existence so we never overwrite the real original with our
/// own frame (e.g. across an apply or a re-login while active).
fn save_original_once() {
    let sf = state_file();
    if sf.exists() {
        return;
    }
    let light = gget("picture-uri");
    let dark = gget("picture-uri-dark");
    if light.is_empty() && dark.is_empty() {
        return;
    }
    if let Some(d) = sf.parent() {
        std::fs::create_dir_all(d).ok();
    }
    std::fs::write(&sf, format!("{light}\n{dark}\n")).ok();
}

fn gnome_available() -> bool {
    Command::new("gsettings")
        .args(["get", SCHEMA, "picture-uri"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn gget(key: &str) -> String {
    Command::new("gsettings")
        .args(["get", SCHEMA, key])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn gset(key: &str, gvariant: &str) {
    let _ = Command::new("gsettings")
        .args(["set", SCHEMA, key, gvariant])
        .status();
}

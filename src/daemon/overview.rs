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

const GNOME_SCHEMA: &str = "org.gnome.desktop.background";
/// Cinnamon (Linux Mint) forked the background settings: muffin and the
/// Cinnamon Wayland session draw `org.cinnamon.desktop.background` and ignore
/// the GNOME schema, so writing only GNOME's key is an invisible wallpaper.
const CINNAMON_SCHEMA: &str = "org.cinnamon.desktop.background";

/// MATE forked them again, and differently: `org.mate.background` keeps the
/// picture in `picture-filename` as a plain path, not a URI. Caja draws it —
/// which is what shows while its desktop is peeked at above the wallpaper.
const MATE_SCHEMA: &str = "org.mate.background";

/// A desktop's background settings: where they live, which keys hold the
/// picture (the first is the one probed for availability), and whether those
/// keys take a `file://` URI or a bare path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Schema {
    name: &'static str,
    keys: &'static [&'static str],
    uri: bool,
}

/// The background schema this session actually draws.
fn schema() -> Schema {
    schema_for(
        crate::capability::is_cinnamon(),
        crate::capability::is_mate(),
    )
}

/// Pure form of [`schema`]. Cinnamon has no `picture-uri-dark`.
fn schema_for(cinnamon: bool, mate: bool) -> Schema {
    if mate {
        Schema {
            name: MATE_SCHEMA,
            keys: &["picture-filename"],
            uri: false,
        }
    } else if cinnamon {
        Schema {
            name: CINNAMON_SCHEMA,
            keys: &["picture-uri"],
            uri: true,
        }
    } else {
        Schema {
            name: GNOME_SCHEMA,
            keys: &["picture-uri", "picture-uri-dark"],
            uri: true,
        }
    }
}

/// Set a still frame of `wallpaper` as the desktop background.
///
/// Skipped while the MATE icon mirror has Caja painting its key colour
/// (`caja_mirror`): the still frame would replace the key, the mirror would
/// then find no key pixels to cut away, and the whole photograph — not just
/// the icons — would be copied over the video. The still is not needed there
/// anyway: Caja never shows its background while the mirror runs.
pub fn apply(wallpaper: &Wallpaper) {
    if super::caja_mirror::key_active() {
        return;
    }
    if !gnome_available() {
        return;
    }
    let Some(frame) = render_still(wallpaper) else {
        return;
    };
    save_original_once();
    // GVariant string literal: 'file:///path' (or '/path' for MATE). Our frame
    // path is a safe cache location (no spaces/quotes), so simple
    // single-quoting is sufficient.
    let s = schema();
    let gv = if s.uri {
        format!("'file://{}'", frame.display())
    } else {
        format!("'{}'", frame.display())
    };
    for key in s.keys {
        gset(key, &gv);
    }
    log::info!("overview background set to {}", frame.display());
}

/// Restore the user's original background (called on Stop / shutdown).
pub fn restore() {
    let sf = state_file();
    let Ok(text) = std::fs::read_to_string(&sf) else {
        return;
    };
    for (key, v) in schema().keys.iter().zip(text.lines()) {
        if !v.is_empty() {
            gset(key, v);
        }
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
    let values: Vec<String> = schema().keys.iter().map(|k| gget(k)).collect();
    if values.iter().all(String::is_empty) {
        return;
    }
    if let Some(d) = sf.parent() {
        std::fs::create_dir_all(d).ok();
    }
    std::fs::write(&sf, values.join("\n") + "\n").ok();
}

/// Whether the GNOME background schema can be driven at all.
///
/// Two very different failures land here and must not read the same. A
/// `gsettings` that runs and says "no such schema" is simply not a GNOME
/// desktop — the documented no-op, and silent. A `gsettings` that will not
/// *spawn* is a missing package, and on GNOME Wayland that is the whole
/// wallpaper: the static frame this module paints is the only backend Mutter
/// allows, so a user who silently gets no wallpaper has nothing to go on. Name
/// the binary and the package once, at `warn`, rather than returning `false`
/// the way a KDE session does.
fn gnome_available() -> bool {
    let s = schema();
    match Command::new("gsettings")
        .args(["get", s.name, s.keys[0]])
        .output()
    {
        Ok(out) => out.status.success(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Once per daemon, not once per apply: a rotating slideshow calls
            // this on every change, and the fix does not get truer by repetition.
            static WARNED: std::sync::Once = std::sync::Once::new();
            WARNED.call_once(|| {
                log::warn!(
                    "overview: `gsettings` is not installed — install libglib2.0-bin \
                     (Debian/Ubuntu), glib2 (Arch/Fedora) or glib2-tools (openSUSE); \
                     without it Fresco cannot set the GNOME desktop background, which \
                     is the only wallpaper surface available on GNOME Wayland"
                );
            });
            false
        }
        Err(e) => {
            log::debug!("overview: gsettings failed to run: {e}");
            false
        }
    }
}

fn gget(key: &str) -> String {
    Command::new("gsettings")
        .args(["get", schema().name, key])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn gset(key: &str, gvariant: &str) {
    let _ = Command::new("gsettings")
        .args(["set", schema().name, key, gvariant])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_desktop_gets_the_background_keys_it_draws() {
        let gnome = schema_for(false, false);
        assert_eq!(gnome.name, GNOME_SCHEMA);
        assert!(gnome.uri && gnome.keys.contains(&"picture-uri-dark"));
        let cinnamon = schema_for(true, false);
        assert_eq!(
            (cinnamon.name, cinnamon.keys),
            (CINNAMON_SCHEMA, &["picture-uri"][..])
        );
        // MATE takes a bare path; a URI there is a picture Caja cannot find.
        let mate = schema_for(false, true);
        assert_eq!(
            (mate.name, mate.keys, mate.uri),
            (MATE_SCHEMA, &["picture-filename"][..], false)
        );
    }
}

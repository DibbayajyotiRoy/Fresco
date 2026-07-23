//! Safe-ish wrapper around one mpv instance embedded in an X11 window.

use anyhow::{anyhow, Result};

use crate::config::{Fit, Scaling, Wallpaper};

use super::ffi::{fns, MpvHandle};

pub struct Player {
    handle: MpvHandle,
}

// SAFETY: mpv's client API is thread-safe; the handle is owned uniquely.
unsafe impl Send for Player {}

impl Player {
    /// Create + initialize an mpv instance rendering into X11 window `wid`,
    /// configured for the given wallpaper, scaling preference, and frame-rate
    /// cap (`framerate` fps, or 0 for the source's original rate).
    pub fn new(
        wid: u32,
        wallpaper: &Wallpaper,
        scaling: Scaling,
        framerate: u16,
    ) -> Result<Player> {
        let f = fns()?;
        let handle = f.create();
        if handle.is_null() {
            return Err(anyhow!("mpv_create returned null"));
        }

        // ── Options that must be set before initialize ──
        let opts: &[(&str, &str)] = &[
            ("wid", &wid.to_string()),
            ("vo", "gpu"),
            // Clockwise rotation in degrees; applied before crop (zoom/pan).
            ("video-rotate", &(wallpaper.rotation % 360).to_string()),
            // Rotated video + NATIVE hw surfaces (vaapi/nvdec) hits driver
            // bugs that corrupt chroma on some stacks; copy-back keeps decode
            // on the GPU but rotates ordinary uploaded textures instead.
            (
                "hwdec",
                if wallpaper.rotation.is_multiple_of(360) {
                    "auto-safe"
                } else {
                    "auto-copy"
                },
            ),
            ("profile", "low-latency"), // small demuxer queues; we override caches below
            ("image-display-duration", "inf"),
            ("osc", "no"),
            ("osd-level", "0"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("input-cursor", "no"),
            ("cursor-autohide", "no"),
            ("stop-screensaver", "no"), // never inhibit screen blank/lock
            ("background", "#000000"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
            ("config", "no"), // never read the user's ~/.config/mpv/mpv.conf
            ("terminal", "no"),
            // Memory: a looping wallpaper doesn't need big read-ahead caches.
            // Small caps cut tens of MB of RSS with no visible effect on a loop.
            ("cache", "no"),
            ("demuxer-max-bytes", "16MiB"),
            ("demuxer-max-back-bytes", "4MiB"),
            ("demuxer-readahead-secs", "1"),
        ];
        // SAFETY: `handle` is the live, non-null handle just created above and
        // is not destroyed until `Drop`. This invariant holds for every mpv
        // call in this module.
        for (k, v) in opts {
            unsafe { f.set_option(handle, k, v) };
        }

        // Looping mode depends on playlist vs single file.
        let is_playlist =
            matches!(wallpaper.kind, crate::config::Kind::Playlist) && wallpaper.paths.len() > 1;
        // SAFETY: `handle` is live (see above) for all calls below.
        unsafe {
            if is_playlist {
                f.set_option(handle, "loop-playlist", "inf");
                f.set_option(handle, "loop-file", "no");
            } else {
                f.set_option(handle, "loop-file", "inf");
            }

            // Audio. When muted (the default) skip audio entirely — no decoder,
            // buffers, output device, or thread — which trims RAM noticeably.
            f.set_option(handle, "mute", if wallpaper.mute { "yes" } else { "no" });
            f.set_option(handle, "volume", &wallpaper.volume.to_string());
            if wallpaper.mute {
                f.set_option(handle, "aid", "no");
                // No audio clock → smoother looping.
                f.set_option(handle, "video-sync", "display-resample");
            }

            // Visually-correct scaling on every profile (ROADMAP 1.8.2): mpv's
            // default bilinear + no dithering visibly softens 8K→4K downscales
            // and bands gradients — the "quality drops on big screens"
            // complaint. correct/linear downscaling + dithering fix that;
            // spline36/mitchell is near gpu-hq quality at a fraction of the cost.
            //
            // A custom CHROMA scaler combined with `video-rotate` corrupts the
            // chroma planes into a green cast (repro: rotated capture means
            // RGB 90,142,64 vs neutral 126,129,127 without cscale — see
            // tests/fidelity). Luma scalers are unaffected, so on rotated
            // video we keep mpv's default chroma path.
            let rotated = !wallpaper.rotation.is_multiple_of(360);
            f.set_option(handle, "correct-downscaling", "yes");
            f.set_option(handle, "linear-downscaling", "yes");
            f.set_option(handle, "dither-depth", "auto");
            if matches!(scaling, Scaling::High) {
                f.set_option(handle, "scale", "lanczos");
                if !rotated {
                    f.set_option(handle, "cscale", "lanczos");
                }
                f.set_option(handle, "dscale", "lanczos");
            } else {
                f.set_option(handle, "scale", "spline36");
                if !rotated {
                    f.set_option(handle, "cscale", "spline36");
                }
                f.set_option(handle, "dscale", "mitchell");
            }

            // Fit mode.
            apply_fit_options(f, handle, wallpaper.fit);

            // Frame-rate cap: the `fps` filter is the sole `vf` occupant (crop
            // and rotation use properties), so we can set it outright.
            if let Some(vf) = crate::config::fps_filter(framerate) {
                f.set_option(handle, "vf", &vf);
            }

            if f.initialize(handle) < 0 {
                f.terminate_destroy(handle);
                return Err(anyhow!("mpv_initialize failed"));
            }
        }

        let player = Player { handle };
        // Crop is applied as a runtime property (post-init) so we can change it live.
        player.apply_crop(wallpaper);
        player.load(wallpaper)?;
        Ok(player)
    }

    /// Load (or replace) the media described by `wallpaper`.
    pub fn load(&self, wallpaper: &Wallpaper) -> Result<()> {
        let f = fns()?;
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        match wallpaper.kind {
            crate::config::Kind::Playlist if wallpaper.paths.len() > 1 => {
                let mut paths: Vec<&std::path::Path> =
                    wallpaper.paths.iter().map(|p| p.as_path()).collect();
                if wallpaper.shuffle {
                    shuffle_in_place(&mut paths);
                }
                if let Some(first) = paths.first() {
                    unsafe {
                        f.command(
                            self.handle,
                            &["loadfile", &first.to_string_lossy(), "replace"],
                        )
                    };
                }
                for p in paths.iter().skip(1) {
                    unsafe {
                        f.command(self.handle, &["loadfile", &p.to_string_lossy(), "append"])
                    };
                }
            }
            _ => {
                if let Some(path) = wallpaper.effective_path() {
                    unsafe {
                        f.command(
                            self.handle,
                            &["loadfile", &path.to_string_lossy(), "replace"],
                        )
                    };
                }
            }
        }
        Ok(())
    }

    /// Load a single file immediately (used by the slideshow timer).
    pub fn load_path(&self, path: &std::path::Path) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid for the lifetime of this Player.
            unsafe {
                f.command(
                    self.handle,
                    &["loadfile", &path.to_string_lossy(), "replace"],
                )
            };
        }
    }

    /// Apply (or clear) the crop via VO-side zoom/pan (keeps hwdec zero-copy).
    pub fn apply_crop(&self, wallpaper: &Wallpaper) {
        let Ok(f) = fns() else { return };
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        unsafe {
            match wallpaper.crop.and_then(|c| c.sanitized()) {
                Some(crop) => {
                    let (zoom, pan_x, pan_y) = crop.to_mpv_zoom_pan();
                    f.set_property(self.handle, "video-zoom", &format!("{zoom:.6}"));
                    f.set_property(self.handle, "video-pan-x", &format!("{pan_x:.6}"));
                    f.set_property(self.handle, "video-pan-y", &format!("{pan_y:.6}"));
                    // Cropping should fill exactly; disable cover panscan to avoid double-zoom.
                    f.set_property(self.handle, "panscan", "0.0");
                }
                None => {
                    f.set_property(self.handle, "video-zoom", "0");
                    f.set_property(self.handle, "video-pan-x", "0");
                    f.set_property(self.handle, "video-pan-y", "0");
                }
            }
        }
    }

    /// Set the VO gamma (-100..=100). Driving it to -100 yields true black on
    /// the GPU; used by the fade-through-black slideshow transition.
    pub fn set_gamma(&self, gamma: i32) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid for the lifetime of this Player.
            unsafe { f.set_property(self.handle, "gamma", &gamma.to_string()) };
        }
    }

    /// Set VO zoom/pan directly (composed on top of any crop's base values);
    /// used by the slide and Ken Burns slideshow transitions.
    pub fn set_zoom_pan(&self, zoom: f64, pan_x: f64, pan_y: f64) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid for the lifetime of this Player.
            unsafe {
                f.set_property(self.handle, "video-zoom", &format!("{zoom:.6}"));
                f.set_property(self.handle, "video-pan-x", &format!("{pan_x:.6}"));
                f.set_property(self.handle, "video-pan-y", &format!("{pan_y:.6}"));
            }
        }
    }

    /// Change rotation at runtime — the scheduled swap replaces media in place
    /// (no respawn), so the previous wallpaper's rotation must not leak onto
    /// the next one. Re-applies the rotated-video constraints from `new`:
    /// copy-back hwdec and mpv's default chroma scaler (green-cast bug above).
    pub fn set_rotation(&self, rotation: u16, scaling: Scaling) {
        let Ok(f) = fns() else { return };
        let rotated = !rotation.is_multiple_of(360);
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        unsafe {
            f.set_property(self.handle, "video-rotate", &(rotation % 360).to_string());
            f.set_property(
                self.handle,
                "hwdec",
                if rotated { "auto-copy" } else { "auto-safe" },
            );
            let cscale = match (rotated, scaling) {
                (true, _) => "bilinear", // mpv's default chroma path
                (false, Scaling::High) => "lanczos",
                (false, _) => "spline36",
            };
            f.set_property(self.handle, "cscale", cscale);
        }
    }

    /// Change the frame-rate cap at runtime (scheduled swaps replace media in
    /// place, so a per-wallpaper cap must not leak onto the next wallpaper).
    /// The `fps` filter is our only `vf`, so an empty value clears the cap.
    pub fn set_framerate(&self, framerate: u16) {
        let Ok(f) = fns() else { return };
        let vf = crate::config::fps_filter(framerate).unwrap_or_default();
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        unsafe { f.set_property(self.handle, "vf", &vf) };
    }

    /// Seek to an absolute position (seconds). Used to keep clones of the same
    /// video in lockstep across monitors.
    pub fn set_time_pos(&self, secs: f64) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid for the lifetime of this Player.
            unsafe { f.set_property(self.handle, "time-pos", &format!("{secs:.3}")) };
        }
    }

    pub fn set_paused(&self, paused: bool) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid for the lifetime of this Player.
            unsafe { f.set_property(self.handle, "pause", if paused { "yes" } else { "no" }) };
        }
    }

    /// Re-select the file's audio track and unmute. Recovery for the case where
    /// mpv dropped the track because no audio server was reachable at load time
    /// (cold boot: we start before PipeWire). Setting `aid=auto` does NOT
    /// recover a dropped track; an explicit track id does — verified against
    /// mpv 0.3x. Returns false when the file has no audio track at all (there
    /// is nothing to restore; callers must stop retrying).
    pub fn try_restore_audio(&self, volume: u8) -> bool {
        let Ok(f) = fns() else { return true };
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        let Some(tracks) = (unsafe { f.get_property(self.handle, "track-list") }) else {
            return true; // transient read failure — worth another attempt
        };
        let Some(id) = first_audio_track_id(&tracks) else {
            return false;
        };
        // SAFETY: as above.
        unsafe {
            f.set_property(self.handle, "aid", &id.to_string());
            f.set_property(self.handle, "mute", "no");
            f.set_property(self.handle, "volume", &volume.to_string());
        }
        true
    }

    /// Live audio state: (audio track selected, muted, volume). The track flag
    /// is the ground truth for "will this ever make sound" — mpv reports
    /// `aid=no` both for our muted-entry optimization and after it dropped the
    /// track because no audio server was reachable at load time.
    pub fn audio_status(&self) -> Option<(bool, bool, u8)> {
        let f = fns().ok()?;
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        let (aid, mute, volume) = unsafe {
            (
                f.get_property(self.handle, "aid")?,
                f.get_property(self.handle, "mute")?,
                f.get_property(self.handle, "volume")?,
            )
        };
        let track = aid != "no" && aid != "false";
        let muted = mute == "yes" || mute == "true";
        let vol = volume.trim().parse::<f64>().ok()?.round().clamp(0.0, 100.0) as u8;
        Some((track, muted, vol))
    }

    /// Raise demuxer read-ahead for high-resolution sources. The spawn
    /// defaults (16MiB, tuned for low RSS on 1080p loops) hold ~2s of a
    /// 50Mbps 4K/8K file and can starve the decoder into stutter — quality
    /// must never silently degrade to save RAM (ROADMAP 1.8.5).
    pub fn raise_demuxer_cache(&self) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid for the lifetime of this Player.
            unsafe {
                f.set_property(self.handle, "demuxer-max-bytes", "64MiB");
                f.set_property(self.handle, "demuxer-max-back-bytes", "8MiB");
                f.set_property(self.handle, "demuxer-readahead-secs", "2");
            }
        }
    }

    /// Decode-honesty snapshot: (source width, height, bit depth, dropped
    /// frames). Lets Status/doctor explain quality problems (software decode
    /// at 4K+, decoder drops) instead of leaving them silent.
    pub fn video_status(&self) -> Option<(u32, u32, u8, u64)> {
        let f = fns().ok()?;
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        let (w, h, pf, drops) = unsafe {
            (
                f.get_property(self.handle, "video-params/w")?,
                f.get_property(self.handle, "video-params/h")?,
                f.get_property(self.handle, "video-params/pixelformat")
                    .unwrap_or_default(),
                f.get_property(self.handle, "frame-drop-count")
                    .unwrap_or_default(),
            )
        };
        Some((
            w.trim().parse().ok()?,
            h.trim().parse().ok()?,
            pixelformat_bit_depth(&pf),
            drops.trim().parse().unwrap_or(0),
        ))
    }

    /// Active hardware decoder, e.g. "nvdec", "vaapi", or "no" (software).
    pub fn hwdec_current(&self) -> Option<String> {
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        unsafe { fns().ok()?.get_property(self.handle, "hwdec-current") }
    }

    /// Current playback position in seconds, if known. Used to detect a cold-boot
    /// VO stall (position frozen while not paused).
    pub fn time_pos(&self) -> Option<f64> {
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        let s = unsafe { fns().ok()?.get_property(self.handle, "time-pos") }?;
        s.trim().parse().ok()
    }

    /// True if mpv reports an idle/failed state (no file loaded).
    pub fn load_failed(&self) -> bool {
        // After a failed loadfile, mpv goes idle: "idle-active" == "yes".
        // SAFETY: `self.handle` is valid for the lifetime of this Player.
        matches!(
            fns().ok().and_then(|f| unsafe { f.get_property(self.handle, "idle-active") }),
            Some(v) if v == "yes"
        )
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid and destroyed exactly once here.
            unsafe { f.terminate_destroy(self.handle) };
        }
    }
}

fn apply_fit_options(f: &super::ffi::MpvFns, h: MpvHandle, fit: Fit) {
    // SAFETY: caller (`Player::new`) passes the live handle being initialized.
    unsafe {
        match fit {
            Fit::Cover => {
                f.set_option(h, "keepaspect", "yes");
                f.set_option(h, "panscan", "1.0");
            }
            Fit::Contain => {
                f.set_option(h, "keepaspect", "yes");
                f.set_option(h, "panscan", "0.0");
            }
            Fit::Stretch => {
                f.set_option(h, "keepaspect", "no");
            }
        }
    }
}

/// Deterministic-enough shuffle without pulling in `rand`. Uses the system
/// nanosecond clock as a seed; quality is irrelevant for wallpaper ordering.
fn shuffle_in_place<T>(v: &mut [T]) {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b9)
        | 1;
    for i in (1..v.len()).rev() {
        // xorshift
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let j = (seed % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

/// Bit depth implied by an mpv pixelformat name ("yuv420p10le" → 10,
/// "p010" → 10, "nv12" → 8). Depth always appears as a `p`-prefixed suffix
/// (planar depth marker or semi-planar pNNN family); bare formats are 8-bit.
pub(crate) fn pixelformat_bit_depth(pf: &str) -> u8 {
    for (needle, depth) in [
        ("p016", 16u8),
        ("p012", 12),
        ("p010", 10),
        ("p16", 16),
        ("p12", 12),
        ("p10", 10),
    ] {
        if pf.contains(needle) {
            return depth;
        }
    }
    8
}

/// First audio track id from an mpv `track-list` JSON dump, or None when the
/// file has no audio track. Shared by both backends' audio recovery.
pub(crate) fn first_audio_track_id(track_list_json: &str) -> Option<i64> {
    let tracks: serde_json::Value = serde_json::from_str(track_list_json).ok()?;
    tracks.as_array()?.iter().find_map(|t| {
        (t.get("type")?.as_str()? == "audio")
            .then(|| t.get("id")?.as_i64())
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::first_audio_track_id;

    #[test]
    fn finds_first_audio_track() {
        let tl = r#"[
            {"id":1,"type":"video","selected":true},
            {"id":1,"type":"audio","codec":"aac"},
            {"id":2,"type":"audio","codec":"ac3"}
        ]"#;
        assert_eq!(first_audio_track_id(tl), Some(1));
    }

    #[test]
    fn pixelformat_depths() {
        use super::pixelformat_bit_depth;
        assert_eq!(pixelformat_bit_depth("yuv420p"), 8);
        assert_eq!(pixelformat_bit_depth("nv12"), 8);
        assert_eq!(pixelformat_bit_depth("yuv420p10le"), 10);
        assert_eq!(pixelformat_bit_depth("p010"), 10);
        assert_eq!(pixelformat_bit_depth("yuv422p12be"), 12);
        assert_eq!(pixelformat_bit_depth("p016"), 16);
        assert_eq!(pixelformat_bit_depth(""), 8);
    }

    #[test]
    fn no_audio_track_means_none() {
        let tl = r#"[{"id":1,"type":"video","selected":true}]"#;
        assert_eq!(first_audio_track_id(tl), None);
        assert_eq!(first_audio_track_id("[]"), None);
        assert_eq!(first_audio_track_id("not json"), None);
    }
}

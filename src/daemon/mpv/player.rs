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
    /// configured for the given wallpaper and scaling preference.
    pub fn new(wid: u32, wallpaper: &Wallpaper, scaling: Scaling) -> Result<Player> {
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
            ("hwdec", "auto-safe"),
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

            // Scaling quality.
            if matches!(scaling, Scaling::High) {
                f.set_option(handle, "scale", "lanczos");
                f.set_option(handle, "cscale", "lanczos");
            }

            // Fit mode.
            apply_fit_options(f, handle, wallpaper.fit);

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

    pub fn set_paused(&self, paused: bool) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid for the lifetime of this Player.
            unsafe { f.set_property(self.handle, "pause", if paused { "yes" } else { "no" }) };
        }
    }

    pub fn set_volume(&self, volume: u8, mute: bool) {
        if let Ok(f) = fns() {
            // SAFETY: `self.handle` is valid for the lifetime of this Player.
            unsafe {
                f.set_property(self.handle, "volume", &volume.to_string());
                f.set_property(self.handle, "mute", if mute { "yes" } else { "no" });
            }
        }
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

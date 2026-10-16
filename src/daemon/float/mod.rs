//! Floating world-clock overlay: an always-on-top, click-through window.
//!
//! `x11` and `wayland` each own one backend; [`start`] picks between them.

mod wayland;
mod x11;

pub struct Overlay {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

/// Try to float the world clock above all windows. Err => caller falls back to the wallpaper widget layer.
///
/// Each backend's `try_start` returns `Err` synchronously when the session
/// cannot float (GNOME Wayland has no layer-shell) and `Ok` once its thread is
/// running; that thread ends when `stop` is set.
pub fn start(cfg: &crate::config::WorldClock) -> anyhow::Result<Overlay> {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let set = |key: &str| std::env::var_os(key).is_some_and(|v| !v.is_empty());
    let join = if set("WAYLAND_DISPLAY") {
        wayland::try_start(cfg, stop.clone())?
    } else if set("DISPLAY") {
        x11::try_start(cfg, stop.clone())?
    } else {
        anyhow::bail!("neither WAYLAND_DISPLAY nor DISPLAY is set");
    };
    Ok(Overlay {
        stop,
        join: Some(join),
    })
}

/// Dropping the overlay stops its thread and waits for it, which takes the
/// window down with it.
impl Drop for Overlay {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

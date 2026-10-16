//! Floating world-clock overlay: an always-on-top, click-through window.
//!
//! `x11` and `wayland` each own one backend; [`start`] picks between them.

#[allow(dead_code)]
mod wayland;
#[allow(dead_code)]
mod x11;

pub struct Overlay {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

/// Try to float the world clock above all windows. Err => caller falls back to the wallpaper widget layer.
pub fn start(_cfg: &crate::config::WorldClock) -> anyhow::Result<Overlay> {
    anyhow::bail!("floating overlay not implemented")
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

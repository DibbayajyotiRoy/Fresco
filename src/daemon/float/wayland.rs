pub fn try_start(
    _cfg: &crate::config::WorldClock,
    _stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    anyhow::bail!("not implemented")
}

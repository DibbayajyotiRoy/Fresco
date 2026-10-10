fn main() {
    // Warnings on by default: the GUI's `log::warn!`s (a missing GTK media
    // backend, no ffmpeg) name the fix for a feature that silently does nothing,
    // and `env_logger::init()` alone only shows errors.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    // Pick the UI language before anything builds a widget: `tr` hands out
    // `&'static str` for the process lifetime, so the catalog has to be settled
    // first. A config that fails to load is not worth failing over here — the
    // default is "follow the desktop locale", which is what an unreadable
    // config should fall back to anyway.
    fresco::i18n::init(fresco::config::Config::load().unwrap_or_default().language);
    let args: Vec<String> = std::env::args().collect();
    // CLI subcommands (doctor/status/logs) run without launching the GUI.
    if let Some(code) = fresco::cli::dispatch(&args) {
        std::process::exit(code);
    }
    let app = fresco::gui::FrescoApplication::new();
    std::process::exit(app.run(&args));
}

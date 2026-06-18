fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    // CLI subcommands (doctor/status/logs) run without launching the GUI.
    if let Some(code) = fresco::cli::dispatch(&args) {
        std::process::exit(code);
    }
    let app = fresco::gui::FrescoApplication::new();
    std::process::exit(app.run(&args));
}

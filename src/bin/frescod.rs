//! Fresco wallpaper daemon binary.
//!
//! Usage:
//!   frescod              run the daemon (reads ~/.config/fresco/config.toml)
//!   frescod --once FILE  render one file on every monitor until Ctrl-C (spike)
//!   frescod --check      print hardware/decode diagnostics and exit

use std::path::PathBuf;

fn main() {
    init_logging();
    let args: Vec<String> = std::env::args().collect();

    let result = match args.get(1).map(String::as_str) {
        Some("--check") => {
            fresco::daemon::check();
            return;
        }
        Some("--once") => match args.get(2) {
            Some(file) => fresco::daemon::run_once(PathBuf::from(file)),
            None => {
                eprintln!("usage: frescod --once <file>");
                std::process::exit(2);
            }
        },
        Some(other) => {
            eprintln!("frescod: unknown argument '{other}'");
            std::process::exit(2);
        }
        None => fresco::daemon::run(),
    };

    if let Err(e) = result {
        log::error!("{e:#}");
        eprintln!("frescod: {e:#}");
        std::process::exit(1);
    }
}

/// Log to stderr and append to ~/.local/state/fresco/frescod.log.
fn init_logging() {
    use std::io::Write;
    if let Some(dir) = dirs::state_dir().or_else(dirs::data_local_dir) {
        let log_dir = dir.join("fresco");
        std::fs::create_dir_all(&log_dir).ok();
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("frescod.log"))
        {
            let _ = writeln!(&file, "--- frescod start ---");
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .target(env_logger::Target::Pipe(Box::new(file)))
                .init();
            return;
        }
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
}

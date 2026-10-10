//! Fresco wallpaper daemon binary.
//!
//! Usage:
//!   frescod              run the daemon (reads ~/.config/fresco/config.toml)
//!   frescod --once FILE  render one file on every monitor until Ctrl-C (spike)
//!   frescod --check      print hardware/decode diagnostics and exit
//!   frescod --version    print the version and exit (also -V, -v, version)
//!   frescod --saver      X11 lock-screen saver module; run by a screensaver
//!                        host (xsecurelock's XSECURELOCK_SAVER, or a
//!                        mate-screensaver/xfce4-screensaver theme), never by
//!                        a person — see `daemon::saver` for the full contract

use std::path::PathBuf;

fn main() {
    // Before logging/i18n init: asking for the version must not touch the log
    // file or the config.
    if let Some("--version" | "-V" | "-v" | "version") = std::env::args().nth(1).as_deref() {
        println!("frescod {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    init_logging();
    // The daemon raises desktop notifications, so it needs the same catalog the
    // GUI uses. Log output stays English — it is read by us, not by the user.
    fresco::i18n::init(fresco::config::Config::load().unwrap_or_default().language);
    let args: Vec<String> = std::env::args().collect();

    let result = match args.get(1).map(String::as_str) {
        Some("--check") => {
            fresco::daemon::check();
            return;
        }
        Some("--saver") => {
            // Own exit-code contract (see `daemon::saver`'s doc comment), not
            // the shared `Result` handling below — mirrors `--check` above in
            // sidestepping it. Any further argv entries are ignored rather
            // than rejected: xsecurelock invokes a saver with `-root`
            // (`saver_child.c`, verified from its source) for XScreenSaver
            // "hack" compatibility, which this binary has no use for and
            // must not choke on.
            std::process::exit(fresco::daemon::saver::run());
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

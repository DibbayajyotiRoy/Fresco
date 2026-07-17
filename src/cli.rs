//! Polished command-line surface for the `fresco` binary.
//!
//! `fresco doctor` / `fresco status` / `fresco logs` run without launching the
//! GUI. They reflect the running daemon over IPC and the detected session
//! capability — the user never needs to know about layer-shell, EGL, or
//! mpvpaper. Anything Fresco can't do is reported as a plain-language hint, not
//! a stack trace.

use std::path::PathBuf;
use std::process::Command;

use crate::capability::{detect, Capability};
use crate::config::Config;
use crate::ipc::{request, Request, Response, StatusReply};

const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Route the command line. Returns `Some(exit_code)` when this is a CLI
/// invocation (the caller must exit with it), or `None` to launch the GUI.
///
/// A typo'd subcommand (`fresco foo`) is a CLI error — it must NOT fall through
/// to GTK. Genuine toolkit options (`fresco --gapplication-service`, `--display`)
/// are left for the GUI's option parser, so D-Bus activation / launch still work.
pub fn dispatch(args: &[String]) -> Option<i32> {
    match args.get(1).map(String::as_str) {
        // No subcommand → launch the GUI.
        None => None,
        Some("doctor") => Some(doctor()),
        Some("status") => Some(status()),
        Some("logs") => Some(logs(args.get(2).map(String::as_str))),
        Some("-h") | Some("--help") | Some("help") => {
            print_help();
            Some(0)
        }
        // Toolkit options are not ours — let the GUI parse them.
        Some(opt) if opt.starts_with('-') => None,
        // An unrecognized word is a CLI typo, not a GUI launch.
        Some(other) => {
            eprintln!("error: unknown command '{other}'");
            eprintln!("Run `fresco --help` for usage.");
            Some(2)
        }
    }
}

fn print_help() {
    println!(
        "Fresco — live wallpapers for Linux\n\n\
         Usage:\n  \
         fresco            Launch the app\n  \
         fresco doctor     Show session, backend, and health diagnostics\n  \
         fresco status     Show the running wallpaper's status\n  \
         fresco logs [N]   Show the last N daemon log lines (default 50)\n  \
         fresco --help     Show this help"
    );
}

// ── doctor ───────────────────────────────────────────────────────────────────

fn doctor() -> i32 {
    let cap = detect();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
    let st = daemon_status();

    println!("{BOLD}Fresco doctor{RESET}\n");
    println!("  Session       {}", session_label(cap));
    println!("  Compositor    {desktop}");
    println!("  Backend       {}", backend_label(cap));
    if let Some(gpu) = gpu_name() {
        println!("  GPU           {gpu}");
    }
    if let Some(n) = st.as_ref().map(|s| s.monitors.len()).filter(|n| *n > 0) {
        println!("  Outputs       {n}");
    }

    println!("\n{BOLD}Checks{RESET}");
    let mut problems = 0u32;

    check("Session detected", true, "", &mut problems);
    match cap {
        Capability::X11 | Capability::WaylandLayerShell => {
            check("Live wallpaper supported", true, "", &mut problems)
        }
        Capability::WaylandGnomeStatic => warn(
            "Live wallpaper supported",
            "GNOME Wayland uses a static frame",
        ),
    }
    check(
        "Hardware acceleration",
        hwaccel_available(),
        "install mesa-va-drivers / intel-media-va-driver",
        &mut problems,
    );
    // mpvpaper only matters on layer-shell Wayland (it's how we render there).
    if matches!(cap, Capability::WaylandLayerShell) {
        match crate::mpvpaper_resolved() {
            Some(p) => println!(
                "  {GREEN}✓{RESET} mpvpaper available {DIM}({}){RESET}",
                p.display()
            ),
            None => {
                problems += 1;
                match crate::mpvpaper_broken() {
                    Some(p) => println!(
                        "  {RED}✗{RESET} mpvpaper available {DIM}({} exists but fails to load — likely a libmpv version mismatch; update Fresco or install the matching libmpv){RESET}",
                        p.display()
                    ),
                    None => println!(
                        "  {RED}✗{RESET} mpvpaper available {DIM}install or build mpvpaper, or use the .deb/Flatpak release which bundles it{RESET}"
                    ),
                }
            }
        }
    }
    let configured = Config::load()
        .map(|c| {
            c.enabled && (c.wallpaper.effective_path().is_some() || !c.wallpaper.paths.is_empty())
        })
        .unwrap_or(false);
    if configured {
        check("Wallpaper configured", true, "", &mut problems);
    } else {
        warn("Wallpaper configured", "none yet — open Fresco to set one");
    }

    println!();
    if problems == 0 {
        println!("{GREEN}System healthy{RESET}");
        0
    } else {
        println!("{YELLOW}{problems} issue(s) found{RESET}");
        1
    }
}

// ── status ───────────────────────────────────────────────────────────────────

fn status() -> i32 {
    match daemon_status() {
        Some(s) => {
            println!("{BOLD}Fresco{RESET}");
            println!("  Backend     {}", backend_label(detect()));
            println!("  Wallpaper   {}", s.wallpaper.as_deref().unwrap_or("—"));
            if !s.monitors.is_empty() {
                println!("  Outputs     {}", s.monitors.len());
            }
            println!("  Decode      {}", decode_label(s.hwdec.as_deref()));
            if let (Some(w), Some(h)) = (s.source_w, s.source_h) {
                let depth = s
                    .bit_depth
                    .map(|d| format!(" · {d}-bit"))
                    .unwrap_or_default();
                println!("  Source      {w}x{h}{depth}");
            }
            if let Some(n) = s.dropped_frames {
                if n > 0 {
                    println!("  Dropped     {n} frames");
                }
            }
            println!("  Memory      {} MB", s.rss_mb);
            println!("  Paused      {}", if s.paused { "yes" } else { "no" });
            // Decode honesty: software-decoding ≥4K is the classic silent cause
            // of stutter/artifacts — name it instead of leaving it a mystery.
            let software = matches!(s.hwdec.as_deref(), None | Some("no") | Some(""));
            if software && s.source_h.unwrap_or(0) >= 2160 {
                println!(
                    "  {YELLOW}Note        this source is ≥4K and your GPU is not \
                     hardware-decoding it (codec/size unsupported) — expect high CPU \
                     and possible dropped frames{RESET}"
                );
            }
            if let Some(e) = s.error {
                println!("  {YELLOW}Note        {e}{RESET}");
            }
        }
        None => {
            println!("Fresco isn't running. Open the app or set a wallpaper to start it.");
        }
    }
    0
}

// ── logs ─────────────────────────────────────────────────────────────────────

fn logs(arg: Option<&str>) -> i32 {
    let path = daemon_log_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("No daemon log yet at {}", path.display());
        return 0;
    };
    let n: usize = arg.and_then(|a| a.parse().ok()).unwrap_or(50);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    for line in &lines[start..] {
        println!("{line}");
    }
    0
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn daemon_status() -> Option<StatusReply> {
    match request(&Request::Status) {
        Ok(Response::Status(s)) => Some(s),
        _ => None,
    }
}

fn session_label(cap: Capability) -> &'static str {
    match cap {
        Capability::X11 => "X11",
        Capability::WaylandLayerShell | Capability::WaylandGnomeStatic => "Wayland",
    }
}

fn backend_label(cap: Capability) -> String {
    match cap {
        Capability::X11 => "X11 (embedded mpv)".into(),
        Capability::WaylandGnomeStatic => "static frame (GNOME Wayland)".into(),
        Capability::WaylandLayerShell => "mpvpaper (layer-shell)".into(),
    }
}

fn decode_label(hwdec: Option<&str>) -> String {
    match hwdec {
        Some("no") | None => "software".into(),
        Some(x) => format!("hardware ({x})"),
    }
}

/// Print a check line. `fail_hint` (a remedy) is shown only when the check
/// fails — never on a passing line, where it would read like unwanted advice.
fn check(label: &str, ok: bool, fail_hint: &str, problems: &mut u32) {
    if ok {
        println!("  {GREEN}✓{RESET} {label}");
    } else {
        *problems += 1;
        if fail_hint.is_empty() {
            println!("  {RED}✗{RESET} {label}");
        } else {
            println!("  {RED}✗{RESET} {label} {DIM}{fail_hint}{RESET}");
        }
    }
}

fn warn(label: &str, hint: &str) {
    println!("  {YELLOW}⚠{RESET} {label} {DIM}{hint}{RESET}");
}

fn hwaccel_available() -> bool {
    std::path::Path::new("/dev/dri/renderD128").exists() || which("vainfo")
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

fn gpu_name() -> Option<String> {
    let out = Command::new("sh")
        .arg("-c")
        .arg("lspci | grep -Ei 'vga|3d|display' | sed 's/.*: //'")
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

fn daemon_log_path() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fresco")
        .join("frescod.log")
}

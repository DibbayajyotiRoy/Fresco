//! Fresco wallpaper daemon: owns X11 desktop windows and embedded mpv players,
//! reconciles them against the config, and serves IPC control commands.

mod control;
pub mod monitors;
pub mod mpv;
mod overview;
mod x11win;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Screen;
use x11rb::rust_connection::RustConnection;

use crate::config::{Config, Kind, Scaling, Wallpaper};
use crate::ipc::{Request, Response, StatusReply};

use monitors::Monitor;
use mpv::Player;
use x11win::{Atoms, WallpaperWindow};

const TICK: Duration = Duration::from_millis(100);
const LOWER_INTERVAL: Duration = Duration::from_secs(2);
const MONITOR_INTERVAL: Duration = Duration::from_secs(3);
const BATTERY_INTERVAL: Duration = Duration::from_secs(30);

struct Slideshow {
    images: Vec<PathBuf>,
    idx: usize,
    interval: Duration,
    last_advance: Instant,
}

struct Renderer {
    window: WallpaperWindow,
    player: Player,
    slideshow: Option<Slideshow>,
}

pub struct Daemon {
    conn: RustConnection,
    screen_num: usize,
    atoms: Atoms,
    renderers: Vec<Renderer>,
    config: Config,
    user_paused: bool,
    battery_paused: bool,
    last_lower: Instant,
    last_monitor_check: Instant,
    last_battery_check: Instant,
    monitors: Vec<Monitor>,
}

impl Daemon {
    pub fn new(config: Config) -> Result<Daemon> {
        let (conn, screen_num) =
            x11rb::connect(None).context("connecting to X11 (is DISPLAY set?)")?;
        let atoms = Atoms::new(&conn)?.reply()?;
        Ok(Daemon {
            conn,
            screen_num,
            atoms,
            renderers: Vec::new(),
            config,
            user_paused: false,
            battery_paused: false,
            last_lower: Instant::now(),
            last_monitor_check: Instant::now(),
            last_battery_check: Instant::now() - BATTERY_INTERVAL,
            monitors: Vec::new(),
        })
    }

    fn screen(&self) -> Screen {
        self.conn.setup().roots[self.screen_num].clone()
    }

    /// Tear down all renderers and rebuild them from the current config and the
    /// current monitor layout. Reveals the native wallpaper momentarily.
    fn rebuild(&mut self) -> Result<()> {
        self.teardown_renderers();
        let screen = self.screen();
        self.monitors = monitors::list_monitors(&self.conn, screen.root)?;

        for monitor in self.monitors.clone() {
            let wallpaper = self.config.wallpaper_for(&monitor.connector).clone();
            if wallpaper.effective_path().is_none() && wallpaper.kind != Kind::Slideshow {
                continue; // nothing configured for this monitor
            }
            match Self::make_renderer(
                &self.conn,
                &screen,
                &self.atoms,
                &monitor,
                &wallpaper,
                self.config.scaling,
            ) {
                Ok(r) => {
                    if self.user_paused || self.battery_paused {
                        r.player.set_paused(true);
                    }
                    self.renderers.push(r);
                }
                Err(e) => log::error!("renderer for {} failed: {e}", monitor.connector),
            }
        }
        Ok(())
    }

    fn make_renderer(
        conn: &RustConnection,
        screen: &Screen,
        atoms: &Atoms,
        monitor: &Monitor,
        wallpaper: &Wallpaper,
        scaling: Scaling,
    ) -> Result<Renderer> {
        let window = WallpaperWindow::create(conn, screen, atoms, monitor)?;
        let player = Player::new(window.window, wallpaper, scaling)?;

        let slideshow = if wallpaper.kind == Kind::Slideshow {
            wallpaper.slideshow.as_ref().map(|s| {
                let images = list_images(&s.folder);
                if let Some(first) = images.first() {
                    player.load_path(first);
                }
                Slideshow {
                    images,
                    idx: 0,
                    interval: Duration::from_secs(s.interval_s.max(2)),
                    last_advance: Instant::now(),
                }
            })
        } else {
            None
        };

        Ok(Renderer {
            window,
            player,
            slideshow,
        })
    }

    /// Main event loop. Returns when a Stop command (or signal) is received.
    pub fn run(&mut self) -> Result<()> {
        let commands = control::start_server()?;
        self.rebuild()?;
        overview::apply(&self.config.wallpaper);
        log::info!("frescod started with {} renderer(s)", self.renderers.len());

        loop {
            while let Ok((req, reply)) = commands.try_recv() {
                let is_stop = matches!(req, Request::Stop);
                let resp = self.handle_request(req);
                let _ = reply.send(resp);
                if is_stop {
                    self.shutdown();
                    return Ok(());
                }
            }

            // Drain X11 events so the queue can't grow unbounded. We must NOT
            // re-lower in response: lowering emits a ConfigureNotify on our own
            // window, which would re-enter and storm the compositor (laptop
            // freeze). The periodic re-lower below handles stacking instead.
            while let Ok(Some(_)) = self.conn.poll_for_event() {}

            let now = Instant::now();
            if now.duration_since(self.last_lower) >= LOWER_INTERVAL {
                self.lower_all();
                self.last_lower = now;
            }
            if now.duration_since(self.last_monitor_check) >= MONITOR_INTERVAL {
                self.check_hotplug();
                self.last_monitor_check = now;
            }
            if now.duration_since(self.last_battery_check) >= BATTERY_INTERVAL {
                self.check_battery();
                self.last_battery_check = now;
            }
            self.advance_slideshows(now);

            std::thread::sleep(TICK);
        }
    }

    fn handle_request(&mut self, req: Request) -> Response {
        match req {
            Request::Apply => {
                self.config = Config::load().unwrap_or_else(|_| self.config.clone());
                match self.rebuild() {
                    Ok(_) => {
                        overview::apply(&self.config.wallpaper);
                        Response::Ok
                    }
                    Err(e) => Response::Err {
                        message: e.to_string(),
                    },
                }
            }
            Request::Stop => Response::Ok, // teardown happens in run()
            Request::Pause => {
                self.user_paused = true;
                self.apply_pause();
                Response::Ok
            }
            Request::Resume => {
                self.user_paused = false;
                self.apply_pause();
                Response::Ok
            }
            Request::Status => Response::Status(self.status()),
        }
    }

    fn status(&self) -> StatusReply {
        let (cpu, rss) = proc_stats();
        let hwdec = self
            .renderers
            .first()
            .and_then(|r| r.player.hwdec_current());
        let error = self
            .renderers
            .iter()
            .find(|r| r.player.load_failed())
            .map(|r| format!("failed to load media on {}", r.window.connector));
        StatusReply {
            running: true,
            paused: self.user_paused || self.battery_paused,
            hwdec,
            wallpaper: self.describe_wallpaper(),
            cpu_percent: cpu,
            rss_mb: rss,
            monitors: self.monitors.iter().map(|m| m.connector.clone()).collect(),
            error,
        }
    }

    fn describe_wallpaper(&self) -> Option<String> {
        let w = &self.config.wallpaper;
        match w.kind {
            Kind::Video | Kind::Image => w
                .effective_path()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned()),
            Kind::Playlist => Some(format!("Playlist ({} items)", w.paths.len())),
            Kind::Slideshow => w
                .slideshow
                .as_ref()
                .and_then(|s| s.folder.file_name())
                .map(|n| format!("Slideshow: {}", n.to_string_lossy())),
        }
    }

    fn apply_pause(&self) {
        let paused = self.user_paused || self.battery_paused;
        for r in &self.renderers {
            r.player.set_paused(paused);
        }
    }

    fn lower_all(&self) {
        for r in &self.renderers {
            let _ = x11win::lower(&self.conn, r.window.window);
        }
        let _ = self.conn.flush();
    }

    /// Tear down all renderers, terminating each mpv instance BEFORE destroying
    /// its X window. mpv's vo=gpu context is bound to the window; destroying the
    /// window first can hang or leak the GPU context (notably on NVIDIA), which
    /// otherwise piles up on every wallpaper change.
    fn teardown_renderers(&mut self) {
        for r in self.renderers.drain(..) {
            let Renderer { window, player, .. } = r;
            drop(player);
            window.destroy(&self.conn);
        }
        let _ = self.conn.flush();
    }

    fn check_hotplug(&mut self) {
        let root = self.screen().root;
        if let Ok(current) = monitors::list_monitors(&self.conn, root) {
            if current != self.monitors {
                log::info!("monitor layout changed → rebuilding");
                let _ = self.rebuild();
            }
        }
    }

    fn check_battery(&mut self) {
        if !self.config.pause_on_battery {
            if self.battery_paused {
                self.battery_paused = false;
                self.apply_pause();
            }
            return;
        }
        let discharging = on_battery();
        if discharging != self.battery_paused {
            self.battery_paused = discharging;
            self.apply_pause();
            log::info!("battery pause = {discharging}");
        }
    }

    fn advance_slideshows(&mut self, now: Instant) {
        for r in &mut self.renderers {
            if let Some(s) = &mut r.slideshow {
                if s.images.len() > 1 && now.duration_since(s.last_advance) >= s.interval {
                    s.idx = (s.idx + 1) % s.images.len();
                    r.player.load_path(&s.images[s.idx]);
                    s.last_advance = now;
                }
            }
        }
    }

    fn shutdown(&mut self) {
        overview::restore();
        self.teardown_renderers();
        std::fs::remove_file(crate::ipc::socket_path()).ok();
        log::info!("frescod stopped");
    }
}

/// List image files in a folder, sorted by name.
fn list_images(folder: &std::path::Path) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase)
                    .as_deref(),
                Some("jpg" | "jpeg" | "png" | "webp" | "bmp" | "tiff" | "gif")
            )
        })
        .collect();
    v.sort();
    v
}

/// Any power-supply reporting "Discharging" means we're on battery.
fn on_battery() -> bool {
    let Ok(dir) = std::fs::read_dir("/sys/class/power_supply") else {
        return false;
    };
    dir.flatten().any(|entry| {
        std::fs::read_to_string(entry.path().join("status"))
            .map(|s| s.trim() == "Discharging")
            .unwrap_or(false)
    })
}

/// (cpu_percent, rss_megabytes) from /proc/self. CPU is left at 0 for now
/// (a meaningful value needs sampling over time); RSS is the key lightweight
/// signal and is reported accurately.
fn proc_stats() -> (f32, u64) {
    let rss_mb = std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
        .and_then(|pages| pages.parse::<u64>().ok())
        .map(|pages| pages * 4096 / 1_048_576)
        .unwrap_or(0);
    (0.0, rss_mb)
}

// ─── Entry points called by frescod.rs ───────────────────────────────────────

/// Normal daemon start: honor `enabled`, guard Wayland, run the loop.
pub fn run() -> Result<()> {
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        anyhow::bail!("Fresco requires an X11 session (Wayland not yet supported)");
    }
    let config = Config::load().unwrap_or_default();
    if !config.enabled {
        // Safety net: if a prior run was killed (not Stopped) it may have left
        // our static frame as the background — put the user's original back.
        overview::restore();
        log::info!("wallpaper disabled (enabled=false) — exiting");
        return Ok(());
    }
    let mut daemon = Daemon::new(config)?;
    daemon.run()
}

/// `--once <file>`: render one file on every monitor until Ctrl-C.
/// Used for the M1 renderer spike; ignores config and IPC.
pub fn run_once(file: PathBuf) -> Result<()> {
    let is_image = file
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "bmp"
            )
        })
        .unwrap_or(false);
    let wallpaper = Wallpaper {
        kind: if is_image { Kind::Image } else { Kind::Video },
        path: Some(file),
        ..Default::default()
    };
    let config = Config {
        wallpaper,
        ..Default::default()
    };

    let mut daemon = Daemon::new(config)?;
    daemon.rebuild()?;
    log::info!(
        "--once: rendering on {} monitor(s); Ctrl-C to quit",
        daemon.renderers.len()
    );
    loop {
        while let Ok(Some(_)) = daemon.conn.poll_for_event() {}
        if Instant::now().duration_since(daemon.last_lower) >= LOWER_INTERVAL {
            daemon.lower_all();
            daemon.last_lower = Instant::now();
        }
        std::thread::sleep(TICK);
    }
}

/// `--check`: print a colored diagnostics table and exit.
pub fn check() {
    const G: &str = "\x1b[32m";
    const R: &str = "\x1b[31m";
    const Y: &str = "\x1b[33m";
    const BLD: &str = "\x1b[1m";
    const X: &str = "\x1b[0m";

    println!("{BLD}Fresco diagnostics{X}");
    println!("──────────────────");

    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());
    let color = if session == "x11" { G } else { R };
    println!("Session         : {color}{session}{X}");

    match mpv::ffi::fns() {
        Ok(f) => {
            let v = f.client_api_version();
            println!(
                "libmpv          : {G}{}{X} (client API {}.{})",
                f.soname,
                v >> 16,
                v & 0xffff
            );
        }
        Err(e) => println!("libmpv          : {R}NOT LOADED{X} ({e})"),
    }

    if let Ok(out) = std::process::Command::new("sh")
        .arg("-c")
        .arg("lspci | grep -Ei 'vga|3d|display' | sed 's/.*: //'")
        .output()
    {
        for (i, line) in String::from_utf8_lossy(&out.stdout).lines().enumerate() {
            println!("GPU {i}           : {line}");
        }
    }

    let vainfo = which("vainfo");
    if vainfo {
        println!("VA-API (vainfo) : {G}available{X}");
    } else {
        println!("VA-API (vainfo) : {Y}not installed{X} (apt install intel-media-va-driver mesa-va-drivers)");
    }

    match Config::load() {
        Ok(c) => println!("Config          : {G}valid{X} (enabled={})", c.enabled),
        Err(e) => println!("Config          : {R}invalid{X} ({e})"),
    }

    match crate::ipc::request(&Request::Status) {
        Ok(Response::Status(s)) => {
            println!("Daemon          : {G}running{X}");
            println!(
                "  decode        : {}",
                s.hwdec.as_deref().unwrap_or("(none)")
            );
            println!(
                "  wallpaper     : {}",
                s.wallpaper.as_deref().unwrap_or("(none)")
            );
            println!("  RAM           : {} MB", s.rss_mb);
            if let Some(err) = s.error {
                println!("  {R}error{X}         : {err}");
            }
        }
        _ => println!("Daemon          : {Y}not running{X}"),
    }
}

fn which(bin: &str) -> bool {
    std::env::var("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(bin).is_file()))
        .unwrap_or(false)
}

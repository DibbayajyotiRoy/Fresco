//! Daemon-owned system-tray icon (D-Bus StatusNotifierItem).
//!
//! The tray runs on its own thread so the main render loop is never blocked.
//! On desktops without a tray host (stock GNOME, minimal containers, etc.) the
//! D-Bus registration simply sits unanswered and Fresco keeps working — no
//! crash, no busy loop.
//!
//! Menu actions talk to the running daemon through its normal Unix control
//! socket (`crate::ipc`), exactly like the GUI/CLI do. Next/Random/Open Fresco
//! shell out to the `fresco` binary so they reuse the existing library cycling
//! logic without dragging GUI/library code into the daemon.
//!
//! Dynamic show/hide: a supervisor thread watches `config.tray_icon` and
//! starts or drops the D-Bus service as it changes, so the Settings switch
//! works without restarting the daemon, and switching the icon off never
//! touches the wallpaper.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use ksni::blocking::TrayMethods;
use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};
use ksni::Tray;

use crate::config::Config;
use crate::ipc::{request, Request, Response};
use crate::t;

/// How often the tray refreshes its pause label from daemon status.
const STATUS_POLL: Duration = Duration::from_secs(2);

/// Runtime state reflected in the tray menu.
#[derive(Debug, Default)]
struct State {
    paused: bool,
    /// `Config::performance_mode`, re-read from config.toml with each status poll.
    performance: bool,
}

struct FrescoTray {
    state: State,
}

impl Tray for FrescoTray {
    fn id(&self) -> String {
        crate::APP_ID.into()
    }

    fn title(&self) -> String {
        crate::APP_NAME.into()
    }

    fn icon_name(&self) -> String {
        // Prefer the Fresco app icon when the package is installed; fall back
        // to a standard symbolic icon that exists on every GTK desktop.
        if fresco_icon_installed() {
            crate::APP_ID.into()
        } else {
            "preferences-desktop-wallpaper".into()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let paused = self.state.paused;
        vec![
            StandardItem {
                label: if paused {
                    t!("Resume playback").into()
                } else {
                    t!("Pause playback").into()
                },
                activate: Box::new(|this: &mut FrescoTray| {
                    let req = if this.state.paused {
                        Request::Resume
                    } else {
                        Request::Pause
                    };
                    // Flip on success so the label and the next click follow
                    // at once instead of waiting for the next status poll.
                    match request(&req) {
                        Ok(Response::Err { message }) => {
                            log::warn!("tray: pause/resume refused: {message}");
                        }
                        Ok(_) => this.state.paused = !this.state.paused,
                        Err(e) => log::warn!("tray: pause/resume request failed: {e:#}"),
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: t!("Next wallpaper").into(),
                activate: Box::new(|_: &mut FrescoTray| {
                    if let Err(e) = spawn_fresco(&["next"]) {
                        log::warn!("tray: failed to launch 'fresco next': {e:#}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: t!("Random wallpaper").into(),
                activate: Box::new(|_: &mut FrescoTray| {
                    if let Err(e) = spawn_fresco(&["random"]) {
                        log::warn!("tray: failed to launch 'fresco random': {e:#}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: t!("Performance mode").into(),
                checked: self.state.performance,
                activate: Box::new(|this: &mut FrescoTray| {
                    this.state.performance = !this.state.performance;
                    let on = this.state.performance;
                    // Apply respawns the renderers; keep the menu responsive.
                    thread::spawn(move || set_performance_mode(on));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: t!("Open main window").into(),
                activate: Box::new(|_: &mut FrescoTray| {
                    if let Err(e) = spawn_fresco(&[]) {
                        log::warn!("tray: failed to launch Fresco: {e:#}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: t!("Stop wallpaper").into(),
                activate: Box::new(|_: &mut FrescoTray| {
                    exit_daemon(Exit::Stop);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: t!("Quit Fresco (until next login)").into(),
                // Like Wallpaper Engine: the desktop is restored now, but
                // `enabled` stays true so autostart brings the wallpaper back
                // at the next login (issue #98).
                activate: Box::new(|_: &mut FrescoTray| {
                    exit_daemon(Exit::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    /// Log why the tray disappeared, but keep the daemon alive.
    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        log::debug!("tray: no tray host available ({reason:?})");
        true
    }
}

/// Start the tray supervisor on a background thread.
///
/// It owns the D-Bus service for the daemon's lifetime: started when
/// `config.tray_icon` is (or becomes) true, dropped when it turns false. A
/// service that cannot register is logged and left alone until the setting is
/// toggled again, so a broken session bus is not retried every poll.
pub fn spawn() {
    thread::spawn(supervise);
}

fn supervise() {
    let mut handle: Option<ksni::blocking::Handle<FrescoTray>> = None;
    let mut wanted = false;
    loop {
        // An unreadable config keeps whatever is running.
        let cfg = Config::load().ok();
        let want = cfg.as_ref().map_or(wanted, |c| c.tray_icon);
        if want != wanted {
            wanted = want;
            if want {
                match start_tray() {
                    Ok(h) => handle = Some(h),
                    Err(e) => log::warn!("tray: could not start: {e:#}"),
                }
            } else if let Some(h) = handle.take() {
                h.shutdown().wait();
                log::info!("tray: icon removed (tray_icon = false)");
            }
        }
        // Keep the pause label in sync without blocking the wallpaper loop.
        if let Some(h) = &handle {
            match request(&Request::Status) {
                Ok(Response::Status(status)) => {
                    // Also follows a switch flipped in the app's menu.
                    let performance = cfg.map(|c| c.performance_mode);
                    h.update(|tray: &mut FrescoTray| {
                        tray.state.paused = status.paused;
                        if let Some(on) = performance {
                            tray.state.performance = on;
                        }
                    });
                }
                Ok(_) => {}
                Err(e) => log::debug!("tray: status poll failed: {e:#}"),
            }
        }
        thread::sleep(STATUS_POLL);
    }
}

fn start_tray() -> anyhow::Result<ksni::blocking::Handle<FrescoTray>> {
    let tray = FrescoTray {
        state: State::default(),
    };

    // In a Flatpak sandbox the well-known D-Bus name is not available.
    let builder = if crate::is_flatpak() {
        tray.disable_dbus_name(true)
    } else {
        tray.disable_dbus_name(false)
    };

    // Assume SNI is available so that missing tray hosts (stock GNOME,
    // headless containers) become a soft offline event rather than a hard
    // error that would prevent the daemon from starting.
    Ok(builder.assume_sni_available(true).spawn()?)
}

/// Best-effort check for a packaged Fresco icon.
fn fresco_icon_installed() -> bool {
    let candidates = [
        PathBuf::from("/usr/share/icons/hicolor/scalable/apps")
            .join(format!("{}.svg", crate::APP_ID)),
        PathBuf::from("/usr/local/share/icons/hicolor/scalable/apps")
            .join(format!("{}.svg", crate::APP_ID)),
        dirs::home_dir()
            .map(|h| {
                h.join(".local/share/icons/hicolor/scalable/apps")
                    .join(format!("{}.svg", crate::APP_ID))
            })
            .unwrap_or_default(),
    ];
    candidates.iter().any(|p| p.exists())
}

/// Launch the `fresco` binary with the given arguments. Prefers the one next
/// to this `frescod` so a staged/dev daemon never opens a different-version
/// `fresco` from PATH (same lookup as `frescod_path` in `gui::daemon_ctl`).
fn spawn_fresco(args: &[&str]) -> anyhow::Result<()> {
    let fresco = std::env::current_exe()
        .map(|mut p| {
            p.set_file_name("fresco");
            p
        })
        .ok()
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("fresco"));
    std::process::Command::new(fresco).args(args).spawn()?;
    Ok(())
}

/// Flip performance mode the way the app's menu switch does: save the flag,
/// then ask the daemon to re-read config.toml and respawn its renderers.
fn set_performance_mode(on: bool) {
    let mut cfg = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => return log::warn!("tray: could not read the config: {e:#}"),
    };
    cfg.performance_mode = on;
    if let Err(e) = cfg.save() {
        return log::warn!("tray: failed to save performance mode: {e:#}");
    }
    if let Err(e) = request(&Request::Apply) {
        log::warn!("tray: apply after performance mode failed: {e:#}");
    }
}

/// How the tray ends the daemon. Both restore the desktop's own background now;
/// they differ in what the next login does.
#[derive(Clone, Copy)]
enum Exit {
    /// "Quit Fresco": `enabled` stays true, so autostart resumes the wallpaper
    /// at the next login.
    Quit,
    /// "Stop wallpaper": `enabled = false`, the same switch the GUI's stop
    /// saves, so the wallpaper stays off until the user starts it again.
    Stop,
}

/// Apply `exit` to `cfg`; true when it changed and must be saved.
fn apply_exit(cfg: &mut Config, exit: Exit) -> bool {
    matches!(exit, Exit::Stop) && std::mem::replace(&mut cfg.enabled, false)
}

/// What the daemon is asked once `cfg` is saved. Stopping the wallpaper while
/// the lock screen is on only switches the desktop off: an `Apply` makes the
/// daemon restore the desktop and keep serving the lock. Anything else ends it.
fn exit_request(cfg: &Config, exit: Exit) -> Request {
    if matches!(exit, Exit::Stop) && cfg.daemon_wanted() {
        Request::Apply
    } else {
        Request::Stop
    }
}

/// Restore the desktop's own background and end the daemon (see [`Exit`]).
fn exit_daemon(exit: Exit) {
    // Disable first, as the GUI does: a daemon that dies mid-stop must not be
    // resurrected with the wallpaper still enabled.
    let req = match Config::load() {
        Ok(mut cfg) => {
            if apply_exit(&mut cfg, exit) {
                if let Err(e) = cfg.save() {
                    log::warn!("tray: failed to disable wallpaper in config: {e:#}");
                }
            }
            exit_request(&cfg, exit)
        }
        Err(e) => {
            log::warn!("tray: could not read the config, 'enabled' left as is: {e:#}");
            Request::Stop
        }
    };
    // `Stop` runs the daemon's full shutdown, which puts the desktop's own
    // background back (DDE, GNOME, COSMIC, KDE, MATE, Xfce); so does the
    // `Apply` that finds the desktop wallpaper off.
    if let Err(e) = request(&req) {
        log::warn!("tray: stop request failed: {e:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_label_reflects_state() {
        let mut tray = FrescoTray {
            state: State::default(),
        };
        let menu = tray.menu();
        assert_eq!(menu.len(), 7);
        assert_eq!(label_at(&menu, 0), "Pause playback");

        tray.state.paused = true;
        let menu = tray.menu();
        assert_eq!(label_at(&menu, 0), "Resume playback");
    }

    #[test]
    fn menu_has_expected_actions() {
        let tray = FrescoTray {
            state: State::default(),
        };
        let labels: Vec<String> = tray.menu().iter().map(label_at_root).collect();
        assert_eq!(
            labels,
            vec![
                "Pause playback",
                "Next wallpaper",
                "Random wallpaper",
                "Performance mode",
                "Open main window",
                "Stop wallpaper",
                "Quit Fresco (until next login)",
            ]
        );
    }

    /// Issue #98: Quit restores the desktop but leaves `enabled = true`, so
    /// autostart resumes the wallpaper at the next login; Stop turns it off for
    /// good, and only once.
    #[test]
    fn quit_keeps_the_wallpaper_enabled_and_stop_turns_it_off() {
        let mut cfg = Config {
            enabled: true,
            ..Default::default()
        };
        assert!(
            !apply_exit(&mut cfg, Exit::Quit),
            "Quit has nothing to save"
        );
        assert!(cfg.enabled, "Quit must leave autostart able to resume it");

        assert!(apply_exit(&mut cfg, Exit::Stop));
        assert!(!cfg.enabled);
        // Already off: nothing to save.
        assert!(!apply_exit(&mut cfg, Exit::Stop));
        assert!(!cfg.enabled);
    }

    /// "Stop wallpaper" must not take the lock screen down with it; "Quit"
    /// and a stop with nothing else to serve end the daemon.
    #[test]
    fn stop_keeps_the_daemon_only_for_the_lock_screen() {
        let mut cfg = Config::default();
        assert_eq!(exit_request(&cfg, Exit::Quit), Request::Stop);
        apply_exit(&mut cfg, Exit::Stop);
        assert_eq!(exit_request(&cfg, Exit::Stop), Request::Stop);

        cfg.lockscreen = Some(crate::config::LockScreen {
            enabled: true,
            ..Default::default()
        });
        assert_eq!(exit_request(&cfg, Exit::Stop), Request::Apply);
        assert_eq!(exit_request(&cfg, Exit::Quit), Request::Stop);
    }

    fn label_at(menu: &[MenuItem<FrescoTray>], idx: usize) -> String {
        label_at_root(&menu[idx])
    }

    fn label_at_root(item: &MenuItem<FrescoTray>) -> String {
        match item {
            MenuItem::Standard(s) => s.label.clone(),
            MenuItem::Checkmark(c) => c.label.clone(),
            _ => panic!("expected a standard or checkmark menu item"),
        }
    }
}

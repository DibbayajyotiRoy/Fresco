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
//! Dynamic show/hide: the tray is created once when the daemon starts if
//! `config.tray_icon` is true at that moment. Changing the setting later
//! requires a daemon restart to take effect.

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
                    "Resume".into()
                } else {
                    "Pause".into()
                },
                activate: Box::new(|this: &mut FrescoTray| {
                    let req = if this.state.paused {
                        Request::Resume
                    } else {
                        Request::Pause
                    };
                    if let Err(e) = request(&req) {
                        log::warn!("tray: pause/resume request failed: {e:#}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next".into(),
                activate: Box::new(|_: &mut FrescoTray| {
                    if let Err(e) = spawn_fresco(&["next"]) {
                        log::warn!("tray: failed to launch 'fresco next': {e:#}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Random".into(),
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
                label: "Open Fresco".into(),
                activate: Box::new(|_: &mut FrescoTray| {
                    if let Err(e) = spawn_fresco(&[]) {
                        log::warn!("tray: failed to launch Fresco: {e:#}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Stop".into(),
                activate: Box::new(|_: &mut FrescoTray| {
                    stop_daemon();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|_: &mut FrescoTray| {
                    quit_daemon();
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

/// Start the tray service on a background thread.
///
/// This is a no-op if the tray cannot register on D-Bus; the error is logged
/// and the thread exits.
pub fn spawn() {
    thread::spawn(|| {
        if let Err(e) = run_tray() {
            log::warn!("tray: could not start: {e:#}");
        }
    });
}

fn run_tray() -> anyhow::Result<()> {
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
    let handle = builder.assume_sni_available(true).spawn()?;

    // Keep the pause label in sync without blocking the wallpaper loop.
    loop {
        thread::sleep(STATUS_POLL);
        match request(&Request::Status) {
            Ok(Response::Status(status)) => {
                // Also follows a switch flipped in the app's menu.
                let performance = Config::load().is_ok_and(|c| c.performance_mode);
                handle.update(|tray: &mut FrescoTray| {
                    tray.state.paused = status.paused;
                    tray.state.performance = performance;
                });
            }
            Ok(_) => {}
            Err(e) => log::debug!("tray: status poll failed: {e:#}"),
        }
    }
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

/// Launch the `fresco` binary with the given arguments.
fn spawn_fresco(args: &[&str]) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("fresco");
    cmd.args(args);
    cmd.spawn()?;
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

/// Stop the wallpaper and keep it stopped on the next login.
fn stop_daemon() {
    // Mirror the GUI's stop behaviour: disable autorestore first so a killed
    // daemon does not resurrect the wallpaper.
    if let Ok(mut cfg) = Config::load() {
        if cfg.enabled {
            cfg.enabled = false;
            if let Err(e) = cfg.save() {
                log::warn!("tray: failed to disable wallpaper in config: {e:#}");
            }
        }
    }
    quit_daemon();
}

/// Ask the daemon to tear down wallpaper windows and exit.
fn quit_daemon() {
    if let Err(e) = request(&Request::Stop) {
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
        assert!(label_at(&menu, 0).contains("Pause"));

        tray.state.paused = true;
        let menu = tray.menu();
        assert!(label_at(&menu, 0).contains("Resume"));
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
                "Pause",
                "Next",
                "Random",
                "Performance mode",
                "Open Fresco",
                "Stop",
                "Quit"
            ]
        );
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

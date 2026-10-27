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
use ksni::menu::{MenuItem, StandardItem};
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
                    if let Err(e) = request(&req) {
                        log::warn!("tray: pause/resume request failed: {e:#}");
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
                    stop_daemon();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: t!("Quit Fresco").into(),
                // Fresco's only process is this daemon, so quitting it is
                // stopping the wallpaper: restore the desktop, stay off at the
                // next login (issue #98).
                activate: Box::new(|_: &mut FrescoTray| {
                    stop_daemon();
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
        let want = Config::load().map_or(wanted, |c| c.tray_icon);
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
                    h.update(|tray: &mut FrescoTray| {
                        tray.state.paused = status.paused;
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

/// Launch the `fresco` binary with the given arguments.
fn spawn_fresco(args: &[&str]) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("fresco");
    cmd.args(args);
    cmd.spawn()?;
    Ok(())
}

/// Mark `cfg` as stopped: the same `enabled = false` the GUI's "Stop
/// wallpaper" saves, so the library no longer shows a wallpaper as playing and
/// login does not start a daemon for it. False when it already was.
fn disable_wallpaper(cfg: &mut Config) -> bool {
    std::mem::replace(&mut cfg.enabled, false)
}

/// Stop the wallpaper, restore the desktop's own background and keep it that
/// way at the next login. "Stop" and "Quit" both land here: the daemon is
/// Fresco's only background process, so quitting it and stopping the
/// wallpaper are one act (issue #98).
fn stop_daemon() {
    // Disable first, as the GUI does: a daemon that dies mid-stop must not be
    // resurrected with the wallpaper still enabled.
    if let Ok(mut cfg) = Config::load() {
        if disable_wallpaper(&mut cfg) {
            if let Err(e) = cfg.save() {
                log::warn!("tray: failed to disable wallpaper in config: {e:#}");
            }
        }
    }
    // `Stop` runs the daemon's full shutdown, which puts the desktop's own
    // background back (DDE, GNOME, COSMIC, KDE, MATE, Xfce).
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
            state: State { paused: false },
        };
        let menu = tray.menu();
        assert_eq!(menu.len(), 6);
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
                "Open main window",
                "Stop wallpaper",
                "Quit Fresco",
            ]
        );
    }

    /// Issue #98: "Quit" used to leave `enabled = true` behind, so the library
    /// kept showing the wallpaper as playing and the next login started a
    /// daemon for it. Quit and Stop now share `stop_daemon`; this is the config
    /// half of it.
    #[test]
    fn stopping_turns_the_wallpaper_off_once() {
        let mut cfg = Config {
            enabled: true,
            ..Default::default()
        };
        assert!(disable_wallpaper(&mut cfg));
        assert!(!cfg.enabled);
        // Already off: nothing to save.
        assert!(!disable_wallpaper(&mut cfg));
        assert!(!cfg.enabled);
    }

    fn label_at(menu: &[MenuItem<FrescoTray>], idx: usize) -> String {
        label_at_root(&menu[idx])
    }

    fn label_at_root(item: &MenuItem<FrescoTray>) -> String {
        match item {
            MenuItem::Standard(s) => s.label.clone(),
            _ => panic!("expected a standard menu item"),
        }
    }
}

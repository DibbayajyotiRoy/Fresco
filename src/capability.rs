//! Session capability detection — which wallpaper backend can run here.
//!
//! X11 sessions use the embedded-mpv backend. Wayland sessions are split:
//!  - GNOME/Mutter has no `wlr-layer-shell`, so we fall back to a static frame.
//!  - Everything else (wlroots, KDE Plasma 6, COSMIC, …) uses the mpvpaper
//!    layer-shell backend for live wallpapers.
//!
//! On Wayland we probe the live registry for `zwlr_layer_shell_v1` ourselves (no
//! external tools) and trust that over the desktop-name heuristic.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// X11 session — the existing in-process mpv backend.
    X11,
    /// Wayland with a layer-shell compositor — live wallpaper backend.
    WaylandLayerShell,
    /// Wayland on GNOME (no layer-shell) — static-frame fallback.
    WaylandGnomeStatic,
}

impl Capability {
    /// Short stable identifier for logs and diagnostics.
    pub fn id(self) -> &'static str {
        match self {
            Capability::X11 => "x11",
            Capability::WaylandLayerShell => "wayland-layer-shell",
            Capability::WaylandGnomeStatic => "wayland-gnome-static",
        }
    }
}

/// Detect the capability of the current session from the environment.
pub fn detect() -> Capability {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let wayland_display = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
    let session_desktop = std::env::var("XDG_SESSION_DESKTOP").ok();

    let is_wayland = match session_type.as_deref() {
        Some("wayland") => true,
        Some("x11") => false,
        // Session type unset/unknown: trust WAYLAND_DISPLAY.
        _ => wayland_display,
    };
    if !is_wayland {
        return Capability::X11;
    }

    // Prefer a real registry probe when available.
    if let Some(has_layer) = probe_layer_shell() {
        return if has_layer {
            Capability::WaylandLayerShell
        } else {
            // No layer-shell → treat like GNOME (static fallback) even if we
            // can't identify the compositor by name.
            Capability::WaylandGnomeStatic
        };
    }

    classify(
        session_type.as_deref(),
        wayland_display,
        current_desktop.as_deref().or(session_desktop.as_deref()),
    )
}

/// Pure desktop-name classification, testable without touching the process
/// environment. `detect()` may override this with a layer-shell registry probe.
fn classify(
    session_type: Option<&str>,
    wayland_display: bool,
    current_desktop: Option<&str>,
) -> Capability {
    let is_wayland = match session_type {
        Some("wayland") => true,
        Some("x11") => false,
        // Session type unset/unknown: trust WAYLAND_DISPLAY.
        _ => wayland_display,
    };
    if !is_wayland {
        return Capability::X11;
    }
    if is_gnome(current_desktop) {
        Capability::WaylandGnomeStatic
    } else {
        Capability::WaylandLayerShell
    }
}

fn is_gnome(desktop: Option<&str>) -> bool {
    desktop
        .map(|d| d.to_ascii_lowercase().contains("gnome"))
        .unwrap_or(false)
}

/// Probe the live Wayland registry for `zwlr_layer_shell_v1` — no external tools.
/// `Some(true/false)` when we could talk to the compositor; `None` only if we
/// couldn't connect at all, leaving the decision to the desktop-name heuristic.
#[cfg(feature = "daemon")]
fn probe_layer_shell() -> Option<bool> {
    use wayland_client::protocol::wl_registry;
    use wayland_client::{Connection, Dispatch, QueueHandle};

    #[derive(Default)]
    struct Probe {
        found: bool,
    }
    impl Dispatch<wl_registry::WlRegistry, ()> for Probe {
        fn event(
            state: &mut Self,
            _: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global { interface, .. } = event {
                if interface == "zwlr_layer_shell_v1" {
                    state.found = true;
                }
            }
        }
    }

    let conn = Connection::connect_to_env().ok()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut probe = Probe::default();
    queue.roundtrip(&mut probe).ok()?;
    Some(probe.found)
}

/// GUI-only builds don't link `wayland-client`; fall back to the name heuristic.
#[cfg(not(feature = "daemon"))]
fn probe_layer_shell() -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x11_session_is_x11() {
        assert_eq!(
            classify(Some("x11"), false, Some("pop:GNOME")),
            Capability::X11
        );
        // Session type wins even if WAYLAND_DISPLAY leaks into an X11 session.
        assert_eq!(classify(Some("x11"), true, Some("GNOME")), Capability::X11);
    }

    #[test]
    fn wayland_gnome_is_static() {
        for d in ["pop:GNOME", "ubuntu:GNOME", "GNOME", "gnome"] {
            assert_eq!(
                classify(Some("wayland"), true, Some(d)),
                Capability::WaylandGnomeStatic,
                "desktop {d}"
            );
        }
    }

    #[test]
    fn wayland_non_gnome_is_layer_shell() {
        for d in ["Hyprland", "sway", "KDE", "wlroots", "COSMIC", "river"] {
            assert_eq!(
                classify(Some("wayland"), true, Some(d)),
                Capability::WaylandLayerShell,
                "desktop {d}"
            );
        }
    }

    #[test]
    fn falls_back_to_wayland_display_when_session_type_unset() {
        assert_eq!(
            classify(None, true, Some("sway")),
            Capability::WaylandLayerShell
        );
        assert_eq!(
            classify(None, true, Some("GNOME")),
            Capability::WaylandGnomeStatic
        );
        assert_eq!(classify(None, false, None), Capability::X11);
    }
}

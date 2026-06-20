//! Per-output fullscreen detection via `wlr-foreign-toplevel-management`.
//!
//! The daemon pauses the wallpaper on any output that currently has a fullscreen
//! window, reclaiming the residual hardware-decode cost while the wallpaper is
//! fully hidden. (Rendering already idles when occluded — the compositor stops
//! sending frame callbacks — but mpv keeps *decoding* until we pause it.)
//!
//! This protocol is implemented by wlroots compositors (Sway/Hyprland) and KWin.
//! GNOME Mutter does not implement it, so there [`FullscreenWatch::new`] finds no
//! manager and returns `None` — the feature is simply absent (and GNOME uses the
//! static-frame path, not this layer-shell path, anyway).
//!
//! Single-threaded by design: the watch owns its own Wayland connection and is
//! pumped from the supervisor loop with a bounded roundtrip. It never spawns a
//! thread and never touches the players — the supervisor reads
//! [`FullscreenWatch::fullscreen_connectors`] and folds it into its one pause
//! decision, so Fresco stays the single authority over each player's pause state.

use std::collections::{HashMap, HashSet};

use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{event_created_child, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as handle, zwlr_foreign_toplevel_manager_v1 as manager,
};

/// Wire value of the `fullscreen` entry in the handle's `state` enum (stable ABI;
/// see wlr-foreign-toplevel-management `zwlr_foreign_toplevel_handle_v1.state`).
const STATE_FULLSCREEN: u32 = 3;

/// Per-toplevel tracked state: is it fullscreen, and which outputs is it on.
#[derive(Default)]
struct Toplevel {
    fullscreen: bool,
    outputs: HashSet<u32>, // wl_output protocol ids
}

#[derive(Default)]
struct State {
    /// wl_output protocol id → connector name (e.g. "DP-1"), matching the keys the
    /// supervisor uses for its per-output `WlOutput` map.
    output_names: HashMap<u32, String>,
    /// toplevel handle protocol id → tracked state.
    toplevels: HashMap<u32, Toplevel>,
    /// Set once the manager global is bound (i.e. the compositor supports it).
    has_manager: bool,
}

/// A live view of which outputs have a fullscreen window.
pub struct FullscreenWatch {
    queue: EventQueue<State>,
    state: State,
    // Kept alive for the life of the watch; dropping it closes the connection.
    _conn: Connection,
}

impl FullscreenWatch {
    /// Connect, bind the toplevel manager + outputs, and return a watch — or
    /// `None` if the compositor doesn't implement wlr-foreign-toplevel-management.
    pub fn new() -> Option<FullscreenWatch> {
        let conn = Connection::connect_to_env().ok()?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        let _registry = conn.display().get_registry(&qh, ());

        let mut state = State::default();
        // 1st roundtrip: registry globals → bind the manager + wl_outputs.
        queue.roundtrip(&mut state).ok()?;
        if !state.has_manager {
            return None;
        }
        // 2nd roundtrip: initial toplevel/state/output + output name events.
        queue.roundtrip(&mut state).ok()?;
        Some(FullscreenWatch {
            queue,
            state,
            _conn: conn,
        })
    }

    /// Drain pending toplevel events (one bounded roundtrip — fast on a local
    /// compositor, never an open-ended wait) and return the connectors that
    /// currently have a fullscreen window. Returns empty on any protocol error.
    pub fn fullscreen_connectors(&mut self) -> HashSet<String> {
        if self.queue.roundtrip(&mut self.state).is_err() {
            return HashSet::new();
        }
        let mut hidden = HashSet::new();
        for tl in self.state.toplevels.values() {
            if !tl.fullscreen {
                continue;
            }
            for oid in &tl.outputs {
                if let Some(name) = self.state.output_names.get(oid) {
                    hidden.insert(name.clone());
                }
            }
        }
        hidden
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<State>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "zwlr_foreign_toplevel_manager_v1" => {
                // v2+ carries the fullscreen state entry we rely on; v3 is current.
                registry.bind::<manager::ZwlrForeignToplevelManagerV1, _, _>(
                    name,
                    version.min(3),
                    qh,
                    (),
                );
                state.has_manager = true;
            }
            "wl_output" => {
                // v4 for the `name` event (connector string).
                registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wl_output::Event::Name { name } = event {
            state.output_names.insert(output.id().protocol_id(), name);
        }
    }
}

impl Dispatch<manager::ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &manager::ZwlrForeignToplevelManagerV1,
        event: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let manager::Event::Toplevel { toplevel } = event {
            state
                .toplevels
                .insert(toplevel.id().protocol_id(), Toplevel::default());
        }
    }

    // The `toplevel` event creates a new handle object; tell wayland-client to
    // give it `()` user-data so its events route to the Dispatch impl below.
    event_created_child!(State, manager::ZwlrForeignToplevelManagerV1, [
        manager::EVT_TOPLEVEL_OPCODE => (handle::ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<handle::ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        hnd: &handle::ZwlrForeignToplevelHandleV1,
        event: handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        let id = hnd.id().protocol_id();
        match event {
            // `state` is a packed array of u32 enum values; fullscreen if present.
            handle::Event::State { state: bytes } => {
                let fullscreen = bytes
                    .chunks_exact(4)
                    .any(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == STATE_FULLSCREEN);
                state.toplevels.entry(id).or_default().fullscreen = fullscreen;
            }
            handle::Event::OutputEnter { output } => {
                state
                    .toplevels
                    .entry(id)
                    .or_default()
                    .outputs
                    .insert(output.id().protocol_id());
            }
            handle::Event::OutputLeave { output } => {
                if let Some(tl) = state.toplevels.get_mut(&id) {
                    tl.outputs.remove(&output.id().protocol_id());
                }
            }
            handle::Event::Closed => {
                state.toplevels.remove(&id);
            }
            _ => {} // title / app_id / done / parent — not needed here
        }
    }
}

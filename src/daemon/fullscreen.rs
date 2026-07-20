//! Per-output fullscreen detection via `wlr-foreign-toplevel-management`, with a
//! COSMIC-native fallback (`zcosmic-toplevel-info-v1`).
//!
//! The daemon pauses the wallpaper on any output that currently has a fullscreen
//! window, reclaiming the residual hardware-decode cost while the wallpaper is
//! fully hidden. (Rendering already idles when occluded — the compositor stops
//! sending frame callbacks — but mpv keeps *decoding* until we pause it.)
//!
//! The wlr protocol is implemented by wlroots compositors (Sway/Hyprland) and
//! KWin. COSMIC's compositor ships its own `zcosmic_toplevel_info_v1` instead,
//! which carries the same per-toplevel state array (fullscreen included) and
//! output enter/leave events, so we mirror the wlr plumbing onto it when the wlr
//! manager is absent. GNOME Mutter implements neither, so there
//! [`FullscreenWatch::new`] finds no manager and returns `None` — the feature is
//! simply absent (and GNOME uses the static-frame path, not this layer-shell
//! path, anyway).
//!
//! Single-threaded by design: the watch owns its own Wayland connection and is
//! pumped from the supervisor loop with a bounded roundtrip. It never spawns a
//! thread and never touches the players — the supervisor reads
//! [`FullscreenWatch::fullscreen_connectors`] and folds it into its one pause
//! decision, so Fresco stays the single authority over each player's pause state.

use std::collections::{HashMap, HashSet};

use cosmic_protocols::toplevel_info::v1::client::{
    zcosmic_toplevel_handle_v1 as cosmic_handle, zcosmic_toplevel_info_v1 as cosmic_info,
};
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{event_created_child, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as handle, zwlr_foreign_toplevel_manager_v1 as manager,
};

/// Wire value of the `fullscreen` entry in the handle's `state` enum. Stable ABI
/// in both protocols, and (deliberately, on COSMIC's side) the same value:
/// wlr `zwlr_foreign_toplevel_handle_v1.state` and COSMIC
/// `zcosmic_toplevel_handle_v1.state` both define fullscreen = 3.
const STATE_FULLSCREEN: u32 = 3;

/// Which protocol the watch bound — surfaced for the startup log line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    Wlr,
    Cosmic,
}

/// `state` arrives as a packed wl_array of native-endian u32 enum values;
/// fullscreen iff the fullscreen entry is present. Trailing partial chunks
/// (malformed arrays) are ignored.
fn state_array_has_fullscreen(bytes: &[u8]) -> bool {
    bytes
        .chunks_exact(4)
        .any(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == STATE_FULLSCREEN)
}

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
    /// toplevel handle protocol id → tracked state (wlr and cosmic handles share
    /// this map; only one manager is ever bound, so ids never collide).
    toplevels: HashMap<u32, Toplevel>,
    /// Set once the wlr manager global is bound (i.e. the compositor supports it).
    has_manager: bool,
    /// COSMIC's `zcosmic_toplevel_info_v1` global, recorded (not bound) during the
    /// registry roundtrip; bound afterwards only if the wlr manager is absent.
    cosmic_global: Option<u32>, // registry `name`
}

impl State {
    /// Connectors that currently have a fullscreen toplevel.
    fn fullscreen_connectors(&self) -> HashSet<String> {
        let mut hidden = HashSet::new();
        for tl in self.toplevels.values() {
            if !tl.fullscreen {
                continue;
            }
            for oid in &tl.outputs {
                if let Some(name) = self.output_names.get(oid) {
                    hidden.insert(name.clone());
                }
            }
        }
        hidden
    }
}

/// A live view of which outputs have a fullscreen window.
pub struct FullscreenWatch {
    queue: EventQueue<State>,
    state: State,
    backend: Backend,
    // Kept alive for the life of the watch; dropping it closes the connection.
    _conn: Connection,
}

impl FullscreenWatch {
    /// Connect, bind a toplevel manager + outputs, and return a watch — or `None`
    /// if the compositor implements neither wlr-foreign-toplevel-management nor
    /// zcosmic-toplevel-info. The wlr protocol wins when both are advertised.
    pub fn new() -> Option<FullscreenWatch> {
        let conn = Connection::connect_to_env().ok()?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        let registry = conn.display().get_registry(&qh, ());

        let mut state = State::default();
        // 1st roundtrip: registry globals → bind the wlr manager + wl_outputs
        // (COSMIC's global is only recorded, so wlr keeps priority regardless of
        // the order globals arrive in).
        queue.roundtrip(&mut state).ok()?;
        let backend = if state.has_manager {
            Backend::Wlr
        } else if let Some(name) = state.cosmic_global {
            // v1 already carries the state array and output enter/leave events we
            // need; bind the lowest sufficient version defensively.
            registry.bind::<cosmic_info::ZcosmicToplevelInfoV1, _, _>(name, 1, &qh, ());
            Backend::Cosmic
        } else {
            return None;
        };
        // 2nd roundtrip: initial toplevel/state/output + output name events.
        queue.roundtrip(&mut state).ok()?;
        Some(FullscreenWatch {
            queue,
            state,
            backend,
            _conn: conn,
        })
    }

    /// Which protocol this watch is using.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// Drain pending toplevel events (one bounded roundtrip — fast on a local
    /// compositor, never an open-ended wait) and return the connectors that
    /// currently have a fullscreen window. Returns empty on any protocol error.
    pub fn fullscreen_connectors(&mut self) -> HashSet<String> {
        if self.queue.roundtrip(&mut self.state).is_err() {
            return HashSet::new();
        }
        self.state.fullscreen_connectors()
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
            "zcosmic_toplevel_info_v1" => {
                // Deferred: bound in `new()` only if the wlr manager is absent.
                state.cosmic_global = Some(name);
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

// ---------- wlr-foreign-toplevel-management ----------

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
            handle::Event::State { state: bytes } => {
                state.toplevels.entry(id).or_default().fullscreen =
                    state_array_has_fullscreen(&bytes);
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

// ---------- zcosmic-toplevel-info (COSMIC fallback) ----------

impl Dispatch<cosmic_info::ZcosmicToplevelInfoV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &cosmic_info::ZcosmicToplevelInfoV1,
        event: cosmic_info::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        // `finished` (the only other event): the compositor will send no
        // further toplevel events. Existing state stays valid; nothing to
        // tear down eagerly.
        if let cosmic_info::Event::Toplevel { toplevel } = event {
            state
                .toplevels
                .insert(toplevel.id().protocol_id(), Toplevel::default());
        }
    }

    event_created_child!(State, cosmic_info::ZcosmicToplevelInfoV1, [
        cosmic_info::EVT_TOPLEVEL_OPCODE => (cosmic_handle::ZcosmicToplevelHandleV1, ()),
    ]);
}

impl Dispatch<cosmic_handle::ZcosmicToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        hnd: &cosmic_handle::ZcosmicToplevelHandleV1,
        event: cosmic_handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        let id = hnd.id().protocol_id();
        match event {
            // Same wire format as the wlr handle: packed u32 enum array.
            cosmic_handle::Event::State { state: bytes } => {
                state.toplevels.entry(id).or_default().fullscreen =
                    state_array_has_fullscreen(&bytes);
            }
            cosmic_handle::Event::OutputEnter { output } => {
                state
                    .toplevels
                    .entry(id)
                    .or_default()
                    .outputs
                    .insert(output.id().protocol_id());
            }
            cosmic_handle::Event::OutputLeave { output } => {
                if let Some(tl) = state.toplevels.get_mut(&id) {
                    tl.outputs.remove(&output.id().protocol_id());
                }
            }
            cosmic_handle::Event::Closed => {
                state.toplevels.remove(&id);
            }
            _ => {} // title / app_id / done / workspace_enter/leave — not needed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packed(vals: &[u32]) -> Vec<u8> {
        vals.iter().flat_map(|v| v.to_ne_bytes()).collect()
    }

    #[test]
    fn state_array_detects_fullscreen() {
        // maximized=0, minimized=1, activated=2, fullscreen=3 (both protocols).
        assert!(state_array_has_fullscreen(&packed(&[3])));
        assert!(state_array_has_fullscreen(&packed(&[2, 3])));
        assert!(state_array_has_fullscreen(&packed(&[0, 2, 3, 4])));
        assert!(!state_array_has_fullscreen(&packed(&[0, 1, 2])));
        assert!(!state_array_has_fullscreen(&packed(&[])));
        // Unknown future state values are ignored, not misread.
        assert!(!state_array_has_fullscreen(&packed(&[7, 42])));
    }

    #[test]
    fn state_array_ignores_trailing_partial_chunk() {
        let mut bytes = packed(&[2]);
        bytes.extend_from_slice(&[3, 0]); // malformed tail, not a full u32
        assert!(!state_array_has_fullscreen(&bytes));
    }

    #[test]
    fn connectors_resolve_only_named_outputs_of_fullscreen_toplevels() {
        let mut s = State::default();
        s.output_names.insert(10, "DP-1".into());
        s.output_names.insert(11, "HDMI-A-1".into());
        // Fullscreen on DP-1 and an unnamed output (12) → only DP-1 reported.
        s.toplevels.insert(
            1,
            Toplevel {
                fullscreen: true,
                outputs: [10, 12].into_iter().collect(),
            },
        );
        // Non-fullscreen on HDMI-A-1 → not reported.
        s.toplevels.insert(
            2,
            Toplevel {
                fullscreen: false,
                outputs: [11].into_iter().collect(),
            },
        );
        assert_eq!(
            s.fullscreen_connectors(),
            HashSet::from(["DP-1".to_string()])
        );
        // Toplevel leaves DP-1 → set empties.
        s.toplevels.get_mut(&1).unwrap().outputs.remove(&10);
        assert!(s.fullscreen_connectors().is_empty());
    }
}

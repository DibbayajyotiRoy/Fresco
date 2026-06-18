//! Minimal Wayland output enumeration (static snapshot).
//!
//! Phase 2 needs the current output connector names to spawn one mpvpaper per
//! output. We bind `wl_output` (v4 for the `name` event, e.g. "DP-1") and read
//! its geometry/mode, producing the same neutral [`Monitor`] shape RandR fills
//! on X11 so per-monitor config keys match across backends.
//!
//! One-shot snapshot at startup / explicit reconcile. Live hotplug (Phase 3)
//! will hook the same registry globals instead of disconnecting.

use std::collections::HashMap;

use anyhow::{Context, Result};
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};

use super::monitors::Monitor;

#[derive(Default)]
struct OutputInfo {
    name: Option<String>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Default)]
struct State {
    outputs: HashMap<u32, OutputInfo>,
}

/// Enumerate connected Wayland outputs into the neutral [`Monitor`] set.
pub fn list_outputs() -> Result<Vec<Monitor>> {
    let conn = Connection::connect_to_env().context("connecting to the Wayland display")?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    let mut state = State::default();
    // 1st roundtrip: registry globals → bind wl_outputs.
    queue
        .roundtrip(&mut state)
        .context("wayland registry roundtrip")?;
    // 2nd roundtrip: each wl_output's geometry/mode/name events arrive.
    queue
        .roundtrip(&mut state)
        .context("wayland output roundtrip")?;

    let mut monitors: Vec<Monitor> = state
        .outputs
        .into_iter()
        .filter(|(_, o)| o.width > 0 && o.height > 0)
        .map(|(id, o)| Monitor {
            connector: o.name.unwrap_or_else(|| format!("output-{id}")),
            x: o.x as i16,
            y: o.y as i16,
            width: o.width as u16,
            height: o.height as u16,
        })
        .collect();
    monitors.sort_by(|a, b| a.connector.cmp(&b.connector));
    Ok(monitors)
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
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_output" {
                let output =
                    registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
                state
                    .outputs
                    .insert(output.id().protocol_id(), OutputInfo::default());
            }
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
        let info = state.outputs.entry(output.id().protocol_id()).or_default();
        match event {
            wl_output::Event::Geometry { x, y, .. } => {
                info.x = x;
                info.y = y;
            }
            wl_output::Event::Mode {
                flags,
                width,
                height,
                ..
            } => {
                // Only the current mode defines the output's pixel size.
                let current =
                    matches!(flags, WEnum::Value(m) if m.contains(wl_output::Mode::Current));
                if current || info.width == 0 {
                    info.width = width;
                    info.height = height;
                }
            }
            wl_output::Event::Name { name } => info.name = Some(name),
            _ => {}
        }
    }
}

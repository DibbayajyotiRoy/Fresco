//! RandR monitor enumeration.

use anyhow::Result;
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xproto::Window;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    pub connector: String,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Monitor {
    pub fn aspect(&self) -> f64 {
        if self.height == 0 {
            return 16.0 / 9.0;
        }
        self.width as f64 / self.height as f64
    }
}

/// List active monitors via RandR `GetMonitors`, keyed by connector name.
pub fn list_monitors<C: Connection>(conn: &C, root: Window) -> Result<Vec<Monitor>> {
    let reply = conn.randr_get_monitors(root, true)?.reply()?;
    let mut out = Vec::new();
    for mon in reply.monitors {
        let name = get_atom_name(conn, mon.name).unwrap_or_else(|| format!("monitor-{}", mon.name));
        out.push(Monitor {
            connector: name,
            x: mon.x,
            y: mon.y,
            width: mon.width,
            height: mon.height,
        });
    }
    // Fallback: if RandR reports nothing, use the root window geometry.
    if out.is_empty() {
        let geo = conn.get_geometry(root)?.reply()?;
        out.push(Monitor {
            connector: "default".to_string(),
            x: 0,
            y: 0,
            width: geo.width,
            height: geo.height,
        });
    }
    Ok(out)
}

fn get_atom_name<C: Connection>(conn: &C, atom: x11rb::protocol::xproto::Atom) -> Option<String> {
    let reply = conn.get_atom_name(atom).ok()?.reply().ok()?;
    Some(String::from_utf8_lossy(&reply.name).into_owned())
}

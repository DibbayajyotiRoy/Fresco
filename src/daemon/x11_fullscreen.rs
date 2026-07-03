//! Poll-based X11 fullscreen detection (ROADMAP 1.3 — parity with the Wayland
//! `fullscreen.rs` auto-pause).
//!
//! Reads the WM-maintained EWMH state: `_NET_CLIENT_LIST_STACKING` on the root
//! plus each client's `_NET_WM_STATE`, and reports which monitors are covered
//! by a viewable fullscreen window. Deliberately POLLED on the daemon's 2s
//! cadence, never event-driven — reacting to X events is how the historical
//! ConfigureNotify storm froze laptops (see the note in `Daemon::run`).
//!
//! On a WM-less X server (or a non-EWMH WM) the root property is absent and
//! this returns an empty set — the feature degrades to "never pauses", which
//! is exactly the pre-existing behavior.

use std::collections::HashSet;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, MapState, Window};

use super::monitors::Monitor;
use super::x11win::Atoms;

/// Connectors whose monitor is ≥50% covered by a viewable fullscreen window.
pub fn covered_connectors<C: Connection>(
    conn: &C,
    root: Window,
    atoms: &Atoms,
    monitors: &[Monitor],
) -> HashSet<String> {
    let mut covered = HashSet::new();
    let Ok(list) = conn
        .get_property(
            false,
            root,
            atoms._NET_CLIENT_LIST_STACKING,
            AtomEnum::WINDOW,
            0,
            u32::MAX,
        )
        .map(|c| c.reply())
    else {
        return covered;
    };
    let Ok(list) = list else { return covered };
    let Some(windows) = list.value32() else {
        return covered;
    };

    for win in windows {
        // Only mapped windows can cover anything.
        let viewable = conn
            .get_window_attributes(win)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|a| a.map_state == MapState::VIEWABLE)
            .unwrap_or(false);
        if !viewable {
            continue;
        }
        let Some(states) = window_states(conn, win, atoms) else {
            continue;
        };
        if !states.contains(&atoms._NET_WM_STATE_FULLSCREEN)
            || states.contains(&atoms._NET_WM_STATE_HIDDEN)
        {
            continue;
        }
        // Absolute geometry: the WM reparents clients, so translate to root.
        let Some((x, y, w, h)) = absolute_geometry(conn, root, win) else {
            continue;
        };
        for m in monitors {
            if overlap_at_least_half(x, y, w, h, m) {
                covered.insert(m.connector.clone());
            }
        }
    }
    covered
}

fn window_states<C: Connection>(conn: &C, win: Window, atoms: &Atoms) -> Option<Vec<u32>> {
    let reply = conn
        .get_property(false, win, atoms._NET_WM_STATE, AtomEnum::ATOM, 0, 64)
        .ok()?
        .reply()
        .ok()?;
    let states: Vec<u32> = reply.value32()?.collect();
    Some(states)
}

fn absolute_geometry<C: Connection>(
    conn: &C,
    root: Window,
    win: Window,
) -> Option<(i32, i32, u32, u32)> {
    let geo = conn.get_geometry(win).ok()?.reply().ok()?;
    let abs = conn
        .translate_coordinates(win, root, 0, 0)
        .ok()?
        .reply()
        .ok()?;
    Some((
        i32::from(abs.dst_x),
        i32::from(abs.dst_y),
        u32::from(geo.width),
        u32::from(geo.height),
    ))
}

/// True when the window rect covers at least half of the monitor's area —
/// fullscreen windows match the monitor exactly; 50% keeps us robust against
/// off-by-frame geometry without pausing for mere large windows on overlap
/// edges of adjacent monitors.
fn overlap_at_least_half(x: i32, y: i32, w: u32, h: u32, m: &Monitor) -> bool {
    let (mx, my) = (i32::from(m.x), i32::from(m.y));
    let (mw, mh) = (i32::from(m.width), i32::from(m.height));
    let ix = (x + w as i32).min(mx + mw) - x.max(mx);
    let iy = (y + h as i32).min(my + mh) - y.max(my);
    if ix <= 0 || iy <= 0 {
        return false;
    }
    let inter = i64::from(ix) * i64::from(iy);
    let marea = i64::from(mw) * i64::from(mh);
    inter * 2 >= marea
}

#[cfg(test)]
mod tests {
    use super::*;
    use x11rb::protocol::xproto::{CreateWindowAux, PropMode, WindowClass};
    use x11rb::wrapper::ConnectionExt as _;
    use x11rb::COPY_DEPTH_FROM_PARENT;

    fn mon(connector: &str, x: i16, y: i16, w: u16, h: u16) -> Monitor {
        Monitor {
            connector: connector.into(),
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn overlap_math() {
        let m = mon("A", 0, 0, 1920, 1080);
        assert!(overlap_at_least_half(0, 0, 1920, 1080, &m)); // exact
        assert!(overlap_at_least_half(-10, -10, 1940, 1100, &m)); // overshoot
        assert!(!overlap_at_least_half(0, 0, 800, 600, &m)); // small window
        assert!(!overlap_at_least_half(1920, 0, 1920, 1080, &m)); // next monitor
        let b = mon("B", 1920, 0, 1920, 1080);
        assert!(overlap_at_least_half(1920, 0, 1920, 1080, &b));
        assert!(!overlap_at_least_half(0, 0, 1920, 1080, &b));
    }

    /// End-to-end against a real X server: this test PLAYS THE WM — it creates
    /// a client window, marks it fullscreen via `_NET_WM_STATE`, and publishes
    /// `_NET_CLIENT_LIST_STACKING` on the root, then asserts detection.
    /// Skips when there is no DISPLAY (plain `cargo test` in CI).
    #[test]
    fn detects_fullscreen_window_on_real_x() {
        // Writes root window properties — must NEVER run against a live
        // desktop session. Opt in explicitly from a scratch X server:
        //   tests/ci/with-compositor.sh x11 -- \
        //     env FRESCO_EWMH_TEST=1 cargo test --features daemon x11_fullscreen
        if std::env::var("FRESCO_EWMH_TEST").ok().as_deref() != Some("1") {
            eprintln!("skip detects_fullscreen_window_on_real_x: set FRESCO_EWMH_TEST=1 inside a scratch X server");
            return;
        }
        if std::env::var("DISPLAY").is_err() {
            eprintln!("skip detects_fullscreen_window_on_real_x: no DISPLAY");
            return;
        }
        let (conn, screen_num) = match x11rb::connect(None) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("skip: cannot connect to X: {e}");
                return;
            }
        };
        let screen = conn.setup().roots[screen_num].clone();
        let atoms = Atoms::new(&conn).unwrap().reply().unwrap();
        let monitors = [mon(
            "TEST-1",
            0,
            0,
            screen.width_in_pixels,
            screen.height_in_pixels,
        )];

        let win = conn.generate_id().unwrap();
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            win,
            screen.root,
            0,
            0,
            screen.width_in_pixels,
            screen.height_in_pixels,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap();
        conn.map_window(win).unwrap();
        conn.change_property32(
            PropMode::REPLACE,
            win,
            atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            &[atoms._NET_WM_STATE_FULLSCREEN],
        )
        .unwrap();
        conn.change_property32(
            PropMode::REPLACE,
            screen.root,
            atoms._NET_CLIENT_LIST_STACKING,
            AtomEnum::WINDOW,
            &[win],
        )
        .unwrap();
        conn.sync().unwrap();

        let covered = covered_connectors(&conn, screen.root, &atoms, &monitors);
        assert!(
            covered.contains("TEST-1"),
            "fullscreen window not detected: {covered:?}"
        );

        // Clearing the state clears the detection.
        conn.change_property32(
            PropMode::REPLACE,
            win,
            atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            &[],
        )
        .unwrap();
        conn.sync().unwrap();
        let covered = covered_connectors(&conn, screen.root, &atoms, &monitors);
        assert!(
            covered.is_empty(),
            "state cleared but still covered: {covered:?}"
        );

        // Unmapped fullscreen window must not pause either.
        conn.change_property32(
            PropMode::REPLACE,
            win,
            atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            &[atoms._NET_WM_STATE_FULLSCREEN],
        )
        .unwrap();
        conn.unmap_window(win).unwrap();
        conn.sync().unwrap();
        let covered = covered_connectors(&conn, screen.root, &atoms, &monitors);
        assert!(covered.is_empty(), "unmapped window counted: {covered:?}");

        // Leave the root property empty so nothing lingers for other tests.
        conn.change_property32(
            PropMode::REPLACE,
            screen.root,
            atoms._NET_CLIENT_LIST_STACKING,
            AtomEnum::WINDOW,
            &[],
        )
        .unwrap();
        conn.sync().unwrap();
    }
}

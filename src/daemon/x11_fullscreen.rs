//! Poll-based X11 fullscreen/covered detection (ROADMAP 1.3 — parity with the
//! Wayland `fullscreen.rs` auto-pause).
//!
//! Reads the WM-maintained EWMH state: `_NET_CLIENT_LIST_STACKING` on the root
//! plus each client's `_NET_WM_STATE`, and reports which monitors are covered
//! by a viewable fullscreen window (always) or, when `pause_on_covered` is
//! enabled, by any large non-fullscreen window overlapping at least 90% of the
//! monitor. Deliberately POLLED on the daemon's 2s cadence, never event-driven
//! — reacting to X events is how the historical ConfigureNotify storm froze
//! laptops (see the note in `Daemon::run`).
//!
//! On a WM-less X server (or a non-EWMH WM) the root property is absent and
//! this returns an empty set — the feature degrades to "never pauses", which
//! is exactly the pre-existing behavior.

use std::collections::HashMap;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, MapState, Window};

use super::monitors::Monitor;
use super::x11win::Atoms;

/// Connectors whose monitor is covered by a viewable window, mapped to a short
/// description of that window (kind + title, for the pause log — so a misfiring
/// compositor surface is identifiable from the log alone).
///
/// Fullscreen windows always count. When `pause_on_covered` is `true`, any
/// viewable, non-minimized, non-skip-taskbar window whose geometry overlaps at
/// least 90% of a monitor also counts.
pub fn covered_connectors<C: Connection>(
    conn: &C,
    root: Window,
    atoms: &Atoms,
    monitors: &[Monitor],
    pause_on_covered: bool,
) -> HashMap<String, String> {
    let mut covered = HashMap::new();
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

        // Ignore Fresco's own wallpaper windows; they are below apps but can
        // appear in the client list on some WMs.
        if wm_class_is_fresco(conn, win, atoms) {
            continue;
        }

        let Some(states) = window_states(conn, win, atoms) else {
            continue;
        };
        let has = |a| states.contains(&a);
        if has(atoms._NET_WM_STATE_HIDDEN) || has(atoms._NET_WM_STATE_SKIP_TASKBAR) {
            continue; // minimized, or a panel/desktop-icon window
        }

        let is_fullscreen = has(atoms._NET_WM_STATE_FULLSCREEN);
        // Without the covered switch, only fullscreen windows can pause.
        if !is_fullscreen && !pause_on_covered {
            continue;
        }

        // Absolute geometry: the WM reparents clients, so translate to root.
        let Some((x, y, w, h)) = absolute_geometry(conn, root, win) else {
            continue;
        };
        let title = window_title(conn, win, atoms);

        // Overlap threshold: 0.9 when covered mode is enabled, 0.5 otherwise.
        // Fullscreen windows match the monitor exactly, so 0.9 is still met.
        let threshold = if pause_on_covered { 0.9 } else { 0.5 };

        for m in monitors {
            if is_fullscreen && overlap_at_least(x, y, w, h, m, threshold) {
                covered
                    .entry(m.connector.clone())
                    .or_insert_with(|| format!("fullscreen window ({:?})", title));
            } else if pause_on_covered && overlap_at_least(x, y, w, h, m, 0.9) {
                covered
                    .entry(m.connector.clone())
                    .or_insert_with(|| format!("covered window ({:?})", title));
            }
        }
    }
    covered
}

/// True when the window's `WM_CLASS` names Fresco's own wallpaper windows.
fn wm_class_is_fresco<C: Connection>(conn: &C, win: Window, atoms: &Atoms) -> bool {
    conn.get_property(false, win, atoms.WM_CLASS, AtomEnum::STRING, 0, 64)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.value.split(|&b| b == 0).any(|s| s == b"fresco-wallpaper"))
        .unwrap_or(false)
}

/// Best-effort window title (`_NET_WM_NAME`), for the pause log.
fn window_title<C: Connection>(conn: &C, win: Window, atoms: &Atoms) -> String {
    conn.get_property(false, win, atoms._NET_WM_NAME, atoms.UTF8_STRING, 0, 64)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| String::from_utf8_lossy(&r.value).into_owned())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| format!("window 0x{win:x}"))
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

/// True when the window rect covers at least `threshold` of the monitor's area.
/// `threshold` is a ratio in the range [0, 1] (e.g. 0.9 for 90%).
fn overlap_at_least(x: i32, y: i32, w: u32, h: u32, m: &Monitor, threshold: f64) -> bool {
    let (mx, my) = (i32::from(m.x), i32::from(m.y));
    let (mw, mh) = (i32::from(m.width), i32::from(m.height));
    let ix = (x + w as i32).min(mx + mw) - x.max(mx);
    let iy = (y + h as i32).min(my + mh) - y.max(my);
    if ix <= 0 || iy <= 0 {
        return false;
    }
    let inter = i64::from(ix) * i64::from(iy);
    let marea = i64::from(mw) * i64::from(mh);
    // Integer-scaled comparison avoids floating-point drift.
    let scale = 1000i64;
    let num = (threshold * scale as f64).round() as i64;
    inter * scale >= marea * num
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
            scale_milli: 1000,
        }
    }

    #[test]
    fn overlap_math() {
        let m = mon("A", 0, 0, 1920, 1080);
        // Exact and overshoot still count at 0.5 and 0.9.
        assert!(overlap_at_least(0, 0, 1920, 1080, &m, 0.9));
        assert!(overlap_at_least(-10, -10, 1940, 1100, &m, 0.9));
        // Small window does not count even at 0.5.
        assert!(!overlap_at_least(0, 0, 800, 600, &m, 0.5));
        // Adjacent monitor does not count.
        assert!(!overlap_at_least(1920, 0, 1920, 1080, &m, 0.5));

        // 90% threshold boundary.
        assert!(overlap_at_least(0, 0, 1920, 1080, &m, 0.9)); // 100%
        assert!(overlap_at_least(0, 0, 1728, 1080, &m, 0.9)); // 90% width
        assert!(!overlap_at_least(0, 0, 1727, 1080, &m, 0.9)); // just under 90%
        assert!(overlap_at_least(0, 0, 1920, 972, &m, 0.9)); // 90% height
        assert!(!overlap_at_least(0, 0, 1920, 971, &m, 0.9)); // just under 90%

        let b = mon("B", 1920, 0, 1920, 1080);
        assert!(overlap_at_least(1920, 0, 1920, 1080, &b, 0.5));
        assert!(!overlap_at_least(0, 0, 1920, 1080, &b, 0.5));
    }

    #[test]
    fn fresco_wallpaper_windows_are_ignored() {
        use crate::WALLPAPER_WM_CLASS;

        // Needs a real X server to read WM_CLASS back.
        if std::env::var("FRESCO_EWMH_TEST").ok().as_deref() != Some("1") {
            eprintln!("skip fresco_wallpaper_windows_are_ignored: set FRESCO_EWMH_TEST=1 inside a scratch X server");
            return;
        }
        if std::env::var("DISPLAY").is_err() {
            eprintln!("skip fresco_wallpaper_windows_are_ignored: no DISPLAY");
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

        // Set WM_CLASS to Fresco's wallpaper class and make it fullscreen-sized.
        let mut wm_class = Vec::new();
        wm_class.extend_from_slice(WALLPAPER_WM_CLASS.as_bytes());
        wm_class.push(0);
        wm_class.extend_from_slice(WALLPAPER_WM_CLASS.as_bytes());
        wm_class.push(0);
        conn.change_property8(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            &wm_class,
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

        let monitors = [mon(
            "TEST-1",
            0,
            0,
            screen.width_in_pixels,
            screen.height_in_pixels,
        )];
        let covered = covered_connectors(&conn, screen.root, &atoms, &monitors, false);
        assert!(
            !covered.contains_key("TEST-1"),
            "fresco-wallpaper window should not count as a cover: {covered:?}"
        );

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

        let covered = covered_connectors(&conn, screen.root, &atoms, &monitors, false);
        assert!(
            covered.contains_key("TEST-1"),
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
        let covered = covered_connectors(&conn, screen.root, &atoms, &monitors, false);
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
        let covered = covered_connectors(&conn, screen.root, &atoms, &monitors, false);
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

//! Desktop-level X11 window: spans one monitor, accepts no input, and hosts an
//! embedded mpv.
//!
//! Two stacking flavours, picked per session — see [`WindowKind`]. Everything
//! else (geometry, input shape, hints, mpv embedding) is identical.

use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::shape::ConnectionExt as _;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

use super::monitors::Monitor;
use crate::WALLPAPER_WM_CLASS;

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_NORMAL,
        _NET_WM_STATE,
        _NET_WM_STATE_BELOW,
        _NET_WM_STATE_STICKY,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_SKIP_PAGER,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_HIDDEN,
        _NET_CLIENT_LIST_STACKING,
        _NET_WM_NAME,
        UTF8_STRING,
        _MOTIF_WM_HINTS,
        WM_HINTS,
    }
}

/// How a wallpaper window declares itself to the window manager.
///
/// `Desktop` is what Fresco has always used and stays the default on every
/// desktop environment. `DdeRaised` exists only for Deepin DDE, so no other WM
/// ever sees a different set of properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowKind {
    /// `_NET_WM_WINDOW_TYPE_DESKTOP` + `_NET_WM_STATE_BELOW`, lowered to the
    /// bottom of the stack: below the desktop-icon window, below everything.
    #[default]
    Desktop,
    /// Deepin DDE only. Measured on Deepin 25 (dde-shell, X11, KWin): a
    /// DESKTOP-only window is pinned to KWin's bottom desktop layer,
    /// permanently under dde-shell's own desktop window, so the video is never
    /// visible. Declaring `[DESKTOP, NORMAL]` — the exact list dde-shell's
    /// desktop window itself declares — and raising (no `BELOW`, which would
    /// fight the raise) puts our window above DDE's desktop while real app
    /// windows and the dock still stack above us. DDE's desktop icons end up
    /// hidden; that is the accepted trade-off of this mode.
    DdeRaised,
}

/// `_NET_WM_WINDOW_TYPE` value for `kind`. Order is significant: DESKTOP first
/// (EWMH "most preferable first"), NORMAL second, mirroring dde-shell.
fn window_type_atoms(kind: WindowKind, atoms: &Atoms) -> Vec<Atom> {
    match kind {
        WindowKind::Desktop => vec![atoms._NET_WM_WINDOW_TYPE_DESKTOP],
        WindowKind::DdeRaised => vec![
            atoms._NET_WM_WINDOW_TYPE_DESKTOP,
            atoms._NET_WM_WINDOW_TYPE_NORMAL,
        ],
    }
}

/// `_NET_WM_STATE` value for `kind`. STICKY + SKIP_TASKBAR + SKIP_PAGER always;
/// BELOW only for `Desktop`, since on the DDE path it would undo the raise.
fn window_state_atoms(kind: WindowKind, atoms: &Atoms) -> Vec<Atom> {
    let mut states = Vec::with_capacity(4);
    if kind == WindowKind::Desktop {
        states.push(atoms._NET_WM_STATE_BELOW);
    }
    states.extend([
        atoms._NET_WM_STATE_STICKY,
        atoms._NET_WM_STATE_SKIP_TASKBAR,
        atoms._NET_WM_STATE_SKIP_PAGER,
    ]);
    states
}

pub struct WallpaperWindow {
    pub window: Window,
    pub connector: String,
}

impl WallpaperWindow {
    /// Create, configure, map, and stack a wallpaper window for `monitor`.
    /// `kind` decides the EWMH declaration and whether the window is lowered
    /// (everywhere) or raised (Deepin DDE) once mapped.
    pub fn create<C: Connection>(
        conn: &C,
        screen: &Screen,
        atoms: &Atoms,
        monitor: &Monitor,
        kind: WindowKind,
    ) -> Result<WallpaperWindow> {
        let window = conn.generate_id()?;
        let aux = CreateWindowAux::new()
            .background_pixel(screen.black_pixel)
            .event_mask(EventMask::STRUCTURE_NOTIFY)
            .override_redirect(0); // let the WM manage it as a desktop window

        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            screen.root,
            monitor.x,
            monitor.y,
            monitor.width,
            monitor.height,
            0,
            WindowClass::INPUT_OUTPUT,
            0, // copy visual from parent
            &aux,
        )?
        .check()
        .context("create_window")?;

        // _NET_WM_WINDOW_TYPE / _NET_WM_STATE — the only per-kind difference.
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms._NET_WM_WINDOW_TYPE,
            AtomEnum::ATOM,
            &window_type_atoms(kind, atoms),
        )?;

        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            &window_state_atoms(kind, atoms),
        )?;

        // WM_CLASS = "fresco-wallpaper\0fresco-wallpaper\0"
        let mut wm_class = Vec::new();
        wm_class.extend_from_slice(WALLPAPER_WM_CLASS.as_bytes());
        wm_class.push(0);
        wm_class.extend_from_slice(WALLPAPER_WM_CLASS.as_bytes());
        wm_class.push(0);
        conn.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            &wm_class,
        )?;

        // WM_NAME
        let name = format!("Fresco Wallpaper ({})", monitor.connector);
        conn.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            name.as_bytes(),
        )?;

        // ICCCM WM_HINTS with InputHint=1, input=False → no keyboard focus.
        // flags, input, initial_state, icon_pixmap, icon_window, icon_x,
        // icon_y, icon_mask, window_group
        let wm_hints: [u32; 9] = [1, 0, 0, 0, 0, 0, 0, 0, 0];
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms.WM_HINTS,
            atoms.WM_HINTS,
            &wm_hints,
        )?;

        // _MOTIF_WM_HINTS: remove decorations (flags=2 -> decorations field, 0).
        let motif: [u32; 5] = [2, 0, 0, 0, 0];
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms._MOTIF_WM_HINTS,
            atoms._MOTIF_WM_HINTS,
            &motif,
        )?;

        // Empty input shape → clicks pass through to the real desktop.
        conn.shape_rectangles(
            x11rb::protocol::shape::SO::SET,
            x11rb::protocol::shape::SK::INPUT,
            ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[],
        )?;

        conn.map_window(window)?;
        restack(conn, window, kind)?;
        conn.flush()?;

        // Wait until the window is actually viewable before the caller embeds mpv
        // in it. On a cold boot the X server + WM are slow to process the map; if
        // mpv brings up its display-synced video output (vo=gpu, video-sync=
        // display-resample) on a not-yet-viewable window, it stalls on the first
        // frame and stays frozen until the next rebuild — the "wallpaper is static
        // after reboot until I reselect it" bug. Bounded poll: returns immediately
        // once mapped (the warm / reselect case) and never blocks startup for long.
        wait_until_viewable(conn, window);

        Ok(WallpaperWindow {
            window,
            connector: monitor.connector.clone(),
        })
    }

    pub fn destroy<C: Connection>(&self, conn: &C) {
        let _ = conn.destroy_window(self.window);
        let _ = conn.flush();
    }
}

/// Poll until `window` is viewable, up to ~3s. Best-effort: any error or a
/// timeout just proceeds — we never block the daemon's startup indefinitely.
fn wait_until_viewable<C: Connection>(conn: &C, window: Window) {
    for _ in 0..60 {
        match conn.get_window_attributes(window) {
            Ok(cookie) => match cookie.reply() {
                Ok(attrs) if attrs.map_state == MapState::VIEWABLE => return,
                Ok(_) => {}
                Err(_) => return,
            },
            Err(_) => return,
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Lower a window to the bottom of the stack.
pub fn lower<C: Connection>(conn: &C, window: Window) -> Result<()> {
    conn.configure_window(
        window,
        &ConfigureWindowAux::new().stack_mode(StackMode::BELOW),
    )?;
    Ok(())
}

/// Raise a window to the top of the stack. Deliberately sibling-less: a
/// sibling-relative `ConfigureWindow` against dde-shell's desktop window fails
/// with BadMatch, because KWin reparents both windows into its own frames and
/// they are no longer siblings.
pub fn raise<C: Connection>(conn: &C, window: Window) -> Result<()> {
    conn.configure_window(
        window,
        &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
    )?;
    Ok(())
}

/// Put `window` where its `kind` belongs in the stack. Called at creation and
/// again on the daemon's periodic stacking pass.
pub fn restack<C: Connection>(conn: &C, window: Window, kind: WindowKind) -> Result<()> {
    match kind {
        WindowKind::Desktop => lower(conn, window),
        WindowKind::DdeRaised => raise(conn, window),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Distinct sentinel atoms, so the assertions below pin down both the
    /// contents and the ORDER of the property values.
    fn atoms() -> Atoms {
        Atoms {
            _NET_WM_WINDOW_TYPE: 1,
            _NET_WM_WINDOW_TYPE_DESKTOP: 2,
            _NET_WM_WINDOW_TYPE_NORMAL: 3,
            _NET_WM_STATE: 4,
            _NET_WM_STATE_BELOW: 5,
            _NET_WM_STATE_STICKY: 6,
            _NET_WM_STATE_SKIP_TASKBAR: 7,
            _NET_WM_STATE_SKIP_PAGER: 8,
            _NET_WM_STATE_FULLSCREEN: 9,
            _NET_WM_STATE_HIDDEN: 10,
            _NET_CLIENT_LIST_STACKING: 11,
            _NET_WM_NAME: 12,
            UTF8_STRING: 13,
            _MOTIF_WM_HINTS: 14,
            WM_HINTS: 15,
        }
    }

    #[test]
    fn default_kind_is_the_historic_desktop_window() {
        assert_eq!(WindowKind::default(), WindowKind::Desktop);
    }

    /// Off Deepin nothing changed: type DESKTOP only, state BELOW + STICKY +
    /// SKIP_TASKBAR + SKIP_PAGER, in that order.
    #[test]
    fn desktop_kind_properties_are_unchanged() {
        let a = atoms();
        assert_eq!(
            window_type_atoms(WindowKind::Desktop, &a),
            vec![a._NET_WM_WINDOW_TYPE_DESKTOP]
        );
        assert_eq!(
            window_state_atoms(WindowKind::Desktop, &a),
            vec![
                a._NET_WM_STATE_BELOW,
                a._NET_WM_STATE_STICKY,
                a._NET_WM_STATE_SKIP_TASKBAR,
                a._NET_WM_STATE_SKIP_PAGER,
            ]
        );
    }

    /// DDE: [DESKTOP, NORMAL] in that order, and no BELOW (it would fight the
    /// raise) while the other three states stay.
    #[test]
    fn dde_kind_declares_desktop_then_normal_without_below() {
        let a = atoms();
        assert_eq!(
            window_type_atoms(WindowKind::DdeRaised, &a),
            vec![a._NET_WM_WINDOW_TYPE_DESKTOP, a._NET_WM_WINDOW_TYPE_NORMAL]
        );
        let states = window_state_atoms(WindowKind::DdeRaised, &a);
        assert!(!states.contains(&a._NET_WM_STATE_BELOW));
        assert_eq!(
            states,
            vec![
                a._NET_WM_STATE_STICKY,
                a._NET_WM_STATE_SKIP_TASKBAR,
                a._NET_WM_STATE_SKIP_PAGER,
            ]
        );
    }
}

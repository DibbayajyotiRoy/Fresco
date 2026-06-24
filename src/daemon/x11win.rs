//! Desktop-level X11 window: sits below everything (and below the desktop-icon
//! window), spans one monitor, accepts no input, and hosts an embedded mpv.

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
        _NET_WM_STATE,
        _NET_WM_STATE_BELOW,
        _NET_WM_STATE_STICKY,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_SKIP_PAGER,
        _MOTIF_WM_HINTS,
        WM_HINTS,
    }
}

pub struct WallpaperWindow {
    pub window: Window,
    pub connector: String,
}

impl WallpaperWindow {
    /// Create, configure, map, and lower a wallpaper window for `monitor`.
    pub fn create<C: Connection>(
        conn: &C,
        screen: &Screen,
        atoms: &Atoms,
        monitor: &Monitor,
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

        // _NET_WM_WINDOW_TYPE = DESKTOP
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms._NET_WM_WINDOW_TYPE,
            AtomEnum::ATOM,
            &[atoms._NET_WM_WINDOW_TYPE_DESKTOP],
        )?;

        // _NET_WM_STATE = BELOW, STICKY, SKIP_TASKBAR, SKIP_PAGER
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            &[
                atoms._NET_WM_STATE_BELOW,
                atoms._NET_WM_STATE_STICKY,
                atoms._NET_WM_STATE_SKIP_TASKBAR,
                atoms._NET_WM_STATE_SKIP_PAGER,
            ],
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
        lower(conn, window)?;
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

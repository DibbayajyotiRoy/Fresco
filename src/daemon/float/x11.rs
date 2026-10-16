//! X11 backend of the floating world clock: an override-redirect, click-through,
//! 32-bit ARGB window that a compositing manager blends over everything.
//!
//! No compositor (or no ARGB visual) means no transparency, so [`try_start`]
//! refuses and the caller falls back to the wallpaper widget layer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::config::{LyricAnchor, WorldClock};
use crate::widgetkit::cards::worldclock::{bleed_px, render_pixmap, to_bgra_premul};
use crate::widgetkit::{scale_for_output, FontStack};

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_NOTIFICATION,
        _NET_WM_STATE,
        _NET_WM_STATE_ABOVE,
        _NET_WM_STATE_STICKY,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_SKIP_PAGER,
    }
}

/// How often the loop looks at `stop` and pending events (Expose). Bounds how
/// long `Overlay::drop` waits for the join.
const SLICE: Duration = Duration::from_millis(250);

/// Start the overlay thread. All the checks that can say "not on this desktop"
/// run here, synchronously, so the caller can fall back before anything is shown.
pub fn try_start(
    cfg: &crate::config::WorldClock,
    stop: Arc<AtomicBool>,
) -> anyhow::Result<JoinHandle<()>> {
    let (conn, screen_num) = x11rb::connect(None).context("connect to the X server")?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let visual = argb_visual(&conn, screen).context("no 32-bit ARGB TrueColor visual")?;

    let cm = conn
        .intern_atom(false, format!("_NET_WM_CM_S{screen_num}").as_bytes())?
        .reply()?
        .atom;
    if conn.get_selection_owner(cm)?.reply()?.owner == x11rb::NONE {
        bail!("no compositing manager is running (transparent overlay impossible)");
    }

    // The slow system font scan runs once, here; the thread only reuses it.
    let fonts = FontStack::system();
    if !fonts.has_fonts() {
        bail!("no fonts installed");
    }
    let atoms = Atoms::new(&conn)?.reply()?;
    let cfg = cfg.clone();

    std::thread::Builder::new()
        .name("worldclock-x11".into())
        .spawn(move || {
            if let Err(e) = run(&conn, root, visual, &atoms, fonts, &cfg, &stop) {
                log::warn!("world clock overlay (X11) stopped: {e:#}");
            }
        })
        .context("spawn world clock thread")
}

/// A 32-bit TrueColor visual whose layout is BGRA in little-endian memory,
/// which is what [`to_bgra_premul`] produces.
fn argb_visual(conn: &RustConnection, screen: &Screen) -> Option<Visualid> {
    if conn.setup().image_byte_order != ImageOrder::LSB_FIRST {
        return None;
    }
    screen
        .allowed_depths
        .iter()
        .filter(|d| d.depth == 32)
        .flat_map(|d| &d.visuals)
        .find(|v| {
            v.class == VisualClass::TRUE_COLOR
                && (v.red_mask, v.green_mask, v.blue_mask) == (0xff0000, 0xff00, 0xff)
        })
        .map(|v| v.visual_id)
}

fn run(
    conn: &RustConnection,
    root: Window,
    visual: Visualid,
    atoms: &Atoms,
    mut fonts: FontStack,
    cfg: &WorldClock,
    stop: &AtomicBool,
) -> Result<()> {
    let win = conn.generate_id()?;
    let result = event_loop(conn, root, visual, atoms, &mut fonts, cfg, stop, win);
    let _ = conn.destroy_window(win);
    let _ = conn.flush();
    result
}

#[allow(clippy::too_many_arguments)]
fn event_loop(
    conn: &RustConnection,
    root: Window,
    visual: Visualid,
    atoms: &Atoms,
    fonts: &mut FontStack,
    cfg: &WorldClock,
    stop: &AtomicBool,
    win: Window,
) -> Result<()> {
    let cmap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, cmap, root, visual)?;
    // Depth 32 needs its own colormap and border pixel, or CreateWindow is BadMatch.
    conn.create_window(
        32,
        win,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &CreateWindowAux::new()
            .background_pixel(0)
            .border_pixel(0)
            .colormap(cmap)
            .override_redirect(1)
            .event_mask(EventMask::EXPOSURE),
    )?;
    conn.change_property32(
        PropMode::REPLACE,
        win,
        atoms._NET_WM_WINDOW_TYPE,
        AtomEnum::ATOM,
        &[atoms._NET_WM_WINDOW_TYPE_NOTIFICATION],
    )?;
    // Override-redirect already keeps the WM away; these help the few that look anyway.
    conn.change_property32(
        PropMode::REPLACE,
        win,
        atoms._NET_WM_STATE,
        AtomEnum::ATOM,
        &[
            atoms._NET_WM_STATE_ABOVE,
            atoms._NET_WM_STATE_STICKY,
            atoms._NET_WM_STATE_SKIP_TASKBAR,
            atoms._NET_WM_STATE_SKIP_PAGER,
        ],
    )?;
    conn.change_property8(
        PropMode::REPLACE,
        win,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        b"frescod\0Fresco\0",
    )?;
    conn.change_property8(
        PropMode::REPLACE,
        win,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        b"Fresco World Clock",
    )?;
    // An empty INPUT shape makes every click fall through to the window below.
    conn.shape_rectangles(SO::SET, SK::INPUT, ClipOrdering::UNSORTED, win, 0, 0, &[])?;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, win, &CreateGCAux::new())?;

    let (mut mapped, mut dirty, mut next) = (false, true, Utc::now());
    while !stop.load(Ordering::Relaxed) {
        while let Some(ev) = conn.poll_for_event()? {
            dirty |= matches!(ev, Event::Expose(_));
        }
        let now = Utc::now();
        // `now >= next` rather than a monotonic deadline: survives suspend and clock steps.
        if dirty || now >= next {
            dirty = false;
            next = crate::worldclock::next_change(now);
            let (mx, my, mw, mh) = monitor_rect(conn, root)?;
            let scale = scale_for_output(mh as u32);
            let rows = crate::worldclock::rows(cfg, now);
            match render_pixmap(fonts, &rows, cfg.font_size_pt, scale) {
                None if mapped => {
                    conn.unmap_window(win)?;
                    mapped = false;
                }
                None => {}
                Some(pixmap) => {
                    let (w, h) = (pixmap.width(), pixmap.height());
                    // The pixmap carries a shadow margin; the glyphs already sit inside it.
                    let margin =
                        (cfg.margin_px as i32 - bleed_px(cfg.font_size_pt, scale) as i32).max(0);
                    let (x, y) = place(cfg.anchor, margin, (mx, my, mw, mh), w as i32, h as i32);
                    // Re-raise on every redraw so a fullscreen-ish window can't bury us.
                    conn.configure_window(
                        win,
                        &ConfigureWindowAux::new()
                            .x(x)
                            .y(y)
                            .width(w)
                            .height(h)
                            .stack_mode(StackMode::ABOVE),
                    )?;
                    if !mapped {
                        conn.map_window(win)?;
                        mapped = true;
                    }
                    put_bgra(conn, win, gc, w, &to_bgra_premul(&pixmap))?;
                }
            }
            conn.flush()?;
        }
        std::thread::sleep(SLICE);
    }
    Ok(())
}

/// Upload `bgra` (`w` pixels per row) in bands that fit one request.
fn put_bgra(conn: &RustConnection, win: Window, gc: Gcontext, w: u32, bgra: &[u8]) -> Result<()> {
    let stride = w as usize * 4;
    // PutImage has a 24-byte header; leave a little slack.
    let band_rows = (conn.maximum_request_bytes().saturating_sub(64) / stride).max(1);
    for (i, band) in bgra.chunks(stride * band_rows).enumerate() {
        let y = i16::try_from(i * band_rows).context("image too tall")?;
        conn.put_image(
            ImageFormat::Z_PIXMAP,
            win,
            gc,
            u16::try_from(w).context("image too wide")?,
            (band.len() / stride) as u16,
            0,
            y,
            0,
            32,
            band,
        )?;
    }
    Ok(())
}

/// The primary RandR monitor as `(x, y, width, height)`; the first monitor if
/// none is primary, the whole root window if RandR has nothing to say.
fn monitor_rect(conn: &RustConnection, root: Window) -> Result<(i32, i32, i32, i32)> {
    let mons = conn
        .randr_get_monitors(root, true)?
        .reply()
        .map(|r| r.monitors);
    if let Some(m) = mons
        .iter()
        .flatten()
        .find(|m| m.primary)
        .or_else(|| mons.iter().flatten().next())
    {
        return Ok((m.x.into(), m.y.into(), m.width.into(), m.height.into()));
    }
    let g = conn.get_geometry(root)?.reply()?;
    Ok((0, 0, g.width.into(), g.height.into()))
}

/// Top-left of a `w` x `h` box on the 9-point grid inside `mon`, `margin` px
/// from whichever edges the anchor names.
fn place(
    anchor: LyricAnchor,
    margin: i32,
    mon: (i32, i32, i32, i32),
    w: i32,
    h: i32,
) -> (i32, i32) {
    use LyricAnchor::*;
    let (mx, my, mw, mh) = mon;
    let x = match anchor {
        TopLeft | MidLeft | BottomLeft => mx + margin,
        TopCenter | MidCenter | BottomCenter => mx + (mw - w) / 2,
        TopRight | MidRight | BottomRight => mx + mw - w - margin,
    };
    let y = match anchor {
        TopLeft | TopCenter | TopRight => my + margin,
        MidLeft | MidCenter | MidRight => my + (mh - h) / 2,
        BottomLeft | BottomCenter | BottomRight => my + mh - h - margin,
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldZone;

    #[test]
    fn anchors_land_on_the_right_edges() {
        let mon = (100, 50, 1920, 1080);
        assert_eq!(place(LyricAnchor::TopLeft, 10, mon, 200, 100), (110, 60));
        assert_eq!(
            place(LyricAnchor::BottomRight, 10, mon, 200, 100),
            (1810, 1020)
        );
        assert_eq!(place(LyricAnchor::MidCenter, 10, mon, 200, 100), (960, 540));
    }

    /// Needs a live X display with a compositing manager; run by hand.
    #[test]
    #[ignore]
    fn live_overlay() {
        let zone = |tz: &str| WorldZone {
            tz: tz.into(),
            label: String::new(),
        };
        let cfg = WorldClock {
            enabled: true,
            zones: vec![zone("Asia/Tokyo"), zone("America/New_York")],
            ..WorldClock::default()
        };
        let stop = Arc::new(AtomicBool::new(false));
        let join = try_start(&cfg, stop.clone()).expect("try_start");
        std::thread::sleep(Duration::from_secs(3));
        stop.store(true, Ordering::Relaxed);
        join.join().expect("overlay thread panicked");
    }
}

//! MATE (issue #18): Caja's desktop icons, drawn over the live wallpaper.
//!
//! # Why this exists
//!
//! On MATE, Caja draws the desktop — its icons *and* its own copy of the
//! background — into one opaque, full-screen, 24-bit window. It never asks for
//! a transparent visual, paints its background over the whole window on every
//! redraw, and has no setting to stop. So no window stacked below it is ever
//! seen, and a window stacked above it hides the icons. Every live-wallpaper
//! tool on MATE picks one of those two failures; Fresco's first MATE fix
//! (`dde::apply_mate`, "restack") picked the second.
//!
//! # What this does instead
//!
//! Caja stays exactly where it is — full-size, *below* the wallpaper — so every
//! click still reaches it: Fresco's windows have an empty input region, and a
//! click falls through them to Caja. Right-click menus, rubber-band selection
//! and drag-and-drop all keep working, because they are still Caja's.
//!
//! What the user *sees* of Caja is a copy:
//!
//! 1. Caja is told to paint a solid key colour ([`KEY_HEX`]) as its background.
//! 2. Its window is redirected offscreen with the Composite extension, so it
//!    keeps rendering into a pixmap even while the wallpaper covers it.
//! 3. For each wallpaper window, a child window — input-less, so clicks still
//!    fall through — receives a server-side copy of that pixmap, cut down with
//!    an X Shape to every pixel that is *not* the key colour: the icons, their
//!    labels, the selection highlight.
//! 4. The Damage extension says which part of Caja changed, so only that part
//!    is re-read and re-copied.
//!
//! A child window draws above its parent's contents — here, mpv's video — and
//! moves with it, so there is no stacking to fight over with the window
//! manager for the icons themselves.
//!
//! The key colour is near-black on purpose. Anti-aliased icon edges and label
//! pixels are blends of the real colour with the key; exact matching keeps
//! them, and blended toward black they read as the label shadow Caja already
//! draws, rather than as a coloured fringe.
//!
//! # The one thing the window manager still does
//!
//! Clicking the desktop makes Marco raise Caja, which would show Caja's
//! key-coloured window over the video. Marco ignores a raise of *our* windows
//! at that point (it only honours stacking requests from the active
//! application), but it does honour lowering Caja's own window — measured on
//! Linux Mint 22 MATE. So this thread watches the stacking order and pushes
//! Caja back down the moment it comes up.
//!
//! # A thread with its own connection
//!
//! A Composite redirect lives only as long as the client that made it, and
//! Damage events are delivered to the client that created the damage object.
//! Owning both on a dedicated connection keeps them out of the daemon's event
//! loop (which deliberately ignores events — see `Daemon::run`) and lets this
//! thread block on the X socket and react within milliseconds, instead of on
//! the daemon's 100 ms / 2 s ticks. When the daemon dies, the connection closes
//! and the X server undoes the redirect, the damage object and the child
//! windows by itself — Caja renders normally again. Only the key-colour
//! background is left, and [`restore_background`] runs at the next start.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::composite::{self, ConnectionExt as _};
use x11rb::protocol::damage::{self, ConnectionExt as _};
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xfixes::ConnectionExt as _;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::{COPY_DEPTH_FROM_PARENT, NONE};

/// The background Caja paints while the mirror runs. Near-black, so the
/// anti-aliased edges it leaves on icons and labels read as a shadow.
pub const KEY_HEX: &str = "#010101";
/// [`KEY_HEX`] as bytes.
const KEY: [u8; 3] = [1, 1, 1];

/// Rows read per `GetImage`, so a full refresh of a 4K desktop is a series of
/// modest replies rather than one 33 MB one.
const BAND_ROWS: u16 = 128;

/// One of the daemon's wallpaper windows and the part of the screen it covers,
/// in root coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parent {
    pub window: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Parent {
    fn rect(&self) -> Rectangle {
        Rectangle {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

enum Cmd {
    Parents(Vec<Parent>),
    Stop,
}

/// Handle to the mirror thread. Dropping it without [`Mirror::stop`] leaves the
/// thread running until the process exits, which is harmless.
pub struct Mirror {
    tx: Sender<Cmd>,
    /// An input-only window owned by the thread's connection; a client message
    /// sent to it wakes the thread out of its blocking wait.
    wake: Window,
    failed: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Mirror {
    /// Start the thread. Fails when it cannot connect or when the server lacks
    /// Composite, Damage or XFixes — the caller then keeps the restack mode.
    pub fn start() -> Result<Mirror> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<Window, String>>();
        let failed = Arc::new(AtomicBool::new(false));
        let thread_failed = failed.clone();
        let thread = std::thread::Builder::new()
            .name("caja-mirror".into())
            .spawn(move || {
                if let Err(e) = run(rx, &ready_tx) {
                    log::warn!("MATE: icon mirror stopped: {e:#}");
                    thread_failed.store(true, Ordering::SeqCst);
                    let _ = ready_tx.send(Err(format!("{e:#}")));
                }
            })
            .context("spawning the Caja mirror thread")?;
        let wake = match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(w)) => w,
            Ok(Err(e)) => bail!("{e}"),
            Err(_) => bail!("the Caja mirror thread did not start"),
        };
        Ok(Mirror {
            tx,
            wake,
            failed,
            thread: Some(thread),
        })
    }

    /// Hand the thread the current wallpaper windows. Children on windows that
    /// are gone are dropped; new windows get one.
    pub fn set_parents<C: Connection>(&self, conn: &C, parents: Vec<Parent>) {
        let _ = self.tx.send(Cmd::Parents(parents));
        self.wake(conn);
    }

    /// True once the thread has given up (lost the display, or found a Caja it
    /// cannot mirror). The daemon falls back to restacking.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }

    /// Stop the thread and wait for it. Closing its connection undoes the
    /// redirect and destroys the icon windows.
    pub fn stop<C: Connection>(mut self, conn: &C) {
        let _ = self.tx.send(Cmd::Stop);
        self.wake(conn);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    fn wake<C: Connection>(&self, conn: &C) {
        let ev = ClientMessageEvent::new(32, self.wake, AtomEnum::NOTICE, [0u32; 5]);
        let _ = conn.send_event(false, self.wake, EventMask::NO_EVENT, ev);
        let _ = conn.flush();
    }
}

/// Caja's desktop window, redirected offscreen.
struct Caja {
    window: Window,
    pixmap: Pixmap,
    damage: damage::Damage,
    /// Where the window's origin is on the root, and its size.
    rect: Rectangle,
}

/// One icon window, a child of one wallpaper window.
struct Child {
    parent: Parent,
    window: Window,
    gc: Gcontext,
    /// Per pixel of the parent: true where Caja drew something other than
    /// the key colour — i.e. where the icon window is visible.
    mask: Vec<bool>,
}

struct State {
    conn: RustConnection,
    root: Window,
    root_depth: u8,
    stacking: Atom,
    wake: Window,
    msb_first: bool,
    caja: Option<Caja>,
    children: Vec<Child>,
}

fn run(rx: Receiver<Cmd>, ready: &Sender<Result<Window, String>>) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(None).context("connecting to X11")?;
    let screen = conn.setup().roots[screen_num].clone();
    for name in [
        composite::X11_EXTENSION_NAME,
        damage::X11_EXTENSION_NAME,
        x11rb::protocol::xfixes::X11_EXTENSION_NAME,
    ] {
        if conn.extension_information(name)?.is_none() {
            bail!("the X server has no {name} extension");
        }
    }
    conn.composite_query_version(0, 4)?.reply()?;
    conn.xfixes_query_version(5, 0)?.reply()?;
    conn.damage_query_version(1, 1)?.reply()?;
    let stacking = conn
        .intern_atom(false, b"_NET_CLIENT_LIST_STACKING")?
        .reply()?
        .atom;
    let msb_first = conn.setup().image_byte_order == ImageOrder::MSB_FIRST;

    let wake = conn.generate_id()?;
    conn.create_window(
        0,
        wake,
        screen.root,
        -1,
        -1,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().override_redirect(1),
    )?;
    // Stacking changes arrive as property changes on the root.
    conn.change_window_attributes(
        screen.root,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?;
    conn.flush()?;
    let _ = ready.send(Ok(wake));

    let mut st = State {
        conn,
        root: screen.root,
        root_depth: screen.root_depth,
        stacking,
        wake,
        msb_first,
        caja: None,
        children: Vec::new(),
    };

    loop {
        loop {
            match rx.try_recv() {
                Ok(Cmd::Parents(p)) => st.set_parents(p)?,
                Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => return Ok(()),
                Err(TryRecvError::Empty) => break,
            }
        }
        if st.caja.is_none() {
            st.attach()?;
        }
        st.guard_stacking()?;
        st.conn.flush()?;

        let mut dirty: Option<Rectangle> = None;
        let mut restack = false;
        let ev = st.conn.wait_for_event()?;
        st.handle(ev, &mut dirty, &mut restack)?;
        while let Some(ev) = st.conn.poll_for_event()? {
            st.handle(ev, &mut dirty, &mut restack)?;
        }
        if let Some(r) = dirty {
            st.refresh(r)?;
        }
        if restack {
            st.raise_children()?;
        }
    }
}

impl State {
    /// Find Caja's desktop window and start mirroring it. A no-op while Caja
    /// is not up yet — a stacking change wakes us when it arrives.
    fn attach(&mut self) -> Result<()> {
        let Some(window) = self.find_caja() else {
            return Ok(());
        };
        let conn = &self.conn;
        let Some(geom) = conn.get_geometry(window)?.reply().ok() else {
            return Ok(());
        };
        if geom.depth != self.root_depth {
            bail!(
                "Caja's desktop window is {}-bit on a {}-bit screen; it cannot be copied \
                 onto the wallpaper",
                geom.depth,
                self.root_depth
            );
        }
        let bpp = conn
            .setup()
            .pixmap_formats
            .iter()
            .find(|f| f.depth == geom.depth)
            .map(|f| f.bits_per_pixel);
        if bpp != Some(32) {
            bail!("unsupported pixel format for Caja's desktop ({bpp:?} bits per pixel)");
        }
        let Some(origin) = conn
            .translate_coordinates(window, self.root, 0, 0)?
            .reply()
            .ok()
        else {
            return Ok(());
        };
        conn.change_window_attributes(
            window,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
        )?;
        conn.composite_redirect_window(window, composite::Redirect::AUTOMATIC)?;
        let pixmap = conn.generate_id()?;
        conn.composite_name_window_pixmap(window, pixmap)?;
        let damage = conn.generate_id()?;
        conn.damage_create(damage, window, damage::ReportLevel::BOUNDING_BOX)?;
        // The fresh offscreen copy holds whatever was on screen; make Caja
        // paint itself into it. The damage that follows drives the first copy.
        conn.clear_area(true, window, 0, 0, 0, 0)?;
        conn.flush()?;
        self.caja = Some(Caja {
            window,
            pixmap,
            damage,
            rect: Rectangle {
                x: origin.dst_x,
                y: origin.dst_y,
                width: geom.width,
                height: geom.height,
            },
        });
        log::info!("MATE: mirroring Caja's desktop icons over the wallpaper");
        Ok(())
    }

    fn find_caja(&self) -> Option<Window> {
        let stack = self.stack();
        stack.into_iter().find(|&w| {
            self.conn
                .get_property(false, w, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some_and(|p| super::dde::wm_class_is_caja_desktop(&p.value))
        })
    }

    /// `_NET_CLIENT_LIST_STACKING`, bottom-most first; empty when unreadable.
    fn stack(&self) -> Vec<Window> {
        self.conn
            .get_property(false, self.root, self.stacking, AtomEnum::WINDOW, 0, 4096)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default()
    }

    /// Push Caja back below the wallpaper if a click brought it up.
    fn guard_stacking(&self) -> Result<()> {
        let Some(caja) = &self.caja else {
            return Ok(());
        };
        let ours: Vec<Window> = self.children.iter().map(|c| c.parent.window).collect();
        if caja_above_ours(&self.stack(), caja.window, &ours) {
            self.conn.configure_window(
                caja.window,
                &ConfigureWindowAux::new().stack_mode(StackMode::BELOW),
            )?;
            log::debug!("MATE: Caja came up over the wallpaper; lowered it");
        }
        Ok(())
    }

    fn set_parents(&mut self, parents: Vec<Parent>) -> Result<()> {
        // Drop children of windows that are gone. Destroying a parent already
        // took its child with it, so a BadWindow here is expected and ignored.
        let mut kept = Vec::new();
        for c in self.children.drain(..) {
            if parents.contains(&c.parent) {
                kept.push(c);
            } else {
                let _ = self.conn.destroy_window(c.window);
                let _ = self.conn.free_gc(c.gc);
            }
        }
        self.children = kept;
        for p in parents {
            if self.children.iter().any(|c| c.parent == p) || p.width == 0 || p.height == 0 {
                continue;
            }
            match self.create_child(p) {
                Ok(c) => self.children.push(c),
                Err(e) => log::warn!(
                    "MATE: could not add icons over window {:#x}: {e:#}",
                    p.window
                ),
            }
        }
        if let Some(caja) = &self.caja {
            let r = caja.rect;
            self.refresh(r)?;
        }
        self.conn.flush()?;
        Ok(())
    }

    fn create_child(&self, p: Parent) -> Result<Child> {
        let conn = &self.conn;
        let window = conn.generate_id()?;
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            p.window,
            0,
            0,
            p.width,
            p.height,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            // No background: the server must not clear it to anything before
            // the first copy lands.
            &CreateWindowAux::new().event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY),
        )?;
        // Clicks fall through to Caja; nothing is visible until the first copy.
        conn.shape_rectangles(
            shape::SO::SET,
            shape::SK::INPUT,
            ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[],
        )?;
        conn.shape_rectangles(
            shape::SO::SET,
            shape::SK::BOUNDING,
            ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[],
        )?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, window, &CreateGCAux::new().graphics_exposures(0))?;
        conn.map_window(window)?;
        conn.configure_window(
            window,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        )?;
        // mpv may map its own window inside the parent later; hear about it so
        // the icons can be put back on top.
        conn.change_window_attributes(
            p.window,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY),
        )?;
        Ok(Child {
            parent: p,
            window,
            gc,
            mask: vec![false; usize::from(p.width) * usize::from(p.height)],
        })
    }

    fn raise_children(&self) -> Result<()> {
        for c in &self.children {
            self.conn.configure_window(
                c.window,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            )?;
        }
        self.conn.flush()?;
        Ok(())
    }

    fn handle(
        &mut self,
        ev: Event,
        dirty: &mut Option<Rectangle>,
        restack: &mut bool,
    ) -> Result<()> {
        match ev {
            Event::DamageNotify(e) => {
                if let Some(caja) = &self.caja {
                    if e.damage == caja.damage {
                        self.conn.damage_subtract(caja.damage, NONE, NONE)?;
                        let area = Rectangle {
                            x: caja.rect.x.saturating_add(e.area.x),
                            y: caja.rect.y.saturating_add(e.area.y),
                            ..e.area
                        };
                        *dirty = Some(union(*dirty, area));
                    }
                }
            }
            Event::Expose(e) => {
                if let Some(c) = self.children.iter().find(|c| c.window == e.window) {
                    let area = Rectangle {
                        x: c.parent.x.saturating_add(e.x as i16),
                        y: c.parent.y.saturating_add(e.y as i16),
                        width: e.width,
                        height: e.height,
                    };
                    *dirty = Some(union(*dirty, area));
                }
            }
            Event::PropertyNotify(e) if e.window == self.root && e.atom == self.stacking => {
                // Handled every loop by `guard_stacking`, and a Caja that has
                // just appeared is attached there too.
            }
            Event::MapNotify(e) => {
                let is_child = self.children.iter().any(|c| c.window == e.window);
                if !is_child && self.children.iter().any(|c| c.parent.window == e.event) {
                    *restack = true;
                }
            }
            Event::ConfigureNotify(e) => {
                if let Some(caja) = &mut self.caja {
                    if e.window == caja.window
                        && (e.width != caja.rect.width || e.height != caja.rect.height)
                    {
                        // The offscreen copy is per size; name the new one.
                        let _ = self.conn.free_pixmap(caja.pixmap);
                        let pixmap = self.conn.generate_id()?;
                        self.conn
                            .composite_name_window_pixmap(caja.window, pixmap)?;
                        caja.pixmap = pixmap;
                        caja.rect.width = e.width;
                        caja.rect.height = e.height;
                        *dirty = Some(union(*dirty, caja.rect));
                    }
                }
            }
            Event::DestroyNotify(e) => {
                if self.caja.as_ref().is_some_and(|c| c.window == e.window) {
                    log::info!("MATE: Caja's desktop window went away; waiting for it to return");
                    self.caja = None;
                    for c in &mut self.children {
                        c.mask.iter_mut().for_each(|m| *m = false);
                        let _ = self.conn.shape_rectangles(
                            shape::SO::SET,
                            shape::SK::BOUNDING,
                            ClipOrdering::UNSORTED,
                            c.window,
                            0,
                            0,
                            &[],
                        );
                    }
                }
                self.children.retain(|c| c.window != e.window);
            }
            Event::ClientMessage(e) if e.window == self.wake => {}
            Event::Error(e) => log::debug!("MATE mirror: X error {e:?}"),
            _ => {}
        }
        Ok(())
    }

    /// Re-read `area` (root coordinates) of Caja's offscreen copy and bring
    /// every icon window it touches up to date.
    fn refresh(&mut self, area: Rectangle) -> Result<()> {
        let Some(caja) = &self.caja else {
            return Ok(());
        };
        let Some(area) = intersect(area, caja.rect) else {
            return Ok(());
        };
        let conn = &self.conn;
        for c in &mut self.children {
            let Some(part) = intersect(area, c.parent.rect()) else {
                continue;
            };
            let pw = usize::from(c.parent.width);
            let mut row = 0u16;
            while row < part.height {
                let h = BAND_ROWS.min(part.height - row);
                let src_x = part.x - caja.rect.x;
                let src_y = part.y - caja.rect.y + row as i16;
                let Some(img) = conn
                    .get_image(
                        ImageFormat::Z_PIXMAP,
                        caja.pixmap,
                        src_x,
                        src_y,
                        part.width,
                        h,
                        !0,
                    )?
                    .reply()
                    .ok()
                else {
                    return Ok(());
                };
                let stride = usize::from(part.width) * 4;
                for dy in 0..usize::from(h) {
                    let py =
                        usize::try_from(part.y - c.parent.y).unwrap_or(0) + usize::from(row) + dy;
                    let px0 = usize::try_from(part.x - c.parent.x).unwrap_or(0);
                    let line = img.data.get(dy * stride..(dy + 1) * stride).unwrap_or(&[]);
                    for (dx, px) in line.chunks_exact(4).enumerate() {
                        if let Some(m) = c.mask.get_mut(py * pw + px0 + dx) {
                            *m = !is_key(px, self.msb_first);
                        }
                    }
                }
                row += h;
            }
            conn.copy_area(
                caja.pixmap,
                c.window,
                c.gc,
                part.x - caja.rect.x,
                part.y - caja.rect.y,
                part.x - c.parent.x,
                part.y - c.parent.y,
                part.width,
                part.height,
            )?;
            let rects = mask_rects(&c.mask, c.parent.width, c.parent.height);
            conn.shape_rectangles(
                shape::SO::SET,
                shape::SK::BOUNDING,
                ClipOrdering::YX_SORTED,
                c.window,
                0,
                0,
                &rects,
            )?;
        }
        conn.flush()?;
        Ok(())
    }
}

/// Whether a 32-bit ZPixmap pixel is the key colour, for either byte order.
fn is_key(px: &[u8], msb_first: bool) -> bool {
    let rgb = if msb_first {
        [px[1], px[2], px[3]]
    } else {
        [px[2], px[1], px[0]]
    };
    rgb == KEY
}

/// The visible pixels of a `width × height` mask as Shape rectangles: one per
/// horizontal run, with identical consecutive rows merged into one taller
/// rectangle. Sorted by y, then x, as `YX_SORTED` promises.
fn mask_rects(mask: &[bool], width: u16, height: u16) -> Vec<Rectangle> {
    let (w, h) = (usize::from(width), usize::from(height));
    let mut out: Vec<Rectangle> = Vec::new();
    // Runs of the previous row, with the index of the rectangle each started.
    let mut prev: Vec<(u16, u16, usize)> = Vec::new();
    for y in 0..h {
        let row = mask.get(y * w..(y + 1) * w).unwrap_or(&[]);
        let mut runs: Vec<(u16, u16)> = Vec::new();
        let mut x = 0;
        while x < row.len() {
            if !row[x] {
                x += 1;
                continue;
            }
            let start = x;
            while x < row.len() && row[x] {
                x += 1;
            }
            runs.push((start as u16, (x - start) as u16));
        }
        let same = runs.len() == prev.len()
            && runs
                .iter()
                .zip(&prev)
                .all(|(a, b)| a.0 == b.0 && a.1 == b.1);
        if same {
            for &(_, _, i) in &prev {
                out[i].height += 1;
            }
        } else {
            prev = runs
                .iter()
                .map(|&(x0, len)| {
                    out.push(Rectangle {
                        x: x0 as i16,
                        y: y as i16,
                        width: len,
                        height: 1,
                    });
                    (x0, len, out.len() - 1)
                })
                .collect();
        }
    }
    // Merging grows earlier rectangles downward, so restore y-then-x order.
    out.sort_by_key(|r| (r.y, r.x));
    out
}

fn intersect(a: Rectangle, b: Rectangle) -> Option<Rectangle> {
    let x0 = i32::from(a.x).max(i32::from(b.x));
    let y0 = i32::from(a.y).max(i32::from(b.y));
    let x1 = (i32::from(a.x) + i32::from(a.width)).min(i32::from(b.x) + i32::from(b.width));
    let y1 = (i32::from(a.y) + i32::from(a.height)).min(i32::from(b.y) + i32::from(b.height));
    (x1 > x0 && y1 > y0).then(|| Rectangle {
        x: x0 as i16,
        y: y0 as i16,
        width: (x1 - x0) as u16,
        height: (y1 - y0) as u16,
    })
}

fn union(a: Option<Rectangle>, b: Rectangle) -> Rectangle {
    let Some(a) = a else { return b };
    let x0 = i32::from(a.x).min(i32::from(b.x));
    let y0 = i32::from(a.y).min(i32::from(b.y));
    let x1 = (i32::from(a.x) + i32::from(a.width)).max(i32::from(b.x) + i32::from(b.width));
    let y1 = (i32::from(a.y) + i32::from(a.height)).max(i32::from(b.y) + i32::from(b.height));
    Rectangle {
        x: x0 as i16,
        y: y0 as i16,
        width: (x1 - x0).min(i32::from(u16::MAX)) as u16,
        height: (y1 - y0).min(i32::from(u16::MAX)) as u16,
    }
}

/// True when `caja` sits above the lowest of `ours` in `stack` (bottom-most
/// first). False whenever either is missing: an unreadable stack is no reason
/// to move anything.
fn caja_above_ours(stack: &[Window], caja: Window, ours: &[Window]) -> bool {
    let pos = |w: Window| stack.iter().position(|&s| s == w);
    let Some(c) = pos(caja) else { return false };
    ours.iter()
        .filter_map(|&w| pos(w))
        .min()
        .is_some_and(|lowest| c > lowest)
}

// -- The key-colour background ------------------------------------------------

const BG_SCHEMA: &str = "org.mate.background";
/// What Caja is asked to paint while the mirror runs: no picture, one solid
/// colour, the key.
const BG_KEYS: [(&str, &str); 3] = [
    ("picture-filename", "''"),
    ("color-shading-type", "'solid'"),
    ("primary-color", "'#010101'"),
];

static KEY_ACTIVE: AtomicBool = AtomicBool::new(false);

fn bg_state_file() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fresco")
        .join("mate-mirror-background")
}

/// True while Caja is painting the key colour on our behalf. The still-frame
/// writer (`overview`) checks it: its picture would replace the key and every
/// icon would vanish into a photograph.
pub fn key_active() -> bool {
    KEY_ACTIVE.load(Ordering::SeqCst)
}

/// Save the user's MATE background once and switch Caja to the key colour.
/// False when the MATE background settings cannot be driven at all.
pub fn apply_key() -> bool {
    let sf = bg_state_file();
    if !sf.exists() {
        let mut saved = String::new();
        for (key, _) in BG_KEYS {
            let Some(v) = gsettings_get(key) else {
                return false;
            };
            saved.push_str(&format!("{key}\t{v}\n"));
        }
        if let Some(d) = sf.parent() {
            std::fs::create_dir_all(d).ok();
        }
        if std::fs::write(&sf, saved).is_err() {
            return false;
        }
    }
    for (key, value) in BG_KEYS {
        gsettings_set(key, value);
    }
    KEY_ACTIVE.store(true, Ordering::SeqCst);
    log::info!("MATE: desktop background set to the icon key colour {KEY_HEX}");
    true
}

/// Put the user's MATE background back. A no-op when nothing was saved — so it
/// is safe on every desktop and at every start, which is how a crashed run's
/// key colour gets cleaned up.
pub fn restore_background() {
    KEY_ACTIVE.store(false, Ordering::SeqCst);
    let sf = bg_state_file();
    let Ok(text) = std::fs::read_to_string(&sf) else {
        return;
    };
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('\t') {
            if BG_KEYS.iter().any(|(k, _)| *k == key) && !value.is_empty() {
                gsettings_set(key, value);
            }
        }
    }
    std::fs::remove_file(&sf).ok();
    log::info!("MATE: desktop background restored");
}

fn gsettings_get(key: &str) -> Option<String> {
    let out = Command::new("gsettings")
        .args(["get", BG_SCHEMA, key])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn gsettings_set(key: &str, gvariant: &str) {
    let _ = Command::new("gsettings")
        .args(["set", BG_SCHEMA, key, gvariant])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// x11rb's `Rectangle` has no `PartialEq`; compare its fields instead.
    fn t(r: Rectangle) -> (i16, i16, u16, u16) {
        (r.x, r.y, r.width, r.height)
    }

    #[test]
    fn the_key_is_matched_in_either_byte_order() {
        // LSB-first ZPixmap is B, G, R, pad; MSB-first is pad, R, G, B.
        assert!(is_key(&[1, 1, 1, 0], false));
        assert!(is_key(&[0, 1, 1, 1], true));
        assert!(!is_key(&[1, 1, 2, 0], false));
        // Pure black is the label shadow, not the key: it must stay visible.
        assert!(!is_key(&[0, 0, 0, 0], false));
        assert_eq!(KEY_HEX, "#010101");
        assert_eq!(BG_KEYS[2].1, "'#010101'");
    }

    #[test]
    fn mask_rects_cover_exactly_the_visible_pixels() {
        // 4×3: an L shape and a lone pixel.
        #[rustfmt::skip]
        let mask = [
            true,  true,  false, false,
            true,  true,  false, true,
            true,  false, false, false,
        ];
        let rects = mask_rects(&mask, 4, 3);
        let mut painted = [false; 12];
        for r in &rects {
            for y in r.y..r.y + r.height as i16 {
                for x in r.x..r.x + r.width as i16 {
                    let i = y as usize * 4 + x as usize;
                    assert!(!painted[i], "pixel {i} covered twice: {rects:?}");
                    painted[i] = true;
                }
            }
        }
        assert_eq!(painted, mask, "{rects:?}");
        // Rows 0 and 1 share their first run, but row 1 has a second, so they
        // do not merge; sorted y-then-x all the same.
        assert!(rects
            .windows(2)
            .all(|w| (w[0].y, w[0].x) <= (w[1].y, w[1].x)));
    }

    #[test]
    fn identical_rows_merge_into_one_rectangle() {
        let mask = [true, true, false, true, true, false, true, true, false];
        let rects: Vec<_> = mask_rects(&mask, 3, 3).into_iter().map(t).collect();
        assert_eq!(rects, vec![(0, 0, 2, 3)]);
        assert!(mask_rects(&[false; 9], 3, 3).is_empty());
    }

    #[test]
    fn rectangles_intersect_and_union() {
        let r = |x, y, width, height| Rectangle {
            x,
            y,
            width,
            height,
        };
        let i = |a, b| intersect(a, b).map(t);
        assert_eq!(i(r(0, 0, 10, 10), r(5, 5, 10, 10)), Some((5, 5, 5, 5)));
        assert_eq!(i(r(0, 0, 10, 10), r(10, 0, 5, 5)), None);
        // A second monitor to the left of the primary.
        assert_eq!(
            i(r(-1920, 0, 1920, 1080), r(-100, 10, 200, 20)),
            Some((-100, 10, 100, 20))
        );
        assert_eq!(t(union(None, r(1, 2, 3, 4))), (1, 2, 3, 4));
        assert_eq!(t(union(Some(r(0, 0, 2, 2)), r(5, 5, 1, 1))), (0, 0, 6, 6));
    }

    #[test]
    fn caja_is_lowered_only_when_it_is_really_above_us() {
        // Bottom-most first; 10 = Caja, 1/2 = our wallpaper windows, 50 = an app.
        assert!(!caja_above_ours(&[10, 1, 2, 50], 10, &[1, 2]));
        assert!(caja_above_ours(&[1, 10, 2, 50], 10, &[1, 2]));
        assert!(caja_above_ours(&[1, 2, 10], 10, &[1, 2]));
        assert!(!caja_above_ours(&[], 10, &[1]));
        assert!(!caja_above_ours(&[1, 2], 10, &[1, 2]));
        assert!(!caja_above_ours(&[10, 50], 10, &[1]));
    }
}

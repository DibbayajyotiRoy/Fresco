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
//!
//! # Deepin (issue #33), experimental
//!
//! The same mirror also runs against Deepin 25's dde-shell desktop window
//! ([`Desktop::Dde`], opt-in via `dde_mode = "mirror"`). Three things differ:
//!
//! * the window is 32-bit ARGB, not the screen's 24-bit, so the icon windows
//!   are created with that window's visual (a `CopyArea` needs source and
//!   destination of one depth) and the key test ignores alpha;
//! * the key colour is a solid PNG applied with DDE's Appearance DBus service
//!   (`dde::apply_key_background`), and DDE's wallpaper cache may re-encode or
//!   rescale it, so a channel within [`DDE_TOLERANCE`] of the key counts;
//! * KWin, not Marco, is the window manager — whether it honours lowering the
//!   DDE window after a click is only knowable on real hardware.
//!
//! Two more things follow from DDE painting into an *opaque* buffer. Edge and
//! shadow pixels are already blended toward the key, so the Deepin flavour does
//! not take "is it the key" per pixel at face value: `mask` fills the key-coloured
//! holes an icon encloses and peels the one-pixel dark fringe that borders the
//! key. And KWin raises DDE's window on a click and composites on its own
//! schedule, so for a frame the key-coloured window would show over the video:
//! `opacity` makes the window invisible to KWin (the offscreen copy we mirror
//! from is unaffected), with a raise of our own windows as a second line.
//!
//! # Xfce
//!
//! xfdesktop paints its backdrop and its icons into one opaque 24-bit window
//! ([`Desktop::Xfce`]) — and, since 4.19, into **one such window per monitor**,
//! so the mirror holds a *list* of sources, each with its own redirect, damage
//! object and offscreen pixmap. A source's pixels land on every wallpaper window
//! its geometry covers (`refresh` intersects each source with each icon window),
//! which for MATE's single window, and for the old screen-wide xfdesktop window,
//! is the same arithmetic as before. Windows that appear later (a monitor
//! plugged in, xfdesktop restarted) are picked up when the stack changes. Two
//! things differ from MATE:
//!
//! * there is **no stacking to guard**. Our wallpaper windows keep their
//!   `DESKTOP` + `BELOW` declaration, which xfwm4 files in a layer above the
//!   one it keeps xfdesktop's `DESKTOP` window in; layers are strict, and xfwm4
//!   ignores restack requests for a `DESKTOP` window, so a click never lifts
//!   xfdesktop over us. The icons are hidden by that layer, not by a race;
//! * the key-colour background is set over xfconf (`xfconf::apply`), on
//!   every monitor/workspace key xfdesktop reads, and put back from a saved
//!   copy. And since dialogs xfdesktop opens share its WM_CLASS, a source must
//!   also be of window type `DESKTOP`, or a dialog would be mirrored too.
//!
//! # How a refresh paints
//!
//! Each icon window has a *staging pixmap* as its background. A refresh copies
//! Caja's new pixels into the staging pixmap, then (only if the visible pixels
//! changed) sets the shape, then clears the window so the server paints it from
//! the staging pixmap. Nothing depends on an Expose arriving: a pixel that the
//! new shape reveals is painted by the server from pixels that are already
//! there. (Copying straight into the window, as this used to, drew only inside
//! the *old* shape, and relied on an Expose after the shape grew.)

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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

use super::x11win;

mod mask;
mod opacity;
mod xfconf;
#[cfg(test)]
mod xvfb_harness;

/// The background Caja paints while the mirror runs. Near-black, so the
/// anti-aliased edges it leaves on icons and labels read as a shadow.
pub const KEY_HEX: &str = "#010101";
/// [`KEY_HEX`] as bytes.
pub(super) const KEY: [u8; 3] = [1, 1, 1];
/// Per-channel slack when matching the key on Deepin, where the wallpaper goes
/// through DDE's image cache and may come back re-encoded or scaled.
const DDE_TOLERANCE: u8 = 2;

/// Whose desktop window is being mirrored. Chooses the window matcher, how the
/// key-colour background is set, and how strictly a pixel counts as the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    /// MATE: Caja's 24-bit desktop window, background via gsettings.
    Caja,
    /// Deepin: dde-shell's 32-bit ARGB desktop window, background via DBus.
    Dde,
    /// Xfce: xfdesktop's 24-bit desktop window — one per monitor — background
    /// via xfconf.
    Xfce,
}

impl Desktop {
    fn label(self) -> &'static str {
        match self {
            Desktop::Caja => "MATE",
            Desktop::Dde => "DDE",
            Desktop::Xfce => "Xfce",
        }
    }

    /// Whether `wm_class` (the raw WM_CLASS property) is this desktop's window.
    fn matches(self, wm_class: &[u8]) -> bool {
        super::dde::wm_class_is_desktop(self, wm_class)
    }
}

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

/// How long [`Mirror::stop`] waits for the mirror thread before detaching
/// and letting shutdown proceed (see `stop`).
const STOP_JOIN_TIMEOUT: Duration = Duration::from_secs(3);

/// Handle to the mirror thread. Dropping it without [`Mirror::stop`] leaves the
/// thread running until the process exits, which is harmless.
pub struct Mirror {
    tx: Sender<Cmd>,
    /// An input-only window owned by the thread's connection; a client message
    /// sent to it wakes the thread out of its blocking wait.
    wake: Window,
    failed: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    desktop: Desktop,
}

impl Mirror {
    /// Start the thread. Fails when it cannot connect or when the server lacks
    /// Composite, Damage or XFixes — the caller then keeps the restack mode.
    pub fn start(desktop: Desktop) -> Result<Mirror> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<Window, String>>();
        let failed = Arc::new(AtomicBool::new(false));
        let thread_failed = failed.clone();
        let thread = std::thread::Builder::new()
            .name("caja-mirror".into())
            .spawn(move || {
                if let Err(e) = run(desktop, rx, &ready_tx) {
                    log::warn!("{}: icon mirror stopped: {e:#}", desktop.label());
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
            desktop,
        })
    }

    /// Which desktop this mirror was started for.
    pub fn desktop(&self) -> Desktop {
        self.desktop
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

    /// Stop the thread and wait for it — bounded. Closing its connection
    /// undoes the redirect and destroys the icon windows.
    ///
    /// The thread may be parked in `wait_for_event()` or mid-`refresh()`'s
    /// banded `GetImage` loop when Stop lands; an unbounded `join` there
    /// would hang `shutdown()` forever while the GUI already got its `Ok`
    /// (issue #33). So the join runs on a throwaway waiter and `stop`
    /// gives it [`STOP_JOIN_TIMEOUT`]: on timeout the waiter keeps the
    /// `JoinHandle` and finishes the thread's own cleanup in the
    /// background (its `State::drop` plus connection close handle the
    /// window side), while shutdown proceeds to the restores and socket
    /// removal. Harmless even when the daemon keeps running (the mid-run
    /// stops in `sync_caja_mirror` and `fall_back_to_restack`): the detached
    /// thread exits on its own once `tx` is dropped.
    pub fn stop<C: Connection>(mut self, conn: &C) {
        let _ = self.tx.send(Cmd::Stop);
        self.wake(conn);
        if let Some(t) = self.thread.take() {
            let (done_tx, done_rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("caja-mirror-join".into())
                .spawn(move || {
                    let _ = t.join();
                    let _ = done_tx.send(());
                })
                .ok();
            if done_rx.recv_timeout(STOP_JOIN_TIMEOUT).is_err() {
                // One last nudge in case the wake was lost, then detach:
                // the waiter still owns the handle and reaps the thread.
                self.wake(conn);
                log::warn!(
                    "{}: icon mirror thread did not stop in {:?}; continuing shutdown without it",
                    self.desktop.label(),
                    STOP_JOIN_TIMEOUT
                );
            }
        }
    }

    fn wake<C: Connection>(&self, conn: &C) {
        let ev = ClientMessageEvent::new(32, self.wake, AtomEnum::NOTICE, [0u32; 5]);
        let _ = conn.send_event(false, self.wake, EventMask::NO_EVENT, ev);
        let _ = conn.flush();
    }
}

/// A desktop window being mirrored (Caja's, dde-shell's, or one of xfdesktop's
/// per-monitor windows), redirected offscreen.
struct Caja {
    window: Window,
    pixmap: Pixmap,
    damage: damage::Damage,
    /// Where the window's origin is on the root, and its size.
    rect: Rectangle,
    /// The window manager's frame around it, when it has reparented the
    /// window (KWin does). Restacking shows up as a configure of the frame.
    frame: Option<Window>,
    /// Diagnostics: when the second, later pixel sample is due (Deepin only —
    /// the DBus wallpaper change lands a moment after we attach).
    late_sample: Option<Instant>,
}

/// How icon windows are created when the mirrored window is not the screen's
/// depth: same depth and visual as the source, since `CopyArea` needs both
/// drawables to match. `None` in [`State`] means "copy from the parent".
#[derive(Clone, Copy, PartialEq, Eq)]
struct ChildFmt {
    depth: u8,
    visual: Visualid,
    colormap: Colormap,
}

/// One icon window, a child of one wallpaper window.
struct Child {
    parent: Parent,
    window: Window,
    gc: Gcontext,
    /// The window's background: the latest copy of Caja's pixels under this
    /// parent. The server paints the window from it, so a pixel the shape newly
    /// reveals is never blank or stale.
    staging: Pixmap,
    /// Per pixel of the parent: true where Caja drew something other than
    /// the key colour — i.e. where the icon window is visible.
    mask: Vec<bool>,
    /// Deepin only: per pixel, [`mask::CLASS_KEY`] / `CLASS_DARK` / `CLASS_OTHER`.
    /// The mask is derived from these, and they are what lets a small damage
    /// rectangle be refined with the icon around it. Empty for Caja.
    classes: Vec<u8>,
}

impl Child {
    /// Destroy the window and free what was made for it. A BadWindow is
    /// expected (and ignored) when the parent went first and took it along.
    fn destroy(self, conn: &RustConnection) {
        let _ = conn.destroy_window(self.window);
        self.release(conn);
    }

    /// Free what outlives the window: its GC and staging pixmap.
    fn release(self, conn: &RustConnection) {
        let _ = conn.free_gc(self.gc);
        let _ = conn.free_pixmap(self.staging);
    }
}

/// A desktop window that came up over the wallpaper, and what has been done
/// about it so far. One per click.
struct Episode {
    started: Instant,
    acted: u8,
}

/// Re-checks after the first reaction to a raised desktop window, as offsets
/// from it: KWin may take a moment to honour a restack.
const RECHECK_AFTER: [Duration; 2] = [Duration::from_millis(30), Duration::from_millis(100)];
/// An episode older than this is a new click, not a stubborn window manager.
const EPISODE_LIFETIME: Duration = Duration::from_secs(2);
/// Reactions per episode before the mirror stops fighting until it ends.
const EPISODE_MAX_ACTIONS: u8 = 6;

struct State {
    desktop: Desktop,
    conn: RustConnection,
    root: Window,
    root_depth: u8,
    stacking: Atom,
    wake: Window,
    msb_first: bool,
    /// `_NET_WM_WINDOW_TYPE` and its `_DESKTOP` value: Xfce sources must carry
    /// it (see `find_sources`).
    wm_type: Atom,
    type_desktop: Atom,
    /// The desktop windows being mirrored: one on MATE and Deepin, one per
    /// monitor on Xfce. Empty until the first is found.
    sources: Vec<Caja>,
    children: Vec<Child>,
    /// The wallpaper windows the daemon last asked us to cover.
    parents: Vec<Parent>,
    child_fmt: Option<ChildFmt>,
    /// Deepin: `_NET_WM_WINDOW_OPACITY`, and the desktop window while it is
    /// held at 0 (see `opacity`).
    opacity_atom: Atom,
    hider: Option<opacity::Hider>,
    /// A stacking-relevant event arrived; `guard_stacking` must look.
    stack_dirty: bool,
    episode: Option<Episode>,
    /// Instants at which the loop must wake to look at the stack again.
    rechecks: Vec<Instant>,
}

fn run(desktop: Desktop, rx: Receiver<Cmd>, ready: &Sender<Result<Window, String>>) -> Result<()> {
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
    let opacity_atom = conn.intern_atom(false, opacity::ATOM_NAME)?.reply()?.atom;
    let wm_type = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
        .reply()?
        .atom;
    let type_desktop = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE_DESKTOP")?
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
    // Stacking changes arrive as property changes on the root. On Deepin the
    // window manager's frames are watched too: KWin restacks them (which we
    // hear as a configure) before it updates the property.
    let mut root_events = EventMask::PROPERTY_CHANGE;
    if desktop == Desktop::Dde {
        root_events |= EventMask::SUBSTRUCTURE_NOTIFY;
    }
    conn.change_window_attributes(
        screen.root,
        &ChangeWindowAttributesAux::new().event_mask(root_events),
    )?;
    conn.flush()?;
    let _ = ready.send(Ok(wake));

    let mut st = State {
        desktop,
        conn,
        root: screen.root,
        root_depth: screen.root_depth,
        stacking,
        wake,
        msb_first,
        wm_type,
        type_desktop,
        sources: Vec::new(),
        children: Vec::new(),
        parents: Vec::new(),
        child_fmt: None,
        opacity_atom,
        hider: None,
        stack_dirty: false,
        episode: None,
        rechecks: Vec::new(),
    };

    // Look at the stack once whatever the events say: on start, and whenever
    // the set of windows we cover changes.
    let mut check_stack = true;
    loop {
        loop {
            match rx.try_recv() {
                Ok(Cmd::Parents(p)) => {
                    st.set_parents(p)?;
                    check_stack = true;
                }
                Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => return Ok(()),
                Err(TryRecvError::Empty) => break,
            }
        }
        if st.sources.is_empty() {
            st.attach()?;
            check_stack = true;
        }
        if check_stack {
            check_stack = false;
            st.guard_stacking(Instant::now())?;
        }
        st.conn.flush()?;

        let mut dirty: Option<Rectangle> = None;
        let mut restack = false;
        // While a deadline is pending (the late diagnostic sample, a re-check
        // of the stack) poll instead of blocking, so it fires on time even if
        // Caja/DDE stays quiet.
        let first = match st.next_wake() {
            Some((deadline, step)) => loop {
                if let Some(ev) = st.conn.poll_for_event()? {
                    break Some(ev);
                }
                let now = Instant::now();
                if now >= deadline {
                    break None;
                }
                std::thread::sleep((deadline - now).min(step));
            },
            None => Some(st.conn.wait_for_event()?),
        };
        let woke = Instant::now();
        if let Some(ev) = first {
            st.handle(ev, &mut dirty, &mut restack)?;
        }
        while let Some(ev) = st.conn.poll_for_event()? {
            st.handle(ev, &mut dirty, &mut restack)?;
        }
        let before = st.rechecks.len();
        st.rechecks.retain(|&t| t > woke);
        st.stack_dirty |= st.rechecks.len() != before;
        // Xfce has a desktop window per monitor, and they come and go with the
        // monitors (and with xfdesktop): look for new ones when the stack moved.
        if st.desktop == Desktop::Xfce && st.stack_dirty {
            st.attach()?;
        }
        // Before the slow part: a desktop window that has come up over the
        // wallpaper is on screen until this runs. Caja is checked on every
        // wake, as it always was; Deepin when an event said the stack moved.
        if st.stack_dirty || st.desktop == Desktop::Caja {
            st.stack_dirty = false;
            st.guard_stacking(woke)?;
            st.conn.flush()?;
        }
        if let Some(r) = dirty {
            st.refresh(r)?;
        }
        if restack {
            st.raise_children()?;
        }
        st.late_diagnostics();
    }
}

impl Drop for State {
    /// Every way out of the thread — a stop, an error, a lost display — leaves
    /// the desktop window as it was found. (The X server frees the rest when
    /// the connection closes; the opacity property is the one thing it would
    /// not undo.)
    fn drop(&mut self) {
        if let Some(mut h) = self.hider.take() {
            h.restore(&self.conn);
        }
        self.drop_children();
        if let Some(f) = self.child_fmt.take() {
            let _ = self.conn.free_colormap(f.colormap);
        }
        let _ = self.conn.flush();
        // Round trip: once the thread has returned, the restore has landed.
        let _ = self.conn.get_input_focus().map(|c| c.reply());
    }
}

impl State {
    /// Find the desktop window(s) and start mirroring those not mirrored yet.
    /// A no-op while the desktop is not up yet — a stacking change wakes us
    /// when it arrives.
    fn attach(&mut self) -> Result<()> {
        for window in self.find_sources() {
            if !self.sources.iter().any(|s| s.window == window) {
                self.attach_one(window)?;
            }
        }
        Ok(())
    }

    /// Start mirroring one desktop window.
    fn attach_one(&mut self, window: Window) -> Result<()> {
        let conn = &self.conn;
        let Some(geom) = conn.get_geometry(window)?.reply().ok() else {
            return Ok(());
        };
        if self.desktop != Desktop::Dde && geom.depth != self.root_depth {
            bail!(
                "the {} desktop window is {}-bit on a {}-bit screen; it cannot be copied \
                 onto the wallpaper",
                self.desktop.label(),
                geom.depth,
                self.root_depth
            );
        }
        let visual = conn.get_window_attributes(window)?.reply()?.visual;
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
        // Deepin's 32-bit window: the icon windows must share its depth and
        // visual. A 24-bit one (older DDE) copies from the parent as usual.
        // Attaching again to a window of the same format (DDE restarted) keeps
        // the colormap the existing icon windows already use.
        let fmt = if geom.depth == self.root_depth {
            None
        } else {
            match self.child_fmt {
                Some(old) if old.depth == geom.depth && old.visual == visual => Some(old),
                _ => {
                    let colormap = conn.generate_id()?;
                    conn.create_colormap(ColormapAlloc::NONE, colormap, self.root, visual)?;
                    Some(ChildFmt {
                        depth: geom.depth,
                        visual,
                        colormap,
                    })
                }
            }
        };
        if fmt.map(|f| (f.depth, f.visual)) != self.child_fmt.map(|f| (f.depth, f.visual)) {
            self.drop_children();
        }
        // The replaced colormap is no window's any more: free it rather than
        // leaving one behind per attach.
        if let Some(old) = self.child_fmt {
            if fmt.map(|f| f.colormap) != Some(old.colormap) {
                let _ = self.conn.free_colormap(old.colormap);
            }
        }
        self.child_fmt = fmt;
        let conn = &self.conn;
        let mut events = EventMask::STRUCTURE_NOTIFY;
        if self.desktop == Desktop::Dde {
            // Opacity changes (DDE resets it) arrive as property changes.
            events |= EventMask::PROPERTY_CHANGE;
        }
        conn.change_window_attributes(
            window,
            &ChangeWindowAttributesAux::new().event_mask(events),
        )?;
        let frame = conn
            .query_tree(window)?
            .reply()
            .ok()
            .map(|t| t.parent)
            .filter(|&p| p != self.root);
        if self.desktop == Desktop::Dde && opacity::enabled() {
            if let Some(old) = self.hider.take() {
                old.forget();
            }
            match opacity::Hider::engage(&self.conn, self.opacity_atom, window) {
                Ok(h) => {
                    self.hider = Some(h);
                    log::info!(
                        "DDE: the desktop window is hidden from the compositor (opacity 0); \
                         its icons are still mirrored"
                    );
                }
                Err(e) => log::warn!(
                    "DDE: could not hide the desktop window from the compositor ({e:#}); a \
                     click on the desktop may flash it over the wallpaper"
                ),
            }
        }
        let conn = &self.conn;
        conn.composite_redirect_window(window, composite::Redirect::AUTOMATIC)?;
        let pixmap = conn.generate_id()?;
        conn.composite_name_window_pixmap(window, pixmap)?;
        let damage = conn.generate_id()?;
        conn.damage_create(damage, window, damage::ReportLevel::BOUNDING_BOX)?;
        // The fresh offscreen copy holds whatever was on screen; make Caja
        // paint itself into it. The damage that follows drives the first copy.
        conn.clear_area(true, window, 0, 0, 0, 0)?;
        conn.flush()?;
        self.sources.push(Caja {
            window,
            pixmap,
            damage,
            rect: Rectangle {
                x: origin.dst_x,
                y: origin.dst_y,
                width: geom.width,
                height: geom.height,
            },
            frame,
            late_sample: (self.desktop == Desktop::Dde).then(|| Instant::now() + LATE_SAMPLE),
        });
        log::info!(
            "{}: mirroring the desktop icons of window {window:#x} ({}x{}{:+}{:+}, {}-bit) \
             over the wallpaper",
            self.desktop.label(),
            geom.width,
            geom.height,
            origin.dst_x,
            origin.dst_y,
            geom.depth
        );
        if self.desktop == Desktop::Dde {
            // Children exist only once the source's depth is known.
            self.sync_children()?;
            self.log_diagnostics("attach", window, &geom, visual);
        }
        Ok(())
    }

    /// One info line per sample, so a remote tester's frescod.log says what
    /// DDE's window really looks like: id, depth, visual class, geometry, the
    /// alpha range and how much of it already reads as the key colour.
    fn log_diagnostics(
        &self,
        when: &str,
        window: Window,
        geom: &GetGeometryReply,
        visual: Visualid,
    ) {
        let class = self
            .conn
            .setup()
            .roots
            .iter()
            .flat_map(|r| r.allowed_depths.iter())
            .flat_map(|d| d.visuals.iter())
            .find(|v| v.visual_id == visual)
            .map(|v| format!("{:?}", v.class))
            .unwrap_or_else(|| "unknown".into());
        let sample = self.sample_pixmap();
        // The probe sets a transparent wallpaper instead of the key colour, to
        // see whether DDE's buffer then carries real alpha.
        let probe = if super::dde::probe_alpha() {
            " [probe=alpha: transparent wallpaper]"
        } else {
            ""
        };
        match sample {
            Some(s) => log::info!(
                "DDE mirror diagnostics ({when}){probe}: window {window:#x} depth {} visual \
                 {visual:#x} class {class} geometry {}x{}{:+}{:+}; sampled {} px: alpha min {} \
                 max {} (partial {:.3}), key-colour fraction {:.3}",
                geom.depth,
                geom.width,
                geom.height,
                geom.x,
                geom.y,
                s.count,
                s.alpha_min,
                s.alpha_max,
                s.alpha_partial,
                s.key_fraction
            ),
            None => log::info!(
                "DDE mirror diagnostics ({when}){probe}: window {window:#x} depth {} visual \
                 {visual:#x} class {class} geometry {}x{}{:+}{:+}; the window's pixels could not \
                 be read",
                geom.depth,
                geom.width,
                geom.height,
                geom.x,
                geom.y
            ),
        }
    }

    /// The second diagnostic line, a few seconds after attach: by then DDE has
    /// repainted with the key-colour wallpaper, so the key fraction says
    /// whether the background change reached the window (and survived its
    /// image cache).
    fn late_diagnostics(&mut self) {
        let now = Instant::now();
        let Some(caja) = self
            .sources
            .iter_mut()
            .find(|c| c.late_sample.is_some_and(|t| now >= t))
        else {
            return;
        };
        caja.late_sample = None;
        let window = caja.window;
        let Some(geom) = self
            .conn
            .get_geometry(window)
            .ok()
            .and_then(|c| c.reply().ok())
        else {
            return;
        };
        let visual = self
            .conn
            .get_window_attributes(window)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|a| a.visual)
            .unwrap_or(0);
        self.log_diagnostics("after repaint", window, &geom, visual);
    }

    /// Sample the redirected window: 16 evenly spaced rows, every 4th pixel.
    fn sample_pixmap(&self) -> Option<Sample> {
        let caja = self.sources.first()?;
        let (w, h) = (caja.rect.width, caja.rect.height);
        if w == 0 || h == 0 {
            return None;
        }
        let mut s = Sample {
            count: 0,
            alpha_min: 255,
            alpha_max: 0,
            alpha_partial: 0.0,
            key_fraction: 0.0,
        };
        let mut keys = 0u32;
        let mut partial = 0u32;
        for i in 0..16u32 {
            let y = (u32::from(h) * i / 16).min(u32::from(h) - 1) as i16;
            let img = self
                .conn
                .get_image(ImageFormat::Z_PIXMAP, caja.pixmap, 0, y, w, 1, !0)
                .ok()?
                .reply()
                .ok()?;
            for px in img.data.chunks_exact(4).step_by(4) {
                let a = if self.msb_first { px[0] } else { px[3] };
                s.alpha_min = s.alpha_min.min(a);
                s.alpha_max = s.alpha_max.max(a);
                if a != 0 && a != 255 {
                    partial += 1;
                }
                s.count += 1;
                if is_key(px, self.msb_first, self.desktop) {
                    keys += 1;
                }
            }
        }
        if s.count > 0 {
            s.key_fraction = keys as f32 / s.count as f32;
            s.alpha_partial = partial as f32 / s.count as f32;
        }
        Some(s)
    }

    /// The desktop windows to mirror, lowest in the stack first: the first
    /// match for Caja and dde-shell (one window each), every match for
    /// xfdesktop (one per monitor). An xfdesktop window must also be of type
    /// `DESKTOP`: its dialogs share the WM_CLASS and must not be mirrored.
    fn find_sources(&self) -> Vec<Window> {
        let many = self.desktop == Desktop::Xfce;
        let mut found = Vec::new();
        for w in self.stack() {
            let class_ok = self
                .conn
                .get_property(false, w, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some_and(|p| self.desktop.matches(&p.value));
            if class_ok && (!many || self.is_desktop_type(w)) {
                found.push(w);
                if !many {
                    break;
                }
            }
        }
        found
    }

    /// Whether `window`'s `_NET_WM_WINDOW_TYPE` lists `_NET_WM_WINDOW_TYPE_DESKTOP`.
    fn is_desktop_type(&self, window: Window) -> bool {
        self.conn
            .get_property(false, window, self.wm_type, AtomEnum::ATOM, 0, 32)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|mut v| v.any(|a| a == self.type_desktop)))
            .unwrap_or(false)
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
    ///
    /// `woke` is when the event that made us look arrived, for the timing in
    /// the log. On Deepin lowering the desktop window is only a request KWin
    /// may ignore (it lowers a window only within its own application), so our
    /// own windows are raised as well — the move verified on Deepin 25 — and
    /// the stack is looked at again shortly after, in case KWin was slow.
    fn guard_stacking(&mut self, woke: Instant) -> Result<()> {
        // Xfce: nothing to guard. xfwm4 keeps xfdesktop in a layer below ours.
        if self.desktop == Desktop::Xfce {
            return Ok(());
        }
        let Some(caja) = self.sources.first() else {
            return Ok(());
        };
        let window = caja.window;
        let ours: Vec<Window> = self.children.iter().map(|c| c.parent.window).collect();
        let label = self.desktop.label();
        if !caja_above_ours(&self.stack(), window, &ours) {
            if let Some(ep) = self.episode.take() {
                log::debug!(
                    "{label}: the desktop is below the wallpaper again, {:.1} ms after it came up",
                    ep.started.elapsed().as_secs_f64() * 1000.0
                );
            }
            return Ok(());
        }
        if self
            .episode
            .as_ref()
            .is_none_or(|e| e.started.elapsed() > EPISODE_LIFETIME)
        {
            self.episode = Some(Episode {
                started: woke,
                acted: 0,
            });
        }
        let Some(ep) = &mut self.episode else {
            return Ok(());
        };
        ep.acted += 1;
        let acted = ep.acted;
        if acted > EPISODE_MAX_ACTIONS {
            // The window manager is not having it; stop shouting until the
            // next click starts a new episode.
            return Ok(());
        }
        self.conn.configure_window(
            window,
            &ConfigureWindowAux::new().stack_mode(StackMode::BELOW),
        )?;
        if self.desktop == Desktop::Dde {
            for &w in &ours {
                x11win::raise(&self.conn, w)?;
            }
        }
        self.conn.flush()?;
        if acted == 1 {
            log::debug!(
                "{label}: the desktop came up over the wallpaper; lowered it ({:.1} ms after the \
                 event)",
                woke.elapsed().as_secs_f64() * 1000.0
            );
            if self.desktop == Desktop::Dde {
                let now = Instant::now();
                self.rechecks.extend(RECHECK_AFTER.iter().map(|d| now + *d));
            }
        } else {
            log::debug!(
                "{label}: the desktop is still over the wallpaper (attempt {acted}); lowered it \
                 again"
            );
        }
        Ok(())
    }

    /// When the loop next has to wake without an event, and how often to look
    /// for one meanwhile. `None` means block until something arrives.
    fn next_wake(&self) -> Option<(Instant, Duration)> {
        let late = self.sources.iter().filter_map(|c| c.late_sample).min();
        let recheck = self.rechecks.iter().min().copied();
        let deadline = [late, recheck].into_iter().flatten().min()?;
        // A re-check is worth a finer poll than the diagnostic sample.
        let step = if recheck.is_some() {
            Duration::from_millis(5)
        } else {
            Duration::from_millis(50)
        };
        Some((deadline, step))
    }

    fn drop_children(&mut self) {
        for c in self.children.drain(..) {
            c.destroy(&self.conn);
        }
    }

    fn set_parents(&mut self, parents: Vec<Parent>) -> Result<()> {
        self.parents = parents;
        self.sync_children()
    }

    /// Bring the icon windows in line with `self.parents`. On Deepin nothing is
    /// created before the source window is attached, because the children need
    /// its depth and visual.
    fn sync_children(&mut self) -> Result<()> {
        if self.desktop == Desktop::Dde && self.sources.is_empty() {
            return Ok(());
        }
        let parents = self.parents.clone();
        // Drop children of windows that are gone. Destroying a parent already
        // took its child with it, so a BadWindow here is expected and ignored.
        let mut kept = Vec::new();
        for c in self.children.drain(..) {
            if parents.contains(&c.parent) {
                kept.push(c);
            } else {
                c.destroy(&self.conn);
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
                    "{}: could not add icons over window {:#x}: {e:#}",
                    self.desktop.label(),
                    p.window
                ),
            }
        }
        // Everything the sources cover, in one go; `refresh` cuts it back to
        // each source.
        let all = self
            .sources
            .iter()
            .fold(None, |acc, s| Some(union(acc, s.rect)));
        if let Some(r) = all {
            self.refresh(r)?;
        }
        self.conn.flush()?;
        Ok(())
    }

    fn create_child(&self, p: Parent) -> Result<Child> {
        let conn = &self.conn;
        let window = conn.generate_id()?;
        let staging = conn.generate_id()?;
        // The staging pixmap is the window's background, so the two must agree
        // on depth: the source's on Deepin, the parent's (which the window
        // inherits) otherwise.
        let pixmap_depth = match self.child_fmt {
            Some(f) => f.depth,
            None => conn.get_geometry(p.window)?.reply()?.depth,
        };
        conn.create_pixmap(pixmap_depth, staging, p.window, p.width, p.height)?;
        // The server paints the window from the staging pixmap whenever any of
        // it becomes visible, so no Expose is needed (or selected).
        let mut aux = CreateWindowAux::new()
            .background_pixmap(staging)
            .event_mask(EventMask::STRUCTURE_NOTIFY);
        let (depth, visual) = match self.child_fmt {
            Some(f) => {
                // A window whose depth differs from its parent's must name its
                // own colormap and border pixel, or CreateWindow is BadMatch.
                aux = aux.colormap(f.colormap).border_pixel(0);
                (f.depth, f.visual)
            }
            None => (COPY_DEPTH_FROM_PARENT, 0),
        };
        conn.create_window(
            depth,
            window,
            p.window,
            0,
            0,
            p.width,
            p.height,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &aux,
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
        let pixels = usize::from(p.width) * usize::from(p.height);
        Ok(Child {
            parent: p,
            window,
            gc,
            staging,
            mask: vec![false; pixels],
            classes: if self.desktop == Desktop::Dde {
                vec![mask::CLASS_KEY; pixels]
            } else {
                Vec::new()
            },
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
                if let Some(caja) = self.sources.iter().find(|c| c.damage == e.damage) {
                    self.conn.damage_subtract(caja.damage, NONE, NONE)?;
                    let area = Rectangle {
                        x: caja.rect.x.saturating_add(e.area.x),
                        y: caja.rect.y.saturating_add(e.area.y),
                        ..e.area
                    };
                    *dirty = Some(union(*dirty, area));
                }
            }
            Event::PropertyNotify(e) if e.window == self.root && e.atom == self.stacking => {
                // The stack moved; `guard_stacking` looks (and a Caja that has
                // just appeared is attached in the main loop).
                self.stack_dirty = true;
            }
            Event::PropertyNotify(e) if e.atom == self.opacity_atom => {
                if let Some(h) = &mut self.hider {
                    if h.window() == e.window {
                        h.on_change(&self.conn, Instant::now());
                    }
                }
            }
            Event::MapNotify(e) => {
                let is_child = self.children.iter().any(|c| c.window == e.window);
                if !is_child && self.children.iter().any(|c| c.parent.window == e.event) {
                    *restack = true;
                }
            }
            Event::ConfigureNotify(e) => {
                let Some(i) = self
                    .sources
                    .iter()
                    .position(|c| c.window == e.window || c.frame == Some(e.window))
                else {
                    return Ok(());
                };
                // Restacked (or moved): look at the stack without waiting
                // for the window manager to update the property.
                self.stack_dirty = true;
                if self.sources[i].window == e.window {
                    self.desktop_reconfigured(i, e.width, e.height, dirty)?;
                }
            }
            Event::DestroyNotify(e) => {
                if let Some(i) = self.sources.iter().position(|c| c.window == e.window) {
                    log::info!(
                        "{}: the desktop window {:#x} went away; waiting for it to return",
                        self.desktop.label(),
                        e.window
                    );
                    let gone = self.sources.remove(i);
                    let _ = self.conn.free_pixmap(gone.pixmap);
                    // Xfce: the other monitors' windows are still there. Their
                    // masks are cleared below with the rest, so have them read
                    // again what they cover.
                    for s in &self.sources {
                        *dirty = Some(union(*dirty, s.rect));
                    }
                    // The window took its opacity with it; only the file is left.
                    if let Some(h) = self.hider.take() {
                        h.forget();
                    }
                    self.episode = None;
                    for c in &mut self.children {
                        c.mask.iter_mut().for_each(|m| *m = false);
                        c.classes.iter_mut().for_each(|k| *k = mask::CLASS_KEY);
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
                if let Some(i) = self.children.iter().position(|c| c.window == e.window) {
                    // The window is gone already; its GC and pixmap are not.
                    self.children.remove(i).release(&self.conn);
                }
            }
            Event::ClientMessage(e) if e.window == self.wake => {}
            Event::Error(e) => log::debug!("{}: X error {e:?}", self.desktop.label()),
            _ => {}
        }
        Ok(())
    }

    /// The desktop window was configured: it may have been resized, or moved.
    ///
    /// A move matters as much as a resize — the pixels are copied from the
    /// window's offscreen pixmap at offsets worked out from where the window
    /// sits on the root. The event's own x/y are relative to the window
    /// manager's frame when it has reparented the window (KWin does), so the
    /// origin is asked for rather than read from the event.
    fn desktop_reconfigured(
        &mut self,
        source: usize,
        width: u16,
        height: u16,
        dirty: &mut Option<Rectangle>,
    ) -> Result<()> {
        let Some(caja) = self.sources.get_mut(source) else {
            return Ok(());
        };
        let origin = self
            .conn
            .translate_coordinates(caja.window, self.root, 0, 0)?
            .reply()
            .ok();
        let (x, y) = origin.map_or((caja.rect.x, caja.rect.y), |o| (o.dst_x, o.dst_y));
        let resized = width != caja.rect.width || height != caja.rect.height;
        if !resized && (x, y) == (caja.rect.x, caja.rect.y) {
            return Ok(());
        }
        if resized {
            // The offscreen copy is per size; name the new one.
            let _ = self.conn.free_pixmap(caja.pixmap);
            let pixmap = self.conn.generate_id()?;
            self.conn
                .composite_name_window_pixmap(caja.window, pixmap)?;
            caja.pixmap = pixmap;
        }
        caja.rect = Rectangle {
            x,
            y,
            width,
            height,
        };
        *dirty = Some(union(*dirty, caja.rect));
        Ok(())
    }

    /// Re-read `area` (root coordinates) of the sources' offscreen copies and
    /// bring every icon window it touches up to date.
    ///
    /// Per source and icon window: read the new pixels and work out which of them are
    /// visible; copy them into the staging pixmap that is the window's
    /// background; change the shape only if the visible pixels differ from
    /// what the shape already has; then clear the window, which has the server
    /// paint it from the staging pixmap. Whatever the shape reveals is painted
    /// from pixels already in place, so the result does not hang on any later
    /// Expose.
    fn refresh(&mut self, area: Rectangle) -> Result<()> {
        let conn = &self.conn;
        let (desktop, msb_first) = (self.desktop, self.msb_first);
        for caja in &self.sources {
            let Some(area) = intersect(area, caja.rect) else {
                continue;
            };
            for c in &mut self.children {
                let Some(part) = intersect(area, c.parent.rect()) else {
                    continue;
                };
                let Some(changed) = read_part(conn, caja, c, part, desktop, msb_first)? else {
                    // One unreadable child must not poison the rest: skip it only
                    // (its cache is untouched, so it catches up on the next
                    // damage) instead of abandoning this and every later parent —
                    // that skew read as "only some icons repaint" on multi-monitor
                    // desktops (issue #33).
                    continue;
                };
                let (dst_x, dst_y) = (part.x - c.parent.x, part.y - c.parent.y);
                conn.copy_area(
                    caja.pixmap,
                    c.staging,
                    c.gc,
                    part.x - caja.rect.x,
                    part.y - caja.rect.y,
                    dst_x,
                    dst_y,
                    part.width,
                    part.height,
                )?;
                if changed {
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
                conn.clear_area(false, c.window, dst_x, dst_y, part.width, part.height)?;
            }
        }
        conn.flush()?;
        Ok(())
    }
}

/// Read `part` (root coordinates) of Caja's offscreen copy into `c`'s mask.
///
/// Returns whether the visible pixels changed (so the shape must be re-sent),
/// or `None` when not even the first band could be read (the cache is then
/// untouched). A band that fails after earlier ones were read stops the read
/// there and reports what those bands changed: they are already in the cache,
/// so dropping that would hide the change from every later read. Caja: a pixel
/// is visible when it is not exactly the key. Deepin: the pixels are classified
/// and the mask is refined from the cached classes of the whole icon — see
/// [`mask`].
fn read_part(
    conn: &RustConnection,
    caja: &Caja,
    c: &mut Child,
    part: Rectangle,
    desktop: Desktop,
    msb_first: bool,
) -> Result<Option<bool>> {
    let pw = usize::from(c.parent.width);
    let ph = usize::from(c.parent.height);
    let px0 = usize::try_from(part.x - c.parent.x).unwrap_or(0);
    let py0 = usize::try_from(part.y - c.parent.y).unwrap_or(0);
    let mut changed = false;
    // Deepin: the box of pixels whose class differs from what was cached.
    let mut classes_changed: Option<(usize, usize, usize, usize)> = None;
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
            if row == 0 {
                return Ok(None);
            }
            break; // keep the earlier bands' changes; see the doc comment
        };
        let stride = usize::from(part.width) * 4;
        for dy in 0..usize::from(h) {
            let py = py0 + usize::from(row) + dy;
            let line = img.data.get(dy * stride..(dy + 1) * stride).unwrap_or(&[]);
            for (dx, px) in line.chunks_exact(4).enumerate() {
                let i = py * pw + px0 + dx;
                match desktop {
                    Desktop::Caja | Desktop::Xfce => {
                        if let Some(m) = c.mask.get_mut(i) {
                            let visible = !is_key(px, msb_first, desktop);
                            changed |= *m != visible;
                            *m = visible;
                        }
                    }
                    Desktop::Dde => {
                        if let Some(k) = c.classes.get_mut(i) {
                            let class =
                                mask::classify(pixel_rgb(px, msb_first), KEY, DDE_TOLERANCE);
                            if *k != class {
                                *k = class;
                                let (x, y) = (px0 + dx, py);
                                classes_changed = Some(match classes_changed {
                                    None => (x, y, x, y),
                                    Some((x0, y0, x1, y1)) => {
                                        (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
        row += h;
    }
    // A repaint that left every pixel in the same class (most of them) changes
    // nothing about what is visible: no refine, no shape.
    if let (Desktop::Dde, Some((x0, y0, x1, y1))) = (desktop, classes_changed) {
        let dirty = mask::Area {
            x: x0,
            y: y0,
            w: x1 - x0 + 1,
            h: y1 - y0 + 1,
        };
        let (area, refined) = mask::refine_area(&c.classes, pw, ph, dirty);
        for (r, new) in refined.chunks_exact(area.w).enumerate() {
            let at = (area.y + r) * pw + area.x;
            if let Some(old) = c.mask.get_mut(at..at + area.w) {
                if old != new {
                    changed = true;
                    old.copy_from_slice(new);
                }
            }
        }
    }
    Ok(Some(changed))
}

/// Diagnostic summary of a pixel sample.
struct Sample {
    count: u32,
    alpha_min: u8,
    alpha_max: u8,
    /// Fraction of pixels whose alpha is neither 0 nor 255.
    alpha_partial: f32,
    key_fraction: f32,
}

/// How long after attaching the second diagnostic sample is taken.
const LATE_SAMPLE: Duration = Duration::from_secs(3);

/// The R, G, B of a 32-bit ZPixmap pixel, for either byte order. LSB-first is
/// B, G, R, pad/alpha; MSB-first is pad/alpha, R, G, B.
fn pixel_rgb(px: &[u8], msb_first: bool) -> [u8; 3] {
    if msb_first {
        [px[1], px[2], px[3]]
    } else {
        [px[2], px[1], px[0]]
    }
}

/// Whether a 32-bit ZPixmap pixel is the key colour, for either byte order.
///
/// Caja and xfdesktop: the three colour bytes must equal the key exactly (the
/// pad byte is ignored). Deepin: alpha is ignored and each channel may be off by
/// [`DDE_TOLERANCE`] — which also means near-black (0..=3) counts as key there,
/// so a pure-black label shadow on Deepin is dropped; the icons themselves and
/// their white labels are unaffected.
fn is_key(px: &[u8], msb_first: bool, desktop: Desktop) -> bool {
    let rgb = pixel_rgb(px, msb_first);
    match desktop {
        Desktop::Caja | Desktop::Xfce => rgb == KEY,
        Desktop::Dde => rgb
            .iter()
            .zip(KEY)
            .all(|(&c, k)| c.abs_diff(k) <= DDE_TOLERANCE),
    }
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

/// Give DDE's desktop window its original opacity back if a mirror run that
/// hid it did not get to (a crash, a kill). Idempotent; a no-op without the
/// state file `opacity` writes before it touches the window.
pub(super) fn restore_desktop_opacity() {
    opacity::restore_saved();
}

/// True while Caja is painting the key colour on our behalf. The still-frame
/// writer (`overview`) checks it: its picture would replace the key and every
/// icon would vanish into a photograph.
pub fn key_active() -> bool {
    KEY_ACTIVE.load(Ordering::SeqCst)
}

/// Switch `desktop`'s background to the key colour, saving the user's own
/// first. False when the background cannot be driven (or restored) at all.
/// `monitors` are the connector names, used by Deepin's per-monitor service.
pub fn apply_key(desktop: Desktop, monitors: &[String]) -> bool {
    match desktop {
        Desktop::Caja => apply_key_mate(),
        Desktop::Dde => {
            if super::dde::apply_key_background(monitors).is_none() {
                return false;
            }
            KEY_ACTIVE.store(true, Ordering::SeqCst);
            true
        }
        Desktop::Xfce => {
            if !xfconf::apply(monitors) {
                return false;
            }
            KEY_ACTIVE.store(true, Ordering::SeqCst);
            true
        }
    }
}

/// Undo [`apply_key`] for `desktop`. Idempotent; a no-op when nothing was saved.
pub fn restore_key_background(desktop: Desktop) {
    match desktop {
        Desktop::Caja | Desktop::Xfce => restore_background(),
        Desktop::Dde => {
            KEY_ACTIVE.store(false, Ordering::SeqCst);
            super::dde::restore();
        }
    }
}

/// Save the user's MATE background once and switch Caja to the key colour.
fn apply_key_mate() -> bool {
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

/// Put the user's MATE background back, and Xfce's xfconf backdrop. A no-op
/// when nothing was saved — so it is safe on every desktop and at every start,
/// which is how a crashed run's key colour gets cleaned up.
pub fn restore_background() {
    KEY_ACTIVE.store(false, Ordering::SeqCst);
    xfconf::restore();
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
        let c = Desktop::Caja;
        // LSB-first ZPixmap is B, G, R, pad; MSB-first is pad, R, G, B.
        assert!(is_key(&[1, 1, 1, 0], false, c));
        assert!(is_key(&[0, 1, 1, 1], true, c));
        assert!(!is_key(&[1, 1, 2, 0], false, c));
        // Pure black is the label shadow, not the key: it must stay visible.
        assert!(!is_key(&[0, 0, 0, 0], false, c));
        // Caja ignores the pad byte only, exactly as before.
        assert!(is_key(&[1, 1, 1, 255], false, c));
        assert_eq!(KEY_HEX, "#010101");
        assert_eq!(BG_KEYS[2].1, "'#010101'");
    }

    #[test]
    fn deepin_key_ignores_alpha_and_tolerates_reencoding() {
        let d = Desktop::Dde;
        // LSB-first B, G, R, A: any alpha.
        for a in [0u8, 128, 255] {
            assert!(is_key(&[1, 1, 1, a], false, d));
            assert!(is_key(&[a, 1, 1, 1], true, d));
        }
        // Off by up to 2 per channel is still the key; 3 away is not.
        assert!(is_key(&[3, 3, 3, 255], false, d));
        assert!(is_key(&[0, 2, 1, 255], false, d));
        assert!(!is_key(&[4, 1, 1, 255], false, d));
        assert!(!is_key(&[1, 1, 4, 255], false, d));
        // Real icon and wallpaper pixels are far from the key.
        assert!(!is_key(&[200, 180, 40, 255], false, d));
        // The exact-match flavour stays strict on the same inputs.
        assert!(!is_key(&[3, 3, 3, 255], false, Desktop::Caja));
    }

    #[test]
    fn desktop_flavours_match_only_their_own_window() {
        let dde = b"dde-shell/desktop\0org.deepin.dde-shell\0";
        let caja = b"desktop_window\0Caja\0";
        assert!(Desktop::Dde.matches(dde));
        assert!(!Desktop::Dde.matches(caja));
        assert!(Desktop::Caja.matches(caja));
        assert!(!Desktop::Caja.matches(dde));
        assert!(!Desktop::Dde.matches(b"dde-shell/dock\0org.deepin.dde-shell\0"));
        let xfdesktop = b"xfdesktop\0Xfdesktop\0";
        assert!(Desktop::Xfce.matches(xfdesktop));
        assert!(!Desktop::Xfce.matches(caja));
        assert!(!Desktop::Xfce.matches(dde));
        assert!(!Desktop::Caja.matches(xfdesktop));
        assert!(!Desktop::Dde.matches(xfdesktop));
    }

    #[test]
    fn xfce_matches_the_key_exactly_like_mate() {
        let x = Desktop::Xfce;
        assert!(is_key(&[1, 1, 1, 0], false, x));
        assert!(is_key(&[0, 1, 1, 1], true, x));
        // A near miss and pure black are icon pixels, not the key.
        assert!(!is_key(&[3, 3, 3, 255], false, x));
        assert!(!is_key(&[0, 0, 0, 0], false, x));
        assert_eq!(x.label(), "Xfce");
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

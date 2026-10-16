//! Wayland backend of the floating world clock: a `wlr-layer-shell` surface on
//! the OVERLAY layer with an empty input region, so it sits above every window
//! and clicks fall through to whatever is underneath.
//!
//! Layer-shell is offered by KWin, wlroots compositors (Sway, Hyprland, ...) and
//! COSMIC; GNOME/Mutter has none, so [`try_start`] errs there and the caller
//! falls back to the wallpaper-layer widget.
//!
//! Pixels go through `wl_shm` from a fresh `memfd` per frame. The clock repaints
//! once a minute, so one pool + buffer per repaint (released when the compositor
//! says so) is simpler than double-buffering and cannot tear.

use std::io::{ErrorKind, Write};
use std::os::fd::AsFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use nix::poll::{poll, PollFd, PollFlags};
use nix::sys::memfd::{memfd_create, MemFdCreateFlag};
use wayland_client::backend::WaylandError;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_output, wl_region, wl_registry, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, QueueHandle, WEnum};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self as ls, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1,
};

use crate::config::{LyricAnchor, WorldClock};
use crate::widgetkit::cards::worldclock::{render_pixmap, to_bgra_premul};
use crate::widgetkit::{scale_for_output, FontStack};

/// Longest the thread sleeps in `poll(2)`, so a `stop` is noticed within it.
const MAX_WAIT_MS: i64 = 500;

struct Out {
    proxy: wl_output::WlOutput,
    /// `wl_output.scale` (integer).
    scale: i32,
    /// Current mode height in device pixels.
    height: i32,
}

#[derive(Default)]
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    shell: Option<ZwlrLayerShellV1>,
    outs: Vec<Out>,
    /// The output the surface was last said to be on (`wl_surface.enter`).
    on: Option<wl_output::WlOutput>,
    configured: bool,
    closed: bool,
    /// Needs a repaint (first configure, output change, minute tick).
    dirty: bool,
}

impl State {
    /// `(buffer scale, logical output height)` for the output we are on, or the
    /// first one before the compositor has placed the surface.
    // ponytail: integer `wl_output.scale` only; a fractional-scale setup reports the
    // rounded-up factor, so the clock renders slightly small there. Upgrade is
    // wp_fractional_scale_v1 (needs wayland-protocols, not a dependency today).
    fn target(&self) -> (i32, u32) {
        let o = self
            .on
            .as_ref()
            .and_then(|on| self.outs.iter().find(|o| o.proxy == *on))
            .or(self.outs.first());
        o.map_or((1, 0), |o| {
            let s = o.scale.max(1);
            (s, u32::try_from(o.height / s).unwrap_or(0))
        })
    }
}

/// One rendered frame: premultiplied BGRA, padded with transparent pixels up to
/// a multiple of the buffer scale (`wl_surface` rejects anything else).
struct Frame {
    px: Vec<u8>,
    w: i32,
    h: i32,
    bs: i32,
}

fn render(fonts: &mut FontStack, cfg: &WorldClock, (bs, logical_h): (i32, u32)) -> Option<Frame> {
    let rows = crate::worldclock::rows(cfg, Utc::now());
    let scale = scale_for_output(logical_h) * bs as f32;
    let pm = render_pixmap(fonts, &rows, cfg.font_size_pt, scale)?;
    let (w, h, b) = (pm.width() as usize, pm.height() as usize, bs as usize);
    let (bw, bh) = (w.div_ceil(b) * b, h.div_ceil(b) * b);
    let src = to_bgra_premul(&pm);
    let mut px = vec![0u8; bw * bh * 4];
    for y in 0..h {
        px[y * bw * 4..][..w * 4].copy_from_slice(&src[y * w * 4..][..w * 4]);
    }
    Some(Frame {
        px,
        w: bw as i32,
        h: bh as i32,
        bs,
    })
}

fn anchor(a: LyricAnchor) -> Anchor {
    match a {
        LyricAnchor::TopLeft => Anchor::Top | Anchor::Left,
        LyricAnchor::TopCenter => Anchor::Top,
        LyricAnchor::TopRight => Anchor::Top | Anchor::Right,
        LyricAnchor::MidLeft => Anchor::Left,
        LyricAnchor::MidCenter => Anchor::empty(),
        LyricAnchor::MidRight => Anchor::Right,
        LyricAnchor::BottomLeft => Anchor::Bottom | Anchor::Left,
        LyricAnchor::BottomCenter => Anchor::Bottom,
        LyricAnchor::BottomRight => Anchor::Bottom | Anchor::Right,
    }
}

struct Overlay {
    cfg: WorldClock,
    conn: Connection,
    queue: EventQueue<State>,
    st: State,
    fonts: FontStack,
    surface: wl_surface::WlSurface,
    layer: ZwlrLayerSurfaceV1,
    /// Logical size last given to `set_size`.
    size: (i32, i32),
    /// The frame rendered to size the surface, shown on the first configure.
    pending: Option<Frame>,
}

impl Overlay {
    /// Render, resize if the text got wider or narrower, attach and commit.
    fn draw(&mut self) -> Result<()> {
        let f = self
            .pending
            .take()
            .or_else(|| render(&mut self.fonts, &self.cfg, self.st.target()));
        let Some(f) = f else {
            log::warn!("world clock overlay: nothing to draw, keeping the last frame");
            return Ok(());
        };
        let shm = self.st.shm.as_ref().context("wl_shm vanished")?;
        let qh = self.queue.handle();

        let mut file = std::fs::File::from(
            memfd_create(c"fresco-worldclock", MemFdCreateFlag::MFD_CLOEXEC)
                .context("memfd_create")?,
        );
        file.write_all(&f.px).context("writing the frame")?;
        let pool = shm.create_pool(file.as_fd(), f.px.len() as i32, &qh, ());
        let buf = pool.create_buffer(0, f.w, f.h, f.w * 4, wl_shm::Format::Argb8888, &qh, ());
        pool.destroy(); // the buffer keeps the mapping alive

        let size = (f.w / f.bs, f.h / f.bs);
        if size != self.size {
            self.layer.set_size(size.0 as u32, size.1 as u32);
            self.size = size;
        }
        self.surface.set_buffer_scale(f.bs);
        self.surface.attach(Some(&buf), 0, 0);
        self.surface.damage(0, 0, i32::MAX, i32::MAX);
        self.surface.commit();
        Ok(())
    }

    /// Flush, then sleep in `poll(2)` on the connection until the compositor
    /// sends something or `wait_ms` passes, and dispatch what arrived.
    fn pump(&mut self, wait_ms: u16) -> Result<()> {
        self.queue.dispatch_pending(&mut self.st)?;
        self.conn.flush()?;
        if let Some(guard) = self.queue.prepare_read() {
            let mut fds = [PollFd::new(guard.connection_fd(), PollFlags::POLLIN)];
            // Timeout or EINTR: drop the guard, which cancels the read.
            if poll(&mut fds, wait_ms).is_ok_and(|n| n > 0) {
                match guard.read() {
                    Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                    r => {
                        r?;
                    }
                }
            }
        }
        self.queue.dispatch_pending(&mut self.st)?;
        Ok(())
    }
}

fn run(mut ov: Overlay, stop: Arc<AtomicBool>) {
    let mut tick = crate::worldclock::next_change(Utc::now());
    while !stop.load(Ordering::Relaxed) && !ov.st.closed {
        if Utc::now() >= tick {
            ov.st.dirty = true;
            tick = crate::worldclock::next_change(Utc::now());
        }
        let drawn = if ov.st.dirty && ov.st.configured {
            ov.st.dirty = false;
            ov.draw()
        } else {
            Ok(())
        };
        // Sleep until the compositor speaks or the minute turns (at least 1 ms,
        // so a boundary we are a hair short of cannot spin).
        let wait = (tick - Utc::now())
            .num_milliseconds()
            .clamp(0, MAX_WAIT_MS - 1) as u16
            + 1;
        if let Err(e) = drawn.and_then(|()| ov.pump(wait)) {
            log::warn!("world clock overlay (wayland): {e:#}");
            break;
        }
    }
    ov.layer.destroy();
    ov.surface.destroy();
    let _ = ov.conn.flush();
}

/// Float the world clock above all windows via layer-shell. Everything that can
/// say "this compositor cannot" happens here, synchronously, so the caller can
/// fall back; the returned thread only paints and exits on `stop`.
pub fn try_start(
    cfg: &crate::config::WorldClock,
    stop: Arc<AtomicBool>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    let conn = Connection::connect_to_env().context("connecting to the Wayland display")?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());

    let mut st = State::default();
    queue
        .roundtrip(&mut st)
        .context("wayland registry roundtrip")?;
    // Second trip: each wl_output's mode/scale events.
    queue
        .roundtrip(&mut st)
        .context("wayland output roundtrip")?;
    let (Some(compositor), Some(shell), true) =
        (st.compositor.take(), st.shell.take(), st.shm.is_some())
    else {
        bail!("compositor has no layer-shell (needs wl_compositor, wl_shm, zwlr_layer_shell_v1); a floating clock is not possible here");
    };

    let mut fonts = FontStack::system();
    if !fonts.has_fonts() {
        bail!("no fonts installed, cannot draw the world clock");
    }
    let first = render(&mut fonts, cfg, st.target()).context("world clock has nothing to draw")?;

    let surface = compositor.create_surface(&qh, ());
    // Click-through: an empty input region.
    let empty = compositor.create_region(&qh, ());
    surface.set_input_region(Some(&empty));
    empty.destroy();

    let layer = shell.get_layer_surface(
        &surface,
        None,
        Layer::Overlay,
        "fresco-worldclock".into(),
        &qh,
        (),
    );
    let m = cfg.margin_px as i32;
    layer.set_anchor(anchor(cfg.anchor));
    layer.set_margin(m, m, m, m);
    layer.set_exclusive_zone(0); // float over panels, never reserve space
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    let size = (first.w / first.bs, first.h / first.bs);
    layer.set_size(size.0 as u32, size.1 as u32);
    surface.commit(); // buffer-less commit; the compositor answers with configure
    queue
        .roundtrip(&mut st)
        .context("creating the layer surface")?;
    if st.closed {
        bail!("compositor refused the layer surface");
    }

    let ov = Overlay {
        cfg: cfg.clone(),
        conn,
        queue,
        st,
        fonts,
        surface,
        layer,
        size,
        pending: Some(first),
    };
    std::thread::Builder::new()
        .name("worldclock-wayland".into())
        .spawn(move || run(ov, stop))
        .context("spawning the overlay thread")
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        st: &mut Self,
        reg: &wl_registry::WlRegistry,
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
            "wl_compositor" => st.compositor = Some(reg.bind(name, version.min(4), qh, ())),
            "wl_shm" => st.shm = Some(reg.bind(name, 1, qh, ())),
            "zwlr_layer_shell_v1" => st.shell = Some(reg.bind(name, 1, qh, ())),
            "wl_output" => st.outs.push(Out {
                proxy: reg.bind(name, version.min(3), qh, ()),
                scale: 1,
                height: 0,
            }),
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        st: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        let Some(o) = st.outs.iter_mut().find(|o| o.proxy == *output) else {
            return;
        };
        match event {
            wl_output::Event::Mode {
                flags: WEnum::Value(m),
                height,
                ..
            } if m.contains(wl_output::Mode::Current) => {
                o.height = height;
            }
            wl_output::Event::Scale { factor } => o.scale = factor,
            _ => return,
        }
        st.dirty = true;
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for State {
    fn event(
        st: &mut Self,
        _: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wl_surface::Event::Enter { output } = event {
            st.on = Some(output);
            st.dirty = true; // the output may have a different scale than the guess
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        st: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: ls::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        match event {
            ls::Event::Configure { serial, .. } => {
                layer.ack_configure(serial);
                if !st.configured {
                    st.configured = true;
                    st.dirty = true;
                }
            }
            ls::Event::Closed => st.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(
        _: &mut Self,
        buf: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wl_buffer::Event::Release = event {
            buf.destroy();
        }
    }
}

delegate_noop!(State: wl_compositor::WlCompositor);
delegate_noop!(State: ZwlrLayerShellV1);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_region::WlRegion);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldZone;
    use std::time::Duration;

    /// Needs a live Wayland session. Either starts (and you see two clock rows
    /// for 3 s in the top-right corner) or errs cleanly when there is no layer-shell.
    #[test]
    #[ignore = "needs a Wayland session"]
    fn overlay_starts_and_stops_or_errs_cleanly() {
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
        match try_start(&cfg, stop.clone()) {
            Ok(join) => {
                eprintln!("overlay started");
                std::thread::sleep(Duration::from_secs(3));
                stop.store(true, Ordering::Relaxed);
                join.join().expect("overlay thread panicked");
                eprintln!("overlay stopped");
            }
            Err(e) => eprintln!("clean Err: {e:#}"),
        }
    }
}

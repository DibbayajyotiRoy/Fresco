//! The Deepin mirror, end to end, against a headless X server.
//!
//! A fake dde-shell desktop window (depth 32, `dde-shell/desktop` WM_CLASS,
//! filled with the key colour and a handful of test icons) sits under two
//! magenta "wallpaper" windows. The real mirror thread runs against it, and the
//! test reads the screen back: every pixel must be either the wallpaper
//! (magenta) or the DDE pixel, exactly as [`mask::refine_mask`] says.
//!
//! Needs an X server with Composite, DAMAGE and a depth-32 visual, so it is
//! `#[ignore]`d. Run it with
//!
//! ```text
//! xvfb-run -a -s "+extension Composite +extension DAMAGE -screen 0 1280x800x24" \
//!     cargo test --lib xvfb_harness -- --ignored --nocapture --test-threads=1
//! ```
//!
//! There is no window manager under Xvfb, so the test plays one: it keeps
//! `_NET_CLIENT_LIST_STACKING` in step with the real stacking order, and
//! "clicks" the desktop by raising the fake DDE window above the wallpaper.

use super::*;
use x11rb::wrapper::ConnectionExt as _;

const W: usize = 1280;
const H: usize = 800;
const PANE: usize = 640;
const MAGENTA: [u8; 3] = [255, 0, 255];
/// What DDE sets `_NET_WM_WINDOW_OPACITY` to itself (0.99).
const DDE_OPACITY: u32 = 0xFD70_A3D7;

/// A `W × H` RGB picture of the fake DDE desktop.
#[derive(Clone)]
struct Scene {
    px: Vec<[u8; 3]>,
}

impl Scene {
    fn new() -> Scene {
        Scene {
            px: vec![KEY; W * H],
        }
    }

    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.px[yy * W + xx] = c;
            }
        }
    }

    /// Icons, ramps and a glyph, left pane and right pane.
    fn test_icons() -> Scene {
        let mut s = Scene::new();
        // A terminal-like icon: coloured body, black screen.
        s.rect(40, 40, 64, 64, [200, 120, 40]);
        s.rect(57, 60, 30, 24, [0, 0, 0]);
        // A ring with an enclosed key-coloured hole.
        s.rect(200, 40, 70, 70, [90, 200, 120]);
        s.rect(220, 60, 30, 30, KEY);
        // A white icon whose left edge ramps into the key (dark fringe).
        s.rect(400, 40, 60, 60, [255, 255, 255]);
        s.rect(398, 40, 1, 60, [90, 90, 90]);
        s.rect(397, 40, 1, 60, [30, 30, 30]);
        // A shadow ramp, 38 down to 5, to the right of the terminal icon.
        for (i, v) in [38u8, 28, 18, 8, 5].iter().enumerate() {
            s.rect(104 + i, 40, 1, 64, [*v, *v, *v]);
        }
        // A label-like glyph with a counter: must stay see-through.
        s.rect(60, 200, 14, 16, [255, 255, 255]);
        s.rect(64, 204, 6, 8, KEY);
        // Right pane: a blue icon with a dark-grey (not key) block inside.
        s.rect(700, 100, 64, 64, [60, 90, 220]);
        s.rect(716, 116, 20, 20, [20, 20, 20]);
        // And a highlighted, selection-like slab with a black icon in it.
        s.rect(900, 300, 120, 140, [10, 40, 90]);
        s.rect(930, 320, 60, 60, [0, 0, 0]);
        s
    }

    /// The `w`-wide strip starting at scene column `x0` (which may be negative
    /// or run off the right: those columns are the key colour), as tight RGB.
    fn strip(&self, x0: i32, w: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(w * H * 3);
        for y in 0..H {
            for dx in 0..w {
                let x = x0 + dx as i32;
                let p = if (0..W as i32).contains(&x) {
                    self.px[y * W + x as usize]
                } else {
                    KEY
                };
                out.extend_from_slice(&p);
            }
        }
        out
    }
}

struct Rig {
    conn: RustConnection,
    root: Window,
    stacking: Atom,
    opacity: Atom,
    dde: Window,
    /// The pixmap that is the fake DDE window's background: change it, clear
    /// the window, and the window's content changes (damage included).
    canvas: Pixmap,
    parents: Vec<Parent>,
}

impl Rig {
    fn new() -> Rig {
        let (conn, screen_num) = x11rb::connect(None).expect("an X server to test against");
        let screen = conn.setup().roots[screen_num].clone();
        assert_eq!(
            conn.setup().image_byte_order,
            ImageOrder::LSB_FIRST,
            "the harness assumes a little-endian server"
        );
        let visual = screen
            .allowed_depths
            .iter()
            .find(|d| d.depth == 32)
            .and_then(|d| {
                d.visuals
                    .iter()
                    .find(|v| v.class == VisualClass::TRUE_COLOR)
                    .map(|v| v.visual_id)
            })
            .expect("a depth-32 TrueColor visual");
        let root = screen.root;
        let stacking = intern(&conn, b"_NET_CLIENT_LIST_STACKING");
        let opacity = intern(&conn, opacity::ATOM_NAME);

        let colormap = conn.generate_id().unwrap();
        conn.create_colormap(ColormapAlloc::NONE, colormap, root, visual)
            .unwrap();
        let canvas = conn.generate_id().unwrap();
        conn.create_pixmap(32, canvas, root, W as u16, H as u16)
            .unwrap();
        let dde = conn.generate_id().unwrap();
        conn.create_window(
            32,
            dde,
            root,
            0,
            0,
            W as u16,
            H as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new()
                .colormap(colormap)
                .border_pixel(0)
                .background_pixmap(canvas),
        )
        .unwrap();
        conn.change_property8(
            PropMode::REPLACE,
            dde,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"dde-shell/desktop\0org.deepin.dde-shell\0",
        )
        .unwrap();
        conn.map_window(dde).unwrap();

        // Two magenta "wallpaper" windows, one per half, mapped after the
        // fake desktop so they start above it.
        let mut parents = Vec::new();
        for x in [0i16, PANE as i16] {
            let w = conn.generate_id().unwrap();
            conn.create_window(
                COPY_DEPTH_FROM_PARENT,
                w,
                root,
                x,
                0,
                PANE as u16,
                H as u16,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new().background_pixel(0x00ff_00ff),
            )
            .unwrap();
            conn.map_window(w).unwrap();
            parents.push(Parent {
                window: w,
                x,
                y: 0,
                width: PANE as u16,
                height: H as u16,
            });
        }
        conn.flush().unwrap();
        let rig = Rig {
            conn,
            root,
            stacking,
            opacity,
            dde,
            canvas,
            parents,
        };
        rig.sync_stack();
        rig
    }

    /// Draw `scene` into the fake desktop window.
    fn paint(&self, scene: &Scene) {
        let gc = self.conn.generate_id().unwrap();
        self.conn
            .create_gc(gc, self.canvas, &CreateGCAux::new())
            .unwrap();
        for y0 in (0..H).step_by(32) {
            let rows = 32.min(H - y0);
            let mut data = Vec::with_capacity(W * rows * 4);
            for p in &scene.px[y0 * W..(y0 + rows) * W] {
                data.extend_from_slice(&[p[2], p[1], p[0], 0xff]);
            }
            self.conn
                .put_image(
                    ImageFormat::Z_PIXMAP,
                    self.canvas,
                    gc,
                    W as u16,
                    rows as u16,
                    0,
                    y0 as i16,
                    0,
                    32,
                    &data,
                )
                .unwrap();
        }
        self.conn.free_gc(gc).unwrap();
        self.conn.clear_area(false, self.dde, 0, 0, 0, 0).unwrap();
        self.conn.flush().unwrap();
    }

    /// Play the window manager: publish the real stacking order of the three
    /// windows as `_NET_CLIENT_LIST_STACKING`.
    fn sync_stack(&self) {
        let ours = self.order();
        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.root,
                self.stacking,
                AtomEnum::WINDOW,
                &ours,
            )
            .unwrap();
        self.conn.flush().unwrap();
    }

    /// The fake desktop and the wallpaper windows, bottom-most first.
    fn order(&self) -> Vec<Window> {
        let tree = self.conn.query_tree(self.root).unwrap().reply().unwrap();
        tree.children
            .into_iter()
            .filter(|w| *w == self.dde || self.parents.iter().any(|p| p.window == *w))
            .collect()
    }

    fn desktop_below_wallpaper(&self) -> bool {
        let order = self.order();
        let pos = |w: Window| order.iter().position(|&o| o == w).unwrap();
        self.parents.iter().all(|p| pos(self.dde) < pos(p.window))
    }

    fn opacity_value(&self) -> Option<u32> {
        let r = self
            .conn
            .get_property(false, self.dde, self.opacity, AtomEnum::CARDINAL, 0, 1)
            .unwrap()
            .reply()
            .unwrap();
        r.value32().and_then(|mut v| v.next())
    }

    fn set_opacity(&self, v: Option<u32>) {
        match v {
            Some(v) => {
                self.conn
                    .change_property32(
                        PropMode::REPLACE,
                        self.dde,
                        self.opacity,
                        AtomEnum::CARDINAL,
                        &[v],
                    )
                    .unwrap();
            }
            None => {
                self.conn.delete_property(self.dde, self.opacity).unwrap();
            }
        }
        self.conn.flush().unwrap();
    }

    /// What the screen shows over `p`, as RGB.
    fn screen(&self, p: &Parent) -> Vec<[u8; 3]> {
        let img = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                p.x,
                p.y,
                p.width,
                p.height,
                !0,
            )
            .unwrap()
            .reply()
            .unwrap();
        img.data
            .chunks_exact(4)
            .map(|b| [b[2], b[1], b[0]])
            .collect()
    }

    /// Mismatches between the screen and what the mirror should show of
    /// `scene`, with the desktop window at `origin_x`. Empty when it is right.
    fn mismatches(&self, scene: &Scene, origin_x: i32) -> Vec<String> {
        let mut bad = Vec::new();
        for p in &self.parents {
            let strip = scene.strip(i32::from(p.x) - origin_x, PANE);
            let want = mask::refine_mask(&strip, PANE * 3, PANE, H, KEY, DDE_TOLERANCE);
            let got = self.screen(p);
            for i in 0..PANE * H {
                let (x, y) = (i % PANE, i / PANE);
                let src = [strip[i * 3], strip[i * 3 + 1], strip[i * 3 + 2]];
                let expected = if want[i] { src } else { MAGENTA };
                if got[i] != expected && bad.len() < 12 {
                    bad.push(format!(
                        "parent@{} ({x},{y}): screen {:?}, expected {:?} (mask {})",
                        p.x, got[i], expected, want[i]
                    ));
                }
            }
        }
        bad
    }

    fn wait_for(&self, what: &str, mut ok: impl FnMut(&Rig) -> bool) -> Duration {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(8) {
            if ok(self) {
                return t0.elapsed();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {what}");
    }

    fn wait_for_picture(&self, what: &str, scene: &Scene, origin_x: i32) -> Duration {
        let mut last = Vec::new();
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(8) {
            last = self.mismatches(scene, origin_x);
            if last.is_empty() {
                return t0.elapsed();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "{what}: the screen never matched; first mismatches:\n{}",
            last.join("\n")
        );
    }
}

fn intern(conn: &RustConnection, name: &[u8]) -> Atom {
    conn.intern_atom(false, name).unwrap().reply().unwrap().atom
}

#[test]
#[ignore = "needs Xvfb with Composite, DAMAGE and a depth-32 visual; see the module docs"]
fn dde_mirror_end_to_end() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(false)
        .try_init();
    let state = std::env::temp_dir().join(format!("fresco-harness-{}", std::process::id()));
    std::env::set_var("XDG_STATE_HOME", &state);
    let state_file = opacity::state_file();
    let _ = std::fs::remove_file(&state_file);

    let rig = Rig::new();
    let scene = Scene::test_icons();
    rig.paint(&scene);
    // DDE's own opacity, as dde-shell sets it.
    rig.set_opacity(Some(DDE_OPACITY));

    let mirror = Mirror::start(Desktop::Dde).expect("the mirror starts");
    mirror.set_parents(&rig.conn, rig.parents.clone());
    rig.conn.flush().unwrap();

    // 1. The first picture: icons visible, key and counters see-through,
    //    enclosed black filled, dark fringe and shadow tail peeled.
    let took = rig.wait_for_picture("the first picture", &scene, 0);
    println!("first picture matched after {took:?}");
    assert_eq!(
        rig.opacity_value(),
        Some(0),
        "the desktop window is hidden from the compositor"
    );
    let saved = std::fs::read_to_string(&state_file).expect("a state file while hidden");
    assert_eq!(
        opacity::parse_saved(&saved),
        Some(opacity::Saved {
            window: rig.dde,
            original: Some(DDE_OPACITY)
        })
    );
    // Spot checks on top of the whole-screen comparison, so a broken
    // `refine_mask` could not make both sides wrong the same way.
    let left = rig.screen(&rig.parents[0]);
    let at = |x: usize, y: usize| left[y * PANE + x];
    assert_eq!(at(70, 70), [0, 0, 0], "black screen of the icon is opaque");
    assert_eq!(at(230, 70), [1, 1, 1], "enclosed key hole is filled");
    assert_eq!(at(66, 207), MAGENTA, "a letter counter stays see-through");
    assert_eq!(at(397, 60), MAGENTA, "dark fringe peeled");
    assert_eq!(at(398, 60), [90, 90, 90], "mid blend kept");
    assert_eq!(
        at(108, 60),
        MAGENTA,
        "shadow tail pixel touching the key peeled"
    );
    assert_eq!(at(104, 60), [38, 38, 38], "shadow body kept");
    assert_eq!(at(10, 10), MAGENTA, "plain key is the wallpaper");

    // 2. DDE resets its opacity (it does): the mirror puts 0 back.
    rig.set_opacity(Some(DDE_OPACITY));
    let t = rig.wait_for("opacity back at 0", |r| r.opacity_value() == Some(0));
    println!("opacity re-applied after {t:?}");

    // 3. The desktop changes: an icon disappears and another appears. The
    //    result must be exact without any help from Expose events.
    let mut scene2 = scene.clone();
    scene2.rect(200, 40, 70, 70, KEY);
    scene2.rect(500, 400, 80, 80, [250, 200, 20]);
    scene2.rect(520, 420, 40, 40, [0, 0, 0]);
    rig.paint(&scene2);
    let took = rig.wait_for_picture("the repainted desktop", &scene2, 0);
    println!("repaint matched after {took:?}");

    // 4. The desktop window moves (KWin-style frame-relative events do not
    //    say where it really is): the copy offsets must follow.
    rig.conn
        .configure_window(rig.dde, &ConfigureWindowAux::new().x(-30))
        .unwrap();
    rig.conn.flush().unwrap();
    let took = rig.wait_for_picture("after moving the desktop window", &scene2, -30);
    println!("move matched after {took:?}");
    rig.conn
        .configure_window(rig.dde, &ConfigureWindowAux::new().x(0))
        .unwrap();
    rig.conn.flush().unwrap();
    rig.wait_for_picture("after moving it back", &scene2, 0);

    // 5. A click: the window manager raises the desktop above the wallpaper.
    //    It must be back underneath within a few frames.
    let click = Instant::now();
    rig.conn
        .configure_window(
            rig.dde,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        )
        .unwrap();
    rig.sync_stack();
    assert!(
        !rig.desktop_below_wallpaper(),
        "the click raised the desktop"
    );
    rig.wait_for("the desktop lowered again", |r| r.desktop_below_wallpaper());
    println!(
        "desktop back under the wallpaper {:?} after the click",
        click.elapsed()
    );
    assert_eq!(rig.opacity_value(), Some(0));
    rig.sync_stack();
    rig.wait_for_picture("after the click", &scene2, 0);
    assert!(!mirror.failed());

    // 6. Stop: DDE's own opacity comes back exactly, the state file goes, and
    //    the icon windows are gone.
    mirror.stop(&rig.conn);
    assert_eq!(rig.opacity_value(), Some(DDE_OPACITY), "restored exactly");
    assert!(!state_file.exists(), "state file removed");
    for p in &rig.parents {
        let kids = rig
            .conn
            .query_tree(p.window)
            .unwrap()
            .reply()
            .unwrap()
            .children;
        assert!(kids.is_empty(), "icon windows destroyed");
    }

    // 7. Again from "absent": it must end absent, not 0 and not a default.
    rig.set_opacity(None);
    let mirror = Mirror::start(Desktop::Dde).expect("the mirror starts again");
    mirror.set_parents(&rig.conn, rig.parents.clone());
    rig.conn.flush().unwrap();
    rig.wait_for_picture("the second run's picture", &scene2, 0);
    assert_eq!(rig.opacity_value(), Some(0));
    mirror.stop(&rig.conn);
    assert_eq!(rig.opacity_value(), None, "absent stays absent");

    // 8. Crash recovery: a run that died leaves the window at 0 and the file
    //    behind; the next start restores it.
    for original in [Some(DDE_OPACITY), None] {
        rig.set_opacity(Some(0));
        std::fs::create_dir_all(state_file.parent().unwrap()).unwrap();
        std::fs::write(
            &state_file,
            opacity::format_saved(opacity::Saved {
                window: rig.dde,
                original,
            }),
        )
        .unwrap();
        opacity::restore_saved();
        assert_eq!(rig.opacity_value(), original, "crash recovery");
        assert!(!state_file.exists());
    }

    let _ = std::fs::remove_dir_all(&state);
}

// -- Xfce ---------------------------------------------------------------------

/// A pane's icons: `(x, y, width, height, rgb)`, drawn over the key colour.
type Icons = Vec<(i16, i16, u16, u16, [u8; 3])>;

fn pixel(c: [u8; 3]) -> u32 {
    (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2])
}

/// What the mirror should show at `(x, y)` of a pane: the icon there, else
/// the wallpaper (the key colour is see-through).
fn icon_at(icons: &Icons, x: usize, y: usize) -> [u8; 3] {
    let (x, y) = (x as i32, y as i32);
    icons
        .iter()
        .rev()
        .find(|&&(ix, iy, w, h, _)| {
            (i32::from(ix)..i32::from(ix) + i32::from(w)).contains(&x)
                && (i32::from(iy)..i32::from(iy) + i32::from(h)).contains(&y)
        })
        .map_or(MAGENTA, |r| r.4)
}

/// Xfce (xfdesktop 4.19+): one 24-bit desktop window per monitor, each its own
/// source. Two fake ones sit side by side under two magenta wallpaper windows,
/// plus a dialog sharing xfdesktop's WM_CLASS that must not be mirrored. The
/// right-hand desktop window is destroyed and brought back, as a hot-plugged
/// monitor would.
#[test]
#[ignore = "needs Xvfb with Composite and DAMAGE; see the module docs"]
fn xfce_mirror_per_monitor_end_to_end() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(false)
        .try_init();
    let (conn, screen_num) = x11rb::connect(None).expect("an X server to test against");
    let screen = conn.setup().roots[screen_num].clone();
    assert_eq!(screen.root_depth, 24, "the harness wants a 24-bit screen");
    let root = screen.root;
    let stacking = intern(&conn, b"_NET_CLIENT_LIST_STACKING");
    let wm_type = intern(&conn, b"_NET_WM_WINDOW_TYPE");
    let desktop_type = intern(&conn, b"_NET_WM_WINDOW_TYPE_DESKTOP");
    let dialog_type = intern(&conn, b"_NET_WM_WINDOW_TYPE_DIALOG");

    // A window of `w × h` at `x` filled from its own background pixmap, so
    // changing the pixmap and clearing the window repaints it (damage and all).
    let make = |x: i16, y: i16, w: u16, h: u16, ty: Atom| -> (Window, Pixmap) {
        let canvas = conn.generate_id().unwrap();
        conn.create_pixmap(24, canvas, root, w, h).unwrap();
        let win = conn.generate_id().unwrap();
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            win,
            root,
            x,
            y,
            w,
            h,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().background_pixmap(canvas),
        )
        .unwrap();
        conn.change_property8(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"xfdesktop\0Xfdesktop\0",
        )
        .unwrap();
        conn.change_property32(PropMode::REPLACE, win, wm_type, AtomEnum::ATOM, &[ty])
            .unwrap();
        (win, canvas)
    };
    let paint = |win: Window, canvas: Pixmap, w: u16, h: u16, icons: &Icons| {
        let gc = conn.generate_id().unwrap();
        conn.create_gc(gc, canvas, &CreateGCAux::new().foreground(pixel(KEY)))
            .unwrap();
        let rect = |x, y, width, height| Rectangle {
            x,
            y,
            width,
            height,
        };
        conn.poly_fill_rectangle(canvas, gc, &[rect(0, 0, w, h)])
            .unwrap();
        for &(x, y, iw, ih, c) in icons {
            conn.change_gc(gc, &ChangeGCAux::new().foreground(pixel(c)))
                .unwrap();
            conn.poly_fill_rectangle(canvas, gc, &[rect(x, y, iw, ih)])
                .unwrap();
        }
        conn.free_gc(gc).unwrap();
        conn.clear_area(false, win, 0, 0, 0, 0).unwrap();
        conn.flush().unwrap();
    };

    let (w, h) = (PANE as u16, H as u16);
    let left: Icons = vec![
        (40, 40, 64, 64, [200, 120, 40]),
        (57, 60, 30, 24, [0, 0, 0]),
    ];
    let right: Icons = vec![
        (100, 200, 80, 80, [60, 90, 220]),
        (120, 220, 20, 20, [20, 20, 20]),
    ];
    let (a, a_canvas) = make(0, 0, w, h, desktop_type);
    let (mut b, mut b_canvas) = make(PANE as i16, 0, w, h, desktop_type);
    // xfdesktop's own dialog: same class, not a desktop window, solid red.
    let (dialog, dialog_canvas) = make(300, 300, 200, 100, dialog_type);
    paint(a, a_canvas, w, h, &left);
    paint(b, b_canvas, w, h, &right);
    let red: Icons = vec![(0, 0, 200, 100, [255, 0, 0])];
    paint(dialog, dialog_canvas, 200, 100, &red);
    for win in [a, b, dialog] {
        conn.map_window(win).unwrap();
    }
    // The wallpaper windows, mapped after, so they start above.
    let mut parents = Vec::new();
    for x in [0i16, PANE as i16] {
        let p = conn.generate_id().unwrap();
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            p,
            root,
            x,
            0,
            w,
            h,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().background_pixel(0x00ff_00ff),
        )
        .unwrap();
        conn.map_window(p).unwrap();
        parents.push(Parent {
            window: p,
            x,
            y: 0,
            width: w,
            height: h,
        });
    }
    conn.flush().unwrap();
    // No window manager under Xvfb: publish the stack as one would.
    let sync_stack = |known: &[Window]| {
        let order: Vec<Window> = conn
            .query_tree(root)
            .unwrap()
            .reply()
            .unwrap()
            .children
            .into_iter()
            .filter(|c| known.contains(c))
            .collect();
        conn.change_property32(PropMode::REPLACE, root, stacking, AtomEnum::WINDOW, &order)
            .unwrap();
        conn.flush().unwrap();
    };
    let known = |b: Window| {
        let mut k = vec![a, b, dialog];
        k.extend(parents.iter().map(|p| p.window));
        k
    };
    sync_stack(&known(b));

    // What the screen shows over each wallpaper window, against what it should.
    let mismatches = |panes: [Option<&Icons>; 2]| -> Vec<String> {
        let mut bad = Vec::new();
        for (p, icons) in parents.iter().zip(panes) {
            let img = conn
                .get_image(ImageFormat::Z_PIXMAP, root, p.x, p.y, w, h, !0)
                .unwrap()
                .reply()
                .unwrap();
            for (i, px) in img.data.chunks_exact(4).enumerate() {
                let (x, y) = (i % PANE, i / PANE);
                let want = icons.map_or(MAGENTA, |icons| icon_at(icons, x, y));
                let got = [px[2], px[1], px[0]];
                if got != want && bad.len() < 12 {
                    bad.push(format!(
                        "parent@{} ({x},{y}): screen {got:?}, expected {want:?}",
                        p.x
                    ));
                }
            }
        }
        bad
    };
    let wait_for_picture = |what: &str, panes: [Option<&Icons>; 2]| {
        let t0 = Instant::now();
        let mut last = Vec::new();
        while t0.elapsed() < Duration::from_secs(8) {
            last = mismatches(panes);
            if last.is_empty() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "{what}: the screen never matched; first mismatches:\n{}",
            last.join("\n")
        );
    };

    let mirror = Mirror::start(Desktop::Xfce).expect("the mirror starts");
    mirror.set_parents(&conn, parents.clone());
    conn.flush().unwrap();

    // 1. Each monitor's icons land on its own wallpaper window, and the dialog
    //    (red, over the left pane) is not copied.
    wait_for_picture("both monitors", [Some(&left), Some(&right)]);

    // 2. The right monitor's desktop window goes away: its icons go with it,
    //    the left monitor's stay.
    conn.destroy_window(b).unwrap();
    conn.flush().unwrap();
    sync_stack(&known(b));
    wait_for_picture("after a monitor's window went away", [Some(&left), None]);

    // 3. It returns with different icons (the mirror notices a new window
    //    when the stack moves).
    let right2: Icons = vec![(300, 100, 100, 100, [10, 200, 90])];
    (b, b_canvas) = make(PANE as i16, 0, w, h, desktop_type);
    paint(b, b_canvas, w, h, &right2);
    conn.map_window(b).unwrap();
    // Under the wallpaper windows, as xfwm4's lower layer keeps it.
    conn.configure_window(
        b,
        &ConfigureWindowAux::new()
            .sibling(parents[0].window)
            .stack_mode(StackMode::BELOW),
    )
    .unwrap();
    conn.flush().unwrap();
    sync_stack(&known(b));
    wait_for_picture("after it came back", [Some(&left), Some(&right2)]);
    assert!(!mirror.failed());

    // 4. Stop: the icon windows are gone.
    mirror.stop(&conn);
    for p in &parents {
        let kids = conn.query_tree(p.window).unwrap().reply().unwrap().children;
        assert!(kids.is_empty(), "icon windows destroyed");
    }
}

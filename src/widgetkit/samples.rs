//! The visual half of the proof.
//!
//! Contrast ratios can be asserted; "this looks like a glass card" cannot. So
//! every widget in this toolkit is also rendered to PNG over two deliberately
//! hostile backdrops and the files are left where a person can look at them.
//!
//! Both backdrops exist because the two themes fail in opposite directions: a
//! dark card is destroyed by a **bright, busy, high-chroma** wallpaper and a
//! light card is destroyed by a **near-black** one. A sample sheet that only
//! showed each theme over the backdrop it likes would prove nothing, so every
//! widget is rendered over both.
//!
//! Output goes to `$FRESCO_WIDGETKIT_SAMPLE_DIR`, or a subdirectory of the
//! system temp dir. The path is never hardcoded: a committed absolute path
//! would be somebody's machine, not a build artifact. The test prints every
//! path it writes.

use super::cards::{battery, clock, disc, greeting, media, nowplaying, visualizer};
use super::lockscene;
use super::*;
use crate::artwork::DiscCfg;
use anyhow::{Context, Result};

fn out_dir() -> std::path::PathBuf {
    std::env::var_os("FRESCO_WIDGETKIT_SAMPLE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("fresco-widgetkit-samples"))
}

/// Which wallpaper the sample is composited over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backdrop {
    /// Bright, busy and high-chroma — the dark theme's worst case, and the one
    /// that eats a translucent card's type alive.
    Hostile,
    /// Near-black with dim colour blooms — the light theme's worst case, and
    /// the one that eats a light card's *shadow*.
    Night,
}

impl Backdrop {
    fn name(self) -> &'static str {
        match self {
            Self::Hostile => "hostile",
            Self::Night => "night",
        }
    }
}

/// Paint a stand-in wallpaper.
///
/// Not a flat colour and not a gentle gradient: hard vertical stripes, a
/// checker and a blown-out hot spot, because those are exactly what a scrim
/// with no backdrop blur handles worst. If a card reads over this it reads over
/// a photograph.
fn backdrop(c: &mut Canvas, kind: Backdrop) {
    let b = c.bounds();
    match kind {
        Backdrop::Hostile => {
            c.rounded_rect(
                b,
                0.0,
                &Fill::linear(
                    Point::new(0.0, 0.0),
                    Point::new(b.w, b.h),
                    Color::rgb8(0xFF, 0xE0, 0x4D),
                    Color::rgb8(0x3D, 0xE0, 0xFF),
                ),
            );
            // High chroma, not just high luminance: a saturated magenta wash is
            // what makes a coloured mottle show through a light card.
            c.rounded_rect(
                b,
                0.0,
                &Fill::radial(
                    Point::new(b.w * 0.24, b.h * 0.74),
                    b.h * 0.85,
                    Color::rgb8(0xFF, 0x2D, 0xC6).with_alpha(0.72),
                    Color::rgb8(0xFF, 0x2D, 0xC6).with_alpha(0.0),
                ),
            );
            c.rounded_rect(
                b,
                0.0,
                &Fill::radial(
                    Point::new(b.w * 0.80, b.h * 0.16),
                    b.h * 0.70,
                    Color::WHITE.with_alpha(0.95),
                    Color::WHITE.with_alpha(0.0),
                ),
            );
        }
        Backdrop::Night => {
            c.rounded_rect(
                b,
                0.0,
                &Fill::linear(
                    Point::new(0.0, 0.0),
                    Point::new(b.w, b.h),
                    Color::rgb8(0x07, 0x09, 0x12),
                    Color::rgb8(0x1A, 0x06, 0x14),
                ),
            );
            for (fx, fy, col) in [
                (0.22_f32, 0.30_f32, Color::rgb8(0x2A, 0x4C, 0xFF)),
                (0.78, 0.72, Color::rgb8(0xFF, 0x3B, 0x5C)),
            ] {
                c.rounded_rect(
                    b,
                    0.0,
                    &Fill::radial(
                        Point::new(b.w * fx, b.h * fy),
                        b.h * 0.6,
                        col.with_alpha(0.35),
                        col.with_alpha(0.0),
                    ),
                );
            }
        }
    }
    // Hard high-frequency structure, so any place a widget fails to separate
    // from its background is obvious rather than plausible.
    let stripe = (b.w / 34.0).max(2.0);
    for i in 0..34 {
        let x = b.w * (i as f32 / 34.0);
        c.rounded_rect(
            Rect::new(x, 0.0, stripe * 0.34, b.h),
            0.0,
            &Fill::solid(Color::BLACK.with_alpha(0.16)),
        );
    }
    let cell = (b.h / 9.0).max(4.0);
    let mut y = 0.0;
    let mut row = 0;
    while y < b.h {
        let mut x = if row % 2 == 0 { 0.0 } else { cell };
        while x < b.w {
            c.rounded_rect(
                Rect::new(x, y, cell, cell),
                0.0,
                &Fill::solid(Color::WHITE.with_alpha(0.07)),
            );
            x += cell * 2.0;
        }
        y += cell;
        row += 1;
    }
}

/// A procedural album cover, so the image path is exercised without shipping a
/// binary asset. Deliberately **not square** — the cover path must centre-crop.
fn cover(w: u32, h: u32, accent: Color) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let t = (fx * 0.7 + fy * 0.3).clamp(0.0, 1.0);
        let base = accent.lerp(Color::BLACK, t * 0.9);
        let ring = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt();
        let c = if (0.26..0.30).contains(&ring) {
            base.lerp(Color::WHITE, 0.65)
        } else if (0.14..0.16).contains(&ring) {
            base.lerp(Color::WHITE, 0.3)
        } else {
            base
        };
        image::Rgba([
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
            255,
        ])
    })
}

/// A photographic stand-in cover: a sky gradient, a sun, two ridge lines and a
/// little grain. Multi-colour and **not** concentric.
///
/// The disc sample uses this rather than [`cover`] for a reason that cost a
/// review cycle: [`cover`]'s two white rings are drawn at a fixed fraction of
/// the image, so once the image is cropped to a circle they land right where a
/// record's grooves would be — and a reviewer looking at the disc reads the
/// *placeholder* as a groove defect. Test art for a record must not be
/// concentric. Still deliberately non-square, so the centre-crop path is
/// exercised exactly as before.
fn photo_cover(w: u32, h: u32) -> image::RgbaImage {
    // A cheap deterministic value hash, so the grain is stable across runs
    // without pulling in a PRNG.
    fn grain(x: u32, y: u32) -> f32 {
        let n = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
        let n = (n ^ (n >> 15)).wrapping_mul(0xC2B2_AE35);
        ((n ^ (n >> 13)) & 0xFFFF) as f32 / 65535.0 - 0.5
    }
    image::RgbaImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        // Sky: warm horizon into a cool zenith.
        let mut c = Color::rgb8(0x2A, 0x1B, 0x5E)
            .lerp(Color::rgb8(0xFF, 0x9E, 0x3D), (fy * 1.25).clamp(0.0, 1.0));
        // Sun, high right.
        let sun = ((fx - 0.68).powi(2) + (fy - 0.34).powi(2)).sqrt();
        if sun < 0.16 {
            c = c.lerp(Color::rgb8(0xFF, 0xF1, 0xB8), 1.0 - (sun / 0.16).powf(0.6));
        }
        // Two ridges, each a different hue, so the crop always has an edge in
        // it wherever it lands.
        let ridge_a = 0.62 + 0.06 * (fx * 7.0).sin() + 0.03 * (fx * 17.0).cos();
        let ridge_b = 0.78 + 0.05 * (fx * 4.0 + 1.7).sin();
        if fy > ridge_b {
            c = Color::rgb8(0x10, 0x1A, 0x2E);
        } else if fy > ridge_a {
            c = Color::rgb8(0x3C, 0x2A, 0x55);
        }
        let g = grain(x, y) * 0.06;
        let px = |v: f32| ((v + g).clamp(0.0, 1.0) * 255.0) as u8;
        image::Rgba([px(c.r), px(c.g), px(c.b), 255])
    })
}

/// A source-app icon for the badge.
fn app_icon(size: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(size, size, |x, y| {
        let f = (x + y) as f32 / (2.0 * size as f32);
        image::Rgba([(30.0 + 120.0 * f) as u8, (200.0 - 60.0 * f) as u8, 120, 255])
    })
}

/// Band magnitudes that look like music rather than like noise.
fn bands(n: usize, phase: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let f = i as f32 / n as f32;
            let env = (1.0 - f).powf(0.7);
            let wob = (f * 21.0 + phase).sin() * 0.5 + 0.5;
            (0.18 + 0.82 * env * (0.35 + 0.65 * wob)).clamp(0.02, 1.0)
        })
        .collect()
}

fn peaks_from(b: &[f32]) -> Vec<f32> {
    b.iter().map(|v| (v + 0.09).min(1.0)).collect()
}

fn sample_clock() -> clock::ClockData<'static> {
    clock::ClockData {
        time: "09:41",
        widest_time: "00:00",
        weekday: "Monday",
        date: "28 July",
        secondary: "Week 31 · GMT+05:30",
        font_size: 64.0,
        variant: clock::ClockVariant::Expanded,
        accent_follow: true,
        day_fraction: 0.41,
    }
}

fn sample_now<'a>(
    art: &'a image::RgbaImage,
    icon: &'a image::RgbaImage,
) -> nowplaying::NowPlayingData<'a> {
    nowplaying::NowPlayingData {
        label: "Now playing",
        title: "Blue Monday",
        artist: "New Order",
        album: "Substance",
        lyric: "I see a ship in the harbour",
        next_lyric: "I can and shall obey",
        lyric_is_stale: false,
        elapsed: "1:34",
        total: "7:29",
        position: Some(0.21),
        art: Some(art),
        badge: Some(icon),
        badge_label: "Spotify",
        chip: "FLAC · 44.1",
        font_size: 28.0,
        accent_follow: false,
        screen_width: 1920.0,
    }
}

fn sample_vis<'a>(
    b: &'a [f32],
    p: &'a [f32],
    variant: visualizer::VisualizerVariant,
) -> visualizer::VisualizerData<'a> {
    visualizer::VisualizerData {
        bands: b,
        peaks: Some(p),
        width: 520.0,
        height: 120.0,
        width_pct: 40.0,
        opacity: 0.92,
        rounded: true,
        paint: BarPaint::Vertical,
        style: SpectrumStyle::default(),
        variant,
        title: "Fatboy Slim — Ya Man",
        status: "4.8 MB · 1:34/3:52",
        elapsed: "01:34",
        bitrate: "278 KBPS",
        samplerate: "44 KHZ",
        position: Some(0.42),
    }
}

fn sample_media<'a>(
    b: &'a [f32],
    p: &'a [f32],
    art: Option<&'a image::RgbaImage>,
) -> media::MediaData<'a> {
    media::MediaData {
        state: media::PlayState::Playing,
        source: "Spotify",
        title: "Blue Monday",
        artist: "New Order",
        album: "Substance",
        lyric: "I see a ship in the harbour",
        next_lyric: "I can and shall obey",
        lyric_is_stale: false,
        show_next_line: true,
        elapsed: "1:34",
        total: "3:52",
        position: Some(0.41),
        bitrate: "278 KBPS",
        samplerate: "44 KHZ",
        art,
        bands: b,
        peaks: Some(p),
        bar_opacity: 0.92,
        rounded: true,
        font_size: 24.0,
        screen_width: 1920.0,
        layer: media::MediaLayer::All,
    }
}

fn sample_disc(art: &image::RgbaImage) -> disc::DiscData<'_> {
    disc::DiscData {
        art: Some(art),
        cfg: DiscCfg {
            size_px: 300,
            rotation_deg: 26.0,
            ..DiscCfg::default()
        },
        title: "Blue Monday",
        artist: "New Order",
    }
}

/// Which widget a sheet is showing.
#[derive(Debug, Clone, Copy)]
enum Widget {
    Clock,
    NowPlaying,
    /// The card treatment, which engages below 45% screen width.
    Visualiser,
    /// The card-less treatment, which is the **shipped default** at 60%.
    VisualiserBare,
    Chassis,
    Disc,
}

impl Widget {
    fn name(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::NowPlaying => "nowplaying",
            Self::Visualiser => "visualiser-panel",
            Self::VisualiserBare => "visualiser-bare",
            Self::Chassis => "visualiser-chassis",
            Self::Disc => "disc",
        }
    }
}

/// Render one widget alone, over `kind`, at `scale`.
fn one(w: Widget, mode: Mode, kind: Backdrop, scale: f32, fonts: &mut FontStack) -> Result<Canvas> {
    let t = Theme::for_accent(mode, crate::config::Accent::Blue);
    let art = cover(420, 300, t.accent);
    let photo = photo_cover(420, 300);
    let icon = app_icon(48);
    let b = bands(32, 1.1);
    let p = peaks_from(&b);

    let size = match w {
        Widget::Clock => clock::measure(fonts, &t, &sample_clock(), scale),
        Widget::NowPlaying => nowplaying::measure(fonts, &t, &sample_now(&art, &icon), scale),
        Widget::Visualiser => visualizer::measure(
            fonts,
            &t,
            &sample_vis(&b, &p, visualizer::VisualizerVariant::Panel),
            scale,
        ),
        Widget::VisualiserBare => visualizer::measure(
            fonts,
            &t,
            &sample_vis(&b, &p, visualizer::VisualizerVariant::Bare),
            scale,
        ),
        Widget::Chassis => visualizer::measure(
            fonts,
            &t,
            &sample_vis(&b, &p, visualizer::VisualizerVariant::Chassis),
            scale,
        ),
        Widget::Disc => disc::measure(fonts, &t, &sample_disc(&photo), scale),
    };
    let buf = size.buffer();
    let mut c = Canvas::for_logical(Size::new(buf.w + 32.0, buf.h + 32.0), scale)?;
    backdrop(&mut c, kind);
    // The card-less visualiser is anchored to the bottom edge in real use, and
    // its bottom scrim gradient is designed against that. Centring it in the
    // frame would show a hard edge below the baseline that never exists on a
    // screen.
    let card = match w {
        Widget::VisualiserBare => {
            let b = c.bounds();
            Rect::at(
                Point::new((b.w - size.card.w) / 2.0, b.h - size.card.h - 2.0),
                size.card,
            )
        }
        _ => size.card_in(c.bounds()),
    };
    match w {
        Widget::Clock => clock::draw_at(&mut c, fonts, &t, &sample_clock(), card),
        Widget::NowPlaying => {
            nowplaying::draw_at(&mut c, fonts, &t, &sample_now(&art, &icon), card)
        }
        Widget::Visualiser => visualizer::draw_at(
            &mut c,
            fonts,
            &t,
            &sample_vis(&b, &p, visualizer::VisualizerVariant::Panel),
            card,
        ),
        Widget::VisualiserBare => visualizer::draw_at(
            &mut c,
            fonts,
            &t,
            &sample_vis(&b, &p, visualizer::VisualizerVariant::Bare),
            card,
        ),
        Widget::Chassis => visualizer::draw_at(
            &mut c,
            fonts,
            &t,
            &sample_vis(&b, &p, visualizer::VisualizerVariant::Chassis),
            card,
        ),
        Widget::Disc => disc::draw_at(&mut c, fonts, &t, &sample_disc(&photo), card),
    }
    Ok(c)
}

/// All four widgets in one frame, which is the only way to see whether they
/// read as one system rather than as four separate exercises.
fn sheet(mode: Mode, kind: Backdrop, fonts: &mut FontStack) -> Result<Canvas> {
    let t = Theme::for_accent(mode, crate::config::Accent::Blue);
    let art = cover(420, 300, t.accent);
    let photo = photo_cover(420, 300);
    let icon = app_icon(48);
    let b = bands(40, 0.4);
    let p = peaks_from(&b);
    let mut c = Canvas::for_logical(Size::new(1080.0, 720.0), 1.0)?;
    backdrop(&mut c, kind);

    let clock_size = clock::measure(fonts, &t, &sample_clock(), 1.0);
    let now = sample_now(&art, &icon);
    let now_size = nowplaying::measure(fonts, &t, &t_now(&now), 1.0);
    let vis = sample_vis(&b, &p, visualizer::VisualizerVariant::Panel);
    let vis_size = visualizer::measure(fonts, &t, &vis, 1.0);
    let d = sample_disc(&photo);
    let disc_size = disc::measure(fonts, &t, &d, 1.0);

    clock::draw_at(
        &mut c,
        fonts,
        &t,
        &sample_clock(),
        Rect::at(Point::new(64.0, 56.0), clock_size.card),
    );
    disc::draw_at(
        &mut c,
        fonts,
        &t,
        &d,
        Rect::at(
            Point::new(1080.0 - 64.0 - disc_size.card.w, 44.0),
            disc_size.card,
        ),
    );
    nowplaying::draw_at(
        &mut c,
        fonts,
        &t,
        &now,
        Rect::at(Point::new(64.0, 400.0), now_size.card),
    );
    visualizer::draw_at(
        &mut c,
        fonts,
        &t,
        &vis,
        Rect::at(Point::new(556.0, 420.0), vis_size.card),
    );
    Ok(c)
}

/// Identity, so the sheet can size a card it also draws without cloning it.
fn t_now<'a>(d: &'a nowplaying::NowPlayingData<'a>) -> nowplaying::NowPlayingData<'a> {
    *d
}

#[test]
fn render_every_widget_over_both_hostile_backdrops() -> Result<()> {
    let mut fonts = FontStack::system();
    if fonts.face_count() == 0 {
        eprintln!("no fonts installed; skipping sample render");
        return Ok(());
    }
    let dir = out_dir();
    std::fs::create_dir_all(&dir)?;
    let mut written = 0usize;

    for w in [
        Widget::Clock,
        Widget::NowPlaying,
        Widget::Visualiser,
        Widget::VisualiserBare,
        Widget::Chassis,
        Widget::Disc,
    ] {
        for mode in [Mode::Dark, Mode::Light] {
            for kind in [Backdrop::Hostile, Backdrop::Night] {
                for scale in [1.0_f32, 2.0] {
                    let c = one(w, mode, kind, scale, &mut fonts)?;
                    let suffix = if scale > 1.5 { "@2x" } else { "" };
                    let theme = if mode.is_dark() { "dark" } else { "light" };
                    let name = format!("{}-{theme}-{}{suffix}.png", w.name(), kind.name());
                    let path = dir.join(&name);
                    c.save_png(&path)?;
                    eprintln!("widgetkit sample: {}", path.display());
                    let bgra = c.into_bgra();
                    assert!(bgra.data.iter().any(|&v| v != 0), "{name} is blank");
                    written += 1;
                }
            }
        }
    }

    for mode in [Mode::Dark, Mode::Light] {
        for kind in [Backdrop::Hostile, Backdrop::Night] {
            let c = sheet(mode, kind, &mut fonts)?;
            let theme = if mode.is_dark() { "dark" } else { "light" };
            let path = dir.join(format!("all-{theme}-{}.png", kind.name()));
            c.save_png(&path)?;
            eprintln!("widgetkit sample: {}", path.display());
            written += 1;
        }
    }

    // 6 treatments x 2 themes x 2 backdrops x 2 scales, plus 4 combined
    // sheets (one per theme per backdrop).
    assert_eq!(written, 52, "the sample set changed size");
    eprintln!("widgetkit samples: {written} files in {}", dir.display());
    Ok(())
}

/// Every spectrum shape draws something, and no two draw the same thing.
///
/// The regression this exists for: `VisualizerData` carried no shape at all
/// for several releases, so the picker offered five styles and the renderer
/// drew bars for every one of them. Nothing failed — the widget rendered
/// perfectly, just not as the thing that had been asked for. A test that only
/// checked "the visualiser draws" passed throughout, which is why this one
/// compares the shapes against *each other* rather than against a threshold.
#[test]
fn every_spectrum_style_draws_and_none_of_them_are_the_same_picture() {
    let mut fonts = FontStack::system();
    if fonts.face_count() == 0 {
        return;
    }
    let t = Theme::for_accent(Mode::Dark, crate::config::Accent::Blue);
    let b = bands(32, 1.1);
    let p = peaks_from(&b);
    let dir = out_dir();
    let _ = std::fs::create_dir_all(&dir);

    let mut seen: Vec<(&str, Vec<u8>)> = Vec::new();
    for (shape, name) in [
        (SpectrumStyle::Bars, "bars"),
        (SpectrumStyle::Mirror, "mirror"),
        (SpectrumStyle::Wave, "wave"),
        (SpectrumStyle::Dots, "dots"),
        (SpectrumStyle::Ring, "ring"),
    ] {
        let data = visualizer::VisualizerData {
            style: shape,
            ..sample_vis(&b, &p, visualizer::VisualizerVariant::Panel)
        };
        let size = visualizer::measure(&mut fonts, &t, &data, 2.0);
        let mut c = Canvas::for_logical(size.buffer(), 2.0).expect("canvas");
        visualizer::draw_at(&mut c, &mut fonts, &t, &data, size.card_rect());
        let _ = c.save_png(dir.join(format!("spectrum-{name}.png")));
        let px = c.into_bgra().data;
        assert!(px.iter().any(|&v| v != 0), "{name} drew nothing at all");
        for (other, prev) in &seen {
            assert_ne!(
                prev, &px,
                "`{name}` and `{other}` rasterise identically — the shape is \
                 being dropped somewhere between the config and the canvas"
            );
        }
        seen.push((name, px));
    }
    assert_eq!(seen.len(), 5, "every shape must have been rendered");
}

/// The composition test: every primitive, both themes, one buffer reused
/// throughout — the shape of a real widget's life on the daemon loop.
#[test]
fn a_whole_scene_stays_premultiplied_and_never_allocates_twice() {
    let mut fonts = FontStack::system();
    if fonts.face_count() == 0 {
        return;
    }
    let art = cover(300, 300, Color::rgb8(0x5E, 0x6A, 0xD2));
    let icon = app_icon(32);
    let b = bands(24, 0.0);
    let p = peaks_from(&b);
    let mut canvas = Canvas::for_logical(Size::new(340.0, 240.0), 1.0).unwrap();
    let mut buf = Vec::new();
    let mut caps = Vec::new();
    for mode in [Mode::Dark, Mode::Light] {
        let t = Theme::for_accent(mode, crate::config::Accent::Amber);
        for _ in 0..2 {
            canvas.reset();
            backdrop(&mut canvas, Backdrop::Hostile);
            let area = canvas.bounds().inset(20.0);
            clock::draw_at(&mut canvas, &mut fonts, &t, &sample_clock(), area);
            nowplaying::draw_at(&mut canvas, &mut fonts, &t, &sample_now(&art, &icon), area);
            visualizer::draw_at(
                &mut canvas,
                &mut fonts,
                &t,
                &sample_vis(&b, &p, visualizer::VisualizerVariant::Panel),
                area,
            );
            disc::draw_at(&mut canvas, &mut fonts, &t, &sample_disc(&art), area);
            canvas.write_bgra(&mut buf);
            caps.push(buf.capacity());
            for (i, px) in buf.chunks_exact(4).enumerate() {
                assert!(
                    px[0] <= px[3] && px[1] <= px[3] && px[2] <= px[3],
                    "{mode:?} pixel {i}: {px:?}"
                );
            }
        }
    }
    // Four full repaints of two themes, and the output buffer grew once.
    assert!(caps.windows(2).all(|w| w[0] == w[1]), "{caps:?}");
}

#[test]
#[ignore]
fn peek_nos() -> Result<()> {
    let mut fonts = FontStack::system();
    let dir = out_dir();
    std::fs::create_dir_all(&dir)?;
    // The short, happy string is the one the design was tuned against, and it
    // is not the one on anybody's desktop: with `show_seconds` the hero is
    // eight glyphs, and a real weekday and month are far longer than
    // "Tuesday · 28 July". The truncation bug this row grew was invisible until
    // these cases were rendered, so they stay rendered.
    let cases: [(&str, &str, &str, &str, &str); 5] = [
        ("plain", "14:32", "00:00", "Tuesday", "28 July"),
        ("seconds", "23:55:38", "00:00:00", "Thursday", "20 August"),
        (
            "longest",
            "23:55:38",
            "00:00:00",
            "Wednesday",
            "28 September",
        ),
        ("nodate", "23:55:38", "00:00:00", "", ""),
        ("cjk", "23:55:38", "00:00:00", "星期四", "八月二十日"),
    ];
    for (mode, name) in [(Mode::Dark, "dark"), (Mode::Light, "light")] {
        let t = Theme::for_accent(mode, crate::config::Accent::Blue);
        for (tag, time, widest_time, weekday, date) in cases {
            let d = clock::ClockData {
                time,
                widest_time,
                weekday,
                date,
                secondary: "9h 27m left today",
                font_size: 64.0,
                variant: clock::ClockVariant::Nos,
                accent_follow: false,
                day_fraction: 0.605,
            };
            let size = clock::measure(&mut fonts, &t, &d, 1.0);
            let buf = size.buffer();
            let mut c = Canvas::for_logical(Size::new(buf.w + 32.0, buf.h + 32.0), 2.0)?;
            backdrop(&mut c, Backdrop::Hostile);
            let at = size.card_in(c.bounds());
            clock::draw_at(&mut c, &mut fonts, &t, &d, at);
            c.save_png(dir.join(format!("peek-nos-{name}-{tag}.png")))?;
        }
        // The #54 customisation as the daemon applies it: recoloured theme copy,
        // no caption, 12-hour time, the whole card faded. On a bare canvas,
        // because the fade would otherwise take a backdrop with it.
        let custom = Theme {
            nos_red: Color::rgb8(0x2E, 0xC4, 0xB6),
            text_primary: Color::rgb8(0xFF, 0xD1, 0x66),
            ..t
        };
        let d = clock::ClockData {
            time: "9:05 PM",
            widest_time: "00:00 AM",
            weekday: "Tuesday",
            date: "28 July",
            secondary: "",
            font_size: 64.0,
            variant: clock::ClockVariant::Nos,
            accent_follow: false,
            day_fraction: 0.605,
        };
        let size = clock::measure(&mut fonts, &custom, &d, 1.0);
        let buf = size.buffer();
        let mut c = Canvas::for_logical(Size::new(buf.w + 32.0, buf.h + 32.0), 2.0)?;
        let at = size.card_in(c.bounds());
        clock::draw_at(&mut c, &mut fonts, &custom, &d, at);
        c.fade(c.bounds(), 0.0, 0.7);
        c.save_png(dir.join(format!("peek-nos-{name}-custom.png")))?;
    }
    Ok(())
}

#[test]
#[ignore]
fn peek_media() -> Result<()> {
    let mut fonts = FontStack::system();
    let dir = out_dir();
    std::fs::create_dir_all(&dir)?;
    let art = cover(420, 300, Color::rgb8(0x5E, 0x6A, 0xD2));
    let b = bands(28, 1.1);
    let p = peaks_from(&b);
    for (mode, name) in [(Mode::Dark, "dark"), (Mode::Light, "light")] {
        let t = Theme::for_accent(mode, crate::config::Accent::Blue);
        for (tag, d) in [
            ("full", sample_media(&b, &p, Some(&art))),
            (
                "bare",
                media::MediaData {
                    lyric: "",
                    next_lyric: "",
                    art: None,
                    position: None,
                    elapsed: "",
                    ..sample_media(&b, &p, None)
                },
            ),
        ] {
            let size = media::measure(&mut fonts, &t, &d, 1.0);
            let buf = size.buffer();
            let mut c = Canvas::for_logical(Size::new(buf.w + 32.0, buf.h + 32.0), 1.5)?;
            backdrop(&mut c, Backdrop::Hostile);
            let at = size.card_in(c.bounds());
            media::draw_at(&mut c, &mut fonts, &t, &d, at);
            c.save_png(dir.join(format!("peek-media-{name}-{tag}.png")))?;
        }
    }
    Ok(())
}

#[test]
#[ignore]
fn peek_lock() -> Result<()> {
    let mut fonts = FontStack::system();
    let dir = out_dir();
    std::fs::create_dir_all(&dir)?;
    let cases: [(&str, &str, &str, &str, &str); 5] = [
        ("plain", "09:41", "00:00", "Tuesday", "15 September"),
        (
            "seconds",
            "23:55:38",
            "00:00:00",
            "Wednesday",
            "28 September",
        ),
        ("12h", "9:41 AM", "00:00 AM", "Tuesday", "15 September"),
        ("nodate", "14:32", "00:00", "", ""),
        ("cjk", "23:55", "00:00", "星期四", "八月二十日"),
    ];
    for (mode, name) in [(Mode::Dark, "dark"), (Mode::Light, "light")] {
        let t = Theme::for_accent(mode, crate::config::Accent::Blue);
        for kind in [Backdrop::Hostile, Backdrop::Night] {
            for (tag, time, widest_time, weekday, date) in cases {
                let d = clock::ClockData {
                    time,
                    widest_time,
                    weekday,
                    date,
                    secondary: "9h 27m left today",
                    font_size: 83.0,
                    variant: clock::ClockVariant::Lock,
                    accent_follow: false,
                    day_fraction: 0.605,
                };
                let size = clock::measure(&mut fonts, &t, &d, 1.0);
                let buf = size.buffer();
                let mut c = Canvas::for_logical(Size::new(buf.w + 48.0, buf.h + 48.0), 2.0)?;
                backdrop(&mut c, kind);
                let at = size.card_in(c.bounds());
                clock::draw_at(&mut c, &mut fonts, &t, &d, at);
                let path = dir.join(format!("peek-lock-{name}-{}-{tag}.png", kind.name()));
                c.save_png(&path)?;
                println!("{}", path.display());
            }
        }
    }
    Ok(())
}

// -- the lock scene ----------------------------------------------------------
//
// `render_every_widget_over_both_hostile_backdrops` pins an exact file count,
// so the lock scene's much larger matrix (five arrangements, three sizes, two
// backdrops, two themes, plus the COSMIC-panel and compose_still cases) goes
// through the same `#[ignore]`d "peek" convention `peek_lock`/`peek_media`/
// `peek_nos` already use above, rather than growing that count.

/// A Canvas sized to `output` device pixels at `out_scale`, degrading the
/// *rendering* density (never the layout, which stays keyed to
/// `output / out_scale` regardless — see `lockscene`'s module docs on units)
/// when `output` alone would exceed [`MAX_CANVAS_AREA`] — exactly
/// [`lockscene::compose_still`]'s own fallback, reused here so a sample at
/// 3440x1440 (4.95 MP, over the 4 MP per-widget cap this toolkit holds every
/// *widget* to) still renders instead of failing the sample generator.
fn scene_canvas(output: Size, out_scale: f32) -> Result<Canvas> {
    let scale = if out_scale.is_finite() && out_scale > 0.0 {
        out_scale
    } else {
        1.0
    };
    let logical = Size::new((output.w / scale).max(1.0), (output.h / scale).max(1.0));
    let mut render_scale = scale;
    loop {
        match Canvas::for_logical(logical, render_scale) {
            Ok(c) => return Ok(c),
            Err(_) if render_scale > 0.1 => render_scale *= 0.5,
            Err(e) => return Err(e),
        }
    }
}

/// [`lockscene::compose_still`] returns a [`crate::artwork::Bgra`] rather than
/// a [`Canvas`] (a still composite is meant for a host to hand to its own
/// image pipeline, not to keep rasterising into) so it has no `save_png` of
/// its own; this un-premultiplies it back to straight RGBA for the PNG
/// encoder, the exact inverse of [`Color::to_premul_rgba8`].
fn save_bgra_png(bgra: &crate::artwork::Bgra, path: &std::path::Path) -> Result<()> {
    let mut img = image::RgbaImage::new(bgra.w, bgra.h);
    for (i, px) in bgra.data.chunks_exact(4).enumerate() {
        let c = Color::from_premul_rgba8([px[2], px[1], px[0], px[3]]);
        img.put_pixel(
            i as u32 % bgra.w,
            i as u32 / bgra.w,
            image::Rgba([
                (c.r * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.b * 255.0).round() as u8,
                (c.a * 255.0).round() as u8,
            ]),
        );
    }
    img.save(path)
        .with_context(|| format!("writing {}", path.display()))
}

const LOCK_ARRANGEMENTS: [LockArrangement; 5] = [
    LockArrangement::Classic,
    LockArrangement::Minimal,
    LockArrangement::Glass,
    LockArrangement::BigType,
    LockArrangement::Terminal,
];

fn lock_clock_variant(a: LockArrangement) -> ClockVariant {
    // Terminal "pairs with the NOS clock" (spec); everywhere else gets the
    // Lock variant's card-less, date-over-time treatment — see
    // `cards::lock`'s module docs for why that one has no card at all.
    if a == LockArrangement::Terminal {
        ClockVariant::Nos
    } else {
        ClockVariant::Lock
    }
}

/// The now-playing card for a lock scene. Lyrics are **off by default** — a
/// real lock screen shows the track, not the words, unless the user opted
/// in — so `with_lyrics` is `false` in every sample except the one dedicated
/// to proving that opt-in case still composes correctly, and `NowPlayingData`
/// already collapses the whole lyric block (and its divider) to nothing when
/// `lyric` is empty (spec §9.2), so there is no empty band to leave behind.
fn lock_scene_media<'a>(
    art: &'a image::RgbaImage,
    icon: &'a image::RgbaImage,
    with_lyrics: bool,
) -> nowplaying::NowPlayingData<'a> {
    nowplaying::NowPlayingData {
        lyric: if with_lyrics {
            "I see a ship in the harbour"
        } else {
            ""
        },
        next_lyric: if with_lyrics {
            "I can and shall obey"
        } else {
            ""
        },
        ..sample_now(art, icon)
    }
}

fn lock_scene_data<'a>(
    arrangement: LockArrangement,
    theme: Theme,
    avatar: &'a image::RgbaImage,
    art: &'a image::RgbaImage,
    with_lyrics: bool,
) -> LockSceneData<'a> {
    // `compose`/`render_slot` size the Clock, Greeting and Battery slots
    // themselves from the arrangement and the output (see lockscene.rs's
    // "suggested sizes" section) and overwrite whatever is set here, so these
    // three are just reasonable stand-ins for a caller inspecting the data
    // directly or measuring it outside `compose`.
    LockSceneData {
        clock: clock::ClockData {
            time: "9:41",
            widest_time: "00:00",
            weekday: "Tuesday",
            date: "15 September",
            secondary: "9h 27m left today",
            font_size: 64.0,
            variant: lock_clock_variant(arrangement),
            accent_follow: false,
            day_fraction: 0.62,
        },
        greeting: Some(greeting::GreetingData {
            text: "Good evening, Roy",
            avatar: Some(avatar),
            initials: "R",
            text_size: 20.0,
        }),
        media: Some(lock_scene_media(art, avatar, with_lyrics)),
        battery: Some(battery::BatteryData {
            percent: 64,
            charging: false,
            full: false,
            size: 0.0,
        }),
        theme,
    }
}

const LOCK_ALL_SLOTS: [LockSlot; 4] = [
    LockSlot::Clock,
    LockSlot::Greeting,
    LockSlot::Media,
    LockSlot::Battery,
];

/// Every arrangement, at the three sizes the spec names by name (a standard
/// 1080p desktop, a phone-proportioned portrait panel, an ultrawide), over
/// both hostile backdrops, in both themes — the full cross product a person
/// actually has to eyeball to sign off on this module's geometry.
#[test]
#[ignore]
fn peek_lockscene() -> Result<()> {
    let mut fonts = FontStack::system();
    let dir = out_dir();
    std::fs::create_dir_all(&dir)?;
    let avatar = cover(96, 96, Color::rgb8(0x6E, 0x4A, 0xD8));
    let art = photo_cover(420, 300);

    let sizes: [(f32, f32, &str); 3] = [
        (1920.0, 1080.0, "1920x1080"),
        (1080.0, 1920.0, "1080x1920"),
        (3440.0, 1440.0, "3440x1440"),
    ];

    for (mode, theme_name) in [(Mode::Dark, "dark"), (Mode::Light, "light")] {
        let theme = Theme::for_accent(mode, crate::config::Accent::Blue);
        for arrangement in LOCK_ARRANGEMENTS {
            let tag = format!("{arrangement:?}").to_lowercase();
            for &(w, h, size_name) in &sizes {
                for kind in [Backdrop::Hostile, Backdrop::Night] {
                    let spec = LockSceneSpec {
                        arrangement,
                        output: Size::new(w, h),
                        scale: 1.0,
                        slots: LOCK_ALL_SLOTS.to_vec(),
                        reserved: Vec::new(),
                    };
                    let data = lock_scene_data(arrangement, theme, &avatar, &art, false);
                    let mut c = scene_canvas(spec.output, spec.scale)?;
                    backdrop(&mut c, kind);
                    lockscene::compose(&mut c, &mut fonts, &spec, &data);
                    let path = dir.join(format!(
                        "peek-lockscene-{tag}-{size_name}-{theme_name}-{}.png",
                        kind.name()
                    ));
                    c.save_png(&path)?;
                    eprintln!("widgetkit sample: {}", path.display());
                }
            }
        }
    }

    // Lyrics are opt-in on the lock screen (spec review) — every sample above
    // is the default, lyric-less card. This is the one dedicated exception,
    // named so it reads as the opt-in case rather than as an inconsistency.
    {
        let theme = Theme::for_accent(Mode::Dark, crate::config::Accent::Blue);
        let spec = LockSceneSpec {
            arrangement: LockArrangement::Classic,
            output: Size::new(1920.0, 1080.0),
            scale: 1.0,
            slots: LOCK_ALL_SLOTS.to_vec(),
            reserved: Vec::new(),
        };
        let data = lock_scene_data(LockArrangement::Classic, theme, &avatar, &art, true);
        let mut c = scene_canvas(spec.output, spec.scale)?;
        backdrop(&mut c, Backdrop::Hostile);
        lockscene::compose(&mut c, &mut fonts, &spec, &data);
        let path = dir.join("peek-lockscene-classic-1920x1080-dark-hostile-with-lyrics.png");
        c.save_png(&path)?;
        eprintln!("widgetkit sample: {}", path.display());
    }
    Ok(())
}

/// One [`lockscene::compose_still`] sample: a synthetic photo-like gradient
/// background, blurred and dimmed exactly as a still-frame host would ask for
/// (blur at slider position 50 %, dim 0.25) — the one case in this module where the wallpaper
/// itself, not just the widgets, is this code's problem.
#[test]
#[ignore]
fn peek_lockscene_still() -> Result<()> {
    let mut fonts = FontStack::system();
    let dir = out_dir();
    std::fs::create_dir_all(&dir)?;
    let avatar = cover(96, 96, Color::rgb8(0x6E, 0x4A, 0xD8));
    let art = photo_cover(420, 300);
    let theme = Theme::for_accent(Mode::Dark, crate::config::Accent::Coral);
    let bg = photo_cover(960, 540);

    let spec = LockSceneSpec {
        arrangement: LockArrangement::Classic,
        output: Size::new(1920.0, 1080.0),
        scale: 1.0,
        slots: LOCK_ALL_SLOTS.to_vec(),
        reserved: Vec::new(),
    };
    let data = lock_scene_data(spec.arrangement, theme, &avatar, &art, false);
    let bgra = lockscene::compose_still(
        &mut fonts,
        &bg,
        &spec,
        &data,
        crate::lockscreen::blur_radius_for(50.0),
        0.25,
    );
    let path = dir.join("peek-lockscene-still-classic-1920x1080-dark.png");
    save_bgra_png(&bgra, &path)?;
    eprintln!("widgetkit sample: {}", path.display());
    Ok(())
}

/// One COSMIC-greeter-style sample per arrangement, at the two size/scale
/// pairs the coordinator asked for: the reserved panel is drawn as a faint
/// outline *in the sample only* — [`lockscene::compose`] itself never draws
/// the regions it was told to avoid, only what it placed — so the layout
/// around it can actually be judged by eye.
#[test]
#[ignore]
fn peek_lockscene_cosmic() -> Result<()> {
    let mut fonts = FontStack::system();
    let dir = out_dir();
    std::fs::create_dir_all(&dir)?;
    let avatar = cover(96, 96, Color::rgb8(0x6E, 0x4A, 0xD8));
    let art = photo_cover(420, 300);
    let theme = Theme::for_accent(Mode::Dark, crate::config::Accent::Teal);

    for &(w, h, scale, size_name) in &[
        (1920.0_f32, 1080.0_f32, 1.0_f32, "1920x1080@1x"),
        (2560.0, 1600.0, 1.5, "2560x1600@1.5x"),
        (1080.0, 1920.0, 1.0, "1080x1920@1x"),
    ] {
        for arrangement in LOCK_ARRANGEMENTS {
            let tag = format!("{arrangement:?}").to_lowercase();
            let output = Size::new(w, h);
            let reserved = lockscene::cosmic_greeter_zone(output, scale);
            let spec = LockSceneSpec {
                arrangement,
                output,
                scale,
                slots: LOCK_ALL_SLOTS.to_vec(),
                reserved: vec![reserved],
            };
            let data = lock_scene_data(arrangement, theme, &avatar, &art, false);
            let mut c = scene_canvas(output, scale)?;
            backdrop(&mut c, Backdrop::Night);
            lockscene::compose(&mut c, &mut fonts, &spec, &data);
            let logical = Rect::new(
                reserved.x / scale,
                reserved.y / scale,
                reserved.w / scale,
                reserved.h / scale,
            );
            c.hairline(
                logical,
                0.0,
                Color::rgb8(0xFF, 0x5A, 0x8C).with_alpha(0.85),
                2.0,
            );
            let path = dir.join(format!("peek-lockscene-cosmic-{tag}-{size_name}.png"));
            c.save_png(&path)?;
            eprintln!("widgetkit sample: {}", path.display());
        }
    }
    Ok(())
}

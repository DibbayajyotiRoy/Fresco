//! Where the lock-screen preview's background picture comes from — and the
//! guarantee that it always has one.
//!
//! # Why this exists (issue #37)
//!
//! The preview used to take the wallpaper's *full-size* still, hand it to
//! `lockscene::compose_still`, and trust it. But the canvas refuses to draw a
//! cover image past [`crate::widgetkit::MAX_CANVAS_PX`] (4096) on a side and
//! only logs a warning — so any wallpaper bigger than that (a 5K/8K video's
//! frame, a 6000x4000 photograph: ordinary downloads) produced a canvas with
//! *no background at all*. What was left was the dim veil (black at 20 %
//! alpha) and the widgets on a transparent PNG, which a GTK window shows over
//! its own dark background: a black screen with the clock and avatar floating
//! in it. Nothing was logged at the level anyone reads, and nothing fell back.
//!
//! # The contract now
//!
//! [`resolve`] walks an ordered [`plan_sources`] list — the wallpaper itself
//! first, the Library's cached thumbnail next — and **always returns a
//! picture**: if every source is missing or undecodable it returns a
//! generated gradient ([`fallback_gradient`]) rather than an error, and says
//! why in the daemon log. With no Fresco wallpaper chosen at all, the plan ends
//! in the user's own desktop wallpaper ([`BgSource::Desktop`], Deepin only)
//! ahead of that gradient: it is what their lock screen shows in that case.
//! [`fit_background`] then shrinks whatever was found
//! to the size the canvas can actually draw, and [`flatten_opaque`] makes the
//! final PNG opaque so that even a drawing failure further down can never
//! show through as a black window.
//!
//! The preview deliberately does **not** call `overview::render_still`: that
//! function deletes every `overview-*` file in the cache directory, and on
//! GNOME/Cinnamon/MATE one of those files *is* the desktop's current
//! background. A preview must never be able to blank the desktop.
//!
//! Everything that decides *which* source to use ([`plan_sources`],
//! [`match_thumbnail`]) or *how to fit it* ([`cover_crop`], [`fit_background`],
//! [`is_blank`], [`flatten_opaque`]) is pure and unit-tested; only
//! [`resolve`]'s injected frame extractor touches ffmpeg.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use serde::Deserialize;

use crate::config::{Kind, Wallpaper};
use crate::widgetkit::Canvas;

/// How many candidate files of one wallpaper are tried before moving on to the
/// Library thumbnail. A playlist or slideshow can hold hundreds; the first few
/// are as representative as the rest, and each failed video costs a process
/// spawn.
pub(super) const MAX_CANDIDATES: usize = 3;

/// Longest side kept in the cache after decoding. A 6000x4000 photograph is
/// ~96 MB as RGBA and would sit in the daemon for as long as it runs;
/// 3840 covers a 4K output with no visible loss (the preview is dimmed and
/// optionally blurred anyway).
pub(super) const MAX_CACHED_SIDE: u32 = 3840;

/// Refuse to decode a still image of more than this many pixels (~150 MP,
/// 600 MB of RGBA). The decoder allocates the whole frame; a hostile or
/// absurd file must not be able to take the daemon down through a preview.
const MAX_SOURCE_PIXELS: u64 = 150_000_000;

/// Total time [`resolve`] spends on sources that need a decode or an external
/// process before giving up on the wallpaper itself and using the (fast)
/// Library thumbnail or the gradient. The GUI waits 15 s for the whole reply.
pub(super) const BUDGET: Duration = Duration::from_secs(8);

/// Once the preview had to fall back to a thumbnail or the gradient, it tries
/// the real wallpaper again after this long — a transient failure (a drive
/// that was not mounted yet) heals itself without a restart, and without
/// re-spawning ffmpeg on every one-second refresh in the meantime.
pub(super) const DEGRADED_RETRY: Duration = Duration::from_secs(30);

/// Colour [`flatten_opaque`] composites under the finished frame. Not
/// "black": a dark slate, so the failure mode of a missing background is a
/// visibly deliberate backdrop rather than a hole in the screen.
pub(super) const BACKDROP: [u8; 3] = [28, 31, 41];

// ─── Source selection (pure) ──────────────────────────────────────────────────

/// One place a background picture might be read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BgSource {
    /// A still image, decoded in-process (with a ffmpeg fallback for the
    /// formats the `image` crate was built without: gif, bmp, tiff, avif).
    Image(PathBuf),
    /// A video (or any media ffmpeg opens): its poster frame, extracted on
    /// demand.
    Frame(PathBuf),
    /// The Library's cached thumbnail of this wallpaper: small, but already on
    /// disk and decodable without any external tool.
    Thumbnail(PathBuf),
    /// The user's own desktop wallpaper, for when Fresco has none to show (see
    /// `dde::user_wallpaper`): a still image, decoded like [`BgSource::Image`].
    Desktop(PathBuf),
}

impl BgSource {
    fn path(&self) -> &Path {
        match self {
            BgSource::Image(p)
            | BgSource::Frame(p)
            | BgSource::Thumbnail(p)
            | BgSource::Desktop(p) => p,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            BgSource::Image(_) => "image",
            BgSource::Frame(_) => "video frame",
            BgSource::Thumbnail(_) => "library thumbnail",
            BgSource::Desktop(_) => "desktop wallpaper",
        }
    }
}

/// What a background ended up being, for the log and the retry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BgOrigin {
    /// The wallpaper's own picture, at full quality.
    Wallpaper,
    /// The Library thumbnail: right picture, low resolution.
    Thumbnail,
    /// The user's own desktop wallpaper: Fresco had no picture of its own.
    Desktop,
    /// The generated gradient: no usable source at all.
    Placeholder,
}

/// The ordered list of sources to try for `w`, best first. Pure: it only reads
/// `w` and the two arguments, never the filesystem, so every wallpaper kind's
/// selection is table-testable. Existence is checked later, per attempt, so
/// that a missing file is *reported* rather than silently skipped.
///
/// * `slideshow_images` — the slideshow's resolved image list (hand-picked
///   `paths`, else a scan of its `folder`); only consulted for
///   [`Kind::Slideshow`].
/// * `thumbnail` — the Library thumbnail for this wallpaper if one is known
///   (see [`match_thumbnail`]); always the last resort before the gradient.
pub(super) fn plan_sources(
    w: &Wallpaper,
    slideshow_images: &[PathBuf],
    thumbnail: Option<PathBuf>,
) -> Vec<BgSource> {
    // The wallpaper's own media: the primary path first, then the list.
    let media = dedup_capped(w.path.iter().chain(w.paths.iter()).cloned());
    let mut plan: Vec<BgSource> = match w.kind {
        Kind::Image => media.into_iter().map(BgSource::Image).collect(),
        Kind::Video | Kind::Playlist => media.into_iter().map(BgSource::Frame).collect(),
        // A slideshow shows its own image list; `path`/`paths` on the same
        // wallpaper are leftovers from another kind and only a fallback.
        Kind::Slideshow => {
            let images = dedup_capped(slideshow_images.iter().cloned());
            let mut v: Vec<BgSource> = images.into_iter().map(BgSource::Image).collect();
            for p in media {
                let s = BgSource::Image(p);
                if v.len() < MAX_CANDIDATES && !v.contains(&s) {
                    v.push(s);
                }
            }
            v
        }
    };
    if let Some(t) = thumbnail {
        plan.push(BgSource::Thumbnail(t));
    }
    plan
}

/// First [`MAX_CANDIDATES`] distinct paths of `it`, order preserved.
fn dedup_capped(it: impl Iterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in it {
        if out.len() >= MAX_CANDIDATES {
            break;
        }
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// The fields of one Library entry (`entries.json`) the preview needs. The
/// GUI's own `LibraryEntry` is not linkable from the daemon (`gui` is a
/// separate feature), so this reads just these keys and ignores the rest.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct LibraryThumb {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub folder: Option<PathBuf>,
    #[serde(default)]
    pub thumbnail: Option<PathBuf>,
}

/// The Library thumbnail that belongs to `w`, if any. Pure.
///
/// An entry is matched by what it *is*, strongest evidence first: the same
/// playlist/slideshow list or folder (3), the same single file (2), the same
/// first file (1). The best score wins, earliest entry on a tie. The thumbnail
/// file is the entry's recorded one, else `thumbs_dir/<id>.png` — the
/// location the GUI writes new ones to.
pub(super) fn match_thumbnail(
    w: &Wallpaper,
    entries: &[LibraryThumb],
    thumbs_dir: &Path,
) -> Option<PathBuf> {
    let slide = w.slideshow.as_ref();
    let my_list: &[PathBuf] = match slide {
        Some(s) if !s.paths.is_empty() => &s.paths,
        _ => &w.paths,
    };
    let my_folder = slide.and_then(|s| s.folder.as_deref());
    let my_first: Option<&Path> = w
        .path
        .as_deref()
        .or_else(|| my_list.first().map(PathBuf::as_path));

    let score = |e: &LibraryThumb| -> u8 {
        let same_list = !my_list.is_empty() && e.paths == my_list;
        let same_folder = my_folder.is_some() && e.folder.as_deref() == my_folder;
        if same_list || same_folder {
            return 3;
        }
        if w.path.is_some() && e.path == w.path {
            return 2;
        }
        let entry_first = e.path.as_deref().or_else(|| e.paths.first().map(|p| &**p));
        if my_first.is_some() && entry_first == my_first {
            return 1;
        }
        0
    };

    let mut best: Option<(u8, &LibraryThumb)> = None;
    for e in entries {
        let s = score(e);
        if s > best.map_or(0, |(b, _)| b) {
            best = Some((s, e));
        }
    }
    let (_, e) = best?;
    e.thumbnail
        .clone()
        .or_else(|| (!e.id.is_empty()).then(|| thumbs_dir.join(format!("{}.png", e.id))))
}

/// [`match_thumbnail`] against the Library under `library_dir` (the GUI's
/// `…/fresco/library`). Any read or parse problem is "no thumbnail": this is a
/// fallback and must not itself be a way to fail.
pub(super) fn library_thumbnail_in(library_dir: &Path, w: &Wallpaper) -> Option<PathBuf> {
    let text = std::fs::read_to_string(library_dir.join("entries.json")).ok()?;
    let entries: Vec<LibraryThumb> = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            log::debug!("lock preview: library index unreadable, no thumbnail fallback: {e}");
            return None;
        }
    };
    match_thumbnail(w, &entries, &library_dir.join("thumbs"))
}

/// The real per-user Library directory — the GUI's `library_dir()`
/// (`…/fresco/library` under the XDG data home).
pub(super) fn default_library_dir() -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join("fresco").join("library"))
}

// ─── Loading (the only impure part, behind an injectable extractor) ───────────

/// Decode a still image file, refusing absurd sizes before allocating.
fn decode_still(path: &Path) -> Result<RgbaImage, String> {
    let (w, h) = image::image_dimensions(path).map_err(|e| format!("not decodable: {e}"))?;
    if u64::from(w) * u64::from(h) > MAX_SOURCE_PIXELS {
        return Err(format!(
            "{w}x{h} is larger than the {MAX_SOURCE_PIXELS}-pixel decode limit"
        ));
    }
    image::open(path)
        .map(image::DynamicImage::into_rgba8)
        .map_err(|e| format!("not decodable: {e}"))
}

/// Extract a frame of `src` through the real tools: `ffmpegthumbnailer`, then
/// plain `ffmpeg` one and five seconds in (a poster frame is often a black
/// fade-in, and `ffmpegthumbnailer` may simply not be installed). The first
/// non-blank frame wins; if every frame is blank the first decodable one is
/// returned, because a genuinely black video's honest still *is* black.
pub(super) fn extract_frame_with_tools(
    src: &Path,
    rotation: u16,
    deadline: Instant,
) -> Result<RgbaImage, String> {
    use crate::daemon::overview;

    let dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("fresco");
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.join(format!("lockpreview-frame-{}.png", std::process::id()));
    let filter = overview::transpose_filter(rotation);

    type Attempt<'a> = (&'static str, Box<dyn Fn() -> bool + 'a>);
    let attempts: [Attempt<'_>; 3] = [
        (
            "ffmpegthumbnailer",
            Box::new(|| overview::extract_frame(src, rotation, &out)),
        ),
        (
            "ffmpeg @1s",
            Box::new(|| overview::ffmpeg_frame(src, Some(1.0), filter, &out)),
        ),
        (
            "ffmpeg @5s",
            Box::new(|| overview::ffmpeg_frame(src, Some(5.0), filter, &out)),
        ),
    ];

    let mut blank: Option<RgbaImage> = None;
    let mut notes: Vec<String> = Vec::new();
    for (name, run) in attempts {
        if Instant::now() >= deadline {
            notes.push(format!("{name}: skipped, time budget used up"));
            break;
        }
        let _ = std::fs::remove_file(&out);
        if !run() {
            notes.push(format!("{name}: failed or not installed"));
            continue;
        }
        match decode_still(&out) {
            Ok(img) if is_blank(&img) => {
                notes.push(format!("{name}: frame was blank"));
                blank.get_or_insert(img);
            }
            Ok(img) => {
                let _ = std::fs::remove_file(&out);
                return Ok(img);
            }
            Err(e) => notes.push(format!("{name}: {e}")),
        }
    }
    let _ = std::fs::remove_file(&out);
    blank.ok_or_else(|| notes.join("; "))
}

/// Load one [`BgSource`]. `frames` extracts a frame from a media file; it is a
/// parameter so the selection logic around it is testable without ffmpeg.
fn load(
    src: &BgSource,
    frames: &dyn Fn(&Path) -> Result<RgbaImage, String>,
) -> Result<RgbaImage, String> {
    let path = src.path();
    if !path.exists() {
        return Err("file does not exist".to_string());
    }
    match src {
        BgSource::Image(_) | BgSource::Desktop(_) => decode_still(path).or_else(|e| {
            // gif/bmp/tiff/avif are not compiled into `image`; ffmpeg reads
            // them. Say why the first attempt failed if this one does too.
            frames(path).map_err(|e2| format!("{e}; frame extraction also failed ({e2})"))
        }),
        BgSource::Frame(_) => frames(path),
        BgSource::Thumbnail(_) => decode_still(path),
    }
}

/// A resolved background and how it was found.
pub(super) struct Resolved {
    pub image: RgbaImage,
    pub origin: BgOrigin,
    /// One line per source that was tried and failed, in order — what the
    /// daemon logs when `origin` is not [`BgOrigin::Wallpaper`] (or when the
    /// first choice failed and a later one rescued the preview).
    pub failures: Vec<String>,
}

/// Walk `plan` and return the first source that loads, else the gradient.
/// **Never fails.** Sources that need real work are skipped once `deadline`
/// passes; the Library thumbnail is still tried (a small PNG decode).
pub(super) fn resolve(
    plan: &[BgSource],
    deadline: Instant,
    frames: &dyn Fn(&Path) -> Result<RgbaImage, String>,
) -> Resolved {
    let mut failures: Vec<String> = Vec::new();
    if plan.is_empty() {
        failures.push(
            "no wallpaper file is configured (no path, playlist or slideshow images)".to_string(),
        );
    }
    for src in plan {
        let is_thumb = matches!(src, BgSource::Thumbnail(_));
        if !is_thumb && Instant::now() >= deadline {
            failures.push(format!(
                "{} {}: skipped, time budget used up",
                src.label(),
                src.path().display()
            ));
            continue;
        }
        match load(src, frames) {
            Ok(image) => {
                let origin = match src {
                    BgSource::Thumbnail(_) => BgOrigin::Thumbnail,
                    BgSource::Desktop(_) => BgOrigin::Desktop,
                    _ => BgOrigin::Wallpaper,
                };
                return Resolved {
                    image,
                    origin,
                    failures,
                };
            }
            Err(e) => failures.push(format!("{} {}: {e}", src.label(), src.path().display())),
        }
    }
    Resolved {
        image: fallback_gradient(),
        origin: BgOrigin::Placeholder,
        failures,
    }
}

// ─── Fitting and finishing (pure) ─────────────────────────────────────────────

/// A soft diagonal gradient, used only when no wallpaper source at all could
/// be read. Opaque, clearly not black, and unmistakably a stand-in.
pub(super) fn fallback_gradient() -> RgbaImage {
    const W: u32 = 640;
    const H: u32 = 360;
    const A: [f32; 3] = [38.0, 56.0, 104.0];
    const B: [f32; 3] = [112.0, 64.0, 126.0];
    RgbaImage::from_fn(W, H, |x, y| {
        let t = (x as f32 / W as f32) * 0.6 + (y as f32 / H as f32) * 0.4;
        let mix = |i: usize| (A[i] + (B[i] - A[i]) * t).round().clamp(0.0, 255.0) as u8;
        image::Rgba([mix(0), mix(1), mix(2), 255])
    })
}

/// Shrink `img` (aspect preserved) so its longest side is at most `max_side`.
/// Unchanged — not copied — when it already fits.
pub(super) fn shrink_to_longest(img: RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    if longest <= max_side || w == 0 || h == 0 {
        return img;
    }
    let k = f64::from(max_side) / f64::from(longest);
    let nw = ((f64::from(w) * k).round() as u32).max(1);
    let nh = ((f64::from(h) * k).round() as u32).max(1);
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
}

/// The largest centred rectangle of aspect `tw:th` that fits inside `sw x sh`,
/// as `(w, h)` — i.e. the part of the source a "cover" fit shows. Total: a
/// zero input yields `(sw, sh)` unchanged.
pub(super) fn cover_crop(sw: u32, sh: u32, tw: u32, th: u32) -> (u32, u32) {
    if sw == 0 || sh == 0 || tw == 0 || th == 0 {
        return (sw, sh);
    }
    let (sw64, sh64, tw64, th64) = (u64::from(sw), u64::from(sh), u64::from(tw), u64::from(th));
    if sw64 * th64 > sh64 * tw64 {
        // Source is wider than the target: keep full height, crop the sides.
        let cw = ((sh64 * tw64 + th64 / 2) / th64).clamp(1, sw64);
        (cw as u32, sh)
    } else {
        // Taller (or equal): keep full width, crop top and bottom.
        let ch = ((sw64 * th64 + tw64 / 2) / tw64).clamp(1, sh64);
        (sw, ch as u32)
    }
}

/// Fit `img` to what the canvas can draw for an `out_w x out_h` render: cropped
/// to the output's aspect (exactly what `Canvas::image_cover` would show) and
/// scaled down to the canvas size if larger. **Never larger than the canvas
/// caps**, which is the whole point — an oversize cover is silently not drawn.
/// Never scaled *up*: a small thumbnail stays small and is stretched, softly,
/// by the canvas itself. A source that is already the right size is shared,
/// not copied.
pub(super) fn fit_background(img: &Arc<RgbaImage>, out_w: u32, out_h: u32) -> Arc<RgbaImage> {
    let (sw, sh) = img.dimensions();
    if sw == 0 || sh == 0 {
        return img.clone();
    }
    let (tw, th) = Canvas::clamp_size(out_w, out_h);
    let (cw, ch) = cover_crop(sw, sh, tw, th);
    let (cx, cy) = ((sw - cw) / 2, (sh - ch) / 2);
    let view = image::imageops::crop_imm(&**img, cx, cy, cw, ch);
    if cw <= tw && ch <= th {
        if (cw, ch) == (sw, sh) {
            return img.clone();
        }
        return Arc::new(view.to_image());
    }
    Arc::new(image::imageops::resize(
        &*view,
        tw,
        th,
        image::imageops::FilterType::Triangle,
    ))
}

/// `(mean, max)` perceived brightness (0..=255) over a coarse grid of `img`,
/// each sample weighted by its alpha — a transparent pixel counts as black.
fn luma_stats(img: &RgbaImage) -> (f32, u8) {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return (0.0, 0);
    }
    let (sx, sy) = ((w / 64).max(1), (h / 64).max(1));
    let (mut sum, mut n, mut max) = (0.0f32, 0u32, 0u8);
    let mut y = 0;
    while y < h {
        let mut x = 0;
        while x < w {
            let [r, g, b, a] = img.get_pixel(x, y).0;
            let l = (299.0 * f32::from(r) + 587.0 * f32::from(g) + 114.0 * f32::from(b)) / 1000.0
                * (f32::from(a) / 255.0);
            sum += l;
            n += 1;
            max = max.max(l.round() as u8);
            x += sx;
        }
        y += sy;
    }
    (sum / n.max(1) as f32, max)
}

/// Whether a freshly extracted video frame is effectively black (or empty): a
/// fade-in, a failed hardware decode. Used only to try another frame — a
/// still *image* that is genuinely dark is never second-guessed.
pub(super) fn is_blank(img: &RgbaImage) -> bool {
    let (mean, max) = luma_stats(img);
    mean < 5.0 && max < 32
}

/// Composite `img` over the opaque `base` colour in place, leaving every pixel
/// fully opaque. A no-op on an already-opaque frame; on a frame whose
/// background failed to draw it turns "transparent, so the window behind shows
/// through" into a deliberate backdrop.
pub(super) fn flatten_opaque(img: &mut RgbaImage, base: [u8; 3]) {
    for px in img.pixels_mut() {
        let a = u16::from(px.0[3]);
        if a == 255 {
            continue;
        }
        for (c, &b) in px.0.iter_mut().zip(base.iter()) {
            // Straight alpha: out = src*a + base*(1-a).
            *c = ((u16::from(*c) * a + u16::from(b) * (255 - a) + 127) / 255) as u8;
        }
        px.0[3] = 255;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Slideshow;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba(rgba))
    }

    fn wp(kind: Kind, path: Option<&str>, paths: &[&str]) -> Wallpaper {
        Wallpaper {
            kind,
            path: path.map(p),
            paths: paths.iter().map(|s| p(s)).collect(),
            ..Default::default()
        }
    }

    // -- plan_sources: one table row per wallpaper kind --------------------

    #[test]
    fn image_wallpaper_plans_the_image_then_the_thumbnail() {
        let plan = plan_sources(
            &wp(Kind::Image, Some("/w/a.jpg"), &[]),
            &[],
            Some(p("/lib/t.png")),
        );
        assert_eq!(
            plan,
            vec![
                BgSource::Image(p("/w/a.jpg")),
                BgSource::Thumbnail(p("/lib/t.png"))
            ]
        );
    }

    #[test]
    fn video_wallpaper_plans_a_frame_not_an_image_decode() {
        let plan = plan_sources(&wp(Kind::Video, Some("/w/a.mp4"), &[]), &[], None);
        assert_eq!(plan, vec![BgSource::Frame(p("/w/a.mp4"))]);
    }

    #[test]
    fn playlist_tries_its_first_items_in_order_without_duplicates() {
        let plan = plan_sources(
            &wp(
                Kind::Playlist,
                Some("/w/1.mp4"),
                &["/w/1.mp4", "/w/2.mp4", "/w/3.mp4", "/w/4.mp4"],
            ),
            &[],
            None,
        );
        assert_eq!(
            plan,
            vec![
                BgSource::Frame(p("/w/1.mp4")),
                BgSource::Frame(p("/w/2.mp4")),
                BgSource::Frame(p("/w/3.mp4")),
            ],
            "primary path first, deduplicated, capped at MAX_CANDIDATES"
        );
    }

    #[test]
    fn playlist_with_only_a_list_uses_the_list() {
        let plan = plan_sources(
            &wp(Kind::Playlist, None, &["/w/x.mp4", "/w/y.mp4"]),
            &[],
            None,
        );
        assert_eq!(
            plan,
            vec![
                BgSource::Frame(p("/w/x.mp4")),
                BgSource::Frame(p("/w/y.mp4"))
            ]
        );
    }

    #[test]
    fn slideshow_plans_its_images_then_leftover_media() {
        let mut w = wp(Kind::Slideshow, Some("/w/stale.mp4"), &[]);
        w.slideshow = Some(Slideshow {
            folder: Some(p("/pics")),
            paths: vec![],
            interval_s: 30,
            transition: Default::default(),
            recursive: false,
        });
        let imgs = vec![p("/pics/a.jpg"), p("/pics/b.jpg")];
        let plan = plan_sources(&w, &imgs, Some(p("/lib/s.png")));
        assert_eq!(
            plan,
            vec![
                BgSource::Image(p("/pics/a.jpg")),
                BgSource::Image(p("/pics/b.jpg")),
                BgSource::Image(p("/w/stale.mp4")),
                BgSource::Thumbnail(p("/lib/s.png")),
            ]
        );
    }

    #[test]
    fn slideshow_with_no_images_still_plans_the_thumbnail() {
        let plan = plan_sources(&wp(Kind::Slideshow, None, &[]), &[], Some(p("/lib/s.png")));
        assert_eq!(plan, vec![BgSource::Thumbnail(p("/lib/s.png"))]);
    }

    #[test]
    fn a_wallpaper_with_nothing_configured_plans_nothing() {
        for kind in [Kind::Image, Kind::Video, Kind::Playlist, Kind::Slideshow] {
            assert!(plan_sources(&wp(kind, None, &[]), &[], None).is_empty());
        }
    }

    // -- match_thumbnail ----------------------------------------------------

    fn entry(id: &str, path: Option<&str>, paths: &[&str], folder: Option<&str>) -> LibraryThumb {
        LibraryThumb {
            id: id.to_string(),
            path: path.map(p),
            paths: paths.iter().map(|s| p(s)).collect(),
            folder: folder.map(p),
            thumbnail: None,
        }
    }

    #[test]
    fn thumbnail_is_matched_by_file_and_defaults_to_the_thumbs_dir() {
        let entries = vec![
            entry("other", Some("/w/other.mp4"), &[], None),
            entry("abc", Some("/w/a.mp4"), &[], None),
        ];
        let got = match_thumbnail(
            &wp(Kind::Video, Some("/w/a.mp4"), &[]),
            &entries,
            Path::new("/lib/thumbs"),
        );
        assert_eq!(got, Some(p("/lib/thumbs/abc.png")));
    }

    #[test]
    fn a_recorded_thumbnail_path_wins_over_the_derived_one() {
        let mut e = entry("abc", Some("/w/a.mp4"), &[], None);
        e.thumbnail = Some(p("/elsewhere/a.png"));
        let got = match_thumbnail(
            &wp(Kind::Video, Some("/w/a.mp4"), &[]),
            &[e],
            Path::new("/lib/thumbs"),
        );
        assert_eq!(got, Some(p("/elsewhere/a.png")));
    }

    #[test]
    fn identical_playlist_beats_a_playlist_that_merely_shares_the_first_item() {
        let entries = vec![
            entry("shares-first", None, &["/w/1.mp4", "/w/9.mp4"], None),
            entry("exact", None, &["/w/1.mp4", "/w/2.mp4"], None),
        ];
        let got = match_thumbnail(
            &wp(Kind::Playlist, None, &["/w/1.mp4", "/w/2.mp4"]),
            &entries,
            Path::new("/t"),
        );
        assert_eq!(got, Some(p("/t/exact.png")));
    }

    #[test]
    fn slideshow_matches_by_folder() {
        let mut w = wp(Kind::Slideshow, None, &[]);
        w.slideshow = Some(Slideshow {
            folder: Some(p("/pics")),
            paths: vec![],
            interval_s: 30,
            transition: Default::default(),
            recursive: false,
        });
        let entries = vec![
            entry("no", None, &[], Some("/elsewhere")),
            entry("yes", None, &[], Some("/pics")),
        ];
        assert_eq!(
            match_thumbnail(&w, &entries, Path::new("/t")),
            Some(p("/t/yes.png"))
        );
    }

    #[test]
    fn no_match_and_no_id_both_mean_no_thumbnail() {
        let entries = vec![entry("abc", Some("/w/a.mp4"), &[], None)];
        let w = wp(Kind::Video, Some("/w/zzz.mp4"), &[]);
        assert_eq!(match_thumbnail(&w, &entries, Path::new("/t")), None);
        let nameless = vec![entry("", Some("/w/zzz.mp4"), &[], None)];
        assert_eq!(match_thumbnail(&w, &nameless, Path::new("/t")), None);
        assert_eq!(
            match_thumbnail(&wp(Kind::Video, None, &[]), &entries, Path::new("/t")),
            None
        );
    }

    #[test]
    fn library_index_is_read_tolerantly() {
        let dir = std::env::temp_dir().join(format!("fresco-lockbg-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let w = wp(Kind::Video, Some("/w/a.mp4"), &[]);

        // No index at all.
        assert_eq!(library_thumbnail_in(&dir, &w), None);
        // Malformed index.
        std::fs::write(dir.join("entries.json"), "{ not json").unwrap();
        assert_eq!(library_thumbnail_in(&dir, &w), None);
        // A real-shaped index with fields this module does not know about.
        std::fs::write(
            dir.join("entries.json"),
            r#"[{"id":"e1","name":"A","kind":"video","path":"/w/a.mp4","last_used":3,"broken":false,"future_field":{"x":1}}]"#,
        )
        .unwrap();
        assert_eq!(
            library_thumbnail_in(&dir, &w),
            Some(dir.join("thumbs").join("e1.png"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- resolve ------------------------------------------------------------

    fn never(_: &Path) -> Result<RgbaImage, String> {
        Err("no frames in this test".to_string())
    }

    fn far() -> Instant {
        Instant::now() + Duration::from_secs(600)
    }

    fn write_png(tag: &str, img: &RgbaImage) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("fresco-lockbg-{}-{tag}.png", std::process::id()));
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn resolve_uses_the_first_source_that_loads() {
        let ok = write_png("first", &solid(8, 8, [200, 10, 10, 255]));
        let plan = vec![
            BgSource::Image(p("/definitely/missing.jpg")),
            BgSource::Image(ok.clone()),
        ];
        let r = resolve(&plan, far(), &never);
        assert_eq!(r.origin, BgOrigin::Wallpaper);
        assert_eq!(r.image.get_pixel(0, 0).0, [200, 10, 10, 255]);
        assert_eq!(r.failures.len(), 1, "the missing file is reported");
        assert!(r.failures[0].contains("missing.jpg"), "{:?}", r.failures);
        let _ = std::fs::remove_file(ok);
    }

    #[test]
    fn a_video_frame_comes_from_the_injected_extractor() {
        let frames = |path: &Path| -> Result<RgbaImage, String> {
            assert_eq!(path, Path::new("/proc/self/exe")); // exists on Linux
            Ok(solid(4, 4, [1, 2, 3, 255]))
        };
        let r = resolve(&[BgSource::Frame(p("/proc/self/exe"))], far(), &frames);
        assert_eq!(r.origin, BgOrigin::Wallpaper);
        assert_eq!(r.image.get_pixel(0, 0).0, [1, 2, 3, 255]);
    }

    #[test]
    fn an_undecodable_image_is_rescued_by_frame_extraction() {
        // Not an image `image` can read; the extractor (ffmpeg in real life)
        // can — this is how a gif/bmp/tiff wallpaper still previews.
        let bogus =
            std::env::temp_dir().join(format!("fresco-lockbg-{}-bogus.gif", std::process::id()));
        std::fs::write(&bogus, b"not really a gif").unwrap();
        let frames = |_: &Path| Ok(solid(4, 4, [9, 9, 9, 255]));
        let r = resolve(&[BgSource::Image(bogus.clone())], far(), &frames);
        assert_eq!(r.origin, BgOrigin::Wallpaper);
        let _ = std::fs::remove_file(bogus);
    }

    #[test]
    fn a_dead_wallpaper_falls_back_to_the_library_thumbnail() {
        let thumb = write_png("thumb", &solid(16, 9, [10, 120, 200, 255]));
        let plan = vec![
            BgSource::Frame(p("/definitely/missing.mp4")),
            BgSource::Thumbnail(thumb.clone()),
        ];
        let r = resolve(&plan, far(), &never);
        assert_eq!(r.origin, BgOrigin::Thumbnail);
        assert_eq!(r.image.get_pixel(1, 1).0, [10, 120, 200, 255]);
        assert!(r.failures.iter().any(|f| f.contains("does not exist")));
        let _ = std::fs::remove_file(thumb);
    }

    #[test]
    fn the_desktop_wallpaper_beats_the_gradient_and_is_marked_degraded() {
        let desk = write_png("desk", &solid(16, 9, [30, 140, 60, 255]));
        let r = resolve(&[BgSource::Desktop(desk.clone())], far(), &never);
        assert_eq!(r.origin, BgOrigin::Desktop);
        assert_eq!(r.image.get_pixel(1, 1).0, [30, 140, 60, 255]);
        // A missing one still ends at the gradient, reported.
        let r = resolve(
            &[BgSource::Desktop(p("/definitely/gone.jpg"))],
            far(),
            &never,
        );
        assert_eq!(r.origin, BgOrigin::Placeholder);
        assert!(r.failures[0].starts_with("desktop wallpaper"));
        let _ = std::fs::remove_file(desk);
    }

    #[test]
    fn with_nothing_usable_the_gradient_is_returned_never_an_error() {
        let plan = vec![
            BgSource::Image(p("/definitely/missing.jpg")),
            BgSource::Thumbnail(p("/definitely/missing-thumb.png")),
        ];
        let r = resolve(&plan, far(), &never);
        assert_eq!(r.origin, BgOrigin::Placeholder);
        assert_eq!(r.failures.len(), 2, "every failed source is reported");
        assert!(!is_blank(&r.image));
        // And an empty plan (nothing configured) is explained, not silent.
        let r = resolve(&[], far(), &never);
        assert_eq!(r.origin, BgOrigin::Placeholder);
        assert_eq!(r.failures.len(), 1);
        assert!(r.failures[0].contains("no wallpaper file"));
    }

    #[test]
    fn past_the_deadline_only_the_cheap_thumbnail_is_still_tried() {
        let ok = write_png("late-img", &solid(8, 8, [200, 10, 10, 255]));
        let thumb = write_png("late-thumb", &solid(8, 8, [0, 200, 0, 255]));
        let plan = vec![
            BgSource::Image(ok.clone()),
            BgSource::Thumbnail(thumb.clone()),
        ];
        let expired = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let r = resolve(&plan, expired, &never);
        assert_eq!(r.origin, BgOrigin::Thumbnail);
        assert!(r.failures[0].contains("time budget"), "{:?}", r.failures);
        let _ = std::fs::remove_file(ok);
        let _ = std::fs::remove_file(thumb);
    }

    // -- fitting ------------------------------------------------------------

    #[test]
    fn cover_crop_keeps_the_target_aspect_and_stays_inside_the_source() {
        // Wider source, taller source, equal aspect, degenerate.
        assert_eq!(cover_crop(8000, 3000, 1920, 1080), (5333, 3000));
        assert_eq!(cover_crop(3000, 8000, 1920, 1080), (3000, 1688));
        assert_eq!(cover_crop(1920, 1080, 1920, 1080), (1920, 1080));
        assert_eq!(cover_crop(0, 10, 1920, 1080), (0, 10));
        assert_eq!(cover_crop(10, 10, 0, 1080), (10, 10));
        for (sw, sh) in [(1, 1), (7, 3), (5000, 5000), (6000, 4000), (4096, 4097)] {
            for (tw, th) in [(1920, 1080), (1080, 1920), (7, 9), (1, 1)] {
                let (cw, ch) = cover_crop(sw, sh, tw, th);
                assert!(
                    cw >= 1 && ch >= 1 && cw <= sw && ch <= sh,
                    "{sw}x{sh} -> {cw}x{ch}"
                );
            }
        }
    }

    #[test]
    fn an_oversize_photograph_is_fitted_to_the_canvas_not_dropped() {
        // The #37 inputs: a 6000x4000 photo and a 5K frame, neither of which
        // the canvas will draw as-is.
        for (sw, sh) in [(6000u32, 4000u32), (5120, 2880), (8000, 3000)] {
            let big = Arc::new(solid(sw, sh, [200, 100, 50, 255]));
            let fitted = fit_background(&big, 1920, 1080);
            assert_eq!(fitted.dimensions(), (1920, 1080), "{sw}x{sh}");
            assert!(
                fitted.width() <= crate::widgetkit::MAX_CANVAS_PX
                    && fitted.height() <= crate::widgetkit::MAX_CANVAS_PX
            );
            assert_eq!(fitted.get_pixel(5, 5).0, [200, 100, 50, 255]);
        }
    }

    #[test]
    fn an_8k_output_asks_for_no_more_than_the_canvas_can_hold() {
        let big = Arc::new(solid(6000, 4000, [1, 2, 3, 255]));
        let fitted = fit_background(&big, 7680, 4320);
        assert!(fitted.width() <= crate::widgetkit::MAX_CANVAS_PX);
        assert!(fitted.height() <= crate::widgetkit::MAX_CANVAS_PX);
        assert!(
            u64::from(fitted.width()) * u64::from(fitted.height())
                <= u64::from(crate::widgetkit::MAX_CANVAS_AREA)
        );
    }

    #[test]
    fn a_small_source_is_cropped_to_aspect_but_never_scaled_up() {
        let thumb = Arc::new(solid(256, 256, [5, 6, 7, 255]));
        let fitted = fit_background(&thumb, 1920, 1080);
        assert_eq!(fitted.dimensions(), (256, 144));
        // A long thin strip past the canvas cap is cropped to the output's
        // aspect too, which is what brings it back under the cap.
        let strip = Arc::new(solid(4097, 100, [5, 6, 7, 255]));
        assert_eq!(fit_background(&strip, 1920, 1080).dimensions(), (178, 100));
    }

    #[test]
    fn a_source_that_already_fits_is_shared_not_copied() {
        let exact = Arc::new(solid(1920, 1080, [5, 6, 7, 255]));
        let fitted = fit_background(&exact, 1920, 1080);
        assert!(Arc::ptr_eq(&exact, &fitted));
    }

    #[test]
    fn shrink_to_longest_only_shrinks() {
        let big = shrink_to_longest(solid(6000, 4000, [1, 1, 1, 255]), MAX_CACHED_SIDE);
        assert_eq!(big.dimensions(), (3840, 2560));
        let small = shrink_to_longest(solid(100, 50, [1, 1, 1, 255]), MAX_CACHED_SIDE);
        assert_eq!(small.dimensions(), (100, 50));
    }

    // -- blank detection, gradient, flatten ---------------------------------

    #[test]
    fn blank_frames_are_detected_but_dark_or_varied_ones_are_not() {
        assert!(is_blank(&solid(64, 64, [0, 0, 0, 255])));
        assert!(is_blank(&solid(64, 64, [3, 3, 3, 255])));
        assert!(is_blank(&solid(64, 64, [200, 200, 200, 0])), "transparent");
        assert!(!is_blank(&solid(64, 64, [60, 60, 60, 255])), "dark grey");
        assert!(!is_blank(&solid(64, 64, [0, 0, 90, 255])), "deep blue");
        let mut lit = solid(64, 64, [0, 0, 0, 255]);
        for x in 0..64 {
            lit.put_pixel(x, 0, image::Rgba([255, 255, 255, 255]));
        }
        assert!(!is_blank(&lit), "a bright row is not a blank frame");
        assert!(is_blank(&RgbaImage::new(0, 0)));
    }

    #[test]
    fn the_fallback_gradient_is_opaque_and_visibly_not_black() {
        let g = fallback_gradient();
        assert!(g.pixels().all(|px| px.0[3] == 255));
        assert!(!is_blank(&g));
        let (mean, _) = luma_stats(&g);
        assert!(mean > 40.0, "mean luma {mean}");
    }

    #[test]
    fn flatten_makes_everything_opaque_and_leaves_opaque_pixels_alone() {
        let mut img = RgbaImage::new(3, 1);
        img.put_pixel(0, 0, image::Rgba([0, 0, 0, 0])); // hole
        img.put_pixel(1, 0, image::Rgba([255, 255, 255, 255])); // already opaque
        img.put_pixel(2, 0, image::Rgba([255, 255, 255, 128])); // half
        flatten_opaque(&mut img, [28, 31, 41]);
        assert_eq!(img.get_pixel(0, 0).0, [28, 31, 41, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [255, 255, 255, 255]);
        let half = img.get_pixel(2, 0).0;
        assert_eq!(half[3], 255);
        assert!(half[0] > 130 && half[0] < 160, "{half:?}");
    }

    #[test]
    fn decode_still_refuses_what_it_cannot_read() {
        assert!(decode_still(Path::new("/definitely/missing.png")).is_err());
        let junk =
            std::env::temp_dir().join(format!("fresco-lockbg-{}-junk.png", std::process::id()));
        std::fs::write(&junk, b"nope").unwrap();
        assert!(decode_still(&junk).is_err());
        let _ = std::fs::remove_file(junk);
    }
}

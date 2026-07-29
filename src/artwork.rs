//! Rotating "vinyl disc" album art for the wallpaper (WIDGETS_ROADMAP W4).
//!
//! # Why this module exists at all
//!
//! ASS — the format the lyrics and clock widgets ride on — has **no bitmap
//! support**, so cover art cannot go through the same path. mpv's
//! `overlay-add` can: it reads raw pixels straight out of a file. This module
//! produces those pixels; the caller writes them to a file and issues the
//! command. Nothing here talks to mpv, and nothing here knows where the disc
//! lands on screen.
//!
//! # Layering
//!
//! Exactly one function does I/O ([`load_bytes`]). Everything else — URL
//! parsing, base64, the whole renderer, the rotation clock, the redraw gate —
//! is pure, so the interesting parts are testable with no network, no D-Bus,
//! no desktop and no player.
//!
//! ```text
//!   mpris::NowPlaying.art_url
//!        │  parse_art_url          (pure)
//!        ▼
//!   ArtSource::{Http,File,Data}
//!        │  load_bytes             (the ONLY I/O, size-capped)
//!        ▼
//!   bytes ── decode_art ─────────► image::RgbaImage ──┐   (or placeholder_art)
//!                                                     │  render_disc  (pure)
//!                                                     ▼
//!                                                    Bgra ──► overlay-add
//! ```
//!
//! # Pixel format and alpha convention
//!
//! [`render_disc`] emits **BGRA with premultiplied alpha**, because that is
//! what mpv demands. From the mpv manual's `overlay-add` entry, on the only
//! defined `fmt`, `bgra`:
//!
//! > The least significant 8 bits are blue, and the most significant 8 bits
//! > are alpha (in little endian, the components are B-G-R-A, with B as first
//! > byte). \[…\] This uses premultiplied alpha: every color component is
//! > already multiplied with the alpha component. This means the numeric value
//! > of each component is equal to or smaller than the alpha component.
//! > Violating this rule will lead to different results with different VOs:
//! > numeric overflows resulting from blending broken alpha values is
//! > considered something that shouldn't happen.
//!
//! So the invariant `max(B, G, R) <= A` is not cosmetic — breaking it produces
//! VO-dependent garbage. Every write in [`render_disc`] clamps the colour
//! bytes to the alpha byte after rounding, and the sampler premultiplies
//! *before* interpolating (interpolating straight alpha bleeds the colour of
//! fully transparent source pixels into the disc edge).
//!
//! Rows are tightly packed: `stride == w * 4`, see [`Bgra::stride`].
//!
//! # Power
//!
//! The roadmap's power model says the disc rotates **only while playing** —
//! paused means no redraw at all. This module contributes two pieces of that:
//! [`rotation_for`] turns elapsed playing time into an angle (so the caller
//! never needs a render loop to "advance" anything), and [`should_redraw`]
//! lets the caller drop frames whose visible rotation change is below
//! perception, which is what makes the pause ease-out settle to zero cost
//! instead of spinning until the angle is exactly equal.

use std::ffi::OsString;
use std::io::Read;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context};
use image::RgbaImage;

// ---------------------------------------------------------------------------
// Caps
// ---------------------------------------------------------------------------

/// Hard ceiling on artwork bytes, enforced on **every** source.
///
/// `mpris:artUrl` is attacker-adjacent input: it is a URL chosen by whatever
/// media the user happened to open, and for `Http` it points at a third-party
/// server that can stream forever. Real cover art is 20 KiB – 2 MiB; 8 MiB
/// clears even an oversized lossless PNG with room to spare while making
/// "exhaust the daemon's memory" impossible.
///
/// For `Http` this is checked twice — once against `Content-Length` and again
/// mid-stream, because servers lie (`download.rs` learned the same lesson).
pub const MAX_ART_BYTES: u64 = 8 * 1024 * 1024;

/// Largest source image edge [`decode_art`] will decode.
///
/// A size cap on the *compressed* bytes is not a cap on the decoded pixels: a
/// few hundred KiB of PNG can expand to gigabytes. 8192 covers every real
/// cover (the biggest in the wild are 3000×3000) and is the guard against a
/// decompression bomb.
pub const MAX_ART_DIMENSION: u32 = 8192;

/// Decode-time allocation ceiling handed to the `image` crate, a second
/// backstop behind [`MAX_ART_DIMENSION`] for formats whose dimensions are only
/// known part-way through decoding.
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

/// Largest disc [`render_disc`] will produce, in pixels per side.
///
/// The buffer is `size²·4` bytes, so this bounds one disc at 16 MiB. A disc on
/// a 4K display is ~600px; 2048 is headroom, not a target.
pub const MAX_DISC_PX: u32 = 2048;

/// Connect timeout for artwork fetches. Short on purpose: artwork is
/// decoration, and a dead host must not hold a worker thread.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Per-read timeout for artwork fetches.
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Whole-call timeout for artwork fetches — the guard against a server that
/// dribbles one byte per read forever, which the per-read timeout never trips.
const HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Turntable speed of a 12" LP, in revolutions per minute — the natural
/// default for [`rotation_for`]. `100/3` rather than a decimal literal so the
/// value is exactly the intended 33⅓.
pub const VINYL_RPM: f32 = 100.0 / 3.0;

/// Suggested `min_step_deg` for [`should_redraw`].
///
/// At [`VINYL_RPM`] the disc sweeps 200°/s, so this never gates normal
/// playback — the frame callback does. What it gates is the ease-out after a
/// pause: once the disc is creeping slower than half a degree per frame (≈0.9
/// px of movement at the rim of a 320px disc) the animation is finished as far
/// as an eye is concerned, and the redraws stop.
pub const DEFAULT_MIN_STEP_DEG: f32 = 0.5;

// ---------------------------------------------------------------------------
// Source resolution (pure)
// ---------------------------------------------------------------------------

/// Where a piece of cover art can be read from.
///
/// The three variants are exactly the three `mpris:artUrl` shapes players
/// actually publish. All three fail routinely and none of those failures may
/// break the widget: `Http` needs a network, and `File` frequently names a
/// path inside *another application's* sandbox or a `/tmp` file that has
/// already been unlinked (the long-standing Firefox-Flatpak behaviour).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtSource {
    /// An `http://` or `https://` URL, kept verbatim for the fetch.
    Http(String),
    /// A local path, already percent-decoded.
    File(PathBuf),
    /// Bytes carried inline in a `data:` URI.
    Data {
        /// The declared media type, lowercased (`"image/png"`). Empty when the
        /// URI omitted it. Informational only — [`decode_art`] sniffs the real
        /// format from the bytes, because players do mislabel this.
        mime: String,
        /// The decoded payload, never empty.
        bytes: Vec<u8>,
    },
}

/// Classify an `mpris:artUrl`.
///
/// Handles `http(s)://`, `file://` and `data:`, plus the bare absolute path
/// some players publish in violation of the spec. Anything else — an unknown
/// scheme, a relative path, a `file://` on a remote host we could not read
/// anyway, an empty payload — is `None`. Returning `None` is always safe: the
/// caller falls back to [`placeholder_art`].
///
/// `file://` paths are percent-decoded into raw bytes and only then turned
/// into a [`PathBuf`], because a Linux path is a byte string and lossy UTF-8
/// conversion would corrupt filenames that are not valid UTF-8.
///
/// One deliberate deviation from RFC 3986: a `#` in a `file://` URL is **not**
/// treated as a fragment. Fragments are meaningless on a local artwork file,
/// whereas a song file literally named `Nu#1.png` is not, and players that
/// forget to percent-encode are common.
///
/// ```text
/// parse_art_url("file:///home/u/a%20b.png") == Some(ArtSource::File("/home/u/a b.png"))
/// ```
pub fn parse_art_url(url: &str) -> Option<ArtSource> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    // Not a URI at all, but an absolute path: some players publish one.
    if url.starts_with('/') {
        return Some(ArtSource::File(path_from_bytes(percent_decode(url))));
    }
    let (scheme, rest) = url.split_once(':')?;
    // Schemes are ASCII case-insensitive (RFC 3986 §3.1) and players do vary.
    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        let after = rest.strip_prefix("//")?;
        // Require a host; `http:///x` and `http://` are not fetchable.
        let host_end = after.find(['/', '?', '#']).unwrap_or(after.len());
        if after[..host_end].is_empty() {
            return None;
        }
        return Some(ArtSource::Http(url.to_string()));
    }
    if scheme.eq_ignore_ascii_case("file") {
        return parse_file_url(rest).map(ArtSource::File);
    }
    if scheme.eq_ignore_ascii_case("data") {
        return parse_data_url(rest);
    }
    None
}

/// The path out of everything after `file:`.
///
/// Accepts `file:///p` (empty authority), `file://localhost/p`, and the
/// authority-less `file:/p` that a few players emit. A non-local authority is
/// rejected rather than guessed at: we cannot read another machine's disk, and
/// silently reinterpreting `file://nas/art.png` as `/art.png` would open a
/// wrong local file.
fn parse_file_url(rest: &str) -> Option<PathBuf> {
    let path_part = match rest.strip_prefix("//") {
        Some(after) => {
            let (host, path) = match after.find('/') {
                Some(i) => (&after[..i], &after[i..]),
                None => (after, ""),
            };
            if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
                return None;
            }
            path
        }
        None => rest,
    };
    let bytes = percent_decode(path_part);
    // Must be absolute: a relative artUrl has no defined base directory.
    if bytes.first() != Some(&b'/') {
        return None;
    }
    Some(path_from_bytes(bytes))
}

/// Everything after `data:`, i.e. `` `[<mediatype>][;base64],<payload>` ``.
///
/// Both payload encodings are handled: `;base64` and, when it is absent, plain
/// percent-encoding (RFC 2397 allows either, and small SVG/PNG art does turn
/// up unencoded). An empty payload yields `None` — it can never decode to an
/// image, and failing here is cheaper than failing three steps later.
fn parse_data_url(rest: &str) -> Option<ArtSource> {
    let (params, payload) = rest.split_once(',')?;
    let mut mime = String::new();
    let mut is_base64 = false;
    for (i, tok) in params.split(';').enumerate() {
        let tok = tok.trim();
        if tok.eq_ignore_ascii_case("base64") {
            is_base64 = true;
        } else if i == 0 && tok.contains('/') {
            // Only the first token can be the media type; the rest are
            // attributes like `charset=utf-8`.
            mime = tok.to_ascii_lowercase();
        }
    }
    let bytes = if is_base64 {
        base64_decode(payload)?
    } else {
        percent_decode(payload)
    };
    if bytes.is_empty() {
        return None;
    }
    Some(ArtSource::Data { mime, bytes })
}

/// Raw bytes into a path. Linux paths are byte strings, not UTF-8, so this
/// must not go through [`String`].
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(OsString::from_vec(bytes))
}

/// Percent-decode to bytes, leniently.
///
/// A `%` not followed by two hex digits is passed through literally rather
/// than rejected. That is the right call for artwork: players that forget to
/// encode a `%` in a filename are far more common than players that mean
/// something by a malformed escape, and a decoder that fails here would lose
/// art it could have opened.
fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(hi), Some(lo)) = (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// One hex digit's value.
fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Decode base64, accepting both the standard (`+/`) and URL-safe (`-_`)
/// alphabets.
///
/// Hand-written rather than pulled in as a dependency: this is thirty lines
/// against a crate, and the only thing that ever reaches it is a `data:` URI.
///
/// Lenient where leniency is harmless — embedded whitespace is skipped (some
/// producers wrap at 76 columns) and missing trailing `=` padding is accepted.
/// Strict where it is not: any character outside the alphabet, any data after
/// the padding, more than two `=`, or a group of only 6 leftover bits (which
/// cannot encode a byte, so the input was truncated) all yield `None`.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3 + 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut pad: usize = 0;
    for &c in s.as_bytes() {
        if c.is_ascii_whitespace() {
            continue;
        }
        if c == b'=' {
            pad += 1;
            continue;
        }
        if pad > 0 {
            return None; // payload resumed after the padding
        }
        acc = (acc << 6) | u32::from(b64_val(c)?);
        bits += 6;
        if bits == 24 {
            out.push((acc >> 16) as u8);
            out.push((acc >> 8) as u8);
            out.push(acc as u8);
            acc = 0;
            bits = 0;
        }
    }
    if pad > 2 {
        return None;
    }
    match bits {
        // A complete final group; padding here would be spurious.
        0 if pad == 0 => {}
        // 6 bits is less than one byte: the input was cut mid-character.
        12 => out.push((acc >> 4) as u8),
        18 => {
            out.push((acc >> 10) as u8);
            out.push((acc >> 2) as u8);
        }
        _ => return None,
    }
    Some(out)
}

/// One base64 character's value, standard or URL-safe alphabet.
fn b64_val(c: u8) -> Option<u8> {
    Some(match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' | b'-' => 62,
        b'/' | b'_' => 63,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// I/O — the only part of this module that touches the outside world
// ---------------------------------------------------------------------------

/// Read the artwork bytes for `src`, never more than [`MAX_ART_BYTES`].
///
/// **Blocking.** `Http` can take up to ten seconds (3s to connect, 5s per
/// read, 10s overall), so this belongs on a worker thread and never on the
/// daemon tick — the same rule `mpris.rs` states for its queries.
///
/// Every failure is expected and non-fatal: the file is gone, it lives in
/// another app's sandbox, the host is down, the art is 40 MiB of nonsense. The
/// caller logs at debug and shows [`placeholder_art`].
pub fn load_bytes(src: &ArtSource) -> anyhow::Result<Vec<u8>> {
    match src {
        ArtSource::Data { bytes, .. } => {
            if bytes.len() as u64 > MAX_ART_BYTES {
                bail!(
                    "inline artwork is {} bytes, over the {MAX_ART_BYTES} byte cap",
                    bytes.len()
                );
            }
            Ok(bytes.clone())
        }
        ArtSource::File(path) => read_file_capped(path),
        ArtSource::Http(url) => fetch_http_capped(url),
    }
}

/// A local artwork file, size-checked before and during the read.
fn read_file_capped(path: &Path) -> anyhow::Result<Vec<u8>> {
    let meta = std::fs::metadata(path)
        .with_context(|| format!("cannot stat artwork {}", path.display()))?;
    if !meta.is_file() {
        bail!("artwork {} is not a regular file", path.display());
    }
    // Cheap pre-check; `read_capped` still guards, since /proc-like files and
    // races can report a size that is not the size we get.
    if meta.len() > MAX_ART_BYTES {
        bail!(
            "artwork {} is {} bytes, over the {MAX_ART_BYTES} byte cap",
            path.display(),
            meta.len()
        );
    }
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open artwork {}", path.display()))?;
    read_capped(file).with_context(|| format!("reading artwork {}", path.display()))
}

/// Remote artwork, with short timeouts and the same cap.
fn fetch_http_capped(url: &str) -> anyhow::Result<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(HTTP_READ_TIMEOUT)
        .timeout(HTTP_TOTAL_TIMEOUT)
        .build();
    let resp = agent
        .get(url)
        .call()
        .with_context(|| format!("fetching artwork {url}"))?;
    // Honest servers let us bail before transferring anything.
    if let Some(len) = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
    {
        if len > MAX_ART_BYTES {
            bail!("artwork {url} declares {len} bytes, over the {MAX_ART_BYTES} byte cap");
        }
    }
    read_capped(resp.into_reader()).with_context(|| format!("reading artwork {url}"))
}

/// Read to end, refusing anything over [`MAX_ART_BYTES`].
///
/// Reads one byte past the cap so "exactly at the cap" and "over the cap" are
/// distinguishable without trusting a declared length.
fn read_capped(source: impl Read) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    source.take(MAX_ART_BYTES + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_ART_BYTES {
        bail!("artwork exceeds the {MAX_ART_BYTES} byte cap");
    }
    Ok(buf)
}

/// Decode artwork bytes to RGBA, with decompression-bomb limits applied.
///
/// The format is **sniffed from the bytes**, not taken from a `data:` URI's
/// media type or a URL extension, because both are routinely wrong. Only PNG,
/// JPEG and WebP are compiled in — the three formats MPRIS players actually
/// serve — so anything else (notably SVG, which Firefox has been seen to
/// publish) fails here, cleanly.
pub fn decode_art(bytes: &[u8]) -> anyhow::Result<RgbaImage> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ART_DIMENSION);
    limits.max_image_height = Some(MAX_ART_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);

    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .context("sniffing the artwork format")?;
    reader.limits(limits);
    Ok(reader.decode().context("decoding artwork")?.to_rgba8())
}

/// Shrink oversized art to what a `size_px` disc can actually show — **once
/// per track**, not per frame.
///
/// [`render_disc`] resamples the source on every frame, and because the
/// rotated sampler walks the source diagonally it has poor cache behaviour on
/// a large image. Measured here on a 3000×3000 cover (the largest size that
/// turns up in the wild): a 320px disc costs about **4×** more per frame than
/// the same disc rendered from an already-small source, essentially all of it
/// cache misses. Paying that once per track instead of 30 times a second is
/// the whole point of this function.
///
/// The target is twice the disc's size, so the bilinear sampler still has
/// detail to draw on at every rotation. Art that is already small enough is
/// returned borrowed, with no copy and no work.
pub fn prepare_source(art: &RgbaImage, size_px: u32) -> std::borrow::Cow<'_, RgbaImage> {
    let target = size_px.clamp(1, MAX_DISC_PX).saturating_mul(2);
    let longest = art.width().max(art.height());
    if longest <= target || art.width() == 0 || art.height() == 0 {
        return std::borrow::Cow::Borrowed(art);
    }
    let k = f64::from(target) / f64::from(longest);
    let w = ((f64::from(art.width()) * k).round() as u32).max(1);
    let h = ((f64::from(art.height()) * k).round() as u32).max(1);
    std::borrow::Cow::Owned(image::imageops::resize(
        art,
        w,
        h,
        // Triangle, not Nearest: this runs once, and a nearest-neighbour
        // downscale of a 10x reduction aliases badly enough to see.
        image::imageops::FilterType::Triangle,
    ))
}

/// A generated stand-in for missing or unreadable art (W4: "a failed art load
/// must never break the widget; fall back to a generated placeholder").
///
/// Deliberately plain — a soft vertical gradient with a lifted centre, so once
/// [`render_disc`] has masked and ringed it, it reads as an unlabelled record
/// rather than as a broken image. `size` is clamped to `8..=512`; there is no
/// reason to generate a placeholder larger than the disc that consumes it.
pub fn placeholder_art(size: u32) -> RgbaImage {
    let size = size.clamp(8, 512);
    let c = size as f32 / 2.0;
    let inv = 1.0 / c;
    RgbaImage::from_fn(size, size, |x, y| {
        let dx = (x as f32 + 0.5 - c) * inv;
        let dy = (y as f32 + 0.5 - c) * inv;
        // Radial lift towards the middle, plus a slight top-to-bottom fade.
        let lift = (1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
        let fade = 1.0 - 0.25 * (y as f32 / size as f32);
        let v = |base: f32, span: f32| ((base + span * lift) * fade).clamp(0.0, 255.0) as u8;
        image::Rgba([v(28.0, 34.0), v(28.0, 36.0), v(36.0, 44.0), 255])
    })
}

// ---------------------------------------------------------------------------
// Disc rendering (pure)
// ---------------------------------------------------------------------------

/// Where the darkened outer ring starts, as a fraction of the disc radius.
/// 0.80 leaves a rim about a tenth of the diameter wide — enough to read as
/// vinyl, not enough to eat the artwork.
const RING_START: f32 = 0.80;

/// How much brighter the label area is than the surrounding artwork. Small:
/// the point is to suggest a paper label, not to wash the art out.
const LABEL_LIGHTEN: f32 = 0.10;

/// Darkening of the thin rim drawn at the label boundary. This one line is
/// most of what makes the thing read as a record rather than a circle crop.
const LABEL_RIM_DARKEN: f32 = 0.45;

/// Label rim thickness, in disc pixels per 200px of disc — i.e. ~1.6px on a
/// 320px disc. Scaled so the rim does not vanish on a small disc or turn into
/// a band on a large one.
const LABEL_RIM_PER_200: f32 = 1.0;

/// Everything about how one disc looks. Every field is sanitised on use
/// (non-finite values become 0, ratios clamp to `0.0..=1.0`, `size_px` clamps
/// to `1..=MAX_DISC_PX`), so no combination of settings can panic or produce
/// an unusable buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscCfg {
    /// Side of the square output, in pixels. Clamped to `1..=MAX_DISC_PX`.
    pub size_px: u32,
    /// Clockwise rotation, in degrees. Any value is accepted; see
    /// [`rotation_for`] for the playback-driven source of it.
    pub rotation_deg: f32,
    /// Label radius as a fraction of the disc radius. 0 disables the label.
    pub label_ratio: f32,
    /// Spindle-hole radius as a fraction of the disc radius. 0 disables the
    /// hole. The hole is punched out of the alpha channel, not painted.
    pub hole_ratio: f32,
    /// How much to darken the outer ring, `0.0` (off) to `1.0` (black rim).
    pub ring_darken: f32,
    /// Overall opacity applied to the finished disc, `0` to `255`.
    pub opacity: u8,
}

impl Default for DiscCfg {
    fn default() -> Self {
        Self {
            size_px: 320,
            rotation_deg: 0.0,
            // Roughly the proportions of a 7" label on a 12" record.
            label_ratio: 0.33,
            hole_ratio: 0.045,
            ring_darken: 0.35,
            opacity: 255,
        }
    }
}

/// A tightly packed BGRA image with **premultiplied alpha**, ready for mpv's
/// `overlay-add` (see the module docs for the exact convention and why it
/// matters).
#[derive(Clone, PartialEq, Eq)]
pub struct Bgra {
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
    /// `w * h * 4` bytes, row-major, B G R A per pixel.
    pub data: Vec<u8>,
}

impl Bgra {
    /// Bytes per row — the `stride` argument of `overlay-add`. Always `w * 4`,
    /// because there is no padding.
    pub fn stride(&self) -> u32 {
        self.w.saturating_mul(4)
    }

    /// Whether there are no pixels at all.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Prints the geometry, never the pixels — a `derive` here would dump megabytes
/// into a log line.
impl std::fmt::Debug for Bgra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bgra")
            .field("w", &self.w)
            .field("h", &self.h)
            .field("bytes", &self.data.len())
            .finish()
    }
}

/// Render `art` as a rotating disc.
///
/// The output is always `size_px × size_px` and fully transparent outside the
/// disc, so the caller can place it with a single `overlay-add` and never
/// think about geometry again.
///
/// # How
///
/// 1. **Centre-square crop, scale to fit.** Non-square art is cropped, not
///    squashed; a stretched cover looks broken in a way a cropped one does not.
/// 2. **Inverse mapping.** For every *destination* pixel we compute where it
///    came from in the source and sample there. Forward mapping (walk the
///    source, paint where each pixel lands) is the classic mistake: rotation
///    is not area-preserving in a raster, so it leaves a lattice of unpainted
///    holes. Inverse mapping cannot, by construction — every destination pixel
///    is written exactly once.
/// 3. **Bilinear sampling**, on premultiplied values so transparent source
///    pixels cannot bleed colour into their neighbours.
/// 4. **Analytic anti-aliasing.** Coverage is `clamp(R - r + 0.5, 0, 1)`, the
///    linear approximation of how much of a pixel the circle covers, which is
///    exact enough at these radii and far cheaper than supersampling — no
///    extra samples at all, one `sqrt` per pixel. The same expression, negated,
///    punches the spindle hole.
/// 5. **Ring, label, hole**, then the global opacity, then premultiplied BGRA
///    with each colour byte clamped to the alpha byte.
///
/// Positive `rotation_deg` turns the disc **clockwise** on screen (image
/// coordinates put `+y` downwards, so the inverse map rotates by `-θ`).
///
/// Degenerate input is handled rather than rejected: a zero-sized source
/// yields a fully transparent buffer of the right size, and extreme aspect
/// ratios are cropped like any other.
///
/// `art` is resampled on **every** call, so a caller animating the disc should
/// put the art through [`prepare_source`] once per track rather than handing a
/// 3000×3000 cover to every frame.
pub fn render_disc(art: &RgbaImage, cfg: &DiscCfg) -> Bgra {
    let size = cfg.size_px.clamp(1, MAX_DISC_PX);
    let px_count = size as usize * size as usize;
    let mut data = vec![0u8; px_count * 4];

    let (sw, sh) = (art.width(), art.height());
    if sw == 0 || sh == 0 {
        // Nothing to sample. A transparent disc is a correct disc.
        return Bgra {
            w: size,
            h: size,
            data,
        };
    }

    let sizef = size as f32;
    let centre = sizef / 2.0;
    let radius = centre;
    let inv_radius = 1.0 / radius;

    let rot = if cfg.rotation_deg.is_finite() {
        cfg.rotation_deg
    } else {
        0.0
    };
    let (sin_t, cos_t) = rot.to_radians().sin_cos();

    let label_r = ratio01(cfg.label_ratio) * radius;
    let hole_r = ratio01(cfg.hole_ratio) * radius;
    let ring_darken = ratio01(cfg.ring_darken);
    let opacity = f32::from(cfg.opacity) / 255.0;
    let rim_half = (sizef * (LABEL_RIM_PER_200 / 200.0)).max(1.0) * 0.5;

    // Sampled through the packed buffer rather than `get_pixel`; see
    // `sample_bilinear_premul`.
    let raw: &[u8] = art.as_raw();
    let sw_i = sw as i32;
    let sh_i = sh as i32;

    // Centre-square crop of the source, in source pixels.
    let side = sw.min(sh) as f32;
    let src_x0 = (sw as f32 - side) * 0.5;
    let src_y0 = (sh as f32 - side) * 0.5;
    let src_mid_x = src_x0 + side * 0.5;
    let src_mid_y = src_y0 + side * 0.5;
    // Source pixels per destination pixel.
    let scale = side / sizef;

    // The outer edge fades out by `radius + 0.5`; nothing beyond that is ever
    // written, so rows and columns outside it are skipped entirely rather than
    // computed and thrown away.
    let reach = radius + 0.5;

    let row_bytes = size as usize * 4;
    for (dy, row) in data.chunks_exact_mut(row_bytes).enumerate() {
        let py = dy as f32 + 0.5 - centre;
        let span_sq = reach * reach - py * py;
        if span_sq <= 0.0 {
            continue;
        }
        let span = span_sq.sqrt();
        let x_lo = (centre - span - 0.5).floor().max(0.0) as u32;
        let x_hi = ((centre + span + 0.5).ceil().max(0.0) as u32).min(size);

        for dx in x_lo..x_hi {
            let px = dx as f32 + 0.5 - centre;
            let r = (px * px + py * py).sqrt();

            // Outer edge coverage, and the hole punched back out of it.
            let mut cov = (radius - r + 0.5).clamp(0.0, 1.0);
            if hole_r > 0.0 {
                cov *= (r - hole_r + 0.5).clamp(0.0, 1.0);
            }
            cov *= opacity;
            if cov <= 0.0 {
                continue;
            }

            // Inverse rotation: where in the source did this pixel come from?
            let sx = px * cos_t + py * sin_t;
            let sy = -px * sin_t + py * cos_t;
            let fx = src_mid_x + sx * scale;
            let fy = src_mid_y + sy * scale;
            // Premultiplied, components in 0..=1.
            let [mut cr, mut cg, mut cb, ca] = sample_bilinear_premul(raw, sw_i, sh_i, fx, fy);

            if ring_darken > 0.0 {
                let rn = r * inv_radius;
                if rn > RING_START {
                    let t = ((rn - RING_START) / (1.0 - RING_START)).clamp(0.0, 1.0);
                    // Quadratic ramp: imperceptible where it starts, decisive
                    // at the rim. A linear one reads as a grey halo.
                    let k = 1.0 - ring_darken * t * t;
                    cr *= k;
                    cg *= k;
                    cb *= k;
                }
            }

            if label_r > 0.0 {
                let inside = (label_r - r + 0.5).clamp(0.0, 1.0);
                if inside > 0.0 {
                    let k = 1.0 + LABEL_LIGHTEN * inside;
                    // Lightening premultiplied colour can push it past alpha,
                    // which mpv forbids outright.
                    cr = (cr * k).min(ca);
                    cg = (cg * k).min(ca);
                    cb = (cb * k).min(ca);
                }
                let rim = (rim_half + 0.5 - (r - label_r).abs()).clamp(0.0, 1.0);
                if rim > 0.0 {
                    let k = 1.0 - LABEL_RIM_DARKEN * rim;
                    cr *= k;
                    cg *= k;
                    cb *= k;
                }
            }

            let a8 = q8(ca * cov);
            let i = dx as usize * 4;
            let Some(out) = row.get_mut(i..i + 4) else {
                continue;
            };
            // B, G, R, A — and never a colour above alpha (mpv: "the numeric
            // value of each component is equal to or smaller than the alpha
            // component"). Independent rounding can put a colour one step
            // over, so clamp after rounding rather than before.
            out[0] = q8(cb * cov).min(a8);
            out[1] = q8(cg * cov).min(a8);
            out[2] = q8(cr * cov).min(a8);
            out[3] = a8;
        }
    }

    Bgra {
        w: size,
        h: size,
        data,
    }
}

/// Sanitise a `0.0..=1.0` configuration ratio; non-finite becomes 0.
fn ratio01(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A `0.0..=1.0` component to a byte, rounded.
fn q8(v: f32) -> u8 {
    (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

/// Bilinear sample at continuous coordinate `(fx, fy)`, returning
/// **premultiplied** `[r, g, b, a]` in `0.0..=1.0`.
///
/// Two details that are easy to get wrong and invisible until they are not:
///
/// * **Pixel centres are at integer + 0.5.** Sampling without the half-pixel
///   shift skews the whole image by half a pixel, which shows up as a
///   shimmering edge once the disc rotates.
/// * **Premultiply before interpolating.** With straight alpha, a fully
///   transparent source pixel still carries some RGB value (usually black, or
///   whatever the encoder left there), and interpolation drags it into the
///   visible neighbour — the classic black fringe around a masked image.
///
/// Coordinates outside the source clamp to the edge pixel. The disc is
/// inscribed in the crop square so this is only ever reached through
/// floating-point slack at the rim, but "only ever" is not "never".
/// This runs once per disc pixel — ~80k times per frame at the default size —
/// so it reads `raw` (the source's packed RGBA bytes) directly rather than
/// going through `get_pixel`, which costs four bounds-checked index
/// computations per sample.
fn sample_bilinear_premul(raw: &[u8], w: i32, h: i32, fx: f32, fy: f32) -> [f32; 4] {
    let gx = fx - 0.5;
    let gy = fy - 0.5;
    let fx0 = gx.floor();
    let fy0 = gy.floor();
    let tx = gx - fx0;
    let ty = gy - fy0;
    // f32 -> i32 casts saturate in Rust, so even a wild coordinate lands at
    // i32::MIN/MAX and is then clamped into range.
    let ix = fx0 as i32;
    let iy = fy0 as i32;

    let x0 = ix.clamp(0, w - 1) as usize * 4;
    let x1 = (ix + 1).clamp(0, w - 1) as usize * 4;
    let row0 = iy.clamp(0, h - 1) as usize * w as usize * 4;
    let row1 = (iy + 1).clamp(0, h - 1) as usize * w as usize * 4;

    let top = lerp4(tap(raw, row0 + x0), tap(raw, row0 + x1), tx);
    let bot = lerp4(tap(raw, row1 + x0), tap(raw, row1 + x1), tx);
    lerp4(top, bot, ty)
}

/// One packed RGBA pixel at byte offset `i` as premultiplied `0.0..=1.0`
/// components. An out-of-range offset reads as transparent; the callers clamp
/// their indices, so this is belt and braces against a future change rather
/// than a reachable path.
fn tap(raw: &[u8], i: usize) -> [f32; 4] {
    let p = raw.get(i..i + 4).unwrap_or(&[0, 0, 0, 0]);
    let a = f32::from(p[3]) * (1.0 / 255.0);
    let k = a * (1.0 / 255.0);
    [
        f32::from(p[0]) * k,
        f32::from(p[1]) * k,
        f32::from(p[2]) * k,
        a,
    ]
}

/// Component-wise linear interpolation.
fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

// ---------------------------------------------------------------------------
// Rotation clock and the redraw gate (pure)
// ---------------------------------------------------------------------------

/// The disc's angle after `elapsed` of playback at `rpm`, wrapped to
/// `0.0..360.0`.
///
/// `elapsed` is *playing* time, not wall time — the caller freezes it while
/// paused, which is what makes rule 6 of the power model ("paused → rotation
/// speed 0 → no redraw") hold without any special case here.
///
/// Accumulates in `f64` and wraps before narrowing, so an eight-hour session
/// is as precise as the first second; wrapping an `f32` that has grown to
/// ~10⁷ degrees would quantise the angle into visible steps.
///
/// A non-finite `rpm` yields `0.0`. A negative one spins the disc backwards,
/// which is a legitimate thing to ask for and wraps correctly.
pub fn rotation_for(elapsed: Duration, rpm: f32) -> f32 {
    if !rpm.is_finite() {
        return 0.0;
    }
    // rev/min → deg/s is ×360/60 = ×6.
    let deg = elapsed.as_secs_f64() * f64::from(rpm) * 6.0;
    if !deg.is_finite() {
        return 0.0;
    }
    deg.rem_euclid(360.0) as f32
}

/// Whether a rotation from `last_deg` to `next_deg` is worth redrawing.
///
/// Compares the **shortest** angular distance, so wrapping past 360° is not
/// mistaken for a 359° jump, and returns true at or above `min_step_deg`.
///
/// This is the power model's "never redraw unless content changed" applied to
/// a continuously varying value: without it, an ease-out that asymptotically
/// approaches its final angle keeps issuing redraws forever, each one moving
/// the picture by less than a pixel.
///
/// Errs towards redrawing: a non-finite input, or a `min_step_deg` of zero or
/// less, returns true. A dropped frame is a visual bug; an extra one is a few
/// microseconds.
pub fn should_redraw(last_deg: f32, next_deg: f32, min_step_deg: f32) -> bool {
    if !last_deg.is_finite() || !next_deg.is_finite() {
        return true;
    }
    if !min_step_deg.is_finite() || min_step_deg <= 0.0 {
        return true;
    }
    let d = (next_deg - last_deg).rem_euclid(360.0);
    let delta = d.min(360.0 - d);
    delta >= min_step_deg
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    // -- helpers ------------------------------------------------------------

    /// An asymmetric test image: four solid quadrants, so any rotation is
    /// visible and unambiguous. Red is top-left, green top-right, blue
    /// bottom-left, white bottom-right.
    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];

    fn quadrants(size: u32) -> RgbaImage {
        RgbaImage::from_fn(size, size, |x, y| {
            let half = size / 2;
            Rgba(match (x < half, y < half) {
                (true, true) => RED,
                (false, true) => GREEN,
                (true, false) => BLUE,
                (false, false) => WHITE,
            })
        })
    }

    fn solid(size: u32, rgba: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(size, size, Rgba(rgba))
    }

    /// `(b, g, r, a)` at a pixel.
    fn px(img: &Bgra, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let i = (y as usize * img.w as usize + x as usize) * 4;
        (
            img.data[i],
            img.data[i + 1],
            img.data[i + 2],
            img.data[i + 3],
        )
    }

    /// A config with every decoration off, so tests see the rotation and
    /// nothing else.
    fn plain(size: u32, rotation_deg: f32) -> DiscCfg {
        DiscCfg {
            size_px: size,
            rotation_deg,
            label_ratio: 0.0,
            hole_ratio: 0.0,
            ring_darken: 0.0,
            opacity: 255,
        }
    }

    /// Which of the four quadrant colours a BGRA pixel is closest to, or
    /// `None` when it is not close to any (an edge or blend pixel).
    fn classify(p: (u8, u8, u8, u8)) -> Option<&'static str> {
        let (b, g, r, _) = p;
        let near = |v: u8, want: u8| (i32::from(v) - i32::from(want)).abs() <= 24;
        if near(r, 255) && near(g, 0) && near(b, 0) {
            Some("red")
        } else if near(r, 0) && near(g, 255) && near(b, 0) {
            Some("green")
        } else if near(r, 0) && near(g, 0) && near(b, 255) {
            Some("blue")
        } else if near(r, 255) && near(g, 255) && near(b, 255) {
            Some("white")
        } else {
            None
        }
    }

    // -- parse_art_url ------------------------------------------------------

    #[test]
    fn parses_http_and_https() {
        assert_eq!(
            parse_art_url("http://i.example.com/a.jpg"),
            Some(ArtSource::Http("http://i.example.com/a.jpg".into()))
        );
        assert_eq!(
            parse_art_url("https://i.scdn.co/image/ab67616d00001e02"),
            Some(ArtSource::Http(
                "https://i.scdn.co/image/ab67616d00001e02".into()
            ))
        );
        // Scheme case is not significant, and surrounding whitespace is not
        // part of the URL.
        assert_eq!(
            parse_art_url("  HTTPS://Example.COM/x.png  "),
            Some(ArtSource::Http("HTTPS://Example.COM/x.png".into()))
        );
        // Query strings survive intact — CDNs sign artwork URLs.
        assert_eq!(
            parse_art_url("https://cdn.example/a.jpg?sig=1&t=2"),
            Some(ArtSource::Http(
                "https://cdn.example/a.jpg?sig=1&t=2".into()
            ))
        );
    }

    #[test]
    fn rejects_http_without_a_host() {
        for bad in ["http://", "https://", "http:///path", "http:/example.com"] {
            assert_eq!(parse_art_url(bad), None, "input {bad}");
        }
    }

    #[test]
    fn parses_file_url_with_percent_encoded_space() {
        assert_eq!(
            parse_art_url("file:///home/u/My%20Music/Cover%20Art.png"),
            Some(ArtSource::File(PathBuf::from(
                "/home/u/My Music/Cover Art.png"
            )))
        );
    }

    #[test]
    fn parses_file_url_with_non_ascii() {
        // Percent-encoded UTF-8, which is what a correct player emits.
        assert_eq!(
            parse_art_url("file:///music/%E6%97%A5%E6%9C%AC%E8%AA%9E/%C3%A9.jpg"),
            Some(ArtSource::File(PathBuf::from("/music/日本語/é.jpg")))
        );
        // Raw UTF-8, which is what a sloppy one emits. Both must work.
        assert_eq!(
            parse_art_url("file:///music/日本語/é.jpg"),
            Some(ArtSource::File(PathBuf::from("/music/日本語/é.jpg")))
        );
    }

    #[test]
    fn file_url_authority_forms() {
        let want = Some(ArtSource::File(PathBuf::from("/tmp/a.png")));
        // Empty authority, `localhost`, and the authority-less sloppy form.
        assert_eq!(parse_art_url("file:///tmp/a.png"), want);
        assert_eq!(parse_art_url("file://localhost/tmp/a.png"), want);
        assert_eq!(parse_art_url("file://LOCALHOST/tmp/a.png"), want);
        assert_eq!(parse_art_url("file:/tmp/a.png"), want);
        // A remote host is not something we can read, and must not be
        // silently reinterpreted as a local path.
        assert_eq!(parse_art_url("file://nas.local/tmp/a.png"), None);
    }

    #[test]
    fn file_url_must_be_absolute_and_non_empty() {
        for bad in ["file://", "file:", "file:relative/a.png", "file://host"] {
            assert_eq!(parse_art_url(bad), None, "input {bad}");
        }
    }

    #[test]
    fn a_bare_absolute_path_is_accepted() {
        // Not spec-compliant, but published in the wild.
        assert_eq!(
            parse_art_url("/home/u/cover.jpg"),
            Some(ArtSource::File(PathBuf::from("/home/u/cover.jpg")))
        );
    }

    #[test]
    fn a_hash_in_a_file_url_is_part_of_the_filename() {
        // Deliberate deviation from RFC 3986: local artwork has no fragments,
        // but filenames with `#` exist.
        assert_eq!(
            parse_art_url("file:///m/Nu%231.png"),
            Some(ArtSource::File(PathBuf::from("/m/Nu#1.png")))
        );
        assert_eq!(
            parse_art_url("file:///m/Nu#1.png"),
            Some(ArtSource::File(PathBuf::from("/m/Nu#1.png")))
        );
    }

    #[test]
    fn parses_data_url_base64() {
        // A 1x1 PNG, base64 — the shape browsers publish.
        let uri = "data:image/png;base64,\
                   iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let Some(ArtSource::Data { mime, bytes }) = parse_art_url(uri) else {
            panic!("expected a Data source");
        };
        assert_eq!(mime, "image/png");
        // PNG magic — proof the base64 decoder produced real bytes.
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        // And it must actually decode as an image.
        let img = decode_art(&bytes).expect("1x1 PNG must decode");
        assert_eq!((img.width(), img.height()), (1, 1));
    }

    #[test]
    fn parses_data_url_without_base64() {
        // Percent-encoded payload: RFC 2397 allows it and it does turn up.
        let Some(ArtSource::Data { mime, bytes }) = parse_art_url("data:image/png,%89PNG%0d%0a hi")
        else {
            panic!("expected a Data source");
        };
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, b"\x89PNG\r\n hi");
    }

    #[test]
    fn data_url_parameter_forms() {
        // Extra attributes around the base64 marker.
        let Some(ArtSource::Data { mime, bytes }) =
            parse_art_url("data:image/jpeg;charset=binary;base64,QUJD")
        else {
            panic!("expected a Data source");
        };
        assert_eq!(mime, "image/jpeg");
        assert_eq!(bytes, b"ABC");
        // Media type omitted entirely.
        let Some(ArtSource::Data { mime, bytes }) = parse_art_url("data:;base64,QUJD") else {
            panic!("expected a Data source");
        };
        assert_eq!(mime, "");
        assert_eq!(bytes, b"ABC");
        // Media type is lowercased.
        let Some(ArtSource::Data { mime, .. }) = parse_art_url("data:IMAGE/PNG;base64,QUJD") else {
            panic!("expected a Data source");
        };
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn rejects_malformed_urls() {
        for bad in [
            "",
            "   ",
            "not a url",
            "cover.jpg",
            "../cover.jpg",
            "ftp://example.com/a.png",
            "smb://server/share/a.png",
            "data:",                       // no comma
            "data:image/png;base64",       // no comma
            "data:image/png;base64,",      // empty payload
            "data:,",                      // empty payload
            "data:image/png;base64,QU*JD", // invalid base64 character
        ] {
            assert_eq!(parse_art_url(bad), None, "input {bad:?}");
        }
    }

    // -- base64 -------------------------------------------------------------

    #[test]
    fn base64_known_vectors() {
        // RFC 4648 §10 test vectors.
        for (enc, dec) in [
            ("", ""),
            ("Zg==", "f"),
            ("Zm8=", "fo"),
            ("Zm9v", "foo"),
            ("Zm9vYg==", "foob"),
            ("Zm9vYmE=", "fooba"),
            ("Zm9vYmFy", "foobar"),
        ] {
            assert_eq!(
                base64_decode(enc).as_deref(),
                Some(dec.as_bytes()),
                "input {enc:?}"
            );
        }
    }

    #[test]
    fn base64_padding_and_whitespace_variants() {
        // Missing padding is accepted — some producers omit it.
        assert_eq!(base64_decode("Zg").as_deref(), Some(&b"f"[..]));
        assert_eq!(base64_decode("Zm8").as_deref(), Some(&b"fo"[..]));
        // Line wrapping is accepted.
        assert_eq!(
            base64_decode("Zm9v\nYmFy\r\n").as_deref(),
            Some(&b"foobar"[..])
        );
        assert_eq!(base64_decode("Zm9v YmFy").as_deref(), Some(&b"foobar"[..]));
        // Both alphabets decode; `-_` are the URL-safe spellings of `+/`.
        assert_eq!(base64_decode("+/8=").as_deref(), Some(&[0xfb, 0xff][..]));
        assert_eq!(base64_decode("-_8=").as_deref(), Some(&[0xfb, 0xff][..]));
        // High bytes round-trip.
        assert_eq!(
            base64_decode("/w==").as_deref(),
            Some(&[0xff][..]),
            "0xff must survive"
        );
    }

    #[test]
    fn base64_rejects_invalid_input() {
        for bad in [
            "Zm9v!",    // character outside both alphabets
            "Zm9v.",    //
            "Z",        // 6 leftover bits cannot encode a byte
            "Zm9vZ",    // ditto, after a complete group
            "Zg===",    // more than two padding characters
            "Zg==Zg==", // data resumed after the padding
            "Zm9v=",    // spurious padding on a complete group
            "Zm9v§",    // non-ASCII
        ] {
            assert_eq!(base64_decode(bad), None, "input {bad:?}");
        }
    }

    // -- render_disc: geometry ---------------------------------------------

    #[test]
    fn output_is_a_square_of_the_requested_size() {
        let art = quadrants(64);
        let out = render_disc(&art, &plain(96, 0.0));
        assert_eq!((out.w, out.h), (96, 96));
        assert_eq!(out.stride(), 96 * 4);
        assert_eq!(out.data.len(), 96 * 96 * 4);
        assert!(!out.is_empty());
    }

    #[test]
    fn size_px_is_clamped_to_a_usable_range() {
        let art = solid(8, WHITE);
        assert_eq!(render_disc(&art, &plain(0, 0.0)).w, 1);
        let huge = render_disc(
            &art,
            &DiscCfg {
                size_px: u32::MAX,
                ..plain(0, 0.0)
            },
        );
        assert_eq!(huge.w, MAX_DISC_PX);
        assert_eq!(huge.data.len(), (MAX_DISC_PX as usize).pow(2) * 4);
    }

    #[test]
    fn corners_are_fully_transparent() {
        let art = solid(64, WHITE);
        let out = render_disc(&art, &DiscCfg::default());
        let last = out.w - 1;
        for (x, y) in [(0, 0), (last, 0), (0, last), (last, last)] {
            assert_eq!(
                px(&out, x, y),
                (0, 0, 0, 0),
                "corner ({x},{y}) must be fully transparent"
            );
        }
    }

    #[test]
    fn centre_is_opaque_without_a_hole() {
        let art = solid(64, WHITE);
        let out = render_disc(&art, &plain(64, 0.0));
        let (_, _, _, a) = px(&out, 32, 32);
        assert_eq!(a, 255, "the middle of a holeless opaque disc must be solid");
    }

    #[test]
    fn the_spindle_hole_is_transparent() {
        let art = solid(64, WHITE);
        let out = render_disc(
            &art,
            &DiscCfg {
                size_px: 64,
                hole_ratio: 0.10,
                ..DiscCfg::default()
            },
        );
        assert_eq!(px(&out, 32, 32), (0, 0, 0, 0), "the hole must be punched");
        // And the disc around it is still there.
        let (_, _, _, a) = px(&out, 32, 20);
        assert!(a > 200, "only the hole should be transparent, got a={a}");
    }

    #[test]
    fn opacity_scales_the_whole_disc() {
        let art = solid(64, WHITE);
        let out = render_disc(
            &art,
            &DiscCfg {
                opacity: 128,
                ..plain(64, 0.0)
            },
        );
        let (b, g, r, a) = px(&out, 32, 32);
        assert_eq!(a, 128);
        // Premultiplied: white at half alpha is (128,128,128,128).
        assert_eq!((b, g, r), (128, 128, 128));
    }

    // -- render_disc: rotation (the inverse-mapping proof) ------------------

    #[test]
    fn rotation_zero_is_the_identity() {
        // Source and disc the same size, so at 0° the sampler lands exactly on
        // pixel centres and the mapping is exact, not merely close.
        let out = render_disc(&quadrants(64), &plain(64, 0.0));
        assert_eq!(classify(px(&out, 16, 16)), Some("red"), "top-left");
        assert_eq!(classify(px(&out, 48, 16)), Some("green"), "top-right");
        assert_eq!(classify(px(&out, 16, 48)), Some("blue"), "bottom-left");
        assert_eq!(classify(px(&out, 48, 48)), Some("white"), "bottom-right");
    }

    /// The test that proves the mapping is *inverse*, not forward.
    ///
    /// Forward mapping would leave unwritten holes at 90°-adjacent angles;
    /// a transposed or sign-flipped matrix would put the quadrants in the
    /// wrong corners. Both are caught here, and 90° with a same-size source
    /// lands exactly on pixel centres so the expected colours are exact.
    #[test]
    fn rotating_ninety_degrees_moves_quadrants_clockwise() {
        let out = render_disc(&quadrants(64), &plain(64, 90.0));
        // Clockwise: top-left → top-right → bottom-right → bottom-left.
        assert_eq!(classify(px(&out, 48, 16)), Some("red"), "top-right");
        assert_eq!(classify(px(&out, 48, 48)), Some("green"), "bottom-right");
        assert_eq!(classify(px(&out, 16, 16)), Some("blue"), "top-left");
        assert_eq!(classify(px(&out, 16, 48)), Some("white"), "bottom-left");

        // Every pixel inside the disc must have been written — the exact
        // failure mode of forward mapping is a lattice of transparent holes.
        for y in 24..40 {
            for x in 24..40 {
                assert_eq!(px(&out, x, y).3, 255, "hole at ({x},{y})");
            }
        }
    }

    #[test]
    fn rotating_a_full_turn_matches_no_rotation() {
        let art = quadrants(64);
        let a = render_disc(&art, &plain(64, 0.0));
        let b = render_disc(&art, &plain(64, 360.0));
        let worst = a
            .data
            .iter()
            .zip(&b.data)
            .map(|(x, y)| (i32::from(*x) - i32::from(*y)).abs())
            .max()
            .unwrap_or(0);
        assert!(worst <= 3, "360° differs from 0° by up to {worst}");
    }

    #[test]
    fn rotation_is_continuous_across_the_wrap() {
        // 359.9° and -0.1° are the same picture; if they were not, the disc
        // would visibly jump once per revolution.
        let art = quadrants(64);
        let a = render_disc(&art, &plain(64, 359.9));
        let b = render_disc(&art, &plain(64, -0.1));
        assert_eq!(a.data, b.data);
    }

    // -- render_disc: quality ----------------------------------------------

    #[test]
    fn the_disc_edge_is_anti_aliased() {
        let art = solid(128, WHITE);
        let out = render_disc(&art, &plain(128, 0.0));
        let partial = out
            .data
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && p[3] < 255)
            .count();
        assert!(
            partial > 0,
            "a hard-edged circle looks cheap: no partially covered edge pixels"
        );
        // A 128px circle has a ~400px circumference, so a real AA edge has
        // hundreds of partial pixels, not the handful a rounding artefact
        // would produce.
        assert!(partial > 100, "only {partial} partial-alpha pixels");
    }

    #[test]
    fn colour_never_exceeds_alpha_anywhere() {
        // mpv: "the numeric value of each component is equal to or smaller
        // than the alpha component". Check it with every decoration on, on
        // art bright enough to expose the label lightening.
        let art = solid(64, WHITE);
        let out = render_disc(
            &art,
            &DiscCfg {
                size_px: 96,
                rotation_deg: 37.0,
                ..DiscCfg::default()
            },
        );
        for (i, p) in out.data.chunks_exact(4).enumerate() {
            assert!(
                p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3],
                "pixel {i} is not premultiplied: bgra={p:?}"
            );
        }
    }

    #[test]
    fn transparent_source_pixels_do_not_bleed_colour() {
        // Straight-alpha interpolation drags the RGB of transparent pixels
        // into their neighbours. Here every source pixel is transparent
        // magenta; nothing may come out.
        let art = solid(64, [255, 0, 255, 0]);
        let out = render_disc(&art, &plain(64, 33.0));
        assert!(
            out.data.iter().all(|&v| v == 0),
            "fully transparent art must render nothing at all"
        );
    }

    #[test]
    fn the_label_and_ring_actually_change_pixels() {
        // Not a look test — just that the decorations are wired up, so a
        // regression that drops them is visible here and not only on a desktop.
        let art = solid(128, [180, 180, 180, 255]);
        let plainer = render_disc(&art, &plain(128, 0.0));
        let dressed = render_disc(
            &art,
            &DiscCfg {
                size_px: 128,
                hole_ratio: 0.0,
                ..DiscCfg::default()
            },
        );
        // The rim is darker than the flat render.
        assert!(px(&dressed, 64, 6).2 < px(&plainer, 64, 6).2, "outer ring");
        // The label rim is darker than its own surroundings.
        let label_r = (0.33 * 64.0) as u32;
        let on_rim = px(&dressed, 64, 64 - label_r).2;
        let inside = px(&dressed, 64, 64 - label_r + 6).2;
        assert!(on_rim < inside, "label rim {on_rim} vs inside {inside}");
    }

    // -- render_disc: degenerate input --------------------------------------

    #[test]
    fn degenerate_sources_do_not_panic() {
        for (w, h) in [(1, 1), (1, 512), (512, 1), (3, 4000), (4000, 3), (2, 1)] {
            // Asymmetric content, so a bad crop of an extreme aspect ratio
            // shows up as a wrong colour rather than passing silently.
            let art = RgbaImage::from_fn(w, h, |x, y| {
                Rgba(if (x + y) % 2 == 0 { WHITE } else { RED })
            });
            let out = render_disc(&art, &plain(48, 41.0));
            assert_eq!((out.w, out.h), (48, 48), "source {w}x{h}");
            assert_eq!(px(&out, 24, 24).3, 255, "source {w}x{h} centre");
        }
    }

    #[test]
    fn a_zero_sized_source_renders_a_transparent_disc() {
        let art = RgbaImage::new(0, 0);
        let out = render_disc(&art, &plain(32, 12.0));
        assert_eq!((out.w, out.h), (32, 32));
        assert!(out.data.iter().all(|&v| v == 0));
    }

    #[test]
    fn nonsense_config_values_are_sanitised() {
        let art = solid(32, WHITE);
        let cfg = DiscCfg {
            size_px: 48,
            rotation_deg: f32::NAN,
            label_ratio: -3.0,
            hole_ratio: f32::INFINITY,
            ring_darken: 9.0,
            opacity: 255,
        };
        let out = render_disc(&art, &cfg);
        assert_eq!((out.w, out.h), (48, 48));
        // NaN rotation falls back to 0, so the disc is still drawn.
        assert!(out.data.iter().any(|&v| v != 0));
    }

    // -- rotation_for / should_redraw ---------------------------------------

    #[test]
    fn rotation_for_converts_and_wraps() {
        // 60 rpm = 360°/s, so the numbers are readable.
        let deg = |ms| rotation_for(Duration::from_millis(ms), 60.0);
        assert!((deg(0) - 0.0).abs() < 1e-3);
        assert!((deg(250) - 90.0).abs() < 1e-3);
        assert!((deg(500) - 180.0).abs() < 1e-3);
        // A full turn wraps to 0, not to 360.
        assert!(deg(1000) < 1e-3, "one revolution must wrap to 0");
        assert!((deg(1250) - 90.0).abs() < 1e-3, "wraps past the first turn");
        // Still exact after an hour of playback — the accumulator is f64.
        assert!(
            (rotation_for(Duration::from_secs(3600) + Duration::from_millis(250), 60.0) - 90.0)
                .abs()
                < 1e-2
        );
    }

    #[test]
    fn rotation_for_handles_odd_speeds() {
        // A 12" LP: one revolution in 1.8s. `VINYL_RPM` is 33⅓ rounded to the
        // nearest f32, so the result lands a hair either side of the wrap —
        // measure the distance to 0 the short way round.
        let turn = rotation_for(Duration::from_millis(1800), VINYL_RPM);
        assert!(
            turn.min(360.0 - turn) < 1e-2,
            "one LP revolution gave {turn}"
        );
        // Stopped means stopped.
        assert_eq!(rotation_for(Duration::from_secs(99), 0.0), 0.0);
        // Backwards is legal and wraps into 0..360.
        let back = rotation_for(Duration::from_millis(250), -60.0);
        assert!((back - 270.0).abs() < 1e-3, "got {back}");
        // Garbage in, zero out — never NaN into the renderer.
        assert_eq!(rotation_for(Duration::from_secs(1), f32::NAN), 0.0);
        assert_eq!(rotation_for(Duration::from_secs(1), f32::INFINITY), 0.0);
    }

    #[test]
    fn should_redraw_respects_the_threshold() {
        // Below, at, and above the threshold.
        assert!(!should_redraw(10.0, 10.4, 0.5), "below the threshold");
        assert!(should_redraw(10.0, 10.5, 0.5), "exactly at the threshold");
        assert!(should_redraw(10.0, 11.0, 0.5), "above the threshold");
        // Identical angles never redraw.
        assert!(!should_redraw(42.0, 42.0, DEFAULT_MIN_STEP_DEG));
    }

    #[test]
    fn should_redraw_measures_the_short_way_round() {
        // 359.8 → 0.1 is a 0.3° step, not a 359.7° one.
        assert!(!should_redraw(359.8, 0.1, 0.5));
        assert!(should_redraw(359.0, 0.1, 0.5));
        // Symmetric in both directions.
        assert!(!should_redraw(0.1, 359.8, 0.5));
        // Half a turn is the largest possible distance.
        assert!(should_redraw(0.0, 180.0, 179.0));
    }

    #[test]
    fn should_redraw_errs_towards_drawing() {
        // A dropped frame is a visible bug; an extra one costs microseconds.
        assert!(should_redraw(f32::NAN, 10.0, 1.0));
        assert!(should_redraw(10.0, f32::NAN, 1.0));
        assert!(should_redraw(10.0, 10.0, 0.0));
        assert!(should_redraw(10.0, 10.0, -1.0));
        assert!(should_redraw(10.0, 10.0, f32::NAN));
    }

    // -- decode / placeholder / load_bytes ----------------------------------

    #[test]
    fn decode_art_round_trips_a_png() {
        let art = quadrants(16);
        let mut png = Vec::new();
        art.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encoding must work");
        let back = decode_art(&png).expect("our own PNG must decode");
        assert_eq!((back.width(), back.height()), (16, 16));
        assert_eq!(back.get_pixel(2, 2).0, RED);
        assert_eq!(back.get_pixel(13, 13).0, WHITE);
    }

    #[test]
    fn decode_art_rejects_non_images() {
        assert!(decode_art(b"").is_err(), "empty");
        assert!(decode_art(b"not an image at all").is_err(), "garbage");
        // An SVG is a real thing players publish, and we have no SVG decoder.
        assert!(decode_art(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_err());
    }

    #[test]
    fn prepare_source_shrinks_only_oversized_art() {
        use std::borrow::Cow;

        // Already small enough: borrowed, not copied.
        let small = quadrants(64);
        assert!(matches!(prepare_source(&small, 320), Cow::Borrowed(_)));
        // Exactly at the 2x target is still small enough.
        let exact = quadrants(640);
        assert!(matches!(prepare_source(&exact, 320), Cow::Borrowed(_)));

        // Oversized: shrunk to the target, aspect ratio preserved.
        let big = RgbaImage::from_fn(1600, 1200, |x, y| {
            Rgba(match (x < 800, y < 600) {
                (true, true) => RED,
                (false, true) => GREEN,
                (true, false) => BLUE,
                (false, false) => WHITE,
            })
        });
        let prepared = prepare_source(&big, 320);
        assert!(matches!(prepared, Cow::Owned(_)));
        assert_eq!((prepared.width(), prepared.height()), (640, 480));

        // And the shrink did not transpose or mirror anything: the disc drawn
        // from it still has its quadrants where the original put them.
        let out = render_disc(&prepared, &plain(64, 0.0));
        assert_eq!(classify(px(&out, 20, 20)), Some("red"), "top-left");
        assert_eq!(classify(px(&out, 44, 44)), Some("white"), "bottom-right");

        // A zero-sized image has nothing to shrink and must not divide by it.
        let empty = RgbaImage::new(0, 0);
        assert!(matches!(prepare_source(&empty, 320), Cow::Borrowed(_)));
    }

    #[test]
    fn placeholder_is_opaque_and_square() {
        let p = placeholder_art(64);
        assert_eq!((p.width(), p.height()), (64, 64));
        assert!(p.pixels().all(|px| px.0[3] == 255));
        // It has some structure, or it would render as a flat coloured circle.
        let centre = p.get_pixel(32, 32).0[2];
        let corner = p.get_pixel(1, 1).0[2];
        assert!(centre > corner, "centre {centre} vs corner {corner}");
        // Absurd sizes are clamped rather than honoured.
        assert_eq!(placeholder_art(0).width(), 8);
        assert_eq!(placeholder_art(u32::MAX).width(), 512);
    }

    #[test]
    fn load_bytes_reads_inline_and_local_sources() {
        // Inline.
        let src = parse_art_url("data:image/png;base64,QUJD").unwrap();
        assert_eq!(load_bytes(&src).unwrap(), b"ABC");

        // Local file, round-tripped through the URL parser so the percent
        // decoding is exercised end to end.
        let dir = std::env::temp_dir();
        let path = dir.join(format!("fresco artwork {}.bin", std::process::id()));
        std::fs::write(&path, b"cover bytes").expect("temp write");
        let url = format!("file://{}", path.display().to_string().replace(' ', "%20"));
        let src = parse_art_url(&url).expect("temp file URL must parse");
        assert_eq!(src, ArtSource::File(path.clone()));
        assert_eq!(load_bytes(&src).unwrap(), b"cover bytes");
        let _ = std::fs::remove_file(&path);

        // The far more common case: the file is gone, or was never ours to
        // read. That must be an error, not a panic.
        assert!(
            load_bytes(&src).is_err(),
            "a removed file must fail cleanly"
        );
        assert!(load_bytes(&ArtSource::File(dir)).is_err(), "a directory");
    }

    #[test]
    fn load_bytes_enforces_the_size_cap_on_inline_art() {
        let src = ArtSource::Data {
            mime: "image/png".into(),
            bytes: vec![0u8; MAX_ART_BYTES as usize + 1],
        };
        assert!(load_bytes(&src).is_err(), "oversized inline art");
    }

    #[test]
    fn read_capped_stops_at_the_cap() {
        // Exactly at the cap is fine; one byte more is not. `io::repeat` is an
        // infinite stream, which is what a hostile server looks like.
        let at_cap = std::io::repeat(7).take(MAX_ART_BYTES);
        assert_eq!(read_capped(at_cap).unwrap().len(), MAX_ART_BYTES as usize);
        assert!(read_capped(std::io::repeat(7)).is_err(), "infinite stream");
    }
}

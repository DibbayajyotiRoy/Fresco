//! Local browser bridge (opt-in, off by default).
//!
//! A tiny hand-rolled HTTP/1.1 server on 127.0.0.1 that the Fresco browser
//! new-tab extension polls to mirror the wallpaper. Hand-rolled because we
//! only need GET, two routes, and fixed headers — pulling in an HTTP crate
//! for that would be the heaviest dependency in the daemon.
//!
//! Security model: the listener binds loopback ONLY (never 0.0.0.0) — that is
//! the boundary. The data served is just the wallpaper the user chose, so the
//! permissive CORS header is safe (and required: extensions fetch cross-origin
//! from 127.0.0.1).
//!
//! The wallpaper served is `config.browser_wallpaper` when the user set a
//! browser-specific one, otherwise the desktop wallpaper (mirror mode).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use crate::config::{Config, Kind, Wallpaper};

/// Fixed production port; tests inject an ephemeral one via `spawn`.
pub const PORT: u16 = 8765;

/// Start the bridge on a background thread. Never fails the daemon: a bind
/// error (port taken) is logged and the wallpaper keeps running without it.
pub fn spawn(port: u16) {
    std::thread::Builder::new()
        .name("webbridge".into())
        .spawn(move || serve(port))
        .ok();
}

fn serve(port: u16) {
    // Loopback only — binding any other interface would expose the wallpaper
    // to the network, which nothing here needs.
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            log::warn!("browser bridge: cannot bind 127.0.0.1:{port}: {e}");
            return;
        }
    };
    log::info!("browser bridge listening on 127.0.0.1:{port}");
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        // Handled inline: requests are tiny and the extension polls slowly, so
        // a thread per connection would be pure overhead.
        if let Err(e) = handle(stream) {
            log::debug!("browser bridge: request failed: {e}");
        }
    }
}

fn handle(mut stream: TcpStream) -> std::io::Result<()> {
    // A stalled or malicious client must not wedge the accept loop forever.
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf)?;
    let text = String::from_utf8_lossy(&buf[..n]);
    let Some(path) = parse_request_path(&text) else {
        // Malformed request: drop silently, never panic.
        log::debug!("browser bridge: malformed request ignored");
        return Ok(());
    };
    // The switch takes effect live for turning OFF: a disabled bridge refuses
    // even while the (already bound) listener thread is still alive.
    let cfg = Config::load().unwrap_or_default();
    if !cfg.browser_bridge {
        return respond(&mut stream, 403, "text/plain", b"bridge disabled");
    }
    match path.as_str() {
        "/status" => {
            let body = status_json(&cfg);
            respond(&mut stream, 200, "application/json", body.as_bytes())
        }
        "/frame" => match frame_bytes(&cfg) {
            Some((bytes, ct)) => respond(&mut stream, 200, ct, &bytes),
            None => respond(&mut stream, 404, "text/plain", b"no frame"),
        },
        _ => respond(&mut stream, 404, "text/plain", b"not found"),
    }
}

/// Extract the path from an HTTP request line; None unless it's a GET.
fn parse_request_path(request: &str) -> Option<String> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let path = parts.next()?;
    parts.next()?.starts_with("HTTP/").then(|| path.to_string())
}

fn respond(
    stream: &mut TcpStream,
    code: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        403 => "Forbidden",
        _ => "Not Found",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)
}

/// The wallpaper the browser should show: the browser-specific override when
/// set, else whatever the desktop is showing RIGHT NOW — including an active
/// day/night schedule. The daemon applies schedule swaps in memory only (the
/// on-disk config keeps the user's own pick), so mirror mode must re-resolve
/// the schedule here or the browser lags the desktop.
fn effective(cfg: &Config) -> (Wallpaper, &'static str) {
    if let Some(w) = &cfg.browser_wallpaper {
        return (w.clone(), "browser");
    }
    if let Some(w) = super::schedule_desired_wallpaper(cfg) {
        return (w, "desktop");
    }
    (cfg.wallpaper.clone(), "desktop")
}

fn status_json(cfg: &Config) -> String {
    let (w, source) = effective(cfg);
    let w = &w;
    let kind = kind_str(w);
    let name = w
        .effective_path()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Slideshow".into());
    // Config mtime is a cheap, good-enough "changed" signal: every wallpaper
    // change rewrites config.toml.
    let changed = std::fs::metadata(Config::path())
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "{{\"app\":\"fresco\",\"version\":\"{}\",\"kind\":\"{kind}\",\"name\":\"{}\",\"source\":\"{source}\",\"changed_epoch\":{changed}}}",
        env!("CARGO_PKG_VERSION"),
        json_escape(&name),
    )
}

/// The extension-facing kind: gif images are called out separately so the
/// extension can animate them.
fn kind_str(w: &Wallpaper) -> &'static str {
    match w.kind {
        Kind::Video | Kind::Playlist => "video",
        Kind::Slideshow => "slideshow",
        Kind::Image => {
            if has_ext(w.effective_path(), "gif") {
                "gif"
            } else {
                "image"
            }
        }
    }
}

fn has_ext(p: Option<&Path>, ext: &str) -> bool {
    p.and_then(|p| p.extension())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Content-Type for a still-image file we serve directly.
fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

/// Bytes + Content-Type for the current still frame.
///
/// Images (and slideshow's current-ish image) are served as-is. Videos go
/// through a cached ffmpeg poster frame, regenerated only when the source
/// changed — extracting on every poll would burn CPU for nothing.
fn frame_bytes(cfg: &Config) -> Option<(Vec<u8>, &'static str)> {
    let (w, _) = effective(cfg);
    let src = match w.kind {
        Kind::Slideshow => {
            let s = w.slideshow.as_ref()?;
            super::slideshow_images(s).into_iter().next()?
        }
        _ => w.effective_path()?.to_path_buf(),
    };
    if !src.exists() {
        return None;
    }
    match w.kind {
        Kind::Video | Kind::Playlist => {
            let poster = cached_poster(&src)?;
            Some((std::fs::read(poster).ok()?, "image/jpeg"))
        }
        _ => Some((std::fs::read(&src).ok()?, content_type(&src))),
    }
}

/// Poster frame for a video, cached under the fresco cache dir. A small state
/// file records "path\nmtime" of the source; ffmpeg runs again only when
/// either changed.
fn cached_poster(src: &Path) -> Option<PathBuf> {
    let dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("fresco");
    std::fs::create_dir_all(&dir).ok();
    let poster = dir.join("bridge-frame.jpg");
    let state = dir.join("bridge-frame.state");
    let mtime = std::fs::metadata(src)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let stamp = format!("{}\n{mtime}", src.display());
    let fresh = poster.exists()
        && std::fs::read_to_string(&state)
            .map(|s| s == stamp)
            .unwrap_or(false);
    if fresh {
        return Some(poster);
    }
    // Same ffmpeg discipline as the GNOME overview path: -nostdin + null stdio,
    // or a shell-launched daemon gets SIGTTIN-stopped by ffmpeg reading the tty.
    let ok = Command::new("ffmpeg")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .args([
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-i",
            &src.to_string_lossy(),
            "-frames:v",
            "1",
            &poster.to_string_lossy(),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        log::debug!("browser bridge: ffmpeg poster extraction failed");
        return None;
    }
    std::fs::write(&state, stamp).ok();
    Some(poster)
}

/// Minimal JSON string escaping — names are file stems, but a quote or
/// backslash in one must not break the payload.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_line_parsing() {
        assert_eq!(
            parse_request_path("GET /status HTTP/1.1\r\nHost: x\r\n\r\n").as_deref(),
            Some("/status")
        );
        assert_eq!(
            parse_request_path("GET /frame HTTP/1.0\r\n\r\n").as_deref(),
            Some("/frame")
        );
        assert!(parse_request_path("POST /status HTTP/1.1\r\n\r\n").is_none());
        assert!(parse_request_path("").is_none());
        assert!(parse_request_path("GET /status").is_none()); // no HTTP version
        assert!(parse_request_path("garbage\r\n").is_none());
    }

    #[test]
    fn content_types() {
        assert_eq!(content_type(Path::new("/a/b.PNG")), "image/png");
        assert_eq!(content_type(Path::new("/a/b.jpeg")), "image/jpeg");
        assert_eq!(content_type(Path::new("/a/b.gif")), "image/gif");
        assert_eq!(content_type(Path::new("/a/b.webp")), "image/webp");
        assert_eq!(content_type(Path::new("/a/b")), "application/octet-stream");
    }

    #[test]
    fn kind_mapping() {
        let mut w = Wallpaper::default(); // Video
        assert_eq!(kind_str(&w), "video");
        w.kind = Kind::Playlist;
        assert_eq!(kind_str(&w), "video");
        w.kind = Kind::Slideshow;
        assert_eq!(kind_str(&w), "slideshow");
        w.kind = Kind::Image;
        w.path = Some("/x/pic.jpg".into());
        assert_eq!(kind_str(&w), "image");
        w.path = Some("/x/anim.GIF".into());
        assert_eq!(kind_str(&w), "gif");
    }

    #[test]
    fn browser_override_wins() {
        let mut cfg = Config::default();
        assert_eq!(effective(&cfg).1, "desktop");
        cfg.browser_wallpaper = Some(Wallpaper {
            kind: Kind::Image,
            path: Some("/b.png".into()),
            ..Default::default()
        });
        let (w, source) = effective(&cfg);
        assert_eq!(source, "browser");
        assert_eq!(w.path.as_deref().unwrap().to_str(), Some("/b.png"));
    }

    #[test]
    fn status_json_shape() {
        let mut cfg = Config::default();
        cfg.wallpaper.path = Some("/videos/My \"best\" clip.mp4".into());
        let j = status_json(&cfg);
        assert!(j.contains("\"app\":\"fresco\""));
        assert!(j.contains("\"kind\":\"video\""));
        assert!(j.contains("\"source\":\"desktop\""));
        assert!(j.contains("My \\\"best\\\" clip")); // escaped quote
        assert!(j.contains("\"changed_epoch\":"));
        // Must be valid JSON.
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["app"], "fresco");
    }

    #[test]
    fn json_escaping() {
        assert_eq!(json_escape("plain"), "plain");
        assert_eq!(json_escape("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }

    // Bind-and-request integration test on an ephemeral port. The bridge
    // reads the real user config (may or may not have the switch on), so only
    // assert protocol-level behavior that holds either way.
    #[test]
    fn serves_http_on_loopback() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener); // free it for spawn (tiny race, fine for a test)
        spawn(port);
        // Wait for the thread to bind.
        let mut stream = None;
        for _ in 0..50 {
            if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
                stream = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut s = stream.expect("bridge did not bind");
        s.write_all(b"GET /nope HTTP/1.1\r\nHost: l\r\n\r\n")
            .unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        // 403 if the user's config has the bridge off, 404 for unknown route
        // when on — either way it's well-formed HTTP with our fixed headers.
        assert!(resp.starts_with("HTTP/1.1 40"), "got: {resp}");
        assert!(resp.contains("Access-Control-Allow-Origin: *"));
        assert!(resp.contains("Cache-Control: no-store"));
    }
}

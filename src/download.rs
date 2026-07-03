//! Portable media downloader (ROADMAP 3.2): direct media URLs only — no
//! yt-dlp/YouTube (ToS risk to Flathub/AUR standing; the catalog is the
//! strategic content path). Brain code, no Linux-isms (macOS readiness):
//! callers hand in the destination directory.
//!
//! Safety properties, all unit-tested:
//! - `Content-Length` pre-check AND a mid-stream cap (servers lie),
//! - atomic delivery: bytes stream to `<name>.part`, renamed only on success,
//! - cancellation leaves nothing behind,
//! - never overwrites an existing file (uniquified name).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Media types we accept from a URL (mirrors the library's file pickers).
pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "webm", "mkv", "avi", "mov", "gif", "jpg", "jpeg", "png", "webp", "bmp",
];

#[derive(Debug)]
pub enum DownloadError {
    /// Not http(s), or no usable media filename in the URL path.
    BadUrl(String),
    /// Larger than the caller's cap (pre-checked or detected mid-stream).
    TooLarge {
        limit: u64,
    },
    Http(String),
    Io(std::io::Error),
    Cancelled,
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::BadUrl(m) => write!(f, "not a direct media URL: {m}"),
            DownloadError::TooLarge { limit } => {
                write!(f, "file exceeds the {} MB limit", limit / 1_048_576)
            }
            DownloadError::Http(m) => write!(f, "download failed: {m}"),
            DownloadError::Io(e) => write!(f, "could not save file: {e}"),
            DownloadError::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Filename implied by a direct media URL, or None when the URL does not end
/// in a recognized media extension (that's what "direct URL only" means).
pub fn media_filename(url: &str) -> Option<String> {
    let no_scheme = url
        .strip_prefix("https://")
        .or(url.strip_prefix("http://"))?;
    let path = no_scheme.split(['?', '#']).next()?;
    let name = path.rsplit('/').next()?.trim();
    if name.is_empty() {
        return None;
    }
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    MEDIA_EXTENSIONS.contains(&ext.as_str()).then(|| {
        // Keep it filesystem-safe.
        name.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    })
}

/// First non-colliding variant of `name` inside `dir` ("clip.mp4" → "clip-1.mp4").
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.to_string(), String::new()),
    };
    (1..)
        .map(|i| dir.join(format!("{stem}-{i}{ext}")))
        .find(|p| !p.exists())
        .expect("some suffix is free")
}

/// Download `url` into `dest_dir` (created if missing). `max_bytes` caps the
/// size before AND during transfer. `cancel` is checked between chunks.
/// `on_progress(received, total)` fires per chunk. Returns the final path.
pub fn download(
    url: &str,
    dest_dir: &Path,
    max_bytes: u64,
    cancel: &AtomicBool,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<PathBuf, DownloadError> {
    let name = media_filename(url).ok_or_else(|| DownloadError::BadUrl(url.to_string()))?;
    std::fs::create_dir_all(dest_dir).map_err(DownloadError::Io)?;

    let resp = ureq::get(url)
        .call()
        .map_err(|e| DownloadError::Http(e.to_string()))?;
    let total: Option<u64> = resp.header("Content-Length").and_then(|v| v.parse().ok());
    if let Some(t) = total {
        if t > max_bytes {
            return Err(DownloadError::TooLarge { limit: max_bytes });
        }
    }

    let final_path = unique_path(dest_dir, &name);
    let part = final_path.with_extension(format!(
        "{}.part",
        final_path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    // Any early exit below must not leave the .part behind.
    let cleanup = |e: DownloadError| -> DownloadError {
        let _ = std::fs::remove_file(&part);
        e
    };

    let mut file = std::fs::File::create(&part).map_err(DownloadError::Io)?;
    let mut reader = resp.into_reader();
    let mut received: u64 = 0;
    let mut buf = [0u8; 65536];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(cleanup(DownloadError::Cancelled));
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return Err(cleanup(DownloadError::Io(e))),
        };
        received += n as u64;
        if received > max_bytes {
            return Err(cleanup(DownloadError::TooLarge { limit: max_bytes }));
        }
        if let Err(e) = file.write_all(&buf[..n]) {
            return Err(cleanup(DownloadError::Io(e)));
        }
        on_progress(received, total);
    }
    if let Err(e) = file.flush() {
        return Err(cleanup(DownloadError::Io(e)));
    }
    drop(file);
    std::fs::rename(&part, &final_path).map_err(|e| cleanup(DownloadError::Io(e)))?;
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// One-shot HTTP server on a random port; serves `body` with the given
    /// headers, then exits. Returns "http://127.0.0.1:PORT".
    fn serve(status_headers: String, body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut discard = [0u8; 4096];
                let _ = std::io::Read::read(&mut sock, &mut discard);
                let _ = sock.write_all(status_headers.as_bytes());
                let _ = sock.write_all(&body);
            }
        });
        format!("http://{addr}")
    }

    fn ok_headers(len: Option<usize>) -> String {
        match len {
            Some(l) => {
                format!("HTTP/1.1 200 OK\r\nContent-Length: {l}\r\nConnection: close\r\n\r\n")
            }
            None => "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_string(),
        }
    }

    #[test]
    fn media_filenames() {
        assert_eq!(
            media_filename("https://x.com/a/clip.mp4?sig=1").as_deref(),
            Some("clip.mp4")
        );
        assert_eq!(
            media_filename("http://x.com/loop.WebM").as_deref(),
            Some("loop.WebM")
        );
        assert_eq!(media_filename("https://x.com/watch?v=abc"), None); // no media ext
        assert_eq!(media_filename("ftp://x.com/a.mp4"), None); // wrong scheme
        assert_eq!(media_filename("https://x.com/"), None);
    }

    #[test]
    fn downloads_and_renames_atomically() {
        let body = vec![7u8; 10_000];
        let url = serve(ok_headers(Some(10_000)), body.clone());
        let dir = tempdir();
        let cancel = AtomicBool::new(false);
        let got = download(
            &format!("{url}/a/clip.mp4"),
            &dir,
            1_000_000,
            &cancel,
            |_, _| {},
        )
        .unwrap();
        assert_eq!(std::fs::read(&got).unwrap(), body);
        assert_eq!(got.file_name().unwrap(), "clip.mp4");
        assert!(no_part_files(&dir), "no .part left behind");
    }

    #[test]
    fn oversize_is_refused_before_and_during() {
        // Pre-check: honest Content-Length above the cap.
        let url = serve(ok_headers(Some(2_000_000)), vec![0u8; 8]);
        let dir = tempdir();
        let cancel = AtomicBool::new(false);
        match download(
            &format!("{url}/big.mp4"),
            &dir,
            1_000_000,
            &cancel,
            |_, _| {},
        ) {
            Err(DownloadError::TooLarge { .. }) => {}
            other => panic!("want TooLarge, got {other:?}"),
        }
        // Mid-stream: server sends more than the cap with no Content-Length.
        let url = serve(ok_headers(None), vec![0u8; 300_000]);
        match download(
            &format!("{url}/liar.mp4"),
            &dir,
            100_000,
            &cancel,
            |_, _| {},
        ) {
            Err(DownloadError::TooLarge { .. }) => {}
            other => panic!("want mid-stream TooLarge, got {other:?}"),
        }
        assert!(no_part_files(&dir));
    }

    #[test]
    fn cancel_leaves_nothing() {
        let url = serve(ok_headers(Some(500_000)), vec![0u8; 500_000]);
        let dir = tempdir();
        let cancel = AtomicBool::new(false);
        let result = download(&format!("{url}/c.mp4"), &dir, 1_000_000, &cancel, |_, _| {
            cancel.store(true, Ordering::Relaxed); // cancel after the first chunk
        });
        assert!(matches!(result, Err(DownloadError::Cancelled)));
        assert!(no_part_files(&dir));
        assert!(!dir.join("c.mp4").exists());
    }

    #[test]
    fn existing_files_are_never_overwritten() {
        let dir = tempdir();
        std::fs::write(dir.join("clip.mp4"), b"original").unwrap();
        let url = serve(ok_headers(Some(4)), b"new!".to_vec());
        let cancel = AtomicBool::new(false);
        let got = download(
            &format!("{url}/clip.mp4"),
            &dir,
            1_000_000,
            &cancel,
            |_, _| {},
        )
        .unwrap();
        assert_eq!(got.file_name().unwrap(), "clip-1.mp4");
        assert_eq!(std::fs::read(dir.join("clip.mp4")).unwrap(), b"original");
    }

    fn tempdir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "fresco-dl-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn no_part_files(dir: &Path) -> bool {
        std::fs::read_dir(dir)
            .map(|it| {
                it.filter_map(|e| e.ok())
                    .all(|e| !e.file_name().to_string_lossy().ends_with(".part"))
            })
            .unwrap_or(true)
    }
}

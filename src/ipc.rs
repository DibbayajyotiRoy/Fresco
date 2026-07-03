use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Requests the GUI (or CLI) sends to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum Request {
    /// Re-read config.toml and apply it (swap wallpaper in place).
    Apply,
    /// Tear down wallpaper windows and exit the daemon.
    Stop,
    Pause,
    Resume,
    Status,
    /// Download and install the latest release in the background (fire-and-forget).
    Update,
}

/// One connected display, as the daemon sees it (RandR on X11, `wl_output`
/// on Wayland). Connector names match the keys `Config.monitors` accepts, so
/// the GUI can offer per-monitor assignment without guessing names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub connector: String,
    pub width: u16,
    pub height: u16,
    pub x: i16,
    pub y: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StatusReply {
    pub running: bool,
    pub paused: bool,
    /// Active mpv hwdec per monitor, e.g. "vaapi" / "nvdec" / "no" (software).
    pub hwdec: Option<String>,
    /// Human-readable description of what's playing.
    pub wallpaper: Option<String>,
    /// CPU of the daemon + renderer children since the previous status poll,
    /// as a percentage of ONE core (`top` semantics — may exceed 100 on
    /// multicore). 0.0 on the first poll (no baseline yet).
    pub cpu_percent: f32,
    /// Resident memory of the daemon + renderer children (mpvpaper), MB.
    pub rss_mb: u64,
    pub monitors: Vec<String>,
    /// Last media load failure, if any (file path + reason).
    pub error: Option<String>,
    /// True when the primary renderer has an audio track selected (mpv `aid`
    /// != no). False means mpv dropped/skipped audio — e.g. muted entries load
    /// with `aid=no`, and mpv deselects the track permanently when no audio
    /// server was reachable at load time. None = unknown / not applicable.
    #[serde(default)]
    pub audio_track: Option<bool>,
    #[serde(default)]
    pub mute: Option<bool>,
    #[serde(default)]
    pub volume: Option<u8>,
    /// Decode honesty (primary renderer): source dimensions, bit depth, and
    /// decoder frame drops — so "quality looks off" is diagnosable instead of
    /// silent (e.g. 8K on a GPU without 8K decode support).
    #[serde(default)]
    pub source_w: Option<u32>,
    #[serde(default)]
    pub source_h: Option<u32>,
    #[serde(default)]
    pub bit_depth: Option<u8>,
    #[serde(default)]
    pub dropped_frames: Option<u64>,
    /// ALL connected displays with geometry — unlike `monitors`, which only
    /// lists outputs that currently have a wallpaper.
    #[serde(default)]
    pub monitors_info: Vec<MonitorInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum Response {
    Ok,
    Status(StatusReply),
    Err { message: String },
}

pub fn socket_dir() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/fresco-{}", libc_getuid())))
        .join("fresco")
}

pub fn socket_path() -> PathBuf {
    socket_dir().join("control.sock")
}

// Avoid pulling in the libc crate for one call.
fn libc_getuid() -> u32 {
    std::fs::metadata("/proc/self")
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
        .unwrap_or(0)
}

/// Blocking request to the daemon. Returns Err if the daemon isn't running
/// (connection refused / socket missing) — callers treat that as "not running".
pub fn request(req: &Request) -> Result<Response> {
    request_at(&socket_path(), req)
}

/// Send `req` to the daemon listening at `path`. Split out from `request` so
/// tests can target an isolated (guaranteed-absent) socket deterministically.
fn request_at(path: &std::path::Path, req: &Request) -> Result<Response> {
    let mut stream = UnixStream::connect(path)
        .with_context(|| format!("daemon not reachable at {}", path.display()))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    let mut reader = BufReader::new(stream);
    let mut reply = String::new();
    reader
        .read_line(&mut reply)
        .context("reading daemon reply")?;
    let resp: Response = serde_json::from_str(reply.trim()).context("parsing daemon reply")?;
    Ok(resp)
}

/// True if a daemon is up and answering.
pub fn daemon_alive() -> bool {
    matches!(request(&Request::Status), Ok(Response::Status(s)) if s.running)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_json_shape() {
        assert_eq!(
            serde_json::to_string(&Request::Apply).unwrap(),
            r#"{"cmd":"apply"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::Status).unwrap(),
            r#"{"cmd":"status"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::Update).unwrap(),
            r#"{"cmd":"update"}"#
        );
    }

    #[test]
    fn response_roundtrip() {
        let r = Response::Status(StatusReply {
            running: true,
            hwdec: Some("vaapi".into()),
            cpu_percent: 1.5,
            rss_mb: 120,
            monitors: vec!["eDP-1".into()],
            ..Default::default()
        });
        let s = serde_json::to_string(&r).unwrap();
        let back: Response = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    /// Replies from an older daemon (without the audio fields) must still parse.
    #[test]
    fn status_reply_backcompat_without_audio_fields() {
        let old = r#"{"result":"status","running":true,"paused":false,"hwdec":null,
                      "wallpaper":null,"cpu_percent":0.0,"rss_mb":10,"monitors":[],"error":null}"#;
        let r: Response = serde_json::from_str(old).unwrap();
        match r {
            Response::Status(s) => {
                assert_eq!(s.audio_track, None);
                assert_eq!(s.mute, None);
                assert_eq!(s.volume, None);
                assert_eq!(s.source_w, None);
                assert_eq!(s.source_h, None);
                assert_eq!(s.bit_depth, None);
                assert_eq!(s.dropped_frames, None);
                assert!(s.monitors_info.is_empty());
            }
            other => panic!("expected Status, got {other:?}"),
        }
    }

    #[test]
    fn unreachable_daemon_errors() {
        // Target a socket path with no listener so the result is deterministic
        // even when a real frescod is running on this machine.
        let path = std::env::temp_dir().join(format!("fresco-absent-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert!(request_at(&path, &Request::Status).is_err());
    }
}

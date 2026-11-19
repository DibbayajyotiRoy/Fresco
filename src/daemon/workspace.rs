//! Per-workspace (virtual desktop) wallpapers, issue #124.
//!
//! Detection only: which workspace is focused, and the precedence that decides
//! what an output shows. The switch itself lives in the backends' loops, next
//! to the scheduler, so it keeps the same transitions.
//!
//! * X11: the root window's `_NET_CURRENT_DESKTOP`, read in the existing 250 ms
//!   fullscreen poll.
//! * sway: `GET_WORKSPACES` over `$SWAYSOCK` (i3-ipc), polled while the map is
//!   non-empty. Polling rather than subscribing keeps one code path and no
//!   thread; a reply is a few hundred bytes on a local socket.
//! * Hyprland: `j/activeworkspace` over `$XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock`,
//!   polled the same way.
//! * Anything else: no source, the feature stays inactive.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::config::Wallpaper;

/// How often a Wayland backend asks sway/Hyprland for the focused workspace.
pub const WAYLAND_POLL: Duration = Duration::from_millis(300);

const I3_MAGIC: &[u8; 6] = b"i3-ipc";
const I3_HEADER: usize = 14;
const SWAY_GET_WORKSPACES: u32 = 1;
/// A workspace list is a few KiB; refuse anything absurd rather than allocate it.
const MAX_REPLY: usize = 1 << 20;
const IPC_TIMEOUT: Duration = Duration::from_millis(200);

/// A Wayland compositor that can report the focused workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// sway's IPC socket (`$SWAYSOCK`).
    Sway(PathBuf),
    /// Hyprland's request socket (`.socket.sock`).
    Hyprland(PathBuf),
}

impl Source {
    /// The Wayland source for this session, if the compositor is one we know.
    pub fn from_env() -> Option<Self> {
        if let Some(sock) = std::env::var_os("SWAYSOCK") {
            return Some(Source::Sway(sock.into()));
        }
        let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
        Some(Source::Hyprland(
            PathBuf::from(runtime)
                .join("hypr")
                .join(sig)
                .join(".socket.sock"),
        ))
    }

    /// Key of the focused workspace, or `None` when the query failed. A failed
    /// query means "no information", so callers keep what is showing.
    pub fn current_key(&self) -> Option<String> {
        match self {
            Source::Sway(path) => {
                let mut s = connect(path)?;
                s.write_all(&encode_i3(SWAY_GET_WORKSPACES, b"")).ok()?;
                let (_, body) = read_i3(&mut s).ok()?;
                sway_focused(&body)
            }
            Source::Hyprland(path) => {
                let mut s = connect(path)?;
                s.write_all(b"j/activeworkspace").ok()?;
                let mut body = String::new();
                s.read_to_string(&mut body).ok()?;
                hypr_active(&body)
            }
        }
    }
}

fn connect(path: &PathBuf) -> Option<UnixStream> {
    let s = UnixStream::connect(path).ok()?;
    s.set_read_timeout(Some(IPC_TIMEOUT)).ok()?;
    s.set_write_timeout(Some(IPC_TIMEOUT)).ok()?;
    Some(s)
}

/// i3-ipc message: magic, payload length, message type, payload. Native
/// endianness, as sway writes it.
pub fn encode_i3(kind: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(I3_HEADER + payload.len());
    out.extend_from_slice(I3_MAGIC);
    out.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
    out.extend_from_slice(&kind.to_ne_bytes());
    out.extend_from_slice(payload);
    out
}

/// Read one i3-ipc reply, returning its type and payload.
pub fn read_i3<R: Read>(r: &mut R) -> io::Result<(u32, Vec<u8>)> {
    let mut head = [0u8; I3_HEADER];
    r.read_exact(&mut head)?;
    if &head[..6] != I3_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not an i3-ipc reply",
        ));
    }
    let len = u32::from_ne_bytes([head[6], head[7], head[8], head[9]]) as usize;
    let kind = u32::from_ne_bytes([head[10], head[11], head[12], head[13]]);
    if len > MAX_REPLY {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "i3-ipc reply too large",
        ));
    }
    let mut payload = vec![0; len];
    r.read_exact(&mut payload)?;
    Ok((kind, payload))
}

/// Name of the focused workspace in a sway `GET_WORKSPACES` reply.
pub fn sway_focused(json: &[u8]) -> Option<String> {
    let ws: Vec<serde_json::Value> = serde_json::from_slice(json).ok()?;
    ws.iter()
        .find(|w| w["focused"].as_bool() == Some(true))
        .and_then(|w| w["name"].as_str())
        .map(str::to_owned)
}

/// Name of the active workspace in a Hyprland `j/activeworkspace` reply.
pub fn hypr_active(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v["name"].as_str().map(str::to_owned)
}

/// Workspace key for X11's `_NET_CURRENT_DESKTOP` (0-based index). Keys are
/// 1-based so the config reads "Workspace 1" for the first desktop.
pub fn x11_key(current_desktop: &[u32]) -> Option<String> {
    let index = current_desktop.first()?.checked_add(1)?;
    Some(index.to_string())
}

/// `_NET_CURRENT_DESKTOP` (a CARDINAL, 0-based) of `root`, if published.
pub fn read_current_desktop<C: x11rb::connection::Connection>(
    conn: &C,
    root: u32,
    atom: u32,
) -> Option<Vec<u32>> {
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};
    let reply = conn
        .get_property(false, root, atom, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    let values: Vec<u32> = reply.value32()?.collect();
    Some(values)
}

/// What an output shows. `None` when a per-monitor override owns the output,
/// which is then left alone. Otherwise the workspace's wallpaper when there is
/// one, else the default (the scheduled or auto-cycled one, or the main one).
pub fn resolve<'a>(
    monitor: Option<&Wallpaper>,
    workspace: Option<&'a Wallpaper>,
    default: &'a Wallpaper,
) -> Option<&'a Wallpaper> {
    if monitor.is_some() {
        return None;
    }
    Some(workspace.unwrap_or(default))
}

/// Whether a workspace poll is due. Always false with no workspaces, so an
/// unused feature costs nothing.
pub fn poll_due(workspaces: &BTreeMap<String, Wallpaper>, last: Instant, now: Instant) -> bool {
    !workspaces.is_empty() && now.duration_since(last) >= WAYLAND_POLL
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn wp(path: &str) -> Wallpaper {
        Wallpaper {
            path: Some(PathBuf::from(path)),
            ..Wallpaper::default()
        }
    }

    /// A short, unique dir under TMPDIR: socket paths must stay under ~108 bytes.
    fn scratch(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let dir = std::env::temp_dir().join(format!("fw{tag}{}{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn i3_frame_round_trips() {
        let frame = encode_i3(7, b"hello");
        assert_eq!(&frame[..6], b"i3-ipc");
        let (kind, body) = read_i3(&mut frame.as_slice()).unwrap();
        assert_eq!(kind, 7);
        assert_eq!(body, b"hello");
    }

    #[test]
    fn i3_rejects_bad_magic_and_oversize() {
        let mut bad = encode_i3(1, b"x");
        bad[0] = b'X';
        assert!(read_i3(&mut bad.as_slice()).is_err());
        let mut big = encode_i3(1, b"");
        big[6..10].copy_from_slice(&((MAX_REPLY as u32) + 1).to_ne_bytes());
        assert!(read_i3(&mut big.as_slice()).is_err());
    }

    #[test]
    fn sway_picks_focused_name() {
        let json = br#"[{"name":"1","focused":false},{"name":"web","focused":true}]"#;
        assert_eq!(sway_focused(json).as_deref(), Some("web"));
        assert_eq!(sway_focused(br#"[{"name":"1","focused":false}]"#), None);
        assert_eq!(sway_focused(b"not json"), None);
    }

    #[test]
    fn hyprland_reads_name() {
        assert_eq!(
            hypr_active(r#"{"id":3,"name":"dev","monitor":"DP-1"}"#).as_deref(),
            Some("dev")
        );
        assert_eq!(hypr_active("garbage"), None);
    }

    #[test]
    fn x11_index_is_one_based() {
        assert_eq!(x11_key(&[0]).as_deref(), Some("1"));
        assert_eq!(x11_key(&[3, 9]).as_deref(), Some("4"));
        assert_eq!(x11_key(&[]), None);
        assert_eq!(x11_key(&[u32::MAX]), None);
    }

    #[test]
    fn precedence_monitor_then_workspace_then_default() {
        let main = wp("/main.mp4");
        let ws = wp("/ws.mp4");
        let mon = wp("/mon.mp4");
        // Per-monitor override owns the output: left alone.
        assert!(resolve(Some(&mon), Some(&ws), &main).is_none());
        // Workspace beats the default.
        assert_eq!(resolve(None, Some(&ws), &main), Some(&ws));
        // No workspace wallpaper: the default (schedule/auto-cycle/main).
        assert_eq!(resolve(None, None, &main), Some(&main));
    }

    #[test]
    fn no_poll_without_workspaces() {
        let now = Instant::now();
        let long_ago = now - Duration::from_secs(60);
        assert!(!poll_due(&BTreeMap::new(), long_ago, now));
        let mut map = BTreeMap::new();
        map.insert("1".to_string(), wp("/a.mp4"));
        assert!(poll_due(&map, long_ago, now));
        assert!(!poll_due(&map, now, now));
    }

    #[test]
    fn sway_query_over_fake_socket() {
        let dir = scratch("s");
        let path = dir.join("sway.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let (kind, body) = read_i3(&mut conn).unwrap();
            assert_eq!(kind, SWAY_GET_WORKSPACES);
            assert!(body.is_empty());
            let reply = br#"[{"name":"1","focused":false},{"name":"3","focused":true}]"#;
            conn.write_all(&encode_i3(SWAY_GET_WORKSPACES, reply))
                .unwrap();
        });
        let key = Source::Sway(path).current_key();
        server.join().unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(key.as_deref(), Some("3"));
    }

    #[test]
    fn hyprland_query_over_fake_socket() {
        let dir = scratch("h");
        let path = dir.join(".socket.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            // Hyprland answers the request bytes without waiting for EOF; the
            // client never half-closes, so reading to EOF would deadlock.
            let mut buf = [0u8; 64];
            let n = conn.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"j/activeworkspace");
            // Hyprland writes the reply and closes the socket.
            conn.write_all(br#"{"id":2,"name":"code","monitor":"DP-1"}"#)
                .unwrap();
        });
        let key = Source::Hyprland(path).current_key();
        server.join().unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(key.as_deref(), Some("code"));
    }

    #[test]
    fn query_on_missing_socket_is_none() {
        let dir = scratch("m");
        let key = Source::Sway(dir.join("absent.sock")).current_key();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(key, None);
    }

    /// Headless X: `_NET_CURRENT_DESKTOP` = 2 reads back as desktop index 2,
    /// which `x11_key` turns into "3"; unset reads as `None`.
    /// Run: `xvfb-run -a cargo test --lib workspace -- --ignored`
    #[test]
    #[ignore = "needs an X server (xvfb-run)"]
    fn x11_current_desktop_round_trips_on_xvfb() {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
        use x11rb::wrapper::ConnectionExt as _;
        let (conn, n) = x11rb::connect(None).unwrap();
        let root = conn.setup().roots[n].root;
        let atom = conn
            .intern_atom(false, b"_NET_CURRENT_DESKTOP")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        assert_eq!(read_current_desktop(&conn, root, atom), None);
        conn.change_property32(PropMode::REPLACE, root, atom, AtomEnum::CARDINAL, &[2])
            .unwrap();
        conn.flush().unwrap();
        let got = read_current_desktop(&conn, root, atom).unwrap();
        assert_eq!(got, vec![2]);
        assert_eq!(x11_key(&got).as_deref(), Some("3"));
    }
}

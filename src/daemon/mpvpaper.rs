//! Wayland live wallpaper via the external `mpvpaper` process — **one process
//! per output**.
//!
//! On layer-shell compositors (wlroots / KDE / COSMIC) we cannot embed mpv into
//! a background surface the way the X11 backend embeds it into a `DESKTOP`
//! window. Instead we drive `mpvpaper` (layer-shell + EGL + libmpv) and steer it
//! over the embedded mpv JSON IPC socket. Each output gets its own mpvpaper and
//! its own socket `$XDG_RUNTIME_DIR/fresco/mpv-<connector>.sock`.
//!
//! `WaylandPlayer` exposes the **same `&self` control surface as the X11
//! `Player`** (load_path / set_paused / set_zoom_pan / set_gamma / apply_crop /
//! hwdec_current / load_failed), using `RefCell` interior mutability for its IPC
//! stream. That lets `PlayerHandle` (in `mod.rs`) drive one shared engine across
//! both backends with no per-call-site branching.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::config::{Fit, Kind, Scaling, Wallpaper};

/// One `mpvpaper` process for one output, plus a client for its mpv IPC socket.
pub struct WaylandPlayer {
    socket_path: PathBuf,
    /// hwdec read once at spawn (doesn't change mid-playback) so `hwdec_current`
    /// can stay `&self` without an IPC round-trip on every status poll.
    hwdec: Option<String>,
    inner: RefCell<Inner>,
}

struct Inner {
    child: Child,
    ipc: MpvIpc,
}

impl WaylandPlayer {
    /// Spawn `mpvpaper <connector> <file>` and connect to its mpv IPC socket.
    /// `file` is the initial media (for a slideshow, the first image — later ones
    /// arrive via `loadfile replace`); the caller resolves it so folder-based
    /// slideshows work too.
    pub fn spawn(
        connector: &str,
        wallpaper: &Wallpaper,
        scaling: Scaling,
        file: &Path,
    ) -> Result<WaylandPlayer> {
        let dir = crate::ipc::socket_dir();
        std::fs::create_dir_all(&dir).ok();
        let socket_path = dir.join(format!("mpv-{}.sock", sanitize(connector)));
        std::fs::remove_file(&socket_path).ok();

        let opts = build_mpv_opts(wallpaper, scaling, &socket_path);
        let bin = crate::mpvpaper_command();
        log::info!(
            "[{connector}] spawning {} -o \"{opts}\" {connector} {}",
            bin.to_string_lossy(),
            file.display()
        );

        let child = Command::new(&bin)
            .arg("-o")
            .arg(&opts)
            .arg(connector)
            .arg(file)
            .spawn()
            .with_context(|| {
                format!(
                    "failed to start mpvpaper at {} for output {connector} — is it bundled next to frescod?",
                    bin.to_string_lossy()
                )
            })?;

        let mut child = child;
        let mut ipc = MpvIpc::new(socket_path.clone());
        // Wait for the IPC socket, but fast-fail if mpvpaper exits first (e.g. a
        // broken GL/EGL stack after a driver update) instead of blocking ~5s.
        let mut connected = false;
        for _ in 0..50 {
            if let Ok(Some(status)) = child.try_wait() {
                std::fs::remove_file(&socket_path).ok();
                return Err(anyhow!(
                    "mpvpaper for {connector} exited immediately ({status})"
                ));
            }
            if ipc.connect_retry(1).is_ok() {
                connected = true;
                break;
            }
        }
        if !connected {
            let _ = child.kill();
            let _ = child.wait();
            std::fs::remove_file(&socket_path).ok();
            return Err(anyhow!("mpv IPC for {connector} never came up"));
        }
        let hwdec = ipc.get("hwdec-current");

        let player = WaylandPlayer {
            socket_path,
            hwdec,
            inner: RefCell::new(Inner { child, ipc }),
        };
        // Crop is a runtime property (matches the X11 Player: post-init).
        player.apply_crop(wallpaper);
        // Playlist: queue the remaining files after the first.
        if wallpaper.kind == Kind::Playlist {
            for p in wallpaper.paths.iter().skip(1) {
                player.command(&[
                    json!("loadfile"),
                    json!(p.to_string_lossy().as_ref()),
                    json!("append"),
                ]);
            }
        }
        Ok(player)
    }

    /// True while the mpvpaper process is still running.
    pub fn is_alive(&self) -> bool {
        matches!(self.inner.borrow_mut().child.try_wait(), Ok(None))
    }

    // ── control surface (mirrors the X11 Player; all &self) ──────────────────

    pub fn load_path(&self, path: &Path) {
        self.command(&[
            json!("loadfile"),
            json!(path.to_string_lossy().as_ref()),
            json!("replace"),
        ]);
    }

    pub fn apply_crop(&self, wallpaper: &Wallpaper) {
        match wallpaper.crop.and_then(|c| c.sanitized()) {
            Some(crop) => {
                let (zoom, pan_x, pan_y) = crop.to_mpv_zoom_pan();
                self.set("video-zoom", json!(zoom));
                self.set("video-pan-x", json!(pan_x));
                self.set("video-pan-y", json!(pan_y));
                self.set("panscan", json!(0.0));
            }
            None => {
                self.set("video-zoom", json!(0));
                self.set("video-pan-x", json!(0));
                self.set("video-pan-y", json!(0));
            }
        }
    }

    pub fn set_gamma(&self, gamma: i32) {
        self.set("gamma", json!(gamma));
    }

    pub fn set_zoom_pan(&self, zoom: f64, pan_x: f64, pan_y: f64) {
        self.set("video-zoom", json!(zoom));
        self.set("video-pan-x", json!(pan_x));
        self.set("video-pan-y", json!(pan_y));
    }

    pub fn set_paused(&self, paused: bool) {
        self.set("pause", json!(paused));
    }

    /// Active hardware decoder, e.g. "vaapi" / "nvdec" / "no" — cached at spawn.
    pub fn hwdec_current(&self) -> Option<String> {
        self.hwdec.clone()
    }

    /// The supervisor tracks real renderer failures via the process exit, so the
    /// per-frame "load failed" notion the X11 path uses is always false here.
    pub fn load_failed(&self) -> bool {
        false
    }

    fn command(&self, args: &[Value]) {
        let _ = self.inner.borrow_mut().ipc.command(args);
    }

    fn set(&self, name: &str, value: Value) {
        self.inner.borrow_mut().ipc.set(name, value);
    }
}

impl Drop for WaylandPlayer {
    fn drop(&mut self) {
        // Killing the process drops its layer surface; the compositor reaps it.
        let mut inner = self.inner.borrow_mut();
        let _ = inner.child.kill();
        let _ = inner.child.wait();
        std::fs::remove_file(&self.socket_path).ok();
    }
}

/// Make a connector name safe for a socket filename ("DP-1" → "DP-1").
fn sanitize(connector: &str) -> String {
    connector
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Build the space-separated mpv option string passed to `mpvpaper -o`. Mirrors
/// the options the X11 `Player` sets (minus `wid`/`vo`, which mpvpaper owns).
fn build_mpv_opts(w: &Wallpaper, scaling: Scaling, sock: &Path) -> String {
    // NOTE: do not pass `background=#000000` — mpvpaper forwards `-o` options
    // through an mpv config file, where `#` begins a comment, so the value is
    // truncated and mpv rejects it. mpv's default letterbox background is black.
    let mut o: Vec<String> = vec![
        format!("input-ipc-server={}", sock.display()),
        "hwdec=auto-safe".into(),
        "image-display-duration=inf".into(),
    ];
    if w.kind == Kind::Playlist && w.paths.len() > 1 {
        o.push("loop-playlist=inf".into());
    } else {
        o.push("loop-file=inf".into());
    }
    if w.mute {
        o.push("mute=yes".into());
        o.push("aid=no".into());
    } else {
        o.push("mute=no".into());
        o.push(format!("volume={}", w.volume));
    }
    match w.fit {
        Fit::Cover => {
            o.push("keepaspect=yes".into());
            o.push("panscan=1.0".into());
        }
        Fit::Contain => {
            o.push("keepaspect=yes".into());
            o.push("panscan=0.0".into());
        }
        Fit::Stretch => o.push("keepaspect=no".into()),
    }
    if matches!(scaling, Scaling::High) {
        o.push("scale=lanczos".into());
        o.push("cscale=lanczos".into());
    }
    o.join(" ")
}

/// Minimal client for mpv's JSON IPC (`--input-ipc-server`). Reconnects on
/// failure; matches replies by `request_id` so async events are ignored.
struct MpvIpc {
    path: PathBuf,
    stream: Option<UnixStream>,
    next_id: i64,
}

impl MpvIpc {
    fn new(path: PathBuf) -> MpvIpc {
        MpvIpc {
            path,
            stream: None,
            next_id: 1,
        }
    }

    /// Connect, retrying every 100ms up to `attempts` times.
    fn connect_retry(&mut self, attempts: u32) -> Result<()> {
        for _ in 0..attempts {
            if let Ok(s) = UnixStream::connect(&self.path) {
                let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
                let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                self.stream = Some(s);
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(anyhow!(
            "mpv IPC socket {} never appeared",
            self.path.display()
        ))
    }

    /// Send `["cmd", arg, ...]` and return the reply matching our request_id.
    fn command(&mut self, args: &[Value]) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({ "command": args, "request_id": id });
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');

        for _ in 0..2 {
            if self.stream.is_none() && self.connect_retry(1).is_err() {
                continue;
            }
            // Owned clone so we don't borrow self across the blocking I/O.
            let sock = match self.stream.as_ref().map(|s| s.try_clone()) {
                Some(Ok(s)) => s,
                _ => {
                    self.stream = None;
                    continue;
                }
            };
            if (&sock).write_all(line.as_bytes()).is_err() {
                self.stream = None;
                continue;
            }
            let mut reader = BufReader::new(&sock);
            let mut found = None;
            // Skip async event lines until our reply (bounded).
            for _ in 0..64 {
                let mut buf = String::new();
                match reader.read_line(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(v) = serde_json::from_str::<Value>(buf.trim()) {
                            if v.get("request_id").and_then(Value::as_i64) == Some(id) {
                                found = Some(v);
                                break;
                            }
                        }
                    }
                    Err(_) => break, // read timeout / closed
                }
            }
            if let Some(v) = found {
                return Ok(v);
            }
            self.stream = None; // no reply → drop and retry once
        }
        Err(anyhow!("mpv IPC: no reply for request {id}"))
    }

    /// Fire-and-forget property set.
    fn set(&mut self, name: &str, value: Value) {
        let _ = self.command(&[json!("set_property"), json!(name), value]);
    }

    /// Read a property as a string (numbers/bools stringified); None on error.
    fn get(&mut self, name: &str) -> Option<String> {
        let v = self.command(&[json!("get_property"), json!(name)]).ok()?;
        match v.get("data")? {
            Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have(bin: &str) -> bool {
        std::env::var("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
            .unwrap_or(false)
    }

    /// Drives the exact IPC commands `WaylandPlayer` sends against a real mpv
    /// (mpvpaper embeds the same mpv). Proves the steering client end-to-end.
    #[test]
    fn ipc_steers_real_mpv() {
        if !have("mpv") {
            eprintln!("skip ipc_steers_real_mpv: mpv not installed");
            return;
        }
        let sock =
            std::env::temp_dir().join(format!("fresco-ipc-test-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&sock);

        let mut child = Command::new("mpv")
            .args([
                "--idle=yes",
                "--vo=null",
                "--ao=null",
                "--no-config",
                "--no-terminal",
                "--really-quiet",
            ])
            .arg(format!("--input-ipc-server={}", sock.display()))
            .spawn()
            .expect("spawn mpv");

        let mut ipc = MpvIpc::new(sock.clone());
        let connected = ipc.connect_retry(50);

        ipc.set("pause", json!(true));
        let paused = ipc.get("pause");
        ipc.set("volume", json!(50));
        ipc.set("video-zoom", json!(-0.5));
        ipc.set("video-pan-x", json!(0.1));
        let zoom = ipc.get("video-zoom"); // crop control (T4) must read back
        let idle = ipc.get("idle-active");

        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&sock);

        assert!(
            connected.is_ok(),
            "should connect to mpv IPC: {connected:?}"
        );
        assert_eq!(
            paused.as_deref(),
            Some("true"),
            "pause must round-trip via IPC (T3)"
        );
        assert_eq!(
            idle.as_deref(),
            Some("true"),
            "idle-active should read back"
        );
        assert!(
            zoom.as_deref()
                .map(|z| z.starts_with("-0.5"))
                .unwrap_or(false),
            "video-zoom (crop, T4) should read back ~-0.5, got {zoom:?}"
        );
    }

    const FAKE_MPVPAPER: &str = "#!/bin/sh\n\
opts=\"$2\"\n\
file=\"$4\"\n\
sock=\"\"\n\
for tok in $opts; do\n\
  case \"$tok\" in\n\
    input-ipc-server=*) sock=\"${tok#input-ipc-server=}\" ;;\n\
  esac\n\
done\n\
[ -n \"$FRESCO_TEST_PIDFILE\" ] && echo $$ > \"$FRESCO_TEST_PIDFILE\"\n\
exec mpv --idle=yes --vo=null --ao=null --no-config --no-terminal --really-quiet --input-ipc-server=\"$sock\" --loop-file=inf \"$file\"\n";

    /// Supervision primitives behind T6 (death detection) and T8 (graceful
    /// failure): no compositor needed — exercises the process layer directly
    /// with a fake mpvpaper that wraps a headless mpv.
    #[test]
    fn mpvpaper_supervision_primitives() {
        use std::os::unix::fs::PermissionsExt;

        let wp = Wallpaper {
            kind: Kind::Video,
            path: Some(std::env::temp_dir().join("fresco-none.mp4")),
            ..Default::default()
        };

        // T8: a missing backend binary fails gracefully (Err, never a panic).
        std::env::set_var("FRESCO_MPVPAPER", "/nonexistent/fresco/mpvpaper");
        assert!(
            WaylandPlayer::spawn(
                "HEADLESS-1",
                &wp,
                Scaling::Balanced,
                &std::env::temp_dir().join("fresco-none.mp4")
            )
            .is_err(),
            "spawn must fail gracefully when the backend binary is missing (T8)"
        );

        // T6: detect the backend dying. Needs mpv (the engine mpvpaper wraps).
        if !have("mpv") {
            eprintln!("skip T6 death-detection: mpv not installed");
            std::env::remove_var("FRESCO_MPVPAPER");
            return;
        }
        let id = std::process::id();
        let fake = std::env::temp_dir().join(format!("fresco-fake-mpvpaper-{id}.sh"));
        let pidfile = std::env::temp_dir().join(format!("fresco-fake-pid-{id}"));
        std::fs::write(&fake, FAKE_MPVPAPER).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("FRESCO_MPVPAPER", &fake);
        std::env::set_var("FRESCO_TEST_PIDFILE", &pidfile);

        let player = WaylandPlayer::spawn(
            "HEADLESS-1",
            &wp,
            Scaling::Balanced,
            &std::env::temp_dir().join("fresco-none.mp4"),
        )
        .expect("spawn the fake mpvpaper backend");
        assert!(
            player.is_alive(),
            "backend should be alive right after spawn"
        );

        // Simulate a crash: kill the backend process out from under us.
        let pid = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .to_string();
        let _ = Command::new("kill").arg("-9").arg(&pid).status();
        std::thread::sleep(Duration::from_millis(600));
        assert!(
            !player.is_alive(),
            "supervisor must detect the backend death (basis of T6 restart)"
        );

        std::env::remove_var("FRESCO_MPVPAPER");
        std::env::remove_var("FRESCO_TEST_PIDFILE");
        let _ = std::fs::remove_file(&fake);
        let _ = std::fs::remove_file(&pidfile);
    }
}

//! System-audio capture — the monitor of the default sink, i.e. *what the user
//! is hearing* — as raw PCM for the audio visualiser widget (WIDGETS_ROADMAP W3).
//!
//! # Why a CLI shell-out and not a crate
//!
//! Same reasoning as `src/daemon/dde.rs`, which drives D-Bus through `gdbus`
//! instead of pulling in a D-Bus stack: a PipeWire/PulseAudio client crate would
//! add a large C-linking dependency (and a second sound-server abstraction) to a
//! wallpaper daemon that needs exactly one thing — a stream of f32 samples. So
//! we spawn the tool the user's sound server already ships, pipe its stdout, and
//! decode raw little-endian f32 ourselves. Zero new dependencies, and the
//! capture runs in its own process: if it wedges, the daemon does not.
//!
//! # Privacy
//!
//! A monitor source carries **everything the user hears** — music, calls, videos,
//! notifications. That is why the roadmap requires this feature to be opt-in,
//! clearly labelled, and never on by default. This module cooperates:
//!
//! * nothing here starts until [`AudioCapture::start`] is called explicitly;
//! * [`AudioCapture::start`] logs one line naming the tool and the source, so an
//!   active capture is visible in the daemon log;
//! * the capture stream is named (`fresco` / `fresco-visualiser`), so it shows up
//!   under "Recording" in the user's sound settings while it is running;
//! * audio is only ever held in a small in-memory ring buffer. It is never
//!   written to disk, never sent anywhere, and the process is killed on `Drop`.
//!
//! # Verified behaviour (PipeWire 1.5.85 + pipewire-pulse, pactl 16.1)
//!
//! Both command lines below were checked against the running server, confirming
//! the stream was linked to the **default sink's monitor** and not to a
//! microphone. Two findings are load-bearing and easy to get wrong:
//!
//! 1. `pw-cat --record` **must** be given `--raw`. Without it, pw-cat writes a
//!    24-byte `.snd` container header before the samples, which shifts the whole
//!    f32 stream by two bytes and turns music into noise.
//! 2. `pw-cat --record` **must** be given `stream.capture.sink=true`. A record
//!    stream targeting a sink by name is *not* enough: measured here,
//!    `--target=<default sink>` alone linked to the microphone, as did an
//!    unmatched target. With the property set, an unknown/bogus target degrades
//!    to the default sink's monitor — never to an input device.
//!
//! For `parec` the equivalent guarantee is simply that we never spawn it without
//! `-d`: bare `parec` records the default *source* (the microphone), so the
//! device argument falls back to PulseAudio's `@DEFAULT_MONITOR@` alias, which
//! pipewire-pulse also implements.

use std::io::{ErrorKind, Read};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use anyhow::{bail, Context, Result};

/// Requested buffer latency. 20 ms is one visualiser frame at 50 Hz: small
/// enough that bars track transients, large enough that the reader thread wakes
/// ~50 times a second instead of thousands.
const LATENCY_MS: u32 = 20;

/// PulseAudio's alias for "the monitor of whatever the default sink is". Used
/// when `pactl` is unavailable so we still never fall through to a microphone.
const DEFAULT_MONITOR_ALIAS: &str = "@DEFAULT_MONITOR@";

/// Extra PipeWire stream properties for `pw-cat`.
///
/// `stream.capture.sink=true` is the flag that makes this a *monitor* capture
/// (see the module docs — without it pw-cat records the microphone). The names
/// make the stream identifiable in `pavucontrol` / GNOME sound settings while it
/// is recording, which the privacy requirement asks for.
const PW_PROPERTIES: &str =
    "{ stream.capture.sink=true node.name=fresco-visualiser media.name=\"Fresco audio visualiser\" }";

/// Bytes read from the child per `read` call. One 8 KiB read is 1024 stereo f32
/// frames — about 21 ms at 48 kHz, so the reader thread is not woken per-sample.
const READ_CHUNK: usize = 8192;

/// The external tool used to capture PCM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTool {
    /// PipeWire's `pw-cat` (package `pipewire-bin` / `pipewire-utils`).
    PwCat,
    /// PulseAudio's `parec`, which pipewire-pulse also serves (package
    /// `pulseaudio-utils`).
    Parec,
}

impl CaptureTool {
    /// The binary name looked up on `PATH`.
    pub fn binary(self) -> &'static str {
        match self {
            CaptureTool::PwCat => "pw-cat",
            CaptureTool::Parec => "parec",
        }
    }

    /// What to tell the user to install when neither tool is present.
    fn package_hint(self) -> &'static str {
        match self {
            CaptureTool::PwCat => "pipewire-bin (PipeWire)",
            CaptureTool::Parec => "pulseaudio-utils (PulseAudio / pipewire-pulse)",
        }
    }
}

impl std::fmt::Display for CaptureTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.binary())
    }
}

/// Parse a `FRESCO_AUDIO_TOOL` value. Unknown/empty values mean "no override".
fn parse_tool(s: &str) -> Option<CaptureTool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "pw-cat" | "pwcat" | "pipewire" => Some(CaptureTool::PwCat),
        "parec" | "pulse" | "pulseaudio" => Some(CaptureTool::Parec),
        _ => None,
    }
}

/// True when `bin` is an executable file on `PATH`.
fn have(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

/// The capture tool to use, or `None` when neither is installed — in which case
/// the caller should disable the widget with a clear message rather than let the
/// visualiser fail mysteriously.
///
/// `pw-cat` is preferred: it is the modern stack and it works on PipeWire-only
/// systems that never installed the Pulse compatibility tools. `parec` covers
/// plain PulseAudio (and PipeWire boxes where only `pulseaudio-utils` is
/// present). `FRESCO_AUDIO_TOOL=pw-cat|parec` forces one, mirroring
/// `FRESCO_DDE_MODE`; an override naming a tool that is not installed is ignored
/// rather than obeyed into a guaranteed failure.
pub fn detect_tool() -> Option<CaptureTool> {
    if let Ok(v) = std::env::var("FRESCO_AUDIO_TOOL") {
        match parse_tool(&v) {
            Some(t) if have(t.binary()) => {
                log::info!("audio: FRESCO_AUDIO_TOOL={v} selects {t}");
                return Some(t);
            }
            Some(t) => log::warn!("audio: FRESCO_AUDIO_TOOL={v} requests {t}, which is not installed — detecting instead"),
            None => log::warn!("audio: ignoring invalid FRESCO_AUDIO_TOOL={v:?} (want pw-cat|parec)"),
        }
    }
    [CaptureTool::PwCat, CaptureTool::Parec]
        .into_iter()
        .find(|t| have(t.binary()))
}

/// Run a command and return its stdout when it exits successfully.
fn run(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The `pactl get-default-sink` output as a sink name.
///
/// `pactl` prints exactly one token; anything else (empty output, a `Failure:`
/// message that still exited 0, a multi-word line) is rejected so we never build
/// a nonsense device name out of it.
fn parse_default_sink(stdout: &str) -> Option<String> {
    let name = stdout.trim();
    if name.is_empty() || name.split_whitespace().count() != 1 {
        return None;
    }
    Some(name.to_string())
}

/// Pick a monitor source out of `pactl list short sources`.
///
/// The columns are index, name, driver, sample spec, state. A source whose name
/// ends in `.monitor` is the loopback of a sink, and a `RUNNING` one is a sink
/// that is actually playing right now — exactly what a visualiser wants when the
/// default sink could not be determined.
fn parse_monitor_from_sources(stdout: &str) -> Option<String> {
    let rank = |state: &str| match state.trim().to_ascii_uppercase().as_str() {
        "RUNNING" => 2,
        "IDLE" => 1,
        _ => 0, // SUSPENDED, or a pactl that prints no state column
    };
    let mut best: Option<(u8, String)> = None;
    for line in stdout.lines() {
        let mut cols = line.split_whitespace();
        // Column 0 is the index; a blank or truncated line is skipped, never
        // fatal — one odd line must not abandon the whole scan.
        let (Some(_index), Some(name)) = (cols.next(), cols.next()) else {
            continue;
        };
        if !name.ends_with(".monitor") {
            continue;
        }
        let score = rank(cols.next_back().unwrap_or(""));
        if best.as_ref().is_none_or(|(b, _)| score > *b) {
            best = Some((score, name.to_string()));
        }
    }
    best.map(|(_, name)| name)
}

/// The monitor source of the **default sink** — the loopback of what is actually
/// playing, never an input device.
///
/// Returns `None` when `pactl` is absent or tells us nothing usable; capture
/// still works in that case (see `capture_args` — `pw-cat` follows the default
/// sink and `parec` uses `@DEFAULT_MONITOR@`), it just cannot be named.
///
/// **Privacy:** the returned source records *everything the user hears*. Nothing
/// in this module opens it on its own; that is [`AudioCapture::start`]'s job, and
/// the feature must stay opt-in.
pub fn default_monitor_source() -> Option<String> {
    if !have("pactl") {
        log::debug!("audio: pactl not installed — cannot name the monitor source");
        return None;
    }
    // Preferred: the monitor of the sink the server is actually using.
    if let Some(sink) = run("pactl", &["get-default-sink"])
        .as_deref()
        .and_then(parse_default_sink)
    {
        // Defensive: never build "x.monitor.monitor" if a server ever reports a
        // monitor here.
        if sink.ends_with(".monitor") {
            return Some(sink);
        }
        return Some(format!("{sink}.monitor"));
    }
    // Fallback for a pactl too old for `get-default-sink` (< 15), or a server
    // that failed the query: take a monitor from the source list.
    let listed = run("pactl", &["list", "short", "sources"])?;
    let found = parse_monitor_from_sources(&listed);
    if found.is_none() {
        log::debug!("audio: no monitor source found in `pactl list short sources`");
    }
    found
}

/// `pw-cat --target` matches a PipeWire **node name**, and sink nodes have no
/// `.monitor` suffix — that suffix is a PulseAudio-ism for the sink's loopback.
/// Passing the Pulse name matches nothing, and pw-cat then silently records the
/// default sink instead of the one asked for, so strip it.
fn pw_target(monitor: &str) -> &str {
    monitor.strip_suffix(".monitor").unwrap_or(monitor)
}

/// The full argument vector for `tool`, capturing f32 little-endian PCM on
/// stdout.
///
/// Both branches keep the invariant that makes this safe to ship: the stream can
/// only ever come from a **sink monitor**. See the module docs for the measured
/// behaviour behind `stream.capture.sink=true` (pw-cat) and the mandatory `-d`
/// (parec).
fn capture_args(
    tool: CaptureTool,
    sample_rate: u32,
    channels: u16,
    monitor: Option<&str>,
) -> Vec<String> {
    match tool {
        CaptureTool::PwCat => {
            let mut args = vec![
                "--record".to_string(),
                // Mandatory: without it pw-cat prefixes a 24-byte .snd header.
                "--raw".to_string(),
                "--format=f32".to_string(),
                format!("--rate={sample_rate}"),
                format!("--channels={channels}"),
                format!("--latency={LATENCY_MS}ms"),
                // Mandatory: this is what makes it a monitor capture.
                "-P".to_string(),
                PW_PROPERTIES.to_string(),
            ];
            if let Some(m) = monitor {
                args.push(format!("--target={}", pw_target(m)));
            }
            // "-" is pw-cat's spelling of stdout.
            args.push("-".to_string());
            args
        }
        CaptureTool::Parec => vec![
            "--raw".to_string(),
            "--format=float32le".to_string(),
            format!("--rate={sample_rate}"),
            format!("--channels={channels}"),
            format!("--latency-msec={LATENCY_MS}"),
            "--client-name=fresco".to_string(),
            // Never omit -d: bare parec records the default *source* (the mic).
            "-d".to_string(),
            monitor.unwrap_or(DEFAULT_MONITOR_ALIAS).to_string(),
        ],
    }
}

/// Decode as many whole little-endian f32 samples as `chunk` allows, appending
/// them to `out`, and carry the trailing 1–3 bytes in `pending` for the next
/// call. Returns the number of samples appended.
///
/// A pipe read is free to return *any* byte count, so a naive
/// `chunks_exact(4)` per read would drop 1–3 bytes each time and shift the whole
/// stream — the classic bug in this kind of code. `pending` is what keeps the
/// f32 boundary aligned across reads.
pub(crate) fn bytes_to_f32_le(pending: &mut Vec<u8>, chunk: &[u8], out: &mut Vec<f32>) -> usize {
    let before = out.len();
    let mut rest = chunk;

    // Finish the sample left half-read by the previous call first.
    if !pending.is_empty() {
        let need = 4 - pending.len();
        let take = need.min(rest.len());
        pending.extend_from_slice(&rest[..take]);
        rest = &rest[take..];
        if pending.len() == 4 {
            let mut word = [0u8; 4];
            word.copy_from_slice(pending);
            out.push(f32::from_le_bytes(word));
            pending.clear();
        }
    }

    let whole = rest.len() - rest.len() % 4;
    out.extend(
        rest[..whole]
            .chunks_exact(4)
            .map(|w| f32::from_le_bytes([w[0], w[1], w[2], w[3]])),
    );
    pending.extend_from_slice(&rest[whole..]);

    out.len() - before
}

/// Average every **complete** interleaved frame in `interleaved` into `out`,
/// removing the consumed samples and leaving any partial frame behind. Returns
/// the number of mono samples appended.
///
/// Downmixing here (in the reader thread) rather than at read time means the
/// ring buffer holds one number per frame: "the latest N samples" then means the
/// same thing whatever the channel count, and a frame can never be split across
/// the ring's wraparound.
pub(crate) fn drain_mono_frames(
    interleaved: &mut Vec<f32>,
    channels: usize,
    out: &mut Vec<f32>,
) -> usize {
    let channels = channels.max(1);
    let frames = interleaved.len() / channels;
    if frames == 0 {
        return 0;
    }
    let consumed = frames * channels;
    let scale = 1.0 / channels as f32;
    out.extend(
        interleaved[..consumed]
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() * scale),
    );
    interleaved.drain(..consumed);
    frames
}

/// Fixed-size circular buffer of mono samples. Writes never allocate and never
/// block the producer; old audio is simply overwritten.
struct Ring {
    buf: Vec<f32>,
    /// Where the next sample goes.
    head: usize,
    /// Total samples ever pushed, including those already overwritten. Monotonic,
    /// so a caller can tell "no new audio" from "same-looking audio".
    written: u64,
}

impl Ring {
    fn new(capacity: usize) -> Ring {
        Ring {
            buf: vec![0.0; capacity.max(1)],
            head: 0,
            written: 0,
        }
    }

    /// How many real samples can currently be read back.
    fn available(&self) -> usize {
        (self.written.min(self.buf.len() as u64)) as usize
    }

    fn push(&mut self, samples: &[f32]) {
        let cap = self.buf.len();
        self.written += samples.len() as u64;
        // Only the newest `cap` samples can survive, so a burst larger than the
        // ring costs one copy rather than one lap per capacity.
        let tail = &samples[samples.len().saturating_sub(cap)..];
        if tail.is_empty() {
            return;
        }
        let first = (cap - self.head).min(tail.len());
        self.buf[self.head..self.head + first].copy_from_slice(&tail[..first]);
        if tail.len() > first {
            self.buf[..tail.len() - first].copy_from_slice(&tail[first..]);
        }
        self.head = (self.head + tail.len()) % cap;
    }

    /// Copy the most recent `out.len()` samples into `out`, oldest first, so
    /// `out[n - 1]` is the newest sample. Returns how many were written; fewer
    /// than `out.len()` means that is all the audio there is.
    fn read_latest(&self, out: &mut [f32]) -> usize {
        let cap = self.buf.len();
        let n = out.len().min(self.available());
        if n == 0 {
            return 0;
        }
        let start = (self.head + cap - n) % cap;
        let first = (cap - start).min(n);
        out[..first].copy_from_slice(&self.buf[start..start + first]);
        if n > first {
            out[first..n].copy_from_slice(&self.buf[..n - first]);
        }
        n
    }
}

/// State shared with the reader thread.
struct Shared {
    ring: Mutex<Ring>,
    error: Mutex<Option<String>>,
    /// Cleared by the reader thread when the stream ends, for any reason.
    alive: AtomicBool,
}

impl Shared {
    fn set_error(&self, msg: String) {
        *lock(&self.error) = Some(msg);
    }
}

/// Lock that survives a poisoned mutex.
///
/// The daemon must not die because a thread panicked while holding this lock,
/// and the data behind it is plain audio samples: the worst case is one glitchy
/// visualiser frame, which is strictly better than taking the wallpaper down.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A running capture: one child process, one reader thread, one ring buffer.
///
/// Dropping this **kills the child and joins the thread**, unconditionally.
pub struct AudioCapture {
    tool: CaptureTool,
    source: Option<String>,
    sample_rate: u32,
    channels: u16,
    pid: u32,
    child: Mutex<Child>,
    shared: Arc<Shared>,
    /// `None` only after [`AudioCapture::stop`] has joined it.
    reader: Option<JoinHandle<()>>,
}

impl AudioCapture {
    /// Start capturing the default sink's monitor at `sample_rate` Hz with
    /// `channels` channels, decoded to mono.
    ///
    /// Fails with an actionable message when no capture tool is installed, when
    /// the parameters are out of range, or when the tool cannot be executed.
    ///
    /// The spawn itself does not prove the stream came up (the server may refuse
    /// it a moment later), so a caller should check [`AudioCapture::is_alive`]
    /// after a beat and, if it is false, back off before retrying — optionally
    /// through [`AudioCapture::start_with`] using the other tool.
    ///
    /// **Privacy:** this opens a stream carrying everything the user hears. Only
    /// call it once the user has opted in.
    pub fn start(sample_rate: u32, channels: u16) -> Result<AudioCapture> {
        let tool = detect_tool().with_context(|| {
            format!(
                "no system-audio capture tool found: install {} or {}",
                CaptureTool::PwCat.package_hint(),
                CaptureTool::Parec.package_hint()
            )
        })?;
        AudioCapture::start_with(tool, sample_rate, channels)
    }

    /// [`AudioCapture::start`] with the backend chosen explicitly — used to fall
    /// back to the other tool after a failed attempt.
    pub fn start_with(tool: CaptureTool, sample_rate: u32, channels: u16) -> Result<AudioCapture> {
        if !(8_000..=192_000).contains(&sample_rate) {
            bail!("audio capture: sample rate {sample_rate} Hz is out of range (8000..=192000)");
        }
        if !(1..=8).contains(&channels) {
            bail!("audio capture: channel count {channels} is out of range (1..=8)");
        }
        if !have(tool.binary()) {
            bail!(
                "audio capture: `{tool}` is not installed — install {}",
                tool.package_hint()
            );
        }

        let source = default_monitor_source();
        let args = capture_args(tool, sample_rate, channels, source.as_deref());
        // One log line so an active capture of the user's audio is never silent.
        log::info!(
            "audio: capturing system output via `{tool} {}` (source: {})",
            args.join(" "),
            source.as_deref().unwrap_or("default sink monitor")
        );

        let mut child = Command::new(tool.binary())
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            // The tools are chatty on stderr and we never read it; an unread
            // pipe would eventually block the child, so drop it on the floor.
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("audio capture: failed to start `{tool}`"))?;
        let pid = child.id();

        let Some(stdout) = child.stdout.take() else {
            // Cannot happen with Stdio::piped(), but a leaked recorder is far
            // worse than a redundant kill.
            let _ = child.kill();
            let _ = child.wait();
            bail!("audio capture: `{tool}` gave us no stdout pipe");
        };

        // ~0.5 s of mono audio: enough history for any FFT window the visualiser
        // asks for, small enough that a stalled reader can never hand back audio
        // from minutes ago (24k samples ≈ 94 KiB at 48 kHz).
        let capacity = (sample_rate as usize / 2).clamp(4096, 96_000);
        let shared = Arc::new(Shared {
            ring: Mutex::new(Ring::new(capacity)),
            error: Mutex::new(None),
            alive: AtomicBool::new(true),
        });

        let worker = Arc::clone(&shared);
        let reader = std::thread::Builder::new()
            .name("fresco-audio".to_string())
            .spawn(move || reader_loop(stdout, channels as usize, &worker));
        let reader = match reader {
            Ok(handle) => handle,
            Err(e) => {
                // No thread means nobody would ever close this pipe.
                let _ = child.kill();
                let _ = child.wait();
                return Err(e).context("audio capture: could not spawn the reader thread");
            }
        };

        Ok(AudioCapture {
            tool,
            source,
            sample_rate,
            channels,
            pid,
            child: Mutex::new(child),
            shared,
            reader: Some(reader),
        })
    }

    /// Copy the most recent `out.len()` mono samples into `out`, oldest first,
    /// and return how many were written (0 before any audio has arrived, fewer
    /// than requested while the ring is still filling).
    ///
    /// Never blocks on the child: the only lock taken is the ring's, which the
    /// reader thread holds for a `memcpy` at a time.
    pub fn read_latest(&self, out: &mut [f32]) -> usize {
        lock(&self.shared.ring).read_latest(out)
    }

    /// Total samples captured since [`AudioCapture::start`]. Monotonic — compare
    /// it between frames to tell "the sink went quiet" from "the same waveform
    /// again", without timing anything.
    pub fn samples_captured(&self) -> u64 {
        lock(&self.shared.ring).written
    }

    /// True while the capture process is still streaming.
    ///
    /// Goes false when the child exits for any reason — the sink was switched,
    /// PipeWire restarted, the user revoked access — so a supervisor can back off
    /// and retry on a timer instead of spinning. Also reaps the exit status, so a
    /// dead capture leaves no zombie behind.
    pub fn is_alive(&self) -> bool {
        if !self.shared.alive.load(Ordering::Relaxed) {
            return false;
        }
        match lock(&self.child).try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                self.shared.alive.store(false, Ordering::Relaxed);
                self.shared
                    .set_error(format!("`{}` exited: {status}", self.tool));
                false
            }
            Err(e) => {
                self.shared.alive.store(false, Ordering::Relaxed);
                self.shared
                    .set_error(format!("waiting on `{}` failed: {e}", self.tool));
                false
            }
        }
    }

    /// The last capture error, if any — for the status line and logs.
    pub fn last_error(&self) -> Option<String> {
        lock(&self.shared.error).clone()
    }

    /// The backend in use.
    pub fn tool(&self) -> CaptureTool {
        self.tool
    }

    /// The monitor source being captured, when it could be named. Show this to
    /// the user: it is the honest answer to "what is Fresco listening to?".
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// The capture rate in Hz — the FFT needs it to map bins to frequencies.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Channels requested from the server. Samples come out of
    /// [`AudioCapture::read_latest`] already downmixed to mono.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// The capture process id, for diagnostics and for tests that assert it is
    /// really gone.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Kill the capture process and join the reader thread. Idempotent, and
    /// called for you by `Drop`.
    ///
    /// Ordering matters: killing the child closes the write end of the pipe,
    /// which is what unblocks the reader thread's `read`, so the join below can
    /// never hang.
    pub fn stop(&mut self) {
        {
            let mut child = lock(&self.child);
            let _ = child.kill();
            let _ = child.wait();
        }
        self.shared.alive.store(false, Ordering::Relaxed);
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AudioCapture {
    /// Teardown is unconditional. A leaked `parec`/`pw-cat` would keep recording
    /// the user's audio for as long as the session lasts, so this must run on
    /// every path — including the error paths inside
    /// [`AudioCapture::start_with`], which kill the child directly because no
    /// `AudioCapture` exists yet to drop.
    ///
    /// If the daemon is `SIGKILL`ed and no destructor runs at all, the capture
    /// still cannot outlive it usefully: the read end of its stdout pipe dies
    /// with us, so the next block the tool writes raises `SIGPIPE` and takes it
    /// down. The kill here is what makes that immediate instead of eventual.
    fn drop(&mut self) {
        self.stop();
    }
}

impl std::fmt::Debug for AudioCapture {
    /// Deliberately describes the *stream*, never its contents: captured audio
    /// must not end up in a log line or a panic message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioCapture")
            .field("tool", &self.tool)
            .field("source", &self.source)
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .field("pid", &self.pid)
            .field("alive", &self.shared.alive.load(Ordering::Relaxed))
            .finish()
    }
}

/// Read PCM from the child until the stream ends, feeding the ring buffer.
///
/// Every exit path clears `alive`, so a caller polling
/// [`AudioCapture::is_alive`] learns about a dead capture whether the child
/// exited, closed the pipe, or the read failed outright. Nothing here can panic
/// the daemon: no unwrap, no indexing that can go out of range.
fn reader_loop(mut stdout: ChildStdout, channels: usize, shared: &Shared) {
    let mut raw = vec![0u8; READ_CHUNK];
    // Carries the 1–3 bytes of a half-read sample between reads.
    let mut pending: Vec<u8> = Vec::with_capacity(4);
    // Carries the samples of a half-read *frame* between reads.
    let mut interleaved: Vec<f32> = Vec::with_capacity(READ_CHUNK / 4 + channels);
    let mut mono: Vec<f32> = Vec::with_capacity(READ_CHUNK / 4);

    loop {
        match stdout.read(&mut raw) {
            // EOF: the child closed its stdout, i.e. it is gone.
            Ok(0) => break,
            Ok(n) => {
                bytes_to_f32_le(&mut pending, &raw[..n], &mut interleaved);
                mono.clear();
                drain_mono_frames(&mut interleaved, channels, &mut mono);
                if !mono.is_empty() {
                    lock(&shared.ring).push(&mono);
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                shared.set_error(format!("reading capture PCM failed: {e}"));
                break;
            }
        }
    }
    shared.alive.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// f32 samples as the little-endian bytes a capture tool would emit.
    fn to_bytes(samples: &[f32]) -> Vec<u8> {
        samples.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    // ── byte stream → f32 ────────────────────────────────────────────────────

    #[test]
    fn bytes_to_f32_exact_multiple() {
        let mut pending = Vec::new();
        let mut out = Vec::new();
        let n = bytes_to_f32_le(&mut pending, &to_bytes(&[1.0, -2.5, 0.0]), &mut out);
        assert_eq!(n, 3);
        assert_eq!(out, vec![1.0, -2.5, 0.0]);
        assert!(pending.is_empty(), "nothing should be left over");
    }

    /// The case the module exists to get right: a 6-byte chunk is one sample
    /// plus two bytes that belong to the *next* one.
    #[test]
    fn bytes_to_f32_six_bytes_keeps_two_pending() {
        let bytes = to_bytes(&[1.0, 2.0]);
        let mut pending = Vec::new();
        let mut out = Vec::new();

        assert_eq!(bytes_to_f32_le(&mut pending, &bytes[..6], &mut out), 1);
        assert_eq!(out, vec![1.0]);
        assert_eq!(pending.len(), 2);

        // The remaining two bytes complete the second sample, not a third.
        assert_eq!(bytes_to_f32_le(&mut pending, &bytes[6..], &mut out), 1);
        assert_eq!(out, vec![1.0, 2.0]);
        assert!(pending.is_empty());
    }

    #[test]
    fn bytes_to_f32_never_misaligns_across_odd_chunks() {
        let samples: Vec<f32> = (0..64).map(|i| i as f32 * 0.5 - 8.0).collect();
        let bytes = to_bytes(&samples);
        // Chunk sizes that are all coprime-ish with 4, plus a huge one.
        for step in [1usize, 2, 3, 5, 6, 7, 9, 13, 17, 4096] {
            let mut pending = Vec::new();
            let mut out = Vec::new();
            let mut total = 0;
            for chunk in bytes.chunks(step) {
                total += bytes_to_f32_le(&mut pending, chunk, &mut out);
                assert!(pending.len() < 4, "pending must never hold a whole sample");
            }
            assert_eq!(total, samples.len(), "step {step}");
            assert_eq!(out, samples, "step {step}");
            assert!(pending.is_empty(), "step {step}");
        }
    }

    #[test]
    fn bytes_to_f32_partial_then_empty_chunk() {
        let bytes = to_bytes(&[42.0]);
        let mut pending = Vec::new();
        let mut out = Vec::new();
        assert_eq!(bytes_to_f32_le(&mut pending, &bytes[..3], &mut out), 0);
        assert_eq!(bytes_to_f32_le(&mut pending, &[], &mut out), 0);
        assert_eq!(pending.len(), 3, "an empty read must not disturb the carry");
        assert_eq!(bytes_to_f32_le(&mut pending, &bytes[3..], &mut out), 1);
        assert_eq!(out, vec![42.0]);
    }

    #[test]
    fn bytes_to_f32_handles_pathological_floats() {
        let samples = [f32::MIN, f32::MAX, -0.0, f32::EPSILON];
        let mut pending = Vec::new();
        let mut out = Vec::new();
        bytes_to_f32_le(&mut pending, &to_bytes(&samples), &mut out);
        assert_eq!(out, samples);
        // NaN survives the round trip too, but must be compared by bits.
        let mut nan_out = Vec::new();
        bytes_to_f32_le(&mut pending, &to_bytes(&[f32::NAN]), &mut nan_out);
        assert!(nan_out[0].is_nan());
    }

    // ── downmix ──────────────────────────────────────────────────────────────

    #[test]
    fn downmix_stereo_averages_channels() {
        let mut interleaved = vec![1.0, 0.0, -1.0, 1.0, 0.5, 0.5];
        let mut out = Vec::new();
        assert_eq!(drain_mono_frames(&mut interleaved, 2, &mut out), 3);
        assert_eq!(out, vec![0.5, 0.0, 0.5]);
        assert!(interleaved.is_empty());
    }

    #[test]
    fn downmix_mono_is_passthrough() {
        let mut interleaved = vec![0.25, -0.75, 1.0];
        let mut out = Vec::new();
        assert_eq!(drain_mono_frames(&mut interleaved, 1, &mut out), 3);
        assert_eq!(out, vec![0.25, -0.75, 1.0]);
    }

    #[test]
    fn downmix_surround_averages_all_channels() {
        // 5.1: two frames of six channels.
        let mut interleaved = vec![6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let mut out = Vec::new();
        assert_eq!(drain_mono_frames(&mut interleaved, 6, &mut out), 2);
        assert_eq!(out, vec![1.0, 1.0]);
    }

    /// A read boundary can land in the middle of a frame; the leftover sample
    /// must join the *next* frame, not become one of its own.
    #[test]
    fn downmix_carries_partial_frame_across_chunks() {
        let mut interleaved = vec![1.0, 0.0, 2.0]; // 1.5 stereo frames
        let mut out = Vec::new();
        assert_eq!(drain_mono_frames(&mut interleaved, 2, &mut out), 1);
        assert_eq!(out, vec![0.5]);
        assert_eq!(interleaved, vec![2.0], "the odd sample stays pending");

        interleaved.push(4.0); // its partner arrives with the next read
        assert_eq!(drain_mono_frames(&mut interleaved, 2, &mut out), 1);
        assert_eq!(out, vec![0.5, 3.0]);
        assert!(interleaved.is_empty());
    }

    #[test]
    fn downmix_of_nothing_is_nothing() {
        let mut interleaved: Vec<f32> = vec![];
        let mut out = Vec::new();
        assert_eq!(drain_mono_frames(&mut interleaved, 2, &mut out), 0);
        // A single sample of a stereo frame is not yet a frame.
        interleaved.push(1.0);
        assert_eq!(drain_mono_frames(&mut interleaved, 2, &mut out), 0);
        assert!(out.is_empty());
        // A zero channel count must not divide by zero.
        assert_eq!(drain_mono_frames(&mut interleaved, 0, &mut out), 1);
    }

    // ── ring buffer ──────────────────────────────────────────────────────────

    #[test]
    fn ring_reads_back_what_was_written() {
        let mut ring = Ring::new(8);
        ring.push(&[1.0, 2.0, 3.0]);
        let mut out = [0.0; 3];
        assert_eq!(ring.read_latest(&mut out), 3);
        assert_eq!(out, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn ring_read_more_than_written() {
        let mut ring = Ring::new(16);
        ring.push(&[1.0, 2.0]);
        let mut out = [-9.0; 5];
        assert_eq!(ring.read_latest(&mut out), 2);
        assert_eq!(&out[..2], &[1.0, 2.0]);
        // Untouched tail: the caller decides whether to zero-fill.
        assert_eq!(&out[2..], &[-9.0, -9.0, -9.0]);

        // And nothing at all before any audio has arrived.
        let empty = Ring::new(16);
        assert_eq!(empty.read_latest(&mut out), 0);
    }

    #[test]
    fn ring_read_exactly_capacity() {
        let mut ring = Ring::new(4);
        ring.push(&[1.0, 2.0, 3.0, 4.0]);
        let mut out = [0.0; 4];
        assert_eq!(ring.read_latest(&mut out), 4);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(ring.available(), 4);
    }

    #[test]
    fn ring_wraps_around_and_keeps_the_newest() {
        let mut ring = Ring::new(4);
        ring.push(&[1.0, 2.0, 3.0]);
        ring.push(&[4.0, 5.0]); // wraps: 1.0 falls off the back
        let mut out = [0.0; 4];
        assert_eq!(ring.read_latest(&mut out), 4);
        assert_eq!(out, [2.0, 3.0, 4.0, 5.0]);

        // A partial read still ends at the newest sample.
        let mut two = [0.0; 2];
        assert_eq!(ring.read_latest(&mut two), 2);
        assert_eq!(two, [4.0, 5.0]);
    }

    #[test]
    fn ring_push_larger_than_capacity_keeps_the_tail() {
        let mut ring = Ring::new(4);
        let burst: Vec<f32> = (1..=10).map(|i| i as f32).collect();
        ring.push(&burst);
        let mut out = [0.0; 4];
        assert_eq!(ring.read_latest(&mut out), 4);
        assert_eq!(out, [7.0, 8.0, 9.0, 10.0]);
        assert_eq!(ring.written, 10, "the counter still sees every sample");
    }

    /// "Latest" must stay ordered oldest → newest across many laps, which is
    /// what makes the FFT window a real waveform instead of a shuffle.
    #[test]
    fn ring_latest_is_monotonic_across_laps() {
        let mut ring = Ring::new(7);
        let mut next = 0.0f32;
        for lap in 1..=20 {
            let chunk: Vec<f32> = (0..lap % 5 + 1)
                .map(|_| {
                    next += 1.0;
                    next
                })
                .collect();
            ring.push(&chunk);

            let mut out = [0.0; 7];
            let n = ring.read_latest(&mut out);
            assert_eq!(n, ring.available());
            // The window always ends at the newest sample and counts up by one.
            assert_eq!(out[n - 1], next);
            for i in 1..n {
                assert_eq!(out[i], out[i - 1] + 1.0, "lap {lap}");
            }
        }
        assert_eq!(ring.written, next as u64);
    }

    #[test]
    fn ring_zero_length_read_is_free() {
        let mut ring = Ring::new(4);
        ring.push(&[1.0]);
        assert_eq!(ring.read_latest(&mut []), 0);
        // Pushing nothing must not move the write head.
        ring.push(&[]);
        let mut out = [0.0; 1];
        assert_eq!(ring.read_latest(&mut out), 1);
        assert_eq!(out, [1.0]);
    }

    /// The whole PCM path end to end, on synthetic bytes: a 440 Hz stereo tone
    /// arriving in awkward chunk sizes comes back out as the mono waveform, in
    /// order and sample-exact.
    #[test]
    fn synthetic_tone_survives_the_whole_pipeline() {
        let rate = 48_000.0f32;
        let frames = 1024;
        let mut interleaved_src = Vec::new();
        let mut expected_mono = Vec::new();
        for i in 0..frames {
            let t = i as f32 / rate;
            let left = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            let right = left * 0.5;
            interleaved_src.push(left);
            interleaved_src.push(right);
            expected_mono.push((left + right) * 0.5);
        }
        let bytes = to_bytes(&interleaved_src);

        let mut ring = Ring::new(frames);
        let mut pending = Vec::new();
        let mut interleaved = Vec::new();
        let mut mono = Vec::new();
        for chunk in bytes.chunks(7) {
            bytes_to_f32_le(&mut pending, chunk, &mut interleaved);
            mono.clear();
            drain_mono_frames(&mut interleaved, 2, &mut mono);
            ring.push(&mono);
        }

        let mut out = vec![0.0; frames];
        assert_eq!(ring.read_latest(&mut out), frames);
        for (got, want) in out.iter().zip(&expected_mono) {
            assert!((got - want).abs() < 1e-6, "got {got}, want {want}");
        }
    }

    // ── command construction (the privacy invariants) ────────────────────────

    #[test]
    fn pw_cat_args_always_capture_a_sink_monitor() {
        let args = capture_args(
            CaptureTool::PwCat,
            48_000,
            2,
            Some("alsa_output.pci-0000_00_1f.3.analog-stereo.monitor"),
        );
        assert!(
            args.iter().any(|a| a.contains("stream.capture.sink=true")),
            "without this pw-cat records the MICROPHONE: {args:?}"
        );
        // Without --raw the stream is prefixed by a 24-byte .snd header.
        assert!(args.iter().any(|a| a == "--raw"), "{args:?}");
        assert!(args.iter().any(|a| a == "--record"), "{args:?}");
        assert!(args.iter().any(|a| a == "--format=f32"), "{args:?}");
        assert!(args.iter().any(|a| a == "--rate=48000"), "{args:?}");
        assert!(args.iter().any(|a| a == "--channels=2"), "{args:?}");
        // --target takes a PipeWire node name, so the Pulse ".monitor" suffix is
        // stripped; keeping it would silently capture the default sink instead.
        assert!(
            args.contains(&"--target=alsa_output.pci-0000_00_1f.3.analog-stereo".to_string()),
            "{args:?}"
        );
        assert_eq!(args.last().map(String::as_str), Some("-"), "stdout sink");
    }

    #[test]
    fn pw_cat_args_without_a_named_monitor_still_capture_a_sink() {
        let args = capture_args(CaptureTool::PwCat, 44_100, 1, None);
        assert!(args.iter().any(|a| a.contains("stream.capture.sink=true")));
        assert!(
            !args.iter().any(|a| a.starts_with("--target=")),
            "no target means 'follow the default sink': {args:?}"
        );
    }

    #[test]
    fn parec_args_never_open_the_default_source() {
        let named = capture_args(CaptureTool::Parec, 48_000, 2, Some("sink.monitor"));
        let d = named.iter().position(|a| a == "-d").expect("device flag");
        assert_eq!(named[d + 1], "sink.monitor");
        assert!(named.iter().any(|a| a == "--format=float32le"), "{named:?}");
        assert!(named.iter().any(|a| a == "--raw"), "{named:?}");

        // With no name we must still not fall through to the microphone.
        let anon = capture_args(CaptureTool::Parec, 48_000, 2, None);
        let d = anon.iter().position(|a| a == "-d").expect("device flag");
        assert_eq!(anon[d + 1], DEFAULT_MONITOR_ALIAS);
    }

    #[test]
    fn pw_target_strips_the_pulse_suffix_only() {
        assert_eq!(pw_target("a_sink.monitor"), "a_sink");
        assert_eq!(pw_target("a_sink"), "a_sink");
        assert_eq!(pw_target("monitor"), "monitor");
        assert_eq!(pw_target("a.monitor.b"), "a.monitor.b");
    }

    // ── source discovery ─────────────────────────────────────────────────────

    #[test]
    fn default_sink_parsing() {
        assert_eq!(
            parse_default_sink("alsa_output.pci-0000_00_1f.3.analog-stereo\n"),
            Some("alsa_output.pci-0000_00_1f.3.analog-stereo".to_string())
        );
        assert_eq!(
            parse_default_sink("   name  \n\n"),
            Some("name".to_string())
        );
        assert_eq!(parse_default_sink(""), None);
        assert_eq!(parse_default_sink("\n"), None);
        // Not a sink name — a server that answered with prose.
        assert_eq!(parse_default_sink("Failure: No such entity"), None);
    }

    #[test]
    fn monitor_selection_from_short_sources() {
        // Real `pactl list short sources` shape (tab separated).
        let listing = "\
59\talsa_output.hdmi.monitor\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED
62\talsa_output.speaker.monitor\tPipeWire\ts32le 2ch 48000Hz\tRUNNING
63\talsa_input.mic.source\tPipeWire\ts32le 2ch 48000Hz\tRUNNING
";
        // The RUNNING monitor wins — that is the sink actually playing.
        assert_eq!(
            parse_monitor_from_sources(listing),
            Some("alsa_output.speaker.monitor".to_string())
        );
        // Never an input device, whatever its state.
        assert_eq!(
            parse_monitor_from_sources("63\talsa_input.mic.source\tPipeWire\ts16le\tRUNNING\n"),
            None
        );
        assert_eq!(parse_monitor_from_sources(""), None);
        // Malformed lines must not panic or be mistaken for a source.
        assert_eq!(parse_monitor_from_sources("\n\n   \ngarbage\n"), None);
        // A listing with no state column still yields the monitor.
        assert_eq!(
            parse_monitor_from_sources("7\tx.monitor\n"),
            Some("x.monitor".to_string())
        );
        // IDLE beats SUSPENDED when nothing is RUNNING.
        assert_eq!(
            parse_monitor_from_sources("1\ta.monitor\td\ts\tSUSPENDED\n2\tb.monitor\td\ts\tIDLE\n"),
            Some("b.monitor".to_string())
        );
    }

    #[test]
    fn tool_override_parsing() {
        assert_eq!(parse_tool("pw-cat"), Some(CaptureTool::PwCat));
        assert_eq!(parse_tool(" PipeWire \n"), Some(CaptureTool::PwCat));
        assert_eq!(parse_tool("parec"), Some(CaptureTool::Parec));
        assert_eq!(parse_tool("PULSE"), Some(CaptureTool::Parec));
        assert_eq!(parse_tool(""), None);
        assert_eq!(parse_tool("arecord"), None);
    }

    // ── environment probes: must degrade, never panic ────────────────────────

    #[test]
    fn detection_degrades_gracefully() {
        // Whatever this machine has (or has not), neither call may panic.
        match detect_tool() {
            Some(tool) => assert!(have(tool.binary()), "detected a tool that is not on PATH"),
            None => assert!(!have("pw-cat") && !have("parec")),
        }
        assert!(!have("fresco-no-such-binary-xyz"));
    }

    /// Must answer on any machine — with or without pactl, with or without a
    /// sound server — and whatever it answers must be a monitor.
    #[test]
    fn monitor_source_is_a_monitor_or_nothing() {
        if let Some(source) = default_monitor_source() {
            assert!(source.ends_with(".monitor"), "not a monitor: {source}");
            assert!(!source.contains(char::is_whitespace), "{source}");
        }
    }

    #[test]
    fn start_rejects_impossible_parameters_without_spawning() {
        for rate in [0, 1, 7_999, 192_001, u32::MAX] {
            let err = AudioCapture::start_with(CaptureTool::PwCat, rate, 2)
                .expect_err("out-of-range rate must fail");
            assert!(format!("{err}").contains("sample rate"), "{err}");
        }
        for channels in [0, 9, u16::MAX] {
            let err = AudioCapture::start_with(CaptureTool::PwCat, 48_000, channels)
                .expect_err("out-of-range channel count must fail");
            assert!(format!("{err}").contains("channel count"), "{err}");
        }
    }

    #[test]
    fn start_reports_a_missing_tool_actionably() {
        // Only meaningful when the tool really is absent; where it exists, the
        // message is exercised by the parameter test above.
        if have(CaptureTool::Parec.binary()) {
            eprintln!("skip start_reports_a_missing_tool_actionably: parec is installed");
            return;
        }
        let err = AudioCapture::start_with(CaptureTool::Parec, 48_000, 2).expect_err("no parec");
        let msg = format!("{err}");
        assert!(msg.contains("parec") && msg.contains("install"), "{msg}");
    }

    /// End-to-end against the real sound server, self-skipping like
    /// `daemon::mpvpaper`'s mpv tests. Captures for a fraction of a second, then
    /// proves the child process is **gone** after `Drop` — a capture that outlived
    /// the daemon would be recording the user's audio indefinitely.
    #[test]
    fn live_capture_starts_and_leaves_nothing_behind() {
        let Some(tool) = detect_tool() else {
            eprintln!("skip live_capture: no pw-cat/parec installed");
            return;
        };
        let Ok(capture) = AudioCapture::start_with(tool, 48_000, 2) else {
            // No sound server in this environment (CI containers, TTY builds).
            eprintln!("skip live_capture: {tool} would not start");
            return;
        };
        let pid = capture.pid();
        assert_eq!(capture.tool(), tool);
        assert_eq!(capture.sample_rate(), 48_000);
        assert_eq!(capture.channels(), 2);
        assert!(std::path::Path::new(&format!("/proc/{pid}")).exists());

        // A reader larger than the whole ring (and than anything captured so
        // far) must still be safe and bounded.
        let mut big = vec![0.0f32; 96_000];
        assert!(capture.read_latest(&mut big) <= big.len());

        std::thread::sleep(std::time::Duration::from_millis(250));
        if capture.is_alive() {
            // Samples only flow while the sink is not suspended, so an idle
            // machine legitimately captures nothing; what must hold is that
            // whatever arrived is readable and bounded. Both reads race the
            // reader thread, so this is an inequality, not an equality.
            let mut window = [0.0f32; 1024];
            let n = capture.read_latest(&mut window);
            assert!(n <= window.len());
            assert!(n as u64 <= capture.samples_captured());
        } else {
            eprintln!(
                "live_capture: stream ended early: {:?}",
                capture.last_error()
            );
        }

        drop(capture);
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "capture process {pid} survived Drop — it would keep recording the user"
        );
    }

    /// `stop` is what `Drop` calls, so it has to be safe to call twice and it
    /// has to leave the handle looking dead.
    #[test]
    fn stop_is_idempotent() {
        let Some(tool) = detect_tool() else {
            eprintln!("skip stop_is_idempotent: no pw-cat/parec installed");
            return;
        };
        let Ok(mut capture) = AudioCapture::start_with(tool, 48_000, 2) else {
            eprintln!("skip stop_is_idempotent: {tool} would not start");
            return;
        };
        let pid = capture.pid();
        capture.stop();
        capture.stop();
        assert!(!capture.is_alive());
        assert_eq!(
            capture.read_latest(&mut [0.0; 8]),
            capture.samples_captured().min(8) as usize
        );
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }
}

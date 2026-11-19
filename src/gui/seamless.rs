//! "Make seamless loop": a re-encoded copy of a video whose last second blends
//! into its first, so the loop point stops snapping back to the start.
//!
//! The trick is a crossfade across the seam. With `D` seconds of crossfade and
//! a clip of length `L`, the output is the source minus its first `D` seconds
//! (the "main" part), and the last `D` seconds of that main part fade into the
//! original first `D` seconds (the "head"). Output frame 0 is source frame `D`,
//! and the output's final frame is (almost entirely) source frame `D`, so
//! looping the output jumps from a frame to its neighbour. The output is
//! `L - D` long.
//!
//! An audio track, if the source has one, is kept (AAC) and crossfaded across
//! the same seam. The copy is written next to the other
//! generated media under the library's `seamless/` directory, never beside the
//! user's file, and the original path is kept on the entry so it can be
//! reverted.
//!
//! As in [`super::preview_proxy`], the pure parts (the command line, the
//! duration and progress parsers) are at the top and testable without a
//! display. [`encode`] does the work, and [`spawn`] runs it on a thread and
//! reports progress back to the GTK side.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use super::library::library_dir;
use super::preview_proxy::{command_line, in_path, nice_available};

/// Length of the crossfade at the loop point, in seconds.
pub const CROSSFADE_S: f64 = 1.0;

/// Whether both `ffmpeg` and `ffprobe` are installed. The menu entry is hidden
/// without them.
pub fn available() -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| in_path("ffmpeg") && in_path("ffprobe"))
}

/// Where seamless copies live: `<library>/seamless/`.
pub fn seamless_dir() -> PathBuf {
    library_dir().join("seamless")
}

/// The copy for `src` made by entry `id`: `<stem>.<id>.seamless.mp4` in `dir`.
/// The id keeps two same-named files from different folders apart.
pub fn output_path(dir: &Path, src: &Path, id: &str) -> PathBuf {
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "video".to_string());
    dir.join(format!("{stem}.{id}.seamless.mp4"))
}

/// Why a seamless copy was not made.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// ffprobe could not tell how long the clip is.
    NoLength,
    /// The clip is shorter than the seam needs (`3 * CROSSFADE_S`).
    TooShort { len: f64, need: f64 },
    /// The user cancelled.
    Cancelled,
    /// ffmpeg failed; the last line it printed.
    Ffmpeg(String),
}

// ---------------------------------------------------------------------------
// Pure helpers: command line and parsers. No I/O.
// ---------------------------------------------------------------------------

/// The ffmpeg arguments that crossfade a `len`-second clip at `src` into `out`
/// with an `xfade`-second seam. See the module docs for the math: the crossfade
/// starts at `len - 2 * xfade` in the main part, so the result is `len - xfade`
/// long. Refuses clips shorter than `3 * xfade`, where the main part and the
/// head would overlap themselves.
pub fn seamless_args(
    len: f64,
    xfade: f64,
    has_audio: bool,
    src: &Path,
    out: &Path,
) -> Result<Vec<String>, Failure> {
    assert!(xfade > 0.0, "crossfade must be positive");
    if !len.is_finite() || len <= 0.0 {
        return Err(Failure::NoLength);
    }
    let need = 3.0 * xfade;
    if len < need {
        return Err(Failure::TooShort { len, need });
    }
    let d = format!("{xfade:.3}");
    let offset = format!("{:.3}", len - 2.0 * xfade);
    let mut filter = format!(
        "[0:v]split[a][b];\
         [a]trim=start={d},setpts=PTS-STARTPTS[main];\
         [b]trim=end={d},setpts=PTS-STARTPTS[head];\
         [main][head]xfade=transition=fade:duration={d}:offset={offset},format=yuv420p[v]"
    );
    if has_audio {
        filter.push_str(&format!(
            ";[0:a]atrim=start={d},asetpts=PTS-STARTPTS[am];\
             [1:a]atrim=end={d},asetpts=PTS-STARTPTS[ah];\
             [am][ah]acrossfade=d={d}[a]"
        ));
    }
    let s = |v: &str| v.to_string();
    let audio: Vec<String> = if has_audio {
        ["-map", "[a]", "-c:a", "aac", "-b:a", "160k"].map(s).into()
    } else {
        vec![s("-an")]
    };
    let mut v = vec![
        s("-nostdin"),
        s("-loglevel"),
        s("error"),
        s("-y"),
        s("-i"),
        src.to_string_lossy().into_owned(),
    ];
    if has_audio {
        // The audio head comes from a second read of the file: ffmpeg 6.1's
        // `acrossfade` yields nothing when one `asplit` feeds both inputs.
        v.extend([s("-i"), src.to_string_lossy().into_owned()]);
    }
    v.extend([
        s("-progress"),
        s("pipe:1"),
        s("-filter_complex"),
        filter,
        s("-map"),
        s("[v]"),
        s("-c:v"),
        s("libx264"),
        s("-preset"),
        s("medium"),
        s("-crf"),
        s("18"),
        s("-movflags"),
        s("+faststart"),
    ]);
    v.extend(audio);
    v.push(out.to_string_lossy().into_owned());
    Ok(v)
}

/// Parse `ffprobe -of csv=p=0 -show_entries format=duration` output.
pub fn parse_duration(text: &str) -> Option<f64> {
    let v: f64 = text.trim().parse().ok()?;
    (v.is_finite() && v > 0.0).then_some(v)
}

/// The encoded time in seconds from one `-progress pipe:1` line. ffmpeg writes
/// microseconds under both `out_time_us` and `out_time_ms` (the latter is a
/// long-standing misnomer), and `N/A` before the first frame.
pub fn parse_progress(line: &str) -> Option<f64> {
    let v = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))?;
    let us: f64 = v.trim().parse().ok()?;
    (us.is_finite() && us >= 0.0).then_some(us / 1_000_000.0)
}

/// Whether `path` is a seamless copy this module made: a file directly inside
/// [`seamless_dir`]. Revert and removal only ever delete files that pass, so an
/// original can never be deleted by mistake. The parent is canonicalised, so
/// `..` segments and a symlinked directory cannot slip a path through, and a
/// symlink as the file itself is refused.
pub fn is_generated(path: &Path) -> bool {
    is_inside(&seamless_dir(), path)
}

fn is_inside(root: &Path, path: &Path) -> bool {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return false;
    };
    let (Ok(parent), Ok(root)) = (parent.canonicalize(), root.canonicalize()) else {
        return false;
    };
    parent == root
        && std::fs::symlink_metadata(parent.join(name)).is_ok_and(|m| m.file_type().is_file())
}

// ---------------------------------------------------------------------------
// I/O: probing and encoding.
// ---------------------------------------------------------------------------

/// Whether the clip has an audio stream.
pub fn probe_has_audio(path: &Path) -> bool {
    Command::new("ffprobe")
        .stdin(Stdio::null())
        .args(["-v", "error", "-select_streams", "a", "-show_entries"])
        .args(["stream=codec_type", "-of", "csv=p=0"])
        .arg(path)
        .output()
        .is_ok_and(|o| o.status.success() && !o.stdout.iter().all(u8::is_ascii_whitespace))
}

/// The clip's length in seconds, from ffprobe.
pub fn probe_duration(path: &Path) -> Option<f64> {
    let out = Command::new("ffprobe")
        .stdin(Stdio::null())
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_duration(&String::from_utf8_lossy(&out.stdout))
}

/// Encode the seamless copy of `src` for entry `id` into `dir`, reporting
/// progress in `0..=1` through `progress`. Blocking; run it off the GTK thread.
///
/// Output is written to `<id>.part.mp4` and renamed only when ffmpeg succeeds,
/// so a cancelled or failed run never leaves a file that looks finished. Setting
/// `cancel` kills ffmpeg (through `nice`, which execs, so the pid is ffmpeg's
/// own) and removes the partial file.
pub fn encode(
    src: &Path,
    dir: &Path,
    id: &str,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f32),
) -> Result<PathBuf, Failure> {
    let len = probe_duration(src).ok_or(Failure::NoLength)?;
    std::fs::create_dir_all(dir).map_err(|e| Failure::Ffmpeg(e.to_string()))?;
    let part = dir.join(format!("{id}.part.mp4"));
    let finished = output_path(dir, src, id);
    let args = seamless_args(len, CROSSFADE_S, probe_has_audio(src), src, &part)?;
    let (program, argv) = command_line(nice_available(), &args);

    let mut child = Command::new(&program)
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failure::Ffmpeg(e.to_string()))?;
    let mut stderr = child.stderr.take();
    if let Some(stdout) = child.stdout.take() {
        let out_len = len - CROSSFADE_S;
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                break;
            }
            if let Some(secs) = parse_progress(&line) {
                progress((secs / out_len).clamp(0.0, 1.0) as f32);
            }
        }
    }
    let status = child.wait().map_err(|e| Failure::Ffmpeg(e.to_string()))?;
    if cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(&part);
        return Err(Failure::Cancelled);
    }
    let mut err = String::new();
    if let Some(s) = stderr.as_mut() {
        let _ = s.read_to_string(&mut err);
    }
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        let last = err.lines().last().unwrap_or("ffmpeg failed").to_string();
        return Err(Failure::Ffmpeg(last));
    }
    std::fs::rename(&part, &finished).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        Failure::Ffmpeg(e.to_string())
    })?;
    Ok(finished)
}

/// What the background job reports back to the GTK thread.
pub enum Event {
    Progress(f32),
    Done(Result<PathBuf, Failure>),
}

/// Run [`encode`] on a thread into `seamless_dir()`, returning the channel its
/// events arrive on. The job ends with exactly one `Done`.
pub fn spawn(src: PathBuf, id: String, cancel: Arc<AtomicBool>) -> async_channel::Receiver<Event> {
    let (tx, rx) = async_channel::unbounded();
    std::thread::spawn(move || {
        let result = encode(&src, &seamless_dir(), &id, &cancel, &mut |f| {
            let _ = tx.try_send(Event::Progress(f));
        });
        let _ = tx.try_send(Event::Done(result));
    });
    rx
}

/// Delete a seamless copy. Refuses anything outside [`seamless_dir`], so a
/// stale or mistaken path cannot reach the user's own file. Returns whether
/// a file was removed.
pub fn discard(generated: &Path) -> bool {
    is_generated(generated) && std::fs::remove_file(generated).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc(args: &[String]) -> &str {
        let i = args.iter().position(|a| a == "-filter_complex").unwrap();
        &args[i + 1]
    }

    #[test]
    fn seam_offset_is_len_minus_two_crossfades() {
        let a =
            seamless_args(6.0, 1.0, false, Path::new("/in.mp4"), Path::new("/out.mp4")).unwrap();
        let f = fc(&a);
        assert!(f.contains("offset=4.000"), "{f}");
        assert!(f.contains("trim=start=1.000"), "{f}");
        assert!(f.contains("trim=end=1.000"), "{f}");
        assert!(f.contains("duration=1.000"), "{f}");
        assert!(a.contains(&"-an".to_string()), "no audio stream, no audio");
        let w = seamless_args(6.0, 1.0, true, Path::new("/in.mp4"), Path::new("/out.mp4")).unwrap();
        assert!(fc(&w).contains("acrossfade=d=1.000"), "{}", fc(&w));
        assert!(w.contains(&"aac".to_string()) && !w.contains(&"-an".to_string()));
        assert_eq!(a.last().unwrap(), "/out.mp4");
    }

    #[test]
    fn refuses_clips_shorter_than_three_crossfades() {
        let p = Path::new("/in.mp4");
        assert_eq!(
            seamless_args(2.9, 1.0, false, p, p),
            Err(Failure::TooShort {
                len: 2.9,
                need: 3.0
            })
        );
        assert!(seamless_args(3.0, 1.0, false, p, p).is_ok());
        assert_eq!(
            seamless_args(f64::NAN, 1.0, false, p, p),
            Err(Failure::NoLength)
        );
    }

    #[test]
    fn parses_duration_and_progress() {
        assert_eq!(parse_duration("5.967000\n"), Some(5.967));
        assert_eq!(parse_duration("N/A\n"), None);
        assert_eq!(parse_duration("0\n"), None);
        assert_eq!(parse_progress("out_time_us=1500000"), Some(1.5));
        assert_eq!(parse_progress("out_time_ms=250000"), Some(0.25));
        assert_eq!(parse_progress("out_time_us=N/A"), None);
        assert_eq!(parse_progress("progress=continue"), None);
    }

    #[test]
    fn copies_go_to_the_generated_dir_and_originals_are_never_discarded() {
        let dir = Path::new("/lib/seamless");
        let out = output_path(dir, Path::new("/home/u/Videos/car.mp4"), "123-4");
        assert_eq!(out, Path::new("/lib/seamless/car.123-4.seamless.mp4"));
        assert!(!is_generated(Path::new("/home/u/Videos/car.mp4")));
        assert!(!discard(Path::new("/home/u/Videos/car.mp4")));
    }

    #[test]
    fn discard_check_rejects_traversal_and_symlinks() {
        use std::os::unix::fs::symlink;
        let base =
            std::env::temp_dir().join(format!("fresco-seamless-guard-{}", std::process::id()));
        let root = base.join("seamless");
        std::fs::create_dir_all(&root).unwrap();
        let mine = root.join("a.seamless.mp4");
        let orig = base.join("orig.mp4");
        std::fs::write(&mine, b"x").unwrap();
        std::fs::write(&orig, b"x").unwrap();
        assert!(is_inside(&root, &mine));
        assert!(!is_inside(&root, &orig));
        assert!(!is_inside(&root, &root.join("..").join("orig.mp4")));
        let link = root.join("link.mp4");
        symlink(&orig, &link).unwrap();
        assert!(!is_inside(&root, &link), "symlink file refused");
        let dirlink = base.join("dl");
        symlink(&base, &dirlink).unwrap();
        assert!(!is_inside(&root, &dirlink.join("orig.mp4")));
        std::fs::remove_dir_all(&base).ok();
    }

    /// Mean absolute difference of two equal-length gray frames, 0..=255.
    fn mad(a: &[u8], b: &[u8]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| (f64::from(*x) - f64::from(*y)).abs())
            .sum::<f64>()
            / a.len() as f64
    }

    /// Gray frames of `path`, decoded through ffmpeg, as one flat buffer.
    fn gray_frames(path: &Path) -> Vec<u8> {
        Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(path)
            .args(["-f", "rawvideo", "-pix_fmt", "gray", "-"])
            .output()
            .expect("ffmpeg runs")
            .stdout
    }

    #[test]
    fn e2e_output_is_one_second_shorter_and_its_seam_is_continuous() {
        if !available() {
            eprintln!("skipping: ffmpeg/ffprobe not installed");
            return;
        }
        const W: usize = 320;
        const H: usize = 240;
        const FRAME: usize = W * H;
        let dir = std::env::temp_dir().join(format!("fresco-seamless-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.mp4");
        let made = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            // testsrc with a brightness ramp (geq, N = frame number): the
            // first and last frames differ a lot, so a plain loop visibly
            // jumps, while neighbouring frames differ only a little. Unlike
            // `gradients`, this is deterministic.
            .arg("testsrc=duration=6:size=320x240:rate=15,geq=lum='clip(lum(X\\,Y)+1.5*N-60\\,0\\,255)':cb='cb(X\\,Y)':cr='cr(X\\,Y)'")
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&src)
            .status()
            .unwrap();
        assert!(made.success(), "test source generated");

        let len = probe_duration(&src).expect("probe source");
        assert!((len - 6.0).abs() < 0.1, "source length {len}");

        // Full run: progress is reported and the output lands under the name.
        let cancel = AtomicBool::new(false);
        let mut last_progress = 0.0f32;
        let out = encode(&src, &dir.join("out"), "e2e", &cancel, &mut |f| {
            last_progress = f;
        })
        .expect("encode");
        assert!(last_progress > 0.9, "progress reached {last_progress}");
        let out_len = probe_duration(&out).expect("probe output");
        eprintln!("source {len:.3}s -> output {out_len:.3}s");
        assert!(
            (out_len - (len - CROSSFADE_S)).abs() <= 0.1,
            "output {out_len}"
        );

        let src_px = gray_frames(&src);
        let out_px = gray_frames(&out);
        let (n_src, n_out) = (src_px.len() / FRAME, out_px.len() / FRAME);
        fn frame(px: &[u8], i: usize) -> &[u8] {
            &px[i * FRAME..(i + 1) * FRAME]
        }
        // Typical motion between neighbouring output frames.
        let mut steps: Vec<f64> = (0..n_out - 1)
            .map(|i| mad(frame(&out_px, i), frame(&out_px, i + 1)))
            .collect();
        steps.sort_by(f64::total_cmp);
        let typical = steps[steps.len() / 2];
        // The wrap: last output frame back to the first.
        let seam = mad(frame(&out_px, n_out - 1), frame(&out_px, 0));
        // What a plain loop of the source jumps across.
        let naive = mad(frame(&src_px, n_src - 1), frame(&src_px, 0));
        eprintln!(
            "frames: source {n_src}, output {n_out}; typical step {typical:.2}, \
             seam {seam:.2}, plain loop {naive:.2} (gray MAD, 0-255)"
        );
        // The seam keeps a faint ghost: the last output frame is crossfade
        // progress (D*fps - 1)/(D*fps), so about 1/fps of the far frame shows.
        // That is the price of the crossfade length, so assert the jump is
        // mostly gone rather than that it is exactly a normal step.
        assert!(
            naive > 5.0 * seam,
            "plain loop {naive:.2} should jump far more than the seam {seam:.2}"
        );

        // Cancel: an already-set flag stops the job and leaves no file behind.
        let cancelled = AtomicBool::new(true);
        let r = encode(
            &src,
            &dir.join("cancel"),
            "e2e-cancel",
            &cancelled,
            &mut |_| {},
        );
        assert_eq!(r, Err(Failure::Cancelled));
        assert!(!dir.join("cancel").join("e2e-cancel.part.mp4").exists());

        // A source with sound keeps it, crossfaded to the same length.
        let src_a = dir.join("src_audio.mp4");
        let made = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=6:size=160x120:rate=15")
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=6"])
            .args(["-map", "0:v", "-map", "1:a"])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"])
            .arg(&src_a)
            .status()
            .unwrap();
        assert!(made.success(), "audio source generated");
        assert!(probe_has_audio(&src_a) && !probe_has_audio(&src));
        let out_a = encode(&src_a, &dir.join("out"), "e2e-audio", &cancel, &mut |_| {})
            .expect("encode with audio");
        assert!(probe_has_audio(&out_a), "audio stream kept");
        let alen = Command::new("ffprobe")
            .args(["-v", "error", "-select_streams", "a", "-show_entries"])
            .args(["stream=duration", "-of", "csv=p=0"])
            .arg(&out_a)
            .output()
            .unwrap();
        let alen: f64 = String::from_utf8_lossy(&alen.stdout)
            .trim()
            .parse()
            .unwrap();
        let vlen = probe_duration(&out_a).unwrap();
        eprintln!("audio source -> audio stream {alen:.3}s, container {vlen:.3}s");
        assert!((alen - (6.0 - CROSSFADE_S)).abs() <= 0.15, "audio {alen}");

        std::fs::remove_dir_all(&dir).ok();
    }
}

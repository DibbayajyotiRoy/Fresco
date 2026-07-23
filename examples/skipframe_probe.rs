//! Does `vd-lavc-skipframe` actually reduce DECODED frames while hardware
//! decoding is active?
//!
//! This exists because the 1.1.32 frame-rate cap shipped unverified and made
//! things worse: a `vf=fps` filter forced every frame off the GPU and doubled
//! video-engine load. Before trusting the replacement mechanism, measure it.
//!
//! Decodes the same file three times with `vo=null` (no rendering, so the only
//! variable is decode) and reports how many frames each level actually decoded.
//!
//!   cargo run --example skipframe_probe --no-default-features \
//!     --features daemon -- /path/to/video.mp4
//!
//! Expected if the mechanism works: Reduced < Full, Minimum <= Reduced, and
//! `hwdec-current` stays a hardware decoder (never "no") in every run.

use fresco::config::PowerSaving;
use fresco::daemon::mpv::ffi::fns;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: skipframe_probe <video>");
            std::process::exit(2);
        }
    };
    let secs: f64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8.0);

    println!("file: {path}\nwindow: {secs}s per level\n");
    println!(
        "{:<10} {:>12} {:>14} {:>14}",
        "level", "output fps", "container fps", "hwdec"
    );

    let mut baseline = None;
    for level in [
        PowerSaving::Full,
        PowerSaving::Reduced,
        PowerSaving::Minimum,
    ] {
        match probe(&path, level, secs) {
            Some((decoded, dropped, hwdec)) => {
                println!("{level:<10?} {decoded:>12.1} {dropped:>14.1} {hwdec:>14}");
                if level == PowerSaving::Full {
                    baseline = Some(decoded);
                }
                if let (Some(b), true) = (baseline, level != PowerSaving::Full) {
                    if b > 0.0 {
                        let pct = 100.0 - (decoded / b * 100.0);
                        println!(
                            "{:<10} {pct:>11.1}% fewer frames reach the output vs Full",
                            ""
                        );
                    }
                }
            }
            None => println!("{level:<10?} {:>10}", "FAILED"),
        }
    }
}

/// Returns (frames decoded, frames dropped by decoder, active hwdec).
fn probe(path: &str, level: PowerSaving, secs: f64) -> Option<(f64, f64, String)> {
    let f = fns().ok()?;
    let h = f.create();
    if h.is_null() {
        return None;
    }
    // SAFETY: `h` is the live handle just created; destroyed at the end.
    unsafe {
        for (k, v) in [
            ("vo", "null"),
            ("ao", "null"),
            (
                "hwdec",
                &std::env::var("PROBE_HWDEC").unwrap_or_else(|_| "vaapi".into()),
            ),
            ("terminal", "no"),
            ("config", "no"),
            ("loop-file", "inf"),
        ] {
            f.set_option(h, k, v);
        }
        if let Some(skip) = level.skipframe() {
            f.set_option(h, "vd-lavc-skipframe", skip);
        }
        if f.initialize(h) < 0 {
            f.terminate_destroy(h);
            return None;
        }
        f.command(h, &["loadfile", path, "replace"]);
        std::thread::sleep(std::time::Duration::from_secs_f64(secs));

        let numf = |name: &str| -> f64 {
            f.get_property(h, name)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(-1.0)
        };
        // mpv has no "frames decoded" counter; estimated-vf-fps is the rate of
        // frames actually reaching the output, which is exactly what decoder
        // skipping reduces. Compare it against the file's own container fps.
        let decoded = numf("estimated-vf-fps");
        let dropped = numf("container-fps");
        let hwdec = f
            .get_property(h, "hwdec-current")
            .unwrap_or_else(|| "?".into());
        f.terminate_destroy(h);
        Some((decoded, dropped, hwdec))
    }
}

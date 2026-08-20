#[cfg(feature = "daemon")]
pub mod artwork;
#[cfg(feature = "daemon")]
pub mod audio_capture;
pub mod autostart;
pub mod capability;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod catalog;
pub mod cli;
pub mod clock;
pub mod config;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod download;
pub mod dsp;
#[cfg(feature = "gui")]
pub mod gui;
pub mod i18n;
pub mod ipc;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod linkresolve;
pub mod lyrics;
#[cfg(feature = "daemon")]
pub mod lyrics_fetch;
#[cfg(feature = "daemon")]
pub mod mpris;
pub mod schedule;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod supabase;
pub mod support;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod telemetry;
#[cfg(any(feature = "gui", feature = "daemon"))]
pub mod update;
pub mod visualizer;

/// Application ID used for the desktop file, autostart entry, and GTK app.
pub const APP_ID: &str = "io.github.dibbayajyotiroy.Fresco";
pub const APP_NAME: &str = "Fresco";
/// WM_CLASS of wallpaper windows, so users/extensions can target them.
pub const WALLPAPER_WM_CLASS: &str = "fresco-wallpaper";

/// True when running inside a Flatpak sandbox. Several host-facing paths
/// (autostart, the daemon launch command) differ in that case.
pub fn is_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Absolute locations to look for the **bundled** mpvpaper, in priority order.
/// Fresco ships it under `<prefix>/lib/fresco/mpvpaper` (e.g. `/usr/lib/fresco`
/// from the .deb, `/app/lib/fresco` in Flatpak) so it never collides with a
/// user-installed `/usr/bin/mpvpaper`.
/// Bundled mpvpaper basenames to try in each location, best first. Fresco ships
/// one build per libmpv soname generation — `mpvpaper-libmpv2` for distros with
/// mpv ≥ 0.35 (Ubuntu 24.04+, Fedora 38+, Arch) and `mpvpaper-libmpv1` for
/// older LTS bases (Ubuntu 22.04, Debian 12). Plain `mpvpaper` is the legacy
/// single-build name from pre-1.1.1 packages and dev trees.
const MPVPAPER_VARIANTS: [&str; 3] = ["mpvpaper-libmpv2", "mpvpaper-libmpv1", "mpvpaper"];

fn mpvpaper_candidates() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Beside our own binary (dev tree / `cargo install`).
            dirs.push(dir.to_path_buf());
            // Prefix-relative libexec: /usr/bin → /usr/lib/fresco,
            // /app/bin → /app/lib/fresco, /usr/local/bin → /usr/local/lib/fresco.
            dirs.push(dir.join("../lib/fresco"));
        }
    }
    // Absolute safety nets if current_exe() is unavailable.
    dirs.push(std::path::PathBuf::from("/usr/lib/fresco"));
    dirs.push(std::path::PathBuf::from("/app/lib/fresco"));
    dirs.iter()
        .flat_map(|d| MPVPAPER_VARIANTS.iter().map(|v| d.join(v)))
        .collect()
}

/// A candidate binary plus the version floor its `--help` probe reported.
/// `None` version means "runs, but we could not identify it".
type Probed = (std::path::PathBuf, Option<(u32, u32)>);

/// Where the mpvpaper we ended up running came from. Surfaced by
/// `fresco doctor` so a "my wallpaper is black" report can be diagnosed from
/// one command instead of by straceing the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpvpaperSource {
    /// `FRESCO_MPVPAPER` pointed at it.
    Override,
    /// Shipped inside the Fresco package, under `<prefix>/lib/fresco/`.
    Bundled,
    /// A copy the user installed themselves, found on `PATH`.
    System,
}

impl MpvpaperSource {
    /// Short human label for `fresco doctor`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Override => "FRESCO_MPVPAPER override",
            Self::Bundled => "bundled",
            Self::System => "system, on PATH",
        }
    }
}

/// A resolved renderer: which binary we picked, roughly how old it is, and
/// where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpvpaperChoice {
    pub path: std::path::PathBuf,
    /// Feature-probed version **floor**, not an exact version — see
    /// [`mpvpaper_version_from_help`]. `None` means we could not tell.
    pub version: Option<(u32, u32)>,
    pub source: MpvpaperSource,
}

impl MpvpaperChoice {
    /// Version as `fresco doctor` should print it. Ranges, not points, because
    /// the probe only yields a floor.
    pub fn version_label(&self) -> String {
        match self.version {
            Some((1, 9)) => "1.9 or newer".to_string(),
            Some((1, 7)) => "1.7–1.8".to_string(),
            Some((1, 4)) => "1.4–1.6".to_string(),
            Some((1, 0)) => "older than 1.4".to_string(),
            Some((maj, min)) => format!("{maj}.{min} or newer"),
            None => "unknown".to_string(),
        }
    }

    /// True when this renderer might predate upstream's fixes for black output
    /// on the NVIDIA proprietary driver — those landed in mpvpaper 1.6 ("fix
    /// support for the Nvidia proprietary drivers") and were extended in 1.7
    /// (reworked compositor render-loop handshake, again called out for
    /// Nvidia). We cannot distinguish 1.4 from 1.6 (identical `--help` text),
    /// so a 1.4 floor counts as *suspect*, not *known bad*.
    pub fn maybe_predates_nvidia_fix(&self) -> bool {
        self.version.is_none_or(|v| v < (1, 7))
    }
}

/// Guess an mpvpaper release from its `--help` output.
///
/// Upstream mpvpaper ships **no** `--version` flag — verified against the 1.4
/// and 1.9 sources and against a locally built 1.9: `--version` is rejected as
/// an unrecognised option, and the `--help` usage block contains no version
/// string anywhere. So instead of parsing a version we fingerprint the feature
/// set in that usage text and return the *lowest* release that matches.
///
/// Upstream only touched the usage text on three releases, which gives us
/// exactly three rungs:
///
/// | marker in `--help`                          | floor |
/// |---------------------------------------------|-------|
/// | `--auto-mode`                               | 1.9   |
/// | `auto options might not work as intended`   | 1.7   |
/// | `--help-output`                             | 1.4   |
/// | just `Usage: mpvpaper`                      | 1.0   |
///
/// 1.4, 1.5 and 1.6 are byte-identical here, which is annoying precisely
/// because 1.6 is where the NVIDIA render fix landed. A 1.4 floor therefore
/// means "1.4, 1.5 or 1.6" — see [`MpvpaperChoice::maybe_predates_nvidia_fix`].
pub fn mpvpaper_version_from_help(help: &str) -> Option<(u32, u32)> {
    if help.contains("--auto-mode") {
        Some((1, 9))
    } else if help.contains("auto options might not work") {
        Some((1, 7))
    } else if help.contains("--help-output") {
        Some((1, 4))
    } else if help.contains("Usage: mpvpaper") {
        Some((1, 0))
    } else {
        None
    }
}

/// What one `--help` run told us about a candidate binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Probe {
    /// Could not spawn, or died in the dynamic linker (exit 127).
    Unloadable,
    /// Runs here. Carries the feature-probed version floor (`None` when the
    /// usage text matched nothing we recognise).
    Loads(Option<(u32, u32)>),
}

/// Run `<path> --help` once and answer both questions we have about a
/// candidate: *does it load on this OS* and *which generation is it*. Merged
/// into a single spawn deliberately — this runs for every candidate at startup,
/// and the results are cached for the life of the process.
fn mpvpaper_probe(path: &std::path::Path) -> Probe {
    let Ok(out) = std::process::Command::new(path)
        .arg("--help")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return Probe::Unloadable;
    };
    // A build linked against a libmpv soname the OS doesn't ship execs fine but
    // dies in the dynamic linker with exit 127, so a plain "file exists" check
    // is not enough — that's exactly the failure mode behind "renderer failed
    // 5×" on distros whose libmpv generation differs from the build host's.
    if out.status.code() == Some(127) {
        return Probe::Unloadable;
    }
    Probe::Loads(mpvpaper_version_from_help(&String::from_utf8_lossy(
        &out.stdout,
    )))
}

/// Whether this mpvpaper binary can actually run here.
pub fn mpvpaper_runnable(path: &std::path::Path) -> bool {
    !matches!(mpvpaper_probe(path), Probe::Unloadable)
}

/// Pick the best of several probed candidates, given in priority order.
/// Highest version wins; ties keep the earlier (more preferred) entry.
///
/// Newest-wins rather than first-wins because the bundled directory can end up
/// holding a *mix* of versions: `install.sh` rebuilds a current mpvpaper into
/// `<prefix>/lib/fresco/mpvpaper` while the packaged `mpvpaper-libmpv2` from an
/// older release is still sitting next to it and still loads fine. First-wins
/// would keep silently using the stale one and the rebuild would do nothing.
/// In the normal case — one upstream version built once per libmpv soname —
/// every candidate ties, so this still prefers libmpv2 over libmpv1 exactly as
/// before.
fn pick_newest(probed: Vec<Probed>) -> Option<Probed> {
    probed
        .into_iter()
        .reduce(|best, cand| if cand.1 > best.1 { cand } else { best })
}

/// The best bundled candidate that exists AND loads, with its version floor.
/// Probed once per process (each probe spawns the binary with `--help`).
fn mpvpaper_bundled() -> Option<&'static Probed> {
    static FOUND: std::sync::OnceLock<Option<Probed>> = std::sync::OnceLock::new();
    FOUND
        .get_or_init(|| {
            pick_newest(
                mpvpaper_candidates()
                    .into_iter()
                    .filter(|c| c.is_file())
                    .filter_map(|c| match mpvpaper_probe(&c) {
                        Probe::Loads(v) => Some((c, v)),
                        Probe::Unloadable => None,
                    })
                    .collect(),
            )
        })
        .as_ref()
}

/// A user-installed `mpvpaper` on `PATH`, with its version floor. Probed once
/// per process, so preferring it costs no extra spawn per wallpaper change.
fn mpvpaper_system() -> Option<&'static Probed> {
    static FOUND: std::sync::OnceLock<Option<Probed>> = std::sync::OnceLock::new();
    FOUND
        .get_or_init(|| {
            let paths = std::env::var_os("PATH")?;
            std::env::split_paths(&paths)
                .map(|dir| dir.join("mpvpaper"))
                .filter(|c| c.is_file())
                .find_map(|c| match mpvpaper_probe(&c) {
                    Probe::Loads(v) => Some((c, v)),
                    Probe::Unloadable => None,
                })
        })
        .as_ref()
}

/// A bundled mpvpaper that exists but cannot load (e.g. built against a libmpv
/// soname this OS doesn't ship). Only reported when NO runnable copy exists;
/// used by `fresco doctor` to explain why rendering fails despite the file
/// being present.
pub fn mpvpaper_broken() -> Option<std::path::PathBuf> {
    if mpvpaper_bundled().is_some() {
        return None;
    }
    mpvpaper_candidates().into_iter().find(|c| c.is_file())
}

/// Decide which mpvpaper to run. Pure, so the policy is unit-testable without
/// spawning anything; [`mpvpaper_choice`] feeds it the probed reality.
///
/// * `FRESCO_MPVPAPER` always wins. It is the documented escape hatch, and the
///   entire point of it is to override our judgement — including when our
///   judgement is that the bundled copy is fine.
/// * Otherwise prefer the **bundled** build, unless a user-installed mpvpaper
///   on `PATH` reports a **strictly newer** version. Fresco bundles mpvpaper so
///   that it works with no setup, but a bundled build goes stale between
///   releases: shipping 1.4 while the user already has 1.9 installed is exactly
///   how you get a black wallpaper on the NVIDIA proprietary driver, which
///   upstream fixed in 1.6/1.7. Ties keep the bundled copy, because it is the
///   one we actually test against.
/// * `None` (version unknown) sorts below every `Some`, so "we could not tell"
///   never outranks "we know this one is 1.9".
fn choose_mpvpaper(
    bundled: Option<Probed>,
    system: Option<Probed>,
    over: Option<std::path::PathBuf>,
) -> Option<MpvpaperChoice> {
    if let Some(path) = over {
        return Some(MpvpaperChoice {
            path,
            // Left unprobed on purpose: this runs on every wallpaper change and
            // must not spawn. `fresco doctor` fills it in via mpvpaper_describe().
            version: None,
            source: MpvpaperSource::Override,
        });
    }
    match (bundled, system) {
        (Some((_bp, bv)), Some((sp, sv))) if sv > bv => Some(MpvpaperChoice {
            path: sp,
            version: sv,
            source: MpvpaperSource::System,
        }),
        (Some((bp, bv)), _) => Some(MpvpaperChoice {
            path: bp,
            version: bv,
            source: MpvpaperSource::Bundled,
        }),
        (None, Some((sp, sv))) => Some(MpvpaperChoice {
            path: sp,
            version: sv,
            source: MpvpaperSource::System,
        }),
        (None, None) => None,
    }
}

/// The renderer Fresco will actually run, with provenance. Cheap and cached:
/// the bundled and PATH probes each happen once per process, and the
/// `FRESCO_MPVPAPER` override is re-read every call so tests (and users) can
/// change it without restarting.
pub fn mpvpaper_choice() -> Option<MpvpaperChoice> {
    choose_mpvpaper(
        mpvpaper_bundled().cloned(),
        mpvpaper_system().cloned(),
        std::env::var_os("FRESCO_MPVPAPER").map(std::path::PathBuf::from),
    )
}

/// Like [`mpvpaper_choice`], but also probes a `FRESCO_MPVPAPER` override for
/// its version. Costs one extra spawn, so it is for `fresco doctor` only —
/// never the per-spawn path.
pub fn mpvpaper_describe() -> Option<MpvpaperChoice> {
    let mut choice = mpvpaper_choice()?;
    if choice.source == MpvpaperSource::Override && choice.path.is_file() {
        if let Probe::Loads(v) = mpvpaper_probe(&choice.path) {
            choice.version = v;
        }
    }
    Some(choice)
}

/// The `mpvpaper` command Fresco runs on Wayland. We ship mpvpaper **bundled**
/// so users never install it: an explicit `FRESCO_MPVPAPER` override wins, then
/// the newest of (the bundled copy, a user-installed one on `PATH`), and only
/// as a last resort the bare name `mpvpaper` for the OS to resolve.
pub fn mpvpaper_command() -> std::ffi::OsString {
    match mpvpaper_choice() {
        Some(c) => c.path.into_os_string(),
        None => std::ffi::OsString::from("mpvpaper"),
    }
}

/// The resolved mpvpaper path if it exists and loads (override, bundled, or on
/// PATH). Used by `fresco doctor` to report availability. `None` when not found
/// or when every found copy fails to load (see [`mpvpaper_broken`]).
pub fn mpvpaper_resolved() -> Option<std::path::PathBuf> {
    let choice = mpvpaper_choice()?;
    // An override is taken on trust by mpvpaper_command() (the user asked for
    // it), but "resolved" means *usable*, so a dangling override reports None.
    (choice.source != MpvpaperSource::Override || choice.path.is_file()).then_some(choice.path)
}

#[cfg(test)]
mod mpvpaper_tests {
    use super::*;

    /// Verbatim excerpts of the real `--help` text upstream prints at each tag
    /// (checked against github.com/GhostNaN/mpvpaper src/main.c). If upstream
    /// reword these, the fingerprint in `mpvpaper_version_from_help` goes stale
    /// and these tests are what should catch it.
    const HELP_1_4: &str = "Usage: mpvpaper [options] <output> <url|path filename>\n\
        --help         -h              Displays this help message\n\
        --help-output  -d              Displays all available outputs and quits\n\
        --layer        -l LAYER        Specifies shell surface layer to run on (background by default)\n\
        * See man page for more details\n";
    const HELP_1_7: &str = "Usage: mpvpaper [options] <output> <url|path filename>\n\
        --help-output  -d              Displays all available outputs and quits\n\
        * The auto options might not work as intended\n\
        See the man page for more details\n";
    const HELP_1_9: &str = "Usage: mpvpaper [options] <output> <url|path filename>\n\
        --help-output  -d              Displays all available <output> and quits\n\
        --auto-mode    -a <FULL|MAX>   Extend auto-pause/stop to trigger when any window is\n\
        * Auto options may vary based on compositor behavior\n";

    #[test]
    fn version_fingerprint_maps_help_text_to_a_floor() {
        assert_eq!(mpvpaper_version_from_help(HELP_1_4), Some((1, 4)));
        assert_eq!(mpvpaper_version_from_help(HELP_1_7), Some((1, 7)));
        assert_eq!(mpvpaper_version_from_help(HELP_1_9), Some((1, 9)));
        // Pre-1.4 had no --help-output, but still printed the usage banner.
        assert_eq!(
            mpvpaper_version_from_help("Usage: mpvpaper [options] <output> <file>\n--fork -f\n"),
            Some((1, 0))
        );
        // Not mpvpaper at all (or output we don't recognise).
        assert_eq!(mpvpaper_version_from_help(""), None);
        assert_eq!(
            mpvpaper_version_from_help("bash: mpvpaper: not found"),
            None
        );
    }

    #[test]
    fn floors_order_so_newer_wins() {
        // The comparison choose_mpvpaper() relies on.
        assert!(Some((1, 9)) > Some((1, 7)));
        assert!(Some((1, 7)) > Some((1, 4)));
        assert!(Some((1, 4)) > None);
    }

    #[test]
    fn nvidia_fix_suspicion_tracks_the_1_6_1_7_fixes() {
        let at = |v| MpvpaperChoice {
            path: "/x".into(),
            version: v,
            source: MpvpaperSource::Bundled,
        };
        // 1.4 floor is ambiguous (could be 1.4, 1.5 or 1.6) → still suspect.
        assert!(at(Some((1, 4))).maybe_predates_nvidia_fix());
        assert!(at(Some((1, 0))).maybe_predates_nvidia_fix());
        assert!(at(None).maybe_predates_nvidia_fix());
        assert!(!at(Some((1, 7))).maybe_predates_nvidia_fix());
        assert!(!at(Some((1, 9))).maybe_predates_nvidia_fix());
    }

    fn bundled(v: Option<(u32, u32)>) -> Option<Probed> {
        Some(("/usr/lib/fresco/mpvpaper-libmpv2".into(), v))
    }
    fn system(v: Option<(u32, u32)>) -> Option<Probed> {
        Some(("/usr/local/bin/mpvpaper".into(), v))
    }

    #[test]
    fn newest_bundled_variant_wins_but_ties_keep_priority_order() {
        let v2 = std::path::PathBuf::from("/usr/lib/fresco/mpvpaper-libmpv2");
        let v1 = std::path::PathBuf::from("/usr/lib/fresco/mpvpaper-libmpv1");
        let plain = std::path::PathBuf::from("/usr/lib/fresco/mpvpaper");

        // Normal case: same upstream version per soname → keep libmpv2.
        let got =
            pick_newest(vec![(v2.clone(), Some((1, 9))), (v1.clone(), Some((1, 9)))]).unwrap();
        assert_eq!(got.0, v2);

        // install.sh rebuilt a current mpvpaper next to a stale packaged one.
        let got = pick_newest(vec![
            (v2.clone(), Some((1, 4))),
            (plain.clone(), Some((1, 9))),
        ])
        .unwrap();
        assert_eq!(got.0, plain, "a freshly rebuilt copy must not be shadowed");

        // Unknown version never outranks a known one.
        let got = pick_newest(vec![(v2.clone(), None), (plain.clone(), Some((1, 4)))]).unwrap();
        assert_eq!(got.0, plain);

        assert!(pick_newest(vec![]).is_none());
    }

    #[test]
    fn override_beats_everything() {
        let c = choose_mpvpaper(
            bundled(Some((1, 9))),
            system(Some((1, 9))),
            Some("/home/u/.local/bin/mpvpaper".into()),
        )
        .unwrap();
        assert_eq!(c.source, MpvpaperSource::Override);
        assert_eq!(
            c.path,
            std::path::PathBuf::from("/home/u/.local/bin/mpvpaper")
        );
    }

    #[test]
    fn newer_system_copy_beats_a_stale_bundle() {
        // The reported bug: bundled 1.4 renders black on NVIDIA, user has 1.9.
        let c = choose_mpvpaper(bundled(Some((1, 4))), system(Some((1, 9))), None).unwrap();
        assert_eq!(c.source, MpvpaperSource::System);
        assert_eq!(c.version, Some((1, 9)));
    }

    #[test]
    fn bundled_wins_ties_and_when_it_is_newer() {
        // Tie → bundled, because that is the build we test against.
        let c = choose_mpvpaper(bundled(Some((1, 9))), system(Some((1, 9))), None).unwrap();
        assert_eq!(c.source, MpvpaperSource::Bundled);
        // Bundled newer than an old system copy → bundled.
        let c = choose_mpvpaper(bundled(Some((1, 9))), system(Some((1, 4))), None).unwrap();
        assert_eq!(c.source, MpvpaperSource::Bundled);
        // Unknown system version never outranks a known bundled one.
        let c = choose_mpvpaper(bundled(Some((1, 4))), system(None), None).unwrap();
        assert_eq!(c.source, MpvpaperSource::Bundled);
    }

    #[test]
    fn falls_back_to_whichever_exists() {
        assert_eq!(
            choose_mpvpaper(bundled(Some((1, 9))), None, None)
                .unwrap()
                .source,
            MpvpaperSource::Bundled
        );
        assert_eq!(
            choose_mpvpaper(None, system(Some((1, 4))), None)
                .unwrap()
                .source,
            MpvpaperSource::System
        );
        // A known-version system copy beats a bundle we could not identify.
        let c = choose_mpvpaper(bundled(None), system(Some((1, 9))), None).unwrap();
        assert_eq!(c.source, MpvpaperSource::System);
        assert!(choose_mpvpaper(None, None, None).is_none());
    }

    #[test]
    fn version_labels_read_as_ranges_not_points() {
        let at = |v| MpvpaperChoice {
            path: "/x".into(),
            version: v,
            source: MpvpaperSource::Bundled,
        };
        assert_eq!(at(Some((1, 9))).version_label(), "1.9 or newer");
        assert_eq!(at(Some((1, 7))).version_label(), "1.7–1.8");
        assert_eq!(at(Some((1, 4))).version_label(), "1.4–1.6");
        assert_eq!(at(None).version_label(), "unknown");
    }
}

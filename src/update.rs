//! Shared update-check logic used by both the GUI and the daemon: version
//! comparison, locating the bundled updater script, running it, and querying
//! the release host for the latest version.
//!
//! "The release host" rather than "GitHub" because Fresco also publishes to a
//! Gitee mirror for mainland China, where github.com is unreliable. Which host
//! an install talks to is fixed at install time — see [`Origin`].

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;

use serde::Deserialize;

/// Exit code `scripts/fresco-update.sh` uses for "already on the latest
/// version" — a benign no-op, matching the script's documented codes.
const EXIT_UP_TO_DATE: i32 = 2;

/// Exit code `scripts/fresco-update.sh` uses for "can't auto-install here"
/// (Flatpak sandbox or no `apt-get`), matching the script's documented codes.
const EXIT_UNSUPPORTED: i32 = 3;

/// Where this copy of Fresco gets its releases from.
///
/// GitHub is unreliable to reach from mainland China, so Fresco also publishes
/// to a Gitee mirror. Which host a given install should talk to is decided
/// **once, at install time**, not per request: a user who installed from Gitee
/// because GitHub was unreachable must not then be sent to GitHub to update —
/// that is precisely how a mirrored install ends up frozen on the version it
/// was installed at.
///
/// The installer records the choice in `~/.config/fresco/install-origin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Origin {
    #[default]
    GitHub,
    /// gitee.com mirror, for mainland China.
    Gitee,
}

impl Origin {
    /// The origin this install should use.
    ///
    /// `FRESCO_ORIGIN` wins (for testing and for anyone who wants to switch),
    /// then the marker the installer wrote, then GitHub. An unrecognised value
    /// falls back to GitHub rather than failing: being sent to the wrong host
    /// is recoverable, having no update path at all is not.
    pub fn current() -> Origin {
        if let Ok(v) = std::env::var("FRESCO_ORIGIN") {
            if let Some(o) = Origin::from_tag(v.trim()) {
                return o;
            }
        }
        std::fs::read_to_string(Origin::marker_path())
            .ok()
            .and_then(|s| Origin::from_tag(s.trim()))
            .unwrap_or(Origin::GitHub)
    }

    /// The stable identifier written to the marker file and passed to the
    /// updater script. Inverse of `Origin::from_tag` (private).
    pub fn tag(self) -> &'static str {
        match self {
            Origin::GitHub => "github",
            Origin::Gitee => "gitee",
        }
    }

    fn from_tag(tag: &str) -> Option<Origin> {
        match tag.to_ascii_lowercase().as_str() {
            "github" => Some(Origin::GitHub),
            "gitee" => Some(Origin::Gitee),
            _ => None,
        }
    }

    /// Where `install.sh` records which host this copy came from. Kept beside
    /// `install-source` (the campaign tag) rather than in `config.toml`,
    /// because it describes the *installation*, not the user's preferences —
    /// it must survive a config reset and must be readable before config loads.
    pub fn marker_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("fresco")
            .join("install-origin")
    }

    /// The REST endpoint describing the newest release. Both hosts answer with
    /// a `tag_name` and `browser_download_url` fields, which is why one
    /// `Release` struct deserialises either.
    pub fn releases_api(self) -> &'static str {
        match self {
            Origin::GitHub => "https://api.github.com/repos/DibbayajyotiRoy/fresco/releases/latest",
            Origin::Gitee => "https://gitee.com/api/v5/repos/dibbayajyoti/fresco/releases/latest",
        }
    }

    /// The human-facing releases page, for "Open releases page" and for the
    /// notification that fires when a new version ships.
    pub fn releases_page(self) -> &'static str {
        match self {
            Origin::GitHub => "https://github.com/DibbayajyotiRoy/fresco/releases/latest",
            Origin::Gitee => "https://gitee.com/dibbayajyoti/fresco/releases/latest",
        }
    }

    /// The one-liner shown when Fresco cannot update itself in place.
    pub fn install_command(self) -> &'static str {
        match self {
            Origin::GitHub => {
                "curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | bash"
            }
            Origin::Gitee => {
                "curl -fsSL https://gitee.com/dibbayajyoti/fresco/releases/latest/download/install.sh | FRESCO_ORIGIN=gitee bash"
            }
        }
    }
}

/// The version this binary was built as.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// True if `candidate` is a strictly newer semver than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let strip = |v: &str| v.trim().trim_start_matches('v').to_string();
    match (
        semver::Version::parse(&strip(candidate)),
        semver::Version::parse(&strip(current)),
    ) {
        (Ok(c), Ok(cur)) => c > cur,
        _ => false,
    }
}

/// Locate the bundled updater script: beside our binary (dev tree), then the
/// prefix-relative libexec dir, then the absolute .deb install path.
pub fn updater_script() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("fresco-update.sh"));
            candidates.push(dir.join("../lib/fresco/fresco-update.sh"));
        }
    }
    candidates.push(PathBuf::from("/usr/lib/fresco/fresco-update.sh"));
    candidates.into_iter().find(|p| p.is_file())
}

/// Result of running the bundled updater script.
#[derive(Debug)]
pub enum UpdateOutcome {
    Success,
    /// The script found the installed version already current and did nothing
    /// (its documented exit 2) — a benign no-op, not a failure.
    AlreadyUpToDate,
    Failed(String),
    /// The install can't be auto-updated this way (Flatpak sandbox or no
    /// `apt-get`) — caller should route to a manual-install fallback.
    Unsupported,
}

/// Map the updater script's documented exit codes onto [`UpdateOutcome`].
fn outcome_from_status(status: std::process::ExitStatus) -> UpdateOutcome {
    match status.code() {
        _ if status.success() => UpdateOutcome::Success,
        Some(EXIT_UP_TO_DATE) => UpdateOutcome::AlreadyUpToDate,
        Some(EXIT_UNSUPPORTED) => UpdateOutcome::Unsupported,
        _ => UpdateOutcome::Failed(format!("updater exited with {status}")),
    }
}

/// Download + install the latest .deb by running the bundled updater script as
/// root via pkexec (the desktop's polkit agent prompts once).
pub fn run_updater_blocking() -> UpdateOutcome {
    let Some(script) = updater_script() else {
        return UpdateOutcome::Failed("updater script not found".into());
    };
    // The origin is passed as an ARGUMENT, not an env var: pkexec runs the
    // script as root with a sanitised environment and root's $HOME, so it can
    // neither inherit FRESCO_ORIGIN nor read the marker from the invoking
    // user's config directory.
    match std::process::Command::new("pkexec")
        .arg(&script)
        .arg("--origin")
        .arg(Origin::current().tag())
        .status()
    {
        Ok(status) => outcome_from_status(status),
        Err(e) => UpdateOutcome::Failed(format!("failed to launch pkexec: {e}")),
    }
}

/// One live progress event from the updater script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// A `STAGE: <name>` line — a new phase started (downloading / installing / done).
    Stage(String),
    /// A `PROGRESS: <0-100>` line — download completion percentage.
    Percent(u8),
}

/// Same as [`run_updater_blocking`], but streams the script's `STAGE:` and
/// `PROGRESS:` stdout lines to `on_progress` as they arrive, so a caller (e.g.
/// the GUI, on a background thread) can show live progress instead of a silent
/// blocking call. Runs entirely with std/anyhow so this stays usable from
/// either the `gui` or `daemon` feature.
pub fn run_updater_with_progress(on_progress: impl Fn(Progress) + Send + 'static) -> UpdateOutcome {
    let Some(script) = updater_script() else {
        return UpdateOutcome::Failed("updater script not found".into());
    };
    let mut cmd = std::process::Command::new("pkexec");
    cmd.arg(&script);
    // See run_updater_blocking: pkexec drops the environment, so the host has
    // to travel as an argument.
    cmd.arg("--origin").arg(Origin::current().tag());
    run_command_with_progress(cmd, on_progress)
}

/// Inner runner, split from the pkexec wrapper so tests can exercise the
/// stage/stderr plumbing with an ordinary command.
fn run_command_with_progress(
    mut cmd: std::process::Command,
    on_progress: impl Fn(Progress) + Send + 'static,
) -> UpdateOutcome {
    let mut child = match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => return UpdateOutcome::Failed(format!("failed to launch updater: {e}")),
    };

    // Drain stderr on its own thread — a piped-but-undrained stderr would
    // deadlock apt if its warnings filled the pipe. Keep the tail so a failure
    // shows WHAT went wrong instead of only an exit code.
    let stderr_tail = child.stderr.take().map(|err| {
        std::thread::spawn(move || {
            let mut tail = std::collections::VecDeque::with_capacity(12);
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                if tail.len() == 12 {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
            tail.into_iter().collect::<Vec<_>>().join("\n")
        })
    });

    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(stage) = line.strip_prefix("STAGE: ") {
                on_progress(Progress::Stage(stage.to_string()));
            } else if let Some(pct) = line.strip_prefix("PROGRESS: ") {
                if let Ok(pct) = pct.trim().parse::<u8>() {
                    on_progress(Progress::Percent(pct.min(100)));
                }
            }
        }
    }
    let stderr_text = stderr_tail.and_then(|h| h.join().ok()).unwrap_or_default();

    match child.wait() {
        Ok(status) => match outcome_from_status(status) {
            UpdateOutcome::Failed(msg) if !stderr_text.trim().is_empty() => {
                UpdateOutcome::Failed(format!("{msg}\n{}", stderr_text.trim()))
            }
            other => other,
        },
        Err(e) => UpdateOutcome::Failed(format!("failed to wait on updater: {e}")),
    }
}

/// The latest published release on GitHub. The .deb asset URL isn't carried
/// here: the updater script resolves it itself at install time.
pub struct LatestRelease {
    pub version: String,
    pub notes_url: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    /// GitHub returns this; Gitee's v5 release object does not. Optional so one
    /// struct deserialises both, with the origin's releases page as the
    /// fallback link.
    #[serde(default)]
    html_url: Option<String>,
}

/// Fetch the latest release from whichever host this copy was installed from
/// (unauthenticated). See [`Origin`].
pub fn fetch_latest() -> anyhow::Result<LatestRelease> {
    let origin = Origin::current();
    let resp = ureq::get(origin.releases_api())
        .set("Accept", "application/vnd.github+json")
        .call()?;
    let release: ReleaseResponse = resp.into_json()?;
    Ok(LatestRelease {
        version: release.tag_name,
        notes_url: release
            .html_url
            .unwrap_or_else(|| origin.releases_page().to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every origin must round-trip through its tag, because the tag is what
    /// crosses two process boundaries: the marker file the installer writes,
    /// and the `--origin` argument handed to the updater under pkexec. A
    /// mismatch on either side silently sends a Gitee install back to GitHub.
    #[test]
    fn origins_round_trip_through_their_tag() {
        for origin in [Origin::GitHub, Origin::Gitee] {
            assert_eq!(Origin::from_tag(origin.tag()), Some(origin));
        }
        // The installer writes lowercase, but be forgiving of a hand-edited file.
        assert_eq!(Origin::from_tag("GITEE"), Some(Origin::Gitee));
        assert_eq!(Origin::from_tag("nonsense"), None);
    }

    /// An unknown or absent marker must degrade to GitHub rather than leaving
    /// the install with no update path at all.
    #[test]
    fn unknown_origin_falls_back_to_github() {
        assert_eq!(Origin::default(), Origin::GitHub);
        assert_eq!(Origin::from_tag(""), None);
    }

    /// The Gitee mirror exists because GitHub is unreachable from mainland
    /// China. An endpoint that still points at github.com would defeat the
    /// entire purpose, so assert the hosts really differ.
    #[test]
    fn gitee_endpoints_never_point_at_github() {
        for url in [
            Origin::Gitee.releases_api(),
            Origin::Gitee.releases_page(),
            Origin::Gitee.install_command(),
        ] {
            assert!(
                !url.contains("github.com") && !url.contains("api.github"),
                "Gitee origin leaks a GitHub host: {url}"
            );
            assert!(url.contains("gitee.com"), "not a Gitee URL: {url}");
        }
        // And the reverse, so a copy-paste edit can't quietly swap them.
        assert!(Origin::GitHub.releases_api().contains("api.github.com"));
        assert!(Origin::GitHub.releases_page().contains("github.com"));
    }

    /// The Gitee install one-liner must carry the origin forward, or the copy
    /// it installs will check GitHub for updates and never find them.
    #[test]
    fn gitee_install_command_records_its_origin() {
        assert!(
            Origin::Gitee
                .install_command()
                .contains("FRESCO_ORIGIN=gitee"),
            "the Gitee one-liner must set FRESCO_ORIGIN so the install is \
             marked as coming from Gitee: {}",
            Origin::Gitee.install_command()
        );
    }

    #[test]
    fn failed_update_carries_stderr_detail() {
        let stages = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Progress>::new()));
        let seen = stages.clone();
        let mut cmd = std::process::Command::new("bash");
        cmd.args([
            "-c",
            "echo 'STAGE: downloading'; echo 'PROGRESS: 40'; echo 'E: apt broke badly' >&2; exit 1",
        ]);
        let outcome = run_command_with_progress(cmd, move |p| seen.lock().unwrap().push(p));
        match outcome {
            UpdateOutcome::Failed(msg) => {
                assert!(msg.contains("E: apt broke badly"), "msg was: {msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert_eq!(
            stages.lock().unwrap().as_slice(),
            [Progress::Stage("downloading".into()), Progress::Percent(40)]
        );
    }

    #[test]
    fn is_newer_compares_semver() {
        assert!(is_newer("0.1.0", "0.0.9"));
        assert!(is_newer("v1.0.0", "0.9.9")); // tolerates a leading "v"
        assert!(!is_newer("0.0.9", "0.0.9")); // equal is not newer
        assert!(!is_newer("0.0.8", "0.0.9"));
        assert!(!is_newer("not-a-version", "0.0.9")); // unparsable => false, never crashes
    }
}

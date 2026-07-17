//! Shared update-check logic used by both the GUI and the daemon: version
//! comparison, locating the bundled updater script, running it, and querying
//! GitHub Releases for the latest version.

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

/// Where releases are published.
const RELEASES_API: &str = "https://api.github.com/repos/DibbayajyotiRoy/fresco/releases/latest";

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
    match std::process::Command::new("pkexec").arg(&script).status() {
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
    html_url: String,
}

/// Fetch the latest release from the GitHub Releases API (unauthenticated).
pub fn fetch_latest() -> anyhow::Result<LatestRelease> {
    let resp = ureq::get(RELEASES_API)
        .set("Accept", "application/vnd.github+json")
        .call()?;
    let release: ReleaseResponse = resp.into_json()?;
    Ok(LatestRelease {
        version: release.tag_name,
        notes_url: release.html_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

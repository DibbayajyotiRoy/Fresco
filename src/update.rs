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

/// Same as [`run_updater_blocking`], but streams the script's `STAGE: <name>`
/// stdout lines to `on_stage` as they arrive, so a caller (e.g. the GUI, on a
/// background thread) can show live progress instead of a silent blocking call.
/// Runs entirely with std/anyhow so this stays usable from either the `gui` or
/// `daemon` feature.
pub fn run_updater_with_progress(on_stage: impl Fn(String) + Send + 'static) -> UpdateOutcome {
    let Some(script) = updater_script() else {
        return UpdateOutcome::Failed("updater script not found".into());
    };
    // stderr is inherited (not piped): nothing reads it here, and a piped-but-
    // undrained stderr would deadlock apt if its warnings ever fill the pipe.
    let mut child = match std::process::Command::new("pkexec")
        .arg(&script)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return UpdateOutcome::Failed(format!("failed to launch pkexec: {e}")),
    };

    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(stage) = line.strip_prefix("STAGE: ") {
                on_stage(stage.to_string());
            }
        }
    }

    match child.wait() {
        Ok(status) => outcome_from_status(status),
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
    fn is_newer_compares_semver() {
        assert!(is_newer("0.1.0", "0.0.9"));
        assert!(is_newer("v1.0.0", "0.9.9")); // tolerates a leading "v"
        assert!(!is_newer("0.0.9", "0.0.9")); // equal is not newer
        assert!(!is_newer("0.0.8", "0.0.9"));
        assert!(!is_newer("not-a-version", "0.0.9")); // unparsable => false, never crashes
    }
}

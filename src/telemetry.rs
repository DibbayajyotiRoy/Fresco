//! Anonymous, opt-out usage telemetry. Everything here is deliberately
//! boring-by-design: a random install id (never derived from hardware or
//! hostname, so it identifies an *install*, not a person), coarse environment
//! facts, feature-usage counts, and error kinds. No paths, no file names, no
//! wallpaper content. The Settings switch ("Share anonymous usage statistics")
//! gates every call; when off, every function returns before touching the
//! network or disk markers.
//!
//! All network I/O runs on a detached thread with short timeouts so telemetry
//! can never slow down or break the app — failures are logged at debug level
//! and otherwise invisible.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Config;

/// Same project/key as `supabase.rs` — RLS protects the data, not key secrecy.
const URL: &str = "https://mmoxgmvrpiaflfnsrynx.supabase.co";
const ANON_KEY: &str = "sb_publishable_eWKJzAuME5rstSxGyCBoHA_8hrTwkQM";

/// Heartbeats self-throttle to roughly daily; 20h (not 24h) so a user who
/// opens their laptop at slightly different times each day still pings daily.
const HEARTBEAT_MIN_AGE: Duration = Duration::from_secs(20 * 60 * 60);

/// Whether the user has telemetry enabled (Settings → "Share anonymous usage
/// statistics"). Reads the config fresh so a toggle takes effect immediately,
/// without plumbing state through every call site.
pub fn enabled() -> bool {
    // Consent-gated: nothing is sent until the user has answered the one-time
    // consent dialog (telemetry_prompted), and then only if they said yes.
    Config::load()
        .map(|c| c.telemetry_prompted && c.telemetry)
        .unwrap_or(false)
}

/// Path of the persisted install id, next to config.toml.
fn install_id_path() -> PathBuf {
    Config::path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(std::env::temp_dir)
        .join("install-id")
}

/// The persistent anonymous install id: a UUID-v4-shaped random string,
/// generated once and stored on disk. Random on purpose — deriving it from
/// hardware or hostname would make it a fingerprint.
pub fn install_id() -> String {
    install_id_at(&install_id_path())
}

fn install_id_at(path: &Path) -> String {
    if let Ok(id) = std::fs::read_to_string(path) {
        let id = id.trim();
        if !id.is_empty() {
            return id.to_string();
        }
    }
    let id = random_uuid_v4();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    // Best-effort persist; a write failure just means a fresh id next run,
    // which only inflates install counts — never leaks anything.
    std::fs::write(path, &id).ok();
    id
}

/// UUID v4 from /dev/urandom (no new deps). Falls back to hashing clock+pid
/// entropy if urandom is unreadable — weaker uniqueness, same anonymity.
fn random_uuid_v4() -> String {
    use std::io::Read as _;
    let mut b = [0u8; 16];
    let filled = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok();
    if !filled {
        use std::hash::BuildHasher;
        for (i, chunk) in b.chunks_mut(8).enumerate() {
            chunk.copy_from_slice(
                &std::collections::hash_map::RandomState::new()
                    .hash_one((std::time::SystemTime::now(), std::process::id(), i))
                    .to_le_bytes()[..chunk.len()],
            );
        }
    }
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13],
        b[14], b[15]
    )
}

/// Marker whose mtime throttles heartbeats; lives in the state dir next to
/// frescod.log (same convention as the feedback-sent marker).
fn heartbeat_marker() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("heartbeat-sent")
}

/// True when no heartbeat was sent within the throttle window.
fn heartbeat_due(marker: &Path, min_age: Duration) -> bool {
    match std::fs::metadata(marker).and_then(|m| m.modified()) {
        Ok(mtime) => match mtime.elapsed() {
            Ok(age) => age >= min_age,
            // mtime in the future (clock jump) — treat as due.
            Err(_) => true,
        },
        Err(_) => true, // never sent
    }
}

/// Value of PRETTY_NAME (preferred) or ID from /etc/os-release — the distro
/// name only, nothing machine-specific.
fn distro() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    let value = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').to_string())
            .filter(|v| !v.is_empty())
    };
    value("PRETTY_NAME").or_else(|| value("ID"))
}

/// Fire one POST on a detached thread; telemetry must never block a caller
/// or surface a failure.
fn post_detached(table: &'static str, payload: serde_json::Value, prefer: &'static str) {
    std::thread::spawn(move || {
        let result = ureq::post(&format!("{URL}/rest/v1/{table}"))
            .timeout(Duration::from_secs(5))
            .set("apikey", ANON_KEY)
            .set("Authorization", &format!("Bearer {ANON_KEY}"))
            .set("Content-Type", "application/json")
            .set("Prefer", prefer)
            .send_json(payload);
        if let Err(e) = result {
            log::debug!("telemetry post to {table} failed: {e}");
        }
    });
}

/// Daily install ping: upserted (merge-duplicates on install_id) so each
/// install is one row, updated in place. `backend`/`decode`/`monitor_count`
/// come from the daemon when handy; None is fine.
pub fn heartbeat(backend: Option<&str>, decode: Option<&str>, monitor_count: Option<u32>) {
    if !enabled() {
        return;
    }
    let marker = heartbeat_marker();
    if !heartbeat_due(&marker, HEARTBEAT_MIN_AGE) {
        return;
    }
    // Touch the marker before the network call — a flapping daemon must not
    // retry-spam even when the server is unreachable.
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&marker, b"").ok();
    let payload = serde_json::json!({
        "install_id": install_id(),
        "version": env!("CARGO_PKG_VERSION"),
        "distro": distro(),
        "compositor": std::env::var("XDG_CURRENT_DESKTOP").ok(),
        "session": std::env::var("XDG_SESSION_TYPE").ok(),
        "backend": backend,
        "decode": decode,
        "monitor_count": monitor_count,
        "source": install_source(),
        "channel": install_channel(),
        "last_seen": chrono::Utc::now().to_rfc3339(),
    });
    post_detached("installs", payload, "resolution=merge-duplicates");
}

/// UTM-style download attribution: the install one-liner persists the tag the
/// copy button embedded (FRESCO_SOURCE=website|github|reddit|…) so acquisition
/// channels are measurable. Absent for installs that predate the tagging or
/// came from a package manager directly.
fn install_source() -> Option<String> {
    let path = Config::path().parent()?.join("install-source");
    let tag = std::fs::read_to_string(path).ok()?;
    let tag: String = tag
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect();
    (!tag.is_empty()).then_some(tag)
}

/// How this copy of Fresco is packaged — detected at runtime, not declared.
fn install_channel() -> &'static str {
    if crate::is_flatpak() {
        return "flatpak";
    }
    // The .deb ships the updater script at this fixed path; AUR/source builds don't.
    if std::path::Path::new("/usr/lib/fresco/fresco-update.sh").exists() {
        return "deb";
    }
    "other"
}

/// Count one feature use. `props` must stay content-free (kinds, outcomes —
/// never names or paths).
pub fn event(name: &str, props: serde_json::Value) {
    if !enabled() {
        return;
    }
    let payload = serde_json::json!({
        "install_id": install_id(),
        "name": name,
        "props": props,
        "version": env!("CARGO_PKG_VERSION"),
    });
    post_detached("events", payload, "return=minimal");
}

/// Report one anonymous error. `detail` is truncated so a runaway message
/// can't smuggle large or unexpected content into the row.
pub fn error(kind: &str, detail: &str) {
    if !enabled() {
        return;
    }
    let detail: String = detail.chars().take(500).collect();
    let payload = serde_json::json!({
        "install_id": install_id(),
        "kind": kind,
        "detail": detail,
        "version": env!("CARGO_PKG_VERSION"),
    });
    post_detached("errors", payload, "return=minimal");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_id_shape_and_persistence() {
        let dir = std::env::temp_dir().join(format!("fresco-telemetry-{}", std::process::id()));
        let path = dir.join("install-id");
        let id = install_id_at(&path);
        // UUID v4 shape: 8-4-4-4-12 lowercase hex, version + variant nibbles.
        assert_eq!(id.len(), 36);
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(id
            .chars()
            .all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert!(parts[2].starts_with('4'));
        assert!(matches!(
            parts[3].chars().next(),
            Some('8' | '9' | 'a' | 'b')
        ));
        // Second call returns the same persisted id.
        assert_eq!(install_id_at(&path), id);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn heartbeat_throttle() {
        let dir = std::env::temp_dir().join(format!("fresco-hb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("heartbeat-sent");
        // No marker: due.
        assert!(heartbeat_due(&marker, HEARTBEAT_MIN_AGE));
        // Fresh marker: not due within the window…
        std::fs::write(&marker, b"").unwrap();
        assert!(!heartbeat_due(&marker, HEARTBEAT_MIN_AGE));
        // …but due once the window is zero (i.e. mtime older than min_age).
        assert!(heartbeat_due(&marker, Duration::ZERO));
        std::fs::remove_dir_all(&dir).ok();
    }
}

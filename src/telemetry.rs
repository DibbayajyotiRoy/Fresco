//! Anonymous, consent-gated usage telemetry. Everything here is deliberately
//! boring-by-design: a random install id (never derived from hardware or
//! hostname, so it identifies an *install*, not a person), coarse environment
//! facts, feature-usage counts, and error kinds. No paths, no file names, no
//! wallpaper content.
//!
//! # The two tiers
//!
//! Knowing how many people use Fresco, and roughly where, is treated as
//! essential to maintaining it: a release cannot be tested where users are if
//! nobody knows where they are. So **both** tiers carry the random install id
//! and the country. What the dialog actually asks about is the detail:
//!
//! * **Accept all** — [`Tier::Full`]. The daily [`heartbeat`] with the full
//!   environment (distro, desktop, session, backend, monitor count, install
//!   source), the precise time of each check-in, [`event`] counts, and
//!   [`error`] kinds.
//! * **Decline optional** — [`Tier::Essential`]. The install id, the country,
//!   the app version, and how it was packaged. No environment, no events, no
//!   errors, and the check-in is recorded to the DAY rather than the moment
//!   (`register_install_minimal` truncates the timestamp server-side, so the
//!   exact time of use is not merely unused, it is never stored).
//!
//! Both tiers can therefore be counted as distinct users over any window.
//!
//! Nothing at all is sent before the dialog is answered ([`Tier::Unanswered`]),
//! and the country in both tiers is resolved by Cloudflare at the network edge
//! from an IP this code never sees, receives, or stores.
//!
//! Setting `telemetry = false` in config.toml by hand leaves the essential
//! heartbeat running; [`opt_out_completely`] documents how to silence
//! everything.
//!
//! All network I/O runs on a detached thread with short timeouts so telemetry
//! can never slow down or break the app — failures are logged at debug level
//! and otherwise invisible.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Config;

/// Current revision of the consent terms, mirroring TERMS.md.
///
/// Installs whose `telemetry_consent_version` is lower answered a materially
/// different question and are asked again, once. Bump this whenever what is
/// collected, or what declining means, changes.
///
/// * Revision 1 — declining stopped being total silence: it sent an
///   identifier-free country tally.
/// * Revision 2 — declining now sends the install id too, so unique users are
///   countable in both tiers. Re-prompting is **mandatory** here and not a
///   nicety: revision 1's dialog said in as many words that declining sends
///   "no install id", and shipping revision 2 silently would make that a lie
///   told to people who are still running on it.
pub const CONSENT_VERSION: u32 = 2;

/// What the user agreed to share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Dialog not answered yet (or answered under older terms). Send nothing.
    Unanswered,
    /// Declined the optional detail: install id, country, version, packaging.
    /// Check-in recorded to the day, not the moment.
    Essential,
    /// Accepted everything: the above plus environment, precise timestamps,
    /// feature events, and error kinds.
    Full,
}

/// Resolve the current tier from config. Read fresh on every call so a toggle
/// in Settings takes effect immediately, without plumbing state through every
/// call site.
pub fn tier() -> Tier {
    let Ok(c) = Config::load() else {
        return Tier::Unanswered;
    };
    if !c.telemetry_prompted || c.telemetry_consent_version < CONSENT_VERSION {
        return Tier::Unanswered;
    }
    if c.telemetry {
        Tier::Full
    } else {
        Tier::Essential
    }
}

/// Same project/key as `supabase.rs` — RLS protects the data, not key secrecy.
const URL: &str = "https://mmoxgmvrpiaflfnsrynx.supabase.co";
const ANON_KEY: &str = "sb_publishable_eWKJzAuME5rstSxGyCBoHA_8hrTwkQM";

/// Heartbeats self-throttle to roughly daily; 20h (not 24h) so a user who
/// opens their laptop at slightly different times each day still pings daily.
const HEARTBEAT_MIN_AGE: Duration = Duration::from_secs(20 * 60 * 60);

/// Where the app asks for its own coarse location.
///
/// Postgres can see the country for free (Cloudflare's `CF-IPCountry`, read by
/// `request_country()`), but not the city: `CF-IPCity` is a Cloudflare
/// Enterprise header that Supabase does not enable. Vercel injects the
/// equivalents on every plan and the Fresco site already runs there, so this
/// endpoint costs nothing extra and adds no third-party service.
///
/// It returns place names only, never coordinates and never an IP — Vercel
/// resolves the address at the edge before the handler runs. See
/// landing/src/app/api/geo/route.ts.
const GEO_URL: &str = "https://fresco.dibbayajyoti.com/api/geo";

/// Short by design. City is a nice-to-have on an optional-tier heartbeat: if
/// the lookup is slow the right answer is to give up and send without it, not
/// to delay or drop the heartbeat itself.
const GEO_TIMEOUT: Duration = Duration::from_secs(4);

/// Whether the optional detail (environment, events, errors, precise times)
/// may be sent.
///
/// Deliberately not "is any network call allowed": [`Tier::Essential`] is
/// false here and still sends its identity + country heartbeat. Gate
/// [`event`] and [`error`] on this, not the heartbeat.
pub fn enabled() -> bool {
    tier() == Tier::Full
}

/// How to send absolutely nothing, documented in one place because the
/// Settings switch alone does not do it — off means [`Tier::Essential`], not
/// silence. Setting both of these in config.toml puts the app back to
/// [`Tier::Unanswered`], which is silent:
///
/// ```toml
/// telemetry = false
/// telemetry_prompted = false
/// ```
///
/// Referenced by TERMS.md; keep the two in step.
pub const fn opt_out_completely() {}

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
///
/// Shared with [`crate::support`] for its ticket. Sharing the *generator* is
/// fine and sharing the *value* would not be: the two ids are drawn
/// independently and must never be equal.
pub(crate) fn random_uuid_v4() -> String {
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

/// Coarse location for the [`Tier::Full`] heartbeat: `(city, region)`.
///
/// Only ever called on the full-consent path. The essential tier does not call
/// it at all, which is what makes "declining means the city is never sent"
/// literally true rather than a promise about what the server does with it.
///
/// Returns `(None, None)` on any failure — offline, endpoint down, malformed
/// response, or Vercel simply not supplying a city for that address. A missing
/// city is a normal outcome and never an error worth surfacing.
///
/// The country deliberately is NOT taken from here even though the endpoint
/// returns it: `request_country()` resolves it server-side from a header the
/// client cannot forge, and a spoofable country would corrupt the one
/// geographic number that matters. City and region are client-supplied and
/// therefore spoofable; nothing is authorised on them.
fn geo() -> (Option<String>, Option<String>) {
    #[derive(serde::Deserialize)]
    struct Geo {
        #[serde(default)]
        city: Option<String>,
        #[serde(default)]
        region: Option<String>,
    }

    let clean = |value: Option<String>| -> Option<String> {
        let text = value?;
        let text = text.trim();
        // Bound what a compromised or confused endpoint can put in a column.
        (!text.is_empty() && text.chars().count() <= 80).then(|| text.to_string())
    };

    // Matched in two steps rather than chained through and_then: ureq::Error
    // is a large enum, and threading it through a closure's Err makes clippy
    // (rightly) complain about the size of the returned Result.
    let response = match ureq::get(GEO_URL).timeout(GEO_TIMEOUT).call() {
        Ok(r) => r,
        Err(e) => {
            log::debug!("geo lookup failed: {e}");
            return (None, None);
        }
    };
    match response.into_json::<Geo>() {
        Ok(g) => (clean(g.city), clean(g.region)),
        Err(e) => {
            log::debug!("geo response was not usable json: {e}");
            (None, None)
        }
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
    std::thread::spawn(move || post_blocking(table, payload, prefer));
}

/// The actual POST. Split out of [`post_detached`] so a caller already running
/// on its own thread (the full heartbeat, which does a geo lookup first) does
/// not spawn a second one just to make one request.
fn post_blocking(table: &str, payload: serde_json::Value, prefer: &str) {
    {
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
    }
}

/// Daily install ping: one row per install, refreshed in place. Goes through
/// the `register_install` RPC rather than a direct table upsert — the table is
/// write-only for anon (never SELECT), and PostgREST's merge-duplicates upsert
/// needs read access to the conflict row, which anon lacks, so a direct upsert
/// is rejected by RLS. The RPC is SECURITY DEFINER and does the upsert as its
/// owner; see `register_install` in supabase/schema.sql. `backend`/`decode`/
/// `monitor_count` come from the daemon when handy; None is fine.
pub fn heartbeat(backend: Option<&str>, decode: Option<&str>, monitor_count: Option<u32>) {
    match tier() {
        Tier::Unanswered => return,
        // Declined the optional detail: identity and country still go out, so
        // this user is counted, but nothing describing their machine does.
        Tier::Essential => return minimal_heartbeat(),
        Tier::Full => {}
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
    // Named args match register_install's parameters (p_-prefixed). last_seen
    // is intentionally omitted: the function stamps now() with the server
    // clock, so a skewed client clock can't distort active-user windows.
    // Everything below runs on its own thread: the geo lookup is a second
    // network round trip, and the heartbeat is called from the daemon's start-up
    // path where blocking would delay the first wallpaper appearing.
    let backend = backend.map(str::to_string);
    let decode = decode.map(str::to_string);
    std::thread::spawn(move || {
        let (city, region) = geo();
        let payload = serde_json::json!({
            "p_install_id": install_id(),
            "p_version": env!("CARGO_PKG_VERSION"),
            "p_distro": distro(),
            "p_compositor": std::env::var("XDG_CURRENT_DESKTOP").ok(),
            "p_session": std::env::var("XDG_SESSION_TYPE").ok(),
            "p_backend": backend,
            "p_decode": decode,
            "p_monitor_count": monitor_count,
            "p_source": install_source(),
            "p_channel": install_channel(),
            "p_city": city,
            "p_region": region,
        });
        post_blocking("rpc/register_install", payload, "return=minimal");
    });
}

/// The [`Tier::Essential`] heartbeat: install id, country, version, packaging.
///
/// What is deliberately absent: the distro, the desktop, the session type, the
/// rendering backend, the monitor count, the install source, every feature
/// event, every error — and the time. The server truncates the timestamp to
/// the day (`register_install_minimal` in supabase/schema.sql), so this records
/// that the user was active on a date and never that they were active at
/// 21:47. The country is resolved at the edge and is never sent by this client.
///
/// This replaces revision 1's identifier-free `count_anonymous_ping`, which is
/// no longer called: writing both would count every essential user twice.
pub fn minimal_heartbeat() {
    if tier() == Tier::Unanswered {
        return;
    }
    // Shares the full heartbeat's marker on purpose: a user who switches tiers
    // must not get a second check-in the same day out of the switch itself.
    let marker = heartbeat_marker();
    if !heartbeat_due(&marker, HEARTBEAT_MIN_AGE) {
        return;
    }
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&marker, b"").ok();

    let payload = serde_json::json!({
        "p_install_id": install_id(),
        "p_version": env!("CARGO_PKG_VERSION"),
        "p_channel": install_channel(),
    });
    post_detached("rpc/register_install_minimal", payload, "return=minimal");
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

/// Human-readable environment block for a bug report.
///
/// Not telemetry: nothing here is posted by us. It is pre-filled into a GitHub
/// issue the user reviews in their browser and submits themselves, so it
/// deliberately ignores the [`enabled`] opt-out — and carries no install id.
///
/// This exists because feedback rows are anonymous and carry no environment at
/// all (`os` is a compile-time constant, always "linux"), which is why a 👎 like
/// "does not work, wallpaper is just black" arrives undiagnosable.
pub fn env_summary() -> String {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| "unknown".into());
    format!(
        "- Fresco: {}\n\
         - Distro: {}\n\
         - Desktop: {}\n\
         - Session: {}\n\
         - Backend: {}\n\
         - Install: {}",
        env!("CARGO_PKG_VERSION"),
        distro().unwrap_or_else(|| "unknown".into()),
        var("XDG_CURRENT_DESKTOP"),
        var("XDG_SESSION_TYPE"),
        crate::capability::detect().id(),
        install_channel(),
    )
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

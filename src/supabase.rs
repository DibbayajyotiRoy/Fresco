//! Minimal Supabase REST client for **anonymous, opt-in** feedback and
//! admin-pushed notifications. Talks to PostgREST over HTTPS with the
//! publishable key; Row-Level Security (see supabase/schema.sql) restricts the
//! anon role to inserting feedback and reading published notifications.
//!
//! These calls block, so callers run them on a background thread.

use anyhow::Result;
use serde::Deserialize;

/// Project URL and publishable (anon) key. Safe to ship in an open-source
/// client — RLS is what protects the data, not key secrecy.
const URL: &str = "https://mmoxgmvrpiaflfnsrynx.supabase.co";
const ANON_KEY: &str = "sb_publishable_eWKJzAuME5rstSxGyCBoHA_8hrTwkQM";

/// An admin-pushed notification (a row in the `notifications` table).
#[derive(Debug, Clone, Deserialize)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// Submit one anonymous feedback row. `rating` is +1 (👍) or -1 (👎).
/// Sends only the rating, an optional comment, the app version, and the OS —
/// no identifiers.
pub fn submit_feedback(rating: i8, comment: Option<String>) -> Result<()> {
    let payload = serde_json::json!({
        "rating": rating,
        "comment": comment,
        "app_version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        // Coarse "where are our users" signal — region-level only, still no
        // identifiers. Timezone gives geography; locale gives language+country.
        "timezone": system_timezone(),
        "locale": system_locale(),
    });
    ureq::post(&format!("{URL}/rest/v1/feedback"))
        .set("apikey", ANON_KEY)
        .set("Authorization", &format!("Bearer {ANON_KEY}"))
        .set("Content-Type", "application/json")
        .set("Prefer", "return=minimal")
        .send_json(payload)?;
    // One successful submission permanently silences the daemon's periodic
    // feedback reminder (see daemon/notifier.rs).
    let marker = feedback_sent_marker();
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&marker, b"").ok();
    Ok(())
}

/// Marker file written after one successful submission; the daemon's feedback
/// reminder stops for good once it exists.
pub fn feedback_sent_marker() -> std::path::PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("feedback-sent")
}

/// IANA timezone name ("Asia/Kolkata"), from /etc/timezone or the
/// /etc/localtime symlink. None when neither is readable.
fn system_timezone() -> Option<String> {
    if let Ok(tz) = std::fs::read_to_string("/etc/timezone") {
        let tz = tz.trim();
        if !tz.is_empty() {
            return Some(tz.to_string());
        }
    }
    let link = std::fs::read_link("/etc/localtime").ok()?;
    let s = link.to_string_lossy();
    s.split("zoneinfo/").nth(1).map(|z| z.to_string())
}

/// The user's locale ("en_IN.UTF-8") — language plus country code.
fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

/// Fetch published notifications, newest first.
pub fn fetch_notifications() -> Result<Vec<Notification>> {
    let resp = ureq::get(&format!(
        "{URL}/rest/v1/notifications\
         ?select=id,title,body,url&published=eq.true&order=created_at.desc&limit=20"
    ))
    .set("apikey", ANON_KEY)
    .set("Authorization", &format!("Bearer {ANON_KEY}"))
    .call()?;
    Ok(resp.into_json()?)
}

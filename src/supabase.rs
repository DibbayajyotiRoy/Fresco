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
    });
    ureq::post(&format!("{URL}/rest/v1/feedback"))
        .set("apikey", ANON_KEY)
        .set("Authorization", &format!("Bearer {ANON_KEY}"))
        .set("Content-Type", "application/json")
        .set("Prefer", "return=minimal")
        .send_json(payload)?;
    Ok(())
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

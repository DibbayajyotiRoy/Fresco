//! Anonymous two-way support threads.
//!
//! Lets a user talk to the maintainer from inside Fresco without either side
//! learning who the other is. The user sees "Fresco maintainer"; the maintainer
//! sees a ticket, the messages, and whatever environment the user chose to
//! attach when opening it.
//!
//! # Identity
//!
//! A thread is addressed by a random `ticket` UUID generated on first use and
//! kept in the state dir. Holding the ticket is what authorises reading and
//! writing that thread — an unguessable capability, like a secret link. The
//! server scopes every RPC to the ticket it is given, so a client can only ever
//! see its own conversation.
//!
//! The ticket is **deliberately not** the telemetry install id
//! ([`crate::telemetry::install_id`]):
//!
//! * Support has to work for users who declined statistics, who have no install
//!   id on the server at all.
//! * A support thread is the one place where a user writes in their own words.
//!   Keying it by the telemetry id would make it possible to attach that
//!   writing to an environment profile, which is exactly the linkage the
//!   telemetry design spends so much effort avoiding.
//!
//! The two ids are generated separately, stored in different files, and never
//! sent in the same request.
//!
//! # What is sent
//!
//! Only what the user types, plus the environment block they can see in the
//! dialog before sending (app version, distro, desktop, session, backend) —
//! the same text [`crate::telemetry::env_summary`] builds for a bug report. No
//! install id, no file names, no wallpaper content. Nothing here is gated on
//! the telemetry consent tier, because nothing here is telemetry: it is a
//! message the user chose to write and chose to send.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;

use crate::t;

/// Same project/key as `supabase.rs` — RLS protects the data, not key secrecy.
const URL: &str = "https://mmoxgmvrpiaflfnsrynx.supabase.co";
const ANON_KEY: &str = "sb_publishable_eWKJzAuME5rstSxGyCBoHA_8hrTwkQM";

/// Network timeout. Support is interactive, so this is longer than telemetry's
/// fire-and-forget budget but still short enough not to hang the UI thread.
const TIMEOUT: Duration = Duration::from_secs(10);

/// One message in a thread.
#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    /// "user" or "maintainer".
    pub sender: String,
    pub body: String,
    pub created_at: String,
}

impl Message {
    /// True when this came from the maintainer, for rendering sides.
    pub fn is_maintainer(&self) -> bool {
        self.sender == "maintainer"
    }
}

/// Where the ticket lives: the state dir, next to the other markers, and
/// pointedly *not* next to `install-id` in the config dir.
fn ticket_path() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("support-ticket")
}

/// The ticket for this install's thread, if one has ever been opened.
///
/// Returns None rather than creating one: a user who has never written to the
/// maintainer has no thread, and [`poll`] must not conjure one just by being
/// called on launch.
pub fn existing_ticket() -> Option<String> {
    let raw = std::fs::read_to_string(ticket_path()).ok()?;
    let raw = raw.trim();
    (raw.len() == 36 && raw.chars().all(|c| c == '-' || c.is_ascii_hexdigit()))
        .then(|| raw.to_string())
}

/// The ticket, creating and persisting one on first use.
fn ticket_or_create() -> Result<String> {
    if let Some(t) = existing_ticket() {
        return Ok(t);
    }
    let id = crate::telemetry::random_uuid_v4();
    let path = ticket_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &id)?;
    Ok(id)
}

/// Whether the user has ever opened a thread. Drives whether the menu entry
/// reads "Message the maintainer" or "Your conversation".
pub fn has_thread() -> bool {
    existing_ticket().is_some()
}

fn post(rpc: &str, payload: serde_json::Value) -> Result<ureq::Response> {
    Ok(ureq::post(&format!("{URL}/rest/v1/rpc/{rpc}"))
        .timeout(TIMEOUT)
        .set("apikey", ANON_KEY)
        .set("Authorization", &format!("Bearer {ANON_KEY}"))
        .set("Content-Type", "application/json")
        .send_json(payload)?)
}

/// Send a message, opening the thread if this is the first one.
///
/// `env` is the environment block the user saw in the dialog. Pass None to
/// attach nothing — the choice is the user's, and the dialog offers it as a
/// checkbox rather than assuming.
///
/// Blocks. Call it off the UI thread.
pub fn send(body: &str, env: Option<&str>) -> Result<()> {
    send_inner(body, env, "direct", None)
}

/// Open a thread from the feedback dialog, carrying the rating that prompted
/// it so the maintainer's inbox can put an unhappy user first.
///
/// Separate from [`send`] because the two are different consent moments: this
/// one only ever runs when the user ticked "let the maintainer reply" while
/// submitting a rating, and the rating travels with it so the thread reads as
/// what it is rather than as an unexplained message.
///
/// Blocks. Call it off the UI thread.
pub fn open_from_feedback(rating: i8, comment: &str, env: Option<&str>) -> Result<()> {
    // A rating with no comment still deserves a thread — a bare 👎 is exactly
    // the case where asking "what broke?" is worth the most — so seed it with
    // something the maintainer can reply to rather than an empty body, which
    // the RPC would discard.
    let body = if comment.trim().is_empty() {
        if rating < 0 {
            t!("(rated Not great, no comment)").to_string()
        } else {
            t!("(rated Loving it, no comment)").to_string()
        }
    } else {
        comment.trim().to_string()
    };
    send_inner(&body, env, "feedback", Some(rating))
}

fn send_inner(body: &str, env: Option<&str>, origin: &str, rating: Option<i8>) -> Result<()> {
    let ticket = ticket_or_create()?;
    post(
        "support_open",
        serde_json::json!({
            "p_ticket": ticket,
            "p_body": body,
            "p_app_version": env!("CARGO_PKG_VERSION"),
            "p_env": env,
            "p_origin": origin,
            "p_rating": rating,
        }),
    )?;
    Ok(())
}

/// The ticket to stamp on a feedback row so the maintainer can reply to it,
/// creating one if this install has never had a thread.
///
/// Returns None only if the ticket could not be persisted, in which case the
/// feedback is still submitted — just without a reply channel. Losing the
/// ability to reply is a far better failure than losing the feedback.
pub fn ticket_for_reply() -> Option<String> {
    ticket_or_create().ok()
}

/// Every message on this install's thread, oldest first. Empty when no thread
/// has been opened — never an error, because "you have not written to anyone"
/// is a normal state and not a failure.
///
/// Blocks. Call it off the UI thread.
pub fn poll() -> Result<Vec<Message>> {
    let Some(ticket) = existing_ticket() else {
        return Ok(Vec::new());
    };
    let resp = post("support_poll", serde_json::json!({ "p_ticket": ticket }))?;
    Ok(resp.into_json()?)
}

/// Clear the unread flag once the conversation has actually been shown.
/// Best-effort: a failure here only means the maintainer's inbox keeps showing
/// the thread as unread on the user side, which is harmless.
pub fn mark_read() {
    let Some(ticket) = existing_ticket() else {
        return;
    };
    std::thread::spawn(move || {
        if let Err(e) = post(
            "support_mark_read",
            serde_json::json!({ "p_ticket": ticket }),
        ) {
            log::debug!("support mark_read failed: {e}");
        }
    });
}

/// Marker recording how many messages had been seen last time the user looked,
/// so a reply arriving while the app is closed can be announced on next launch.
fn seen_marker() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("support-seen")
}

/// How many messages the user has already been shown.
pub fn seen_count() -> usize {
    std::fs::read_to_string(seen_marker())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// Record that `n` messages have been shown.
pub fn set_seen_count(n: usize) {
    let path = seen_marker();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(path, n.to_string()).ok();
}

/// Number of maintainer replies the user has not seen yet. Blocks; used by the
/// daemon on its existing notification cadence, and by the GUI on launch.
pub fn unread_replies() -> usize {
    let Ok(messages) = poll() else {
        return 0;
    };
    messages.len().saturating_sub(seen_count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_is_not_the_install_id() {
        // Different files, so one can never be read as the other. The paths are
        // asserted rather than the values because generating either in a test
        // would write into the real state dir.
        assert_ne!(
            ticket_path().file_name(),
            Some(std::ffi::OsStr::new("install-id"))
        );
        assert!(ticket_path().ends_with("fresco/support-ticket"));
    }

    #[test]
    fn existing_ticket_rejects_junk() {
        let dir = std::env::temp_dir().join(format!("fresco-support-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t");
        // Anything that is not a uuid-shaped string is not a ticket: a
        // truncated or half-written file must not be sent as one.
        for junk in ["", "  ", "nope", "1234"] {
            std::fs::write(&path, junk).unwrap();
            let raw = std::fs::read_to_string(&path).unwrap();
            let raw = raw.trim();
            assert!(!(raw.len() == 36 && raw.chars().all(|c| c == '-' || c.is_ascii_hexdigit())));
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn maintainer_side_is_detected() {
        let m = Message {
            sender: "maintainer".into(),
            body: "hi".into(),
            created_at: String::new(),
        };
        assert!(m.is_maintainer());
        assert!(!Message {
            sender: "user".into(),
            ..m
        }
        .is_maintainer());
    }
}

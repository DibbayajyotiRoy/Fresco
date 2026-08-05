//! Online synced-lyric lookup against LRCLIB, with an on-disk cache.
//!
//! The lyric widget originally read only local `.lrc` sidecar files, which
//! covers a local music library and nothing else. Most people drive it from
//! Spotify, YouTube or a browser tab, where there is no file on disk to read
//! and the widget therefore showed nothing at all. This module is the missing
//! source: given the MPRIS metadata the daemon already has, it asks LRCLIB for
//! the track's `.lrc` body.
//!
//! # Division of labour
//!
//! This module does **network and cache I/O and nothing else**. It returns raw
//! `.lrc` **text**; `crate::lyrics::parse_lrc` remains the single owner of
//! parsing, timing, `[offset:]` handling and ASS escaping. Two parsers for one
//! format is how the two drift apart, so there is deliberately no timestamp
//! logic below this line.
//!
//! # Blocking I/O — read this before calling anything
//!
//! **Every function here that can touch the network blocks the calling
//! thread**, for up to [`HTTP_TOTAL_TIMEOUT`] per request and
//! [`MAX_ARTIST_ATTEMPTS`] requests. That is [`fetch`], [`fetch_outcome`],
//! [`fetch_cached`] and [`fetch_cached_outcome`]. The daemon loop drives the
//! wallpaper, the overlay clock
//! and Smart Sleep; blocking it for even one second is a visible stall. Call
//! these from a worker thread (`std::thread::spawn`) and hand the result back
//! over a channel. [`cached`] alone is a cheap local read and may be called
//! inline, but it can still hit a slow filesystem, so prefer the worker there
//! too.
//!
//! # About the service
//!
//! LRCLIB (<https://lrclib.net>) is a free, community-run synced-lyric
//! database. It needs no API key and no registration, but its documentation
//! *requires* clients to identify themselves in the `User-Agent` header and
//! *requires* clients to honour `Retry-After` on `429`. Both are implemented
//! here — see [`USER_AGENT`] and the rate-limit backoff in [`fetch`].
//!
//! # Cache is private
//!
//! Everything this module writes lives under the current user's XDG cache
//! directory. It is a **per-user, local-only** copy that exists to avoid
//! re-asking a free service for the same track on every replay. Fresco never
//! uploads it, never shares it between users, and never redistributes it. It
//! is disposable: deleting the directory only costs one refetch.
//!
//! # Legal posture — read [`ATTRIBUTION`]
//!
//! Lyrics are third-party content of uncertain provenance. Fresco displays
//! what LRCLIB returns and claims nothing about it. See [`ATTRIBUTION`].

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

/// LRCLIB's track-signature endpoint.
///
/// `/api/get` is the only endpoint used. `/api/search` would return up to 20
/// fuzzy candidates and force us to guess which one the user is listening to;
/// picking wrong shows the wrong words in sync, which is worse than showing
/// none. `/api/get-cached` appears in older third-party clients but is **gone**
/// from the current service (it answers `404` with an empty body, i.e. no such
/// route), so it is not used.
const API_GET: &str = "https://lrclib.net/api/get";

/// Client identification, in exactly the shape LRCLIB's docs ask for:
/// application name, version, and a link to the project page.
///
/// Their documentation states they *require* clients to identify themselves,
/// and their example is of the form `LRCGET v0.2.0 (https://github.com/…)`.
/// Sending a generic or absent UA is how a free service ends up unable to tell
/// a well-behaved client from a scraper, and their edge has been observed
/// blocking specific unhelpful UA tokens outright.
pub const USER_AGENT: &str = concat!(
    "Fresco v",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/DibbayajyotiRoy/fresco)"
);

/// User-facing credit and disclaimer for the online lyric source.
///
/// Shown wherever online lyrics are enabled (settings toggle, about page). Two
/// separate jobs, both deliberate:
///
/// 1. **Credit.** LRCLIB is run by one person, for free, with no ads and no
///    API keys. Naming the source is the minimum courtesy, and it is what the
///    other open-source clients that depend on it do.
/// 2. **Disclaimer.** Lyrics are user-contributed to LRCLIB and their
///    copyright provenance is not established. Fresco neither owns nor
///    licenses them, so it must not imply that it does.
pub const ATTRIBUTION: &str =
    "Lyrics provided by LRCLIB (https://lrclib.net), a free community database. \
     Fresco does not host, own or license lyric content; it is fetched on demand \
     and cached only on this device.";

/// Connect timeout. A dead or hijacked-DNS host must not pin a worker thread.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Per-read timeout.
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Whole-request ceiling, and therefore the **worst-case time [`fetch`] blocks
/// its thread** — public so the daemon can size its worker and its own
/// cancellation window against a real number rather than a guess.
///
/// Longer than the artwork fetcher's, on purpose: LRCLIB's docs warn that a
/// signature it has never seen sends *it* to external sources first, so a cold
/// miss is legitimately slow. Still bounded, because this runs on a worker
/// thread that a track change wants back. Note that [`fetch`] may make up to
/// [`MAX_ARTIST_ATTEMPTS`] requests (see [`fetch_outcome`]'s Retries section),
/// so its true ceiling is that many times this, plus the pauses between them.
pub const HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(15);

/// How long a cached `.lrc` body stays valid: 30 days.
///
/// The timings of a released track do not change. Revisions do get published
/// to LRCLIB, but a correction landing inside any given month for a track this
/// user happens to play is rare, whereas replaying the same album daily is the
/// normal case. A month makes the common case free and bounds staleness to
/// something a user would never notice.
pub const HIT_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// How long a "no lyrics for this track" marker stays valid: 24 hours.
///
/// Much shorter than [`HIT_TTL`], because a miss is not a fact about the world,
/// it is a fact about the database *today* — LRCLIB is continuously
/// contributed to, so today's miss is next week's hit. But without *any*
/// negative cache, an instrumental track on repeat would fire a request every
/// single play, which is precisely the behaviour a free service should not have
/// to absorb. One request per track per day is the compromise: invisible to the
/// service, and a user who leaves a track playing overnight still picks up
/// newly published lyrics tomorrow.
pub const MISS_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Format marker for cache records, so a future format change can be detected
/// rather than misparsed. Bump on any layout change; unknown versions are
/// treated as a cache miss and simply refetched.
const CACHE_MAGIC: &str = "fresco-lrc-cache/1";

/// What LRCLIB needs to identify a track, distilled from MPRIS metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Track title. Never empty in a `Query` built by [`query_from`].
    pub title: String,
    /// Artist. Never empty in a `Query` built by [`query_from`].
    pub artist: String,
    /// Album, when the player published one. Optional: browsers and many web
    /// players publish a title and artist and nothing else.
    pub album: Option<String>,
    /// Track length in whole seconds, when known.
    pub duration_s: Option<u32>,
}

/// Build a [`Query`] from what a player is currently reporting, or `None` when
/// the metadata is too thin to search with.
///
/// Pure — no I/O, no clock. The bar for "searchable" is a non-blank title
/// **and** a non-blank artist: LRCLIB matches on a track signature, and a
/// title alone matches hundreds of unrelated records. Players that publish
/// only a stream name (many web radios) fall out here, which is correct —
/// there is nothing to look up.
///
/// Album and duration are passed through when present and omitted when not.
/// Both genuinely help the match, and neither is worth refusing to search over.
pub fn query_from(np: &crate::mpris::NowPlaying) -> Option<Query> {
    let title = collapse_ws(&np.title);
    if title.is_empty() {
        return None;
    }
    // Same `", "` join the rest of the UI shows (`NowPlaying::artist_line`), but
    // blank entries are dropped first: players do publish `xesam:artist` arrays
    // containing empty strings, and joining those blindly yields `", "`, which
    // is a non-empty string that would sail past the emptiness check below and
    // send a garbage artist to the service.
    let artist = np
        .artists
        .iter()
        .map(|a| collapse_ws(a))
        .filter(|a| !a.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if artist.is_empty() {
        return None;
    }
    let album = {
        let a = collapse_ws(&np.album);
        (!a.is_empty()).then_some(a)
    };
    // `mpris:length` is microseconds and is signed; players occasionally
    // publish 0 or a negative for a live stream, which is not a duration.
    let duration_s = np
        .length_us
        .filter(|us| *us > 0)
        .map(|us| (us / 1_000_000) as u32)
        .filter(|s| *s > 0);
    Some(Query {
        title,
        artist,
        album,
        duration_s,
    })
}

/// Hard ceiling on `/api/get` requests for a single lookup, **including** the
/// first attempt with the full artist string.
///
/// LRCLIB is a free service run by one person; its documentation asks clients
/// to identify themselves, to space requests out, and to honour `Retry-After`.
/// So "one request per credited artist" is not an option: film soundtracks and
/// compilations routinely credit six or eight people, and a track change would
/// turn into an eight-request burst against somebody else's server.
///
/// Four is the smallest cap that resolves the case it was sized against — a
/// Bengali film track published by a browser as one string,
/// `"Indraadip Dasgupta, Prasen, Arijit Singh, Anweshaa Dutta Gupta"`, whose
/// LRCLIB record is filed under the *third* credited name. Three would stop one
/// short of it. Going higher buys very little: credit lists put the performer
/// near the front, and past the first few names what remains is lyricists,
/// composers and featured guests that essentially no record is filed under —
/// near-certain misses paid for in requests to a free service.
///
/// The worst case is bounded and paid at most once per track per
/// [`MISS_TTL`]: four requests, spaced by [`INTER_REQUEST_PAUSE`], and only
/// ever on a cache miss. Single-artist tracks — the common case — still make
/// exactly one.
///
/// Public for the same reason [`HTTP_TOTAL_TIMEOUT`] is: together they are the
/// worst-case blocking time of a lookup, and a caller sizing a worker or a
/// cancellation window should be able to read it rather than guess.
pub const MAX_ARTIST_ATTEMPTS: usize = 4;

/// Pause between consecutive requests within one lookup.
///
/// LRCLIB asks clients to space requests by 200–500 ms rather than pipelining
/// them. This is the only place Fresco sends more than one request in a row.
pub const INTER_REQUEST_PAUSE: Duration = Duration::from_millis(350);

/// Strip a trailing feature credit from one artist name, normalising whitespace.
///
/// `"Alpha feat. Beta"` becomes `"Alpha"`. Returns an empty string when nothing
/// survives, which the caller drops rather than sending as a query.
fn strip_feature_credits(name: &str) -> String {
    // Feature separators as they actually appear in MPRIS metadata.
    let head = ["feat.", "ft.", " & ", " x ", " with "].iter().fold(
        name.trim().to_string(),
        |acc, sep| {
            // `to_ascii_lowercase` and not `to_lowercase`: the separators are
            // all ASCII, and full Unicode lowercasing can *change the byte
            // length* (U+0130 becomes two chars), so an index taken from the
            // folded string can land mid-character in the original and panic
            // the slice. ASCII folding is byte-for-byte stable.
            match acc.to_ascii_lowercase().find(sep) {
                Some(i) => acc[..i].trim().to_string(),
                None => acc,
            }
        },
    );
    collapse_ws(&head)
}

impl Query {
    /// Artist strings to try, most specific first, capped at
    /// [`MAX_ARTIST_ATTEMPTS`].
    ///
    /// Streaming players and browsers publish *every* credited artist, so a
    /// track arrives as `"A, B, C"` while the LRCLIB record was contributed
    /// from a tag that says just one of those names. That mismatch is the most
    /// common reason a track that *is* in the database comes back empty.
    ///
    /// The order is deliberate:
    ///
    /// 1. **The full string as published.** Unchanged from what the player
    ///    said, and correct for the overwhelmingly common single-artist track —
    ///    which therefore yields exactly one candidate and makes exactly one
    ///    request. It is also the most specific query available, so trying it
    ///    first is what stops a broad name from matching the wrong record.
    /// 2. **Each individually credited artist, in published order**, split on
    ///    `,` and `;` with feature credits (`feat.`, `ft.`, `&`, `x`, `with`)
    ///    stripped. Published order is used rather than any cleverness about
    ///    which name "looks like" the performer: guessing wrong costs the same
    ///    request as guessing right, and the metadata carries no field that
    ///    says which credit is the recording artist.
    ///
    /// Blank candidates are dropped and duplicates are skipped, so no request
    /// is ever repeated verbatim. An empty artist yields no candidates at all
    /// and therefore no request: `artist_name=` matches nothing and is junk
    /// traffic.
    fn artist_candidates(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let full = collapse_ws(&self.artist);
        if !full.is_empty() {
            out.push(full);
        }
        for piece in self.artist.split([',', ';']) {
            if out.len() >= MAX_ARTIST_ATTEMPTS {
                break;
            }
            let name = strip_feature_credits(piece);
            // ASCII-case-insensitive for the same reason the separator scan is:
            // it is a comparison over untrusted Unicode that must never be able
            // to surprise us, and two candidates differing only in case would
            // be two requests for one answer.
            if name.is_empty() || out.iter().any(|c| c.eq_ignore_ascii_case(&name)) {
                continue;
            }
            out.push(name);
        }
        out
    }
}

/// Lowercase-insensitive whitespace normalisation: trim, and collapse every
/// internal run of whitespace to one space.
///
/// Applied to query fields *and* to cache keys, so that the same track arriving
/// with a stray double space or a trailing tab is one cache entry, not two.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// What a lookup actually established about a track.
///
/// `Option<String>` cannot express the difference between the two ways a
/// lookup ends without usable words, and they are *different facts*:
///
/// - the track is not in LRCLIB at all, versus
/// - the track **is** in LRCLIB but the record carries no timings.
///
/// Both leave the timed overlay with nothing to draw, so both are `None` to
/// [`fetch`] and [`fetch_cached`], whose signatures are unchanged. But they
/// deserve different words on screen: "no synced lyrics for this track" is a
/// finished answer the user can act on (LRCLIB accepts contributions), whereas
/// silence looks like a bug in Fresco. Callers that want to say so should use
/// [`fetch_cached_outcome`] and match on this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Synced `.lrc` text, ready for `crate::lyrics::parse_lrc`.
    Synced(String),
    /// A record exists, but there is nothing timed to display: it carries only
    /// `plainLyrics`, or it is flagged `instrumental`. Suggested phrasing is
    /// "no synced lyrics for this track", which is true of both — as opposed
    /// to "not found", which would be false.
    NoTimings,
    /// No record matched this signature. Suggested phrasing is "lyrics not
    /// found".
    NotFound,
}

impl Outcome {
    /// The synced `.lrc` text, discarding *why* there was none.
    ///
    /// This is the adapter the `Option`-returning entry points are built from.
    #[must_use]
    pub fn into_synced(self) -> Option<String> {
        match self {
            Outcome::Synced(lrc) => Some(lrc),
            Outcome::NoTimings | Outcome::NotFound => None,
        }
    }
}

/// Classify an LRCLIB `/api/get` response body.
///
/// Pure: takes text, returns a verdict. No HTTP, so every branch below is unit
/// testable without a network.
///
/// - [`Outcome::Synced`] — the record has non-blank `syncedLyrics`.
/// - [`Outcome::NoTimings`] — the record exists but is marked `instrumental`,
///   or carries only `plainLyrics`. **Unsynced text is deliberately
///   discarded** — the widget is a timed overlay driven by `.lrc` timestamps,
///   and a static wall of text pinned over the wallpaper for the whole song is
///   worse than showing nothing.
/// - [`Outcome::NotFound`] — the body is a `TrackNotFound` (or other) error
///   object, or valid JSON of a shape with no record in it.
/// - `Err` — the body is not JSON at all (truncated response, captive portal
///   login page, proxy error page). Distinct from a clean verdict on purpose: a
///   clean miss earns a negative cache entry, garbage must not, or one flaky
///   minute would silence a track for [`MISS_TTL`].
pub(crate) fn parse_outcome(body: &str) -> Result<Outcome> {
    let v: serde_json::Value =
        serde_json::from_str(body).context("LRCLIB response was not valid JSON")?;

    // Error objects are JSON too and carry `code`/`name`, e.g. TrackNotFound.
    // Treat any of them as "nothing found" rather than reading fields off them.
    if v.get("code").is_some() && v.get("syncedLyrics").is_none() {
        return Ok(Outcome::NotFound);
    }
    if v.get("instrumental").and_then(serde_json::Value::as_bool) == Some(true) {
        return Ok(Outcome::NoTimings);
    }
    // JSON `null` and `""` both mean "we have no synced version", and both
    // occur in real responses alongside a populated `plainLyrics`.
    if let Some(s) = v.get("syncedLyrics").and_then(serde_json::Value::as_str) {
        if !s.trim().is_empty() {
            return Ok(Outcome::Synced(s.to_string()));
        }
    }
    // A record we can see (it has an `id`, or lyric fields at all) but with no
    // timings is "found, nothing timed". Anything else — `{}`, `[]`, `null`, a
    // bare string — is not a record and must not be reported as one.
    let looks_like_a_record = v.get("id").is_some()
        || v.get("plainLyrics").is_some()
        || v.get("syncedLyrics").is_some()
        || v.get("instrumental").is_some();
    if looks_like_a_record {
        Ok(Outcome::NoTimings)
    } else {
        Ok(Outcome::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

// Per-thread cache-directory override, used only by the tests.
//
// Thread-*local* rather than an environment variable on purpose: `cargo test`
// runs tests concurrently, and a process-wide override is shared mutable state
// that one test can yank out from under another mid-assertion. This was not a
// hypothetical — the env-var version of this failed four tests on its first
// run. A thread-local makes each test's redirection invisible to every other
// test, so the suite is correct at any `--test-threads`.
#[cfg(test)]
thread_local! {
    static CACHE_DIR_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// Directory holding cache records. Matches how the rest of Fresco resolves
/// caches (`dirs::cache_dir()` then a `fresco` subdirectory).
fn cache_dir() -> PathBuf {
    #[cfg(test)]
    if let Some(dir) = CACHE_DIR_OVERRIDE.with(|c| c.borrow().clone()) {
        return dir;
    }
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("fresco")
        .join("lyrics")
}

/// Normalised identity of a track for caching purposes.
///
/// Case- and whitespace-insensitive, and **duration is deliberately excluded**:
/// the same track reported as 213 s by one player and 214 s by another is one
/// song, LRCLIB itself matches durations with tolerance, and folding the number
/// into the key would scatter near-duplicate entries across the cache and
/// refetch each one.
///
/// Fields are joined with U+001F (unit separator) so that `("ab", "c")` cannot
/// collide with `("a", "bc")`.
fn cache_key(q: &Query) -> String {
    let album = q.album.as_deref().unwrap_or("");
    format!(
        "{}\u{1f}{}\u{1f}{}",
        collapse_ws(&q.title).to_lowercase(),
        collapse_ws(&q.artist).to_lowercase(),
        collapse_ws(album).to_lowercase(),
    )
}

/// 64-bit FNV-1a over bytes, with a caller-chosen offset basis.
///
/// Hand-rolled because the alternative is a new dependency for something this
/// small. Not cryptographic and does not need to be: it names cache files.
fn fnv1a(bytes: &[u8], basis: u64) -> u64 {
    let mut h = basis;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Where the cache record for `q` lives.
///
/// Stable for a given track and independent of process, locale and run order,
/// so a second play finds the first play's file.
///
/// The filename is **128 bits of hash rendered as hex, and nothing else**. That
/// is not laziness: track titles legitimately contain `/`, `..`, newlines,
/// right-to-left overrides and arbitrary Unicode, and any scheme that keeps a
/// readable fragment of user-controlled text in a path is a path-traversal bug
/// waiting for the right song title. A fixed 32-character ASCII name cannot
/// escape the directory, cannot exceed `NAME_MAX`, and cannot contain a
/// separator or a NUL.
pub fn cache_path(q: &Query) -> PathBuf {
    let key = cache_key(q);
    let bytes = key.as_bytes();
    // Two independent bases give 128 bits; at that width, collisions across a
    // personal music library are not a thing that happens.
    let a = fnv1a(bytes, 0xcbf2_9ce4_8422_2325);
    let b = fnv1a(bytes, 0x9e37_79b9_7f4a_7c15);
    cache_dir().join(format!("{a:016x}{b:016x}.lrccache"))
}

/// A cache record as found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Record {
    /// Fresh `.lrc` text.
    Hit(String),
    /// Fresh "LRCLIB has this track, but with no timings" marker.
    NoTimings,
    /// Fresh "LRCLIB does not have this track" marker.
    Miss,
    /// Absent, unreadable, malformed, or past its TTL — all mean "ask again".
    Stale,
}

/// The verdict half of a cache record, borrowed from whatever the caller
/// already owns so that writing a hit does not copy the whole `.lrc` body.
#[derive(Debug, Clone, Copy)]
enum Verdict<'a> {
    Hit(&'a str),
    NoTimings,
    Miss,
}

/// Whether a record written at `written_at` is still fresh at `now`.
///
/// Pure function of two timestamps so the policy is testable without sleeping.
/// A record with a timestamp *in the future* (clock stepped back, or the file
/// was copied from another machine) is treated as stale rather than
/// valid-forever: refetching once is cheap, being wedged is not.
fn is_fresh(written_at: u64, now: u64, ttl: Duration) -> bool {
    now >= written_at && now - written_at < ttl.as_secs()
}

/// Seconds since the Unix epoch, saturating at 0 if the clock predates it.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Serialise a record. One file per track holds both the verdict and its age:
/// a header line, then the payload verbatim.
///
/// Header: `fresco-lrc-cache/1 <unix-seconds> <HIT|PLAIN|MISS>`.
///
/// `PLAIN` was added after the format was in the field, and deliberately did
/// **not** bump [`CACHE_MAGIC`]: [`decode`] already answers `Stale` to any
/// verdict token it does not recognise, so an older build reading a newer
/// cache simply refetches that one track. Bumping the magic would have thrown
/// away every user's whole cache to add one token.
fn encode(now: u64, verdict: Verdict<'_>) -> String {
    match verdict {
        Verdict::Hit(lrc) => format!("{CACHE_MAGIC} {now} HIT\n{lrc}"),
        Verdict::NoTimings => format!("{CACHE_MAGIC} {now} PLAIN\n"),
        Verdict::Miss => format!("{CACHE_MAGIC} {now} MISS\n"),
    }
}

/// Parse a record and apply the TTL for its kind. Pure, given `now`.
fn decode(raw: &str, now: u64) -> Record {
    let (header, body) = match raw.split_once('\n') {
        Some(pair) => pair,
        // A file with no newline cannot be one of ours.
        None => return Record::Stale,
    };
    let mut parts = header.split(' ');
    if parts.next() != Some(CACHE_MAGIC) {
        return Record::Stale;
    }
    let Some(written_at) = parts.next().and_then(|t| t.parse::<u64>().ok()) else {
        return Record::Stale;
    };
    match parts.next() {
        Some("HIT") if is_fresh(written_at, now, HIT_TTL) && !body.trim().is_empty() => {
            Record::Hit(body.to_string())
        }
        // Both negatives expire on `MISS_TTL`, and for the same reason: a
        // record with no timings today may be revised to carry them tomorrow,
        // exactly as an absent track may be contributed tomorrow.
        Some("PLAIN") if is_fresh(written_at, now, MISS_TTL) => Record::NoTimings,
        Some("MISS") if is_fresh(written_at, now, MISS_TTL) => Record::Miss,
        _ => Record::Stale,
    }
}

/// Read and interpret the record for `q`.
fn lookup(q: &Query) -> Record {
    match std::fs::read_to_string(cache_path(q)) {
        Ok(raw) => decode(&raw, now_secs()),
        Err(_) => Record::Stale,
    }
}

/// Locally cached `.lrc` text for `q`, if a fresh positive record exists.
///
/// `None` covers "never fetched", "fetched and there were none", and "expired"
/// alike — a caller that needs to tell those apart wants [`fetch_cached`],
/// which is the whole point of that function. Cheap: one small file read, no
/// network.
pub fn cached(q: &Query) -> Option<String> {
    match lookup(q) {
        Record::Hit(lrc) => Some(lrc),
        Record::NoTimings | Record::Miss | Record::Stale => None,
    }
}

/// Write `lrc` to the cache for `q`, stamped now.
///
/// Written to a temporary file and renamed into place, so a crash or a
/// concurrent reader never sees a half-written record. Best-effort by nature —
/// the cache is disposable — but errors are returned rather than swallowed so
/// a caller can log a genuinely broken cache directory once.
pub fn store(q: &Query, lrc: &str) -> Result<()> {
    write_record(q, encode(now_secs(), Verdict::Hit(lrc)))
}

/// Record that LRCLIB does not have `q` at all, for [`MISS_TTL`].
///
/// Separate from [`store`] because the two have very different expiry, and
/// because an empty-string "hit" would be indistinguishable from a corrupt
/// record on read.
pub fn store_miss(q: &Query) -> Result<()> {
    write_record(q, encode(now_secs(), Verdict::Miss))
}

/// Record that LRCLIB has `q` but with no timings, for [`MISS_TTL`].
///
/// Distinct from [`store_miss`] so that a replay can still tell the user *why*
/// the overlay is empty without asking the service again.
pub fn store_no_timings(q: &Query) -> Result<()> {
    write_record(q, encode(now_secs(), Verdict::NoTimings))
}

fn write_record(q: &Query, content: String) -> Result<()> {
    let path = cache_path(q);
    let dir = path
        .parent()
        .context("lyric cache path has no parent directory")?;
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating lyric cache dir {}", dir.display()))?;
    // Include the pid so two Fresco processes cannot collide on the temp name.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, content.as_bytes())
        .with_context(|| format!("writing lyric cache {}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        // Leave nothing behind if the rename is the thing that failed.
        let _ = std::fs::remove_file(&tmp);
        return Err(
            anyhow::Error::new(e).context(format!("renaming lyric cache into {}", path.display()))
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rate-limit backoff
// ---------------------------------------------------------------------------

/// Marker recording the instant LRCLIB told us to stop until.
fn backoff_path() -> PathBuf {
    cache_dir().join("rate-limit-until")
}

/// Whether a recorded backoff deadline is still in force at `now`. Pure.
///
/// A deadline absurdly far in the future is ignored: a corrupt file must not be
/// able to disable the feature permanently. One hour is far longer than any
/// `Retry-After` a lyric lookup should ever receive.
fn backoff_active(until: u64, now: u64) -> bool {
    const MAX_BACKOFF_SECS: u64 = 3600;
    now < until && until - now <= MAX_BACKOFF_SECS
}

/// Read the recorded deadline, if any.
fn backoff_until() -> Option<u64> {
    std::fs::read_to_string(backoff_path())
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

/// Record that we must not send another request for `secs`.
fn set_backoff(secs: u64) {
    let dir = cache_dir();
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(backoff_path(), (now_secs() + secs).to_string());
    }
}

/// `Retry-After` in seconds, clamped to something sane.
///
/// The header is defined as either a delay in seconds or an HTTP-date; LRCLIB
/// documents the seconds form. An unparsable or missing value still earns a
/// pause — being told "too many requests" and then not slowing down is exactly
/// what their docs warn results in a ban.
fn retry_after_secs(header: Option<&str>) -> u64 {
    const DEFAULT: u64 = 60;
    const MAX: u64 = 3600;
    header
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT)
        .min(MAX)
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

/// Fetch synced lyrics for `q` from LRCLIB. **Blocks. Worker thread only.**
///
/// - `Ok(Some(lrc))` — synced `.lrc` text, ready for `crate::lyrics::parse_lrc`.
/// - `Ok(None)` — the service answered cleanly and there is nothing usable:
///   `404 TrackNotFound`, an instrumental record, or a record with only plain
///   lyrics. This is an *answer*, not a failure, and callers should treat it as
///   "this track has no lyrics" rather than retrying.
/// - `Err(_)` — a real failure: no network, DNS failure, TLS failure, timeout,
///   a 5xx, a rate limit, or a body that is not JSON. Nothing is cached, and
///   the next call will try again.
///
/// Does **not** consult or populate the cache; [`fetch_cached`] does that. Kept
/// separate so a "refresh lyrics" action can force a real request.
///
/// See [`fetch_outcome`], which this wraps, for the retry sequence and its
/// bounds.
pub fn fetch(q: &Query) -> Result<Option<String>> {
    fetch_outcome(q).map(Outcome::into_synced)
}

/// [`fetch`], keeping the difference between "no timings" and "not in the
/// database". **Blocks. Worker thread only.**
///
/// # Retries
///
/// One request per candidate artist string (see `Query::artist_candidates`),
/// in order, stopping at
/// the first [`Outcome::Synced`] — so a single-artist track makes exactly one
/// request, and no track makes more than [`MAX_ARTIST_ATTEMPTS`] however many
/// artists it credits. Consecutive requests are spaced by
/// [`INTER_REQUEST_PAUSE`], per LRCLIB's throttling guidance.
///
/// An [`Outcome::NoTimings`] does **not** stop the sequence: it leaves the
/// overlay just as empty as a miss does, while a *different* LRCLIB record for
/// the same song — filed under one of the other credited artists — may well
/// carry timings. It is remembered as the answer to return if nothing better
/// turns up, and costs no requests beyond the cap that already bounds the
/// sequence.
///
/// A transport error aborts the whole sequence immediately rather than
/// continuing: if the network or the service is unwell, three more requests are
/// not going to help it.
pub fn fetch_outcome(q: &Query) -> Result<Outcome> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(HTTP_READ_TIMEOUT)
        .timeout(HTTP_TOTAL_TIMEOUT)
        .user_agent(USER_AGENT)
        .build();
    resolve(q, INTER_REQUEST_PAUSE, |artist| {
        request_once(&agent, q, artist)
    })
}

/// The candidate sequence itself, with the transport injected.
///
/// Split out from [`fetch_outcome`] so the ordering, the cap, the deduplication
/// and the rate-limit short-circuit are all testable without a network, and so
/// the tests can pass a zero `pause` instead of sleeping for real.
fn resolve<F>(q: &Query, pause: Duration, mut send: F) -> Result<Outcome>
where
    F: FnMut(&str) -> Result<Outcome>,
{
    // Honour a previously received `Retry-After` before opening a socket, and
    // before *any* candidate — being told to wait applies to the lookup, not to
    // one artist spelling of it. Their docs are explicit that ignoring it risks
    // a ban, and a ban would break the feature for every Fresco user, not just
    // this one.
    if let Some(until) = backoff_until() {
        let now = now_secs();
        if backoff_active(until, now) {
            anyhow::bail!(
                "LRCLIB rate limit in effect for another {}s",
                until.saturating_sub(now)
            );
        }
    }

    let mut best = Outcome::NotFound;
    for (i, artist) in q.artist_candidates().iter().enumerate() {
        if i > 0 && !pause.is_zero() {
            std::thread::sleep(pause);
        }
        match send(artist)? {
            Outcome::Synced(lrc) => return Ok(Outcome::Synced(lrc)),
            // Keep the *first* such answer: candidates run most-specific first,
            // so an earlier one is the better description of this track.
            Outcome::NoTimings => {
                if best == Outcome::NotFound {
                    best = Outcome::NoTimings;
                }
            }
            Outcome::NotFound => {}
        }
    }
    Ok(best)
}

/// One `/api/get` round trip with an explicit artist string.
fn request_once(agent: &ureq::Agent, q: &Query, artist: &str) -> Result<Outcome> {
    // `.query()` percent-encodes, which matters: titles contain `&`, `#`, `+`
    // and every kind of Unicode.
    let mut req = agent
        .get(API_GET)
        .query("track_name", &q.title)
        .query("artist_name", artist);
    if let Some(album) = q.album.as_deref() {
        req = req.query("album_name", album);
    }
    if let Some(d) = q.duration_s {
        req = req.query("duration", &d.to_string());
    }

    match req.call() {
        Ok(resp) => {
            let body = resp.into_string().context("reading LRCLIB response body")?;
            parse_outcome(&body)
        }
        // A 404 is the documented "no such track" answer and is the single most
        // common outcome. It is not an error.
        Err(ureq::Error::Status(404, _)) => Ok(Outcome::NotFound),
        Err(ureq::Error::Status(429, resp)) => {
            let wait = retry_after_secs(resp.header("Retry-After"));
            set_backoff(wait);
            anyhow::bail!("LRCLIB rate limited us; backing off for {wait}s");
        }
        Err(ureq::Error::Status(code, _)) => {
            anyhow::bail!("LRCLIB returned HTTP {code}")
        }
        Err(e) => Err(anyhow::Error::new(e).context("LRCLIB request failed")),
    }
}

/// Cache-first lyric lookup: **this is what the daemon should call.**
/// **Blocks on a cache miss. Worker thread only.**
///
/// 1. A fresh cached hit returns immediately with no network at all.
/// 2. A fresh cached miss returns `Ok(None)` with no network at all — that is
///    what stops a track with no lyrics from being re-requested on every replay.
/// 3. Otherwise it calls [`fetch`] and records the outcome, positive or
///    negative, before returning it.
///
/// A failure to *write* the cache is not a failure to fetch: the lyrics are
/// returned regardless, and the only cost of an unwritable cache directory is
/// that the next play fetches again.
pub fn fetch_cached(q: &Query) -> Result<Option<String>> {
    fetch_cached_outcome(q).map(Outcome::into_synced)
}

/// [`fetch_cached`], keeping the difference between "no timings" and "not in
/// the database" so the widget can say which. **Blocks on a cache miss.**
///
/// Exactly **one** cache record is written per lookup, whatever the outcome and
/// however many artist candidates were tried on the way to it — the record is
/// keyed on the track, not on the artist spelling that happened to answer, so a
/// replay short-circuits the entire sequence rather than repeating it.
pub fn fetch_cached_outcome(q: &Query) -> Result<Outcome> {
    cached_outcome_with(q, fetch_outcome)
}

/// [`fetch_cached_outcome`] with the fetch injected, so the cache-writing
/// policy is testable without a network.
fn cached_outcome_with<F>(q: &Query, fetch: F) -> Result<Outcome>
where
    F: FnOnce(&Query) -> Result<Outcome>,
{
    match lookup(q) {
        Record::Hit(lrc) => return Ok(Outcome::Synced(lrc)),
        Record::NoTimings => return Ok(Outcome::NoTimings),
        Record::Miss => return Ok(Outcome::NotFound),
        Record::Stale => {}
    }
    let fetched = fetch(q)?;
    let written = match &fetched {
        Outcome::Synced(lrc) => store(q, lrc),
        Outcome::NoTimings => store_no_timings(q),
        Outcome::NotFound => store_miss(q),
    };
    if let Err(e) = written {
        log::debug!("could not cache lyrics for {:?}: {e:#}", q.title);
    }
    Ok(fetched)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Every lyric body below is invented placeholder text. Real lyrics are
// third-party copyrighted content and have no business being checked into this
// repository, and the parser does not care what the words are.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpris::{NowPlaying, PlaybackStatus};

    /// Only the four fields this module reads are set; the rest come from
    /// `NowPlaying::default()` so that a field added to `mpris.rs` later does
    /// not break this file.
    fn np(title: &str, artists: &[&str], album: &str, length_us: Option<i64>) -> NowPlaying {
        NowPlaying {
            title: title.into(),
            artists: artists.iter().map(|s| (*s).to_string()).collect(),
            album: album.into(),
            length_us,
            status: PlaybackStatus::Playing,
            ..NowPlaying::default()
        }
    }

    /// The `Option`-shaped view of a response body: what a caller that only
    /// wants drawable text sees. [`parse_outcome`] is the real function; this
    /// keeps the assertions that predate [`Outcome`] reading as they did, and
    /// pins the mapping between the two.
    fn parse_response(body: &str) -> Result<Option<String>> {
        parse_outcome(body).map(Outcome::into_synced)
    }

    fn q(title: &str, artist: &str, album: Option<&str>, duration_s: Option<u32>) -> Query {
        Query {
            title: title.into(),
            artist: artist.into(),
            album: album.map(str::to_string),
            duration_s,
        }
    }

    /// Redirects this thread's cache at a private temp directory, and removes
    /// it again on drop — including when the test fails, so a panic never
    /// leaves litter in `/tmp` or state for the next run to trip over.
    struct TempCache {
        dir: PathBuf,
    }

    impl TempCache {
        fn new(tag: &str) -> Self {
            // Unique per test *and* per run: two invocations of the suite may
            // overlap, and a shared name would make them fight.
            let dir = std::env::temp_dir().join(format!(
                "fresco-lyrics-test-{}-{}-{tag}",
                std::process::id(),
                now_secs()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self::point_at(dir.clone());
            Self { dir }
        }

        fn point_at(dir: PathBuf) {
            CACHE_DIR_OVERRIDE.with(|c| *c.borrow_mut() = Some(dir));
        }
    }

    impl Drop for TempCache {
        fn drop(&mut self) {
            CACHE_DIR_OVERRIDE.with(|c| *c.borrow_mut() = None);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    // -- query_from ------------------------------------------------------

    #[test]
    fn query_from_full_metadata() {
        let got = query_from(&np(
            "Sample Title",
            &["Sample Artist"],
            "Sample Album",
            Some(213_000_000),
        ))
        .unwrap();
        assert_eq!(
            got,
            q(
                "Sample Title",
                "Sample Artist",
                Some("Sample Album"),
                Some(213)
            )
        );
    }

    #[test]
    fn query_from_missing_album_and_duration() {
        // A browser tab publishing only title+artist is the single most common
        // real case, and it must still produce a usable query.
        let got = query_from(&np("Sample Title", &["Sample Artist"], "", None)).unwrap();
        assert_eq!(got.album, None);
        assert_eq!(got.duration_s, None);

        let no_album = query_from(&np("T", &["A"], "   ", Some(90_000_000))).unwrap();
        assert_eq!(no_album.album, None);
        assert_eq!(no_album.duration_s, Some(90));
    }

    #[test]
    fn query_from_rejects_unsearchable_metadata() {
        // No title, or no artist, means there is nothing to look up: refusing
        // here is what keeps us from sending junk queries to a free service.
        assert!(query_from(&np("", &["Sample Artist"], "Al", Some(1_000_000))).is_none());
        assert!(query_from(&np("   \t ", &["Sample Artist"], "Al", None)).is_none());
        assert!(query_from(&np("Sample Title", &[], "Al", None)).is_none());
        assert!(query_from(&np("Sample Title", &["", "  "], "Al", None)).is_none());
        assert!(query_from(&np("", &[], "", None)).is_none());
    }

    #[test]
    fn query_from_normalises_whitespace_and_durations() {
        let got = query_from(&np(
            "  Spaced   Out  ",
            &["Some   Artist"],
            "  An   Album ",
            Some(1_500_000),
        ))
        .unwrap();
        assert_eq!(got.title, "Spaced Out");
        assert_eq!(got.artist, "Some Artist");
        assert_eq!(got.album.as_deref(), Some("An Album"));
        // 1.5 s truncates to 1 s, not 2 — LRCLIB matches with tolerance and
        // rounding up would be a lie about the metadata we were given.
        assert_eq!(got.duration_s, Some(1));

        // Live streams publish 0 or negative lengths; neither is a duration.
        assert_eq!(
            query_from(&np("T", &["A"], "", Some(0)))
                .unwrap()
                .duration_s,
            None
        );
        assert_eq!(
            query_from(&np("T", &["A"], "", Some(-5)))
                .unwrap()
                .duration_s,
            None
        );
        assert_eq!(
            query_from(&np("T", &["A"], "", Some(999_999)))
                .unwrap()
                .duration_s,
            None,
            "under one second is not a track length"
        );
    }

    // -- artist candidates -----------------------------------------------

    #[test]
    fn multi_artist_joins_and_offers_each_credit_as_a_fallback() {
        let got = query_from(&np("T", &["A", "B", "C"], "", None)).unwrap();
        assert_eq!(got.artist, "A, B, C");
        assert_eq!(got.artist_candidates(), ["A, B, C", "A", "B", "C"]);
    }

    #[test]
    fn a_single_artist_is_exactly_one_candidate() {
        // Regression: the common case must not gain a second request. Every
        // shape that reduces to one name belongs here.
        for artist in ["Solo", "  Solo  ", "Solo\t", " Solo\n"] {
            assert_eq!(
                q("T", artist, None, None).artist_candidates(),
                ["Solo"],
                "artist: {artist:?}"
            );
        }
        // A multi-word name is one artist, not several: no separator, no split.
        assert_eq!(
            q("T", "Anweshaa Dutta Gupta", None, None).artist_candidates(),
            ["Anweshaa Dutta Gupta"]
        );
        // The separators that need surrounding spaces must not fire mid-word.
        assert_eq!(
            q("T", "Maxwell", None, None).artist_candidates(),
            ["Maxwell"]
        );
        assert_eq!(
            q("T", "Withers", None, None).artist_candidates(),
            ["Withers"]
        );
    }

    #[test]
    fn feature_credits_are_stripped_from_every_candidate() {
        assert_eq!(
            q("T", "Alpha feat. Beta", None, None).artist_candidates(),
            ["Alpha feat. Beta", "Alpha"]
        );
        assert_eq!(
            q("T", "Alpha ft. Beta", None, None).artist_candidates(),
            ["Alpha ft. Beta", "Alpha"]
        );
        assert_eq!(
            q("T", "Alpha & Beta", None, None).artist_candidates(),
            ["Alpha & Beta", "Alpha"]
        );
        assert_eq!(
            q("T", "Alpha x Beta", None, None).artist_candidates(),
            ["Alpha x Beta", "Alpha"]
        );
        assert_eq!(
            q("T", "Alpha with Beta", None, None).artist_candidates(),
            ["Alpha with Beta", "Alpha"]
        );
        // Stripping applies per credit, not just to the first one.
        assert_eq!(
            q("T", "Alpha feat. X, Beta feat. Y", None, None).artist_candidates(),
            ["Alpha feat. X, Beta feat. Y", "Alpha", "Beta"]
        );
        // Nothing left after stripping means no candidate, not an empty query
        // — `artist_name=` matches nothing and is junk traffic.
        assert_eq!(
            q("T", "feat. Beta", None, None).artist_candidates(),
            ["feat. Beta"]
        );
        // And an artist that is *only* a separator yields nothing to ask about.
        assert!(q("T", "", None, None).artist_candidates().is_empty());
        assert!(q("T", "   ", None, None).artist_candidates().is_empty());
    }

    #[test]
    fn duplicate_credits_are_never_requested_twice() {
        // Players do publish the same name twice (a remix credited to the
        // artist and the artist's alias, an array with a repeat). Each spelling
        // must cost at most one request.
        assert_eq!(
            q("T", "Alpha, Alpha, Beta", None, None).artist_candidates(),
            ["Alpha, Alpha, Beta", "Alpha", "Beta"]
        );
        // Case alone is not a different query.
        assert_eq!(
            q("T", "Alpha, ALPHA, alpha", None, None).artist_candidates(),
            ["Alpha, ALPHA, alpha", "Alpha"]
        );
        // Nor is the full string worth repeating as its own credit.
        assert_eq!(
            q("T", "Alpha", None, None).artist_candidates(),
            ["Alpha"],
            "the full string must not be re-sent as a credit"
        );
    }

    #[test]
    fn the_attempt_cap_bounds_a_long_credit_list() {
        // A free service must not absorb one request per credited name.
        let many = "A1, A2, A3, A4, A5, A6, A7, A8";
        let got = q("T", many, None, None).artist_candidates();
        assert_eq!(got.len(), MAX_ARTIST_ATTEMPTS);
        assert_eq!(got, [many, "A1", "A2", "A3"]);

        // Semicolons are a credit separator too, and count against the cap.
        let semis = q("T", "B1; B2; B3; B4; B5", None, None).artist_candidates();
        assert_eq!(semis.len(), MAX_ARTIST_ATTEMPTS);
    }

    /// The reported failure, verbatim: a Bengali film track whose LRCLIB record
    /// is filed under the *third* credited name. The full string 404s and the
    /// old "first credit only" retry asked for "Indraadip Dasgupta", which also
    /// misses — so a track that is in the database was reported as absent.
    #[test]
    fn the_reported_four_artist_track_reaches_the_matching_name() {
        let np = np(
            "Bhalolaage Tomake",
            &["Indraadip Dasgupta, Prasen, Arijit Singh, Anweshaa Dutta Gupta"],
            "Tomake Chai (Original Motion Picture Soundtrack)",
            None,
        );
        let got = query_from(&np).unwrap().artist_candidates();
        assert_eq!(
            got,
            [
                "Indraadip Dasgupta, Prasen, Arijit Singh, Anweshaa Dutta Gupta",
                "Indraadip Dasgupta",
                "Prasen",
                "Arijit Singh",
            ]
        );
        // The whole point: the name that answers is reached, and within the cap.
        assert!(got.iter().any(|c| c == "Arijit Singh"));
        assert!(got.len() <= MAX_ARTIST_ATTEMPTS);
    }

    // -- parse_response --------------------------------------------------

    #[test]
    fn parses_a_synced_response() {
        let body = r#"{
            "id": 1,
            "trackName": "Sample Title",
            "artistName": "Sample Artist",
            "albumName": "Sample Album",
            "duration": 213,
            "instrumental": false,
            "plainLyrics": "placeholder line one\nplaceholder line two",
            "syncedLyrics": "[00:12.00] placeholder line one\n[00:15.00] placeholder line two"
        }"#;
        let got = parse_response(body).unwrap().unwrap();
        assert!(got.starts_with("[00:12.00]"));
        // Returned verbatim: parsing is `crate::lyrics::parse_lrc`'s job, and
        // any trimming here would silently change timings.
        assert!(got.contains("[00:15.00] placeholder line two"));
    }

    #[test]
    fn duration_may_be_fractional() {
        // The live service answers with `"duration": 214.0`, not an integer.
        // Deserialising into a struct with `duration: u32` would fail the whole
        // response, so nothing here reads that field.
        let body = r#"{"id":1,"duration":214.0,"instrumental":false,
                       "plainLyrics":"x","syncedLyrics":"[00:01.00] x"}"#;
        assert_eq!(
            parse_response(body).unwrap().as_deref(),
            Some("[00:01.00] x")
        );
    }

    #[test]
    fn plain_only_is_not_usable() {
        // A timed overlay cannot be driven by untimed text, and pinning a
        // static block over the wallpaper for four minutes is worse than
        // showing nothing.
        let missing = r#"{"id":2,"instrumental":false,"plainLyrics":"placeholder line"}"#;
        let null = r#"{"id":2,"instrumental":false,"plainLyrics":"x","syncedLyrics":null}"#;
        let empty = r#"{"id":2,"instrumental":false,"plainLyrics":"x","syncedLyrics":""}"#;
        let blank = r#"{"id":2,"instrumental":false,"plainLyrics":"x","syncedLyrics":"  \n "}"#;
        for body in [missing, null, empty, blank] {
            assert_eq!(parse_response(body).unwrap(), None, "body: {body}");
            // …but it is still a *record*, and that is a different fact from
            // "not in the database". This is what lets the widget say so.
            assert_eq!(
                parse_outcome(body).unwrap(),
                Outcome::NoTimings,
                "body: {body}"
            );
        }
    }

    #[test]
    fn found_without_timings_is_distinguishable_from_not_found() {
        let plain = r#"{"id":6569086,"trackName":"Bhalolaage Tomake",
                        "instrumental":false,"plainLyrics":"placeholder line",
                        "syncedLyrics":null}"#;
        let absent = r#"{"code":404,"name":"TrackNotFound",
                         "message":"Failed to find specified track"}"#;
        assert_eq!(parse_outcome(plain).unwrap(), Outcome::NoTimings);
        assert_eq!(parse_outcome(absent).unwrap(), Outcome::NotFound);
        // Both are still "nothing to draw" to the `Option`-shaped callers, so
        // no existing behaviour changed.
        assert_eq!(parse_response(plain).unwrap(), None);
        assert_eq!(parse_response(absent).unwrap(), None);

        // An instrumental is a record too: "no synced lyrics for this track" is
        // true of it, "not found" would not be.
        let instrumental = r#"{"id":3,"instrumental":true,"plainLyrics":null}"#;
        assert_eq!(parse_outcome(instrumental).unwrap(), Outcome::NoTimings);

        // Shapes with no record in them must not claim one was found.
        for body in ["{}", "[]", "null", r#""a string""#, "42"] {
            assert_eq!(
                parse_outcome(body).unwrap(),
                Outcome::NotFound,
                "body: {body}"
            );
        }
    }

    #[test]
    fn instrumental_wins_over_any_lyrics_present() {
        let body = r#"{"id":3,"instrumental":true,
                       "plainLyrics":null,"syncedLyrics":"[00:01.00] placeholder"}"#;
        assert_eq!(parse_response(body).unwrap(), None);
    }

    #[test]
    fn track_not_found_body_is_a_clean_miss() {
        // The exact 404 body the live service returns.
        let body = r#"{"code":404,"name":"TrackNotFound",
                       "message":"Failed to find specified track"}"#;
        assert_eq!(parse_response(body).unwrap(), None);
        // Any other error object is a miss too, not a panic.
        let other = r#"{"code":429,"name":"TooManyRequests","message":"Rate limit exceeded"}"#;
        assert_eq!(parse_response(other).unwrap(), None);
    }

    #[test]
    fn malformed_bodies_error_rather_than_panic() {
        // Captive portals, proxy error pages and truncated responses all land
        // here. They must be errors: a clean miss earns a 24 h negative cache
        // entry, and a flaky minute must not silence a track for a day.
        for body in [
            "",
            "not json at all",
            "<html><body>502 Bad Gateway</body></html>",
            r#"{"syncedLyrics": "unterminated"#,
            "\u{feff}{}",
        ] {
            assert!(parse_response(body).is_err(), "body: {body:?}");
        }
        // Valid JSON of an unexpected shape is a miss, not an error: the
        // service answered, it just had nothing for us.
        assert_eq!(parse_response("{}").unwrap(), None);
        assert_eq!(parse_response("[]").unwrap(), None);
        assert_eq!(parse_response("null").unwrap(), None);
        assert_eq!(parse_response(r#""a string""#).unwrap(), None);
        assert_eq!(parse_response(r#"{"syncedLyrics": 42}"#).unwrap(), None);
    }

    // -- cache keys ------------------------------------------------------

    #[test]
    fn cache_path_is_stable_and_discriminating() {
        let a = q("Sample Title", "Sample Artist", Some("Album"), Some(213));
        assert_eq!(cache_path(&a), cache_path(&a.clone()));
        // Duration is deliberately not part of the identity: one song reported
        // as 213 s and 214 s must not become two cache entries.
        let other_duration = q("Sample Title", "Sample Artist", Some("Album"), Some(999));
        assert_eq!(cache_path(&a), cache_path(&other_duration));
        // Case and stray whitespace are not identity either.
        let noisy = q("  sample   TITLE ", "SAMPLE artist", Some("album"), None);
        assert_eq!(cache_path(&a), cache_path(&noisy));

        // Everything that *is* identity must separate.
        for different in [
            q("Other Title", "Sample Artist", Some("Album"), Some(213)),
            q("Sample Title", "Other Artist", Some("Album"), Some(213)),
            q("Sample Title", "Sample Artist", Some("Other"), Some(213)),
            q("Sample Title", "Sample Artist", None, Some(213)),
        ] {
            assert_ne!(cache_path(&a), cache_path(&different), "{different:?}");
        }
    }

    #[test]
    fn field_boundaries_cannot_be_smuggled_across() {
        // Without a separator, ("ab","c") and ("a","bc") would hash the same
        // bytes and share one cache entry, showing one song's words over
        // another's.
        assert_ne!(
            cache_path(&q("ab", "c", None, None)),
            cache_path(&q("a", "bc", None, None))
        );
        assert_ne!(
            cache_path(&q("a", "b", Some("c"), None)),
            cache_path(&q("a", "bc", None, None))
        );
    }

    #[test]
    fn cache_filenames_survive_hostile_titles() {
        let hostile = [
            q("../../../../etc/passwd", "x", None, None),
            q("/absolute/path", "x", None, None),
            q("with\0nul", "x", None, None),
            q("with/slash", "with\\backslash", None, None),
            q("\n\r\t", "x", None, None),
            q(".", "..", None, None),
            q("CON", "PRN", None, None),
            q("日本語のタイトル", "アーティスト", Some("アルバム"), None),
            q("emoji \u{1f3b5}\u{1f3b6}", "\u{1f984}", None, None),
            q("rtl \u{202e}txet\u{202c}", "x", None, None),
            q(&"x".repeat(4096), &"y".repeat(4096), None, None),
        ];
        let dir = cache_dir();
        for hq in &hostile {
            let path = cache_path(hq);
            assert_eq!(path.parent(), Some(dir.as_path()), "escaped dir: {hq:?}");

            let name = path.file_name().unwrap().to_str().unwrap();
            assert_eq!(name.len(), 32 + ".lrccache".len(), "name: {name}");
            // NAME_MAX is 255 on every filesystem Fresco targets; a fixed
            // 41-char ASCII name cannot approach it however long the title is.
            assert!(name.len() < 255);
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_hexdigit() || c == '.' || c.is_ascii_lowercase()),
                "non-safe character in {name}"
            );
            assert!(!name.contains('/') && !name.contains('\\') && !name.contains('\0'));
            assert!(!name.starts_with('.'), "hidden file: {name}");
        }
        // Distinct hostile inputs must still get distinct files.
        let mut names: Vec<_> = hostile.iter().map(cache_path).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), hostile.len(), "hostile inputs collided");
    }

    // -- TTL logic (pure) ------------------------------------------------

    #[test]
    fn freshness_is_a_pure_function_of_timestamps() {
        let ttl = Duration::from_secs(100);
        assert!(is_fresh(1_000, 1_000, ttl), "written this instant");
        assert!(is_fresh(1_000, 1_099, ttl), "one second inside the window");
        assert!(
            !is_fresh(1_000, 1_100, ttl),
            "exactly at the TTL is expired"
        );
        assert!(!is_fresh(1_000, 9_999, ttl));
        // Clock stepped backwards, or the file came from another machine.
        // Treating a future stamp as valid-forever would wedge the cache.
        assert!(!is_fresh(2_000, 1_000, ttl));
    }

    #[test]
    fn negative_records_expire_far_sooner_than_positive_ones() {
        // The policy this module is built around: a miss is a fact about the
        // database today, a hit is a fact about a released recording.
        assert!(MISS_TTL < HIT_TTL);
        let base = 1_700_000_000;
        let hit = encode(base, Verdict::Hit("[00:01.00] placeholder"));
        let miss = encode(base, Verdict::Miss);
        let plain = encode(base, Verdict::NoTimings);

        let day = 24 * 60 * 60;
        assert!(matches!(decode(&hit, base + day + 1), Record::Hit(_)));
        assert_eq!(decode(&miss, base + day + 1), Record::Stale);
        assert_eq!(decode(&miss, base + day - 1), Record::Miss);
        assert_eq!(decode(&hit, base + 31 * day), Record::Stale);
        // "Found, but no timings" is a negative too: a record can be revised to
        // carry timings, so it expires with the misses rather than the hits.
        assert_eq!(decode(&plain, base + day - 1), Record::NoTimings);
        assert_eq!(decode(&plain, base + day + 1), Record::Stale);
    }

    #[test]
    fn corrupt_records_are_refetched_not_trusted() {
        let now = 1_700_000_000;
        for raw in [
            "",
            "garbage",
            "fresco-lrc-cache/0 1700000000 HIT\nbody",
            "fresco-lrc-cache/1 notanumber HIT\nbody",
            "fresco-lrc-cache/1 1700000000 WAT\nbody",
            "fresco-lrc-cache/1 1700000000 HIT",
            // A HIT with an empty payload is indistinguishable from a
            // truncated write, so it is not trusted as lyrics.
            "fresco-lrc-cache/1 1700000000 HIT\n   \n  ",
        ] {
            assert_eq!(decode(raw, now), Record::Stale, "raw: {raw:?}");
        }
    }

    #[test]
    fn record_encoding_round_trips_multiline_bodies() {
        let now = 1_700_000_000;
        // `.lrc` bodies contain newlines, blank lines and a trailing newline;
        // the header must be the only line the decoder consumes.
        let lrc = "[00:01.00] placeholder one\n\n[00:09.50] placeholder two\n";
        match decode(&encode(now, Verdict::Hit(lrc)), now) {
            Record::Hit(got) => assert_eq!(got, lrc),
            other => panic!("expected a hit, got {other:?}"),
        }
        assert_eq!(decode(&encode(now, Verdict::Miss), now), Record::Miss);
        assert_eq!(
            decode(&encode(now, Verdict::NoTimings), now),
            Record::NoTimings
        );
        // The three verdicts must not be confusable on read.
        assert_ne!(
            encode(now, Verdict::Miss),
            encode(now, Verdict::NoTimings),
            "a miss and a timings-less record share one on-disk form"
        );
    }

    // -- cache round-trip (temp dir) -------------------------------------

    #[test]
    fn cache_round_trip_and_isolation() {
        let _tmp = TempCache::new("roundtrip");
        let a = q("Sample Title", "Sample Artist", Some("Album"), Some(213));
        let b = q("Other Title", "Sample Artist", Some("Album"), Some(213));

        assert_eq!(cached(&a), None, "nothing cached yet");
        let lrc = "[00:01.00] placeholder one\n[00:05.00] placeholder two\n";
        store(&a, lrc).unwrap();
        assert_eq!(cached(&a).as_deref(), Some(lrc));
        assert_eq!(cached(&b), None, "one track's cache is not another's");

        // Overwriting replaces rather than appends.
        let updated = "[00:02.00] replacement\n";
        store(&a, updated).unwrap();
        assert_eq!(cached(&a).as_deref(), Some(updated));

        // A negative marker reads back as "no lyrics", not as a hit.
        store_miss(&b).unwrap();
        assert_eq!(cached(&b), None);
        assert!(matches!(lookup(&b), Record::Miss));
        assert!(matches!(lookup(&a), Record::Hit(_)));

        // No temporary files left behind by either write.
        let leftovers: Vec<_> = std::fs::read_dir(cache_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("tmp"))
            .collect();
        assert!(leftovers.is_empty(), "left temp files: {leftovers:?}");
    }

    #[test]
    fn a_fresh_negative_short_circuits_the_network() {
        let _tmp = TempCache::new("negative");
        let track = q("Instrumental Piece", "Sample Artist", None, Some(180));
        store_miss(&track).unwrap();
        // `fetch_cached` must answer from disk here. If it ever reached the
        // network this test would be slow and flaky, which is the signal.
        let started = std::time::Instant::now();
        assert_eq!(fetch_cached(&track).unwrap(), None);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "fetch_cached hit the network despite a fresh negative record"
        );
    }

    #[test]
    fn a_fresh_positive_short_circuits_the_network() {
        let _tmp = TempCache::new("positive");
        let track = q("Cached Track", "Sample Artist", Some("Album"), Some(200));
        let lrc = "[00:03.00] placeholder\n";
        store(&track, lrc).unwrap();
        let started = std::time::Instant::now();
        assert_eq!(fetch_cached(&track).unwrap().as_deref(), Some(lrc));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_fresh_timings_less_record_short_circuits_the_network() {
        let _tmp = TempCache::new("plain");
        let track = q("Bhalolaage Tomake", "Arijit Singh", None, Some(300));
        store_no_timings(&track).unwrap();
        let started = std::time::Instant::now();
        // The distinction survives a replay, so the widget can still say
        // *why* it is empty without asking the service again.
        assert_eq!(fetch_cached_outcome(&track).unwrap(), Outcome::NoTimings);
        assert_eq!(fetch_cached(&track).unwrap(), None);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "fetch_cached hit the network despite a fresh record"
        );
    }

    // -- the candidate sequence (no network) -----------------------------

    /// Runs `resolve` with a recording transport and no inter-request pause,
    /// returning the artist strings that were actually requested.
    fn attempts_for(
        query: &Query,
        mut answer: impl FnMut(&str) -> Outcome,
    ) -> (Vec<String>, Outcome) {
        let mut tried = Vec::new();
        let outcome = resolve(query, Duration::ZERO, |artist| {
            tried.push(artist.to_string());
            Ok(answer(artist))
        })
        .expect("no transport failure was injected");
        (tried, outcome)
    }

    #[test]
    fn a_single_artist_track_makes_exactly_one_request() {
        let _tmp = TempCache::new("one-request");
        let (tried, outcome) = attempts_for(&q("T", "Solo", None, None), |_| Outcome::NotFound);
        assert_eq!(tried, ["Solo"], "a solo artist earned a second request");
        assert_eq!(outcome, Outcome::NotFound);

        // And it stops at the first hit rather than confirming it.
        let (tried, outcome) = attempts_for(&q("T", "Solo", None, None), |_| {
            Outcome::Synced("[00:01.00] x".into())
        });
        assert_eq!(tried.len(), 1);
        assert_eq!(outcome, Outcome::Synced("[00:01.00] x".into()));
    }

    #[test]
    fn the_sequence_stops_at_the_first_synced_hit() {
        let _tmp = TempCache::new("stops");
        let track = q("T", "A, B, C", None, None);
        let (tried, outcome) = attempts_for(&track, |artist| {
            if artist == "B" {
                Outcome::Synced("[00:01.00] x".into())
            } else {
                Outcome::NotFound
            }
        });
        assert_eq!(tried, ["A, B, C", "A", "B"], "did not stop at the hit");
        assert!(matches!(outcome, Outcome::Synced(_)));
    }

    #[test]
    fn the_reported_track_resolves_on_the_third_credited_name() {
        let _tmp = TempCache::new("bengali");
        // Exactly what the live service answers: 404 for the full string and
        // for the first two credits, and a plain-lyrics record for the third.
        let track = q(
            "Bhalolaage Tomake",
            "Indraadip Dasgupta, Prasen, Arijit Singh, Anweshaa Dutta Gupta",
            Some("Tomake Chai (Original Motion Picture Soundtrack)"),
            None,
        );
        let (tried, outcome) = attempts_for(&track, |artist| {
            if artist == "Arijit Singh" {
                Outcome::NoTimings
            } else {
                Outcome::NotFound
            }
        });
        assert!(tried.contains(&"Arijit Singh".to_string()));
        assert!(tried.len() <= MAX_ARTIST_ATTEMPTS);
        // Before the fix this was indistinguishable from "not in the database".
        assert_eq!(outcome, Outcome::NoTimings);
    }

    #[test]
    fn no_track_exceeds_the_attempt_cap() {
        let _tmp = TempCache::new("cap");
        for artist in [
            "A1, A2, A3, A4, A5, A6, A7, A8",
            "A1; A2; A3; A4; A5; A6",
            "A1 feat. B, A2 feat. C, A3 feat. D, A4, A5",
        ] {
            let (tried, _) = attempts_for(&q("T", artist, None, None), |_| Outcome::NotFound);
            assert!(
                tried.len() <= MAX_ARTIST_ATTEMPTS,
                "{} requests for {artist:?}",
                tried.len()
            );
        }
    }

    #[test]
    fn no_artist_string_is_requested_twice() {
        let _tmp = TempCache::new("dedupe");
        let (tried, _) = attempts_for(&q("T", "Alpha, Alpha, ALPHA, Beta", None, None), |_| {
            Outcome::NotFound
        });
        let mut unique = tried.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), tried.len(), "repeated a request: {tried:?}");
    }

    #[test]
    fn a_later_synced_record_beats_an_earlier_timings_less_one() {
        let _tmp = TempCache::new("upgrade");
        // A plain-lyrics record leaves the overlay just as empty as a miss, so
        // the sequence keeps going — within the cap — and returns the timed
        // record it finds under another credited name.
        let (tried, outcome) =
            attempts_for(&q("T", "A, B, C", None, None), |artist| match artist {
                "A" => Outcome::NoTimings,
                "C" => Outcome::Synced("[00:02.00] x".into()),
                _ => Outcome::NotFound,
            });
        assert_eq!(tried, ["A, B, C", "A", "B", "C"]);
        assert!(matches!(outcome, Outcome::Synced(_)));

        // With nothing better anywhere, the timings-less answer is what the
        // caller gets — not a bare "not found".
        let (_, outcome) = attempts_for(&q("T", "A, B, C", None, None), |artist| {
            if artist == "B" {
                Outcome::NoTimings
            } else {
                Outcome::NotFound
            }
        });
        assert_eq!(outcome, Outcome::NoTimings);
    }

    #[test]
    fn a_transport_failure_aborts_the_whole_sequence() {
        let _tmp = TempCache::new("transport");
        let mut tried = 0;
        let got = resolve(&q("T", "A, B, C", None, None), Duration::ZERO, |_| {
            tried += 1;
            anyhow::bail!("connection reset")
        });
        assert!(got.is_err());
        assert_eq!(tried, 1, "kept hammering a service that is failing");
    }

    #[test]
    fn an_active_backoff_short_circuits_every_candidate() {
        let _tmp = TempCache::new("backoff-seq");
        std::fs::create_dir_all(cache_dir()).unwrap();
        std::fs::write(backoff_path(), (now_secs() + 120).to_string()).unwrap();
        let mut tried = 0;
        let got = resolve(&q("T", "A, B, C, D, E", None, None), Duration::ZERO, |_| {
            tried += 1;
            Ok(Outcome::NotFound)
        });
        assert!(got.is_err(), "ignored a Retry-After we were given");
        assert_eq!(tried, 0, "sent {tried} request(s) while rate limited");
    }

    #[test]
    fn a_miss_across_every_candidate_writes_one_negative_record() {
        let _tmp = TempCache::new("one-negative");
        let track = q("T", "A1, A2, A3, A4, A5", Some("Album"), Some(200));

        let mut requests = 0;
        let outcome = cached_outcome_with(&track, |query| {
            resolve(query, Duration::ZERO, |_| {
                requests += 1;
                Ok(Outcome::NotFound)
            })
        })
        .unwrap();
        assert_eq!(outcome, Outcome::NotFound);
        assert!(requests > 1, "the sequence did not actually run");

        // One record for the track, not one per artist candidate: the cache is
        // keyed on the track, so a replay skips the entire sequence.
        let records: Vec<_> = std::fs::read_dir(cache_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".lrccache"))
            .collect();
        assert_eq!(records.len(), 1, "wrote {records:?}");
        assert!(matches!(lookup(&track), Record::Miss));

        let mut replayed = false;
        let outcome = cached_outcome_with(&track, |_| {
            replayed = true;
            Ok(Outcome::NotFound)
        })
        .unwrap();
        assert!(!replayed, "a replay repeated the whole candidate sequence");
        assert_eq!(outcome, Outcome::NotFound);
    }

    #[test]
    fn a_timings_less_result_is_cached_as_itself() {
        let _tmp = TempCache::new("cache-plain");
        let track = q("T", "A, B", None, None);
        let outcome = cached_outcome_with(&track, |query| {
            resolve(query, Duration::ZERO, |artist| {
                Ok(if artist == "B" {
                    Outcome::NoTimings
                } else {
                    Outcome::NotFound
                })
            })
        })
        .unwrap();
        assert_eq!(outcome, Outcome::NoTimings);
        // Not flattened into a plain miss on the way to disk.
        assert!(matches!(lookup(&track), Record::NoTimings));
        assert_eq!(fetch_cached_outcome(&track).unwrap(), Outcome::NoTimings);
    }

    #[test]
    fn store_reports_an_unusable_cache_directory() {
        let tmp = TempCache::new("unusable");
        // A regular file where the directory should be: `create_dir_all` fails,
        // and the caller learns about it instead of silently losing the cache.
        let blocker = tmp.dir.join("blocked");
        std::fs::write(&blocker, b"not a directory").unwrap();
        TempCache::point_at(blocker);

        let track = q("T", "A", None, None);
        assert!(store(&track, "[00:01.00] placeholder").is_err());
        assert!(store_miss(&track).is_err());
        // An unwritable cache must degrade to "no cache", never to a panic or
        // a poisoned read.
        assert_eq!(cached(&track), None);
    }

    // -- rate-limit backoff ----------------------------------------------

    #[test]
    fn retry_after_is_honoured_and_clamped() {
        assert_eq!(retry_after_secs(Some("30")), 30);
        assert_eq!(retry_after_secs(Some("  45 ")), 45);
        // Missing, unparsable, zero and negative all still earn a pause: being
        // told "too many requests" and not slowing down is what gets clients
        // banned.
        assert_eq!(retry_after_secs(None), 60);
        assert_eq!(retry_after_secs(Some("")), 60);
        assert_eq!(retry_after_secs(Some("Wed, 21 Oct 2015 07:28:00 GMT")), 60);
        assert_eq!(retry_after_secs(Some("0")), 60);
        assert_eq!(retry_after_secs(Some("-5")), 60);
        // A hostile or buggy value must not disable lyrics for a week.
        assert_eq!(retry_after_secs(Some("999999999")), 3600);
    }

    #[test]
    fn backoff_window_is_bounded_in_both_directions() {
        let now = 1_700_000_000;
        assert!(!backoff_active(now, now), "expired exactly now");
        assert!(!backoff_active(now - 1, now), "already past");
        assert!(backoff_active(now + 1, now));
        assert!(backoff_active(now + 3600, now));
        // A corrupt far-future deadline must not permanently disable lookups.
        assert!(!backoff_active(now + 3601, now));
        assert!(!backoff_active(u64::MAX, now));
    }

    #[test]
    fn an_active_backoff_refuses_to_open_a_socket() {
        let _tmp = TempCache::new("backoff");
        std::fs::create_dir_all(cache_dir()).unwrap();
        std::fs::write(backoff_path(), (now_secs() + 120).to_string()).unwrap();
        let started = std::time::Instant::now();
        assert!(fetch(&q("T", "A", None, None)).is_err());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "fetch tried the network while rate limited"
        );
    }

    // -- constants -------------------------------------------------------

    #[test]
    fn user_agent_matches_the_shape_lrclib_asks_for() {
        // "application's name, version, and a link to its homepage or project
        // page" — e.g. `LRCGET v0.2.0 (https://github.com/…)`.
        assert!(USER_AGENT.starts_with("Fresco v"));
        assert!(USER_AGENT.contains("github.com/DibbayajyotiRoy/fresco"));
        assert!(USER_AGENT.ends_with(')'));
        assert!(USER_AGENT.contains(env!("CARGO_PKG_VERSION")));
        // Must be a legal header value: no controls, no newlines.
        assert!(USER_AGENT.is_ascii());
        assert!(!USER_AGENT.chars().any(char::is_control));
    }

    #[test]
    fn attribution_credits_the_source_and_disclaims_ownership() {
        assert!(ATTRIBUTION.contains("LRCLIB"));
        assert!(ATTRIBUTION.contains("lrclib.net"));
        assert!(ATTRIBUTION.contains("does not host, own or license"));
    }

    #[test]
    fn artist_candidates_never_panic_on_unicode() {
        // Regression guard. The separator scan folds case before searching, and
        // full Unicode lowercasing can change a string's byte length (U+0130
        // LATIN CAPITAL I WITH DOT ABOVE folds to two chars), which would make
        // an index from the folded string slice the original mid-character and
        // panic. ASCII-only folding is what keeps the indices aligned.
        for artist in [
            "\u{130}stanbul feat. Guest",
            "\u{130}\u{130}\u{130} & Other",
            "\u{fdfa} x \u{1f984}",
            "\u{1f3b5}\u{1f3b6} with \u{1f984}",
            "\u{202e}esrever\u{202c} feat. X",
            "\u{feff}\u{feff}",
            "\u{130}",
            // The same shapes again as one credit among several, since every
            // credit now goes through the same scan, not just the first.
            "Safe, \u{130}stanbul feat. Guest, \u{130}",
            "\u{130}; \u{130}\u{130} & X; \u{fdfa} with Y",
        ] {
            let candidates = q("T", artist, None, None).artist_candidates();
            // The exact result is unspecified; not panicking is the assertion,
            // plus the invariants that hold for every input.
            assert!(candidates.len() <= MAX_ARTIST_ATTEMPTS);
            assert!(candidates.iter().all(|c| !c.trim().is_empty()));
        }
        // U+0130 lowercases to two chars; a truncation bug here would surface
        // as a panic or as a mangled candidate, not as a wrong lookup.
        assert_eq!(
            q("T", "\u{130}stanbul feat. Guest", None, None).artist_candidates(),
            ["\u{130}stanbul feat. Guest", "\u{130}stanbul"]
        );
    }

    // -- live network (opt-in only) --------------------------------------

    /// Real request against LRCLIB. Ignored by default so CI never depends on
    /// a third-party service: run with `cargo test -- --ignored live_lrclib`.
    #[test]
    #[ignore = "hits the live LRCLIB service"]
    fn live_lrclib_returns_synced_lyrics() {
        let _tmp = TempCache::new("live");
        let track = q(
            "Never Gonna Give You Up",
            "Rick Astley",
            Some("Whenever You Need Somebody"),
            Some(213),
        );
        let got = fetch(&track).expect("transport failure");
        let lrc = got.expect("expected synced lyrics for a very well-known track");
        assert!(lrc.contains("[00:"), "does not look like .lrc text");
        assert!(!crate::lyrics::parse_lrc(&lrc).is_empty());
    }

    /// A signature that certainly does not exist must be a clean miss, not an
    /// error — this is the branch that decides whether we cache a negative.
    #[test]
    #[ignore = "hits the live LRCLIB service"]
    fn live_lrclib_unknown_track_is_a_clean_miss() {
        let _tmp = TempCache::new("live-miss");
        let track = q(
            "zzz fresco nonexistent track zzz",
            "zzz fresco nonexistent artist zzz",
            None,
            Some(1),
        );
        assert_eq!(fetch(&track).expect("transport failure"), None);
    }

    /// The reported failure, end to end against the live service. Before the
    /// candidate list this answered `NotFound`, because only the full string
    /// and the first credit were ever tried and neither matches.
    #[test]
    #[ignore = "hits the live LRCLIB service"]
    fn live_lrclib_finds_the_reported_bengali_track() {
        let _tmp = TempCache::new("live-bengali");
        let track = query_from(&np(
            "Bhalolaage Tomake",
            &["Indraadip Dasgupta, Prasen, Arijit Singh, Anweshaa Dutta Gupta"],
            "Tomake Chai (Original Motion Picture Soundtrack)",
            None,
        ))
        .unwrap();
        // Not asserted as exactly `NoTimings`: the record (id 6569086) carries
        // only plain lyrics today, but LRCLIB takes contributions and a synced
        // version landing tomorrow must not turn into a red build.
        assert_ne!(
            fetch_outcome(&track).expect("transport failure"),
            Outcome::NotFound,
            "the track is in LRCLIB under its third credited artist"
        );
    }
}

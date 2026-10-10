//! Who is locked out: login, display name and avatar for the lock-screen
//! greeting/avatar widgets (lock-screen aesthetics feature, wave 1 — the
//! daemon-side glue that turns this into a card lands separately, in wave 2;
//! this module only owns the data).
//!
//! # Identity sources
//!
//! AccountsService (`org.freedesktop.Accounts`, system bus) is asked first for
//! the display name, because it is what every desktop's own greeter and
//! user-switcher already read — GNOME, KDE and COSMIC all let a user set a
//! "full name" and a picture through it, and neither is guaranteed to appear
//! anywhere in `/etc/passwd`. `/etc/passwd` is the fallback: the GECOS field's
//! first comma-separated entry is the traditional home of a real name, and it
//! exists on every Linux system with no service required.
//!
//! # Where the avatar comes from
//!
//! Desktops do not agree on where a user's picture lives, so [`current`] walks
//! an ordered list (`avatar_source_order`) and takes the first candidate that
//! is a readable PNG/JPEG/WebP (`avatar_file_ok`):
//!
//! 1. **dde-daemon's own account service** — `org.deepin.dde.Accounts1`
//!    (deepin 25; `com.deepin.daemon.Accounts` on deepin 20/23), user object
//!    `.../User<uid>`, property `IconFile`. deepin's Control Center edits
//!    *this* service, not accounts-daemon: `User.SetIconFile` copies the new
//!    picture to `/var/lib/AccountsService/icons/local/<login>-<ns>.png` and
//!    records it in `/var/lib/AccountsService/deepin/users/<login>`, and
//!    nothing under `accounts1/` ever calls `org.freedesktop.Accounts`, so
//!    accounts-daemon's own record (when it has one at all) is not the picture
//!    the user chose (`linuxdeepin/dde-daemon`, `accounts1/user.go` and
//!    `accounts1/user_ifc.go`).
//!    The value is a **`file://` URI** (`defaultUserIcon` is
//!    `file:///var/lib/AccountsService/icons/default`, a symlink into
//!    `dde-account-faces`' `icons/animal/*.png` set), so it has to be decoded
//!    before it is a path (`parse_icon_location`). On deepin this source is
//!    tried first; elsewhere it is a cheap last resort (the call fails fast
//!    with `ServiceUnknown`).
//! 2. **dde-daemon's per-user keyfile**, `/var/lib/AccountsService/deepin/
//!    users/<login>`, `[User]` `Icon=` — the same record read straight off disk
//!    for when the bus is unavailable or slow.
//! 3. **AccountsService** — `FindUserById` (so the user object is guaranteed to
//!    exist; accounts-daemon only exports users it has cached) then
//!    `IconFile`.
//! 4. `/var/lib/AccountsService/icons/<login>`, where stock accountsservice
//!    keeps its copy of the picture.
//! 5. `~/.face`, then `~/.face.icon`.
//!
//! A candidate that exists but is not a decodable raster (an SVG, a
//! truncated file) does **not** end the search: the next source gets its
//! turn, and if every source fails the greeting draws the user's initials
//! ([`initials`]) rather than an app-style placeholder.
//!
//! No D-Bus crate, matching `src/mpris.rs` and `src/daemon/dde.rs`: we shell
//! out to `gdbus`, which both Flatpak runtimes ship. Unlike those two modules
//! this one talks to the **system** bus, not the session bus — accounts are
//! system-wide state, not a per-session one — and it only ever needs to parse
//! a single string variant, never a dictionary, so it gets its own tiny
//! parser, `parse_gvariant_string`, rather than reusing
//! [`crate::mpris::parse_gvariant`]. Two reasons, not one: that parser handles
//! a much bigger grammar than this needs, and reusing it would pull the
//! `daemon` feature gate `mpris` lives behind into a module that otherwise
//! needs nothing beyond `std` and the crate's own i18n macros.
//!
//! # Bounded time, never panics
//!
//! [`current`] does real I/O — a handful of `gdbus --system --timeout 2` round
//! trips in the worst case, plus some local file reads — but it runs once per
//! lock, not on a render loop. The first `gdbus` call that *times out* (as
//! opposed to failing fast, which is what an absent service does) marks the
//! bus as wedged and every later call in the same resolution is skipped, so a
//! hung accounts service costs one timeout, not one per question. Every
//! failure mode (`gdbus` missing, no system bus, no AccountsService, an
//! unreadable `/etc/passwd`) degrades to a fallback rather than an error or a
//! hang.
//!
//! # Pure helpers
//!
//! [`first_name`], [`initials`] and [`greeting`] take already-resolved data
//! and do no I/O at all, so the lock-screen card layout can call them straight
//! from a render function without worrying about blocking. The avatar
//! resolver is built the same way: `parse_icon_location`,
//! `keyfile_value`, `avatar_source_order` and `pick_avatar` are pure and
//! unit-tested; only the thin layer that actually asks `gdbus` or the
//! filesystem is not.

use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

/// `gdbus --timeout`, seconds. Matches `mpris.rs`'s reasoning: short enough
/// that a wedged or absent AccountsService cannot hold up the lock screen.
const CALL_TIMEOUT_SECS: &str = "2";

/// Who is logged in, resolved once per lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserInfo {
    /// Unix login name. Always set — see [`current`]'s fallback chain.
    pub login: String,
    /// Display name, when one could be found. Never `Some("")` or
    /// whitespace-only; see [`current`]'s filtering.
    pub real_name: Option<String>,
    /// Path to an avatar image that exists, could be opened for reading and
    /// starts with a PNG/JPEG/WebP signature, when one could be found. See the
    /// module docs for the sources and their order.
    pub avatar: Option<PathBuf>,
}

/// Resolve [`UserInfo`] for the process's own user. Does I/O; never panics;
/// bounded time (see the module docs).
pub fn current() -> UserInfo {
    resolve_user(true)
}

/// [`current`] without the avatar: `avatar` is always `None` and no picture
/// source is consulted. For callers that only want the name — the settings
/// page's greeting placeholder, a lock whose Avatar widget is off, where a
/// photo (more identifying than a first name) should not even be looked up.
pub fn current_identity() -> UserInfo {
    resolve_user(false)
}

fn resolve_user(with_avatar: bool) -> UserInfo {
    let who = Who::detect();
    let mut bus = SystemBus::default();

    let real_name = who
        .uid
        .and_then(|u| bus.accounts_service_property(u, "RealName"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| who.gecos.as_deref().and_then(gecos_real_name));

    let avatar = if with_avatar {
        resolve_avatar(&who, &mut bus)
    } else {
        None
    };

    UserInfo {
        login: who.login,
        real_name,
        avatar,
    }
}

/// Re-resolve only the avatar path for the process's own user — what a
/// long-lived caller (the lock preview) calls to notice that the user picked a
/// new picture since [`current`] ran. Same sources, same order and same bounds
/// as [`current`]'s avatar half, without the display-name round trips.
pub fn current_avatar() -> Option<PathBuf> {
    resolve_avatar(&Who::detect(), &mut SystemBus::default())
}

/// The process user's identity as `/etc/passwd` knows it — the inputs every
/// other lookup in this module is keyed on.
struct Who {
    uid: Option<u32>,
    /// Empty only when neither passwd nor `$USER`/`$LOGNAME` knows a name.
    login: String,
    gecos: Option<String>,
}

impl Who {
    fn detect() -> Who {
        let uid = current_uid();
        let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
        let entry = uid.and_then(|u| passwd_entry_for_uid(&passwd, u));
        let login = entry
            .as_ref()
            .map(|e| e.name.clone())
            .or_else(|| non_empty_env("USER"))
            .or_else(|| non_empty_env("LOGNAME"))
            .unwrap_or_default();
        Who {
            uid,
            login,
            gecos: entry.map(|e| e.gecos),
        }
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

/// The process's own uid, with no `libc` dependency — the same trick as
/// `ipc.rs`'s private `libc_getuid`, but kept as `Option` rather than
/// defaulting to 0: a wrong uid would look up the wrong (or root's) passwd
/// row, which is worse than simply having none to look up with.
pub(crate) fn current_uid() -> Option<u32> {
    std::fs::metadata("/proc/self")
        .ok()
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
}

/// The process's own login name, which is what Deepin's root helpers compare
/// against: `/etc/passwd`, else `$USER`/`$LOGNAME`. No D-Bus round trips.
pub(crate) fn current_login() -> Option<String> {
    Some(Who::detect().login).filter(|l| !l.is_empty())
}

// ---------------------------------------------------------------------------
// /etc/passwd
// ---------------------------------------------------------------------------

/// The fields this module needs from one `/etc/passwd` row.
#[derive(Debug, PartialEq, Eq)]
struct PasswdEntry {
    name: String,
    uid: u32,
    /// Raw GECOS field, comma-separated; index 0 is the real name.
    gecos: String,
}

/// Parse `/etc/passwd` text (`name:passwd:uid:gid:gecos:home:shell`) into
/// rows. Blank lines and `#`-comments are skipped (not part of the format,
/// but tolerated the way glibc's own reader is); a line with too few fields or
/// a non-numeric uid is dropped rather than guessed at — a parser run over
/// this file must never panic on it, however it got hand-edited.
fn parse_passwd(text: &str) -> Vec<PasswdEntry> {
    text.lines()
        .filter_map(|line| {
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut fields = line.split(':');
            let name = fields.next()?.to_string();
            let _password = fields.next()?;
            let uid = fields.next()?.parse().ok()?;
            let _gid = fields.next()?;
            let gecos = fields.next().unwrap_or("").to_string();
            Some(PasswdEntry { name, uid, gecos })
        })
        .collect()
}

/// The first row matching `uid`. "First" is a deliberate, tested choice for a
/// hand-edited file with duplicate uids — real systems never have two rows
/// for the same uid, but a parser over untrusted text must still answer
/// *something* rather than pick arbitrarily or panic.
fn passwd_entry_for_uid(text: &str, uid: u32) -> Option<PasswdEntry> {
    parse_passwd(text).into_iter().find(|e| e.uid == uid)
}

/// The real name out of a GECOS field: its first comma-separated entry,
/// trimmed. Empty (a bare `,office,phone` or an empty field entirely) is
/// `None`, not an empty string — matching [`current`]'s contract that
/// `real_name` is never `Some("")`.
fn gecos_real_name(gecos: &str) -> Option<String> {
    let name = gecos.split(',').next().unwrap_or("").trim();
    (!name.is_empty()).then(|| name.to_string())
}

// ---------------------------------------------------------------------------
// Account services (system bus)
// ---------------------------------------------------------------------------

/// How to reach one account service's per-user object on the system bus.
struct AccountsBus {
    dest: &'static str,
    /// `<prefix><uid>` is the user's object path.
    user_path_prefix: &'static str,
    /// The interface that carries `RealName` / `IconFile`.
    user_interface: &'static str,
}

/// accounts-daemon, the freedesktop service GNOME, KDE and COSMIC share.
const FREEDESKTOP_ACCOUNTS: AccountsBus = AccountsBus {
    dest: "org.freedesktop.Accounts",
    user_path_prefix: "/org/freedesktop/Accounts/User",
    user_interface: "org.freedesktop.Accounts.User",
};

/// dde-daemon's own account service, newest name first. Verified against
/// `linuxdeepin/dde-daemon`: `accounts1/manager_ifc.go` and `user_ifc.go` on
/// `release/2500` (deepin 25), `accounts/*` on `release/5.4.4` (deepin 20).
const DEEPIN_ACCOUNTS: [AccountsBus; 2] = [
    AccountsBus {
        dest: "org.deepin.dde.Accounts1",
        user_path_prefix: "/org/deepin/dde/Accounts1/User",
        user_interface: "org.deepin.dde.Accounts1.User",
    },
    AccountsBus {
        dest: "com.deepin.daemon.Accounts",
        user_path_prefix: "/com/deepin/daemon/Accounts/User",
        user_interface: "com.deepin.daemon.Accounts.User",
    },
];

/// A `gdbus --system` caller that stops asking once the bus has proven it
/// hangs. One per resolution, so a wedged service costs one timeout rather
/// than one per question.
#[derive(Default)]
struct SystemBus {
    wedged: bool,
}

impl SystemBus {
    /// One `gdbus call`; the raw stdout on success. `None` for every failure —
    /// an absent service (the common case, and a fast one) is not worth a log
    /// line on every lock.
    fn call(
        &mut self,
        dest: &str,
        object_path: &str,
        method: &str,
        args: &[&str],
    ) -> Option<String> {
        if self.wedged {
            return None;
        }
        let out = std::process::Command::new("gdbus")
            .args(["call", "--system", "--timeout", CALL_TIMEOUT_SECS])
            .args(["--dest", dest, "--object-path", object_path])
            .args(["--method", method])
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            if is_bus_timeout(&String::from_utf8_lossy(&out.stderr)) {
                self.wedged = true;
            }
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// `Properties.Get` of a string property on `svc`'s object for `uid`.
    fn get_property(&mut self, svc: &AccountsBus, uid: u32, property: &str) -> Option<String> {
        let path = format!("{}{uid}", svc.user_path_prefix);
        self.get_property_at(svc, &path, property)
    }

    fn get_property_at(
        &mut self,
        svc: &AccountsBus,
        object_path: &str,
        property: &str,
    ) -> Option<String> {
        let out = self.call(
            svc.dest,
            object_path,
            "org.freedesktop.DBus.Properties.Get",
            &[svc.user_interface, property],
        )?;
        parse_gvariant_string(&out)
    }

    /// A property of the user's `org.freedesktop.Accounts` object.
    ///
    /// accounts-daemon only exports the objects of users it has *cached*, so a
    /// straight `Get` on `/org/freedesktop/Accounts/User<uid>` can answer
    /// "unknown object" for a perfectly real account. On that failure
    /// `FindUserById` makes the daemon load the user and hands back the object
    /// path to ask again at.
    fn accounts_service_property(&mut self, uid: u32, property: &str) -> Option<String> {
        let svc = &FREEDESKTOP_ACCOUNTS;
        if let Some(v) = self.get_property(svc, uid, property) {
            return Some(v);
        }
        let found = self.call(
            svc.dest,
            "/org/freedesktop/Accounts",
            "org.freedesktop.Accounts.FindUserById",
            &[&format!("int64 {uid}")],
        )?;
        let path = parse_gvariant_object_path(&found)?;
        self.get_property_at(svc, &path, property)
    }
}

/// Did this `gdbus` failure mean "the peer is not answering" (as opposed to
/// "there is no such service", which fails immediately and is no reason to
/// stop asking)?
fn is_bus_timeout(stderr: &str) -> bool {
    stderr.contains("Timeout was reached") || stderr.contains("Error.NoReply")
}

/// Parse a `gdbus` object-path reply: `(objectpath '/org/freedesktop/Accounts/
/// User1000',)`, or the same without the `objectpath ` annotation. The path is
/// validated against the D-Bus object-path alphabet, because it goes straight
/// back into a command line.
fn parse_gvariant_object_path(out: &str) -> Option<String> {
    let inner = out.trim().strip_prefix('(')?.strip_suffix(",)")?;
    let inner = inner.strip_prefix("objectpath ").unwrap_or(inner);
    let path = inner.strip_prefix('\'')?.strip_suffix('\'')?;
    let valid = path.starts_with('/')
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'/');
    valid.then(|| path.to_string())
}

/// Parse the one reply shape every property call above can produce: a
/// 1-tuple wrapping a single string variant, e.g. `(<'Roy Das'>,)` or, once the value itself
/// contains an apostrophe, `(<"Roy O'Das">,)`. `g_variant_print` picks
/// whichever quote character the content does not already contain and escapes
/// only that quote and a literal backslash — see `mpris.rs`'s
/// [`crate::mpris::GVal`] docs for the general rule this is a narrow slice of.
/// Anything that is not exactly this shape — an error message, empty output, a
/// dictionary, a non-string scalar — returns `None`.
fn parse_gvariant_string(out: &str) -> Option<String> {
    let b = out.trim().as_bytes();
    let mut i = 0usize;
    if *b.get(i)? != b'(' {
        return None;
    }
    i += 1;
    if *b.get(i)? != b'<' {
        return None;
    }
    i += 1;
    let quote = *b.get(i)?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    i += 1;
    let mut content: Vec<u8> = Vec::with_capacity(b.len());
    loop {
        let c = *b.get(i)?;
        i += 1;
        if c == quote {
            break;
        }
        if c == b'\\' {
            content.push(*b.get(i)?);
            i += 1;
        } else {
            content.push(c);
        }
    }
    if b.get(i) != Some(&b'>') || b.get(i + 1) != Some(&b',') || b.get(i + 2) != Some(&b')') {
        return None;
    }
    String::from_utf8(content).ok()
}

// ---------------------------------------------------------------------------
// Avatar resolution
// ---------------------------------------------------------------------------

/// Where stock accountsservice keeps its copy of each user's picture
/// (`<dir>/<login>`), and where deepin's `dde-account-faces` package installs
/// its shared set (`<dir>/animal/tiger.png`, `<dir>/default`, ...).
const ACCOUNTS_ICONS_DIR: &str = "/var/lib/AccountsService/icons";
/// dde-daemon's per-user keyfile directory (`userConfigDir` in
/// `accounts1/manager.go`): `[User]` / `Icon=file:///...`.
const DEEPIN_USERS_DIR: &str = "/var/lib/AccountsService/deepin/users";
/// Largest picture worth opening — the same cap the album-art path uses.
const MAX_AVATAR_BYTES: u64 = 8 * 1024 * 1024;

/// One place a user's picture can be recorded; see [`avatar_source_order`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AvatarSource {
    /// `IconFile` on dde-daemon's account service (system bus).
    DeepinAccounts,
    /// `Icon=` in dde-daemon's per-user keyfile, read off disk.
    DeepinUserConfig,
    /// `IconFile` on accounts-daemon (system bus).
    AccountsService,
    /// `/var/lib/AccountsService/icons/<login>`.
    AccountsIconsDir,
    /// `~/.face`.
    FaceFile,
    /// `~/.face.icon`.
    FaceIconFile,
}

/// The order the sources are tried in. On deepin the Control Center edits
/// dde-daemon's record, so it goes first; everywhere else it is a last resort
/// that costs one fast-failing `gdbus` call and one missing file — and it still
/// sits *ahead of* `~/.face`, so a deepin session whose environment did not
/// make it to the daemon is still found.
fn avatar_source_order(deepin: bool) -> [AvatarSource; 6] {
    use AvatarSource::*;
    if deepin {
        [
            DeepinAccounts,
            DeepinUserConfig,
            AccountsService,
            AccountsIconsDir,
            FaceFile,
            FaceIconFile,
        ]
    } else {
        [
            AccountsService,
            AccountsIconsDir,
            DeepinAccounts,
            DeepinUserConfig,
            FaceFile,
            FaceIconFile,
        ]
    }
}

/// Is this a deepin (DDE) session? `env` looks a variable up. Checks the three
/// variables desktops actually set; `XDG_CURRENT_DESKTOP` is a colon-separated
/// list (`deepin`, or `DDE`, or `X-Deepin:...`).
fn is_deepin_desktop(env: impl Fn(&str) -> Option<String>) -> bool {
    [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
    ]
    .iter()
    .filter_map(|k| env(k))
    .flat_map(|v| {
        v.split(':')
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>()
    })
    .any(|d| d.contains("deepin") || d == "dde")
}

/// The first acceptable candidate, trying `order` source by source. `candidates`
/// is asked **lazily** — a later source's `gdbus` call or file read never runs
/// once an earlier one produced something `accept` likes — and a candidate that
/// `accept` rejects (missing, unreadable, an SVG) moves on to the next one
/// rather than ending the search.
fn pick_avatar(
    order: &[AvatarSource],
    mut candidates: impl FnMut(AvatarSource) -> Vec<PathBuf>,
    accept: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    order
        .iter()
        .flat_map(|&src| candidates(src))
        .find(|p| accept(p))
}

fn resolve_avatar(who: &Who, bus: &mut SystemBus) -> Option<PathBuf> {
    let home = dirs::home_dir();
    let order = avatar_source_order(is_deepin_desktop(non_empty_env));
    pick_avatar(
        &order,
        |src| source_candidates(src, who, home.as_deref(), bus),
        avatar_file_ok,
    )
}

/// The paths one source suggests. The I/O half of [`pick_avatar`]'s contract;
/// everything it parses goes through the pure functions below.
fn source_candidates(
    src: AvatarSource,
    who: &Who,
    home: Option<&Path>,
    bus: &mut SystemBus,
) -> Vec<PathBuf> {
    match src {
        AvatarSource::DeepinAccounts => {
            let Some(uid) = who.uid else {
                return Vec::new();
            };
            // Both service generations are never installed together, so the
            // first one that answers at all is the one to believe.
            DEEPIN_ACCOUNTS
                .iter()
                .find_map(|svc| bus.get_property(svc, uid, "IconFile"))
                .and_then(|raw| parse_icon_location(&raw, home))
                .into_iter()
                .collect()
        }
        AvatarSource::DeepinUserConfig => per_user_file(DEEPIN_USERS_DIR, &who.login)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| keyfile_value(&text, "User", "Icon"))
            .and_then(|raw| parse_icon_location(&raw, home))
            .into_iter()
            .collect(),
        AvatarSource::AccountsService => who
            .uid
            .and_then(|uid| bus.accounts_service_property(uid, "IconFile"))
            .and_then(|raw| parse_icon_location(&raw, home))
            .into_iter()
            .collect(),
        AvatarSource::AccountsIconsDir => per_user_file(ACCOUNTS_ICONS_DIR, &who.login)
            .into_iter()
            .collect(),
        AvatarSource::FaceFile => home.map(|h| h.join(".face")).into_iter().collect(),
        AvatarSource::FaceIconFile => home.map(|h| h.join(".face.icon")).into_iter().collect(),
    }
}

/// `<dir>/<login>`, or `None` for a login that is not a single plain path
/// component — `$USER` is environment-supplied, and `../../etc/shadow` must
/// not become a path we open.
fn per_user_file(dir: &str, login: &str) -> Option<PathBuf> {
    let plain = !login.is_empty() && login != "." && login != ".." && !login.contains(['/', '\0']);
    plain.then(|| Path::new(dir).join(login))
}

/// Turn an `IconFile` value into a filesystem path.
///
/// AccountsService hands back a plain absolute path. dde-daemon hands back a
/// **`file://` URI** — its `EncodeURI(path, SCHEME_FILE)` is `"file://" +
/// url.URL{Path: path}.String()`, so `file:///var/lib/AccountsService/icons/
/// animal/tiger.png`, with the path percent-encoded (`go-lib/utils/uri.go`).
/// Both are handled, plus the RFC 8089 forms `file:/path` and
/// `file://localhost/path`, and a leading `~/` (expanded against `home`).
///
/// Anything that is not a local absolute path — a relative path, an `http:`
/// URI, a themed icon *name*, a remote `file://host/` — is `None`, never a
/// guess.
fn parse_icon_location(raw: &str, home: Option<&Path>) -> Option<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() || raw.contains('\0') {
        return None;
    }
    // `file:` is matched case-insensitively (RFC 3986 §3.1); `get` keeps this
    // total on a string whose 5th byte is mid-character.
    if raw
        .get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("file:"))
    {
        let rest = &raw[5..];
        let path = match rest.strip_prefix("//") {
            Some(after) => {
                let (authority, path) = after.split_at(after.find('/')?);
                if !(authority.is_empty() || authority.eq_ignore_ascii_case("localhost")) {
                    return None;
                }
                path
            }
            None => rest,
        };
        if !path.starts_with('/') {
            return None;
        }
        // A literal `?`/`#` in a path is always percent-encoded in a URI, so
        // the first bare one starts the query/fragment.
        let path = path.split(['?', '#']).next().unwrap_or("");
        let bytes = percent_decode(path)?;
        if bytes.contains(&0) {
            return None;
        }
        return Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home.map(|h| h.join(rest));
    }
    raw.starts_with('/').then(|| PathBuf::from(raw))
}

/// `%XX` decoding to raw bytes (a path need not be UTF-8). A `%` that is not
/// followed by two hex digits is malformed — `None`, as Go's `url.Parse`
/// (which wrote these URIs) would reject it too.
fn percent_decode(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b.get(i + 1..i + 3)?;
            let hex = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    Some(out)
}

/// One value out of GKeyFile-format text (`/var/lib/AccountsService/deepin/
/// users/<login>` is one). Only `group`'s own keys are considered, `#`
/// comments and blank lines are skipped, and a key given twice takes its last
/// value, as GKeyFile does. `None` when the group or key is absent.
fn keyfile_value(text: &str, group: &str, key: &str) -> Option<String> {
    let mut in_group = false;
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_group = name == group;
            continue;
        }
        if !in_group {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                found = Some(unescape_keyfile_value(v.trim_start()));
            }
        }
    }
    found
}

/// GKeyFile's value escapes: `\s` is a space, `\n`/`\t`/`\r` control
/// characters, `\\` a backslash. Any other backslash is left as written.
fn unescape_keyfile_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Does `head` (a file's first bytes) start with a PNG, JPEG or WebP
/// signature? Those are exactly the formats the `image` crate is built with
/// (`Cargo.toml`), so a file that passes here is one the widget kit can decode
/// — and an SVG, GIF or BMP, which it cannot, is turned away *before* it
/// shadows a later source that has a usable picture.
fn is_supported_raster(head: &[u8]) -> bool {
    head.starts_with(b"\x89PNG\r\n\x1a\n")
        || head.starts_with(&[0xFF, 0xD8, 0xFF])
        || (head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP")
}

/// A regular file (symlinks followed — deepin's `icons/default` is one, via
/// `update-alternatives`) of sane size that opens for reading and begins with a
/// supported raster signature.
fn avatar_file_ok(path: &Path) -> bool {
    use std::io::Read;
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_AVATAR_BYTES {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 12];
    let n = f.read(&mut head).unwrap_or(0);
    is_supported_raster(&head[..n])
}

// ---------------------------------------------------------------------------
// Pure presentation helpers
// ---------------------------------------------------------------------------

/// The name to greet with: the first word of `real_name`, or the whole thing
/// when it has none (as most CJK names do — there is no family/given split to
/// make), or the login with its first letter upper-cased when there is no
/// real name at all.
pub fn first_name(info: &UserInfo) -> String {
    let real = info
        .real_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match real {
        Some(name) => name.split_whitespace().next().unwrap_or(name).to_string(),
        None => capitalize_first(&info.login),
    }
}

/// The letters for the avatar disc when there is no picture: the first letter
/// of the first and of the last word of the display name (`"Roy Das"` →
/// `"RD"`), a single letter for a one-word name (`"小明"` → `"小"`), and the
/// same rule over the login's `.`/`_`/`-`-separated parts when the name gives
/// nothing (`"roy.das"` → `"RD"`). Empty only when neither has a letter or
/// digit to show, which the card draws as a plain disc.
///
/// This replaces what the disc used to carry — the first letter of the
/// *greeting* ("G", from "Good evening"), which read as an app icon rather
/// than as a person.
pub fn initials(info: &UserInfo) -> String {
    let from_name = info
        .real_name
        .as_deref()
        .map(|n| initials_of(n, |c| c.is_whitespace()))
        .unwrap_or_default();
    if !from_name.is_empty() {
        return from_name;
    }
    initials_of(&info.login, |c| !c.is_alphanumeric())
}

/// First alphanumeric of each `is_sep`-separated word, first and last word
/// only, upper-cased.
fn initials_of(s: &str, is_sep: impl Fn(char) -> bool) -> String {
    let letters: Vec<char> = s
        .split(is_sep)
        .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
        .collect();
    let upper = |c: &char| c.to_uppercase().collect::<String>();
    match letters.as_slice() {
        [] => String::new(),
        [only] => upper(only),
        [first, .., last] => upper(first) + &upper(last),
    }
}

/// Upper-case just the first character. Unicode-aware — not every login is
/// ASCII — and total: empty input returns empty rather than panicking.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The lock-screen greeting for `hour` (0..=23, local time), optionally naming
/// `name`. A `name` of `None` or all-whitespace gets the bare phrase.
///
/// Boundaries: 05:00–11:59 morning, 12:00–16:59 afternoon, 17:00–21:59
/// evening, everything else (including an out-of-range `hour`, which cannot
/// happen from a real clock but must not panic here) night.
pub fn greeting(hour: u32, name: Option<&str>) -> String {
    let name = name.map(str::trim).filter(|s| !s.is_empty());
    match (hour, name) {
        (5..=11, Some(n)) => crate::tf!("Good morning, {name}", "name" => n),
        (5..=11, None) => crate::t!("Good morning").to_string(),
        (12..=16, Some(n)) => crate::tf!("Good afternoon, {name}", "name" => n),
        (12..=16, None) => crate::t!("Good afternoon").to_string(),
        (17..=21, Some(n)) => crate::tf!("Good evening, {name}", "name" => n),
        (17..=21, None) => crate::t!("Good evening").to_string(),
        (_, Some(n)) => crate::tf!("Good night, {name}", "name" => n),
        (_, None) => crate::t!("Good night").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- /etc/passwd -----------------------------------------------------

    #[test]
    fn parses_normal_rows() {
        let text =
            "root:x:0:0:root:/root:/bin/bash\nroy:x:1000:1000:Roy Das,,,:/home/roy:/bin/zsh\n";
        let rows = parse_passwd(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].name, "roy");
        assert_eq!(rows[1].uid, 1000);
        assert_eq!(rows[1].gecos, "Roy Das,,,");
        assert_eq!(gecos_real_name(&rows[1].gecos).as_deref(), Some("Roy Das"));
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let text = "# a comment\n\nroy:x:1000:1000:Roy Das:/home/roy:/bin/zsh\n";
        let rows = parse_passwd(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "roy");
    }

    #[test]
    fn drops_malformed_lines_without_panicking() {
        let text = "no-colons-at-all\n\
             roy:x:notanumber:1000:Roy:/home/roy:/bin/zsh\n\
             tooshort:x:1000\n\
             valid:x:42:42:Valid User:/home/valid:/bin/sh\n";
        let rows = parse_passwd(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "valid");
    }

    #[test]
    fn uid_collisions_resolve_to_the_first_row() {
        let text = "first:x:1000:1000:First Person:/home/first:/bin/sh\n\
             second:x:1000:1000:Second Person:/home/second:/bin/sh\n";
        let entry = passwd_entry_for_uid(text, 1000).unwrap();
        assert_eq!(entry.name, "first");
    }

    #[test]
    fn gecos_edge_cases() {
        assert_eq!(gecos_real_name(""), None);
        assert_eq!(gecos_real_name(",office,555"), None);
        assert_eq!(gecos_real_name("   "), None);
        assert_eq!(gecos_real_name("Only Name"), Some("Only Name".to_string()));
        assert_eq!(
            gecos_real_name("Roy Das,Building 2,555-1234"),
            Some("Roy Das".to_string())
        );
    }

    // -- GVariant string parsing ------------------------------------------

    #[test]
    fn gvariant_string_plain() {
        assert_eq!(
            parse_gvariant_string("(<'Roy Das'>,)\n"),
            Some("Roy Das".to_string())
        );
        assert_eq!(
            parse_gvariant_string("(<'/var/lib/AccountsService/icons/roy'>,)"),
            Some("/var/lib/AccountsService/icons/roy".to_string())
        );
    }

    #[test]
    fn gvariant_string_escaped_quote() {
        // Contains an apostrophe, so g_variant_print delimits with double
        // quotes; the literal double quotes around "Big" then have to be
        // escaped so they don't end the string early.
        assert_eq!(
            parse_gvariant_string(r#"(<"O'Brien \"Big\" Roy">,)"#),
            Some("O'Brien \"Big\" Roy".to_string())
        );
        // A literal backslash must round-trip too.
        assert_eq!(
            parse_gvariant_string(r"(<'Roy\\Das'>,)"),
            Some(r"Roy\Das".to_string())
        );
    }

    #[test]
    fn gvariant_string_unicode() {
        assert_eq!(
            parse_gvariant_string("(<'小明'>,)"),
            Some("小明".to_string())
        );
    }

    #[test]
    fn gvariant_string_empty() {
        assert_eq!(parse_gvariant_string("(<''>,)"), Some(String::new()));
    }

    #[test]
    fn gvariant_string_error_output_is_none() {
        assert_eq!(
            parse_gvariant_string(
                "Error: GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown: \
                 The name org.freedesktop.Accounts was not provided by any .service files\n"
            ),
            None
        );
        assert_eq!(parse_gvariant_string(""), None);
        assert_eq!(parse_gvariant_string("()"), None);
        assert_eq!(parse_gvariant_string("(<42>,)"), None); // not a string variant
    }

    // -- first_name --------------------------------------------------------

    fn info(real_name: Option<&str>, login: &str) -> UserInfo {
        UserInfo {
            login: login.to_string(),
            real_name: real_name.map(str::to_string),
            avatar: None,
        }
    }

    #[test]
    fn first_name_western_takes_the_first_word() {
        assert_eq!(first_name(&info(Some("Roy Das"), "roy")), "Roy");
    }

    #[test]
    fn first_name_cjk_has_no_spaces_so_uses_the_whole_name() {
        assert_eq!(first_name(&info(Some("小明"), "xiaoming")), "小明");
    }

    #[test]
    fn first_name_falls_back_to_capitalized_login() {
        assert_eq!(first_name(&info(None, "royd")), "Royd");
        assert_eq!(first_name(&info(Some("   "), "royd")), "Royd");
        assert_eq!(first_name(&info(None, "")), "");
    }

    // -- greeting ------------------------------------------------------------

    #[test]
    fn greeting_covers_every_hour_boundary() {
        for h in 0..24u32 {
            let (plain, named) = match h {
                5..=11 => ("Good morning", "Good morning, Roy"),
                12..=16 => ("Good afternoon", "Good afternoon, Roy"),
                17..=21 => ("Good evening", "Good evening, Roy"),
                _ => ("Good night", "Good night, Roy"),
            };
            assert_eq!(greeting(h, None), plain, "hour {h}");
            assert_eq!(greeting(h, Some("Roy")), named, "hour {h}");
        }
    }

    #[test]
    fn greeting_treats_blank_name_as_no_name() {
        assert_eq!(greeting(9, Some("   ")), "Good morning");
        assert_eq!(greeting(9, Some("")), "Good morning");
    }

    #[test]
    fn greeting_out_of_range_hour_is_night() {
        assert_eq!(greeting(24, None), "Good night");
        assert_eq!(greeting(100, None), "Good night");
    }

    #[test]
    fn current_does_not_panic() {
        // Whatever this machine/CI container actually is, resolving the
        // current user must never panic — only degrade. Not panicking here
        // *is* the assertion; there is no fixed expected value to compare to.
        let info = current();
        let _ = first_name(&info);
        let _ = initials(&info);
        let _ = current_avatar();
    }

    #[test]
    fn identity_only_resolution_never_looks_for_a_picture() {
        let who_only = current_identity();
        assert!(who_only.avatar.is_none());
        // ...and otherwise agrees with the full resolution.
        let full = current();
        assert_eq!(who_only.login, full.login);
        assert_eq!(who_only.real_name, full.real_name);
    }

    // -- avatar: icon location parsing -------------------------------------

    fn loc(raw: &str) -> Option<PathBuf> {
        parse_icon_location(raw, Some(Path::new("/home/roy")))
    }

    #[test]
    fn icon_location_decodes_the_file_uris_dde_daemon_returns() {
        // `defaultUserIcon` and a standard avatar, verbatim from dde-daemon.
        assert_eq!(
            loc("file:///var/lib/AccountsService/icons/default"),
            Some(PathBuf::from("/var/lib/AccountsService/icons/default"))
        );
        assert_eq!(
            loc("file:///var/lib/AccountsService/icons/animal/tiger.png"),
            Some(PathBuf::from(
                "/var/lib/AccountsService/icons/animal/tiger.png"
            ))
        );
        // A custom avatar: `<login>-<base36 ns>.png` under icons/local.
        assert_eq!(
            loc("file:///var/lib/AccountsService/icons/local/roy-1k2j3h4g5f.png"),
            Some(PathBuf::from(
                "/var/lib/AccountsService/icons/local/roy-1k2j3h4g5f.png"
            ))
        );
    }

    #[test]
    fn icon_location_percent_decodes_like_go_url_does() {
        assert_eq!(
            loc("file:///home/roy/My%20Pics/me%2B1.png"),
            Some(PathBuf::from("/home/roy/My Pics/me+1.png"))
        );
        // A path need not be UTF-8.
        let p = loc("file:///a%FFb.png").unwrap();
        assert_eq!(p.as_os_str().as_encoded_bytes(), b"/a\xFFb.png");
        // Malformed escapes are refused, not passed through.
        assert_eq!(loc("file:///bad%zz.png"), None);
        assert_eq!(loc("file:///short%2"), None);
        assert_eq!(loc("file:///nul%00.png"), None);
    }

    #[test]
    fn icon_location_accepts_the_other_local_file_uri_spellings() {
        let want = Some(PathBuf::from("/x/y.png"));
        assert_eq!(loc("file:/x/y.png"), want);
        assert_eq!(loc("file://localhost/x/y.png"), want);
        assert_eq!(loc("FILE:///x/y.png"), want);
        assert_eq!(loc("  file:///x/y.png\n"), want);
        assert_eq!(loc("file:///x/y.png?size=64#frag"), want);
    }

    #[test]
    fn icon_location_accepts_plain_absolute_paths_and_tilde() {
        // AccountsService, and deepin's data dirs, are plain paths.
        assert_eq!(
            loc("/usr/share/dde-api/data/avatar/tiger.png"),
            Some(PathBuf::from("/usr/share/dde-api/data/avatar/tiger.png"))
        );
        assert_eq!(
            loc("/home/roy/.face"),
            Some(PathBuf::from("/home/roy/.face"))
        );
        // A `%` in a plain path is a literal `%`, not an escape.
        assert_eq!(loc("/tmp/100%.png"), Some(PathBuf::from("/tmp/100%.png")));
        assert_eq!(
            loc("~/.local/share/avatars/me.png"),
            Some(PathBuf::from("/home/roy/.local/share/avatars/me.png"))
        );
        assert_eq!(parse_icon_location("~/me.png", None), None);
    }

    #[test]
    fn icon_location_refuses_anything_that_is_not_a_local_absolute_path() {
        for raw in [
            "",
            "   ",
            "avatar.png",
            "./avatar.png",
            "avatar-default-symbolic",
            "http://example.com/a.png",
            "https://example.com/a.png",
            "file://otherhost/x.png",
            "file://",
            "file:relative.png",
            "file:///x\0y",
        ] {
            assert_eq!(loc(raw), None, "{raw:?}");
        }
    }

    // -- avatar: keyfile ----------------------------------------------------

    const DEEPIN_USER_FILE: &str = "[User]\n\
        XSession=deepin\n\
        SystemAccount=false\n\
        Layout=us;\n\
        Locale=en_US.UTF-8\n\
        Icon=file:///var/lib/AccountsService/icons/animal/tiger.png\n\
        CustomIcon=file:///var/lib/AccountsService/icons/local/roy-abc.png\n\
        DesktopBackgrounds=file:///usr/share/backgrounds/default.jpg;\n";

    #[test]
    fn keyfile_reads_the_current_icon_not_the_custom_one() {
        assert_eq!(
            keyfile_value(DEEPIN_USER_FILE, "User", "Icon").as_deref(),
            Some("file:///var/lib/AccountsService/icons/animal/tiger.png")
        );
        assert_eq!(
            keyfile_value(DEEPIN_USER_FILE, "User", "CustomIcon").as_deref(),
            Some("file:///var/lib/AccountsService/icons/local/roy-abc.png")
        );
        assert_eq!(keyfile_value(DEEPIN_USER_FILE, "User", "Missing"), None);
    }

    #[test]
    fn keyfile_scopes_to_the_group_and_tolerates_noise() {
        let text = "# comment\n\n[Other]\nIcon=wrong\n\n[User]\n  Icon = /right.png \n# Icon=no\n";
        assert_eq!(
            keyfile_value(text, "User", "Icon").as_deref(),
            Some("/right.png")
        );
        assert_eq!(keyfile_value(text, "Nope", "Icon"), None);
        assert_eq!(keyfile_value("", "User", "Icon"), None);
        assert_eq!(keyfile_value("Icon=/x.png\n", "User", "Icon"), None);
        // Last duplicate wins, as in GKeyFile.
        assert_eq!(
            keyfile_value("[User]\nIcon=/a\nIcon=/b\n", "User", "Icon").as_deref(),
            Some("/b")
        );
    }

    #[test]
    fn keyfile_unescapes_gkeyfile_escapes() {
        assert_eq!(
            keyfile_value("[User]\nIcon=/my\\spics/a\\\\b.png\n", "User", "Icon").as_deref(),
            Some("/my pics/a\\b.png")
        );
        assert_eq!(unescape_keyfile_value("a\\qb\\"), "a\\qb\\");
    }

    // -- avatar: file acceptance -------------------------------------------

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fresco-userinfo-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const PNG_HEAD: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn raster_signatures() {
        assert!(is_supported_raster(PNG_HEAD));
        assert!(is_supported_raster(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]));
        assert!(is_supported_raster(b"RIFF\x10\0\0\0WEBPVP8 "));
        for not in [
            &b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"[..],
            b"<?xml version=\"1.0\"?><svg/>",
            b"GIF89a\x01\0\x01\0",
            b"BM\x36\0\0\0\0\0",
            b"RIFF\x10\0\0\0WAVEfmt ",
            b"RIFF",
            b"",
        ] {
            assert!(!is_supported_raster(not), "{not:?}");
        }
    }

    #[test]
    fn avatar_file_ok_checks_kind_size_and_signature() {
        let d = scratch("ok");
        let png = d.join("a.png");
        std::fs::write(&png, PNG_HEAD).unwrap();
        let svg = d.join("a.svg");
        std::fs::write(&svg, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        let empty = d.join("empty");
        std::fs::write(&empty, "").unwrap();
        // A JPEG named .png (deepin copies a small upload as-is under a .png
        // name) is judged by its bytes, not its extension.
        let jpeg_as_png = d.join("b.png");
        std::fs::write(&jpeg_as_png, [0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]).unwrap();

        assert!(avatar_file_ok(&png));
        assert!(avatar_file_ok(&jpeg_as_png));
        assert!(!avatar_file_ok(&svg));
        assert!(!avatar_file_ok(&empty));
        assert!(!avatar_file_ok(&d));
        assert!(!avatar_file_ok(&d.join("missing.png")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn deepins_default_avatar_symlink_chain_resolves() {
        // /var/lib/AccountsService/icons/default ->
        //   /etc/alternatives/default-account-icon -> .../animal/raccoon.png
        // (dde-account-faces' postinst, via update-alternatives).
        let d = scratch("chain");
        std::fs::create_dir_all(d.join("icons/animal")).unwrap();
        std::fs::create_dir_all(d.join("alternatives")).unwrap();
        std::fs::write(d.join("icons/animal/raccoon.png"), PNG_HEAD).unwrap();
        std::os::unix::fs::symlink(
            d.join("icons/animal/raccoon.png"),
            d.join("alternatives/default-account-icon"),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            d.join("alternatives/default-account-icon"),
            d.join("icons/default"),
        )
        .unwrap();

        let uri = format!("file://{}", d.join("icons/default").display());
        let path = parse_icon_location(&uri, None).unwrap();
        assert!(avatar_file_ok(&path), "{path:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    // -- avatar: source order and fallback ----------------------------------

    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Run `pick_avatar` over canned per-source candidates, recording which
    /// sources were actually asked and in what order.
    fn pick(
        deepin: bool,
        canned: &[(AvatarSource, &[&str])],
        accepted: &[&str],
    ) -> (Option<PathBuf>, Vec<AvatarSource>) {
        let canned: HashMap<AvatarSource, Vec<PathBuf>> = canned
            .iter()
            .map(|(s, ps)| (*s, ps.iter().map(PathBuf::from).collect()))
            .collect();
        let asked = RefCell::new(Vec::new());
        let got = pick_avatar(
            &avatar_source_order(deepin),
            |src| {
                asked.borrow_mut().push(src);
                canned.get(&src).cloned().unwrap_or_default()
            },
            |p| accepted.iter().any(|a| Path::new(a) == p),
        );
        (got, asked.into_inner())
    }

    #[test]
    fn deepin_session_asks_the_deepin_service_first_and_stops_at_a_hit() {
        use AvatarSource::*;
        let (got, asked) = pick(
            true,
            &[
                (DeepinAccounts, &["/icons/animal/tiger.png"]),
                (AccountsService, &["/other.png"]),
                (FaceFile, &["/home/roy/.face"]),
            ],
            &["/icons/animal/tiger.png", "/other.png", "/home/roy/.face"],
        );
        assert_eq!(got, Some(PathBuf::from("/icons/animal/tiger.png")));
        assert_eq!(asked, vec![DeepinAccounts], "later sources must not run");
    }

    #[test]
    fn non_deepin_session_prefers_accounts_service_then_icons_dir_then_face() {
        use AvatarSource::*;
        let (got, asked) = pick(
            false,
            &[
                (AccountsService, &["/missing.png"]),
                (AccountsIconsDir, &["/var/lib/AccountsService/icons/roy"]),
                (FaceFile, &["/home/roy/.face"]),
            ],
            &["/var/lib/AccountsService/icons/roy", "/home/roy/.face"],
        );
        assert_eq!(
            got,
            Some(PathBuf::from("/var/lib/AccountsService/icons/roy"))
        );
        assert_eq!(asked, vec![AccountsService, AccountsIconsDir]);
    }

    #[test]
    fn a_rejected_candidate_falls_through_to_the_next_source() {
        use AvatarSource::*;
        // The deepin service names an SVG nothing can decode; ~/.face is fine.
        let (got, asked) = pick(
            true,
            &[
                (DeepinAccounts, &["/icons/me.svg"]),
                (FaceFile, &["/home/roy/.face"]),
            ],
            &["/home/roy/.face"],
        );
        assert_eq!(got, Some(PathBuf::from("/home/roy/.face")));
        assert_eq!(
            asked,
            vec![
                DeepinAccounts,
                DeepinUserConfig,
                AccountsService,
                AccountsIconsDir,
                FaceFile
            ]
        );
    }

    #[test]
    fn several_candidates_from_one_source_are_tried_in_order() {
        use AvatarSource::*;
        let (got, _) = pick(
            false,
            &[(AccountsService, &["/a.png", "/b.png", "/c.png"])],
            &["/b.png", "/c.png"],
        );
        assert_eq!(got, Some(PathBuf::from("/b.png")));
    }

    #[test]
    fn nothing_found_asks_every_source_once_and_returns_none() {
        for deepin in [true, false] {
            let (got, asked) = pick(deepin, &[], &[]);
            assert_eq!(got, None);
            assert_eq!(asked, avatar_source_order(deepin).to_vec());
        }
    }

    #[test]
    fn every_source_appears_exactly_once_in_either_order() {
        for deepin in [true, false] {
            let order = avatar_source_order(deepin);
            for (i, a) in order.iter().enumerate() {
                for b in &order[i + 1..] {
                    assert_ne!(a, b);
                }
            }
            assert_eq!(order.len(), 6);
        }
        assert_eq!(avatar_source_order(true)[0], AvatarSource::DeepinAccounts);
        assert_eq!(avatar_source_order(false)[0], AvatarSource::AccountsService);
        // `~/.face` is the last resort in both.
        assert_eq!(avatar_source_order(true)[5], AvatarSource::FaceIconFile);
        assert_eq!(avatar_source_order(false)[5], AvatarSource::FaceIconFile);
    }

    #[test]
    fn deepin_desktop_detection() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(name, _)| *name == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert!(is_deepin_desktop(env(&[("XDG_CURRENT_DESKTOP", "Deepin")])));
        assert!(is_deepin_desktop(env(&[("XDG_CURRENT_DESKTOP", "DDE")])));
        assert!(is_deepin_desktop(env(&[(
            "XDG_CURRENT_DESKTOP",
            "X-Deepin:GNOME"
        )])));
        assert!(is_deepin_desktop(env(&[("DESKTOP_SESSION", "deepin")])));
        assert!(is_deepin_desktop(env(&[("XDG_SESSION_DESKTOP", "deepin")])));
        assert!(!is_deepin_desktop(env(&[("XDG_CURRENT_DESKTOP", "GNOME")])));
        assert!(!is_deepin_desktop(env(&[(
            "XDG_CURRENT_DESKTOP",
            "COSMIC"
        )])));
        assert!(!is_deepin_desktop(env(&[
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("DESKTOP_SESSION", "plasma")
        ])));
        assert!(!is_deepin_desktop(env(&[])));
    }

    #[test]
    fn per_user_files_refuse_logins_that_are_not_one_plain_component() {
        assert_eq!(
            per_user_file("/var/lib/AccountsService/icons", "roy"),
            Some(PathBuf::from("/var/lib/AccountsService/icons/roy"))
        );
        for bad in ["", ".", "..", "a/b", "../etc/shadow", "/etc/passwd", "a\0b"] {
            assert_eq!(per_user_file("/d", bad), None, "{bad:?}");
        }
    }

    // -- avatar: bus replies ------------------------------------------------

    #[test]
    fn object_path_reply_parsing() {
        assert_eq!(
            parse_gvariant_object_path("(objectpath '/org/freedesktop/Accounts/User1000',)\n")
                .as_deref(),
            Some("/org/freedesktop/Accounts/User1000")
        );
        assert_eq!(
            parse_gvariant_object_path("('/org/freedesktop/Accounts/User1000',)").as_deref(),
            Some("/org/freedesktop/Accounts/User1000")
        );
        for bad in [
            "",
            "()",
            "(objectpath '',)",
            "(objectpath 'relative/path',)",
            "(objectpath '/has space',)",
            "(objectpath '/a'; rm -rf,)",
            "Error: GDBus.Error:org.freedesktop.Accounts.Error.Failed: nope",
            "(<'/org/freedesktop/Accounts/User1000'>,)",
        ] {
            assert_eq!(parse_gvariant_object_path(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn only_a_silent_peer_wedges_the_bus() {
        assert!(is_bus_timeout("Error: Timeout was reached\n"));
        assert!(is_bus_timeout(
            "Error: GDBus.Error:org.freedesktop.DBus.Error.NoReply: Did not receive a reply."
        ));
        // An absent service fails fast; the next question is still worth asking.
        assert!(!is_bus_timeout(
            "Error: GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown: The name is not activatable"
        ));
        assert!(!is_bus_timeout(
            "Error: GDBus.Error:org.freedesktop.DBus.Error.UnknownObject: nope"
        ));
        assert!(!is_bus_timeout(""));
    }

    #[test]
    fn a_wedged_bus_makes_no_further_calls() {
        let mut bus = SystemBus { wedged: true };
        assert_eq!(bus.accounts_service_property(1000, "IconFile"), None);
        assert_eq!(
            bus.get_property(&DEEPIN_ACCOUNTS[0], 1000, "IconFile"),
            None
        );
    }

    // -- initials -----------------------------------------------------------

    #[test]
    fn initials_from_the_display_name() {
        assert_eq!(initials(&info(Some("Roy Das"), "roy")), "RD");
        assert_eq!(initials(&info(Some("roy das"), "roy")), "RD");
        assert_eq!(initials(&info(Some("Mary Jane Watson"), "mjw")), "MW");
        assert_eq!(initials(&info(Some("Roy"), "roy")), "R");
        assert_eq!(initials(&info(Some("  Roy   Das  "), "roy")), "RD");
        assert_eq!(initials(&info(Some("(admin) Roy"), "roy")), "AR");
        assert_eq!(initials(&info(Some("Éloïse Ünal"), "e")), "ÉÜ");
    }

    #[test]
    fn initials_cjk_names_have_no_word_split() {
        assert_eq!(initials(&info(Some("小明"), "xiaoming")), "小");
        assert_eq!(initials(&info(Some("王 小明"), "wang")), "王小");
    }

    #[test]
    fn initials_fall_back_to_the_login() {
        assert_eq!(initials(&info(None, "roy")), "R");
        assert_eq!(initials(&info(None, "roy.das")), "RD");
        assert_eq!(initials(&info(None, "roy_das-jr")), "RJ");
        assert_eq!(initials(&info(Some("   "), "roy")), "R");
        // A name with nothing letter-like in it is no better than no name.
        assert_eq!(initials(&info(Some("!!! ???"), "roy")), "R");
    }

    #[test]
    fn initials_are_empty_only_when_there_is_nothing_to_show() {
        assert_eq!(initials(&info(None, "")), "");
        assert_eq!(initials(&info(Some(""), "...")), "");
    }
}

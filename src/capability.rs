//! Session capability detection — which wallpaper backend can run here.
//!
//! X11 sessions use the embedded-mpv backend. Wayland sessions are split:
//!  - GNOME/Mutter has no `wlr-layer-shell`, so we fall back to a static frame.
//!  - Cinnamon's muffin *used to* have none either, but as of muffin PR #803
//!    it now implements `zwlr_layer_shell_v1` for every client — so a current
//!    Cinnamon session gets the same live layer-shell backend as everything
//!    else. See `daemon::cinnamon_bg` for the restack this newer muffin needs
//!    (it stacks new BACKGROUND surfaces under old ones, hiding mpvpaper
//!    behind `cinnamon-background-daemon`'s own window unless that daemon is
//!    restarted after mpvpaper comes up).
//!  - Everything else (wlroots, KDE Plasma 6, COSMIC, …) uses the mpvpaper
//!    layer-shell backend for live wallpapers.
//!
//! On Wayland we probe the live registry for `zwlr_layer_shell_v1` ourselves (no
//! external tools) and trust that over the desktop-name heuristic below, which
//! only runs when no Wayland connection could be made at all (so a real probe
//! is impossible) — there we still have to guess, and guessing layer-shell for
//! GNOME or an old Cinnamon means mpvpaper fails outright at login, so both
//! keep defaulting to the static fallback in that fallback path only.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// X11 session — the existing in-process mpv backend.
    X11,
    /// Wayland with a layer-shell compositor — live wallpaper backend.
    WaylandLayerShell,
    /// Wayland on GNOME (no layer-shell) — static-frame fallback.
    WaylandGnomeStatic,
}

impl Capability {
    /// Short stable identifier for logs and diagnostics.
    pub fn id(self) -> &'static str {
        match self {
            Capability::X11 => "x11",
            Capability::WaylandLayerShell => "wayland-layer-shell",
            Capability::WaylandGnomeStatic => "wayland-gnome-static",
        }
    }
}

/// Detect the capability of the current session from the environment.
pub fn detect() -> Capability {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let wayland_display = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
    let session_desktop = std::env::var("XDG_SESSION_DESKTOP").ok();

    let is_wayland = match session_type.as_deref() {
        Some("wayland") => true,
        Some("x11") => false,
        // Session type unset/unknown: trust WAYLAND_DISPLAY.
        _ => wayland_display,
    };
    if !is_wayland {
        return Capability::X11;
    }

    // Prefer a real registry probe when available.
    if let Some(has_layer) = probe_layer_shell() {
        return if has_layer {
            Capability::WaylandLayerShell
        } else {
            // No layer-shell → treat like GNOME (static fallback) even if we
            // can't identify the compositor by name.
            Capability::WaylandGnomeStatic
        };
    }

    classify(
        session_type.as_deref(),
        wayland_display,
        current_desktop.as_deref().or(session_desktop.as_deref()),
    )
}

/// Pure desktop-name classification, testable without touching the process
/// environment. `detect()` may override this with a layer-shell registry probe.
fn classify(
    session_type: Option<&str>,
    wayland_display: bool,
    current_desktop: Option<&str>,
) -> Capability {
    let is_wayland = match session_type {
        Some("wayland") => true,
        Some("x11") => false,
        // Session type unset/unknown: trust WAYLAND_DISPLAY.
        _ => wayland_display,
    };
    if !is_wayland {
        return Capability::X11;
    }
    // Name-only fallback, used only when no Wayland connection could be made
    // at all (so `probe_layer_shell` returned `None`) — a real Cinnamon
    // session almost always reaches the probe above instead. We cannot tell
    // an old muffin (no layer-shell) from a current one (has it, PR #803)
    // by name alone, and guessing layer-shell for either GNOME or Cinnamon
    // means mpvpaper fails outright at login if we guess wrong — so both
    // still default to the static fallback here.
    if is_gnome(current_desktop) || is_cinnamon_name(current_desktop) {
        Capability::WaylandGnomeStatic
    } else {
        Capability::WaylandLayerShell
    }
}

/// Is this session Deepin's DDE? Its `dde-shell` paints an opaque desktop
/// window that covers other DESKTOP-type windows, so the X11 backend applies
/// extra quirks (see `daemon::dde`).
pub fn is_deepin_dde() -> bool {
    classify_deepin_dde(
        std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        std::env::var("XDG_SESSION_DESKTOP").ok().as_deref(),
    )
}

/// Pure DDE classification, testable without touching the process environment.
/// Desktop vars are colon-separated lists (e.g. "Deepin:GNOME"); a segment
/// containing "deepin" or equal to "dde" (case-insensitive) means DDE.
fn classify_deepin_dde(current_desktop: Option<&str>, session_desktop: Option<&str>) -> bool {
    [current_desktop, session_desktop]
        .into_iter()
        .flatten()
        .any(|v| {
            v.split(':').any(|seg| {
                let s = seg.trim().to_ascii_lowercase();
                s.contains("deepin") || s == "dde"
            })
        })
}

/// Is this session MATE? Caja, MATE's file manager, draws the desktop — its
/// icons and its own copy of the background — into one opaque full-screen
/// window that covers any other DESKTOP-type window, so a wallpaper stacked the
/// ordinary way is never seen (issue #18). The X11 backend raises the wallpaper
/// above that window instead; see `daemon::dde`.
pub fn is_mate() -> bool {
    classify_mate(
        std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        std::env::var("XDG_SESSION_DESKTOP").ok().as_deref(),
    )
}

/// Pure MATE classification. A whole segment of the colon-separated list must
/// be `MATE`, so no desktop whose name merely *contains* those letters matches.
fn classify_mate(current_desktop: Option<&str>, session_desktop: Option<&str>) -> bool {
    [current_desktop, session_desktop]
        .into_iter()
        .flatten()
        .any(|v| {
            v.split(':')
                .any(|seg| seg.trim().eq_ignore_ascii_case("mate"))
        })
}

/// Is this session Xfce? xfdesktop paints the backdrop and the icons into one
/// opaque window per monitor, and xfwm4 keeps it below the layer our wallpaper
/// window lives in, so the icons are hidden by the video: the X11 backend
/// mirrors them onto the wallpaper instead (`daemon::caja_mirror`).
pub fn is_xfce() -> bool {
    classify_xfce(
        std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        std::env::var("XDG_SESSION_DESKTOP").ok().as_deref(),
    )
}

/// Pure Xfce classification. A whole segment of the colon-separated list must
/// be `XFCE` (or `XUBUNTU`), so no desktop whose name merely *contains* those
/// letters matches.
fn classify_xfce(current_desktop: Option<&str>, session_desktop: Option<&str>) -> bool {
    [current_desktop, session_desktop]
        .into_iter()
        .flatten()
        .any(|v| {
            v.split(':').any(|seg| {
                let s = seg.trim();
                s.eq_ignore_ascii_case("xfce") || s.eq_ignore_ascii_case("xubuntu")
            })
        })
}

/// Is this session KDE Plasma? plasmashell draws the desktop — wallpaper and
/// icons — into one opaque full-screen surface (X11 desktop layer / Wayland
/// layer-shell background) that no window of ours can sit under or beside, so
/// on Plasma the wallpaper is set through plasmashell itself (issue #44); see
/// `daemon::kde_desktop`.
pub fn is_kde() -> bool {
    classify_kde(
        std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        std::env::var("XDG_SESSION_DESKTOP").ok().as_deref(),
    )
}

/// Pure Plasma classification. A whole segment of the colon-separated list
/// must be one of Plasma's own names (`KDE` from `XDG_CURRENT_DESKTOP`;
/// `plasma`/`plasmax11`/`plasmawayland` from some distros' `XDG_SESSION_DESKTOP`).
fn classify_kde(current_desktop: Option<&str>, session_desktop: Option<&str>) -> bool {
    [current_desktop, session_desktop]
        .into_iter()
        .flatten()
        .any(|v| {
            v.split(':').any(|seg| {
                matches!(
                    seg.trim().to_ascii_lowercase().as_str(),
                    "kde" | "plasma" | "plasmax11" | "plasmawayland"
                )
            })
        })
}

/// Is this session Cinnamon (Linux Mint)? Its muffin compositor has no
/// layer-shell on Wayland and reads its own background schema.
pub fn is_cinnamon() -> bool {
    [
        std::env::var("XDG_CURRENT_DESKTOP").ok(),
        std::env::var("XDG_SESSION_DESKTOP").ok(),
    ]
    .iter()
    .flatten()
    .any(|v| is_cinnamon_name(Some(v)))
}

/// Is this a real GNOME session (the desktop name says so)? The still-frame
/// backend is GNOME's, but any Wayland compositor without layer-shell lands in
/// it too (an older Cinnamon, say); GNOME-specific advice (log into an Xorg
/// session, wait for a Fresco GNOME extension) must not be shown to those.
pub fn is_gnome_session() -> bool {
    [
        std::env::var("XDG_CURRENT_DESKTOP").ok(),
        std::env::var("XDG_SESSION_DESKTOP").ok(),
    ]
    .iter()
    .flatten()
    .any(|v| is_gnome(Some(v)))
}

fn is_cinnamon_name(desktop: Option<&str>) -> bool {
    desktop
        .map(|d| d.to_ascii_lowercase().contains("cinnamon"))
        .unwrap_or(false)
}

fn is_gnome(desktop: Option<&str>) -> bool {
    desktop
        .map(|d| d.to_ascii_lowercase().contains("gnome"))
        .unwrap_or(false)
}

/// Can this machine start a GNOME session on X11 instead of Wayland?
///
/// GNOME on Wayland is the one session where Fresco can only show a still
/// frame (no `zwlr_layer_shell_v1`; see [`Capability::WaylandGnomeStatic`]),
/// and the only way to a live wallpaper there *today* is to log out and pick
/// an Xorg session on the greeter. Whether that choice exists depends on the
/// distro: Ubuntu 22.04 / 24.04 still ship "Ubuntu on Xorg", but GNOME 49
/// disabled its X11 session at build time and GNOME 50 removed the code, so
/// Ubuntu 25.10+ and 26.04 LTS, Fedora 43 and newer ship no Xorg session at
/// all. Telling the user to "log in on Xorg" when no such entry exists is
/// worse than saying nothing, so the banner and `fresco doctor` ask first.
///
/// Reads the display manager's own session list — the `*.desktop` files under
/// each `xsessions` directory — and never fails: an unreadable or missing
/// directory is simply "none".
pub fn gnome_x11_session_available() -> bool {
    let dirs = xsession_dirs(std::env::var("XDG_DATA_DIRS").ok().as_deref());
    dirs.iter().any(|dir| {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|e| {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "desktop") {
                return false;
            }
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            std::fs::read_to_string(&path)
                .map(|contents| xsession_is_gnome(&stem, &contents))
                .unwrap_or(false)
        })
    })
}

/// `xsessions` directories to scan: every `XDG_DATA_DIRS` entry (spec default
/// `/usr/local/share:/usr/share` when unset or empty) plus `/usr/share`
/// itself, which is where GDM looks regardless of the variable. Order is kept
/// and duplicates dropped.
fn xsession_dirs(xdg_data_dirs: Option<&str>) -> Vec<std::path::PathBuf> {
    let raw = xdg_data_dirs
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("/usr/local/share:/usr/share");
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    for base in raw.split(':').chain(std::iter::once("/usr/share")) {
        let base = base.trim();
        if base.is_empty() {
            continue;
        }
        let dir = std::path::Path::new(base).join("xsessions");
        if !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

/// Is this `xsessions/<stem>.desktop` a GNOME Shell session on X11?
///
/// Pure over the file's name and contents so it is testable without a display
/// manager. Only the `[Desktop Entry]` group counts, and plain `Name=` only
/// (not the localized `Name[xx]=`). A session qualifies when it launches
/// `gnome-session` (Ubuntu on Xorg, GNOME on Xorg, GNOME Classic on Xorg all
/// do) or is named "GNOME/Ubuntu on Xorg/X11". Entries the greeter itself
/// would not list (`Hidden=true`, `NoDisplay=true`) and GNOME Flashback (a
/// different shell Fresco has never been verified on) do not.
fn xsession_is_gnome(file_stem: &str, contents: &str) -> bool {
    let (mut name, mut exec, mut try_exec) = (String::new(), String::new(), String::new());
    let (mut hidden, mut in_entry) = (false, false);
    for line in contents.lines() {
        let line = line.trim();
        if let Some(group) = line.strip_prefix('[') {
            in_entry = group.trim_end().strip_suffix(']') == Some("Desktop Entry");
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Name" => name = value.to_ascii_lowercase(),
            "Exec" => exec = value.to_ascii_lowercase(),
            "TryExec" => try_exec = value.to_ascii_lowercase(),
            "Hidden" | "NoDisplay" if value.eq_ignore_ascii_case("true") => hidden = true,
            _ => {}
        }
    }
    let stem = file_stem.to_ascii_lowercase();
    if hidden
        || [&stem, &name, &exec]
            .iter()
            .any(|v| v.contains("flashback"))
    {
        return false;
    }
    let launches_gnome = exec.contains("gnome-session") || try_exec.contains("gnome-session");
    let named_gnome_xorg = (name.contains("gnome") || name.contains("ubuntu"))
        && (name.contains("xorg") || name.contains("x11"));
    launches_gnome || named_gnome_xorg
}

/// The running GNOME Shell's version string (`"49.0"`, `"50.beta"`), or `None`
/// when it cannot be read — not GNOME, no session bus, `gdbus` missing, or the
/// shell not answering within 2 s. Blocking, so for `fresco doctor` and other
/// one-shot callers, never a render or poll loop.
pub fn gnome_shell_version() -> Option<String> {
    let out = std::process::Command::new("gdbus")
        .args(["call", "--session", "--timeout", "2"])
        .args(["--dest", "org.gnome.Shell"])
        .args(["--object-path", "/org/gnome/Shell"])
        .args([
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.gnome.Shell",
            "ShellVersion",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_shell_version(&String::from_utf8_lossy(&out.stdout))
}

/// Pull the version out of `gdbus call`'s reply to `Properties.Get`, a
/// one-tuple wrapping a string variant: `(<'49.0'>,)`. Anything else (an error
/// message, empty output, a non-string variant, a version with characters no
/// GNOME release has ever used) is `None`.
fn parse_shell_version(out: &str) -> Option<String> {
    let inner = out.trim().strip_prefix("(<")?.strip_suffix(">,)")?.trim();
    let quote = inner.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
    let version = inner.strip_prefix(quote)?.strip_suffix(quote)?;
    let valid = !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | '~'));
    valid.then(|| version.to_string())
}

/// Probe the live Wayland registry for `zwlr_layer_shell_v1` — no external tools.
/// `Some(true/false)` when we could talk to the compositor; `None` only if we
/// couldn't connect at all, leaving the decision to the desktop-name heuristic.
fn probe_layer_shell() -> Option<bool> {
    probe_wayland_globals().map(|g| g.layer_shell)
}

/// Which lock-related Wayland globals this compositor advertises, as probed
/// by [`probe_wayland_globals`] in a single registry roundtrip.
///
/// `bool` fields, not `Option`: a successful roundtrip that simply never
/// sees a given global IS the answer "not present" — it is
/// [`probe_wayland_globals`]'s own `Option<WaylandGlobals>` return type that
/// carries "couldn't even connect" (`None`), same contract the private
/// `probe_layer_shell` already had before this struct existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WaylandGlobals {
    /// `zwlr_layer_shell_v1` — live wallpaper backend (see [`Capability`]).
    pub layer_shell: bool,
    /// `ext_session_lock_manager_v1` — lets a client implement a real
    /// session locker (swaylock-plugin, and Fresco's own wave-2b wlroots
    /// lock host); see `daemon::lock::hosts::HostKind::Wlroots`.
    pub session_lock_manager: bool,
    /// `cosmic_session_lock_layer_manager_v1` — cosmic-comp's opt-in
    /// show-on-lock flag for a layer-shell surface; see
    /// `daemon::lock::hosts::HostKind::Cosmic`.
    pub cosmic_lock_layer_manager: bool,
}

/// Probe the live Wayland registry for every lock-related global Fresco cares
/// about, in one roundtrip — no external tools, and no new dependency on a
/// protocol-bindings crate for the two globals besides `zwlr_layer_shell_v1`:
/// telling whether a global is *advertised at all* only ever needs its
/// interface *name* (`wl_registry::Event::Global`'s `interface: String`), so
/// there is nothing here `wayland-client` (already a dependency) cannot do on
/// its own — `wayland-protocols`/`cosmic-protocols`'s lock-layer extension
/// would only earn their keep once something actually *binds* one of these
/// globals to create an object from it, which is wave 2/2b's job, not this
/// probe's.
///
/// `None` only if no Wayland connection could be made at all — same contract
/// the private `probe_layer_shell` (now built on this) always had.
#[cfg(feature = "daemon")]
pub fn probe_wayland_globals() -> Option<WaylandGlobals> {
    use wayland_client::protocol::wl_registry;
    use wayland_client::{Connection, Dispatch, QueueHandle};

    #[derive(Default)]
    struct Probe {
        globals: WaylandGlobals,
    }
    impl Dispatch<wl_registry::WlRegistry, ()> for Probe {
        fn event(
            state: &mut Self,
            _: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global { interface, .. } = event {
                match interface.as_str() {
                    "zwlr_layer_shell_v1" => state.globals.layer_shell = true,
                    "ext_session_lock_manager_v1" => state.globals.session_lock_manager = true,
                    "cosmic_session_lock_layer_manager_v1" => {
                        state.globals.cosmic_lock_layer_manager = true
                    }
                    _ => {}
                }
            }
        }
    }

    let conn = Connection::connect_to_env().ok()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut probe = Probe::default();
    queue.roundtrip(&mut probe).ok()?;
    Some(probe.globals)
}

/// GUI-only builds don't link `wayland-client`; fall back to "couldn't connect".
#[cfg(not(feature = "daemon"))]
pub fn probe_wayland_globals() -> Option<WaylandGlobals> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x11_session_is_x11() {
        assert_eq!(
            classify(Some("x11"), false, Some("pop:GNOME")),
            Capability::X11
        );
        // Session type wins even if WAYLAND_DISPLAY leaks into an X11 session.
        assert_eq!(classify(Some("x11"), true, Some("GNOME")), Capability::X11);
    }

    #[test]
    fn wayland_gnome_is_static() {
        for d in ["pop:GNOME", "ubuntu:GNOME", "GNOME", "gnome"] {
            assert_eq!(
                classify(Some("wayland"), true, Some(d)),
                Capability::WaylandGnomeStatic,
                "desktop {d}"
            );
        }
    }

    #[test]
    fn wayland_cinnamon_is_static_in_the_name_only_fallback() {
        // Only reached when no Wayland connection could be made at all; a
        // live probe (see `daemon::cinnamon_bg`) is what actually tells a
        // current, layer-shell-capable muffin apart from an old one.
        for d in ["X-Cinnamon", "Cinnamon", "cinnamon"] {
            assert_eq!(
                classify(Some("wayland"), true, Some(d)),
                Capability::WaylandGnomeStatic,
                "desktop {d}"
            );
            assert!(is_cinnamon_name(Some(d)));
        }
        assert!(!is_cinnamon_name(Some("GNOME")));
    }

    #[test]
    fn wayland_non_gnome_is_layer_shell() {
        for d in ["Hyprland", "sway", "KDE", "wlroots", "COSMIC", "river"] {
            assert_eq!(
                classify(Some("wayland"), true, Some(d)),
                Capability::WaylandLayerShell,
                "desktop {d}"
            );
        }
    }

    #[test]
    fn deepin_dde_detection() {
        for d in ["Deepin", "deepin", "DDE", "dde", "X-Deepin", "Deepin:GNOME"] {
            assert!(classify_deepin_dde(Some(d), None), "current {d}");
            assert!(classify_deepin_dde(None, Some(d)), "session {d}");
        }
        for d in [
            "GNOME",
            "KDE",
            "pop:GNOME",
            "ubuntu:GNOME",
            "kddesomething",
            "",
        ] {
            assert!(!classify_deepin_dde(Some(d), None), "current {d}");
        }
        assert!(!classify_deepin_dde(None, None));
        // Second var still detected when the first is a non-DDE desktop.
        assert!(classify_deepin_dde(Some("GNOME"), Some("dde")));
    }

    #[test]
    fn mate_detection() {
        for d in ["MATE", "mate", "X-Generic:MATE"] {
            assert!(classify_mate(Some(d), None), "current {d}");
            assert!(classify_mate(None, Some(d)), "session {d}");
        }
        for d in ["GNOME", "X-Cinnamon", "ultimate", "mate-ish", "Deepin", ""] {
            assert!(!classify_mate(Some(d), None), "current {d}");
        }
        assert!(!classify_mate(None, None));
    }

    #[test]
    fn xfce_detection() {
        for d in ["XFCE", "xfce", "X-Generic:XFCE", "XFCE:Xubuntu", "Xubuntu"] {
            assert!(classify_xfce(Some(d), None), "current {d}");
            assert!(classify_xfce(None, Some(d)), "session {d}");
        }
        for d in [
            "GNOME",
            "MATE",
            "xfce4-ish",
            "notxfce",
            "LXQt",
            "Deepin",
            "",
        ] {
            assert!(!classify_xfce(Some(d), None), "current {d}");
        }
        assert!(!classify_xfce(None, None));
    }

    #[test]
    fn kde_detection() {
        for d in ["KDE", "kde", "plasma", "ubuntu:KDE", "plasmax11"] {
            assert!(classify_kde(Some(d), None), "current {d}");
            assert!(classify_kde(None, Some(d)), "session {d}");
        }
        for d in [
            "GNOME",
            "X-Cinnamon",
            "kdelike",
            "KDE-ish",
            "Deepin",
            "COSMIC",
            "",
        ] {
            assert!(!classify_kde(Some(d), None), "current {d}");
        }
        assert!(!classify_kde(None, None));
    }

    const UBUNTU_XORG: &str = "[Desktop Entry]\n\
Name=Ubuntu on Xorg\n\
Name[de]=Ubuntu auf Xorg\n\
Comment=This session logs you into Ubuntu\n\
Exec=env GNOME_SHELL_SESSION_MODE=ubuntu /usr/bin/gnome-session --session=ubuntu\n\
TryExec=/usr/bin/gnome-shell\n\
Type=Application\n\
DesktopNames=ubuntu:GNOME\n";
    const FEDORA_GNOME_XORG: &str =
        "[Desktop Entry]\nName=GNOME on Xorg\nExec=gnome-session\nType=Application\n";

    #[test]
    fn xsession_parser_accepts_gnome_and_ubuntu_xorg_sessions() {
        assert!(xsession_is_gnome("ubuntu-xorg", UBUNTU_XORG));
        assert!(xsession_is_gnome("gnome-xorg", FEDORA_GNOME_XORG));
        let classic = "[Desktop Entry]\nName=GNOME Classic on Xorg\n\
                       Exec=gnome-session --session=gnome-classic\n";
        assert!(xsession_is_gnome("gnome-classic-xorg", classic));
        // Named like one even if Exec is a wrapper script.
        let wrapped = "[Desktop Entry]\nName=GNOME on X11\nExec=/usr/bin/start-session\n";
        assert!(xsession_is_gnome("gnome-x11", wrapped));
    }

    #[test]
    fn xsession_parser_rejects_other_desktops_and_unlisted_entries() {
        let plasma = "[Desktop Entry]\nName=Plasma (X11)\nExec=/usr/bin/startplasma-x11\n\
                      DesktopNames=KDE\n";
        assert!(!xsession_is_gnome("plasmax11", plasma));
        let xfce = "[Desktop Entry]\nName=Xfce Session\nExec=startxfce4\n";
        assert!(!xsession_is_gnome("xfce", xfce));
        // Budgie advertises GNOME in DesktopNames but is not a GNOME Shell session.
        let budgie = "[Desktop Entry]\nName=Budgie Desktop\nExec=budgie-desktop\n\
                      DesktopNames=Budgie:GNOME\n";
        assert!(!xsession_is_gnome("budgie-desktop", budgie));
        // GNOME Flashback is a different shell.
        let flashback = "[Desktop Entry]\nName=GNOME Flashback (Metacity)\n\
                         Exec=gnome-session --session=gnome-flashback-metacity\n";
        assert!(!xsession_is_gnome("gnome-flashback-metacity", flashback));
        // The greeter does not list these, so neither do we.
        for flag in ["Hidden=true", "NoDisplay=true", "NoDisplay=TRUE"] {
            let hidden = format!("{FEDORA_GNOME_XORG}{flag}\n");
            assert!(!xsession_is_gnome("gnome-xorg", &hidden), "{flag}");
        }
        assert!(xsession_is_gnome(
            "gnome-xorg",
            &format!("{FEDORA_GNOME_XORG}NoDisplay=false\n")
        ));
        assert!(!xsession_is_gnome("anything", ""));
    }

    #[test]
    fn xsession_parser_reads_only_the_desktop_entry_group() {
        let other_group = "[Desktop Entry]\nName=Custom\nExec=custom-session\n\
                           [Desktop Action Foo]\nExec=gnome-session\n";
        assert!(!xsession_is_gnome("custom", other_group));
        let before_group = "Exec=gnome-session\n[Desktop Entry]\nName=Custom\n";
        assert!(!xsession_is_gnome("custom", before_group));
        // A localized name alone never makes a session GNOME.
        let localized = "[Desktop Entry]\nName=Custom\nName[de]=GNOME auf Xorg\nExec=custom\n";
        assert!(!xsession_is_gnome("custom", localized));
    }

    #[test]
    fn xsession_dirs_follow_xdg_data_dirs_and_always_include_usr_share() {
        let p = std::path::PathBuf::from;
        assert_eq!(
            xsession_dirs(None),
            vec![p("/usr/local/share/xsessions"), p("/usr/share/xsessions")]
        );
        assert_eq!(xsession_dirs(Some("  ")), xsession_dirs(None));
        assert_eq!(
            xsession_dirs(Some("/opt/share::/usr/share")),
            vec![p("/opt/share/xsessions"), p("/usr/share/xsessions")]
        );
        assert_eq!(
            xsession_dirs(Some("/var/lib/flatpak/exports/share")),
            vec![
                p("/var/lib/flatpak/exports/share/xsessions"),
                p("/usr/share/xsessions")
            ]
        );
    }

    #[test]
    fn shell_version_parser_reads_the_properties_get_reply() {
        assert_eq!(parse_shell_version("(<'49.0'>,)\n"), Some("49.0".into()));
        assert_eq!(
            parse_shell_version("(<'50.beta'>,)"),
            Some("50.beta".into())
        );
        assert_eq!(
            parse_shell_version("  (<\"46.2\">,)  "),
            Some("46.2".into())
        );
    }

    #[test]
    fn shell_version_parser_rejects_everything_else() {
        for bad in [
            "",
            "\n",
            "()",
            "(<''>,)",
            "(<49>,)",
            "(<int32 49>,)",
            "(<'49.0'>)",
            "('49.0',)",
            "Error: GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown",
            "(<'49.0\\n'>,)",
            "(<'49 0'>,)",
            "(<'49.0\">,)",
        ] {
            assert_eq!(parse_shell_version(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn falls_back_to_wayland_display_when_session_type_unset() {
        assert_eq!(
            classify(None, true, Some("sway")),
            Capability::WaylandLayerShell
        );
        assert_eq!(
            classify(None, true, Some("GNOME")),
            Capability::WaylandGnomeStatic
        );
        assert_eq!(classify(None, false, None), Capability::X11);
    }
}

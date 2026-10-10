//! wlroots family (Sway, Hyprland, niri, river, labwc, Wayfire) lock host:
//! `fresco lock` runs **swaylock-plugin** with Fresco's own mpvpaper as its
//! per-output background, so the wallpaper (and, once the widget engine
//! lands, Fresco's widgets) keep showing behind swaylock's own password
//! surface. See `docs/plan-lock-screen.md` §4.2 for the design; this module
//! is the wave-2b implementation of the stub that shipped with the contract.
//!
//! # swaylock-plugin facts this implementation relies on
//!
//! Verified 2026-09-28 against the actual source at
//! <https://github.com/mstoeckl/swaylock-plugin> (`main` branch: `main.c`,
//! `pam.c`, `comm.c`, `meson.build`, `swaylock.1.scd`, `README.md`) rather
//! than assumed from the upstream `swaylock` this is forked from:
//!
//! * **`--command-each <cmd>` runs the exact same string once per output**,
//!   via `posix_spawnp(&pid, "sh", ..., {"sh", "-c", cmd, NULL}, envp)`
//!   (`main.c`'s `spawn_command`) — a real `sh -c`, not a direct `execvp` of
//!   `cmd`. Every command this module hands to swaylock-plugin is therefore
//!   POSIX shell source, not an argv list, and must be quoted accordingly —
//!   see [`sh_quote`].
//! * **Each instance learns its output from the environment, not argv**:
//!   `spawn_command` sets `WAYLAND_SOCKET` (a fresh, private connection to
//!   swaylock-plugin's own embedded Wayland proxy — see `forward.c` —
//!   forwarded to the real compositor) plus `SWAYLOCK_PLUGIN_OUTPUT_NAME`
//!   and `SWAYLOCK_PLUGIN_OUTPUT_DESC` (the compositor's `wl_output::name`/
//!   `description`) before running the *same* command text again for the
//!   next output. One generated script therefore has to branch on
//!   `$SWAYLOCK_PLUGIN_OUTPUT_NAME` at run time to pick the right media file
//!   and IPC socket per output — see [`plugin_command_script`].
//! * **PAM service name is `"swaylock-plugin"`**, both compiled in
//!   (`pam.c`: `pam_start("swaylock-plugin", username, &conv, &auth_handle)`,
//!   and its own error text: `"check /etc/pam.d/swaylock-plugin has been
//!   installed properly"`) and where `meson.build` installs the shipped
//!   `pam/swaylock-plugin` file (`install_data(...,  install_dir:
//!   get_option('sysconfdir') / 'pam.d')`, i.e. `/etc/pam.d/swaylock-plugin`
//!   with the default `sysconfdir`). A locker with no PAM file for that
//!   service name can *never* successfully authenticate — `pam_start` itself
//!   would still return `PAM_SUCCESS` (it doesn't read the service file),
//!   but every subsequent `pam_authenticate` fails, so the user is locked
//!   out with a password that is provably correct. [`pam_service_available`]
//!   is the gate that stops this before it ever locks anyone out.
//! * **`-f`/`--daemonize` forks only *after* the compositor confirms the
//!   lock.** `main()`'s tail is, in order: spin
//!   `while (!state.locked && state.run_display)` dispatching Wayland events
//!   (this is what lets the per-output plugin commands above actually start
//!   and draw, since compositors may wait for lock surfaces to be ready);
//!   once `ext_session_lock_v1`'s `locked` event fires (`state.locked =
//!   true`), write `--ready-fd` if given; only then call `daemonize()`. And
//!   `daemonize()` itself blocks the original process on a pipe read until
//!   the forked, `setsid()`-ed grandchild reports success, so the **original
//!   process exiting 0 is a two-part proof**: the compositor granted the
//!   session lock, and the daemonizing fork/setsid also succeeded. If the
//!   lock instead fails (e.g. another locker already holds it),
//!   `ext_session_lock_v1`'s `finished` handler calls `exit(2)` immediately,
//!   without ever reaching the daemonize tail — a non-zero exit, not a hang.
//!   This is exactly the "wait for the foreground process to exit 0" signal
//!   [`wait_for_lock_confirmation`] uses; no `--ready-fd` plumbing is needed.
//! * **Background command processes are not explicitly killed on
//!   unlock/exit.** There is no `kill()`/`waitpid()`-on-plugin-pid anywhere
//!   in `main.c`; `SIGCHLD` is set to `SIG_IGN` purely so they are reaped
//!   without a `wait()` call. What actually ends them is indirect: each is a
//!   Wayland client of swaylock-plugin's own embedded proxy server
//!   (`forward.c`), and when swaylock-plugin's process exits (password
//!   accepted, `SIGUSR1`, or a crash) that embedded server goes away with
//!   it, which breaks the background command's Wayland connection and makes
//!   *it* exit on its own shortly after — not a direct signal, but still a
//!   real, observable "this output's renderer stopped". **Consequence for
//!   whoever builds unlock detection on top of [`LockTargets::Sockets`]**:
//!   polling whether a lock socket is still connectable is a valid signal
//!   that the session unlocked (or that one output's renderer merely
//!   crashed — swaylock-plugin restarts a crashed `--command-each` instance
//!   on its own, matching the README's "restarted if it closes that
//!   connection", so a single blip is not proof of unlock, only a sustained
//!   one across every output is).
//! * **Styling flags survive in this fork.** `swaylock.1.scd` (this fork's
//!   own man page, not upstream's) documents every flag this module sets —
//!   `--indicator-radius`, `--indicator-thickness`, `--indicator-y-position`,
//!   `--inside-color`, `--ring-color`, `--key-hl-color`, `--line-color`,
//!   `--text-color`, `--font`, `--font-size`, `-F`/`-k`/`-l` — unchanged from
//!   upstream `swaylock`. `--indicator-idle-visible` is also still present
//!   but deliberately **not** set: its absence is what makes the ring hidden
//!   while idle and only appear once the user starts typing, which is the
//!   look asked for here. `-e`/`--ignore-empty-password` is left unset too —
//!   Fresco has no opinion on that trade-off, and setting it would weaken
//!   authentication behaviour this feature must never touch.
//!
//! # Why only `--indicator-y-position` is overridden, never `-x-position`
//!
//! `render.c`'s `render_frame` computes each output's own indicator
//! position independently, using *that surface's* `width`/`height`:
//! `subsurf_xpos` defaults to `surface->width / 2 - …` and
//! `subsurf_ypos` to `surface->height / 2 - …`, and an override
//! (`--indicator-x/y-position`) replaces the whole expression with a single
//! global absolute pixel value reused as-is on every output. Leaving
//! `--indicator-x-position` unset gets horizontal centering **correct on
//! every output at once**, regardless of resolution — and it also agrees
//! with `widgetkit::lockscene::prompt_zone`, whose own `x` is always
//! `(output.w - w) / 2`, i.e. `output.w / 2` once centred: exactly the point
//! swaylock's own unoverridden default already converges on.
//!
//! There is no equivalent free lunch for Y: swaylock's own default is the
//! surface's vertical middle, not `prompt_zone`'s ~60%-down centre, so
//! [`indicator_y_position`] computes an explicit override from
//! `widgetkit::lockscene::prompt_zone` — the exact reservation the widget
//! layout itself uses for a password prompt, so this host's ring and
//! Fresco's own widgets never disagree about where the prompt lives. A
//! *global* override, though, is only ever correct for whichever output's
//! size it was computed from, so with outputs of different heights
//! [`smallest_output_size`] picks the **smallest** one; that function's own
//! doc comment, and [`indicator_y_position`]'s, work out exactly how far a
//! taller output's own zone can drift from that choice before the ring lands
//! outside it. A multi-monitor setup mismatched enough to exceed that bound
//! gets the exact centre on only its smallest output — a limitation of
//! swaylock-plugin's one-flag-for-every-output design, not something a
//! cleverer argument list can fix.
//!
//! # What this module does not do
//!
//! No PAM, no password, no auth decision of any kind — swaylock-plugin's own
//! already-audited core owns all of that, exactly as
//! `docs/plan-lock-screen.md` §5 requires. This module's whole job is
//! building the argv/shell-script swaylock-plugin runs and deciding whether
//! it is even safe to try (the PAM gate above).

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{HostCtx, HostKind, LockHost, LockTargets, RunningHost};
use crate::config::{Config, WidgetTheme};
use crate::daemon::widgets::OutputGeom;
use crate::ipc::{LockSetupState, LockSocket};
use crate::widgetkit::{prompt_zone, Color, Mode, Size, Theme};

pub struct WlrootsHost;

/// PAM service name swaylock-plugin is compiled to request. See this
/// module's own doc comment for the citation.
const PAM_SERVICE: &str = "swaylock-plugin";

/// Where a PAM service file might live, checked in this order.
/// `/etc/pam.d` is where every mainstream distro's package puts it (it is
/// also literally `meson.build`'s default `sysconfdir`); the other two cover
/// distros that ship factory PAM config under `/usr/lib/pam.d` (with `/etc`
/// reserved for local overrides) or run a read-only-`/usr` layout that
/// symlinks `/etc` under `/usr/etc`.
const PAM_ROOTS: [&str; 3] = ["/etc/pam.d", "/usr/lib/pam.d", "/usr/etc/pam.d"];

/// The swaylock-plugin binary name to look for on `PATH`.
const SWAYLOCK_PLUGIN_BIN: &str = "swaylock-plugin";

/// Bundled fallback location, mirroring how Fresco bundles mpvpaper next to
/// itself (`packaging/mpvpaper/`) so wlroots users need nothing extra
/// installed.
const BUNDLED_SWAYLOCK_PLUGIN: &str = "/usr/lib/fresco/swaylock-plugin";

/// Env var a user (or a test) can set to override [`locate_swaylock_plugin`]
/// entirely, mirroring `FRESCO_MPVPAPER`'s "always wins, unprobed" contract
/// (`crate::choose_mpvpaper`'s own doc comment) — the whole point of an
/// override is to replace Fresco's judgement, including when that judgement
/// would otherwise say "not found".
const SWAYLOCK_PLUGIN_ENV: &str = "FRESCO_SWAYLOCK_PLUGIN";

/// Longest [`LockHost::lock`] waits for swaylock-plugin's foreground process
/// to confirm the lock (exit 0 after `-f`/`--daemonize`; see this module's
/// doc comment for why that exit code is trustworthy proof) before treating
/// it as hung/failed. Generous: waiting for a compositor to hand back
/// lock-surface configure events for every output is normally near-instant,
/// but a busy/loaded system is not this module's problem to diagnose beyond
/// "did not lock in time".
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// `docs/plan-lock-screen.md` §3.4's reserved "prompt zone" on this host *is*
/// wherever swaylock-plugin draws its own indicator — there is no second,
/// Fresco-drawn prompt zone to keep clear of, so [`indicator_y_position`] and
/// [`indicator_radius`] simply read `widgetkit::lockscene::prompt_zone`
/// directly rather than re-deciding where the indicator goes independently.
/// This used to be a standalone placeholder constant (a flat 62% of the
/// output height) with a comment promising exactly this switch once
/// `prompt_zone` existed; it now does.
///
/// Indicator ring radius/thickness are otherwise in the logical pixels
/// swaylock's own `--indicator-radius`/`--indicator-thickness` take, not tied
/// to `widgetkit::theme::Metrics` (that scale is for card-shaped widgets
/// Fresco itself rasterises; swaylock's ring is a different visual language
/// drawn by a different program). [`INDICATOR_RADIUS`] is a cap — picked to
/// read clearly at both 1080p and 4K without dominating the screen the way
/// swaylock's own default (50/10) tends to look undersized on a 4K panel —
/// rather than a fixed value: [`indicator_radius`] shrinks it on a small
/// enough output so the ring still fits inside `prompt_zone`.
const INDICATOR_RADIUS: u32 = 90;
const INDICATOR_THICKNESS: u32 = 8;

/// Floor under [`indicator_radius`]'s result. Realistically unreachable — no
/// real monitor is a handful of pixels tall — but a pure function should
/// still return something drawable rather than a radius so small (or, past
/// `u32`'s saturation at zero, so negative) it would round away to nothing on
/// a pathological/synthetic output size.
const MIN_INDICATOR_RADIUS: u32 = 8;

/// Indicator font. "Inter" is Fresco's own primary UI face
/// (`widgetkit::text::LATIN_FAMILIES[0]`) — used here purely for brand
/// consistency with the rest of Fresco's on-screen text; swaylock falls back
/// to a system default if this particular family is not installed.
const INDICATOR_FONT: &str = "Inter";
const INDICATOR_FONT_SIZE: u32 = 22;

impl LockHost for WlrootsHost {
    fn kind(&self) -> HostKind {
        HostKind::Wlroots
    }

    fn lock(&self, ctx: &HostCtx) -> Result<RunningHost, String> {
        let Some(bin) = locate_swaylock_plugin() else {
            return Err(format!(
                "swaylock-plugin not found (set {SWAYLOCK_PLUGIN_ENV}, install swaylock-plugin, \
                 or place a copy at {BUNDLED_SWAYLOCK_PLUGIN})"
            ));
        };
        if !pam_service_available() {
            return Err(format!(
                "swaylock-plugin's PAM service file ({PAM_SERVICE}) is missing from {} — a \
                 locker with no PAM file can never unlock, so Fresco refuses to use it",
                PAM_ROOTS.join(", "),
            ));
        }

        let mpv_bin = PathBuf::from(crate::mpvpaper_command());
        let playing_live = ctx
            .config
            .lockscreen
            .as_ref()
            .map(|l| l.live_video)
            .unwrap_or_default()
            .plays(crate::battery::on_battery());

        let (script, sockets) = build_plugin_command(
            ctx.config,
            ctx.outputs,
            &ctx.runtime_dir,
            &mpv_bin,
            playing_live,
        );
        if sockets.is_empty() {
            return Err("no output has a wallpaper configured to show on the lock screen".into());
        }

        let mode = widget_mode(
            ctx.config
                .widgets
                .as_ref()
                .map(|w| w.theme)
                .unwrap_or_default(),
        );
        let theme = Theme::for_accent(mode, ctx.config.accent);

        let mut cmd = Command::new(&bin);
        cmd.arg("--command-each")
            .arg(&script)
            .arg("-f") // --daemonize: fork only once locked (see module docs)
            .arg("-F") // --show-failed-attempts
            .arg("-k") // --show-keyboard-layout
            .arg("-l"); // --indicator-caps-lock
        match smallest_output_size(ctx.outputs) {
            Some(reference) => {
                cmd.arg("--indicator-y-position")
                    .arg(indicator_y_position(reference).to_string())
                    .arg("--indicator-radius")
                    .arg(indicator_radius(reference).to_string());
            }
            None => {
                // Nothing to size the ring from (in practice the
                // `sockets.is_empty()` check above already returns before
                // this point for a truly empty `ctx.outputs`, but
                // `smallest_output_size` stays total rather than this
                // function inventing a position with nothing to base it on):
                // leave `--indicator-y-position` unset entirely so
                // swaylock-plugin falls back to its own vertical-middle
                // default, and keep the plain radius cap.
                cmd.arg("--indicator-radius")
                    .arg(INDICATOR_RADIUS.to_string());
            }
        }
        cmd.arg("--indicator-thickness")
            .arg(INDICATOR_THICKNESS.to_string())
            .arg("--ring-color")
            .arg(hex_rrggbbaa(theme.accent_fill))
            .arg("--inside-color")
            .arg(hex_rrggbbaa(theme.well))
            .arg("--line-color")
            .arg(hex_rrggbbaa(theme.edge))
            .arg("--key-hl-color")
            .arg(hex_rrggbbaa(theme.accent_fill))
            .arg("--text-color")
            .arg(hex_rrggbbaa(theme.text_primary))
            .arg("--font")
            .arg(INDICATOR_FONT)
            .arg("--font-size")
            .arg(INDICATOR_FONT_SIZE.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        {
            use std::os::unix::process::CommandExt;
            // Own process group: a frescod crash/restart must never take the
            // lock down with it (docs/plan-lock-screen.md §5's fail-closed
            // invariant covers the *locker*, and this is what keeps it
            // independent of the process that merely asked it to start).
            cmd.process_group(0);
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("failed to start {}: {e}", bin.display()))?;
        wait_for_lock_confirmation(child, LOCK_TIMEOUT)?;

        Ok(RunningHost {
            child: None,
            targets: LockTargets::Sockets(sockets),
        })
    }

    fn targets_while_locked(&self, _ctx: &HostCtx) -> LockTargets {
        // Nothing to paint into until `lock()` itself is what did the
        // locking — if this session got locked some other way (a compositor
        // keybind calling swaylock directly, say), Fresco has no surface on
        // that lock screen at all.
        LockTargets::None
    }

    fn notes(&self, _ctx: &HostCtx) -> Vec<String> {
        vec![match current_availability() {
            Availability::Ready => crate::t!(
                "Locks with swaylock-plugin, showing Fresco's wallpaper behind the password prompt"
            ),
            Availability::MissingBinary => crate::t!(
                "swaylock-plugin isn't installed, so Fresco can't draw on the lock screen here"
            ),
            Availability::MissingPam => crate::t!(
                "swaylock-plugin is missing its PAM setup, so Fresco won't use it to lock the screen"
            ),
        }
        .to_string()]
    }

    fn setup_state(&self, _ctx: &HostCtx) -> LockSetupState {
        match current_availability() {
            Availability::Ready => LockSetupState::NotNeeded,
            Availability::MissingBinary | Availability::MissingPam => LockSetupState::Unavailable,
        }
    }
}

// ── availability ─────────────────────────────────────────────────────────────

/// Whether this host can actually use swaylock-plugin right now, and if not,
/// which of the two independent reasons applies — the single source of truth
/// [`LockHost::notes`] and [`LockHost::setup_state`] both read, so the GUI's
/// status row and the log line `lock()` would produce on failure never say
/// two different things about the same machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Availability {
    Ready,
    MissingBinary,
    MissingPam,
}

fn availability(bin: Option<&Path>, pam_ok: bool) -> Availability {
    match (bin, pam_ok) {
        (None, _) => Availability::MissingBinary,
        (Some(_), false) => Availability::MissingPam,
        (Some(_), true) => Availability::Ready,
    }
}

fn current_availability() -> Availability {
    availability(locate_swaylock_plugin().as_deref(), pam_service_available())
}

// ── binary lookup ────────────────────────────────────────────────────────────

/// Pure core of [`locate_swaylock_plugin`]: env override, then `PATH`, then
/// the bundled copy, with `PATH`/filesystem access injected so this is
/// testable without touching the real environment. Mirrors
/// `crate::choose_mpvpaper`'s "override always wins, unprobed" contract: an
/// override is returned as-is even if `is_file` says it doesn't exist,
/// exactly like `FRESCO_MPVPAPER` — the point of an override is to replace
/// Fresco's own judgement, and a `Command::spawn` failure on a bad override
/// is a clearer signal than silently substituting a different binary.
fn resolve_swaylock_plugin(
    env_override: Option<&str>,
    path_var: Option<&str>,
    is_file: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(p) = env_override {
        return Some(PathBuf::from(p));
    }
    if let Some(path_var) = path_var {
        for dir in std::env::split_paths(path_var) {
            let candidate = dir.join(SWAYLOCK_PLUGIN_BIN);
            if is_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    let bundled = PathBuf::from(BUNDLED_SWAYLOCK_PLUGIN);
    is_file(&bundled).then_some(bundled)
}

fn locate_swaylock_plugin() -> Option<PathBuf> {
    let env = std::env::var(SWAYLOCK_PLUGIN_ENV).ok();
    let path = std::env::var("PATH").ok();
    resolve_swaylock_plugin(env.as_deref(), path.as_deref(), &|p: &Path| p.is_file())
}

// ── PAM gate ─────────────────────────────────────────────────────────────────

/// Pure core of [`pam_service_available`]: does any of `roots` contain a
/// [`PAM_SERVICE`] file? Injected `is_file` so this is testable against a
/// temp directory instead of the real `/etc`.
fn pam_service_present(roots: &[&Path], is_file: &dyn Fn(&Path) -> bool) -> bool {
    roots.iter().any(|r| is_file(&r.join(PAM_SERVICE)))
}

fn pam_service_available() -> bool {
    let roots: Vec<&Path> = PAM_ROOTS.iter().map(Path::new).collect();
    pam_service_present(&roots, &|p: &Path| p.is_file())
}

// ── per-output background command ───────────────────────────────────────────

/// Build the `--command-each` shell script and the [`LockSocket`] list
/// [`LockHost::lock`] reports back, one mpvpaper invocation per output in
/// `outputs` plus a best-effort default for any output the daemon did not
/// know about when it built this (a hot-plug racing the lock, say) — see
/// [`plugin_command_script`] for why a `case` over
/// `$SWAYLOCK_PLUGIN_OUTPUT_NAME` is what makes one shared script string
/// correct for every output despite `--command-each` never substituting
/// anything into the text itself.
///
/// `playing_live` is a plain `bool`, decided by the caller from
/// `LiveVideo::plays` — kept out of this function so it stays pure and
/// testable without touching `/sys/class/power_supply`.
fn build_plugin_command(
    config: &Config,
    outputs: &[OutputGeom],
    runtime_dir: &Path,
    mpv_bin: &Path,
    playing_live: bool,
) -> (String, Vec<LockSocket>) {
    let mut arms = Vec::new();
    let mut sockets = Vec::new();
    for out in outputs {
        let wallpaper = config.lock_source(Some(&out.connector));
        let Some(media) = wallpaper.effective_path() else {
            continue;
        };
        let sock = lock_socket_path(runtime_dir, &out.connector);
        let power_saving = wallpaper.effective_power_saving(config.power_saving);
        let mut opts =
            crate::daemon::mpvpaper::build_mpv_opts(wallpaper, config.scaling, power_saving, &sock);
        if !playing_live {
            // Paused at the file's start = its first frame for a video, and a
            // no-op for a still image — see the module doc on
            // `LiveVideo::plays` for why a lock screen left up for hours
            // makes this the honest default off AC power.
            opts.push_str(" pause=yes");
        }
        arms.push((
            out.connector.clone(),
            mpvpaper_shell_line(mpv_bin, &opts, media),
        ));
        sockets.push(LockSocket {
            connector: out.connector.clone(),
            path: sock.to_string_lossy().into_owned(),
        });
    }

    let fallback = config.lock_source(None);
    let default = fallback.effective_path().map(|media| {
        let sock = lock_socket_path(runtime_dir, "unknown");
        let power_saving = fallback.effective_power_saving(config.power_saving);
        let mut opts =
            crate::daemon::mpvpaper::build_mpv_opts(fallback, config.scaling, power_saving, &sock);
        if !playing_live {
            opts.push_str(" pause=yes");
        }
        mpvpaper_shell_line(mpv_bin, &opts, media)
    });

    (plugin_command_script(&arms, default.as_deref()), sockets)
}

/// One `sh -c` line: `exec <mpvpaper> -o <opts> "$SWAYLOCK_PLUGIN_OUTPUT_NAME" <media>`.
///
/// The connector is written as a literal shell variable reference, **not**
/// passed through [`sh_quote`] like every other piece: it must stay
/// `$SWAYLOCK_PLUGIN_OUTPUT_NAME` shell syntax so the running shell expands
/// it to whichever output actually invoked this line (verified:
/// swaylock-plugin sets that variable per spawn — see this module's top doc
/// comment). Double-quoting it (`"$VAR"`, not bare `$VAR`) is what keeps a
/// connector name containing spaces or shell metacharacters from being
/// word-split or glob-expanded, without needing to know its value in
/// advance. `exec` replaces the `sh` process outright rather than leaving it
/// as a wrapper, so signals and exit status go straight to mpvpaper.
fn mpvpaper_shell_line(mpv_bin: &Path, opts: &str, media: &Path) -> String {
    format!(
        "exec {} -o {} \"$SWAYLOCK_PLUGIN_OUTPUT_NAME\" {}",
        sh_quote(&mpv_bin.to_string_lossy()),
        sh_quote(opts),
        sh_quote(&media.to_string_lossy()),
    )
}

/// Build the full `--command-each` script: a `case` over
/// `$SWAYLOCK_PLUGIN_OUTPUT_NAME` selecting one of `arms` by connector name,
/// or `default` (or a plain `exit 0`, if none) for anything else.
///
/// This is what makes a **single** shared command string
/// (`--command-each`'s whole contract — see this module's top doc comment)
/// correct for outputs that need genuinely different arguments (a different
/// media file via `Config::monitors`, a different mpv IPC socket path): the
/// dispatch happens inside the shell script every time swaylock-plugin runs
/// it, not by Fresco generating a different string per output (it can't —
/// there is only one `--command-each` flag).
fn plugin_command_script(arms: &[(String, String)], default: Option<&str>) -> String {
    let mut s = String::from("case \"$SWAYLOCK_PLUGIN_OUTPUT_NAME\" in\n");
    for (connector, line) in arms {
        s.push_str(&format!("{})\n  {line}\n  ;;\n", sh_quote(connector)));
    }
    s.push_str("*)\n");
    s.push_str(&format!("  {}\n", default.unwrap_or("exit 0")));
    s.push_str("  ;;\nesac\n");
    s
}

/// POSIX `sh` single-quote escaping: wraps `s` in `'...'`, replacing every
/// embedded `'` with `'\''` (end the quoted string, an escaped literal
/// quote, start a new quoted string) — the standard, complete way to turn an
/// arbitrary byte string into one shell word with **no** character treated
/// specially. Needed because this text reaches a real `sh -c`, not `argv`
/// (verified: swaylock-plugin's `spawn_command` calls
/// `posix_spawnp(&pid, "sh", ..., {"sh", "-c", plugin_command, NULL}, ...)`)
/// — a file path or connector name containing spaces, quotes, `$`,
/// backticks, or arbitrary Unicode must survive as **data**, never as shell
/// syntax.
fn sh_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Mirrors `daemon::mpvpaper::sanitize` (private to that module): make a
/// connector name safe to use verbatim in a filename.
fn sanitize_connector(connector: &str) -> String {
    connector
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The mpv IPC socket path for one output's background renderer, reported
/// back to the daemon as a [`LockSocket`].
fn lock_socket_path(runtime_dir: &Path, connector: &str) -> PathBuf {
    runtime_dir.join(format!("lock-{}.sock", sanitize_connector(connector)))
}

// ── styling ──────────────────────────────────────────────────────────────────

/// Mirrors `daemon::widgets::widget_mode` (private to that module): `Auto`
/// reads as dark, for the reasons documented on
/// `crate::config::WidgetTheme` — a wallpaper's own brightness carries no
/// information about the desktop's light/dark preference, so `Auto` is a
/// fixed choice here too, not a follow-the-system one.
fn widget_mode(theme: WidgetTheme) -> Mode {
    match theme {
        WidgetTheme::Auto | WidgetTheme::Dark => Mode::Dark,
        WidgetTheme::Light => Mode::Light,
    }
}

/// `#RRGGBBAA`, straight (non-premultiplied) alpha — the form every
/// swaylock colour flag documents (`<rrggbb[aa]>`; the man page's own
/// config-file example shows `ring-color=#ff00ff`).
fn hex_rrggbbaa(c: Color) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        byte(c.r),
        byte(c.g),
        byte(c.b),
        byte(c.a)
    )
}

/// The output [`indicator_y_position`] and [`indicator_radius`] should be
/// computed from when more than one output is being locked at once: the one
/// with the smallest height. `None` for an empty slice — the signal
/// [`LockHost::lock`] uses to leave `--indicator-y-position` unset entirely
/// rather than invent a position with no output to base it on.
///
/// The smallest, specifically, because swaylock-plugin's `render.c` takes
/// `--indicator-y-position` as one absolute, **unscaled** pixel offset and
/// reuses it as-is on every output's own surface (this module's top doc
/// comment) — so the smallest output is the tightest constraint: it has the
/// least vertical room of any output in the set, and its own `prompt_zone`
/// sits at the smallest absolute pixel range. See [`indicator_y_position`]'s
/// own doc comment for exactly how far that choice can drift before it lands
/// outside a taller output's own zone.
fn smallest_output_size(outputs: &[OutputGeom]) -> Option<Size> {
    outputs
        .iter()
        .min_by_key(|o| o.h)
        .map(|o| Size::new(o.w as f32, o.h as f32))
}

/// The `--indicator-y-position` value for `reference`'s own output size: the
/// vertical centre of `crate::widgetkit::lockscene::prompt_zone` — the exact
/// spot the widget layout itself reserves for a password prompt, so this
/// host's ring and Fresco's own widgets (once the widget engine paints them
/// behind swaylock-plugin's surface) never disagree about where the prompt
/// lives. See this module's top doc comment for why there is no matching
/// `--indicator-x-position`.
///
/// `reference` should be [`smallest_output_size`] when more than one output
/// is locked at once. Concretely, for a landscape output `prompt_zone` is
/// `0.30 * height` tall, centred at `0.60 * height` (i.e. spans `[0.45,
/// 0.75] * height`), so a value taken from the smallest output's own centre
/// still falls inside a taller output's own span provided `taller.height <=
/// (4 / 3) * smallest.height` (solve `0.60 * Hmin >= 0.45 * Hmax`) — exactly
/// the ratio between, say, a 1080p and a 1440p panel. A larger mismatch
/// (1080p next to 4K, say) can still miss the taller output's zone: the same
/// "no free lunch" limitation this module's top doc comment already
/// documents for Y, restated here in terms of `prompt_zone` instead of a
/// flat percentage.
fn indicator_y_position(reference: Size) -> i32 {
    prompt_zone(reference).center().y.round() as i32
}

/// The `--indicator-radius` value for `reference`'s own output size: the
/// largest radius — capped at the usual [`INDICATOR_RADIUS`] — whose ring,
/// *including* half of [`INDICATOR_THICKNESS`] (swaylock draws the stroke
/// centred *on* the radius, so the visible ring's outer edge sits at the
/// radius plus half the thickness), still fits inside `reference`'s own
/// `prompt_zone`. Checked against both the zone's width and height, since a
/// narrow (portrait, or simply small) output can constrain either one.
///
/// Unlike [`indicator_y_position`], a wrong radius here is purely cosmetic —
/// an over-large ring spills past the zone without breaking anything — but
/// there is no reason to let it when the zone itself is right here to check
/// against. [`MIN_INDICATOR_RADIUS`] keeps a pathologically tiny/synthetic
/// output from shrinking the ring to nothing.
fn indicator_radius(reference: Size) -> u32 {
    let zone = prompt_zone(reference);
    let half_thickness = INDICATOR_THICKNESS as f32 / 2.0;
    let fits = ((zone.w.min(zone.h) / 2.0 - half_thickness).floor() as i64)
        .clamp(MIN_INDICATOR_RADIUS as i64, INDICATOR_RADIUS as i64);
    fits as u32
}

// ── spawn + bounded wait ─────────────────────────────────────────────────────

/// Wait up to `timeout` for `child` (swaylock-plugin's foreground process)
/// to exit, draining its stdout/stderr concurrently so its own logging can
/// never fill a pipe and deadlock this wait — a real risk here specifically
/// because, unlike `loginctl lock-session`, swaylock-plugin keeps running
/// its full Wayland event loop (and can keep logging) for as long as it
/// takes the compositor to confirm the lock, not just for a single instant
/// D-Bus call. Exit 0 is the proof the session is locked (see this module's
/// top doc comment); anything else — non-zero, or no exit before the
/// deadline — is `Err` with whatever stderr was captured.
fn wait_for_lock_confirmation(mut child: Child, timeout: Duration) -> Result<(), String> {
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let stderr_thread = child.stderr.take().map(|mut pipe| {
        let buf = Arc::clone(&stderr_buf);
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = pipe.read_to_string(&mut s);
            if let Ok(mut guard) = buf.lock() {
                *guard = s;
            }
        })
    });
    // stdout is not diagnostic here (mirrors `loginctl_lock_session`'s
    // stderr-only reporting) but must still be drained for the same
    // pipe-buffer reason as stderr.
    if let Some(mut out) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = out.read_to_end(&mut sink);
        });
    }

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(t) = stderr_thread {
                    let _ = t.join();
                }
                return if status.success() {
                    Ok(())
                } else {
                    let stderr = stderr_buf.lock().map(|s| s.clone()).unwrap_or_default();
                    Err(format!(
                        "swaylock-plugin exited with {status}: {}",
                        stderr.trim()
                    ))
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "swaylock-plugin did not confirm a lock within {timeout:?}"
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(format!("failed to wait on swaylock-plugin: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LockScreen, Wallpaper};

    // -- resolve_swaylock_plugin: binary lookup, injected PATH/fs -----------

    #[test]
    fn resolve_swaylock_plugin_env_override_always_wins() {
        // Matches FRESCO_MPVPAPER's contract: trusted even if `is_file` says
        // it doesn't exist, and even with a PATH hit available too.
        let got = resolve_swaylock_plugin(Some("/custom/slp"), Some("/usr/bin"), &|_| true);
        assert_eq!(got, Some(PathBuf::from("/custom/slp")));

        let got = resolve_swaylock_plugin(Some("/missing/slp"), Some("/usr/bin"), &|_| false);
        assert_eq!(got, Some(PathBuf::from("/missing/slp")));
    }

    #[test]
    fn resolve_swaylock_plugin_falls_back_to_path_then_bundled() {
        let on_path = |p: &Path| p == Path::new("/usr/bin/swaylock-plugin");
        let got = resolve_swaylock_plugin(None, Some("/opt/bin:/usr/bin"), &on_path);
        assert_eq!(got, Some(PathBuf::from("/usr/bin/swaylock-plugin")));

        let only_bundled = |p: &Path| p == Path::new(BUNDLED_SWAYLOCK_PLUGIN);
        let got = resolve_swaylock_plugin(None, Some("/opt/bin:/usr/bin"), &only_bundled);
        assert_eq!(got, Some(PathBuf::from(BUNDLED_SWAYLOCK_PLUGIN)));
    }

    #[test]
    fn resolve_swaylock_plugin_none_when_nothing_matches() {
        assert_eq!(
            resolve_swaylock_plugin(None, Some("/opt/bin"), &|_| false),
            None
        );
        assert_eq!(resolve_swaylock_plugin(None, None, &|_| false), None);
    }

    // -- pam_service_present: PAM gate, injected fs (temp dir) --------------

    #[test]
    fn pam_service_present_true_when_file_exists_in_any_root() {
        let dir = std::env::temp_dir().join(format!(
            "fresco-wlroots-pam-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(PAM_SERVICE), "auth include login\n").unwrap();

        let other = dir.join("does-not-exist-root");
        let roots = [other.as_path(), dir.as_path()];
        assert!(pam_service_present(&roots, &|p| p.is_file()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pam_service_present_false_when_absent_everywhere() {
        let dir = std::env::temp_dir().join(format!(
            "fresco-wlroots-pam-test-absent-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let roots = [dir.as_path()];
        assert!(!pam_service_present(&roots, &|p| p.is_file()));
        std::fs::remove_dir_all(&dir).ok();
    }

    // -- availability ---------------------------------------------------------

    #[test]
    fn availability_reasons() {
        assert_eq!(availability(None, true), Availability::MissingBinary);
        assert_eq!(availability(None, false), Availability::MissingBinary);
        assert_eq!(
            availability(Some(Path::new("/bin/x")), false),
            Availability::MissingPam
        );
        assert_eq!(
            availability(Some(Path::new("/bin/x")), true),
            Availability::Ready
        );
    }

    // -- smallest_output_size / indicator_y_position / indicator_radius --------
    //
    // The fixed numbers below are `crate::widgetkit::lockscene::prompt_zone`'s
    // own formula (`h = 0.30 * min(w, h)`, centred at `0.60 * h`) evaluated by
    // hand for each output size — a deliberate re-derivation, not a mirror of
    // `indicator_y_position`'s/`indicator_radius`'s own arithmetic, so a bug in
    // either one would actually be caught here.

    #[test]
    fn smallest_output_size_is_none_for_an_empty_list() {
        assert_eq!(smallest_output_size(&[]), None);
    }

    #[test]
    fn smallest_output_size_picks_by_height_not_by_position_or_width() {
        let outputs = [
            OutputGeom {
                connector: "HDMI-1".to_string(),
                w: 2560,
                h: 1440,
                scale_milli: 1000,
            },
            OutputGeom {
                connector: "DP-1".to_string(),
                w: 1920,
                h: 1080,
                scale_milli: 1000,
            },
        ];
        // The 1080p output is listed *second* and is *narrower*; only its
        // smaller height should win.
        assert_eq!(
            smallest_output_size(&outputs),
            Some(Size::new(1920.0, 1080.0))
        );
    }

    #[test]
    fn indicator_y_position_and_radius_for_a_single_output() {
        let size = Size::new(1920.0, 1080.0);
        // prompt_zone(1920x1080) is centred at 648 (0.60 * 1080, unclamped)
        // — not the old placeholder 62%, which would have rounded to 670.
        assert_eq!(indicator_y_position(size), 648);
        assert_eq!(indicator_radius(size), 90); // unchanged from the flat cap
    }

    #[test]
    fn indicator_y_position_from_the_smallest_output_still_lands_in_a_taller_outputs_zone() {
        // 1080p next to 1440p is exactly the 4:3 height ratio
        // `indicator_y_position`'s own doc comment works out as the boundary
        // this technique still covers.
        let outputs = [
            OutputGeom {
                connector: "DP-1".to_string(),
                w: 1920,
                h: 1080,
                scale_milli: 1000,
            },
            OutputGeom {
                connector: "HDMI-1".to_string(),
                w: 2560,
                h: 1440,
                scale_milli: 1000,
            },
        ];
        let reference = smallest_output_size(&outputs).expect("non-empty outputs");
        assert_eq!(reference, Size::new(1920.0, 1080.0));

        let y = indicator_y_position(reference);
        assert_eq!(y, 648);

        // The larger output's own zone is y in [648, 1080] (device pixels on
        // *that* output's own surface) — 648 lands exactly on its top edge,
        // still "inside" under an inclusive bound.
        let large_zone = prompt_zone(Size::new(2560.0, 1440.0));
        assert!(
            (y as f32) >= large_zone.y - 0.5 && (y as f32) <= large_zone.bottom() + 0.5,
            "y={y} not inside the larger output's own zone {large_zone:?}"
        );
    }

    #[test]
    fn indicator_y_position_and_radius_for_a_portrait_output() {
        let size = Size::new(1080.0, 1920.0);
        assert_eq!(indicator_y_position(size), 1152);
        assert_eq!(indicator_radius(size), 90);
        // Sanity: the ring must land on screen, not past either edge.
        let y = indicator_y_position(size);
        assert!(y > 0 && (y as f32) < size.h);
    }

    #[test]
    fn indicator_radius_shrinks_to_fit_a_tiny_output() {
        let size = Size::new(640.0, 480.0);
        assert_eq!(indicator_y_position(size), 288);
        assert_eq!(indicator_radius(size), 68); // shrunk well below the cap
        assert!(indicator_radius(size) < INDICATOR_RADIUS);
        assert!(indicator_radius(size) >= MIN_INDICATOR_RADIUS);
    }

    #[test]
    fn indicator_position_and_radius_always_land_inside_their_own_reference_zone() {
        // A broad regression guard, independent of the hand-picked cases
        // above: for a single reference output, the ring's position, and its
        // radius plus half the fixed thickness, must always land inside
        // *that same output's* own `prompt_zone`, on both axes.
        for &(w, h) in &[
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3840.0, 2160.0),
            (1080.0, 1920.0), // portrait
            (640.0, 480.0),   // tiny
            (800.0, 600.0),
        ] {
            let size = Size::new(w, h);
            let zone = prompt_zone(size);

            let y = indicator_y_position(size) as f32;
            assert!(
                y >= zone.y - 0.5 && y <= zone.bottom() + 0.5,
                "{w}x{h}: y={y} outside its own zone {zone:?}"
            );

            let half_thickness = INDICATOR_THICKNESS as f32 / 2.0;
            let outer = indicator_radius(size) as f32 + half_thickness;
            assert!(
                outer <= zone.w / 2.0 + 0.5,
                "{w}x{h}: radius {outer} overflows the zone's width {}",
                zone.w
            );
            assert!(
                outer <= zone.h / 2.0 + 0.5,
                "{w}x{h}: radius {outer} overflows the zone's height {}",
                zone.h
            );
        }
    }

    // -- hex_rrggbbaa -----------------------------------------------------------

    #[test]
    fn hex_rrggbbaa_formats_straight_alpha_hex() {
        assert_eq!(hex_rrggbbaa(Color::WHITE), "#FFFFFFFF");
        assert_eq!(hex_rrggbbaa(Color::BLACK), "#000000FF");
        assert_eq!(hex_rrggbbaa(Color::TRANSPARENT), "#00000000");
        assert_eq!(
            hex_rrggbbaa(Color::rgba8(0x12, 0x34, 0x56, 0.5)),
            "#12345680"
        );
    }

    // -- sanitize_connector / lock_socket_path -----------------------------------

    #[test]
    fn sanitize_connector_keeps_alnum_dash_underscore_only() {
        assert_eq!(sanitize_connector("DP-1"), "DP-1");
        assert_eq!(sanitize_connector("HDMI-A-1"), "HDMI-A-1");
        assert_eq!(
            sanitize_connector("weird name/with:chars"),
            "weird_name_with_chars"
        );
    }

    #[test]
    fn lock_socket_path_is_per_connector() {
        let dir = Path::new("/run/user/1000/fresco");
        assert_eq!(
            lock_socket_path(dir, "DP-1"),
            PathBuf::from("/run/user/1000/fresco/lock-DP-1.sock")
        );
        assert_ne!(
            lock_socket_path(dir, "DP-1"),
            lock_socket_path(dir, "HDMI-A-1")
        );
    }

    // -- sh_quote: spaces, quotes, unicode, and injection attempts --------------

    /// Round-trip `s` through a real `sh -c 'printf %s <quoted>'` and assert
    /// the shell hands it back byte-for-byte unchanged — the strongest proof
    /// available that [`sh_quote`] is safe for whatever swaylock-plugin's own
    /// `sh -c` will do with it (see this module's top doc comment for why
    /// that is the real boundary, not `Command`'s argv splitting).
    fn assert_sh_round_trips(s: &str) {
        let quoted = sh_quote(s);
        let script = format!("printf %s {quoted}");
        let out = Command::new("/bin/sh")
            .arg("-c")
            .arg(&script)
            .env("PATH", "/usr/bin:/bin")
            .output()
            .expect("sh must be available to run this test");
        assert!(
            out.status.success(),
            "sh -c {script:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            s,
            "round-trip mismatch for {s:?} (quoted as {quoted:?})"
        );
    }

    #[test]
    fn sh_quote_known_format() {
        assert_eq!(sh_quote("abc"), "'abc'");
        assert_eq!(sh_quote("a'b"), "'a'\\''b'");
        assert_eq!(sh_quote(""), "''");
    }

    #[test]
    fn sh_quote_round_trips_through_a_real_shell() {
        for s in [
            "simple",
            "has space",
            "trailing space ",
            "it's got a quote",
            "''double single quotes''",
            "unicode 世界 \u{1F389}", // CJK + emoji
            "$(rm -rf /)",
            "`echo pwned`",
            "a\"b\\c",
            "$HOME and ${PATH}",
            "newline\nin\npath", // pathological but must not break quoting
            "-flag-like-string",
        ] {
            assert_sh_round_trips(s);
        }
    }

    // -- mpvpaper_shell_line -----------------------------------------------------

    #[test]
    fn mpvpaper_shell_line_shape() {
        let line = mpvpaper_shell_line(
            Path::new("/usr/lib/fresco/mpvpaper"),
            "input-ipc-server=/run/x.sock hwdec=auto-safe",
            Path::new("/home/user/video.mp4"),
        );
        assert!(line.starts_with("exec "));
        // The connector is a live shell variable reference, not a quoted
        // literal — see this function's own doc comment for why.
        assert!(line.contains("\"$SWAYLOCK_PLUGIN_OUTPUT_NAME\""));
        assert!(line.contains("'/usr/lib/fresco/mpvpaper'"));
        assert!(line.contains("'input-ipc-server=/run/x.sock hwdec=auto-safe'"));
        assert!(line.contains("'/home/user/video.mp4'"));
        // The dedicated case-dispatch test below proves the env var actually
        // reaches the right place at run time, using `echo` as a stand-in
        // program (no real mpvpaper binary is available in the test sandbox).
    }

    // -- plugin_command_script: case-dispatch through a real shell --------------

    #[test]
    fn plugin_command_script_dispatches_by_output_name_through_a_real_shell() {
        let arms = vec![
            ("DP-1".to_string(), "echo first".to_string()),
            ("weird output: name".to_string(), "echo second".to_string()),
        ];
        let script = plugin_command_script(&arms, Some("echo default"));

        let run = |output_name: &str| -> String {
            let out = Command::new("/bin/sh")
                .arg("-c")
                .arg(&script)
                .env("PATH", "/usr/bin:/bin")
                .env("SWAYLOCK_PLUGIN_OUTPUT_NAME", output_name)
                .output()
                .expect("sh must be available to run this test");
            assert!(out.status.success(), "script failed for {output_name:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        assert_eq!(run("DP-1"), "first");
        assert_eq!(run("weird output: name"), "second");
        assert_eq!(run("some-unknown-output"), "default");
    }

    #[test]
    fn plugin_command_script_without_default_exits_cleanly_on_unknown_output() {
        let arms = vec![("DP-1".to_string(), "echo only".to_string())];
        let script = plugin_command_script(&arms, None);
        let out = Command::new("/bin/sh")
            .arg("-c")
            .arg(&script)
            .env("PATH", "/usr/bin:/bin")
            .env("SWAYLOCK_PLUGIN_OUTPUT_NAME", "HDMI-1")
            .output()
            .expect("sh must be available to run this test");
        assert!(out.status.success());
        assert!(out.stdout.is_empty());
    }

    // -- widget_mode --------------------------------------------------------------

    #[test]
    fn widget_mode_matches_daemon_widgets_widget_mode() {
        assert_eq!(widget_mode(WidgetTheme::Auto), Mode::Dark);
        assert_eq!(widget_mode(WidgetTheme::Dark), Mode::Dark);
        assert_eq!(widget_mode(WidgetTheme::Light), Mode::Light);
    }

    // -- wait_for_lock_confirmation: bounded wait, using real `sh -c` children ---

    fn spawn_sh(script: &str) -> Child {
        // Absolute shell + pinned PATH: other tests temporarily repoint the
        // process-wide PATH, which would make `sleep` vanish under this child.
        Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sh must be available to run this test")
    }

    #[test]
    fn wait_for_lock_confirmation_ok_on_exit_zero() {
        let child = spawn_sh("exit 0");
        assert!(wait_for_lock_confirmation(child, Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn wait_for_lock_confirmation_err_on_nonzero_exit_with_stderr() {
        let child = spawn_sh("echo boom >&2; exit 7");
        let err = wait_for_lock_confirmation(child, Duration::from_secs(5)).unwrap_err();
        assert!(err.contains("boom"), "{err}");
    }

    #[test]
    fn wait_for_lock_confirmation_err_on_timeout() {
        let child = spawn_sh("sleep 5");
        let started = Instant::now();
        let err = wait_for_lock_confirmation(child, Duration::from_millis(150)).unwrap_err();
        assert!(err.contains("did not confirm"), "{err}");
        // Bounded: must not have waited anywhere near the child's `sleep 5`.
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn wait_for_lock_confirmation_does_not_deadlock_on_chatty_output() {
        // Regression guard for the exact risk documented on this function:
        // enough output to exceed a pipe buffer, produced *before* exit,
        // must not hang the wait.
        let child = spawn_sh("yes boom | head -c 200000; exit 0");
        assert!(wait_for_lock_confirmation(child, Duration::from_secs(5)).is_ok());
    }

    // -- build_plugin_command: end-to-end wiring, no real mpvpaper needed --------

    #[test]
    fn build_plugin_command_skips_outputs_with_no_media_and_reports_sockets() {
        let config = Config {
            wallpaper: Wallpaper {
                path: Some(PathBuf::from("/wall/default.mp4")),
                ..Wallpaper::default()
            },
            lockscreen: Some(LockScreen::default()),
            ..Config::default()
        };

        let outputs = vec![
            OutputGeom {
                connector: "DP-1".to_string(),
                w: 1920,
                h: 1080,
                scale_milli: 1000,
            },
            OutputGeom {
                connector: "HDMI-1".to_string(),
                w: 2560,
                h: 1440,
                scale_milli: 1000,
            },
        ];
        let runtime_dir = Path::new("/run/user/1000/fresco");
        let mpv_bin = Path::new("/usr/lib/fresco/mpvpaper");

        let (script, sockets) = build_plugin_command(&config, &outputs, runtime_dir, mpv_bin, true);

        assert_eq!(sockets.len(), 2);
        assert!(sockets
            .iter()
            .any(|s| s.connector == "DP-1" && s.path == "/run/user/1000/fresco/lock-DP-1.sock"));
        assert!(
            sockets
                .iter()
                .any(|s| s.connector == "HDMI-1"
                    && s.path == "/run/user/1000/fresco/lock-HDMI-1.sock")
        );
        assert!(script.contains("DP-1"));
        assert!(script.contains("HDMI-1"));
        assert!(!script.contains("pause=yes"), "playing live must not pause");
    }

    #[test]
    fn build_plugin_command_pauses_when_not_playing_live() {
        let config = Config {
            wallpaper: Wallpaper {
                path: Some(PathBuf::from("/wall/default.mp4")),
                ..Wallpaper::default()
            },
            ..Config::default()
        };
        let outputs = vec![OutputGeom {
            connector: "DP-1".to_string(),
            w: 1920,
            h: 1080,
            scale_milli: 1000,
        }];
        let (script, sockets) = build_plugin_command(
            &config,
            &outputs,
            Path::new("/run/user/1000/fresco"),
            Path::new("/usr/lib/fresco/mpvpaper"),
            false,
        );
        assert_eq!(sockets.len(), 1);
        assert!(script.contains("pause=yes"));
    }

    #[test]
    fn build_plugin_command_empty_when_nothing_has_media() {
        let config = Config::default(); // default wallpaper has no path/paths
        let outputs = vec![OutputGeom {
            connector: "DP-1".to_string(),
            w: 1920,
            h: 1080,
            scale_milli: 1000,
        }];
        let (_script, sockets) = build_plugin_command(
            &config,
            &outputs,
            Path::new("/run/user/1000/fresco"),
            Path::new("/usr/lib/fresco/mpvpaper"),
            true,
        );
        assert!(sockets.is_empty());
    }

    // -- WlrootsHost trait surface ------------------------------------------------

    #[test]
    fn kind_is_wlroots() {
        assert_eq!(WlrootsHost.kind(), HostKind::Wlroots);
    }

    #[test]
    fn targets_while_locked_is_always_none() {
        let config = Config::default();
        let ctx = HostCtx {
            config: &config,
            outputs: &[],
            runtime_dir: PathBuf::from("/run/user/1000/fresco"),
        };
        assert_eq!(WlrootsHost.targets_while_locked(&ctx), LockTargets::None);
    }
}

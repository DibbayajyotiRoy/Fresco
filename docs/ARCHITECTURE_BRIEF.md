# Fresco — Architecture Brief

*How a solo developer built a self-healing live-wallpaper engine for Linux in three weeks of Rust.*

This document is the source material for a video / article / Reddit posts about how Fresco is
actually built. Every claim below is grounded in the code — file paths are given so you can show
them on screen.

---

## 1. The elevator pitch

Windows has Wallpaper Engine. Linux had a pile of terminal tools, abandoned GNOME extensions, and
compositor-specific hacks. **Fresco** is a proper desktop app — install a `.deb`, pick a video,
click *Set*, close the app — and the wallpaper keeps playing, survives reboots, pauses when you go
fullscreen or unplug the charger, and heals itself when the GPU driver or audio server misbehaves.

**The numbers:**

| Fact | Value |
|---|---|
| Language / stack | Rust (edition 2021), GTK4 + libadwaita, libmpv, Wayland + X11 |
| Size | ~12,600 lines of Rust across `src/` |
| Timeline | **31 commits, June 12 → July 4, 2026 — a ~3-week solo sprint from scaffold to 1.0.1** |
| Binaries | 2 from one crate: `fresco` (GUI) and `frescod` (daemon) |
| Tests | ~40+ Rust `#[test]`s + 5 shell verification harnesses (README counts 73 automated checks total) |
| CI | 5 headless compositors + 7 distro containers, weekly |
| Measured quality | SSIM 0.74 vs 0.54 default on 8K→4K downscale; 256/256 luma levels (no banding); pixel-exact HiDPI |
| License | GPL-3.0 |

---

## 2. High-level architecture

One Cargo crate, two binaries, feature-gated so each only compiles what it needs
(`Cargo.toml` `[features]`):

```
                ┌──────────────────────────────────────────┐
                │              shared core                  │
                │  config.rs (TOML, atomic writes)          │
                │  ipc.rs (Unix socket, JSON protocol)      │
                │  schedule.rs (pure fns, NOAA solar math)  │
                │  catalog.rs / supabase.rs / download.rs   │
                │  capability.rs (session detection)        │
                └───────────▲──────────────▲───────────────┘
                            │              │
        ┌───────────────────┴──┐      ┌────┴──────────────────────┐
        │  fresco (GUI)        │      │  frescod (daemon)          │
        │  GTK4 + libadwaita   │ IPC  │  no GTK, no tokio          │
        │  library, editor,    │─────▶│  3 backends:               │
        │  catalog browser,    │ JSON │   • X11: embedded libmpv   │
        │  schedules, updates  │ sock │   • Wayland: mpvpaper      │
        └──────────────────────┘      │   • GNOME: static gsettings│
                                      └────────────────────────────┘
```

- The **GUI is optional**. It saves config and sends one JSON command over a Unix socket. The
  daemon is the product; the GUI is a remote control.
- The daemon is **deliberately not async** — no tokio, no async runtime. Two hand-written
  single-threaded loops plus a couple of helper threads. At idle it wakes ~10×/second, compares a
  few fields, and sleeps (`src/daemon/mod.rs`). GNOME-static mode literally blocks on
  `recv()` — 0% CPU between commands.

---

## 3. The rendering problem: three backends behind one interface

The core difficulty of live wallpapers on Linux is that "the desktop background" means three
different things depending on your session. Fresco probes at startup
(`src/capability.rs`) — and it doesn't trust `XDG_CURRENT_DESKTOP`; it opens a real Wayland
connection and round-trips the registry to check whether `zwlr_layer_shell_v1` actually exists.

### Backend 1 — X11: mpv embedded in a fake desktop window

- **libmpv is dlopen'd at runtime, not linked** (`src/daemon/mpv/ffi.rs`, 168 lines). It tries
  `libmpv.so.2` → `.so.1` → `.so` and binds only ~11 symbols whose signatures are identical
  across mpv ABI 1 and 2 — so one binary runs on Ubuntu 22.04 (libmpv1) *and* Debian 12 / Ubuntu
  24.04 (libmpv2). It never touches the unstable `mpv_render_*` API; embedding is via the classic
  `wid` option.
- **The window** (`src/daemon/x11win.rs`): one per monitor, typed
  `_NET_WM_WINDOW_TYPE_DESKTOP`, states BELOW + STICKY + SKIP_TASKBAR + SKIP_PAGER, input
  hints off — and the killer detail: an **empty input SHAPE region**, so every click passes
  straight through the wallpaper to your real desktop icons.
- **The cold-boot bug**: on login, mpv's display-synced output stalls forever if the window isn't
  paint-ready when playback starts. Fix: `wait_until_viewable` polls window attributes up to ~3s
  before embedding (`x11win.rs:175`). Plus a belt-and-braces self-heal (see §5).
- **The ConfigureNotify storm** — a hard-won lesson documented right in the main loop
  (`src/daemon/mod.rs:373`): re-lowering the window in response to X events emits a
  `ConfigureNotify` on its own window, which re-enters, which storms the compositor and **froze
  the laptop**. So the daemon drains-and-discards all X events and re-lowers strictly on a 2s
  timer. Fullscreen detection is likewise polled, never event-driven.
- **GNOME overview trick** (`src/daemon/overview.rs`): the wallpaper window is invisible in
  GNOME's Activities overview and lock screen, so the daemon extracts a still frame and sets it as
  the `org.gnome.desktop.background` — saving and restoring the user's original image.

### Backend 2 — Wayland: one bundled mpvpaper per output, driven over IPC

You cannot embed a foreign window in a layer-shell surface, so Fresco spawns the external
renderer **mpvpaper** (layer-shell + EGL + libmpv), one process per monitor, and steers each
through mpv's JSON IPC socket at `$XDG_RUNTIME_DIR/fresco/mpv-<connector>.sock`
(`src/daemon/mpvpaper.rs`, 607 lines).

- **mpvpaper is not packaged in any apt repo**, so the build clones it pinned at v1.4, builds it
  with meson, and ships it inside the .deb at `/usr/lib/fresco/mpvpaper` — deliberately *not*
  `/usr/bin`, so it can never collide with a user-installed copy (`scripts/build-mpvpaper.sh`,
  `Cargo.toml` deb assets).
- The Wayland player exposes **the exact same `&self` API as the X11 player**, unified behind a
  `PlayerHandle` enum — so slideshows, transitions, pause logic, and audio healing are each
  written *once* and run on both backends with zero call-site branching.
- Fun trap: you can't pass `background=#000000` to mpvpaper, because it forwards options through
  an mpv config file where `#` starts a comment (`mpvpaper.rs:289`).

### Backend 3 — GNOME Wayland: honest fallback

GNOME implements neither layer-shell nor foreign-toplevel protocols. Instead of pretending,
Fresco extracts a static frame and sets it via gsettings, tells the user, and idles at literally
0% CPU (blocking `recv()` loop).

---

## 4. Efficiency: how "CPU near zero" actually works

1. **Hardware decode** (VA-API / NVDEC) with a self-heal: on hybrid Intel+NVIDIA laptops, libva
   probes the NVIDIA render node, fails, and mpv silently falls back to software decode. Fresco
   detects the Intel GPU and pins `LIBVA_DRIVER_NAME=iHD` (`src/daemon/mod.rs:1073`).
2. **Coarse timers, no busy loops**: 100ms tick normally, 16ms only while a slideshow transition
   is animating; battery every 30s; fullscreen every 2s.
3. **Pause when nobody's watching**:
   - X11: polls EWMH (`_NET_CLIENT_LIST_STACKING` + `_NET_WM_STATE`) and pauses when a fullscreen
     window covers **≥50% of a monitor** (`src/daemon/x11_fullscreen.rs`).
   - Wayland: `wlr-foreign-toplevel-management` tracks fullscreen state per output
     (`src/daemon/fullscreen.rs`). Key insight in the comments: the compositor already stops
     *rendering* an occluded surface, but **mpv keeps decoding** — pausing saves real CPU.
   - Battery: scans `/sys/class/power_supply/*/status` for `Discharging`.
   - All three pause sources fold through one function (`reconcile_pause`) with a change-detection
     cell, so they never fight over mpv's pause property.
4. **Muted wallpapers load with `aid=no`** — no audio decoder, buffers, or thread at all — plus
   `video-sync=display-resample` for smooth looping without an audio clock.
5. **Adaptive demuxer cache**: spawns at 16 MiB for low RSS on 1080p loops, and one-shot raises to
   64 MiB when a ≥4K source is detected so 50 Mbps files don't stutter.
6. **Honest reporting**: `fresco status` shows *real* CPU (diffed `/proc/self/stat` ticks including
   child mpvpaper processes), RSS, source resolution, bit depth, and dropped frames — and warns
   explicitly when your GPU can't hardware-decode the file instead of letting it stutter silently.

---

## 5. The self-healing daemon (the best story in the codebase)

Every failure mode here was a real bug first:

| Failure | Heal | Where |
|---|---|---|
| Wallpaper frozen after reboot (mpv VO stall) | For the first 60s after login, sample `time-pos` every 3s; two identical readings on an unpaused video ⇒ rebuild the renderer, up to 5 times | `src/daemon/mod.rs:656` |
| **Silent wallpaper after cold boot** — daemon starts before PipeWire, mpv *permanently* drops the audio track, and `aid=auto` does NOT bring it back (an explicit track id does) | Retry with exponential backoff at ~5/10/20/40/80/160s | `AudioHeal`, `src/daemon/mod.rs:118` |
| mpvpaper crashes (GL context died after a driver update) | Respawn, max 5 restarts; past the cap, spawn one **paused static frame** so the screen never goes black — "Fresco never paints black itself" | `supervise`, `src/daemon/mod.rs:1665` |
| mpvpaper alive but frozen | 3 consecutive 2s ticks with no `playback-time` progress ⇒ treated as wedged, respawned | same |
| Autostart entry deleted / prior run killed | Rewrites its own `.desktop` entry on start; restores the GNOME background it saved | `mod.rs`, `overview.rs` |

The audio one is the flagship: the test suite contains a **cold-boot repro harness**
(`tests/audio/verify-audio.sh`, leg L3) that starts the daemon inside a private runtime dir with
*no audio server*, then symlinks the real PipeWire sockets in a moment later — simulating "PipeWire
comes up after login." The bug was encoded in CI as `[REPRO-CONFIRMED]` *before* the fix existed,
then flipped to a hard gate.

---

## 6. Video quality, measured not promised

Fresco ships its proof harness in-tree (`tests/fidelity/verify-fidelity.sh`). It plays synthetic
torture patterns through the **real daemon** on a headless compositor, screenshots the composited
output (`grim` on Wayland, ImageMagick on X11), and scores:

- **Crispness**: a near-lossless 1-pixel checkerboard, scored as RMSE between the capture and
  itself shifted 1px — a perfect checkerboard is maximally anti-correlated, any interpolation blur
  collapses the score. Result: **100/100, pixel-exact at HiDPI scale 1× and 2×.**
- **Banding**: an 8-bit smooth gradient; count unique luma levels in the center band.
  **256/256** post-1.0 (was 220 before dithering was enabled on every profile).
- **Downscale quality**: an 8K zone plate (a `geq` sinusoid designed to alias) downscaled to 4K,
  scored with ffmpeg's SSIM filter against a Lanczos reference. **0.74 with Fresco's scaler stack
  vs 0.54 with the defaults most players use.**
- All metrics are range-tolerant so they survive software rendering (llvmpipe/pixman) in CI, where
  exact pixels are nondeterministic.

A quality war story worth showing: **the green-cast bug** (`src/daemon/mpv/player.rs:99`) — a
custom chroma scaler combined with `video-rotate` corrupts chroma planes green (the harness
measured RGB 90,142,64 where neutral gray 126,129,127 should be), so `cscale` is skipped on
rotated video, and rotation forces `hwdec=auto-copy` because native hwdec surfaces + rotation
corrupt chroma on some driver stacks.

Test fixtures are generated offline in under two minutes by `tests/assets/make-fixtures.sh`
(1080p/4K/8K H.264 + VP9, a 440 Hz sine audio clip, the checkerboard, the zone plate, 8- and
10-bit gradients, a deliberately truncated `broken.mp4`), self-verified by ffprobe assertions.

---

## 7. The GUI (~5,200 lines of GTK4)

- **Stack**: GTK4 (4.6 floor) + libadwaita 1.1 — old enough floors that `AdwBreakpoint` doesn't
  exist, so responsive layout is hand-rolled: a `LayoutBucket` (Compact/Regular/Wide) resolved
  from window width drives FlowBox column caps, margins, and CSS classes (`src/gui/window.rs`,
  2,882 lines).
- **Threading discipline**: one idiom, repeated everywhere — blocking work (IPC, HTTP, downloads,
  the updater subprocess) on `std::thread::spawn`, results back via `async_channel` +
  `glib::spawn_future_local`. The GTK main thread never blocks.
- **Zero-copy crop** (`src/config.rs`, `src/gui/preview.rs`): the crop editor stores a normalized
  rect and converts it to mpv's VO-side `video-zoom = log2(1/w)` + pan — **never `vf=crop`**,
  which would break hardware-decode zero-copy. The math has its own unit test. The editor itself
  is a Cairo-drawn overlay with rule-of-thirds guides and corner handles, aspect-locked to the
  monitor.
- **Hover-to-play cards** (`src/gui/hover_preview.rs`): swaps only the `Picture`'s paintable
  (thumbnail texture ↔ live `MediaFile`) so overlays never re-layout; a 140ms leave-grace debounce
  kills flicker when the pointer crosses the revealed Edit button; the live preview only swaps in
  after the first decoded frame, otherwise a missing GStreamer plugin would blank the card forever.
- **Thumbnails**: `ffmpegthumbnailer` normally — but it can't rotate, so rotated entries go
  through ffmpeg with `transpose` filters instead (`src/gui/library.rs`).
- **Theming** (`src/gui/theme.rs`): runtime-rebuilt CSS provider; "obsidian" dark (never
  pitch-black) and "paper" light (never pure-white), six locked accents. GTK4 CSS is a *subset* of
  web CSS — no `transform`, `filter`, `var()`, or `calc()` — and there's no backdrop-blur, so
  "glass" modals fake it with high-opacity smoked panels.
- **Library storage**: `~/.local/share/fresco/library/entries.json`, atomic write-then-rename;
  broken entries (source file deleted) get a MISSING badge and a Relink action instead of
  vanishing.

---

## 8. The shared brain

- **Config** (`src/config.rs`): TOML at `~/.config/fresco/config.toml`, atomic writes, versioned
  for migrations. Per-wallpaper: fit, rotation, crop, mute, volume, slideshow interval and
  transition. Per-monitor overrides keyed by connector name.
- **Schedule engine** (`src/schedule.rs`, 380 lines): **pure functions, zero I/O** — designed
  explicitly for testability (and a future macOS port). Three modes: day/night pairs, arbitrary
  time slots (with correct wrap past midnight), and sunrise/sunset via the **full NOAA solar
  position algorithm implemented inline, dependency-free** — fractional year, equation of time,
  declination, hour angle at the 90.833° official-sunrise zenith. Tested to ±2 minutes against
  published NOAA values (Greenwich solstices, NYC, Svalbard polar night). Location is manual
  lat/lon — no geoclue, for privacy and dependency weight. The daemon evaluates it on its tick;
  DST is transparent because only the wall clock is ever consulted (with EU spring-forward /
  fall-back tests).
- **IPC** (`src/ipc.rs`): newline-delimited JSON over a Unix socket. Six commands: `apply`,
  `stop`, `pause`, `resume`, `status`, `update`. The socket doubles as the single-instance lock.
  `StatusReply` is forward/backward compatible — every newer field is `#[serde(default)]`, with a
  dedicated test proving old daemon replies still parse. Documented for scripting
  (waybar toggles, workspace hooks) in `docs/SCRIPTING.md`.
- **Catalog** (`src/catalog.rs`, `src/gui/gallery.rs`): curated wallpapers hosted on Supabase,
  read via PostgREST with a shipped anon key (safe: Row-Level Security protects the data, not key
  secrecy). **License and author are displayed on every card** — treated as a launch requirement.
  Downloads are capped at 500 MB, cached offline. The only analytics is a fire-and-forget install
  counter with no client identifiers.
- **Downloads** (`src/download.rs`): Content-Length pre-check *and* a mid-stream cap ("servers
  lie"), atomic `.part`→rename, never overwrites, each property tested against an in-process HTTP
  server. Direct URLs only — explicitly no yt-dlp/YouTube, to protect Flathub/AUR standing.

---

## 9. Update system: push, not poll

The always-running daemon holds **one Supabase Realtime websocket** open
(`src/daemon/notifier.rs`, 400 lines) — Phoenix-channel protocol over `tungstenite`, 25s
heartbeats piggybacked on the socket read timeout, exponential backoff reconnect, catch-up read on
connect (Realtime never replays history). When a release row arrives:

1. Semver-gate against the running version.
2. Raise a native desktop notification with an **"Update now"** action.
3. Clicking it runs `pkexec /usr/lib/fresco/fresco-update.sh` — polkit prompts once, the script
   fetches the latest .deb from GitHub Releases, compares versions with
   `dpkg --compare-versions` (translating semver `-rc1` to Debian `~rc1` so pre-releases sort
   correctly), validates, and apt-installs. Structured exit codes (0 ok / 2 up-to-date / 3
   unsupported) drive the UI; Flatpak's read-only sandbox routes to the releases page instead.
4. The GUI streams the updater's `STAGE:` lines to a live label — and drains stderr on a separate
   thread, because an undrained pipe would deadlock apt.

Releasing is equally automated: **merging to main publishes a release only if `Cargo.toml`'s
version is newer than any existing tag** (`.github/workflows/publish.yml`). Routine merges run the
full gate and ship nothing — no update spam.

---

## 10. CI: a compositor zoo on stock GitHub runners

- **`tests/ci/with-compositor.sh`** boots, headless and GPU-less: Xvfb (X11), Sway and Hyprland
  (headless wlroots, pixman software rendering), KWin (`kwin_wayland --virtual` under
  `dbus-run-session`), and Weston (the deliberate no-layer-shell fallback case) — each in a
  private `XDG_RUNTIME_DIR` so CI can't touch a real desktop.
- The environment gate requires **≥3 of 5 compositors passing** — the robust trio (X11 / Sway /
  Weston) carries it; KDE and Hyprland add coverage when the runner cooperates. Flaky compositors
  can't block a release, but real regressions can't hide either.
- **`distros.yml`** runs weekly across Ubuntu 22.04/24.04, Debian 12, Mint 21/22, Pop!_OS 22.04,
  and elementary 7 — each container builds its own .deb (because of the libmpv soname split), then
  a *clean* container with no dev packages must apt-resolve every declared dependency, pass
  `ldd -r` on both binaries, and run `frescod --check`.
- Strict lane: rustfmt, clippy `-D warnings` across three feature combos, `cargo doc` with broken
  intra-doc links as errors, Rust pinned at 1.91.0, everything `--locked`.
- Release profile: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"` (documented as safer
  across the GTK/mpv FFI boundary), stripped.

### The honesty ledger

`docs/WAYLAND_VERIFICATION.md` is a commit-scoped evidence ledger: 22 numbered tests, each tagged
**PROVEN / UNPROVEN / "UNPROVEN — NOT IMPLEMENTED"**, with screenshots and pids as evidence
(the renderer-kill test literally records pid 224482 → 224837). The roadmap opens with a self-audit
calling out **"honesty debt"** — the README had claimed Hyprland/KDE support that wasn't proven on
screen, and the fix was to downgrade the claims to "experimental" in the same release. The README's
comparison table still marks them experimental today.

---

## 11. Development timeline (the video's narrative spine)

| Date (2026) | Milestone |
|---|---|
| Jun 12 | Scaffold: two-binary layout, config/IPC/autostart **with tests from day one** |
| Jun 13 | **v0.0.1 ships — a working GUI live-wallpaper app for X11 in one day**; v0.0.2 same day |
| Jun 14 | v0.0.3: theming, slideshows, VA-API hwdec; landing page added |
| Jun 18–19 | Flatpak/Flathub packaging groundwork |
| Jun 24–27 | The Wayland era: per-output fullscreen auto-pause, cold-boot freeze fix, restore-on-login and stall self-heal, rotation, per-entry audio |
| Jul 2 | v0.0.91: in-app updates, responsive UI, one-step install |
| Jul 3 | Formal ROADMAP + per-task proof tests; green baseline; fixture generator |
| Jul 3–4 | **1.0.0**: reliable audio, pixel-true 4K/8K, catalog, per-display wallpapers, schedules. 1.0.1 polish |

Arc: *ship on X11 in a day → harden across the compositor zoo → a disciplined, test-first push to
a real 1.0.*

---

## 12. Content package

### Video outline (10–14 min dev-story video)

1. **Cold open** — the problem: Wallpaper Engine exists, Linux has nothing simple. Demo Fresco in
   15 seconds (pick video → Set → close app → still playing).
2. **The architecture in one diagram** — two binaries, JSON socket, three rendering backends.
3. **The X11 hack tour** — the fake desktop window, the click-through input shape, the empty-event
   loop and the ConfigureNotify storm that froze a laptop.
4. **Wayland is a different planet** — why you can't embed anything, bundling mpvpaper because
   nobody packages it, one process per monitor driven over mpv's IPC.
5. **The self-healing segment** (strongest chapter) — cold-boot stall, the PipeWire audio race
   ("why is my wallpaper silent after reboot?"), crash respawn that never paints black.
6. **Measured, not promised** — show the zone plate, the checkerboard, SSIM 0.54 → 0.74, the
   green-cast bug screenshots.
7. **CI runs five compositors with no GPU** — show the workflow matrix.
8. **The honesty ledger** — PROVEN/UNPROVEN tags, "honesty debt." Rare and very likable.
9. **Close** — 3 weeks, ~12.6K lines, solo, GPL. Install one-liner on screen.

### Article angle

"**What it actually takes to put a video behind your desktop icons on Linux**" — structure the
article around the three backends and the self-healing table (§3–§5). Alternative angle for a
testing-focused outlet: "**Screenshot-testing video quality in CI with five headless
compositors**" (§6 + §10).

### Reddit drafts

**r/rust** — title: *"I built a live-wallpaper engine for Linux in Rust — three rendering
backends, a dlopen'd libmpv that runs on two ABIs, and a daemon that heals itself"*
Body: lead with the PlayerHandle unification (one engine, X11 in-process libmpv vs Wayland
out-of-process mpvpaper, same `&self` API), the runtime dlopen trick for libmpv1/2, no-tokio
hand-rolled event loops, and the pure-function NOAA schedule engine. Link repo, invite code review.

**r/linux** — title: *"Linux finally has a Wallpaper Engine alternative that isn't a terminal
script — free, GPL, X11 + Wayland, and it publishes its quality benchmarks"*
Body: user-facing framing — the comparison table, hardware decode near-0% CPU, pause on
fullscreen/battery, day-night schedules with real sunrise math, and "measured, not promised" with
the SSIM/banding numbers. Screenshot of the gallery.

**r/unixporn** (screenshot post) — a clean rice with a live 4K wallpaper + `fresco status` in a
terminal showing hardware decode and ~0% CPU. Title: *"[Sway] Live 8K wallpaper at 0.3% CPU —
built my own wallpaper engine in Rust"*.

### One-liners you can quote verbatim

- "The wallpaper window has an empty input shape — every click falls straight through to your
  desktop icons."
- "mpv permanently drops the audio track if PipeWire isn't up yet, and `aid=auto` will not bring
  it back. An explicit track id will. That one sentence cost a test harness."
- "Fresco never paints black itself — if the renderer crash-loops, it leaves one paused frame on
  screen and stops."
- "Crop is `video-zoom = log2(1/w)`, not `vf=crop` — because a video filter would break zero-copy
  hardware decode."
- "CI boots Sway, Hyprland, KWin, Weston, and Xvfb on a GitHub runner with no GPU, and a release
  needs three of five to pass."
- "The roadmap opens with a self-audit called 'honesty debt.'"

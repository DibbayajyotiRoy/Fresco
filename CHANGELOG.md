# Changelog

All notable changes to Fresco are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.34] — 2026-07-24

### Changed
- **Power saving now targets the actual bottleneck (GPU scaling).** 1.1.33's
  Power saving used decoder-level frame skipping (`vd-lavc-skipframe`). Tested
  on real Deepin 25 hardware by @175624 with `intel_gpu_top`, that changed the
  visible frame rate but saved nothing: for a hardware-decoded video wallpaper
  the load is **Render/3D (~99%)**, not decode (~17%), and skipping decoded
  frames touches neither (the GPU still decodes the stream and presents at
  display refresh). Power saving now instead reduces the per-frame **scaler**
  cost — Reduced and Minimum drop from the quality scalers (spline36 / lanczos
  with linear-light downscaling and dithering) toward cheap bilinear, trading
  sharpness for GPU-render load. It can only reduce or match GPU work, never
  increase it, and hardware decoding is untouched. This is a quality/perf
  trade-off, not a promised number; the magnitude of the win is pending
  confirmation on the reporter's Intel box.

### Fixed
- **App icon now appears in the Deepin launcher without restarting it.** Since
  the icon landed (1.1.3), it only showed after `killall dde-shell`. Root cause,
  found by diffing our `.deb` against galculator's (which the reporter confirmed
  works): we shipped a custom postinst that ran `gtk-update-icon-cache` /
  `update-desktop-database` during package configure. That refresh is already
  the job of the standard `hicolor-icon-theme` and `desktop-file-utils` dpkg
  triggers (fired automatically when files land in their dirs), and running it
  early interfered with Deepin's launcher refresh. Fresco now ships no
  maintainer scripts — identical to a plain debhelper GUI package — and relies
  on those triggers. (Fix pending confirmation on real Deepin 25.)
- **Removing or stopping the active wallpaper now reverts the desktop.** A new
  "Stop wallpaper" item (right-click the active card) turns the wallpaper off
  and restores the desktop's own background without deleting the entry;
  removing the active card does the same. Previously the daemon kept playing a
  wallpaper you'd deleted until the app was force-closed.

## [1.1.33] — 2026-07-23

### Changed
- **The frame-rate cap is replaced by a Power saving control** (Full quality /
  Reduced / Minimum), in the same two places: a global default in Settings →
  Advanced → Video quality and a per-wallpaper override in the editor.

  1.1.32's frame-rate cap did the opposite of what it promised. Capping fps used
  an `fps` video filter, and a video filter is *software*: inserting one into a
  hardware-decoding (VA-API) pipeline forces every frame to be copied off the
  GPU. A user on Intel Alder Lake-N measured video-engine load roughly
  **doubling** — about 17% to 34% — when capping 60fps to 30. Thanks to
  @175624 for catching it with `intel_gpu_top`.

  Power saving instead uses decoder-level frame skipping
  (`--vd-lavc-skipframe`), which discards frames inside libavcodec *before*
  they are decoded, so the work is never done and hardware decoding stays
  active. (Superseded in 1.1.34: this changed the visible frame rate but did
  not reduce GPU load on hardware-decoded video.)

  Existing `framerate` settings migrate automatically — any cap becomes Reduced.

### Fixed
- **Light mode readability.** Several surfaces were unreadable or unstyled in
  the light scheme: the wallpaper right-click menu's "Remove from library" was
  invisible (a flat destructive button inherited Adwaita's white label, leaving
  white text on paper); glass modals let the content behind them bleed through
  and collide with their own text; error messages rendered as ordinary grey
  text; and the capability notice and crop/transition stage had no styling at
  all. Dark mode is unchanged.

## [1.1.32] — 2026-07-23

### Added
- **Frame-rate cap** for video wallpapers — limit to 24/30/48/60 fps (or keep
  the original rate). **Superseded in the next release: this made decode load
  worse on hardware-decoded video, not better — see 1.1.33 above.** Two
  levels: a global default in Settings → Advanced → Video quality, and a
  per-wallpaper override in the crop/rotate editor.

### Fixed
- **Consistent app icon.** The scalable icon still shipped the old v0.0.1
  artwork while the fixed-size PNGs were the current logo, so the launcher
  (which converts the SVG to DCI on Deepin) and the taskbar could disagree. The
  scalable SVG is now regenerated from the current logo — every surface matches.

## [1.1.31] — 2026-07-23

### Fixed
- **Live wallpaper now actually shows on Deepin 25 (DDE)** ([#2]). 1.1.3's DDE
  support never worked: two bugs made it silently do nothing, both found by
  running Fresco on a real Deepin 25 desktop.
  - The scan for DDE's desktop window asked the X server for the client list
    with `long_length = u32::MAX`, which Xorg rejects — so the window was
    never found and Fresco always chose the wrong strategy.
  - The WM_CLASS matcher expected `"dde-shell"` + `"desktop"` as two separate
    strings; Deepin 25 reports the single token `"dde-shell/desktop"` with
    class `"org.deepin.dde-shell"`, so it never matched.

  The strategy itself changed too. Making DDE's wallpaper transparent cannot
  work on Deepin 25, so Fresco now declares its wallpaper window as
  `_NET_WM_WINDOW_TYPE_DESKTOP` **and** `_NET_WM_WINDOW_TYPE_NORMAL` — the
  same pair dde-shell uses — and raises it with a sibling-less
  `ConfigureWindow(Above)`. A sibling-relative restack is impossible here:
  KWin reparents both windows, so they are not siblings and the request fails
  with BadMatch. Verified on Deepin 25: the wallpaper window sits above
  dde-shell's desktop, app windows and the dock still stack above it, and
  clicks pass through to the desktop (right-click menu still works).

  Trade-off: desktop icons are hidden while a live wallpaper is set, because
  DDE draws icons and wallpaper inside one window. Set `dde_mode` in
  config.toml (or `FRESCO_DDE_MODE`) to `transparent` for the old behaviour on
  Deepin 20/23, or `restack` to force the new one.
- Behaviour on every non-Deepin desktop is byte-identical to 1.1.3: the extra
  window type and the raise are only used when DDE is detected.

### Added
- Deepin 25 (crimson) added to the distro CI matrix (build + clean-install),
  plus an install-time check that all icon sizes and the `.desktop` entry
  land correctly on every distro.

### Fixed
- **App icon missing under Deepin's bloom icon theme** ([#1]). The icon was
  shipped only as `hicolor/scalable` SVG, which bloom (and other
  fixed-size-only themes) never look up. The packages now also install
  48/64/128/256/512 px PNGs into hicolor, and the .deb refreshes the icon
  cache in postinst/postrm.

[#1]: https://github.com/DibbayajyotiRoy/fresco/issues/1
[#2]: https://github.com/DibbayajyotiRoy/fresco/issues/2

## [1.1.2] — 2026-07-20

### Added
- **Add from a link.** Paste a Pinterest pin (pin.it short links and story
  pins included) or any direct video/image URL — Fresco resolves it, downloads
  it, and opens the crop/rotate editor so you can frame it before setting.
- **Your wallpaper on every new tab.** An optional local-only browser bridge
  (127.0.0.1, off by default) plus a companion extension in `extension/`
  (Chrome, Brave, Edge, Firefox) mirrors your wallpaper — or a
  browser-specific pick via right-click → "Set as browser wallpaper".
- **Command palette** (Ctrl+K): set any wallpaper by name, random wallpaper,
  and jump to any feature from the keyboard.
- Favorites with hover actions, drag-and-drop import, a first-run feature
  tour, and a quick day/night schedule pause switch in Settings.

### Privacy
- Fresco can send anonymous usage statistics — a daily ping (random install
  id, app version, distro name, desktop/compositor, session type, renderer
  backend, decode mode, monitor count), feature-usage counts, and error kinds.
- **Nothing is sent until you say yes**: a one-time consent dialog asks on
  first launch, and the answer can be changed anytime in Settings →
  "Share anonymous usage statistics".
- No personal data, no file names, no wallpaper content is ever sent.

### Fixed
- **Fullscreen auto-pause now works on COSMIC.** The wallpaper pauses while a
  window is fullscreen (no more decoding a hidden wallpaper under your video),
  via COSMIC's native toplevel-info protocol — previously this protection was
  silently unavailable there.
- Clicking the feedback reminder notification now opens the feedback dialog
  directly instead of just launching the app.

### Changed
- **Media-first redesign**: larger wallpaper grid, resolution/fps/size on
  every card with 4K badges, a cleaner now-playing pill, and a real
  drop-files-here empty state.

## [1.1.1] — 2026-07-17

### Fixed
- **Live wallpapers work on Ubuntu 24.04-based systems (COSMIC, Pop!_OS 24.04,
  Mint 22…).** The bundled renderer was built against an older libmpv and
  silently failed to start on newer distros, leaving the desktop's default
  wallpaper. Fresco now ships one renderer build per libmpv generation and
  picks the one that works on your system automatically.
- The install command detects a renderer that can't load and rebuilds it
  against your system's libmpv on the spot, so a fresh install always ends
  with a working live wallpaper.
- **Library cards no longer resize or jump around while hovering** — the
  hover-to-play preview could push the whole grid into a glitchy reflow loop
  on high-resolution videos.
- `fresco doctor` now catches a renderer that exists but can't load, instead
  of reporting a healthy system while nothing renders.

### Changed
- **In-app updates finish themselves.** Updating now shows a real progress
  bar with live download percentage, and the app restarts automatically a few
  seconds after the update completes (cancellable) — no more wondering whether
  a restart is needed. The wallpaper daemon restarts too, so fixes apply
  immediately.
- The "what's new" notes now always appear after an update.

## [1.1.0] — 2026-07-12

### Fixed
- **Multi-monitor video sync** — the same video on several displays now stays
  in step instead of slowly drifting apart.
- Scheduled wallpaper swaps no longer leak the previous entry's rotation and
  crop onto the next wallpaper.
- Smoother playback on Wayland: display-matched frame timing now applies there
  just like on X11.

### Added
- Occasional feedback reminders (can be turned off in Settings) so it's easy
  to tell us what to improve; reports now carry your timezone and locale for
  region-aware fixes.

## [1.0.1] — 2026-07-04

### Fixed
- **Editing a wallpaper's rotation now updates its card thumbnail** — the
  library card kept the old orientation before (thumbnails were only generated
  at import).
- Hover-to-play is skipped on rotated entries: GTK's inline player can't
  rotate, and motion in the wrong orientation read as a bug. The static
  (correctly rotated) thumbnail shows instead.

## [1.0.0] — 2026-07-03

The biggest Fresco release yet — sound that always works, pixel-true quality
on big screens, per-display control, schedules, and an in-app wallpaper catalog.

### Fixed
- **Per-wallpaper sound is reliable now.** If Fresco started before your audio
  system on login, mpv silently dropped the audio track forever; the daemon now
  detects it and restores audio automatically (both X11 and Wayland).
- **4K/8K quality on large displays.** Correct downscaling + dithering are on
  for every quality profile: sharper 8K→4K downscales (SSIM 0.54 → 0.74 on our
  fidelity harness), no gradient banding, pixel-exact rendering verified at
  HiDPI scale 1 and 2.
- Update failures now show the actual error output, not just an exit code.
- **Rotated wallpapers no longer distort colors.** A custom chroma scaler
  combined with rotation corrupted chroma into a green cast (affected the High
  quality profile before this release too); rotated video now keeps the
  default chroma path.
- **Workspace switcher / overview now shows the ROTATED wallpaper.** The still
  frame GNOME surfaces use is generated with your rotation applied (ffmpeg).
- **Hovering a video card no longer blanks it.** The live preview swaps in
  only once the first frame is decoded; with missing codecs the thumbnail
  simply stays.

### Added
- **Wallpaper catalog**: browse curated wallpapers in-app (menu → "Browse
  wallpapers…") and set one in two clicks; license + author shown on every card.
- **Per-display wallpapers from the GUI**: right-click a wallpaper → "Set on
  <display>"; "Show default on all displays" clears overrides.
- **Day & night schedules** (Advanced): switch between two wallpapers on a
  timer; times/solar modes available via config.toml (docs/SCRIPTING.md).
- **Add from URL**: paste a direct .mp4/.webm/image link to import it.
- **X11 fullscreen auto-pause** (parity with Wayland): per-monitor pause while
  a window is fullscreen.
- Wayland: newly plugged displays are picked up on the next apply — no daemon
  restart.
- Honest status: real CPU%, renderer memory included in RSS, source
  resolution/bit-depth/dropped frames, and a warning when a ≥4K file can't be
  hardware-decoded.
- Scripting docs (docs/SCRIPTING.md) with verified copy-paste recipes.

### Verification
- New machine-proof harnesses in-tree: audio (tests/audio), visual fidelity
  (tests/fidelity), plus schedule/download/catalog unit suites — 73 tests total.

## [0.0.91] — 2026-07-02

### Added
- **Update from inside the app.** Fresco now checks GitHub for new releases on
  its own (at most once a day) and shows an "Update available" banner — click
  **Update now**, authenticate once, and the new version installs with live
  progress and a one-click restart. No more trips to the releases page. A
  manual **Check for updates** lives in the menu, and Flatpak or non-apt
  installs get a copyable install command instead.
- **Live status in the header.** A status pill shows what's playing, whether
  hardware decoding is active, and current CPU use — with a pause/resume
  button right next to it.
- **Relink broken wallpapers.** If a wallpaper's source file was moved or
  deleted, the card menu now offers "Relink…" to point it at the file's new
  home instead of removing and re-adding it.
- **About dialog and keyboard shortcuts.** Ctrl+F focuses search, Ctrl+comma
  opens the menu, Ctrl+Q quits.

### Changed
- **The window now adapts to any size.** Wallpaper cards scale fluidly with
  the window, the grid reflows from a single narrow column up to wide layouts,
  and content stays centered and readable on ultrawide and 4K displays.
- **One-step install from the website.** The landing page now leads with the
  one-line installer instead of sending visitors to browse GitHub releases.

### Fixed
- Setting a wallpaper now confirms with a toast, and launching Fresco while
  it's already open brings the existing window forward instead of opening a
  duplicate.

## [0.0.9] — 2026-06-27

### Added
- **Rotate a video or image wallpaper.** A new "Rotate 90°" button in the editor
  turns the media — fixing sideways phone photos and videos — with hardware
  decoding intact. The orientation is remembered per wallpaper.

### Fixed
- **Video wallpaper sound now works.** Setting a video from the gallery always
  re-muted it, so audio never came out unless you went through the editor every
  time. Your mute/volume choice is now remembered per wallpaper, so turning sound
  on sticks.
- **Gallery hover no longer glitches.** Hovering a video card flickered between the
  thumbnail and the inline video preview as the pointer crossed the card's
  buttons; the preview now holds steady.

### Changed
- **More reliable Wayland detection.** Fresco now probes the compositor's
  protocols directly instead of shelling out to an external tool, so live-wallpaper
  support is detected correctly even on minimal sessions.

## [0.0.8] — 2026-06-26

### Fixed
- **The wallpaper now actually restores on login.** With `autostart` enabled,
  the login-restore entry was only written when you toggled the setting in the
  app — so a default/fresh install never got one and the daemon never started
  on boot (you'd see a static still-frame until you opened the app). The daemon
  now ensures the entry exists on startup, and the entry uses an **absolute
  path** to `frescod` so it launches even when `frescod` isn't on the login PATH.
- **Cold-boot video stall self-heal (X11).** If a video isn't advancing shortly
  after login, the daemon rebuilds it automatically — what re-selecting the
  wallpaper used to do by hand.

## [0.0.7] — 2026-06-24

### Fixed
- **X11: the live wallpaper no longer comes up frozen after a reboot.** On a
  cold boot the X server and window manager could leave the wallpaper window
  not-yet-viewable when mpv started, so its display-synced video output stalled
  on the first frame and stayed static until you re-selected the wallpaper. The
  daemon now waits for the window to become viewable before embedding mpv.

### Added
- **Wayland live wallpaper support** on layer-shell compositors (COSMIC,
  Hyprland, Sway, KDE Plasma 6) via the bundled `mpvpaper` backend. The backend
  is enabled by default and supervised over mpv's IPC socket.
- **Auto-pause on fullscreen** (wlroots / KDE Plasma 6 / COSMIC): the wallpaper
  on an output pauses while a window there is fullscreen and resumes when it
  leaves, reclaiming hardware-decode cost while hidden. GNOME doesn't expose the
  protocol, so it's inactive there.
- **Event-driven update notifications**: the daemon raises a desktop prompt when
  a newer version is published, with one-click update on `.deb` installs.
- **Wayland capability probe**: when `wayland-info`/`weston-info` is installed,
  Fresco checks the registry for `zwlr_layer_shell_v1` instead of guessing from
  the desktop name.
- **Build helper** `scripts/build-mpvpaper.sh` for source builds on Wayland.

### Changed
- **GNOME Wayland** now uses the existing static-frame fallback instead of
  blocking the app; the live limitation is explained in the UI and `doctor`.
- `fresco doctor` and `frescod --check` report the detected backend capability
  and mpvpaper availability.
- The installer no longer refuses to run on Wayland; it explains live vs static
  behavior and continues.

## [0.0.3] — Theming, polish & performance

### Added
- **Theme & accent colors** — light / dark / system, with six accent palettes.
- **Right-click context menu** on library cards: Set, Edit / Crop, Rename,
  and Remove from library (deletes the entry + thumbnail, not your media file).
- **Multi-image slideshows** — pick several images (or a folder) and loop them on
  an adjustable interval (default 30s).
- **In-app feedback** (anonymous, opt-in) and **update notifications**.
- **"What's new" modal** after an update; **glass (translucent) modals**.

### Changed
- **Big memory drop.** Hardware decode auto-enabled on Intel hybrid laptops
  (auto-pins the `iHD` VA-API driver), audio fully skipped when muted, and mpv
  read-ahead caches trimmed — typical RSS dropped from ~215 MB toward ~120–150 MB.
- **~20% smaller binaries** (fat LTO, single codegen unit, `panic=abort`).
- **Simpler controls** — removed the Pause/Stop buttons; setting a wallpaper just
  runs it and picking another switches it (no more "stuck/stopped" state).

## [0.0.2] — Bug fixes

### Fixed
- **Fixed a freeze/crash when changing the wallpaper.** Re-lowering the desktop
  window in response to X11 stacking events caused an infinite restack loop that
  flooded the compositor; stacking is now handled by a periodic pass instead.
  Also, each mpv instance is now terminated *before* its window is destroyed
  (the GPU context is bound to the window), so switching wallpapers no longer
  leaks stuck decoders.
- **Add / Add Folder now work.** The native file chooser is kept alive until it
  responds, so files you pick actually register and open the editor (previously
  the portal's reply was dropped because the chooser was freed too early).
- The file picker now defaults to an **"All supported"** filter showing both
  videos and images (it was videos-only before).
- **GNOME overview, workspace switcher, and lock screen** now show a still frame
  matching the live wallpaper instead of the old desktop background. Your
  original background is saved and restored when you press Stop.

### Changed
- CI toolchain pinned to Rust 1.91 for reproducible lint results; fixed a
  clippy lint and the release workflow's smoke-test step.

## [0.0.1] — Initial release

First public release. A GUI-first live-wallpaper setter for Debian-based Linux
(Pop!_OS, Ubuntu, Mint, Debian) on X11.

### Added
- **GUI wallpaper setter** (GTK4 / libadwaita) — pick media, click Set, close
  the app; the wallpaper keeps playing via a detached daemon.
- **Wallpaper types**: looping video (mp4/webm/mkv/avi/mov), animated GIF,
  static image, auto-rotating image **slideshow**, and multi-video **playlist**.
- **Hardware-accelerated playback** via libmpv (`hwdec=auto-safe` → VA-API /
  NVDEC / VDPAU) so CPU usage stays low without degrading quality.
- **Drag-to-crop editor** — frame the exact region of a video/image, applied
  through VO-side zoom/pan so hardware decode stays zero-copy.
- **Wallpaper library** — saved wallpapers as a thumbnail grid with a recently
  used row, search, and broken-entry (missing file) badges.
- **Pause / resume** and **pause-on-battery** (no extra daemons; reads
  `/sys/class/power_supply`).
- **Restore on login** via an XDG autostart entry (toggleable).
- **Multi-monitor** support with per-connector overrides and live monitor
  hotplug handling.
- **Scaling quality** toggle (Balanced / High-Lanczos) under Advanced settings.
- **`frescod --check`** diagnostics command: session type, libmpv version,
  GPUs, VA-API availability, config validity, and live daemon status.
- **Packaging**: `.deb` built in CI and attached to GitHub Releases, a
  `curl | bash` installer, and download-count tracking via a README badge.

### Known limitations
- X11 sessions only — Wayland support is planned for a future release.
- Web/HTML wallpapers are out of scope for this release.

[0.0.9]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.8...v0.0.9
[0.0.8]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.3...v0.0.7
[0.0.3]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/DibbayajyotiRoy/fresco/releases/tag/v0.0.1

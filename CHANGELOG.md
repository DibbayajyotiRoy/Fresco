# Changelog

All notable changes to Fresco are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.0.7]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.3...v0.0.7
[0.0.3]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/DibbayajyotiRoy/fresco/releases/tag/v0.0.1

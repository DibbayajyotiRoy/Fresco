# Changelog

All notable changes to Fresco are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/DibbayajyotiRoy/fresco/releases/tag/v0.0.1

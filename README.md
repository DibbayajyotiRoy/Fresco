<div align="center">

<img src="data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Fresco logo — live wallpaper app for Linux" />

# Fresco — Live Wallpapers for Linux

**Set any video, GIF, or image as an animated desktop wallpaper.** A free, open-source **Wallpaper Engine alternative for Linux**, working on **X11 and Wayland** (COSMIC, Hyprland, Sway, KDE Plasma 6, Deepin DDE).

[![Release](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![License](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![Stars](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Website](https://fresco.dibbayajyoti.com) · [Install](#install) · [FAQ](#faq) · [Changelog](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="data/screenshots/gallery.png" alt="Fresco wallpaper library showing video wallpapers on a Linux desktop" width="800" />

</div>

## What is Fresco?

Fresco is a free, open-source live wallpaper app for Linux. It sets videos, GIFs, images, slideshows, and video playlists as your animated desktop wallpaper through a GTK4 GUI — no terminal required. Playback is hardware-accelerated through mpv (VA-API / NVDEC), so an animated wallpaper costs near-zero CPU. It installs as a `.deb` and restores your wallpaper on login.

## Quick facts

| | |
|---|---|
| **What it is** | Live / video wallpaper app for the Linux desktop |
| **Works on** | X11 and Wayland layer-shell (COSMIC, Hyprland, Sway, KDE Plasma 6, Deepin DDE) |
| **Distros** | Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25 |
| **Media formats** | mp4, webm, mkv, avi, mov, GIF, jpg/png/webp, slideshows, playlists |
| **Price** | Free — GPL-3.0-or-later, no ads, no account |
| **Built with** | Rust, GTK4 / libadwaita, libmpv |
| **Install** | `.deb` package or one-line script |
| **Latest version** | 1.1.35 |

## Install

**One-liner** (Debian, Ubuntu, Pop!_OS, Linux Mint, elementary OS, Deepin):

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

**Manual:** download the `.deb` from [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) and run:

```bash
sudo apt install ./fresco_*.deb
```

**Build from source:** see [docs/INSTALL.md](docs/INSTALL.md).

## How to set a video as your wallpaper on Linux

1. Install Fresco (above) and open it from your app launcher.
2. Click **Add** and pick a video, GIF, or image — or paste a link with **From link**.
3. Optionally crop or rotate it in the editor.
4. Click **Set**, then close the app.

The wallpaper keeps playing after the window closes and comes back automatically on login.

## Features

- **Any media** — looping video (mp4/webm/mkv), animated GIF, static image, image slideshow, multi-video playlist
- **Add from a link** — paste a Pinterest pin or any direct video/image URL; Fresco downloads it and opens the crop editor
- **Hardware decode** — GPU video decoding (VA-API / NVDEC) keeps CPU usage near zero
- **Power saving** — cheaper GPU scaling for laptops; measured to roughly halve GPU power (see [Performance](#performance-and-battery-life))
- **Multi-monitor** — a different wallpaper per display, with synced playback for the same video across monitors
- **Day & night schedules** — swap wallpapers on a timer, arbitrary time slots, or sunrise/sunset
- **Batch management** — select several wallpapers at once and remove them in one step
- **Built-in catalog** — browse curated, properly licensed wallpapers in-app
- **Command palette** — Ctrl+K to set any wallpaper or reach any feature from the keyboard
- **Fullscreen auto-pause** — per monitor, including on COSMIC; plus pause-on-battery
- **Browser new-tab extension** — mirror your wallpaper on every new tab (Chrome/Brave/Edge/Firefox; load unpacked from [`./extension`](extension))
- **Deepin DDE support** — on Deepin 25, Fresco adapts the DDE desktop automatically so live wallpapers show through with desktop icons intact
- **Crop & rotate editor**, per-wallpaper sound/volume, slideshow transitions, and a searchable library

## Supported environments

| Environment | Live wallpaper | Notes |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, MATE, …) | ✅ | Embedded renderer |
| Deepin 25 (DDE, X11) | ✅ | Automatic DDE adaptation |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| KDE Plasma 6 (Wayland) | ✅ | layer-shell |
| GNOME on Wayland | ⚠️ | Static-frame fallback — Mutter exposes no live wallpaper surface |

Every environment above is exercised headlessly in CI on each release.

## Fresco vs other live wallpaper options

| | Fresco | Wallpaper Engine | mpvpaper | xwinwrap |
|---|---|---|---|---|
| **Native Linux app** | ✅ | ❌ Windows; on Linux only via Steam Play/Proton | ✅ | ✅ |
| **Graphical app (no terminal)** | ✅ | ✅ | ❌ command line | ❌ command line |
| **X11** | ✅ | via Proton | ❌ | ✅ |
| **Wayland layer-shell** | ✅ | ❌ | ✅ | ❌ |
| **Hardware decode** | ✅ VA-API / NVDEC | ✅ | ✅ | depends on player |
| **Multi-monitor, per-display** | ✅ | ✅ | one instance per output | one instance per output |
| **Wallpaper library + scheduling** | ✅ | ✅ | ❌ | ❌ |
| **Price** | Free (GPL-3.0) | Paid | Free (GPL) | Free |

Fresco bundles `mpvpaper` as its Wayland renderer, so it builds on that project rather than competing with it. Comparison reflects these projects as of July 2026.

## Performance and battery life

A contributor measured package power with `turbostat` on an Intel N150 (Deepin 25, VA-API, two runs per level) while a video wallpaper played:

| Video | Power saving | GPU power | Total package power |
|---|---|---|---|
| 1080p 60fps | Full quality | 1.37 W | 6.00 W |
| 1080p 60fps | **Reduced** (default) | **0.63 W** (−54%) | **4.03 W** (−33%) |
| 4K 60fps | Full quality | 2.77 W | 7.94 W |
| 4K 60fps | **Reduced** (default) | **1.60 W** (−42%) | **5.95 W** (−25%) |
| 4K 60fps | Minimum | 0.99 W (−65%) | 4.97 W (−37%) |

Power saving reduces per-frame GPU scaling cost. No frames are dropped and hardware decoding is untouched, so playback stays smooth — the trade-off is image sharpness, not motion. Reduced is the default; Minimum is worth choosing for 4K sources.

## FAQ

### Does Wallpaper Engine work on Linux?

Not natively. Wallpaper Engine is a Windows application; on Linux it can only be run through Steam Play/Proton, which is unofficial and does not work on every setup. Fresco is a native Linux alternative that installs as a `.deb` and needs no compatibility layer.

### How do I set a video as my wallpaper on Ubuntu?

Install Fresco, open it, click **Add**, pick your video, and click **Set**. The video plays as your desktop background and is restored on login. Ubuntu's default GNOME-on-Wayland session falls back to a static frame — log into an **Ubuntu on Xorg** session for full live playback.

### Do live wallpapers use a lot of CPU or battery?

Not with hardware decoding. Fresco decodes video on the GPU (VA-API / NVDEC), keeping CPU usage near zero. On an Intel N150, a 1080p wallpaper drew 0.63 W of GPU power at the default Power saving level. Fresco also pauses automatically when a window goes fullscreen, and can pause on battery.

### Does Fresco work on Wayland?

Yes, on compositors that implement the layer-shell protocol — COSMIC, Hyprland, Sway, and KDE Plasma 6. GNOME on Wayland is the exception: Mutter exposes no wallpaper surface, so Fresco falls back to a static frame there. X11 sessions are fully supported.

### Is Fresco free?

Yes. Fresco is free and open source under GPL-3.0-or-later. There are no ads, no accounts, and no paid tier.

### Can I use a different wallpaper on each monitor?

Yes. Fresco supports per-display wallpapers, and when the same video is used across several monitors, playback is kept in sync.

### Does it support GIFs and image slideshows?

Yes — animated GIFs, static images, image slideshows with transitions (crossfade, fade, slide, Ken Burns), and multi-video playlists, in addition to video files.

### Which Linux distros are supported?

Fresco ships a `.deb` package for Debian- and Ubuntu-based distributions: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, and Deepin 25. Other distributions can build from source — see [docs/INSTALL.md](docs/INSTALL.md).

### How do I remove several wallpapers at once?

Click **Select** in the footer (or right-click a wallpaper and choose **Select…**), tick the ones you want, and click **Remove**. **Select all** respects the current search, so you can search first and then clear a whole batch. Removing a wallpaper takes it out of your Fresco library — the source file on disk is kept.

## Privacy

Fresco can send anonymous usage statistics, but **nothing is sent until you opt in** — a one-time consent dialog asks on first launch, and the choice can be changed anytime in Settings. No personal data, file names, or wallpaper content is ever collected. Details in the [changelog privacy notes](CHANGELOG.md#privacy).

## Contributing & feedback

Bug reports, feature requests, and PRs are welcome — open an [issue](https://github.com/DibbayajyotiRoy/fresco/issues), or use the in-app feedback dialog.

## License

[GPL-3.0-or-later](LICENSE) — free and open source.

---

<sub>Fresco — live wallpaper, video wallpaper, and animated desktop background for Linux (X11 and Wayland). A Wallpaper Engine alternative for Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, and Deepin. Last updated: 2026-07-26.</sub>

<div align="center">

<img src="data/icons/io.github.dibbayajyotiroy.Fresco.svg" width="96" alt="Fresco icon" />

# Fresco — Live Wallpapers for Linux, Made Easy

**Set any video, GIF, or image as your desktop wallpaper on Debian, Ubuntu, Pop!_OS & Mint** — a simple, GUI alternative to **Wallpaper Engine** and **Lively** for Linux, with hardware-accelerated playback.

![Downloads](https://img.shields.io/github/downloads/DibbayajyotiRoy/fresco/total?style=flat-square&color=brightgreen&label=downloads)
![License](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)
![Platform](https://img.shields.io/badge/platform-Linux%20%C2%B7%20X11%20%26%20Wayland-blue?style=flat-square)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20GTK4-orange?style=flat-square)

</div>

> Windows has Wallpaper Engine. macOS has Lively. **Linux had nothing simple — until Fresco.**
> Pick a video, click *Set*, close the app. Your wallpaper keeps playing and comes back on login.

<!-- Add a hero GIF/screenshot here for instant comprehension:
     ![Fresco screenshot](data/screenshots/library.png) -->

---

## Why Fresco?

Every other Linux live-wallpaper option is terminal-only, abandoned, locked to one GNOME version, or breaks under the compositor. Fresco is a **proper desktop app**: install a `.deb`, open it from your app menu, pick media, done.

- 🎬 **Any media** — looping video (mp4/webm/mkv), animated GIF, static image, image **slideshow**, or a multi-video **playlist**
- ⚡ **Hardware-accelerated** — GPU video decode (VA-API / NVDEC) keeps CPU near zero without degrading quality
- ✂️ **Drag-to-crop editor** — frame exactly the region you want (no other Linux tool has this)
- 🖼 **Wallpaper library** — saved thumbnails, recently-used, and search
- 🔁 **Set & forget** — close the app, the wallpaper keeps playing; restored automatically on login
- ⏸ **Battery-aware** — pause on battery, pause/resume any time
- 🖥 **Multi-monitor** — a different wallpaper per display

## Fresco vs other Linux options

| | **Fresco** | Hidamari | Komorebi | mpvpaper | Wallpaper Engine |
|---|:---:|:---:|:---:|:---:|:---:|
| GUI app (no terminal) | ✅ | ✅ | ✅ | ❌ | ✅ |
| Works on GNOME/X11 | ✅ | ✅ | ✅ | ❌ (Wayland-only) | ❌ (needs compositor off) |
| Video quality (mpv hwdec) | ✅ | ⚠️ VLC | ⚠️ | ✅ | ✅ |
| Drag-to-crop | ✅ | ❌ | ❌ | ❌ | ✅ |
| Playlists | ✅ | ❌ | ❌ | manual | ✅ |
| Wallpaper library | ✅ | ❌ | ❌ | ❌ | ✅ |
| Actively maintained | ✅ | ✅ | ❌ | ✅ | ✅ |
| Free & open source | ✅ | ✅ | ✅ | ✅ | ❌ (paid, Windows) |

## Supported distributions

| Distro | Versions | Status |
|--------|----------|--------|
| Pop!_OS | 22.04 | ✅ Primary target |
| Ubuntu | 22.04, 24.04 | ✅ |
| Linux Mint | 21, 22 | ✅ |
| Debian | 12 (Bookworm) | ✅ |
| elementary OS | 7 | ✅ |

> **Wayland support:** live wallpapers on layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6) via bundled `mpvpaper`. GNOME Wayland shows a static frame fallback.

## Install

**One-liner:**
```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | bash
```

**Or download the `.deb`** from [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) and double-click it (or `sudo apt install ./fresco_*.deb`).

For the lowest CPU usage, install your GPU's hardware-decode driver:
```bash
sudo apt install intel-media-va-driver   # Intel
sudo apt install mesa-va-drivers          # AMD / Mesa
# NVIDIA: the proprietary driver provides NVDEC
```

## Usage

Launch **Fresco** from your app menu → **+ Add** → pick a video → drag a crop frame → **Set as Wallpaper** → close the window. That's it. Run `frescod --check` any time for hardware-decode diagnostics.

## FAQ

**Is there a Wallpaper Engine for Linux?**
Yes — Fresco is a free, open-source live-wallpaper app for Linux that works like Wallpaper Engine: pick a video, set it as your animated desktop background.

**How do I set a video as my wallpaper on Ubuntu / Pop!_OS / Debian?**
Install the Fresco `.deb`, open it, click **+ Add**, choose your video, and click **Set as Wallpaper**.

**Does it work on Wayland or GNOME?**
It runs on **GNOME and any X11 session** today (Pop!_OS, Ubuntu, Mint, Debian). On **Wayland layer-shell compositors** (COSMIC, Hyprland, Sway, KDE Plasma 6) live wallpapers work via the bundled `mpvpaper` backend. **GNOME Wayland** shows a static-frame fallback because Mutter does not expose a live wallpaper surface.

**Will a video wallpaper drain my battery or CPU?**
Fresco uses GPU hardware decoding so CPU stays near zero, and it can **automatically pause on battery**.

**What video formats are supported?**
mp4, webm, mkv, avi, mov, plus animated GIFs, static images (jpg/png/webp), folders as slideshows, and multi-video playlists.

## How it works

Two binaries: `fresco` (the GTK4/libadwaita GUI you can close) and `frescod` (a lightweight daemon).

- **X11:** `frescod` paints a desktop-level X11 window with an embedded [mpv](https://mpv.io) instance per monitor.
- **Wayland (layer-shell):** `frescod` supervises the bundled [mpvpaper](https://github.com/GhostNaN/mpvpaper) process and steers it over mpv's IPC socket.
- **GNOME Wayland:** a static frame is set as the desktop background (Mutter has no live wallpaper surface).

See [docs/AUDIT.md](docs/AUDIT.md) for the full design and competitive analysis, and [docs/FLATHUB.md](docs/FLATHUB.md) for Flatpak packaging.

## Building from source

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev ffmpegthumbnailer libmpv-dev
cargo build --release
# On Wayland, also build the bundled mpvpaper backend:
scripts/build-mpvpaper.sh
./target/release/fresco
```

## Contributing

Issues and PRs welcome. See [CHANGELOG.md](CHANGELOG.md) for release history.

## License

[GPL-3.0-or-later](LICENSE) — free and open source forever.

---

<div align="center">
<sub>Fresco — live wallpaper / video wallpaper / animated desktop background for Debian-based Linux. Made with ☕ for the Linux desktop community.</sub>
</div>

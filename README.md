<div align="center">

<img src="data/icons/io.github.dibbayajyotiroy.Fresco.svg" width="96" alt="Fresco icon" />

# Fresco — Live Wallpapers for Linux, Made Easy

**The most complete free live-wallpaper engine on Linux.** Set any video, GIF, or image as your desktop wallpaper — browse a built-in catalog, put a different wallpaper on every display, schedule day/night pairs, and let hardware decoding keep your CPU near zero. A real GUI alternative to **Wallpaper Engine** and **Lively**, free forever.

![Downloads](https://img.shields.io/github/downloads/DibbayajyotiRoy/fresco/total?style=flat-square&color=brightgreen&label=downloads)
![License](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)
![Platform](https://img.shields.io/badge/platform-Linux%20%C2%B7%20X11%20%26%20Wayland-blue?style=flat-square)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20GTK4-orange?style=flat-square)

</div>

> Windows has Wallpaper Engine and Lively. **Linux had nothing simple — until Fresco.**
> Pick a video, click *Set*, close the app. Your wallpaper keeps playing and comes back on login.

![Fresco — wallpaper library](data/screenshots/gallery.png)

---

## Why Fresco?

Every other Linux live-wallpaper option is terminal-only, abandoned, locked to one GNOME version, or breaks under the compositor. Fresco is a **proper desktop app**: install a `.deb`, open it from your app menu, pick media, done. And unlike everything else on this list, it doesn't just claim quality — **it ships the test harnesses that prove it** (see the numbers below).

- 🎬 **Any media** — looping video (mp4/webm/mkv), animated GIF, static image, image **slideshow**, a multi-video **playlist**, or a direct **URL**
- 🗂 **Built-in wallpaper catalog** — browse curated, properly-licensed wallpapers in-app and set one in two clicks (menu → *Browse wallpapers…*)
- ⚡ **Hardware-accelerated** — GPU video decode (VA-API / NVDEC) keeps CPU near zero without degrading quality
- 🐧 **X11 _and_ Wayland** — desktop-window backend on X11, plus a bundled `mpvpaper` layer-shell backend for Sway (verified) and COSMIC, Hyprland & KDE Plasma 6 (experimental — verification in progress)
- ✂️ **Crop & rotate editor** — drag a frame to pick the exact region, and rotate 90° to fix sideways phone clips (no other Linux tool has this)
- 🔊 **Per-wallpaper sound that just works** — per-wallpaper mute/volume, and if your login raced the audio server, Fresco restores the track automatically
- 🎞 **Slideshow transitions** — crossfade, fade, slide, or a slow Ken Burns pan between images
- 🖼 **Wallpaper library** — saved thumbnails, recently-used, and search
- 🔁 **Set & forget** — close the app, the wallpaper keeps playing; restored automatically on login
- ⏸ **Power-aware** — pause on battery, and auto-pause per monitor when a window there goes fullscreen (X11 and Wayland)
- 🖥 **Multi-monitor from the GUI** — right-click any wallpaper → *Set on \<display\>*; live hotplug on X11, and on Wayland newly plugged displays pick up on the next apply
- 🌗 **Day & night schedules** — switch between two wallpapers on a timer (or arbitrary time slots / sunrise-sunset via config)
- 🩺 **Self-healing engine** — cold-boot stall recovery, crashed-renderer respawn with anti-flap, automatic audio recovery when the sound server starts late, and honest diagnostics (`fresco status` shows real CPU, memory, source resolution, and dropped frames)
- 🧩 **Scriptable** — a documented JSON control socket for waybar toggles and workspace hooks ([docs/SCRIPTING.md](docs/SCRIPTING.md))
- 🎨 **Themes & accents** — light / dark / system with six accent palettes

## Measured, not promised

Fresco ships its proof harnesses in-tree (`tests/fidelity`, `tests/audio`) — every number below is reproducible on a headless compositor with one command, and CI gates releases on them.

| What we measure | Result |
|---|---|
| 8K→4K downscale quality (SSIM vs a Lanczos reference, zone-plate torture test) | **0.74** with Fresco's scaler stack vs **0.54** with the defaults most players use |
| Gradient banding (distinct luma levels reaching your screen, 256 = perfect) | **256** — dithering on every profile (was 220 before v1.0) |
| Pixel-fidelity at HiDPI scale 1× and 2× (1-pixel checkerboard crispness) | **100 / 100** — pixel-exact, verified by screenshot on X11 *and* Wayland |
| Audio recovery when the daemon starts before PipeWire (the classic silent-wallpaper bug) | **automatic**, within seconds — proven by a cold-boot repro harness |
| Decoder honesty | playing media your GPU can't hardware-decode warns you instead of silently stuttering |
| Automated tests | **73** across engine, schedule (NOAA solar ±2 min), downloads, catalog, and EWMH detection |

## Fresco vs other Linux options

| | **Fresco** | Hidamari | Komorebi | mpvpaper | Wallpaper Engine |
|---|:---:|:---:|:---:|:---:|:---:|
| GUI app (no terminal) | ✅ | ✅ | ✅ | ❌ | ✅ |
| Works on GNOME/X11 | ✅ | ✅ | ✅ | ❌ (Wayland-only) | ❌ (needs compositor off) |
| Works on Wayland (layer-shell) | ✅ | ⚠️ partial | ❌ | ✅ | ❌ |
| Video quality (mpv hwdec) | ✅ | ⚠️ VLC | ⚠️ | ✅ | ✅ |
| Crop & rotate | ✅ | ❌ | ❌ | ❌ | ⚠️ crop only |
| Per-wallpaper audio | ✅ | ✅ | ❌ | ⚠️ manual | ✅ |
| Playlists | ✅ | ❌ | ❌ | manual | ✅ |
| Wallpaper library | ✅ | ❌ | ❌ | ❌ | ✅ |
| Built-in wallpaper catalog | ✅ | ❌ | ❌ | ❌ | ✅ Workshop |
| Per-display wallpapers (GUI) | ✅ | ❌ | ❌ | ⚠️ manual | ✅ |
| Day/night schedules | ✅ | ❌ | ❌ | ❌ | ⚠️ |
| Scriptable control socket | ✅ | ❌ | ❌ | ⚠️ | ❌ |
| Self-healing + published benchmarks | ✅ | ❌ | ❌ | ❌ | ❌ |
| Actively maintained | ✅ | ✅ | ❌ | ✅ | ✅ |
| Free & open source | ✅ | ✅ | ✅ | ✅ | ❌ (paid, Windows) |

## Supported distributions

| Distro | Versions | Status |
|--------|----------|--------|
| Pop!_OS | 22.04 | ✅ Primary target |
| Pop!_OS | 24.04 (COSMIC) | ⚠️ Installs & runs (24.04 base is CI-tested weekly); live wallpapers via layer-shell, on-screen verification pending |
| Ubuntu | 22.04, 24.04 | ✅ |
| Linux Mint | 21, 22 | ✅ |
| Debian | 12 (Bookworm) | ✅ |
| elementary OS | 7 | ✅ |

> **Wayland support:** live wallpapers on layer-shell compositors via bundled `mpvpaper` — verified on Sway; COSMIC, Hyprland and KDE Plasma 6 are experimental while real-session verification lands (docs/WAYLAND_VERIFICATION.md). GNOME Wayland shows a static frame fallback.

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
It runs on **GNOME and any X11 session** today (Pop!_OS, Ubuntu, Mint, Debian). On **Wayland layer-shell compositors** live wallpapers work via the bundled `mpvpaper` backend — verified on Sway; COSMIC, Hyprland and KDE Plasma 6 are experimental while real-session verification lands. **GNOME Wayland** shows a static-frame fallback because Mutter does not expose a live wallpaper surface.

**Will a video wallpaper drain my battery or CPU?**
Fresco uses GPU hardware decoding so CPU stays near zero. It can **automatically pause on battery**, and it **auto-pauses per monitor** when a window there goes fullscreen.

**Can a video wallpaper play sound?**
Yes. Each wallpaper remembers its own mute state and volume, so you can unmute one specific video and the choice sticks. Wallpapers start muted by default. If your session's audio server starts after Fresco on login, the daemon detects the dropped track and restores it automatically.

**Where do I find wallpapers?**
Right in the app: menu → **Browse wallpapers…** opens a curated catalog (every item shows its license and author), or paste a direct video/image URL via **Add from URL…**.

**Can I put different wallpapers on each monitor?**
Yes — right-click any wallpaper in the library and choose **Set on \<display\>**. "Show default on all displays" clears the overrides.

**Can wallpapers change with the time of day?**
Yes. In the app: menu → **Advanced… → Day & night wallpaper** — pick a day wallpaper, a night wallpaper, and the two switch times. The daemon swaps them automatically (no restart, no flash), and a manual wallpaper choice holds until the next boundary. Power users get more modes in `~/.config/fresco/config.toml`:

```toml
# Arbitrary time slots
[schedule]
mode = "times"
[[schedule.at]]
time = "06:30"
[schedule.at.wallpaper]
kind = "video"
path = "/home/you/Videos/sunrise.mp4"
[[schedule.at]]
time = "22:00"
[schedule.at.wallpaper]
kind = "video"
path = "/home/you/Videos/night.mp4"

# Or sunrise/sunset (manual coordinates, no location services)
# [schedule]
# mode = "solar"
# lat = 26.1
# lon = 91.7
# [schedule.day]  / [schedule.night] = wallpaper blocks as above
```

**Can I crop or rotate a wallpaper?**
Yes. The editor has a **drag-to-crop** frame and a **90° rotate** (great for sideways phone videos). Both run on the GPU and are remembered per wallpaper.

**What video formats are supported?**
mp4, webm, mkv, avi, mov, plus animated GIFs, static images (jpg/png/webp), folders as slideshows (with crossfade / fade / slide / Ken Burns transitions), and multi-video playlists.

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

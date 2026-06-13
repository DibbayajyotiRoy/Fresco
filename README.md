# Fresco 🖼

**Live wallpapers for Linux** — set video, GIF, image, slideshow, or playlist wallpapers on any Debian/Ubuntu-based distro, with hardware-accelerated playback and a clean GUI.

![Downloads](https://img.shields.io/github/downloads/DibbayajyotiRoy/fresco/total?style=flat-square&color=brightgreen)
![License](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)
![Platform](https://img.shields.io/badge/platform-Linux%20X11-blue?style=flat-square)

---

## What it does

- **Pick any video** (mp4, webm, mkv, gif) or image (jpg, png, webp) → it plays as your desktop wallpaper
- **Close the app** — the wallpaper keeps playing (detached background process)
- **Restores on login** — no manual steps after a reboot
- **Hardware decode** — GPU-accelerated via VA-API/NVDEC, so CPU usage stays near 0%
- **Drag-to-crop** — frame exactly the region you want before setting
- **Playlist mode** — queue multiple videos to cycle on a loop
- **Wallpaper library** — thumbnail grid, one-click switching, recently used section

## Supported distros

| Distro | Version | Status |
|--------|---------|--------|
| Pop!_OS | 22.04 | ✅ Primary target |
| Ubuntu | 22.04, 24.04 | ✅ |
| Linux Mint | 21, 22 | ✅ |
| Debian | 12 (Bookworm) | ✅ |
| elementary OS | 7 | ✅ |

> X11 session required. Wayland support is planned.

## Install

### One-liner (recommended)
```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | bash
```

### Manual .deb
1. Download `fresco_*.deb` from [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
2. Double-click the file in your file manager, or run:
```bash
sudo apt install ./fresco_*.deb
```

### Optional: hardware decode drivers
For the lowest CPU usage, install your GPU's VA-API driver:
```bash
# Intel (Skylake and newer)
sudo apt install intel-media-va-driver

# AMD / Intel (Mesa-based)
sudo apt install mesa-va-drivers

# NVIDIA — use the proprietary driver (already installed if nvidia-smi works)
```

## Usage

Launch **Fresco** from your application menu. On first run, drag a video onto the window or click **Add** to pick a file. Click **Set as Wallpaper** — done. Close the window; the wallpaper keeps playing.

### Diagnostics / bug reports
```bash
frescod --check
```
Prints: session type, libmpv version loaded, GPU detected, hardware decode status, current wallpaper. Paste the output when filing a bug.

## Supported formats

| Type | Formats |
|------|---------|
| Video | mp4, webm, mkv, avi, mov, flv |
| Animated | gif (treated as video) |
| Image | jpg, png, webp, bmp, tiff |
| Slideshow | Any folder of images |
| Playlist | Multiple video files, cycled on a loop |

## Building from source

```bash
# Install build deps
sudo apt install libgtk-4-dev libadwaita-1-dev ffmpegthumbnailer libmpv-dev

# Build
cargo build --release

# Run
./target/release/fresco
```

## Architecture

Two binaries:
- `fresco` — GTK4/libadwaita GUI; can be closed while wallpaper plays
- `frescod` — headless daemon; manages X11 windows + mpv instances; communicates via Unix socket at `$XDG_RUNTIME_DIR/fresco/control.sock`

See [docs/AUDIT.md](docs/AUDIT.md) for the full competitive landscape and design decisions.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE)

---

Made with ☕ for the Linux desktop community.

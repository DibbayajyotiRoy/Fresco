<div align="center">

<img src="./data/icons/hicolor/256x256/apps/io.github.dibbayajyotiroy.Fresco.png" width="112" alt="Fresco application icon — a free, open-source live wallpaper app for Linux" />

# Fresco — Free Live and Video Wallpaper App for Linux

English | [简体中文](README.zh-CN.md)

**Fresco is a free, open-source live wallpaper app for Linux that sets any video, GIF or image as an animated desktop wallpaper on X11 and Wayland.** It is a native Linux alternative to Wallpaper Engine, with a graphical app, GPU-accelerated playback and desktop widgets.

[![Latest Fresco release on GitHub](https://img.shields.io/github/v/release/DibbayajyotiRoy/fresco?style=flat-square&label=release)](https://github.com/DibbayajyotiRoy/fresco/releases/latest)
[![GPL-3.0-or-later license](https://img.shields.io/github/license/DibbayajyotiRoy/fresco?style=flat-square)](LICENSE)
[![Publish workflow status](https://img.shields.io/github/actions/workflow/status/DibbayajyotiRoy/fresco/publish.yml?style=flat-square&label=publish)](https://github.com/DibbayajyotiRoy/fresco/actions/workflows/publish.yml)
[![GitHub stars](https://img.shields.io/github/stars/DibbayajyotiRoy/fresco?style=flat-square)](https://github.com/DibbayajyotiRoy/fresco/stargazers)

[Website](https://fresco.dibbayajyoti.com) · [Installation](#installation) · [Supported environments](#supported-environments) · [Comparison](#fresco-vs-wallpaper-engine-hidamari-komorebi-and-mpvpaper) · [FAQ](#faq) · [Widgets](#desktop-widgets-lyrics-clocks-visualiser-and-album-art) · [Changelog](CHANGELOG.md) · [Issues](https://github.com/DibbayajyotiRoy/fresco/issues)

<img src="./data/screenshots/gallery.png" alt="Fresco's wallpaper library on a dark Linux desktop: a searchable grid of video, image and slideshow wallpapers, with one video marked Active and a status pill showing NVDEC hardware decoding" width="800" />

</div>

## What is Fresco?

Fresco turns a video, GIF, image, slideshow or playlist into the wallpaper behind your windows, through a GTK4 / libadwaita app that needs no terminal. Playback runs through mpv, so decoding happens on the GPU and CPU usage stays near idle — see [Performance](#performance-and-battery-life) for what that costs at the wall. Your wallpaper is restored on every login.

- **Plays:** videos (mp4, webm, mkv, avi, mov), animated GIFs, images, image slideshows and video playlists.
- **Desktops:** X11 sessions (GNOME on Xorg, Cinnamon, Xfce, MATE, Deepin 25) and Wayland (COSMIC, Hyprland, Sway, KDE Plasma 6, Cinnamon 6.8+). GNOME on Wayland shows a still frame only — see [Supported environments](#supported-environments).
- **Hardware decoding:** VA-API (Intel, AMD) and NVDEC (NVIDIA), through mpv.
- **Widgets:** time-synced lyrics, clocks (including a world clock), an audio visualiser and album art, drawn into the wallpaper. All off by default.
- **Control:** the app, a Ctrl+K command palette, and `fresco next` / `fresco prev` / `fresco random` for keyboard shortcuts.
- **Install:** a `.deb` for Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS, Kali and Deepin (one-line script, manual download or the Deepin App Store); build from source on Fedora, Arch and others — see [Installation](#installation).
- **License:** free and open source under [GPL-3.0-or-later](LICENSE), with no ads and no account.
- **Users:** 2,500+ people in 127 countries. Latest version: 1.1.51.

## Installation

Fresco is packaged as a `.deb` for Debian-family distributions and builds from source everywhere else.

### Ubuntu, Debian, Linux Mint, Pop!_OS, elementary OS and Kali Linux

One-line install — it downloads the latest `.deb` from GitHub Releases and installs it with apt:

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=github bash
```

Or download the `.deb` from [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) and run:

```bash
sudo apt install ./fresco_*.deb
```

### Deepin 25

Open **App Store**, search for **Fresco**, and click **Install**. Fresco is published on the deepin Community App Store, so there is nothing to download by hand and updates arrive through the store. The one-line install above also works on Deepin.

### Fedora, Arch Linux and other distributions

There is no prebuilt package for these yet, so build from source. You need Rust (via [rustup](https://rustup.rs)) plus the GTK4, libadwaita and libmpv development headers:

```bash
git clone https://github.com/DibbayajyotiRoy/fresco.git && cd fresco
cargo build --release --all-features
```

On Wayland compositors Fresco also needs the `mpvpaper` renderer, which the `.deb` bundles and a source build does not:

```bash
# Arch Linux, CachyOS, EndeavourOS (AUR)
yay -S mpvpaper

# Fedora and anything else: build it against your system libmpv
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # must be on PATH
```

Fresco is not on the AUR or Flathub — the AUR package named `fresco` is an unrelated physics program. Details and troubleshooting are in [docs/INSTALL.md](docs/INSTALL.md).

After installing, open **Fresco** from your app launcher. If something looks wrong, run `fresco doctor`.

## How to set a video as your wallpaper on Linux

Install Fresco, click **Add**, pick your video, and click **Set as wallpaper**. The video plays behind your windows and comes back automatically on login.

1. Install Fresco (above) and open it from your app launcher.
2. Click **Add** and pick a video, GIF, or image — or paste a link with **From link**.
3. Optionally crop or rotate it in the editor.
4. Click **Set as wallpaper**, then close the app.

The wallpaper keeps playing after the window closes.

## Features

- **Any media** — looping video (mp4/webm/mkv), animated GIF, static image, image slideshow, multi-video playlist
- **Add from a link** — paste a Pinterest pin or any direct video/image URL; Fresco downloads it and opens the crop editor
- **Hardware decode** — GPU video decoding (VA-API / NVDEC) keeps CPU usage near idle by moving the work to the video engine, where it is measurably cheaper (not free — see [Performance](#performance-and-battery-life))
- **Power saving** — cheaper GPU scaling for laptops; measured to roughly halve GPU power (see [Performance](#performance-and-battery-life))
- **Multi-monitor** — a different wallpaper per display, with synced playback for the same video across monitors
- **Day & night schedules** — swap wallpapers on a timer, arbitrary time slots, or sunrise/sunset
- **Desktop widgets** — synced song lyrics, themed clocks, a world clock, an audio visualiser, and a turning album-art disc, drawn into the wallpaper itself so nothing covers your windows (the world clock has an optional float-above-windows mode); all off by default (see [Desktop widgets](#desktop-widgets-lyrics-clocks-visualiser-and-album-art))
- **Lock screen** — show your wallpaper and widgets (clock, greeting, now playing, battery, and more) on your real lock screen instead of the stock one — live video and widgets on COSMIC 1.9+ and KDE Plasma 6, wallpaper-and-widgets via `fresco lock` on Sway/Hyprland/niri and X11, a still frame elsewhere. Off by default, and Fresco never sees your password — see [docs/LOCKSCREEN.md](docs/LOCKSCREEN.md)
- **Keyboard control** — `fresco next`, `fresco prev` and `fresco random` for compositor keybindings, and a Ctrl+K command palette inside the app
- **Batch management** — select several wallpapers at once and remove them in one step
- **Built-in catalog** — browse curated, properly licensed wallpapers in-app
- **Fullscreen auto-pause** — per monitor, on X11, wlroots compositors, COSMIC and KDE Plasma (through its wallpaper plugin); optionally for maximized windows too, plus pause-on-battery
- **Browser new-tab extension** — mirror your wallpaper on every new tab (Chrome/Brave/Edge/Firefox; load unpacked from [`./extension`](extension))
- **Deepin DDE support** — on Deepin 25, Fresco adapts the DDE desktop automatically, and clicking the desktop brings the icons back for ten seconds whenever you need them (see [Troubleshooting](#my-desktop-icons-are-hidden-while-the-wallpaper-plays-on-deepin))
- **Crop & rotate editor**, per-wallpaper sound/volume, slideshow transitions, and a searchable library

## Supported environments

| Environment | Live wallpaper | Notes |
|---|---|---|
| X11 (GNOME, Cinnamon, XFCE, …) | ✅ | Embedded renderer. GNOME only where the distro still ships an Xorg session (see the GNOME note below) |
| Kali Linux (Xfce, X11) | ✅ | Runs on Kali's default Xfce-on-X11 session via the same embedded renderer as other X11 desktops |
| MATE (X11) | ✅ | Desktop icons stay visible and clickable over the wallpaper |
| Deepin 25 (DDE, X11) | ✅ | Automatic DDE adaptation — community-verified on Deepin 25 Community build1 |
| COSMIC (Wayland) | ✅ | layer-shell |
| Hyprland | ✅ | layer-shell |
| Sway | ✅ | layer-shell |
| Cinnamon 6.8 and newer (Wayland) | ✅ | Cinnamon's compositor gained layer-shell in 6.8. Cinnamon 6.6 and older have no layer-shell and keep a still frame on Wayland |
| KDE Plasma 6 (X11 and Wayland) | ✅ | Set through plasmashell's own wallpaper plugin, so desktop icons stay visible. Needs `qml6-module-qtmultimedia` for video. Playback is Qt's, not mpv's: muted, no hwdec tuning, crop or transitions; a playlist plays its first file and a slideshow shows its first frame |
| GNOME on Wayland | ⚠️ | Still frame only — Mutter exposes no live wallpaper surface, so no video and no widgets. Live video needs a Fresco GNOME extension (planned) |

Every environment above is exercised headlessly in CI on each release.

**GNOME note.** GNOME on Wayland is the one mainstream desktop where Fresco can only show a still frame. The old way around it, logging into an Xorg session, is going away: GNOME 49 disabled its X11 session and GNOME 50 removed it, so Ubuntu 25.10 and newer (including 26.04 LTS) and Fedora 43 and newer ship no Xorg session to switch to. Ubuntu 22.04 and 24.04 still offer "Ubuntu on Xorg". Where there is no Xorg session, live video on the stock GNOME desktop has to wait for a Fresco GNOME extension, which is planned; Fresco says so in the app instead of pretending the video is playing. KDE Plasma, COSMIC, Hyprland, Sway and every X11 desktop are unaffected.

> "Easy to use with a clean interface — one of the few live wallpaper apps properly adapted for Deepin 25, installable via .deb and running smoothly with hardware-accelerated playback."
>
> — 柒玖 (deepin forum) / 柒仈玖 (GitHub), tested on Deepin 25 Community build1, X11 session, Intel Alder Lake-N [Intel Graphics]

Deepin 25 ships X11 as its default session, and that is the session Fresco is verified on there. Deepin's own Wayland compositor, [Treeland](https://github.com/linuxdeepin/treeland), is still under development, so Fresco makes no claim about Deepin on Wayland yet.

## Fresco vs Wallpaper Engine, Hidamari, Komorebi and mpvpaper

| Tool | Platform | Wayland | X11 | Interface | Plays | License |
|---|---|---|---|---|---|---|
| **Fresco** | Linux | ✅ layer-shell compositors and KDE Plasma 6; still frame on GNOME | ✅ | GTK4 app, plus a `fresco` CLI | Video, GIF, images, slideshows, playlists | GPL-3.0-or-later |
| **Wallpaper Engine** | Windows (Steam); no official Linux version | n/a | n/a | Graphical app | Video, scene and web wallpapers | Paid, closed source |
| **[linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)** | Linux; needs the assets from a Steam copy of Wallpaper Engine | ✅ compositors with layer-shell | ⚠️ not when a desktop environment (GNOME, KDE) draws the background | Command line; its README lists separate community GUIs | Wallpaper Engine wallpapers | GPL-3.0 |
| **[Hidamari](https://github.com/jeffshee/hidamari)** | Linux, GNOME-focused | ⚠️ GNOME Wayland | ✅ | Graphical app | Video | GPL-3.0 |
| **[Komorebi](https://github.com/cheesecakeufo/komorebi)** | Linux | — | — | Graphical app | Image, video and web-page wallpapers | GPL-3.0 |
| **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** | Linux, wlroots-based compositors | ✅ layer-shell | ❌ | Command line | Video, through mpv | GPL-3.0 |
| **xwinwrap** | Linux, X11 only | ❌ | ✅ | Command line | Whatever player you attach (mpv, gifview) | — |
| **swww** (now [awww](https://codeberg.org/LGFae/awww)) | Linux, layer-shell compositors; not GNOME | ✅ | ❌ | Command-line daemon | Animated GIFs and images, not video | GPL-3.0 |

Fresco bundles `mpvpaper` as its Wayland renderer, so it builds on that project rather than competing with it. A `—` means the project's own documentation does not clearly say. Checked against each project's public repository on 2026-10-16: Komorebi's last repository push was in July 2025, and swww's GitHub repository is archived because the project moved to Codeberg under the name awww.

## Performance and battery life

柒玖 (deepin forum) / 柒仈玖 (GitHub) measured package power with `turbostat` on an Intel N150 (Deepin 25, VA-API, two runs per level) while a video wallpaper played:

| Video | Power saving | GPU power | Total package power |
|---|---|---|---|
| 1080p 60fps | Full quality | 1.37 W | 6.00 W |
| 1080p 60fps | **Reduced** (default) | **0.63 W** (−54%) | **4.03 W** (−33%) |
| 4K 60fps | Full quality | 2.77 W | 7.94 W |
| 4K 60fps | **Reduced** (default) | **1.60 W** (−42%) | **5.95 W** (−25%) |
| 4K 60fps | Minimum | 0.99 W (−65%) | 4.97 W (−37%) |

Power saving reduces per-frame GPU scaling cost. No frames are dropped and hardware decoding is untouched, so playback stays smooth — the trade-off is image sharpness, not motion. Reduced is the default; Minimum is worth choosing for 4K sources.

**How to read these numbers.** Hardware decoding does not make a live wallpaper free — it moves the work from the CPU to the video engine, where it is cheaper, not absent. "Low CPU usage" on its own is a weak claim, because the alternative being compared against is software decoding, which is far worse; the honest measure is whole-system draw, which is why the table reports **total package power** and not just GPU power.

Two caveats on the figures above, so they are not read as more than they are:

- They are **total package power while a wallpaper is playing**, not the marginal cost of the wallpaper. No idle baseline was recorded on the same machine, so the difference between these numbers and an idle desktop is not established here. The percentages compare power-saving *levels* against each other, which is what they are valid for.
- They are one machine (Intel N150, Alder Lake-N, VA-API, Deepin 25, two runs per level). Discrete GPUs, NVDEC, and other drivers will differ.

A live wallpaper always costs more than a static one. Power saving, fullscreen auto-pause and pause-on-battery exist to bound that cost, not to pretend it is zero.

## FAQ

### Is there a Wallpaper Engine alternative for Linux?

Yes. Fresco is a free, open-source Linux app that plays your own video, GIF and image wallpapers through a graphical interface, with hardware decoding and no compatibility layer. It does not open Wallpaper Engine's own wallpaper files; if you own Wallpaper Engine and want those on Linux, the separate [linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine) project runs them from the command line and needs the assets from a Steam copy of Wallpaper Engine. See the [comparison table](#fresco-vs-wallpaper-engine-hidamari-komorebi-and-mpvpaper).

### Does Wallpaper Engine work on Linux?

Not natively. Wallpaper Engine is a Windows application with no official Linux version. Fresco is a native Linux alternative that installs as a `.deb` and needs no compatibility layer.

### How do I set a video as my wallpaper on Ubuntu?

Install Fresco, open it, click **Add**, pick your video, and click **Set as wallpaper**. The video plays as your desktop background and is restored on login. Ubuntu's default GNOME-on-Wayland session can only show a still frame. On Ubuntu 22.04 and 24.04 you can log into an **Ubuntu on Xorg** session for full live playback; Ubuntu 25.10 and newer (including 26.04 LTS) ship no Xorg session, so there live video on the stock GNOME desktop needs a Fresco GNOME extension, which is planned. Ubuntu flavours that use KDE Plasma, Xfce or MATE are not affected.

### How do I get a live wallpaper on Ubuntu, Linux Mint, Pop!_OS, Fedora or Arch Linux?

Install the Fresco `.deb` on Ubuntu, Linux Mint and Pop!_OS, or build it from source on Fedora and Arch Linux. Commands for each are in [Installation](#installation).

- **Ubuntu:** `.deb`. On the default GNOME desktop, live playback needs an Xorg session (Ubuntu 22.04 and 24.04); KDE Plasma, Xfce and MATE flavours are unaffected — see the Ubuntu answer above.
- **Linux Mint:** `.deb`. Cinnamon on X11 is supported, and Cinnamon on Wayland from version 6.8.
- **Pop!_OS:** `.deb`. The COSMIC desktop (Wayland layer-shell) is supported.
- **Fedora:** build from source. Fedora 43 and newer GNOME is Wayland-only, so it shows a still frame; KDE Plasma and X11 desktops are supported.
- **Arch Linux:** build from source and install `mpvpaper` from the AUR (`yay -S mpvpaper`).

Only the `.deb` path is covered by Fresco's CI.

### Which Linux distros are supported?

Fresco ships a `.deb` package for Debian- and Ubuntu-based distributions: Ubuntu, Pop!_OS, Linux Mint, Debian, elementary OS, Deepin 25, and Kali Linux. Other distributions can build from source — see [Installation](#installation) and [docs/INSTALL.md](docs/INSTALL.md).

### Do live wallpapers use a lot of CPU, GPU or battery?

CPU, no. Battery, some — a live wallpaper is never free.

Fresco decodes video on the GPU (VA-API / NVDEC), so CPU usage stays near idle. That moves the cost to the video engine rather than removing it. Measured on an Intel N150 at the default Power saving level, a 1080p wallpaper drew 0.63 W of GPU power with total package power at 4.03 W while playing (see [Performance](#performance-and-battery-life) for how to read that, including what it does *not* say). Fullscreen auto-pause and pause-on-battery exist to keep that cost off your battery when it would matter most.

### Does Fresco work on Wayland?

Yes, on compositors that implement the layer-shell protocol — COSMIC, Hyprland, and Sway — and on KDE Plasma 6 (X11 and Wayland) through plasmashell's own wallpaper plugin. GNOME on Wayland is the exception: Mutter exposes no wallpaper surface, so Fresco shows a still frame there and says so in the app. X11 sessions are fully supported.

### Does Fresco work on GNOME?

Yes on X11, with the full live wallpaper — video, widgets, everything. On **GNOME under Wayland**, Mutter exposes no wallpaper surface for any app to draw into, so Fresco shows a still frame there and widgets are unavailable. If your distro still ships an Xorg session (Ubuntu 22.04 and 24.04 do), log out and choose "GNOME on Xorg" or "Ubuntu on Xorg" for full live playback. GNOME 49 disabled that session and GNOME 50 removed it, so Ubuntu 25.10 and newer (including 26.04 LTS) and Fedora 43 and newer have none: on those, live video on GNOME needs a Fresco GNOME extension, which is planned. `fresco doctor` tells you which case you are in.

### Does Fresco work on KDE Plasma, COSMIC, Hyprland or Sway?

Yes, on all four. COSMIC, Hyprland and Sway play live video through layer-shell. KDE Plasma 6 (X11 and Wayland) uses Fresco's own wallpaper plugin inside plasmashell, which keeps desktop icons visible but plays through Qt instead of mpv: muted, with no crop or transitions, and it needs `qml6-module-qtmultimedia` for video.

### Does Fresco work on Linux Mint, Xfce, Cinnamon or MATE?

Yes. Cinnamon, Xfce and MATE on X11 use the embedded renderer, and desktop icons stay visible over the wallpaper on Xfce and MATE. Cinnamon on Wayland works from Cinnamon 6.8, which added layer-shell.

### How do I switch wallpapers with a keyboard shortcut?

Bind `fresco next`, `fresco prev` or `fresco random` to a key in your compositor or desktop settings — they switch the wallpaper without opening the app (`random` never picks the one already playing). They step through your library in the order the app shows it, or through just the folder you have open there, and start the Fresco service if it is not running. Inside the app, Ctrl+K opens a command palette. For example in Sway:

```
bindsym $mod+Right  exec fresco next
bindsym $mod+Left   exec fresco prev
bindsym $mod+Return exec fresco random
```

### Can I use a different wallpaper on each monitor?

Yes. Fresco supports per-display wallpapers, and when the same video is used across several monitors, playback is kept in sync.

### Does it support GIFs and image slideshows?

Yes — animated GIFs, static images, image slideshows with transitions (crossfade, fade, slide, Ken Burns), and multi-video playlists, in addition to video files.

### Can I use a YouTube video as my wallpaper?

Not by pasting its link. **From link** accepts Pinterest pins and direct links to a video or image file (mp4, webm, mkv, mov, gif, jpg, png, webp); other links are rejected with a message. Add a video file you already have with **Add**, or paste a direct file link.

### Is Fresco free and open source?

Yes. Fresco is free and open source under GPL-3.0-or-later. There are no ads, no accounts, and no paid tier.

### Does Fresco collect my data?

Nothing is sent until you answer a one-time consent dialog. You choose between a daily anonymous headcount (random install id, country, app version) and that plus optional detail; setting `telemetry = false` in `~/.config/fresco/config.toml` sends nothing at all. See [Privacy & terms of use](#privacy--terms-of-use) for every field.

### What's the difference between Fresco and mpvpaper?

`mpvpaper` is a command-line mpv wrapper that plays one video as a Wayland layer-shell background — no GUI, no library, no X11 support. Fresco is a GTK4 desktop app built around it: a wallpaper library, crop/rotate editor, scheduling, multi-monitor sync, desktop widgets, and X11 support, with no terminal required. Fresco bundles `mpvpaper` as its Wayland renderer rather than replacing it — see the [comparison table](#fresco-vs-wallpaper-engine-hidamari-komorebi-and-mpvpaper).

### Can I use Fresco on Kali Linux?

Yes. Kali Linux is Debian-based, so Fresco installs the same way as on Debian or Ubuntu — with the `.deb` package or the one-line install script (see [Installation](#installation)). Kali's default desktop, Xfce on X11, is supported through Fresco's embedded X11 renderer.

### How do I remove several wallpapers at once?

Click **Select** in the footer (or right-click a wallpaper and choose **Select…**), tick the ones you want, and click **Remove**. **Select all** respects the current search, so you can search first and then clear a whole batch. Removing a wallpaper takes it out of your Fresco library — the source file on disk is kept.

### How do I uninstall Fresco?

If you installed the `.deb` or used the install script, run `sudo apt remove fresco`. Your library and config live in `~/.local/share/fresco/` and `~/.config/fresco/`; delete those directories to remove every trace. To get your desktop's own wallpaper back without uninstalling, open Fresco and click **Stop wallpaper**.

## Desktop widgets: lyrics, clocks, visualiser and album art

Fresco draws five optional widgets onto your wallpaper. All are **off by default**; turn them on from the app menu (Ctrl+,) → **Advanced…**.

| Widget | What you get |
|---|---|
| **Lyrics** | The current line, in time with the music. Four presets (Minimal, Karaoke, Subtitle, Card), a nine-point placement grid, a sync-offset slider, an optional dimmed next line, and optional track title and artist |
| **Clock** | Eight themes — Digital, Minimal, Segment, Stacked, Wordy, Card, NOS and a phone-style Lock screen face. 12- or 24-hour, optional date, colour and opacity controls. Seconds are off by default because they cost 60× the repaints |
| **World clock** | The time in several cities at once, picked from the full list of time zones, as plain 12-hour text (for example `Tokyo  9:41 PM`) with an optional label per city. Repaints once a minute. New in 1.1.50 |
| **Audio visualiser** | Five styles — Bars, Mirror, Wave, Dots, Ring — with a colour picker, a two-colour blend, or rainbow |
| **Album art** | The current track's cover on a turning record; it stops turning when playback pauses |

Except for the world clock's optional **Float above windows** mode, widgets are painted into the wallpaper through mpv's OSD layer rather than into a window of their own, so they never sit above your windows, never intercept a click, and behave identically on X11 and on every layer-shell compositor. With music playing and the lyrics, clock, visualiser and album-art widgets on, the measured cost was **0.8% of one CPU core** — nearly all of it the audio capture, since nothing repaints unless its content changed.

Widgets appear on **every display** by default. To keep them on one, add `monitor = "DP-1"` to the `[widgets]` block of `config.toml` (there is no GUI control for this yet). They are **not available on GNOME under Wayland**, which has no live wallpaper surface for Fresco to draw into — the same reason wallpapers fall back to a static frame there.

### Can I add a clock to my Linux desktop wallpaper?

Yes. Turn on the **Clock** widget for a themed clock drawn into the wallpaper, or the **World clock** to show several cities at once. The world clock sits on the wallpaper like the other widgets; its optional **Float above windows** mode lifts it above every app with clicks passing through, which works on X11 with a compositor and on layer-shell compositors (KDE Plasma, COSMIC, Sway, Hyprland), and falls back to the wallpaper where a session cannot float it, such as GNOME on Wayland.

### Can I show song lyrics on my Linux desktop?

Yes. Fresco draws **time-synced lyrics** onto your wallpaper, following whatever is playing on your system over MPRIS — browsers, music apps, video players. Pick a preset, placement and sync offset in the Lyrics widget settings (see the table above).

### Does Linux have desktop widgets like Conky?

Yes, and Fresco adds five that need no panel, no extension, and no support from your desktop: a clock, a world clock, synced lyrics, an audio visualiser, and a turning album-art disc. Because they are painted into the wallpaper instead of a window, they work on desktops that have no widget layer of their own — COSMIC, Hyprland and Sway included. Unlike Conky, Fresco has **no system-monitor widgets** (no CPU, RAM, temperature or network readouts), so it sits alongside Conky as a music-and-time companion rather than replacing it. GNOME on Wayland is the one place it can't run.

### Can I get a music visualiser on my desktop background?

Yes. Fresco's audio visualiser reacts to whatever your system is playing, in one of five styles — Bars, Mirror, Wave, Dots, or Ring — with a colour picker, a two-colour blend, or rainbow. It is off by default and asks for consent the first time you enable it, because it has to listen to your audio output; the consent is also enforced on config load, so editing `config.toml` by hand can't switch it on behind your back. Pair it with the album-art widget for a turning record of the current track's cover.

### Which music players work with the lyrics widget?

Anything that publishes standard MPRIS metadata, but they are not equally reliable:

| Player | Works? | Notes |
|---|---|---|
| **Firefox** | ✅ | The reliable choice, and what the feature was verified against |
| Chrome / Brave / Edge / Vivaldi / Opera | ⚠️ | See below |
| Spotify — **in a browser** | ✅ | Reports playback position correctly |
| Spotify — **native Linux client** | ⚠️ | Reports its position as 0 forever, so lyrics can't stay in sync |
| Local players (VLC, mpv, Rhythmbox, …) | ✅ | Also the case where a local `.lrc` file is most likely to exist |

Chromium-family browsers claim an MPRIS name the first time any tab plays media
and **never release it** — a long-standing Chromium bug. When playback ends, the
title clears but a stale "zombie" session is left on the bus, sometimes still
carrying artwork. Fresco ignores any player publishing no track title, which
skips those sessions, but Chromium's own reporting stays inconsistent enough
that Firefox is the browser to use for this.

Spotify's native Linux client has returned `Position: 0` and never emitted a
seek since 2018, across native and snap builds. Fresco detects that
behaviorally — three spaced-out zero readings while playing — and free-runs the
lyric clock from the track change instead, which drifts if you skip around.
Spotify in a browser has no such problem.

### Where do the lyrics come from?

A local `.lrc` file first: one sitting next to the audio file, or a matching one
in a lyrics folder you point Fresco at. That path works offline and is the best
match, because it's the file you chose.

When there is no local file — which is most of the time if you stream — Fresco
looks the track up on [LRCLIB](https://lrclib.net), a free community-run synced
lyrics database, and caches the answer under `~/.cache/fresco/lyrics` so the
same song is never fetched twice. That lookup sends the track's title, artist
and album to LRCLIB; nothing is sent unless the lyrics widget is on and the
track has no local file. Fresco does not host, own or license lyric content —
LRCLIB's entries are contributed by its users and LRCLIB states no license over
them, so Fresco fetches on demand and caches per-user rather than shipping a
lyrics database of its own.

Lyric timing in `.lrc` files is hand-made and often a little off. There is a
sync offset slider in the lyrics settings for exactly that.

## Troubleshooting

Run `fresco doctor` first: it reports your session, which renderer Fresco picked and where it came from, and whether the GNOME Xorg workaround applies. [docs/INSTALL.md](docs/INSTALL.md) covers more cases, including high CPU use. Include the output of `fresco doctor` when you file an issue.

### Fresco doesn't show up in the Deepin launcher after installing — why?

A known dde-launchpad issue: its post-install refresh doesn't pick Fresco up.
Run `killall dde-shell` (it restarts automatically) or log out and back in, and
the entry appears permanently. Fresco itself installs correctly — it's listed by
Deepin's own application manager, and its icon resolves in every installed
theme. If it happens on every reinstall on a machine that has had Fresco before,
run `sh scripts/dde-launcher-diag.sh` — it is read-only and prints a log worth
attaching to an issue.

### My desktop icons are hidden while the wallpaper plays on Deepin

Deepin 25 draws its wallpaper and its desktop icons into a single opaque
window, so a live wallpaper you can see is one stacked above that window —
there is no layer that sits between them. **Click the desktop and the icons
come back for ten seconds**, long enough to open what you were after; the
wallpaper returns after that, and another click buys another ten seconds. To
change the delay, set `dde_icon_peek_secs` in `~/.config/fresco/config.toml`
(`0` keeps the wallpaper on top always). Deepin's own wallpaper still shows
whenever Fresco is off or paused.

### Why does my MATE desktop background look dark after a crash?

To keep Caja's desktop icons on top of the live wallpaper, Fresco sets MATE's
desktop background to a near-black key colour (`#010101`) while it runs and
copies everything that isn't that colour — the icons — onto the wallpaper. Your
own background is saved first and put back when Fresco stops or is disabled. If
Fresco crashes before it can do that, the desktop stays dark; **start Fresco
again and it restores your background**. On an X server without the Composite
and Damage extensions, Fresco instead works as it does on
[Deepin](#my-desktop-icons-are-hidden-while-the-wallpaper-plays-on-deepin):
icons hidden, a click on the desktop brings them back.

### My wallpaper is black on Wayland (NVIDIA, COSMIC, Hyprland, Sway)

A black wallpaper — everything else working, no errors in the log — almost
always means the mpvpaper renderer that Fresco bundles is too old for your
setup. Versions before 1.6 initialise EGL, report success and then never
present a frame on the NVIDIA proprietary driver.

Run `fresco doctor` first. It prints the renderer it picked, where that binary
came from (bundled vs. one you installed) and roughly which version it is, and
it warns when that version predates the fix:

```
  ✓ mpvpaper available (/usr/lib/fresco/mpvpaper-libmpv2)
      source: bundled · version: 1.4–1.6
  ⚠ mpvpaper may be too old …
```

Fresco already prefers a newer `mpvpaper` on your `PATH` over an older bundled
one, so installing your distro's `mpvpaper` package (or building
[upstream](https://github.com/GhostNaN/mpvpaper) with `scripts/build-mpvpaper.sh`)
is usually enough. To point Fresco at a specific binary, set **`FRESCO_MPVPAPER`**
to its full path — this overrides every other choice:

```bash
mkdir -p ~/.config/environment.d
echo 'FRESCO_MPVPAPER=/home/YOU/.local/bin/mpvpaper' > ~/.config/environment.d/fresco.conf
```

Log out and back in (systemd reads `environment.d` at session start), then
confirm with `fresco doctor` that the `source:` line now says
`FRESCO_MPVPAPER override`.

Similarly, **`FRESCO_HWDEC`** overrides mpv's hardware decoder (e.g. `nvdec`,
`nvdec-copy`, `vaapi`, `no`) — a diagnostic for high CPU use. By default Fresco
picks `nvdec,vaapi,auto-safe` when an NVIDIA GPU is present, otherwise `auto-safe`
(rotated video uses copy-back: `nvdec-copy,auto-copy` / `auto-copy`).

## Privacy & terms of use

**Nothing is sent until you answer.** A consent dialog asks once on first launch, and the choice can be changed anytime in Settings.

Either way, once a day, Fresco records that one install was active, in which country, on which version. That is the headcount. What the dialog asks about is the **detail**:

| You choose | What Fresco sends |
| --- | --- |
| **Accept all** | The headcount, plus distro, desktop, session type, video backend, monitor count, which features you use, error kinds, city and region, and the exact time of each check-in. |
| **Decline optional** | The headcount only: a random install id (never derived from your hardware or name), country, app version, packaging. Your check-in is stored as a **date, not a time**. |

**City and the exact time of use are optional** — sent only if you accept all, never sent if you decline. Coordinates are never collected at any level: the geolocation endpoint drops latitude and longitude rather than returning them.

These numbers exist to decide which distros and desktops get tested before a release, and where downloads need a mirror. They are never sold, shared, or used for advertising, and there is no analytics vendor involved.

**Never collected, either way:** personal data, file names, your wallpapers, your IP address, audio, keystrokes, or clipboard contents. The country is resolved from your IP by Cloudflare at the network edge, so only a two-letter code ever reaches this project. (Text you deliberately write and press send on — a feedback comment, or a message to the maintainer below — is a message you chose to send, not collection.)

To send nothing at all, set `telemetry = false` and `telemetry_prompted = false` in `~/.config/fresco/config.toml`.

### Talk to the maintainer, anonymously

**Menu → Message the maintainer** opens a private two-way thread with the person who makes Fresco. No account, no email address, no GitHub login. It is anonymous in both directions: you never learn who they are beyond "the maintainer", and they never learn who you are — only your messages and, if you leave the box ticked, the setup summary shown in the dialog.

The thread is keyed by a random ticket generated separately from telemetry and stored in a different file, so a conversation can never be joined to a usage profile. It works exactly the same whether you accepted all or declined optional. Nothing exists until you send a first message.

📄 **[Full terms of use and privacy policy →](TERMS.md)** — every field, in a table, with nothing omitted. Every line that sends anything lives in [`src/telemetry.rs`](src/telemetry.rs), so you can check rather than trust.

## Project facts

| | |
|---|---|
| **Name** | Fresco (application ID `io.github.dibbayajyotiroy.Fresco`) |
| **What it is** | A live and video wallpaper app for the Linux desktop |
| **License** | [GPL-3.0-or-later](LICENSE) — free and open source, no ads, no account |
| **Language and stack** | Rust, GTK4 / libadwaita, libmpv; a bundled mpvpaper renderer on Wayland |
| **First release** | v0.0.1, June 2026 |
| **Latest version** | 1.1.51 (2026-10-23) — see [Releases](https://github.com/DibbayajyotiRoy/fresco/releases/latest) for the newest |
| **Packages** | `.deb` (amd64), one-line install script, Deepin App Store; source build elsewhere |
| **Interface languages** | English plus 12 translations: Simplified Chinese, Russian, Spanish, French, German, Portuguese (Brazil), Turkish, Vietnamese, Indonesian, Hindi, Bengali and Romanian |
| **Users** | 2,500+ people in 127 countries |
| **Maintainer** | Dibbayajyoti Roy |
| **Source code** | <https://github.com/DibbayajyotiRoy/fresco> |
| **Website** | <https://fresco.dibbayajyoti.com> |

## Contributing & feedback

Bug reports, feature requests, and PRs are welcome — open an [issue](https://github.com/DibbayajyotiRoy/fresco/issues), or use the in-app feedback dialog. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[GPL-3.0-or-later](LICENSE) — free and open source.

---

<sub>Fresco is a free, open-source live wallpaper and video wallpaper app for Linux (X11 and Wayland), with lyrics, clock, audio-visualiser and album-art widgets drawn into the wallpaper. It is a native Linux alternative to Wallpaper Engine and a music-and-clock companion to tools like Conky. Used by 2,500+ people in 127 countries. Last updated: 2026-10-23.</sub>

# Fresco — Competitive Audit: Linux Live Wallpaper Tools

> Research date: June 2026 | Target system: Debian/Ubuntu/Pop!_OS (GNOME, X11)

---

## Why this gap exists

Windows and macOS users have polished GUI apps for live wallpapers — **Wallpaper Engine** (Steam, $3.99) and **Lively Wallpaper** (free, open-source) — that require zero terminal interaction. Linux users have no equivalent. Every option either requires the terminal, is abandoned, is GNOME-version-locked, or breaks under GNOME's compositor. Fresco fills this gap.

---

## Top 10 existing tools — detailed analysis

### 1. Hidamari ⭐ Best existing option
**Project:** [github.com/jeffshee/hidamari](https://github.com/jeffshee/hidamari)  
**Stack:** Python + GTK3/4 + VLC backend  
**Distribution:** Flatpak on Flathub (excellent), unofficial AUR  
**Last release:** v3.6 — January 2025 (active)

| | |
|---|---|
| GUI | ✓ (simple, functional) |
| X11 + GNOME | ✓ |
| Wayland | Partial (GNOME Wayland experimental) |
| Hardware decode | VLC-based; limited VA-API; broken on NVIDIA+Wayland |
| Video quality | Good for 1080p; some reports of color issues |
| Multi-monitor | ✓ |
| Playlist | ✗ |
| Drag-to-crop | ✗ |
| Wallpaper library | ✗ |
| Autostart | ✓ |

**Verdict:** Closest to Fresco's UX goal. Weaknesses: VLC backend has worse hwdec story than mpv; no playlist, no crop, no library. Python + Flatpak means higher startup RAM. Fresco differentiates on all three missing features plus better hwdec.

---

### 2. Hanabi (GNOME Shell Extension)
**Project:** [github.com/jeffshee/gnome-ext-hanabi](https://github.com/jeffshee/gnome-ext-hanabi)  
**Stack:** GNOME Shell extension + GStreamer / Clapper  
**Distribution:** GNOME Extensions website  
**Last update:** 2023–2024

| | |
|---|---|
| GUI | Via GNOME extension preferences |
| X11 + Wayland | ✓ both |
| Hardware decode | GStreamer VA-API; high CPU without Clapper installed |
| Video quality | Good when Clapper is installed |
| GNOME version lock | ✓ (must match Shell version) |
| Playlist | ✗ |
| Drag-to-crop | ✗ |
| Non-GNOME DEs | ✗ |

**Verdict:** Elegant GNOME-only solution. Main weaknesses: locks you to one GNOME major version; breaks every Ubuntu LTS upgrade until a new version ships; very high CPU on proprietary NVIDIA without Clapper. Note: the dev machine already has Hanabi installed — Fresco supersedes it.

---

### 3. mpvpaper
**Project:** [github.com/GhostNaN/mpvpaper](https://github.com/GhostNaN/mpvpaper)  
**Stack:** C + libmpv + wlroots layer-shell  
**Distribution:** AUR, nixpkgs; manual build elsewhere  
**Wayland only**

| | |
|---|---|
| GUI | ✗ (terminal only) |
| X11 | ✗ (Wayland/wlroots only — Sway, Hyprland, etc.) |
| Hardware decode | Excellent (libmpv `hwdec=auto-safe`) |
| Video quality | Best-in-class |
| Ease of use | Poor — command-line flags only |

**Verdict:** Best video quality of any Linux option, but terminal-only and Wayland-only. Not usable on Pop!_OS X11 session. Fresco inherits the same mpv quality story while adding GUI and X11 support.

---

### 4. linux-wallpaperengine (Almamu)
**Project:** [github.com/Almamu/linux-wallpaperengine](https://github.com/Almamu/linux-wallpaperengine)  
**Stack:** C++ + OpenGL  
**Purpose:** Run Steam Wallpaper Engine `.pkg` assets on Linux

| | |
|---|---|
| GUI | Third-party (jagrat7/linux-wallpaper-engine) |
| GNOME X11 | ✗ — **does not work if GNOME draws the background** |
| Hardware decode | GPU-based OpenGL rendering |
| Requires | Steam + Wallpaper Engine purchase |
| Ease of use | Poor — significant setup, GNOME incompatibility |

**Verdict:** Niche use case (Steam asset replay). Incompatible with GNOME/mutter by design. Fresco targets users who don't own Wallpaper Engine and just have local video files.

---

### 5. xwinwrap + mpv
**Technique:** `xwinwrap -ov -fs -s -st -sp -b -nf -- mpv --wid=%WID ...`  
**Stack:** Xorg utility + mpv

| | |
|---|---|
| GUI | ✗ (terminal script) |
| Video quality | Excellent (mpv) |
| Ease of use | Very poor — multi-step, breaks with compositor changes |
| GNOME ding coexistence | Problematic — window stacking unreliable |
| Maintenance | xwinwrap abandoned; various forks |

**Verdict:** The "DIY" approach that works for technically savvy users but is completely inaccessible to everyone else. Fresco replaces this entirely with a proper daemon that handles stacking correctly.

---

### 6. Komorebi
**Project:** [github.com/christianloopp/komorebi](https://github.com/christianloopp/komorebi) (fork of cheesecakeufo/komorebi)  
**Stack:** Vala + GTK  
**Status:** Original abandoned 3+ years; fork has infrequent updates

| | |
|---|---|
| GUI | ✓ (dated but functional) |
| Video quality | Limited (GStreamer) |
| Content types | Video, particle effects, custom images |
| Wayland | ✗ |
| Active maintenance | ✗ effectively |

**Verdict:** The original "easy GUI" option that died. Fork exists but stagnant. Many users search for alternatives. Fresco is the modern replacement.

---

### 7. Waypaper + swww / swaybg
**Projects:** [github.com/anufrievroman/waypaper](https://github.com/anufrievroman/waypaper)  
**Stack:** Python + swww/swaybg/hyprpaper backends

| | |
|---|---|
| GUI | ✓ (static images only via GUI) |
| X11 | ✗ (Wayland only) |
| Video | ✗ (static images only) |
| Ease of use | Good for static wallpapers |

**Verdict:** Good static-image solution for Wayland/Hyprland users. Not relevant to X11/GNOME. No video support.

---

### 8. Variety
**Project:** [github.com/varietywalls/variety](https://github.com/varietywalls/variety)  
**Stack:** Python + GTK  
**Distribution:** Ubuntu/Debian repos (`apt install variety`)

| | |
|---|---|
| GUI | ✓ (polished) |
| Video | ✗ (static only) |
| Auto-rotate | ✓ (online sources: Wallhaven, Unsplash, Flickr) |
| GNOME | ✓ |
| Ease of use | Excellent |

**Verdict:** Best static-wallpaper-changer on Linux. No video support. Fresco covers the video/live use case that Variety explicitly doesn't address.

---

### 9. Lively Wallpaper (Linux port)
**Project:** [github.com/rocksdanister/lively-linux](https://github.com/rocksdanister/lively-linux)  
**Stack:** .NET / Avalonia (in progress)

| | |
|---|---|
| Status | Experimental — "wallpapers do not work yet" |
| GUI | Basic UI works |
| Linux usable | ✗ not yet |

**Verdict:** Watch space; may become relevant in 1–2 years. Currently not usable.

---

### 10. GNOME Wallpaper Engine extension (achu94 / Techlm77)
**Projects:** Several small extensions with ≤10 commits  
**Stack:** Python + GStreamer

| | |
|---|---|
| Video quality | GStreamer (no VA-API config) |
| Ease of use | Medium (drop file at specific path) |
| Maintenance | Minimal — small personal projects |

**Verdict:** Fills a quick-and-dirty need but not maintained at production quality.

---

## Summary comparison

| Tool | GUI | X11+GNOME | Video quality | Playlist | Crop | Library | Maintained |
|------|-----|-----------|---------------|----------|------|---------|------------|
| **Fresco** (this) | ✓ | ✓ | mpv/hwdec | ✓ | ✓ | ✓ | ✓ |
| Hidamari | ✓ | ✓ | VLC/ok | ✗ | ✗ | ✗ | ✓ |
| Hanabi | ext. | GNOME only | GStreamer | ✗ | ✗ | ✗ | slow |
| mpvpaper | ✗ | Wayland only | mpv/excellent | ✓ | ✗ | ✗ | ✓ |
| linux-wallpaperengine | 3rd-party | ✗ | OpenGL | ✓ | ✗ | ✗ | ✓ |
| xwinwrap+mpv | ✗ | ✓ | mpv/excellent | manual | ✗ | ✗ | abandoned |
| Komorebi | ✓ | ✓ | GStreamer | ✗ | ✗ | ✗ | ✗ |
| Variety | ✓ | ✓ | static only | ✗ | ✗ | ✓ | ✓ |

---

## Fresco's differentiation

1. **Zero terminal** — install from .deb, launch from app menu, pick a video, click Set.
2. **mpv backend** — same video quality and hwdec path as mpvpaper, the gold standard.
3. **Drag-to-crop** — no other Linux live wallpaper tool has an interactive crop editor.
4. **Playlist mode** — cycle multiple videos on a loop (Hidamari and Komorebi don't have this).
5. **Wallpaper library** — saved thumbnails, one-click switching, recent row.
6. **Survives login** — detached daemon + XDG autostart, configurable.
7. **Supports all DEs** — anything running X11 (GNOME, XFCE, MATE, Cinnamon, i3, …).
8. **Hardware decode that works on NVIDIA hybrid laptops** — `hwdec=auto-safe` with clear status, not hidden.

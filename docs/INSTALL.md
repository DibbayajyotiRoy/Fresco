# Installing Fresco

Fresco runs on Debian-based distributions (Pop!_OS, Ubuntu, Linux Mint, Debian,
elementary OS) running an **X11** or **Wayland** session.

- **X11:** full live wallpapers (embedded mpv).
- **Wayland layer-shell compositors** (COSMIC, Hyprland, Sway): live
  wallpapers via the bundled `mpvpaper` backend.
- **KDE Plasma 6** (X11 or Wayland): Fresco's Plasma wallpaper plugin, set through
  plasmashell (see [X11 vs Wayland](#x11-vs-wayland)).
- **GNOME Wayland:** still frame only (Mutter has no live wallpaper surface). See
  [X11 vs Wayland](#x11-vs-wayland) for what that means on your release.

## Quick install (one-liner)

```bash
curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | bash
```

This detects your distro and session, downloads the latest `.deb` from GitHub
Releases, installs it (dependencies resolved automatically), and points you at
the next step. Re-running it upgrades an existing install.

## Manual install

1. Download `fresco_<version>_amd64.deb` from the
   [latest release](https://github.com/DibbayajyotiRoy/fresco/releases/latest).
2. Install it by double-clicking in your file manager, or:

   ```bash
   sudo apt install ./fresco_*.deb
   ```

Then launch **Fresco** from your application menu (or run `fresco`).

## Other distributions (Fedora, Arch, CachyOS, EndeavourOS)

Build Fresco from source with `cargo build --release --all-features`. A source
build does **not** include the `mpvpaper` renderer the `.deb` bundles, and
Wayland live wallpapers need it. Install it separately:

```bash
# Arch / CachyOS / EndeavourOS (AUR)
yay -S mpvpaper

# Fedora and anything else: build it against your system libmpv
scripts/build-mpvpaper.sh
install -Dm755 target/release/mpvpaper ~/.local/bin/mpvpaper   # must be on PATH
```

If Fresco was already running, there is no need to restart it: playback starts
by itself within a few seconds of `mpvpaper` appearing. `fresco doctor` shows
which copy it found.

## Optional: hardware-accelerated decoding

Fresco plays video through your GPU when a VA-API/NVDEC driver is present, which
keeps CPU usage near zero. If `frescod --check` reports software decoding,
install the driver for your GPU:

```bash
# Intel (Skylake / Gen8 and newer)
sudo apt install intel-media-va-driver

# AMD, or older Intel via Mesa
sudo apt install mesa-va-drivers

# NVIDIA — the proprietary driver provides NVDEC; install it from
# Software & Updates → Additional Drivers (or your distro's driver tool)
```

## Diagnostics

If something isn't working, run:

```bash
frescod --check
```

It prints your session type, backend capability, mpvpaper availability (on
Wayland), the libmpv version in use, detected GPUs, VA-API availability, config
validity, and the live daemon status. Include this output when filing a bug
report.

## X11 vs Wayland

Run:

```bash
echo $XDG_SESSION_TYPE     # x11 or wayland
```

- **X11:** everything works out of the box.
- **Wayland layer-shell compositors** (COSMIC, Hyprland, Sway): live
  wallpapers work out of the box using the bundled `mpvpaper` backend.
- **KDE Plasma 6 (X11 and Wayland):** plasmashell paints the desktop and its
  icons into one opaque window, so Fresco sets its own Plasma wallpaper plugin
  on every desktop instead of opening a window. Install
  `qml6-module-qtmultimedia` (Debian/Ubuntu) for video; without it you get a
  still frame. Playback is Qt's: muted, no hwdec tuning, crop or transitions;
  a playlist plays its first file, a slideshow shows its first frame. Set
  `FRESCO_KDE_DESKTOP=0` to use the window backend instead (it only shows in
  the Overview). `frescod --check` prints what plasmashell currently shows;
  the log is `~/.local/state/fresco/frescod.log`.
- **GNOME Wayland:** Fresco sets a still frame as the desktop background and
  says so in the app (the status pill reads "STILL FRAME"). Whether live video is
  possible depends on your release:
  - **Ubuntu 22.04 / 24.04 and other releases that still ship an Xorg session:**
    log out and choose the **Xorg** session on the login screen (e.g. "Pop (on
    Xorg)" or "Ubuntu on Xorg") for full live playback.
  - **GNOME 50, Ubuntu 25.10 and newer (including 26.04 LTS), Fedora 43 and
    newer:** there is no Xorg session to choose. GNOME 49 disabled it and GNOME
    50 removed it. Live video on GNOME Wayland needs a Fresco GNOME extension,
    which is planned; until then use a desktop with live support (KDE Plasma,
    COSMIC, Hyprland, Sway, or any X11 desktop).

  `fresco doctor` shows your GNOME Shell version and whether an Xorg session is
  installed, and gives the matching advice.

## FAQ / troubleshooting

**The wallpaper doesn't appear.**
Confirm you're on X11 (`echo $XDG_SESSION_TYPE`) and run `frescod --check`. If
the daemon isn't running, re-open Fresco and set a wallpaper again.

**CPU usage is high.**
You're probably on software decoding. Install the VA-API driver for your GPU
(see above) and re-apply the wallpaper. Verify with `frescod --check`.

**The wallpaper is gone after a reboot.**
Open Fresco → menu → enable **Restore on login**. (It's on by default the first
time you set a wallpaper.)

**A library item shows a ⚠ badge.**
The source file was moved or deleted. Re-add it, or remove the entry.

**Deepin: no icon in the launcher after installing.**
Fresco installed correctly — Deepin's own application manager lists it and its
icon resolves in every installed theme; only `dde-launchpad` has not refreshed.
Run `killall dde-shell` (it restarts itself), or log out and back in, and the
entry appears permanently. It happens at most once per install and nothing
recurs afterwards. If you have installed Fresco on this machine before and it
keeps happening, run `sh scripts/dde-launcher-diag.sh` (read-only) and attach
the output to an issue — that is the case we are still tracking.

**I want the native desktop wallpaper back.**
Open Fresco and click **Stop** — this reveals your desktop environment's normal
wallpaper and keeps it stopped across reboots until you set a new one.

## Uninstall

```bash
sudo apt remove fresco
```

Your library and config live in `~/.local/share/fresco/` and
`~/.config/fresco/`; delete those directories to remove all traces.

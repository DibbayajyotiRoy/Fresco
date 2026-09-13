# Installing Fresco

Fresco runs on Debian-based distributions (Pop!_OS, Ubuntu, Linux Mint, Debian,
elementary OS) running an **X11** or **Wayland** session.

- **X11:** full live wallpapers (embedded mpv).
- **Wayland layer-shell compositors** (COSMIC, Hyprland, Sway, KDE Plasma 6): live
  wallpapers via the bundled `mpvpaper` backend.
- **GNOME Wayland:** static-frame fallback (Mutter has no live wallpaper surface).

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
- **Wayland layer-shell compositors** (COSMIC, Hyprland, Sway, KDE Plasma 6): live
  wallpapers work out of the box using the bundled `mpvpaper` backend.
- **GNOME Wayland:** Fresco sets a static frame as the desktop background. For
  full live playback on GNOME, log out and choose the **Xorg** session on the
  login screen (e.g. "Pop (on Xorg)" or "Ubuntu on Xorg").

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

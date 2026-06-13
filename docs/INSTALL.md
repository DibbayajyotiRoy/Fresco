# Installing Fresco

Fresco runs on Debian-based distributions (Pop!_OS, Ubuntu, Linux Mint, Debian,
elementary OS) running an **X11** session.

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

It prints your session type, the libmpv version in use, detected GPUs, VA-API
availability, config validity, and the live daemon status. Include this output
when filing a bug report.

## X11 vs Wayland

Fresco currently requires an **X11** session. To check:

```bash
echo $XDG_SESSION_TYPE     # should print: x11
```

If it prints `wayland`, log out, and on the login screen click the gear/settings
icon and choose the **Xorg** session (e.g. "Pop (on Xorg)" or
"Ubuntu on Xorg"), then log back in. Wayland support is planned for a future
release.

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

**I want the native desktop wallpaper back.**
Open Fresco and click **Stop** — this reveals your desktop environment's normal
wallpaper and keeps it stopped across reboots until you set a new one.

## Uninstall

```bash
sudo apt remove fresco
```

Your library and config live in `~/.local/share/fresco/` and
`~/.config/fresco/`; delete those directories to remove all traces.

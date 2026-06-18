# Fresco Wayland Verification — Agent C Report

## VERIFICATION BLOCKED
**Reason: Not running under Wayland.**

Halted at First Actions per protocol. No rendering test (T1–T22) was executed,
because no Wayland session, compositor output, `grim`, or `mpvpaper` is present.

## Environment (evidence: `environment.json`, `logs/`)

| Field | Value |
|---|---|
| timestamp | 2026-06-16T02:54:25Z |
| hostname | pop-os |
| kernel | Linux 6.17.9-76061709-generic |
| session_type | **x11** ← blocker |
| wayland_display | **unset** ← blocker |
| compositor | pop:GNOME |
| desktop_session | pop |
| fresco_version | 0.0.3 |
| commit | 93572f2 |
| mpvpaper_version | **not found** |

`fresco doctor` independently reports `Session: X11`, `Backend: X11 (embedded mpv)`.
`fresco status`: not running. All three agree: this host cannot exercise Wayland.

## Evidence-integrity findings (adversarial)
1. **Not a Wayland host.** Hyprland / KDE Plasma 6 / Sway are all required and none
   is present. Nested compositors are disallowed by this prompt unless instructed.
2. **mpvpaper absent.** The Wayland render backend binary is not installed/bundled
   on this host, so even a forced run would fail to render.
3. **Code under test is uncommitted.** HEAD is `93572f2`; the Wayland implementation
   exists only as dirty working-tree changes. Verification must be run against a
   committed, identified revision.

## Test ledger

| Tests | Result |
|---|---|
| T1, T1A, T2, T3, T4, T5 (on-screen), T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T17, T21, T22 | **NOT TESTED — BLOCKED** (no Wayland session) |
| Resource metrics (CPU/RAM/GPU) | **NOT TESTED — BLOCKED** |
| T16 GNOME detection | NOT TESTED on Wayland (host is X11) |

Note: daemon-logic unit/integration tests (pause/crop IPC, backend-death detection,
graceful failure) pass in `cargo test`, but **they are not on-Wayland artifacts** and
do **not** count toward Wayland proof under this mandate.

## To unblock
Run on a real Hyprland, KDE Plasma 6, and Sway session, each with mpvpaper present:
```
fresco doctor                       # must show Backend: mpvpaper (layer-shell)
FRESCO_WAYLAND=1 tests/wayland/verify.sh <video.mp4>
```
Store the resulting screenshots, logs, and `report.json` per compositor. Re-run
Agent C against those artifacts.

---

# WAYLAND SUPPORT: NOT PROVEN

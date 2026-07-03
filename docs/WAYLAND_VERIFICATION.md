# Fresco — Wayland Verification Spec & Evidence Ledger

> Standard: **assume broken until proven.** No "works on my machine." Only
> reproducible proof with artifacts (timestamp, commit, compositor + mpvpaper
> versions, logs, screenshots, metrics).

This file is the source of truth for what "Fresco supports Wayland" means and
what has actually been proven. The release gate is defined at the bottom.

## How to produce rendering proof

The rendering tests (T1, T2, T9–T22) **require a real Wayland compositor with a
GPU**. They cannot be produced on an X11/GNOME developer box. Run:

```sh
# On a Hyprland / Sway / KDE Plasma 6 session (or headless Sway, see CI note):
tests/wayland/verify.sh /path/to/test-video.mp4
```

The script provisions the backend, applies the wallpaper, captures `grim`
screenshots + logs + CPU samples, and writes `artifacts/<compositor>-<ts>/report.json`
with the full evidence header. It exits non-zero if any automated check fails.

Headless CI note: Sway runs headless on a stock Linux runner via
`WLR_BACKENDS=headless WLR_RENDERER=pixman`, screenshotted with `grim` — suitable
for a GitHub Actions job (build mpvpaper in the job, run `verify.sh`). Hyprland
and KDE Plasma 6 need a GPU VM runner; document the same `verify.sh` invocation.

## Evidence ledger (commit-scoped — update every run)

| # | Test | Severity | Status (this commit) | Evidence |
|---|------|----------|----------------------|----------|
| T3 | Pause/resume (control) | HIGH | **PROVEN** | `cargo test ipc_steers_real_mpv` — pause round-trips on real mpv |
| T4 | Crop controls (control) | HIGH | **PROVEN** | same test — `video-zoom` reads back the set value |
| T6 | Backend death detected/restart | BLOCKER | **PROVEN (Sway, end-to-end)** | killed mpvpaper on live headless Sway; frescod respawned it (pid 224482→224837). Also `mpvpaper_supervision_primitives` unit test. |
| T7 | Crash-loop backoff | BLOCKER | **PROVEN (logic)** | bounded 2s restarts + "giving up at MAX_RESTARTS" observed; no tight loop. On-screen behavior pending compositor. |
| T8 | Invalid/missing media | HIGH | **PROVEN** | same test — missing backend → `Err`, no panic; daemon stays up |
| T16 | GNOME detection | — | **PROVEN** | `capability::tests` — Wayland+GNOME → static fallback |
| — | X11 / IPC / config regression | — | **PROVEN** | full `cargo test` (15/15), X11 path code unchanged |
| T1 | **Wallpaper visible** | BLOCKER | **PROVEN on Sway** (Hyprland/KDE pending) | `verification-artifacts/sway/clean-frame1.png` — controlled testsrc color bars rendered via mpvpaper layer-shell on headless Sway (Intel GPU), grim-captured |
| T1A | **Wallpaper animated** | BLOCKER | **PROVEN on Sway** | two grim frames differ; moving rainbow strip confirms live playback |
| T2 | **Click-through** | BLOCKER | **UNPROVEN** | needs compositor |
| T5 | Slideshow advance on screen | HIGH | **UNPROVEN** | IPC `loadfile` works; on-screen transition needs compositor |
| T9 | Dual monitor (`mpvpaper ALL`) | — | **UNPROVEN** | needs 2-output compositor |
| T10 | Hotplug | BLOCKER | **UNPROVEN — NOT IMPLEMENTED** | output reconciliation is Phase 3; expected to fail until then |
| T11 | Resolution change | — | **UNPROVEN — NOT IMPLEMENTED** | Phase 3 |
| T12 | Login autostart | — | **UNPROVEN** | needs session |
| T13 | Logout clean shutdown | — | **UNPROVEN** | needs session |
| T14 | Compositor restart | BLOCKER | **UNPROVEN** | needs compositor |
| T15 | Suspend/resume | BLOCKER | **UNPROVEN** | needs hardware |
| T16 | GNOME detection | — | **PROVEN (runtime)** | daemon logged `GNOME Wayland static-frame mode` on the live GNOME-Wayland session |
| T17 | GNOME restore on stop | BLOCKER | **PROVEN (runtime)** | GNOME-Wayland: daemon log shows apply→`overview background restored`→stop; bg back to original, state file deleted |
| T18 | Idle CPU < 1% | — | **UNPROVEN** | needs running wallpaper; `verify.sh` samples it |
| T19 | Memory / RSS | — | **UNPROVEN** | needs running wallpaper |
| T20 | GPU usage | — | **UNPROVEN** | needs GPU |
| T21 | 8-hour run | — | **UNPROVEN** | needs soak run |
| T22 | 24-hour run | — | **UNPROVEN** | needs soak run |

## Test definitions

(Severity and pass/fail per the mandate. T1–T22 method summarized; `verify.sh`
automates T1, T6, T18; the rest are procedures — automate as runners are added.)

- **T1 Wallpaper visible** — apply video; two `grim` frames 1s apart must (a) not
  be uniform/black (stddev > threshold) and (b) differ from each other (animated).
- **T2 Click-through** — wallpaper surface input region must be empty; a click on
  the desktop must not be captured. (`mpvpaper` sets an empty input region.)
- **T6 Kill mpvpaper** — kill backend; within one supervise interval a new backend
  is spawned and the IPC socket returns.
- **T7 Crash loop** — backend that exits immediately must restart with backoff and
  stop at `MAX_RESTARTS`; daemon CPU must not spike.
- **T14 Compositor restart** — backend dies with the compositor; on restart,
  autostart/daemon respawns or falls back cleanly. No orphaned state.
- **T15 Suspend/resume** — wallpaper visible after wake (re-capture not black).
- **T17 GNOME restore** — on GNOME-Wayland, stopping restores the original
  `org.gnome.desktop.background` (`picture-uri`/`-dark`).

## Release gate

Release is **BLOCKED** unless ALL hold:
- Hyprland: all BLOCKER + HIGH pass
- KDE Plasma 6: all BLOCKER + HIGH pass
- Sway: all BLOCKER + HIGH pass
- No BLOCKER fails; evidence artifacts stored for each run.

**Current gate state: BLOCKED (but materially advanced).** First real on-screen
proof captured: **Sway** passes T1, T1A, T6 with artifacts; **GNOME Wayland**
passes T16, T17 at runtime. Remaining for the gate: the same runs on **Hyprland**
and **KDE Plasma 6**; plus T2 (click-through), T14 (compositor restart), T15
(suspend), and T9/T18–T22. T10/T11 remain unimplemented (Phase 3). A real
`background=#000000` bug was found via this testing and fixed.

## Fidelity + audio rows (added 2026-07-03, tests/fidelity + tests/audio)

| ID | Claim | Status | Evidence |
|----|-------|--------|----------|
| F1 | 4K source pixel-exact on 4K output @ scale 1 (sway + x11) | PROVEN | crispness 100.0 both legs; verification-artifacts/fidelity-20260703-135907, -140044 |
| F2 | Buffer = physical pixels @ integer scale 2 (sway) | PROVEN | capture 3840x2160, crispness 100.0 |
| F2b | Fractional scale 1.25 crispness | KNOWN-LIMITED (76.5) | compositor resamples mpvpaper buffer; fix = native backend (ROADMAP 5.1) |
| F3 | No added gradient banding; dithering active | PROVEN | 256 unique levels (source 220), both legs |
| F4 | 8K→4K downscale quality ≥ 0.70 SSIM vs lanczos ref | PROVEN | 0.737 both legs (pre-fix baseline: 0.54) |
| A1 | Unmuted apply selects audio track (aid/mute/volume) | PROVEN (headless) | tests/audio L1, sway + x11 |
| A2 | Live unmute via Apply restores audio | PROVEN (headless) | tests/audio L2, sway + x11 |
| A3 | Cold-boot: track auto-restored after audio server appears | PROVEN (headless) | tests/audio L3 with FRESCO_EXPECT_LATE_AUDIO=1; real-login listen test pending (Roy) |

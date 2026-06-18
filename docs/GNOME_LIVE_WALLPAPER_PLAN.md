# Fresco — GNOME Wayland live wallpaper (better-than-Hanabi plan)

## Goal
Real live video wallpaper on GNOME Wayland (the majority desktop) that beats the
Hanabi-class extensions on **time** (CPU/GPU/battery), **space** (RSS/VRAM), and
**UX** — while never risking the user's session.

## The key insight
You don't beat Hanabi at the pixel level — you beat it on **policy**. Hanabi-class
extensions decode the video **continuously, per-monitor, regardless of whether
anyone can see it.** The wins are:

1. **Decode only when visible** (occlusion / overview / lockscreen / idle aware).
2. **Decode once, present to N monitors** (shared texture, not N pipelines).
3. **Pause/throttle on battery & power-saver.**
4. **Free decode buffers after sustained invisibility** (reclaim RAM/VRAM).

GNOME Shell *knows* the window stacking, workspace, overview, idle and lock
state — so the extension can drive these policies precisely. That knowledge is
exactly what a standalone tool (mpvpaper) lacks and what makes the GNOME path,
done right, the **most efficient** backend, not the worst.

## Honest constraint that shapes the design
A direct `dmabuf → Cogl texture → background actor` path (your diagram's ideal)
is **not cleanly exposed to a gjs extension** — Mutter does dmabuf import for
*client windows*, but there's no stable GI API to wrap an arbitrary dmabuf fd as
a Cogl texture from an extension. So we keep the **proven compositing substrate**
(a renderer surface that Mutter imports for free, cloned behind the desktop), and
put all the intelligence *around* it. We get hardware decode + zero-copy *into
the renderer* via Mutter, without an exotic, fragile texture path.

## Architecture

```
┌── fresco daemon (Rust) — the BRAIN ──────────────────────────────┐
│  owns: config (which video, fit/crop), battery policy, the       │
│  library, transitions; talks to the renderer + extension.        │
└───────────────┬───────────────────────────────┬─────────────────┘
                │ control (D-Bus/socket)         │ dconf (which video, fit)
                ▼                                 ▼
┌── fresco-gnome-renderer (crash-isolated) ──┐   ┌── fresco@… Shell extension (gjs) ──┐
│  minimal GStreamer pipeline:               │   │  THIN. Per monitor: one Clutter    │
│   filesrc→decode(HW: VA-API/NVDEC)→sink     │   │  actor in the background layer =   │
│  ONE decode for the chosen video.          │   │  a Clone of the renderer surface.  │
│  Presents a surface Mutter imports         │──►│  Owns the VISIBILITY POLICY:       │
│  (one hidden output the shell clones).     │   │  occlusion / overview / idle /     │
│  Obeys play/pause/seek/load from control.  │◄──│  lock / battery → pause/resume the │
│  No GTK chrome; smallest viable surface.   │   │  renderer. Crossfade on switch.    │
└────────────────────────────────────────────┘   └────────────────────────────────────┘
        crash here → respawn; desktop safe                composites; never decodes
```

- **Renderer** = a tiny dedicated process (one decode, hardware path, smallest
  surface). Crash-isolated: if it dies, the extension respawns it; the shell is
  never at risk.
- **Extension** = thin compositor glue **+ the policy engine**. It never decodes.
  It clones the renderer surface onto **N** monitor background actors (decode
  once → present many) and gates the renderer on visibility/power.
- **Daemon** = the brain and single source of truth (library, config, transitions,
  battery). The extension reads "which video / fit" from dconf and reports nothing
  back it doesn't have to.

## Why this is better — complexity per state

Let R = decode+upload cost (∝ fps × resolution), M = monitor count, K = cheap
composite of one texture.

| State (typical desktop) | Hanabi-class | Fresco GNOME backend |
|---|---|---|
| Visible, 1 monitor | `R` | `R` (tie) |
| Visible, M monitors, same video | `M·R` (pipeline/monitor) | `R + M·K` ✅ decode once |
| **Fully covered** (maximized app — most of the day) | `R` (still decoding!) | **~0** ✅ decode paused |
| In Activities overview | `R` | gated (pause or 1 frame) ✅ |
| Screen locked / DPMS off / idle | `R` | **~0** ✅ |
| On battery / power-saver | `R` | **~0 or static frame** ✅ |
| RSS/VRAM | `~M ×` (GTK renderer + pipeline per monitor) | `~1 ×` + cheap clones; **buffers freed** after sustained invisibility ✅ |

The dominant real-world saving is **"fully covered"** and **"on battery"** — that's
where laptops actually live. Hanabi burns a decoder there; we burn nothing.

## The policy engine (the actual IP) — a small state machine in the extension

Inputs (all available to a Shell extension): `global.display` window stacking +
opacity (is a monitor fully occluded?), `Main.overview` visible, `Main.screenShield`
locked, the session idle signal, monitor DPMS/`power-save`, and UPower/`power-profiles`
(on battery / power-saver). Output: per-monitor renderer state.

```
ACTIVE (decode @ fps)
  ── monitor fully occluded ──────────────► IDLE_PAUSE (renderer paused, last frame shown)
  ── overview opened ─────────────────────► throttle to ~1fps or hold a frame
  ── on battery / power-saver ────────────► STATIC (hold a frame, decoder torn down)
  ── locked / DPMS off / session idle ────► SUSPEND (decoder torn down; 0 cost)
  ── sustained pause > N s ───────────────► free decode buffers (reclaim RAM/VRAM)
reveal / on-AC / unlock ───────────────────► resume (warm if buffers kept, else re-open)
```

This is what makes Fresco "install-and-forget": months later it's still there,
and it has cost the user ~nothing whenever they couldn't see it.

## UX wins over Hanabi

- **One app for everything.** The user uses Fresco's GUI; the extension is an
  invisible implementation detail Fresco installs and drives. No `extensions.app`,
  no dconf-editor.
- **Instant switching, no reload.** Once enabled, changing the video is a dconf
  poke → the extension crossfades to the new video. (Hanabi changes mean fiddling
  with its own settings.)
- **Crossfade / Ken Burns** on switch and slideshow advance — reuse Fresco's
  transition vocabulary.
- **"Battery-aware" that's real**, not a TODO — the wallpaper visibly respects
  power, which users notice and trust.
- **Guided one-time setup.** Enabling a new extension on Wayland needs one
  logout — we make it a single clear step ("Enable live wallpapers — one-time, will
  sign you out"), then never again.

## Honest costs / risks (unchanged truths)
- **Third renderer + second runtime.** GNOME path is GStreamer + a gjs extension,
  separate from Rust (X11=mpv, wlroots=mpvpaper). Real ongoing surface.
- **Per-GNOME-version maintenance.** Extension APIs churn every 6 months (Hanabi
  runs 3 branches). The thin extension minimizes exposure but doesn't remove it.
- **One-time logout** to first-enable on Wayland — unavoidable; we make it clean.
- **dmabuf-to-Cogl wall** (above): we deliberately use the renderer-surface clone
  substrate, not a hand-rolled texture path, to stay on supported APIs.

## Phases
- **P0 — Spike (validate the substrate):** minimal GStreamer renderer surface +
  a ~50-line extension that clones it behind the desktop and plays a file on this
  GNOME Wayland session. Prove real video shows. Go/no-go.
- **P1 — Policy engine:** occlusion + overview + lock + idle gating; measure CPU
  in "covered" and "visible" states (target: covered ≈ idle baseline).
- **P2 — Efficiency:** decode-once → N-monitor clones; battery/power-profile →
  static; buffer reclaim after sustained pause. Measure RSS/VRAM vs Hanabi.
- **P3 — Integration + UX:** daemon drives renderer/extension via dconf/D-Bus;
  Fresco GUI "Enable live wallpapers" one-time flow; crossfade on switch.
- **P4 — Packaging:** ship the extension with Fresco; auto-install/enable; version
  matrix across supported GNOME releases.

## Verification (must measure, not assume)
- CPU/GPU sampled in each state (visible / covered / overview / locked / battery)
  vs Hanabi on the same video+hardware. Win condition: **covered & battery ≈ 0**,
  visible ≈ parity.
- RSS/VRAM single- vs dual-monitor vs Hanabi. Win: ~1× vs ~M×.
- Reliability: kill the renderer → desktop survives, wallpaper respawns.
- Switch latency + crossfade smoothness; one-time enable flow on a clean GNOME.

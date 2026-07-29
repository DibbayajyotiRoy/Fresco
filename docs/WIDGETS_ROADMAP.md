# Fresco Widgets — Roadmap

Status: **W0 spiked and W1 shipped (2026-07-29)** — lyrics and clock are live on
both backends. W3 (visualiser) and W4's disc exist as tested modules but are not
wired into the daemon yet, so they are **not user-visible**. W2 (Fresco's own
surface) is untouched and, unexpectedly, is no longer what gates W3/W4 — see the
notes on those sections. Owner: @DibbayajyotiRoy.
Parent roadmap: [ROADMAP.md](ROADMAP.md) §6.5 (this document is the detail).

**Status key:** ✅ shipped · 🟡 code complete, not wired · ⬜ not started.

| | Phase | Status |
|---|---|---|
| W0 | OSD spike | ✅ GO, with one constraint (below) |
| W1 | Now playing + synced lyrics | ✅ shipped |
| W2 | Fresco's own widget surface | ⬜ not started |
| W3 | Audio visualiser | 🟡 `dsp` + `audio_capture` + `visualizer` complete and tested; no daemon wiring |
| W4 | Album-art disc + clock | Clock ✅ shipped · disc 🟡 (`artwork`) |
| W5 | Themes & styling | Partial — per-widget presets and accent-follow shipped; no shared theme layer |

## What we're building

Things drawn **on top of the wallpaper**, below windows and desktop icons:

| Widget | What it shows | Needs audio samples? | Needs bitmaps? |
|---|---|---|---|
| **Now playing / lyrics** | Current synced lyric line, title, artist | no | no |
| **Audio visualiser** | Level waves / bars reacting to the music | **yes** | no |
| **Album-art disc** | Spinning "vinyl" carrying the cover art | no | **yes** |
| **Clock** | Time / date, themed | no | maybe (faces) |
| **Themes** | Shared styling across all of the above | — | — |

The pull is real: this is the feature people most often name when asking for a Wallpaper Engine equivalent on Linux. Nothing on Linux does lyrics-over-wallpaper well.

## The one decision that shapes everything

**Text is cheap. Everything else is not.**

Fresco already has a command channel to mpv on both backends (libmpv FFI on X11, JSON IPC on Wayland). mpv's `osd-overlay` renders **ASS markup** through it — so text can be drawn into the wallpaper with *no new window, no new surface, no compositor-specific stacking, and no click-through problem*, working identically on X11, COSMIC, Hyprland, Sway, KDE and DDE.

ASS cannot do the rest:
- **No bitmap support** → no album art, ever.
- Vector drawing exists (`\p1`) but driving it at 30–60fps means re-pushing overlay strings every frame — not smooth, not cheap.

So anything animated or image-bearing needs a **surface Fresco owns**: a second layer-shell surface at `bottom` on Wayland (wallpaper stays at `background`) with an empty input region, and a second override-redirect window on X11. That is the surface work of **ROADMAP §5.1 (native Wayland backend)** — the same dependency shader wallpapers have.

> **Correction (2026-07-29).** The bitmap half of that is wrong, and it changes the dependency graph. **ASS** carries no bitmaps, but mpv's **`overlay-add`** command — a different command on the same IPC channel — takes a raw pixel buffer (`bgra`, premultiplied alpha, explicit stride) straight off disk. So album art *can* reach today's substrate without W2, and `src/artwork.rs` renders a disc into exactly that format. `overlay-add`/`overlay-remove` are implemented on **both** `PlayerHandle` backends alongside `set_overlay`.
>
> The vector half stands as written but is **untested at rate**: `src/visualizer.rs` draws its five styles as ASS `\p1` paths, which means the visualiser is re-pushing an overlay string per frame — precisely the pattern this section calls "not smooth, not cheap". **Nobody has measured it.** That measurement, not W2, is what should decide whether the visualiser ships on this substrate.
>
> W2 is therefore no longer the gate on W3/W4 that the diagram below assumed. It is still the durable home for widgets, and still the only thing that gives us damage tracking, texture caching and a single composite pass.

And audio reactivity needs something MPRIS *cannot* provide: MPRIS carries metadata and position, **never audio samples**. Levels require capturing the PipeWire/PulseAudio monitor source and running an FFT — the pipeline **§6.1 already reserves** for audio-reactive shaders. **W3 must share that module, not grow a second one.**

Planned dependency graph:

```
W0 spike ──> W1 lyrics (text, ships on today's substrate)
                        │
        §5.1 native backend ──> W2 widget surface ──> W3 visualiser ─┬─> W5 themes
                                                 └──> W4 disc/clock ─┘
                                  §6.1 FFT module ──> W3
```

What actually happened (2026-07-29) — W2 turned out not to gate anything yet:

```
W0 spike ──✅──> W1 lyrics ──✅──> W4 clock ──✅──┐   all on the OSD substrate
                                                 ├─> per-widget presets (partial W5)
        dsp + audio_capture ──🟡──> W3 visualiser ┤   (ASS \p1, unmeasured)
                    artwork ──🟡──> W4 disc       ┘   (overlay-add, not ASS)

        W2 widget surface ── ⬜ not started; still the durable home
```

---

## W0 — Spike: is the OSD path real? (gates W1)

The cheap path rests on assumptions nobody has tested. Answer these before writing a feature. **This is a day, and it can kill W1.**

1. **Does `osd-overlay` render when `osd-level=0`?** X11 sets `("osd-level","0")` (`mpv/player.rs:52`); `build_mpv_opts` sets neither `osd-level` nor `osc` (`mpvpaper.rs:318`). **The two backends do not start from the same OSD state** — divergence risk #1.
2. **Do crop / `video-rotate` / zoom-pan transform the OSD layer?** Crop drives `video-zoom`/`video-pan`, and Ken Burns rewrites them every 16ms. If the OSD follows, lyrics will drift and rotate.
3. **Do fade transitions dim it?** Fades drive gamma to −100 (`mod.rs:957`). If the OSD dims too, lyrics pulse during slideshows.
4. **What does a 200ms overlay update cost?** Measure with `turbostat` on the reporter's Intel N150 — the rig that verified §6.4.
- **AC:** a one-off harness pushes an ASS string through **both** backends; screenshots prove it renders, survives rotation/crop, and behaves during a fade; a power delta is recorded. Written up here as GO / NO-GO.

### W0 RESULT (2026-07-26): **GO, with one constraint**

Driven through libmpv 2.2.0 by ctypes, mirroring Fresco's exact X11 option set (`osd-level=0`, `osc=no`, `config=no`, `load-scripts=no`), on Xvfb. Screenshots in the scratchpad.

1. **`osd-overlay` renders at `osd-level=0`. Fresco needs no option change.** Decisive A/B: at `osd-level=3` both mpv's playback timer and our overlay draw; at `osd-level=0` **the timer disappears and our overlay still draws**. So `osd-level` gates mpv's *own* OSD, not `osd-overlay` — and the two backends starting from different OSD states (divergence risk #1) turns out **not to matter** for this feature. `mpv_command` returned 0 in every case.
2. **Rotation does NOT rotate the text — but it DOES move it.** With `video-rotate=90`, the overlay stayed upright (good) but was rescaled and **clipped to the rotated video's area** rather than the screen: the OSD coordinate space follows the video's render area. **Constraint: pass explicit `res_x`/`res_y` on every `osd-overlay` call** (`lyrics::PLAY_RES_X`/`PLAY_RES_Y`) and re-push the overlay whenever rotation changes. Without this, lyrics on a rotated wallpaper are oversized and cut off.
3. **Fade (`gamma=-100`) — INCONCLUSIVE.** The software VO used for the spike ignores `gamma` (a `vo=gpu` feature), so neither video nor overlay changed. Must be re-checked on `vo=gpu` before slideshows with fade transitions are considered safe.
4. **Power — NOT YET MEASURED.** Needs the reference Intel N150 with `turbostat`, per the power budget.

---

## W1 — Now playing + synced lyrics (text only) · ✅ SHIPPED 2026-07-29

Ships on today's mpvpaper substrate. **Off by default.**

### W1 RESULT — what actually shipped

Confirmed working on a live Wayland/COSMIC session against Firefox. The plan
below is kept as written; this block records where the shipped thing differs
from it, because the differences are the load-bearing part.

**Delivered as planned**

- Current lyric line, plus the upcoming line dimmed underneath when
  `show_next_line` is on. Style presets, 9-point grid, margin, type size,
  accent-follow, and an `offset_ms` sync correction. Config lives in a
  `[widgets]` block that is absent from `config.toml` until something is turned
  on; settings groups sit in the GUI's Advanced dialog.
- Four style presets, not "3–4": `Minimal` (default), `Karaoke`, `Subtitle`,
  `Card`.
- MPRIS over `gdbus` with no D-Bus crate. The dedicated GVariant scanner was
  written rather than reusing `dde.rs::parse_first_string`, as instructed.
- The player-choice ladder, sticky selection, and behavioural detection of
  broken positions (three spaced-out zero polls while `Playing` → free-run the
  lyric clock from the track change, log once). No hardcoded blocklist.
- `.lrc` parsing, line-at-timestamp, ASS generation and the `#RRGGBB` →
  `&HBBGGRR&` conversion are platform-neutral and unit-tested with no desktop.
- W0's constraint is honoured: explicit `res_x`/`res_y` on every push, and a
  forced re-push when rotation changes.
- `set_overlay` lands on **both** `PlayerHandle` backends, per the
  backend-divergence risk.
- Silently absent on GNOME-Wayland static mode: that path never constructs a
  widget engine.

**Changed on purpose**

- **Every display by default, not one.** The plan's "one monitor by default" was
  reversed during implementation: the widget is part of the wallpaper, and a
  two-monitor desktop showing the lyric on one screen reads as half-broken.
  `widgets.monitor` narrows to a named connector. *(The doc comment on
  `config::Widgets::monitor` in `src/config.rs` still describes the old
  behaviour and is stale — fix it there.)*
- **An online lyrics source shipped in W1**, ahead of the plan, because almost
  nobody streaming has a local `.lrc` and a local-only lookup showed nothing for
  most people. Local files are still tried first; LRCLIB is the fallback,
  cache-first, with a 30-day hit TTL and a 24-hour miss TTL under
  `~/.cache/fresco/lyrics`. **LRCLIB states no licence over the lyrics data**, so
  Fresco fetches on demand, caches per-user, and claims nothing about the
  content — see Open Questions, which this narrows but does not close.
- **A `has_title` usability filter that was not in the plan.** Chromium-family
  browsers claim `org.mpris.MediaPlayer2.<brand>.instance<PID>` on first
  playback and never release it; when the session ends the `xesam:` fields are
  dropped but the artwork survives on a separate debounce timer, leaving a
  "zombie" session with a cover image and no title. Fresco previously selected
  such a player and then polled it forever without ever showing anything. Any
  player publishing no track title now loses at every rung of the ladder. This
  is why **Firefox is the browser to recommend for this feature.**

**Not delivered**

- **Title/artist display.** The scope line says "current lyric line (+ optional
  next), title/artist"; only the lyric line shipped. There is no now-playing
  text widget, and a track with no lyrics shows nothing.
- **The event plane.** The design called for parked `gdbus monitor` subprocesses
  at zero idle CPU. What shipped is an adaptive **poll** (see `scan_interval` in
  `src/daemon/widgets.rs`, which documents the schedule and its cost): a
  `ListNames` every 15s on an empty bus, a `GetAll` every 2s while playing, a
  `Position` every 1s. At the measured 3.1ms per `gdbus` call that is ~0.02% of
  one core on an idle desktop — **not the "zero daemon CPU" the AC asks for.**
  The worker parks on a condvar between cycles rather than spinning, and the
  code is shaped so the monitor lands as "stop polling" rather than a rewrite.
- **Smart Sleep, at the loop.** `LyricsRuntime::next_deadline_us` and
  `WidgetEngine::next_deadline` exist and are tested, but **no daemon loop calls
  them** — both loops still tick at 100ms. The redraw discipline is met (an
  unchanged tick returns nothing and builds no string), the *wakeup* discipline
  is not.
- **Power measurement.** Still not done, on any phase. The AC that says
  "measured, not asserted" remains unmet.
- **Overlay restore after a Wayland renderer respawn.** `clear_all` /
  `invalidate` are wired into the X11 rebuild path and into `Apply` on both
  backends, but the Wayland supervisor's respawn (the wedged/healed renderer
  path) has no route back to the engine, so a restarted mpvpaper comes back
  without its overlays until the content next changes. Small, real, unfixed.

> **On sequencing.** W1 is worth doing first not because it is quick, but because it de-risks everything after it: it proves the now-playing pipeline, the `.lrc` engine, the config/GUI surface and the power budget *without* betting on a new renderer. All of that carries forward to W2 unchanged. But be clear-eyed that the OSD substrate is a **compromise** — text-only, no bitmaps, and subject to whatever the W0 spike finds about crop/rotate/fade. The durable home for widgets is Fresco's own surface (W2). W1 is the beachhead, not the destination.

**Scope:** current lyric line (+ optional next), title/artist, 3–4 style presets, 9-point position grid, accent-follow, one monitor by default.

**Now-playing source — settled by research:**
- MPRIS over `gdbus`, matching the existing `dde.rs` shell-out. `gdbus` is the **only** D-Bus CLI present in both the `org.freedesktop.Platform` and `org.gnome.Platform` Flatpak runtimes (`busctl`/`dbus-monitor` are absent), so this is also the Flathub-safe choice. Measured: 3.1ms CPU per call; a parked `gdbus monitor` costs **0 CPU ticks over 10s** at 6MB RSS. zbus would add 66 crates plus an async runtime to a deliberately synchronous codebase — rejected.
- **Sync design:** long-lived `gdbus monitor` subprocesses for `PropertiesChanged`/`Seeked`/`NameOwnerChanged` (event plane, zero idle CPU) + a monotonic `Instant` anchor advanced on a 100ms local tick + a 1s `Position` resync while playing. Slew errors under ~300ms, hard-snap beyond — snapping every second reads worse than being 150ms off. **Never poll while paused.** Key track identity on `mpris:trackid`, not title (repeat-one won't retrigger otherwise).
- **Player choice ladder:** actively `Playing` → single `Paused` → most recently active, with a user-overridable priority list. Selection must be sticky so a background browser tab can't steal the overlay mid-song.

**Known-broken players (mitigate, don't ignore):** Spotify's native Linux client returns `Position: 0` and never emits `Seeked` — reported since 2018, unfixed, identical across native/Flatpak/snap. Same failure class in QQ Music and some Electron clients. Detect **behaviourally** (three consecutive zero polls while `Playing` → mark unreliable → free-run from track change, log once); never hardcode a blocklist. Spotify *in a browser* reports correct positions.

**Parsing hazard:** GVariant text output switches quote style by content (`<"Don't stop">` vs `<'Album "Quoted"'>`), `xesam:artist` is an **array** whose elements contain commas, and scalars carry inconsistent type prefixes. `dde.rs`'s `parse_first_string` **must not** be reused for `Metadata` — write a small dedicated scanner with tests against captured real output. Fetch `Metadata` only on track change; keep the hot path on the trivially-parsed integer `Position`.

**Lyrics source:** local `.lrc` files first — no API, no licensing exposure, works offline (see [Build vs. use](#build-vs-use--licensing-first)). Any online source is a later, opt-in addition and is BLOCKED on the licensing question in Open Questions.

- **AC:**
  - Lyric line within ~200ms of audio on a well-behaved player, visible on a 60fps capture.
  - **Zero** daemon CPU when no player runs or the overlay is off — measured, not asserted.
  - Redraw only when the **line changes**, never per tick. A libass re-render every 200ms at 4K on every output is new Render/3D work on exactly the machines §6.4 fought from 2.77W to 0.99W.
  - Smart Sleep: between lyric lines the daemon waits on an interruptible deadline, not a poll — a 30s gap between lines costs one wake, not 300.
  - Overlay cleared on wallpaper swap, teardown and output respawn — no leak between wallpapers.
  - Silent, complete no-op on GNOME-Wayland static mode (no player exists there).
  - `.lrc` parsing, line-at-timestamp, ASS generation and `#RRGGBB`→`&HBBGGRR&` live in a platform-neutral `src/lyrics.rs` with full unit tests and **no** desktop required.

---

## W2 — The widget surface (gates W3–W5) · depends §5.1

Fresco's own render surface for anything animated or image-bearing.

- Wayland: layer-shell surface at `bottom`, wallpaper stays `background`; empty input region for click-through; `wp_viewporter` + fractional-scale so widgets are pixel-exact on HiDPI.
- X11: override-redirect window above the wallpaper window, below icons — including the DDE restack path, which has bitten us twice.
- One widget host per output, honouring the existing per-connector model.
- **Built event-driven from the first commit** — frame-callback driven, damage-tracked, texture-cached, single composite pass. See [Power model](#power-model--event-driven-by-construction). This is architecture, not optimisation; it cannot be retrofitted.
- **AC:**
  - Click-through asserted on headless Sway (empty input region) and manually on a live session — a widget must never eat a desktop right-click.
  - A surface with unchanged content issues **zero** draw calls and requests no further frame callbacks.
  - Damage rectangles are asserted to cover only changed regions, not the full surface.
  - Widgets render above video wallpaper and below windows/icons on X11, Sway, Hyprland, KDE **and** DDE.
  - Zero GPU cost when all widgets are disabled (no surface created at all).
  - Surviving `plasmashell --replace` and a DDE `killall dde-shell` without orphaning.

---

## W3 — Audio visualiser (waves / levels) · 🟡 modules done, NOT WIRED

**Status 2026-07-29: not available to users.** `src/dsp.rs`, `src/audio_capture.rs`
and `src/visualizer.rs` are complete and unit-tested, but nothing in
`src/daemon/widgets.rs` drives them — the widget engine knows only about lyrics
and the clock — so there is no way to turn a visualiser on. **Do not describe it
as shipped anywhere.** What exists:

- `src/dsp.rs` — a **hand-written radix-2 Cooley–Tukey FFT** with Hann /
  Hamming / Blackman-Harris windowing and a streaming log-spaced band analyser.
  This **contradicts the "buy the primitives" table below**, which says use
  `rustfft` and not to build our own. The stated reason is that a radix-2 FFT is
  ~100 lines and the alternative was a new dependency tree; the trade is a
  deliberate one, not an oversight, but the table should be read as superseded
  here rather than as guidance that was followed.
- `src/audio_capture.rs` — monitor-source capture by shelling out to `pw-cat
  --record` (with `--raw` and `stream.capture.sink=true`) or `parec -d`, in the
  same spirit as the `gdbus` shell-out, and therefore **also with no crate**.
  Both invariants exist to guarantee we can never fall through to the
  microphone; that guarantee is the single most important thing in the module.
- `src/visualizer.rs` — five styles (`Bars`, `Mirror`, `Wave`, `Dots`, `Ring`)
  drawn as **ASS `\p1` vector paths**, i.e. on the OSD substrate rather than on
  W2's surface. See the correction above: this means an overlay string per
  frame, and **nobody has measured what that costs.** Measure before wiring.
- `dsp` and `audio_capture` are the shared modules §6.1 reserves. Audio-reactive
  shaders must use these, not a second implementation.

Capture the PipeWire/PulseAudio **monitor** source, FFT it, drive bars/waves.

- **Share §6.1's `audio_capture` + FFT module.** Two implementations of this is the failure mode to avoid.
- Must degrade cleanly when PipeWire is absent, when the monitor source is unavailable, and when the user denies permission.
- **Privacy:** capturing system audio output is a genuine privacy surface. Opt-in, clearly labelled, never on by default, and it must be obvious when it's active — same standard as the telemetry consent dialog.
- **AC:**
  - Synthetic 440Hz buffer lights the correct FFT bin (unit test, shared with §6.1).
  - No audio playing → visualiser idles at zero cost, not a busy loop.
  - Capture failure degrades to a static/hidden widget with one log line, never a crash or a black desktop.

---

## W4 — Album-art disc + clock · clock ✅ SHIPPED · disc 🟡 NOT WIRED

**Clock — shipped 2026-07-29, and not on W2.** It went out on the same OSD
substrate as the lyrics, which the "best first widget on the new surface"
reasoning had not anticipated; the reasoning still holds for W2 when W2 happens.
`src/clock.rs` renders five themes — `Digital`, `Minimal`, `Segment`, `Stacked`,
`Wordy` — with 12/24-hour, optional date, the same 9-point grid as the lyric
overlay, and accent-follow. Two themes deliberately overrule `show_date`
(`Stacked` always shows it, `Minimal` never does) and `Wordy` ignores
`show_seconds` outright, because there is no way to say "and seventeen seconds".
**Seconds are off by default as a power decision**, per the refresh table below:
one repaint a minute becomes sixty otherwise. DST/locale correctness is asserted
in unit tests; the ≤1% CPU idle figure has **not** been measured on hardware.

**Disc — not available.** `src/artwork.rs` renders a cover-art disc to
premultiplied **BGRA** for mpv's `overlay-add`, which is a real path on today's
substrate (see the correction above) rather than something W2 gates. It is fully
tested and **not wired into the widget engine**, so no user can enable it. It is
the one part of this work that adds a crate: `image`, restricted to PNG/JPEG/WebP
— the formats MPRIS players actually serve — to keep the build small.

- **Disc:** cover art from `mpris:artUrl`, rotating while playing, easing to a stop on pause. Handle `http(s)://`, `file://` **and** `data:` — `artUrl` frequently points into *another app's* sandbox or a `/tmp` file Fresco cannot read (a known Firefox-Flatpak issue). A failed art load must never break the widget; fall back to a generated placeholder.
- **Clock:** no audio and no MPRIS dependency, so it is the **best first widget on the new surface** — it isolates surface bugs from data-source bugs.
- **AC:** art fetch failures, missing art and enormous art are all handled without visual breakage; clock is correct across DST and locale; both run at ≤1% CPU idle.

---

## W5 — Themes & styling · partial

**Shipped as per-widget presets, not as a layer.** The lyric overlay has four
presets and the clock has five themes, and both take a placement anchor, margin,
type size and accent-follow — so the *user-facing* bargain ("pick a feeling, not
a dozen knobs") is met. What does not exist is the shared styling layer: each
widget carries its own copy of the vocabulary, and nothing enforces that a
future widget speaks the same one. The `#RRGGBB` → `&HBBGGRR&` conversion lives
in `src/lyrics.rs` and is the shared piece that does exist. The mpvpaper `-o`
`#`-in-a-config-file constraint below never bit, because widget colours go over
IPC at runtime as the constraint required.

One styling layer across every widget: font, size, colour, opacity, position, accent-follow, light/dark awareness.

- Ship a small set of opinionated presets ("Minimal", "Karaoke", "Vibes", "Corner card") rather than exposing every knob — presets are what make this feel designed rather than configurable.
- Accent-follow reuses `theme.rs::accent_pair` (currently private — needs a `pub` accessor or a `pub fn accent_hex`).
- **Constraint:** `theme.rs` colours are `#RRGGBB`, but **`#` cannot be passed through mpvpaper's `-o` options** — mpvpaper forwards them through an mpv config file where `#` starts a comment (`mpvpaper.rs:325`). Widget colours must go over IPC at runtime, or be pre-converted to ASS `&HBBGGRR&`.

---

## Power model — event-driven by construction

**Non-negotiable: nothing here runs a render loop.** §6.4 was corrected twice and only settled when measured with `turbostat`; a widget layer that quietly burns a watt has undone it. The architecture below is a hard constraint on every phase, not an optimisation to do later — retrofitting event-driven rendering onto a `while(true){draw()}` is a rewrite.

```
MPRIS / audio / clock events
        │
        ▼
   State manager      ── "what changed?"  (never "what should I draw?")
        │
        ▼
  Dirty-region manager
        │
        ▼
   Layout cache       ── shape text once per string, not per frame
        │
        ▼
  Texture cache       ── rasterise once, reuse until content changes
        │
        ▼
     Renderer         ── driven by frame callbacks, idle otherwise
```

**A distinction that matters:** on the **W1 OSD path we control only *when we push*, not how it is drawn** — libass and mpv own caching and damage internally. So rules 1, 6, 7, 8 and Smart Sleep apply at W1; rules 2, 3, 4, 5, 9 and 10 only become ours to enforce at **W2+**, on our own surface. Do not claim W1 benefits it cannot deliver.

| # | Rule | Applies |
|---|---|---|
| 1 | **Never redraw unless content changed.** A lyric changes every 2–8s, not 60×/s. | W1+ |
| 2 | **Cache shaped text as a texture.** Shape + rasterise once per string; reuse until it changes. Zero layout, zero glyph rasterisation, zero allocation on an unchanged frame. | W2+ |
| 3 | **Damage-based redraw.** Invalidate only the old and new rectangles (`wl_surface::damage_buffer`), never the whole surface. | W2+ |
| 4 | **Separate layout from paint.** `00:13 Hello` → `00:14 Hello` must repaint nothing. | W2+ |
| 5 | **Frame-callback driven, never a busy loop.** `request_redraw()` → draw once → swap → idle. On Wayland the compositor's frame callback is the clock; on X11, expose/damage events. | W2+ |
| 6 | **Animate only when required.** Paused → disc rotation speed 0 → no redraw. No audio → no FFT → no redraw. | W1+ |
| 7 | **Decouple update frequencies** (table below). | W1+ |
| 8 | **Freeze invisible widgets** — reuse the existing fullscreen/battery policy verbatim, plus screen-locked and covered. | W1+ |
| 9 | **One composite pass.** All widgets render into a single widget layer, not one surface per widget. | W2+ |
| 10 | **Adaptive quality.** Under GPU pressure or on battery: 60 → 30 → 15fps → static, degrading the visualiser first. | W3+ |

| Widget | Refresh |
|---|---|
| Lyric line | on line change |
| Title / artist | on track change |
| Album art | on track change |
| Disc rotation | frame callback **while playing only** |
| Progress bar | 4–10 Hz |
| Clock (minutes) | 1 / minute |
| Clock (seconds) | 1 Hz, only if seconds are enabled |
| Visualiser | 30–60 Hz **only while audio is present** |

### Smart Sleep

`.lrc` timestamps are known ahead of time, so the next visual change is a *known instant*. Between lines there is nothing to poll for:

```
render line          →  sleep until next_lyric_ts  →  wake  →  render next line
```

No timer tick, no polling, no redraw loop — genuinely ~0 CPU between lines while still appearing instant, because nothing visible is changing.

**The caveat that makes it correct:** the sleep must be *interruptible*, not a bare `thread::sleep`. Pause, seek, track change and player exit all invalidate the scheduled wake, so it must be a channel/condvar wait with a deadline — `wait_timeout(next_lyric_ts - now)`, woken early by any MPRIS event. A bare sleep would leave lyrics running after a pause, which is worse than polling. This also composes with rule 8: when frozen, there is no deadline at all, only the wake channel.

- **AC (every widget phase):**
  - An idle desktop with widgets enabled and no music shows **no measurable CPU** over a 60s sample, and no wakeups attributable to Fresco in `powertop`.
  - A static lyric line held for 30s triggers **exactly one** paint.
  - Pausing playback stops all animation within one frame and drops to zero wakeups.
  - `turbostat` on the reference Intel N150: widgets-on-idle within noise of widgets-off.

## Build vs. use — licensing first

Fresco is **GPL-3.0-or-later**, so permissive dependencies (MIT, Apache-2.0, BSD, ISC, Zlib) are all compatible and safe. Apache-2.0 is one-way compatible with GPLv3, which is fine in this direction. Avoid anything proprietary, SSPL/BUSL, or GPL-incompatible; check `cargo license` before adding.

**Rule of thumb: buy the primitives, build the product.** FFTs and JPEG decoders are solved and boring — writing our own wins nothing. The *look* of the widgets is the differentiation, and that part should be custom.

| Need | Recommended | License | Build our own? |
|---|---|---|---|
| FFT (visualiser) | `rustfft`, or `realfft` for real-input | MIT / Apache-2.0 | **No.** Mature, fast, well-tested. |
| Audio capture | `pipewire-rs`; `libpulse-binding` as fallback; `cpal` if a portable abstraction is wanted | MIT / MIT / Apache-2.0 | **No** — but *do* write our own thin monitor-source selection layer; that logic is Linux-specific and fiddly. |
| Album-art decode | `image` (PNG/JPEG/WebP) | MIT / Apache-2.0 | **No.** |
| HTTP fetch | `ureq` — **already a dependency** | MIT / Apache-2.0 | **No**, and add nothing new. |
| MPRIS | `gdbus` shell-out — **no crate at all** | — | Already decided; `zbus` (MIT) rejected on 66-crate cost. |
| `.lrc` parsing | — | — | **Yes, build it.** ~150 lines, pure, fully unit-testable, no dependency justified. |
| GVariant text parsing | — | — | **Yes, build it.** Small dedicated scanner; the format's quirks need our own tests. |
| 2D widget rendering (W2+) | `tiny-skia` (CPU) or `femtovg` (GPU/OpenGL) | BSD-3 / MIT+Apache-2.0 | **Evaluate at W2.** Pairs with the hand-dlopened EGL approach §5.1 already commits to. |
| Text shaping / fonts | `cosmic-text` (shaping + layout) or `fontdue` (lighter) | MIT / Apache-2.0 or MIT | **No.** Text shaping is a trap; do not hand-roll. |
| Widget layout, animation, theming | — | — | **Yes, build it.** This is the product. |

### Lyrics without the licensing problem

The cleanest answer to "what free API?" is: **make local `.lrc` files the first-class path, and treat any network source as an optional enhancement.**

- Fresco reads a `.lrc` sidecar next to the audio file, or from a user-chosen lyrics folder, keyed by track metadata.
- **Zero licensing exposure** — the user supplies their own files, exactly as subtitle-capable players have done for decades.
- Works offline, works for local music libraries, and is testable with no network and no desktop.
- It also makes the network source *replaceable* later instead of load-bearing.

Only once that works should an online source be added, opt-in, behind the same consent standard as telemetry. **LRCLIB is the leading candidate but its terms and data licensing are still unverified** (that research pass was cut short) — verify before writing a single request.

> **What happened (2026-07-29).** The local-first path works and is still tried first, and LRCLIB shipped alongside it in W1 rather than after it. The licensing finding is the important one and it is *not* "we verified a licence": **LRCLIB states no licence over the lyrics data at all** — entries are contributed by its users. Fresco's posture is therefore to fetch on demand, cache per-user, and claim nothing (`lyrics_fetch::ATTRIBUTION`, which is written and tested but **not shown anywhere in the GUI**). The "opt-in, behind the same consent standard as telemetry" half was **not** honoured: there is no toggle for the network lookup and no consent step. See Open Questions #2.

**Do not** build a custom lyrics database or scrape a commercial provider. Hosting lyrics is the one part of this that carries real legal risk, and it is the part with the least product value.

## Cross-cutting

**Config.** A `widgets: Option<Widgets>` on `Config`, absent from `config.toml` until set — the same shape as `browser_wallpaper`/`schedule`. Enums follow the `Transition`/`PowerSaving` pattern. `Config::default()` is hand-written and asserted against empty TOML, so every new field must be added there too.

**Per-monitor.** `Widgets { monitor: Option<String> }` — ~~`None` = primary~~, **as shipped `None` = every display** (the widget is part of the wallpaper), `Some("DP-1")` = that connector. No new plumbing needed; the loops already know each renderer's connector and `MonitorInfo` already reaches the GUI.

**Daemon loop.** No new thread: both the X11 and Wayland loops already tick at 100ms, and the codebase's idiom is a `last_*: Instant` field plus an interval check. Blocking work (D-Bus, HTTP, audio capture) goes on a detached thread publishing into a shared snapshot the tick reads — **never inline**, the loop must stay non-blocking.

**GUI.** A toggle in the header popover (model: the browser-bridge `switch_row`) plus a widget group in the Advanced dialog (model: `add_schedule_group`). Mind the borrow discipline: mutate + `save()` inside a scoped `borrow_mut()`, drop, then re-borrow to push to the daemon.

**IPC.** `Request::Apply` already means "re-read config and apply", so v1 may need nothing new. But **X11 `Apply` does a full `rebuild()`**, tearing down every window and briefly revealing the native wallpaper — a widget toggle routed through it will flash the desktop. A narrower `WidgetsChanged` request avoids that. **As shipped, v1 does route widget toggles through `Apply`, and the X11 flash is real and unfixed** — `WidgetsChanged` was not added.

**Power budget.** §6.4 was corrected **twice** and only settled when measured with `turbostat` on real hardware. Every widget phase re-measures on that rig. A widget that quietly costs 1W has undone the feature we just shipped.

---

## Open questions

**Closed**

1. ~~**`osd-overlay` behaviour at `osd-level=0`, and under crop/rotate/fade.**~~
   Answered by the W0 spike: it renders at `osd-level=0`, and rotation moves and
   clips it unless `res_x`/`res_y` are passed explicitly. **Fade is still
   inconclusive** — see the still-open list. Crop was not separately re-tested
   after W0 but has caused no reported problem in the shipped feature.

**Still blocking**

2. **Lyrics licensing — narrowed, not closed.** LRCLIB shipped in W1 on these
   terms: fetched at runtime, never bundled or redistributed, cached per-user
   under `~/.cache/fresco/lyrics`, client identified in `User-Agent` as its docs
   require, requests spaced per its throttling guidance, and an attribution
   string that credits LRCLIB while **disclaiming any licence over the content**
   — because LRCLIB states none. What is *not* settled: (a) the attribution
   string exists in code but is not surfaced anywhere in the GUI; (b) the
   network lookup has **no consent step and no separate toggle** — enabling the
   lyrics widget enables it, which does not meet the opt-in-with-consent
   standard telemetry sets, and the doc comment on `config::Lyrics` still claims
   "no network, no API"; (c) the Flathub implications were never worked through.
3. **Flatpak permission — unchanged and now actually blocking.**
   `flatpak/io.github.dibbayajyotiroy.Fresco.yaml` carries **no** MPRIS
   `--talk-name`, so lyrics cannot see any player in the Flatpak build today.
   `--talk-name=org.mpris.MediaPlayer2.*` is shipped by a synced-lyrics app on
   the current runtime, so there is precedent. Do **not** use
   `--socket=session-bus` — the docs call it a security risk and it would draw a
   Flathub review objection. Also unverified: the shipped implementation
   discovers players with `ListNames` on `org.freedesktop.DBus`, and whether the
   Flatpak D-Bus proxy returns the MPRIS names through it once the talk-name is
   granted has not been tested.

**Opened by the work**

4. **Fade (`gamma=-100`) on `vo=gpu`** — W0 could not answer it and nothing
   since has. Unknown whether lyrics dim during slideshow fade transitions.
5. **Power, on every phase.** Never measured. The reference Intel N150 with
   `turbostat` is still the rig, and the "measured, not asserted" ACs are all
   outstanding. The visualiser's per-frame ASS push is the one most likely to
   cost something real.
6. **Does the visualiser belong on the OSD substrate at all?** It draws vector
   paths as ASS and re-pushes per frame. Answer with a measurement before wiring
   it, not after.
7. **Consent for system-audio capture.** W3's privacy requirement (opt-in,
   clearly labelled, obvious when active) has no implementation yet, because the
   visualiser has no user-facing surface yet. It must exist *before* the first
   build that can turn capture on.

## Risks

- **Scope creep into "a widget platform."** One widget people love beats a framework nobody fills. Ship W1, measure, then commit to W2.
- **Backend divergence.** `raise_demuxer_cache` is the standing example of a `PlayerHandle` method that silently does nothing on one backend. A widget that no-ops on X11 would be the same bug somewhere far more visible. Every widget method lands on both backends in the same change.
- **Power regression** — see budget above and the [Power model](#power-model--event-driven-by-construction).
- **Shipping a render loop "for now".** The single most expensive mistake available here. Event-driven rendering is structural; a continuous loop written early becomes a rewrite later, and every widget added on top of it deepens the hole.
- **Building W2 before §5.1.** Do not attempt the widget surface on the mpvpaper substrate; it is rework by construction.

## Non-goals

- Interactive widgets (buttons, input). Click-through is the requirement; anything clickable fights the desktop.
- A user scripting/plugin API. Not before the built-in widgets are proven.
- Widgets on GNOME-Wayland static mode — there is no player and no surface. Say so plainly rather than shipping a dead toggle.
- Scraping Spotify's own lyrics (Musixmatch-licensed, not available to third parties).

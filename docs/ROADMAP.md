# Fresco Roadmap — the world's best wallpaper app for Linux (macOS next)

Status: adopted 2026-07-03. Baseline: v0.0.91.

## Why this roadmap

Fresco is stable and well-engineered (self-healing renderers, hwdec, strong CI) but its value is capped by five things:

1. **Broken funnel** — landing page has placeholder screenshots; install is .deb-only (no Flathub/AUR), excluding the Arch/Hyprland ricing crowd and everyone outside Debian-family.
2. **No content** — users must bring their own mp4. Wallpaper Engine's lesson: the Workshop is ~90% of its value.
3. **GNOME Wayland is static-only** — the majority Linux desktop gets no live wallpaper, and GNOME is removing X11 (disabled in GNOME 49), so the "works on GNOME X11" story is melting.
4. **Honesty debt** — README claims "live hotplug handling" (README.md:37) and Hyprland/KDE support (README.md:30,65,96), but Wayland hotplug is unimplemented and only Sway is proven on-screen (docs/WAYLAND_VERIFICATION.md).
5. **Core-fidelity defects (user-reported)** — 4K/8K content renders soft or with broken pixels on large/high-res displays, worse as screens get bigger (suspected: Wayland HiDPI/fractional-scale buffer handling, cheap default scalers without dithering, hwdec capability cliffs at 4K/8K); and per-wallpaper audio can be silent (suspected: muted entries load with the audio track dropped entirely — the `aid=no` RAM optimization — and unmute may not restore it; plus daemon-before-PipeWire ordering at login). The core promise must hold before anything else matters — these are fix-first items in Phase 1.

The ambition: **make Fresco the world's best wallpaper app for Linux users, with macOS to follow.** This roadmap is the Linux-first path: **10x = adoption × retention × capability**, with every new core module written portably so macOS later is a new backend + shell on a shared core, not a rewrite.

## Thesis

| Lever | Multiplier | Cost |
|---|---|---|
| Fix reach (Flathub, AUR, real screenshots, true claims) | 3–4x reachable installs | days–weeks |
| Content catalog (kill "bring your own mp4") | 1.5–2x retention + conversion | 2–3 weeks |
| GNOME Wayland live wallpapers | 2–3x addressable users | XL, gated by a 1-week spike |
| Engine v1.0 (native backend) | enables shaders + perf wins | XL, invisible to users until then |

Sequencing philosophy: **prove before advertising, grow before rewriting.** The native-backend rewrite comes late because the gallery/GUI features are backend-agnostic (they produce library entries both engines consume) — nothing built before it gets reworked.

**What "world's best on Linux" means concretely:** after P1–P2 Fresco is the most trustworthy + easiest-to-install option (no competitor has proven claims + Flathub + AUR); after P3 it's the only Linux wallpaper app with a content ecosystem (Wallpaper Engine's actual moat); after P4 it's the only GUI app with live wallpapers on GNOME, the majority desktop; after P5–P6 it has engine quality and one-click shaders nothing else on Linux offers. The Linux competition (Komorebi abandoned, Hidamari GNOME-only, mpvpaper CLI-only) is cleared by P2; the real benchmark is Wallpaper Engine.

**macOS readiness principle (applies from P3 onward):** the portable assets are the *brain* — catalog, library schema, scheduler, config, update abstractions — not the Linux backends. New core modules (`catalog.rs`, `schedule.rs`, download worker) must contain no Linux-isms (no XDG paths hardcoded outside a platform layer, no Unix-socket assumptions in core types). This costs ~nothing now and makes Phase 7 a port, not a rewrite. macOS work itself starts only after the Linux position is secured — a solo dev splitting across two platforms before winning one risks both.

Timeline (solo dev, rough): P1 ~4–5wk → P2 ~2–3wk → P3 ~4–6wk → P4 ~6–8wk (conditional) → P5 ~6–8wk → P6 ongoing. ~5–6 months to v1.0.

---

## Phase 1 — Truth & Foundation (~4–5 weeks)

Theme: make every claim true — including the implicit ones: the picture is pixel-perfect and the sound works. Then make Fresco installable and visible where its audience lives. Exit: a Hyprland user and a Debian user each install idiomatically in <2 min; nothing in README/landing is untrue; a 4K wallpaper looks sharp on a 4K display with working audio. **Within this phase, 1.7 (audio) and 1.8 (fidelity) are user-reported defects and rank first.**

### 1.1 Close out the v0.0.91 update-flow gaps
v0.0.91 shipped in-app updates, responsive UI, and the status pill. Remaining gaps: updater stderr is never captured (failures show only an exit code); the status pill's CPU% reads a hardcoded 0.0.
- **AC:**
  - A failed update shows the script's stderr detail, not just "exited with status N."
  - Status pill shows real CPU% under playback (interval sampling of daemon + mpvpaper children), or the field is removed from StatusReply + UI (engine-notes item D: never ship a metric that's always 0).
  - CI green (fmt/clippy -D warnings ×3 feature combos, compositor matrix ≥3 pass).

### 1.2 Prove Hyprland + KDE Plasma 6 (or stop claiming them)
Only Sway is proven on-screen. Hyprland/KDE users are the highest-intent audience; a silently-broken first run there is negative marketing in exactly the community (r/unixporn, r/hyprland) where reputation compounds. Known product risk: on Plasma, the layer-shell surface may sit below Plasma's desktop containment / fight FolderView (engine-notes item F) — test this first; if broken, try mpvpaper's `-l bottom` per-compositor before downgrading the claim.
- **AC:**
  - Evidence ledger rows T1/T1A/T6 flip to PROVEN for Hyprland and KDE with committed artifacts (real sessions: live-USB or 2+ community testers running tests/wayland/verify.sh, screen recordings).
  - T2 click-through PROVEN on both: desktop right-click menu + icon interaction work over live video; KDE FolderView icons render above the video; `plasmashell --replace` doesn't orphan mpvpaper.
  - CI hyprland leg passes ≥9/10 consecutive runs with the grim screenshot check enabled (pin a known-good Hyprland; post-aquamarine headless env vars).
  - Suspend/resume on one physical machine: post-wake capture non-black (T15).
  - Any claim that fails verification is downgraded to "experimental" in README/landing/GUI capability banner within the same release.

### 1.3 X11 fullscreen auto-pause (parity)
Wayland pauses per-output on fullscreen; X11 doesn't pause at all. Small, real battery win, proven pattern to copy. Poll-based (piggyback the existing 2s cadence) — **not** event-driven, respecting the hard-won "never react to X events" restack lesson in Daemon::run. Read `_NET_CLIENT_LIST_STACKING` + `_NET_WM_STATE_FULLSCREEN`, intersect geometry with monitor rects, fold into a per-renderer `reconcile_pause` mirroring WlOutput's.
- **AC:**
  - Xvfb CI: `wmctrl -b add,fullscreen` on a test window → pause logged ≤3s and mpv time-pos stops; removing it → resume ≤3s.
  - Dual-head: fullscreen on monitor A pauses only A.
  - Battery pause and manual Pause/Resume behavior unchanged (existing assertions).
  - Cold-boot stall heal never fires against a fullscreen-paused renderer.
  - Wallpaper stacking unchanged after 20 fullscreen toggles.

### 1.4 Honest multi-monitor story + interim Wayland re-enumeration
README claims "live hotplug handling" — true only on X11. Full Wayland hotplug belongs to the native backend (P5; building it on the mpvpaper path is rework, per engine-notes item C). Interim: `run_wayland_layershell` reconciles Apply against its startup output snapshot, so a monitor plugged after daemon start can never be assigned until restart — fix by re-running output enumeration inside the Apply handler.
- **AC:**
  - Headless Sway: `swaymsg create_output` after daemon start, then Apply → new output gets a wallpaper without daemon restart.
  - README/landing multi-monitor claim scoped truthfully (e.g. "live hotplug on X11; on Wayland new displays pick up on next apply — automatic hotplug tracked for v1.0").

### 1.5 AUR packages
The ricing community installs via AUR reflexively; Fresco is invisible to them. Also recruits exactly the Hyprland/KDE testers 1.2 needs.
- **AC:**
  - `fresco-bin` (repackaged release) and `fresco` (source PKGBUILD, builds mpvpaper or depends on Arch's packaged mpvpaper — `mpvpaper_command()` already falls back to PATH) both live in AUR.
  - `namcap` clean; `yay -S fresco-bin` on a clean Arch VM installs and sets a video wallpaper on Hyprland.
  - Both packages added to the release checklist (bump on every release).

### 1.6 Landing page made real + first launch moment
Placeholder stock photos on a visual product's landing page destroy conversion; there is currently no way to *see* Fresco before installing.
- **AC:**
  - Zero `picsum.photos` references in landing/src; README hero screenshot uncommented with a real capture.
  - A ≤60s demo video (open app → pick video → set → close app → wallpaper persists → per-monitor) above the fold and linked in README.
  - Baseline metrics recorded first (GitHub release download counts via API + stars + Vercel Analytics), then one coordinated launch post (r/unixporn or Show HN) timed with the next release.

### 1.7 Per-wallpaper audio: make it actually work (user-reported bug)
README and the landing page promise per-wallpaper sound; setting an unmuted video wallpaper can produce no audio. Prime suspect from the code: the RAM optimization loads muted entries with the audio track dropped entirely (`aid=no` in both `mpv/player.rs` and `mpvpaper.rs`); if the unmute path only flips the `mute` property without restoring the audio track (`aid=auto`, possibly requiring a reload), sound can never start. Secondary suspect: on login, the autostarted daemon can spawn mpv before PipeWire/PulseAudio is up, so mpv binds no audio device and never retries. **Reproduce first, then fix** — write the failing assertion before touching the players.
- **AC:**
  - Failing-then-passing repro committed: on X11 and headless Sway, applying an entry with `mute=false, volume=70` yields mpv properties `aid != "no"`, `mute=false`, `volume=70` within 3s (X11: in-process property read; Wayland: via MpvIpc).
  - Unmuting an already-running muted wallpaper from the GUI produces audio live without re-applying (audio track restored); re-muting returns to the track-dropped state so the RAM saving is preserved (property assertions both ways).
  - Mute/volume persist across daemon restart and login restore; a cold boot with autostart plays audio on first login without a manual re-apply (retry/reload once the audio server appears; log assertion + one physical-machine verification).
  - Manual listen check on one PipeWire and one PulseAudio system recorded in the verification ledger; the status pill reflects audible state.

### 1.8 4K/8K visual fidelity: pixel-perfect on any monitor (user-reported bug)
Quality degrades on large/high-res displays — softness and occasional broken pixels, worse the bigger the screen. Suspected causes, in order: **(a) Wayland HiDPI/fractional scaling** — the layer-shell surface rendering at logical resolution and being upscaled by the compositor is systemic blur that scales with display size (mpvpaper's buffer-scale handling; we build mpvpaper from source, so it is patchable); **(b) scaler defaults** — `balanced` leaves mpv on cheap bilinear with no dithering: soft 8K→4K downscales, banding in gradients; **(c) hwdec capability cliffs** — many GPUs cannot hardware-decode 8K (or 4K on some codecs) HEVC/VP9/AV1; mpv then artifacts or silently falls back to software ("broken pixels", stutter); **(d) the crop/zoom path** resamples through the same cheap scalers. **Build the measurement harness first, then fix what it convicts.**
- Fidelity harness: ffmpeg-generated test patterns (1px checkerboard, resolution chart, smooth gradient) at 1080p/4K/8K, played on headless outputs at scale 1 / 1.25 / 2 (Sway supports scaled headless outputs; Xvfb for X11), captured with grim/import, center-crop compared against reference — pixel-exact required at integer scales, SSIM threshold at fractional. Added to the verification ledger + CI.
- Fixes on the current architecture: assert X11 windows are 1:1 physical pixels; verify the Wayland buffer size equals output physical pixels and patch the bundled mpvpaper if it doesn't; raise defaults so **Balanced is visually correct** (`correct-downscaling`, `linear-downscaling`, `dither-depth=auto`; spline36-class up/downscale — cost is modest at wallpaper framerates) and High = Lanczos as today; optional deband toggle.
- Decode honesty: Status/`fresco doctor` gain source resolution, bit depth, active hwdec, and dropped-frame count; playing media beyond the GPU's decode caps (probe VA-API/NVDEC profiles) produces an explicit warning with expected impact — never silent artifacts.
- **AC:**
  - Harness in CI: 4K checkerboard on a scale-1 4K output captures pixel-identical (zero interpolation blur); at scale 1.25/2 the buffer equals physical pixels and SSIM ≥ threshold; new T-rows in docs/WAYLAND_VERIFICATION.md.
  - 8K→4K downscale of the resolution chart shows no aliasing/moiré (SSIM vs an mpv `gpu-hq` reference within threshold); the gradient pattern shows no visible banding (`dither-depth` property asserted active); a 10-bit source plays without posterization.
  - 10-minute 4K@60 soak on reference hardware: dropped frames ≤0.1% (`frame-drop-count`); demuxer cache scales up (bounded) for ≥4K/high-bitrate sources so quality never degrades to save RAM silently.
  - Cropped/zoomed 4K source shows no added softness vs uncropped at the same effective scale (SSIM within tolerance).
  - An 8K file on hardware without 8K decode support yields a visible in-app warning naming the limit, and playback uses the best honest path — no unexplained artifacts.

**Phase 1 non-goal:** zero new wallpaper features, by design (1.7/1.8 are defect fixes of already-advertised behavior).

---

## Phase 2 — Reach (~2–3 weeks)

### 2.1 Flathub, for real
The manifest (flatpak/…yaml) is an unbuilt draft pinned to v0.0.2. Flathub covers Fedora/openSUSE/Arch-adjacent/Steam Deck at once and provides public install stats (the measurement instrument for later phases). Sandbox work required: autostart via the Background portal (gio D-Bus, no new crate; keep file-write path outside Flatpak), update UI + daemon update-notifications fully suppressed in-sandbox, mpvpaper + libmpv built as manifest modules (dlopen resolves in /app/lib), dconf access for the GNOME static path, runtime retargeted to current GNOME.
- **AC:**
  - Manifest builds reproducibly from a pinned tag+commit with `org.flatpak.Builder`; `flatpak-builder-lint` zero errors.
  - Inside the sandbox: `frescod --check` reports bundled libmpv + correct capability; live wallpaper works on X11 and Sway; autostart created via portal; update prompts absent (log assertion).
  - Published on Flathub; app appears in GNOME Software/Discover with real screenshots; store listing explicitly sets the GNOME Wayland static-frame expectation (Flathub's core audience is GNOME — do not oversell).
  - Landing "Coming soon" replaced with a live Flathub badge; metainfo.xml release list updated (currently stale at 0.0.2).

### 2.2 Per-monitor assignment GUI
Config (`Config.monitors` map) and both daemon reconcile paths already honor per-connector wallpapers — this is GUI + one additive IPC field. Multi-monitor is the enthusiast norm and this is the cheapest "wow" in the backlog. Monitor names must come from the daemon (GDK connector API needs GTK 4.10; crate pins v4_6): add `monitors_info: Vec<MonitorInfo>` to StatusReply with `#[serde(default)]` (old-daemon compatible), listing **all** connected outputs.
- **AC:**
  - Two different wallpapers on two monitors achievable entirely in-GUI (display strip with to-scale rectangles + "Set on display" in the card menu), ≤4 clicks per monitor, zero config-file edits; persists across reboot.
  - "All displays" choice removes the override; override badge shown on assigned cards; single-monitor users see no extra chrome.
  - Headless Sway 2-output: StatusReply.monitors_info lists both with geometry; per-connector config yields two mpvpaper processes with correct file args → T9 (dual-monitor) flips to PROVEN.
  - GUI write touches only `[monitors."<connector>"]` in config.toml (file assertion); default wallpaper untouched.

### 2.3 IPC/scripting docs
Ricers script everything; documenting the control socket turns Fresco into a building block (waybar toggles, workspace-based wallpaper scripts) and earns community respect cheaply.
- **AC:** a docs page with 5 copy-pasteable recipes (set wallpaper from script, per-output set, pause/resume, status JSON, playlist-next); each verified in headless Sway.

**Second launch moment:** next release + Flathub announcement.

---

## Phase 3 — The Content Engine (~4–6 weeks)

Theme: kill "bring your own mp4." Exit: app-open → great live wallpaper in under a minute without leaving the app.

### 3.1 In-app wallpaper catalog (curated)
The Workshop lever — converts acquisition (gallery = demo + SEO), activation (first wallpaper <1 min), and retention (new items = reason to return) at once; structurally unavailable to hidamari/komorebi/mpvpaper. Design constraints (decided):
- **Metadata in Supabase** (new `catalog_items` table + RLS mirroring the existing notifications pattern; admin CRUD in the existing admin/ app). **Media on a zero-egress host** — Cloudflare R2 or GitHub Releases of a dedicated `fresco-wallpapers` repo — never Supabase Storage (free-tier egress dies at ~100 installs of one 20MB video).
- Schema includes `content_type` (video/image/shader-later), `license`, `author`, `source_url`, `size_bytes`, `checksum` from day 1 — shaders slot in at P6 without migration; every item legally attributable. Curated CC0/verified-license only; 50–100 hand-picked loops across ~8 categories. **No user submissions yet** (moderation is a solo-dev trap).
- Client: new `src/catalog.rs` (ureq, modeled on supabase.rs) + `src/gui/gallery.rs` Stack page; catalog JSON + thumbs cached in ~/.cache/fresco/gallery/; downloads on a worker thread (`.part`-then-rename), landing as ordinary `LibraryEntry` rows (+ optional `catalog_id` field) — **no parallel content system**.
- Server-side per-item download counts (edge function or count column) = engagement measurement with zero client telemetry.
- **AC:**
  - From app open: Browse → item → "Set as wallpaper" = ≤3 clicks; a 20MB item downloads + applies ≤30s on 50Mbps; progress and cancel work; cancel/SIGKILL mid-download leaves no partial file in the library.
  - ≥50 items at launch, each verified to loop cleanly at 1080p+; every card shows license + author; unpublished rows invisible via anon key (RLS test).
  - Offline/failed fetch degrades gracefully: cached catalog renders, clear error + retry, library fully functional.
  - Catalog refresh requires no app release; admin app can add/publish/unpublish and see per-item install counts.
  - Measurable goal: within 4 weeks, catalog installs ≥ 40% of new-version downloads.

### 3.2 Add-from-URL (direct media URLs only)
Paste an `https://….mp4/.webm` link → downloads into the library (shares the 3.1 worker). **No yt-dlp / YouTube integration** — ToS gray zone that endangers Flathub/AUR standing, perpetual bitrot, and it competes with the catalog (the strategic asset).
- **AC:** direct URL downloads to library + entry + thumbnail (testable against a local http.server fixture); Content-Length pre-check with size cap and mid-stream abort; cancel leaves no partial; clear error surface.

### 3.3 Scheduled / time-of-day wallpapers
Day/night pairs are a beloved Wallpaper Engine pattern; slideshow machinery is 80% of the work. Daemon-evaluated: new pure `src/schedule.rs` (`desired(schedule, local_time) → Option<&Wallpaper>`, fully unit-testable; chrono dep), config gains an optional `schedule` block (day/night wallpapers or `[[schedule.at]]` time rules; sunrise/sunset via hand-rolled NOAA with **manual coordinates — no geoclue**). Swaps ride the media-only `load_path` fast path — never `rebuild()` — to avoid the X11 teardown/flash and stay clear of the restack/NVIDIA machinery.
- **AC:**
  - `cargo test`: `desired()` fixtures across midnight wrap, DST spring/fall, 3 NOAA sunrise cases within ±2 min.
  - Headless CI: boundary set to now+60s → Status wallpaper name changes ≤90s later; daemon PID stable, no renderer rebuild logged.
  - Pause state and manual Apply survive: a manual Apply holds until the next boundary; paused-before → paused-after a swap.
  - Config without `schedule` roundtrips byte-identical (serde compat).

### 3.4 GNOME live-wallpaper spike (timeboxed 1 week, go/no-go)
Exactly P0 from docs/GNOME_LIVE_WALLPAPER_PLAN.md, with one simplification to try first: the renderer can be the already-bundled **mpv** (`--wayland-app-id=fresco-renderer`, steered by the existing MpvIpc client from mpvpaper.rs) instead of a new GStreamer process; a ~50-line gjs extension finds the window and puts a Clutter.Clone per monitor into the background group. The outcome decides Phase 4, and GNOME's X11-removal clock makes deciding late the expensive failure mode.
- **AC (the gate):**
  - GO = video visibly animating behind desktop icons on a real GNOME 46+ Wayland session; `kill -9` the renderer ×10 → shell never crashes, wallpaper respawns ≤2s; fully-covered state CPU within ~1pt of paused baseline.
  - Either way, a decision doc with measurements is committed — no vibes-based decision.

**Third launch moment:** catalog release + r/unixporn post showcasing a 3-monitor rice done entirely with in-app content.

---

## Phase 4 — The GNOME Bet (conditional on 3.4 GO; ~6–8 weeks)

Ordered **before** the native backend deliberately: GNOME live is the single biggest user unlock (Ubuntu/Fedora/Debian defaults), it does not depend on the native backend (renderer-window + clone substrate), and wlroots users are already served by mpvpaper meanwhile. The native backend is invisible to users; this isn't.

### If GO: ship the GNOME Shell extension (P1–P4 of the existing plan doc)
Thin gjs extension (<400 lines, ESM, GNOME 45+ style) owning the visibility policy; daemon stays the brain (new `Capability::WaylandGnomeExtension` + `run_gnome_extension()` branch; control via dconf/D-Bus). Beat Hanabi on **policy**, not pixels: decode only when visible, decode once for N monitors, battery-aware, buffers freed on sustained invisibility. Ship the extension zip in the packages (`gnome-extensions install`); treat extensions.gnome.org as marketing, not the delivery path.
- **AC:**
  - Works on the two most recent GNOME stable releases; loads error-free (journal grep) on each; version matrix documented and CI-covered where headless-testable.
  - One-time enable flow: ≤2 clicks in Fresco + one clearly-explained logout; never sends users to dconf-editor/Extensions app.
  - Policy engine **measured, not assumed**: fully-covered and locked states ≈ idle CPU baseline (pidstat); on-battery → static frame; decode-once verified with 2 monitors (1 pipeline, 2 clones); RSS ~1x vs Hanabi on the same hardware/video.
  - Crash isolation: `kill -9` renderer ×10 → desktop unaffected, wallpaper back ≤3s; disabling the extension auto-falls back to the static-frame path (T16/T17 still pass).
  - Only after this ships: COPR package, Fedora-targeted outreach, and README/landing claim "live wallpapers on GNOME."

### If NO-GO
Publish the decision doc openly; redirect the time to best-in-class GNOME static (slideshow transitions, still-frame at ~10% of duration instead of frame 0 — engine-notes item E) and pull Phase 5 forward. GNOME messaging becomes honest and specific ("automatic still-frame today; live support tracked here").

---

## Phase 5 — Engine v1.0 (~6–8 weeks)

Theme: own the render path. The "road to v1.0" from engine-notes — sequenced late because nothing built earlier gets reworked by it.

### 5.1 Native Wayland backend (layer-shell + mpv_render_context)
New `src/daemon/native_wl/` (surface.rs: layer-shell + empty input region + wp_viewporter/fractional-scale; egl.rs: hand-dlopen'd EGL consistent with the libmpv philosophy; render.rs: extend mpv/ffi.rs with `mpv_render_*`). New `PlayerHandle::NativeWayland` variant (the enum was built for this). **Hard rule: side-by-side behind `FRESCO_NATIVE=1`; mpvpaper stays bundled and default until the native path passes the full T1–T22 ledger on Sway + Hyprland + KDE; delete mpvpaper only at v1.0.** Render threads must fail as Results, not panics (`panic=abort` would kill the daemon).
- **AC:**
  - `FRESCO_NATIVE=1 tests/wayland/verify.sh` on headless Sway: T1/T1A pass with the same screenshot thresholds as mpvpaper.
  - **Pixel-perfect HiDPI (the permanent fix for 1.8's Wayland cause):** `wp_viewporter` + `wp_fractional_scale_v1` from day one; buffer equals output physical pixels at scales 1/1.25/1.5/2; the 1.8 fidelity harness passes pixel-exact at integer scales and ≥ SSIM threshold at fractional scales.
  - Audio parity: the 1.7 audio assertions pass identically on the native backend.
  - Click-through asserted (empty input region) + manual T2 on a live session; NVIDIA box in the test matrix before any default flip.
  - Crash supervision parity: a wedged render context (frozen time-pos) is rebuilt without daemon restart.
  - 8-hour headless soak: RSS growth <10MB, zero respawns.

### 5.2 Single-decode multi-monitor
Same video on N outputs = one mpv core rendering once into an FBO, blitted per-output (handles per-output size/scale) — strictly safer than N render calls per frame. Outputs with different media keep their own cores; `Config.monitors` semantics unchanged.
- **AC:** dual-output headless Sway, same video: exactly 1 decode instance (Status reports 1 core/N outputs); RSS ≤1.5x single-output (vs ~2x today); total CPU ≤1.3x single-monitor.

### 5.3 Full Wayland output hotplug + remaining parity
Registry-driven (`global`/`global_remove` on wl_output) → reconcile against config.
- **AC:**
  - `swaymsg create_output` mid-run → wallpaper on the new output ≤5s without restart; `output … disable` → surface destroyed, RSS returns to baseline; T10/T11 flip to PROVEN; README hotplug claim restored to unconditional.
  - **v1.0 release criteria (the public promise):** no bundled third-party binaries; one decode per unique video; hotplug; fullscreen pause on X11 *and* Wayland; **pixel-perfect rendering at every output scale (fidelity harness green)**; **per-wallpaper audio verified on every backend**; verified compositor matrix published.

---

## Phase 6 — Differentiation & Delight (post-v1.0, ongoing)

### 6.1 Shader wallpapers (GLSL, Shadertoy-compatible subset) — hard-depends on 5.1
The sharpest available differentiation ("the only GUI Linux wallpaper app with one-click shaders") and it transforms catalog economics: shader items are <100KB vs 20MB videos. Scope line drawn: single-pass `mainImage` with `iTime`/`iResolution`/`iFrame` (+ audio channel later); no multipass/buffers/keyboard. Load from .frag file/paste — **no in-app Shadertoy browser** (default license is CC BY-NC-SA). Wayland-native-backend only at first; X11 deferred.
- **AC:**
  - Animated .frag on headless Sway → two captures 1s apart differ (reuse T1 method); 60fps at 1080p on an Intel iGPU ≤15% GPU / ≤5% CPU with the mandatory fps cap honored.
  - Broken shader → previous wallpaper retained + error in Status — never a black desktop.
  - Battery/power-save/fullscreen policies apply to shaders identically to video (battery → frozen static frame).
  - Catalog gains a Shaders category (≥15 curated, license-verified) using the `content_type` field reserved in 3.1 — zero schema migration.
- Audio-reactive (PipeWire capture → FFT → iChannel0 texture) is a stretch **after** shader MVP. AC: synthetic 440Hz buffer lights the correct FFT texel bin (unit test); live capture degrades gracefully when PipeWire absent.

### 6.2 Catalog v2: community submissions
Only now, once traffic justifies moderation: submissions via GitHub PRs to the `fresco-wallpapers` repo — deliberately not an in-app upload flow (no accounts, no abuse surface; reuses GitHub identity + review tooling).
- **AC:** submission-to-published ≤1 week; CI validates license field, loop cleanliness (first/last frame diff), size caps; contributor credited in-app.

### 6.3 Wallpaper Engine library import (evaluate, then build if the spike is clean)
The killer migration story for the target audience: point Fresco at an existing Steam Workshop directory (`steamapps/workshop/content/431960/`) and import the **video-type** wallpapers (a large share of the popular ones — the media is an ordinary mp4/webm inside the item folder, plus a project.json with title/preview) as ordinary library entries. Legal line: reading the user's own locally-downloaded files is fine (this is what the community `linux-wallpaperengine` project does); **never redistribute Workshop content or ship it in the catalog.** Scene/web/application types are out of scope — detect and skip them with an honest count ("imported 34 video wallpapers; 12 scene/web items skipped — not supported").
- **AC:**
  - "Import from Wallpaper Engine" scans a user-picked Workshop directory, lists detected items with previews, and imports selected video-type items (title + preview from project.json) as normal LibraryEntry rows — no file copies unless the user opts to copy into the library.
  - Non-video types are skipped with a per-type count shown; nothing crashes on malformed project.json (fixture tests).
  - Imported entries survive the source directory disappearing gracefully (broken-file badge, relink flow — both already exist).
  - Docs/marketing copy never implies Workshop redistribution; feature copy reviewed against Steam subscriber agreement framing ("use your own downloaded library").

---

## Phase 7 — macOS (after Linux is won; earliest start after Phase 5)

Direction, not a committed plan — priced properly when Phase 5 ships. Shape of the port:

- **Shared Rust core** (config, library, catalog client, scheduler, update abstractions — kept portable since P3) compiled for macOS; a new **macOS renderer backend**: mpv (works on macOS) rendering into a borderless window at desktop level behind icons — the proven Plash/ScreenPlay approach — one window per NSScreen, with occlusion/battery pause via NSWorkspace/IOKit.
- **Native shell, not GTK**: the GTK4/libadwaita GUI does not carry over acceptably on macOS; the GUI layer is rebuilt native (SwiftUI shell talking to the Rust core over the existing IPC model). This is why the brain/backend split matters now.
- **Distribution**: notarized .dmg + Homebrew cask; the catalog works day one (it's platform-neutral — content is the cross-platform moat).
- **Go/no-go inputs (decided later):** Linux adoption trend post-P4, maintenance load of the GNOME extension, and a 1-week macOS renderer spike (mpv-behind-icons + click-through + multi-display on Sonoma+).

---

## Sequencing dependencies (explicit)

1. **Prove before advertise:** 1.2 before any Hyprland/KDE marketing; 4 before any "GNOME live" claim; 5.3 before restoring the unconditional hotplug claim. README violates this today — 1.2/1.4 remediate.
2. v0.0.91's update flow → Flathub (2.1): the updater must branch on packaging type before a second packaging type exists (shipped; 2.1 verifies in-sandbox).
3. Flathub (2.1) → measurement: its public stats are the adoption instrument for P3+.
4. Catalog `content_type` (3.1) → shaders (6.1): reserve the field now, no migration later.
5. GNOME spike (3.4) → Phase 4 scope → COPR/Fedora outreach only after GNOME live exists.
6. Native backend (5.1) → shaders (6.1): impossible on the mpvpaper substrate.
7. Curated catalog (3.1) → community submissions (6.2): never open submissions before moderation tooling and traffic exist.
8. Gallery before native backend (not after): the gallery is backend-agnostic; reversing the order delays every visible feature by ~2 months and puts the riskiest change in front of the least regression coverage.
9. Fidelity harness (1.8) → native backend (5.1): the harness built to convict today's quality bugs becomes the regression gate the new engine must pass — build it once, use it twice. Any HiDPI blur only partially fixable on the mpvpaper path is documented in 1.8 and closed permanently by 5.1's viewporter/fractional-scale AC.
10. Audio fix (1.7) → catalog curation (3.1): catalog items with sound are only worth curating once per-wallpaper audio provably works.

## Non-goals (killed, with reasons)

- **Web/HTML wallpapers** — stays killed (as at 0.0.1): WebKitGTK/CEF per monitor is a giant RAM/security/maintenance surface; popular web wallpapers are mostly reproducible as video or shaders. Revisit only if shader adoption proves demand.
- **yt-dlp / YouTube download** — ToS gray zone poisoning Flathub/AUR standing + perpetual bitrot + competes with the catalog. Direct media URLs (3.2) cover the legitimate need.
- **Lock-screen wallpapers** — GDM doesn't permit it; per-compositor divergence; payoff is one glance a day.
- **Snap** — audience covered by .deb + Flathub; hostile perception in the exact community being courted.
- **Mouse-parallax interactivity** — pointer access on Wayland background surfaces is privileged/fragile; niche payoff.
- **Accounts, ratings, in-app uploads, "marketplace"** — moderation/abuse/GDPR surface a solo dev cannot carry; GitHub-PR submissions capture ~90% of the value at ~5% of the cost, and preserve the no-accounts privacy differentiation.
- **Windows port, monetization** — out of scope by identity ("free forever, no paid tier"). macOS is NOT killed — it is deferred to Phase 7 and designed-for from Phase 3 (see macOS readiness principle).
- **macOS work before the Linux position is secured** — no macOS code, packaging, or marketing before Phase 5 lands; only the portability discipline applies until then.

## Do-not-touch list (hard-won stability)

The event-discarding X11 loop + 2s re-lower heuristic in Daemon::run; the WlOutput supervise/anti-flap/static-fallback ladder; mpv-before-window teardown ordering (NVIDIA); overview.rs save/restore semantics (T17). Features 2.2/3.1/3.2/3.3 never touch them; 1.3 touches only pause folding; 1.7 touches only mpv option/property paths (aid/mute/volume); 1.8 touches spawn options and possibly a small patch to the bundled mpvpaper (harness-first protects both); 5.1 is quarantined behind `FRESCO_NATIVE`.

## Measurement (no client telemetry)

- **Baseline (P1 week 1):** GitHub release download counts (API) + stars slope; then Flathub install stats + AUR votes.
- **Retention proxy:** catalog per-item server-side installs ÷ release downloads (target ≥40% by P3+4wk).
- **Reputation:** r/unixporn post performance; existing opt-in 👍/👎 feedback ratio trending positive.
- **Target:** 10x weekly installs vs baseline by end of Phase 4 (distribution 3–4x compounded with GNOME 2–3x).

## Key files (for implementers)

- engine-notes/ENGINE_IMPROVEMENTS.md — items C (hotplug) & D (cpu%) open; A & B already shipped
- docs/GNOME_LIVE_WALLPAPER_PLAN.md — the 3.4 spike + Phase 4 architecture (policy-based differentiation)
- docs/WAYLAND_VERIFICATION.md — the evidence ledger 1.2/2.2/5.x flip to PROVEN
- flatpak/io.github.dibbayajyotiroy.Fresco.yaml — stale draft 2.1 makes real
- supabase/schema.sql + admin/ — extend with catalog_items for 3.1
- src/gui/library.rs (LibraryEntry/entries.json) — the store catalog installs converge on
- src/ipc.rs (StatusReply) — additive monitors_info for 2.2
- src/daemon/mod.rs (PlayerHandle, run_wayland_layershell) — 1.4 re-enumeration; 5.1 NativeWayland variant

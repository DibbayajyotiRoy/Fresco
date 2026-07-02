# Fresco Development Task List

Derived from docs/ROADMAP.md (baseline v0.0.91). Tasks are in **build order** (dependency-minimizing), grouped by roadmap item. Every task carries a **Proof** — the test that demonstrates it works before it counts as done.

**Tags:**
- `AUTO` — buildable **and provable on this machine** (cargo, headless Sway + grim, Xvfb + xdotool, ffmpeg, ImageMagick compare). These are the overnight-run tasks.
- `MANUAL` — needs the human: real Hyprland/KDE/GNOME session, physical monitor, actually listening to audio, suspend/resume.
- `EXTERNAL` — needs accounts/review queues/other people: AUR, Flathub review, Supabase live project, content curation, launch posts.

**Standing rules for every task:** run `cargo fmt --check`, `cargo clippy -D warnings` (all-features, daemon-only, gui-only) and `cargo test --no-default-features --features daemon` before AND after; one local commit per completed task with its proof output in the commit message; **never push**; never modify the do-not-touch paths (X11 event-discard/re-lower loop, WlOutput supervise ladder, mpv-before-window teardown, overview.rs restore) except where a task explicitly scopes into them.

---

## Stage 0 — Baseline (do first, everything diffs against this)

### T0.1 `AUTO` Record a green baseline
- Build all three feature combos; run daemon tests; run `tests/ci/env-smoke.sh` under headless Sway and Xvfb via `tests/ci/with-compositor.sh`; run `tests/wayland/verify.sh` T1/T1A on headless Sway.
- **Proof:** all commands exit 0; capture output to `verification-artifacts/baseline-<date>/`. Any pre-existing failure is recorded, not absorbed.

### T0.2 `AUTO` Test-asset fixtures
- `tests/assets/make-fixtures.sh`: ffmpeg-generate committed-free (regenerated, gitignored) fixtures: 5s loop videos at 1080p/4K/8K (H.264 + VP9), one with a sine-audio track (440Hz), 1px checkerboard video, resolution-chart video, smooth-gradient video (8-bit + 10-bit), a tiny broken/corrupt file.
- **Proof:** script runs offline in <2 min; `ffprobe` confirms each fixture's resolution/codec/audio stream; harness scripts consume only these paths.

---

## Stage 1 — Core-promise defects (ROADMAP 1.7, 1.8, 1.1)

### T1.7.1 `AUTO` Audio repro harness (write the failing test first)
- New `tests/audio/verify-audio.sh`: start daemon with an entry `mute=false, volume=70` (sine fixture); on Wayland read mpv properties via the per-output IPC socket (`$XDG_RUNTIME_DIR/fresco/mpv-<out>.sock`); on X11 add a debug IPC/Status field or use `--once` mode property dump. Assert `aid != "no"`, `mute == false`, `volume == 70` within 3s.
- **Proof:** the harness runs and (expectedly) FAILS on current code if the bug is real — the failure output is the diagnosis. If it passes, bisect the user's repro (unmute-after-start path, cold-boot ordering) until a failing case is captured. Commit the harness with findings.

### T1.7.2 `AUTO` Fix the audio state machine
- Suspects in order: (a) spawn opts bake `aid=no` for muted entries and unmute only flips `mute` — fix: unmute restores `aid=auto` (X11 `Player::set_option`/property; Wayland via MpvIpc `set aid auto`), re-mute returns to `aid=no`; (b) Apply with `mute=false` still spawning with `aid=no`; (c) volume applied before track selection.
- Files: `src/daemon/mpv/player.rs`, `src/daemon/mpvpaper.rs`, `src/daemon/mod.rs` (apply paths).
- **Proof:** T1.7.1 harness passes on BOTH backends; toggle test: apply muted → assert `aid=no` (RAM saving intact) → unmute via IPC → assert `aid!=no, mute=false` → re-mute → assert `aid=no` again. All existing tests stay green.

### T1.7.3 `AUTO` Audio-server-not-ready resilience
- On spawn, if mpv reports no audio device / ao init failure for an unmuted entry, retry audio init (cycle `audio-device` or reload) with backoff for ~30s. Detect via mpv property/log.
- **Proof:** unit-testable retry logic + headless run with `--ao=null` forced absent → daemon logs retry attempts and succeeds once available (simulate by flipping an env var between attempts). No behavior change for muted entries.

### T1.7.4 `MANUAL` Listen test
- Roy: PipeWire machine + one PulseAudio machine/VM; set unmuted wallpaper, hear sound; toggle from status pill; reboot → sound persists on login without re-apply.
- **Proof:** results recorded in docs/WAYLAND_VERIFICATION.md ledger (new A1–A3 rows).

### T1.8.1 `AUTO` Fidelity harness
- New `tests/fidelity/verify-fidelity.sh` + reference generator: play checkerboard/res-chart/gradient fixtures on headless Sway outputs configured at 4K scale 1, scale 1.25, scale 2 (and Xvfb 4K for X11); grim/import capture; ImageMagick `compare` — pixel-exact (AE=0) required at integer scales for the checkerboard center crop; SSIM/PSNR threshold elsewhere; also assert Wayland buffer size == physical px (`swaymsg -t get_outputs` current_mode vs captured size).
- **Proof:** harness runs end-to-end and emits a scorecard (`verification-artifacts/fidelity-<date>/report.json`). Current-code failures are the conviction list, committed as findings. New T-rows drafted in docs/WAYLAND_VERIFICATION.md.

### T1.8.2 `AUTO` Visually-correct scaler defaults
- `balanced` gains `correct-downscaling=yes`, `linear-downscaling=yes`, `dither-depth=auto`, `scale=spline36`, `dscale=mitchell` (both backends: `player.rs` opts + `mpvpaper.rs` spawn opts); `high` stays Lanczos + same correctness flags; add optional `deband` config field (default off).
- **Proof:** T1.8.1 rerun — gradient banding check passes (dither active asserted via mpv property), 8K→4K res-chart SSIM vs `gpu-hq` reference within threshold, checkerboard still pixel-exact at scale 1 (no regression); config roundtrip test for the new field; CPU delta measured and recorded (<15% relative increase on the 4K soak, else tune).

### T1.8.3 `AUTO` Wayland HiDPI buffer correctness
- From T1.8.1 evidence: if buffer < physical px at scale >1, patch bundled mpvpaper (scripts/build-mpvpaper.sh gains a patch step) or set integer `buffer_scale`; document any fractional-scale residual as known-limited-until-5.1.
- **Proof:** T1.8.1 scale-2 case: buffer == physical px, checkerboard pixel-exact; scale-1.25 case documented with measured SSIM either way.

### T1.8.4 `AUTO` Decode honesty
- `StatusReply` gains `source_w/h`, `bit_depth`, `dropped_frames` (serde-default, additive); fill from mpv `video-params`/`frame-drop-count` on both backends; `fresco doctor` + `frescod --check` print them; warn when source resolution/codec exceeds probed decode caps (parse `vainfo` when present; degrade gracefully without it).
- **Proof:** ipc serde tests; headless run shows real values (4K fixture → 3840×2160 reported); doctor output includes the warning line when an 8K fixture is played with hwdec absent (headless = software decode, which is exactly the warning case); GUI builds with the extra fields ignored or shown.

### T1.8.5 `AUTO` Adaptive demuxer cache for ≥4K
- Raise the 16MiB/4MiB/1s demuxer caps for sources ≥4K or high bitrate (bounded, e.g. 64MiB), both backends.
- **Proof:** 10-min 4K@60 headless soak: `frame-drop-count` ≤0.1% of frames, RSS bounded (recorded before/after); 1080p RSS unchanged (±10%).

### T1.8.6 `MANUAL` Real-display fidelity check
- Roy: 4K (and any HiDPI-scaled) physical monitor — checkerboard fixture sharp to the eye at 1:1, no banding on the gradient, a real 4K wallpaper visibly crisp vs pre-fix build.
- **Proof:** before/after photos or captures + ledger rows.

### T1.1.1 `AUTO` Updater stderr capture
- `src/update.rs::run_updater_with_progress` drains stderr (thread) and includes its tail in `UpdateOutcome::Failed`; `src/gui/updates.rs` failure dialog shows it.
- **Proof:** unit test with a fake updater script that writes to stderr and exits 1 → Failed contains the stderr text; clippy/fmt green.

### T1.1.2 `AUTO` Real cpu_percent
- `proc_stats()` samples `/proc/self/stat` (+ child mpvpaper PIDs on Wayland) over the existing poll interval; delta-based %.
- **Proof:** unit test on the sampling math; headless run: Status `cpu_percent > 0` during 4K playback, near 0 when paused; status pill displays it (build check).

---

## Stage 2 — Parity + truth (ROADMAP 1.3, 1.4, 1.2)

### T1.3.1 `AUTO` X11 fullscreen auto-pause
- New `src/daemon/x11_fullscreen.rs`: every 2s (piggyback existing cadence) read `_NET_CLIENT_LIST_STACKING`, check `_NET_WM_STATE_FULLSCREEN`, intersect geometry with monitor rects → covered connectors. Refactor `apply_pause` into per-renderer `reconcile_pause(user || battery || fullscreen_here)` mirroring WlOutput's. Poll-based only — never react to X events (restack lesson).
- **Proof:** Xvfb CI script: map an mpv/xterm window, `xdotool key`-toggle fullscreen (or set the EWMH state via xdotool/xprop) → daemon logs pause ≤3s and time-pos freezes; unset → resume ≤3s; battery + manual pause tests still green; cold-boot-heal skip assertion (paused renderers excluded from stall sampling — unit/log test); 20-toggle loop leaves stacking intact (screenshot compare).

### T1.4.1 `AUTO` Wayland Apply-time output re-enumeration
- In `run_wayland_layershell`'s Apply handler: re-run `wayland_outputs::list_outputs()` and reconcile before applying.
- **Proof:** headless Sway: start daemon with 1 output → `swaymsg create_output` → send Apply → assert a second mpvpaper spawns for the new output (pgrep + Status); disable output → next Apply reaps it.

### T1.4.2 `AUTO` Truthful claims pass
- README.md + landing content: scope hotplug claim ("live hotplug on X11; Wayland displays picked up on apply — automatic hotplug tracked for v1.0"); mark Hyprland/KDE "verified on Sway; Hyprland/KDE verification in progress" until T1.2.x flips them; fix stale metainfo.xml releases list.
- **Proof:** grep assertions (no unscoped "live hotplug handling"); `appstreamcli validate` on metainfo; landing builds (`pnpm build` in landing/).

### T1.2.1 `AUTO` Harden Hyprland/KDE CI legs
- `tests/ci/with-compositor.sh`: pin known-good Hyprland env (post-aquamarine headless vars), add grim T1 screenshot check to hyprland + kde legs; keep `min_pass` soft-gate.
- **Proof:** 10 consecutive local runs of the hyprland leg ≥9 pass with screenshot check on (loop the harness); kde leg captures via screencopy or documents the D-Bus fallback.

### T1.2.2 `MANUAL` Real-session verification (Hyprland + KDE Plasma 6)
- Roy or community testers: live USB / real sessions, run `tests/wayland/verify.sh`; specifically test KDE FolderView occlusion (engine-notes item F) and `plasmashell --replace`; suspend/resume on one physical machine.
- **Proof:** ledger rows T1/T1A/T2/T6/T14/T15 flip to PROVEN with committed artifacts; claims un-scoped in README where proven, downgraded where not.

---

## Stage 3 — Reach (ROADMAP 1.5, 1.6, 2.1)

### T1.5.1 `AUTO` PKGBUILD authoring
- `packaging/aur/fresco/PKGBUILD` (source: cargo build + bundled mpvpaper or system-mpvpaper dep) and `packaging/aur/fresco-bin/PKGBUILD` (repack .deb); .SRCINFO for both.
- **Proof:** shellcheck clean; PKGBUILD lints (namcap requires Arch — defer to T1.5.2); source PKGBUILD's build() steps mirror CI exactly (diff against `_release.yml`).

### T1.5.2 `EXTERNAL` AUR publish
- Roy: AUR account, push both packages, add to release checklist; clean-chroot `makepkg` + `namcap` on an Arch VM.
- **Proof:** `yay -S fresco-bin` works on a clean Arch VM; wallpaper sets on Hyprland (doubles as T1.2.2 evidence).

### T1.6.1 `AUTO` Real screenshots (best-effort headless)
- Run the GUI under Xvfb at 1920×1080 with a populated demo library (fixtures + thumbnails), capture with `import`/grim: library view, editor view, status pill. Replace `picsum.photos` blocks in landing/src and the README hero.
- **Proof:** zero `picsum.photos` matches in landing/src; screenshots present in landing/public + referenced; `pnpm build` green; captures visually sane (non-blank, checked via ImageMagick mean-brightness bounds).

### T1.6.2 `MANUAL` Demo video + launch
- Roy: ≤60s screen capture on a real session; record baseline metrics (GH API downloads/stars); one launch post (r/unixporn or Show HN) with the next release.
- **Proof:** video linked on landing + README; baseline JSON committed to `verification-artifacts/metrics-baseline.json`.

### T2.1.1 `AUTO` Flatpak readiness code
- `src/autostart.rs`: Background-portal path when `is_flatpak()` (gio D-Bus via gtk's gio; keep file-write path otherwise). Verify update UI + notifier fully suppress update prompts in-sandbox (audit + tests). Refresh `flatpak/io.github.dibbayajyotiroy.Fresco.yaml`: current tag/commit, current GNOME runtime, mpvpaper + libmpv modules.
- **Proof:** unit/feature tests for the branch logic (`is_flatpak()` faked via env/file); manifest passes `flatpak-builder-lint` if flatpak-builder available locally, else YAML-validated + reviewed against docs/FLATHUB.md checklist; no pkexec call sites reachable when flatpak-detected (grep + test).

### T2.1.2 `EXTERNAL` Flathub build + submission
- Roy: local `org.flatpak.Builder` full build, in-sandbox smoke (`frescod --check`, X11 + Sway wallpaper, portal autostart), submit to Flathub, review round-trips, landing badge swap.
- **Proof:** ROADMAP 2.1 AC checklist ticked; Flathub listing live.

---

## Stage 4 — Multi-monitor GUI + scripting (ROADMAP 2.2, 2.3)

### T2.2.1 `AUTO` `monitors_info` in StatusReply
- `src/ipc.rs`: `MonitorInfo{connector,w,h,x,y}` + `monitors_info: Vec<_>` with `#[serde(default)]`; fill in all three daemon status paths (X11 from RandR cache, Wayland from output list — ALL outputs, not just wallpapered ones; GNOME-static likewise).
- **Proof:** serde compat tests (old reply parses, new field defaults); headless Sway 2-output: both listed with geometry; Xvfb: RandR monitors listed.

### T2.2.2 `AUTO` Per-monitor assignment GUI
- `src/gui/window.rs` (+ small new module): display strip of to-scale rectangles from `monitors_info`; card context menu gains "Set on display ▸ <connector> / All displays"; apply path writes `config.monitors` and calls `ensure_daemon_and_apply`; override badge on assigned cards; `entry_is_active` made monitor-aware; strip hidden when 1 monitor.
- **Proof:** GUI builds + runs under Xvfb (smoke: window maps, no criticals in stderr); config-write unit test: set-on-display mutates only `[monitors."X"]`, "All displays" removes it; headless Sway 2-output end-to-end: two different fixtures per connector → two mpvpaper processes with correct file args (T9 evidence); reboot-persistence = config reload test.

### T2.3.1 `AUTO` IPC scripting docs
- `docs/SCRIPTING.md`: socket location + protocol, 5 recipes (set from script, per-output set, pause/resume, status JSON via `jq`, playlist-next) as copy-paste `printf | socat`/python one-liners.
- **Proof:** each recipe executed verbatim against a live daemon in headless Sway; outputs captured into the doc.

---

## Stage 5 — Content engine (ROADMAP 3.2, 3.3, 3.1)

### T3.2.1 `AUTO` Download worker (shared, portable)
- New `src/download.rs` (no Linux-isms in core types): fetch to `<library>/downloads/<name>.part` → rename; Content-Length pre-check + hard cap + mid-stream abort; progress callback; cancellation token. GUI "Add from URL" dialog (direct `.mp4/.webm/.mkv/.gif` etc. only) → LibraryEntry + thumbnail via existing paths.
- **Proof:** `cargo test` against a local fixture HTTP server (std TcpListener in-test): success, over-cap refusal (pre + mid-stream), cancel-no-partial, bad-URL error string; GUI dialog smoke under Xvfb; SIGKILL mid-download leaves no partial in library (temp `.part` orphan is cleaned on next scan — test).

### T3.3.1 `AUTO` Schedule engine
- New `src/schedule.rs`: pure `desired(&Schedule, DateTime<Local>) -> Option<&Wallpaper>`; `Schedule` config types (`daynight` / `[[at]]` slots / `solar` with manual lat/lon, hand-rolled NOAA sunrise); `chrono` dep.
- **Proof:** unit fixtures: midnight wrap, DST spring/fall (fixed-offset simulation), 3 NOAA sunrise cases ±2 min vs published tables; serde: config without `schedule` roundtrips byte-identical.

### T3.3.2 `AUTO` Daemon schedule evaluation
- Both loops evaluate on existing cadence; on change swap via media-only `load_path` (never `rebuild()`); manual Apply sets hold-until-next-boundary; pause state preserved across swaps.
- **Proof:** headless Sway + Xvfb: boundary at now+60s → Status name changes ≤90s, daemon PID stable, log asserts loadfile (no rebuild); paused-before → paused-after; manual-Apply-hold test.

### T3.3.3 `AUTO` Schedule GUI
- Editor gains a Schedule section: day/night entry pickers + time rows + solar coords.
- **Proof:** Xvfb smoke; config write assertions; disabled state leaves config untouched.

### T3.1.1 `AUTO` Catalog schema + admin + client (fixture-backed)
- `supabase/schema.sql`: `catalog_items` (content_type, title, category, tags, media_url, thumb_url, size_bytes, checksum, license NOT NULL, author, source_url, published, install_count) + RLS anon-SELECT-published, mirroring notifications; admin/ page for CRUD + counts. New `src/catalog.rs` (portable: fetch/parse/cache types; ureq behind a small trait so tests inject fixtures); `src/gui/gallery.rs` Stack page: card grid (reuse library card pattern), category filter, search, license+author on every card, download via T3.2.1 worker → LibraryEntry with `catalog_id`.
- **Proof:** `cargo test`: catalog JSON fixture parse, cache round-trip (`~/.cache/fresco/gallery/`), download→entry integration against local HTTP fixture; offline start → cached/empty-state (no crash) under Xvfb; RLS asserted by SQL review + (when live) anon-key curl test; GUI smoke under Xvfb with fixture catalog: cards render, ≤3-click apply path wired.

### T3.1.2 `EXTERNAL` Catalog goes live
- Roy: apply schema to the Supabase project; create `fresco-wallpapers` GH repo (or R2 bucket); curate ≥50 CC0/verified loops across ~8 categories (loop-cleanliness checked: first/last frame diff); publish via admin app.
- **Proof:** ROADMAP 3.1 AC end-to-end on a real install; per-item counts visible in admin.

### T3.4.1 `MANUAL` GNOME live spike (timeboxed 1 week)
- Real GNOME 46+ Wayland session: mpv renderer window (`--wayland-app-id=fresco-renderer`) + ~50-line gjs extension cloning it into the background group. Kill -9 ×10 test, coverage-pause CPU measurement.
- **Proof:** `docs/GNOME_SPIKE_DECISION.md` committed with measurements → GO/NO-GO gates Phase 4. (AUTO-assist: the extension skeleton, renderer spawn code, and measurement scripts can be written headlessly in advance.)

---

## Stage 6 — The big bets (sequenced by 3.4 outcome)

### T4.x GNOME Shell extension (if GO) — `MANUAL`-heavy, AUTO-assist
- Full extension (policy engine: occlusion/overview/idle/lock/battery), `Capability::WaylandGnomeExtension` probe + `run_gnome_extension()` daemon branch, enable flow in GUI, version matrix.
- **Proof:** ROADMAP Phase 4 AC (journal-clean loads on 2 GNOME stables, measured policy wins vs Hanabi, kill -9 isolation, fallback intact). Extension JS unit-testable pieces (state machine) get gjs tests; the rest is session-manual.

### T5.1.x Native Wayland backend — `AUTO`-provable behind `FRESCO_NATIVE=1`
- T5.1.1 `AUTO` EGL + layer-shell surface module (`src/daemon/native_wl/`): wp_viewporter + wp_fractional_scale from day one; empty input region.
  **Proof:** surface maps on headless Sway (grim shows solid-color test render); buffer==physical px at scales 1/1.25/2.
- T5.1.2 `AUTO` `mpv_render_context` FFI extension + render loop; `PlayerHandle::NativeWayland`.
  **Proof:** `FRESCO_NATIVE=1 tests/wayland/verify.sh` T1/T1A pass; fidelity harness (T1.8.1) pixel-exact at integer scales AND ≥threshold at fractional (the permanent HiDPI fix); audio harness (T1.7.1) passes.
- T5.1.3 `AUTO` Supervision parity (frozen-context detect + rebuild, Results-not-panics).
  **Proof:** induced-stall test respawns context without daemon restart; 8h soak RSS <10MB growth (overnight-friendly).
- T5.2.1 `AUTO` Single-decode multi-output (one core → FBO → blit per output).
  **Proof:** 2-output same-video: Status reports 1 core; RSS ≤1.5× single-output; per-output sizes correct via grim.
- T5.3.1 `AUTO` Registry-driven hotplug.
  **Proof:** create_output → wallpaper ≤5s; disable → surface destroyed, RSS baseline; T10/T11 PROVEN; then README hotplug claim restored.
- T5.4 `MANUAL` NVIDIA box + real-session matrix before default flip; mpvpaper removed only at v1.0 criteria (see ROADMAP 5.3).

### T6.1.x Shaders (after 5.1) — `AUTO`-provable
- Shadertoy-subset wrapper (iTime/iResolution/iFrame), .frag load/paste, fps cap, compile-fail → previous wallpaper + Status error; catalog `content_type=shader`.
  **Proof:** animated .frag on headless Sway → two grim frames differ; broken .frag → previous wallpaper retained (grim identical), error in Status; fps-cap frame-callback count test; battery-pause test. Audio-reactive later: FFT unit test (440Hz fixture → correct bin).

### T6.3.1 `AUTO` Wallpaper Engine import (parser + GUI; evaluate first)
- Scanner for `steamapps/workshop/content/431960/*/project.json`; import video-type as LibraryEntry (no copy by default); skip scene/web with per-type counts.
  **Proof:** fixture Workshop dir (synthetic project.json files: video, scene, web, malformed) → correct import/skip counts, no crash on malformed; imported entry plays (fixture video); missing-source → broken-badge + relink flow works.

---

## Overnight execution order (AUTO tasks only)

T0.1 → T0.2 → T1.7.1 → T1.7.2 → T1.7.3 → T1.8.1 → T1.8.2 → T1.8.3 → T1.8.4 → T1.8.5 → T1.1.1 → T1.1.2 → T1.3.1 → T1.4.1 → T1.4.2 → T1.2.1 → T2.2.1 → T2.2.2 → T2.3.1 → T3.3.1 → T3.3.2 → T3.3.3 → T3.2.1 → T3.1.1 → T1.5.1 → T2.1.1 → T1.6.1

Stage 6 tasks (native backend, shaders, WE import) are deliberately **not** in the overnight queue: they're gated on Stage 1–5 landing and on the 3.4/real-session outcomes per ROADMAP sequencing. Each completed task = one local commit (message cites task ID + proof result). **No pushes. No release tags.** MANUAL/EXTERNAL tasks are queued for Roy in `docs/TASKS-FOR-ROY.md` as they become unblocked.

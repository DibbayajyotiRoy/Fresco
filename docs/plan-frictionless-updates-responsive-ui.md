# Plan — Frictionless in-app updates, responsive viewport, clearer UI

**Repo:** `fresco` (live wallpapers for Linux) — GTK4 + libadwaita Rust GUI (`fresco`) + daemon (`frescod`), Next.js landing page under `landing/`.
**Author of plan:** exploratory pass over the codebase (2026-07-01).
**Executor target:** Claude Sonnet 5 driving subagents + a review Workflow. This document is the source of truth; each task lists exact files, the change, and acceptance criteria.

> **Ground rule for the executor:** read every file listed in a task before touching it. Follow existing patterns (see the anchors below). Keep diffs surgical. Build with the right feature flags after every change (`--features gui` and/or `--features daemon`). Do **not** reformat untouched code.

---

## 1. Why (the three problems, in the user's words)

1. **Update UX is a detour.** "For automatic download, and no separately navigating to github releases page and from there he will download it again." The user wants the app (and landing page) to fetch → download → install the latest version itself, with no manual trip to the GitHub releases page.
2. **Viewport is not dynamic.** "Make the viewport of the desktop app more dynamic for all screen sizes." The window/library should adapt from tiny tiling-WM tiles to 4K.
3. **UI should be more meaningful and easy to use.** Communicate state and next actions clearly.

---

## 2. Current state (verified against the code)

### 2.1 Update path — what already exists
- `scripts/fresco-update.sh` already **downloads the latest `.deb` from the GitHub Releases API and `apt-get install`s it**. It is bundled into the `.deb` at `/usr/lib/fresco/fresco-update.sh` (see `Cargo.toml` `[package.metadata.deb].assets`). It runs as root (no inner `sudo`) because it is launched via `pkexec`.
- `src/daemon/notifier.rs` holds a Supabase Realtime websocket. When an **admin manually publishes** an `update` row, the daemon raises a **desktop notification** with an "Update now" button → `run_updater()` → `pkexec /usr/lib/fresco/fresco-update.sh` (`notifier.rs:309-330`). Flatpak installs get an "Open" button to the releases page instead (`notifier.rs:240-245`). Semver gating in `is_newer()` (`notifier.rs:254-263`). Script location logic in `updater_script()` (`notifier.rs:334-344`).

### 2.2 Update path — the gaps that make it "bad UX"
- **The GUI has no update UI at all.** It only polls Supabase notifications (`window.rs:2119-2139`), shows a toast → modal whose only action is **"Open link" → `xdg-open` a URL** (`window.rs:2191-2201`), i.e. it sends the user to the GitHub releases page. There is **no in-app "check for updates"** and **no in-app "install now."**
- **No self-driven version check.** The GUI never queries GitHub Releases to compare its own `CARGO_PKG_VERSION` against the latest tag. Update *discovery* is push-only and depends on an admin remembering to publish a Supabase row.
- **The one working auto-installer is reachable only from a daemon desktop notification** — the flakiest channel (many DEs, notably GNOME, silently drop notification action buttons). When that button never fires, the only fallback is the releases page + manual download.
- **Landing page primary CTA points at the releases *listing* page** (`landing/src/components/download.tsx:64-75` → `RELEASES_URL`), so web visitors browse and download by hand. A `curl | bash` one-liner exists (`INSTALL_ONELINER`) but is secondary.

### 2.3 Viewport / responsiveness — current state
- Window: `set_default_size(880, 660)`, `set_size_request(420, 480)` (`window.rs:66-67`). **No `adw::Breakpoint` anywhere** → no adaptive layout.
- Cards are **fixed size**: library card `set_size_request(230, 130)` (`window.rs:706, 711`), mini card `150×84` (`window.rs:648, 652`). They don't scale with the viewport — dead space on large screens, cramped on small ones.
- `FlowBox` reflow is capped: `min_children_per_line(2)`, `max_children_per_line(6)` (`window.rs:617-618`).
- The library content is **not width-clamped**, so on ultrawide/4K rows stretch arbitrarily. (The editor view already uses `adw::Clamp` at `window.rs:1316-1320` — the pattern exists to reuse.)

### 2.4 UI meaningfulness — current state
- Header is just a title + hamburger menu (`window.rs:171-179`). **No "now playing" / status surface.**
- The daemon already exposes rich status over IPC — `StatusReply { running, paused, hwdec, wallpaper, cpu_percent, rss_mb, monitors, error }` (`src/ipc.rs:22-35`) — but the library view never shows any of it.
- CSS for a status surface **already exists but is unused in the library**: `.status-pill`, `.dot-ok`, `.dot-warn`, `.dot-off` (`theme.rs:205-209`).
- The IPC `Request` enum has `Apply/Stop/Pause/Resume/Status` (`ipc.rs:12-20`) but **no `Update`**.
- Config persistence (`src/config.rs:186-222`) has `last_seen_version`, `first_run_epoch`, `feedback_prompted`, `seen_notifications`, etc. — but **no update-check bookkeeping**.

---

## 3. Goals & success criteria

| # | Goal | Done when… |
|---|------|-----------|
| G1 | In-app update with zero browser detours | A user on an out-of-date apt/.deb install sees an in-app "Update available" banner, clicks **Update now**, authenticates once via polkit, and the app installs the new `.deb` and offers **Restart** — without ever opening a browser or GitHub. |
| G2 | Self-driven discovery | The GUI detects a newer release on its own (GitHub Releases API), throttled to ≤ once/day, independent of any admin Supabase push. Manual **Check for updates** menu item also works. |
| G3 | Graceful non-apt fallback | Flatpak / non-Debian installs get a correct path (Flathub or one-liner), never a broken `apt install`. |
| G4 | Responsive viewport | The library looks intentional and usable at 360×640, 1366×768, 1920×1080, and 3840×2160: cards scale, columns reflow, content is width-clamped on ultrawide, compact layout works at min size. |
| G5 | Meaningful status | The library surfaces live status (playing/paused, wallpaper name, hwdec, CPU) using the existing `StatusReply` + status-pill CSS. |
| G6 | Landing page one-step install | The landing hero/download CTA gives a direct `.deb` link and/or a copy-run one-liner as the primary action, not the releases listing page. |

Non-goals for this pass: signing/notarizing the `.deb`, a full per-monitor assignment UI, delta updates, an in-app media browser/store.

---

## 4. Workstreams

Four workstreams. **WS0 is an enabling refactor that must land first** because it removes duplication and, critically, reduces edit contention on the 2 200-line `window.rs` so the other workstreams can be parallelized safely.

### WS0 — Enabling refactor (do first, sequential)

**0a. Extract a shared update module.** Create `src/update.rs` (compiled for both `gui` and `daemon`) that owns:
- `current_version() -> &str` (wrap `env!("CARGO_PKG_VERSION")`).
- `is_newer(candidate, current) -> bool` — move the semver logic out of `notifier.rs:254-263`.
- `updater_script() -> Option<PathBuf>` — move from `notifier.rs:334-344`.
- `run_updater_blocking() -> Result<UpdateOutcome>` — the `pkexec` invocation, returning structured stages/outcome instead of only logging.
- A `LatestRelease { version, deb_url, notes_url }` fetcher hitting `https://api.github.com/repos/DibbayajyotiRoy/fresco/releases/latest` (unauthenticated; 60 req/hr/IP is ample for once-a-day). Reuse `ureq` (already in **both** feature sets — `gui` and `daemon` both depend on it).
- Move `is_flatpak()` reference usage here or keep calling the existing crate root helper.

Update `Cargo.toml`: add `semver` to the **`gui`** feature (currently only `daemon` has it) so the GUI can compare versions. Repoint `notifier.rs` to call `crate::update::*` (behavior unchanged — this is a pure move; verify `--features daemon` still builds and the daemon still updates).

**0b. Split `window.rs` into focused GUI modules** to cut contention. Extract, without behavior change:
- `src/gui/updates.rs` — will own the new update banner/dialog/flow (WS1). For now, create the module and move the existing `build_update_banner` / changelog "What's new" helpers (`window.rs:1736-1979`) here.
- `src/gui/status.rs` — will own the new status surface (WS3). Create empty scaffold + a `pub fn build_status_pill(...)`.
- Keep `window.rs` as the composition root that calls into these.

**Acceptance (WS0):** `cargo build --features gui` and `cargo build --features daemon` both succeed; no behavior change; `git diff` is a mechanical move (reviewer can confirm no logic edits). This is the only workstream that reshapes `window.rs`'s module boundaries — everything after edits *different* files.

---

### WS1 — Frictionless in-app updates (headline)

**Delivery model — poll vs. push (decided).** A desktop client cannot receive a GitHub webhook directly (webhooks require a public HTTPS endpoint; GitHub offers no SSE for releases), so any push path must go through a relay — which we already run (Supabase). The daemon *already* holds a Supabase Realtime websocket and is pushed `update` rows (`notifier.rs:106-117`); that push channel exists today. Design decision for this pass:
- **GUI (short-lived): poll on launch.** A window open for seconds gains nothing from holding a socket; the on-launch GitHub check (1a) is the most reliable moment and GitHub is the source of truth. **No SSE in the GUI.**
- **Daemon (long-running): keep the existing Realtime push**, and optionally **automate its trigger** (1g) so releases self-publish. Push latency is irrelevant for a wallpaper updater (minutes vs. a day changes nothing); the value of automation is "never forget to publish," not speed.
- **Realtime stays reserved for announcements** (the existing `info` kind), where instant delivery genuinely matters.

**1a. Client-driven update check.** In `src/gui/updates.rs`, on GUI startup (call from `run_startup_checks`, `window.rs:1990-2012`, alongside `poll_notifications`) spawn a background thread (mirror the `poll_notifications` thread + `glib::spawn_future_local` pattern at `window.rs:2119-2139`) that:
- Throttles: skip if `now - config.last_update_check < 24h`. Add `last_update_check: u64` and `update_skipped_version: String` to `Config` (`src/config.rs:186-222`, with `#[serde(default)]` like the neighbours) and persist after each check.
- Calls `update::fetch_latest()`, compares with `update::is_newer`.
- On newer → surface the **update banner** (1b). Never blocks the UI thread; failures are logged and ignored (offline is normal).

**1b. In-app "Update available" banner + dialog.** Reuse the existing `.banner` CSS (`theme.rs:211-212`) and the insertion point where the "What's new" banner is added (`window.rs:181-184`). Banner reads: **"Fresco X.Y.Z is available"** with actions **Update now** · **What's new** · **Later**.
- **What's new** → reuse `show_changelog_modal` (`window.rs:1907-1979`) if notes are local, else open the release notes.
- **Later** → set `update_skipped_version = X.Y.Z`, hide until a newer version appears.
- **Update now** → open a small modal (reuse the `glass` dialog pattern, e.g. `show_notification_modal` at `window.rs:2168-2206`) with an indeterminate `gtk4::Spinner` and staged labels ("Downloading…" → "Installing…" → "Done").

**1c. Run the installer from the GUI.** Preferred: the GUI runs the existing script directly — `pkexec <update::updater_script()>` on a background thread — so it works **even when the daemon is not running** and reuses the exact, already-shipped installer. polkit prompts once; no passwords stored. On success show **"Updated — Restart now"** (offer a button that relaunches: spawn the new `fresco` and quit).
- Also add IPC `Request::Update` (`src/ipc.rs:12-20`) + daemon handler so the daemon path and GUI path converge on one code path; but the GUI's default is to run it itself (don't hard-depend on the daemon being alive). Keep the daemon's existing notification path working.

**1d. Manual "Check for updates".** Add a menu item to the hamburger popover (`build_menu_popover`, `window.rs:340+`) that runs the check immediately (ignoring the 24h throttle) and, if already current, toasts "You're on the latest version (X.Y.Z)."

**1e. Harden `scripts/fresco-update.sh`.** Keep it drop-in compatible with the daemon caller. Add: fail clearly on no network; verify the downloaded file is non-empty and a real `.deb` (`dpkg-deb -I`); print stage markers on their own lines (`STAGE: downloading` / `STAGE: installing` / `STAGE: done`) so the GUI can advance the dialog; no-op with a distinct exit code if the installed version already matches latest; detect Flatpak/non-apt and exit with a documented code so the GUI routes to fallback. Document exit codes in a header comment.

**1f. Non-apt fallback.** In `update.rs`, branch on `is_flatpak()` / absence of `apt-get`: route to Flathub (when published) or show `INSTALL_ONELINER`/direct `.deb` link in the dialog instead of attempting `apt install`.

**1g. (Optional) Automate the daemon push trigger.** Removes the "admin must remember to publish" gap so a GitHub release self-announces to every running daemon. No client changes — reuses the existing Realtime subscription (`notifier.rs:106-117`) and "Update now" handler.
- GitHub repo → Settings → Webhooks → `release` event → POST to a **Supabase Edge Function**.
- Edge Function verifies `X-Hub-Signature-256` (HMAC with the webhook secret), parses the release, and `INSERT`s a `notifications` row (`kind='update'`, `version=<tag>`, `url=<release url>`). Existing RLS already lets anon read published rows; Realtime already broadcasts the INSERT.
- **Simpler alternative (no server code):** have the daemon poll the GitHub Releases API on a slow timer (e.g. every few hours) and reuse the same semver-gated notify path. Prefer this if you don't want to own an Edge Function + webhook secret — the only thing you lose is minutes-vs-hours latency, which doesn't matter here.
- New surface if taken: one Supabase Edge Function (TS) + a webhook secret; no Rust GUI changes. Verify with a test release that a running daemon raises the "Update now" notification without any manual publish.

**Acceptance (WS1):**
- Build a `.deb` at an artificially low version, install it, publish nothing to Supabase, launch `fresco` → banner appears within a few seconds; **Update now** installs latest and offers Restart; no browser opens.
- `frescod --check`/daemon path still works.
- Offline launch: no banner, no error dialog, no hang.
- Flatpak build: no `apt` attempt; fallback path shown.
- `cargo build --features gui` and `--features daemon` clean; existing tests pass (`src/ipc.rs` tests still green after adding `Update`).

---

### WS2 — Dynamic viewport for all screen sizes

Touches `src/gui/window.rs` (layout wiring) and `src/gui/theme.rs` (CSS). Coordinate with WS3 (both add to the library header/top region) — assign the *same* subagent to WS2+WS3 **or** land WS2 first, since both edit the library view builder.

**2a. Add breakpoint-style adaptive layout to the window.** **Correction (verified during execution):** `AdwBreakpoint` requires libadwaita ≥ 1.4; this system builds against libadwaita 1.1.7 with the `v1_1` binding feature only — that widget does not exist here. Use the standard pre-1.4 GTK4 fallback instead: `window.connect_notify_local(Some("default-width"), ...)` (a real, always-available `GtkWindow` property-change signal that fires on interactive resize) to read the current width and apply/remove a small set of CSS classes + adjust FlowBox column caps at defined thresholds:
- **compact** (< 600px): 1–2 columns, tighter margins (8px), condense footer buttons to icons.
- **regular** (default): current spacing.
- **wide** (≥ 1200px): more columns, wider margins.
Debounce trivial: only act when the resolved bucket actually changes, not on every pixel.

**2b. Make cards fluid instead of fixed.** Replace the hard `set_size_request(230, 130)` / `150×84` (`window.rs:706-711`, `648-652`) with a **minimum** width + aspect-preserving height:
- Wrap the thumbnail in `gtk4::AspectFrame` (ratio 16:9) so height derives from allocated width; set a sensible `min` (e.g. 180px) and let cards grow.
- Set the `FlowBox` `homogeneous(true)` and drive columns by available width rather than a fixed footprint, so cards scale up on 4K and reflow to fewer columns when narrow.
- Preserve the existing overlay/scrim/badge structure (`window.rs:719-761`); only the sizing strategy changes.

**2c. Width-clamp the library** by wrapping the scrolled content in `adw::Clamp` (reuse the editor's pattern, `window.rs:1316-1320`) with a max content width (~1280–1400px) so ultrawide/4K rows stay centered and readable.

**2d. Raise/derive column caps.** Replace the fixed `max_children_per_line(6)` (`window.rs:617`) with breakpoint-driven values (e.g. 8–10 on wide, 2 on compact). Keep `min_children_per_line` at 1 for compact.

**2e. Compact-mode audit.** Verify the app is usable at `set_size_request` minimum; consider lowering min width toward ~360 for tiling WMs after confirming the compact layout holds (header title ellipsizes, footer condenses, single-column grid).

**2f. HiDPI check.** Confirm thumbnails are requested at device scale (respect `Widget::scale_factor`) so they aren't blurry at 200%; confirm the bundled SVG icon and CSS radii look right at fractional scaling. No fixed-pixel assumption should break.

**Acceptance (WS2):** manual resize/screenshots at 360×640, 768×1024, 1366×768, 1920×1080, 2560×1440, 3840×2160 all look intentional (documented expected column count per size in the PR). No horizontal scrollbar at any width; no absurd stretch on ultrawide; cards keep 16:9. `cargo build --features gui` clean.

---

### WS3 — More meaningful, easier-to-use UI

Touches `src/gui/status.rs` (new, from WS0), `window.rs` (wire-in), `theme.rs` (minor).

**3a. "Now playing" status surface.** In `src/gui/status.rs`, build a compact status pill from `ipc::request(&Request::Status)` (`ipc.rs`), placed in the header (`window.rs:171-179`) or just above the grid:
- green/amber/off dot (`.dot-ok/.dot-warn/.dot-off` already in `theme.rs:207-209`) + wallpaper name + hwdec badge (e.g. "VA-API") + CPU%.
- On `StatusReply.error`, show the error succinctly with a tooltip.
- Poll lightly (e.g. every few seconds while the window is focused; stop when unfocused) using the existing thread→`glib::spawn_future_local` idiom. Handle "daemon not running" (IPC returns Err) as the **off** state — don't error.

**3b. Pause/Resume toggle (done right).** Add a single header toggle bound to `StatusReply.paused`, issuing `Request::Pause`/`Request::Resume`. (The codebase deliberately dropped a confusing "Stop"; a *pause/resume* toggle paired with the live status pill is unambiguous — see the rationale comment at `window.rs:167-170`. Keep "Stop" out.)

**3c. Apply confirmation.** When a wallpaper is applied (`apply_entry_by_idx`, `window.rs:1051-1088`), show an "Applied ✓" toast (toast system already present, `AppState.toast`).

**3d. Menu clarity.** In `build_menu_popover` (`window.rs:340+`), add **Check for updates** (WS1d), an **About** entry showing the version, and group appearance vs behavior with labels. Add tooltips to header/footer buttons that lack them.

**3e. Broken-entry affordance.** Cards already flag `MISSING` (`window.rs:751-761`). Add a card-menu action to **Relink** (re-pick the source file) or **Remove** so a broken entry isn't a dead end.

**3f. Keyboard shortcuts.** Wire `Ctrl+F` → focus search, `Ctrl+,` → open menu/settings, `Ctrl+Q` → quit, using GTK accelerators. (Low risk, high polish.)

**Acceptance (WS3):** with the daemon running, the pill reflects real playing/paused/hwdec/CPU and updates on change; with the daemon down it shows "off" cleanly; pause/resume works; applying shows a toast; About shows the correct version. `cargo build --features gui` clean.

---

### WS4 — Landing page one-step install (smaller, independent)

Only touches `landing/` (Next.js) — fully parallelizable, no Rust build impact.

**4a. Primary CTA = direct install, not the releases browser.** In `landing/src/components/download.tsx` and `landing/src/components/hero.tsx`, make the primary button either a **direct latest `.deb` link** (`https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/<asset>.deb`) or promote the **`INSTALL_ONELINER`** (`landing/src/lib/site.ts`) to the hero as the copy-run primary action. Keep the releases page only as a small "all releases" secondary link.
- If the `.deb` asset filename is versioned (not stable), prefer the one-liner (`install.sh` already resolves "latest" via the API) or add a stable `latest` asset name in CI so a direct link is possible. Confirm the actual asset name from a recent release before hardcoding.

**Acceptance (WS4):** landing build succeeds (`cd landing && npm run build`); primary download action installs without the user browsing GitHub; copy button works; no dead links.

---

## 5. Orchestration for Sonnet 5 (subagents + Workflow)

### 5.1 Dependency graph
```
WS0 (refactor, SEQUENTIAL, must land first)
        │
        ├── WS1 (updates)      ── edits src/gui/updates.rs, update.rs, ipc.rs, config.rs, scripts/  ─┐
        ├── WS2 (viewport)     ── edits src/gui/window.rs (library builder), theme.rs               ─┤ WS2+WS3 share the
        ├── WS3 (status/UX)    ── edits src/gui/status.rs, window.rs (header), theme.rs             ─┘ library builder → same agent OR serialize
        └── WS4 (landing)      ── edits landing/**  (fully independent, can run anytime)
```
- **WS4** has zero overlap with the Rust code → run it in parallel from the start (even before WS0).
- **WS1** mostly lands in *new* files (`update.rs`, `gui/updates.rs`) + `ipc.rs`/`config.rs`/`scripts/` → low contention with WS2/WS3 once WS0 has moved the update helpers out of `window.rs`.
- **WS2 and WS3 both edit the library-view builder and `theme.rs`.** Give them to **one subagent sequentially**, or land WS2 then WS3, to avoid conflicting edits in `window.rs`. Do **not** run WS2 and WS3 as parallel worktree agents on the same file.

### 5.2 Suggested execution
1. **Phase A (parallel):** one subagent does **WS0** (Rust refactor); one subagent does **WS4** (landing). WS4 can merge independently.
2. **Gate:** WS0 must build clean (`--features gui` and `--features daemon`) and be a verified no-op refactor before Phase B.
3. **Phase B (parallel, non-overlapping files):** subagent-1 → **WS1**; subagent-2 → **WS2+WS3** (sequential within the agent, since they share `window.rs`/`theme.rs`).
4. **Phase C (Workflow — review + verify):** run a review Workflow (e.g. `/code-review high` or an adversarial fan-out) over the combined diff, focused on: FFI/thread-safety in the GUI update thread, `pkexec` invocation correctness and exit-code handling, no UI-thread blocking, breakpoint math, and IPC roundtrip. Then a manual verify pass using the matrices in §6.

Because Phase B agents mutate files in parallel, run them with `isolation: "worktree"` **only if** they truly touch disjoint files (WS1's new files vs WS2/WS3's `window.rs`); otherwise serialize the ones that share `window.rs`.

### 5.3 Guardrails for every subagent
- Read the anchor files/lines in the task before editing.
- Match the existing thread→`async_channel`→`glib::spawn_future_local` pattern for anything async in the GUI (see `window.rs:2119-2139`); never block the GTK main thread on network or `pkexec`.
- GTK4 CSS is a subset (no `transform`/`var()`/`calc()` — see `theme.rs:12-15`); keep new CSS within it.
- Rebuild with the correct feature flags after each task; report any pre-existing warnings/failures rather than absorbing them.
- Keep `panic = "abort"` FFI-safety in mind across the GTK/mpv boundary.

---

## 6. Verification

### 6.1 Update flow (WS1)
- [ ] Low-version `.deb` installed → in-app banner appears without any Supabase push.
- [ ] **Update now** → single polkit prompt → installs → **Restart** offered → relaunch runs new version. No browser.
- [ ] **Check for updates** on an up-to-date install → "latest version" toast.
- [ ] Offline → silent, no dialog, no hang.
- [ ] Daemon notification path still installs (regression check).
- [ ] Flatpak/non-apt → fallback, no broken `apt install`.
- [ ] `src/ipc.rs` unit tests pass with `Request::Update` added.

### 6.2 Responsive (WS2) — screenshot at each, record expected columns
`360×640` · `768×1024` · `1366×768` · `1920×1080` · `2560×1440` · `3840×2160`. No horizontal scrollbar; 16:9 cards; centered/clamped on ultrawide; compact layout usable at min size; crisp at 200% scale.

### 6.3 UX (WS3)
- [ ] Status pill reflects real `StatusReply`; updates on change; "off" when daemon down.
- [ ] Pause/Resume toggles playback and matches `paused`.
- [ ] Applied toast fires; About shows correct version; broken-entry Relink/Remove works.

### 6.4 Landing (WS4)
- [ ] `npm run build` clean; primary CTA installs without browsing GitHub; copy button works.

### 6.5 Build gates
`cargo build --features gui` · `cargo build --features daemon` · `cargo test` · `cd landing && npm run build` — all green.

---

## 7. Risks & open questions
- **polkit availability:** `pkexec` must exist and a polkit agent must be running (true on the target Debian/Ubuntu/Pop!_OS/Mint desktops). If absent, fall back to the one-liner in a copyable dialog. (Confirm behavior on minimal/tiling setups.)
- **No package signing:** the `.deb` isn't signed; the updater trusts GitHub over TLS. Hardening (publish + verify `SHA256SUMS`) is a follow-up, out of scope here — note it in the PR.
- **GitHub API rate limit:** 60/hr/IP unauthenticated; the 24h throttle keeps us far under it. Handle 403/rate-limit as "no update info," never an error dialog.
- **Direct `.deb` link (WS4):** depends on a stable asset name in releases. Confirm the actual asset filename from a recent release; if versioned, prefer the one-liner or add a stable-named asset in CI.
- **`window.rs` contention:** the single biggest execution risk. WS0's module split is what makes parallelism safe — do not skip it, and do not parallelize two agents over `window.rs`.

---

## 8. File index (quick reference for the executor)
| Concern | File(s) |
|---|---|
| Updater script | `scripts/fresco-update.sh` |
| Daemon update notify | `src/daemon/notifier.rs` |
| Shared update logic (new) | `src/update.rs` |
| GUI update banner/dialog (new) | `src/gui/updates.rs` |
| GUI status surface (new) | `src/gui/status.rs` |
| GUI main window / library | `src/gui/window.rs` |
| Theme / CSS | `src/gui/theme.rs` |
| IPC contract | `src/ipc.rs` |
| Persistent config | `src/config.rs` |
| Feature flags / deb assets | `Cargo.toml` |
| Landing download CTA | `landing/src/components/download.tsx`, `landing/src/components/hero.tsx`, `landing/src/lib/site.ts` |

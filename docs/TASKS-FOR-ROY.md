# Manual/external tasks — unblocked as the overnight run progresses

## From T1.7 (audio — code fix landed and machine-proven)
- **T1.7.4 Listen test**: on your PipeWire machine (+ a PulseAudio VM if handy):
  set an unmuted video wallpaper → hear sound; toggle mute from the status pill;
  reboot → sound plays on first login WITHOUT re-applying (this was the cold-boot
  bug: mpv dropped the audio track when frescod started before PipeWire; the
  daemon now restores it automatically within ~5–160s backoff window, typically 5s).
  Record results as A1–A3 rows in docs/WAYLAND_VERIFICATION.md.
- Note: the machine harness (tests/audio/verify-audio.sh) passes 3/3 legs on both
  sway and x11, including the late-audio cold-boot repro with the fix gated on
  (FRESCO_EXPECT_LATE_AUDIO=1). Run it yourself:
  `FRESCO_EXPECT_LATE_AUDIO=1 tests/ci/with-compositor.sh sway -- tests/audio/verify-audio.sh`

## From T1.8 (4K/8K fidelity — fixes landed and machine-proven)
- **T1.8.6 Real-display check**: on your 4K (and any HiDPI-scaled) monitor,
  compare a real 4K/8K wallpaper before/after this build. Expected: sharper
  8K→4K downscales (correct-downscaling + mitchell), no gradient banding
  (dither-depth=auto now on). Machine numbers: downscale SSIM 0.54 → 0.74,
  gradient levels 220 → 256, 1px-checkerboard crispness 100/100 at scale 1 & 2.
- **Known-limited**: fractional scale (e.g. 1.25) measures 76.5 crispness —
  compositor-side resampling of the mpvpaper buffer; permanent fix is the
  native backend (ROADMAP 5.1, wp_fractional_scale). Integer scales are
  pixel-perfect.
- Harness: `WITH_COMPOSITOR_NO_BG=1 tests/ci/with-compositor.sh sway -- tests/fidelity/verify-fidelity.sh`
  (x11: `WITH_COMPOSITOR_X11_GEOM=3840x2160x24 tests/ci/with-compositor.sh x11 -- ...`)

## From T1.3 (X11 fullscreen auto-pause — landed, detection machine-proven)
- **Real-session check**: on an X11 session (any EWMH WM), fullscreen a video
  (mpv/YouTube F11) → wallpaper on that monitor pauses within ~2–3s (log:
  "fullscreen window detected; pausing wallpaper"); un-fullscreen → resumes.
  On dual-head, only the covered monitor pauses. Battery + manual pause
  behavior unchanged. Detection itself is proven by an integration test that
  plays the WM on scratch Xvfb (FRESCO_EWMH_TEST=1); what needs eyes is a real
  WM driving the EWMH state.

## T1.2.1 blocker (Hyprland/KDE CI legs) — needs binaries this box lacks
- Hyprland and kwin_wayland are not installed here, so the hyprland/kde
  with-compositor legs cannot be exercised locally (the sway and x11 legs are
  proven — the fidelity + audio harnesses run green on both).
- To unblock: `sudo apt install hyprland kwin-wayland` (or test on the CI
  runners), then: pin post-aquamarine headless env vars in
  tests/ci/with-compositor.sh (AQ_DRM_DEVICES= etc.), enable the grim T1
  screenshot check on those legs, and run 10 consecutive
  `tests/ci/with-compositor.sh hyprland -- tests/ci/env-smoke.sh hyprland wayland-layer-shell`.
- Real-session Hyprland/KDE verification (T1.2.2) remains the gate for
  un-scoping the README claims (now marked "experimental" — T1.4.2 done).

## From T2.2 (per-monitor GUI — shipped; one polish deferred)
- Right-click a wallpaper card with 2+ displays connected → "Set on <connector> (WxH)"
  per display + "Show default on all displays". Config writes only
  [monitors."<connector>"]; proven per-connector routing on 2 headless outputs.
- Deferred polish: the mini to-scale display-layout preview (strip) — the menu
  covers the ≤4-click AC; the visual layout map is a follow-up.

## From T3.1.1 (catalog — code complete; go-live is yours)
1. Apply the new schema section to Supabase (dashboard → SQL editor → run the
   `catalog_items` block at the end of supabase/schema.sql).
2. Create the media host: a `fresco-wallpapers` GitHub repo whose Releases hold
   the files (zero-egress), or a Cloudflare R2 public bucket.
3. Curate ≥50 CC0/verified loops (~8 categories); check loop cleanliness
   (first/last frame diff); fill rows via the admin app's new Catalog page
   (license + author are mandatory).
4. In-app: menu → "Browse wallpapers…". Server-side install counts appear in
   the admin Catalog table (Installs column) — zero client telemetry.
- Machine-proofs already green: client parse/cache/fetch (3/3), gallery
  fixture render + offline behavior (4/4), download safety (5/5).

## From T1.5.1 (AUR — files authored; publishing is yours)
- packaging/aur/{fresco,fresco-bin}/ hold PKGBUILD + .SRCINFO (bash -n clean;
  source build mirrors CI: cargo build --release --all-features --locked;
  -bin repacks the release .deb, keeping the bundled mpvpaper).
- Before publishing: replace both sha256sums=('SKIP') with real hashes,
  run `makepkg --printsrcinfo > .SRCINFO` and `namcap` in a clean Arch
  chroot, then push to AUR (ssh://aur@aur.archlinux.org/fresco[-bin].git).
- Add both to the release checklist: bump pkgver + hashes each release.
- Note: on Arch the in-app updater exits "unsupported" (apt-based) — expected;
  users update via the AUR helper.

## From T2.1.1 (Flatpak readiness — manifest retargeted; submission is yours)
- Manifest now targets v0.0.91 + GNOME 48 runtime; YAML validates. Still needed
  before submission (needs flatpak-builder + network):
  1. `git rev-parse v0.0.91^{commit}` → replace the commit placeholder.
  2. `python3 flatpak/flatpak-cargo-generator.py Cargo.lock -o flatpak/cargo-sources.json`
     (script not yet vendored — grab from flatpak/flatpak-builder-tools).
  3. Local build per the manifest header, then in-sandbox smoke:
     `frescod --check` (bundled libmpv + capability), X11 + Sway wallpaper,
     update UI absent (already gated on /.flatpak-info — verified in code).
  4. `flatpak run --command=flatpak-builder-lint org.flatpak.Builder …` → zero errors.
  5. Reviewer likely asks about --filesystem=host:ro and xdg-config/autostart —
     defenses are pre-written in docs/FLATHUB.md; a Background-portal autostart
     variant is a nice-to-have follow-up, not a blocker.
- Store listing MUST set the GNOME Wayland static-frame expectation (Flathub's
  core audience is GNOME) — copy in docs/FLATHUB.md.

## From T1.6.1 (screenshots — real captures wired; polish at launch)
- data/screenshots/{library,gallery}.png are REAL app captures (headless, demo
  library + fixture catalog) — now in the landing hero/features and README;
  zero picsum placeholders remain. metainfo screenshot URL now resolves once
  pushed (data/screenshots/library.png exists).
- At launch, reshoot with beautiful curated content (the fixtures are test
  patterns): rerun the capture script after the catalog is live, or shoot on
  your real desktop. Demo video (≤60s) + baseline metrics + launch post remain
  yours (T1.6.2).

# Publishing Fresco on Flathub

This is the end-to-end guide for building, testing, submitting, and maintaining
Fresco on Flathub, following the official
[submission](https://docs.flathub.org/docs/for-app-authors/submission) and
[maintenance](https://docs.flathub.org/docs/for-app-authors/maintenance) docs.

Files involved:
- `flatpak/io.github.dibbayajyotiroy.Fresco.yaml` — the Flatpak manifest
- `data/io.github.dibbayajyotiroy.Fresco.metainfo.xml` — AppStream metadata (required)
- `data/io.github.dibbayajyotiroy.Fresco.desktop` — desktop entry
- `data/icons/io.github.dibbayajyotiroy.Fresco.svg` — app icon
- `flatpak/cargo-sources.json` — **generated**; offline Rust dependency sources

> ⚠️ **Status:** the manifest is a complete first draft with real source hashes
> for libass/mpv/ffmpegthumbnailer, but it has **not yet been built** with
> `flatpak-builder`. The dependency chain (libplacebo → mpv in particular) and
> the sandbox behaviour of the detached daemon **must be verified locally**
> before opening the submission PR. Expect one or two iterations on the mpv
> module. See "Known iteration points" at the bottom.

---

## Architecture notes (why this app is non-trivial to sandbox)

Fresco is not a single-process GUI; the manifest's `finish-args` exist for these reasons:

| Behaviour | Sandbox handling |
|---|---|
| Creates a desktop-level **X11** window and restacks it | `--socket=x11 --share=ipc` (X11-only app) |
| Renders the wallpaper via **wlr-layer-shell** on Wayland (bundled mpvpaper) | **No** `--socket=wayland`; instead `--filesystem=xdg-run/wayland-0` + `wayland-1` and a `WAYLAND_DISPLAY`-restoring wrapper — see "Wayland security context & layer-shell" below |
| GPU decode/render via libmpv (`vo=gpu`) | `--device=dri` + bundled libmpv (built in the manifest) |
| Daemon opens the user's media files long after the picker closes | `--filesystem=host:ro` (document portal is per-file/per-process, insufficient) |
| Writes the login **autostart** entry | `--filesystem=xdg-config/autostart:create`; the app writes `Exec=flatpak run --command=frescod …` (handled in `src/autostart.rs`) |
| Sets the GNOME desktop background (overview fallback) | dconf access (`--talk-name=ca.desrt.dconf` + dconf dirs) |
| GUI spawns `frescod` which outlives the GUI window | both binaries live in `/app/bin`; the sandbox instance stays alive while `frescod` runs |

The `--filesystem=host:ro` permission will likely draw a reviewer question —
be ready to explain that a wallpaper daemon must re-open arbitrary local media
paths on every login, which the document portal cannot provide.

---

## Wayland security context & layer-shell (reviewer note)

**Problem.** Flatpak ≥ 1.16 no longer hands the app the host's Wayland socket.
With `--socket=wayland` it creates a per-app proxy socket via the
`wp_security_context_manager_v1` protocol, so the compositor knows the client
is sandboxed. Compositors that implement security-context filtering then hide
*privileged* globals from such clients — and `zwlr_layer_shell_v1` (the only
way to render a wallpaper on wlroots-style compositors) is privileged.
cosmic-comp is explicit about this: only clients **without** a security
context (or with the internal `com.system76.CosmicPanel` context) receive
privileged globals. The mechanism is not COSMIC-specific — wlroots ships
`security-context-v1` since 0.17 and sway uses it to withhold privileged
protocols from sandboxed clients — so this will spread, not shrink.

**Observed on COSMIC (flatpak 1.16.6, verified 2026-07):**

- Sandboxed registry probe sees no `zwlr_layer_shell_v1`; `frescod --check`
  falls back to `wayland-gnome-static` (static frames only).
- Sandboxed `mpvpaper` dies with SIGSEGV (exit 139) right after opening the
  video; the identical host binary works perfectly on the same session.
- `--filesystem=xdg-run/wayland-1` *alone* does **not** help: flatpak mounts
  its security-context proxy socket over the same path (different inode).
- Flatpak source has **no opt-out** (no env var, no permission check): the
  proxy is skipped only when the compositor lacks the protocol.

**Workaround (what the manifest does).** Drop `--socket=wayland` entirely —
then flatpak creates no proxy — and expose the real host socket with
`--filesystem=xdg-run/wayland-0` + `--filesystem=xdg-run/wayland-1` (the name
is machine-dependent; 0 and 1 cover practically all sessions, and missing
paths are silently skipped). One wrinkle: flatpak scrubs `WAYLAND_DISPLAY`
whenever the wayland socket permission is absent, so
`flatpak/fresco-wrapper.sh` (installed as both `/app/bin/fresco` and
`/app/bin/frescod`, real binaries renamed `*-real`) re-derives it by globbing
`$XDG_RUNTIME_DIR/wayland-*` before exec. Verified end-to-end on COSMIC with
run-time flags equivalent to these finish-args:

```bash
# capability flips to layer-shell:
flatpak run --nosocket=wayland --filesystem=xdg-run/wayland-1 \
    --command=sh io.github.dibbayajyotiroy.Fresco \
    -c 'export WAYLAND_DISPLAY=wayland-1; exec frescod --check'
#   → Session: wayland (wayland-layer-shell)

# mpvpaper renders instead of segfaulting (124 = killed by timeout, i.e. ran fine):
timeout 8 flatpak run --nosocket=wayland --filesystem=xdg-run/wayland-1 \
    --command=sh io.github.dibbayajyotiroy.Fresco \
    -c 'export WAYLAND_DISPLAY=wayland-1; exec mpvpaper eDP-1 video.mp4'; echo $?
#   → 124 (previously 139/SIGSEGV via the security-context proxy)
```

**What to tell reviewers.** There is no wallpaper or layer-shell portal, so a
sandboxed wallpaper app fundamentally cannot do its job through the
security-context socket on filtering compositors. The `--filesystem` grant is
the minimal escape hatch: it only re-establishes what every Flatpak had
before 1.16 (a direct compositor connection), and the app already carries
`--filesystem=host:ro` + `--talk-name=org.freedesktop.Flatpak`, so this adds
no meaningful sandbox weakening beyond the existing permission set. Prior
art: Flathub's only comparable video-wallpaper app (Hidamari) avoids the
problem by being X11-only and talking to `org.freedesktop.Flatpak` to run
things on the host — strictly broader access than this socket grant.

**Fallback if reviewers reject it.** Revert to plain `--socket=wayland` and
document that on security-context-filtering compositors (COSMIC today) the
Flatpak degrades to static frames — `frescod --check` handles this sanely
(reports `wayland-gnome-static`, exit 0, no crash loop) — and recommend the
.deb for COSMIC users. That is shippable but quietly loses the app's
headline feature on exactly the sessions mpvpaper targets, which is why the
workaround is the preferred submission.

Side effects to keep in mind: without a security context the compositor can
no longer associate the connection with the app ID (minor: window↔app
matching cosmetics), and portals are unaffected.

---

## 1. Prerequisites

```bash
flatpak remote-add --if-not-exists --user flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install -y flathub org.flatpak.Builder
flatpak install -y flathub \
    org.gnome.Platform//47 org.gnome.Sdk//47 \
    org.freedesktop.Sdk.Extension.rust-stable//24.08 \
    org.freedesktop.Platform.ffmpeg-full//24.08
```

## 2. Generate offline cargo sources

Flathub builds with **no network**, so every crate must be listed as a source.
Use `flatpak-cargo-generator.py` from
[flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools):

```bash
curl -fsSLO https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
pip install --user aiohttp toml   # script deps
python3 flatpak-cargo-generator.py Cargo.lock -o flatpak/cargo-sources.json
```

Re-run this whenever `Cargo.lock` changes (it reads the checksums already
recorded in the lockfile). Commit the resulting `cargo-sources.json`.

## 3. Build, run, and lint locally (the Flathub way)

The docs require building with `org.flatpak.Builder` (not bare `flatpak-builder`)
so you reproduce Flathub's CI environment:

```bash
# For local testing of the working tree, temporarily switch the `fresco` module
# source in the manifest from the git block to:   - type: dir
#                                                    path: ..
flatpak run --command=flathub-build org.flatpak.Builder --install \
    flatpak/io.github.dibbayajyotiroy.Fresco.yaml

flatpak run io.github.dibbayajyotiroy.Fresco           # launch the GUI
flatpak run --command=frescod io.github.dibbayajyotiroy.Fresco --check   # diagnostics
```

Run the linters (must pass for submission):

```bash
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest \
    flatpak/io.github.dibbayajyotiroy.Fresco.yaml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
```

Validate the AppStream metadata:

```bash
flatpak run --command=appstreamcli org.flatpak.Builder validate \
    data/io.github.dibbayajyotiroy.Fresco.metainfo.xml
```

## 4. Add a screenshot (required)

Flathub requires at least one screenshot, and the metainfo references
`data/screenshots/library.png` on the `main` branch. Add a real PNG there and
commit it (screenshots don't render from test builds — only after the first
official build).

## 5. Submit to Flathub

Per the docs, submissions are PRs to `flathub/flathub` against the **`new-pr`**
branch (NOT `master`):

```bash
gh repo fork --clone flathub/flathub && cd flathub && git checkout --track origin/new-pr
git checkout -b fresco-submission new-pr
mkdir io.github.dibbayajyotiroy.Fresco
# copy the manifest in (Flathub expects the manifest at the repo root of the new app dir),
# plus cargo-sources.json and any local module files it references
git add . && git commit -m "Add io.github.dibbayajyotiroy.Fresco"
git push -u origin fresco-submission
# Open a PR titled "Add io.github.dibbayajyotiroy.Fresco" against base branch new-pr
```

Before opening the PR, set the `commit:` field in the manifest's `fresco` git
source to the exact SHA the `v0.0.x` tag points to:

```bash
git rev-list -n 1 v0.0.2     # paste this into the manifest's commit: field
```

In the PR, comment `bot, build` to trigger a test build; install and test the
resulting bundle before it's merged.

## 6. Verification (get the blue check)

Because the app ID is `io.github.dibbayajyotiroy.*`, verification is automatic
once published: log in to [flathub.org](https://flathub.org) with the
**DibbayajyotiRoy** GitHub account and click *Verify* on the app page.

## 7. Maintenance & updates

- After the app repo is created under `flathub/`, you get write access — updates
  are **PRs to that repo**, never the submission process again.
- With the `x-checker-data` in the manifest, the Flathub bot opens an update PR
  automatically whenever you push a new `vX.Y.Z` tag here. **Test the PR build,
  then merge** — avoid auto-merge (the docs strongly discourage it).
- Each new `<release>` you add to `metainfo.xml` shows up as the in-app/store
  changelog, so keep it in sync with `CHANGELOG.md` on every release.
- Flathub publishes per-app **download statistics** at flathub.org/stats — this
  becomes your install-count source alongside the GitHub Releases badge.

---

## Known iteration points (verify on first local build)

1. **mpv module** — `libplacebo` tag vs mpv 0.38 compatibility, and mpv's meson
   options against the ffmpeg-full extension, are the most likely to need
   adjustment. Flathub requires git sources to also pin `commit:`; add commits
   for the `libplacebo` git source before submitting.
2. **Build size/time** — mpv + libplacebo + libass + ffmpegthumbnailer + a large
   Rust dependency tree may exceed Flathub's standard CI limits and need a
   request for external runners (open an issue if the build times out).
3. **Detached daemon lifetime** — confirm that closing the GUI keeps `frescod`
   (and the wallpaper) alive inside the sandbox, and that autostart relaunches
   it via `flatpak run --command=frescod`.
4. **gsettings background** — confirm the overview fallback actually writes the
   host's `org.gnome.desktop.background` through the granted dconf access.
5. **`--filesystem=host:ro`** — be ready to justify it to reviewers, or switch to
   a narrower set (`home:ro` + removable media) if they prefer.
6. **Wayland socket workaround** — after any rebuild, re-verify on a
   security-context compositor (COSMIC): `frescod --check` must report
   `wayland-layer-shell`, and confirm hosts whose socket is neither
   `wayland-0` nor `wayland-1` (rare) gracefully fall back to X11/static.

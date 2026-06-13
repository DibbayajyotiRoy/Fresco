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
| GPU decode/render via libmpv (`vo=gpu`) | `--device=dri` + bundled libmpv (built in the manifest) |
| Daemon opens the user's media files long after the picker closes | `--filesystem=host:ro` (document portal is per-file/per-process, insufficient) |
| Writes the login **autostart** entry | `--filesystem=xdg-config/autostart:create`; the app writes `Exec=flatpak run --command=frescod …` (handled in `src/autostart.rs`) |
| Sets the GNOME desktop background (overview fallback) | dconf access (`--talk-name=ca.desrt.dconf` + dconf dirs) |
| GUI spawns `frescod` which outlives the GUI window | both binaries live in `/app/bin`; the sandbox instance stays alive while `frescod` runs |

The `--filesystem=host:ro` permission will likely draw a reviewer question —
be ready to explain that a wallpaper daemon must re-open arbitrary local media
paths on every login, which the document portal cannot provide.

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

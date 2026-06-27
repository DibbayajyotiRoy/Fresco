# CI, environment gate & automatic releases

Fresco's CI proves the app actually **runs** on the desktop environments users
have — not just that it compiles — and turns a merge to `main` into a published
release, gated on that proof.

## The pieces

| Workflow | Trigger | What it does |
|---|---|---|
| `ci.yml` | pull request → `main`, manual | Runs the strict checks **and** the environment gate on every PR. |
| `publish.yml` | push/merge → `main` | Runs the strict checks + environment gate, then **publishes a new release if the version was bumped**. |
| `release.yml` | push a `v*` tag | Manual/explicit release of a specific tag. |
| `distros.yml` | weekly + manual | Cross-*distro* build/install matrix (unchanged). |
| `_ci.yml` | reusable | Strict fmt / clippy / build / test / doc. |
| `_environments.yml` | reusable + manual | Headless desktop-environment matrix + the "≥3 must pass" gate. |
| `_release.yml` | reusable | Build the `.deb`, create the GitHub release, notify clients. |

The `_`-prefixed workflows are reusable building blocks (`workflow_call`); they
never run on their own, so the real pipelines stay DRY and identical between the
manual and automatic release paths.

## 1. Stricter build & test (`_ci.yml`)

Beyond the old "fmt + clippy + test", every run now:

- builds with `--locked` (a stale `Cargo.lock` fails instead of being rewritten);
- runs **clippy with `-D warnings` and `--all-targets`** across **all three
  feature combinations** that ship — `--all-features`, `--no-default-features
  --features daemon`, and `--features gui` — so a mistake that only shows up in
  the GUI-only or daemon-only build is caught;
- builds both binaries;
- runs the unit tests; and
- builds the docs with `RUSTDOCFLAGS=-D warnings` (broken intra-doc links fail).

## 2. The environment gate (`_environments.yml`)

This is the core of "does Fresco work where people run it?". Each target
environment is started **headlessly** on a runner and Fresco is exercised inside
it by the harness in [`tests/ci/`](../tests/ci):

| env id | environment | started with | expected backend |
|---|---|---|---|
| `x11` | X11 | `Xvfb` | `x11` |
| `sway` | Sway (wlroots) | headless wlroots backend | `wayland-layer-shell` |
| `hyprland` | Hyprland (wlroots) | headless wlroots backend | `wayland-layer-shell` |
| `kde` | KDE Plasma | `kwin_wayland --virtual` | `wayland-layer-shell` |
| `weston` | Weston | headless backend (no layer-shell) | `wayland-gnome-static` |

For each environment the harness asserts the things that must hold for Fresco to
be *usable* there, independent of GPU pixel output (which is unreliable under
software rendering on CI):

**Required (these gate the result):**
1. `frescod --check` detects the **expected backend capability** for the env.
2. **libmpv loads** at runtime in that environment.
3. `frescod` launches with a real video config and **stays alive** through
   startup — no crash, no Rust panic.

**Best-effort (reported, never fail the gate — flaky on software rendering):**
the renderer/IPC backend actually came up, idle CPU, the self-heal restart, and
a visible-and-animated screenshot.

### "At least 3 must pass"

The matrix is `fail-fast: false` and each leg records `PASS`/`FAIL` as an
artifact instead of failing its job. The `gate` job then requires **≥ 3**
environments to pass (configurable via the `min_pass` input). If fewer than 3
pass, the gate fails and **no release is published**.

The robust trio — **X11, Sway, Weston** — reliably carries the gate; **KDE** and
**Hyprland** add coverage when their packages and headless modes cooperate on the
runner. (X11 and Sway are validated to pass; the rest reuse the exact same
harness.)

Run it by hand anytime: **Actions → `_environments` → Run workflow** (you can set
a different `min_pass`).

## 3. Automatic publishing on merge (`publish.yml`)

On every push/merge to `main`:

1. **checks** — `_ci.yml` (must pass)
2. **environments** — `_environments.yml`, ≥3 must pass (must pass)
3. **version** — reads `version` from `Cargo.toml`; is there already a `v<version>` tag?
4. **release** — runs **only when (1) and (2) passed and the version is new**:
   builds the `.deb`, creates the `v<version>` tag + GitHub release, and notifies
   running clients over Supabase.

### How to ship a release

Bump `version` in `Cargo.toml` (and update `CHANGELOG.md`) in the change you
merge. When it lands on `main` and the gate passes, the release publishes
automatically.

**Why version-gated and not "every commit"?** Publishing a brand-new GitHub
release — and pushing an "update available" notification to every running
client — on every routine commit would be spam. Tying the release to a version
bump makes "ship a release" an explicit, reviewable act (the version + changelog
edit) while still being fully automatic on merge. Commits that don't bump the
version still run the **full** strict + environment gate; they just don't
publish.

> The release tag is created with the workflow's `GITHUB_TOKEN`. GitHub does not
> start new workflow runs for events triggered by `GITHUB_TOKEN`, so the
> auto-created tag does **not** re-trigger `release.yml` — no double release.

## Making the gate block merges

To enforce the gate before merge, add the PR checks as **required status checks**
in the branch-protection rules for `main` (Settings → Branches): the jobs from
`CI / checks` and `CI / environments` (the `gate` job in particular). The check
names changed when CI moved to reusable workflows, so update any existing
required-check list.

## Local use

The harness runs locally for any environment you have installed:

```sh
cargo build --no-default-features --features daemon --bin frescod
# X11:
tests/ci/with-compositor.sh x11  -- tests/ci/env-smoke.sh x11  x11
# Sway (headless):
tests/ci/with-compositor.sh sway -- tests/ci/env-smoke.sh sway wayland-layer-shell
```

`env-smoke.sh` generates its own test video with `ffmpeg` (override with
`TEST_VIDEO=/path/to.mp4`). It is **fully isolated from your live session** and
safe to run on your own desktop: `with-compositor.sh` always mints a private
`XDG_RUNTIME_DIR` (so it never trips a running daemon's single-instance lock or
touches the real compositor), and `env-smoke.sh` redirects `HOME` and the XDG
base dirs and drops `DBUS_SESSION_BUS_ADDRESS` — so frescod's GNOME static-frame
path can't change your real desktop background via gsettings/dconf. All temp dirs
are removed on exit.

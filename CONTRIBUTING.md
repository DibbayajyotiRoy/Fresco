# Contributing to Fresco

Thanks for your interest in improving Fresco! Contributions of all kinds are welcome — bug reports, code, docs, and wallpaper catalog suggestions.

## Quick start

```bash
git clone https://github.com/DibbayajyotiRoy/fresco
cd fresco
cargo build --all-features --locked
cargo run --features gui            # launch the GUI
```

**Requirements**

- Rust (stable, edition 2021) via [rustup](https://rustup.rs)
- GTK4 + libadwaita development headers
- `mpv` / `libmpv` and `mpvpaper` (for live playback; see `scripts/build-mpvpaper.sh`)
- A Linux desktop — X11 or a wlr-layer-shell Wayland compositor (COSMIC, Hyprland, Sway, KDE Plasma 6)

On Debian/Ubuntu:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libmpv-dev mpv
```

## Project layout

| Path | What it is |
|---|---|
| `src/` | Rust crate — CLI, config, IPC, scheduling, downloads |
| `src/gui/` | GTK4/libadwaita application |
| `src/daemon/` | Background wallpaper daemon |
| `extension/` | Browser new-tab extension |
| `landing/` | Website |
| `flatpak/`, `packaging/` | Distribution packaging |
| `docs/` | Architecture notes, roadmap, install docs |
| `tests/` | Integration, Wayland, and fidelity tests |

## Before you open a PR

CI enforces all of these — run them locally first:

```bash
cargo fmt --all --check
cargo clippy --all-features --all-targets --locked -- -D warnings
cargo test --no-default-features --features daemon --locked
```

Feature-gated builds must also stay clean:

```bash
cargo clippy --no-default-features --features daemon --all-targets --locked -- -D warnings
cargo clippy --no-default-features --features gui --all-targets --locked -- -D warnings
```

## Pull request guidelines

1. **Open an issue first** for anything non-trivial, so we can agree on the approach before you invest time.
2. **Keep PRs focused** — one fix or feature per PR.
3. **Match existing style** — idiomatic Rust, no new dependencies without discussion.
4. **Test on your desktop environment** and say which one(s) you tested in the PR description (X11 / COSMIC / Hyprland / Sway / KDE Wayland).
5. **Update docs** (`README.md`, `docs/`, `CHANGELOG.md`) if behavior changes.
6. Write clear commit messages: a short imperative summary line, details in the body if needed.

## Reporting bugs

Use the [bug report template](https://github.com/DibbayajyotiRoy/fresco/issues/new/choose). The single most useful thing you can include is your **desktop environment + display server** (e.g. "COSMIC, Wayland") and the output of:

```bash
fresco --version
echo $XDG_CURRENT_DESKTOP $XDG_SESSION_TYPE
```

## Feature requests

Open an issue describing the **problem** you're trying to solve, not just the solution — it helps us find the best fit for the project. Check [docs/ROADMAP.md](docs/ROADMAP.md) first; it may already be planned.

## Code of conduct

Be respectful and constructive. Harassment or personal attacks are not tolerated.

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (see [LICENSE](LICENSE)).

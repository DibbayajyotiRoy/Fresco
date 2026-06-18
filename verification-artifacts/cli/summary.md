# Fresco CLI Verification — runtime evidence

Binary under test: `./target/debug/fresco` (commit working-tree, uncommitted).
Installed binary present: `/usr/bin/fresco`.

| Phase | Command | Exit | Observed (runtime) | Result |
|---|---|---|---|---|
| 4 | `fresco doctor` | 0 | Prints the doctor report (command-specific) | **PASS** |
| 4 | `fresco status` | 0 | "Fresco isn't running…" (command-specific) | **PASS** |
| 4 | `fresco logs` | 0 | Dumps daemon log lines (command-specific) | **PASS** |
| 3 | `fresco --help` | 0 | **GApplication help only** (`--help-gapplication`); no doctor/status/logs | **FAIL** |
| 6 | `fresco invalid-command` | 1 | **GTK launched** → `GLib-GIO-CRITICAL: This application can not open files.` No CLI error | **FAIL** |
| 7 | installed `/usr/bin/fresco doctor` | — | **GLib-GIO-CRITICAL: cannot open files** — installed binary has NO `doctor` subcommand | **MISMATCH** |

## Findings (adversarial)

1. **`--help` is not handled by `cli::dispatch`.** `dispatch()` matches only
   `doctor`/`status`/`logs`; `--help` returns `None` and falls through to
   `adw::Application`/GApplication, which prints generic GTK help. Phase 3 fail
   criteria ("Only GTK/GApplication help appears") is met.

2. **Invalid commands fall through to GTK.** `fresco invalid-command` returns
   `None` from dispatch → GApplication runs and emits `GLib-GIO-CRITICAL: This
   application can not open files` (exit 1). No clean "unknown command" message.
   Phase 6 fail criteria ("GTK launches") is met.

3. **INSTALLED_BINARY_MISMATCH (severe).** `which fresco` → `/usr/bin/fresco`,
   a stale build **without** the CLI. For any real user on the installed binary,
   `fresco doctor` does NOT run doctor — it errors with a GLib critical. The new
   CLI exists only in `target/debug/fresco` and is **uncommitted and uninstalled**.

4. (Observation, not a CLI defect) `fresco doctor` reported `Session: Wayland`
   in this run vs `X11` in the earlier environment-collection run — the env seen
   by the process varies between invocations (WAYLAND_DISPLAY/XDG_SESSION_TYPE).
   Does not affect dispatch reachability.

## Verdict

The three subcommands **execute correctly on the built binary**, but the CLI as a
whole fails the mandate: `--help` does not expose commands, invalid input launches
GTK instead of erroring, and the **installed binary does not contain the CLI at
all**. Trusting only runtime behavior:

# CLI STATUS: FAILED

#!/usr/bin/env bash
# Start a headless display server / Wayland compositor for the given environment,
# export the right display variables, run the command after `--` inside it, then
# tear everything down and propagate the command's exit code.
#
# This encapsulates the finicky headless-compositor incantations so CI (and you,
# locally, for the ones you have installed) can exercise Fresco on each target
# environment with a single line:
#
#   tests/ci/with-compositor.sh sway -- tests/ci/env-smoke.sh sway wayland-layer-shell
#
# Environments:
#   x11       Xvfb                          → DISPLAY
#   sway      Sway, headless wlroots        → WAYLAND_DISPLAY (layer-shell)
#   hyprland  Hyprland, headless wlroots    → WAYLAND_DISPLAY (layer-shell)
#   kde       KWin, virtual backend         → WAYLAND_DISPLAY (layer-shell)
#   cosmic    cosmic-comp, nested in Xvfb   → WAYLAND_DISPLAY (layer-shell)
#   weston    Weston, headless backend      → WAYLAND_DISPLAY (NO layer-shell)
#
# Exit codes: the command's own exit code, or 70 if the compositor never came up.
set -u

ENV_ID="${1:?usage: with-compositor.sh <env> -- <command...>}"
shift
[ "${1:-}" = "--" ] && shift
[ "$#" -ge 1 ] || { echo "with-compositor.sh: no command after --" >&2; exit 64; }

log() { printf '[with-compositor:%s] %s\n' "$ENV_ID" "$*" >&2; }

# ── always use a PRIVATE XDG_RUNTIME_DIR (0700) ──────────────────────────────
# Never reuse the caller's: a private dir isolates us from a running daemon's
# single-instance lock and from the real compositor's sockets, and it's removed
# on exit. Xvfb doesn't need it; the nested Wayland compositors create their
# socket inside it.
PRIV_XDG="$(mktemp -d "${TMPDIR:-/tmp}/fresco-xdg.XXXXXX")"
chmod 700 "$PRIV_XDG"
export XDG_RUNTIME_DIR="$PRIV_XDG"

COMP_PID=""
XVFB_PID=""
# shellcheck disable=SC2317  # invoked indirectly via trap
cleanup() {
  [ -n "$COMP_PID" ] && kill "$COMP_PID" 2>/dev/null
  [ -n "$XVFB_PID" ] && kill "$XVFB_PID" 2>/dev/null
  # Backends Fresco may have spawned (Wayland renderer).
  pkill -f 'mpvpaper' 2>/dev/null || true
  wait 2>/dev/null
  [ -n "${PRIV_XDG:-}" ] && rm -rf "$PRIV_XDG"
}
trap cleanup EXIT INT TERM

# Wait up to ~15s for a predicate to hold.
wait_for() { # <description> <shell-test...>
  local desc="$1"; shift
  local _
  for _ in $(seq 1 75); do
    if "$@"; then return 0; fi
    sleep 0.2
  done
  log "timed out waiting for: $desc"
  return 1
}

# Print existing wayland-* display sockets (one per line). Lets us detect the NEW
# socket our compositor creates (the runner may already have a session socket).
wl_sockets() {
  local s
  for s in "$XDG_RUNTIME_DIR"/wayland-*; do
    [ -S "$s" ] && printf '%s\n' "$s"
  done
}

start_wayland_common() {
  # Force software rendering so this works on headless CI with no GPU.
  export WLR_RENDERER=pixman
  export WLR_BACKENDS=headless
  export WLR_HEADLESS_OUTPUTS=1
  export WLR_LIBINPUT_NO_DEVICES=1
  export LIBGL_ALWAYS_SOFTWARE=1
  export XDG_SESSION_TYPE=wayland
  unset DISPLAY
}

# Start a backgrounded Wayland compositor (passed as "$@") and adopt the first
# NEW wayland-* socket it creates as WAYLAND_DISPLAY.
adopt_new_wayland_socket() {
  local before newsock=""
  before="$(wl_sockets)"
  "$@" &
  COMP_PID=$!
  local _
  for _ in $(seq 1 75); do
    newsock="$(comm -13 <(printf '%s\n' "$before" | sort) <(wl_sockets | sort) | head -1)"
    [ -n "$newsock" ] && break
    kill -0 "$COMP_PID" 2>/dev/null || { log "$ENV_ID compositor exited before creating a socket"; return 1; }
    sleep 0.2
  done
  [ -n "$newsock" ] || { log "no new wayland socket appeared for $ENV_ID"; return 1; }
  WAYLAND_DISPLAY="$(basename "$newsock")"
  export WAYLAND_DISPLAY
  log "WAYLAND_DISPLAY=$WAYLAND_DISPLAY (pid $COMP_PID)"
}

case "$ENV_ID" in
  x11)
    export DISPLAY=:99
    export XDG_SESSION_TYPE=x11
    unset WAYLAND_DISPLAY
    # WITH_COMPOSITOR_X11_GEOM lets pixel tests pick e.g. 3840x2160x24.
    Xvfb :99 -screen 0 "${WITH_COMPOSITOR_X11_GEOM:-1920x1080x24}" -nolisten tcp >/dev/null 2>&1 &
    XVFB_PID=$!
    wait_for "Xvfb on :99" bash -c 'xdpyinfo -display :99 >/dev/null 2>&1 || xset -display :99 q >/dev/null 2>&1' \
      || { log "Xvfb never came up"; exit 70; }
    ;;

  sway)
    start_wayland_common
    cfg="$(mktemp)"
    # WITH_COMPOSITOR_NO_BG=1 skips the solid background: swaybg sits ON TOP of
    # mpvpaper's background layer, so pixel tests (fidelity) must not start it.
    if [ "${WITH_COMPOSITOR_NO_BG:-0}" = 1 ]; then
      printf 'default_border none\n' > "$cfg"
    else
      printf 'output * bg #1a1a2e solid_color\ndefault_border none\n' > "$cfg"
    fi
    adopt_new_wayland_socket sway --unsupported-gpu -c "$cfg" || { log "sway never came up"; exit 70; }
    ;;

  hyprland)
    start_wayland_common
    cfg="$(mktemp)"; printf 'misc { disable_hyprland_logo = true }\nanimations { enabled = false }\n' > "$cfg"
    # Hyprland honours the wlroots/aquamarine headless env vars set above.
    adopt_new_wayland_socket Hyprland --config "$cfg" || { log "Hyprland never came up"; exit 70; }
    ;;

  kde)
    export XDG_SESSION_TYPE=wayland
    export LIBGL_ALWAYS_SOFTWARE=1
    unset DISPLAY
    # (Don't set QT_QPA_PLATFORM here — that's for Qt *clients*; kwin_wayland is
    # the compositor and must not try to act as a nested Wayland client.)
    # KWin needs a session bus; --virtual selects its headless output backend.
    if command -v dbus-run-session >/dev/null 2>&1; then
      adopt_new_wayland_socket dbus-run-session -- kwin_wayland --virtual --width 1920 --height 1080 \
        || { log "kwin_wayland never came up"; exit 70; }
    else
      adopt_new_wayland_socket kwin_wayland --virtual --width 1920 --height 1080 \
        || { log "kwin_wayland never came up"; exit 70; }
    fi
    ;;

  cosmic)
    # COSMIC's real compositor (smithay-based). It has no headless backend, so
    # we nest it inside Xvfb via its X11 backend — the same compositor code
    # path COSMIC users run, exercising cosmic-comp's actual layer-shell
    # implementation (which differs from wlroots': this is the environment
    # where mpvpaper/libmpv bugs bit real users).
    export DISPLAY=:98
    Xvfb :98 -screen 0 1920x1080x24 -nolisten tcp >/dev/null 2>&1 &
    XVFB_PID=$!
    wait_for "Xvfb on :98" bash -c 'xset -display :98 q >/dev/null 2>&1' \
      || { log "Xvfb (for cosmic-comp) never came up"; exit 70; }
    export XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=COSMIC
    export LIBGL_ALWAYS_SOFTWARE=1
    adopt_new_wayland_socket cosmic-comp || { log "cosmic-comp never came up"; exit 70; }
    unset DISPLAY # clients must pick the Wayland path, not the outer Xvfb
    ;;

  weston)
    export XDG_SESSION_TYPE=wayland
    export LIBGL_ALWAYS_SOFTWARE=1
    unset DISPLAY
    # Deterministic socket name; weston's headless backend has no layer-shell, so
    # this is the Wayland fallback case (Fresco should pick wayland-gnome-static).
    WAYLAND_DISPLAY=wayland-fresco
    if command -v dbus-run-session >/dev/null 2>&1; then
      dbus-run-session -- weston --backend=headless-backend.so --socket="$WAYLAND_DISPLAY" \
        --width=1920 --height=1080 --idle-time=0 >/dev/null 2>&1 &
    else
      weston --backend=headless-backend.so --socket="$WAYLAND_DISPLAY" \
        --width=1920 --height=1080 --idle-time=0 >/dev/null 2>&1 &
    fi
    COMP_PID=$!
    export WAYLAND_DISPLAY
    wait_for "weston socket $WAYLAND_DISPLAY" test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" \
      || { log "weston never came up"; exit 70; }
    log "WAYLAND_DISPLAY=$WAYLAND_DISPLAY (pid $COMP_PID)"
    ;;

  *)
    echo "with-compositor.sh: unknown env '$ENV_ID'" >&2
    exit 64
    ;;
esac

log "running: $*"
"$@"
rc=$?
log "command exited $rc"
exit "$rc"

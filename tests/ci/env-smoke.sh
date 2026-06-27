#!/usr/bin/env bash
# Fresco per-environment runtime smoke test.
#
# Assumes a display is ALREADY live for this environment (DISPLAY for x11,
# WAYLAND_DISPLAY for wayland) — tests/ci/with-compositor.sh sets that up and
# runs this inside it.
#
# It asserts the things that must hold for Fresco to be USABLE in this
# environment, independent of GPU pixel output (which is unreliable on headless
# software rendering):
#
#   REQUIRED (gate the result):
#     1. frescod --check detects the EXPECTED backend capability for this env
#     2. libmpv loads at runtime here
#     3. frescod launches with a real video config and stays alive — no crash,
#        no Rust panic — through startup
#
#   BEST-EFFORT (reported, do NOT gate — flaky under software rendering):
#     - a renderer/IPC backend actually came up (X11 renderer / Wayland socket)
#     - idle CPU stays low
#     - self-heal: killing the backend makes the daemon restart it
#     - the wallpaper is visibly non-black and animated (screenshot)
#
# Usage: tests/ci/env-smoke.sh <env-id> <expected-capability>
#   expected-capability ∈ x11 | wayland-layer-shell | wayland-gnome-static
# Exit:  0 = PASS (all REQUIRED checks), 1 = FAIL
set -u

ENV_ID="${1:?usage: env-smoke.sh <env-id> <expected-cap>}"
EXPECT="${2:?usage: env-smoke.sh <env-id> <expected-cap>}"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FRESCOD="${FRESCOD:-}"
if [ -z "$FRESCOD" ]; then
  for c in "$ROOT/target/release/frescod" "$ROOT/target/debug/frescod"; do
    [ -x "$c" ] && FRESCOD="$c" && break
  done
fi
if [ -z "$FRESCOD" ] || [ ! -x "$FRESCOD" ]; then
  echo "FATAL: frescod binary not found (build it or set FRESCOD)"; exit 1
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/fresco-smoke.XXXXXX")"
trap 'kill "${DPID:-}" 2>/dev/null; pkill -f mpvpaper 2>/dev/null; rm -rf "$WORK"' EXIT INT TERM

# FULL isolation — keep the test off the REAL desktop. Isolating only config/state
# is NOT enough: frescod's GNOME path calls gsettings/dconf via the session bus,
# which would change the user's actual desktop background, and it writes overview
# frames under ~/.cache. Redirect HOME + the XDG base dirs and cut the session bus
# so nothing here can touch the live session.
export HOME="$WORK/home"
export XDG_CONFIG_HOME="$WORK/config"
export XDG_STATE_HOME="$WORK/state"
export XDG_CACHE_HOME="$WORK/cache"
export XDG_DATA_HOME="$WORK/data"
unset DBUS_SESSION_BUS_ADDRESS
mkdir -p "$HOME" "$XDG_CONFIG_HOME/fresco" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME"
DLOG="$XDG_STATE_HOME/fresco/frescod.log"   # where init_logging() writes

strip_ansi() { sed 's/\x1b\[[0-9;]*m//g'; }

req_pass=0; req_fail=0; bonus=()
pass()  { printf '  \033[32m[PASS]\033[0m %s\n' "$*"; req_pass=$((req_pass+1)); }
fail()  { printf '  \033[31m[FAIL]\033[0m %s\n' "$*"; req_fail=$((req_fail+1)); }
note()  { printf '  \033[36m[ ?? ]\033[0m %s\n' "$*"; bonus+=("$*"); }

echo "=============================================================="
echo " Fresco environment smoke: $ENV_ID (expect capability: $EXPECT)"
echo "   frescod : $FRESCOD"
echo "   display : DISPLAY=${DISPLAY:-} WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-}"
echo "=============================================================="

# ── generate a tiny looping test video (no asset to commit) ──────────────────
VIDEO="${TEST_VIDEO:-$WORK/test.mp4}"
if [ ! -f "$VIDEO" ]; then
  if command -v ffmpeg >/dev/null 2>&1; then
    ffmpeg -hide_banner -loglevel error -f lavfi -i testsrc=size=640x360:rate=15 \
      -t 3 -pix_fmt yuv420p "$VIDEO" </dev/null \
      || { echo "FATAL: could not generate test video"; exit 1; }
  else
    echo "FATAL: ffmpeg not available and no TEST_VIDEO provided"; exit 1
  fi
fi

cat > "$XDG_CONFIG_HOME/fresco/config.toml" <<EOF
enabled = true
autostart = false
[wallpaper]
kind = "video"
path = "$VIDEO"
mute = true
EOF

# ── REQUIRED 1 + 2: capability detection and libmpv, via frescod --check ─────
CHECK="$("$FRESCOD" --check 2>&1 | strip_ansi)"
echo "--- frescod --check ---"; echo "$CHECK" | sed 's/^/    /'; echo "-----------------------"

if grep -q "($EXPECT)" <<<"$CHECK"; then
  pass "capability detected as '$EXPECT'"
else
  got="$(grep -oE '\((x11|wayland-layer-shell|wayland-gnome-static)\)' <<<"$CHECK" | head -1 | tr -d '()')"
  fail "capability mismatch: expected '$EXPECT', got '${got:-unknown}'"
fi

if grep -q 'libmpv .*NOT LOADED' <<<"$CHECK"; then
  fail "libmpv did NOT load at runtime in this environment"
else
  pass "libmpv loaded at runtime"
fi

# ── REQUIRED 3: the daemon launches and survives startup without crashing ────
RUST_BACKTRACE=1 "$FRESCOD" >"$WORK/frescod.stdio" 2>&1 &
DPID=$!
alive() { kill -0 "$DPID" 2>/dev/null; }

# Give it time to detect the session, build renderers / spawn the backend.
survived=1
for _ in $(seq 1 30); do
  alive || { survived=0; break; }
  sleep 0.2
done

panicked() {
  grep -qiE 'panicked|RUST_BACKTRACE|thread .* panicked' "$WORK/frescod.stdio" "$DLOG" 2>/dev/null
}

if [ "$survived" = 1 ] && alive && ! panicked; then
  pass "daemon launched and stayed alive through startup"
elif panicked; then
  fail "daemon panicked during startup"
  grep -iE 'panicked|error' "$WORK/frescod.stdio" "$DLOG" 2>/dev/null | sed 's/^/      /' | head -20
else
  fail "daemon exited during startup (not alive)"
  sed 's/^/      /' "$WORK/frescod.stdio" 2>/dev/null | head -20
  sed 's/^/      /' "$DLOG" 2>/dev/null | head -20
fi

# ── BEST-EFFORT depth checks (never fail the gate) ───────────────────────────
if alive; then
  # backend present?
  case "$EXPECT" in
    wayland-layer-shell)
      if ls "${XDG_RUNTIME_DIR:-/nonexistent}"/fresco/mpv-*.sock >/dev/null 2>&1; then
        note "Wayland renderer IPC socket present (mpvpaper + mpv up)"
      else
        note "no mpvpaper IPC socket yet (renderer may have failed under software GL)"
      fi
      ;;
    x11)
      if grep -qE 'frescod started with [1-9]' "$DLOG" 2>/dev/null; then
        note "X11 renderer window created"
      else
        note "no X11 renderer reported (mpv GL context may be unavailable under Xvfb)"
      fi
      ;;
    wayland-gnome-static)
      if grep -qi 'static-frame' "$DLOG" 2>/dev/null; then
        note "static-frame fallback mode started (expected: no layer-shell here)"
      else
        note "static-frame mode not confirmed in log"
      fi
      ;;
  esac

  # idle CPU over ~3s
  read -r u1 s1 < <(awk '{print $14, $15}' "/proc/$DPID/stat" 2>/dev/null || echo "0 0")
  sleep 3
  read -r u2 s2 < <(awk '{print $14, $15}' "/proc/$DPID/stat" 2>/dev/null || echo "0 0")
  hz=$(getconf CLK_TCK 2>/dev/null || echo 100)
  cpu=$(awk -v a="$u1" -v b="$s1" -v c="$u2" -v d="$s2" -v hz="$hz" \
    'BEGIN{printf "%.1f", ((c+d)-(a+b))/hz/3*100}')
  note "idle CPU ≈ ${cpu}%"

  # self-heal: kill the Wayland backend, expect a restart
  if [ "$EXPECT" = "wayland-layer-shell" ] && pgrep -f mpvpaper >/dev/null 2>&1; then
    pkill -f mpvpaper 2>/dev/null
    healed=0
    for _ in $(seq 1 40); do
      sleep 0.25
      if grep -qiE 'restart|respawn' "$DLOG" 2>/dev/null; then healed=1; break; fi
    done
    if [ "$healed" = 1 ]; then
      note "self-heal: daemon restarted the killed renderer"
    else
      note "self-heal: no restart observed in 10s"
    fi
  fi
fi

# ── teardown ─────────────────────────────────────────────────────────────────
kill "$DPID" 2>/dev/null
pkill -f mpvpaper 2>/dev/null || true
wait 2>/dev/null

echo "--------------------------------------------------------------"
echo " required: ${req_pass} passed, ${req_fail} failed; ${#bonus[@]} best-effort notes"
if [ "$req_fail" -eq 0 ] && [ "$req_pass" -ge 3 ]; then
  echo " RESULT: PASS"
  echo "--------------------------------------------------------------"
  exit 0
else
  echo " RESULT: FAIL"
  echo "--------------------------------------------------------------"
  exit 1
fi

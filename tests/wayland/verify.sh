#!/usr/bin/env bash
# Fresco Wayland runtime verification harness.
#
# RUN ON A REAL (or headless) WAYLAND COMPOSITOR — Hyprland, Sway, or KDE Plasma 6.
# It cannot produce valid results on X11/GNOME. Headless Sway works for CI:
#   WLR_BACKENDS=headless WLR_RENDERER=pixman sway &  # then run this inside it
#
# Automates: T1 (visible + animated), T6 (kill→restart), T18 (idle CPU).
# Documents (see docs/WAYLAND_VERIFICATION.md) the rest as procedures.
#
# Usage: tests/wayland/verify.sh /path/to/test-video.mp4
set -u

VIDEO="${1:?usage: verify.sh <video-file>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
COMP="${XDG_CURRENT_DESKTOP:-unknown}"
ART="$ROOT/tests/wayland/artifacts/${COMP}-${TS}"
mkdir -p "$ART"

FRESCOD="${FRESCOD:-$ROOT/target/release/frescod}"
[ -x "$FRESCOD" ] || FRESCOD="$ROOT/target/debug/frescod"

pass=0; fail=0; skip=0
declare -a RESULTS
record() { # name result detail
  RESULTS+=("{\"test\":\"$1\",\"result\":\"$2\",\"detail\":\"$3\"}")
  printf '  [%s] %-22s %s\n' "$2" "$1" "$3"
  case "$2" in PASS) pass=$((pass+1));; FAIL) fail=$((fail+1));; *) skip=$((skip+1));; esac
}

have() { command -v "$1" >/dev/null 2>&1; }

# ── evidence header ──────────────────────────────────────────────────────────
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
MPVPAPER_BIN="${FRESCO_MPVPAPER:-mpvpaper}"
MPVPAPER_VER="$("$MPVPAPER_BIN" --version 2>/dev/null | head -1 || echo 'not found')"
echo "Fresco Wayland verification"
echo "  timestamp   : $TS"
echo "  commit      : $COMMIT"
echo "  compositor  : $COMP"
echo "  mpvpaper    : $MPVPAPER_VER"
echo "  artifacts   : $ART"
echo

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "FATAL: WAYLAND_DISPLAY is unset — not a Wayland session. Aborting." >&2
  exit 2
fi

# ── isolated config pointing at the test video ───────────────────────────────
export XDG_CONFIG_HOME="$ART/config"
export XDG_STATE_HOME="$ART/state"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$ART/run}"
mkdir -p "$XDG_CONFIG_HOME/fresco" "$XDG_STATE_HOME" "$ART/run"
cat > "$XDG_CONFIG_HOME/fresco/config.toml" <<EOF
enabled = true
[wallpaper]
kind = "video"
path = "$VIDEO"
mute = true
EOF

# ── launch the daemon ────────────────────────────────────────────────────────
RUST_LOG=info "$FRESCOD" >"$ART/frescod.log" 2>&1 &
DPID=$!
cleanup() { kill "$DPID" 2>/dev/null; }
trap cleanup EXIT

SOCK="$XDG_RUNTIME_DIR/fresco/mpvpaper.sock"
for _ in $(seq 1 50); do [ -S "$SOCK" ] && break; sleep 0.2; done
if [ ! -S "$SOCK" ]; then
  record "setup" "FAIL" "mpvpaper IPC socket never appeared (see frescod.log)"
fi

# ── T1: wallpaper visible + animated ─────────────────────────────────────────
if have grim; then
  sleep 1; grim "$ART/frame1.png" 2>/dev/null
  sleep 1; grim "$ART/frame2.png" 2>/dev/null
  if [ -s "$ART/frame1.png" ] && have magick; then
    sd=$(magick "$ART/frame1.png" -format '%[fx:standard_deviation]' info: 2>/dev/null)
    diff=$(magick compare -metric RMSE "$ART/frame1.png" "$ART/frame2.png" null: 2>&1 | awk '{print $1}')
    notblack=$(awk -v v="${sd:-0}" 'BEGIN{print (v>0.02)?1:0}')
    animated=$(awk -v v="${diff:-0}" 'BEGIN{print (v>50)?1:0}')
    if [ "$notblack" = 1 ] && [ "$animated" = 1 ]; then
      record "T1_visible_animated" "PASS" "stddev=$sd rmse=$diff"
    else
      record "T1_visible_animated" "FAIL" "stddev=$sd rmse=$diff (black or static)"
    fi
  else
    record "T1_visible_animated" "SKIP" "captured frames; install ImageMagick to auto-judge"
  fi
else
  record "T1_visible_animated" "SKIP" "grim not installed"
fi

# ── T18: idle CPU of the daemon ──────────────────────────────────────────────
read -r u1 s1 < <(awk '{print $14, $15}' "/proc/$DPID/stat" 2>/dev/null || echo "0 0")
sleep 3
read -r u2 s2 < <(awk '{print $14, $15}' "/proc/$DPID/stat" 2>/dev/null || echo "0 0")
hz=$(getconf CLK_TCK 2>/dev/null || echo 100)
cpu=$(awk -v a="$u1" -v b="$s1" -v c="$u2" -v d="$s2" -v hz="$hz" 'BEGIN{printf "%.2f", ((c+d)-(a+b))/hz/3*100}')
if awk -v c="${cpu:-100}" 'BEGIN{exit !(c<1.0)}'; then
  record "T18_idle_cpu" "PASS" "${cpu}% (<1%)"
else
  record "T18_idle_cpu" "FAIL" "${cpu}% (target <1%)"
fi

# ── T6: kill the backend, expect supervisor to restart it ────────────────────
pkill -f "input-ipc-server=$SOCK" 2>/dev/null
ok=0
for _ in $(seq 1 30); do
  sleep 0.3
  if grep -q "restarting" "$ART/frescod.log" && [ -S "$SOCK" ]; then ok=1; break; fi
done
[ "$ok" = 1 ] && record "T6_kill_restart" "PASS" "backend restarted, socket back" \
              || record "T6_kill_restart" "FAIL" "no restart within 9s"

# ── report ───────────────────────────────────────────────────────────────────
cleanup
{
  printf '{\n  "timestamp":"%s","commit":"%s","compositor":"%s","mpvpaper":"%s",\n' \
    "$TS" "$COMMIT" "$COMP" "$MPVPAPER_VER"
  printf '  "summary":{"pass":%d,"fail":%d,"skip":%d},\n  "tests":[\n    ' "$pass" "$fail" "$skip"
  (IFS=$'\n'; echo "${RESULTS[*]}" | paste -sd, -)
  printf '\n  ]\n}\n'
} > "$ART/report.json"

echo
echo "Summary: $pass pass, $fail fail, $skip skip  →  $ART/report.json"
[ "$fail" -eq 0 ]

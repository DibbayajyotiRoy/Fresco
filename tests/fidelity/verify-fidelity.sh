#!/usr/bin/env bash
# Visual-fidelity verification (ROADMAP 1.8 / TASKS T1.8.1).
#
# Plays generated test patterns through the REAL daemon on a headless output,
# captures the composited result, and scores it:
#
#   C1  4K 1px checkerboard on 4K@scale1  → crispness (1px alternation) + size
#   C2  4K 1px checkerboard on 4K@scale2  → buffer == physical px (HiDPI path)
#   C3  4K smooth gradient on 4K@scale1   → banding (unique luma levels)
#   C4  8K zone plate on 4K@scale1        → downscale quality (SSIM vs lanczos)
#
# Metrics are range-tolerant (no dependence on exact yuv↔rgb rounding):
#   crisp    RMSE between the capture and itself rolled 1px right — a perfect
#            1px checkerboard is anti-correlated (≈ max), any interpolation
#            blur collapses it.
#   levels   unique gray values along the gradient axis (center row band) —
#            banding/posterization shows as a low count; dithering raises it.
#   ssim     ffmpeg SSIM vs an ffmpeg-lanczos reference downscale.
#
# Run under a compositor:
#   tests/ci/with-compositor.sh sway -- tests/fidelity/verify-fidelity.sh
#   tests/ci/with-compositor.sh x11  -- tests/fidelity/verify-fidelity.sh   (C1/C3/C4)
#
# Modes:
#   FRESCO_FIDELITY_RECORD=1   record metrics only, never fail (baseline runs)
# Thresholds (override to recalibrate):
#   CRISP_MIN=90 LEVELS_MIN=120 SSIM_MIN=0.90
#
# Exit: 0 = all applicable cases pass (or RECORD mode), 1 = failure,
#       75 = environment missing tools.
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FRESCOD="${FRESCOD:-}"
if [ -z "$FRESCOD" ]; then
  for c in "$ROOT/target/release/frescod" "$ROOT/target/debug/frescod"; do
    [ -x "$c" ] && FRESCOD="$c" && break
  done
fi
[ -n "$FRESCOD" ] && [ -x "$FRESCOD" ] || { echo "FATAL: frescod not built"; exit 1; }

GEN="$ROOT/tests/assets/generated"
[ -f "$GEN/checkerboard-4k.mp4" ] || "$ROOT/tests/assets/make-fixtures.sh" >/dev/null || exit 1

for tool in ffmpeg identify convert compare python3; do
  command -v "$tool" >/dev/null || { echo "SKIP: $tool missing"; exit 75; }
done

RECORD="${FRESCO_FIDELITY_RECORD:-0}"
# Calibrated 2026-07-03 on headless sway (llvmpipe): pre-fix baseline scored
# crisp=100 / levels=220 / ssim=0.54; post-1.8.2 scores 100 / 256 / 0.74.
CRISP_MIN="${CRISP_MIN:-90}"      # % RMSE of 1px-rolled self-diff (perfect ≈ 100)
LEVELS_MIN="${LEVELS_MIN:-180}"   # unique luma levels across the gradient
SSIM_MIN="${SSIM_MIN:-0.70}"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/fresco-fid.XXXXXX")"
ART="$ROOT/verification-artifacts/fidelity-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$ART"
DPID=""
trap 'stop_daemon; rm -rf "$WORK"' EXIT INT TERM

export HOME="$WORK/home" XDG_CONFIG_HOME="$WORK/config" XDG_STATE_HOME="$WORK/state" \
       XDG_CACHE_HOME="$WORK/cache" XDG_DATA_HOME="$WORK/data"
unset DBUS_SESSION_BUS_ADDRESS
mkdir -p "$HOME" "$XDG_CONFIG_HOME/fresco" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME"

if [ -n "${WAYLAND_DISPLAY:-}" ]; then ENVKIND=wayland; OUTPUT="${FRESCO_OUTPUT:-HEADLESS-1}";
elif [ -n "${DISPLAY:-}" ]; then ENVKIND=x11; OUTPUT=root;
else echo "FATAL: no display"; exit 1; fi

# swaymsg needs the compositor's IPC socket; as a sibling of sway (not a child)
# we must discover it in the (private) runtime dir ourselves.
if [ "$ENVKIND" = wayland ] && [ -z "${SWAYSOCK:-}" ]; then
  for s in "$XDG_RUNTIME_DIR"/sway-ipc.*.sock; do
    [ -S "$s" ] && export SWAYSOCK="$s" && break
  done
fi

command -v grim >/dev/null || [ "$ENVKIND" = x11 ] || { echo "SKIP: grim missing"; exit 75; }
[ "$ENVKIND" = x11 ] && ! command -v import >/dev/null && { echo "SKIP: import missing"; exit 75; }

write_config() { # <video-path>
  cat > "$XDG_CONFIG_HOME/fresco/config.toml" <<EOF
enabled = true
autostart = false
[wallpaper]
kind = "video"
path = "$1"
mute = true
fit = "cover"
EOF
}
CTL="$XDG_RUNTIME_DIR/fresco/control.sock"
ipc() { # <json-line>
  python3 - "$CTL" "$1" <<'PY' 2>/dev/null
import socket, sys
s = socket.socket(socket.AF_UNIX); s.settimeout(4)
s.connect(sys.argv[1]); s.sendall((sys.argv[2]+"\n").encode())
print(s.makefile().readline().strip())
PY
}
start_daemon() {
  RUST_BACKTRACE=1 "$FRESCOD" >"$WORK/frescod.stdio" 2>&1 &
  DPID=$!
  for _ in $(seq 1 50); do [ -S "$CTL" ] && return 0; sleep 0.2; done
  echo "FATAL: daemon never came up"; tail -5 "$WORK/frescod.stdio"; return 1
}
stop_daemon() {
  if [ -n "$DPID" ]; then
    kill "$DPID" 2>/dev/null
    for _ in $(seq 1 15); do kill -0 "$DPID" 2>/dev/null || break; sleep 0.2; done
    kill -9 "$DPID" 2>/dev/null; wait "$DPID" 2>/dev/null
  fi
  DPID=""; pkill -f mpvpaper 2>/dev/null; return 0
}

set_output() { # <WxH> <scale>   (wayland only; x11 uses Xvfb's fixed screen)
  [ "$ENVKIND" = wayland ] || return 0
  local want="$1" scale="$2" got=""
  swaymsg output "$OUTPUT" mode --custom "${want}@30Hz" >/dev/null 2>&1 \
    || swaymsg output "$OUTPUT" mode --custom "$want" >/dev/null 2>&1 \
    || swaymsg output "$OUTPUT" resolution "$want" >/dev/null 2>&1
  swaymsg output "$OUTPUT" scale "$scale" >/dev/null 2>&1
  sleep 1
  got="$(swaymsg -t get_outputs 2>/dev/null | python3 -c "
import json,sys
for o in json.load(sys.stdin):
    if o.get('name')=='$OUTPUT':
        m=o.get('current_mode') or {}
        print(f\"{m.get('width')}x{m.get('height')} scale={o.get('scale')}\")" 2>/dev/null)"
  echo "  (output $OUTPUT now: ${got:-unknown}, wanted $want scale=$scale)"
}
capture() { # <out.png>
  if [ "$ENVKIND" = wayland ]; then grim -o "$OUTPUT" "$1" 2>/dev/null
  else import -window root -silent "$1" 2>/dev/null; fi
}
wait_playing() { # wait_playing [tries] — wait for PATTERNED content (solid bg has sd≈0)
  local tries="${1:-40}" i sd
  for i in $(seq 1 "$tries"); do
    sleep 0.5
    capture "$WORK/probe.png" || continue
    sd="$(identify -format "%[fx:int(standard_deviation*255)]" "$WORK/probe.png" 2>/dev/null || echo 0)"
    [ "${sd:-0}" -gt 15 ] && return 0
  done
  echo "  (no patterned frame appeared; last sd=${sd:-0})"
  return 1
}

# ── metrics ──────────────────────────────────────────────────────────────────
center_crop() { # <in> <out> <WxH>
  convert "$1" -gravity center -crop "$3+0+0" +repage -colorspace Gray "$2"
}
crisp_score() { # <png> → 0..100 (RMSE% of 1px-rolled self-diff)
  convert "$1" -roll +1+0 "$WORK/rolled.png"
  compare -metric RMSE "$1" "$WORK/rolled.png" null: 2>&1 | grep -oP '\(\K[0-9.]+' \
    | python3 -c "import sys; print(round(float(sys.stdin.read() or 0)*100,1))"
}
unique_levels() { # <png> → count of distinct gray values in a 3px center band
  convert "$1" -gravity center -crop x3+0+0 +repage -colorspace Gray -depth 8 txt:- \
    | grep -oP 'gray\(\K[0-9]+' | sort -un | wc -l
}
ssim_vs() { # <a.png> <b.png> → SSIM All value
  ffmpeg -hide_banner -i "$1" -i "$2" -lavfi "ssim" -f null - 2>&1 \
    | grep -oP 'All:\K[0-9.]+' | tail -1
}

pass=0; fail=0
check() { # <case> <metric-desc> <value> <cmp-python-expr with v>
  local case="$1" desc="$2" v="$3" expr="$4" ok
  ok="$(python3 -c "v=float('${v:-0}' or 0); print(1 if ($expr) else 0)")"
  echo "{\"case\":\"$case\",\"metric\":\"$desc\",\"value\":\"$v\",\"pass\":$ok}" >> "$ART/report.jsonl"
  if [ "$RECORD" = 1 ]; then echo "[RECORD] $case: $desc = $v"; return 0; fi
  if [ "$ok" = 1 ]; then echo "[PASS] $case: $desc = $v"; pass=$((pass+1));
  else echo "[FAIL] $case: $desc = $v (needed: $expr)"; fail=$((fail+1)); fi
}

echo "== fidelity verification ($ENVKIND) — record=$RECORD =="
echo "artifacts: $ART"

# ── C1: 4K checkerboard @ scale 1 ────────────────────────────────────────────
set_output 3840x2160 1
write_config "$GEN/checkerboard-4k.mp4"
start_daemon || exit 1
if wait_playing; then
  capture "$ART/c1-checker-4k-s1.png"
  dims="$(identify -format "%wx%h" "$ART/c1-checker-4k-s1.png")"
  center_crop "$ART/c1-checker-4k-s1.png" "$WORK/c1.png" 512x512
  crisp="$(crisp_score "$WORK/c1.png")"
  check C1 "capture dims (expect 3840x2160)" "$( [ "$dims" = 3840x2160 ] && echo 1 || echo 0 )" "v==1"
  check C1 "1px alternation crispness %" "$crisp" "v>=$CRISP_MIN"
else
  check C1 "renderer produced non-black frames" 0 "v==1"
fi

# ── C2: 4K checkerboard @ scale 2 (wayland HiDPI buffer path) ────────────────
if [ "$ENVKIND" = wayland ]; then
  set_output 3840x2160 2
  ipc '{"cmd":"apply"}' >/dev/null
  sleep 4
  if wait_playing; then
    capture "$ART/c2-checker-4k-s2.png"
    dims="$(identify -format "%wx%h" "$ART/c2-checker-4k-s2.png")"
    center_crop "$ART/c2-checker-4k-s2.png" "$WORK/c2.png" 512x512
    crisp2="$(crisp_score "$WORK/c2.png")"
    check C2 "physical capture dims (expect 3840x2160)" "$( [ "$dims" = 3840x2160 ] && echo 1 || echo 0 )" "v==1"
    check C2 "1px alternation at scale 2 %" "$crisp2" "v>=$CRISP_MIN"
  else
    check C2 "renderer produced non-black frames" 0 "v==1"
  fi
  # C2b: fractional scale — without wp_fractional_scale the compositor
  # resamples the buffer, so crispness collapse here is EXPECTED until the
  # native backend (ROADMAP 5.1). Recorded, never gated (informational).
  set_output 3840x2160 1.25
  ipc '{"cmd":"apply"}' >/dev/null
  sleep 4
  if wait_playing; then
    capture "$ART/c2b-checker-4k-s125.png"
    center_crop "$ART/c2b-checker-4k-s125.png" "$WORK/c2b.png" 512x512
    crisp2b="$(crisp_score "$WORK/c2b.png")"
    echo "[INFO] C2b: 1px alternation at scale 1.25 % = $crisp2b (not gated; fixed by native backend)"
    echo "{\"case\":\"C2b\",\"metric\":\"alternation at scale 1.25 (informational)\",\"value\":\"$crisp2b\",\"pass\":null}" >> "$ART/report.jsonl"
  fi
  set_output 3840x2160 1
fi

# ── C3: gradient banding @ scale 1 ───────────────────────────────────────────
write_config "$GEN/gradient-8bit.mp4"
ipc '{"cmd":"apply"}' >/dev/null
sleep 4
if wait_playing; then
  capture "$ART/c3-gradient-s1.png"
  levels="$(unique_levels "$ART/c3-gradient-s1.png")"
  check C3 "unique gradient luma levels" "$levels" "v>=$LEVELS_MIN"
else
  check C3 "renderer produced non-black frames" 0 "v==1"
fi

# ── C4: 8K zone plate downscaled to 4K ───────────────────────────────────────
write_config "$GEN/zoneplate-8k.mp4"
ipc '{"cmd":"apply"}' >/dev/null
sleep 5
if wait_playing 90; then  # 8K software decode can need ~45s for a first frame
  capture "$ART/c4-zoneplate-4k.png"
  # Reference: the same source frame lanczos-downscaled to the CAPTURED size,
  # so the comparison stays valid at any screen resolution (x11 leg may differ).
  dims="$(identify -format "%wx%h" "$ART/c4-zoneplate-4k.png")"
  ffmpeg -hide_banner -loglevel error -y -i "$GEN/zoneplate-8k.mp4" -frames:v 1 \
    -vf "scale=${dims%x*}:${dims#*x}:flags=lanczos" "$WORK/zp-ref.png"
  ssim="$(ssim_vs "$ART/c4-zoneplate-4k.png" "$WORK/zp-ref.png")"
  check C4 "downscale SSIM vs lanczos reference ($dims)" "${ssim:-0}" "v>=$SSIM_MIN"
else
  check C4 "renderer produced non-black frames" 0 "v==1"
fi

stop_daemon
cp "$WORK/frescod.stdio" "$ART/frescod.log" 2>/dev/null

echo "== result: $pass passed, $fail failed (record=$RECORD) — report: $ART/report.jsonl =="
[ "$RECORD" = 1 ] && exit 0
[ "$fail" -eq 0 ] || exit 1
exit 0

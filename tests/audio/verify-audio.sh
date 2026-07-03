#!/usr/bin/env bash
# Per-wallpaper audio verification (ROADMAP 1.7 / TASKS T1.7.1).
#
# Run inside a live display environment, e.g.:
#   tests/ci/with-compositor.sh sway -- tests/audio/verify-audio.sh
#   tests/ci/with-compositor.sh x11  -- tests/audio/verify-audio.sh
#
# Asserts the daemon's ground-truth audio state via the new StatusReply fields
# (audio_track / mute / volume), which both backends read live from mpv.
#
# Legs:
#   L1 apply-unmuted  config has mute=false,volume=70 → audio_track=true
#   L2 live-unmute    start muted (audio_track=false) → rewrite config
#                     unmuted + Apply → audio_track=true
#   L3 late-audio     audio server sockets appear only AFTER the daemon
#                     started (cold-boot repro). mpv permanently drops the
#                     track when no server is reachable at load time, so this
#                     leg FAILS until the T1.7.3 retry fix lands.
#
# The daemon-spawned mpv finds the audio server through $XDG_RUNTIME_DIR
# (pipewire-0 / pulse/native). with-compositor.sh gives us a PRIVATE runtime
# dir, so L1/L2 symlink the real session's sockets in ("the bridge") and L3
# creates the bridge late. If the real session has no audio server at all,
# L1/L2 exit 75 (environment cannot test audio).
#
# Exit: 0 = all applicable legs passed (L3 counted only with
#       FRESCO_EXPECT_LATE_AUDIO=1, i.e. after the retry fix); 1 = failure;
#       75 = environment can't test audio.
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FRESCOD="${FRESCOD:-}"
if [ -z "$FRESCOD" ]; then
  for c in "$ROOT/target/release/frescod" "$ROOT/target/debug/frescod"; do
    [ -x "$c" ] && FRESCOD="$c" && break
  done
fi
[ -n "$FRESCOD" ] && [ -x "$FRESCOD" ] || { echo "FATAL: frescod not built"; exit 1; }

SINE="$ROOT/tests/assets/generated/audio-sine-1080p.mp4"
[ -f "$SINE" ] || "$ROOT/tests/assets/make-fixtures.sh" >/dev/null || exit 1

REAL_RUNTIME="/run/user/$(id -u)"
have_real_audio() {
  [ -S "$REAL_RUNTIME/pipewire-0" ] || [ -S "$REAL_RUNTIME/pulse/native" ]
}

bridge_audio() { # link the real session's audio sockets into our private dir
  for f in pipewire-0 pipewire-0.lock pipewire-0-manager pipewire-0-manager.lock; do
    [ -e "$REAL_RUNTIME/$f" ] && ln -sf "$REAL_RUNTIME/$f" "$XDG_RUNTIME_DIR/$f"
  done
  [ -d "$REAL_RUNTIME/pulse" ] && ln -sfn "$REAL_RUNTIME/pulse" "$XDG_RUNTIME_DIR/pulse"
  return 0
}
unbridge_audio() {
  rm -f "$XDG_RUNTIME_DIR"/pipewire-0* "$XDG_RUNTIME_DIR/pulse" 2>/dev/null
  return 0
}

# ── status query helpers (control socket lives in the private runtime dir) ──
CTL="$XDG_RUNTIME_DIR/fresco/control.sock"
status_json() {
  python3 - "$CTL" <<'PY' 2>/dev/null
import json, socket, sys
s = socket.socket(socket.AF_UNIX)
s.settimeout(3)
try:
    s.connect(sys.argv[1])
    s.sendall(b'{"cmd":"status"}\n')
    print(s.makefile().readline().strip())
except Exception:
    pass
PY
}
field() { # field <name> — prints python-repr of the field, e.g. True/False/70/None
  status_json | python3 -c "import json,sys
try: print(json.load(sys.stdin).get('$1'))
except Exception: print('None')"
}
send_apply() {
  python3 - "$CTL" <<'PY' 2>/dev/null
import socket, sys
s = socket.socket(socket.AF_UNIX); s.settimeout(3)
s.connect(sys.argv[1]); s.sendall(b'{"cmd":"apply"}\n'); s.makefile().readline()
PY
}

wait_field() { # wait_field <name> <want> <seconds>
  local name="$1" want="$2" secs="$3" got=""
  local tries=$((secs * 2)) i
  for i in $(seq 1 "$tries"); do
    got="$(field "$name")"
    [ "$got" = "$want" ] && return 0
    sleep 0.5
  done
  echo "    (last: $name=$got, wanted $want)"
  return 1
}

write_config() { # write_config <mute:true|false>
  cat > "$XDG_CONFIG_HOME/fresco/config.toml" <<EOF
enabled = true
autostart = false
[wallpaper]
kind = "video"
path = "$SINE"
mute = $1
volume = 70
EOF
}

DPID=""
start_daemon() {
  RUST_BACKTRACE=1 "$FRESCOD" >"$WORK/frescod.stdio" 2>&1 &
  DPID=$!
  local i
  for i in $(seq 1 50); do
    [ -S "$CTL" ] && return 0
    kill -0 "$DPID" 2>/dev/null || break
    sleep 0.2
  done
  echo "FATAL: daemon/control socket never came up"; tail -5 "$WORK/frescod.stdio"
  return 1
}
stop_daemon() {
  if [ -n "$DPID" ]; then
    kill "$DPID" 2>/dev/null
    local i
    for i in $(seq 1 15); do
      kill -0 "$DPID" 2>/dev/null || break
      sleep 0.2
    done
    kill -9 "$DPID" 2>/dev/null   # never let teardown hang the harness
    wait "$DPID" 2>/dev/null
  fi
  DPID=""
  pkill -f mpvpaper 2>/dev/null
  return 0
}

pass=0; fail=0; skip=0
report() { # report <leg> <ok:0|1> [gate:hard|soft]
  local leg="$1" ok="$2" gate="${3:-hard}"
  if [ "$ok" = 0 ]; then
    echo "[PASS] $leg"; pass=$((pass+1))
  elif [ "$gate" = soft ]; then
    echo "[REPRO-CONFIRMED] $leg (known defect — fails until T1.7.3 fix; set FRESCO_EXPECT_LATE_AUDIO=1 to gate)"
    skip=$((skip+1))
  else
    echo "[FAIL] $leg"; fail=$((fail+1))
    tail -8 "$WORK/frescod.stdio" 2>/dev/null | sed 's/^/    | /'
  fi
}

# ── full isolation (same discipline as env-smoke.sh) ─────────────────────────
WORK="$(mktemp -d "${TMPDIR:-/tmp}/fresco-audio.XXXXXX")"
trap 'stop_daemon; rm -rf "$WORK"' EXIT INT TERM
export HOME="$WORK/home" XDG_CONFIG_HOME="$WORK/config" XDG_STATE_HOME="$WORK/state" \
       XDG_CACHE_HOME="$WORK/cache" XDG_DATA_HOME="$WORK/data"
unset DBUS_SESSION_BUS_ADDRESS
mkdir -p "$HOME" "$XDG_CONFIG_HOME/fresco" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME"

echo "== audio verification (display: DISPLAY=${DISPLAY:-} WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-}) =="

if ! have_real_audio; then
  echo "SKIP: no audio server in $REAL_RUNTIME — environment cannot test audio"
  exit 75
fi

# ── L1: apply-unmuted ────────────────────────────────────────────────────────
echo "-- L1 apply-unmuted"
bridge_audio
write_config false
start_daemon || exit 1
ok=1
if wait_field running True 15 && wait_field audio_track True 15 \
   && wait_field mute False 5 && wait_field volume 70 5; then ok=0; fi
report "L1 apply-unmuted: audio_track=true mute=false volume=70" "$ok"
stop_daemon

# ── L2: live-unmute (muted start → config rewrite → Apply) ──────────────────
echo "-- L2 live-unmute"
write_config true
start_daemon || exit 1
ok=1
if wait_field running True 15 && wait_field audio_track False 15; then
  write_config false
  send_apply
  if wait_field audio_track True 20 && wait_field mute False 5; then ok=0; fi
fi
report "L2 live-unmute via Apply restores the audio track" "$ok"
stop_daemon

# ── L3: late-audio (cold-boot repro — server appears after daemon start) ────
echo "-- L3 late-audio (cold-boot repro)"
unbridge_audio
write_config false
start_daemon || exit 1
ok=1
if wait_field running True 15; then
  sleep 2                      # let mpv finish (failing) audio init
  bridge_audio                 # "PipeWire comes up" a moment after login
  if wait_field audio_track True 35; then ok=0; fi
fi
gate=soft; [ "${FRESCO_EXPECT_LATE_AUDIO:-0}" = 1 ] && gate=hard
report "L3 late-audio: track recovers after audio server appears" "$ok" "$gate"
stop_daemon

echo "== result: $pass passed, $fail failed, $skip known-defect =="
[ "$fail" -eq 0 ] || exit 1
exit 0

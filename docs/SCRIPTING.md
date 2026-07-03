# Scripting Fresco

The daemon (`frescod`) listens on a Unix socket and speaks newline-delimited
JSON — one request, one reply. Everything the GUI does goes through this
socket, so your scripts can do it too: waybar toggles, workspace hooks,
cron-driven wallpaper changes.

**Socket:** `$XDG_RUNTIME_DIR/fresco/control.sock`
(fallback when `XDG_RUNTIME_DIR` is unset: `/tmp/fresco-<uid>/fresco/control.sock`)

**Requests:** `{"cmd":"status"}`, `{"cmd":"apply"}`, `{"cmd":"pause"}`,
`{"cmd":"resume"}`, `{"cmd":"stop"}`, `{"cmd":"update"}`

Every recipe below is copy-pasteable and uses only `socat` (or python3) and `jq`.

Define this once in your script:

```bash
FRESCO_SOCK="${XDG_RUNTIME_DIR:-/tmp/fresco-$(id -u)}/fresco/control.sock"
fresco_cmd() { printf '{"cmd":"%s"}\n' "$1" | socat - "UNIX-CONNECT:$FRESCO_SOCK"; }
```

## 1. Status as JSON

```bash
fresco_cmd status | jq
# e.g. just the essentials:
fresco_cmd status | jq '{wallpaper, paused, hwdec, cpu_percent, rss_mb, audio_track, monitors_info}'
```

Useful fields: `wallpaper` (name), `paused`, `hwdec` (`vaapi`/`nvdec`/`no`),
`cpu_percent` (daemon + renderers, % of one core), `rss_mb`, `audio_track` /
`mute` / `volume`, `source_w`/`source_h`/`bit_depth`/`dropped_frames`, and
`monitors_info` (every connected display with geometry — connector names are
exactly what `[monitors."<connector>"]` in config.toml expects).

## 2. Pause / resume (waybar toggle)

```bash
fresco_cmd pause    # freeze playback (state survives until resume)
fresco_cmd resume
```

Waybar `custom` module example:

```json
"custom/fresco": {
  "exec": "printf '{\"cmd\":\"status\"}\n' | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/fresco/control.sock | jq -r 'if .paused then \"\" else \"\" end'",
  "on-click": "printf '{\"cmd\":\"status\"}\n' | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/fresco/control.sock | jq -e .paused >/dev/null && printf '{\"cmd\":\"resume\"}\n' | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/fresco/control.sock || printf '{\"cmd\":\"pause\"}\n' | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/fresco/control.sock",
  "interval": 5
}
```

## 3. Set a wallpaper from a script

The config file is the source of truth; `apply` makes the daemon re-read it.
Change the file, then apply:

```bash
# set-wallpaper.sh <path-to-video>
CONF="${XDG_CONFIG_HOME:-$HOME/.config}/fresco/config.toml"
python3 - "$1" "$CONF" <<'PY'
import sys, re, pathlib
video, conf = sys.argv[1], pathlib.Path(sys.argv[2])
s = conf.read_text() if conf.exists() else 'enabled = true\n[wallpaper]\nkind = "video"\n'
s = re.sub(r'(?m)^path = .*$', f'path = "{video}"', s, count=1) \
    if re.search(r'(?m)^path = ', s) else s.replace('[wallpaper]', f'[wallpaper]\npath = "{video}"')
conf.write_text(s)
PY
fresco_cmd apply
```

(For anything beyond swapping the path, edit `config.toml` with a proper TOML
tool and then `fresco_cmd apply`.)

## 4. Different wallpaper on one display

Connector names come from `monitors_info` (recipe 1). Add a per-monitor
override table and apply:

```bash
CONF="${XDG_CONFIG_HOME:-$HOME/.config}/fresco/config.toml"
cat >> "$CONF" <<EOF

[monitors."HDMI-1"]
kind = "video"
path = "/home/you/Videos/side-display.mp4"
mute = true
EOF
fresco_cmd apply
```

Displays plugged in later are picked up on the next `apply` — no daemon
restart needed. Remove the `[monitors."…"]` table (or use the GUI's "Show
default on all displays") to go back.

## 5. Stop (and keep it stopped)

```bash
fresco_cmd stop
```

`stop` tears down the wallpaper and exits the daemon. To prevent it from
returning on next login, also set `enabled = false` in config.toml (the GUI's
Stop does both).

---

### python3 instead of socat

```bash
python3 - <<'PY'
import json, socket, os
sock = os.path.join(os.environ.get("XDG_RUNTIME_DIR", f"/tmp/fresco-{os.getuid()}"), "fresco/control.sock")
s = socket.socket(socket.AF_UNIX); s.connect(sock)
s.sendall(b'{"cmd":"status"}\n')
print(json.dumps(json.loads(s.makefile().readline()), indent=2))
PY
```

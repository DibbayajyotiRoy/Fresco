#!/bin/bash
# Check a built fresco-legacy .deb in a clean debian:buster container.
#   verify.sh <path-to-deb>
# Installs the package with apt (so Depends must resolve from buster alone), then:
#   1. ldd on every binary and bundled library: no "not found"
#   2. highest GLIBC_ symbol version <= 2.28 (what V20 / UOS 20 ships)
#   3. fresco --version and frescod --version, which must not need a display
#   4. the GUI under Xvfb for a few seconds with a throwaway HOME: it must still
#      be running when the timeout fires (exit 124) and must not print a
#      loader or GTK fatal error
set -euo pipefail

DEB="${1:?usage: verify.sh <fresco-legacy.deb>}"
SMOKE_SECONDS="${SMOKE_SECONDS:-8}"

fail() { echo "verify: FAIL: $*" >&2; exit 1; }
section() { printf '\n== %s\n' "$*"; }

cat > /etc/apt/sources.list <<'EOF'
deb http://archive.debian.org/debian buster main
deb http://archive.debian.org/debian buster-updates main
deb http://archive.debian.org/debian-security buster/updates main
EOF
echo 'Acquire::Check-Valid-Until "false";' > /etc/apt/apt.conf.d/99archive
export DEBIAN_FRONTEND=noninteractive
apt-get update -q
apt-get install -y -q --no-install-recommends "$DEB" binutils xvfb xauth dbus dbus-x11 >/tmp/install.log 2>&1 \
    || { tail -40 /tmp/install.log; fail "apt could not install $DEB with its Depends"; }

section "installed"
dpkg-query -W -f='${Package} ${Version} (${Status})\n' fresco-legacy
echo "os: $(. /etc/os-release && echo "$PRETTY_NAME"), glibc: $(ldd --version | head -1)"
dpkg-query -W -f='${Package} ${Version}\n' libglib2.0-0 libcairo2 libmpv1 2>/dev/null || true

ELFS=(/usr/bin/fresco-bin /usr/bin/frescod /opt/fresco-legacy/lib/*.so* /opt/fresco-legacy/lib/gtk-4.0/*/media/*.so)

section "ldd: unresolved libraries"
for f in "${ELFS[@]}"; do
    [ -e "$f" ] || continue
    if ldd "$f" 2>&1 | grep -q 'not found'; then
        ldd "$f" | grep 'not found'
        fail "$f has unresolved libraries"
    fi
done
echo "all $(for f in "${ELFS[@]}"; do [ -e "$f" ] && echo x; done | wc -l) ELF files resolve"

section "RUNPATH of the binaries (must be /opt/fresco-legacy/lib, not LD_LIBRARY_PATH)"
for b in /usr/bin/fresco-bin /usr/bin/frescod; do
    echo "$b: $(readelf -d "$b" | grep -E 'RUNPATH|RPATH' || echo none)"
done

section "highest GLIBC symbol version"
max=""
for f in "${ELFS[@]}"; do
    [ -e "$f" ] || continue
    v=$(objdump -T "$f" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -1 || true)
    [ -n "$v" ] || continue
    if [ -z "$max" ] || [ "$(printf '%s\n%s\n' "$max" "$v" | sort -V | tail -1)" = "$v" ]; then max="$v"; fi
    printf '  %-60s %s\n' "$f" "$v"
done
echo "max: GLIBC_$max"
[ -n "$max" ] || fail "no GLIBC symbol versions found (binutils missing?)"
[ "$(printf '%s\n%s\n' "$max" 2.28 | sort -V | tail -1)" = 2.28 ] || fail "needs GLIBC_$max (> 2.28)"

section "fresco --version / frescod --version (no display)"
env -u DISPLAY -u WAYLAND_DISPLAY /usr/bin/fresco --version
env -u DISPLAY -u WAYLAND_DISPLAY /usr/bin/frescod --version

section "GUI under Xvfb for ${SMOKE_SECONDS}s, throwaway HOME"
HOME="$(mktemp -d)"
XDG_RUNTIME_DIR="$(mktemp -d)"
export HOME XDG_RUNTIME_DIR
export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share"
chmod 700 "$XDG_RUNTIME_DIR"
log="$HOME/gui.log"
rc=0
xvfb-run -a -s "-screen 0 1280x800x24" \
    dbus-run-session -- timeout "$SMOKE_SECONDS" /usr/bin/fresco >"$log" 2>&1 || rc=$?
echo "exit code: $rc (124 = still running when the timeout fired)"
tail -20 "$log" || true
[ "$rc" -eq 124 ] || fail "GUI exited early with $rc"
if grep -E 'error while loading shared|symbol lookup|GLib-GIO-ERROR|Settings schema .* is not installed|Gtk-CRITICAL|panicked' "$log"; then
    fail "GUI printed a loader or GTK fatal error"
fi

echo
echo "verify: OK"

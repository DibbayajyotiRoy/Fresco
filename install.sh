#!/usr/bin/env bash
# Fresco — one-line installer for Debian/Ubuntu/Pop!_OS/Linux Mint
# Usage: curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | bash
set -euo pipefail

REPO="DibbayajyotiRoy/fresco"
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
RESET='\033[0m'

ok()   { echo -e "${GREEN}✓${RESET} $*"; }
fail() { echo -e "${RED}✗${RESET} $*"; exit 1; }
info() { echo -e "${BOLD}→${RESET} $*"; }
warn() { echo -e "${YELLOW}⚠${RESET} $*"; }

echo
echo -e "${BOLD}  Fresco — Live Wallpaper for Linux${RESET}"
echo    "  ───────────────────────────────────"
echo

# 1. Check OS family
if ! command -v apt-get >/dev/null 2>&1; then
  fail "Fresco requires a Debian/Ubuntu-based distro (apt-get not found)"
fi
ok "Debian-based distro detected"

# 2. Check session type
SESSION="${XDG_SESSION_TYPE:-unknown}"
if [[ "$SESSION" == "wayland" ]]; then
  info "Wayland session detected"
  info "Live wallpapers work on layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6)"
  info "GNOME Wayland shows a static frame; for full live playback log out and choose the Xorg session"
else
  ok "X11 session: $SESSION"
fi

# 3. Fetch latest .deb URL from GitHub Releases API
info "Fetching latest release from GitHub…"
API_URL="https://api.github.com/repos/${REPO}/releases/latest"
DEB_URL=$(curl -fsSL "$API_URL" \
  | grep '"browser_download_url"' \
  | grep '\.deb"' \
  | head -1 \
  | sed 's/.*"browser_download_url": "\(.*\)".*/\1/')

if [[ -z "$DEB_URL" ]]; then
  fail "Could not find a .deb in the latest release. Check https://github.com/${REPO}/releases"
fi
ok "Found package: $(basename "$DEB_URL")"

# 4. Download
TMP_DEB=$(mktemp /tmp/fresco-XXXXXX.deb)
info "Downloading…"
curl -fsSL --progress-bar -o "$TMP_DEB" "$DEB_URL"
ok "Downloaded"

# 5. Install (apt install handles deps automatically)
info "Installing (may ask for your password)…"
sudo apt-get install -y "$TMP_DEB" 2>&1 | grep -v '^Reading\|^Building\|^Selecting\|^Unpacking\|^Setting' || true
rm -f "$TMP_DEB"
ok "Installed"

# 6. Verify the bundled Wayland renderer actually loads on this OS.
# The package ships one mpvpaper build per libmpv soname generation
# (mpvpaper-libmpv2 / mpvpaper-libmpv1; older packages shipped a single
# "mpvpaper"). A build linked against a libmpv this distro doesn't ship execs
# but dies in the dynamic linker with exit 127 — apt can't catch that, so we
# probe here and, if every bundled copy is unloadable, build one locally
# against the system libmpv.
probe() { "$1" --help >/dev/null 2>&1; [[ $? -ne 127 ]]; }

renderer_ok() {
  local bin
  for bin in /usr/lib/fresco/mpvpaper-libmpv2 /usr/lib/fresco/mpvpaper-libmpv1 /usr/lib/fresco/mpvpaper; do
    [[ -x "$bin" ]] || continue
    if probe "$bin"; then return 0; fi
  done
  return 1
}

if [[ "$SESSION" == "wayland" ]] && ! renderer_ok; then
  warn "The bundled wallpaper renderer can't load this system's libmpv — building a local copy (one-time)"
  info "Installing build tools (may ask for your password)…"
  sudo apt-get install -y git gcc meson ninja-build pkg-config libmpv-dev \
    libwayland-dev wayland-protocols libegl1-mesa-dev libgl1-mesa-dev >/dev/null
  BUILD_DIR=$(mktemp -d)
  git clone -q --depth 1 --branch 1.4 https://github.com/GhostNaN/mpvpaper.git "$BUILD_DIR/mpvpaper"
  (cd "$BUILD_DIR/mpvpaper" && meson setup build >/dev/null && meson compile -C build >/dev/null)
  sudo install -m 755 "$BUILD_DIR/mpvpaper/build/mpvpaper" /usr/lib/fresco/mpvpaper
  rm -rf "$BUILD_DIR"
  if renderer_ok; then
    ok "Renderer rebuilt against this system's libmpv"
    # Restart the daemon so it picks up the fixed renderer right away.
    if pkill -x frescod 2>/dev/null; then
      (setsid frescod >/dev/null 2>&1 &) || true
    fi
  else
    warn "Renderer still can't load — run 'fresco doctor' and report the output at https://github.com/${REPO}/issues"
  fi
fi

# 7. VA-API hint
if ! command -v vainfo >/dev/null 2>&1; then
  echo
  warn "Hardware decode drivers not found — playback still works, but CPU usage will be higher"
  warn "To fix:  sudo apt install mesa-va-drivers intel-media-va-driver"
fi

echo
echo -e "${GREEN}${BOLD}  Done!${RESET}"
echo    "  Launch Fresco from your application menu, or run: fresco"
echo    "  Run 'frescod --check' to verify hardware decode."
echo

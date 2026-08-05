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

# Download-source attribution (UTM-style): the copy buttons on the website /
# README / posts prefix the one-liner with FRESCO_SOURCE=<tag>. Persisted for
# the app's anonymous telemetry (reported only if the user opts in). No tag =
# "installer".
FRESCO_SOURCE="${FRESCO_SOURCE:-installer}"
mkdir -p "$HOME/.config/fresco" 2>/dev/null || true
printf '%s' "$FRESCO_SOURCE" > "$HOME/.config/fresco/install-source" 2>/dev/null || true

# Record which HOST this copy came from, so the in-app updater keeps using it.
# Separate from install-source above: that is a campaign tag for telemetry, this
# decides where updates are fetched from. Read by update::Origin::current().
printf '%s' "${FRESCO_ORIGIN:-github}" > "$HOME/.config/fresco/install-origin" 2>/dev/null || true

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

# 3. Fetch latest .deb URL from the release API of whichever host we're using.
#
# FRESCO_ORIGIN=gitee installs from the Gitee mirror, for users in mainland
# China who cannot reliably reach GitHub. The choice is RECORDED (below) so the
# in-app updater keeps talking to the same host — an install that can't update
# is worse than no mirror at all.
FRESCO_ORIGIN="${FRESCO_ORIGIN:-github}"
case "$FRESCO_ORIGIN" in
  github)
    API_URL="https://api.github.com/repos/${REPO}/releases/latest"
    RELEASES_PAGE="https://github.com/${REPO}/releases"
    ORIGIN_LABEL="GitHub"
    ;;
  gitee)
    API_URL="https://gitee.com/api/v5/repos/${GITEE_REPO:-dibbayajyoti/fresco}/releases/latest"
    RELEASES_PAGE="https://gitee.com/${GITEE_REPO:-dibbayajyoti/fresco}/releases"
    ORIGIN_LABEL="Gitee"
    ;;
  *)
    fail "Unknown FRESCO_ORIGIN='$FRESCO_ORIGIN' (expected 'github' or 'gitee')"
    ;;
esac

info "Fetching latest release from ${ORIGIN_LABEL}…"

# Both hosts return a "browser_download_url" field, but GitHub pretty-prints its
# JSON and Gitee minifies it. The old `sed 's/.*"browser_download_url": "\(.*\)".*/\1/'`
# depended on the pretty-printed spacing AND was greedy, so it only ever worked
# by accident on one host. Match the whole key/value pair instead, tolerating any
# whitespace and no whitespace, then take just the quoted URL.
DEB_URL=$(curl -fsSL "$API_URL" \
  | grep -o '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*\.deb"' \
  | head -1 \
  | grep -o 'https\?://[^"]*\.deb')

if [[ -z "$DEB_URL" ]]; then
  fail "Could not find a .deb in the latest release. Check ${RELEASES_PAGE}"
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

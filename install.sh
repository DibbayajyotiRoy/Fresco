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
  warn "Wayland session detected — Fresco currently requires X11"
  warn "Log out and select 'Pop (on Xorg)' or similar at your login screen"
  exit 1
fi
ok "X11 session: $SESSION"

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

# 6. VA-API hint
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

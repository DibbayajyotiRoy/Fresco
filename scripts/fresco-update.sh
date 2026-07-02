#!/usr/bin/env bash
# Fresco auto-updater — fetch the latest .deb from GitHub Releases and install it.
#
# The daemon (frescod) and the GUI both launch this via `pkexec`, so it runs as
# root: there is no `sudo` inside, and apt installs without further prompting.
# Run by hand for testing with:  pkexec /usr/lib/fresco/fresco-update.sh
#
# Exit codes (callers branch on these):
#   0 = updated successfully
#   1 = generic failure (network/download/install error)
#   2 = already up to date (no-op, not an error)
#   3 = unsupported install (Flatpak, or no apt-get) — caller should route to
#       the manual-install fallback UI instead of retrying.
set -euo pipefail

REPO="${FRESCO_REPO:-DibbayajyotiRoy/fresco}"
API="https://api.github.com/repos/${REPO}/releases/latest"

# Unsupported installs first: a Flatpak sandbox can't apt-install, and a
# non-Debian system has no apt-get to install with.
if [ -e "/.flatpak-info" ]; then
  echo "fresco-update: running inside Flatpak; not supported" >&2
  exit 3
fi
if ! command -v apt-get >/dev/null 2>&1; then
  echo "fresco-update: apt-get not found; not supported" >&2
  exit 3
fi

echo "fresco-update: querying latest release of ${REPO}…"
RELEASE_JSON=$(curl -fsSL "$API")
LATEST_TAG=$(printf '%s' "$RELEASE_JSON" \
  | grep '"tag_name"' \
  | head -1 \
  | sed 's/.*"tag_name": *"\(.*\)".*/\1/')
DEB_URL=$(printf '%s' "$RELEASE_JSON" \
  | grep '"browser_download_url"' \
  | grep '\.deb"' \
  | head -1 \
  | sed 's/.*"browser_download_url": "\(.*\)".*/\1/')

if [ -z "${DEB_URL}" ]; then
  echo "fresco-update: no .deb found in the latest release" >&2
  exit 1
fi

# Compare the installed version against the latest tag with dpkg's own
# version-comparison logic rather than hand-rolling semver parsing in bash.
# A semver pre-release hyphen (1.0.0-rc1) is translated to Debian's `~`
# (1.0.0~rc1) so both comparators agree that a pre-release sorts BELOW its
# release — to dpkg a raw hyphen means "revision", which sorts ABOVE.
INSTALLED_VERSION=$(dpkg-query -W -f='${Version}' fresco 2>/dev/null || true)
LATEST_VERSION="${LATEST_TAG#v}"
LATEST_VERSION="${LATEST_VERSION/-/\~}"
if [ -n "${INSTALLED_VERSION}" ] && dpkg --compare-versions "${INSTALLED_VERSION}" ge "${LATEST_VERSION}"; then
  echo "fresco-update: already up to date (installed ${INSTALLED_VERSION}, latest ${LATEST_VERSION})"
  exit 2
fi

TMP_DEB="$(mktemp /tmp/fresco-update-XXXXXX.deb)"
trap 'rm -f "${TMP_DEB}"' EXIT

echo "STAGE: downloading"
echo "fresco-update: downloading $(basename "${DEB_URL}")…"
curl -fsSL -o "${TMP_DEB}" "${DEB_URL}"

if [ ! -s "${TMP_DEB}" ] || ! dpkg-deb -I "${TMP_DEB}" >/dev/null 2>&1; then
  echo "fresco-update: downloaded file is empty or not a valid .deb" >&2
  exit 1
fi

# apt treats an absolute path ending in .deb as a local file and resolves its
# dependencies from the repos automatically.
export DEBIAN_FRONTEND=noninteractive
echo "STAGE: installing"
echo "fresco-update: installing…"
apt-get install -y "${TMP_DEB}"

echo "STAGE: done"
echo "fresco-update: installed $(basename "${DEB_URL}")"

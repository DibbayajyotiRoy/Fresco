#!/usr/bin/env bash
# Fresco auto-updater — fetch the latest .deb from GitHub Releases and install it.
#
# The daemon (frescod) launches this via `pkexec`, so it runs as root: there is
# no `sudo` inside, and apt installs without further prompting. Run by hand for
# testing with:  pkexec /usr/lib/fresco/fresco-update.sh
set -euo pipefail

REPO="${FRESCO_REPO:-DibbayajyotiRoy/fresco}"
API="https://api.github.com/repos/${REPO}/releases/latest"

echo "fresco-update: querying latest release of ${REPO}…"
DEB_URL=$(curl -fsSL "$API" \
  | grep '"browser_download_url"' \
  | grep '\.deb"' \
  | head -1 \
  | sed 's/.*"browser_download_url": "\(.*\)".*/\1/')

if [ -z "${DEB_URL}" ]; then
  echo "fresco-update: no .deb found in the latest release" >&2
  exit 1
fi

TMP_DEB="$(mktemp /tmp/fresco-update-XXXXXX.deb)"
trap 'rm -f "${TMP_DEB}"' EXIT

echo "fresco-update: downloading $(basename "${DEB_URL}")…"
curl -fsSL -o "${TMP_DEB}" "${DEB_URL}"

# apt treats an absolute path ending in .deb as a local file and resolves its
# dependencies from the repos automatically.
export DEBIAN_FRONTEND=noninteractive
echo "fresco-update: installing…"
apt-get install -y "${TMP_DEB}"

echo "fresco-update: installed $(basename "${DEB_URL}")"

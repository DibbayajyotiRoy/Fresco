#!/usr/bin/env bash
# Fresco auto-updater — fetch the latest package for this distro from GitHub
# Releases (or the Gitee mirror) and install it.
#
# The package manager picks the asset and the install command:
#   apt-get          -> fresco-*.deb              (apt-get install)
#   pacman           -> fresco-*-x86_64.pkg.tar.zst (pacman -U)
#   dnf5 / dnf / yum -> fresco-*.x86_64.rpm       (dnf install)
#   zypper           -> not supported yet: no openSUSE asset is published
#
# The daemon (frescod) and the GUI both launch this via `pkexec`, so it runs as
# root: there is no `sudo` inside, and the package manager installs without
# further prompting. Run by hand for testing with:
#   pkexec /usr/lib/fresco/fresco-update.sh
#
# Exit codes (callers branch on these):
#   0 = updated successfully
#   1 = generic failure (network/download/install error)
#   2 = already up to date (no-op, not an error)
#   3 = unsupported install (no apt-get, pacman, dnf/dnf5/yum; or zypper) —
#       caller should route to the manual-install fallback UI instead of
#       retrying.
set -euo pipefail

REPO="${FRESCO_REPO:-DibbayajyotiRoy/fresco}"

# Which host to fetch from. Passed as `--origin <github|gitee>` by the caller
# rather than read from the environment or the user's config: this script runs
# under pkexec as root, so it has root's $HOME and a sanitised environment and
# can see neither. Defaults to github when invoked by hand.
ORIGIN="github"
while [ $# -gt 0 ]; do
  case "$1" in
    --origin) ORIGIN="${2:-github}"; shift 2 ;;
    --origin=*) ORIGIN="${1#*=}"; shift ;;
    *) echo "fresco-update: unknown argument '$1'" >&2; exit 1 ;;
  esac
done

case "$ORIGIN" in
  github) API="https://api.github.com/repos/${REPO}/releases/latest" ;;
  gitee)  API="https://gitee.com/api/v5/repos/${FRESCO_GITEE_REPO:-dibbayajyoti/fresco}/releases/latest" ;;
  *) echo "fresco-update: unknown origin '$ORIGIN'" >&2; exit 1 ;;
esac

# Unsupported installs first. The package manager decides which asset we need,
# so detect it before touching the network. zypper is checked before dnf so an
# openSUSE box that happens to have dnf installed is not sent an rpm built for
# Fedora/RHEL: no openSUSE asset is published yet.
if command -v apt-get >/dev/null 2>&1; then
  PKG_KIND="deb"
  PKG_EXT="deb"
elif command -v pacman >/dev/null 2>&1; then
  # The AUR source package `fresco` is owned by its AUR helper. Never pacman -U
  # over it; leave that install to the helper (manual-update UI instead).
  if pacman -Q fresco >/dev/null 2>&1 && ! pacman -Q fresco-bin >/dev/null 2>&1; then
    echo "fresco-update: fresco is installed from the AUR; update it with your AUR helper" >&2
    exit 3
  fi
  PKG_KIND="pkg"
  PKG_EXT="pkg.tar.zst"
elif command -v zypper >/dev/null 2>&1; then
  echo "fresco-update: zypper found but no openSUSE package is published; not supported" >&2
  exit 3
else
  PKG_KIND="rpm"
  PKG_EXT="rpm"
  PKG_TOOL=""
  for c in dnf5 dnf yum; do
    if command -v "$c" >/dev/null 2>&1; then
      PKG_TOOL="$c"
      break
    fi
  done
  if [ -z "${PKG_TOOL}" ]; then
    echo "fresco-update: no apt-get, pacman, dnf or yum found; not supported" >&2
    exit 3
  fi
fi

echo "fresco-update: querying latest release of ${REPO} from ${ORIGIN}…"
RELEASE_JSON=$(curl -fsSL "$API")
# GitHub pretty-prints its JSON; Gitee minifies it. Match the whole key/value
# pair with optional whitespace instead of assuming `": "`, and avoid the greedy
# `.*` that made the old expression swallow everything up to the last quote.
LATEST_TAG=$(printf '%s' "$RELEASE_JSON" \
  | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' \
  | head -1 \
  | sed 's/.*"\([^"]*\)"$/\1/')
case "$PKG_KIND" in
  deb)
    ASSET_MATCH='"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*\.deb"'
    ASSET_URL='https\?://[^"]*\.deb' ;;
  rpm)
    ASSET_MATCH='"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*/fresco-[^"/]*\.x86_64\.rpm"'
    ASSET_URL='https\?://[^"]*\.x86_64\.rpm' ;;
  pkg)
    ASSET_MATCH='"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*/fresco-[^"/]*-x86_64\.pkg\.tar\.zst"'
    ASSET_URL='https\?://[^"]*\.pkg\.tar\.zst' ;;
esac
DEB_URL=$(printf '%s' "$RELEASE_JSON" \
  | grep -o "$ASSET_MATCH" \
  | head -1 \
  | grep -o "$ASSET_URL" || true)

if [ -z "${DEB_URL}" ]; then
  echo "fresco-update: no .${PKG_EXT} found in the latest release" >&2
  exit 1
fi

# Compare the installed version against the latest tag. A semver pre-release
# hyphen (1.0.0-rc1) is translated to Debian's `~` (1.0.0~rc1) so both
# comparators agree that a pre-release sorts BELOW its release.
LATEST_VERSION="${LATEST_TAG#v}"
LATEST_VERSION="${LATEST_VERSION/-/\~}"
case "$PKG_KIND" in
  deb)
    # Use dpkg's own version-comparison logic rather than hand-rolling semver
    # parsing in bash. To dpkg a raw hyphen means "revision", which sorts ABOVE.
    INSTALLED_VERSION=$(dpkg-query -W -f='${Version}' fresco 2>/dev/null || true)
    if [ -n "${INSTALLED_VERSION}" ] && dpkg --compare-versions "${INSTALLED_VERSION}" ge "${LATEST_VERSION}"; then
      echo "fresco-update: already up to date (installed ${INSTALLED_VERSION}, latest ${LATEST_VERSION})"
      exit 2
    fi ;;
  rpm)
    # rpm's %{VERSION} excludes the release, matching the tag. sort -V is a
    # best-effort ordering (it does not know Debian's ~), which is enough for
    # plain X.Y.Z tags.
    # Only query when rpm says fresco is installed: `rpm -q` prints "package
    # fresco is not installed" on stdout, which would otherwise pass as a version.
    INSTALLED_VERSION=""
    if rpm -q fresco >/dev/null 2>&1; then
      INSTALLED_VERSION=$(rpm -q --qf '%{VERSION}' fresco 2>/dev/null || true)
    fi
    if [ -n "${INSTALLED_VERSION}" ] && [ "$(printf '%s\n%s\n' "${INSTALLED_VERSION}" "${LATEST_VERSION}" | sort -V | tail -n 1)" = "${INSTALLED_VERSION}" ]; then
      echo "fresco-update: already up to date (installed ${INSTALLED_VERSION}, latest ${LATEST_VERSION})"
      exit 2
    fi ;;
  pkg)
    # pacman -Q prints "name version-pkgrel"; strip the pkgrel to match the tag.
    # fresco-bin is the package we publish; fresco is the AUR source package.
    INSTALLED_VERSION=""
    for n in fresco-bin fresco; do
      if pacman -Q "$n" >/dev/null 2>&1; then
        INSTALLED_VERSION=$(pacman -Q "$n" | awk '{print $2}' | sed 's/-[^-]*$//')
        break
      fi
    done
    if [ -n "${INSTALLED_VERSION}" ] && [ "$(printf '%s\n%s\n' "${INSTALLED_VERSION}" "${LATEST_VERSION}" | sort -V | tail -n 1)" = "${INSTALLED_VERSION}" ]; then
      echo "fresco-update: already up to date (installed ${INSTALLED_VERSION}, latest ${LATEST_VERSION})"
      exit 2
    fi ;;
esac

TMP_PKG="$(mktemp /tmp/fresco-update-XXXXXX.${PKG_EXT})"
trap 'rm -f "${TMP_PKG}"' EXIT

echo "STAGE: downloading"
echo "fresco-update: downloading $(basename "${DEB_URL}")…"
# Emit "PROGRESS: <0-100>" lines so the GUI can draw a real progress bar:
# resolve the payload size, download in the background, and poll the growing
# file. Best-effort — if the size can't be determined the GUI just shows an
# indeterminate bar.
TOTAL=$(curl -fsSLI "${DEB_URL}" | tr -d '\r' | awk 'tolower($1)=="content-length:" {n=$2} END {print n+0}')
curl -fsSL -o "${TMP_PKG}" "${DEB_URL}" &
CURL_PID=$!
if [ "${TOTAL:-0}" -gt 0 ]; then
  while kill -0 "${CURL_PID}" 2>/dev/null; do
    SIZE=$(stat -c%s "${TMP_PKG}" 2>/dev/null || echo 0)
    echo "PROGRESS: $((SIZE * 100 / TOTAL))"
    sleep 0.25
  done
fi
wait "${CURL_PID}"
echo "PROGRESS: 100"

# Sanity-check the download against its own package format before handing it
# to the package manager. These check the file header only; none of the three
# package managers verifies a signature for a local file.
pkg_valid() {
  case "$PKG_KIND" in
    deb) dpkg-deb -I "$1" >/dev/null 2>&1 ;;
    rpm) rpm -qp "$1" >/dev/null 2>&1 ;;
    pkg) pacman -Qp "$1" >/dev/null 2>&1 ;;
  esac
}
if [ ! -s "${TMP_PKG}" ] || ! pkg_valid "${TMP_PKG}"; then
  echo "fresco-update: downloaded file is empty or not a valid .${PKG_EXT}" >&2
  exit 1
fi

# Each package manager treats an absolute path to a local package file as a
# local install and resolves its dependencies from the repos automatically.
case "$PKG_KIND" in
  deb)
    export DEBIAN_FRONTEND=noninteractive
    echo "STAGE: installing"
    echo "fresco-update: installing…"
    apt-get install -y "${TMP_PKG}" ;;
  rpm)
    echo "STAGE: installing"
    echo "fresco-update: installing…"
    "${PKG_TOOL}" install -y "${TMP_PKG}" ;;
  pkg)
    echo "STAGE: installing"
    echo "fresco-update: installing…"
    pacman -U --noconfirm "${TMP_PKG}" ;;
esac

echo "STAGE: done"
echo "fresco-update: installed $(basename "${DEB_URL}")"

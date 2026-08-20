#!/usr/bin/env bash
# Build the mpvpaper backend so Fresco can render live wallpapers on Wayland
# layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6, …).
#
# Usage:
#   scripts/build-mpvpaper.sh          # builds target/release/mpvpaper
#   CARGO_TARGET_DIR=foo scripts/build-mpvpaper.sh
#
# Requires: git, meson, ninja, gcc, pkg-config, libmpv-dev, libwayland-dev,
#           libwayland-egl1-mesa-dev (or equivalent), libegl1-mesa-dev.
set -euo pipefail

# Pinned upstream mpvpaper release. Keep this in sync with install.sh's
# local-rebuild fallback.
#
# 1.9 (not 1.4) because 1.4 renders a *black* wallpaper on the NVIDIA
# proprietary driver: it brings up EGL, reports success and then never presents
# a frame. Upstream's own 1.4 notes admit "some Nvidia GPU users still
# experiencing issues" after the render-loop rewrite; 1.6 shipped the fix ("fix
# support for the Nvidia proprietary drivers", 3 commits) and 1.7 reworked the
# compositor render-loop handshake, again calling out Nvidia. Nothing in Fresco
# can work around it — the bug is entirely inside mpvpaper.
#
# One pin for BOTH release runners (ubuntu-24.04 → libmpv2, ubuntu-22.04 →
# libmpv1). Verified 1.9 still builds on the 22.04 base:
#   * meson.build declares no meson_version and no dependency version floors.
#   * The only 1.9 build change is get_pkgconfig_variable() → get_variable(),
#     which meson has supported since 0.58; 22.04 ships 0.61.
#   * 1.9's extra protocol (wlr-foreign-toplevel-management) is vendored in the
#     repo's proto/, so wayland-protocols 1.25 on 22.04 is still enough (only
#     stable/xdg-shell is pulled from it).
#   * The only libmpv API 1.9 adds over 1.4 is mpv_free() and
#     mpv_render_context_report_swap(), both present in mpv 0.34 (libmpv1).
VERSION="1.9"
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}/release"

command -v meson >/dev/null 2>&1 || { echo "meson is required"; exit 1; }
command -v ninja >/dev/null 2>&1 || { echo "ninja is required"; exit 1; }

mkdir -p "$TARGET"
BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "$BUILD_DIR"' EXIT

echo "Building mpvpaper $VERSION into $TARGET ..."
cd "$BUILD_DIR"
git clone --depth 1 --branch "$VERSION" https://github.com/GhostNaN/mpvpaper.git mpvpaper
cd mpvpaper
meson setup build
meson compile -C build
cp build/mpvpaper "$TARGET/mpvpaper"
echo "Built: $TARGET/mpvpaper"

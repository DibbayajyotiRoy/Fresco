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

VERSION="1.4"
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

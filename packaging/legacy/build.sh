#!/bin/bash
# Build the fresco-legacy .deb: Fresco for Deepin V20 / UOS 20 / Debian 10
# (glibc 2.28, no GTK 4, no libadwaita, glib 2.58).
#
# Run inside debian:buster, with the checkout mounted read-only:
#   docker run --rm -v "$PWD":/src:ro -v fresco-legacy-cache:/cache \
#       -v "$PWD/out":/out debian:buster bash /src/packaging/legacy/build.sh /src /out
#
# Stages (each is skipped when its stamp under $PREFIX/.stamps exists):
#   1. apt: build tools and -dev packages buster does have
#   2. stack: glib, graphene, fribidi, harfbuzz, pango, GTK 4.6, libadwaita 1.1
#      built into $PREFIX (/opt/fresco-legacy). Their RUNPATH is $PREFIX/lib, so
#      the binaries find them without LD_LIBRARY_PATH and child processes keep
#      the system libraries.
#   3. rustup 1.91.0 + cargo build --release --all-features --locked
#   4. stage and dpkg-deb the package (packaging/legacy/stage.py)
#
# No mpvpaper in this package. It is the Wayland layer-shell renderer, and 1.9
# does not compile against buster's wayland 1.16 headers (wl_output v4
# name/description). V20 runs Fresco on X11, which plays through the in-process
# libmpv.so.1 (src/daemon/mpv), so nothing here needs it.
#
# Env: JOBS (parallel make jobs, default nproc), FRESCO_LEGACY_CACHE (downloads,
# rustup, cargo and the cargo target dir; default /cache), PREFIX.
set -euo pipefail

REPO="${1:?usage: build.sh <repo-dir> <out-dir>}"
OUT="${2:?usage: build.sh <repo-dir> <out-dir>}"
PREFIX="${PREFIX:-/opt/fresco-legacy}"
CACHE="${FRESCO_LEGACY_CACHE:-/cache}"
JOBS="${JOBS:-$(nproc)}"
RUST_VERSION=1.91.0
HERE="$REPO/packaging/legacy"
. "$HERE/versions.env"

log() { printf '\n==> %s\n' "$*" >&2; }
die() { echo "build.sh: $*" >&2; exit 1; }

# Everything we build links against $PREFIX/lib; the -rpath makes that stick.
export DEBIAN_FRONTEND=noninteractive
export PATH="$PREFIX/bin:/usr/local/bin:$PATH"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig:$PREFIX/share/pkgconfig"
export LDFLAGS="-Wl,-rpath,$PREFIX/lib -Wl,--enable-new-dtags ${LDFLAGS:-}"

SRC="$CACHE/src"
DL="$CACHE/dl"
mkdir -p "$SRC" "$DL" "$OUT" "$PREFIX/.stamps"

# ── 1. apt ──────────────────────────────────────────────────────────────────
apt_setup() {
    # buster left the regular mirrors; its last packages live on the archive.
    cat > /etc/apt/sources.list <<'EOF'
deb http://archive.debian.org/debian buster main
deb http://archive.debian.org/debian buster-updates main
deb http://archive.debian.org/debian-security buster/updates main
EOF
    echo 'Acquire::Check-Valid-Until "false";' > /etc/apt/apt.conf.d/99archive
    apt-get update -q
    apt-get install -y -q --no-install-recommends \
        build-essential pkg-config gettext curl ca-certificates xz-utils \
        python3 python3-pip python3-setuptools ninja-build \
        libffi-dev libpcre3-dev zlib1g-dev libmount-dev \
        libcairo2-dev libgdk-pixbuf2.0-dev libepoxy-dev libxkbcommon-dev \
        libfontconfig1-dev libfreetype6-dev libpng-dev \
        libx11-dev libxext-dev libxrandr-dev libxi-dev libxinerama-dev \
        libxcursor-dev libxdamage-dev libxfixes-dev libxcomposite-dev libxrender-dev \
        libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev \
        libwayland-dev libegl1-mesa-dev libgl1-mesa-dev
    # buster's meson is 0.49; GTK 4.6 and libadwaita need >= 0.59. meson 1.3 is
    # the last series that still runs on buster's python 3.7.
    python3 -m pip install -q --no-cache-dir "meson==1.3.2" "tomli==2.0.1"
}

# ── helpers ─────────────────────────────────────────────────────────────────
fetch() { # url sha256 dest
    local url="$1" sha="$2" dest="$3"
    if [ ! -f "$dest" ]; then
        curl -fsSL --retry 3 -o "$dest.part" "$url"
        mv "$dest.part" "$dest"
    fi
    echo "$sha  $dest" | sha256sum -c --quiet || { rm -f "$dest"; die "checksum mismatch: $url"; }
}

# unpack_src <name-version> <url> <sha256> → $SRC/<name-version>, fresh each time
unpack_src() {
    local name="$1" url="$2" sha="$3" file
    file="$DL/$(basename "$url")"
    fetch "$url" "$sha" "$file"
    rm -rf "${SRC:?}/${name:?}"
    tar -xf "$file" -C "$SRC"
}

stamped() { [ -f "$PREFIX/.stamps/$1" ]; }
mark() { touch "$PREFIX/.stamps/$1"; }

# meson_build <stamp> <source-dir> [meson options...]
meson_build() {
    local stamp="$1" dir="$2"; shift 2
    stamped "$stamp" && { log "$stamp: cached"; return; }
    log "building $stamp"
    (
        cd "$dir"
        rm -rf build
        meson setup build --prefix="$PREFIX" --libdir=lib --buildtype=release "$@"
        ninja -C build -j"$JOBS"
        ninja -C build install
    )
    mark "$stamp"
}

# ── 2. the GTK stack ────────────────────────────────────────────────────────
build_stack() {
    # The stamp prefix is a revision of this recipe: bump it when the options change.
    local r=r1

    unpack_src "glib-$GLIB_VERSION" "$GLIB_URL" "$GLIB_SHA256"
    meson_build "glib-$GLIB_VERSION-$r" "$SRC/glib-$GLIB_VERSION" \
        -Dman=false -Dgtk_doc=false -Dtests=false \
        -Dsysprof=disabled -Dselinux=disabled -Dxattr=false -Dlibelf=disabled

    unpack_src "graphene-$GRAPHENE_VERSION" "$GRAPHENE_URL" "$GRAPHENE_SHA256"
    meson_build "graphene-$GRAPHENE_VERSION-$r" "$SRC/graphene-$GRAPHENE_VERSION" \
        -Dintrospection=disabled -Dgtk_doc=false -Dtests=false -Dinstalled_tests=false

    unpack_src "fribidi-$FRIBIDI_VERSION" "$FRIBIDI_URL" "$FRIBIDI_SHA256"
    meson_build "fribidi-$FRIBIDI_VERSION-$r" "$SRC/fribidi-$FRIBIDI_VERSION" \
        -Ddocs=false -Dtests=false

    unpack_src "harfbuzz-$HARFBUZZ_VERSION" "$HARFBUZZ_URL" "$HARFBUZZ_SHA256"
    meson_build "harfbuzz-$HARFBUZZ_VERSION-$r" "$SRC/harfbuzz-$HARFBUZZ_VERSION" \
        -Dglib=disabled -Dgobject=disabled -Dicu=disabled -Dcairo=disabled \
        -Dintrospection=disabled -Ddocs=disabled -Dtests=disabled -Dbenchmark=disabled

    unpack_src "pango-$PANGO_VERSION" "$PANGO_URL" "$PANGO_SHA256"
    meson_build "pango-$PANGO_VERSION-$r" "$SRC/pango-$PANGO_VERSION" \
        -Dintrospection=disabled -Dgtk_doc=false -Dlibthai=disabled -Dxft=disabled

    # X11 only: V20 is an X11 desktop and GTK's Wayland backend needs
    # wayland >= 1.20 and xkbcommon work that buster cannot provide.
    unpack_src "gtk-$GTK_VERSION" "$GTK_URL" "$GTK_SHA256"
    meson_build "gtk-$GTK_VERSION-$r" "$SRC/gtk-$GTK_VERSION" \
        -Dx11-backend=true -Dwayland-backend=false -Dbroadway-backend=false \
        -Dmedia-gstreamer=enabled -Dmedia-ffmpeg=disabled \
        -Dprint-cups=disabled -Dvulkan=disabled -Dcloudproviders=disabled \
        -Dsysprof=disabled -Dtracker=disabled -Dcolord=disabled \
        -Dintrospection=disabled -Dgtk_doc=false -Dman-pages=false \
        -Ddemos=false -Dbuild-examples=false -Dbuild-tests=false -Dinstall-tests=false

    unpack_src "libadwaita-$LIBADWAITA_VERSION" "$LIBADWAITA_URL" "$LIBADWAITA_SHA256"
    meson_build "libadwaita-$LIBADWAITA_VERSION-$r" "$SRC/libadwaita-$LIBADWAITA_VERSION" \
        -Dintrospection=disabled -Dvapi=false -Dgtk_doc=false \
        -Dtests=false -Dexamples=false

    # Schemas GTK's file chooser reads; the install step normally compiles them.
    glib-compile-schemas "$PREFIX/share/glib-2.0/schemas"
}

# ── 3. Rust ─────────────────────────────────────────────────────────────────
build_rust() {
    export RUSTUP_HOME="$CACHE/rustup" CARGO_HOME="$CACHE/cargo"
    export PATH="$CARGO_HOME/bin:$PATH"
    if [ ! -x "$CARGO_HOME/bin/cargo" ]; then
        log "installing rust $RUST_VERSION"
        local init="$DL/rustup-init"
        fetch "https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init" \
            "$(curl -fsSL https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init.sha256 | cut -d' ' -f1)" "$init"
        chmod +x "$init"
        "$init" -y --profile minimal --default-toolchain "$RUST_VERSION"
    fi
    rustc --version
}

# ── 4. cargo ────────────────────────────────────────────────────────────────
build_app() {
    local work="$CACHE/work"
    rm -rf "$work" && mkdir -p "$work"
    tar -C "$REPO" --exclude=./target --exclude=./.git -cf - . | tar -C "$work" -xf -

    # rpath on the Rust binaries too (see the header comment).
    export RUSTFLAGS="-C link-arg=-Wl,-rpath,$PREFIX/lib -C link-arg=-Wl,--enable-new-dtags"
    # Same telemetry channel as the .deb; the build is still a deb-family build.
    export FRESCO_CHANNEL=deb
    export CARGO_TARGET_DIR="$CACHE/target"

    log "cargo build --release --all-features --locked"
    (cd "$work" && cargo build --release --all-features --locked -j "$JOBS")
    echo "$work" > "$CACHE/work.path"
}

# ── 5. package ──────────────────────────────────────────────────────────────
stage_deb() {
    local work
    work="$(cat "$CACHE/work.path")"
    python3 "$HERE/stage.py" \
        --cargo-toml "$work/Cargo.toml" \
        --root "$work" \
        --release-dir "$CARGO_TARGET_DIR/release" \
        --prefix "$PREFIX" \
        --versions "$HERE/versions.env" \
        --postinst "$work/packaging/debian/postinst" \
        --launcher "$HERE/fresco-launcher" \
        --out "$OUT"
}

apt_setup
build_stack
build_rust
build_app
stage_deb
ls -l "$OUT"

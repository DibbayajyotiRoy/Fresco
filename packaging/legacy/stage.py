#!/usr/bin/env python3
"""Stage the fresco-legacy .deb payload and run dpkg-deb (called by build.sh).

The payload is the main package's asset list from Cargo.toml
([package.metadata.deb].assets), so both packages install the same files at the
same paths. Four things differ:

  * usr/bin/fresco is packaging/legacy/fresco-launcher (no GTK check, sets the
    GDK/GSK defaults and the XDG_DATA_DIRS for the bundled schemas).
  * both mpvpaper renderers are dropped (Wayland only; see build.sh).
  * the bundled GTK stack from $PREFIX/lib, schemas and locales goes to
    /opt/fresco-legacy (the binaries reach it through their RUNPATH).
  * Depends lists buster / Deepin V20 package names, not the Fedora/Ubuntu ones.
"""
import argparse
import os
import shutil
import subprocess
import sys

import tomli

# Hard deps: everything the binaries and bundled libraries NEEDED that buster
# does not put in the base system. verify.sh checks this list with ldd in a
# clean buster container, so a missing name shows up there, not on a user's box.
DEPENDS = ", ".join([
    "libglib2.0-bin", "ffmpegthumbnailer",
    "libmpv1",
    "libwayland-client0", "libwayland-egl1", "libwayland-cursor0",
    "libegl1", "libgl1",
    "libcairo2", "libcairo-gobject2", "libgdk-pixbuf2.0-0", "libepoxy0",
    "libxkbcommon0", "libfontconfig1", "libfreetype6", "libpng16-16",
    "libcairo-script-interpreter2", "libjpeg62-turbo", "libtiff5",
    "libffi6", "libpcre3", "zlib1g", "libmount1",
    "libx11-6", "libxext6", "libxi6", "libxrandr2", "libxcursor1",
    "libxdamage1", "libxfixes3", "libxinerama1", "libxcomposite1", "libxrender1",
    "libgstreamer1.0-0", "libgstreamer-plugins-base1.0-0",
    "libgstreamer-plugins-bad1.0-0", "libgstreamer-gl1.0-0",
])
RECOMMENDS = ", ".join([
    "gstreamer1.0-plugins-good", "gstreamer1.0-libav",
    "xdg-utils", "policykit-1 | pkexec", "fonts-noto-core", "fonts-inter",
    "adwaita-icon-theme", "hicolor-icon-theme",
    "pulseaudio-utils | pipewire-bin", "ffmpeg",
])
DESCRIPTION = """Live wallpapers for Linux (Deepin V20 / UOS 20 / Debian 10 build)
 Fresco sets videos, GIFs, images, slideshows and playlists as your desktop
 wallpaper. This build carries its own GTK 4.6, libadwaita 1.1, pango 1.50 and
 glib 2.68 in /opt/fresco-legacy, because those distributions ship GTK 3 at
 most. Versions and sources of the bundled libraries are listed in
 /usr/share/doc/fresco-legacy/bundled-versions.env.
"""

# Payload entries that differ from the main package.
DROP = {"target/release/mpvpaper-libmpv1", "target/release/mpvpaper-libmpv2"}
REPLACE = {"packaging/debian/fresco-launcher": "launcher"}


def cargo_metadata(path):
    with open(path, "rb") as f:
        cfg = tomli.load(f)
    pkg = cfg["package"]
    deb = pkg["metadata"]["deb"]
    return pkg["version"], deb["maintainer"], pkg.get("repository", ""), deb["assets"]


def dest_path(src, dst):
    # Cargo.toml writes destinations either as a full path or as a directory
    # with a trailing slash (the file keeps its basename).
    return os.path.join(dst, os.path.basename(src)) if dst.endswith("/") else dst


def copy_file(src, dst, mode):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copy2(src, dst)
    os.chmod(dst, int(mode, 8))


def copy_tree_preserving_links(src, dst):
    # cp -a keeps the soname symlinks (libgtk-4.so.1 -> libgtk-4.so.1.0.0) intact.
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    subprocess.run(["cp", "-a", src, dst], check=True)


def installed_kib(root):
    total = 0
    for dirpath, _dirs, files in os.walk(root):
        for name in files:
            p = os.path.join(dirpath, name)
            if not os.path.islink(p):
                total += os.path.getsize(p)
    return (total + 1023) // 1024


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-toml", required=True)
    ap.add_argument("--root", required=True, help="repo copy the asset sources are relative to")
    ap.add_argument("--release-dir", required=True, help="cargo target/release (target/release/* sources)")
    ap.add_argument("--prefix", required=True, help="GTK stack prefix, e.g. /opt/fresco-legacy")
    ap.add_argument("--versions", required=True)
    ap.add_argument("--postinst", required=True)
    ap.add_argument("--launcher", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    version, maintainer, repo_url, assets = cargo_metadata(a.cargo_toml)
    stage = os.path.join(a.out, "stage")
    shutil.rmtree(stage, ignore_errors=True)
    os.makedirs(os.path.join(stage, "DEBIAN"))

    for src, dst, mode in assets:
        if src in DROP:
            continue
        if src in REPLACE:
            src_path = a.launcher
        elif src.startswith("target/release/"):
            src_path = os.path.join(a.release_dir, os.path.basename(src))
        else:
            src_path = os.path.join(a.root, src)
        if not os.path.exists(src_path):
            sys.exit(f"stage.py: missing payload file {src_path} (asset {src})")
        copy_file(src_path, os.path.join(stage, dest_path(src, dst)), mode)

    # The bundled stack. lib/ holds only what build.sh built (glib, graphene,
    # fribidi, harfbuzz, pango, gtk, libadwaita, gstreamer-gl shims): no headers,
    # no pkgconfig, no static archives.
    opt = os.path.join(stage, a.prefix.lstrip("/"))
    os.makedirs(os.path.join(opt, "lib"))
    libs = [f for f in os.listdir(os.path.join(a.prefix, "lib")) if ".so" in f]
    for f in sorted(libs):
        copy_tree_preserving_links(os.path.join(a.prefix, "lib", f), os.path.join(opt, "lib", f))
    copy_tree_preserving_links(os.path.join(a.prefix, "lib", "gtk-4.0"), os.path.join(opt, "lib", "gtk-4.0"))
    for share in ("share/glib-2.0/schemas", "share/locale", "share/gtk-4.0", "share/libadwaita-1"):
        src = os.path.join(a.prefix, share)
        if os.path.exists(src):
            copy_tree_preserving_links(src, os.path.join(opt, share))

    doc = os.path.join(stage, "usr/share/doc/fresco-legacy")
    os.makedirs(doc)
    shutil.copy2(a.versions, os.path.join(doc, "bundled-versions.env"))

    postinst = os.path.join(stage, "DEBIAN/postinst")
    shutil.copy2(a.postinst, postinst)
    os.chmod(postinst, 0o755)

    size = installed_kib(stage)
    control = "\n".join([
        "Package: fresco-legacy",
        f"Version: {version}-1",
        "Architecture: amd64",
        f"Maintainer: {maintainer}",
        f"Installed-Size: {size}",
        f"Depends: {DEPENDS}",
        f"Recommends: {RECOMMENDS}",
        "Conflicts: fresco",
        "Replaces: fresco",
        "Section: video",
        "Priority: optional",
        f"Homepage: {repo_url}",
        "Description: " + DESCRIPTION.rstrip("\n"),
        "",
    ])
    with open(os.path.join(stage, "DEBIAN/control"), "w", encoding="utf-8") as f:
        f.write(control)

    deb = os.path.join(a.out, f"fresco-legacy_{version}-1_amd64.deb")
    subprocess.run(["dpkg-deb", "--build", "--root-owner-group", stage, deb], check=True)
    shutil.rmtree(stage)
    print(f"staged {deb} ({os.path.getsize(deb)} bytes, installed {size} KiB)")


if __name__ == "__main__":
    main()

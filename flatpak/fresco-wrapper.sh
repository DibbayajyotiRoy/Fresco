#!/bin/sh
# Wayland security-context workaround — see the finish-args comments in the
# manifest and docs/FLATHUB.md ("Wayland security context & layer-shell").
#
# The manifest deliberately does NOT request --socket=wayland: with it,
# flatpak >= 1.16 connects the app through a wp_security_context_manager_v1
# proxy socket, and compositors that filter privileged globals for
# security-context clients (COSMIC's cosmic-comp confirmed; sway/wlroots have
# the same mechanism) hide zwlr_layer_shell_v1 — which the wallpaper backend
# (bundled mpvpaper) requires; without it mpvpaper crashes. Instead the host's
# real Wayland socket is exposed via --filesystem=xdg-run/wayland-*, but
# flatpak scrubs WAYLAND_DISPLAY whenever the wayland socket permission is
# absent, so restore it here before exec'ing the real binary.
#
# Installed as both /app/bin/fresco and /app/bin/frescod; the real binaries
# are installed with a -real suffix. mpvpaper is spawned by frescod and
# inherits the restored environment.
if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    for s in "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"/wayland-*; do
        case "$s" in
            *.lock | *-renderD*) continue ;;
        esac
        # Inside the sandbox the granted socket appears as a symlink into
        # /run/flatpak, so accept symlinks as well as sockets.
        if [ -S "$s" ] || [ -h "$s" ]; then
            WAYLAND_DISPLAY=$(basename "$s")
            export WAYLAND_DISPLAY
            break
        fi
    done
fi
exec "/app/bin/$(basename "$0")-real" "$@"

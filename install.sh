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
  # This script only installs .deb packages. Point other distros at the route
  # that matches their package manager instead of a dead end.
  OS_FAMILY=$(. /etc/os-release 2>/dev/null; echo " ${ID:-} ${ID_LIKE:-} ") || true
  case "$OS_FAMILY" in
    *" arch "*|*" manjaro "*|*" endeavouros "*|*" cachyos "*|*" garuda "*)
      fail "Arch-based system: Fresco is in the AUR as 'fresco' and 'fresco-bin'. Install one with an AUR helper, e.g. yay -S fresco-bin" ;;
    *" fedora "*|*" rhel "*|*" centos "*|*" rocky "*|*" almalinux "*|*" nobara "*|*" bazzite "*)
      fail "Fedora/RHEL-based system: download the .rpm from https://github.com/${REPO}/releases, then run: sudo dnf install ./fresco-*.rpm" ;;
    *)
      fail "This installer supports Debian/Ubuntu-based distros (apt-get not found). Download a package for your distro from https://github.com/${REPO}/releases" ;;
  esac
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

# 6. Verify the bundled Wayland renderer is both loadable AND recent enough.
# The package ships one mpvpaper build per libmpv soname generation
# (mpvpaper-libmpv2 / mpvpaper-libmpv1; older packages shipped a single
# "mpvpaper"). Two things can be wrong with it, and apt can catch neither:
#
#   1. It doesn't load. A build linked against a libmpv this distro doesn't ship
#      execs but dies in the dynamic linker with exit 127.
#   2. It loads but is too old. mpvpaper before 1.6 brings up EGL, reports
#      success and then renders nothing on the NVIDIA proprietary driver — a
#      completely black wallpaper with a clean log.
#
# Both end in the same place, so both trigger the same one-time local rebuild
# against the system libmpv. The age check is a local feature probe, not a
# network lookup: mpvpaper ships no --version, but --auto-mode was added in 1.9
# (the version scripts/build-mpvpaper.sh pins), so its presence in --help is our
# "new enough" signal. Keep MPVPAPER_VERSION in sync with that script.
MPVPAPER_VERSION="1.9"

probe() { "$1" --help >/dev/null 2>&1; [[ $? -ne 127 ]]; }
# Same 1.9 marker frescod itself fingerprints (see mpvpaper_version_from_help).
recent() { "$1" --help 2>/dev/null | grep -q -- '--auto-mode'; }

renderer_ok() {
  local bin
  for bin in /usr/lib/fresco/mpvpaper-libmpv2 /usr/lib/fresco/mpvpaper-libmpv1 /usr/lib/fresco/mpvpaper; do
    [[ -x "$bin" ]] || continue
    if probe "$bin" && recent "$bin"; then return 0; fi
  done
  return 1
}

# Distinguish the two failures so the message tells the truth about which it is.
renderer_loads() {
  local bin
  for bin in /usr/lib/fresco/mpvpaper-libmpv2 /usr/lib/fresco/mpvpaper-libmpv1 /usr/lib/fresco/mpvpaper; do
    [[ -x "$bin" ]] || continue
    if probe "$bin"; then return 0; fi
  done
  return 1
}

if [[ "$SESSION" == "wayland" ]] && ! renderer_ok; then
  if renderer_loads; then
    warn "The bundled wallpaper renderer is older than $MPVPAPER_VERSION (can render black on NVIDIA) — building a current copy (one-time)"
  else
    warn "The bundled wallpaper renderer can't load this system's libmpv — building a local copy (one-time)"
  fi
  info "Installing build tools (may ask for your password)…"
  sudo apt-get install -y git gcc meson ninja-build pkg-config libmpv-dev \
    libwayland-dev wayland-protocols libegl1-mesa-dev libgl1-mesa-dev >/dev/null
  BUILD_DIR=$(mktemp -d)
  git clone -q --depth 1 --branch "$MPVPAPER_VERSION" https://github.com/GhostNaN/mpvpaper.git "$BUILD_DIR/mpvpaper"
  # EGL context fallback for older GPUs: upstream 1.9 only tries desktop GL
  # core contexts 4.6-3.0 and exits 1 if none work, even though mpv only
  # needs GL 2.1/GLES 2.0. Embedded inline (this script runs standalone via
  # curl | bash, with no repo checkout to read a patch file from) — keep in
  # sync with packaging/mpvpaper/0001-egl-context-gl-compat-and-gles-fallback.patch
  # and scripts/build-mpvpaper.sh.
  MPVPAPER_PATCH="$BUILD_DIR/egl-fallback.patch"
  cat > "$MPVPAPER_PATCH" <<'MPVPAPER_PATCH_EOF'
--- a/src/main.c
+++ b/src/main.c
@@ -69,6 +69,7 @@ struct toplevel_handle_state {
 static EGLConfig egl_config;
 static EGLDisplay *egl_display;
 static EGLContext *egl_context;
+static bool egl_is_gles;
 
 static mpv_handle *mpv;
 static mpv_render_context *mpv_glcontext;
@@ -659,25 +660,89 @@ static void init_egl(struct wl_state *state) {
     }
 
     // Check for OpenGL compatibility for creating egl context
-    static const struct { int major, minor; } gl_versions[] = {
-        {4, 6}, {4, 5}, {4, 4}, {4, 3}, {4, 2}, {4, 1}, {4, 0},
-        {3, 3}, {3, 2}, {3, 1}, {3, 0},
-        {0, 0}
+    //
+    // Try desktop GL core contexts first (4.6 down to 3.0), then finally
+    // desktop GL compatibility-profile 2.1 (mpv's documented desktop-GL
+    // floor), before giving up on desktop GL entirely. Older/legacy drivers
+    // (pre-~2014 Intel/Radeon/NVIDIA, and some software rasterizers) cannot
+    // satisfy a GL 3.0+ core context, which is all the original code ever
+    // asked for, so eglCreateContext failed on them even though mpv itself
+    // only needs GL 2.1 or GLES 2.0.
+    //
+    // Per EGL_KHR_create_context, EGL_CONTEXT_OPENGL_PROFILE_MASK is ignored
+    // below GL 3.2, so there is no separate 3.0-compatibility entry here —
+    // it would just retry the same request as the 3.0-core entry above. It's
+    // the version drop to 2.1 that unlocks compatibility-only drivers, not
+    // the profile bit.
+    static const struct { int major, minor; EGLenum profile; } gl_versions[] = {
+        {4, 6, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {4, 5, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {4, 4, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {4, 3, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {4, 2, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {4, 1, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {4, 0, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {3, 3, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {3, 2, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {3, 1, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {3, 0, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT},
+        {2, 1, EGL_CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT},
+        {0, 0, 0}
     };
     egl_context = NULL;
     for (uint i=0; gl_versions[i].major > 0; i++) {
         const EGLint ctx_attrib[] = {
             EGL_CONTEXT_MAJOR_VERSION, gl_versions[i].major,
             EGL_CONTEXT_MINOR_VERSION, gl_versions[i].minor,
+            EGL_CONTEXT_OPENGL_PROFILE_MASK, gl_versions[i].profile,
             EGL_NONE
         };
         egl_context = eglCreateContext(egl_display, egl_config, EGL_NO_CONTEXT, ctx_attrib);
         if (egl_context) {
             if (VERBOSE)
-                cflp_info("OpenGL %i.%i EGL context created", gl_versions[i].major, gl_versions[i].minor);
+                cflp_info("OpenGL %i.%i (%s) EGL context created", gl_versions[i].major, gl_versions[i].minor,
+                          gl_versions[i].profile == EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT ? "core" : "compatibility");
             break;
         }
     }
+
+    // Final fallback: OpenGL ES 2.0. This is mpv's documented GLES floor, and
+    // is what remains after llvmpipe/softpipe, very old GPUs, and some
+    // embedded/ARM GPU drivers that never expose desktop GL 2.1+.
+    if (!egl_context) {
+        if (eglBindAPI(EGL_OPENGL_ES_API)) {
+            const EGLint gles_win_attrib[] = {
+                EGL_SURFACE_TYPE, EGL_WINDOW_BIT,
+                EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT,
+                EGL_RED_SIZE, 8,
+                EGL_GREEN_SIZE, 8,
+                EGL_BLUE_SIZE, 8,
+                EGL_ALPHA_SIZE, 8,
+                EGL_NONE
+            };
+            EGLConfig gles_config;
+            EGLint gles_num_config;
+            if (eglChooseConfig(egl_display, gles_win_attrib, &gles_config, 1, &gles_num_config)) {
+                const EGLint gles_ctx_attrib[] = {
+                    EGL_CONTEXT_CLIENT_VERSION, 2,
+                    EGL_NONE
+                };
+                egl_context = eglCreateContext(egl_display, gles_config, EGL_NO_CONTEXT, gles_ctx_attrib);
+                if (egl_context) {
+                    egl_config = gles_config;
+                    egl_is_gles = true;
+                    if (VERBOSE)
+                        cflp_info("OpenGL ES 2.0 EGL context created");
+                }
+            } else {
+                cflp_error("No GLES 2.0 frame buffer config: %s", eglGetErrorString(eglGetError()));
+            }
+        }
+        // If eglBindAPI(EGL_OPENGL_ES_API) itself failed, egl_context stays
+        // NULL and we fall straight through to the "Failed to create EGL
+        // context" error below — same as any other exhausted fallback.
+    }
+
     if (!egl_context) {
         cflp_error("Failed to create EGL context %s", eglGetErrorString(eglGetError()));
         exit_mpvpaper(EXIT_FAILURE);
@@ -888,7 +953,13 @@ static void layer_surface_configure(void *data, struct zwlr_layer_surface_v1 *su
         // After making EGL_NO_SURFACE current to a context
         // Only with the Nvidia Pro drivers will set the draw buffer state to GL_NONE
         // So we are going to force GL_BACK just like Mesa's EGL implementation
-        glDrawBuffer(GL_BACK);
+        //
+        // glDrawBuffer doesn't exist in GLES (only desktop GL); calling it on
+        // the GLES 2.0 fallback context resolves to a NULL/bogus function
+        // pointer and crashes. GLES's default framebuffer draw buffer is
+        // already fixed to the back buffer, so skipping it here is correct.
+        if (!egl_is_gles)
+            glDrawBuffer(GL_BACK);
 
         glClearColor(0.0f, 0.0f, 0.0f, 0.0f);
 
MPVPAPER_PATCH_EOF
  (cd "$BUILD_DIR/mpvpaper" && git apply --verbose "$MPVPAPER_PATCH") || { echo "mpvpaper EGL fallback patch failed to apply against $MPVPAPER_VERSION"; exit 1; }

  # Opt-in lock-screen wallpaper (cosmic-comp's session-lock-layer extension):
  # embedded inline for the same reason as the EGL patch above (this script
  # runs standalone via curl | bash, with no repo checkout to read a patch
  # file from) -- keep in sync with
  # packaging/mpvpaper/0002-cosmic-session-lock-show-on-lock.patch and
  # scripts/build-mpvpaper.sh.
  MPVPAPER_SHOW_ON_LOCK_PATCH="$BUILD_DIR/show-on-lock.patch"
  cat > "$MPVPAPER_SHOW_ON_LOCK_PATCH" <<'MPVPAPER_SHOW_ON_LOCK_PATCH_EOF'
--- a/meson.build
+++ b/meson.build
@@ -13,15 +13,21 @@ scanner=find_program('wayland-scanner')
 scanner_private_code=generator(scanner,output: '@BASENAME@-protocol.c',arguments: ['private-code','@INPUT@','@OUTPUT@'])
 scanner_client_header=generator(scanner,output: '@BASENAME@-client-protocol.h',arguments: ['client-header','@INPUT@','@OUTPUT@'])

+# cosmic-session-lock-layer-v1: optional, only used when MPVPAPER_SHOW_ON_LOCK
+# is set and the compositor advertises cosmic_session_lock_layer_manager_v1
+# (currently cosmic-comp). Generated unconditionally like the other vendored
+# protocols; binding it at runtime is what stays optional.
 protocols_src=[
   scanner_private_code.process('proto/wlr-layer-shell-unstable-v1.xml'),
   scanner_private_code.process('proto/wlr-foreign-toplevel-management-unstable-v1.xml'),
+  scanner_private_code.process('proto/cosmic-session-lock-layer-v1.xml'),
   scanner_private_code.process(wl_protocols.get_variable('pkgdatadir')+'/stable/xdg-shell/xdg-shell.xml')
 ]

 protocols_headers=[
   scanner_client_header.process('proto/wlr-layer-shell-unstable-v1.xml'),
-  scanner_client_header.process('proto/wlr-foreign-toplevel-management-unstable-v1.xml')
+  scanner_client_header.process('proto/wlr-foreign-toplevel-management-unstable-v1.xml'),
+  scanner_client_header.process('proto/cosmic-session-lock-layer-v1.xml')
 ]

 lib_protocols=static_library('protocols',protocols_src+protocols_headers,dependencies: wl_client)
diff --git a/proto/cosmic-session-lock-layer-v1.xml b/proto/cosmic-session-lock-layer-v1.xml
new file mode 100644
index 0000000..a320457
--- /dev/null
+++ b/proto/cosmic-session-lock-layer-v1.xml
@@ -0,0 +1,65 @@
+<protocol name="cosmic_session_lock_v1">
+  <copyright>
+    Copyright © 2026 System76
+
+    Permission is hereby granted, free of charge, to any person obtaining a
+    copy of this software and associated documentation files (the "Software"),
+    to deal in the Software without restriction, including without limitation
+    the rights to use, copy, modify, merge, publish, distribute, sublicense,
+    and/or sell copies of the Software, and to permit persons to whom the
+    Software is furnished to do so, subject to the following conditions:
+
+    The above copyright notice and this permission notice (including the next
+    paragraph) shall be included in all copies or substantial portions of the
+    Software.
+
+    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
+    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
+    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL
+    THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
+    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
+    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
+    DEALINGS IN THE SOFTWARE.
+  </copyright>
+
+  <description summary="Show layer surfaces in locked session.">
+    Normally, `ext-session-lock-v1` hides all other surfaces. This protocol allows
+    a client to indicate that a layer surface should also be shown when the session
+    is locked.
+  </description>
+
+  <interface name="cosmic_session_lock_layer_manager_v1" version="1">
+    <request name="set_show_on_lock" since="1">
+      <description
+        summary="Show layer on lock."
+      >
+      Show on lock is double buffered, see wl_surface.commit.
+      </description>
+      <arg
+        name="layer"
+        type="object"
+        interface="zwlr_layer_surface_v1"
+        summary="the layer surface"
+      />
+    </request>
+
+    <request name="unset_show_on_lock" since="1">
+      <description
+        summary="Do not show layer on lock."
+      >
+      Show on lock is double buffered, see wl_surface.commit.
+      </description>
+      <arg
+        name="layer"
+        type="object"
+        interface="zwlr_layer_surface_v1"
+        summary="the layer surface"
+      />
+    </request>
+
+    <request name="destroy" type="destructor">
+      <description summary="destroy the global">
+      </description>
+    </request>
+  </interface>
+</protocol>
diff --git a/src/main.c b/src/main.c
index 34549c8..3c6bd84 100644
--- a/src/main.c
+++ b/src/main.c
@@ -14,6 +14,7 @@

 #include "wlr-layer-shell-unstable-v1-client-protocol.h"
 #include "wlr-foreign-toplevel-management-unstable-v1-client-protocol.h"
+#include "cosmic-session-lock-layer-v1-client-protocol.h"
 #include <wayland-client.h>
 #include <wayland-egl.h>

@@ -71,6 +72,15 @@ static EGLDisplay *egl_display;
 static EGLContext *egl_context;
 static bool egl_is_gles;

+// Opt-in lock-screen wallpaper (cosmic-comp and compatible compositors only).
+// show_on_lock_requested is read once from MPVPAPER_SHOW_ON_LOCK at startup;
+// session_lock_layer_manager is only non-NULL when the compositor actually
+// advertises cosmic_session_lock_layer_manager_v1. Both gate the
+// set_show_on_lock() call in create_layer_surface() — when either is
+// false/NULL, behavior is identical to unpatched mpvpaper.
+static bool show_on_lock_requested;
+static struct cosmic_session_lock_layer_manager_v1 *session_lock_layer_manager;
+
 static mpv_handle *mpv;
 static mpv_render_context *mpv_glcontext;
 static int wakeup_fd;
@@ -134,6 +144,12 @@ static void exit_cleanup() {
     if (egl_display)
         eglTerminate(egl_display);

+    // Best-effort, like the EGL/mpv teardown above: the process is about to
+    // exit either way, but send the destructor request while the display
+    // connection is still open rather than relying solely on disconnect.
+    if (session_lock_layer_manager)
+        cosmic_session_lock_layer_manager_v1_destroy(session_lock_layer_manager);
+
     if (wakeup_fd >= 0)
         close(wakeup_fd);
 }
@@ -1004,6 +1020,18 @@ static void create_layer_surface(struct display_output *output) {
     );
     zwlr_layer_surface_v1_set_exclusive_zone(output->layer_surface, -1);
     zwlr_layer_surface_v1_add_listener(output->layer_surface, &layer_surface_listener, output);
+
+    // Ask the compositor to keep drawing this output's layer surface while
+    // the session is locked (cosmic-comp's session_lock_layer extension; see
+    // cosmic_session_lock_layer_manager_v1 in handle_global()). Must happen
+    // before the first wl_surface_commit below: show-on-lock is double
+    // buffered on that commit, same as the anchor/exclusive-zone state just
+    // set above. A no-op — same as unpatched mpvpaper — unless the user opted
+    // in AND the compositor advertised the manager.
+    if (show_on_lock_requested && session_lock_layer_manager) {
+        cosmic_session_lock_layer_manager_v1_set_show_on_lock(session_lock_layer_manager, output->layer_surface);
+    }
+
     wl_surface_commit(output->surface);
 }

@@ -1119,6 +1147,12 @@ static void handle_global(void *data, struct wl_registry *registry, uint32_t nam

     } else if (strcmp(interface, zwlr_layer_shell_v1_interface.name) == 0) {
         state->layer_shell = wl_registry_bind(registry, name, &zwlr_layer_shell_v1_interface, 1);
+    } else if (show_on_lock_requested && strcmp(interface, cosmic_session_lock_layer_manager_v1_interface.name) == 0) {
+        // Only bind when actually requested: an unused binding on compositors
+        // that don't support (or don't need) show-on-lock is harmless, but
+        // skipping it keeps the "opt-in changes nothing when off" guarantee
+        // obvious from this handler alone, without reading create_layer_surface().
+        session_lock_layer_manager = wl_registry_bind(registry, name, &cosmic_session_lock_layer_manager_v1_interface, 1);
     }
     if (halt_info.auto_pause > 1 || halt_info.auto_stop > 1) {
         if (strcmp(interface, zwlr_foreign_toplevel_manager_v1_interface.name) == 0) {
@@ -1450,6 +1484,12 @@ int main(int argc, char **argv) {

     check_paper_processes();

+    // Opt-in, read once before the registry is even created: see the
+    // show_on_lock_requested declaration near the top of this file for what
+    // it gates. Anything other than unset/empty/"0" enables it (e.g. "1").
+    const char *show_on_lock_env = getenv("MPVPAPER_SHOW_ON_LOCK");
+    show_on_lock_requested = show_on_lock_env && *show_on_lock_env && strcmp(show_on_lock_env, "0") != 0;
+
     struct wl_state state = {0};
     wl_list_init(&state.outputs);
     wl_list_init(&state.toplevel_handles);
@@ -1498,6 +1538,14 @@ int main(int argc, char **argv) {
         cflp_error("Missing a required Wayland interface");
         return EXIT_FAILURE;
     }
+    if (show_on_lock_requested && !session_lock_layer_manager) {
+        // cflp_* helpers (cflogprinter.c) all print to stdout regardless of
+        // name (cflp_error/cflp_warning included) — this diagnostic is
+        // required on stderr, so write it directly instead.
+        fprintf(stderr, "mpvpaper: MPVPAPER_SHOW_ON_LOCK was requested, but this compositor does not "
+                         "advertise cosmic_session_lock_layer_manager_v1 - the wallpaper will not be "
+                         "shown on the lock screen\n");
+    }

     // Check outputs
     wl_display_roundtrip(state.display);
MPVPAPER_SHOW_ON_LOCK_PATCH_EOF
  (cd "$BUILD_DIR/mpvpaper" && git apply --verbose "$MPVPAPER_SHOW_ON_LOCK_PATCH") || { echo "mpvpaper show-on-lock patch failed to apply against $MPVPAPER_VERSION"; exit 1; }
  (cd "$BUILD_DIR/mpvpaper" && meson setup build >/dev/null && meson compile -C build >/dev/null)
  sudo install -m 755 "$BUILD_DIR/mpvpaper/build/mpvpaper" /usr/lib/fresco/mpvpaper
  rm -rf "$BUILD_DIR"
  if renderer_ok; then
    ok "Renderer rebuilt (mpvpaper $MPVPAPER_VERSION) against this system's libmpv"
    # Restart the daemon so it picks up the fixed renderer right away.
    if pkill -x frescod 2>/dev/null; then
      (setsid frescod >/dev/null 2>&1 &) || true
    fi
  else
    warn "Renderer still isn't usable — run 'fresco doctor' and report the output at https://github.com/${REPO}/issues"
    warn "Workaround: install mpvpaper yourself and set FRESCO_MPVPAPER=/path/to/mpvpaper"
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

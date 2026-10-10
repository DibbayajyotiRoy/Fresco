/*
 * Fresco lock-screen wallpaper -- a Plasma 6 "Plasma/Wallpaper" package.
 * Copyright (C) 2026 Dibbayajyoti Roy
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * This file is loaded by two different hosts:
 *
 *   1. kscreenlocker_greet -- the real lock screen. It is a separate process
 *      (spawned fresh by ksldapp.cpp's KSldApp::lock() every time the screen
 *      locks) whose QQmlEngine has its QQmlNetworkAccessManagerFactory
 *      replaced with one that answers every non-local request with
 *      ContentAccessDenied (kscreenlocker's greeter/noaccessnetworkaccess-
 *      managerfactory.cpp, wired up in greeter/greeterapp.cpp). There is no
 *      OS-level seccomp filter in current kscreenlocker -- the "no network"
 *      rule is enforced at the Qt/QML layer instead -- but the practical
 *      effect is the same, so this file must never touch the network or XHR.
 *      Local file:// reads (our video, still image, and widget-layer PNGs)
 *      are unaffected: KProtocolInfo classifies "file" as a local protocol,
 *      which the factory always lets through.
 *   2. plasmashell, as the *desktop* wallpaper. plasmashell paints the
 *      desktop (icons included) into one opaque window, so a Fresco window
 *      can never show the video there; instead Fresco's daemon selects this
 *      plugin on every desktop through plasmashell's scripting DBus API and
 *      writes VideoPath/StillPath/PlayVideo/PauseMode (src/daemon/
 *      kde_desktop.rs, issue #44). The plugin only ever reads local files
 *      Fresco already wrote, so almost nothing differs between the two; the
 *      one exception is the desktop-only auto-pause (WindowWatcher.qml).
 *
 * All configuration comes from Fresco's own daemon, which writes plain
 * KConfigXT values into kscreenlockerrc (see ../config/main.xml and
 * packaging/kde/README.md for the exact keys) and continuously rewrites the
 * PNGs under LayerDir while the screen is locked.
 */

import QtQuick
import QtQuick.Window // for the Screen attached property (Screen.name, used to pick the per-output layer PNG)
import org.kde.plasma.plasmoid
// Deliberately no "import QtMultimedia" here -- see the Loader below and
// VideoLayer.qml's header for why that import lives in its own file.

WallpaperItem {
    id: root

    // ---------------------------------------------------------------
    // Media: video (looping, muted) with a still-image fallback, with a
    // solid colour under both so there is never a gap.
    // ---------------------------------------------------------------

    readonly property bool haveVideo: root.configuration.PlayVideo && root.configuration.VideoPath !== ""
    readonly property bool haveStill: root.configuration.StillPath !== ""
    // True if video is unusable for *either* reason the spec cares about:
    // - Loader.Error: VideoLayer.qml's "import QtMultimedia" failed to
    //   resolve (module not installed). Caught here, not as a load failure
    //   of this whole file -- see the Loader below.
    // - item.hasError: the module is fine but the file itself failed to
    //   play (bad codec, corrupt file, etc), surfaced from VideoLayer.qml.
    // "Empty path" is already covered by haveVideo above.
    readonly property bool videoFailed: haveVideo &&
        (videoLoader.status === Loader.Error ||
         (videoLoader.item !== null && videoLoader.item.hasError))

    // Bottom-most and always present. If both VideoPath and StillPath are
    // unset (or the video fails and there is no still to fall back to),
    // this is the only thing on screen -- a plain dark background rather
    // than a blank/undefined one.
    Rectangle {
        anchors.fill: parent
        color: "#0b0b10"
    }

    Image {
        id: stillImage
        anchors.fill: parent
        // Shown whenever video isn't the thing on screen right now: either
        // there is no (usable) video at all, or it errored out.
        visible: (!root.haveVideo || root.videoFailed) && root.haveStill && status === Image.Ready
        source: root.haveStill ? Qt.resolvedUrl(root.configuration.StillPath) : ""
        fillMode: Image.PreserveAspectCrop
        asynchronous: true // decode off the UI thread; the greeter must stay responsive to keypresses
        // The still is a large, mostly-static photo scaled down to fit the
        // output. smooth+mipmap avoid the moire/aliasing "smoothing
        // artefacts" that nearest-neighbour minification would otherwise
        // leave in fine detail (foliage, text, etc).
        smooth: true
        mipmap: true
        cache: false // StillPath can be repointed by Fresco; never hold on to a stale decode
    }

    Loader {
        id: videoLoader
        anchors.fill: parent
        // Loaded by *source* URL, not as an inline Component: QML resolves
        // a document's imports when that document is compiled. An inline
        // Component here would still be part of *this* file's compile unit,
        // so "import QtMultimedia" would have to live at the top of
        // main.qml -- and on a system without that module installed, this
        // entire file (still image, dim veil, widget layer included) would
        // fail to load. Pointing a Loader at a separate file makes
        // VideoLayer.qml its own document with its own import resolution:
        // if it fails, only this Loader's `status` becomes Loader.Error and
        // `item` stays null, and everything else below keeps working.
        source: root.haveVideo ? "VideoLayer.qml" : ""
        visible: status === Loader.Ready && !root.videoFailed
    }

    // Loader.source (unlike Loader.sourceComponent) has no inline syntax
    // for passing properties into the loaded item, so the video path is
    // pushed in with a Binding instead. Guarded by `when` so it only
    // applies once VideoLayer.qml has actually finished loading -- setting
    // a property on a still-null `item` would otherwise be a silent no-op
    // the first time this becomes true.
    Binding {
        target: videoLoader.item
        property: "videoSource"
        value: Qt.resolvedUrl(root.configuration.VideoPath)
        when: videoLoader.status === Loader.Ready && videoLoader.item !== null
    }

    // ---------------------------------------------------------------
    // Auto-pause (desktop only): freeze the video while a fullscreen -- or,
    // with PauseMode 1, a maximized -- window covers the wallpaper anyway.
    // ---------------------------------------------------------------

    // The greeter's window is plasma-desktop's LockScreen.qml; the desktop's
    // is not. The lock screen has no windows to watch (and a fullscreen app
    // behind it must not freeze the clock-over-video), so skip it there.
    // windowKnown keeps the watcher from loading before this is decided.
    property bool windowKnown: false
    property bool lockScreenMode: false
    Item {
        onWindowChanged: window => {
            if (!window) {
                return;
            }
            root.lockScreenMode = "source" in window && window.source.toString().endsWith("LockScreen.qml");
            root.windowKnown = true;
        }
    }

    // Loaded by source for the same reason as videoLoader above: if
    // org.kde.taskmanager is unavailable, status becomes Loader.Error, item
    // stays null, and `windowPaused` stays false -- the video just never
    // auto-pauses.
    Loader {
        id: watcherLoader
        active: root.haveVideo && root.windowKnown && !root.lockScreenMode && root.configuration.PauseMode !== 2
        source: "WindowWatcher.qml"
    }

    Binding {
        target: watcherLoader.item
        property: "screenGeometry"
        value: (root.parent && root.parent.screenGeometry) ? root.parent.screenGeometry : Qt.rect(0, 0, 0, 0)
        when: watcherLoader.item !== null
    }

    readonly property bool windowPaused: watcherLoader.item !== null &&
        (watcherLoader.item.fullscreen || (root.configuration.PauseMode === 1 && watcherLoader.item.maximized))

    Binding {
        target: videoLoader.item
        property: "paused"
        value: root.windowPaused
        when: videoLoader.status === Loader.Ready && videoLoader.item !== null
    }

    // ---------------------------------------------------------------
    // Dim veil -- a black rectangle between the media and the widget layer,
    // so Fresco's clock/text stays legible over a bright video or photo.
    // ---------------------------------------------------------------

    Rectangle {
        anchors.fill: parent
        color: "black"
        // main.xml already constrains Dim to [0, 0.8], but config files are
        // hand-editable, so clamp again rather than trust it blindly.
        opacity: Math.max(0, Math.min(root.configuration.Dim, 0.8))
    }

    // ---------------------------------------------------------------
    // Widget layer: Fresco's clock/greeting/now-playing/album-art/battery
    // widgets, pre-rendered by the daemon into a transparent PNG per output
    // (LayerDir/layer-<connector>.png, plus a connector-less layer.png
    // fallback), rewritten roughly once a second while the screen is locked.
    // ---------------------------------------------------------------

    Item {
        id: widgetLayer
        anchors.fill: parent

        // Bumped on every tick and appended as a query string below. This is
        // what actually forces a reload: Fresco keeps the filename stable
        // and rewrites it atomically (write, then rename), so the path never
        // changes on its own, and QML's Image only re-fetches when its
        // `source` *string* changes. cache:false on top of that stops QML
        // from ever serving a remembered pixmap for a URL it thinks it has
        // already seen.
        property int generation: 0
        // Which of layerA/layerB is the one currently on screen. The other
        // one is always the target of the *next* load attempt, so a
        // still-loading or failed fetch is never visible -- the on-screen
        // image only ever changes on a confirmed Image.Ready, which is the
        // whole no-flicker trick.
        property int visibleBuffer: 0
        // "connector" is tried first each tick; a load error there retries
        // once as "fallback" (layer.png) on the same hidden buffer before
        // giving up for that tick.
        property string stage: "connector"

        readonly property string connectorPath: root.configuration.LayerDir + "/layer-" + Screen.name + ".png"
        readonly property string fallbackPath: root.configuration.LayerDir + "/layer.png"

        function hiddenBuffer() {
            return visibleBuffer === 0 ? layerB : layerA;
        }

        function pathForStage(stageName) {
            return stageName === "connector" ? connectorPath : fallbackPath;
        }

        function loadIntoHidden(stageName) {
            const img = hiddenBuffer();
            widgetLayer.stage = stageName;
            // Qt.resolvedUrl() turns the absolute local path into a proper
            // file:// URL (percent-encoding included); the ?v= query is then
            // just appended so the string differs from last time. Qt ignores
            // an unknown query component when resolving a file:// URL back
            // to a local path, so this does not affect *which* file loads.
            img.source = Qt.resolvedUrl(pathForStage(stageName)) + "?v=" + widgetLayer.generation;
        }

        function startTick() {
            if (root.configuration.LayerDir === "") {
                return; // nothing configured yet -- leave things as they are
            }
            widgetLayer.generation += 1;
            loadIntoHidden("connector");
        }

        // Shared handler for both buffers below. Only ever acts on the
        // buffer that is currently hidden (i.e. mid-fetch); a status change
        // on the visible buffer is not something we triggered and is
        // ignored.
        function handleStatusChanged(img) {
            if (img !== hiddenBuffer()) {
                return;
            }
            if (img.status === Image.Ready) {
                visibleBuffer = (visibleBuffer === 0) ? 1 : 0;
            } else if (img.status === Image.Error && widgetLayer.stage === "connector") {
                // Per-connector file missing/unreadable: retry once with the
                // connector-less fallback before giving up for this tick.
                loadIntoHidden("fallback");
            }
            // Any other error (fallback also missing), or Image.Loading/
            // Null: do nothing. The previously-visible buffer keeps showing
            // its last good frame (or nothing, if there has never been one)
            // -- never a broken-image icon.
        }

        Image {
            id: layerA
            anchors.fill: parent
            visible: widgetLayer.visibleBuffer === 0 && status === Image.Ready
            fillMode: Image.Stretch // Fresco already renders this at the output's native size
            asynchronous: true
            cache: false
            onStatusChanged: widgetLayer.handleStatusChanged(layerA)
        }

        Image {
            id: layerB
            anchors.fill: parent
            visible: widgetLayer.visibleBuffer === 1 && status === Image.Ready
            fillMode: Image.Stretch
            asynchronous: true
            cache: false
            onStatusChanged: widgetLayer.handleStatusChanged(layerB)
        }

        Timer {
            // A 0ms/negative RefreshMs would otherwise busy-loop; 200ms is a
            // conservative floor, well under the documented 1000ms default.
            interval: Math.max(200, root.configuration.RefreshMs)
            repeat: true
            triggeredOnStart: true
            // Don't decode PNGs on every tick when nothing is showing this
            // item anyway (e.g. the view is off-screen).
            running: root.visible
            onTriggered: widgetLayer.startTick()
        }
    }
}

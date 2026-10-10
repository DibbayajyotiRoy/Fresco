/*
 * Fresco lock-screen wallpaper -- video layer.
 * Copyright (C) 2026 Dibbayajyoti Roy
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * This is the ONLY file in this package that imports QtMultimedia, and that
 * is deliberate. QML resolves a document's imports when that document is
 * compiled/loaded; if "import QtMultimedia" lived in main.qml itself, a
 * system without the Qt6 Multimedia QML module installed would fail to load
 * the *entire* wallpaper -- still image and widget layer included, not just
 * the video. main.qml instead loads this file by URL through a Loader
 * (contents/ui/main.qml, the videoLoader item), which is Qt's documented
 * mechanism for isolating exactly this kind of failure: if this file's
 * imports can't resolve, the Loader's `status` becomes Loader.Error and
 * `item` stays null, and main.qml falls back to StillPath. See
 * packaging/kde/README.md for the confirmed package names
 * (qml6-module-qtmultimedia on Debian/Ubuntu, qt6-multimedia on Arch,
 * qt6-qtmultimedia on Fedora) and why this is a *recommended*, not hard,
 * dependency of the overall package.
 */
import QtQuick
import QtMultimedia

Item {
    id: videoLayer

    // Set from main.qml via a Binding -- Loader.source (as opposed to
    // Loader.sourceComponent) has no syntax for passing initial/live
    // property values into the loaded item, so the caller binds this
    // property from the outside once the Loader reports Ready.
    property url videoSource

    // Set from main.qml (same Binding mechanism) while a fullscreen/maximized
    // window hides the wallpaper; frees the GPU on iGPUs.
    property bool paused: false
    onPausedChanged: paused ? player.pause() : player.play()

    // Surfaced to main.qml so it can fall back to StillPath on a playback
    // error (bad codec, corrupt file, etc). This is a *different* failure
    // mode from "QtMultimedia isn't installed" -- that one never gets this
    // far, since this whole file fails to load first; main.qml tells the
    // two apart via Loader.status vs. this property.
    readonly property bool hasError: player.error !== MediaPlayer.NoError

    VideoOutput {
        id: videoOutput
        anchors.fill: parent
        fillMode: VideoOutput.PreserveAspectCrop
    }

    AudioOutput {
        id: audioOutput
        // Belt-and-braces: a lock screen must never make sound, regardless
        // of what the source video contains.
        muted: true
        volume: 0
    }

    MediaPlayer {
        id: player
        source: videoLayer.videoSource
        videoOutput: videoOutput
        audioOutput: audioOutput
        // Also keeps a source change from restarting playback mid-pause.
        autoPlay: !videoLayer.paused
        loops: MediaPlayer.Infinite
    }
}

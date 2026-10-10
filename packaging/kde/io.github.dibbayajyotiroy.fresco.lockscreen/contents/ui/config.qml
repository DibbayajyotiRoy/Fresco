/*
 * Fresco lock-screen wallpaper -- config page.
 * Copyright (C) 2026 Dibbayajyoti Roy
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Shown by System Settings > Screen Locking > Appearance and by the desktop
 * wallpaper settings when this wallpaper is selected. It deliberately has no
 * controls: Fresco's own app is the only writer of VideoPath, StillPath,
 * PlayVideo, Dim, LayerDir and RefreshMs (see ../config/main.xml), so a
 * second settings UI here would just be an easily-stale duplicate.
 *
 * This is a fully supported shape, not a workaround: kscreenlocker's own
 * WallpaperConfig.qml loader (kcm/ui/WallpaperConfig.qml) only wires up a
 * `cfg_<Key>` property for a KConfigXT key if this file declares one, so a
 * page that declares none simply gets no bindings -- see
 * packaging/kde/README.md for how this was confirmed against source.
 */
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: root

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        type: Kirigami.MessageType.Information
        visible: true
        text: i18n("This wallpaper is set by Fresco. Open Fresco to change the video or image.")
    }

    // Push the message to the top instead of letting the layout stretch it.
    Item {
        Layout.fillHeight: true
    }
}

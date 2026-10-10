/*
 * Fresco desktop wallpaper -- window watcher.
 * Copyright (C) 2026 Dibbayajyoti Roy
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Reports whether a window on this screen (current virtual desktop and
 * activity, not minimized) is fullscreen or maximized, so main.qml can pause
 * the video while the wallpaper is hidden anyway. KWin does not expose
 * wlr-foreign-toplevel, so Fresco's daemon cannot see windows on Plasma;
 * plasmashell's own libtaskmanager can.
 *
 * Like VideoLayer.qml this is a separate file that main.qml loads through a
 * Loader: org.kde.taskmanager may be missing in the lock-screen greeter, and
 * a failed import must only disable auto-pause, not the whole wallpaper.
 */
import QtQuick
import org.kde.taskmanager as TaskManager

Item {
    id: watcher

    // Bound from main.qml (Loader.source cannot pass properties in).
    property rect screenGeometry

    property bool fullscreen: false
    property bool maximized: false

    function scan() {
        let fs = false;
        let mx = false;
        for (let i = 0; i < tasks.count; i++) {
            const task = tasks.index(i, 0);
            fs = fs || tasks.data(task, TaskManager.AbstractTasksModel.IsFullScreen) === true;
            mx = mx || tasks.data(task, TaskManager.AbstractTasksModel.IsMaximized) === true;
        }
        fullscreen = fs;
        maximized = mx;
    }

    TaskManager.VirtualDesktopInfo {
        id: virtualDesktopInfo
    }

    TaskManager.ActivityInfo {
        id: activityInfo
    }

    TaskManager.TasksModel {
        id: tasks
        screenGeometry: watcher.screenGeometry
        // An empty rect means main.qml could not tell which screen this is;
        // watch every screen rather than none.
        filterByScreen: watcher.screenGeometry.width > 0
        activity: activityInfo.currentActivity
        filterByActivity: true
        filterMinimized: true
        groupMode: TaskManager.TasksModel.GroupDisabled
        onCountChanged: Qt.callLater(watcher.scan)
        onDataChanged: Qt.callLater(watcher.scan)
        Component.onCompleted: {
            // Plasma 6.7 made virtual desktops per output; older versions
            // only have the global current desktop.
            if (tasks.hasOwnProperty("filterByCurrentVirtualDesktop")) {
                tasks.filterByCurrentVirtualDesktop = true;
            } else {
                tasks.virtualDesktop = Qt.binding(() => virtualDesktopInfo.currentDesktop);
                tasks.filterByVirtualDesktop = true;
            }
            watcher.scan();
        }
    }
}

#!/bin/sh
# Fresco DDE diagnostic — run on Deepin 25 (X11) with a video wallpaper set.
# Collects the facts needed to debug issue #2 (video not visible under DDE).
# 诊断脚本：请在已通过 Fresco 设置视频壁纸的 Deepin 25 (X11) 会话中运行。
# Usage: sh dde-diag.sh   (no root needed; output is safe to paste into GitHub)

echo "=== Fresco DDE diagnostic / Fresco DDE 诊断 ==="
echo "date: $(date -Is)"
echo

echo "--- session ---"
echo "XDG_CURRENT_DESKTOP=$XDG_CURRENT_DESKTOP"
echo "XDG_SESSION_TYPE=$XDG_SESSION_TYPE"
echo

find_win() { # $1 = grep pattern over wmctrl -lx
    wmctrl -lx 2>/dev/null | grep -i "$1" | head -1 | awk '{print $1}'
}

FRESCO=$(find_win "fresco-wallpaper")
DDE=$(find_win "dde-shell/desktop\|dde-desktop\|org.deepin")
echo "--- windows ---"
echo "fresco window: ${FRESCO:-NOT FOUND}"
echo "dde desktop window: ${DDE:-NOT FOUND}"
echo

echo "--- compositing ---"
# KWin compositing must be active for any transparency to work at all.
qdbus org.kde.KWin /Compositor org.kde.kwin.Compositing.active 2>/dev/null \
    || dbus-send --session --print-reply --dest=org.kde.KWin /Compositor \
        org.freedesktop.DBus.Properties.Get string:org.kde.kwin.Compositing string:active 2>/dev/null \
    || echo "could not query KWin compositing state"
xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null
echo

echo "--- visual depth (32 = ARGB, transparency possible) ---"
for w in "$FRESCO" "$DDE"; do
    [ -n "$w" ] && xwininfo -id "$w" | grep -E "xwininfo|Depth|Visual|Map State"
done
echo

echo "--- stacking (bottom→top, first 8) ---"
xprop -root _NET_CLIENT_LIST_STACKING | tr ',' '\n' | head -8
echo

echo "--- current DDE wallpaper URI per monitor ---"
for m in $(xrandr --query 2>/dev/null | awk '/ connected/{print $1}'); do
    printf "%s: " "$m"
    gdbus call --session --dest org.deepin.dde.Appearance1 \
        --object-path /org/deepin/dde/Appearance1 \
        --method org.deepin.dde.Appearance1.GetCurrentWorkspaceBackgroundForMonitor "$m" 2>&1
done
echo

echo "--- is mpv actually rendering? (two snapshots of the Fresco window, 2s apart) ---"
# If the two checksums DIFFER, video frames are being drawn and the problem is
# purely compositing/stacking. If they are IDENTICAL (or all-black), Fresco's
# renderer is the problem, not DDE.
# 如果两个校验和不同，说明视频正在渲染，问题只在窗口堆叠/合成；
# 如果相同，说明是渲染问题而不是 DDE 遮挡问题。
if [ -n "$FRESCO" ] && command -v xwd >/dev/null; then
    xwd -id "$FRESCO" -silent | cksum
    sleep 2
    xwd -id "$FRESCO" -silent | cksum
else
    echo "xwd or fresco window unavailable — skipped"
fi
echo

echo "--- raise test / 置顶测试 ---"
# Temporarily raise the Fresco window to the top for 5 seconds.
# WATCH THE SCREEN: does the video appear (over everything)?
# 临时将 Fresco 窗口置顶 5 秒。请观察屏幕：视频是否出现（覆盖一切）？
# It restores itself afterwards. Please report YES or NO.
if [ -n "$FRESCO" ] && command -v wmctrl >/dev/null; then
    echo "raising fresco window for 5s — WATCH THE SCREEN NOW / 请立即观察屏幕"
    wmctrl -i -r "$FRESCO" -b add,above
    sleep 5
    wmctrl -i -r "$FRESCO" -b remove,above
    wmctrl -i -r "$FRESCO" -b add,below
    echo "restored. Was the video visible during those 5 seconds? YES/NO"
    echo "已恢复。刚才 5 秒内是否看到了视频？请回答 是/否"
else
    echo "wmctrl not installed — run: sudo apt install wmctrl && re-run this script"
fi

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
echo

# ─────────────────────────────────────────────────────────────────────────────
# Z-ORDER PROBE — added for the desktop-icon visibility problem.
#
# The question everything hinges on: does dde-shell paint the wallpaper AND the
# desktop icons into ONE window, or are the icons a separate child/sibling
# window?
#
#   * One window  -> no stacking order can put Fresco between them. Fresco must
#                    be above (icons hidden, today's behaviour) or below
#                    (wallpaper invisible). A dde-shell plugin would be the only
#                    real fix, exactly like the GNOME/Mutter situation.
#   * Two windows -> Fresco can sit between them, and this is fixable.
#
# Clicks still reaching the icons does NOT distinguish these: Fresco sets an
# empty input shape, so clicks pass through it either way.
# 点击仍然有效并不能区分这两种情况：Fresco 的输入区域为空，点击本来就会穿透。
# ─────────────────────────────────────────────────────────────────────────────

echo "=== Z-ORDER PROBE / 层级探测 ==="
echo

echo "--- every window declaring _NET_WM_WINDOW_TYPE_DESKTOP ---"
# More than one is expected (dde-shell's and ours). Note the ORDER: this list is
# bottom-most first.
for id in $(xprop -root _NET_CLIENT_LIST_STACKING 2>/dev/null \
            | sed 's/.*# //' | tr -d ' ' | tr ',' '\n'); do
    [ -n "$id" ] || continue
    type=$(xprop -id "$id" _NET_WM_WINDOW_TYPE 2>/dev/null)
    case "$type" in
        *DESKTOP*)
            echo "  id=$id"
            xprop -id "$id" WM_CLASS _NET_WM_NAME _NET_WM_WINDOW_TYPE _NET_WM_STATE 2>/dev/null \
                | sed 's/^/    /'
            xwininfo -id "$id" 2>/dev/null \
                | grep -E "Absolute|Width|Height|Depth|Visual Class|Override Redirect|Map State" \
                | sed 's/^/    /'
            ;;
    esac
done
echo

echo "--- FULL stacking order, bottom→top, with class (the decisive list) ---"
n=0
for id in $(xprop -root _NET_CLIENT_LIST_STACKING 2>/dev/null \
            | sed 's/.*# //' | tr -d ' ' | tr ',' '\n'); do
    [ -n "$id" ] || continue
    n=$((n + 1))
    cls=$(xprop -id "$id" WM_CLASS 2>/dev/null | sed 's/WM_CLASS(STRING) = //')
    printf "  %2d  %-12s %s\n" "$n" "$id" "$cls"
done
echo

echo "--- window TREE under the DDE desktop window (H4 decider) ---"
# If this shows CHILD windows sized like the screen, the icons may live in their
# own window and Fresco could be stacked between them. If dde-shell's desktop
# window has NO children, icons are painted into the same surface as the
# wallpaper and stacking cannot solve this.
# 若下面显示有子窗口，图标可能在独立窗口中；若没有子窗口，则图标与壁纸同属一个
# 窗口，调整层级无法解决。
if [ -n "$DDE" ]; then
    xwininfo -id "$DDE" -children 2>/dev/null | sed 's/^/  /'
else
    echo "  dde desktop window NOT FOUND — cannot probe"
fi
echo

echo "--- override-redirect windows the WM does not manage ---"
# These never appear in _NET_CLIENT_LIST_STACKING. If the icon layer is here,
# EWMH hints cannot affect it at all.
xwininfo -root -tree 2>/dev/null \
    | grep -iE "desktop|icon|deepin|dde" \
    | head -30 \
    | sed 's/^/  /'
echo

echo "--- TRANSPARENCY RE-TEST (please run and report) ---"
# The earlier conclusion "a transparent DDE wallpaper composites onto black" was
# reached by putting a red ROOT window underneath. Under a compositing WM the
# root window is generally NOT shown — the compositor paints over it — so that
# test may have proved the wrong thing. This re-tests it with a REAL window
# (Fresco's own), which is the case that actually matters.
# 之前的结论用的是根窗口，在合成器下根窗口本来就不显示，该结论可能不成立。
# 这里用真实窗口重新验证。
cat <<'PROBE'
  Run these two commands, and report whether the VIDEO is visible each time
  (ignore the icons for a moment — we only want to know if the video shows):

    1) systemctl --user stop fresco 2>/dev/null; pkill -x frescod
       FRESCO_DDE_MODE=transparent frescod

       -> Video visible?  YES / NO
       -> Desktop icons visible?  YES / NO

    2) pkill -x frescod
       FRESCO_DDE_MODE=restack frescod

       -> Video visible?  YES / NO
       -> Desktop icons visible?  YES / NO

  Expected from the current code: (1) video hidden, icons visible.
                                  (2) video visible, icons hidden.
  If (1) shows the VIDEO as well as the icons, the transparency path works
  after all and this is fixed by defaulting to it on Deepin 25.
  如果第 (1) 项视频和图标都可见，说明透明方案其实可行。
PROBE

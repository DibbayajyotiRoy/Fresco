#!/bin/sh
# Fresco Deepin launcher diagnostic — why no icon in dde-launchpad after install.
# 诊断脚本：安装后启动器中看不到 Fresco 图标。
#
# READ-ONLY. It changes nothing: no files are written, no services restarted,
# no packages touched. Output is safe to paste into a forum post or an issue.
# 本脚本只读取信息，不修改任何文件、不重启任何服务，输出可直接粘贴。
#
# Usage / 用法:
#   sh dde-launcher-diag.sh            # run it three times:
#   sh dde-launcher-diag.sh            #   1. BEFORE installing the .deb
#   sh dde-launcher-diag.sh            #   2. ~20s AFTER installing
#                                      #   3. after `killall dde-shell`
#   请运行三次：安装前 / 安装后约 20 秒 / 执行 killall dde-shell 之后。
#
# What it is trying to tell apart (docs/plan-dde-launcher-and-integration.md §1.2):
#   H1  stale item-arrangement.ini entry surviving the previous uninstall
#   H2  stale per-user ApplicationManager / appstore-daemon record
#   H3  the postinst re-announce never runs (detached child reaped)
#   H4  something else about "fresh system"

APP=io.github.dibbayajyotiroy.Fresco
DESKTOP=/usr/share/applications/$APP.desktop
LAUNCHPAD="$HOME/.config/deepin/dde-launchpad"
ARRANGEMENT="$LAUNCHPAD/item-arrangement.ini"

echo "=== Fresco Deepin launcher diagnostic / Fresco 启动器诊断 ==="
echo "date: $(date -Is)"
echo "XDG_CURRENT_DESKTOP=$XDG_CURRENT_DESKTOP  XDG_SESSION_TYPE=$XDG_SESSION_TYPE"
[ -r /etc/os-release ] && . /etc/os-release && echo "os: $PRETTY_NAME"
echo

# ─── 1. item-arrangement.ini — the H1 experiment ─────────────────────────────
# THE decisive observation: Fresco PRESENT in this file while ABSENT from the
# launcher UI means the launcher already has a decision about us on file from a
# previous install, and a fresh system has none. That confirms H1 outright.
# 关键判据：本文件中有 Fresco，但启动器界面里看不到 —— 即为 H1（残留状态）。
echo "--- 1. item-arrangement.ini (H1) ---"
if [ -f "$ARRANGEMENT" ]; then
    echo "path:  $ARRANGEMENT"
    echo "mtime: $(date -Is -r "$ARRANGEMENT" 2>/dev/null)"
    echo "fresco lines:"
    grep -in fresco "$ARRANGEMENT" | sed 's/^/    /' || echo "    (none — Fresco is NOT listed)"
    echo "full contents:"
    sed 's/^/    /' "$ARRANGEMENT"
else
    echo "  NOT PRESENT — $ARRANGEMENT"
    echo "  (on a never-launched launcher this is normal)"
fi
echo

# ─── 2. every other per-user record mentioning us, with mtimes ───────────────
# A record older than this install is residue by definition.
# 时间戳早于本次安装的记录，按定义就是残留。
echo "--- 2. other per-user records mentioning Fresco (H1/H2) ---"
for dir in "$LAUNCHPAD" "$HOME/.config/deepin" "$HOME/.local/share/deepin" \
           "$HOME/.cache/deepin" "$HOME/.local/share/applications"; do
    [ -d "$dir" ] || continue
    grep -ril "fresco" "$dir" 2>/dev/null | while read -r f; do
        printf "  %s  %s\n" "$(date -Is -r "$f" 2>/dev/null)" "$f"
    done
done
echo "  (nothing listed above = no per-user residue found)"
echo

# ─── 3. does the ApplicationManager know about us? ───────────────────────────
# If AM lists us while the launcher does not show us, the fault is in the
# launcher's own view, not in the package.
# 若 AM 能列出我们而启动器不显示，问题在启动器一侧，而不在软件包。
echo "--- 3. org.desktopspec.ApplicationManager1 (H2) ---"
if command -v gdbus >/dev/null 2>&1; then
    gdbus call --session -d org.desktopspec.ApplicationManager1 \
        -o /org/desktopspec/ApplicationManager1 \
        -m org.freedesktop.DBus.Properties.Get \
        org.desktopspec.ApplicationManager1 List 2>&1 \
        | tr ',' '\n' | grep -i "fresco" | sed 's/^/  listed: /' \
        || echo "  Fresco NOT listed by ApplicationManager1"
    echo "  methods it exposes (looking for a reload/refresh we could call):"
    gdbus introspect --session -d org.desktopspec.ApplicationManager1 \
        -o /org/desktopspec/ApplicationManager1 2>&1 \
        | grep -iE "method|ReloadApplications|Refresh|Update" | sed 's/^/    /'
else
    echo "  gdbus not available — skipped"
fi
echo "  GAppInfo view (what a standard desktop would see):"
if command -v gio >/dev/null 2>&1; then
    gio info "$DESKTOP" >/dev/null 2>&1 && echo "    $DESKTOP readable by gio"
    grep -l "$APP" /usr/share/applications/mimeinfo.cache 2>/dev/null | sed 's/^/    in /'
fi
echo

# ─── 4. did the postinst re-announce actually fire? — settles H3 on its own ──
# packaging/debian/postinst rewrites the .desktop file ~8s after configure, from
# a detached child. If the .desktop mtime is NOT ~8s after the install, that
# child was reaped and H3 is confirmed — a packaging bug, not user residue.
# 若 .desktop 的时间戳并不比安装时间晚约 8 秒，说明 postinst 的后台子进程被回收，
# 即 H3 成立 —— 这是打包问题，而非用户残留。
echo "--- 4. postinst re-announce (H3) ---"
if [ -f "$DESKTOP" ]; then
    echo "  desktop mtime:  $(date -Is -r "$DESKTOP" 2>/dev/null)"
else
    echo "  $DESKTOP MISSING — the package did not install it"
fi
echo "  dpkg 'fresco' install/configure times, most recent last:"
grep -h "fresco" /var/log/dpkg.log /var/log/dpkg.log.1 2>/dev/null \
    | grep -E "status installed|configure" | tail -5 | sed 's/^/    /'
echo "  stranded re-announce temp file (its presence alone proves H3):"
ls -la /usr/share/applications/.$APP.desktop.renotify 2>/dev/null \
    | sed 's/^/    /' || echo "    none (good)"
echo
echo "  >>> Compare the two timestamps above. The desktop mtime should be"
echo "      about 8 seconds LATER than the configure line. If it is not,"
echo "      the re-announce never ran."
echo "  >>> 请对比上面两个时间：.desktop 应比 configure 晚约 8 秒。"
echo

# ─── 5. what the package actually installed ──────────────────────────────────
echo "--- 5. installed files, icons, DCI ---"
dpkg -L fresco 2>/dev/null | grep -E "applications|icons|dsg" | sed 's/^/  /' \
    || echo "  fresco not installed via dpkg"
echo "  generated DCI icon (deepin-desktop-theme's trigger owns this):"
ls -la /usr/share/dsg/icons/*/apps/*Fresco* 2>/dev/null | sed 's/^/    /' \
    || echo "    none found"
echo "  desktop file validation:"
command -v desktop-file-validate >/dev/null 2>&1 \
    && { desktop-file-validate "$DESKTOP" 2>&1 | sed 's/^/    /'; echo "    (no output above = valid)"; } \
    || echo "    desktop-file-validate not installed — skipped"
echo

# ─── 6. dpkg trigger ORDER — comparable to the 2026-07-26 log ────────────────
# The trigger-order theory: deepin-home-appstore-daemon (which tells the
# launcher to hot-refresh) must fire AFTER desktop-file-utils and
# deepin-desktop-theme, or the launcher refreshes against an incomplete state.
# 触发器顺序：appstore-daemon 必须晚于 desktop-file-utils 与 desktop-theme 触发，
# 否则启动器会在状态未就绪时刷新。
echo "--- 6. dpkg trigger order for the last transaction (H3 context) ---"
grep -h "trigproc" /var/log/dpkg.log /var/log/dpkg.log.1 2>/dev/null \
    | tail -12 | sed 's/^/  /'
echo

# ─── 7. how was it installed? — the input to H3 ─────────────────────────────
echo "--- 7. install path / 安装方式 ---"
echo "  most recent apt/dpkg invocations mentioning fresco:"
grep -h "fresco" /var/log/apt/history.log 2>/dev/null | tail -5 | sed 's/^/    /' \
    || echo "    nothing in apt history (installed with dpkg -i, or via the Store)"
echo "  lastore / PackageKit running? (the Store's install path):"
pgrep -a "lastore|packagekit" 2>/dev/null | sed 's/^/    /' || echo "    no"
echo

echo "=== end / 诊断结束 ==="
echo
echo "Next step if section 1 shows Fresco listed while the launcher shows nothing:"
echo "  that is H1 confirmed — remove that line, killall dde-shell, reinstall,"
echo "  and report whether the icon then appears."
echo "如果第 1 节中 item-arrangement.ini 里有 Fresco 但启动器不显示，即为 H1。"
echo "请删除该行后执行 killall dde-shell 并重装，再反馈图标是否出现。"

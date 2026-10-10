# Fresco lock-screen wallpaper (KDE Plasma 6)

This directory holds `io.github.dibbayajyotiroy.fresco.lockscreen`, a Plasma 6
`Plasma/Wallpaper` KPackage that plays Fresco's live wallpaper and draws its
widget layer (clock, greeting, now playing, album art, battery) on the real
KDE lock screen. It is a QML-only package: **Fresco never handles passwords**.
On Plasma 6 the actual lock screen is drawn by `kscreenlocker_greet`
(PAM auth, the password field) which merely hosts this package as its
background layer.

This package is complete and self-contained. It does **not** write its own
config or install itself -- that is Fresco's daemon's job. This README is the
contract between the two: the exact files, paths and config keys the daemon
must produce for this package to do anything.

## Also the desktop wallpaper (issue #44)

plasmashell draws the desktop and its icons into one opaque window, so no
window of Fresco's can show the video there. On Plasma, `frescod` therefore
selects this same package as the **desktop** wallpaper of every screen
(`src/daemon/kde_desktop.rs`): the DBus call `plasma-apply-wallpaperimage`
makes (`org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript`),
with `wallpaperPlugin` set to this package's id and `VideoPath`, `StillPath`,
`PlayVideo`, `PauseMode` and `Dim=0` written to `[Wallpaper][<id>][General]` of each
desktop. The previous plugin of each desktop is saved to
`~/.local/state/fresco/kde-desktop-saved.json` and selected again on Stop.
Plasma offers no way to hide a wallpaper package from the desktop chooser
(`WallpaperConfigModel` lists every valid `Plasma/Wallpaper`), hence the plain
name "Fresco". `FRESCO_KDE_DESKTOP=0` turns this off.

## Files in this package

```
io.github.dibbayajyotiroy.fresco.lockscreen/
  metadata.json                 # KPackage id/name/version (see spec below)
  contents/
    config/main.xml             # KConfigXT schema: VideoPath, StillPath,
                                 # PlayVideo, PauseMode, Dim, LayerDir, RefreshMs
    ui/main.qml                 # WallpaperItem: still/widgets, loads video by URL
    ui/VideoLayer.qml           # The only file that imports QtMultimedia
    ui/WindowWatcher.qml        # Desktop auto-pause (org.kde.taskmanager), loaded by URL
    ui/config.qml               # "Configured by Fresco" pointer page
```

`main.qml` never imports `QtMultimedia` itself. It loads `VideoLayer.qml`
through a `Loader` (by `source` URL, not as an inline `Component`), which is
Qt's documented mechanism for containing a failed import to just that
`Loader`: if the Qt6 Multimedia QML module isn't installed, `Loader.status`
becomes `Loader.Error` and `main.qml` falls back to `StillPath` instead of
the whole wallpaper failing to load. See "Optional runtime dependency" below.

## Install / upgrade / remove

Plasma wallpaper packages (`KPackageStructure: "Plasma/Wallpaper"`) install
under the `plasma/wallpapers/` package root, which resolves to:

- System-wide: `/usr/share/plasma/wallpapers/io.github.dibbayajyotiroy.fresco.lockscreen/`
- Per-user: `~/.local/share/plasma/wallpapers/io.github.dibbayajyotiroy.fresco.lockscreen/`

**Verified** from `libplasma`'s wallpaper package-structure plugin
(`src/plasma/packagestructure/qmlWallpaper/wallpaper.cpp`):
`package->setDefaultPackageRoot(QStringLiteral("plasma/wallpapers/"));`.

### Via `kpackagetool6` (preferred -- registers/refreshes the KPackage cache)

```sh
# Install (per-user; drop --global for that, or add it for system-wide)
kpackagetool6 --type Plasma/Wallpaper --install packaging/kde/io.github.dibbayajyotiroy.fresco.lockscreen

# Upgrade (same package already installed, contents changed -- e.g. a new Fresco version)
kpackagetool6 --type Plasma/Wallpaper --upgrade packaging/kde/io.github.dibbayajyotiroy.fresco.lockscreen

# System-wide install (needs root; Fresco's .deb postinst should use this)
sudo kpackagetool6 --global --type Plasma/Wallpaper --install packaging/kde/io.github.dibbayajyotiroy.fresco.lockscreen

# Remove (by plugin id, not path)
kpackagetool6 --type Plasma/Wallpaper --remove io.github.dibbayajyotiroy.fresco.lockscreen
```

**Verified** flag names and semantics from `frameworks/kpackage`
(`src/kpackagetool/options.h`): `-t/--type`, `-i/--install <path>`,
`-u/--upgrade <path>`, `-r/--remove <name>`, `-g/--global` ("operates on
packages installed for all users"), `-l/--list`, `-s/--show <name>`.

### Via plain copy (works too; skip if using kpackagetool6)

`kpackagetool6` mostly validates `metadata.json` and copies the directory
tree, so a `.deb` postinst can instead just copy this directory to the system
path above (owned by root, world-readable) and skip the `kpackagetool6`
dependency entirely. There is no separate "registration" database to update
for wallpaper packages -- Plasma discovers them by scanning
`plasma/wallpapers/*/metadata.json` under the standard XDG data dirs.

### Optional runtime dependency: Qt6 Multimedia QML module

Video playback needs the Qt6 Multimedia QML module, but the package degrades
gracefully without it: `contents/ui/main.qml` never imports `QtMultimedia`
itself, and instead loads `contents/ui/VideoLayer.qml` (the only file that
does) through a `Loader` by *source URL*, not as an inline `Component`. QML
resolves a document's imports when that document is compiled, and a `Loader`
is Qt's documented way to contain that: if `VideoLayer.qml`'s
`import QtMultimedia` can't resolve, only `videoLoader.status` becomes
`Loader.Error` -- `main.qml` itself keeps loading, `videoFailed` becomes
true, and it falls back to `StillPath` (or the plain `#0b0b10` background if
there is no still either). The dim veil and the widget layer are unaffected
either way, since neither depends on the video Loader's state. The same
fallback path also covers a video that fails to *play* (bad codec, corrupt
file) once the module is present, via `VideoLayer.qml`'s own `hasError`.

Because of that fallback, this is now a **recommended**, not hard,
dependency for whatever packages this KPackage (the `.deb` postinst, most
likely) -- add it as a `Recommends:`/`optdepends`, not `Depends:`, so systems
without it still get a working (photo-only) lock-screen wallpaper instead of
failing to install:

- Debian/Ubuntu (incl. Pop!_OS): `qml6-module-qtmultimedia` -- confirmed via
  `apt-cache policy` on this dev box: available in the repos, but **not
  installed by default**, i.e. this is a real gap in practice, not a
  hypothetical one.
- Arch: `qt6-multimedia`
- Fedora: `qt6-qtmultimedia`

## Selecting it as the lock-screen wallpaper

Everything the lock screen reads lives in `~/.config/kscreenlockerrc`
(an ordinary KConfig ini file). Nested KConfig groups are written as a single
literal bracket-chained section header, e.g. `[Greeter][Wallpaper][id]`, not
as separate nested `.ini` sections -- confirmed by reading how
`KConfigGroup::group()` chains are actually consumed on both the read and
write side below.

### 1. Pick the plugin: `[Greeter] WallpaperPlugin=`

```ini
[Greeter]
WallpaperPlugin=io.github.dibbayajyotiroy.fresco.lockscreen
```

**Verified** in `plasma/kscreenlocker`, `settings/kscreenlockersettings.kcfg`:

```xml
<kcfgfile name="kscreenlockerrc" />
<group name="Greeter">
  <entry name="wallpaperPluginId" key="WallpaperPlugin" type="String">
    <default>org.kde.image</default>
  </entry>
</group>
```

Command: `kwriteconfig6 --file kscreenlockerrc --group Greeter --key WallpaperPlugin io.github.dibbayajyotiroy.fresco.lockscreen`

### 2. Set this plugin's own config: `[Greeter][Wallpaper][<id>][General]`

```ini
[Greeter][Wallpaper][io.github.dibbayajyotiroy.fresco.lockscreen][General]
VideoPath=/home/user/Videos/wallpaper.mp4
StillPath=/home/user/Pictures/poster.jpg
PlayVideo=true
Dim=0.2
LayerDir=/run/user/1000/fresco/lockscreen
RefreshMs=1000
```

**Verified independently on both the read side and the write side**, and
they agree:

- Read side, `greeter/greeterapp.cpp` (what `kscreenlocker_greet` loads when
  it starts):
  ```cpp
  const KConfigGroup cfg = KScreenSaverSettingsBase::self()->sharedConfig()
      ->group(QStringLiteral("Greeter"))
      .group(QStringLiteral("Wallpaper"))
      .group(KScreenSaverSettingsBase::self()->wallpaperPluginId());
  ```
- Write side, `settings/wallpaper_integration.cpp` (what the System Settings
  KCM writes when a human uses the GUI):
  ```cpp
  const KConfigGroup cfg = m_config->group(QStringLiteral("Greeter"))
      .group(QStringLiteral("Wallpaper")).group(m_pluginName);
  ```

`KConfigLoader` then applies `contents/config/main.xml`'s own
`<group name="General">` on top of that group, giving the final
`[Greeter][Wallpaper][io.github.dibbayajyotiroy.fresco.lockscreen][General]`.

Commands (one per key; `--type bool` for `PlayVideo`, no type flag = string,
`Dim`/`RefreshMs` are written as plain numeric strings):

```sh
kwriteconfig6 --file kscreenlockerrc \
  --group Greeter --group Wallpaper --group io.github.dibbayajyotiroy.fresco.lockscreen --group General \
  --key VideoPath "/home/user/Videos/wallpaper.mp4"

kwriteconfig6 --file kscreenlockerrc \
  --group Greeter --group Wallpaper --group io.github.dibbayajyotiroy.fresco.lockscreen --group General \
  --key PlayVideo --type bool true
```

**Verified** `--group` repeats-for-nesting and `--type bool` behaviour from
`frameworks/kconfig`, `src/kreadconfig/kwriteconfig.cpp`.

### 3. Optional: hide KDE's own lock-screen clock / media controls

These are **not** part of this KPackage or of kscreenlocker itself -- they
belong to the default Plasma *shell* package (`org.kde.plasma.desktop`,
which is what ships the actual `LockScreen.qml`/`LockScreenUi.qml` that hosts
whichever wallpaper plugin is selected). Because they are not scoped to a
plugin id, **changing them affects the clock/media-controls overlay no
matter which lock-screen wallpaper is active**, not just Fresco's.

```ini
[Greeter][LnF][General]
alwaysShowClock=false
hideClockWhenIdle=false
showMediaControls=false
```

**Verified**, group path from `plasma/kscreenlocker`,
`settings/shell_integration.cpp`:

```cpp
const KConfigGroup cfg = m_config->group(QStringLiteral("Greeter")).group(QStringLiteral("LnF"));
```

**Verified**, schema/entry names from `plasma/plasma-desktop`,
`desktoppackage/contents/lockscreen/config.xml` (group `General`):
`alwaysShowClock` (Bool, default `true`), `hideClockWhenIdle` (Bool, default
`false`), `showMediaControls` (Bool, default `true`) -- and their actual use
confirmed by grepping `desktoppackage/contents/lockscreen/LockScreenUi.qml`
(`visible: ... config.alwaysShowClock`, media controls `Loader { active:
config.showMediaControls }`).

```sh
kwriteconfig6 --file kscreenlockerrc --group Greeter --group LnF --group General --key alwaysShowClock --type bool false
kwriteconfig6 --file kscreenlockerrc --group Greeter --group LnF --group General --key showMediaControls --type bool false
```

If Fresco's own clock/now-playing widgets are enabled in `LayerDir`'s PNGs,
setting both of the above to `false` avoids showing two clocks / two sets of
media controls. Leave them `true` (the KDE defaults) if the user wants both.

### When do changes take effect: next lock, not live

**Verified, not a guess**: `kscreenlocker_greet` is a separate process,
spawned fresh on every lock by `KSldApp` in `ksldapp.cpp`
(`m_lockProcess = new QProcess(); ... m_lockProcess->start(greeterPath, args);`),
and `greeterapp.cpp` builds its `KConfigLoader`/`KConfigPropertyMap` for the
wallpaper exactly once, in `createViewForScreen()`, at that process's
startup. There is no code path that re-reads `kscreenlockerrc` while a
greeter process is already on screen. **So: write the config, then the
change is visible next time the screen locks -- not while already locked.**

One nuance left **unverified**: `wallpaper_integration.cpp` calls
`m_configuration->setNotify(true)` with a comment about a `kded` module
("picture of the day") monitoring changes live. Whether `kwriteconfig6 ...
--notify` on these keys has any live effect on anything is not confirmed
either way; treat "next lock" as the only guaranteed contract.

## The widget-layer PNG contract (for the daemon)

`main.qml` polls `LayerDir` every `RefreshMs` (clamped to a 200ms floor) and
expects:

- `LayerDir/layer-<connector>.png` -- preferred, one per output, where
  `<connector>` should match QML's `Screen.name` for that output (typically
  the RandR/`wl_output` name, e.g. `eDP-1`, `DP-2`). **Not independently
  verified** that `Screen.name` is byte-for-byte identical to the connector
  name on every driver/compositor combination -- see Risks below.
- `LayerDir/layer.png` -- fallback used whenever the connector-specific file
  is missing or fails to decode.
- Both must be **transparent PNGs at the output's native pixel size**
  (`main.qml` stretches them with `Image.Stretch`, doing no scaling itself).
- Rewrites must be **atomic** (write to a temp file, then rename over the
  target) so the QML side never reads a half-written file. `main.qml`
  tolerates a transient read failure gracefully (keeps the last good frame),
  but a torn/partial PNG that *does* decode would flash garbage for one tick.
- Absence of both files is a fully supported, silent no-op (no layer drawn,
  no error icon) -- useful before Fresco's daemon has started, or if widgets
  are all disabled.

## Config schema reference (`contents/config/main.xml`)

| Key | Type | Default | Notes |
|---|---|---|---|
| `VideoPath` | String | `""` | Absolute path; empty = no video |
| `StillPath` | String | `""` | Absolute path; empty = no still fallback |
| `PlayVideo` | Bool | `true` | If false, goes straight to `StillPath` |
| `PauseMode` | Int | `0` | Desktop only: `0` pause while a window is fullscreen, `1` also while one is maximized, `2` never. Windows on other screens/desktops/activities and minimized ones are ignored |
| `Dim` | Double | `0.2` | Clamped to `[0, 0.8]` in both the schema (`<min>`/`<max>`) and `main.qml` |
| `LayerDir` | String | `""` | Directory Fresco rewrites; see contract above |
| `RefreshMs` | Int | `1000` | Widget-layer poll interval |

## Open risks / things the packaging agent should double-check

1. **QtMultimedia availability** -- now handled by design, not just a
   dependency note (see "Optional runtime dependency" above): a missing
   module degrades to the still image / plain background via
   `Loader.status === Loader.Error`, rather than failing the whole plugin.
   Still worth double-checking on a real Plasma 6 + `kscreenlocker_greet`
   install that a `Loader.source` import failure actually behaves as
   documented there too, since this was reasoned from QML/Loader semantics
   and Qt's own documentation, not exercised against a running greeter (no
   `qml6`/`qmllint` on this machine -- see validation notes below).
2. **`Screen.name` vs connector name** -- verified that each `kscreenlocker_greet`
   `QQuickView` is bound to a specific `QScreen` via `view->setScreen(screen)`
   (`greeterapp.cpp`), so `Screen.name` is per-output, but the exact string
   Qt reports for `Screen.name` on a given driver (X11 vs. the Wayland
   session `kscreenlocker_greet` actually runs under) was not independently
   confirmed against `wlr-output-management`/`xrandr` connector names in this
   task. If Fresco names its PNGs from a different source (e.g. `wl_output`
   name reported to *Fresco's own* Wayland connection) the two could
   mismatch; `layer.png` exists specifically as the safety net for that case.
3. **No literal OS-level seccomp filter was found** in current
   `plasma/kscreenlocker` source (searched `greeter/`, `greeter/worker/`,
   top-level and `greeter/CMakeLists.txt` for "seccomp"/"sandbox": no hits).
   What *is* verified is `greeter/noaccessnetworkaccessmanagerfactory.cpp`,
   installed onto the QML engine in `greeterapp.cpp`
   (`view->engine()->setNetworkAccessManagerFactory(new
   NoAccessNetworkAccessManagerFactory)`, asserted to stay installed at line
   402), which fails every non-local request with `ContentAccessDenied`.
   Local (`file://`-class) reads are explicitly exempted
   (`KProtocolInfo::protocolClass(scheme) == ":local"`), which is why this
   package can read local video/image/PNG files but must never touch the
   network. Practically the same guarantee the task assumed, just enforced
   one layer up the stack from a kernel seccomp-bpf filter.
4. **Icon theme assumption**: `metadata.json`'s `Icon` field uses the generic
   `preferences-desktop-wallpaper` (same fallback both reference
   implementations use) rather than Fresco's own hicolor icon, so the
   wallpaper-type list has a sensible icon even if this package is ever
   installed before Fresco's own `.desktop`/icon files are.
5. This package was written and reasoned about, but **never executed**: no
   `qml`/`qml6` runtime or `qmllint` binary exists on this machine (checked
   `which`, common `/usr/lib*/qt6/bin` paths, and `dpkg -l` -- only the
   runtime libraries `libqt6qml6`/`libqt6quick6` are installed, not the
   dev/tooling package). Validated instead: `metadata.json` is valid JSON
   (`python3 -m json.tool`), `contents/config/main.xml` is well-formed XML
   (`python3 -c 'import xml.dom.minidom; ...'`; caught and fixed one bad
   `--` inside an XML comment this way), and all three `.qml` files
   (`main.qml`, `config.qml`, `VideoLayer.qml`) have balanced braces/parens.
   The double-buffered widget-layer logic was reasoned through
   by hand (see the comments in `main.qml`) but not exercised against a real
   `QQuickView`.

## Sources consulted

- `github.com/luisbocanegra/plasma-smart-video-wallpaper-reborn` --
  `package/metadata.json`, `package/contents/ui/main.qml` (lock-screen
  detection via `window.source.toString().endsWith("LockScreen.qml")`,
  `shouldPlay`/`lockScreenMode` handling), `package/contents/ui/VideoPlayer.qml`
  (MediaPlayer/VideoOutput/AudioOutput idiom), and its `README.md` (confirms
  the System Settings > Screen Locking > Appearance UI path and that
  `kscreenlockerrc` is where a third-party wallpaper's own keys land).
- `invent.kde.org/plasma/plasma-workspace` --
  `wallpapers/image/imagepackage/metadata.json`, `contents/config/main.xml`,
  `contents/ui/main.qml`, `contents/ui/config.qml` (canonical
  `WallpaperItem`/`root.configuration.<Key>`/`wallpaper.configuration`
  structure for a first-party Plasma 6 wallpaper).
- `invent.kde.org/plasma/kscreenlocker` --
  `settings/kscreenlockersettings.kcfg` (`WallpaperPlugin` key),
  `settings/wallpaper_integration.cpp` and `greeter/greeterapp.cpp`
  (`[Greeter][Wallpaper][<id>]` group path, read and write side),
  `settings/shell_integration.cpp` (`[Greeter][LnF]` group path),
  `greeter/noaccessnetworkaccessmanagerfactory.{cpp,h}` and `greeterapp.cpp`
  (network denial mechanism), `ksldapp.cpp` (greeter spawned as a fresh
  `QProcess` per lock), `kcm/ui/Appearance.qml` and `kcm/ui/WallpaperConfig.qml`
  (how `contents/ui/config.qml` gets instantiated and why no `cfg_*`
  properties are required).
- `invent.kde.org/plasma/plasma-desktop` --
  `desktoppackage/contents/lockscreen/config.xml` and `LockScreenUi.qml`
  (`alwaysShowClock`, `hideClockWhenIdle`, `showMediaControls`).
- `invent.kde.org/plasma/libplasma` --
  `src/plasma/packagestructure/qmlWallpaper/wallpaper.cpp` (`plasma/wallpapers/`
  package root, required `ui/main.qml`).
- `invent.kde.org/frameworks/kpackage` -- `src/kpackagetool/options.h`
  (`kpackagetool6` flags).
- `invent.kde.org/frameworks/kconfig` -- `src/kreadconfig/kwriteconfig.cpp`
  (`kwriteconfig6` flags, repeated `--group` for nesting).

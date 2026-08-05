# Changelog

All notable changes to Fresco are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.36] — 2026-07-30

### Added
- **Widgets on your wallpaper: synced lyrics and a clock.** The thing people ask
  for when they ask for a Wallpaper Engine equivalent, and the thing nothing on
  Linux does well. Both are drawn *into* the wallpaper itself rather than into a
  new window, so there is no extra surface to stack, nothing to click through,
  and they behave the same on X11 and on every layer-shell compositor. **Both
  are off until you turn them on** — app menu (Ctrl+,) → **Advanced…** → the
  **Lyrics** and **Clock** groups — and when they are off nothing is created at
  all: no watcher, no overlay, no wakeups.

  **Lyrics** follow whatever is playing anywhere on the system (any player that
  publishes MPRIS: browsers, music apps, video players) and show the current
  line in time with the music, optionally with the next line dimmed underneath.
  Four looks (Minimal, Karaoke, Subtitle, Card), a nine-point placement grid, a
  margin and type size, tinting that follows your accent color, and a sync
  offset slider — because `.lrc` timing is contributed by strangers and a file
  can simply sit a second off no matter how exact Fresco's own clock is.
  Lyrics come from a local `.lrc` file first (next to the audio file, or in a
  lyrics folder you choose) and only then from the online database; see the
  privacy note below.

  **The clock** needs no music and no network. Five themes (Digital, Minimal,
  Segment, Stacked, Wordy), 12- or 24-hour, optional date, the same placement
  grid and accent-follow. Seconds are **off by default and that is a battery
  decision, not an oversight**: without seconds the clock repaints once a
  minute, with them sixty times — a permanent 60× increase in wakeups on an idle
  desktop, for a digit pair few people look at.

  Neither widget repaints unless what it says has actually changed: a lyric line
  held for eight seconds paints once, and a clock reading `14:32` paints nothing
  until `14:33`. Widgets appear on **every display** by default, matching the
  wallpaper itself; set `monitor` in the `[widgets]` block of `config.toml` to a
  connector name (`"DP-1"`) to keep them on one screen.

  **Where it does not work:** GNOME on Wayland, which has no live wallpaper
  surface to draw into and so has no widget layer either — the same limitation
  that makes it a static-frame fallback today.

- **Two more widgets: an audio visualiser and a spinning album-art disc.** The
  visualiser draws the music itself — five looks (bars, mirrored, wave, dots and
  a radial ring) with a colour you pick, a two-colour blend, or a rainbow sweep.
  The disc shows the current track's cover art on a turning record, and stops
  turning the moment playback pauses, because a disc spinning over a paused song
  is a lie about what your computer is doing.

  Both are off by default, and the visualiser asks before it listens — see the
  privacy note. Measured on a live desktop with music playing and all four
  widgets on: **0.8% of one CPU core**, essentially all of it the audio capture;
  the daemon's own share rounds to zero, because nothing repaints unless what it
  shows has actually changed.

- **A sixth clock theme: Card.** A rounded glass panel carrying the time,
  weekday and date with an analog face beneath them. The card is genuinely
  translucent — your wallpaper shows through it — with a lit edge and a soft
  scrim behind the text so the type survives a bright frame. (ASS has no
  backdrop blur, so this is translucency and edge lighting rather than true
  frosted glass; the scrim is what stands in for the blur that would otherwise
  protect contrast.)

- **Colours you choose, for lyrics and the visualiser.** A colour picker for
  each, plus a blend across the visualiser's bars. The blend interpolates around
  the colour wheel rather than straight through it, so a pink-to-cyan ramp stays
  vivid the whole way instead of passing through mud in the middle.

- **Optional track title and artist above the lyric line.** Off by default. With
  it on, a song with no lyrics available still shows what is playing, rather than
  showing nothing at all — which is exactly when you would want it.

- **A new app icon**, across every size, the scalable variant, Deepin's bloom
  theme, and the website.

- **The first-run tour now covers setting a wallpaper from a link, and where the
  widget settings live.** It is a two-step flow, and step one arrives with a
  working link already in the box so it can be finished in one click. Anyone who
  has already seen the old tour sees the new one once.

### Privacy
- **Lyrics lookups leave your machine on a cache miss.** Local `.lrc` files are
  tried first and cost nothing. When there is no local file, Fresco asks
  [LRCLIB](https://lrclib.net) — a free, community-run database — for that one
  track, which means **the track title, artist and album are sent to a third
  party**. The result is cached under `~/.cache/fresco/lyrics`, so replaying the
  same song never touches the network again. Nothing is sent unless you turn the
  lyrics widget on and something is playing that has no local lyrics file.
  Fresco does not host, own or license lyric content; LRCLIB's entries are
  user-contributed and LRCLIB states no license over them, so Fresco fetches on
  demand and caches per-user rather than bundling or redistributing anything.
- **The audio visualiser reads your system's audio output, and asks first.** It
  has to: bars that react to the music need the actual sound, which MPRIS never
  carries. So the first time you switch it on, Fresco asks — plainly, in a
  one-time dialog — and does nothing until you agree, exactly as the
  usage-statistics prompt does. The audio is analysed on your machine, is never
  recorded, stored or sent anywhere, and the capture stops the moment you turn
  the widget off. Declining leaves it off. The consent is enforced when the
  config is *loaded*, not in the settings window, so hand-editing `config.toml`
  cannot start a capture you never agreed to.

### Fixed
- **Lyrics now follow an automatic track change.** They did not: playing through
  a queue in Firefox left the previous song's words on screen until you toggled
  the widget off and on. Firefox publishes a **constant** MPRIS track id — its
  own object path, identical for every song it will ever play — and Fresco
  treated that id as authoritative, so every advance looked like the same track.
  Track identity now requires the metadata *and* the id to agree. Verified
  against a live capture of three automatic advances across four songs: four
  titles, one id.

- **Chromium-family browsers no longer block the lyrics widget.** Chrome, Brave
  and Edge claim an MPRIS name the first time anything plays and never release
  it, so a finished session lingers on the bus carrying artwork but no track
  title. Fresco would select that corpse and sit there. Sessions without a title
  are now skipped, with a log line naming the bus so the reason is visible
  rather than mysterious. Firefox remains the most reliable browser for this.

- **Widgets appear on every display**, matching the wallpaper itself. They were
  drawn on one, which read as half-broken on a two-monitor desk; naming a
  connector in `[widgets] monitor` is the way back to a single screen.

- **A healed renderer keeps its widgets.** When the supervisor restarts a wedged
  or dead mpvpaper the new player starts with no overlays, and nothing told the
  widget layer to redraw — so a recovered wallpaper came back bare until its
  content next changed. Every respawn is now counted, and the widgets are
  re-pushed once when the count moves.

- **"Follow accent colour" now does something when you turn it off.** The
  visualiser passed the accent colour unconditionally, so the switch had no
  effect in either position.

- **Image and slideshow wallpapers no longer reload themselves every 8 seconds
  on Wayland.** The supervisor restarts a renderer whose playback clock stops —
  a real symptom of a wedged mpvpaper. But Fresco starts every renderer with
  `image-display-duration=inf`, so a still image holds `time-pos` at 0 *by
  design*: measured against a live mpvpaper, an image reports the same position
  forever while claiming to be playing. Every image was therefore diagnosed as
  wedged after 6 seconds and killed, on repeat, for as long as it was on screen.
  **Slideshows were the worst hit: a respawn rebuilds the slideshow from image
  one, so a cycle that respawned faster than the interval — 8 seconds against
  the 30-second default — could never reach the second image at all.** The
  detector now asks the player whether the media is a still frame (mpv reports
  duration 0) before counting a strike, so a genuinely frozen *video* is still
  caught. The X11 backend has always skipped stills in its equivalent check;
  the Wayland supervisor was written without that guard.

- **A sleeping monitor no longer permanently disables its wallpaper.** When a
  display goes away — DisplayPort links commonly drop when a monitor powers
  off — mpvpaper for that connector dies and cannot be started again, because
  the compositor no longer advertises the output. Fresco counted those as
  renderer failures, burned all five restarts in ten seconds and gave up on the
  output *for good*: the display came back to a dead wallpaper until the user
  re-applied one or restarted the daemon. The supervisor now asks the
  compositor whether the connector is still there before spending a restart, and
  only when a renderer is already down, so a stale enumeration can never tear
  down a healthy one. A vanished display parks its output instead; when it
  returns, playback is restored from a clean slate.

- **`renderer_giveup` reports now say what actually broke.** The warning carried
  only "renderer failed 5×", which cannot distinguish a missing mpvpaper from a
  broken GL stack or a display that walked away. It now also carries the failure
  mode (dead or frozen), the wallpaper kind, and a spawn-failure code — all
  content-free, no paths or file names.

- **"Renderer failed 5×" no longer stands in for "that folder has no images in
  it".** Field reports showed the give-up warning landing 10–15 seconds after a
  slideshow was set, over and over — the shape of a wallpaper with nothing
  behind it. If the media cannot be resolved, every respawn bails before it
  starts anything, five ticks spend the whole anti-flap budget, and the output
  is given up on, reported as a renderer failure. That is not what went wrong,
  and it points you at the wrong thing. A wallpaper is now checked for a file
  that is actually *there* before a restart is spent, and an output with nothing
  to play says so — "the wallpaper's file or slideshow folder is empty, missing,
  or unreadable" — and waits, picking playback back up by itself once a readable
  file exists. Media that was deleted, or that lives on a drive which is
  currently away, is treated the same way, and a slideshow now starts on its
  first image that still exists rather than failing on a stale entry.
  `renderer_giveup` gained a `no_file` cause so this can no longer hide behind a
  generic spawn failure.

## [1.1.35] — 2026-07-25

### Added
- **Select several wallpapers and remove them in one go.** Requested by @175624
  for managing a large library: the counterpart to adding many at once with
  "Add folder". Enter select mode from the footer's **Select** button or a
  card's right-click **Select…** (which pre-ticks that card), tick any number of
  cards, then **Remove**. **Select all** honors the active search, so searching
  and then selecting all is the fast path for clearing a batch. Removal is
  confirmed first, states that the source files on disk are kept, and if one of
  the selected wallpapers is the one on screen the desktop reverts to its own
  background.

- **A Deepin-styled app icon on DDE.** Fresco's icon is redrawn to the bloom
  theme's conventions — full-bleed squircle, a single frame instead of the
  standard icon's nested rings, stronger interior contrast so it still reads in
  the launcher grid — while keeping the same framed-wallpaper identity. It ships
  inside Deepin's `bloom` theme under the same icon name, so **only a DDE
  session resolves it**; every other desktop is untouched.

### Changed
- **Power saving now defaults to Reduced instead of Full quality.** @175624
  measured actual package power with `turbostat` on an Intel N150 (Deepin 25,
  VA-API), two runs per level: at 1080p, Reduced halves GPU power (1.37 W →
  0.63 W, −54%) and cuts total package power by 33%, while Minimum adds nothing
  further; at 4K, Reduced saves 42% of GPU power and Minimum 65% (2.77 W →
  0.99 W), nearly 3 W off the package. Reduced captures most of the available
  saving at every resolution for a softening that is hard to notice on a
  wallpaper sitting behind windows, so it is the better default — Minimum is
  worth choosing for 4K sources, and Full remains available for the sharpest
  image. **Existing settings are untouched:** any config that already records a
  power-saving level keeps it, so this only changes fresh installs.

### Fixed
- **Light/Dark and the accent colors now actually apply while the app is
  running.** Choosing Light, Dark, or a different accent changed the setting but
  left the window painted in whatever theme it started with. Every rule in the
  stylesheet referenced a named `@define-color`, and GTK keeps an already-defined
  named color at its startup value — runtime redefinitions are silently ignored —
  so no theme change could ever take effect. The stylesheet is now built with
  literal colors and repaints live. The Add button also follows the accent
  instead of staying a fixed blue.
- **Deepin 25: another attempt at the launcher not showing Fresco until
  `killall dde-shell`.** Not an icon problem — @175624's 2026-07-26 test log
  rules out our whole side of it: the `.desktop` file validates, GAppInfo and
  `ApplicationManager1` both list Fresco, a `.dci` icon is generated, and
  `GtkIconTheme` resolves our icon in bloom, hicolor **and** Papirus. (The bloom
  icons 1.1.35 briefly shipped for this are reverted; they changed nothing.)
  What differs from a package that *does* appear (`xpad`) is dpkg's trigger
  order: `deepin-home-appstore-daemon`, the trigger that makes the launcher
  hot-refresh, runs **second** for Fresco — before `desktop-file-utils` rebuilds
  the desktop database and before the `.dci` is generated — but **last** for
  xpad. The launcher appears to refresh against an incomplete state and drop us.
  Fresco now re-announces its `.desktop` file a few seconds after install, on
  Deepin only, using the exact write pattern the log shows is picked up. If it
  still doesn't appear, `killall dde-shell` (or logging out) remains the
  workaround and the diagnosis continues in `docs/AUDIT.md`.

## [1.1.34] — 2026-07-24

### Changed
- **Power saving now targets the actual bottleneck (GPU scaling).** 1.1.33's
  Power saving used decoder-level frame skipping (`vd-lavc-skipframe`). Tested
  on real Deepin 25 hardware by @175624 with `intel_gpu_top`, that changed the
  visible frame rate but saved nothing: for a hardware-decoded video wallpaper
  the load is **Render/3D (~99%)**, not decode (~17%), and skipping decoded
  frames touches neither (the GPU still decodes the stream and presents at
  display refresh). Power saving now instead reduces the per-frame **scaler**
  cost — Reduced and Minimum drop from the quality scalers (spline36 / lanczos
  with linear-light downscaling and dithering) toward cheap bilinear, trading
  sharpness for GPU-render load. It can only reduce or match GPU work, never
  increase it, and hardware decoding is untouched. This is a quality/perf
  trade-off, not a promised number; the magnitude of the win is pending
  confirmation on the reporter's Intel box.

### Fixed
- **App icon now appears in the Deepin launcher without restarting it.** Since
  the icon landed (1.1.3), it only showed after `killall dde-shell`. Root cause,
  found by diffing our `.deb` against galculator's (which the reporter confirmed
  works): we shipped a custom postinst that ran `gtk-update-icon-cache` /
  `update-desktop-database` during package configure. That refresh is already
  the job of the standard `hicolor-icon-theme` and `desktop-file-utils` dpkg
  triggers (fired automatically when files land in their dirs), and running it
  early interfered with Deepin's launcher refresh. Fresco now ships no
  maintainer scripts — identical to a plain debhelper GUI package — and relies
  on those triggers. (Fix pending confirmation on real Deepin 25.)
- **Removing or stopping the active wallpaper now reverts the desktop.** A new
  "Stop wallpaper" item (right-click the active card) turns the wallpaper off
  and restores the desktop's own background without deleting the entry;
  removing the active card does the same. Previously the daemon kept playing a
  wallpaper you'd deleted until the app was force-closed.

## [1.1.33] — 2026-07-23

### Changed
- **The frame-rate cap is replaced by a Power saving control** (Full quality /
  Reduced / Minimum), in the same two places: a global default in Settings →
  Advanced → Video quality and a per-wallpaper override in the editor.

  1.1.32's frame-rate cap did the opposite of what it promised. Capping fps used
  an `fps` video filter, and a video filter is *software*: inserting one into a
  hardware-decoding (VA-API) pipeline forces every frame to be copied off the
  GPU. A user on Intel Alder Lake-N measured video-engine load roughly
  **doubling** — about 17% to 34% — when capping 60fps to 30. Thanks to
  @175624 for catching it with `intel_gpu_top`.

  Power saving instead uses decoder-level frame skipping
  (`--vd-lavc-skipframe`), which discards frames inside libavcodec *before*
  they are decoded, so the work is never done and hardware decoding stays
  active. (Superseded in 1.1.34: this changed the visible frame rate but did
  not reduce GPU load on hardware-decoded video.)

  Existing `framerate` settings migrate automatically — any cap becomes Reduced.

### Fixed
- **Light mode readability.** Several surfaces were unreadable or unstyled in
  the light scheme: the wallpaper right-click menu's "Remove from library" was
  invisible (a flat destructive button inherited Adwaita's white label, leaving
  white text on paper); glass modals let the content behind them bleed through
  and collide with their own text; error messages rendered as ordinary grey
  text; and the capability notice and crop/transition stage had no styling at
  all. Dark mode is unchanged.

## [1.1.32] — 2026-07-23

### Added
- **Frame-rate cap** for video wallpapers — limit to 24/30/48/60 fps (or keep
  the original rate). **Superseded in the next release: this made decode load
  worse on hardware-decoded video, not better — see 1.1.33 above.** Two
  levels: a global default in Settings → Advanced → Video quality, and a
  per-wallpaper override in the crop/rotate editor.

### Fixed
- **Consistent app icon.** The scalable icon still shipped the old v0.0.1
  artwork while the fixed-size PNGs were the current logo, so the launcher
  (which converts the SVG to DCI on Deepin) and the taskbar could disagree. The
  scalable SVG is now regenerated from the current logo — every surface matches.

## [1.1.31] — 2026-07-23

### Fixed
- **Live wallpaper now actually shows on Deepin 25 (DDE)** ([#2]). 1.1.3's DDE
  support never worked: two bugs made it silently do nothing, both found by
  running Fresco on a real Deepin 25 desktop.
  - The scan for DDE's desktop window asked the X server for the client list
    with `long_length = u32::MAX`, which Xorg rejects — so the window was
    never found and Fresco always chose the wrong strategy.
  - The WM_CLASS matcher expected `"dde-shell"` + `"desktop"` as two separate
    strings; Deepin 25 reports the single token `"dde-shell/desktop"` with
    class `"org.deepin.dde-shell"`, so it never matched.

  The strategy itself changed too. Making DDE's wallpaper transparent cannot
  work on Deepin 25, so Fresco now declares its wallpaper window as
  `_NET_WM_WINDOW_TYPE_DESKTOP` **and** `_NET_WM_WINDOW_TYPE_NORMAL` — the
  same pair dde-shell uses — and raises it with a sibling-less
  `ConfigureWindow(Above)`. A sibling-relative restack is impossible here:
  KWin reparents both windows, so they are not siblings and the request fails
  with BadMatch. Verified on Deepin 25: the wallpaper window sits above
  dde-shell's desktop, app windows and the dock still stack above it, and
  clicks pass through to the desktop (right-click menu still works).

  Trade-off: desktop icons are hidden while a live wallpaper is set, because
  DDE draws icons and wallpaper inside one window. Set `dde_mode` in
  config.toml (or `FRESCO_DDE_MODE`) to `transparent` for the old behaviour on
  Deepin 20/23, or `restack` to force the new one.
- Behaviour on every non-Deepin desktop is byte-identical to 1.1.3: the extra
  window type and the raise are only used when DDE is detected.

### Added
- Deepin 25 (crimson) added to the distro CI matrix (build + clean-install),
  plus an install-time check that all icon sizes and the `.desktop` entry
  land correctly on every distro.

### Fixed
- **App icon missing under Deepin's bloom icon theme** ([#1]). The icon was
  shipped only as `hicolor/scalable` SVG, which bloom (and other
  fixed-size-only themes) never look up. The packages now also install
  48/64/128/256/512 px PNGs into hicolor, and the .deb refreshes the icon
  cache in postinst/postrm.

[#1]: https://github.com/DibbayajyotiRoy/fresco/issues/1
[#2]: https://github.com/DibbayajyotiRoy/fresco/issues/2

## [1.1.2] — 2026-07-20

### Added
- **Add from a link.** Paste a Pinterest pin (pin.it short links and story
  pins included) or any direct video/image URL — Fresco resolves it, downloads
  it, and opens the crop/rotate editor so you can frame it before setting.
- **Your wallpaper on every new tab.** An optional local-only browser bridge
  (127.0.0.1, off by default) plus a companion extension in `extension/`
  (Chrome, Brave, Edge, Firefox) mirrors your wallpaper — or a
  browser-specific pick via right-click → "Set as browser wallpaper".
- **Command palette** (Ctrl+K): set any wallpaper by name, random wallpaper,
  and jump to any feature from the keyboard.
- Favorites with hover actions, drag-and-drop import, a first-run feature
  tour, and a quick day/night schedule pause switch in Settings.

### Privacy
- Fresco can send anonymous usage statistics — a daily ping (random install
  id, app version, distro name, desktop/compositor, session type, renderer
  backend, decode mode, monitor count), feature-usage counts, and error kinds.
- **Nothing is sent until you say yes**: a one-time consent dialog asks on
  first launch, and the answer can be changed anytime in Settings →
  "Share anonymous usage statistics".
- No personal data, no file names, no wallpaper content is ever sent.

### Fixed
- **Fullscreen auto-pause now works on COSMIC.** The wallpaper pauses while a
  window is fullscreen (no more decoding a hidden wallpaper under your video),
  via COSMIC's native toplevel-info protocol — previously this protection was
  silently unavailable there.
- Clicking the feedback reminder notification now opens the feedback dialog
  directly instead of just launching the app.

### Changed
- **Media-first redesign**: larger wallpaper grid, resolution/fps/size on
  every card with 4K badges, a cleaner now-playing pill, and a real
  drop-files-here empty state.

## [1.1.1] — 2026-07-17

### Fixed
- **Live wallpapers work on Ubuntu 24.04-based systems (COSMIC, Pop!_OS 24.04,
  Mint 22…).** The bundled renderer was built against an older libmpv and
  silently failed to start on newer distros, leaving the desktop's default
  wallpaper. Fresco now ships one renderer build per libmpv generation and
  picks the one that works on your system automatically.
- The install command detects a renderer that can't load and rebuilds it
  against your system's libmpv on the spot, so a fresh install always ends
  with a working live wallpaper.
- **Library cards no longer resize or jump around while hovering** — the
  hover-to-play preview could push the whole grid into a glitchy reflow loop
  on high-resolution videos.
- `fresco doctor` now catches a renderer that exists but can't load, instead
  of reporting a healthy system while nothing renders.

### Changed
- **In-app updates finish themselves.** Updating now shows a real progress
  bar with live download percentage, and the app restarts automatically a few
  seconds after the update completes (cancellable) — no more wondering whether
  a restart is needed. The wallpaper daemon restarts too, so fixes apply
  immediately.
- The "what's new" notes now always appear after an update.

## [1.1.0] — 2026-07-12

### Fixed
- **Multi-monitor video sync** — the same video on several displays now stays
  in step instead of slowly drifting apart.
- Scheduled wallpaper swaps no longer leak the previous entry's rotation and
  crop onto the next wallpaper.
- Smoother playback on Wayland: display-matched frame timing now applies there
  just like on X11.

### Added
- Occasional feedback reminders (can be turned off in Settings) so it's easy
  to tell us what to improve; reports now carry your timezone and locale for
  region-aware fixes.

## [1.0.1] — 2026-07-04

### Fixed
- **Editing a wallpaper's rotation now updates its card thumbnail** — the
  library card kept the old orientation before (thumbnails were only generated
  at import).
- Hover-to-play is skipped on rotated entries: GTK's inline player can't
  rotate, and motion in the wrong orientation read as a bug. The static
  (correctly rotated) thumbnail shows instead.

## [1.0.0] — 2026-07-03

The biggest Fresco release yet — sound that always works, pixel-true quality
on big screens, per-display control, schedules, and an in-app wallpaper catalog.

### Fixed
- **Per-wallpaper sound is reliable now.** If Fresco started before your audio
  system on login, mpv silently dropped the audio track forever; the daemon now
  detects it and restores audio automatically (both X11 and Wayland).
- **4K/8K quality on large displays.** Correct downscaling + dithering are on
  for every quality profile: sharper 8K→4K downscales (SSIM 0.54 → 0.74 on our
  fidelity harness), no gradient banding, pixel-exact rendering verified at
  HiDPI scale 1 and 2.
- Update failures now show the actual error output, not just an exit code.
- **Rotated wallpapers no longer distort colors.** A custom chroma scaler
  combined with rotation corrupted chroma into a green cast (affected the High
  quality profile before this release too); rotated video now keeps the
  default chroma path.
- **Workspace switcher / overview now shows the ROTATED wallpaper.** The still
  frame GNOME surfaces use is generated with your rotation applied (ffmpeg).
- **Hovering a video card no longer blanks it.** The live preview swaps in
  only once the first frame is decoded; with missing codecs the thumbnail
  simply stays.

### Added
- **Wallpaper catalog**: browse curated wallpapers in-app (menu → "Browse
  wallpapers…") and set one in two clicks; license + author shown on every card.
- **Per-display wallpapers from the GUI**: right-click a wallpaper → "Set on
  <display>"; "Show default on all displays" clears overrides.
- **Day & night schedules** (Advanced): switch between two wallpapers on a
  timer; times/solar modes available via config.toml (docs/SCRIPTING.md).
- **Add from URL**: paste a direct .mp4/.webm/image link to import it.
- **X11 fullscreen auto-pause** (parity with Wayland): per-monitor pause while
  a window is fullscreen.
- Wayland: newly plugged displays are picked up on the next apply — no daemon
  restart.
- Honest status: real CPU%, renderer memory included in RSS, source
  resolution/bit-depth/dropped frames, and a warning when a ≥4K file can't be
  hardware-decoded.
- Scripting docs (docs/SCRIPTING.md) with verified copy-paste recipes.

### Verification
- New machine-proof harnesses in-tree: audio (tests/audio), visual fidelity
  (tests/fidelity), plus schedule/download/catalog unit suites — 73 tests total.

## [0.0.91] — 2026-07-02

### Added
- **Update from inside the app.** Fresco now checks GitHub for new releases on
  its own (at most once a day) and shows an "Update available" banner — click
  **Update now**, authenticate once, and the new version installs with live
  progress and a one-click restart. No more trips to the releases page. A
  manual **Check for updates** lives in the menu, and Flatpak or non-apt
  installs get a copyable install command instead.
- **Live status in the header.** A status pill shows what's playing, whether
  hardware decoding is active, and current CPU use — with a pause/resume
  button right next to it.
- **Relink broken wallpapers.** If a wallpaper's source file was moved or
  deleted, the card menu now offers "Relink…" to point it at the file's new
  home instead of removing and re-adding it.
- **About dialog and keyboard shortcuts.** Ctrl+F focuses search, Ctrl+comma
  opens the menu, Ctrl+Q quits.

### Changed
- **The window now adapts to any size.** Wallpaper cards scale fluidly with
  the window, the grid reflows from a single narrow column up to wide layouts,
  and content stays centered and readable on ultrawide and 4K displays.
- **One-step install from the website.** The landing page now leads with the
  one-line installer instead of sending visitors to browse GitHub releases.

### Fixed
- Setting a wallpaper now confirms with a toast, and launching Fresco while
  it's already open brings the existing window forward instead of opening a
  duplicate.

## [0.0.9] — 2026-06-27

### Added
- **Rotate a video or image wallpaper.** A new "Rotate 90°" button in the editor
  turns the media — fixing sideways phone photos and videos — with hardware
  decoding intact. The orientation is remembered per wallpaper.

### Fixed
- **Video wallpaper sound now works.** Setting a video from the gallery always
  re-muted it, so audio never came out unless you went through the editor every
  time. Your mute/volume choice is now remembered per wallpaper, so turning sound
  on sticks.
- **Gallery hover no longer glitches.** Hovering a video card flickered between the
  thumbnail and the inline video preview as the pointer crossed the card's
  buttons; the preview now holds steady.

### Changed
- **More reliable Wayland detection.** Fresco now probes the compositor's
  protocols directly instead of shelling out to an external tool, so live-wallpaper
  support is detected correctly even on minimal sessions.

## [0.0.8] — 2026-06-26

### Fixed
- **The wallpaper now actually restores on login.** With `autostart` enabled,
  the login-restore entry was only written when you toggled the setting in the
  app — so a default/fresh install never got one and the daemon never started
  on boot (you'd see a static still-frame until you opened the app). The daemon
  now ensures the entry exists on startup, and the entry uses an **absolute
  path** to `frescod` so it launches even when `frescod` isn't on the login PATH.
- **Cold-boot video stall self-heal (X11).** If a video isn't advancing shortly
  after login, the daemon rebuilds it automatically — what re-selecting the
  wallpaper used to do by hand.

## [0.0.7] — 2026-06-24

### Fixed
- **X11: the live wallpaper no longer comes up frozen after a reboot.** On a
  cold boot the X server and window manager could leave the wallpaper window
  not-yet-viewable when mpv started, so its display-synced video output stalled
  on the first frame and stayed static until you re-selected the wallpaper. The
  daemon now waits for the window to become viewable before embedding mpv.

### Added
- **Wayland live wallpaper support** on layer-shell compositors (COSMIC,
  Hyprland, Sway, KDE Plasma 6) via the bundled `mpvpaper` backend. The backend
  is enabled by default and supervised over mpv's IPC socket.
- **Auto-pause on fullscreen** (wlroots / KDE Plasma 6 / COSMIC): the wallpaper
  on an output pauses while a window there is fullscreen and resumes when it
  leaves, reclaiming hardware-decode cost while hidden. GNOME doesn't expose the
  protocol, so it's inactive there.
- **Event-driven update notifications**: the daemon raises a desktop prompt when
  a newer version is published, with one-click update on `.deb` installs.
- **Wayland capability probe**: when `wayland-info`/`weston-info` is installed,
  Fresco checks the registry for `zwlr_layer_shell_v1` instead of guessing from
  the desktop name.
- **Build helper** `scripts/build-mpvpaper.sh` for source builds on Wayland.

### Changed
- **GNOME Wayland** now uses the existing static-frame fallback instead of
  blocking the app; the live limitation is explained in the UI and `doctor`.
- `fresco doctor` and `frescod --check` report the detected backend capability
  and mpvpaper availability.
- The installer no longer refuses to run on Wayland; it explains live vs static
  behavior and continues.

## [0.0.3] — Theming, polish & performance

### Added
- **Theme & accent colors** — light / dark / system, with six accent palettes.
- **Right-click context menu** on library cards: Set, Edit / Crop, Rename,
  and Remove from library (deletes the entry + thumbnail, not your media file).
- **Multi-image slideshows** — pick several images (or a folder) and loop them on
  an adjustable interval (default 30s).
- **In-app feedback** (anonymous, opt-in) and **update notifications**.
- **"What's new" modal** after an update; **glass (translucent) modals**.

### Changed
- **Big memory drop.** Hardware decode auto-enabled on Intel hybrid laptops
  (auto-pins the `iHD` VA-API driver), audio fully skipped when muted, and mpv
  read-ahead caches trimmed — typical RSS dropped from ~215 MB toward ~120–150 MB.
- **~20% smaller binaries** (fat LTO, single codegen unit, `panic=abort`).
- **Simpler controls** — removed the Pause/Stop buttons; setting a wallpaper just
  runs it and picking another switches it (no more "stuck/stopped" state).

## [0.0.2] — Bug fixes

### Fixed
- **Fixed a freeze/crash when changing the wallpaper.** Re-lowering the desktop
  window in response to X11 stacking events caused an infinite restack loop that
  flooded the compositor; stacking is now handled by a periodic pass instead.
  Also, each mpv instance is now terminated *before* its window is destroyed
  (the GPU context is bound to the window), so switching wallpapers no longer
  leaks stuck decoders.
- **Add / Add Folder now work.** The native file chooser is kept alive until it
  responds, so files you pick actually register and open the editor (previously
  the portal's reply was dropped because the chooser was freed too early).
- The file picker now defaults to an **"All supported"** filter showing both
  videos and images (it was videos-only before).
- **GNOME overview, workspace switcher, and lock screen** now show a still frame
  matching the live wallpaper instead of the old desktop background. Your
  original background is saved and restored when you press Stop.

### Changed
- CI toolchain pinned to Rust 1.91 for reproducible lint results; fixed a
  clippy lint and the release workflow's smoke-test step.

## [0.0.1] — Initial release

First public release. A GUI-first live-wallpaper setter for Debian-based Linux
(Pop!_OS, Ubuntu, Mint, Debian) on X11.

### Added
- **GUI wallpaper setter** (GTK4 / libadwaita) — pick media, click Set, close
  the app; the wallpaper keeps playing via a detached daemon.
- **Wallpaper types**: looping video (mp4/webm/mkv/avi/mov), animated GIF,
  static image, auto-rotating image **slideshow**, and multi-video **playlist**.
- **Hardware-accelerated playback** via libmpv (`hwdec=auto-safe` → VA-API /
  NVDEC / VDPAU) so CPU usage stays low without degrading quality.
- **Drag-to-crop editor** — frame the exact region of a video/image, applied
  through VO-side zoom/pan so hardware decode stays zero-copy.
- **Wallpaper library** — saved wallpapers as a thumbnail grid with a recently
  used row, search, and broken-entry (missing file) badges.
- **Pause / resume** and **pause-on-battery** (no extra daemons; reads
  `/sys/class/power_supply`).
- **Restore on login** via an XDG autostart entry (toggleable).
- **Multi-monitor** support with per-connector overrides and live monitor
  hotplug handling.
- **Scaling quality** toggle (Balanced / High-Lanczos) under Advanced settings.
- **`frescod --check`** diagnostics command: session type, libmpv version,
  GPUs, VA-API availability, config validity, and live daemon status.
- **Packaging**: `.deb` built in CI and attached to GitHub Releases, a
  `curl | bash` installer, and download-count tracking via a README badge.

### Known limitations
- X11 sessions only — Wayland support is planned for a future release.
- Web/HTML wallpapers are out of scope for this release.

[0.0.9]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.8...v0.0.9
[0.0.8]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.3...v0.0.7
[0.0.3]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/DibbayajyotiRoy/fresco/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/DibbayajyotiRoy/fresco/releases/tag/v0.0.1

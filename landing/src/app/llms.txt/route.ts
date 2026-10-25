import { createLlmsTxtRoute } from "@ahtmljs/next/llms-txt";
import { ahtmlConfig, SITE_URL, GITHUB_URL, RELEASES_URL } from "@/lib/ahtml-config";

const site = SITE_URL;

export const { GET } = createLlmsTxtRoute(
  () => ({
    title: "Fresco - Live wallpapers for Linux",
    description:
      "Fresco is a free, open-source (GPL-3.0) live-wallpaper app for Linux, a native Wallpaper Engine alternative and a GUI for mpvpaper. Used by 3,000+ people in 128 countries; latest version 1.1.52. It sets video, GIF, image, slideshow, and playlist wallpapers with GPU hardware decoding (VA-API for Intel and AMD, NVDEC for NVIDIA), which moves decode off the CPU to the video engine rather than making playback free — CPU stays near idle, and measured total package power on an Intel N150 was 4.03 W at 1080p on the default Reduced level. Runs on any X11 desktop (including Deepin 25/DDE, where it adapts the desktop automatically, playing above DDE's own desktop window — because Deepin draws its wallpaper and its icons into one opaque window, clicking the desktop brings the icons back for ten seconds, tunable via dde_icon_peek_secs; community-verified by a Deepin tester on Deepin 25 Community Edition build1, X11 session, Intel Alder Lake-N graphics, and Deepin support is X11-only for now because Deepin's own Wayland compositor Treeland is still under development) and on Wayland layer-shell compositors: COSMIC (Pop!_OS 24.04), Hyprland, Sway, Cinnamon 6.8+, and KDE Plasma 6 on X11 and Wayland with the Fit option (Cover, Contain, Stretch) respected (GNOME Wayland shows a still frame only, and says so in the app: GNOME 49 disabled and GNOME 50 removed the X11 session, so Ubuntu 25.10+ including 26.04 LTS and Fedora 43+ have no Xorg session to fall back to — live video on GNOME Wayland needs a Fresco GNOME extension, which is planned, while Ubuntu 22.04/24.04 can still log into Ubuntu on Xorg; v1.1.1 ships dual mpvpaper builds with runtime probing for libmpv1/libmpv2 distros). On Xfce the desktop icons stay visible over the wallpaper and can be clicked, selected and dragged. Features: built-in catalog of curated licensed wallpapers, add-from-link (paste a video/image URL), per-display wallpapers with multi-monitor video sync, day-and-night schedules (plus time slots and solar via config), drag-to-crop and rotate editor, per-wallpaper sound, multi-select to remove many wallpapers at once, Link folder… (in the menu, or Link this folder after Add folder: use a wallpaper folder you already have, with its images, GIFs and videos, without importing it; it stays in sync as files are added and removed, Include subfolders is optional, favourites and edits are kept per file, and a missing or unplugged folder stays marked (unavailable) and comes back by itself, and Unlink folder never deletes a file), a library that stays quick with thousands of wallpapers (a 4,000-wallpaper test library opened in about 2 seconds and used about 325 MB, down from 9 to 13 seconds and 1.6 GB; development build under load), Ctrl+K command palette, `fresco next` / `fresco prev` / `fresco random` for keyboard shortcuts (an open Fresco window picks the change up instead of reverting it), pause on battery and on fullscreen, self-healing autostart/installer, scriptable JSON control socket, browser new-tab wallpaper via a companion MV3 extension for Chrome/Brave/Edge/Firefox that mirrors the desktop wallpaper over a local 127.0.0.1 bridge (extension: coming soon; available in-repo). Since v1.1.36 it also draws desktop widgets into the wallpaper itself, five as of v1.1.50 (no extra window, nothing to click through, identical on X11 and layer-shell, all off by default; the one exception is the world clock's optional Float above windows mode): time-synced song lyrics that follow any MPRIS player (four presets, sync offset, local .lrc first then LRCLIB), a clock with eight themes (Digital, Minimal, Segment, Stacked, Wordy, Card, NOS, and a phone-style Lock screen face), a world clock showing several cities at once as plain 12-hour text (new in v1.1.50), an audio visualiser with five styles (Bars, Mirror, Wave, Dots, Ring) that asks consent before listening to system audio, and the current track's album art on a turning record that stops when playback pauses. Measured at 0.8% of one CPU core with music playing and the lyrics, clock, visualiser and album-art widgets on; they come back on their own after the machine suspends and resumes; shown on every display unless a connector is named via monitor in the [widgets] block of config.toml; unavailable on GNOME Wayland, which has no live wallpaper surface (still frame only there). There are no system-monitor widgets, so it is not a full Conky replacement. Install: a .deb through the one-line install script, a manual download or the Deepin App Store, or a source build on Fedora, Arch and other distributions; it is not on the AUR or Flathub. Power saving (Full quality / Reduced / Minimum) trades image sharpness for GPU load using cheaper scalers without dropping frames or disabling hardware decode; measured with turbostat on an Intel N150, a 1080p wallpaper drops from 1.37 W to 0.63 W of GPU power (-54%) on the default Reduced level, and a 4K wallpaper from 2.77 W to 0.99 W (-65%) on Minimum.",
    sections: [
      {
        name: "Get Fresco",
        items: [
          {
            title: "Download (.deb / releases)",
            url: RELEASES_URL,
            description: "Latest .deb package and one-line installer",
          },
          {
            title: "Source on GitHub",
            url: GITHUB_URL,
            description: "Rust + GTK4 + mpv. GPL-3.0.",
          },
          {
            title: "Install guide",
            url: `${GITHUB_URL}#installation`,
            description: "Install and troubleshooting",
          },
          {
            title: "Per-distro install details",
            url: `${GITHUB_URL}/blob/main/docs/INSTALL.md`,
            description: "Source build, mpvpaper and troubleshooting",
          },
          {
            title: "Changelog",
            url: `${GITHUB_URL}/blob/main/CHANGELOG.md`,
            description: "What changed in each release",
          },
        ],
      },
      {
        name: "Compare",
        items: [
          {
            title: "Wallpaper Engine for Linux (alternative)",
            url: site + "/alternatives/wallpaper-engine-linux",
            description:
              "Native, free alternative to Wallpaper Engine on Linux",
          },
          {
            title: "Hidamari alternative",
            url: site + "/alternatives/hidamari-alternative",
            description: "Fresco vs Hidamari feature comparison",
          },
          {
            title: "Komorebi alternative",
            url: site + "/alternatives/komorebi-alternative",
            description: "Actively maintained Komorebi replacement",
          },
          {
            title: "mpvpaper GUI",
            url: site + "/alternatives/mpvpaper-gui",
            description: "Fresco as a desktop GUI for mpvpaper on Wayland",
          },
        ],
      },
      {
        name: "Machine-readable",
        items: [
          {
            title: "Site manifest",
            url: site + "/.well-known/ahtml.json",
          },
          { title: "AHTML snapshot (compact)", url: site + "/ahtml" },
          { title: "AHTML snapshot (json)", url: site + "/ahtml?fmt=json" },
          { title: "MCP tools", url: site + "/ahtml/mcp.json" },
          { title: "OpenAPI 3.1", url: site + "/ahtml/openapi.json" },
        ],
      },
    ],
    ahtml_manifest_url: site + "/.well-known/ahtml.json",
  }),
  ahtmlConfig
);

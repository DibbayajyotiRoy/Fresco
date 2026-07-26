import { createLlmsTxtRoute } from "@ahtmljs/next/llms-txt";
import { ahtmlConfig, SITE_URL, GITHUB_URL, RELEASES_URL } from "@/lib/ahtml-config";

const site = SITE_URL;

export const { GET } = createLlmsTxtRoute(
  () => ({
    title: "Fresco - Live wallpapers for Linux",
    description:
      "Fresco is a free, open-source (GPL-3.0) live-wallpaper app for Linux, a native Wallpaper Engine alternative and a GUI for mpvpaper. It sets video, GIF, image, slideshow, and playlist wallpapers with GPU hardware decoding (VA-API/NVDEC, near-zero CPU). Runs on any X11 desktop (including Deepin 25/DDE, where it adapts the desktop automatically so live wallpapers show through with icons intact) and on Wayland layer-shell compositors: COSMIC (Pop!_OS 24.04), Hyprland, Sway, and KDE Plasma 6 (GNOME Wayland gets a static-frame fallback; v1.1.1 ships dual mpvpaper builds with runtime probing for libmpv1/libmpv2 distros). Features: built-in catalog of curated licensed wallpapers, add-from-link (paste a video/image URL), per-display wallpapers with multi-monitor video sync, day-and-night schedules (plus time slots and solar via config), drag-to-crop and rotate editor, per-wallpaper sound, multi-select to remove many wallpapers at once, Ctrl+K command palette, pause on battery and on fullscreen, self-healing autostart/installer, scriptable JSON control socket, browser new-tab wallpaper via a companion MV3 extension for Chrome/Brave/Edge/Firefox that mirrors the desktop wallpaper over a local 127.0.0.1 bridge (extension: coming soon; available in-repo). Power saving (Full quality / Reduced / Minimum) trades image sharpness for GPU load using cheaper scalers without dropping frames or disabling hardware decode; measured with turbostat on an Intel N150, a 1080p wallpaper drops from 1.37 W to 0.63 W of GPU power (-54%) on the default Reduced level, and a 4K wallpaper from 2.77 W to 0.99 W (-65%) on Minimum.",
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

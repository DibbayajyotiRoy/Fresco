import { createLlmsTxtRoute } from "@ahtmljs/next/llms-txt";
import { ahtmlConfig, SITE_URL, GITHUB_URL, RELEASES_URL } from "@/lib/ahtml-config";

const site = SITE_URL;

export const { GET } = createLlmsTxtRoute(
  () => ({
    title: "Fresco - Live wallpapers for Linux",
    description:
      "Fresco is a free, open-source live-wallpaper app for Linux. Browse a built-in wallpaper catalog, set video, GIF, image, slideshow, or playlist wallpapers, put a different wallpaper on each monitor, and schedule day and night pairs. GUI-first, hardware-accelerated, on X11 and Wayland. A free Wallpaper Engine alternative.",
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

import { snapshot, computeEtag, type Snapshot } from "@ahtmljs/schema";
import {
  GITHUB_URL,
  RELEASES_URL,
  LICENSE_URL,
} from "@/lib/site";
import { SITE_URL, ISSUES_URL } from "@/lib/ahtml-config";

// --- Canonical Fresco data -------------------------------------------------
// The project does not ship a `lib/content.ts` SITE bundle, so the values the
// snapshot needs live here, sourced from `@/lib/site` (URLs) and the marketing
// components (features/formats/distros). Keep this in sync with the page copy.

const SITE = {
  author: { name: "Fresco", github: "DibbayajyotiRoy" },
  repo: GITHUB_URL,
  releases: RELEASES_URL,
  // Latest release page; the .deb and one-line installer both hang off it.
  latestRelease: RELEASES_URL,
  issues: ISSUES_URL,
  installGuide: `${GITHUB_URL}#installation`,
  licenseUrl: LICENSE_URL,
};

const FORMATS = ["video", "GIF", "image", "slideshow", "playlist"];

const DISTROS = [
  "Pop!_OS",
  "Ubuntu",
  "Linux Mint",
  "Debian",
  "elementary OS",
];

const FEATURES = [
  { title: "Video & GIF wallpapers" },
  { title: "Image slideshows with transitions" },
  { title: "Video playlists" },
  { title: "Drag-to-crop and 90-degree rotate" },
  { title: "Per-wallpaper sound and volume" },
  { title: "Works on X11 and Wayland (layer-shell)" },
  { title: "Theme & accent picker" },
  { title: "Hardware-accelerated, near-zero CPU" },
  { title: "Auto-pause on battery and on fullscreen" },
  { title: "Restores on login" },
];

export function homeSnapshot(siteUrl: string): Snapshot {
  const s = snapshot(siteUrl, "home")
    .ttl(3600)
    .add({
      id: "document:fresco-home",
      type: "document",
      title: "Fresco - Live wallpapers for Linux",
      summary:
        "Fresco is a free, open-source live-wallpaper app for Linux. It sets video, GIF, image, slideshow, and playlist wallpapers as your animated desktop background. GUI-first, hardware-accelerated via mpv (VA-API, NVDEC) so CPU stays near zero. Runs on any X11 session (Pop!_OS, Ubuntu, Linux Mint, Debian, elementary OS) and on Wayland layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6) via a bundled mpvpaper backend; GNOME Wayland uses a static-frame fallback. A free alternative to Wallpaper Engine. License GPL-3.0. Supported formats: " +
        FORMATS.join(", ") +
        ". Supported distros: " +
        DISTROS.join(", ") +
        ". Key features: " +
        FEATURES.map((f) => f.title).join(", ") +
        ".",
      language: "en",
      tags: [
        "live-wallpaper",
        "linux",
        "video-wallpaper",
        "wallpaper-engine-alternative",
        "animated-wallpaper",
        "gtk4",
        "mpv",
        "x11",
        "wayland",
        "open-source",
      ],
      author: SITE.author.name,
      canonical_url: siteUrl,
      freshness: "static",
    })
    .action(
      {
        id: "download_deb",
        label: "Download the .deb package",
        category: "read",
        method: "GET",
        execute_url: SITE.latestRelease,
        auth: "none",
        cost: { category: "free" },
      },
      {
        id: "install_script",
        label: "Install via the one-line script",
        category: "read",
        method: "GET",
        execute_url: SITE.releases,
        auth: "none",
        cost: { category: "free" },
      },
      {
        id: "view_source",
        label: "View the source on GitHub",
        category: "read",
        method: "GET",
        execute_url: SITE.repo,
        auth: "none",
        cost: { category: "free" },
      },
      {
        id: "read_install_guide",
        label: "Read the install guide",
        category: "read",
        method: "GET",
        execute_url: SITE.installGuide,
        auth: "none",
        cost: { category: "free" },
      },
      {
        id: "report_issue",
        label: "Report a bug",
        category: "read",
        method: "GET",
        execute_url: SITE.issues,
        auth: "none",
        cost: { category: "free" },
      }
    )
    .links({ canonical: siteUrl })
    .meta({ generated_by: "@ahtmljs/next" })
    .build();
  s.etag = computeEtag(s);
  return s;
}

export function allSnapshots(siteUrl: string): Snapshot[] {
  return [homeSnapshot(siteUrl)];
}

export function buildSnapshotForPath(
  segments: string[],
  _req: Request
): Snapshot | null {
  const siteUrl = SITE_URL;
  if (
    !segments ||
    segments.length === 0 ||
    (segments.length === 1 && segments[0] === "home")
  ) {
    return homeSnapshot(siteUrl);
  }
  return null;
}

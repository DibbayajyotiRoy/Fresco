/**
 * Shared marketing content reused by both the UI components and the JSON-LD
 * structured data, so on-page copy and machine-readable data never drift.
 *
 * Copy rules: answer-first (for AI answer engines), fact-dense, NO em-dashes.
 * Keep in sync with ../../../CHANGELOG.md and ../../../README.md.
 */

/** Q&A written from real user phrasings (AskUbuntu, Mint forums, Reddit, HN). */
export const FAQ: { q: string; a: string }[] = [
  {
    q: "Is there a Wallpaper Engine for Linux?",
    a: "Yes. Fresco is a free, open-source live-wallpaper app for Linux that works like Wallpaper Engine: pick a video, GIF, or image and set it as your animated desktop background. It is GUI-first and needs no Steam or Proton.",
  },
  {
    q: "How do I set a video as my wallpaper on Ubuntu or Pop!_OS?",
    a: "Install the Fresco .deb, open it from your app menu, click Add, choose your video, optionally crop or rotate it, then click Set as Wallpaper. Close the app and the video keeps playing as your desktop background.",
  },
  {
    q: "Will a video wallpaper drain my CPU or battery?",
    a: "No. Fresco decodes video on the GPU through mpv (VA-API and NVDEC), so CPU usage stays near zero and memory sits around 120 to 150 MB. It can pause automatically while you are on battery, and it auto-pauses on any monitor that has a fullscreen window.",
  },
  {
    q: "Does it work on Wayland or GNOME?",
    a: "Both, with one caveat. Fresco runs on any X11 session (Pop!_OS, Ubuntu, Linux Mint, Debian, elementary OS) and on Wayland layer-shell compositors through a bundled mpvpaper backend, verified on Sway. COSMIC, Hyprland, and KDE Plasma 6 are experimental while real-session verification lands. GNOME on Wayland shows a static frame instead, because Mutter does not expose a live wallpaper surface.",
  },
  {
    q: "Can a video wallpaper play sound?",
    a: "Yes. Each wallpaper remembers its own mute state and volume, so you can unmute one specific video and the choice sticks every time it is set. Wallpapers start muted by default.",
  },
  {
    q: "Can I crop or rotate a wallpaper?",
    a: "Yes. The editor has a drag-to-crop frame and a 90-degree rotate, so you can pick the exact region or turn a sideways phone video upright. Both are applied on the GPU and remembered per wallpaper.",
  },
  {
    q: "Will the wallpaper stay after I reboot?",
    a: "Yes. Fresco adds an autostart entry that restores your live wallpaper automatically on login, and self-heals the entry if it is missing. You can turn this off in settings.",
  },
  {
    q: "What media formats are supported?",
    a: "Looping video (mp4, webm, mkv, avi, mov), animated GIFs, static images (jpg, png, webp), a folder of images as a slideshow with crossfade, fade, slide, or Ken Burns transitions, and multi-video playlists.",
  },
  {
    q: "Does it support multiple monitors?",
    a: "Yes. You can set a different wallpaper on each display, and Fresco pauses the wallpaper per output when a window there goes fullscreen. Monitor hotplug is live on X11; on Wayland a newly plugged display picks up on the next apply (automatic hotplug lands with the v1.0 engine).",
  },
  {
    q: "How is Fresco different from Hidamari, Komorebi, and mpvpaper?",
    a: "Fresco is GUI-first, hardware-accelerated, and handles video, GIF, image, slideshow, and playlist wallpapers in one app, on both X11 and Wayland. It is actively maintained, unlike Komorebi, and needs no command line, unlike mpvpaper.",
  },
  {
    q: "Where do I find live wallpapers for Linux?",
    a: "Inside Fresco itself. The built-in catalog (menu, then Browse wallpapers) offers curated, properly licensed video wallpapers you can set in two clicks, with the license and author shown on every item. You can also paste a direct video or image URL, or add your own files.",
  },
  {
    q: "Can my wallpaper change automatically between day and night?",
    a: "Yes. Open the menu, choose Advanced, then Day & night wallpaper: pick two wallpapers and switch times, and the daemon swaps them automatically with no restart. Arbitrary time slots and sunrise or sunset switching (with manual coordinates) are available through config.toml.",
  },
  {
    q: "How do I set a different wallpaper on each monitor?",
    a: "Right-click any wallpaper in the library and choose Set on a specific display. Each connected monitor is listed with its resolution. Choosing Show default on all displays clears the per-monitor overrides.",
  },
  {
    q: "Is Fresco free?",
    a: "Yes. Fresco is completely free and open source under the GPL-3.0 license. There is no paid tier.",
  },
];

/** Install steps, shown in the How-it-works section. */
export const INSTALL_STEPS: { name: string; text: string }[] = [
  {
    name: "Download Fresco",
    text: "Download the latest .deb package from the Fresco releases page, or run the one-line install script.",
  },
  {
    name: "Install it",
    text: "Double-click the .deb to install it, or run sudo apt install ./fresco_*.deb in a terminal.",
  },
  {
    name: "Set your wallpaper",
    text: "Open Fresco, click Add, pick a video or image, optionally crop it, and click Set as Wallpaper.",
  },
];

/** Highlights from the 1.0 release, for the What's New section. */
export const WHATS_NEW: { icon: string; title: string; body: string }[] = [
  {
    icon: "catalog",
    title: "Built-in wallpaper catalog",
    body: "Browse curated, licensed wallpapers in-app and set one in two clicks. License and author on every card.",
  },
  {
    icon: "displays",
    title: "Per-display wallpapers",
    body: "Right-click a wallpaper and set it on one specific monitor. Each display can run its own.",
  },
  {
    icon: "schedule",
    title: "Day and night schedules",
    body: "Two wallpapers, two switch times. The daemon swaps them automatically, no restart needed.",
  },
  {
    icon: "quality",
    title: "Measured picture quality",
    body: "Sharper 8K to 4K downscaling, zero banding, pixel-exact HiDPI. Verified by an in-tree fidelity harness.",
  },
];

/** Feature names, used for the SoftwareApplication featureList in JSON-LD. */
export const FEATURE_LIST = [
  "Built-in catalog of curated, licensed wallpapers",
  "Video, GIF, image, slideshow, and playlist wallpapers",
  "Add wallpapers from a direct URL",
  "Day and night wallpaper schedules (plus time slots and solar via config)",
  "Per-display wallpapers from the GUI",
  "Automatic audio recovery when the sound server starts late",
  "Scriptable JSON control socket",
  "Hardware-accelerated playback (VA-API, NVDEC)",
  "Works on X11 and Wayland layer-shell compositors",
  "Drag-to-crop and 90-degree rotate editor",
  "Per-wallpaper sound and volume",
  "Slideshow transitions (crossfade, fade, slide, Ken Burns)",
  "Wallpaper library with search",
  "Different wallpaper per monitor",
  "Pause on battery and auto-pause on fullscreen",
  "Restores automatically on login",
  "Themes and accent colors",
];

export const FORMATS = [
  "mp4",
  "webm",
  "mkv",
  "avi",
  "mov",
  "GIF",
  "jpg",
  "png",
  "webp",
  "image slideshow",
  "video playlist",
];

export const DISTROS = [
  "Pop!_OS 22.04",
  "Pop!_OS 24.04 (COSMIC, experimental)",
  "Ubuntu 22.04 / 24.04",
  "Linux Mint 21 / 22",
  "Debian 12",
  "elementary OS 7",
];

/**
 * Competitor comparison. Cells: true (yes), false (no), or a short qualifier.
 * Sourced from README.md. Komorebi is unmaintained; Wallpaper Engine is a
 * paid, Windows-first product.
 */
export type CompareCell = boolean | string;
export const COMPARISON: {
  tools: string[];
  note: string;
  rows: { label: string; values: CompareCell[] }[];
} = {
  tools: ["Fresco", "Hidamari", "Komorebi", "mpvpaper", "Wallpaper Engine"],
  note: "Wallpaper Engine is a paid, Windows-first product. Komorebi is no longer maintained.",
  rows: [
    { label: "GUI app, no terminal", values: [true, true, true, false, true] },
    { label: "Works on X11", values: [true, true, true, false, "Compositor off"] },
    { label: "Works on Wayland (layer-shell)", values: [true, "Partial", false, true, false] },
    { label: "Hardware decode, low CPU", values: [true, "Partial", "Partial", true, true] },
    { label: "Drag-to-crop and rotate", values: [true, false, false, false, "Crop only"] },
    { label: "Playlists", values: [true, false, false, "Manual", true] },
    { label: "Image slideshow", values: [true, false, false, false, true] },
    { label: "Wallpaper library", values: [true, false, false, false, true] },
    { label: "Built-in wallpaper catalog", values: [true, false, false, false, "Workshop"] },
    { label: "Per-display wallpapers (GUI)", values: [true, false, false, "Manual", true] },
    { label: "Day and night schedules", values: [true, false, false, false, "Partial"] },
    { label: "Actively maintained", values: [true, true, false, true, true] },
    { label: "Free and open source", values: [true, true, true, true, false] },
  ],
};

/** Author / maintainer, used in JSON-LD trust signals. */
export const AUTHOR = {
  name: "Dibbayajyoti Roy",
  github: "https://github.com/DibbayajyotiRoy",
  portfolio: "https://dibbayajyoti.com",
};

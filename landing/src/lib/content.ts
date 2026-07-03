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
    a: "Both, with one caveat. Fresco runs on any X11 session (Pop!_OS, Ubuntu, Linux Mint, Debian, elementary OS) and on Wayland layer-shell compositors through a bundled mpvpaper backend — verified on Sway; COSMIC, Hyprland, and KDE Plasma 6 are experimental while real-session verification lands. GNOME on Wayland shows a static frame instead, because Mutter does not expose a live wallpaper surface.",
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
    q: "Is Fresco free?",
    a: "Yes. Fresco is completely free and open source under the GPL-3.0 license. There is no paid tier.",
  },
];

/** Install steps, reused by the HowTo JSON-LD. */
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

/** Highlights from the latest releases (0.0.7 to 0.0.9), for the What's New section. */
export const WHATS_NEW: { icon: string; title: string; body: string }[] = [
  {
    icon: "wayland",
    title: "Wayland live wallpapers",
    body: "Live video via a bundled mpvpaper backend — verified on Sway; COSMIC, Hyprland, and KDE Plasma 6 are experimental.",
  },
  {
    icon: "audio",
    title: "Per-wallpaper sound",
    body: "Unmute a video and set its volume. Fresco remembers the choice for that wallpaper.",
  },
  {
    icon: "rotate",
    title: "Rotate 90 degrees",
    body: "Turn a sideways phone video or photo upright, with hardware decoding intact.",
  },
  {
    icon: "pause",
    title: "Fullscreen auto-pause",
    body: "The wallpaper pauses on any monitor with a fullscreen window and resumes when it leaves.",
  },
];

/** Feature names, used for the SoftwareApplication featureList in JSON-LD. */
export const FEATURE_LIST = [
  "Video, GIF, image, slideshow, and playlist wallpapers",
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

/**
 * Dedicated "<competitor> alternative" landing pages. Each targets a real
 * search query already in our keyword set (wallpaper engine linux, hidamari
 * alternative, komorebi alternative, mpvpaper gui) with UNIQUE, accurate copy,
 * so they are not thin duplicates of one another or of the homepage.
 *
 * Copy rules match the homepage: answer-first, fact-dense, NO em-dashes.
 * The `tool` field maps to a column in COMPARISON.tools so the comparison
 * table can render Fresco vs that one competitor.
 */

export type Alternative = {
  slug: string;
  /** Competitor display name, matches a COMPARISON.tools entry. */
  tool: string;
  metaTitle: string;
  metaDescription: string;
  h1: string;
  /** Answer-first lead, also used as the page summary in JSON-LD. */
  lead: string;
  /** Two to three unique paragraphs of context. */
  body: string[];
  /** Concrete reasons to switch, rendered as a small grid. */
  reasons: { title: string; body: string }[];
  /** Competitor-specific FAQ, distinct from the homepage FAQ. */
  faq: { q: string; a: string }[];
};

export const ALTERNATIVES: Alternative[] = [
  {
    slug: "wallpaper-engine-linux",
    tool: "Wallpaper Engine",
    metaTitle: "Wallpaper Engine for Linux | Free Native Alternative (Fresco)",
    metaDescription:
      "Want Wallpaper Engine on Linux? Fresco is a free, native, open-source alternative: set video, GIF, and image wallpapers on X11 and Wayland with near-zero CPU.",
    h1: "Wallpaper Engine for Linux, done natively.",
    lead: "Wallpaper Engine is a paid, Windows-first app. On Linux it only runs through Steam Play and Proton, with no real desktop integration. Fresco is the native, free, open-source alternative: pick a video, GIF, or image, click Set, and it plays as your desktop background on X11 and Wayland.",
    body: [
      "If you came to Linux from Windows, Wallpaper Engine is probably what you miss. The community workarounds run it under Proton through a third-party launcher, but you are emulating a Windows app to paint your Linux desktop, and it cannot touch your real wallpaper layer, multi-monitor setup, or login session cleanly.",
      "Fresco does the same job the way a Linux app should. It is a GTK4 desktop app that hands your media to mpv for hardware-accelerated playback (VA-API or NVDEC), so a 4K video wallpaper costs about as much CPU as a static image. You set it, close the app, and a lightweight daemon keeps it running and restores it on login.",
      "You give up the Steam Workshop scene catalog, but you get a tool that is free, open source under the GPL, and built for the Linux desktop instead of bolted onto it.",
    ],
    reasons: [
      {
        title: "Free and native",
        body: "No Steam, no Proton, no license. Install a .deb or build from source. GPL-3.0.",
      },
      {
        title: "Real desktop integration",
        body: "A proper wallpaper layer per monitor on X11 and Wayland, restored on login.",
      },
      {
        title: "Near-zero CPU",
        body: "GPU decoding through mpv keeps a video wallpaper as cheap as a still image.",
      },
    ],
    faq: [
      {
        q: "Can I run Wallpaper Engine on Linux?",
        a: "Only indirectly, through Steam Play and Proton with a community launcher, and it cannot integrate with the native Linux wallpaper layer. Fresco is a native alternative that needs neither Steam nor Proton.",
      },
      {
        q: "Is there a free alternative to Wallpaper Engine for Linux?",
        a: "Yes. Fresco is completely free and open source under the GPL-3.0 license, with video, GIF, image, slideshow, and playlist wallpapers and hardware-accelerated playback.",
      },
      {
        q: "Does the Fresco alternative support the Steam Workshop?",
        a: "No. Fresco plays your own media files (video, GIF, image, folders, and playlists). It does not browse the Wallpaper Engine Steam Workshop.",
      },
    ],
  },
  {
    slug: "hidamari-alternative",
    tool: "Hidamari",
    metaTitle: "Hidamari Alternative for Linux Live Wallpapers | Fresco",
    metaDescription:
      "Looking for a Hidamari alternative? Fresco adds hardware mpv decoding, a crop and rotate editor, a wallpaper library, playlists, and Wayland layer-shell support.",
    h1: "A Hidamari alternative with more under the hood.",
    lead: "Hidamari is a friendly GTK app for video wallpapers on GNOME. Fresco covers the same ground and adds hardware-accelerated mpv playback, a crop and rotate editor, a searchable wallpaper library, playlists, slideshow transitions, and a Wayland layer-shell backend.",
    body: [
      "Hidamari is a solid, GNOME-focused choice for looping a video on your desktop. Where it stops is where Fresco keeps going: Fresco decodes through mpv with VA-API or NVDEC for genuinely low CPU, and it treats wallpapers as a managed library with thumbnails, a recently-used row, and search rather than a single current file.",
      "Fresco also adds editing and variety. You can drag a crop frame, rotate a sideways clip 90 degrees, set per-wallpaper sound and volume, run a folder of images as a slideshow with crossfade or Ken Burns transitions, or queue several clips as a playlist. On multi-monitor setups you can give each display its own wallpaper.",
      "Both are free and open source, so trying Fresco costs nothing. If Hidamari already does what you need on GNOME, keep it; if you want more control and lower CPU, Fresco is the step up.",
    ],
    reasons: [
      {
        title: "Hardware mpv decoding",
        body: "VA-API and NVDEC keep CPU near zero, instead of heavier software playback.",
      },
      {
        title: "Crop, rotate, library",
        body: "Frame a region, fix sideways clips, and manage saved wallpapers with search.",
      },
      {
        title: "X11 and Wayland",
        body: "Runs on X11 and on layer-shell compositors like Hyprland, Sway, and KDE Plasma 6.",
      },
    ],
    faq: [
      {
        q: "How is Fresco different from Hidamari?",
        a: "Fresco adds hardware-accelerated mpv decoding, a crop and rotate editor, a searchable wallpaper library, playlists, slideshow transitions, per-wallpaper audio, and a Wayland layer-shell backend, while remaining a simple GUI app.",
      },
      {
        q: "Does the Hidamari alternative work outside GNOME?",
        a: "Yes. Fresco runs on any X11 desktop and on Wayland layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6). GNOME on Wayland uses a static-frame fallback.",
      },
      {
        q: "Is Fresco free like Hidamari?",
        a: "Yes. Fresco is free and open source under the GPL-3.0 license, with no paid tier.",
      },
    ],
  },
  {
    slug: "komorebi-alternative",
    tool: "Komorebi",
    metaTitle: "Komorebi Alternative (Maintained) for Linux Wallpapers | Fresco",
    metaDescription:
      "Komorebi is no longer maintained. Fresco is an actively developed alternative for live video, GIF, and image wallpapers on Linux, with hardware-accelerated playback.",
    h1: "A maintained Komorebi alternative.",
    lead: "Komorebi was a popular animated-wallpaper app, but it is no longer actively maintained and breaks on newer distributions. Fresco is an actively developed alternative for live video, GIF, image, slideshow, and playlist wallpapers, with hardware-accelerated playback on X11 and Wayland.",
    body: [
      "Komorebi introduced a lot of people to live wallpapers on Linux with its video and parallax backgrounds. The problem now is maintenance: development has stalled, and getting it running on a current Ubuntu, Pop!_OS, or Debian release can mean fighting old dependencies.",
      "Fresco is built and released today. It ships as a .deb with a one-line installer, decodes video on the GPU through mpv for low CPU, and keeps your wallpaper alive through a small daemon that restores it on login. It adds a crop and rotate editor, a wallpaper library with search, per-wallpaper sound, and a Wayland layer-shell backend that Komorebi never had.",
      "Both are free and open source, so there is no cost to moving. If you liked Komorebi but it no longer runs cleanly, Fresco is the actively-maintained place to land.",
    ],
    reasons: [
      {
        title: "Actively maintained",
        body: "Regular releases that install cleanly on current Ubuntu, Pop!_OS, Mint, and Debian.",
      },
      {
        title: "Modern playback",
        body: "Hardware mpv decoding (VA-API, NVDEC) for low CPU, with video, GIF, and image support.",
      },
      {
        title: "Wayland ready",
        body: "A layer-shell backend for COSMIC, Hyprland, Sway, and KDE Plasma 6.",
      },
    ],
    faq: [
      {
        q: "Is Komorebi still maintained?",
        a: "Komorebi is no longer actively maintained and can be hard to run on current distributions. Fresco is an actively developed alternative.",
      },
      {
        q: "What is a good replacement for Komorebi?",
        a: "Fresco. It plays video, GIF, image, slideshow, and playlist wallpapers with hardware acceleration, installs as a .deb, and works on X11 and Wayland.",
      },
      {
        q: "Will my live wallpaper survive a reboot in Fresco?",
        a: "Yes. Fresco adds an autostart entry that restores your live wallpaper on login and self-heals the entry if it goes missing.",
      },
    ],
  },
  {
    slug: "mpvpaper-gui",
    tool: "mpvpaper",
    metaTitle: "mpvpaper GUI for Linux Video Wallpapers | Fresco",
    metaDescription:
      "Want a GUI for mpvpaper? Fresco gives mpvpaper a friendly app: pick media, crop, rotate, and manage wallpapers, with mpvpaper bundled as the Wayland backend.",
    h1: "mpvpaper, with a GUI in front of it.",
    lead: "mpvpaper is a powerful command-line tool that paints an mpv video onto your Wayland wallpaper. Fresco gives it a friendly desktop app: pick media, crop, rotate, set sound, and manage a library, with mpvpaper bundled and supervised as Fresco's Wayland backend.",
    body: [
      "If you already use mpvpaper, you know it works, and you know the cost: every wallpaper is a shell command with output names, mpv flags, and a process to babysit per monitor. There is no library, no editor, and no GUI.",
      "Fresco wraps that workflow in a GTK4 app. On Wayland layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6) it actually bundles mpvpaper and steers it over mpv's IPC socket, so you keep mpvpaper's playback while gaining a real interface: a searchable wallpaper library, a drag-to-crop and rotate editor, per-wallpaper volume, slideshows, playlists, and per-monitor wallpapers. On X11 it uses its own desktop-window backend instead.",
      "It is the same engine you trust, with the manual parts removed. Both are free and open source.",
    ],
    reasons: [
      {
        title: "A real GUI",
        body: "Pick media and set it in a click. No output names, mpv flags, or processes to manage.",
      },
      {
        title: "mpvpaper under the hood",
        body: "On Wayland, Fresco bundles and supervises mpvpaper over mpv's IPC socket.",
      },
      {
        title: "Library and editor",
        body: "Searchable saved wallpapers, drag-to-crop, 90-degree rotate, and per-wallpaper sound.",
      },
    ],
    faq: [
      {
        q: "Is there a GUI for mpvpaper?",
        a: "Yes. Fresco is a GTK4 desktop app that uses mpvpaper as its bundled Wayland backend, so you get mpvpaper playback with a real interface instead of the command line.",
      },
      {
        q: "Does Fresco replace mpvpaper or use it?",
        a: "Both, depending on session. On Wayland layer-shell compositors Fresco bundles and drives mpvpaper; on X11 it uses its own desktop-window backend.",
      },
      {
        q: "Do I still need to edit config files?",
        a: "No. Fresco manages playback, cropping, rotation, audio, and per-monitor wallpapers from the app, with no manual mpv or mpvpaper configuration.",
      },
    ],
  },
];

export function getAlternative(slug: string): Alternative | undefined {
  return ALTERNATIVES.find((a) => a.slug === slug);
}

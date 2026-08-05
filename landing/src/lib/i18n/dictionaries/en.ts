/**
 * English source dictionary. This file defines the `Dictionary` type that
 * every other locale must satisfy, so a missing or renamed key is a compile
 * error rather than a blank spot on the page.
 *
 * Copy rules (all locales): answer-first, fact-dense, NO em-dashes.
 * Product nouns stay untranslated everywhere: Fresco, Wallpaper Engine,
 * Hidamari, Komorebi, mpvpaper, mpv, Conky, X11, Wayland, layer-shell,
 * COSMIC, Hyprland, Sway, KDE Plasma 6, GNOME, Mutter, MPRIS, LRCLIB,
 * VA-API, NVDEC, GTK4, Rust, GPL-3.0, .deb, .lrc, config.toml.
 * Keep in sync with ../../../../../CHANGELOG.md and README.md.
 */
export const en = {
  meta: {
    title:
      "Fresco - Live Wallpaper for Linux | Free Wallpaper Engine Alternative",
    description:
      "Free, open-source live wallpaper app for Linux. Browse a built-in wallpaper catalog, set videos or GIFs as your desktop, per-monitor wallpapers, day and night schedules. Hardware-accelerated on X11 and Wayland.",
    ogTitle: "Fresco - Live Wallpapers for Linux",
    ogDescription:
      "Built-in wallpaper catalog, per-monitor wallpapers, day and night schedules, and hardware-accelerated near-zero-CPU playback on X11 and Wayland. A free Wallpaper Engine alternative.",
    twitterDescription:
      "Hardware-accelerated live wallpapers for Linux, on X11 and Wayland. A free, open-source Wallpaper Engine alternative.",
    ogImageAlt: "Fresco. Finally, a Linux wallpaper that just works.",
    /** Locale-specific search terms, appended to the shared English set. */
    keywords: [
      "live wallpaper linux",
      "video wallpaper linux",
      "animated wallpaper ubuntu",
      "wallpaper engine linux alternative",
    ],
  },

  nav: {
    home: "Fresco home",
    features: "Features",
    compare: "Compare",
    whatsNew: "What's new",
    download: "Download",
    cta: "Get Fresco",
    star: "Star Fresco on GitHub",
    starWithCount: (n: string) => `Star Fresco on GitHub (${n} stars)`,
  },

  language: {
    label: "Language",
    change: "Change language",
  },

  theme: {
    toggle: "Toggle theme",
    light: "Light",
    dark: "Dark",
  },

  hero: {
    titleLead: "Finally, a Linux wallpaper",
    /** Separator before the accented tail; empty where CJK needs none. */
    titleGap: " ",
    titleEm: "that just works.",
    body: "Set any video, GIF, or image as your Linux desktop. Hardware-accelerated playback keeps CPU near zero, on X11 and Wayland. Close the app; the daemon keeps it playing.",
    install: "Install Fresco",
    star: "Star on GitHub",
  },

  stats: {
    ariaLabel: "Project stats",
    downloads: "total downloads",
    downloadsUnknown: "downloads on github",
    stars: "github stars",
    version: "latest release",
    license: "free and open source",
  },

  glance: {
    ariaLabel: "Fresco at a glance",
    caption: "fresco at a glance",
    labelWhat: "what it is",
    labelPlatforms: "platforms",
    labelWidgets: "desktop widgets",
    labelLicense: "license",
    labelInstall: "install",
    what: "Fresco is a free, open-source live wallpaper app for Linux: it sets video, GIF, image, slideshow, and playlist wallpapers as your animated desktop background, with GPU hardware decoding. A free Wallpaper Engine alternative and a GUI for mpvpaper on Wayland.",
    platforms:
      "Any X11 desktop (Ubuntu, Pop!_OS, Linux Mint, Debian), plus Wayland layer-shell compositors: COSMIC, Hyprland, Sway, KDE Plasma 6. GNOME Wayland falls back to a static frame.",
    widgets:
      "Four widgets painted into the wallpaper itself, not into a window: time-synced song lyrics, a clock with six themes, an audio visualiser, and album art on a turning record. Nothing floats over your windows and nothing intercepts a click. All off by default; unavailable on GNOME Wayland, which has no live wallpaper surface.",
    licenseLead: "GPL-3.0, free forever.",
    licenseLink: "Source on GitHub",
    licenseTail: "Built with Rust, GTK4, and mpv.",
  },

  features: {
    kicker: "features",
    title: "Any media. Any monitor. No CPU drama.",
    lead: "Fresco sets video, GIF, image, slideshow, and playlist wallpapers on X11 and Wayland, decoded on the GPU so a live wallpaper costs about as much as a static one. The full spec sheet:",
    manifest: (n: number) => `manifest: ${n} capabilities`,
    /** Sentence-final mark after each row title. */
    titleSuffix: ".",
    thCapability: "Capability",
    thWhatYouGet: "What you get",
    thStatus: "Status",
    footnote:
      "gnome wayland: static-frame fallback (mutter exposes no live surface), and widgets need that surface too, so they are unavailable there. everything else above is live.",
    tally: (shipping: number, total: number, soon: number) =>
      `${shipping} of ${total} shipping · ${soon} in-preview · 0 deprecated`,
    rows: {
      hwDecode: {
        tag: "hw decode",
        title: "Hardware-accelerated playback",
        description:
          "Decoding runs on the GPU through mpv (VA-API or NVDEC). A 4K video wallpaper costs about as much CPU as a static image.",
        status: "near-zero cpu",
      },
      sessions: {
        tag: "sessions",
        title: "X11 and Wayland",
        description:
          "A desktop-window backend on any X11 desktop, plus a layer-shell backend for COSMIC, Hyprland, Sway, and KDE Plasma 6. GNOME Wayland gets a static-frame fallback.",
        status: "x11 · layer-shell",
      },
      catalog: {
        tag: "catalog",
        title: "Built-in wallpaper catalog",
        description:
          "Browse curated, licensed wallpapers in-app (menu, then Browse wallpapers) and set one in two clicks. You can also paste a direct link.",
        status: "in-app",
      },
      video: {
        tag: "video · gif",
        title: "Video & GIF wallpapers",
        description: "Loop any mp4, webm, mkv, or animated GIF as your desktop.",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "slideshow",
        title: "Slideshows with transitions",
        description:
          "Rotate a folder of images with crossfade, fade, or Ken Burns.",
        status: "4 transitions",
      },
      playlist: {
        tag: "playlist",
        title: "Video playlists",
        description: "Queue several clips and let Fresco cycle through them.",
        status: "auto-cycle",
      },
      lyrics: {
        tag: "lyrics · clock",
        title: "Lyrics and clock widgets",
        description:
          "Time-synced song lyrics for whatever is playing over MPRIS (local .lrc first, then LRCLIB), and a clock in one of six themes. Drawn into the wallpaper, so nothing floats over your windows. Off by default.",
        status: "off by default",
      },
      visualiser: {
        tag: "visualiser",
        title: "Audio visualiser and album art",
        description:
          "Five visualiser styles (Bars, Mirror, Wave, Dots, Ring) with a colour picker, blend, or rainbow, plus the current track's cover on a turning record. The visualiser asks before it listens to your audio.",
        status: "0.8% of one core",
      },
      editor: {
        tag: "editor",
        title: "Crop and rotate",
        description:
          "Drag a frame to pick the region, rotate 90 degrees to fix sideways clips. Both stay zero-copy on the GPU.",
        status: "zero-copy",
      },
      audio: {
        tag: "audio",
        title: "Per-wallpaper sound",
        description:
          "Unmute a video and set its volume. Fresco remembers the choice for that wallpaper.",
        status: "per-wallpaper",
      },
      displays: {
        tag: "displays",
        title: "Per-display wallpapers",
        description:
          "Right-click any wallpaper and Set on a specific display. Each monitor can run its own.",
        status: "per-monitor",
      },
      schedule: {
        tag: "schedule",
        title: "Day and night schedules",
        description:
          "Two wallpapers, two switch times, swapped automatically by the daemon. Time slots and solar switching via config.",
        status: "automatic",
      },
      power: {
        tag: "power",
        title: "Power-aware",
        description:
          "Pause on battery, and auto-pause per monitor when a window there goes fullscreen.",
        status: "auto-pause",
      },
      newTab: {
        tag: "browser new tab",
        title: "Your wallpaper on every new tab",
        description:
          "A companion browser extension (Chrome, Brave, Edge, Firefox) mirrors your desktop wallpaper, or a browser-specific pick, on the new-tab page via a local bridge that talks only to 127.0.0.1. In the repo today; store listings pending.",
        status: "coming soon",
      },
      themes: {
        tag: "themes",
        title: "Themes and accents",
        description: "Light, dark, or follow the system, with six accent palettes.",
        status: "6 palettes",
      },
    },
  },

  compare: {
    kicker: "compare",
    title: "Fresco vs the Linux wallpaper field.",
    lead: "Fresco is the only actively maintained Linux live-wallpaper app in this table that combines a GUI, hardware decoding, X11 and Wayland support, and a built-in catalog, free. Here is the full comparison with Hidamari, Komorebi, mpvpaper, and Wallpaper Engine.",
    meter: (tools: number, caps: number) =>
      `compare · ${tools} tools · ${caps} capabilities`,
    thFeature: "Feature",
    yes: "Yes",
    no: "No",
    note: "Wallpaper Engine is a paid, Windows-first product. Komorebi is no longer maintained.",
    detailLabel: "Compare in detail:",
    vs: (tool: string) => `Fresco vs ${tool}`,
    rows: {
      gui: "GUI app, no terminal",
      x11: "Works on X11",
      wayland: "Works on Wayland (layer-shell)",
      hwDecode: "Hardware decode, low CPU",
      cropRotate: "Drag-to-crop and rotate",
      playlists: "Playlists",
      slideshow: "Image slideshow",
      library: "Wallpaper library",
      catalog: "Built-in wallpaper catalog",
      perDisplay: "Per-display wallpapers (GUI)",
      schedules: "Day and night schedules",
      maintained: "Actively maintained",
      foss: "Free and open source",
    },
    cells: {
      partial: "Partial",
      manual: "Manual",
      compositorOff: "Compositor off",
      cropOnly: "Crop only",
      workshop: "Workshop",
    },
  },

  whatsNew: {
    kicker: (version: string) => `what's new · v${version}`,
    title: "Four desktop widgets, painted into the wallpaper.",
    lead: (version: string) =>
      `What shipped in v${version}. No extra window, nothing to click through, identical on X11 and layer-shell. All four are off by default, and with music playing and every one of them on the measured cost was 0.8% of one CPU core. Each entry here is reproduced in the CHANGELOG on GitHub.`,
    changelog: "Full changelog",
    patch: (n: string) => `patch ${n}`,
    items: {
      lyrics: {
        title: "Synced song lyrics",
        body: "The current line, in time with whatever is playing over MPRIS. Local .lrc files first, then LRCLIB. Four presets and a sync offset.",
      },
      clock: {
        title: "Clock, six themes",
        body: "Digital, Minimal, Segment, Stacked, Wordy, and Card, a translucent panel with a drawn analog face. 12 or 24-hour, optional date.",
      },
      visualizer: {
        title: "Audio visualiser",
        body: "Bars, Mirror, Wave, Dots, or Ring, with a colour picker, a two-colour blend, or rainbow. Asks before it listens to your audio.",
      },
      disc: {
        title: "Album art on a record",
        body: "The current track's cover on a turning disc. It stops turning the moment playback pauses.",
      },
    },
  },

  howItWorks: {
    kicker: "how it works",
    title: "Three clicks, then forget about it.",
    lead: "Open Fresco, click add, click set, close. The daemon keeps the wallpaper running, even after you reboot.",
    step: (n: string) => `step ${n}`,
    steps: {
      pick: {
        title: "Pick your media",
        description:
          "Open Fresco from your app menu and choose a video, GIF, image, folder, or playlist.",
      },
      set: {
        title: "Click Set",
        description:
          "Set it as your wallpaper. It starts playing on your desktop right away.",
      },
      close: {
        title: "Close the app",
        description:
          "Quit the window. A lightweight daemon keeps the wallpaper running, even after a reboot.",
      },
    },
  },

  videos: {
    kicker: "watch it run",
    title: "Under a minute each. No narration required.",
    lead: "Short screen recordings of Fresco on a real desktop. Nothing loads from YouTube until you press play.",
    more: "More on YouTube",
    inDevelopment: "in development",
    play: (title: string) => `Play: ${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "add from link",
        blurb:
          "Copy a Pinterest link, paste it into Fresco, set it as your wallpaper. No download step, no file juggling.",
      },
      C1MqrhGkovQ: {
        tag: "lyrics widgets",
        blurb:
          "Synchronized lyrics and a clock drawn into a live wallpaper on Wayland and X11. Shipped in v1.1.36, along with an audio visualiser and an album-art disc.",
      },
    },
  },

  supported: {
    kicker: "deployed environments",
    title: "Where Fresco runs.",
    lead: "On any X11 desktop, including Deepin 25's DDE, and on Wayland layer-shell compositors (COSMIC, Hyprland, Sway, and KDE Plasma 6) across the popular Debian and Ubuntu distributions. GNOME Wayland gets a static-frame fallback.",
    deployed: (distros: number, formats: number) =>
      `deployed: 6 live compositors · 1 static fallback · ${distros} distros · ${formats} formats`,
    sessionsTitle: "sessions and compositors",
    distrosTitle: (n: number) => `tested distributions · ${n}`,
    formatsTitle: (n: number) => `supported formats · ${n}`,
    live: "Live wallpaper",
    fallback: "Static fallback",
    sessions: {
      x11: {
        label: "X11 (any desktop)",
        detail: "GNOME, KDE, XFCE, MATE, Cinnamon, Budgie",
      },
      deepin: {
        label: "Deepin 25 (DDE, X11)",
        detail:
          "Automatic DDE adaptation, icons stay visible. Community-verified on Deepin 25 Community build1.",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC, Hyprland, Sway, KDE Plasma 6, wlroots",
      },
      gnome: {
        label: "GNOME on Wayland",
        detail: "Static-frame fallback (Mutter has no live surface)",
      },
    },
    fieldReport: "field report · deepin 25",
    verifiedEnv: "verified environment",
    testimonialRole: "Deepin community tester",
    envLabels: {
      session: "session",
      os: "os",
      gpu: "gpu",
    },
    footnote:
      "deepin 25 defaults to x11, and that is the session fresco is verified on there. deepin's own wayland compositor, treeland, is still under development, so fresco makes no claim about deepin on wayland yet.",
  },

  download: {
    kicker: "download",
    title: "Deploy on Debian, Ubuntu, Pop!_OS, and Mint.",
    badge: "x11 · wayland",
    lead: "The official one-line installer or the .deb release. Either path copies to your clipboard and runs instantly. Fresco keeps playing after you close the window.",
    cardTitle: "one-line install",
    cardBody:
      "Run this in a terminal. It downloads and installs the latest .deb for you, always the newest release:",
    terminalTitle: "fresco install",
    aptComment: "already have the .deb downloaded?",
    releases: "Browse all releases",
    gpuNote:
      "For the lowest CPU usage, install your GPU's hardware-decode driver (Intel media VA driver, Mesa VA drivers, or the NVIDIA proprietary driver for NVDEC).",
    copy: "Copy",
    copied: "Copied",
  },

  faq: {
    kicker: "faq",
    title: "Questions, answered.",
    lead: "Everything you need to know before setting your first live wallpaper on Linux.",
    /** Q&A written from real user phrasings (AskUbuntu, Mint forums, Reddit, HN). */
    items: [
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
        q: "Does Fresco work on Wayland and the COSMIC desktop?",
        a: "Yes. Fresco runs animated wallpapers on Wayland layer-shell compositors through a bundled, supervised mpvpaper backend: COSMIC (Pop!_OS 24.04), Hyprland, Sway, KDE Plasma 6, and other wlroots compositors. Since v1.1.1 it ships two mpvpaper builds and probes at runtime, so it works on both libmpv1 and libmpv2 distributions. On X11 it works on any desktop.",
      },
      {
        q: "Does Fresco work on GNOME?",
        a: "On GNOME with an X11 session, yes, full live wallpapers. On GNOME with Wayland, Mutter does not expose a live wallpaper surface, so Fresco falls back to showing a static frame of your chosen wallpaper instead of pretending to animate.",
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
        q: "How is Fresco different from Wallpaper Engine?",
        a: "Wallpaper Engine is a paid, Windows-first product that only runs on Linux through Steam Play and Proton. Fresco is free, open source (GPL-3.0), and native to Linux: no Steam, no Proton, no compatibility layer. Instead of the Steam Workshop it has a built-in catalog of curated, licensed wallpapers, and it supports X11 and Wayland layer-shell compositors directly.",
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
        q: "Can I show song lyrics on my Linux desktop?",
        a: "Yes. Fresco draws time-synced lyrics onto your wallpaper, following whatever is playing on your system over MPRIS: browsers, music apps, video players. There are four presets, a nine-point placement grid, a sync-offset slider, an optional next line, and optional track title and artist. Lyrics come from a local .lrc file first, then from LRCLIB, a free community-run database. Firefox is the most reliable player; Spotify's native Linux client reports a broken playback position, though Spotify in a browser is fine.",
      },
      {
        q: "Does Linux have desktop widgets like Conky?",
        a: "Yes, and Fresco adds four of them that need no panel, no extension, and no support from your desktop: synced song lyrics, a clock with six themes, an audio visualiser with five styles, and the current track's cover art on a turning record. They are painted into the wallpaper itself rather than into a window, so they never sit above your windows, never intercept a click, and work on desktops that have no widget layer of their own, including COSMIC, Hyprland, and Sway. All four are off by default. Unlike Conky there are no system-monitor widgets yet, so no CPU, RAM, or network readouts. GNOME on Wayland is the one place widgets cannot run, because there is no live wallpaper surface to draw into.",
      },
      {
        q: "Can I get a music visualiser on my desktop background?",
        a: "Yes. Fresco's audio visualiser reacts to whatever your system is playing, in one of five styles (Bars, Mirror, Wave, Dots, Ring) with a colour picker, a two-colour blend, or rainbow. It is off by default and asks for consent the first time you enable it, because it has to listen to your audio output. With music playing and all four widgets on, the measured cost was 0.8% of one CPU core, nearly all of it the audio capture, because nothing repaints unless its content changed.",
      },
      {
        q: "Is Fresco free?",
        a: "Yes. Fresco is completely free and open source under the GPL-3.0 license. There is no paid tier.",
      },
    ],
  },

  footer: {
    github: "GitHub",
    license: "License",
    tagline: "rust + gtk4 + mpv",
    sound: "Toggle sound",
  },

  /** SoftwareApplication.featureList in the JSON-LD graph. */
  featureList: [
    "Built-in catalog of curated, licensed wallpapers",
    "Video, GIF, image, slideshow, and playlist wallpapers",
    "Desktop widgets drawn into the wallpaper: synced song lyrics, clock, audio visualiser, album art",
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
  ],

  /** SoftwareApplication.description in the JSON-LD graph. */
  softwareDescription:
    "Fresco is a free, open-source live-wallpaper app for Linux. It sets video, GIF, image, slideshow, and playlist wallpapers as your animated desktop background, with hardware-accelerated playback, and can draw four desktop widgets into the wallpaper itself: time-synced song lyrics, a clock, an audio visualiser, and album art on a turning record. A free Wallpaper Engine alternative for Pop!_OS, Ubuntu, Linux Mint, Debian, and elementary OS, on X11 and on Wayland layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6).",
};

/**
 * Deliberately inferred from `en` rather than hand-written: adding a key here
 * makes all six translations fail to compile until they carry it too. No
 * `as const` above, so string literals widen and translations type-check.
 */
export type Dictionary = typeof en;

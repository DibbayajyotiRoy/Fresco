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
      "kali linux live wallpaper",
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
    /** Mobile menu toggle (icon-only button). */
    menu: "Menu",
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
    /** Phone-only primary CTA (Fresco is a Linux desktop app). */
    sendToComputer: "Send to my computer",
    sendToComputerShareTitle: "Fresco",
    sendToComputerShareText: "Fresco: a free, open-source live wallpaper app for Linux.",
    linkCopied: "Link copied. Open it on your Linux PC.",
    howToInstall: "How to install",
    /** Live social proof beside the CTAs (installs first seen in 24h). */
    newUsers24h: (n: string) =>
      `${n} people started using Fresco in the last 24 hours`,
    /** Creator credit under the product preview; the name follows. */
    builtBy: "Built by",
    /** Live pill: installs active in 24h, then installs first seen in 24h. */
    activeToday: (n: string) => `${n} people used Fresco in the last 24 hours`,
    newToday: (n: string) => `${n} new`,
    /** Decorative desktop-scene overlay drawn over the demo video (aria-hidden). */
    desktopActivities: "Activities",
    desktopClock: "10:42",
    desktopIconHome: "Home",
    desktopIconPictures: "Pictures",
    desktopIconDocument: "Notes",
    desktopIconTrash: "Trash",
  },

  stats: {
    ariaLabel: "Project stats",
    downloads: "total downloads",
    downloadsUnknown: "downloads on GitHub",
    stars: "GitHub stars",
    version: "Latest release",
    license: "free and open source",
    users: "users",
    countries: "countries",
    cohortNote:
      "Users and countries are counted from installs that opted in to anonymous telemetry.",
    active24h: "active in the last 24h",
  },

  testimonials: {
    kicker: "Field reports",
    title: (users: string, countries: string) =>
      `Running on ${users} Linux desktops in ${countries} countries.`,
    lead: "Unedited notes from people running Fresco, sent from inside the app. Anonymous feedback is attributed by country only.",
    fromCountry: (country: string) => `Fresco user · ${country}`,
    anonymous: "Fresco user",
    namedLabel: "Field report",
    /** Visible marquee toggle (WCAG 2.2.2 pause control). */
    pause: "Pause testimonials",
    play: "Play testimonials",
  },

  glance: {
    ariaLabel: "Fresco at a glance",
    caption: "Fresco at a glance",
    labelWhat: "What it is",
    labelPlatforms: "Platforms",
    labelWidgets: "Desktop widgets",
    labelLicense: "License",
    labelInstall: "Install",
    what: "Fresco is a free, open-source live wallpaper app for Linux: it sets video, GIF, image, slideshow, and playlist wallpapers as your animated desktop background, with GPU hardware decoding. A free Wallpaper Engine alternative and a GUI for mpvpaper on Wayland.",
    platforms:
      "Any X11 desktop (Ubuntu, Pop!_OS, Linux Mint, Debian, Kali Linux), plus Wayland layer-shell compositors: COSMIC, Hyprland, Sway, KDE Plasma 6. GNOME Wayland shows a still frame only until a Fresco GNOME extension ships.",
    widgets:
      "Five widgets painted into the wallpaper itself, not into a window: time-synced song lyrics, a clock with eight themes, a world clock, an audio visualiser, and album art on a turning record. Nothing floats over your windows unless you turn on the world clock's optional Float above windows mode, and nothing intercepts a click. All off by default; unavailable on GNOME Wayland, which has no live wallpaper surface.",
    licenseLead: "GPL-3.0, free forever.",
    licenseLink: "Source on GitHub",
    licenseTail: "Built with Rust, GTK4, and mpv.",
  },

  features: {
    kicker: "Features",
    title: "Any media. Any monitor. No CPU drama.",
    lead: "Fresco sets video, GIF, image, slideshow, and playlist wallpapers on X11 and Wayland, decoded on the GPU so a live wallpaper costs about as much as a static one.",
    /** Heading of the widgets block; independent of the pinned `whatsNew` copy. */
    widgetsTitle: "Five desktop widgets, painted into the wallpaper.",
    /** The fifth card of the widget showcase; the pinned `whatsNew` copy has four. */
    worldClock: {
      title: "World clock",
      body:
        "Several cities at once, as plain 12-hour text with a soft shadow and no card behind it. Pick any time zones and label each one. It repaints once a minute, and an optional mode floats it above your windows, with clicks passing through.",
    },
    manifest: (n: number) => `Manifest: ${n} capabilities`,
    /** Sentence-final mark after each row title. */
    titleSuffix: ".",
    thCapability: "Capability",
    thWhatYouGet: "What you get",
    thStatus: "Status",
    footnote:
      "GNOME Wayland: still frame only. Mutter exposes no live surface, and GNOME 50, Ubuntu 25.10+ and Fedora 43+ no longer ship an Xorg session to fall back to. Widgets need that surface too, so they are unavailable there, and live video waits on a planned Fresco GNOME extension. Everything else above is live.",
    tally: (shipping: number, total: number, soon: number) =>
      `${shipping} of ${total} shipping · ${soon} in-preview · 0 deprecated`,
    rows: {
      hwDecode: {
        tag: "HW decode",
        title: "Hardware-accelerated playback",
        description:
          "Decoding runs on the GPU through mpv (VA-API or NVDEC). A 4K video wallpaper costs about as much CPU as a static image.",
        status: "Near-zero CPU",
      },
      sessions: {
        tag: "Sessions",
        title: "X11 and Wayland",
        description:
          "A desktop-window backend on any X11 desktop, plus a layer-shell backend for COSMIC, Hyprland, Sway, and KDE Plasma 6. GNOME Wayland shows a still frame until the planned GNOME extension.",
        status: "X11 · layer-shell",
      },
      catalog: {
        tag: "Catalog",
        title: "Built-in wallpaper catalog",
        description:
          "Browse curated, licensed wallpapers in-app (menu, then Browse wallpapers) and set one in two clicks. You can also paste a direct link.",
        status: "In-app",
      },
      linkedFolders: {
        tag: "Folders",
        title: "Link a folder",
        description:
          "Use a wallpaper folder you already have (images, GIFs and videos) without importing it. It stays in sync as you add and remove files, subfolders are optional, and your favourites and edits are kept per file.",
        status: "No import",
      },
      bigLibrary: {
        tag: "Library",
        title: "Thousands of wallpapers",
        description:
          "The library only draws what is on screen. A 4,000-wallpaper test library opened in about 2 seconds and used about 325 MB, down from 9 to 13 seconds and 1.6 GB.",
        status: "On demand",
      },
      video: {
        tag: "Video · GIF",
        title: "Video & GIF wallpapers",
        description: "Loop any mp4, webm, mkv, or animated GIF as your desktop.",
        status: "mp4 webm mkv gif",
      },
      slideshow: {
        tag: "Slideshow",
        title: "Slideshows with transitions",
        description:
          "Rotate a folder of images with a fade through black, Ken Burns, zoom, or blur.",
        status: "4 transitions",
      },
      playlist: {
        tag: "Playlist",
        title: "Video playlists",
        description: "Queue several clips and let Fresco cycle through them.",
        status: "Auto-cycle",
      },
      lyrics: {
        tag: "Lyrics · clock",
        title: "Lyrics and clock widgets",
        description:
          "Time-synced song lyrics for whatever is playing over MPRIS (local .lrc first, then LRCLIB), a clock in one of eight themes, and a world clock for several cities at once. Drawn into the wallpaper, so nothing floats over your windows unless you switch on the world clock's Float above windows mode. Off by default.",
        status: "Off by default",
      },
      visualiser: {
        tag: "Visualiser",
        title: "Audio visualiser and album art",
        description:
          "Five visualiser styles (Bars, Mirror, Wave, Dots, Ring) with a colour picker, blend, or rainbow, plus the current track's cover on a turning record. The visualiser asks before it listens to your audio.",
        status: "0.8% of one core",
      },
      editor: {
        tag: "Editor",
        title: "Crop and rotate",
        description:
          "Drag a frame to pick the region, rotate 90 degrees to fix sideways clips. Both stay zero-copy on the GPU.",
        status: "Zero-copy",
      },
      audio: {
        tag: "Audio",
        title: "Per-wallpaper sound",
        description:
          "Unmute a video and set its volume. Fresco remembers the choice for that wallpaper.",
        status: "Per-wallpaper",
      },
      displays: {
        tag: "Displays",
        title: "Per-display wallpapers",
        description:
          "Right-click any wallpaper and Set on a specific display. Each monitor can run its own.",
        status: "Per-monitor",
      },
      schedule: {
        tag: "Schedule",
        title: "Day and night schedules",
        description:
          "Two wallpapers, two switch times, swapped automatically by the daemon. Time slots and solar switching via config.",
        status: "Automatic",
      },
      power: {
        tag: "Power",
        title: "Power-aware",
        description:
          "Pause on battery, and auto-pause per monitor when a window there goes fullscreen.",
        status: "Auto-pause",
      },
      newTab: {
        tag: "Browser new tab",
        title: "Your wallpaper on every new tab",
        description:
          "A companion browser extension (Chrome, Brave, Edge, Firefox) mirrors your desktop wallpaper, or a browser-specific pick, on the new-tab page via a local bridge that talks only to 127.0.0.1. In the repo today; store listings pending.",
        status: "Coming soon",
      },
      themes: {
        tag: "Themes",
        title: "Themes and accents",
        description: "Light, dark, or follow the system, with six accent palettes.",
        status: "6 palettes",
      },
    },
  },

  compare: {
    kicker: "Compare",
    title: "Fresco vs the Linux wallpaper field.",
    lead: "Fresco is the only actively maintained Linux live-wallpaper app in this table that combines a GUI, hardware decoding, X11 and Wayland support, and a built-in catalog, free. Here is the full comparison with Hidamari, Komorebi, mpvpaper, and Wallpaper Engine.",
    meter: (tools: number, caps: number) =>
      `Compare · ${tools} tools · ${caps} capabilities`,
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
      unclear: "Unclear",
      na: "n/a",
    },
  },

  whatsNew: {
    kicker: (version: string) => `What's new · v${version}`,
    title: "Four desktop widgets, painted into the wallpaper.",
    lead: (version: string) =>
      `What shipped in v${version}. No extra window, nothing to click through, identical on X11 and layer-shell. All four are off by default, and with music playing and every one of them on the measured cost was 0.8% of one CPU core. Each entry here is reproduced in the CHANGELOG on GitHub.`,
    changelog: "Full changelog",
    patch: (n: string) => `Patch ${n}`,
    items: {
      lyrics: {
        title: "Synced song lyrics",
        body: "The current line, in time with whatever is playing over MPRIS. Local .lrc files first, then LRCLIB. Four presets and a sync offset.",
      },
      clock: {
        title: "Clock, eight themes",
        body: "Digital, Minimal, Segment, Stacked, Wordy, Card, NOS, and a phone-style Lock screen face. 12 or 24-hour, optional date.",
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
    kicker: "How it works",
    title: "Three clicks, then forget about it.",
    lead: "Open Fresco, click add, click set, close. The daemon keeps the wallpaper running, even after you reboot.",
    step: (n: string) => `Step ${n}`,
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
    kicker: "Watch it run",
    title: "Under a minute each. No narration required.",
    lead: "Short screen recordings of Fresco on a real desktop. Nothing loads from YouTube until you press play.",
    more: "More on YouTube",
    inDevelopment: "In development",
    play: (title: string) => `Play: ${title}`,
    items: {
      "YWzD3-xkCEc": {
        tag: "Add from link",
        blurb:
          "Copy a Pinterest link, paste it into Fresco, set it as your wallpaper. No download step, no file juggling.",
      },
      C1MqrhGkovQ: {
        tag: "Lyrics widgets",
        blurb:
          "Synchronized lyrics and a clock drawn into a live wallpaper on Wayland and X11. Shipped in v1.1.36, along with an audio visualiser and an album-art disc.",
      },
    },
  },

  supported: {
    title: "Where Fresco runs.",
    lead: "On any X11 desktop, including deepin 25's DDE, and on Wayland layer-shell compositors (COSMIC, Hyprland, Sway, and KDE Plasma 6) across the popular Debian, Ubuntu, and Kali Linux distributions. GNOME Wayland shows a still frame only for now.",
    sessionsTitle: "Sessions and compositors",
    distrosTitle: (n: number) => `Tested distributions · ${n}`,
    formatsTitle: (n: number) => `Supported formats · ${n}`,
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
          "Automatic DDE adaptation, icons stay visible. Community-verified on deepin 25 Community build1. Also in the deepin App Store.",
      },
      wayland: {
        label: "Wayland layer-shell",
        detail: "COSMIC, Hyprland, Sway, KDE Plasma 6, wlroots",
      },
      gnome: {
        label: "GNOME on Wayland",
        detail: "Still frame only (Mutter has no live surface)",
      },
    },
    fieldReport: "Field report · deepin 25",
    verifiedEnv: "Verified environment",
    testimonialRole: "Deepin community tester",
    envLabels: {
      session: "Session",
      os: "OS",
      gpu: "GPU",
    },
    footnote:
      "Deepin 25 defaults to X11, and that is the session Fresco is verified on there. Deepin's own Wayland compositor, Treeland, is still under development, so Fresco makes no claim about deepin on Wayland yet.",
  },

  download: {
    kicker: "Download",
    title: "Install on Debian, Ubuntu, Pop!_OS, Mint, and Kali.",
    badge: "X11 · Wayland",
    lead: "The official one-line installer or the .deb release. Either path copies to your clipboard and runs instantly. Fresco keeps playing after you close the window.",
    worksOnTitle: "Works on",
    cardTitle: "One-line install",
    cardBody:
      "Run this in a terminal. It downloads and installs the latest .deb for you, always the newest release:",
    terminalTitle: "fresco install",
    aptComment: "Already have the .deb downloaded?",
    releases: "Browse all releases",
    gpuNote:
      "For the lowest CPU usage, install your GPU's hardware-decode driver (Intel media VA driver, Mesa VA drivers, or the NVIDIA proprietary driver for NVDEC).",
    storeLabel: "Also on the deepin App Store",
    storeDesc:
      "Built and verified for deepin 25's DDE desktop, icons and all.",
    storeStep1: "Open App Store",
    storeStep2: "Search for \"Fresco\"",
    storeStep3: "Click Install. Updates arrive through the store.",
    copy: "Copy",
    copied: "Copied",
  },

  faq: {
    kicker: "FAQ",
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
        a: "On GNOME with an X11 session, yes, full live wallpapers. But GNOME 49 disabled its X11 session and GNOME 50 removed it, so Ubuntu 25.10 and newer (including 26.04 LTS) and Fedora 43 and newer have none; Ubuntu 22.04 and 24.04 still offer \"Ubuntu on Xorg\". On GNOME with Wayland, Mutter does not expose a live wallpaper surface, so Fresco shows a still frame of your chosen wallpaper and says so instead of pretending to animate. Live video on GNOME Wayland needs a Fresco GNOME extension, which is planned.",
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
        a: "Looping video (mp4, webm, mkv, avi, mov), animated GIFs, static images (jpg, png, webp), a folder of images as a slideshow with fade through black, Ken Burns, zoom, or blur transitions, and multi-video playlists.",
      },
      {
        q: "Can Fresco use my existing wallpaper folder without importing?",
        a: "Yes. Use Link a folder to point Fresco at a folder you already keep wallpapers in. Its images, GIFs and videos show up in your library where they are, without being imported, and the library stays in sync as you add and remove files in that folder. Subfolders are optional, and your favourites and edits are remembered per file. Add still imports individual files as before.",
      },
      {
        q: "Can Fresco handle thousands of wallpapers?",
        a: "Yes. The library only builds the cards for the wallpapers near where you are scrolling, instead of every wallpaper at once, so a big collection does not weigh the app down. On a 4,000-wallpaper test library (half images, half videos) the library appeared in about 2 seconds instead of 9 to 13, and the app used roughly 325 MB of memory instead of 1.6 GB. Those figures come from a development build under load on one machine, so read them as a guide rather than a promise.",
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
        a: "Yes, and Fresco adds five of them that need no panel, no extension, and no support from your desktop: synced song lyrics, a clock with eight themes, a world clock for several cities at once, an audio visualiser with five styles, and the current track's cover art on a turning record. They are painted into the wallpaper itself rather than into a window, so they sit behind your windows, never intercept a click, and work on desktops that have no widget layer of their own, including COSMIC, Hyprland, and Sway. The one exception is the world clock's optional Float above windows mode, which lifts it above every app with clicks passing through. All five are off by default. Unlike Conky there are no system-monitor widgets yet, so no CPU, RAM, or network readouts. GNOME on Wayland is the one place widgets cannot run, because there is no live wallpaper surface to draw into.",
      },
      {
        q: "Can I get a music visualiser on my desktop background?",
        a: "Yes. Fresco's audio visualiser reacts to whatever your system is playing, in one of five styles (Bars, Mirror, Wave, Dots, Ring) with a colour picker, a two-colour blend, or rainbow. It is off by default and asks for consent the first time you enable it, because it has to listen to your audio output. With music playing and the lyrics, clock, visualiser and album-art widgets on, the measured cost was 0.8% of one CPU core, nearly all of it the audio capture, because nothing repaints unless its content changed.",
      },
      {
        q: "Is Fresco in the deepin App Store?",
        a: "Yes. On deepin 25, open App Store, search for Fresco, and click Install. It is published on the deepin Community App Store, so updates arrive through the store. The .deb and the one-line installer work too.",
      },
      {
        q: "Is Fresco free?",
        a: "Yes. Fresco is completely free and open source under the GPL-3.0 license. There is no paid tier.",
      },
    ],
  },

  report: {
    title: "Report a problem",
    lead: "Found a bug or have an idea? Tell us on GitHub.",
    tipBefore: "Include your desktop, Fresco version and the output of ",
    tipAfter: ".",
    bug: "Report a bug",
    feature: "Request a feature",
  },

  footer: {
    github: "GitHub",
    license: "License",
    reportIssue: "Report an issue",
    tagline: "Rust + GTK4 + mpv",
    sound: "Toggle sound",
  },

  /** SoftwareApplication.featureList in the JSON-LD graph. */
  featureList: [
    "Built-in catalog of curated, licensed wallpapers",
    "Video, GIF, image, slideshow, and playlist wallpapers",
    "Desktop widgets drawn into the wallpaper: synced song lyrics, clock, world clock, audio visualiser, album art",
    "Add wallpapers from a direct URL",
    "Day and night wallpaper schedules (plus time slots and solar via config)",
    "Per-display wallpapers from the GUI",
    "Automatic audio recovery when the sound server starts late",
    "Scriptable JSON control socket",
    "Hardware-accelerated playback (VA-API, NVDEC)",
    "Works on X11 and Wayland layer-shell compositors",
    "Drag-to-crop and 90-degree rotate editor",
    "Per-wallpaper sound and volume",
    "Slideshow transitions (fade through black, Ken Burns, zoom, blur)",
    "Wallpaper library with search",
    "Link an existing wallpaper folder without importing it, kept in sync as files change",
    "Library that stays quick with thousands of wallpapers",
    "Different wallpaper per monitor",
    "Pause on battery and auto-pause on fullscreen",
    "Restores automatically on login",
    "Themes and accent colors",
  ],

  /** SoftwareApplication.description in the JSON-LD graph. */
  softwareDescription:
    "Fresco is a free, open-source live-wallpaper app for Linux. It sets video, GIF, image, slideshow, and playlist wallpapers as your animated desktop background, with hardware-accelerated playback, and can draw five desktop widgets into the wallpaper itself: time-synced song lyrics, a clock, a world clock, an audio visualiser, and album art on a turning record. A free Wallpaper Engine alternative for Pop!_OS, Ubuntu, Linux Mint, Debian, Kali Linux, and elementary OS, on X11 and on Wayland layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6).",
};

/**
 * Deliberately inferred from `en` rather than hand-written: adding a key here
 * makes all six translations fail to compile until they carry it too. No
 * `as const` above, so string literals widen and translations type-check.
 */
export type Dictionary = typeof en;

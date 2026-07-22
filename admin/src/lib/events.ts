/**
 * Human-readable names for the raw telemetry event ids the app sends.
 *
 * The app emits machine ids ("wallpaper_set"); a dashboard that shows those
 * verbatim makes the reader translate. Each entry pairs the id with a plain
 * title and a one-line statement of what actually had to happen for the row
 * to exist — the detail that decides whether a count means "people love this"
 * or "people tried and it failed".
 *
 * Unknown ids fall back to a de-slugged title, so a newly instrumented event
 * degrades to readable instead of disappearing.
 */
export type EventMeta = {
  title: string;
  /** What the user did to produce one of these. */
  meaning: string;
};

const EVENTS: Record<string, EventMeta> = {
  wallpaper_set: {
    title: "Wallpaper set",
    meaning:
      "A wallpaper was applied to the desktop and the daemon confirmed it started. Failed applies send nothing.",
  },
  browser_wallpaper_set: {
    title: "Browser wallpaper set",
    meaning:
      "The browser-only wallpaper was chosen. Counts the click, not a confirmed success.",
  },
  add_from_link: {
    title: "Added from link",
    meaning:
      "A Pinterest or direct media link was pasted and downloaded. Carries source, kind and outcome.",
  },
  tutorial_opened: {
    title: "Tutorial opened",
    meaning:
      "The demo video was opened in a browser. Counts intent only — the app cannot see whether it was watched.",
  },
  update: {
    title: "In-app update",
    meaning: "A self-update ran. Carries the outcome (success or failure).",
  },
};

/**
 * Every event the client is instrumented to send. The table renders these
 * even at zero: an absent row is indistinguishable from a feature nobody
 * touched, and "this feature has never been used" is the single most
 * actionable reading on the page.
 */
export const KNOWN_EVENTS = Object.keys(EVENTS);

/** "wallpaper_set" -> "Wallpaper set" for ids not in the table. */
function deslug(name: string): string {
  const words = name.replace(/[_-]+/g, " ").trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

export function eventMeta(name: string): EventMeta {
  return EVENTS[name] ?? { title: deslug(name), meaning: "" };
}

/**
 * Plain-English labels and explanations for the install breakdown columns.
 * "Decode" alone is unreadable to anyone who didn't write the client.
 */
export const BREAKDOWN_HELP: Record<string, string> = {
  Distro: "Linux distribution, from /etc/os-release",
  Desktop: "Desktop environment (GNOME, KDE, DDE…)",
  "Session type": "Wayland or X11",
  "Video decode": "Hardware or software decode path",
  "Download source": "Where the install one-liner was copied from",
  "Install channel": "How it was packaged: deb, flatpak, or source",
};

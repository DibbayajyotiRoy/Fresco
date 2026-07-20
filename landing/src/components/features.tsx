import { ACHIEVEMENTS } from "@/lib/game";

type FeatureRow = {
  tag: string;
  title: string;
  description: string;
  status: string;
  soon?: boolean;
};

const FEATURE_ROWS: FeatureRow[] = [
  {
    tag: "hw decode",
    title: "Hardware-accelerated playback",
    description:
      "Decoding runs on the GPU through mpv (VA-API or NVDEC). A 4K video wallpaper costs about as much CPU as a static image.",
    status: "near-zero cpu",
  },
  {
    tag: "sessions",
    title: "X11 and Wayland",
    description:
      "A desktop-window backend on any X11 desktop, plus a layer-shell backend for COSMIC, Hyprland, Sway, and KDE Plasma 6. GNOME Wayland gets a static-frame fallback.",
    status: "x11 · layer-shell",
  },
  {
    tag: "catalog",
    title: "Built-in wallpaper catalog",
    description:
      "Browse curated, licensed wallpapers in-app (menu, then Browse wallpapers) and set one in two clicks. You can also paste a direct link.",
    status: "in-app",
  },
  {
    tag: "video · gif",
    title: "Video & GIF wallpapers",
    description: "Loop any mp4, webm, mkv, or animated GIF as your desktop.",
    status: "mp4 webm mkv gif",
  },
  {
    tag: "slideshow",
    title: "Slideshows with transitions",
    description:
      "Rotate a folder of images with crossfade, fade, or Ken Burns.",
    status: "4 transitions",
  },
  {
    tag: "playlist",
    title: "Video playlists",
    description: "Queue several clips and let Fresco cycle through them.",
    status: "auto-cycle",
  },
  {
    tag: "editor",
    title: "Crop and rotate",
    description:
      "Drag a frame to pick the region, rotate 90 degrees to fix sideways clips. Both stay zero-copy on the GPU.",
    status: "zero-copy",
  },
  {
    tag: "audio",
    title: "Per-wallpaper sound",
    description:
      "Unmute a video and set its volume. Fresco remembers the choice for that wallpaper.",
    status: "per-wallpaper",
  },
  {
    tag: "displays",
    title: "Per-display wallpapers",
    description:
      "Right-click any wallpaper and Set on a specific display. Each monitor can run its own.",
    status: "per-monitor",
  },
  {
    tag: "schedule",
    title: "Day and night schedules",
    description:
      "Two wallpapers, two switch times, swapped automatically by the daemon. Time slots and solar switching via config.",
    status: "automatic",
  },
  {
    tag: "power",
    title: "Power-aware",
    description:
      "Pause on battery, and auto-pause per monitor when a window there goes fullscreen.",
    status: "auto-pause",
  },
  {
    tag: "browser new tab",
    title: "Your wallpaper on every new tab",
    description:
      "A companion browser extension (Chrome, Brave, Edge, Firefox) mirrors your desktop wallpaper, or a browser-specific pick, on the new-tab page via a local bridge that talks only to 127.0.0.1. In the repo today; store listings pending.",
    status: "coming soon",
    soon: true,
  },
  {
    tag: "themes",
    title: "Themes and accents",
    description:
      "Light, dark, or follow the system, with six accent palettes.",
    status: "6 palettes",
  },
];

const SPECS = ACHIEVEMENTS.find((a) => a.id === "specs")!;
const TOTAL = FEATURE_ROWS.length;
const SOON = FEATURE_ROWS.filter((r) => r.soon).length;
const SHIPPING = TOTAL - SOON;

export function Features() {
  return (
    <section id="features" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
            <p className="instrument-label !text-ink-faint">
              {SPECS.code} <span className="text-accent/70">·</span> +{SPECS.xp} xp
            </p>
            <p className="instrument-label !text-ink-faint">
              {SPECS.code} · inventory
            </p>
          </div>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            Any media. Any monitor. No CPU drama.
          </h2>
          <p className="mt-4 text-pretty text-ink-subtle">
            Fresco sets video, GIF, image, slideshow, and playlist wallpapers
            on X11 and Wayland, decoded on the GPU so a live wallpaper costs
            about as much as a static one. The full spec sheet:
          </p>
          <p className="mt-2 font-mono text-meta uppercase tracking-widest text-ink-faint">
            manifest: {FEATURE_ROWS.length} capabilities
          </p>
        </div>

        <div className="mt-10 overflow-x-auto rounded-md border border-hairline bg-surface">
          <table className="w-full min-w-[720px] border-collapse">
            <thead>
              <tr className="border-b-2 border-hairline">
                <th
                  scope="col"
                  className="instrument-label w-[130px] px-4 py-3 text-left font-semibold"
                >
                  Capability
                </th>
                <th
                  scope="col"
                  className="instrument-label border-l border-hairline px-4 py-3 text-left font-semibold"
                >
                  What you get
                </th>
                <th
                  scope="col"
                  className="instrument-label w-[190px] border-l border-hairline px-4 py-3 text-right font-semibold"
                >
                  Status
                </th>
              </tr>
            </thead>
            <tbody>
              {FEATURE_ROWS.map((row) => (
                <tr
                  key={row.tag}
                  className="border-b border-hairline transition-colors last:border-0 even:bg-raised/50 hover:bg-raised"
                >
                  <th
                    scope="row"
                    className="px-4 py-2.5 text-left align-top font-mono text-meta font-medium uppercase tracking-wide text-ink-faint"
                  >
                    {row.tag}
                  </th>
                  <td className="border-l border-hairline px-4 py-2.5 align-top">
                    <span className="text-sm font-medium text-ink">
                      {row.title}.
                    </span>{" "}
                    <span className="text-sm text-ink-subtle">
                      {row.description}
                    </span>
                  </td>
                  <td className="border-l border-hairline px-4 py-2.5 text-right align-top font-mono text-meta tracking-wide text-ink-subtle">
                    {row.soon ? (
                      <span className="inline-flex items-center rounded-sm border border-hairline bg-raised px-1.5 py-0.5 uppercase text-ink-faint">
                        {row.status}
                      </span>
                    ) : (
                      <>
                        <span aria-hidden className="mr-1.5 text-ok">
                          ✓
                        </span>
                        {row.status}
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <p className="mt-4 font-mono text-meta tracking-wide text-ink-faint">
          gnome wayland: static-frame fallback (mutter exposes no live surface).
          everything else above is live.
        </p>
        <p className="mt-3 font-mono text-meta uppercase tracking-widest text-ink-faint">
          {SHIPPING} of {TOTAL} shipping · {SOON} in-preview · 0 deprecated
        </p>
      </div>
    </section>
  );
}
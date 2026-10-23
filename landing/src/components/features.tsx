import "@/styles/spec.css";
import "@/styles/showcase.css";
import type { ReactNode } from "react";
import {
  ArrowUpRight,
  AudioLines,
  BatteryCharging,
  Check,
  Clock,
  Cpu,
  Film,
  FolderSync,
  Gauge,
  Globe,
  Images,
  LayoutGrid,
  Layers,
  ListMusic,
  Mic2,
  Monitor,
  Palette,
  SlidersHorizontal,
  Volume2,
  type LucideIcon,
} from "lucide-react";
import { GITHUB_URL } from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";
import { SpecHead } from "@/components/spec/spec-head";
import { MonitorVideo } from "@/components/spec/monitor-video";
import { Showcase } from "@/components/showcase/showcase";
import { WIDGET_IDS } from "@/components/showcase/widget-ids";

type Rows = Dictionary["features"]["rows"];
type RowId = keyof Rows;

/**
 * Row order of the full capability list and which entry is still in preview
 * are structural, so they live here; every string comes from the dictionary.
 * Adding a row means adding its id here and its copy to every locale (the
 * Dictionary type enforces that).
 */
const ROW_ORDER = [
  "hwDecode",
  "sessions",
  "catalog",
  "linkedFolders",
  "bigLibrary",
  "video",
  "slideshow",
  "playlist",
  "lyrics",
  "visualiser",
  "editor",
  "audio",
  "displays",
  "schedule",
  "power",
  "newTab",
  "themes",
] as const satisfies readonly RowId[];

/** Not shipped in a stable release yet: marked with a muted badge. */
const SOON_ROWS = new Set<RowId>(["newTab"]);

/** A small, sensible icon per capability, for the compact manifest grid. */
const ROW_ICONS: Record<RowId, LucideIcon> = {
  hwDecode: Cpu,
  sessions: Layers,
  catalog: LayoutGrid,
  linkedFolders: FolderSync,
  bigLibrary: Gauge,
  video: Film,
  slideshow: Images,
  playlist: ListMusic,
  lyrics: Mic2,
  visualiser: AudioLines,
  editor: SlidersHorizontal,
  audio: Volume2,
  displays: Monitor,
  schedule: Clock,
  power: BatteryCharging,
  newTab: Globe,
  themes: Palette,
};

/**
 * Features, told as two things: the desktop widgets (the real-widget
 * showcase, which carries the `whats-new` anchor the nav links to) and
 * per-monitor wallpapers. Every capability is still listed, as a real
 * <table> inside a collapsed <details> at the bottom, so it stays in the DOM
 * for search engines without lengthening the page.
 *
 * The widgets block shows no release version: the only version on the page
 * is the live one from GitHub.
 */
export function Features({ dict }: { dict: Dictionary }) {
  const steps = WIDGET_IDS.map((id) => ({
    id,
    title: dict.whatsNew.items[id].title,
    body: dict.whatsNew.items[id].body,
  }));

  return (
    <section
      id="features"
      aria-labelledby="features-title"
      className="border-b border-hairline py-16 sm:py-20"
    >
      <div className="wrap">
        <div className="mx-auto max-w-6xl">
          <SpecHead
            id="features-title"
            kicker={dict.features.kicker}
            title={dict.features.title}
            lead={dict.features.lead}
          />

          {/* 1. Desktop widgets. */}
          <div id="whats-new" className="mt-12 sm:mt-16">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between lg:gap-16">
              <h3
                data-reveal="fade"
                className="max-w-3xl font-display text-display-sm text-ink [overflow-wrap:anywhere]"
              >
                {dict.whatsNew.title}
              </h3>
              <a
                href={`${GITHUB_URL}/blob/main/CHANGELOG.md`}
                target="_blank"
                rel="noopener noreferrer"
                className="group/cl inline-flex min-h-10 shrink-0 items-center gap-1 self-start rounded-md text-sm font-medium text-accent underline-offset-4 hover:underline lg:self-auto"
              >
                {dict.whatsNew.changelog}
                <ArrowUpRight
                  aria-hidden
                  className="size-4 transition-transform duration-200 ease-out group-hover/cl:-translate-y-0.5 group-hover/cl:translate-x-0.5"
                />
              </a>
            </div>
            <Showcase steps={steps} />
          </div>

          {/* 2. Multi-monitor. */}
          <MultiMonitor rows={dict.features.rows} />

          <Manifest dict={dict} />
        </div>
      </div>
    </section>
  );
}

/**
 * "Each monitor can run its own": the displays row, the power row's
 * per-monitor auto-pause as the supporting line, and three CSS monitor
 * frames each playing a DIFFERENT video wallpaper from the maintainer's own
 * Fresco library (public/wallpapers). The frames are decoration, so the
 * group is aria-hidden.
 */
function MultiMonitor({ rows }: { rows: Rows }) {
  const displays = rows.displays;
  const power = rows.power;

  return (
    <div className="mt-10 sm:mt-14 lg:mt-16">
      <div className="grid gap-8 lg:grid-cols-2 lg:items-end lg:gap-16">
        <div data-reveal="fade">
          <p className="inline-flex items-center gap-1.5 rounded-full bg-accent/10 px-3 py-1 text-sm font-medium text-accent">
            <Monitor aria-hidden className="size-4" />
            {displays.status}
          </p>
          <h3 className="mt-4 font-display text-display-sm text-ink [overflow-wrap:anywhere]">
            {displays.title}
          </h3>
          <p className="mt-4 max-w-xl text-lg text-ink-subtle">{displays.description}</p>
        </div>
        <div
          data-reveal="fade"
          data-delay="0.1"
          className="flex gap-3 rounded-[16px] border border-hairline bg-surface p-5 sm:p-6"
        >
          <span
            aria-hidden
            className="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-accent/10 text-accent"
          >
            <Check className="size-3.5" strokeWidth={2.5} />
          </span>
          <p className="text-lg">
            <span className="font-medium text-ink">{power.title}</span>
            <span className="mt-0.5 block text-ink-subtle">{power.description}</span>
          </p>
        </div>
      </div>

      {/* Phones: the video screen full width, the other two side by side under
          it. From sm: three across, the middle one larger, stands aligned. */}
      <div
        aria-hidden
        data-reveal="stagger"
        className="mt-8 grid grid-cols-2 items-end gap-3 sm:mt-10 sm:grid-cols-[1fr_1.3fr_1fr] sm:gap-5 lg:gap-8"
      >
        <Screen>
          <MonitorVideo name="ocean" className="absolute inset-0 size-full object-cover" />
        </Screen>
        <Screen className="order-first col-span-2 sm:order-none sm:col-span-1">
          <MonitorVideo name="windmill" className="absolute inset-0 size-full object-cover" />
        </Screen>
        <Screen>
          <MonitorVideo name="neon-city" className="absolute inset-0 size-full object-cover" />
        </Screen>
      </div>
    </div>
  );
}

/** A plain monitor: bezel, 16:9 screen, neck and foot. CSS only. */
function Screen({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={className}>
      <div className="rounded-[12px] border border-hairline-strong bg-surface p-1 shadow-[0_1px_2px_rgb(0_0_0/0.04),0_12px_32px_-12px_rgb(0_0_0/0.12)] sm:p-1.5">
        <div className="relative aspect-video overflow-hidden rounded-[8px] bg-raised">
          {children}
        </div>
      </div>
      <div className="mx-auto h-3 w-[10%] bg-hairline-strong sm:h-5" />
      <div className="mx-auto h-1.5 w-[32%] rounded-full bg-hairline-strong" />
    </div>
  );
}

/** Every capability, always visible as a compact icon grid (no disclosure). */
function Manifest({ dict }: { dict: Dictionary }) {
  const f = dict.features;
  const total = ROW_ORDER.length;
  const soon = SOON_ROWS.size;
  const shipping = total - soon;

  return (
    <div className="mt-10 sm:mt-12">
      <h3
        data-reveal="fade"
        className="text-lg font-semibold text-ink first-letter:uppercase"
      >
        {f.manifest(total)}
      </h3>

      <ul
        data-reveal="stagger"
        className="mt-4 grid gap-2 sm:grid-cols-2 lg:grid-cols-3"
      >
        {ROW_ORDER.map((id) => {
          const row = f.rows[id];
          const isSoon = SOON_ROWS.has(id);
          const Icon = ROW_ICONS[id];
          return (
            <li
              key={id}
              className="flex gap-3 rounded-[12px] border border-hairline bg-surface p-3.5"
            >
              <span
                aria-hidden
                className="mt-0.5 grid size-7 shrink-0 place-items-center rounded-lg bg-accent/10 text-accent"
              >
                <Icon className="size-3.5" strokeWidth={2} />
              </span>
              <div className="min-w-0">
                <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-base font-medium text-ink">
                  {row.title}
                  {isSoon && (
                    <span className="inline-flex rounded-md border border-hairline-strong bg-raised px-1.5 py-0.5 text-xs font-medium text-ink-subtle">
                      {row.status}
                    </span>
                  )}
                </p>
                {row.description && (
                  <p className="mt-1 text-sm leading-snug text-ink-subtle">{row.description}</p>
                )}
              </div>
            </li>
          );
        })}
      </ul>

      <div className="mt-3 text-sm">
        <p className="text-ink-faint first-letter:uppercase">{f.footnote}</p>
        <p className="mt-2 text-ink-muted first-letter:uppercase">
          {f.tally(shipping, total, soon)}
        </p>
      </div>
    </div>
  );
}

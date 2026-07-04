import Image from "next/image";
import {
  Cpu,
  Crop,
  Film,
  Images,
  Layers,
  LayoutGrid,
  ListVideo,
  Monitor,
  Palette,
  PauseCircle,
  SunMoon,
  Volume2,
} from "lucide-react";
import { Card } from "@/components/ui/card";

type Feature = {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
};

const MEDIA_FEATURES: Feature[] = [
  {
    icon: LayoutGrid,
    title: "Built-in wallpaper catalog",
    description:
      "Browse curated, licensed wallpapers in-app (menu, then Browse wallpapers) and set one in two clicks.",
  },
  {
    icon: Film,
    title: "Video & GIF wallpapers",
    description: "Loop any mp4, webm, mkv, or animated GIF as your desktop.",
  },
  {
    icon: Images,
    title: "Slideshows with transitions",
    description: "Rotate a folder of images with crossfade, fade, or Ken Burns.",
  },
  {
    icon: ListVideo,
    title: "Video playlists",
    description: "Queue several clips and let Fresco cycle through them.",
  },
];

const SECONDARY_FEATURES: Feature[] = [
  {
    icon: Layers,
    title: "X11 and Wayland",
    description:
      "A desktop-window backend on X11, plus a layer-shell backend for COSMIC, Hyprland, Sway, and KDE Plasma 6.",
  },
  {
    icon: Crop,
    title: "Crop and rotate",
    description:
      "Drag a frame to pick the region, rotate 90 degrees to fix sideways clips. Both stay zero-copy on the GPU.",
  },
  {
    icon: Volume2,
    title: "Per-wallpaper sound",
    description:
      "Unmute a video and set its volume. Fresco remembers the choice for that wallpaper.",
  },
  {
    icon: Monitor,
    title: "Per-display wallpapers",
    description:
      "Right-click any wallpaper and Set on a specific display. Each monitor can run its own.",
  },
  {
    icon: SunMoon,
    title: "Day and night schedules",
    description:
      "Advanced, then Day & night wallpaper: two wallpapers, two switch times, swapped automatically.",
  },
  {
    icon: PauseCircle,
    title: "Power-aware",
    description:
      "Pause on battery, and auto-pause per monitor when a window there goes fullscreen.",
  },
  {
    icon: Palette,
    title: "Themes and accents",
    description: "Light, dark, or follow the system, with six accent palettes.",
  },
];

function FeatureRow({ icon: Icon, title, description }: Feature) {
  return (
    <div className="flex gap-4">
      <div className="flex size-10 shrink-0 items-center justify-center rounded-lg border border-border bg-surface-2 text-ink-muted">
        <Icon className="size-5" />
      </div>
      <div>
        <h3 className="text-sm font-semibold tracking-tight text-ink">{title}</h3>
        <p className="mt-1 text-sm text-ink-subtle">{description}</p>
      </div>
    </div>
  );
}

export function Features() {
  return (
    <section id="features" className="border-b border-border py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="text-sm font-medium text-ink-subtle">Features</p>
          <h2 className="mt-2 text-3xl font-semibold tracking-tight text-ink sm:text-4xl">
            Any media. Any monitor. No CPU drama.
          </h2>
          <p className="mt-4 text-pretty text-ink-subtle">
            Fresco plays your wallpaper through mpv with GPU hardware decoding,
            so a 4K video costs about as much as a static image.
          </p>
        </div>

        {/* Highlight bento: hardware decoding lifted to surface-2 for hierarchy,
            media types alongside it. Lift, not color, carries the emphasis. */}
        <div className="mt-12 grid gap-5 lg:grid-cols-5">
          <Card className="flex flex-col justify-between overflow-hidden border-hairline-strong bg-surface-2 shadow-none lg:col-span-3">
            <div className="p-7">
              <div className="flex size-11 items-center justify-center rounded-lg border border-border bg-surface-3 text-ink">
                <Cpu className="size-6" />
              </div>
              <h3 className="mt-5 text-xl font-semibold tracking-tight text-ink">
                Hardware-accelerated, near-zero CPU
              </h3>
              <p className="mt-2 max-w-md text-sm text-ink-subtle">
                Decoding runs on the GPU through VA-API or NVDEC. Your processor
                stays free for everything else, with no loss of quality.
              </p>
            </div>
            <div className="relative mt-2 h-44 w-full overflow-hidden border-t border-border sm:h-52">
              <Image
                src="/screenshots/gallery.png"
                alt="A high-resolution live wallpaper running smoothly on the desktop"
                fill
                sizes="(max-width: 1024px) 100vw, 60vw"
                className="object-cover"
              />
            </div>
          </Card>

          {/* Media types */}
          <Card className="shadow-none lg:col-span-2">
            <div className="flex h-full flex-col gap-6 p-7">
              <p className="text-sm font-medium text-ink-subtle">
                Set anything as your wallpaper
              </p>
              <div className="flex flex-col gap-6">
                {MEDIA_FEATURES.map((feature) => (
                  <FeatureRow key={feature.title} {...feature} />
                ))}
              </div>
            </div>
          </Card>
        </div>

        {/* Secondary feature grid: 6 cells, no empty slots. */}
        <div className="mt-5 grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {SECONDARY_FEATURES.map((feature) => (
            <Card key={feature.title} className="p-7 shadow-none">
              <FeatureRow {...feature} />
            </Card>
          ))}
        </div>
      </div>
    </section>
  );
}

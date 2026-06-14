import Image from "next/image";
import {
  Cpu,
  Film,
  Images,
  ListVideo,
  Palette,
  RotateCcw,
  Sparkles,
} from "lucide-react";
import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";

type Feature = {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
};

const MEDIA_FEATURES: Feature[] = [
  {
    icon: Film,
    title: "Video & GIF wallpapers",
    description: "Loop any mp4, webm, mkv, or animated GIF as your desktop.",
  },
  {
    icon: Images,
    title: "Image slideshows",
    description:
      "Point at a folder and rotate through stills on your own interval.",
  },
  {
    icon: ListVideo,
    title: "Video playlists",
    description: "Queue several clips and let Fresco cycle through them.",
  },
];

const SECONDARY_FEATURES: Feature[] = [
  {
    icon: Sparkles,
    title: "Slideshow transitions",
    description: "Crossfade, fade, or a slow Ken Burns pan between images.",
  },
  {
    icon: Palette,
    title: "Theme & accent picker",
    description: "Light or dark, with an accent color that suits your setup.",
  },
  {
    icon: RotateCcw,
    title: "Restores on login",
    description: "Set it once and close the app. It comes back every session.",
  },
];

function FeatureRow({ icon: Icon, title, description }: Feature) {
  return (
    <div className="flex gap-4">
      <div className="flex size-10 shrink-0 items-center justify-center rounded-lg border border-border/70 bg-secondary/40 text-primary">
        <Icon className="size-5" />
      </div>
      <div>
        <h3 className="text-sm font-semibold tracking-tight">{title}</h3>
        <p className="mt-1 text-sm text-muted-foreground">{description}</p>
      </div>
    </div>
  );
}

export function Features() {
  return (
    <section
      id="features"
      className="border-b border-border/60 py-20 sm:py-28"
    >
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="text-sm font-medium text-primary">Features</p>
          <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
            Any media. Any monitor. No CPU drama.
          </h2>
          <p className="mt-4 text-pretty text-muted-foreground">
            Fresco plays your wallpaper through mpv with GPU hardware decoding,
            so a 4K video costs about as much as a static image.
          </p>
        </div>

        {/* Highlight card: hardware decoding */}
        <div className="mt-12 grid gap-5 lg:grid-cols-5">
          <Card className="flex flex-col justify-between overflow-hidden lg:col-span-3">
            <div className="p-7">
              <div className="flex size-11 items-center justify-center rounded-lg bg-primary/15 text-primary">
                <Cpu className="size-6" />
              </div>
              <h3 className="mt-5 text-xl font-semibold tracking-tight">
                Hardware-accelerated, near-zero CPU
              </h3>
              <p className="mt-2 max-w-md text-sm text-muted-foreground">
                Decoding runs on the GPU through VA-API or NVDEC. Your processor
                stays free for everything else, with no loss of quality.
              </p>
            </div>
            <div className="relative mt-2 h-44 w-full overflow-hidden border-t border-border/60 sm:h-52">
              <Image
                src="https://picsum.photos/seed/fresco-decode/1000/420"
                alt="A high-resolution live wallpaper running smoothly on the desktop"
                fill
                sizes="(max-width: 1024px) 100vw, 60vw"
                className="object-cover"
              />
            </div>
          </Card>

          {/* Media types */}
          <Card className="lg:col-span-2">
            <div className="flex h-full flex-col gap-6 p-7">
              <p className="text-sm font-medium text-muted-foreground">
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

        {/* Secondary feature trio */}
        <div className="mt-5 grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {SECONDARY_FEATURES.map((feature, i) => (
            <Card
              key={feature.title}
              className={cn(
                "p-7",
                i === 2 && "sm:col-span-2 lg:col-span-1"
              )}
            >
              <FeatureRow {...feature} />
            </Card>
          ))}
        </div>
      </div>
    </section>
  );
}

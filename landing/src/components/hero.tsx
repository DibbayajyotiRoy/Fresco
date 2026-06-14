import Image from "next/image";
import { Download, Github } from "lucide-react";
import { Button } from "@/components/ui/button";
import { GITHUB_URL, RELEASES_URL } from "@/lib/site";

export function Hero() {
  return (
    <section
      id="top"
      className="relative overflow-hidden border-b border-border/60"
    >
      {/* soft sunset glow, echoing the logo gradient (deep indigo to rose) */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 -top-40 mx-auto h-80 max-w-3xl"
      >
        <div className="absolute inset-0 translate-x-16 rounded-full bg-primary/25 blur-3xl" />
        <div
          className="absolute inset-0 -translate-x-16 rounded-full blur-3xl"
          style={{ backgroundColor: "rgba(58, 28, 113, 0.4)" }}
        />
      </div>

      <div className="mx-auto flex max-w-6xl flex-col items-center px-5 pb-16 pt-14 text-center sm:pt-20">
        <p className="mb-5 inline-flex items-center gap-2 rounded-full border border-border/70 bg-secondary/40 px-3 py-1 text-xs font-medium text-muted-foreground">
          <span className="size-1.5 rounded-full bg-primary" />
          Free and open source · GPL-3.0
        </p>

        <h1 className="max-w-3xl text-balance text-4xl font-semibold leading-[1.04] tracking-tighter sm:text-5xl md:text-6xl">
          Live wallpapers for Linux,
          <br className="hidden sm:block" />{" "}
          <span className="text-sunset">without the CPU tax.</span>
        </h1>

        <p className="mt-5 max-w-xl text-pretty text-base text-muted-foreground sm:text-lg">
          Set a video, GIF, image, slideshow, or playlist as your desktop.
          Hardware-accelerated playback keeps CPU near zero.
        </p>

        <div className="mt-8 flex flex-col gap-3 sm:flex-row">
          <Button asChild size="lg" className="font-medium">
            <a href={RELEASES_URL} target="_blank" rel="noopener noreferrer">
              <Download />
              Download .deb
            </a>
          </Button>
          <Button asChild size="lg" variant="outline" className="font-medium">
            <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer">
              <Github />
              View on GitHub
            </a>
          </Button>
        </div>

        {/* Framed app screenshot mockup */}
        <div className="relative mt-14 w-full max-w-4xl">
          <div className="rounded-2xl border border-border/70 bg-card/60 p-2 shadow-2xl shadow-black/40 ring-1 ring-white/5 backdrop-blur">
            <div className="flex items-center gap-1.5 px-3 py-2">
              <span className="size-2.5 rounded-full bg-red-400/70" />
              <span className="size-2.5 rounded-full bg-amber-400/70" />
              <span className="size-2.5 rounded-full bg-emerald-400/70" />
            </div>
            <div className="overflow-hidden rounded-xl border border-border/60">
              <Image
                src="https://picsum.photos/seed/fresco-desktop/1280/720"
                alt="The Fresco app showing a live video wallpaper set on the Linux desktop"
                width={1280}
                height={720}
                priority
                className="h-auto w-full"
              />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

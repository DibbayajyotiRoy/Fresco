import Image from "next/image";
import { Download, Github } from "lucide-react";
import { Button } from "@/components/ui/button";
import { AnimatedGlyph } from "@/components/animated-glyph";
import { GITHUB_URL } from "@/lib/site";

export function Hero() {
  return (
    <section id="top" className="relative border-b border-hairline">
      <div className="mx-auto flex max-w-6xl flex-col items-center px-5 pb-16 pt-16 text-center sm:pt-24">
        <p className="instrument-label mb-6 inline-flex items-center gap-2 rounded-full border border-hairline bg-surface px-3 py-1">
          <AnimatedGlyph name="scanline" className="text-accent" />
          live wallpaper · free · gpl-3.0
        </p>

        <h1 className="max-w-3xl text-balance font-serif text-display text-ink">
          Finally, live wallpapers
          <br className="hidden sm:block" /> that{" "}
          <em className="text-accent">just work</em> on Linux.
        </h1>

        <p className="mt-6 max-w-xl text-pretty text-lg text-ink-subtle">
          Set any video as your desktop, or browse the built-in catalog.
          Near-zero CPU, on X11 and Wayland. Free forever.
        </p>

        <div className="mt-8 flex flex-col gap-3 sm:flex-row">
          <Button asChild size="lg" className="font-medium">
            <a href="#download">
              <Download />
              Install Fresco
            </a>
          </Button>
          <Button asChild size="lg" variant="secondary" className="font-medium">
            <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer">
              <Github />
              View on GitHub
            </a>
          </Button>
        </div>

        {/* Product frame: the real capture, framed by 1px hairlines with a
            mono window-chrome strip — no glow, no gradients. */}
        <div className="relative mt-16 w-full max-w-4xl">
          <div className="rounded-lg border border-hairline bg-surface p-2">
            <div className="flex items-center justify-between px-2 py-1.5">
              <span className="instrument-label">fresco — library</span>
              <span className="flex items-center gap-1.5" aria-hidden>
                <span className="size-2 rounded-full border border-hairline-strong" />
                <span className="size-2 rounded-full border border-hairline-strong" />
                <span className="size-2 rounded-full border border-hairline-strong" />
              </span>
            </div>
            <div className="overflow-hidden rounded-md border border-hairline">
              <Image
                src="/screenshots/gallery.png"
                alt="Fresco wallpaper library on Linux showing video wallpapers with an active live wallpaper and hardware decode status"
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

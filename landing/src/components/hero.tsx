import Image from "next/image";
import { Download, Github } from "lucide-react";
import { Button } from "@/components/ui/button";
import { GITHUB_URL } from "@/lib/site";

export function Hero() {
  return (
    <section id="top" className="relative overflow-hidden border-b border-border">
      <div className="mx-auto flex max-w-6xl flex-col items-center px-5 pb-16 pt-16 text-center sm:pt-24">
        <p className="mb-6 inline-flex items-center rounded-full border border-border bg-surface-1 px-3 py-1 text-xs font-medium text-ink-subtle">
          Free and open source · GPL-3.0
        </p>

        <h1 className="max-w-3xl text-balance text-4xl font-semibold leading-[1.05] tracking-[-0.035em] text-ink sm:text-5xl md:text-6xl">
          Finally, live wallpapers
          <br className="hidden sm:block" />{" "}
          that <span className="text-lavender-accent">just work</span> on Linux.
        </h1>

        <p className="mt-6 max-w-xl text-pretty text-base text-ink-subtle sm:text-lg">
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

        {/* Product panel: the real app capture is the protagonist, framed
            Linear-style (surface-1 lift, hairline border, top white edge). */}
        <div className="relative mt-16 w-full max-w-4xl">
          <div className="rounded-2xl border border-border bg-surface-1 p-2 shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]">
            <div className="flex items-center gap-1.5 px-3 py-2">
              <span className="size-2.5 rounded-full bg-hairline-strong" />
              <span className="size-2.5 rounded-full bg-hairline-strong" />
              <span className="size-2.5 rounded-full bg-hairline-strong" />
            </div>
            <div className="overflow-hidden rounded-xl border border-border">
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

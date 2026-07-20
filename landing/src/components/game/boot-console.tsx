"use client";

import { MockLaptop } from "@/components/mock-laptop";
import { Download, Github } from "lucide-react";
import { Button } from "@/components/ui/button";
import { GITHUB_URL } from "@/lib/site";

export function BootConsole() {
  return (
    <section id="top" className="relative border-b border-hairline">
      <div className="mx-auto flex max-w-3xl flex-col items-center px-5 pt-10 pb-8 text-center sm:pt-14">
        <h1 className="text-balance font-serif text-2xl text-ink sm:text-display-sm">
          Finally, a Linux wallpaper{" "}
          <em className="italic text-accent">that just works.</em>
        </h1>

        <p className="mt-4 max-w-[540px] text-pretty text-base text-ink-subtle sm:text-lg">
          Set any video, GIF, or image as your Linux desktop.
          Hardware-accelerated playback keeps CPU near zero, on X11 and
          Wayland. Close the app; the daemon keeps it playing.
        </p>

        <div className="mt-6 flex flex-col gap-3 sm:flex-row">
          <Button asChild size="lg" className="font-medium">
            <a href="#download">
              <Download />
              Install Fresco
            </a>
          </Button>
          <Button
            asChild
            size="lg"
            variant="secondary"
            className="font-medium"
          >
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noopener noreferrer"
            >
              <Github />
              Star on GitHub
            </a>
          </Button>
        </div>
      </div>

      {/* Live proof: the demo video looping as a wallpaper on a pure-CSS
          laptop (single video element on the page; poster + pause under
          reduced motion; fixed aspect box = zero CLS). Width capped so the
          laptop stays believable in this wide slot. */}
      <div className="mx-auto max-w-6xl px-5 pb-16">
        <div className="mx-auto max-w-5xl">
          <MockLaptop />
        </div>
      </div>
    </section>
  );
}
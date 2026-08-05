/* Server component: it has no interactivity of its own (<MockLaptop /> carries
   its own "use client"), so the dictionary never crosses the client boundary. */
import { MockLaptop } from "@/components/mock-laptop";
import { Download, Github } from "lucide-react";
import { Button } from "@/components/ui/button";
import { GITHUB_URL } from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";

export function BootConsole({ dict }: { dict: Dictionary }) {
  return (
    <section id="top" className="relative border-b border-hairline">
      <div className="mx-auto flex max-w-3xl flex-col items-center px-5 pt-10 pb-8 text-center sm:pt-14">
        <h1 className="text-balance font-serif text-2xl text-ink sm:text-display-sm">
          {dict.hero.titleLead}
          {dict.hero.titleGap}
          <em className="italic text-accent">{dict.hero.titleEm}</em>
        </h1>

        <p className="mt-4 max-w-[540px] text-pretty text-base text-ink-subtle sm:text-lg">
          {dict.hero.body}
        </p>

        <div className="mt-6 flex flex-col gap-3 sm:flex-row">
          <Button asChild size="lg" className="font-medium">
            <a href="#download">
              <Download />
              {dict.hero.install}
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
              {dict.hero.star}
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

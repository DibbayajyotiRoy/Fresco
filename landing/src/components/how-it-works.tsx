import { FolderOpen, MousePointerClick, X } from "lucide-react";
import type { Dictionary } from "@/lib/i18n";

/**
 * Step order, icon, and the mono command line under each step. The commands
 * are literal CLI strings, so they read the same in every language.
 */
const STEPS = [
  { id: "pick", n: "01", command: "fresco — add wallpaper.mp4", Icon: FolderOpen },
  { id: "set", n: "02", command: "fresco — set-as-wallpaper", Icon: MousePointerClick },
  { id: "close", n: "03", command: "frescod — detach", Icon: X },
] as const;

export function HowItWorks({ dict }: { dict: Dictionary }) {
  return (
    <section
      id="how-it-works"
      className="border-b border-hairline bg-surface py-20 sm:py-28"
    >
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="instrument-label !text-ink-faint">
            {dict.howItWorks.kicker}
          </p>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            {dict.howItWorks.title}
          </h2>
          <p className="mt-4 max-w-2xl text-pretty text-ink-subtle">
            {dict.howItWorks.lead}
          </p>
        </div>

        <ol className="relative mt-14 grid gap-x-10 gap-y-12 md:grid-cols-3">
          <div
            aria-hidden
            className="pointer-events-none absolute left-[15%] right-[15%] top-6 hidden h-px bg-accent/30 md:block"
          />

          {STEPS.map((step) => {
            const Icon = step.Icon;
            const copy = dict.howItWorks.steps[step.id];
            return (
              <li
                key={step.n}
                className="group relative flex flex-col items-start text-left md:items-center md:text-center"
              >
                <div className="relative z-10 flex size-12 items-center justify-center rounded-md border border-hairline bg-raised text-ink-muted transition-colors group-hover:border-accent/40">
                  <Icon className="size-5" aria-hidden />
                  <span className="absolute -right-2 -top-2 flex size-5 items-center justify-center rounded-full border border-hairline bg-paper font-mono text-meta tabular-nums text-ink-subtle">
                    {Number(step.n)}
                  </span>
                </div>
                <span className="instrument-label mt-5">
                  {dict.howItWorks.step(step.n)}
                </span>
                <h3 className="mt-2 text-lg font-semibold text-ink">
                  {copy.title}
                </h3>
                <p className="mt-2 max-w-xs text-sm text-ink-subtle">
                  {copy.description}
                </p>
                <code className="mt-3 block font-mono text-sm text-ink-faint">
                  {step.command}
                </code>
              </li>
            );
          })}
        </ol>
      </div>
    </section>
  );
}

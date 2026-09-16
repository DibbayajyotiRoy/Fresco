import "@/styles/spec.css";
import { ArrowRight, FolderOpen, MonitorPlay, MousePointerClick } from "lucide-react";
import type { Dictionary } from "@/lib/i18n";
import { SpecHead } from "@/components/spec/spec-head";

/** Step order and icon; the copy comes from the dictionary. */
const STEPS = [
  { id: "pick", n: "01", Icon: FolderOpen },
  { id: "set", n: "02", Icon: MousePointerClick },
  { id: "close", n: "03", Icon: MonitorPlay },
] as const;

/**
 * Three steps in a row (stacked on mobile), revealed with a light stagger.
 * The <ol> carries the order for assistive tech; the visible step label and
 * the connector arrows are decorative.
 */
export function HowItWorks({ dict }: { dict: Dictionary }) {
  return (
    <section
      id="how-it-works"
      aria-labelledby="how-it-works-title"
      className="border-b border-hairline py-24 sm:py-32"
    >
      <div className="wrap">
        <div className="mx-auto max-w-6xl">
          <SpecHead
            id="how-it-works-title"
            kicker={dict.howItWorks.kicker}
            title={dict.howItWorks.title}
            lead={dict.howItWorks.lead}
          />

          <ol data-reveal="stagger" className="mt-14 grid gap-4 sm:mt-16 md:grid-cols-3 md:gap-6">
            {STEPS.map(({ id, n, Icon }, i) => {
              const copy = dict.howItWorks.steps[id];
              return (
                <li key={id} className="spec-card relative flex flex-col p-6 sm:p-8">
                  {/* Connector into this step from the previous one. Owned by
                      the later card so it paints above the earlier one. */}
                  {i > 0 ? (
                    <span
                      aria-hidden
                      className="absolute top-1/2 -left-[26px] hidden size-7 -translate-y-1/2 place-items-center rounded-full border border-hairline bg-paper text-ink-faint md:grid"
                    >
                      <ArrowRight className="size-3.5" />
                    </span>
                  ) : null}

                  <div className="flex items-center justify-between gap-4">
                    <span
                      aria-hidden
                      className="grid size-11 place-items-center rounded-[10px] bg-accent/10 text-accent"
                    >
                      <Icon className="size-5" />
                    </span>
                    <span
                      aria-hidden
                      className="block text-sm font-medium tabular-nums text-ink-faint first-letter:uppercase"
                    >
                      {dict.howItWorks.step(n)}
                    </span>
                  </div>

                  <h3 className="mt-6 text-xl font-semibold text-ink">{copy.title}</h3>
                  <p className="mt-2 text-lg text-ink-subtle">{copy.description}</p>
                </li>
              );
            })}
          </ol>
        </div>
      </div>
    </section>
  );
}

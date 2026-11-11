import { LiteYouTube } from "@/components/lite-youtube";
import { SplitWords } from "@/components/motion/split-words";
import { TUTORIAL } from "@/lib/videos";
import type { Dictionary } from "@/lib/i18n";
import "@/styles/finale.css";

/**
 * One community tutorial, in Romanian, by a third-party creator. Same facade
 * as the demo reel (<LiteYouTube />): no YouTube script or cookie loads until
 * someone presses play. The title stays verbatim and the label and credit are
 * translated per locale.
 */
export function TutorialVideo({ dict }: { dict: Dictionary }) {
  const t = dict.tutorial;

  return (
    <section
      id="tutorial"
      aria-labelledby="tutorial-title"
      className="border-b border-hairline py-24 sm:py-32"
    >
      <div className="wrap">
        <div className="mx-auto max-w-3xl">
          <h2
            id="tutorial-title"
            data-reveal="words"
            className="font-display text-section text-ink"
          >
            <SplitWords text={t.title} />
          </h2>

          <article className="finale-card mt-12 overflow-hidden rounded-xl border border-hairline bg-surface sm:mt-16">
            <LiteYouTube video={TUTORIAL} playLabel={t.play(TUTORIAL.title)} />

            <div className="p-5 sm:p-6">
              <span className="inline-block rounded-full bg-accent/10 px-2.5 py-0.5 text-sm font-medium text-accent first-letter:uppercase">
                {t.label}
              </span>
              <h3 className="mt-3 text-xl font-semibold tracking-tight text-ink">
                <a
                  href={TUTORIAL.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  hrefLang="ro"
                  dir="auto"
                  className="rounded-sm transition-colors duration-150 hover:text-accent focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                >
                  {TUTORIAL.title}
                </a>
              </h3>
              <p className="mt-2 text-base leading-6 text-ink-subtle">
                <a
                  href={TUTORIAL.channel}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="rounded-sm underline-offset-4 transition-colors duration-150 hover:text-accent hover:underline focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                >
                  {t.by(TUTORIAL.author)}
                </a>
              </p>
            </div>
          </article>
        </div>
      </div>
    </section>
  );
}

import { ArrowUpRight } from "lucide-react";
import { LiteYouTube } from "@/components/lite-youtube";
import { SplitWords } from "@/components/motion/split-words";
import { CHANNEL_URL, VIDEOS, watchUrl } from "@/lib/videos";
import type { Dictionary } from "@/lib/i18n";
import "@/styles/finale.css";

/**
 * The demo reel: two short screen recordings in clean cards, placed just
 * before Supported/Download, where people decide.
 *
 * Every player is a facade (see <LiteYouTube />): no YouTube script or cookie
 * loads unless someone presses play.
 *
 * Video titles stay in English on every locale: they are the verbatim titles
 * of the uploads people land on if they click through, and the schema.org
 * VideoObject has to match what YouTube serves. Only the tag and the blurb,
 * which are ours, are translated.
 */
export function VideoShowcase({ dict }: { dict: Dictionary }) {
  const v = dict.videos;

  return (
    <section
      id="demos"
      aria-labelledby="demos-title"
      className="border-b border-hairline py-24 sm:py-32"
    >
      <div className="wrap">
        <div className="mx-auto max-w-6xl">
          <header className="flex flex-col gap-6 md:flex-row md:items-end md:justify-between md:gap-12">
            <div className="max-w-2xl">
              <h2
                id="demos-title"
                data-reveal="words"
                className="font-display text-section text-ink"
              >
                <SplitWords text={v.title} />
              </h2>
              <p
                data-reveal="fade"
                data-delay="0.15"
                className="mt-5 text-lg text-ink-subtle"
              >
                {v.lead}
              </p>
            </div>
            <a
              href={CHANNEL_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex h-10 shrink-0 items-center gap-1.5 self-start rounded-[10px] border border-hairline-strong bg-surface px-4 text-base font-medium text-ink transition-colors duration-150 hover:bg-raised md:self-auto"
            >
              {v.more}
              <ArrowUpRight className="size-4 text-ink-faint" aria-hidden />
            </a>
          </header>

          <ul
            data-reveal="stagger"
            className="mt-12 grid gap-6 sm:mt-16 md:grid-cols-2"
          >
            {VIDEOS.map((video) => {
              const copy = v.items[video.id as keyof typeof v.items];
              return (
                <li key={video.id}>
                  <article className="finale-card finale-lift flex h-full flex-col overflow-hidden rounded-xl border border-hairline bg-surface">
                    <LiteYouTube video={video} playLabel={v.play(video.title)} />

                    <div className="flex flex-1 flex-col p-5 sm:p-6">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="inline-block rounded-full bg-accent/10 px-2.5 py-0.5 text-sm font-medium text-accent first-letter:uppercase">
                          {copy.tag}
                        </span>
                        {video.preview ? (
                          <span className="inline-block rounded-full border border-hairline px-2.5 py-0.5 text-sm text-ink-subtle first-letter:uppercase">
                            {v.inDevelopment}
                          </span>
                        ) : null}
                        <span className="ml-auto text-sm tabular-nums text-ink-faint">
                          {video.runtime}
                        </span>
                      </div>

                      <h3 className="mt-3 text-xl font-semibold tracking-tight text-ink">
                        <a
                          href={watchUrl(video.id)}
                          target="_blank"
                          rel="noopener noreferrer"
                          hrefLang="en"
                          className="transition-colors duration-150 hover:text-accent"
                        >
                          {video.title}
                        </a>
                      </h3>
                      <p className="mt-2 text-base leading-6 text-ink-subtle">
                        {copy.blurb}
                      </p>
                    </div>
                  </article>
                </li>
              );
            })}
          </ul>
        </div>
      </div>
    </section>
  );
}

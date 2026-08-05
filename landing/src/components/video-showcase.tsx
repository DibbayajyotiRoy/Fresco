import { ArrowUpRight } from "lucide-react";
import { LiteYouTube } from "@/components/lite-youtube";
import { CHANNEL_URL, VIDEOS, watchUrl } from "@/lib/videos";
import type { Dictionary } from "@/lib/i18n";

/**
 * The demo reel. Sits after HowItWorks: by then the reader knows what Fresco
 * is and how little work it takes, so moving footage is proof rather than
 * introduction — and it lands immediately before Supported/Download, where
 * they decide.
 *
 * Every player is a facade (see <LiteYouTube />): no YouTube script loads
 * unless someone presses play.
 *
 * Video titles stay in English on every locale: they are the verbatim titles
 * of the uploads people will land on if they click through, and the schema.org
 * VideoObject has to match what YouTube serves. Only the tag and the blurb,
 * which are ours, are translated.
 */
export function VideoShowcase({ dict }: { dict: Dictionary }) {
  return (
    <section id="demos" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
          <div className="max-w-2xl">
            <p className="instrument-label !text-ink-faint">
              {dict.videos.kicker}
            </p>
            <h2 className="mt-3 font-serif text-display-sm text-ink">
              {dict.videos.title}
            </h2>
            <p className="mt-4 text-pretty text-ink-subtle">
              {dict.videos.lead}
            </p>
          </div>
          <a
            href={CHANNEL_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex shrink-0 items-center gap-1 font-mono text-meta uppercase tracking-widest text-ink-subtle transition-colors hover:text-ink"
          >
            {dict.videos.more}
            <ArrowUpRight className="size-3.5" aria-hidden />
          </a>
        </div>

        <ul className="mt-12 grid gap-x-8 gap-y-12 md:grid-cols-2">
          {VIDEOS.map((video) => {
            const copy = dict.videos.items[video.id as keyof typeof dict.videos.items];
            return (
              <li key={video.id} className="group/video flex flex-col">
                <LiteYouTube
                  video={video}
                  playLabel={dict.videos.play(video.title)}
                />

                <div className="mt-5 flex items-center gap-2">
                  <span className="instrument-label">{copy.tag}</span>
                  {video.preview ? (
                    <span className="inline-flex items-center rounded-sm border border-hairline bg-raised px-1.5 py-0.5 font-mono text-meta uppercase tracking-wide text-ink-faint">
                      {dict.videos.inDevelopment}
                    </span>
                  ) : null}
                </div>

                <h3 className="mt-2 text-lg font-semibold text-ink">
                  <a
                    href={watchUrl(video.id)}
                    target="_blank"
                    rel="noopener noreferrer"
                    hrefLang="en"
                    className="rounded-sm transition-colors hover:text-accent"
                  >
                    {video.title}
                  </a>
                </h3>
                <p className="mt-2 text-sm text-ink-subtle">{copy.blurb}</p>
              </li>
            );
          })}
        </ul>
      </div>
    </section>
  );
}

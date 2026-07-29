import { ArrowUpRight } from "lucide-react";
import { LiteYouTube } from "@/components/lite-youtube";
import { CHANNEL_URL, VIDEOS, watchUrl } from "@/lib/videos";

/**
 * The demo reel. Sits after HowItWorks: by then the reader knows what Fresco
 * is and how little work it takes, so moving footage is proof rather than
 * introduction — and it lands immediately before Supported/Download, where
 * they decide.
 *
 * Every player is a facade (see <LiteYouTube />): no YouTube script loads
 * unless someone presses play.
 */
export function VideoShowcase() {
  return (
    <section id="demos" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
          <div className="max-w-2xl">
            <p className="instrument-label !text-ink-faint">watch it run</p>
            <h2 className="mt-3 font-serif text-display-sm text-ink">
              Under a minute each. No narration required.
            </h2>
            <p className="mt-4 text-pretty text-ink-subtle">
              Short screen recordings of Fresco on a real desktop. Nothing
              loads from YouTube until you press play.
            </p>
          </div>
          <a
            href={CHANNEL_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex shrink-0 items-center gap-1 font-mono text-meta uppercase tracking-widest text-ink-subtle transition-colors hover:text-ink"
          >
            More on YouTube
            <ArrowUpRight className="size-3.5" aria-hidden />
          </a>
        </div>

        <ul className="mt-12 grid gap-x-8 gap-y-12 md:grid-cols-2">
          {VIDEOS.map((video) => (
            <li key={video.id} className="group/video flex flex-col">
              <LiteYouTube video={video} />

              <div className="mt-5 flex items-center gap-2">
                <span className="instrument-label">{video.tag}</span>
                {video.preview ? (
                  <span className="inline-flex items-center rounded-sm border border-hairline bg-raised px-1.5 py-0.5 font-mono text-meta uppercase tracking-wide text-ink-faint">
                    in development
                  </span>
                ) : null}
              </div>

              <h3 className="mt-2 text-lg font-semibold text-ink">
                <a
                  href={watchUrl(video.id)}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="rounded-sm transition-colors hover:text-accent"
                >
                  {video.title}
                </a>
              </h3>
              <p className="mt-2 text-sm text-ink-subtle">{video.blurb}</p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

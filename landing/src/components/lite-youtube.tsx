"use client";

import Image from "next/image";
import { useCallback, useState } from "react";
import { Play } from "lucide-react";
import {
  embedUrl,
  fallbackPosterUrl,
  posterUrl,
  type Video,
} from "@/lib/videos";

/**
 * Facade ("lite") YouTube embed. Until someone actually presses play this is
 * a poster image and a button — zero YouTube JS, zero cookies, zero third-party
 * requests. The real <iframe> is only mounted on click, and it points at
 * youtube-nocookie.com.
 *
 * Why bother: a stock YouTube iframe pulls roughly a megabyte of script per
 * embed and blocks the main thread while doing it. Two of them on a page that
 * currently ships almost nothing would be the single heaviest thing on it.
 *
 * Layout: the 16/9 box is reserved by `aspect-video` on the wrapper, so the
 * poster, the iframe, and the empty state all occupy identical space. Nothing
 * on this page ever reflows because of the video.
 *
 * Motion: the hover lift is a transform on the play badge only, and it is
 * disabled under prefers-reduced-motion (globals.css also zeroes transition
 * durations there, this is the belt to that's braces).
 */
let warmed = false;

/** Open the TCP/TLS handshakes YouTube will need, but only once, and only
 *  when the viewer signals intent by hovering or focusing the button. */
function warmConnections() {
  if (warmed || typeof document === "undefined") return;
  warmed = true;
  for (const href of [
    "https://www.youtube-nocookie.com",
    "https://www.google.com",
    "https://googlevideo.com",
    "https://i.ytimg.com",
  ]) {
    const link = document.createElement("link");
    link.rel = "preconnect";
    link.href = href;
    link.crossOrigin = "";
    document.head.appendChild(link);
  }
}

export function LiteYouTube({
  video,
  playLabel,
}: {
  video: Video;
  /** Already interpolated with the (verbatim, English) YouTube title. */
  playLabel: string;
}) {
  const [playing, setPlaying] = useState(false);
  const [poster, setPoster] = useState(() => posterUrl(video.id));

  const play = useCallback(() => setPlaying(true), []);

  return (
    <div className="relative aspect-video w-full overflow-hidden rounded-md border border-hairline bg-terminal">
      {playing ? (
        <iframe
          className="absolute inset-0 size-full"
          src={`${embedUrl(video.id)}?autoplay=1&rel=0&modestbranding=1`}
          title={video.title}
          allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
          referrerPolicy="strict-origin-when-cross-origin"
          allowFullScreen
        />
      ) : (
        <>
          <Image
            src={poster}
            alt={playLabel}
            fill
            loading="lazy"
            sizes="(min-width: 1024px) 592px, (min-width: 640px) 90vw, 100vw"
            className="object-cover"
            // maxresdefault is not generated for every upload; drop to the
            // hqdefault YouTube always produces if it 404s.
            onError={() => setPoster(fallbackPosterUrl(video.id))}
          />

          {/* Scrim: keeps the play badge legible over bright frames and gives
              the poster the same stone-dark cast as the rest of the site. */}
          <div
            aria-hidden
            className="absolute inset-0 bg-stone-950/25 transition-colors group-hover/video:bg-stone-950/10"
          />

          <button
            type="button"
            onClick={play}
            onPointerEnter={warmConnections}
            onFocus={warmConnections}
            aria-label={playLabel}
            className="group/play absolute inset-0 flex cursor-pointer items-center justify-center focus-visible:outline-2"
          >
            <span
              aria-hidden
              className="flex size-14 items-center justify-center rounded-full border border-white/25 bg-stone-950/70 text-white backdrop-blur-[2px] transition-[transform,background-color] duration-200 group-hover/play:scale-110 group-hover/play:bg-accent group-focus-visible/play:scale-110 group-focus-visible/play:bg-accent motion-reduce:transition-none motion-reduce:group-hover/play:scale-100 motion-reduce:group-focus-visible/play:scale-100 sm:size-16"
            >
              <Play className="size-6 translate-x-[2px] fill-current" />
            </span>
          </button>

          <span
            aria-hidden
            className="pointer-events-none absolute bottom-2.5 right-2.5 rounded-sm bg-stone-950/70 px-1.5 py-0.5 font-mono text-meta tabular-nums text-stone-100 backdrop-blur-[2px]"
          >
            {video.runtime}
          </span>
        </>
      )}
    </div>
  );
}

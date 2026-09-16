"use client";

import Image from "next/image";
import { useCallback, useState } from "react";
import { Play } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  embedUrl,
  fallbackPosterUrl,
  posterUrl,
  type Video,
} from "@/lib/videos";

/**
 * Facade ("lite") YouTube embed. Until someone actually presses play this is
 * a poster image and a button: zero YouTube JS, zero cookies. The real
 * <iframe> is only mounted on click, and it points at youtube-nocookie.com.
 *
 * Why bother: a stock YouTube iframe pulls roughly a megabyte of script per
 * embed and blocks the main thread while doing it. Two of them would be the
 * single heaviest thing on the page.
 *
 * Layout: the 16/9 box is reserved by `aspect-video`, so the poster and the
 * iframe occupy identical space and nothing reflows because of the video.
 * The play button scales on hover/focus (transform only, off under reduced
 * motion); a light scrim fades out with it (opacity only).
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
  className,
}: {
  video: Video;
  /** Already interpolated with the (verbatim, English) YouTube title. */
  playLabel: string;
  className?: string;
}) {
  const [playing, setPlaying] = useState(false);
  const [poster, setPoster] = useState(() => posterUrl(video.id));

  const play = useCallback(() => setPlaying(true), []);

  return (
    <div
      className={cn(
        "group/yt relative aspect-video w-full overflow-hidden bg-terminal",
        className,
      )}
    >
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
            sizes="(min-width: 1200px) 560px, (min-width: 768px) 46vw, 100vw"
            className="object-cover"
            // maxresdefault is not generated for every upload; drop to the
            // hqdefault YouTube always produces if it 404s.
            onError={() => setPoster(fallbackPosterUrl(video.id))}
          />

          {/* Keeps the play button legible over bright frames. */}
          <div
            aria-hidden
            className="absolute inset-0 bg-black/15 transition-opacity duration-200 group-hover/yt:opacity-0"
          />

          <button
            type="button"
            onClick={play}
            onPointerEnter={warmConnections}
            onFocus={warmConnections}
            aria-label={playLabel}
            className="group/play absolute inset-0 flex cursor-pointer items-center justify-center focus-visible:outline-offset-[-4px]"
          >
            <span
              aria-hidden
              className="flex size-14 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-[0_8px_24px_-6px_rgb(0_0_0/0.45)] transition-transform duration-200 ease-out group-hover/play:scale-105 group-focus-visible/play:scale-105 motion-reduce:transition-none motion-reduce:group-hover/play:scale-100 motion-reduce:group-focus-visible/play:scale-100 sm:size-16"
            >
              <Play className="size-6 translate-x-[2px] fill-current" />
            </span>
          </button>
        </>
      )}
    </div>
  );
}

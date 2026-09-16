"use client";

import { useEffect, useRef } from "react";

/** public/hero-poster.* was extracted at this time (ffmpeg -ss 6). */
/* The loop was trimmed to start 3s into the source clip (its opening is a
   near-white frame), so the poster frame, extracted at 6s of the source,
   now sits at 3s. */
const POSTER_TIME = 3;

/**
 * The demo wallpaper, looping inside the hero's desktop-window frame.
 *
 * Policy: muted + playsInline, never sound. It plays only while in view and
 * while the tab is visible. Under prefers-reduced-motion it is paused and
 * parked on the poster's frame (the clip's own first frame is near-white).
 *
 * It also flags the enclosing .hero with `data-paused` whenever it stops, so
 * the hero's one CSS loop (the live dot's pulse, which sits above the video)
 * pauses with it instead of needing an observer of its own.
 */
export function DemoVideo() {
  const ref = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = ref.current;
    if (!video) return;
    const hero = video.closest<HTMLElement>(".hero");
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");
    let inView = true;

    /* React does not reliably server-render the `muted` attribute, and
       browsers only autoplay muted media. */
    video.muted = true;

    const apply = () => {
      const run = inView && !document.hidden && !reduce.matches;
      hero?.toggleAttribute("data-paused", !run);
      if (reduce.matches) {
        video.pause();
        const park = () => {
          try {
            video.currentTime = POSTER_TIME;
          } catch {
            /* not seekable yet; the poster still shows */
          }
        };
        if (video.readyState >= 1) park();
        else video.addEventListener("loadedmetadata", park, { once: true });
        return;
      }
      if (run) video.play().catch(() => {});
      else video.pause();
    };

    const io = new IntersectionObserver(
      (entries) => {
        inView = entries[0]?.isIntersecting ?? true;
        apply();
      },
      { rootMargin: "120px" },
    );
    io.observe(video);
    apply();
    reduce.addEventListener("change", apply);
    document.addEventListener("visibilitychange", apply);
    return () => {
      io.disconnect();
      reduce.removeEventListener("change", apply);
      document.removeEventListener("visibilitychange", apply);
    };
  }, []);

  return (
    <div className="relative aspect-[16/10] bg-raised">
      <video
        ref={ref}
        poster="/hero-poster.webp"
        autoPlay
        loop
        muted
        playsInline
        preload="metadata"
        disablePictureInPicture
        aria-label="Demo video wallpaper looping on a Linux desktop"
        className="absolute inset-0 size-full object-cover object-[50%_30%]"
      >
        <source src="/demo-wallpaper.webm" type='video/webm; codecs="av01.0.05M.08"' />
        <source src="/demo-wallpaper.mp4" type="video/mp4" />
      </video>
    </div>
  );
}

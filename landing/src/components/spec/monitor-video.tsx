"use client";

import { useEffect, useRef } from "react";

/**
 * One wallpaper playing on a screen of the multi-monitor mock. Decorative:
 * the enclosing mock is aria-hidden and the hero carries the labelled video.
 *
 * `name` picks public/wallpapers/<name>.{webm,mp4,webp}: short silent loops
 * cut from the maintainer's own Fresco library, AV1 first with an H.264
 * fallback, each poster being its loop's first frame.
 *
 * Policy, same as the hero: muted + playsInline, never sound, preload="none"
 * so nothing downloads until it is actually on screen. It plays only while
 * in view and while the tab is visible. Under prefers-reduced-motion it never
 * starts, so the poster shows; if it was playing it pauses and rewinds to the
 * poster frame.
 */
export function MonitorVideo({
  name,
  className,
}: {
  name: string;
  className?: string;
}) {
  const ref = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = ref.current;
    if (!video) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");
    let inView = false;

    /* React does not reliably server-render `muted`; autoplay needs it. */
    video.muted = true;

    const apply = () => {
      if (reduce.matches) {
        if (!video.paused) video.pause();
        if (video.readyState >= 1 && video.currentTime !== 0) {
          try {
            video.currentTime = 0;
          } catch {
            /* not seekable; the current frame stays */
          }
        }
        return;
      }
      if (inView && !document.hidden) video.play().catch(() => {});
      else if (!video.paused) video.pause();
    };

    const io = new IntersectionObserver(
      (entries) => {
        inView = entries[entries.length - 1]?.isIntersecting ?? false;
        apply();
      },
      { threshold: 0.15 },
    );
    io.observe(video);
    reduce.addEventListener("change", apply);
    document.addEventListener("visibilitychange", apply);
    return () => {
      io.disconnect();
      reduce.removeEventListener("change", apply);
      document.removeEventListener("visibilitychange", apply);
      video.pause();
    };
  }, []);

  return (
    <video
      ref={ref}
      poster={`/wallpapers/${name}.webp`}
      muted
      loop
      playsInline
      preload="none"
      tabIndex={-1}
      disablePictureInPicture
      className={className}
    >
      <source src={`/wallpapers/${name}.webm`} type='video/webm; codecs="av01.0.05M.08"' />
      <source src={`/wallpapers/${name}.mp4`} type="video/mp4" />
    </video>
  );
}

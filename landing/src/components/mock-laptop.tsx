"use client";

import { useEffect, useRef } from "react";

/**
 * Pure-CSS mock laptop with the demo wallpaper looping on its screen —
 * "this video runs as your wallpaper". House grammar only: stone surfaces,
 * 1px hairlines, 8/12 radii, no stock imagery. A slim top bar and three dock
 * dots hint at a desktop so the video reads as a wallpaper, not a clip.
 *
 * Motion rules: muted + playsInline, never any sound. Under
 * prefers-reduced-motion the video is paused on its first frame (poster jpg
 * pre-extracted with ffmpeg serves as the static fallback before metadata
 * loads). preload="metadata" + a fixed aspect box keep LCP/CLS safe.
 */
export function MockLaptop() {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const apply = () => {
      if (mq.matches) {
        video.pause();
        try {
          video.currentTime = 0;
        } catch {
          /* metadata not ready yet */
        }
      } else {
        video.play().catch(() => {});
      }
    };
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, []);

  return (
    <figure className="mx-auto w-full">
      {/* Lid: bezel + screen */}
      <div className="mx-auto w-[88%] rounded-t-lg border border-hairline-strong border-b-0 bg-stone-800 p-1.5 shadow-none dark:bg-stone-900 sm:p-2">
        <div className="relative aspect-[16/10] overflow-hidden rounded-sm bg-terminal">
          <video
            ref={videoRef}
            src="/demo-wallpaper.mp4"
            poster="/demo-wallpaper-poster.jpg"
            autoPlay
            loop
            muted
            playsInline
            preload="metadata"
            className="absolute inset-0 size-full object-cover"
            aria-label="Demo video wallpaper looping on a Linux desktop"
          />
          {/* Fake desktop hints: slim top bar + dock dots, subtle stone-on-dark */}
          <div
            aria-hidden
            className="absolute inset-x-0 top-0 flex h-[7%] min-h-4 items-center justify-between bg-stone-950/45 px-2 backdrop-blur-[2px]"
          >
            <span className="h-1 w-8 rounded-full bg-stone-400/50" />
            <span className="flex items-center gap-1">
              <span className="size-1 rounded-full bg-stone-400/50" />
              <span className="h-1 w-3 rounded-full bg-stone-400/50" />
            </span>
          </div>
          <div
            aria-hidden
            className="absolute inset-x-0 bottom-[4%] flex justify-center"
          >
            <span className="flex items-center gap-1.5 rounded-full bg-stone-950/45 px-2 py-1 backdrop-blur-[2px]">
              <span className="size-1.5 rounded-sm bg-stone-300/60" />
              <span className="size-1.5 rounded-sm bg-stone-300/60" />
              <span className="size-1.5 rounded-sm bg-stone-400/40" />
            </span>
          </div>
        </div>
      </div>
      {/* Deck / base */}
      <div className="relative mx-auto h-3 rounded-b-md border border-hairline-strong bg-raised sm:h-3.5">
        <span
          aria-hidden
          className="absolute left-1/2 top-0 h-1 w-16 -translate-x-1/2 rounded-b-sm bg-hairline"
        />
      </div>
      <figcaption className="mt-5 text-center font-serif text-lg italic text-ink-muted">
        Your wallpaper, <em className="not-italic text-accent">alive</em>. Looping right on the desktop.
      </figcaption>
    </figure>
  );
}

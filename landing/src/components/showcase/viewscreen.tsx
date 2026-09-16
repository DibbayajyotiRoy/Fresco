"use client";

import { useEffect, useRef, useState } from "react";
import { DISC, NOS, NP, VIZ, lu } from "@/components/showcase/desk";
import {
  Disc,
  NosClock,
  NowPlaying,
  Visualiser,
} from "@/components/showcase/widgets";

/* Placeholder track lines, written for this page: not from any song. */
const LYRICS = [
  "Streetlights hum in time",
  "the city keeps the beat",
  "we fold the night in half",
  "and drive it home slow",
  "every window burns gold",
  "the radio remembers us",
];
/** Seconds each placeholder line holds before the next one takes over. */
const LINE_SECONDS = 4;

/**
 * The What's New screen: the demo wallpaper in a plain window frame, with
 * replicas of Fresco's four widgets laid out as they sit on the real desktop
 * (clock top right, record mid right, lyric card bottom left, visualiser
 * bottom right). Decoration only: the whole frame is aria-hidden, and the
 * hero carries the labelled video.
 *
 * Widgets are plain DOM layers marked `data-widget`, visible in the server
 * markup; showcase.tsx hides and paints them in on desktop scroll.
 *
 * While the screen is in view and the tab is visible, a 1 Hz tick keeps the
 * clock on the visitor's local time. The video, the bars, the turning record
 * and the lyric lines additionally need motion allowed; otherwise the poster
 * and a still frame stand in.
 */
export function Viewscreen() {
  const screenRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [now, setNow] = useState<Date | null>(null);
  const [line, setLine] = useState(0);

  useEffect(() => {
    const screen = screenRef.current;
    const video = videoRef.current;
    if (!screen || !video) return;

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");
    let inView = false;
    let timer = 0;
    let ticks = 0;

    const tick = () => {
      const d = new Date();
      setNow(d);
      if (!reduce.matches && ++ticks % LINE_SECONDS === 0) {
        setLine((n) => (n + 1) % LYRICS.length);
      }
      timer = window.setTimeout(tick, 1005 - d.getMilliseconds());
    };

    const sync = () => {
      const visible = inView && !document.hidden;
      const live = visible && !reduce.matches;
      screen.toggleAttribute("data-playing", live);
      if (live) video.play().catch(() => {});
      else if (!video.paused) video.pause();
      window.clearTimeout(timer);
      timer = 0;
      if (visible) tick();
    };

    const io = new IntersectionObserver(
      (entries) => {
        inView = entries[entries.length - 1]?.isIntersecting ?? false;
        sync();
      },
      { threshold: 0.15 },
    );
    io.observe(screen);
    document.addEventListener("visibilitychange", sync);
    reduce.addEventListener("change", sync);

    return () => {
      io.disconnect();
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", sync);
      reduce.removeEventListener("change", sync);
      screen.removeAttribute("data-playing");
      video.pause();
    };
  }, []);

  return (
    <div
      aria-hidden
      data-reveal="fade"
      className="sc-frame rounded-2xl border border-hairline bg-surface p-2"
    >
      <div
        ref={screenRef}
        className="sc-screen relative aspect-video overflow-hidden rounded-lg"
      >
        <video
          ref={videoRef}
          poster="/hero-poster.webp"
          muted
          loop
          playsInline
          preload="none"
          tabIndex={-1}
          className="absolute inset-0 size-full object-cover"
        >
          <source
            src="/demo-wallpaper.webm"
            type='video/webm; codecs="av01.0.05M.08"'
          />
          <source src="/demo-wallpaper.mp4" type="video/mp4" />
        </video>

        <div className="sc-desk absolute inset-0">
          <div
            data-widget="clock"
            className="absolute"
            style={{ left: lu(NOS.left), top: lu(NOS.top) }}
          >
            <NosClock now={now} />
          </div>
          <div
            data-widget="disc"
            className="absolute"
            style={{ left: lu(DISC.left), top: lu(DISC.top) }}
          >
            <Disc />
          </div>
          <div
            data-widget="lyrics"
            className="absolute"
            style={{ left: lu(NP.left), top: lu(NP.top) }}
          >
            <NowPlaying
              lyric={LYRICS[line]}
              next={LYRICS[(line + 1) % LYRICS.length]}
            />
          </div>
          <div
            data-widget="visualizer"
            className="absolute"
            style={{ left: lu(VIZ.left), top: lu(VIZ.top) }}
          >
            <Visualiser />
          </div>
        </div>
      </div>
    </div>
  );
}

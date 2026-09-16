"use client";

import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { Pause, Play } from "lucide-react";

/** Drift speed, px per second. Slow enough to read a quote as it passes. */
const SPEED = 30;

/**
 * One marquee row. Children are the groups, rendered by the server: first
 * the real semantic <ul>, then `copies` aria-hidden duplicates. Each group
 * carries its trailing gap as padding, so the track is exactly
 * (1 + copies) periods wide and the CSS keyframe loops seamlessly by
 * shifting it -100 / (1 + copies) percent.
 *
 * Compositor-only: the drift is a CSS animation on transform. This
 * component does no per-frame work; it only
 *   - sets the duration (period / SPEED) from a ResizeObserver, so on
 *     first layout and on resize, never per frame;
 *   - flags the row as running while it intersects the viewport and the
 *     tab is visible (IntersectionObserver + visibilitychange). Hover and
 *     focus-within pause it in CSS (animation-play-state);
 *   - holds the visitor's explicit pause (WCAG 2.2.2) from the toggle
 *     below the row. That choice wins over every automatic resume.
 * Without JS, or under reduced motion, the row is a static wrapped grid and
 * the toggle is hidden (CSS shows it only next to a live row).
 */
export function MarqueeRow({
  direction,
  copies,
  pauseLabel,
  playLabel,
  children,
}: {
  direction: "left" | "right";
  copies: number;
  pauseLabel: string;
  playLabel: string;
  children: ReactNode;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    const row = root.current;
    const track = row?.firstElementChild as HTMLElement | null | undefined;
    const list = track?.firstElementChild as HTMLElement | null | undefined;
    if (!row || !list) return;

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");
    let inView = false;
    let enabled = false;

    const sync = () => {
      if (enabled && inView && !document.hidden) row.dataset.tmRun = "";
      else delete row.dataset.tmRun;
    };
    const ro = new ResizeObserver(() => {
      const period = list.offsetWidth;
      if (period > 0) {
        row.style.setProperty("--tm-dur", `${Math.max(12, period / SPEED).toFixed(2)}s`);
      }
    });
    const io = new IntersectionObserver((entries) => {
      inView = entries[entries.length - 1]?.isIntersecting ?? false;
      sync();
    });

    const enable = () => {
      if (enabled) return;
      enabled = true;
      row.dataset.tmLive = "";
      ro.observe(list);
      io.observe(row);
      document.addEventListener("visibilitychange", sync);
    };
    const disable = () => {
      if (!enabled) return;
      enabled = false;
      inView = false;
      ro.disconnect();
      io.disconnect();
      document.removeEventListener("visibilitychange", sync);
      delete row.dataset.tmLive;
      delete row.dataset.tmRun;
      row.style.removeProperty("--tm-dur");
    };
    const apply = () => (reduce.matches ? disable() : enable());

    apply();
    reduce.addEventListener("change", apply);
    return () => {
      reduce.removeEventListener("change", apply);
      disable();
    };
  }, []);

  const label = paused ? playLabel : pauseLabel;

  return (
    <>
      <div
        ref={root}
        className="tm-row"
        data-dir={direction}
        data-tm-paused={paused ? "" : undefined}
      >
        <div
          className="tm-track"
          style={{ "--tm-shift": `${(-100 / (copies + 1)).toFixed(4)}%` } as CSSProperties}
        >
          {children}
        </div>
      </div>
      <div className="wrap tm-controls">
        <button
          type="button"
          className="tm-toggle"
          aria-label={label}
          title={label}
          onClick={() => setPaused((p) => !p)}
        >
          {paused ? <Play aria-hidden /> : <Pause aria-hidden />}
        </button>
      </div>
    </>
  );
}

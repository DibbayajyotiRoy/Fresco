"use client";

import * as React from "react";
import dynamic from "next/dynamic";

import { countryName } from "@/lib/geo";
import { formatNumber } from "@/lib/format";

/**
 * Where Fresco actually runs, on a globe.
 *
 * Two layers over the same data. The choropleth answers "which countries at
 * all" at a glance — a lit country has users, and its shade and altitude
 * encode how many. The markers answer "where exactly", because at 1:110m
 * resolution Hong Kong and Singapore have no polygon to light up and would
 * otherwise be invisible on a dashboard where they are real users.
 *
 * This file is the chrome; `install-globe-canvas.tsx` is the WebGL. Loading
 * the canvas through `next/dynamic({ ssr: false })` is not optional —
 * react-globe.gl touches `window` at import time, so it cannot be rendered on
 * the server — and it keeps three.js out of the bundle for every page that
 * does not show a globe.
 */

const GlobeCanvas = dynamic(() => import("./install-globe-canvas"), {
  ssr: false,
  loading: () => <GlobeSkeleton />,
});

export type CountryCount = { code: string; count: number };

function GlobeSkeleton() {
  return (
    <div className="flex aspect-square w-full items-center justify-center">
      <div className="size-3/4 animate-pulse rounded-full bg-stone-100 dark:bg-stone-800" />
    </div>
  );
}

/** Sizes the canvas to its container — react-globe.gl needs explicit pixels. */
function useMeasuredWidth() {
  const ref = React.useRef<HTMLDivElement>(null);
  const [width, setWidth] = React.useState(0);

  React.useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      setWidth(Math.round(entry.contentRect.width));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return [ref, width] as const;
}

/** The dashboard theme is a class toggled on <html> at runtime, so the globe
 *  has to watch for it rather than read it once at mount. */
function useDarkMode() {
  const [dark, setDark] = React.useState(false);

  React.useEffect(() => {
    const read = () =>
      setDark(document.documentElement.classList.contains("dark"));
    read();
    const observer = new MutationObserver(read);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });
    return () => observer.disconnect();
  }, []);

  return dark;
}

export function InstallGlobe({
  counts,
  className,
}: {
  counts: CountryCount[];
  className?: string;
}) {
  const [wrapRef, width] = useMeasuredWidth();
  const dark = useDarkMode();
  const [hovered, setHovered] = React.useState<string | null>(null);

  const hoveredCount = React.useMemo(() => {
    if (!hovered) return 0;
    return counts.find((c) => c.code === hovered)?.count ?? 0;
  }, [hovered, counts]);

  const total = React.useMemo(
    () => counts.reduce((s, c) => s + c.count, 0),
    [counts]
  );

  return (
    <div ref={wrapRef} className={className}>
      <div className="relative">
        {width > 0 ? (
          <GlobeCanvas
            counts={counts}
            size={width}
            dark={dark}
            onHover={setHovered}
          />
        ) : (
          <GlobeSkeleton />
        )}

        {/* Hover readout. The canvas tooltip follows the cursor and is easy to
            miss; this stays in one place so the figure is always legible. */}
        <div className="pointer-events-none absolute inset-x-0 bottom-0 flex items-end justify-between gap-2">
          <span className="font-mono text-meta tracking-wide text-stone-400 uppercase">
            drag to spin
          </span>
          {hovered && hoveredCount > 0 ? (
            <span className="rounded-md border border-stone-200 bg-white/95 px-2 py-1 text-sm text-stone-900 backdrop-blur dark:border-stone-700 dark:bg-stone-900/95 dark:text-stone-100">
              {countryName(hovered)}{" "}
              <span className="font-mono tabular-nums text-stone-500">
                {formatNumber(hoveredCount)}
              </span>
            </span>
          ) : (
            <span className="font-mono text-meta tracking-wide text-stone-400 uppercase tabular-nums">
              {formatNumber(total)} located
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

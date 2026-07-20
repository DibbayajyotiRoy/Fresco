"use client";

import { useEffect, useRef, useState } from "react";

/**
 * Animates a number from 0 to `value` when it scrolls into view. The SSR
 * markup renders the final value (crawlers and no-JS users see the real
 * number); the animation is progressive enhancement and is skipped under
 * reduced motion. Renders a plain hyphen when value is null.
 */
export function CountUp({
  value,
  className = "",
  suffix = "",
}: {
  value: number | null;
  className?: string;
  suffix?: string;
}) {
  const [display, setDisplay] = useState(value ?? 0);
  const ref = useRef<HTMLSpanElement>(null);
  const started = useRef(false);

  useEffect(() => {
    if (value === null) return;
    const el = ref.current;
    if (!el) return;

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce || typeof IntersectionObserver === "undefined") {
      setDisplay(value);
      return;
    }

    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting && !started.current) {
            started.current = true;
            const duration = 1300;
            let start = 0;
            const tick = (ts: number) => {
              if (!start) start = ts;
              const p = Math.min((ts - start) / duration, 1);
              const eased = 1 - Math.pow(1 - p, 3);
              setDisplay(Math.round(value * eased));
              if (p < 1) requestAnimationFrame(tick);
            };
            requestAnimationFrame(tick);
            io.disconnect();
          }
        }
      },
      { threshold: 0.4 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [value]);

  // Data honesty (§7): missing value renders as a greyed em-dash, never blank.
  if (value === null)
    return <span className={`text-ink-faint ${className}`}>{"—"}</span>;

  return (
    <span ref={ref} className={className}>
      {display.toLocaleString("en-US")}
      {suffix}
    </span>
  );
}

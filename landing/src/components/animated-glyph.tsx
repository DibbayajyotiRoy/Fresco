"use client";

import { useEffect, useState } from "react";
import spinnerData from "unicode-animations";
import { cn } from "@/lib/utils";

/**
 * Braille glyph system (Warm Terminal §5). Frame data comes from the
 * `unicode-animations` package (each spinner already ships as
 * `{ frames: string[], interval: ms }`, matching the spec's shape); this
 * wrapper keeps the house API on top: a fixed 1ch box so motion slots into
 * text rhythm, `active={false}` (or prefers-reduced-motion) freezes frame 0
 * and stops the timer. Motion = "we're checking"; a frozen glyph = settled.
 */
type SpinnerSpec = { frames: readonly string[]; interval: number };

/* The spec's core cast (§5), sourced from the package. */
export const SPINNERS: Record<
  "braille" | "breathe" | "pulse" | "scan" | "scanline" | "snake" | "dna",
  SpinnerSpec
> = {
  braille: spinnerData.braille,
  breathe: spinnerData.breathe,
  pulse: spinnerData.pulse,
  scan: spinnerData.scan,
  scanline: spinnerData.scanline,
  snake: spinnerData.snake,
  dna: spinnerData.dna,
};

export type SpinnerName = keyof typeof SPINNERS;

export function useSpinner(name: SpinnerName, active: boolean): string {
  const spec = SPINNERS[name];
  const [frame, setFrame] = useState(0);

  useEffect(() => {
    if (!active) {
      setFrame(0);
      return;
    }
    if (
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      setFrame(0);
      return;
    }
    const id = window.setInterval(
      () => setFrame((f) => (f + 1) % spec.frames.length),
      spec.interval,
    );
    return () => window.clearInterval(id);
  }, [active, spec]);

  return spec.frames[active ? frame : 0];
}

export function AnimatedGlyph({
  name,
  active = true,
  staticChar,
  title,
  className,
}: {
  name: SpinnerName;
  active?: boolean;
  /** Character shown when inactive (defaults to frame 0). */
  staticChar?: string;
  title?: string;
  className?: string;
}) {
  const frame = useSpinner(name, active);
  return (
    <span
      aria-hidden={title ? undefined : true}
      title={title}
      className={cn(
        "inline-block min-w-[1ch] text-center font-mono leading-none tabular-nums",
        className,
      )}
    >
      {active ? frame : (staticChar ?? frame)}
    </span>
  );
}

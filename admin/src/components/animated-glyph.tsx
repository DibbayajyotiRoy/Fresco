"use client";

import * as React from "react";
import { spinners } from "unicode-animations";

/* Braille glyph system (§5), backed by the unicode-animations package —
 * each spinner is a { frames: string[], interval: ms } record, matching the
 * spec's shape 1:1. We only expose the house cast. */

export type SpinnerName =
  | "braille"
  | "breathe"
  | "pulse"
  | "scan"
  | "scanline"
  | "snake"
  | "dna";

export function getSpinner(name: SpinnerName) {
  return spinners[name];
}

/** Current frame of a named braille spinner. Timer runs only while `active`;
 *  when inactive the glyph freezes on frame 0 (settled = frozen). Respects
 *  prefers-reduced-motion by never starting the timer. */
export function useSpinner(name: SpinnerName, active: boolean): string {
  const { frames, interval } = spinners[name];
  const [i, setI] = React.useState(0);

  React.useEffect(() => {
    if (
      !active ||
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      setI(0);
      return;
    }
    const id = setInterval(() => {
      setI((prev) => (prev + 1) % frames.length);
    }, interval);
    return () => clearInterval(id);
  }, [active, frames.length, interval]);

  return frames[i % frames.length];
}

/** A braille glyph in a fixed 1ch box — motion that slots into text rhythm.
 *  Motion says "we're checking"; color says the answer; frozen = settled. */
export function AnimatedGlyph({
  name = "braille",
  active,
  staticChar,
  title,
  className,
}: {
  name?: SpinnerName;
  active: boolean;
  /** Character shown when inactive (defaults to the spinner's frame 0). */
  staticChar?: string;
  title?: string;
  className?: string;
}) {
  const frame = useSpinner(name, active);
  const char = active ? frame : (staticChar ?? spinners[name].frames[0]);
  return (
    <span
      aria-hidden
      title={title}
      className={
        "inline-block min-w-[1ch] text-center font-mono leading-none tabular-nums " +
        (className ?? "")
      }
    >
      {char}
    </span>
  );
}

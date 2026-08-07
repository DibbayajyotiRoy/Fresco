"use client";

import { AnimatedGlyph } from "@/components/animated-glyph";
import { cn } from "@/lib/utils";

/** Dashed "awaiting data" box — visually distinct from a real card, so an
 *  empty panel reads as intentional, never broken (§7). */
export function EmptyState({
  title,
  description,
  className,
}: {
  title: string;
  description?: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        // No shadow, unlike Panel: the dashed hairline is doing the work of
        // saying "this is a placeholder, not a card", and elevating it would
        // promote an absence to a surface.
        "flex flex-col items-center justify-center gap-1.5 rounded-lg border border-dashed border-stone-300 bg-stone-50/50 px-6 py-10 text-center",
        className
      )}
    >
      {/* font-medium + stone-500 is the house ribbon: the same weight and
          colour step as the ribbons on Notice and ErrorPanel, so the three
          "something to tell you" boxes read as one family. */}
      <p className="font-mono text-meta font-medium tracking-widest text-stone-500 uppercase select-none">
        <AnimatedGlyph name="dna" active={false} staticChar="⠿" /> awaiting data
      </p>
      <p className="text-sm font-medium text-stone-800">{title}</p>
      {description ? (
        <p className="max-w-sm text-sm text-stone-500">{description}</p>
      ) : null}
    </div>
  );
}

/** Inline loading state: live braille glyph + mono label. */
export function LoadingState({
  label = "loading",
  className,
}: {
  label?: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex items-center justify-center gap-2 py-8 font-mono text-meta font-medium tracking-widest text-stone-500 uppercase",
        className
      )}
      role="status"
    >
      <AnimatedGlyph name="braille" active />
      {label}
    </div>
  );
}

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
        "flex flex-col items-center justify-center gap-1.5 rounded-lg border border-dashed border-stone-300 bg-stone-50/50 px-6 py-10 text-center",
        className
      )}
    >
      <p className="font-mono text-meta tracking-widest text-stone-400 uppercase">
        <AnimatedGlyph name="dna" active={false} staticChar="⠿" /> awaiting data
      </p>
      <p className="text-sm font-medium text-stone-700">{title}</p>
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
        "flex items-center justify-center gap-2 py-8 font-mono text-meta tracking-widest text-stone-400 uppercase",
        className
      )}
      role="status"
    >
      <AnimatedGlyph name="braille" active />
      {label}
    </div>
  );
}

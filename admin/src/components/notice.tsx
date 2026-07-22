"use client";

import { AnimatedGlyph } from "@/components/animated-glyph";
import { cn } from "@/lib/utils";

/**
 * Inline diagnostic for a metric that is zero because the pipeline is broken,
 * not because nothing happened. A dashboard of honest zeros is indistinguishable
 * from a dead product, so where the data itself proves an inconsistency
 * (events arriving from installs that were never recorded) we say so, and say
 * what to do about it, rather than letting the reader draw the wrong
 * conclusion.
 */
export function Notice({
  label,
  title,
  children,
  className,
}: {
  label: string;
  title: string;
  children?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "rounded-lg border border-amber-600/30 bg-amber-600/5 px-4 py-3 dark:bg-amber-600/10",
        className
      )}
      role="status"
    >
      <p className="flex items-center gap-2 font-mono text-meta tracking-widest text-amber-600 uppercase">
        <AnimatedGlyph name="pulse" active={false} staticChar="!" />
        {label}
      </p>
      <p className="mt-1.5 text-sm font-medium text-stone-900">{title}</p>
      {children ? (
        <div className="mt-1 text-sm leading-relaxed text-stone-600">
          {children}
        </div>
      ) : null}
    </div>
  );
}

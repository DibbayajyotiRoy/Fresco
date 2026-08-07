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
  tone = "warn",
}: {
  label: string;
  title: string;
  children?: React.ReactNode;
  className?: string;
  /**
   * `warn` = something is broken and a number below is wrong because of it.
   * `info` = the number below is correct but reads oddly, and the reader
   * would otherwise mis-diagnose it. Keeping these visually distinct matters:
   * if an explanation looks like a fault, every explanation gets ignored.
   */
  tone?: "warn" | "info";
}) {
  const warn = tone === "warn";
  return (
    <div
      // shadow-panel matches ErrorPanel and Panel: an explanation that sits
      // flat on the page next to elevated cards reads as debris left behind
      // by a failed render rather than something we chose to say.
      className={cn(
        "rounded-lg border px-4 py-3 shadow-panel",
        warn
          ? "border-amber-600/30 bg-amber-600/5 dark:bg-amber-600/10"
          : "border-sky-600/30 bg-sky-600/5 dark:bg-sky-600/10",
        className
      )}
      role="status"
    >
      <p
        className={cn(
          "flex items-center gap-2 font-mono text-meta font-medium tracking-widest uppercase select-none",
          warn ? "text-amber-600" : "text-sky-600"
        )}
      >
        <AnimatedGlyph
          name="pulse"
          active={false}
          staticChar={warn ? "!" : "i"}
        />
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

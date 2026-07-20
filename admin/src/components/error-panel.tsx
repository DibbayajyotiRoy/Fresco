"use client";

import { AnimatedGlyph } from "@/components/animated-glyph";
import { cn } from "@/lib/utils";

type ErrorKind = "upstream" | "down" | "unknown";

/** Classify a fetch error message so the panel tone matches the failure. */
function classify(message: string): ErrorKind {
  const m = message.toLowerCase();
  if (/(github api|api \d{3}|rate limit|status|40[0-9]|50[0-9])/.test(m))
    return "upstream";
  if (/(failed to reach|network|fetch|econn|timeout|set supabase)/.test(m))
    return "down";
  return "unknown";
}

const TONES: Record<ErrorKind, { box: string; ribbon: string; label: string }> =
  {
    upstream: {
      box: "border-amber-600/30 bg-amber-600/5 dark:bg-amber-600/10",
      ribbon: "text-amber-600",
      label: "upstream error",
    },
    down: {
      box: "border-rose-500/30 bg-rose-500/5 dark:bg-rose-500/10",
      ribbon: "text-rose-500",
      label: "source unreachable",
    },
    unknown: {
      box: "border-stone-300 bg-stone-50/50",
      ribbon: "text-stone-400",
      label: "error",
    },
  };

/** Failed-fetch panel: tone by classified kind, mono ribbon, honest message. */
export function ErrorPanel({
  title,
  message,
  className,
}: {
  title: string;
  message: string;
  className?: string;
}) {
  const tone = TONES[classify(message)];
  return (
    <div
      className={cn("rounded-lg border px-4 py-4", tone.box, className)}
      role="alert"
    >
      <p
        className={cn(
          "flex items-center gap-2 font-mono text-meta tracking-widest uppercase",
          tone.ribbon
        )}
      >
        <AnimatedGlyph name="pulse" active={false} staticChar="!" />
        {tone.label}
      </p>
      <p className="mt-1.5 text-sm font-medium text-stone-900">{title}</p>
      <p className="mt-0.5 font-mono text-sm break-words text-stone-500">
        {message}
      </p>
    </div>
  );
}

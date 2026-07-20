import { cn } from "@/lib/utils";

/* ── Severity ramp — status ONLY, never chrome (§2.1) ─────────────────────*/

export type Severity = "ok" | "info" | "warning" | "error" | "critical";

const SEVERITY: Record<Severity, { dot: string; text: string }> = {
  ok: { dot: "bg-emerald-500", text: "text-emerald-600 dark:text-emerald-400" },
  info: { dot: "bg-gray-500", text: "text-stone-500" },
  warning: { dot: "bg-amber-600", text: "text-amber-600" },
  error: { dot: "bg-orange-600", text: "text-orange-600" },
  critical: { dot: "bg-red-600", text: "text-red-600" },
};

/** Severity dot + optional 11px uppercase mono label. Status only. */
export function SeverityBadge({
  severity,
  label,
  className,
}: {
  severity: Severity;
  label?: string;
  className?: string;
}) {
  const tone = SEVERITY[severity];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 font-mono text-meta tracking-wide uppercase",
        tone.text,
        className
      )}
    >
      <span className={cn("size-1.5 rounded-full", tone.dot)} aria-hidden />
      {label ?? severity}
    </span>
  );
}

/* ── Categorical Badge — hash-toned pill, never severity, never interactive.
     Full literal class strings for JIT safety. ─────────────────────────────*/

const BADGE_TONES = [
  "border-blue-600/30 bg-blue-600/5 text-blue-700 dark:bg-blue-400/10 dark:text-blue-300",
  "border-green-600/30 bg-green-600/5 text-green-700 dark:bg-green-400/10 dark:text-green-300",
  "border-amber-600/30 bg-amber-600/5 text-amber-700 dark:bg-amber-400/10 dark:text-amber-300",
  "border-orange-600/30 bg-orange-600/5 text-orange-700 dark:bg-orange-400/10 dark:text-orange-300",
  "border-red-600/30 bg-red-600/5 text-red-700 dark:bg-red-400/10 dark:text-red-300",
  "border-purple-600/30 bg-purple-600/5 text-purple-700 dark:bg-purple-400/10 dark:text-purple-300",
  "border-pink-600/30 bg-pink-600/5 text-pink-700 dark:bg-pink-400/10 dark:text-pink-300",
  "border-teal-600/30 bg-teal-600/5 text-teal-700 dark:bg-teal-400/10 dark:text-teal-300",
  "border-indigo-600/30 bg-indigo-600/5 text-indigo-700 dark:bg-indigo-400/10 dark:text-indigo-300",
  "border-stone-400/40 bg-stone-100 text-stone-600 dark:bg-stone-400/10 dark:text-stone-300",
] as const;

/** Deterministic label -> tone hash so a category keeps its color forever. */
function hashTone(label: string): string {
  let h = 0;
  for (let i = 0; i < label.length; i++) {
    h = (h * 31 + label.charCodeAt(i)) | 0;
  }
  return BADGE_TONES[Math.abs(h) % BADGE_TONES.length];
}

/** Categorical pill: colored text + hairline colored border + faint tint. */
export function Badge({
  label,
  className,
}: {
  label: string;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-2 py-0.5 text-meta font-medium",
        hashTone(label),
        className
      )}
    >
      {label}
    </span>
  );
}

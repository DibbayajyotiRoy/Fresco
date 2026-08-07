import { cn } from "@/lib/utils";

/** House card: white surface, 1px stone hairline, 8px corners, dense.
 *  `.surface` carries the resting shadow and the pointer-only hover lift, so
 *  every card in the app reacts identically without re-deriving the values. */
export function Panel({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <section
      className={cn(
        // `border` with no colour utility on purpose. Tailwind's utilities
        // layer outranks its components layer, so an explicit
        // `border-stone-200` here would beat `.surface:hover` and kill the
        // border half of the hover lift. The base layer already paints every
        // border `var(--border)` — the same stone-200/stone-800 pair — so the
        // resting look is unchanged and the hover now actually fires.
        "surface rounded-lg border bg-white p-3",
        className
      )}
    >
      {children}
    </section>
  );
}

/** Panel heading: 16px section title + 11px mono instrument meta.
 *  The two are deliberately far apart in size and near in colour weight: the
 *  title is the question, the meta is the reading. Meta sits one step above
 *  the old stone-400 — 11px uppercase mono at stone-400 on white is right at
 *  the edge of legible, and this line usually carries the denominator. */
export function PanelHeader({
  title,
  meta,
}: {
  title: string;
  meta?: string;
}) {
  return (
    <div className="mb-2.5 flex items-baseline justify-between gap-3">
      {/* min-w-0 so a long title truncates instead of shoving the meta out. */}
      <h2 className="min-w-0 truncate text-lg font-medium tracking-tight text-stone-900">
        {title}
      </h2>
      {meta ? (
        <span className="shrink-0 truncate font-mono text-meta tracking-wide text-stone-500 uppercase tabular-nums">
          {meta}
        </span>
      ) : null}
    </div>
  );
}

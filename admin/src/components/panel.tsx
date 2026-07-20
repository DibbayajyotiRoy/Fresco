import { cn } from "@/lib/utils";

/** House card: white surface, 1px stone hairline, 8px corners, dense. */
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
        "rounded-lg border border-stone-200 bg-white p-3",
        className
      )}
    >
      {children}
    </section>
  );
}

/** Panel heading: 16px section title + 11px mono instrument meta. */
export function PanelHeader({
  title,
  meta,
}: {
  title: string;
  meta?: string;
}) {
  return (
    <div className="mb-2.5 flex items-baseline justify-between gap-3">
      <h2 className="text-lg font-medium text-stone-900">{title}</h2>
      {meta ? (
        <span className="truncate font-mono text-meta tracking-wide text-stone-400 uppercase tabular-nums">
          {meta}
        </span>
      ) : null}
    </div>
  );
}

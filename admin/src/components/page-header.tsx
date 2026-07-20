/** §9 page header: serif display title + mono tabular meta on one baseline. */
export function PageHeader({
  title,
  meta,
  action,
}: {
  title: string;
  /** Mono meta line — usually a count, e.g. "128 rows · 42 installs". */
  meta?: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2">
      <h1 className="font-serif text-2xl tracking-tight text-stone-900">
        {title}
      </h1>
      <div className="flex items-baseline gap-3">
        {meta ? (
          <span className="font-mono text-meta tracking-wide text-stone-400 uppercase tabular-nums">
            {meta}
          </span>
        ) : null}
        {action}
      </div>
    </div>
  );
}

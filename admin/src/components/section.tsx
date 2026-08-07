/**
 * A labelled band of panels.
 *
 * The Usage page used to be one flat run of twelve panels, which made every
 * figure look equally important and left the reader to work out which numbers
 * answered which question. Sections carry that structure: one heading per
 * question the page answers, one sentence saying how to read what follows.
 */
export function Section({
  title,
  description,
  meta,
  children,
}: {
  title: string;
  /** One line on how to read this band. Optional, but usually worth it. */
  description?: string;
  /** Right-aligned instrument reading — scope, denominator, freshness. */
  meta?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-2">
      <div className="flex items-baseline justify-between gap-3 border-t border-stone-200 pt-3">
        <h2 className="font-mono text-meta font-medium tracking-widest text-stone-500 uppercase">
          {title}
        </h2>
        {meta ? (
          <span className="shrink-0 truncate font-mono text-meta tracking-wide text-stone-400 tabular-nums">
            {meta}
          </span>
        ) : null}
      </div>
      {description ? (
        <p className="max-w-3xl text-sm leading-snug text-stone-500">
          {description}
        </p>
      ) : null}
      {children}
    </section>
  );
}

/**
 * `Section`, with a meta slot that can arrive late.
 *
 * `@/components/section` takes `meta` as a `string`, which means the heading
 * cannot be emitted until the number in it is known — and every meta on this
 * page is a count that costs a Supabase or GitHub round-trip. That is exactly
 * the wait streaming is here to remove: the heading, the description and the
 * skeletons should paint immediately and the reading should stream in with
 * its band.
 *
 * So this is the same markup with `meta` widened to a `ReactNode`, letting the
 * page pass a `<Suspense>` into the slot. Kept in lockstep with
 * `@/components/section` — if that file's header changes, change this one too.
 */
export function StreamingSection({
  title,
  description,
  meta,
  children,
}: {
  title: string;
  /** One line on how to read this band. Optional, but usually worth it. */
  description?: string;
  /** Right-aligned instrument reading — scope, denominator, freshness. */
  meta?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-2">
      <div className="flex items-baseline justify-between gap-3 border-t border-stone-200 pt-3">
        <h2 className="font-mono text-meta font-medium tracking-widest text-stone-500 uppercase">
          {title}
        </h2>
        {meta ? (
          // A `div` rather than `Section`'s `span`: the slot holds a Suspense
          // fallback, and `Skeleton` renders a div — which is invalid inside a
          // span and trips hydration. Identical as a flex item either way.
          <div className="shrink-0 truncate font-mono text-meta tracking-wide text-stone-400 tabular-nums">
            {meta}
          </div>
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

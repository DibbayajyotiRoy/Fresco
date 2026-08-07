import { cn } from "@/lib/utils";

/**
 * Placeholders for Suspense boundaries.
 *
 * Every page here talks to Supabase (~400-700ms per query) and GitHub (~1.1s),
 * and used to `await` all of them before emitting anything — so navigating to
 * Usage meant a second and a half of the previous page still on screen. These
 * hold the shape of what is coming so each section can stream in on its own.
 *
 * They mirror real layout on purpose. A skeleton that is the wrong size is
 * worse than none: the page reflows when data lands and the eye loses its
 * place. The heights below are therefore derived from the real components'
 * line boxes, not eyeballed — see the arithmetic in each one.
 */

export function Skeleton({
  className,
  style,
}: {
  className?: string;
  style?: React.CSSProperties;
}) {
  return (
    <div
      // `.shimmer` sweeps left-to-right over the stone fill instead of pulsing
      // the whole block: a directional wipe reads as "this is being fetched",
      // a pulse reads as "this is blinking at you". Dark mode comes from the
      // global bg-stone-100 remap, so no `dark:` variant is needed here.
      className={cn("shimmer rounded-sm bg-stone-100", className)}
      style={style}
      aria-hidden
    />
  );
}

/** Matches `StatCard`: mono label, big figure, hint line.
 *  Height: 16 (label line) + 8 + 20 (value line) + 6 + 16 (hint line) = 66px
 *  of content. The bars are shorter than their line boxes, so the margins
 *  below are padded to keep the same 66 and stop the tile resizing when the
 *  real figure lands. */
export function StatCardSkeleton() {
  return (
    <div className="rounded-lg border border-stone-200 bg-white p-3 shadow-panel">
      <Skeleton className="h-2.5 w-20" />
      <Skeleton className="mt-3.5 h-5 w-14" />
      <Skeleton className="mt-3 h-2.5 w-28" />
    </div>
  );
}

/** A row of KPI tiles. `count` should match the real grid. */
export function StatRowSkeleton({
  count = 4,
  className = "grid grid-cols-2 gap-2 lg:grid-cols-4",
}: {
  count?: number;
  className?: string;
}) {
  return (
    <div className={className}>
      {Array.from({ length: count }, (_, i) => (
        <StatCardSkeleton key={i} />
      ))}
    </div>
  );
}

/** Matches `Panel` + `PanelHeader` with `rows` proportion bars or table rows.
 *  The header box is pinned to 22px (the 16px title's line box) and each row
 *  to 24px (18px label line + 2px gap + 4px bar), which is exactly what
 *  `DistributionList` occupies. */
export function PanelSkeleton({
  rows = 5,
  className,
}: {
  rows?: number;
  className?: string;
}) {
  return (
    <section
      className={cn(
        "rounded-lg border border-stone-200 bg-white p-3 shadow-panel",
        className
      )}
    >
      <div className="mb-2.5 flex h-5.5 items-center justify-between gap-3">
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-2.5 w-20" />
      </div>
      <div className="space-y-2">
        {Array.from({ length: rows }, (_, i) => (
          <div key={i} className="space-y-0.5">
            <div className="flex h-4.5 items-center justify-between gap-3">
              {/* Staggered widths so it reads as content, not a loading bar. */}
              <Skeleton className="h-3" style={{ width: `${58 - i * 6}%` }} />
              <Skeleton className="h-2.5 w-10" />
            </div>
            <Skeleton className="h-1 w-full rounded-full" />
          </div>
        ))}
      </div>
    </section>
  );
}

/** Square placeholder for the globe, which is both slow and large. */
export function GlobeSkeletonPanel() {
  return (
    <section className="rounded-lg border border-stone-200 bg-white p-3 shadow-panel">
      <div className="mb-2.5 flex h-5.5 items-center justify-between gap-3">
        <Skeleton className="h-4 w-36" />
        <Skeleton className="h-2.5 w-20" />
      </div>
      <div className="mx-auto flex aspect-square w-full max-w-[420px] items-center justify-center">
        <Skeleton className="size-3/4 rounded-full" />
      </div>
    </section>
  );
}

/** Full-page placeholder for `loading.tsx`: header plus a KPI row.
 *  The header box is pinned to the 28px serif title's 32px line box. */
export function PageSkeleton({
  stats = 4,
  panels = 2,
}: {
  stats?: number;
  panels?: number;
}) {
  return (
    <div className="space-y-3">
      <div className="flex h-8 items-center justify-between gap-3">
        <Skeleton className="h-7 w-40" />
        <Skeleton className="h-3 w-48" />
      </div>
      <StatRowSkeleton count={stats} />
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        {Array.from({ length: panels }, (_, i) => (
          <PanelSkeleton key={i} />
        ))}
      </div>
    </div>
  );
}

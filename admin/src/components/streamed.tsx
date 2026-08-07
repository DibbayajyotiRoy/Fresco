import { cn } from "@/lib/utils";

/**
 * Wraps the resolved contents of a Suspense boundary so a band fades and
 * rises the last 4px into place instead of snapping in.
 *
 * Placement matters and is easy to get wrong. This goes *inside* the boundary,
 * around the async component:
 *
 *     <Suspense fallback={<StatRowSkeleton />}>
 *       <Streamed><ReachSection /></Streamed>
 *     </Suspense>
 *
 * Wrapping the `<Suspense>` itself would animate the skeleton on first paint
 * and then do nothing at all on the swap — which is the moment worth covering.
 * Inside, the whole subtree mounts only when the data lands, so the animation
 * fires exactly then.
 *
 * No `animation-delay` anywhere: each boundary resolves when its own query
 * returns, and those are already 300-1100ms apart, so the stagger is real
 * rather than choreographed. Hand-tuned delays on top would only make fast
 * sections wait for no reason.
 *
 * The `space-y-3` is the band's internal rhythm. It has to live here because
 * wrapping previously-sibling elements in a div would otherwise collapse the
 * spacing the parent `<Section>` was providing.
 */
export function Streamed({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div className={cn("section-in space-y-3", className)}>{children}</div>
  );
}

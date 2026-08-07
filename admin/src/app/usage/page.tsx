import { Suspense } from "react";

import { PageHeader } from "@/components/page-header";
import { Streamed } from "@/components/streamed";
import {
  GlobeSkeletonPanel,
  PanelSkeleton,
  Skeleton,
  StatRowSkeleton,
} from "@/components/skeleton";

import { ActivityMeta, ActivitySection } from "./sections/activity-section";
import { DownloadsMeta, DownloadsSection } from "./sections/downloads-section";
import {
  EnvironmentMeta,
  EnvironmentSection,
} from "./sections/environment-section";
import { UsageHeaderMeta } from "./sections/header-meta";
import { ReachMeta, ReachSection } from "./sections/reach-section";
import { StreamingSection } from "./sections/section-shell";
import { DAY_MS } from "./sections/shared";
import { WhereMeta, WhereSection } from "./sections/where-section";

export const dynamic = "force-dynamic";
export const revalidate = 0;

/** Placeholder for a section meta, which is always a short mono count. */
const metaFallback = <Skeleton className="h-2.5 w-32" />;

/**
 * Usage — five bands, five Suspense boundaries.
 *
 * This component is deliberately synchronous: it awaits nothing, so the
 * headings, the descriptions and the skeletons are in the HTML on the first
 * flush and the previous page goes away immediately. Each band then fetches
 * its own data and streams in when that data lands, in whatever order the
 * network decides — which matters most for "Downloads vs installs", whose
 * GitHub call costs about a second on its own and used to hold up the four
 * KPI cards at the top of the page behind it.
 *
 * No band passes data to another. Sections that need the same query just call
 * it; the data layer dedupes identical calls within a render, so five
 * `getInstalls()` calls are one query. That is why the clock below is computed
 * here and handed down: the 30-day cutoff has to be byte-identical in every
 * section for those calls to look identical.
 *
 * No daily_country fetch: that table is the consent-revision-1 tally, and
 * since revision 2 the essential tier writes a real install row instead. The
 * rows already in it are historical and are deliberately not mixed into these
 * counts, which would double-count anyone who spanned both revisions.
 */
export default function UsagePage() {
  const now = Date.now();
  const since30d = new Date(now - 30 * DAY_MS).toISOString();
  const time = { now, since30d };

  return (
    <div className="space-y-4">
      <PageHeader
        title="Usage"
        // The count belongs in `meta`, but that prop is a string and would
        // hold the title back for two round-trips; `action` takes a node, so
        // it can hold a boundary instead.
        action={
          <Suspense fallback={<Skeleton className="h-2.5 w-44" />}>
            <UsageHeaderMeta since30d={since30d} />
          </Suspense>
        }
      />

      {/* ── How many people ────────────────────────────────────────────── */}
      <StreamingSection
        title="Reach"
        description="A distinct count of install ids in each window — not extrapolated, and not a request count. Both consent tiers register an install id, so nobody is missing from these figures except users who never answered the consent dialog, who send nothing at all."
        meta={<Suspense fallback={metaFallback}><ReachMeta /></Suspense>}
      >
        <Suspense fallback={<StatRowSkeleton count={4} />}>
          <Streamed>
            <ReachSection {...time} />
          </Streamed>
        </Suspense>
      </StreamingSection>

      {/* ── Where they are ─────────────────────────────────────────────── */}
      <StreamingSection
        title="Where"
        description="Country is resolved server-side from the network edge under both consent tiers, so it cannot be spoofed and nobody is excluded for declining the optional statistics. City is optional-tier only and is client-supplied — fine for a chart, never to be trusted."
        meta={<Suspense fallback={metaFallback}><WhereMeta /></Suspense>}
      >
        <Suspense
          fallback={
            <>
              <div className="grid grid-cols-1 gap-3 lg:grid-cols-5">
                <div className="lg:col-span-3">
                  <GlobeSkeletonPanel />
                </div>
                <div className="flex flex-col gap-3 lg:col-span-2">
                  <PanelSkeleton rows={6} />
                  <PanelSkeleton rows={5} />
                </div>
              </div>
              <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
                <PanelSkeleton rows={6} />
                <PanelSkeleton rows={4} />
                <PanelSkeleton rows={2} />
              </div>
            </>
          }
        >
          <Streamed>
            <WhereSection />
          </Streamed>
        </Suspense>
      </StreamingSection>

      {/* ── Downloads vs installs ──────────────────────────────────────── */}
      <StreamingSection
        title="Downloads vs installs"
        description="These two never match, and should not be expected to. GitHub counts asset fetches; telemetry counts machines that checked in. The arithmetic below is the whole reconciliation."
        meta={<Suspense fallback={metaFallback}><DownloadsMeta /></Suspense>}
      >
        <Suspense
          fallback={
            <>
              <StatRowSkeleton count={4} />
              <PanelSkeleton rows={6} />
            </>
          }
        >
          <Streamed>
            <DownloadsSection {...time} />
          </Streamed>
        </Suspense>
      </StreamingSection>

      {/* ── What they do ───────────────────────────────────────────────── */}
      <StreamingSection
        title="What people do"
        description="Feature events, from full-consent installs only. A row reading “never used” is a real zero, not a gap — every instrumented event is listed whether or not it has ever fired."
        meta={
          <Suspense fallback={metaFallback}>
            <ActivityMeta since30d={since30d} />
          </Suspense>
        }
      >
        <Suspense
          fallback={
            <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
              <PanelSkeleton rows={8} />
              <div className="flex flex-col gap-3">
                <PanelSkeleton rows={4} />
                <PanelSkeleton rows={6} />
              </div>
            </div>
          }
        >
          <Streamed>
            <ActivitySection {...time} />
          </Streamed>
        </Suspense>
      </StreamingSection>

      {/* ── What they run it on ────────────────────────────────────────── */}
      <StreamingSection
        title="Environment"
        description="Full-consent installs only. Percentages are of every install, so these bars deliberately do not sum to 100% — the shortfall is the essential-tier cohort, whose machine details were never collected."
        meta={<Suspense fallback={metaFallback}><EnvironmentMeta /></Suspense>}
      >
        <Suspense
          fallback={
            <>
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
                {Array.from({ length: 6 }, (_, i) => (
                  <PanelSkeleton key={i} rows={5} />
                ))}
              </div>
              <Skeleton className="h-2.5 w-full max-w-3xl" />
            </>
          }
        >
          <Streamed>
            <EnvironmentSection {...time} />
          </Streamed>
        </Suspense>
      </StreamingSection>
    </div>
  );
}

import { Suspense } from "react";

import {
  PanelSkeleton,
  ReleasesPanelSkeleton,
  StatCardSkeleton,
} from "@/components/skeleton";
import {
  OverviewHeader,
  OverviewHeaderFallback,
} from "@/app/_sections/overview-header";
import {
  FeedbackCards,
  OpenIssuesCard,
  ReleaseCards,
  StarsCard,
} from "@/app/_sections/overview-kpis";
import { ReleasesPanel } from "@/app/_sections/overview-releases";
import { FeedbackBreakdowns } from "@/app/_sections/overview-breakdowns";
import {
  LatestNotificationsPanel,
  RecentFeedbackPanel,
} from "@/app/_sections/overview-activity";

export const dynamic = "force-dynamic";
export const revalidate = 0;

/**
 * Composition only — this component awaits nothing.
 *
 * Every band is an async server component behind its own `<Suspense>`, so the
 * shell paints immediately and each panel fills in as its query lands instead
 * of the whole page waiting on the slowest of them (GitHub releases, ~1.1s).
 * The data functions are cached per request, so the sections that read the
 * same source still cost one round-trip between them.
 */
export default function OverviewPage() {
  return (
    <div className="space-y-3">
      <Suspense fallback={<OverviewHeaderFallback />}>
        <OverviewHeader />
      </Suspense>

      {/* KPI strip — six figures across one row at xl. */}
      <div className="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-6">
        <Suspense fallback={<StatCardSkeleton />}>
          <StarsCard />
        </Suspense>
        <Suspense
          fallback={
            <>
              <StatCardSkeleton />
              <StatCardSkeleton />
            </>
          }
        >
          <ReleaseCards />
        </Suspense>
        <Suspense
          fallback={
            <>
              <StatCardSkeleton />
              <StatCardSkeleton />
            </>
          }
        >
          <FeedbackCards />
        </Suspense>
        <Suspense fallback={<StatCardSkeleton />}>
          <OpenIssuesCard />
        </Suspense>
      </div>

      {/* Downloads beside the feedback breakdowns. */}
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
        <Suspense fallback={<ReleasesPanelSkeleton className="lg:col-span-2" />}>
          <ReleasesPanel />
        </Suspense>

        <div className="flex flex-col gap-3">
          <Suspense
            fallback={
              <>
                <PanelSkeleton rows={4} />
                <PanelSkeleton rows={4} />
              </>
            }
          >
            <FeedbackBreakdowns />
          </Suspense>
        </div>
      </div>

      {/* Recent activity. */}
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <Suspense fallback={<PanelSkeleton rows={7} />}>
          <RecentFeedbackPanel />
        </Suspense>
        <Suspense fallback={<PanelSkeleton rows={5} />}>
          <LatestNotificationsPanel />
        </Suspense>
      </div>
    </div>
  );
}

import {
  PanelSkeleton,
  ReleasesPanelSkeleton,
  Skeleton,
  StatRowSkeleton,
} from "@/components/skeleton";

/**
 * Shown the instant Overview is entered, before the server component tree even
 * starts. Shapes match the real page: a six-wide KPI strip at xl, the releases
 * panel spanning two of three columns, then the two activity panels.
 */
export default function OverviewLoading() {
  return (
    <div className="space-y-3">
      <div className="flex items-baseline justify-between gap-3">
        <Skeleton className="h-7 w-40" />
        <Skeleton className="h-3 w-48" />
      </div>

      <StatRowSkeleton
        count={6}
        className="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-6"
      />

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
        <ReleasesPanelSkeleton className="lg:col-span-2" />
        <div className="flex flex-col gap-3">
          <PanelSkeleton rows={4} />
          <PanelSkeleton rows={4} />
        </div>
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <PanelSkeleton rows={7} />
        <PanelSkeleton rows={5} />
      </div>
    </div>
  );
}

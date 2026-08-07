import {
  PanelSkeleton,
  Skeleton,
  StatRowSkeleton,
} from "@/components/skeleton";

/** Four KPIs, then the inbox: a 320px thread list beside the conversation. */
export default function SupportLoading() {
  return (
    <div className="space-y-3">
      <div className="flex items-baseline justify-between gap-3">
        <Skeleton className="h-7 w-28" />
        <Skeleton className="h-3 w-44" />
      </div>

      <StatRowSkeleton count={4} />

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-[320px_1fr]">
        <PanelSkeleton rows={6} />
        <PanelSkeleton rows={8} />
      </div>
    </div>
  );
}

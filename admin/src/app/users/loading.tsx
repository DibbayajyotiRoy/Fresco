import { PanelSkeleton, Skeleton, StatRowSkeleton } from "@/components/skeleton";

export default function UsersLoading() {
  return (
    <div className="space-y-4">
      <div className="flex h-8 items-center justify-between gap-3">
        <Skeleton className="h-7 w-24" />
        <Skeleton className="h-3 w-32" />
      </div>
      <StatRowSkeleton count={5} className="grid grid-cols-2 gap-2 lg:grid-cols-5" />
      <PanelSkeleton rows={6} />
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-5">
        <div className="lg:col-span-2">
          <PanelSkeleton rows={5} />
        </div>
        <div className="lg:col-span-3">
          <PanelSkeleton rows={6} />
        </div>
      </div>
      <PanelSkeleton rows={8} />
    </div>
  );
}

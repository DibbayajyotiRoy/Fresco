import { PanelSkeleton, Skeleton } from "@/components/skeleton";

export default function CountriesLoading() {
  return (
    <div className="space-y-4">
      <div className="flex h-8 items-center justify-between gap-3">
        <Skeleton className="h-7 w-32" />
        <Skeleton className="h-3 w-28" />
      </div>
      <PanelSkeleton rows={10} />
    </div>
  );
}

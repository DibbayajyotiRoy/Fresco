import { StatRowSkeleton } from "@/components/skeleton";
import { PageHeaderSkeleton, TableSkeleton } from "@/app/_skeletons/skeletons";

/** Three KPIs over the grouped-errors table. */
export default function ReliabilityLoading() {
  return (
    <div className="space-y-3">
      <PageHeaderSkeleton titleWidth="w-40" metaWidth="w-48" />
      <StatRowSkeleton
        count={3}
        className="grid grid-cols-2 gap-2 lg:grid-cols-3"
      />
      <TableSkeleton
        rows={9}
        cols={[
          "w-[90px]",
          "w-[180px]",
          "w-[100px]",
          "w-[80px] text-right",
          "w-[100px] text-right",
          undefined,
        ]}
      />
    </div>
  );
}

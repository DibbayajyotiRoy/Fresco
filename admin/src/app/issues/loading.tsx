import { StatRowSkeleton } from "@/components/skeleton";
import { PageHeaderSkeleton, TableSkeleton } from "@/app/_skeletons/skeletons";

/** Two KPIs on the four-column grid, then the open-issues table. */
export default function IssuesLoading() {
  return (
    <div className="space-y-3">
      <PageHeaderSkeleton titleWidth="w-28" metaWidth="w-24" />
      <StatRowSkeleton
        count={2}
        className="grid grid-cols-2 gap-2 lg:grid-cols-4"
      />
      <TableSkeleton
        rows={9}
        cols={[
          "w-[60px]",
          undefined,
          "hidden w-[190px] lg:table-cell",
          "hidden w-[120px] md:table-cell",
          "w-[70px] text-right",
          "w-[100px] text-right",
        ]}
      />
    </div>
  );
}

import { StatRowSkeleton } from "@/components/skeleton";
import {
  FilterRowSkeleton,
  PageHeaderSkeleton,
  TableSkeleton,
} from "@/app/_skeletons/skeletons";

/** Four KPIs, the sentiment filter, then the feedback table. */
export default function FeedbackLoading() {
  return (
    <div className="space-y-3">
      <PageHeaderSkeleton titleWidth="w-32" metaWidth="w-28" />
      <StatRowSkeleton count={4} />
      <FilterRowSkeleton />
      <TableSkeleton
        rows={9}
        cols={[
          "w-[90px]",
          undefined,
          "w-[110px]",
          "w-[130px]",
          "w-[90px]",
          "w-[100px] text-right",
        ]}
      />
    </div>
  );
}

import { PageHeaderSkeleton, TableSkeleton } from "@/app/_skeletons/skeletons";

/** Header with the compose button, then the notifications table. */
export default function NotificationsLoading() {
  return (
    <div className="space-y-3">
      <PageHeaderSkeleton titleWidth="w-40" metaWidth="w-40" action />
      <TableSkeleton
        rows={8}
        twoLine
        cols={[
          undefined,
          "w-[100px]",
          "w-[70px]",
          "hidden w-[170px] md:table-cell",
          "w-[90px] text-right",
          "w-[44px]",
        ]}
      />
    </div>
  );
}

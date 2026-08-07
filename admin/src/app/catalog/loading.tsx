import { PageHeaderSkeleton, TableSkeleton } from "@/app/_skeletons/skeletons";

/** No KPI strip here — a header with the add button, then the catalog table.
 *  `twoLine` because the first cell stacks a title over a mono sub-line. */
export default function CatalogLoading() {
  return (
    <div className="space-y-3">
      <PageHeaderSkeleton titleWidth="w-28" metaWidth="w-20" action />
      <TableSkeleton
        rows={10}
        twoLine
        cols={[
          undefined,
          "hidden w-[90px] xl:table-cell",
          "w-[110px]",
          "hidden w-[100px] md:table-cell",
          "w-[80px] text-right",
          "w-[80px] text-right",
          "w-[90px]",
          "w-[44px]",
        ]}
      />
    </div>
  );
}

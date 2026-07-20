import { DataTable, TBody, TD, TH, THead, TR } from "@/components/data-table";
import type { Release } from "@/lib/types";
import { formatDate, formatNumber } from "@/lib/format";

/**
 * Per-release table in the Excel grammar: mono version/date, right-aligned
 * tabular counts, share-of-total. Newest first — the exact numbers behind
 * the downloads chart.
 */
export function ReleaseTable({ releases }: { releases: Release[] }) {
  const total = releases.reduce((s, r) => s + r.downloads, 0) || 1;
  const rows = [...releases].reverse();

  return (
    <DataTable>
      <THead>
        <TR>
          <TH className="w-[110px]">Version</TH>
          <TH className="hidden sm:table-cell">Published</TH>
          <TH className="w-[110px] text-right">Downloads</TH>
          <TH className="w-[80px] text-right">Share</TH>
        </TR>
      </THead>
      <TBody>
        {rows.map((r) => {
          const share = Math.round((r.downloads / total) * 100);
          return (
            <TR key={r.tag}>
              <TD>
                <span className="truncate font-mono text-sm text-stone-900">
                  {r.tag}
                </span>
              </TD>
              <TD className="hidden sm:table-cell">
                <span className="font-mono text-sm text-stone-500">
                  {formatDate(r.publishedAt)}
                </span>
              </TD>
              <TD className="text-right text-sm text-stone-900 tabular-nums">
                {formatNumber(r.downloads)}
              </TD>
              <TD className="text-right">
                <span className="font-mono text-meta text-stone-500 tabular-nums">
                  {share}%
                </span>
              </TD>
            </TR>
          );
        })}
      </TBody>
    </DataTable>
  );
}

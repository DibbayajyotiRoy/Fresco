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
                {/* `block`, because `truncate` does nothing to an inline span:
                    a long tag used to push the fixed column open. */}
                <span className="block truncate font-mono text-sm text-stone-900">
                  {r.tag}
                </span>
              </TD>
              <TD className="hidden sm:table-cell">
                <span className="block truncate font-mono text-sm text-stone-500">
                  {formatDate(r.publishedAt)}
                </span>
              </TD>
              <TD className="text-right text-sm text-stone-900 tabular-nums">
                {formatNumber(r.downloads)}
              </TD>
              <TD className="text-right">
                <div className="flex items-center justify-end gap-1.5">
                  {/* The same 4px track as DistributionList, so "share" is a
                      picture as well as a number and the two proportion
                      displays in the app read as one idea. Dropped below sm,
                      where the column has no room for it. */}
                  <span
                    className="hidden h-1 w-6 shrink-0 overflow-hidden rounded-full bg-stone-100 sm:block"
                    aria-hidden
                  >
                    <span
                      className="block h-full rounded-full bg-sky-600 dark:bg-sky-400"
                      style={{
                        width: `${share}%`,
                        // A 2% share of a 24px track is a quarter of a pixel;
                        // floor it so a small release still shows a mark.
                        minWidth: share > 0 ? "2px" : undefined,
                      }}
                    />
                  </span>
                  <span className="font-mono text-meta text-stone-500 tabular-nums">
                    {share}%
                  </span>
                </div>
              </TD>
            </TR>
          );
        })}
      </TBody>
    </DataTable>
  );
}

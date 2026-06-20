import type { Release } from "@/lib/types";
import { formatDate, formatNumber } from "@/lib/format";

/**
 * Compact per-release table: version, publish date, download count, and a thin
 * share-of-total bar. Newest first. Pairs with the downloads chart to give the
 * exact numbers behind the trend in the same vertical space.
 */
export function ReleaseTable({ releases }: { releases: Release[] }) {
  const total = releases.reduce((s, r) => s + r.downloads, 0) || 1;
  // Newest first for the table (the chart already reads oldest -> newest).
  const rows = [...releases].reverse();

  return (
    <div className="overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-muted-foreground border-border border-b text-[11px] tracking-wide uppercase">
            <th className="py-2 pr-3 text-left font-medium">Version</th>
            <th className="hidden py-2 pr-3 text-left font-medium sm:table-cell">
              Published
            </th>
            <th className="py-2 pr-3 text-right font-medium">Downloads</th>
            <th className="w-24 py-2 text-right font-medium">Share</th>
          </tr>
        </thead>
        <tbody className="divide-border divide-y">
          {rows.map((r) => {
            const share = Math.round((r.downloads / total) * 100);
            return (
              <tr key={r.tag} className="text-foreground">
                <td className="py-2.5 pr-3 font-medium">
                  <span className="font-mono text-xs">{r.tag}</span>
                </td>
                <td className="text-muted-foreground hidden py-2.5 pr-3 text-xs sm:table-cell">
                  {formatDate(r.publishedAt)}
                </td>
                <td className="py-2.5 pr-3 text-right tabular-nums">
                  {formatNumber(r.downloads)}
                </td>
                <td className="py-2.5">
                  <div className="flex items-center justify-end gap-2">
                    <div className="bg-muted hidden h-1.5 w-12 overflow-hidden rounded-full sm:block">
                      <div
                        className="bg-brand h-full rounded-full"
                        style={{ width: `${share}%` }}
                      />
                    </div>
                    <span className="text-muted-foreground w-8 text-right text-xs tabular-nums">
                      {share}%
                    </span>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

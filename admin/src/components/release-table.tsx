import { DataTable, TBody, TD, TH, THead, TR } from "@/components/data-table";
import type { Release } from "@/lib/types";
import { formatDate, formatNumber, formatRelative } from "@/lib/format";
import { splitAssets } from "@/lib/releases";

/**
 * Per-release table in the Excel grammar: mono version/date, right-aligned
 * tabular counts, share-of-total. Newest first — the exact numbers behind
 * the downloads chart.
 *
 * The rows scroll inside the grid rather than growing it. One row per release
 * is 27px, so at two dozen releases an uncapped table was adding ~650px to a
 * panel that sits beside a fixed-height column — the page grew a screenful of
 * scroll to show numbers that were already summarised above it. Capped, every
 * release is still reachable, just without the page paying for it.
 */
export function ReleaseTable({
  releases,
  maxHeight = "13rem",
}: {
  releases: Release[];
  /** Height of the scroll box. ~7.5 rows at the default. */
  maxHeight?: string;
}) {
  const total = releases.reduce((s, r) => s + r.downloads, 0) || 1;
  const rows = [...releases].reverse();

  return (
    <DataTable maxHeight={maxHeight}>
      {/* container, not page: the header now pins to the top of the scroll
          box, so it stays put while the rows move under it. */}
      <THead sticky="container">
        <TR>
          <TH className="w-[94px]">Version</TH>
          {/* The unsized column, so `table-fixed` pools the leftover width
              here — and it earns it by carrying both readings of the date
              rather than one short string floating in dead space. */}
          <TH className="hidden lg:table-cell">Published</TH>
          <TH className="hidden w-[74px] text-right sm:table-cell">.deb</TH>
          <TH className="hidden w-[80px] text-right sm:table-cell">
            install.sh
          </TH>
          <TH className="w-[80px] text-right">Total</TH>
          <TH className="w-[78px] text-right">Share</TH>
        </TR>
      </THead>
      <TBody>
        {rows.map((r) => {
          const share = Math.round((r.downloads / total) * 100);
          const split = splitAssets(r.assets);
          return (
            <TR key={r.tag}>
              <TD>
                {/* `block`, because `truncate` does nothing to an inline span:
                    a long tag used to push the fixed column open. */}
                <span className="block truncate font-mono text-sm text-stone-900">
                  {r.tag}
                </span>
              </TD>
              <TD className="hidden lg:table-cell">
                {/* Both readings, one column: relative answers "is this
                    recent?" at a glance, absolute answers "which release was
                    that?" — and the second one is why this column deserves
                    the table's spare width instead of padding. */}
                <span className="flex min-w-0 items-baseline gap-1.5 font-mono text-sm">
                  <span className="shrink-0 text-stone-500">
                    {formatRelative(r.publishedAt)}
                  </span>
                  <span className="truncate text-stone-400">
                    {formatDate(r.publishedAt)}
                  </span>
                </span>
              </TD>
              <TD className="hidden text-right text-sm text-stone-700 tabular-nums sm:table-cell">
                <AssetCount value={split.deb} />
              </TD>
              <TD className="hidden text-right text-sm text-stone-700 tabular-nums sm:table-cell">
                <AssetCount value={split.script} />
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

/** A real zero is a fact ("nobody took this one"); it just shouldn't shout. */
function AssetCount({ value }: { value: number }) {
  return value === 0 ? (
    <span className="text-stone-400">0</span>
  ) : (
    <>{formatNumber(value)}</>
  );
}

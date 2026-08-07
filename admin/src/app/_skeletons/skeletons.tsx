import {
  DataTable,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/data-table";
import { Skeleton } from "@/components/skeleton";

/**
 * Route-level placeholders for markup the page layer owns.
 *
 * `@/components/skeleton` covers the shared primitives (panels, KPI tiles).
 * What it cannot cover is the shape of a `DataTable`, because five of the
 * eight routes end in one and a `PanelSkeleton` — proportion bars under a
 * heading — is the wrong shape for it: wrong row height, wrong rhythm, no
 * header rule. Swapping one for a real table moved the page by ~80px.
 *
 * So these build out of the real primitives instead of imitating them. A
 * `TableSkeleton` row is a `TR` with real `TD`s, which means its height is the
 * table's height by construction and stays correct if the table's density ever
 * changes.
 */

/**
 * Mirrors `PageHeader`'s box: serif 28px/32px title on one baseline with the
 * mono meta and the optional action button.
 *
 * The title bar is `h-8`, not `h-7` — the `text-2xl` line box is 32px, and a
 * 28px placeholder drops every route's first band by 4px the moment the real
 * header lands.
 */
export function PageHeaderSkeleton({
  titleWidth = "w-40",
  metaWidth = "w-44",
  action = false,
}: {
  /** Roughly the width of the real title, so the swap is not a jump sideways. */
  titleWidth?: string;
  metaWidth?: string;
  /** Reserve the trailing button (New item, New notification, GitHub). */
  action?: boolean;
}) {
  return (
    <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2">
      <Skeleton className={`h-8 ${titleWidth}`} />
      <div className="flex items-baseline gap-3">
        <Skeleton className={`h-4 ${metaWidth}`} />
        {action ? <Skeleton className="h-7 w-24 rounded-md" /> : null}
      </div>
    </div>
  );
}

/**
 * A `DataTable` with its content greyed out.
 *
 * `cols` are the real header widths — pass exactly what the table passes, or
 * the columns snap into place when the data lands. `twoLine` matches the
 * tables whose first cell stacks a title over a mono sub-line (catalog,
 * notifications): 18px + 16px inside the same `py-1`.
 */
export function TableSkeleton({
  rows = 9,
  cols,
  twoLine = false,
}: {
  rows?: number;
  cols: (string | undefined)[];
  twoLine?: boolean;
}) {
  return (
    <DataTable>
      <THead>
        <TR>
          {cols.map((c, i) => (
            <TH key={i} className={c}>
              {/* text-meta is a 16px line box; anything shorter shortens the
                  header row and the whole table rides up. */}
              <span className="flex h-4 items-center">
                <Skeleton className="h-2.5 w-12" />
              </span>
            </TH>
          ))}
        </TR>
      </THead>
      <TBody>
        {Array.from({ length: rows }, (_, r) => (
          <TR key={r}>
            {cols.map((_c, i) => (
              <TD key={i}>
                {twoLine && i === 0 ? (
                  <>
                    <span className="flex h-[18px] items-center">
                      <Skeleton className="h-3 w-3/5" />
                    </span>
                    <span className="flex h-4 items-center">
                      <Skeleton className="h-2.5 w-2/5" />
                    </span>
                  </>
                ) : (
                  <span className="flex h-[18px] items-center">
                    {/* Staggered widths so it reads as content, not a bar
                        chart. Deterministic — a random width would differ
                        between server and client render. */}
                    <Skeleton
                      className="h-3"
                      style={{ width: `${45 + ((r * 7 + i * 13) % 45)}%` }}
                    />
                  </span>
                )}
              </TD>
            ))}
          </TR>
        ))}
      </TBody>
    </DataTable>
  );
}

/** The sentiment filter row above the feedback table: label, three chips, count. */
export function FilterRowSkeleton() {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <Skeleton className="h-4 w-12" />
      {/* Chips are text-sm (18px) + py-0.5 + hairline = 24px. */}
      <Skeleton className="h-6 w-11 rounded-md" />
      <Skeleton className="h-6 w-10 rounded-md" />
      <Skeleton className="h-6 w-14 rounded-md" />
      <Skeleton className="ml-auto h-4 w-16" />
    </div>
  );
}

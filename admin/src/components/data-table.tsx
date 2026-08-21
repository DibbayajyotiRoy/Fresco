import { cn } from "@/lib/utils";

/* DataTable — the Excel grammar (§4): 1px grid on every cell, table-fixed,
 * sticky white 11px uppercase mono header with a stronger bottom border,
 * zebra white/stone-50 rows, hover stone-100, 28–32px dense rows. Columns
 * are fixed; content truncates in-cell — never a horizontal page scroll. */

export function DataTable({
  className,
  maxHeight,
  children,
}: {
  className?: string;
  /**
   * Cap the grid's height and scroll the rows inside it (e.g. "15rem").
   *
   * Without this the table grows with the row count and pushes everything
   * below it off-screen — fine for a full-page table, wrong for a panel that
   * shares a grid row with shorter neighbours. Pair it with
   * `<THead sticky="container">`, since a header pinned to the viewport is
   * meaningless once the rows scroll in their own box.
   */
  maxHeight?: string;
  children: React.ReactNode;
}) {
  const table = (
    /* tabular-nums on the table, not per cell: every column that turns out
       to hold digits then aligns by construction, instead of depending on
       each call site remembering. */
    <table className="w-full table-fixed border-collapse text-sm tabular-nums">
      {children}
    </table>
  );

  return (
    <div
      className={cn(
        // overflow-clip (not hidden) keeps corners tidy without creating a
        // scroll container, so the sticky header can pin to the viewport.
        // shadow-panel (the resting half of `.surface`) puts the grid on the
        // same plane as every Panel; no hover half, because the rows already
        // own pointer feedback and a whole table lifting under the cursor
        // fights with the row highlight.
        "overflow-clip rounded-lg border border-stone-200 bg-white shadow-panel",
        className
      )}
    >
      {maxHeight ? (
        // The scroll box is a child of the rounded border, not the border
        // itself, so the corners stay clipped while the rows move.
        <div className="overflow-y-auto overscroll-contain" style={{ maxHeight }}>
          {table}
        </div>
      ) : (
        table
      )}
    </div>
  );
}

export function THead({
  sticky = "page",
  children,
}: {
  /**
   * What the header pins to. "page" clears the 56px topbar and is right for a
   * table that scrolls with the document; "container" pins to the top of a
   * `maxHeight` scroll box, where a 56px offset would park the header
   * mid-table.
   */
  sticky?: "page" | "container";
  children: React.ReactNode;
}) {
  return (
    <thead className={sticky === "container" ? "sticky top-0 z-10" : "sticky top-14 z-10"}>
      {children}
    </thead>
  );
}

export function TH({
  className,
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) {
  return (
    <th
      className={cn(
        // stone-500, not stone-400: this is a pinned label the eye returns to
        // on every scroll, and 11px uppercase mono is already fighting for
        // legibility. select-none stops a drag-select of a column of numbers
        // from picking up the header word.
        "border border-t-0 border-stone-200 border-b-stone-300 bg-white px-2.5 py-1.5 text-left font-mono text-meta font-semibold tracking-wide text-stone-500 uppercase select-none first:border-l-0 last:border-r-0",
        className
      )}
    >
      {children}
    </th>
  );
}

export function TBody({ children }: { children: React.ReactNode }) {
  return <tbody>{children}</tbody>;
}

export function TR({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <tr
      className={cn(
        // 100ms: row highlight is a tracking aid while the eye runs across a
        // wide row, so it has to land before the eye does. The named curve
        // keeps it identical to every other hover in the app.
        "transition-colors duration-100 ease-hover odd:bg-white even:bg-stone-50 hover:bg-stone-100",
        className
      )}
    >
      {children}
    </tr>
  );
}

export function TD({
  className,
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) {
  return (
    <td
      className={cn(
        "border border-stone-200 px-2.5 py-1 align-middle first:border-l-0 last:border-r-0",
        className
      )}
    >
      {children}
    </td>
  );
}

/** Greyed em-dash null sentinel (§7). Hidden from assistive tech behind a
 *  real word: read aloud, a bare em-dash is either silence or "em dash",
 *  neither of which says "we have no value for this". select-none keeps the
 *  sentinel out of a copied column. */
export function NullCell() {
  return (
    <>
      <span className="text-stone-400 select-none" aria-hidden>
        —
      </span>
      <span className="sr-only">No value</span>
    </>
  );
}

import { cn } from "@/lib/utils";

/* DataTable — the Excel grammar (§4): 1px grid on every cell, table-fixed,
 * sticky white 11px uppercase mono header with a stronger bottom border,
 * zebra white/stone-50 rows, hover stone-100, 28–32px dense rows. Columns
 * are fixed; content truncates in-cell — never a horizontal page scroll. */

export function DataTable({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div
      className={cn(
        // overflow-clip (not hidden) keeps corners tidy without creating a
        // scroll container, so the sticky header can pin to the viewport.
        "overflow-clip rounded-lg border border-stone-200 bg-white",
        className
      )}
    >
      <table className="w-full table-fixed border-collapse text-sm">
        {children}
      </table>
    </div>
  );
}

export function THead({ children }: { children: React.ReactNode }) {
  return <thead className="sticky top-14 z-10">{children}</thead>;
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
        "border border-t-0 border-stone-200 border-b-stone-300 bg-white px-2.5 py-1.5 text-left font-mono text-meta font-semibold tracking-wide text-stone-400 uppercase first:border-l-0 last:border-r-0",
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
        "transition-colors odd:bg-white even:bg-stone-50 hover:bg-stone-100",
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

/** Greyed em-dash null sentinel (§7). */
export function NullCell() {
  return <span className="text-stone-400">—</span>;
}

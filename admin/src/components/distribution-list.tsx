import { formatNumber } from "@/lib/format";

export type DistributionItem = { label: string; value: number };

/**
 * Horizontal proportion bars, instrument-panel style: 1px stone track, data
 * bar, mono tabular counts. Sorted by value, biggest first; widest row
 * anchors the scale. Built from real counts only.
 */
export function DistributionList({
  items,
  total,
}: {
  items: DistributionItem[];
  /** Denominator for the percentage. Defaults to the sum of item values. */
  total?: number;
}) {
  const sorted = [...items].sort((a, b) => b.value - a.value);
  const max = Math.max(...sorted.map((i) => i.value), 1);
  const denom = (total ?? sorted.reduce((s, i) => s + i.value, 0)) || 1;

  return (
    <ul className="space-y-2">
      {sorted.map((item) => {
        const pct = Math.round((item.value / denom) * 100);
        return (
          <li key={item.label} className="space-y-0.5">
            <div className="flex items-baseline justify-between gap-3">
              <span className="truncate text-sm text-stone-700">
                {item.label}
              </span>
              <span className="shrink-0 font-mono text-meta text-stone-500 tabular-nums">
                {formatNumber(item.value)}
                <span className="text-stone-400"> · {pct}%</span>
              </span>
            </div>
            <div className="h-1 w-full overflow-hidden rounded-full bg-stone-100">
              <div
                className="h-full rounded-full bg-sky-600 dark:bg-sky-400"
                style={{ width: `${(item.value / max) * 100}%` }}
              />
            </div>
          </li>
        );
      })}
    </ul>
  );
}

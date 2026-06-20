import { formatNumber } from "@/lib/format";

export type DistributionItem = { label: string; value: number };

/**
 * Horizontal proportion bars — a compact way to read a breakdown (platform,
 * app version, …) at a glance. Rows are sorted by value, biggest first; the
 * widest row anchors the scale. Built from real counts only.
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
    <ul className="space-y-2.5">
      {sorted.map((item) => {
        const pct = Math.round((item.value / denom) * 100);
        return (
          <li key={item.label} className="space-y-1">
            <div className="flex items-baseline justify-between gap-3 text-xs">
              <span className="text-foreground truncate font-medium">
                {item.label}
              </span>
              <span className="text-muted-foreground shrink-0 tabular-nums">
                {formatNumber(item.value)}
                <span className="text-muted-foreground/70"> · {pct}%</span>
              </span>
            </div>
            <div className="bg-muted h-1.5 w-full overflow-hidden rounded-full">
              <div
                className="bg-brand h-full rounded-full"
                style={{ width: `${(item.value / max) * 100}%` }}
              />
            </div>
          </li>
        );
      })}
    </ul>
  );
}

import type { Icon } from "@phosphor-icons/react";

/**
 * Compact KPI tile in the Geist/Vercel idiom: a small uppercase label, one big
 * tabular figure, and an optional one-line hint. Dense by design so several
 * fit across a single row. An optional real sparkline draws to the right of the
 * value when given >= 2 points.
 */
export function StatCard({
  label,
  value,
  hint,
  icon: Icon,
  data,
}: {
  label: string;
  value: string;
  hint?: string;
  icon: Icon;
  /** Optional real series (>= 2 points). Never fabricated. */
  data?: number[];
}) {
  return (
    <div className="bg-card border-border rounded-lg border p-4">
      <div className="flex items-center justify-between gap-2">
        <span className="text-muted-foreground truncate text-[11px] font-medium tracking-wide uppercase">
          {label}
        </span>
        <Icon className="text-muted-foreground size-4 shrink-0" weight="bold" />
      </div>
      <div className="mt-3 flex items-end justify-between gap-3">
        <span className="text-foreground text-2xl leading-none font-semibold tracking-tight tabular-nums">
          {value}
        </span>
        {data && data.length >= 2 ? (
          <Sparkline data={data} />
        ) : null}
      </div>
      {hint ? (
        <p className="text-muted-foreground mt-2 truncate text-xs">{hint}</p>
      ) : null}
    </div>
  );
}

/** Minimal dependency-free sparkline. Data viz, not an icon — drawn from real
 *  points only. */
function Sparkline({ data }: { data: number[] }) {
  const w = 72;
  const h = 26;
  const max = Math.max(...data);
  const min = Math.min(...data);
  const range = max - min || 1;
  const points = data
    .map((v, i) => {
      const x = (i / (data.length - 1)) * w;
      const y = h - ((v - min) / range) * (h - 2) - 1;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");

  return (
    <svg
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      fill="none"
      className="shrink-0 overflow-visible"
      aria-hidden
    >
      <polyline
        points={points}
        stroke="var(--brand)"
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

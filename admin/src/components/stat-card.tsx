/**
 * Dense KPI tile: 11px uppercase mono instrument label, one big tabular
 * figure, optional one-line hint. Absent metrics render "—" greyed — never
 * fabricated (§7). Optional real sparkline (>= 2 points) drawn in the accent.
 */
export function StatCard({
  label,
  value,
  hint,
  data,
}: {
  label: string;
  value: string;
  hint?: string;
  /** Optional real series (>= 2 points). Never fabricated. */
  data?: number[];
}) {
  const absent = value === "—";
  return (
    <div className="rounded-lg border border-stone-200 bg-white p-3">
      <span className="block truncate font-mono text-meta font-medium tracking-widest text-stone-400 uppercase">
        {label}
      </span>
      <div className="mt-2 flex items-end justify-between gap-3">
        <span
          className={
            "text-xl leading-none font-semibold tracking-tight tabular-nums " +
            (absent ? "text-stone-400" : "text-stone-900")
          }
        >
          {value}
        </span>
        {data && data.length >= 2 ? <Sparkline data={data} /> : null}
      </div>
      {hint ? (
        <p className="mt-1.5 truncate font-mono text-meta text-stone-400">
          {hint}
        </p>
      ) : null}
    </div>
  );
}

/** Dependency-free sparkline from real points only. Accent = interactivity
 *  lane is not borrowed here — this is the data-line color from §4 charts. */
function Sparkline({ data }: { data: number[] }) {
  const w = 72;
  const h = 22;
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
        className="stroke-sky-600 dark:stroke-sky-400"
        strokeWidth={1.25}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

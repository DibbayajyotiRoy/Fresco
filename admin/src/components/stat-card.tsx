/**
 * Dense KPI tile: 11px uppercase mono instrument label, one big tabular
 * figure, optional one-line hint. Absent metrics render "—" greyed — never
 * fabricated (§7). Optional real sparkline (>= 2 points) drawn in the accent.
 *
 * Hierarchy is carried by three distinct greys rather than three sizes, since
 * the tile can't afford the height: label stone-500 (what this is), value
 * stone-900 semibold (the thing itself), hint stone-400 (footnote). The old
 * label and hint shared stone-400, which flattened the card into "small grey,
 * big black, small grey" and made the label read as an afterthought.
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
    // Bare `border`, as in Panel: an explicit border colour utility would
    // outrank `.surface:hover` and the tile would only get half its lift.
    <div className="surface rounded-lg border bg-white p-3">
      <span className="block truncate font-mono text-meta font-medium tracking-widest text-stone-500 uppercase">
        {label}
      </span>
      <div className="mt-2 flex items-end justify-between gap-3">
        <span
          className={
            "text-xl leading-none tracking-tight tabular-nums " +
            // An absent metric keeps the slot but drops the weight: at
            // semibold a stone-400 em-dash still reads as a figure you could
            // squint at. Normal weight makes it read as an empty slot, which
            // is what it is.
            (absent
              ? "font-normal text-stone-400 select-none"
              : "font-semibold text-stone-900")
          }
        >
          {absent ? (
            <>
              <span aria-hidden>{value}</span>
              <span className="sr-only">No data</span>
            </>
          ) : (
            value
          )}
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
 *  lane is not borrowed here — this is the data-line color from §4 charts.
 *
 *  Drawn 20px tall so it occupies exactly the value's line box: the trace then
 *  sits *on* the figure's baseline instead of hanging 2px below it, and the
 *  tile's height no longer depends on whether a series happens to exist. The
 *  faint area fill and the terminal dot give the line a floor and an "you are
 *  here" end — a bare 1px polyline reads as a decoration pasted on the card.
 */
function Sparkline({ data }: { data: number[] }) {
  const w = 72;
  const h = 20;
  const max = Math.max(...data);
  const min = Math.min(...data);
  const range = max - min || 1;
  const xy = data.map((v, i) => {
    const x = (i / (data.length - 1)) * w;
    const y = h - ((v - min) / range) * (h - 2) - 1;
    return [x, y] as const;
  });
  const points = xy.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  // Same trace, closed along the bottom edge, for the area wash.
  const area = `0,${h} ${points} ${w},${h}`;
  const [lastX, lastY] = xy[xy.length - 1];

  return (
    <svg
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      fill="none"
      className="shrink-0 overflow-visible"
      aria-hidden
    >
      <polygon
        points={area}
        className="fill-sky-600/10 dark:fill-sky-400/10"
        stroke="none"
      />
      <polyline
        points={points}
        className="stroke-sky-600 dark:stroke-sky-400"
        strokeWidth={1.25}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle
        cx={lastX}
        cy={lastY}
        r={1.5}
        className="fill-sky-600 dark:fill-sky-400"
      />
    </svg>
  );
}

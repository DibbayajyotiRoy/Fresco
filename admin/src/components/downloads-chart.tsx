"use client";

import { Bar } from "@/components/dither-kit/bar";
import { BarChart } from "@/components/dither-kit/bar-chart";
import { BlockLegend } from "@/components/dither-kit/block-legend";
import { Grid } from "@/components/dither-kit/grid";
import { Tooltip } from "@/components/dither-kit/tooltip";
import { XAxis } from "@/components/dither-kit/x-axis";
import { YAxis } from "@/components/dither-kit/y-axis";
import type { ChartConfig } from "@/components/dither-kit/chart-context";
import { formatNumber } from "@/lib/format";
import { splitAssets } from "@/lib/releases";
import type { Release } from "@/lib/types";

/**
 * Downloads per release as a dither-kit dot-matrix bar chart, stacked by what
 * was fetched rather than one flat bar per release.
 *
 * The stack is the point: the total alone can't distinguish a release people
 * installed from one the installer script merely walked past, and the two
 * counters move independently. Stacking costs no extra height — the same
 * box now answers "how many" and "how" at once.
 */
export function DownloadsChart({ releases }: { releases: Release[] }) {
  const data = releases.map((r) => {
    const split = splitAssets(r.assets);
    return {
      tag: r.tag,
      deb: split.deb,
      script: split.script,
      other: split.other,
    };
  });

  // "Other" earns a series only when something is actually published under it;
  // an always-on third colour that is zero on every release is a lie the
  // legend then has to carry.
  const hasOther = data.some((d) => d.other > 0);

  const config: ChartConfig = {
    deb: { label: ".deb package", color: "blue" },
    script: { label: "install.sh", color: "purple" },
    ...(hasOther ? { other: { label: "Other assets", color: "grey" } } : {}),
  };

  const totals = {
    deb: data.reduce((s, d) => s + d.deb, 0),
    script: data.reduce((s, d) => s + d.script, 0),
    ...(hasOther ? { other: data.reduce((s, d) => s + d.other, 0) } : {}),
  };

  return (
    <div className="space-y-1.5">
      {/* pr-0.5 keeps the last bar off the panel's hairline, and select-none
          stops a drag across the plot from highlighting the axis labels — the
          chart is a hover surface, not text. */}
      <div className="h-[160px] w-full pr-0.5 select-none">
        <BarChart
          data={data}
          config={config}
          stackType="stacked"
          animate={false}
        >
          <Grid strokeDasharray="3 3" />
          {/* Six ticks, not the default eight: version tags are ~7 mono
              characters, and at eight they collided into a grey smear on a
              half-width panel. Fewer, legible labels beat a complete set. */}
          <XAxis dataKey="tag" maxTicks={6} />
          <YAxis tickFormatter={(v) => formatNumber(v)} />
          <Bar dataKey="deb" variant="solid" />
          <Bar dataKey="script" variant="solid" />
          {hasOther ? <Bar dataKey="other" variant="solid" /> : null}
          <Tooltip labelKey="tag" valueFormatter={(v) => formatNumber(v)} />
        </BarChart>
      </div>
      {/* In-flow, not the overlay <Legend>: this one carries running totals,
          so it doubles as the denominator for the table below. */}
      <BlockLegend
        config={config}
        values={totals}
        valueFormatter={(v) => formatNumber(v)}
      />
    </div>
  );
}

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

/**
 * The client half of the DAU chart: dither-kit's chart primitives are client
 * components, and functions like `tickFormatter` cannot cross the server ->
 * client boundary as props, so the chart itself — and the small formatter
 * closures it needs — live here, downstream of the data-fetching server
 * component (`DauChart` in `dau-chart.tsx`), which passes only plain,
 * serialisable data in.
 */
export function DauChartClient({
  data,
  totals,
}: {
  data: { day: string; checkins: number; new: number }[];
  totals: { checkins: number; new: number };
}) {
  const config: ChartConfig = {
    checkins: { label: "Check-ins", color: "blue" },
    new: { label: "New installs", color: "purple" },
  };

  return (
    <>
      <div className="h-[180px] w-full pr-0.5 select-none">
        <BarChart data={data} config={config} stackType="default" animate={false}>
          <Grid strokeDasharray="3 3" />
          <XAxis dataKey="day" maxTicks={6} />
          <YAxis tickFormatter={(v) => formatNumber(v)} />
          <Bar dataKey="checkins" variant="solid" />
          <Bar dataKey="new" variant="solid" />
          <Tooltip labelKey="day" valueFormatter={(v) => formatNumber(v)} />
        </BarChart>
      </div>
      <BlockLegend
        config={config}
        values={totals}
        valueFormatter={(v) => formatNumber(v)}
      />
    </>
  );
}

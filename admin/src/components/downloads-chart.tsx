"use client";

import { Bar } from "@/components/dither-kit/bar";
import { BarChart } from "@/components/dither-kit/bar-chart";
import { Grid } from "@/components/dither-kit/grid";
import { Tooltip } from "@/components/dither-kit/tooltip";
import { XAxis } from "@/components/dither-kit/x-axis";
import { YAxis } from "@/components/dither-kit/y-axis";
import { formatNumber } from "@/lib/format";
import type { Release } from "@/lib/types";

/** Downloads per release as a dither-kit dot-matrix bar chart: stone
 *  gridlines, 9-10px mono axis labels, blue (accent-family) bars. */
export function DownloadsChart({ releases }: { releases: Release[] }) {
  const data = releases.map((r) => ({
    tag: r.tag,
    downloads: r.downloads,
  }));

  return (
    <div className="h-[200px] w-full">
      <BarChart
        data={data}
        config={{ downloads: { label: "Downloads", color: "blue" } }}
        animate={false}
      >
        <Grid strokeDasharray="3 3" />
        <XAxis dataKey="tag" />
        <YAxis tickFormatter={(v) => formatNumber(v)} />
        <Bar dataKey="downloads" variant="solid" />
        <Tooltip
          labelKey="tag"
          valueFormatter={(v) => formatNumber(v)}
        />
      </BarChart>
    </div>
  );
}

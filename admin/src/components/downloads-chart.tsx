"use client";

import { CartesianGrid, Line, LineChart, XAxis, YAxis } from "recharts";

import {
  type ChartConfig,
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from "@/components/ui/chart";
import type { Release } from "@/lib/types";

const chartConfig = {
  downloads: {
    label: "Downloads",
    color: "var(--brand)",
  },
} satisfies ChartConfig;

export function DownloadsChart({ releases }: { releases: Release[] }) {
  const data = releases.map((r) => ({
    tag: r.tag,
    downloads: r.downloads,
  }));

  return (
    <ChartContainer
      config={chartConfig}
      className="aspect-auto h-[280px] w-full"
    >
      <LineChart
        accessibilityLayer
        data={data}
        margin={{ left: 4, right: 8, top: 8 }}
      >
        <CartesianGrid
          vertical={false}
          strokeDasharray="3 3"
          strokeOpacity={0.15}
        />
        <XAxis
          dataKey="tag"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          minTickGap={4}
        />
        <YAxis
          tickLine={false}
          axisLine={false}
          width={40}
          allowDecimals={false}
        />
        <ChartTooltip
          cursor={{ strokeDasharray: "3 3" }}
          content={<ChartTooltipContent indicator="line" />}
        />
        <Line
          type="monotone"
          dataKey="downloads"
          stroke="var(--color-downloads)"
          strokeWidth={2}
          dot={false}
          activeDot={{ r: 4, strokeWidth: 0 }}
          isAnimationActive={false}
        />
      </LineChart>
    </ChartContainer>
  );
}

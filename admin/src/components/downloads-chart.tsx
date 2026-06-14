"use client";

import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  XAxis,
  YAxis,
} from "recharts";

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
      <ComposedChart
        accessibilityLayer
        data={data}
        margin={{ left: 4, right: 8, top: 8 }}
      >
        <defs>
          <linearGradient id="downloadsFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--color-downloads)" stopOpacity={0.25} />
            <stop offset="100%" stopColor="var(--color-downloads)" stopOpacity={0} />
          </linearGradient>
        </defs>
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
          cursor={false}
          content={<ChartTooltipContent indicator="dot" />}
        />
        <Area
          type="monotone"
          dataKey="downloads"
          stroke="none"
          fill="url(#downloadsFill)"
          isAnimationActive={false}
        />
        <Line
          type="monotone"
          dataKey="downloads"
          stroke="var(--color-downloads)"
          strokeWidth={2}
          dot={{ r: 3, strokeWidth: 0, fill: "var(--color-downloads)" }}
          activeDot={{ r: 4, strokeWidth: 0 }}
          isAnimationActive={false}
        />
      </ComposedChart>
    </ChartContainer>
  );
}

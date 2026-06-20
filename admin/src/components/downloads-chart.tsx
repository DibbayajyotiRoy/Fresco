"use client";

import { Bar, BarChart, CartesianGrid, XAxis, YAxis } from "recharts";

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
    <ChartContainer config={chartConfig} className="aspect-auto h-[200px] w-full">
      <BarChart
        accessibilityLayer
        data={data}
        margin={{ left: 4, right: 8, top: 4, bottom: 0 }}
      >
        <CartesianGrid vertical={false} strokeDasharray="3 3" strokeOpacity={0.12} />
        <XAxis
          dataKey="tag"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          minTickGap={4}
          className="text-xs"
        />
        <YAxis
          tickLine={false}
          axisLine={false}
          width={36}
          allowDecimals={false}
          className="text-xs"
        />
        <ChartTooltip
          cursor={{ fill: "var(--muted)", opacity: 0.4 }}
          content={<ChartTooltipContent indicator="dot" />}
        />
        <Bar
          dataKey="downloads"
          fill="var(--color-downloads)"
          radius={[4, 4, 0, 0]}
          maxBarSize={56}
          isAnimationActive={false}
        />
      </BarChart>
    </ChartContainer>
  );
}

import { Download, Star, Tag, Scale } from "lucide-react";
import { CountUp } from "@/components/count-up";
import type { GitHubStats } from "@/lib/github";

/**
 * Live release stats. Downloads and stars come from the GitHub API (rendered
 * server-side into the HTML); the count-up is progressive enhancement only.
 */
export function StatsStrip({ stats }: { stats: GitHubStats }) {
  const cells = [
    {
      icon: Download,
      value: <CountUp value={stats.downloads} />,
      label: stats.downloads === null ? "Downloads on GitHub" : "Total downloads",
    },
    {
      icon: Star,
      value: <CountUp value={stats.stars} />,
      label: "GitHub stars",
    },
    {
      icon: Tag,
      value: `v${stats.version}`,
      label: "Latest release",
    },
    {
      icon: Scale,
      value: "GPL-3.0",
      label: "Free and open source",
    },
  ];

  return (
    <section
      aria-label="Project stats"
      className="border-b border-border/60 bg-secondary/20"
    >
      <div className="mx-auto grid max-w-6xl grid-cols-2 divide-x divide-y divide-border/60 sm:grid-cols-4 sm:divide-y-0">
        {cells.map((cell) => (
          <div key={cell.label} className="px-5 py-7 text-center sm:text-left">
            <div className="flex items-center justify-center gap-2 sm:justify-start">
              <cell.icon className="size-4 text-primary" aria-hidden />
              <span className="text-2xl font-semibold tracking-tight tabular-nums sm:text-3xl">
                {cell.value}
              </span>
            </div>
            <p className="mt-1.5 text-xs text-muted-foreground">{cell.label}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

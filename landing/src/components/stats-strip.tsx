import { CountUp } from "@/components/count-up";
import type { GitHubStats } from "@/lib/github";

/**
 * Live release stats. Downloads and stars come from the GitHub API (rendered
 * server-side into the HTML); the count-up is progressive enhancement only.
 * Missing values render as a greyed em-dash — never fabricated (§7).
 */
export function StatsStrip({ stats }: { stats: GitHubStats }) {
  const cells = [
    {
      value: <CountUp value={stats.downloads} />,
      label:
        stats.downloads === null ? "downloads on github" : "total downloads",
    },
    {
      value: <CountUp value={stats.stars} />,
      label: "github stars",
    },
    {
      value: `v${stats.version}`,
      label: "latest release",
    },
    {
      value: "GPL-3.0",
      label: "free and open source",
    },
  ];

  return (
    <section
      aria-label="Project stats"
      className="border-b border-hairline bg-surface"
    >
      <div className="mx-auto grid max-w-6xl grid-cols-2 divide-x divide-y divide-hairline sm:grid-cols-4 sm:divide-y-0">
        {cells.map((cell) => (
          <div key={cell.label} className="px-5 py-6 text-center sm:text-left">
            <span className="font-mono text-xl tabular-nums text-ink">
              {cell.value}
            </span>
            <p className="instrument-label mt-1.5">{cell.label}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

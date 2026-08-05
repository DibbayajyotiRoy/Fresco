import { CountUp } from "@/components/count-up";
import type { GitHubStats } from "@/lib/github";
import type { Dictionary } from "@/lib/i18n";

/**
 * Live release stats. Downloads and stars come from the GitHub API (rendered
 * server-side into the HTML); the count-up is progressive enhancement only.
 * Missing values render as a greyed em-dash — never fabricated (§7).
 */
export function StatsStrip({
  stats,
  dict,
}: {
  stats: GitHubStats;
  dict: Dictionary;
}) {
  const cells = [
    {
      value: <CountUp value={stats.downloads} />,
      label:
        stats.downloads === null
          ? dict.stats.downloadsUnknown
          : dict.stats.downloads,
    },
    {
      value: <CountUp value={stats.stars} />,
      label: dict.stats.stars,
    },
    {
      value: `v${stats.version}`,
      label: dict.stats.version,
    },
    {
      value: "GPL-3.0",
      label: dict.stats.license,
    },
  ];

  return (
    <section
      aria-label={dict.stats.ariaLabel}
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

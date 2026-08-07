import { StatCard } from "@/components/stat-card";
import { getFeedback, getReleases, getRepo } from "@/lib/data";
import { formatNumber, formatRelative } from "@/lib/format";

/**
 * The KPI strip, split by source rather than by tile.
 *
 * The six figures come from three round-trips of very different cost (repo
 * ~700ms, releases ~1.1s, feedback ~500ms). Each group suspends on its own so
 * the cheap ones land first; `<Suspense>` emits no DOM node, so the tiles stay
 * direct children of the strip's grid and never move.
 */

/** Stars — first tile. */
export async function StarsCard() {
  const repoRes = await getRepo();
  const repo = repoRes.ok ? repoRes.data : null;

  return (
    <StatCard
      label="Stars"
      value={repo ? formatNumber(repo.stars) : "—"}
      hint={
        repo
          ? `${formatNumber(repo.forks)} forks · ${formatNumber(repo.watchers)} watching`
          : repoRes.ok
            ? undefined
            : repoRes.error
      }
    />
  );
}

/** Downloads + latest version — both off the releases list. */
export async function ReleaseCards() {
  const releasesRes = await getReleases();
  const releases = releasesRes.ok ? releasesRes.data : [];

  const totalDownloads = releases.reduce((s, r) => s + r.downloads, 0);
  let running = 0;
  const downloadsTrend = releases.map((r) => (running += r.downloads));
  const latest = releases.at(-1) ?? null;

  return (
    <>
      <StatCard
        label="Downloads"
        value={releasesRes.ok ? formatNumber(totalDownloads) : "—"}
        hint={
          releasesRes.ok
            ? `across ${releases.length} release${releases.length === 1 ? "" : "s"}`
            : releasesRes.error
        }
        data={downloadsTrend}
      />
      <StatCard
        label="Latest version"
        value={latest ? latest.tag : "—"}
        hint={
          latest?.publishedAt
            ? `released ${formatRelative(latest.publishedAt)}`
            : undefined
        }
      />
    </>
  );
}

/** Feedback volume + satisfaction. */
export async function FeedbackCards() {
  const feedbackRes = await getFeedback();
  const feedback = feedbackRes.ok ? feedbackRes.data : [];

  const up = feedback.filter((f) => f.rating > 0).length;
  const down = feedback.filter((f) => f.rating < 0).length;
  const satisfaction = up + down > 0 ? Math.round((up / (up + down)) * 100) : 0;

  return (
    <>
      <StatCard
        label="Feedback"
        value={feedbackRes.ok ? formatNumber(feedback.length) : "—"}
        hint={
          feedbackRes.ok
            ? `${formatNumber(up)} up · ${formatNumber(down)} down`
            : feedbackRes.error
        }
      />
      <StatCard
        label="Satisfaction"
        value={up + down > 0 ? `${satisfaction}%` : "—"}
        hint="up / (up + down)"
      />
    </>
  );
}

/** Open issues — last tile, same repo payload as `StarsCard`. */
export async function OpenIssuesCard() {
  const repoRes = await getRepo();
  const repo = repoRes.ok ? repoRes.data : null;

  return (
    <StatCard
      label="Open issues"
      value={repo ? formatNumber(repo.openIssues) : "—"}
      hint="issues + PRs on GitHub"
    />
  );
}

import { ArrowSquareOut, GithubLogo } from "@phosphor-icons/react/dist/ssr";

import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { SentimentBadge } from "@/components/sentiment-badge";
import { DownloadsChart } from "@/components/downloads-chart";
import { ReleaseTable } from "@/components/release-table";
import { Panel, PanelHeader } from "@/components/panel";
import { Badge } from "@/components/badges";
import {
  DistributionList,
  type DistributionItem,
} from "@/components/distribution-list";
import { getFeedback, getNotifications, getReleases, getRepo } from "@/lib/data";
import { formatNumber, formatRelative } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const REPO_URL = "https://github.com/DibbayajyotiRoy/fresco";

/** Bucket freeform values into a top-N breakdown with an "Other" rollup. */
function topDistribution(
  values: (string | null)[],
  n = 6
): DistributionItem[] {
  const counts = new Map<string, number>();
  for (const v of values) {
    const key = v?.trim() || "Unknown";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const sorted = [...counts.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value);

  if (sorted.length <= n) return sorted;
  const head = sorted.slice(0, n);
  const other = sorted.slice(n).reduce((s, i) => s + i.value, 0);
  return [...head, { label: "Other", value: other }];
}

export default async function OverviewPage() {
  const [repoRes, releasesRes, feedbackRes, notificationsRes] =
    await Promise.all([
      getRepo(),
      getReleases(),
      getFeedback(),
      getNotifications(),
    ]);

  const repo = repoRes.ok ? repoRes.data : null;
  const releases = releasesRes.ok ? releasesRes.data : [];
  const feedback = feedbackRes.ok ? feedbackRes.data : [];
  const notifications = notificationsRes.ok ? notificationsRes.data : [];

  const totalDownloads = releases.reduce((s, r) => s + r.downloads, 0);
  let running = 0;
  const downloadsTrend = releases.map((r) => (running += r.downloads));
  const latest = releases.at(-1) ?? null;

  const up = feedback.filter((f) => f.rating > 0).length;
  const down = feedback.filter((f) => f.rating < 0).length;
  const satisfaction = up + down > 0 ? Math.round((up / (up + down)) * 100) : 0;

  const osDist = topDistribution(feedback.map((f) => f.os));
  const versionDist = topDistribution(feedback.map((f) => f.app_version));

  const recentFeedback = feedback.slice(0, 7);
  const latestNotifications = notifications.slice(0, 5);

  return (
    <div className="space-y-3">
      <PageHeader
        title="Overview"
        meta={`${formatNumber(totalDownloads)} downloads · ${formatNumber(feedback.length)} feedback`}
        action={
          <a
            href={REPO_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex h-7 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-2 text-sm font-medium text-stone-700 transition-colors hover:bg-stone-100"
          >
            <GithubLogo className="size-3.5" weight="fill" />
            GitHub
            <ArrowSquareOut className="size-3 text-stone-400" />
          </a>
        }
      />

      {/* KPI strip — six figures across one row at xl. */}
      <div className="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-6">
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
        <StatCard
          label="Open issues"
          value={repo ? formatNumber(repo.openIssues) : "—"}
          hint="issues + PRs on GitHub"
        />
      </div>

      {/* Downloads beside the feedback breakdowns. */}
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
        <Panel className="lg:col-span-2">
          <PanelHeader
            title="Downloads per release"
            meta={`${formatNumber(totalDownloads)} total`}
          />
          {!releasesRes.ok ? (
            <ErrorPanel
              title="Couldn't load GitHub releases"
              message={releasesRes.error}
            />
          ) : releases.length === 0 ? (
            <EmptyState
              title="No releases yet"
              description="Published GitHub releases with assets will appear here."
            />
          ) : (
            <div className="space-y-3">
              <DownloadsChart releases={releases} />
              <ReleaseTable releases={releases} />
            </div>
          )}
        </Panel>

        <div className="flex flex-col gap-3">
          <Panel>
            <PanelHeader title="Platform" meta="by feedback" />
            {osDist.length === 0 ? (
              <EmptyState
                className="py-6"
                title="No data yet"
                description="The OS field arrives with app feedback."
              />
            ) : (
              <DistributionList items={osDist} total={feedback.length} />
            )}
          </Panel>

          <Panel>
            <PanelHeader title="App version" meta="by feedback" />
            {versionDist.length === 0 ? (
              <EmptyState
                className="py-6"
                title="No data yet"
                description="The version field arrives with app feedback."
              />
            ) : (
              <DistributionList items={versionDist} total={feedback.length} />
            )}
          </Panel>
        </div>
      </div>

      {/* Recent activity. */}
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <Panel>
          <PanelHeader
            title="Recent feedback"
            meta={`${formatNumber(recentFeedback.length)} shown`}
          />
          {!feedbackRes.ok ? (
            <ErrorPanel
              title="Couldn't load feedback"
              message={feedbackRes.error}
            />
          ) : recentFeedback.length === 0 ? (
            <EmptyState
              title="No feedback yet"
              description="Ratings from the app will show up here."
            />
          ) : (
            <ul className="divide-y divide-stone-200">
              {recentFeedback.map((f) => (
                <li
                  key={f.id}
                  className="flex items-start gap-3 py-2 first:pt-0 last:pb-0"
                >
                  <SentimentBadge rating={f.rating} />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm text-stone-900">
                      {f.comment ? (
                        f.comment
                      ) : (
                        <span className="text-stone-400 italic">
                          No comment
                        </span>
                      )}
                    </p>
                    <p className="mt-0.5 truncate font-mono text-meta text-stone-400">
                      {[f.app_version, f.os].filter(Boolean).join(" · ") || "—"}
                    </p>
                  </div>
                  <span className="shrink-0 font-mono text-meta text-stone-400">
                    {formatRelative(f.created_at)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Panel>
          <PanelHeader
            title="Latest notifications"
            meta={`${formatNumber(latestNotifications.length)} shown`}
          />
          {!notificationsRes.ok ? (
            <ErrorPanel
              title="Couldn't load notifications"
              message={notificationsRes.error}
            />
          ) : latestNotifications.length === 0 ? (
            <EmptyState
              title="No notifications yet"
              description="Create one on the Notifications page."
            />
          ) : (
            <ul className="divide-y divide-stone-200">
              {latestNotifications.map((n) => (
                <li
                  key={n.id}
                  className="flex items-start gap-3 py-2 first:pt-0 last:pb-0"
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium text-stone-900">
                      {n.title}
                    </p>
                    <p className="mt-0.5 line-clamp-1 text-sm text-stone-500">
                      {n.body}
                    </p>
                  </div>
                  <Badge label={n.published ? "published" : "draft"} />
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>
    </div>
  );
}

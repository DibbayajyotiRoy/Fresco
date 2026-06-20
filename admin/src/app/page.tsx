import {
  ArrowSquareOut,
  Bell,
  Bug,
  ChatCircle,
  DownloadSimple,
  Gauge,
  GithubLogo,
  Star,
  Tag,
  ThumbsUp,
} from "@phosphor-icons/react/dist/ssr";

import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { SentimentBadge } from "@/components/sentiment-badge";
import { DownloadsChart } from "@/components/downloads-chart";
import { ReleaseTable } from "@/components/release-table";
import {
  DistributionList,
  type DistributionItem,
} from "@/components/distribution-list";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { getFeedback, getNotifications, getReleases, getRepo } from "@/lib/data";
import { formatNumber, formatRelative } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const REPO_URL = "https://github.com/DibbayajyotiRoy/fresco";

/** Bucket freeform values into a top-N breakdown with an "Other" rollup, so a
 *  long tail of one-off OS strings doesn't drown the common ones. */
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
  // Cumulative downloads (oldest -> newest) — a real growth curve for the KPI
  // sparkline. Draws only with >= 2 releases.
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
    <>
      <PageHeader
        title="Overview"
        description="Stars, downloads, feedback and notifications at a glance"
        action={
          <a
            href={REPO_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="border-border bg-card text-foreground hover:bg-accent inline-flex h-8 items-center gap-1.5 rounded-md border px-2.5 text-xs font-medium transition-colors"
          >
            <GithubLogo className="size-3.5" weight="fill" />
            GitHub
            <ArrowSquareOut className="text-muted-foreground size-3" />
          </a>
        }
      />

      <div className="flex flex-1 flex-col gap-4 p-4 md:p-5">
        {/* KPI strip — six figures across one row at xl. */}
        <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
          <StatCard
            label="Stars"
            value={repo ? formatNumber(repo.stars) : "—"}
            hint={
              repo
                ? `${formatNumber(repo.forks)} forks · ${formatNumber(repo.watchers)} watching`
                : (repoRes.ok ? undefined : repoRes.error)
            }
            icon={Star}
          />
          <StatCard
            label="Downloads"
            value={formatNumber(totalDownloads)}
            hint={
              releasesRes.ok
                ? `Across ${releases.length} release${releases.length === 1 ? "" : "s"}`
                : releasesRes.error
            }
            icon={DownloadSimple}
            data={downloadsTrend}
          />
          <StatCard
            label="Latest version"
            value={latest ? latest.tag : "—"}
            hint={
              latest?.publishedAt
                ? `Released ${formatRelative(latest.publishedAt)}`
                : undefined
            }
            icon={Tag}
          />
          <StatCard
            label="Feedback"
            value={formatNumber(feedback.length)}
            hint={
              feedbackRes.ok
                ? `${formatNumber(up)} up · ${formatNumber(down)} down`
                : feedbackRes.error
            }
            icon={ChatCircle}
          />
          <StatCard
            label="Satisfaction"
            value={up + down > 0 ? `${satisfaction}%` : "—"}
            hint="up / (up + down)"
            icon={Gauge}
          />
          <StatCard
            label="Open issues"
            value={repo ? formatNumber(repo.openIssues) : "—"}
            hint="issues + PRs on GitHub"
            icon={Bug}
          />
        </div>

        {/* Main: downloads (chart + table) beside the feedback breakdowns. */}
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
          <Card className="gap-4 lg:col-span-2">
            <CardHeader>
              <CardTitle>Downloads per release</CardTitle>
              <CardDescription>
                GitHub release asset downloads ·{" "}
                {formatNumber(totalDownloads)} total
              </CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              {!releasesRes.ok ? (
                <EmptyState
                  title="Couldn't load GitHub releases"
                  description={releasesRes.error}
                />
              ) : releases.length === 0 ? (
                <EmptyState
                  title="No releases yet"
                  description="Published GitHub releases with assets will appear here."
                />
              ) : (
                <>
                  <DownloadsChart releases={releases} />
                  <Separator />
                  <ReleaseTable releases={releases} />
                </>
              )}
            </CardContent>
          </Card>

          <div className="flex flex-col gap-4">
            <Card className="gap-4">
              <CardHeader>
                <CardTitle>Platform</CardTitle>
                <CardDescription>By feedback reports</CardDescription>
              </CardHeader>
              <CardContent>
                {osDist.length === 0 ? (
                  <EmptyState
                    className="py-8"
                    title="No data yet"
                    description="The OS field arrives with app feedback."
                  />
                ) : (
                  <DistributionList items={osDist} total={feedback.length} />
                )}
              </CardContent>
            </Card>

            <Card className="gap-4">
              <CardHeader>
                <CardTitle>App version</CardTitle>
                <CardDescription>By feedback reports</CardDescription>
              </CardHeader>
              <CardContent>
                {versionDist.length === 0 ? (
                  <EmptyState
                    className="py-8"
                    title="No data yet"
                    description="The version field arrives with app feedback."
                  />
                ) : (
                  <DistributionList
                    items={versionDist}
                    total={feedback.length}
                  />
                )}
              </CardContent>
            </Card>
          </div>
        </div>

        {/* Recent activity. */}
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          <Card className="flex flex-col gap-4">
            <CardHeader>
              <CardTitle>Recent feedback</CardTitle>
              <CardDescription>Latest ratings from the app</CardDescription>
            </CardHeader>
            <CardContent className="flex-1">
              {!feedbackRes.ok ? (
                <EmptyState title="No feedback" description={feedbackRes.error} />
              ) : recentFeedback.length === 0 ? (
                <EmptyState
                  title="No feedback yet"
                  icon={ThumbsUp}
                  description="Ratings from the app will show up here."
                />
              ) : (
                <ul className="divide-border divide-y">
                  {recentFeedback.map((f) => (
                    <li
                      key={f.id}
                      className="flex items-start gap-3 py-2.5 first:pt-0 last:pb-0"
                    >
                      <SentimentBadge rating={f.rating} />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm">
                          {f.comment ? (
                            f.comment
                          ) : (
                            <span className="text-muted-foreground italic">
                              No comment
                            </span>
                          )}
                        </p>
                        <p className="text-muted-foreground mt-0.5 text-xs">
                          {[f.app_version, f.os].filter(Boolean).join(" · ") ||
                            "unknown"}
                        </p>
                      </div>
                      <span className="text-muted-foreground shrink-0 text-xs">
                        {formatRelative(f.created_at)}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </CardContent>
          </Card>

          <Card className="flex flex-col gap-4">
            <CardHeader>
              <CardTitle>Latest notifications</CardTitle>
              <CardDescription>Most recent announcements</CardDescription>
            </CardHeader>
            <CardContent className="flex-1">
              {!notificationsRes.ok ? (
                <EmptyState
                  title="No notifications"
                  description={notificationsRes.error}
                />
              ) : latestNotifications.length === 0 ? (
                <EmptyState
                  title="No notifications yet"
                  icon={Bell}
                  description="Create one on the Notifications page."
                />
              ) : (
                <ul className="divide-border divide-y">
                  {latestNotifications.map((n) => (
                    <li
                      key={n.id}
                      className="flex items-start gap-3 py-2.5 first:pt-0 last:pb-0"
                    >
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium">{n.title}</p>
                        <p className="text-muted-foreground mt-0.5 line-clamp-1 text-xs">
                          {n.body}
                        </p>
                      </div>
                      <Badge
                        variant={n.published ? "default" : "secondary"}
                        className="shrink-0"
                      >
                        {n.published ? "Published" : "Draft"}
                      </Badge>
                    </li>
                  ))}
                </ul>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}

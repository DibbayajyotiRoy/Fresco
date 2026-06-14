import {
  Bell,
  ChatCircle,
  DownloadSimple,
  Gauge,
  ThumbsUp,
} from "@phosphor-icons/react/dist/ssr";

import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { SentimentBadge } from "@/components/sentiment-badge";
import { DownloadsChart } from "@/components/downloads-chart";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { getFeedback, getNotifications, getReleases } from "@/lib/data";
import { formatNumber, formatRelative } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function OverviewPage() {
  const [releasesRes, feedbackRes, notificationsRes] = await Promise.all([
    getReleases(),
    getFeedback(),
    getNotifications(),
  ]);

  const releases = releasesRes.ok ? releasesRes.data : [];
  const feedback = feedbackRes.ok ? feedbackRes.data : [];
  const notifications = notificationsRes.ok ? notificationsRes.data : [];

  const totalDownloads = releases.reduce((s, r) => s + r.downloads, 0);
  const up = feedback.filter((f) => f.rating > 0).length;
  const down = feedback.filter((f) => f.rating < 0).length;
  const satisfaction = up + down > 0 ? Math.round((up / (up + down)) * 100) : 0;
  const publishedCount = notifications.filter((n) => n.published).length;

  const recentFeedback = feedback.slice(0, 8);
  const latestNotifications = notifications.slice(0, 5);

  return (
    <>
      <PageHeader
        title="Overview"
        description="Downloads, feedback and notifications at a glance"
      />
      <div className="flex flex-1 flex-col gap-6 p-4 md:p-6">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <StatCard
            label="Total downloads"
            value={formatNumber(totalDownloads)}
            hint={
              releasesRes.ok
                ? `Across ${releases.length} release${releases.length === 1 ? "" : "s"}`
                : releasesRes.error
            }
            icon={DownloadSimple}
          />
          <StatCard
            label="Total feedback"
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
            label="Published"
            value={formatNumber(publishedCount)}
            hint={`${formatNumber(notifications.length)} total notifications`}
            icon={Bell}
          />
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Downloads per release</CardTitle>
            <CardDescription>
              GitHub release asset download counts
            </CardDescription>
          </CardHeader>
          <CardContent>
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
              <DownloadsChart releases={releases} />
            )}
          </CardContent>
        </Card>

        <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
          <Card className="flex flex-col">
            <CardHeader>
              <CardTitle>Recent feedback</CardTitle>
              <CardDescription>Last 8 ratings</CardDescription>
            </CardHeader>
            <CardContent className="flex-1">
              {!feedbackRes.ok ? (
                <EmptyState
                  title="No feedback"
                  description={feedbackRes.error}
                />
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
                      className="flex items-start gap-3 py-3 first:pt-0 last:pb-0"
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

          <Card className="flex flex-col">
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
                      className="flex items-start gap-3 py-3 first:pt-0 last:pb-0"
                    >
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium">
                          {n.title}
                        </p>
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

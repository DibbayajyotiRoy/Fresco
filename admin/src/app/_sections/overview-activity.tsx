import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { SentimentBadge } from "@/components/sentiment-badge";
import { Panel, PanelHeader } from "@/components/panel";
import { Badge } from "@/components/badges";
import { getFeedback, getNotifications } from "@/lib/data";
import { formatNumber, formatRelative } from "@/lib/format";

/** Recent activity — feedback and notifications are separate round-trips, so
 *  each of the two panels streams as soon as its own query lands. */

export async function RecentFeedbackPanel() {
  const feedbackRes = await getFeedback();
  const feedback = feedbackRes.ok ? feedbackRes.data : [];
  const recentFeedback = feedback.slice(0, 7);

  return (
    <Panel className="section-in">
      <PanelHeader
        title="Recent feedback"
        meta={`${formatNumber(recentFeedback.length)} shown`}
      />
      {!feedbackRes.ok ? (
        <ErrorPanel title="Couldn't load feedback" message={feedbackRes.error} />
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
                    <span className="text-stone-400 italic">No comment</span>
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
  );
}

export async function LatestNotificationsPanel() {
  const notificationsRes = await getNotifications();
  const notifications = notificationsRes.ok ? notificationsRes.data : [];
  const latestNotifications = notifications.slice(0, 5);

  return (
    <Panel className="section-in">
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
  );
}

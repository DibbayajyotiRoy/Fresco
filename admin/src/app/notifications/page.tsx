import { PageHeader } from "@/components/page-header";
import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { NotificationDialog } from "@/app/notifications/notification-dialog";
import { NotificationsTable } from "@/app/notifications/notifications-table";
import { getNotifications } from "@/lib/data";
import { formatNumber } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function NotificationsPage() {
  const res = await getNotifications();
  const published = res.ok
    ? res.data.filter((n) => n.published).length
    : 0;

  return (
    <div className="space-y-3">
      <PageHeader
        title="Notifications"
        meta={
          res.ok
            ? `${formatNumber(res.data.length)} total · ${formatNumber(published)} published`
            : undefined
        }
        action={<NotificationDialog />}
      />
      {!res.ok ? (
        <ErrorPanel title="Couldn't load notifications" message={res.error} />
      ) : res.data.length === 0 ? (
        <EmptyState
          title="No notifications yet"
          description="Create your first announcement with the button above."
        />
      ) : (
        <NotificationsTable notifications={res.data} />
      )}
    </div>
  );
}

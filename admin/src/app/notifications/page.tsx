import { Bell } from "@phosphor-icons/react/dist/ssr";

import { PageHeader } from "@/components/page-header";
import { EmptyState } from "@/components/empty-state";
import { Card, CardContent } from "@/components/ui/card";
import { NotificationDialog } from "@/app/notifications/notification-dialog";
import { NotificationsTable } from "@/app/notifications/notifications-table";
import { getNotifications } from "@/lib/data";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function NotificationsPage() {
  const res = await getNotifications();

  return (
    <>
      <PageHeader
        title="Notifications"
        description="Changelog and announcements pushed to the app"
        action={<NotificationDialog />}
      />
      <div className="flex flex-1 flex-col gap-6 p-4 md:p-6">
        <Card>
          <CardContent className="px-0">
            {!res.ok ? (
              <EmptyState
                title="Couldn't load notifications"
                description={res.error}
                className="m-4"
              />
            ) : res.data.length === 0 ? (
              <EmptyState
                title="No notifications yet"
                icon={Bell}
                description="Create your first announcement with the button above."
                className="m-4"
              />
            ) : (
              <NotificationsTable notifications={res.data} />
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}

"use client";

import * as React from "react";
import {
  DotsThree,
  LinkSimple,
  PencilSimple,
  Trash,
} from "@phosphor-icons/react/dist/ssr";

import { toast } from "@/components/toaster";
import { confirm } from "@/components/confirm-dialog";
import { Badge } from "@/components/badges";
import {
  DataTable,
  NullCell,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/data-table";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Switch } from "@/components/ui/switch";
import { NotificationDialog } from "@/app/notifications/notification-dialog";
import { deleteNotification, setPublished } from "@/app/notifications/actions";
import { formatDateTime } from "@/lib/format";
import type { Notification } from "@/lib/types";

function PublishToggle({ notification }: { notification: Notification }) {
  const [checked, setChecked] = React.useState(notification.published);
  const [pending, startTransition] = React.useTransition();

  React.useEffect(() => {
    setChecked(notification.published);
  }, [notification.published]);

  function onChange(next: boolean) {
    setChecked(next);
    startTransition(async () => {
      const result = await setPublished(notification.id, next);
      if (result.ok) {
        toast.success(next ? "Published" : "Unpublished");
      } else {
        setChecked(!next);
        toast.error(result.error);
      }
    });
  }

  return (
    <Switch
      checked={checked}
      onCheckedChange={onChange}
      disabled={pending}
      aria-label="Toggle published"
    />
  );
}

function RowActions({ notification }: { notification: Notification }) {
  const [editOpen, setEditOpen] = React.useState(false);
  const [, startTransition] = React.useTransition();

  async function onDelete() {
    const ok = await confirm({
      title: "Delete notification?",
      description: `This permanently removes "${notification.title}". This action cannot be undone.`,
    });
    if (!ok) return;
    startTransition(async () => {
      const result = await deleteNotification(notification.id);
      if (result.ok) toast.success("Notification deleted");
      else toast.error(result.error);
    });
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className="inline-flex size-6 items-center justify-center rounded-md text-stone-400 transition-colors hover:bg-stone-100 hover:text-stone-900"
          >
            <DotsThree className="size-4" weight="bold" />
            <span className="sr-only">Open actions</span>
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="rounded-md border-stone-200">
          <DropdownMenuItem onSelect={() => setEditOpen(true)}>
            <PencilSimple className="size-3.5" />
            Edit
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive" onSelect={onDelete}>
            <Trash className="size-3.5" />
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <NotificationDialog
        notification={notification}
        open={editOpen}
        onOpenChange={setEditOpen}
      />
    </>
  );
}

export function NotificationsTable({
  notifications,
}: {
  notifications: Notification[];
}) {
  return (
    <DataTable>
      <THead>
        <TR>
          <TH>Title</TH>
          <TH className="w-[100px]">Status</TH>
          <TH className="w-[70px]">Link</TH>
          <TH className="hidden w-[170px] md:table-cell">Created</TH>
          <TH className="w-[90px] text-right">Published</TH>
          <TH className="w-[44px]" />
        </TR>
      </THead>
      <TBody>
        {notifications.map((n) => (
          <TR key={n.id}>
            <TD>
              <span className="block truncate text-sm font-medium text-stone-900">
                {n.title}
              </span>
              <span className="block truncate font-mono text-meta text-stone-400">
                {n.body}
              </span>
            </TD>
            <TD>
              <Badge label={n.published ? "published" : "draft"} />
            </TD>
            <TD>
              {n.url ? (
                <a
                  href={n.url}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex items-center gap-1 text-sm text-sky-700 hover:underline"
                >
                  <LinkSimple className="size-3" weight="bold" />
                  Link
                </a>
              ) : (
                <NullCell />
              )}
            </TD>
            <TD className="hidden md:table-cell">
              <span className="font-mono text-meta text-stone-500">
                {formatDateTime(n.created_at)}
              </span>
            </TD>
            <TD className="text-right">
              <span className="flex justify-end">
                <PublishToggle notification={n} />
              </span>
            </TD>
            <TD className="text-right">
              <RowActions notification={n} />
            </TD>
          </TR>
        ))}
      </TBody>
    </DataTable>
  );
}

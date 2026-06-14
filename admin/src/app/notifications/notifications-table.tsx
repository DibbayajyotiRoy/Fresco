"use client";

import * as React from "react";
import {
  DotsThree,
  LinkSimple,
  PencilSimple,
  Trash,
} from "@phosphor-icons/react/dist/ssr";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { NotificationDialog } from "@/app/notifications/notification-dialog";
import { deleteNotification, setPublished } from "@/app/notifications/actions";
import { formatDate } from "@/lib/format";
import type { Notification } from "@/lib/types";

function PublishToggle({ notification }: { notification: Notification }) {
  const [checked, setChecked] = React.useState(notification.published);
  const [pending, startTransition] = React.useTransition();

  // Keep in sync if the server data changes after a revalidate.
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
  const [confirmOpen, setConfirmOpen] = React.useState(false);
  const [pending, startTransition] = React.useTransition();

  function onDelete() {
    startTransition(async () => {
      const result = await deleteNotification(notification.id);
      if (result.ok) {
        toast.success("Notification deleted");
        setConfirmOpen(false);
      } else {
        toast.error(result.error);
      }
    });
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" className="size-8">
            <DotsThree className="size-4" weight="bold" />
            <span className="sr-only">Open actions</span>
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={() => setEditOpen(true)}>
            <PencilSimple className="size-4" />
            Edit
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            variant="destructive"
            onSelect={() => setConfirmOpen(true)}
          >
            <Trash className="size-4" />
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <NotificationDialog
        notification={notification}
        open={editOpen}
        onOpenChange={setEditOpen}
      />

      <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete notification?</DialogTitle>
            <DialogDescription>
              This permanently removes &ldquo;{notification.title}&rdquo;. This
              action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setConfirmOpen(false)}
              disabled={pending}
            >
              Cancel
            </Button>
            <Button variant="destructive" onClick={onDelete} disabled={pending}>
              {pending ? "Deleting…" : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

export function NotificationsTable({
  notifications,
}: {
  notifications: Notification[];
}) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Title</TableHead>
          <TableHead className="w-[120px]">Status</TableHead>
          <TableHead className="w-[90px]">Link</TableHead>
          <TableHead className="w-[140px]">Created</TableHead>
          <TableHead className="w-[110px] text-right">Published</TableHead>
          <TableHead className="w-[48px]" />
        </TableRow>
      </TableHeader>
      <TableBody>
        {notifications.map((n) => (
          <TableRow key={n.id}>
            <TableCell className="max-w-[320px]">
              <div className="truncate font-medium">{n.title}</div>
              <div className="text-muted-foreground line-clamp-1 text-xs">
                {n.body}
              </div>
            </TableCell>
            <TableCell>
              <Badge variant={n.published ? "default" : "secondary"}>
                {n.published ? "Published" : "Draft"}
              </Badge>
            </TableCell>
            <TableCell>
              {n.url ? (
                <a
                  href={n.url}
                  target="_blank"
                  rel="noreferrer"
                  className="text-primary inline-flex items-center gap-1 text-xs hover:underline"
                >
                  <LinkSimple className="size-3" weight="bold" />
                  Link
                </a>
              ) : (
                <span className="text-muted-foreground text-xs">—</span>
              )}
            </TableCell>
            <TableCell className="text-muted-foreground font-mono text-xs">
              {formatDate(n.created_at)}
            </TableCell>
            <TableCell className="text-right">
              <div className="flex justify-end">
                <PublishToggle notification={n} />
              </div>
            </TableCell>
            <TableCell>
              <RowActions notification={n} />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

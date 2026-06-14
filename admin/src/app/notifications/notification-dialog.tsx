"use client";

import * as React from "react";
import { Plus } from "@phosphor-icons/react/dist/ssr";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import {
  createNotification,
  updateNotification,
} from "@/app/notifications/actions";
import type { Notification } from "@/lib/types";

type NotificationDialogProps = {
  notification?: Notification;
  /** When provided, the dialog is controlled and renders no trigger. */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
};

export function NotificationDialog({
  notification,
  open: openProp,
  onOpenChange,
}: NotificationDialogProps) {
  const isEdit = Boolean(notification);
  const controlled = openProp !== undefined;
  const [openState, setOpenState] = React.useState(false);
  const open = controlled ? openProp : openState;
  const setOpen = React.useCallback(
    (next: boolean) => {
      if (controlled) onOpenChange?.(next);
      else setOpenState(next);
    },
    [controlled, onOpenChange]
  );

  const [published, setPublished] = React.useState(
    notification?.published ?? false
  );
  const [pending, startTransition] = React.useTransition();

  // Reset the switch to the source value each time the dialog opens.
  React.useEffect(() => {
    if (open) setPublished(notification?.published ?? false);
  }, [open, notification?.published]);

  function onSubmit(formData: FormData) {
    formData.set("published", published ? "on" : "off");

    startTransition(async () => {
      const result = isEdit
        ? await updateNotification(notification!.id, formData)
        : await createNotification(formData);

      if (result.ok) {
        toast.success(isEdit ? "Notification updated" : "Notification created");
        setOpen(false);
      } else {
        toast.error(result.error);
      }
    });
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      {!controlled ? (
        <DialogTrigger asChild>
          <Button size="sm">
            <Plus className="size-4" weight="bold" />
            New notification
          </Button>
        </DialogTrigger>
      ) : null}
      <DialogContent className="sm:max-w-xl">
        <form action={onSubmit}>
          <DialogHeader>
            <DialogTitle>
              {isEdit ? "Edit notification" : "New notification"}
            </DialogTitle>
            <DialogDescription>
              Authored content pushed to the app&apos;s notification modal.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="title">Title</Label>
              <Input
                id="title"
                name="title"
                required
                defaultValue={notification?.title}
                placeholder="What's new in v0.0.4"
              />
            </div>

            <div className="grid gap-2">
              <Label htmlFor="body">Body</Label>
              <Textarea
                id="body"
                name="body"
                required
                rows={8}
                defaultValue={notification?.body}
                placeholder={
                  "- Faster startup\n- Fixed slideshow looping\n- New theme accents"
                }
                className="font-mono text-sm leading-relaxed"
              />
              <p className="text-muted-foreground text-xs">
                Shown in the app&apos;s notification modal. Line breaks are
                preserved.
              </p>
            </div>

            <div className="grid gap-2">
              <Label htmlFor="url">Link URL (optional)</Label>
              <Input
                id="url"
                name="url"
                type="url"
                defaultValue={notification?.url ?? ""}
                placeholder="https://github.com/DibbayajyotiRoy/fresco/releases"
              />
            </div>

            <div className="flex items-center justify-between rounded-lg border p-3">
              <div className="space-y-0.5">
                <Label htmlFor="published">Published</Label>
                <p className="text-muted-foreground text-xs">
                  Visible to the app once published.
                </p>
              </div>
              <Switch
                id="published"
                checked={published}
                onCheckedChange={setPublished}
              />
            </div>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setOpen(false)}
              disabled={pending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={pending}>
              {pending
                ? "Saving…"
                : isEdit
                  ? "Save changes"
                  : "Create notification"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

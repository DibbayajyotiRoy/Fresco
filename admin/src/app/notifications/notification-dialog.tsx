"use client";

import * as React from "react";
import { Plus } from "@phosphor-icons/react/dist/ssr";

import { toast } from "@/components/toaster";
import { AnimatedGlyph } from "@/components/animated-glyph";
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

function FieldLabel({
  htmlFor,
  children,
}: {
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <label
      htmlFor={htmlFor}
      className="font-mono text-meta tracking-wider text-stone-400 uppercase"
    >
      {children}
    </label>
  );
}

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
          <button
            type="button"
            className="inline-flex h-7 items-center gap-1.5 rounded-md bg-sky-600 px-2.5 text-sm font-medium text-white transition-colors hover:bg-sky-700"
          >
            <Plus className="size-3.5" weight="bold" />
            New notification
          </button>
        </DialogTrigger>
      ) : null}
      <DialogContent className="rounded-xl border-stone-200 sm:max-w-xl">
        <form action={onSubmit}>
          <DialogHeader>
            <DialogTitle className="text-lg font-semibold">
              {isEdit ? "Edit notification" : "New notification"}
            </DialogTitle>
            <DialogDescription className="text-sm text-stone-500">
              Authored content pushed to the app&apos;s notification modal.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-3 py-4">
            <div className="grid gap-1.5">
              <FieldLabel htmlFor="title">Title</FieldLabel>
              <Input
                id="title"
                name="title"
                required
                defaultValue={notification?.title}
                placeholder="What's new in v0.0.4"
                className="h-8 text-sm"
              />
            </div>

            <div className="grid gap-1.5">
              <FieldLabel htmlFor="body">Body</FieldLabel>
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
              <p className="text-meta text-stone-400">
                Shown in the app&apos;s notification modal. Line breaks are
                preserved.
              </p>
            </div>

            <div className="grid gap-1.5">
              <FieldLabel htmlFor="url">Link URL (optional)</FieldLabel>
              <Input
                id="url"
                name="url"
                type="url"
                defaultValue={notification?.url ?? ""}
                placeholder="https://github.com/DibbayajyotiRoy/fresco/releases"
                className="h-8 text-sm"
              />
            </div>

            <div className="flex items-center justify-between rounded-md border border-stone-200 p-2.5">
              <div className="space-y-0.5">
                <FieldLabel htmlFor="published">Published</FieldLabel>
                <p className="text-meta text-stone-400">
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
            <button
              type="button"
              onClick={() => setOpen(false)}
              disabled={pending}
              className="inline-flex h-7 items-center rounded-md border border-stone-200 bg-white px-2.5 text-sm font-medium text-stone-700 transition-colors hover:bg-stone-100"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={pending}
              className="inline-flex h-7 items-center gap-1.5 rounded-md bg-sky-600 px-2.5 text-sm font-medium text-white transition-colors hover:bg-sky-700 disabled:opacity-60"
            >
              {pending ? (
                <>
                  <AnimatedGlyph name="braille" active /> Saving…
                </>
              ) : isEdit ? (
                "Save changes"
              ) : (
                "Create notification"
              )}
            </button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

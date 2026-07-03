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
import { createCatalogItem } from "@/app/catalog/actions";

export function CatalogDialog() {
  const [open, setOpen] = React.useState(false);
  const [published, setPublished] = React.useState(false);
  const [pending, startTransition] = React.useTransition();

  function onSubmit(formData: FormData) {
    formData.set("published", published ? "on" : "off");
    startTransition(async () => {
      const result = await createCatalogItem(formData);
      if (result.ok) {
        toast.success("Catalog item created");
        setOpen(false);
        setPublished(false);
      } else {
        toast.error(result.error);
      }
    });
  }

  const field = (
    name: string,
    label: string,
    placeholder: string,
    required = false
  ) => (
    <div className="grid gap-2">
      <Label htmlFor={name}>{label}</Label>
      <Input id={name} name={name} placeholder={placeholder} required={required} />
    </div>
  );

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>
          <Plus /> New item
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>New catalog item</DialogTitle>
          <DialogDescription>
            Metadata only — host the media on a zero-egress CDN (GitHub
            Releases / R2), never Supabase storage. License and attribution are
            mandatory.
          </DialogDescription>
        </DialogHeader>
        <form action={onSubmit} className="grid gap-4">
          {field("title", "Title", "Rainy Window", true)}
          {field("category", "Category", "nature")}
          {field("tags", "Tags (comma-separated)", "rain, cozy")}
          {field("media_url", "Media URL", "https://…/rainy.mp4", true)}
          {field("thumb_url", "Thumbnail URL", "https://…/rainy.jpg")}
          {field("size_bytes", "Size (bytes)", "20000000")}
          {field("license", "License", "CC0-1.0", true)}
          {field("author", "Author", "Jane Doe")}
          {field("source_url", "Source URL", "https://…/original")}
          <div className="flex items-center gap-2">
            <Switch checked={published} onCheckedChange={setPublished} />
            <Label>Published</Label>
          </div>
          <DialogFooter>
            <Button type="submit" disabled={pending}>
              {pending ? "Saving…" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

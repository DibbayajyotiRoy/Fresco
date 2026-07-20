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
    <div className="grid gap-1.5">
      <label
        htmlFor={name}
        className="font-mono text-meta tracking-wider text-stone-400 uppercase"
      >
        {label}
      </label>
      <Input
        id={name}
        name={name}
        placeholder={placeholder}
        required={required}
        className="h-8 text-sm"
      />
    </div>
  );

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <button
          type="button"
          className="inline-flex h-7 items-center gap-1.5 rounded-md bg-sky-600 px-2.5 text-sm font-medium text-white transition-colors hover:bg-sky-700"
        >
          <Plus className="size-3.5" weight="bold" /> New item
        </button>
      </DialogTrigger>
      <DialogContent className="max-h-[85vh] overflow-y-auto rounded-xl border-stone-200 sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="text-lg font-semibold">
            New catalog item
          </DialogTitle>
          <DialogDescription className="text-sm text-stone-500">
            Metadata only — host the media on a zero-egress CDN (GitHub
            Releases / R2), never Supabase storage. License and attribution
            are mandatory.
          </DialogDescription>
        </DialogHeader>
        <form action={onSubmit} className="grid gap-3">
          {field("title", "Title", "Rainy Window", true)}
          <div className="grid grid-cols-2 gap-3">
            {field("category", "Category", "nature")}
            {field("tags", "Tags (comma-separated)", "rain, cozy")}
          </div>
          {field("media_url", "Media URL", "https://…/rainy.mp4", true)}
          {field("thumb_url", "Thumbnail URL", "https://…/rainy.jpg")}
          <div className="grid grid-cols-2 gap-3">
            {field("size_bytes", "Size (bytes)", "20000000")}
            {field("license", "License", "CC0-1.0", true)}
          </div>
          <div className="grid grid-cols-2 gap-3">
            {field("author", "Author", "Jane Doe")}
            {field("source_url", "Source URL", "https://…/original")}
          </div>
          <div className="flex items-center gap-2">
            <Switch
              id="published"
              checked={published}
              onCheckedChange={setPublished}
            />
            <label
              htmlFor="published"
              className="font-mono text-meta tracking-wider text-stone-400 uppercase"
            >
              Published
            </label>
          </div>
          <DialogFooter>
            <button
              type="submit"
              disabled={pending}
              className="inline-flex h-7 items-center gap-1.5 rounded-md bg-sky-600 px-2.5 text-sm font-medium text-white transition-colors hover:bg-sky-700 disabled:opacity-60"
            >
              {pending ? (
                <>
                  <AnimatedGlyph name="braille" active /> Saving…
                </>
              ) : (
                "Create"
              )}
            </button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

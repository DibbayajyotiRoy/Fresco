"use client";

import * as React from "react";
import { Trash } from "@phosphor-icons/react/dist/ssr";

import { toast } from "@/components/toaster";
import { confirm } from "@/components/confirm-dialog";
import { Badge } from "@/components/badges";
import { AnimatedGlyph } from "@/components/animated-glyph";
import {
  DataTable,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/data-table";
import { Switch } from "@/components/ui/switch";
import { deleteCatalogItem, setCatalogPublished } from "@/app/catalog/actions";
import { formatBytes, formatNumber, truncateId } from "@/lib/format";
import type { CatalogItem } from "@/lib/types";

function PublishToggle({ item }: { item: CatalogItem }) {
  const [checked, setChecked] = React.useState(item.published);
  const [pending, startTransition] = React.useTransition();

  React.useEffect(() => {
    setChecked(item.published);
  }, [item.published]);

  return (
    <Switch
      checked={checked}
      disabled={pending}
      aria-label="Toggle published"
      onCheckedChange={(next) => {
        setChecked(next);
        startTransition(async () => {
          const res = await setCatalogPublished(item.id, next);
          if (!res.ok) {
            setChecked(!next);
            toast.error(res.error);
          }
        });
      }}
    />
  );
}

function DeleteButton({ item }: { item: CatalogItem }) {
  const [pending, startTransition] = React.useTransition();
  return (
    <button
      type="button"
      disabled={pending}
      aria-label={`Delete ${item.title}`}
      onClick={async () => {
        const ok = await confirm({
          title: `Delete "${item.title}"?`,
          description:
            "This permanently removes the catalog entry. The media file itself is not touched.",
        });
        if (!ok) return;
        startTransition(async () => {
          const res = await deleteCatalogItem(item.id);
          if (!res.ok) toast.error(res.error);
          else toast.success("Item deleted");
        });
      }}
      className="inline-flex size-6 items-center justify-center rounded-md text-stone-400 transition-colors hover:bg-stone-100 hover:text-rose-500"
    >
      {pending ? (
        <AnimatedGlyph name="braille" active className="text-meta" />
      ) : (
        <Trash className="size-3.5" />
      )}
    </button>
  );
}

export function CatalogTable({ items }: { items: CatalogItem[] }) {
  return (
    <DataTable>
      <THead>
        <TR>
          <TH>Title</TH>
          <TH className="hidden w-[90px] xl:table-cell">Id</TH>
          <TH className="w-[110px]">Category</TH>
          <TH className="hidden w-[100px] md:table-cell">License</TH>
          <TH className="w-[80px] text-right">Size</TH>
          <TH className="w-[80px] text-right">Installs</TH>
          <TH className="w-[90px]">Published</TH>
          <TH className="w-[44px]" />
        </TR>
      </THead>
      <TBody>
        {items.map((item) => (
          <TR key={item.id}>
            <TD>
              <span className="block truncate text-sm font-medium text-stone-900">
                {item.title}
              </span>
              <span className="block truncate font-mono text-meta text-stone-400">
                {item.author || "unknown author"} · {item.content_type}
              </span>
            </TD>
            <TD className="hidden xl:table-cell">
              <span className="font-mono text-meta text-stone-500" title={item.id}>
                {truncateId(item.id)}
              </span>
            </TD>
            <TD>
              <Badge label={item.category} />
            </TD>
            <TD className="hidden md:table-cell">
              <span className="block truncate font-mono text-meta text-stone-500">
                {item.license}
              </span>
            </TD>
            <TD className="text-right">
              <span className="font-mono text-meta text-stone-500 tabular-nums">
                {formatBytes(item.size_bytes)}
              </span>
            </TD>
            <TD className="text-right text-sm text-stone-900 tabular-nums">
              {item.install_count ? formatNumber(item.install_count) : "0"}
            </TD>
            <TD>
              <PublishToggle item={item} />
            </TD>
            <TD className="text-right">
              <DeleteButton item={item} />
            </TD>
          </TR>
        ))}
      </TBody>
    </DataTable>
  );
}

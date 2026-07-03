"use client";

import * as React from "react";
import { Trash } from "@phosphor-icons/react/dist/ssr";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { deleteCatalogItem, setCatalogPublished } from "@/app/catalog/actions";
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
    <Button
      variant="ghost"
      size="icon"
      disabled={pending}
      onClick={() =>
        startTransition(async () => {
          const res = await deleteCatalogItem(item.id);
          if (!res.ok) toast.error(res.error);
          else toast.success("Item deleted");
        })
      }
    >
      <Trash />
    </Button>
  );
}

export function CatalogTable({ items }: { items: CatalogItem[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Title</TableHead>
          <TableHead>Category</TableHead>
          <TableHead>License</TableHead>
          <TableHead>Size</TableHead>
          <TableHead>Installs</TableHead>
          <TableHead>Published</TableHead>
          <TableHead />
        </TableRow>
      </TableHeader>
      <TableBody>
        {items.map((item) => (
          <TableRow key={item.id}>
            <TableCell>
              <div className="font-medium">{item.title}</div>
              <div className="text-muted-foreground text-xs">
                {item.author || "unknown author"} · {item.content_type}
              </div>
            </TableCell>
            <TableCell>
              <Badge variant="secondary">{item.category}</Badge>
            </TableCell>
            <TableCell>{item.license}</TableCell>
            <TableCell>
              {(item.size_bytes / 1_048_576).toFixed(1)} MB
            </TableCell>
            <TableCell>{item.install_count}</TableCell>
            <TableCell>
              <PublishToggle item={item} />
            </TableCell>
            <TableCell className="text-right">
              <DeleteButton item={item} />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

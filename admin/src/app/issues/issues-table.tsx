import { ArrowSquareOut } from "@phosphor-icons/react/dist/ssr";

import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatRelative } from "@/lib/format";
import type { Issue } from "@/lib/types";

export function IssuesTable({ issues }: { issues: Issue[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead className="w-[64px]">#</TableHead>
          <TableHead>Title</TableHead>
          <TableHead className="w-[180px]">Labels</TableHead>
          <TableHead className="w-[120px]">Author</TableHead>
          <TableHead className="w-[90px] text-right">Comments</TableHead>
          <TableHead className="w-[140px] text-right">Opened</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {issues.map((i) => (
          <TableRow key={i.number}>
            <TableCell className="text-muted-foreground font-mono text-xs">
              #{i.number}
            </TableCell>
            <TableCell className="max-w-[420px]">
              <a
                href={i.url}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1 text-sm font-medium hover:underline"
              >
                {i.title}
                <ArrowSquareOut className="size-3 opacity-60" />
              </a>
            </TableCell>
            <TableCell>
              {i.labels.length ? (
                i.labels.slice(0, 3).map((l) => (
                  <Badge key={l} variant="outline" className="mr-1 text-xs">
                    {l}
                  </Badge>
                ))
              ) : (
                <span className="text-muted-foreground text-xs">—</span>
              )}
            </TableCell>
            <TableCell className="text-muted-foreground text-sm">
              {i.author ?? "—"}
            </TableCell>
            <TableCell className="text-muted-foreground text-right text-sm">
              {i.comments}
            </TableCell>
            <TableCell className="text-muted-foreground text-right font-mono text-xs">
              {formatRelative(i.createdAt)}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

"use client";

import * as React from "react";

import { SentimentBadge } from "@/components/sentiment-badge";
import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatRelative } from "@/lib/format";
import type { Feedback } from "@/lib/types";

type Filter = "all" | "up" | "down";

export function FeedbackTable({ feedback }: { feedback: Feedback[] }) {
  const [filter, setFilter] = React.useState<Filter>("all");

  const rows = React.useMemo(() => {
    if (filter === "up") return feedback.filter((f) => f.rating > 0);
    if (filter === "down") return feedback.filter((f) => f.rating < 0);
    return feedback;
  }, [feedback, filter]);

  return (
    <div className="flex flex-col gap-4">
      <Tabs
        value={filter}
        onValueChange={(v) => setFilter(v as Filter)}
        className="w-fit"
      >
        <TabsList>
          <TabsTrigger value="all">All</TabsTrigger>
          <TabsTrigger value="up">👍 Up</TabsTrigger>
          <TabsTrigger value="down">👎 Down</TabsTrigger>
        </TabsList>
      </Tabs>

      {rows.length === 0 ? (
        <EmptyState
          title="No feedback in this view"
          description="Try a different filter."
        />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[110px]">Sentiment</TableHead>
              <TableHead>Comment</TableHead>
              <TableHead className="w-[120px]">Version</TableHead>
              <TableHead className="w-[120px]">OS</TableHead>
              <TableHead className="w-[140px] text-right">When</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((f) => (
              <TableRow key={f.id}>
                <TableCell>
                  <SentimentBadge rating={f.rating} />
                </TableCell>
                <TableCell className="max-w-[420px]">
                  {f.comment ? (
                    <span className="text-sm whitespace-pre-wrap">
                      {f.comment}
                    </span>
                  ) : (
                    <span className="text-muted-foreground text-sm italic">
                      No comment
                    </span>
                  )}
                </TableCell>
                <TableCell>
                  {f.app_version ? (
                    <Badge variant="outline" className="font-mono text-xs">
                      {f.app_version}
                    </Badge>
                  ) : (
                    <span className="text-muted-foreground text-xs">—</span>
                  )}
                </TableCell>
                <TableCell className="text-muted-foreground text-sm">
                  {f.os ?? "—"}
                </TableCell>
                <TableCell className="text-muted-foreground text-right font-mono text-xs">
                  {formatRelative(f.created_at)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}

"use client";

import * as React from "react";

import { SentimentBadge } from "@/components/sentiment-badge";
import { EmptyState } from "@/components/empty-state";
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
import { playTick } from "@/lib/sound";
import { formatNumber, formatRelative } from "@/lib/format";
import type { Feedback } from "@/lib/types";

type Filter = "all" | "up" | "down";

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "up", label: "Up" },
  { value: "down", label: "Down" },
];

export function FeedbackTable({ feedback }: { feedback: Feedback[] }) {
  const [filter, setFilter] = React.useState<Filter>("all");

  const rows = React.useMemo(() => {
    if (filter === "up") return feedback.filter((f) => f.rating > 0);
    if (filter === "down") return feedback.filter((f) => f.rating < 0);
    return feedback;
  }, [feedback, filter]);

  return (
    <div className="space-y-3">
      {/* Filter chip row (§9). Active = accent lane. */}
      <div
        className="flex flex-wrap items-center gap-2"
        role="group"
        aria-label="Sentiment filter"
      >
        <span className="font-mono text-meta tracking-widest text-stone-400 uppercase">
          Filter:
        </span>
        {FILTERS.map((f) => {
          const active = filter === f.value;
          return (
            <button
              key={f.value}
              type="button"
              aria-pressed={active}
              onClick={() => {
                if (!active) {
                  setFilter(f.value);
                  playTick();
                }
              }}
              className={
                "rounded-md border px-1.5 py-0.5 text-sm font-medium transition-colors " +
                (active
                  ? "border-sky-600/40 bg-sky-600/10 text-sky-600"
                  : "border-stone-200 bg-white text-stone-500 hover:bg-stone-100")
              }
            >
              {f.label}
            </button>
          );
        })}
        <span className="ml-auto font-mono text-meta text-stone-400 tabular-nums">
          {formatNumber(rows.length)} rows
        </span>
      </div>

      {rows.length === 0 ? (
        <EmptyState
          title="No feedback in this view"
          description="Try a different filter."
        />
      ) : (
        <DataTable>
          <THead>
            <TR>
              <TH className="w-[90px]">Sentiment</TH>
              <TH>Comment</TH>
              <TH className="w-[110px]">Version</TH>
              <TH className="w-[130px]">OS</TH>
              <TH className="w-[100px] text-right">When</TH>
            </TR>
          </THead>
          <TBody>
            {rows.map((f) => (
              <TR key={f.id}>
                <TD>
                  <SentimentBadge rating={f.rating} />
                </TD>
                <TD>
                  {f.comment ? (
                    <span className="block truncate text-sm text-stone-900" title={f.comment}>
                      {f.comment}
                    </span>
                  ) : (
                    <NullCell />
                  )}
                </TD>
                <TD>
                  {f.app_version ? <Badge label={f.app_version} /> : <NullCell />}
                </TD>
                <TD>
                  {f.os ? (
                    <span className="block truncate text-sm text-stone-500">
                      {f.os}
                    </span>
                  ) : (
                    <NullCell />
                  )}
                </TD>
                <TD className="text-right">
                  <span className="font-mono text-meta text-stone-500">
                    {formatRelative(f.created_at)}
                  </span>
                </TD>
              </TR>
            ))}
          </TBody>
        </DataTable>
      )}
    </div>
  );
}

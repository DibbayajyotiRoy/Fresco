import { ArrowSquareOut } from "@phosphor-icons/react/dist/ssr";

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
import { formatNumber, formatRelative } from "@/lib/format";
import type { Issue } from "@/lib/types";

export function IssuesTable({ issues }: { issues: Issue[] }) {
  return (
    <DataTable>
      <THead>
        <TR>
          <TH className="w-[60px]">#</TH>
          <TH>Title</TH>
          <TH className="hidden w-[190px] lg:table-cell">Labels</TH>
          <TH className="hidden w-[120px] md:table-cell">Author</TH>
          <TH className="w-[70px] text-right">Cmts</TH>
          <TH className="w-[100px] text-right">Opened</TH>
        </TR>
      </THead>
      <TBody>
        {issues.map((i) => (
          <TR key={i.number}>
            <TD>
              <span className="font-mono text-meta text-stone-500">
                #{i.number}
              </span>
            </TD>
            <TD>
              <a
                href={i.url}
                target="_blank"
                rel="noreferrer"
                className="inline-flex max-w-full items-center gap-1 text-sm font-medium text-sky-700 hover:underline"
              >
                <span className="truncate">{i.title}</span>
                <ArrowSquareOut className="size-3 shrink-0 opacity-60" />
              </a>
            </TD>
            <TD className="hidden lg:table-cell">
              {i.labels.length ? (
                <span className="flex flex-wrap gap-1">
                  {i.labels.slice(0, 3).map((l) => (
                    <Badge key={l} label={l} />
                  ))}
                </span>
              ) : (
                <NullCell />
              )}
            </TD>
            <TD className="hidden md:table-cell">
              {i.author ? (
                <span className="block truncate font-mono text-sm text-stone-500">
                  {i.author}
                </span>
              ) : (
                <NullCell />
              )}
            </TD>
            <TD className="text-right text-sm text-stone-500 tabular-nums">
              {formatNumber(i.comments)}
            </TD>
            <TD className="text-right">
              <span className="font-mono text-meta text-stone-500">
                {formatRelative(i.createdAt)}
              </span>
            </TD>
          </TR>
        ))}
      </TBody>
    </DataTable>
  );
}

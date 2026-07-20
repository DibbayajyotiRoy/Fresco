import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { SeverityBadge, type Severity } from "@/components/badges";
import {
  DataTable,
  NullCell,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/data-table";
import { getErrorsSince } from "@/lib/data";
import { formatNumber, formatRelative } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const DAY_MS = 24 * 60 * 60 * 1000;

/** Volume-based severity for an error group — status lane only. */
function groupSeverity(count: number): Severity {
  if (count >= 100) return "critical";
  if (count >= 20) return "error";
  if (count >= 5) return "warning";
  return "info";
}

export default async function ReliabilityPage() {
  const now = Date.now();
  const since30d = new Date(now - 30 * DAY_MS).toISOString();

  const res = await getErrorsSince(since30d);
  const errors = res.ok ? res.data : [];

  const cutoff24h = now - DAY_MS;
  const cutoff7d = now - 7 * DAY_MS;

  const errors24h = errors.filter(
    (e) => Date.parse(e.created_at) >= cutoff24h
  ).length;
  const errors7dRows = errors.filter(
    (e) => Date.parse(e.created_at) >= cutoff7d
  );
  const affected7d = new Set(
    errors7dRows.map((e) => e.install_id).filter(Boolean)
  ).size;

  type Group = {
    kind: string;
    version: string;
    count: number;
    lastSeen: string;
    latestDetail: string | null;
  };
  const groups = new Map<string, Group>();
  for (const e of errors) {
    const version = e.version?.trim() || "unknown";
    const key = `${e.kind} ${version}`;
    const g = groups.get(key);
    if (g) {
      g.count += 1;
    } else {
      groups.set(key, {
        kind: e.kind,
        version,
        count: 1,
        lastSeen: e.created_at,
        latestDetail: e.detail,
      });
    }
  }
  const grouped = [...groups.values()].sort((a, b) => b.count - a.count);

  return (
    <div className="space-y-3">
      <PageHeader
        title="Reliability"
        meta={
          res.ok
            ? `${formatNumber(errors.length)} reports / 30d · ${formatNumber(grouped.length)} groups`
            : undefined
        }
      />

      <div className="grid grid-cols-2 gap-2 lg:grid-cols-3">
        <StatCard
          label="Errors 24h"
          value={res.ok ? formatNumber(errors24h) : "—"}
          hint={res.ok ? "reports in the last 24 h" : res.error}
        />
        <StatCard
          label="Errors 7d"
          value={res.ok ? formatNumber(errors7dRows.length) : "—"}
          hint="reports in the last 7 days"
        />
        <StatCard
          label="Affected installs 7d"
          value={res.ok ? formatNumber(affected7d) : "—"}
          hint="distinct installs reporting errors"
        />
      </div>

      {!res.ok ? (
        <ErrorPanel title="Couldn't load errors" message={res.error} />
      ) : grouped.length === 0 ? (
        <EmptyState
          title="No errors in the last 30 days"
          description="Error reports sent by the app will appear here."
        />
      ) : (
        <DataTable>
          <THead>
            <TR>
              <TH className="w-[90px]">Severity</TH>
              <TH className="w-[180px]">Kind</TH>
              <TH className="w-[100px]">Version</TH>
              <TH className="w-[80px] text-right">Count</TH>
              <TH className="w-[100px] text-right">Last seen</TH>
              <TH>Latest detail</TH>
            </TR>
          </THead>
          <TBody>
            {grouped.map((g) => (
              <TR key={`${g.kind}-${g.version}`}>
                <TD>
                  <SeverityBadge severity={groupSeverity(g.count)} />
                </TD>
                <TD>
                  <span className="block truncate font-mono text-sm font-medium text-stone-900">
                    {g.kind}
                  </span>
                </TD>
                <TD>
                  <span className="font-mono text-meta text-stone-500">
                    {g.version}
                  </span>
                </TD>
                <TD className="text-right text-sm text-stone-900 tabular-nums">
                  {formatNumber(g.count)}
                </TD>
                <TD className="text-right">
                  <span className="font-mono text-meta text-stone-500">
                    {formatRelative(g.lastSeen)}
                  </span>
                </TD>
                <TD>
                  {g.latestDetail ? (
                    <span
                      className="block truncate font-mono text-meta text-stone-500"
                      title={g.latestDetail}
                    >
                      {g.latestDetail}
                    </span>
                  ) : (
                    <NullCell />
                  )}
                </TD>
              </TR>
            ))}
          </TBody>
        </DataTable>
      )}
    </div>
  );
}

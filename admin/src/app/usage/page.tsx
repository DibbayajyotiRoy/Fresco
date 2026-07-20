import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { Panel, PanelHeader } from "@/components/panel";
import {
  DataTable,
  NullCell,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/data-table";
import {
  DistributionList,
  type DistributionItem,
} from "@/components/distribution-list";
import {
  getEventsSince,
  getFeatureEventsSince,
  getInstalls,
} from "@/lib/data";
import type { FeatureEvent } from "@/lib/types";
import { formatNumber, formatRelative, truncateId } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const DAY_MS = 24 * 60 * 60 * 1000;

/** Bucket freeform values into a top-N breakdown with an "Other" rollup. */
function topDistribution(
  values: (string | null)[],
  n = 6
): DistributionItem[] {
  const counts = new Map<string, number>();
  for (const v of values) {
    const key = v?.trim() || "Unknown";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const sorted = [...counts.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value);

  if (sorted.length <= n) return sorted;
  const head = sorted.slice(0, n);
  const other = sorted.slice(n).reduce((s, i) => s + i.value, 0);
  return [...head, { label: "Other", value: other }];
}

const DEPTH_EVENTS = ["add_from_link", "browser_wallpaper_set"] as const;

/** Bucket a freeform prop value, mapping absent/odd values to "unknown". */
function propBucket(props: Record<string, unknown> | null, key: string): string {
  const v = props?.[key];
  return typeof v === "string" && v.trim() ? v : "unknown";
}

type LinkRow = {
  source: string;
  kind: string;
  outcome: string;
  c7: number;
  c30: number;
};

/** source × kind × outcome counts over 7d/30d for add_from_link. */
function linkBreakdown(events: FeatureEvent[], cutoff7d: number): LinkRow[] {
  const rows = new Map<string, LinkRow>();
  for (const e of events) {
    if (e.name !== "add_from_link") continue;
    const source = propBucket(e.props, "source");
    const kind = propBucket(e.props, "kind");
    const ok = e.props?.ok;
    const outcome = ok === true ? "ok" : ok === false ? "failed" : "unknown";
    const key = `${source}|${kind}|${outcome}`;
    const row = rows.get(key) ?? { source, kind, outcome, c7: 0, c30: 0 };
    row.c30 += 1;
    if (Date.parse(e.created_at) >= cutoff7d) row.c7 += 1;
    rows.set(key, row);
  }
  return [...rows.values()].sort(
    (a, b) =>
      b.c30 - a.c30 ||
      a.source.localeCompare(b.source) ||
      a.kind.localeCompare(b.kind)
  );
}

type DepthSummary = {
  event: string;
  installs: number;
  median: number;
  max: number;
};

type TopInstall = {
  installId: string;
  event: string;
  c30: number;
  lastUsed: string;
};

/** Per-install depth (30d): distinct installs, median/max events per
 *  using-install, and the heaviest install×event pairs. */
function installDepth(events: FeatureEvent[]): {
  summaries: DepthSummary[];
  top: TopInstall[];
} {
  const perPair = new Map<string, TopInstall>();
  for (const e of events) {
    if (!e.install_id) continue;
    const key = `${e.name}|${e.install_id}`;
    const entry =
      perPair.get(key) ??
      ({ installId: e.install_id, event: e.name, c30: 0, lastUsed: e.created_at } as TopInstall);
    entry.c30 += 1;
    if (e.created_at > entry.lastUsed) entry.lastUsed = e.created_at;
    perPair.set(key, entry);
  }

  const summaries = DEPTH_EVENTS.map((event) => {
    const counts = [...perPair.values()]
      .filter((p) => p.event === event)
      .map((p) => p.c30)
      .sort((a, b) => a - b);
    const n = counts.length;
    const median =
      n === 0
        ? 0
        : n % 2 === 1
          ? counts[(n - 1) / 2]
          : (counts[n / 2 - 1] + counts[n / 2]) / 2;
    return { event, installs: n, median, max: n ? counts[n - 1] : 0 };
  });

  const top = [...perPair.values()]
    .sort((a, b) => b.c30 - a.c30 || b.lastUsed.localeCompare(a.lastUsed))
    .slice(0, 8);

  return { summaries, top };
}

export default async function UsagePage() {
  const now = Date.now();
  const since30d = new Date(now - 30 * DAY_MS).toISOString();

  const [installsRes, eventsRes, depthRes] = await Promise.all([
    getInstalls(),
    getEventsSince(since30d),
    getFeatureEventsSince(since30d, [...DEPTH_EVENTS]),
  ]);

  const installs = installsRes.ok ? installsRes.data : [];
  const events = eventsRes.ok ? eventsRes.data : [];

  const activeIn = (days: number) => {
    const cutoff = now - days * DAY_MS;
    return installs.filter((i) => Date.parse(i.last_seen) >= cutoff).length;
  };
  const activeToday = activeIn(1);
  const active7d = activeIn(7);
  const active30d = activeIn(30);

  const versionDist = topDistribution(installs.map((i) => i.version));
  const distroDist = topDistribution(installs.map((i) => i.distro));
  const compositorDist = topDistribution(installs.map((i) => i.compositor));
  const sessionDist = topDistribution(installs.map((i) => i.session));
  const decodeDist = topDistribution(installs.map((i) => i.decode));
  const sourceDist = topDistribution(installs.map((i) => i.source));
  const channelDist = topDistribution(installs.map((i) => i.channel));

  const cutoff7d = now - 7 * DAY_MS;
  const featureCounts = new Map<string, { c7: number; c30: number }>();
  for (const e of events) {
    const entry = featureCounts.get(e.name) ?? { c7: 0, c30: 0 };
    entry.c30 += 1;
    if (Date.parse(e.created_at) >= cutoff7d) entry.c7 += 1;
    featureCounts.set(e.name, entry);
  }
  const features = [...featureCounts.entries()]
    .map(([name, c]) => ({ name, ...c }))
    .sort((a, b) => b.c30 - a.c30);

  const depthEvents = depthRes.ok ? depthRes.data : [];
  const linkRows = linkBreakdown(depthEvents, cutoff7d);
  const { summaries, top } = installDepth(depthEvents);

  const breakdowns: { title: string; items: DistributionItem[] }[] = [
    { title: "Distro", items: distroDist },
    { title: "Compositor", items: compositorDist },
    { title: "Session", items: sessionDist },
    { title: "Decode", items: decodeDist },
    { title: "Download source", items: sourceDist },
    { title: "Channel", items: channelDist },
  ];

  return (
    <div className="space-y-3">
      <PageHeader
        title="Usage"
        meta={
          installsRes.ok
            ? `${formatNumber(installs.length)} installs · ${formatNumber(events.length)} events / 30d`
            : undefined
        }
      />

      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        <StatCard
          label="Active today"
          value={installsRes.ok ? formatNumber(activeToday) : "—"}
          hint={installsRes.ok ? "seen in the last 24 h" : installsRes.error}
        />
        <StatCard
          label="Active 7d"
          value={installsRes.ok ? formatNumber(active7d) : "—"}
          hint="seen in the last 7 days"
        />
        <StatCard
          label="Active 30d"
          value={installsRes.ok ? formatNumber(active30d) : "—"}
          hint="seen in the last 30 days"
        />
        <StatCard
          label="Total installs"
          value={installsRes.ok ? formatNumber(installs.length) : "—"}
          hint="all installs ever seen"
        />
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
        <Panel>
          <PanelHeader title="App version" meta="by install" />
          {!installsRes.ok ? (
            <ErrorPanel
              title="Couldn't load installs"
              message={installsRes.error}
            />
          ) : versionDist.length === 0 ? (
            <EmptyState
              className="py-6"
              title="No data yet"
              description="Versions arrive with install telemetry."
            />
          ) : (
            <DistributionList items={versionDist} total={installs.length} />
          )}
        </Panel>

        <Panel className="lg:col-span-2">
          <PanelHeader
            title="Feature usage"
            meta="events by name · last 30 days"
          />
          {!eventsRes.ok ? (
            <ErrorPanel title="Couldn't load events" message={eventsRes.error} />
          ) : features.length === 0 ? (
            <EmptyState
              title="No events yet"
              description="Feature events sent by the app will appear here."
            />
          ) : (
            <DataTable>
              <THead>
                <TR>
                  <TH>Event</TH>
                  <TH className="w-[100px] text-right">7d</TH>
                  <TH className="w-[100px] text-right">30d</TH>
                </TR>
              </THead>
              <TBody>
                {features.map((f) => (
                  <TR key={f.name}>
                    <TD>
                      <span className="block truncate font-mono text-sm text-stone-900">
                        {f.name}
                      </span>
                    </TD>
                    <TD className="text-right text-sm text-stone-900 tabular-nums">
                      {formatNumber(f.c7)}
                    </TD>
                    <TD className="text-right text-sm text-stone-900 tabular-nums">
                      {formatNumber(f.c30)}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </DataTable>
          )}
        </Panel>
      </div>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
        {breakdowns.map((b) => (
          <Panel key={b.title}>
            <PanelHeader title={b.title} meta="by install" />
            {b.items.length === 0 ? (
              <EmptyState
                className="py-6"
                title="No data yet"
                description="Arrives with install telemetry."
              />
            ) : (
              <DistributionList items={b.items} total={installs.length} />
            )}
          </Panel>
        ))}
      </div>

      <div>
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
          <Panel>
            <PanelHeader
              title="Feature depth"
              meta="add_from_link · source × kind × outcome"
            />
            {!depthRes.ok ? (
              <ErrorPanel
                title="Couldn't load events"
                message={depthRes.error}
              />
            ) : linkRows.length === 0 ? (
              <EmptyState
                title="No add_from_link events yet"
                description="Sent when a wallpaper is added from a pasted link."
              />
            ) : (
              <DataTable>
                <THead>
                  <TR>
                    <TH>Source</TH>
                    <TH>Kind</TH>
                    <TH>Outcome</TH>
                    <TH className="w-[72px] text-right">7d</TH>
                    <TH className="w-[72px] text-right">30d</TH>
                  </TR>
                </THead>
                <TBody>
                  {linkRows.map((r) => (
                    <TR key={`${r.source}|${r.kind}|${r.outcome}`}>
                      <TD className="font-mono text-sm text-stone-900">
                        {r.source}
                      </TD>
                      <TD className="font-mono text-sm text-stone-900">
                        {r.kind === "unknown" ? (
                          <NullCell />
                        ) : (
                          r.kind
                        )}
                      </TD>
                      <TD
                        className={
                          r.outcome === "failed"
                            ? "font-mono text-sm text-stone-500"
                            : "font-mono text-sm text-stone-900"
                        }
                      >
                        {r.outcome}
                      </TD>
                      <TD className="text-right text-sm text-stone-900 tabular-nums">
                        {formatNumber(r.c7)}
                      </TD>
                      <TD className="text-right text-sm text-stone-900 tabular-nums">
                        {formatNumber(r.c30)}
                      </TD>
                    </TR>
                  ))}
                </TBody>
              </DataTable>
            )}
          </Panel>

          <Panel>
            <PanelHeader
              title="Per-install depth"
              meta="events per using-install · last 30 days"
            />
            {!depthRes.ok ? (
              <ErrorPanel
                title="Couldn't load events"
                message={depthRes.error}
              />
            ) : depthEvents.length === 0 ? (
              <EmptyState
                title="No feature events yet"
                description="add_from_link and browser_wallpaper_set events will appear here."
              />
            ) : (
              <div className="space-y-3">
                <DataTable>
                  <THead>
                    <TR>
                      <TH>Event</TH>
                      <TH className="w-[90px] text-right">Installs</TH>
                      <TH className="w-[90px] text-right">Median</TH>
                      <TH className="w-[90px] text-right">Max</TH>
                    </TR>
                  </THead>
                  <TBody>
                    {summaries.map((s) => (
                      <TR key={s.event}>
                        <TD>
                          <span className="block truncate font-mono text-sm text-stone-900">
                            {s.event}
                          </span>
                        </TD>
                        <TD className="text-right text-sm text-stone-900 tabular-nums">
                          {formatNumber(s.installs)}
                        </TD>
                        <TD className="text-right text-sm text-stone-900 tabular-nums">
                          {s.installs ? formatNumber(s.median) : <NullCell />}
                        </TD>
                        <TD className="text-right text-sm text-stone-900 tabular-nums">
                          {s.installs ? formatNumber(s.max) : <NullCell />}
                        </TD>
                      </TR>
                    ))}
                  </TBody>
                </DataTable>

                <DataTable>
                  <THead>
                    <TR>
                      <TH className="w-[110px]">Install</TH>
                      <TH>Event</TH>
                      <TH className="w-[72px] text-right">30d</TH>
                      <TH className="w-[110px] text-right">Last used</TH>
                    </TR>
                  </THead>
                  <TBody>
                    {top.map((t) => (
                      <TR key={`${t.event}|${t.installId}`}>
                        <TD className="font-mono text-sm text-stone-900">
                          {truncateId(t.installId)}
                        </TD>
                        <TD>
                          <span className="block truncate font-mono text-sm text-stone-900">
                            {t.event}
                          </span>
                        </TD>
                        <TD className="text-right text-sm text-stone-900 tabular-nums">
                          {formatNumber(t.c30)}
                        </TD>
                        <TD className="text-right font-mono text-sm text-stone-500 tabular-nums">
                          {formatRelative(t.lastUsed)}
                        </TD>
                      </TR>
                    ))}
                  </TBody>
                </DataTable>
              </div>
            )}
          </Panel>
        </div>
        <p className="mt-1.5 font-mono text-meta text-stone-400">
          Counts include only users who opted into anonymous statistics.
        </p>
      </div>
    </div>
  );
}

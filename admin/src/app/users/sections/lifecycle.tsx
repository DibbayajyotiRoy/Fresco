import { DataTable, NullCell, TBody, TD, TH, THead, TR } from "@/components/data-table";
import { DistributionList } from "@/components/distribution-list";
import { EmptyState } from "@/components/empty-state";
import { InfoHint } from "@/components/info-hint";
import { Panel, PanelHeader } from "@/components/panel";
import { SeverityBadge } from "@/components/badges";
import { getInstallDaysSince, getInstalls } from "@/lib/data";
import {
  ACTIVE_WITHIN_DAYS,
  IDLE_WITHIN_DAYS,
  activationFunnel,
  lifecycleCounts,
  retentionCohorts,
  stickiness,
} from "@/lib/install-analytics";
import { formatDate, formatNumber } from "@/lib/format";

function pct(n: number | null): string {
  return n === null ? "—" : `${Math.round(n * 100)}%`;
}

/**
 * Lifecycle breakdown (active / idle / lapsed), weekly stickiness (DAU/WAU/
 * MAU), and a D1/D7/D30 retention cohort table.
 *
 * These three share one data fetch (installs + install_days) and one
 * question: not just "how many users", but "are they staying". A KPI strip
 * says how many; this section is where the owner sees whether that number is
 * growing because people stay, or only because new installs keep replacing
 * ones that left.
 */
export async function LifecycleSection() {
  const now = Date.now();
  // Full history, not a recent window: `activationFunnel`'s "retained" stage
  // needs to know whether an install EVER checked in on a second distinct
  // day, and `retentionCohorts` reports its own "collecting since" bound
  // from whatever it is given — passing everything makes that bound the
  // true earliest date rather than an artificial 120-day cutoff.
  const [installsRes, daysRes] = await Promise.all([
    getInstalls(),
    getInstallDaysSince("1970-01-01"),
  ]);

  if (!installsRes.ok) {
    return (
      <Panel>
        <PanelHeader title="Lifecycle" />
        <EmptyState title="Couldn't load installs" description={installsRes.error} />
      </Panel>
    );
  }

  const installs = installsRes.data;
  const counts = lifecycleCounts(installs, now);
  const installDays = daysRes.ok ? daysRes.data : [];
  const sticky = stickiness(installDays, now);
  const { cohorts, historySince } = retentionCohorts(installDays, installs, now);
  const funnel = activationFunnel(installs, installDays, now);

  return (
    <>
      <FunnelPanel funnel={funnel} />
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-5">
      <Panel className="lg:col-span-2">
        <PanelHeader
          title="Lifecycle"
          meta={`${formatNumber(counts.total)} installs ever`}
        />
        <p className="mb-2 text-sm leading-snug text-stone-500">
          Active: checked in within {ACTIVE_WITHIN_DAYS} days. Idle: within{" "}
          {IDLE_WITHIN_DAYS} days. Lapsed: silent longer than that — the best
          inference of an uninstall this telemetry can make, since there is no
          uninstall event.
        </p>
        <DistributionList
          items={[
            { label: "Active", value: counts.active },
            { label: "Idle", value: counts.idle },
            { label: "Lapsed", value: counts.lapsed },
          ]}
          total={counts.total}
        />
        <dl className="mt-3 grid grid-cols-3 gap-2 border-t border-stone-200 pt-2.5">
          <div>
            <dt className="font-mono text-meta text-stone-400">DAU</dt>
            <dd className="text-sm font-medium text-stone-900 tabular-nums">
              {formatNumber(sticky.dau)}
            </dd>
          </div>
          <div>
            <dt className="font-mono text-meta text-stone-400">WAU</dt>
            <dd className="text-sm font-medium text-stone-900 tabular-nums">
              {formatNumber(sticky.wau)}
            </dd>
          </div>
          <div>
            <dt className="font-mono text-meta text-stone-400">MAU</dt>
            <dd className="text-sm font-medium text-stone-900 tabular-nums">
              {formatNumber(sticky.mau)}
            </dd>
          </div>
        </dl>
        <p className="mt-2 font-mono text-meta text-stone-400">
          Stickiness (DAU/MAU): {sticky.stickinessPct === null ? "—" : `${sticky.stickinessPct}%`}
        </p>
      </Panel>

      <Panel className="lg:col-span-3">
        <PanelHeader
          title="Retention by week of first install"
          meta={
            historySince
              ? `collecting since ${formatDate(historySince)}`
              : "no history yet"
          }
        />
        <p className="mb-2 text-sm leading-snug text-stone-500">
          Of installs first seen in a given week, the share that checked in
          again 1 / 7 / 30 days later. A dash means that horizon has not been
          reached yet for any install in the cohort, not zero.
        </p>
        {cohorts.length === 0 ? (
          <EmptyState
            className="py-8"
            title="No cohorts yet"
            description="Needs install_days history — see supabase/migrations/2026-09-24_install_days.sql."
          />
        ) : (
          <DataTable maxHeight="16rem">
            <THead sticky="container">
              <TR>
                <TH className="w-[110px]">Week of</TH>
                <TH className="w-[70px] text-right">Size</TH>
                <TH className="w-[70px] text-right">D1</TH>
                <TH className="w-[70px] text-right">D7</TH>
                <TH className="text-right">D30</TH>
              </TR>
            </THead>
            <TBody>
              {cohorts.map((c) => (
                <TR key={c.weekStart}>
                  <TD className="font-mono text-sm">{formatDate(c.weekStart)}</TD>
                  <TD className="text-right text-sm">{formatNumber(c.size)}</TD>
                  <TD className="text-right text-sm">
                    {c.d1 === null ? <NullCell /> : pct(c.d1)}
                  </TD>
                  <TD className="text-right text-sm">
                    {c.d7 === null ? <NullCell /> : pct(c.d7)}
                  </TD>
                  <TD className="text-right text-sm">
                    {c.d30 === null ? <NullCell /> : pct(c.d30)}
                  </TD>
                </TR>
              ))}
            </TBody>
          </DataTable>
        )}
      </Panel>
      </div>
    </>
  );
}

/**
 * The activation funnel: installs ever -> activated -> retained -> active
 * 7d/30d. "Retained" (checked in on a second distinct day) is called out as
 * the headline "real users" figure — see `activationFunnel` in
 * install-analytics.ts for exactly what filters an install out at each
 * stage and why.
 */
function FunnelPanel({ funnel }: { funnel: ReturnType<typeof activationFunnel> }) {
  const pctOf = (n: number) =>
    funnel.installsEver > 0 ? `${Math.round((n / funnel.installsEver) * 100)}%` : "—";

  const stages: { label: string; value: number; hint?: string; headline?: boolean }[] = [
    { label: "Installs ever", value: funnel.installsEver, hint: "distinct install_id, all time" },
    { label: "Activated", value: funnel.activated, hint: "≥1 check-in — every install row" },
    {
      label: "Retained",
      value: funnel.retained,
      hint: "checked in on a 2nd distinct day",
      headline: true,
    },
    { label: "Active 7d", value: funnel.active7d, hint: "checked in within 7 days" },
    { label: "Active 30d", value: funnel.active30d, hint: "checked in within 30 days" },
  ];

  return (
    <Panel>
      <PanelHeader title="Activation funnel" meta="of installs ever" />
      <p className="mb-2 max-w-3xl text-sm leading-snug text-stone-500">
        One install_id is one real user: reinstalling or re-running the
        installer keeps the same id (it is stored next to the app&rsquo;s
        config and survives an update or reinstall), so this never
        double-counts a user for updating. A new id only appears for a
        genuinely different install — another machine, another OS user
        account, or the Flatpak build running alongside the .deb (it uses a
        separate, sandboxed config directory). GitHub download counts are a
        different, larger number — re-running the installer re-downloads —
        and are not mixed in here.
      </p>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
        {stages.map((s) => (
          <div key={s.label}>
            <div className="flex items-center gap-1">
              <span className="font-mono text-meta text-stone-500">{s.label}</span>
              {s.hint ? <InfoHint label={s.hint} /> : null}
            </div>
            <div
              className={
                "mt-1 tabular-nums " +
                (s.headline
                  ? "text-xl font-semibold text-stone-900"
                  : "text-base font-medium text-stone-700")
              }
            >
              {formatNumber(s.value)}
            </div>
            <div className="font-mono text-meta text-stone-400">{pctOf(s.value)}</div>
          </div>
        ))}
      </div>
    </Panel>
  );
}

/** Exported so a table row elsewhere (installs table) can render the same
 *  status pill without re-deriving the tone mapping. */
export function LifecycleBadge({ status }: { status: "active" | "idle" | "lapsed" }) {
  if (status === "active") return <SeverityBadge severity="ok" label="Active" />;
  if (status === "idle") return <SeverityBadge severity="warning" label="Idle" />;
  return <SeverityBadge severity="info" label="Lapsed" />;
}

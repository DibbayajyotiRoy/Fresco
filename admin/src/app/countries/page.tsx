import { Suspense } from "react";

import { DataTable, NullCell, TBody, TD, TH, THead, TR } from "@/components/data-table";
import { EmptyState } from "@/components/empty-state";
import { PageHeader } from "@/components/page-header";
import { Panel, PanelHeader } from "@/components/panel";
import { PanelSkeleton } from "@/components/skeleton";
import { getInstallDaysSince, getInstalls } from "@/lib/data";
import { countryLabel, countryRegion } from "@/lib/geo";
import { perCountryTable } from "@/lib/install-analytics";
import { formatNumber } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

async function CountriesMeta() {
  const res = await getInstalls();
  if (!res.ok) return undefined;
  const n = new Set(res.data.map((i) => i.country).filter(Boolean)).size;
  return `${formatNumber(n)} countries`;
}

async function CountriesSection() {
  const now = Date.now();
  // Full history, not just "since yesterday": `retained` needs to know
  // whether an install EVER checked in on a second distinct day.
  const [installsRes, daysRes] = await Promise.all([
    getInstalls(),
    getInstallDaysSince("1970-01-01"),
  ]);

  if (!installsRes.ok) {
    return <EmptyState title="Couldn't load installs" description={installsRes.error} />;
  }

  const rows = perCountryTable(installsRes.data, daysRes.ok ? daysRes.data : [], now);
  const total = installsRes.data.length;

  if (rows.length === 0) {
    return (
      <EmptyState
        title="No countries recorded yet"
        description="Country is resolved server-side as installs check in."
      />
    );
  }

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <a
          href="/api/export/countries.csv"
          className="press h-8 rounded-md border border-stone-300 bg-white px-3 text-sm font-medium text-stone-700 hover:bg-stone-100"
        >
          Export CSV
        </a>
      </div>
      <DataTable maxHeight="34rem">
        <THead sticky="container">
          <TR>
            <TH className="w-[180px]">Country</TH>
            <TH className="w-[110px]">Region</TH>
            <TH className="text-right">Total</TH>
            <TH className="text-right">Retained</TH>
            <TH className="text-right">Retained share</TH>
            <TH className="text-right">Active 7d</TH>
            <TH className="text-right">Check-ins today</TH>
            <TH className="text-right">New 30d</TH>
            <TH className="text-right">Lapsed</TH>
            <TH className="text-right">Lapse rate</TH>
            <TH className="w-[100px]">Top version</TH>
          </TR>
        </THead>
        <TBody>
          {rows.map((r) => (
            <TR key={r.country}>
              <TD className="truncate text-sm" title={countryLabel(r.country)}>
                {countryLabel(r.country)}
              </TD>
              <TD className="truncate text-sm text-stone-500">
                {countryRegion(r.country === "??" ? null : r.country)}
              </TD>
              <TD className="text-right text-sm">{formatNumber(r.total)}</TD>
              <TD className="text-right text-sm">{formatNumber(r.retained)}</TD>
              <TD className="text-right text-sm">
                {r.retainedRatePct === null ? <NullCell /> : `${r.retainedRatePct}%`}
              </TD>
              <TD className="text-right text-sm">{formatNumber(r.active7d)}</TD>
              <TD className="text-right text-sm">{formatNumber(r.checkinsToday)}</TD>
              <TD className="text-right text-sm">{formatNumber(r.new30d)}</TD>
              <TD className="text-right text-sm">{formatNumber(r.lapsed)}</TD>
              <TD className="text-right text-sm">
                {r.lapseRatePct === null ? <NullCell /> : `${r.lapseRatePct}%`}
              </TD>
              <TD className="text-sm">{r.topVersion ?? <NullCell />}</TD>
            </TR>
          ))}
        </TBody>
      </DataTable>
      <p className="font-mono text-meta text-stone-400">
        {formatNumber(rows.length)} countries · {formatNumber(total)} installs total
      </p>
    </div>
  );
}

/**
 * Countries — the full per-country breakdown, with region rollup, that the
 * Usage page's globe/top-12 view deliberately does not try to be. Every
 * country a user has ever checked in from, sorted by install count, each
 * with its own activity and lapse rate — the drilldown the owner asked for.
 */
export default function CountriesPage() {
  return (
    <div className="space-y-4">
      <PageHeader
        title="Countries"
        action={
          <Suspense fallback={null}>
            <CountriesMeta />
          </Suspense>
        }
      />
      <Panel>
        <PanelHeader title="Installs by country" meta="all time" />
        <p className="mb-2 max-w-3xl text-sm leading-snug text-stone-500">
          Country is resolved server-side from the network edge (Cloudflare),
          never from a stored IP, so it applies under both consent tiers and
          cannot be spoofed. &ldquo;Unknown&rdquo; is a real answer — see the
          Usage page&rsquo;s Where section for why some installs carry it.
          &ldquo;Retained&rdquo; is installs that checked in on a second
          distinct day — its share tells apart a country with many real users
          from one padded by one-off tries, without either looking bigger
          than it is next to raw totals alone.
        </p>
        <Suspense fallback={<PanelSkeleton rows={10} />}>
          <CountriesSection />
        </Suspense>
      </Panel>
    </div>
  );
}

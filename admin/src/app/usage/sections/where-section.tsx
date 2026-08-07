import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { InstallGlobe } from "@/components/install-globe";
import { Notice } from "@/components/notice";
import { Panel, PanelHeader } from "@/components/panel";
import {
  DistributionList,
  type DistributionItem,
} from "@/components/distribution-list";
import { getInstalls } from "@/lib/data";
import type { Install } from "@/lib/types";
import { countryLabel, countryRegion } from "@/lib/geo";
import { formatDate, formatNumber } from "@/lib/format";
import { cohorts, topDistribution } from "./shared";

/**
 * Why some installs have no country, told apart rather than lumped together.
 *
 * `installs.country` is stamped server-side from Cloudflare's `CF-IPCountry`
 * by `request_country()` (supabase/schema.sql). The column and that function
 * were added after telemetry had already been collecting for weeks, and
 * `register_install` only ever fills a country forward
 * (`coalesce(excluded.country, installs.country)`) — there is no stored IP to
 * backfill from, by design. So a row written before that migration carries a
 * null country until its install checks in again, at which point it fills
 * itself in.
 *
 * That produces two populations that look identical in a `group by country`
 * and mean completely different things:
 *
 *   - **stale** — last seen before country resolution went live. Not a fault.
 *     Self-heals on the next check-in; the ones that never return are simply
 *     users who stopped running Fresco before we started recording it.
 *   - **live** — checked in *after* it went live and still arrived without a
 *     country. That is the only population worth investigating: a VPN or Tor
 *     exit ('T1'), an anonymising proxy, or Cloudflare answering 'XX'.
 *
 * The boundary is derived from the data rather than hardcoded, so it stays
 * correct if the migration is ever re-run or the project moves: it is the
 * earliest `last_seen` among rows that *do* carry a country.
 */
function countryCoverage(installs: Install[]) {
  const resolved = installs.filter((i) => i.country);
  const missing = installs.filter((i) => !i.country);

  const liveSince =
    resolved.length > 0
      ? resolved.reduce(
          (min, i) => (i.last_seen < min ? i.last_seen : min),
          resolved[0].last_seen
        )
      : null;

  // With no resolved row at all there is no boundary to compare against, so
  // nothing can be called stale — every gap is treated as live, which is the
  // conservative reading (it under-claims that things are fine).
  const stale = liveSince
    ? missing.filter((i) => i.last_seen < liveSince)
    : [];
  const live = missing.length - stale.length;

  return {
    resolved: resolved.length,
    missing: missing.length,
    stale: stale.length,
    live,
    liveSince,
    coverage:
      installs.length > 0
        ? Math.round((resolved.length / installs.length) * 100)
        : null,
  };
}

/** Keyed by raw code for the globe, which needs ISO alpha-2 to match polygons. */
function countsByCode(installs: Install[]): Map<string, number> {
  const byCode = new Map<string, number>();
  for (const i of installs) {
    if (!i.country || i.country === "??") continue;
    byCode.set(i.country, (byCode.get(i.country) ?? 0) + 1);
  }
  return byCode;
}

export async function WhereMeta() {
  const installsRes = await getInstalls();
  const installs = installsRes.ok ? installsRes.data : [];
  const coverage = countryCoverage(installs);
  if (coverage.coverage === null) return null;
  return `${coverage.coverage}% of installs located · ${formatNumber(countsByCode(installs).size)} countries`;
}

export async function WhereSection() {
  const installsRes = await getInstalls();
  const installs = installsRes.ok ? installsRes.data : [];
  const { full: fullInstalls, minimal: minimalInstalls } = cohorts(installs);

  const coverage = countryCoverage(installs);

  const byCode = countsByCode(installs);
  const globeCounts = [...byCode.entries()]
    .map(([code, count]) => ({ code, count }))
    .sort((a, b) => b.count - a.count);

  const countryDist: DistributionItem[] = globeCounts
    .slice(0, 12)
    .map(({ code, count }) => ({ label: countryLabel(code), value: count }));

  // Region rollup: twelve country rows is a list, six region rows is a shape.
  const regionCounts = new Map<string, number>();
  for (const i of installs) {
    if (!i.country || i.country === "??") continue;
    const r = countryRegion(i.country);
    regionCounts.set(r, (regionCounts.get(r) ?? 0) + 1);
  }
  const regionDist: DistributionItem[] = [...regionCounts.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value);

  // City is optional-tier only, so the base is full-consent rows and the panel
  // says so — an essential row has no city because it was never sent, which is
  // a different thing from a full-consent row whose city did not resolve.
  const cityDist = topDistribution(
    fullInstalls.map((i) =>
      i.city ? (i.region ? `${i.city}, ${i.region}` : i.city) : null
    ),
    10
  );

  const versionDist = topDistribution(fullInstalls.map((i) => i.version));

  return (
    <>
      {coverage.missing > 0 ? (
        <Notice
          tone={coverage.live > 0 ? "warn" : "info"}
          label={
            coverage.live > 0
              ? `${formatNumber(coverage.live)} unresolved`
              : "unknown explained"
          }
          title={
            coverage.stale > 0 && coverage.live === 0
              ? `All ${formatNumber(coverage.missing)} "Unknown" installs predate country resolution. Nothing is broken.`
              : `${formatNumber(coverage.missing)} installs show "Unknown" — ${formatNumber(coverage.stale)} historical, ${formatNumber(coverage.live)} genuinely unresolved.`
          }
        >
          <ul className="ml-4 list-disc space-y-1">
            {coverage.stale > 0 && coverage.liveSince ? (
              <li>
                <strong className="font-medium text-stone-900">
                  {formatNumber(coverage.stale)} historical.
                </strong>{" "}
                Country resolution went live on{" "}
                <span className="font-mono text-[0.9em]">
                  {formatDate(coverage.liveSince)}
                </span>
                , and these installs have not checked in since. There is no
                stored IP to backfill from — deliberately — so each one fills
                in by itself the next time it pings. The ones that never come
                back are users who stopped running Fresco before the country
                was ever recorded, and they will read Unknown permanently.
              </li>
            ) : null}
            {coverage.live > 0 ? (
              <li>
                <strong className="font-medium text-stone-900">
                  {formatNumber(coverage.live)} genuinely unresolved.
                </strong>{" "}
                These checked in after resolution was live and still arrived
                without a country: a VPN or Tor exit (Cloudflare reports{" "}
                <code className="font-mono text-[0.85em]">T1</code>), an
                anonymising proxy, or an address Cloudflare could not place (
                <code className="font-mono text-[0.85em]">XX</code>). Both are
                discarded rather than stored as a place.
              </li>
            ) : null}
            <li className="text-stone-500">
              To confirm the edge header still arrives, call the diagnostic:{" "}
              <code className="font-mono text-[0.85em]">
                POST /rest/v1/rpc/whats_my_country
              </code>
              . It returns the caller&rsquo;s own two-letter code, or null if
              the header is missing.
            </li>
          </ul>
        </Notice>
      ) : null}

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-5">
        <Panel className="lg:col-span-3">
          <PanelHeader
            title="Where Fresco runs"
            meta={
              byCode.size === 0
                ? "no data"
                : `${formatNumber(byCode.size)} countries`
            }
          />
          {globeCounts.length === 0 ? (
            <EmptyState
              className="py-10"
              title="Nothing to plot yet"
              description="Countries arrive as clients check in. The edge header is confirmed working."
            />
          ) : (
            <InstallGlobe
              counts={globeCounts}
              className="mx-auto w-full max-w-[420px]"
            />
          )}
        </Panel>

        <div className="flex flex-col gap-3 lg:col-span-2">
          <Panel>
            <PanelHeader
              title="Country"
              meta={`${formatNumber(coverage.resolved)} located · every tier`}
            />
            {countryDist.length === 0 ? (
              <EmptyState
                className="py-6"
                title="No country data yet"
                description="Countries arrive as clients check in."
              />
            ) : (
              <DistributionList
                items={countryDist}
                total={coverage.resolved}
              />
            )}
          </Panel>

          <Panel>
            <PanelHeader title="Region" meta="located installs" />
            {regionDist.length === 0 ? (
              <EmptyState
                className="py-6"
                title="No data yet"
                description="Rolls up from country."
              />
            ) : (
              <DistributionList
                items={regionDist}
                total={coverage.resolved}
              />
            )}
          </Panel>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
        <Panel>
          <PanelHeader
            title="City"
            meta={`${formatNumber(fullInstalls.length)} full-consent installs`}
          />
          {cityDist.length === 0 ? (
            <EmptyState
              className="py-6"
              title="No city data yet"
              description="City is optional-tier only and arrives from the landing site's /api/geo."
            />
          ) : (
            <DistributionList items={cityDist} total={fullInstalls.length} />
          )}
        </Panel>

        <Panel>
          <PanelHeader title="App version" meta="full-consent installs" />
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

        <Panel>
          <PanelHeader title="Consent split" meta="all installs" />
          <p className="mb-2 text-sm leading-snug text-stone-500">
            Essential-tier rows are counted everywhere above and excluded from
            the environment breakdowns below — their machine details were
            never collected, which is not the same as unknown.
          </p>
          {installs.length === 0 ? (
            <EmptyState
              className="py-6"
              title="No data yet"
              description="Arrives with install telemetry."
            />
          ) : (
            <DistributionList
              items={[
                { label: "Accepted all", value: fullInstalls.length },
                { label: "Declined optional", value: minimalInstalls.length },
              ]}
              total={installs.length}
            />
          )}
        </Panel>
      </div>
    </>
  );
}

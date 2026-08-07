import { ErrorPanel } from "@/components/error-panel";
import { Panel, PanelHeader } from "@/components/panel";
import { StatCard } from "@/components/stat-card";
import {
  DataTable,
  NullCell,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/data-table";
import { getEventsSince, getInstalls, getReleases } from "@/lib/data";
import { formatNumber } from "@/lib/format";
import { telemetryHealth, type SectionTime } from "./shared";

export async function DownloadsMeta() {
  const releasesRes = await getReleases();
  return releasesRes.ok
    ? `${formatNumber(releasesRes.data.length)} releases`
    : "github unavailable";
}

/**
 * The slowest band on the page — GitHub's releases endpoint is paginated and
 * costs roughly a second, several times what Supabase costs. It is last in
 * the fetch order and behind its own boundary so that second is spent while
 * everything above it is already on screen, rather than in front of it.
 */
export async function DownloadsSection({ since30d }: SectionTime) {
  const [releasesRes, installsRes, eventsRes] = await Promise.all([
    getReleases(),
    getInstalls(),
    getEventsSince(since30d),
  ]);

  if (!releasesRes.ok) {
    return (
      <ErrorPanel
        title="Couldn't load GitHub releases"
        message={releasesRes.error}
      />
    );
  }

  const releases = releasesRes.data;
  const events = eventsRes.ok ? eventsRes.data : [];
  const { installs, dash } = telemetryHealth(installsRes, events.length);

  // ── Downloads, reconciled against installs ───────────────────────────────
  // These two numbers get compared constantly and they are not the same unit,
  // so the page does the arithmetic instead of leaving it to be eyeballed.
  const assetTotal = (match: (name: string) => boolean) =>
    releases.reduce(
      (sum, r) =>
        sum +
        r.assets
          .filter((a) => match(a.name))
          .reduce((t, a) => t + a.downloads, 0),
      0
    );
  const totalDownloads = releases.reduce((s, r) => s + r.downloads, 0);
  const debDownloads = assetTotal((n) => n.endsWith(".deb"));
  const scriptDownloads = assetTotal((n) => n.endsWith(".sh"));
  const otherDownloads = totalDownloads - debDownloads - scriptDownloads;
  const latestRelease = releases.at(-1) ?? null;
  const latestDeb = latestRelease
    ? latestRelease.assets
        .filter((a) => a.name.endsWith(".deb"))
        .reduce((t, a) => t + a.downloads, 0)
    : 0;

  return (
    <>
      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        <StatCard
          label="Total downloads"
          value={formatNumber(totalDownloads)}
          hint={`every asset, all ${releases.length} releases`}
        />
        <StatCard
          label=".deb downloads"
          value={formatNumber(debDownloads)}
          hint="the package itself, all releases"
        />
        <StatCard
          label="install.sh fetches"
          value={formatNumber(scriptDownloads)}
          hint="the one-liner, which then fetches a .deb"
        />
        <StatCard
          label="Installs known"
          value={dash ? "—" : formatNumber(installs.length)}
          hint="machines that ever checked in"
        />
      </div>

      <Panel>
        <PanelHeader
          title="Why the numbers differ"
          meta={
            installs.length > 0 && debDownloads > 0
              ? `${(debDownloads / installs.length).toFixed(1)}× .deb per known install`
              : undefined
          }
        />
        <DataTable>
          <THead>
            <TR>
              <TH>Step</TH>
              <TH className="w-[110px] text-right">Count</TH>
              <TH>What it means</TH>
            </TR>
          </THead>
          <TBody>
            <TR>
              <TD className="text-sm text-stone-900">
                All asset downloads
              </TD>
              <TD className="text-right text-sm text-stone-900 tabular-nums">
                {formatNumber(totalDownloads)}
              </TD>
              <TD className="text-sm text-stone-500">
                What the GitHub API reports. Includes bots, mirrors and CI
                — GitHub counts every fetch and cannot tell them apart.
              </TD>
            </TR>
            <TR>
              <TD className="text-sm text-stone-900">
                less install.sh fetches
              </TD>
              <TD className="text-right text-sm text-stone-500 tabular-nums">
                −{formatNumber(scriptDownloads)}
              </TD>
              <TD className="text-sm text-stone-500">
                The one-liner fetches the script <em>and then</em> a{" "}
                <code className="font-mono text-[0.85em]">.deb</code>, so
                one install through it counts twice in the total above.
              </TD>
            </TR>
            {otherDownloads !== 0 ? (
              <TR>
                <TD className="text-sm text-stone-900">
                  less other assets
                </TD>
                <TD className="text-right text-sm text-stone-500 tabular-nums">
                  −{formatNumber(otherDownloads)}
                </TD>
                <TD className="text-sm text-stone-500">
                  Assets that are neither the package nor the installer.
                </TD>
              </TR>
            ) : null}
            <TR>
              <TD className="text-sm font-medium text-stone-900">
                .deb downloads
              </TD>
              <TD className="text-right text-sm font-medium text-stone-900 tabular-nums">
                {formatNumber(debDownloads)}
              </TD>
              <TD className="text-sm text-stone-500">
                Still not people: every existing user re-downloads on each
                release, so an upgrader counts once per version they took.
              </TD>
            </TR>
            <TR>
              <TD className="text-sm text-stone-900">
                latest release only
                {latestRelease ? (
                  <span className="ml-1.5 font-mono text-meta text-stone-400">
                    {latestRelease.tag}
                  </span>
                ) : null}
              </TD>
              <TD className="text-right text-sm text-stone-900 tabular-nums">
                {formatNumber(latestDeb)}
              </TD>
              <TD className="text-sm text-stone-500">
                The closest single figure to &ldquo;new installs since the
                last release&rdquo;, still inflated by upgraders.
              </TD>
            </TR>
            <TR>
              <TD className="text-sm font-medium text-stone-900">
                Installs known to telemetry
              </TD>
              <TD className="text-right text-sm font-medium text-stone-900 tabular-nums">
                {dash ? <NullCell /> : formatNumber(installs.length)}
              </TD>
              <TD className="text-sm text-stone-500">
                Distinct machines that answered the consent dialog and
                checked in at least once. Anyone who never answered it
                sends nothing and is invisible here — which is the floor
                under this number, not a bug.
              </TD>
            </TR>
          </TBody>
        </DataTable>
        <p className="mt-2 text-sm leading-snug text-stone-500">
          The honest reading: {formatNumber(debDownloads)} package
          downloads is an upper bound inflated by upgrades and bots,{" "}
          {dash ? "—" : formatNumber(installs.length)} is a lower bound
          deflated by everyone who declined to be counted. The real
          population sits between them, closer to the lower bound.
        </p>
      </Panel>
    </>
  );
}

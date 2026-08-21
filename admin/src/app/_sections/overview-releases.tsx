import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { DownloadsChart } from "@/components/downloads-chart";
import { ReleaseTable } from "@/components/release-table";
import { Panel, PanelHeader } from "@/components/panel";
import { getReleases } from "@/lib/data";
import { formatNumber } from "@/lib/format";
import { median, splitReleases } from "@/lib/releases";

/**
 * The slowest band on the page (~1.1s for the GitHub releases list), which is
 * exactly why it sits behind its own boundary — nothing else waits on it.
 *
 * It is also the only unbounded panel on the Overview, and it shares a grid
 * row with a column that tops out around 560px. Left to grow at 27px a
 * release it dictated the row height and pushed a screenful of scroll onto
 * the page while its neighbour sat in whitespace. Now the height is fixed:
 * the figures that were only derivable by reading every row are stated up
 * front, the chart carries the per-asset split it always had in the data,
 * and the rows scroll inside their own box.
 */
export async function ReleasesPanel() {
  const releasesRes = await getReleases();
  const releases = releasesRes.ok ? releasesRes.data : [];
  const split = splitReleases(releases);
  const latest = releases.at(-1) ?? null;
  const latestDownloads = latest?.downloads ?? 0;
  const pct = (n: number) =>
    split.total > 0 ? Math.round((n / split.total) * 100) : 0;

  return (
    <Panel className="section-in lg:col-span-2">
      <PanelHeader
        title="Downloads per release"
        meta={
          releasesRes.ok
            ? `${formatNumber(split.total)} across ${releases.length} release${releases.length === 1 ? "" : "s"}`
            : undefined
        }
      />
      {!releasesRes.ok ? (
        <ErrorPanel
          title="Couldn't load GitHub releases"
          message={releasesRes.error}
        />
      ) : releases.length === 0 ? (
        <EmptyState
          title="No releases yet"
          description="Published GitHub releases with assets will appear here."
        />
      ) : (
        <div className="space-y-3">
          {/* The four readings that previously required scanning the whole
              table: what was fetched, how the newest release is doing, and
              what a typical release looks like next to it. */}
          <dl className="grid grid-cols-2 gap-x-4 gap-y-2 sm:grid-cols-4">
            <Figure
              label=".deb package"
              value={formatNumber(split.deb)}
              hint={`${pct(split.deb)}% of fetches`}
            />
            <Figure
              label="install.sh"
              value={formatNumber(split.script)}
              hint={`${pct(split.script)}% of fetches`}
            />
            <Figure
              label={latest ? `Latest ${latest.tag}` : "Latest release"}
              value={formatNumber(latestDownloads)}
              hint={`${pct(latestDownloads)}% of all downloads`}
            />
            <Figure
              label="Median release"
              value={formatNumber(median(releases.map((r) => r.downloads)))}
              hint="half of releases sit below"
            />
          </dl>

          <DownloadsChart releases={releases} />
          <ReleaseTable releases={releases} />

          {/* Stated once, here, because every number above it is a file
              counter: the one-liner fetches install.sh and then the .deb it
              points at, so a single install can tick two of these. */}
          <p className="font-mono text-meta text-stone-400">
            Counts are files fetched, not users — a one-liner install ticks
            both install.sh and .deb.
          </p>
        </div>
      )}
    </Panel>
  );
}

/** One reading in the strip: mono instrument label, figure, footnote — the
 *  same three-grey hierarchy as StatCard, at panel-inset scale. */
function Figure({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint: string;
}) {
  return (
    <div className="min-w-0">
      <dt className="truncate font-mono text-meta tracking-wide text-stone-500 uppercase">
        {label}
      </dt>
      <dd className="mt-0.5 text-sm leading-none font-semibold text-stone-900 tabular-nums">
        {value}
      </dd>
      <dd className="mt-1 truncate font-mono text-meta text-stone-400">
        {hint}
      </dd>
    </div>
  );
}

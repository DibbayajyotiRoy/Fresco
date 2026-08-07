import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { DownloadsChart } from "@/components/downloads-chart";
import { ReleaseTable } from "@/components/release-table";
import { Panel, PanelHeader } from "@/components/panel";
import { getReleases } from "@/lib/data";
import { formatNumber } from "@/lib/format";

/**
 * The slowest band on the page (~1.1s for the GitHub releases list), which is
 * exactly why it sits behind its own boundary — nothing else waits on it.
 */
export async function ReleasesPanel() {
  const releasesRes = await getReleases();
  const releases = releasesRes.ok ? releasesRes.data : [];
  const totalDownloads = releases.reduce((s, r) => s + r.downloads, 0);

  return (
    <Panel className="section-in lg:col-span-2">
      <PanelHeader
        title="Downloads per release"
        meta={`${formatNumber(totalDownloads)} total`}
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
          <DownloadsChart releases={releases} />
          <ReleaseTable releases={releases} />
        </div>
      )}
    </Panel>
  );
}

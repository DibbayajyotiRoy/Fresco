import { EmptyState } from "@/components/empty-state";
import { Panel, PanelHeader } from "@/components/panel";
import {
  DistributionList,
  type DistributionItem,
} from "@/components/distribution-list";
import { getEventsSince, getInstalls } from "@/lib/data";
import { BREAKDOWN_HELP } from "@/lib/events";
import { formatNumber } from "@/lib/format";
import { cohorts, telemetryHealth, topDistribution, type SectionTime } from "./shared";

export async function EnvironmentMeta() {
  const installsRes = await getInstalls();
  const installs = installsRes.ok ? installsRes.data : [];
  const { full, detailShare } = cohorts(installs);
  return `${formatNumber(full.length)} of ${formatNumber(installs.length)} installs · ${detailShare === null ? "—" : `${detailShare}%`}`;
}

export async function EnvironmentSection({ since30d }: SectionTime) {
  // Events are fetched only to tell "nobody uses this" apart from "the
  // heartbeat write is failing" — see `telemetryHealth`. The empty states
  // below say which one it is.
  const [installsRes, eventsRes] = await Promise.all([
    getInstalls(),
    getEventsSince(since30d),
  ]);

  const events = eventsRes.ok ? eventsRes.data : [];
  const { installs, installsBroken } = telemetryHealth(
    installsRes,
    events.length
  );
  const { full: fullInstalls } = cohorts(installs);

  const distroDist = topDistribution(fullInstalls.map((i) => i.distro));
  const compositorDist = topDistribution(fullInstalls.map((i) => i.compositor));
  const sessionDist = topDistribution(fullInstalls.map((i) => i.session));
  const decodeDist = topDistribution(fullInstalls.map((i) => i.decode));
  const sourceDist = topDistribution(fullInstalls.map((i) => i.source));
  const channelDist = topDistribution(fullInstalls.map((i) => i.channel));

  const breakdowns: { title: string; items: DistributionItem[] }[] = [
    { title: "Distro", items: distroDist },
    { title: "Desktop", items: compositorDist },
    { title: "Session type", items: sessionDist },
    { title: "Video decode", items: decodeDist },
    { title: "Download source", items: sourceDist },
    { title: "Install channel", items: channelDist },
  ];

  return (
    <>
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
        {breakdowns.map((b) => (
          <Panel key={b.title}>
            <PanelHeader title={b.title} meta="by install" />
            <p className="mb-2 text-sm leading-snug text-stone-500">
              {BREAKDOWN_HELP[b.title]}
            </p>
            {b.items.length === 0 ? (
              <EmptyState
                className="py-6"
                title={installsBroken ? "Not being recorded" : "No data yet"}
                description={
                  installsBroken
                    ? "Blocked by the install telemetry failure above."
                    : "Arrives with install telemetry."
                }
              />
            ) : (
              <DistributionList items={b.items} total={installs.length} />
            )}
          </Panel>
        ))}
      </div>
      <p className="font-mono text-meta text-stone-400">
        Feature events, error reports and machine details come only from users
        who accepted the optional statistics. Identity and country come from
        everyone who answered the dialog either way.
      </p>
    </>
  );
}

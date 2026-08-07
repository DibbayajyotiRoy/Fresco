import { Notice } from "@/components/notice";
import { StatCard } from "@/components/stat-card";
import { getEventsSince, getInstalls } from "@/lib/data";
import { formatNumber } from "@/lib/format";
import { DAY_MS, telemetryHealth, type SectionTime } from "./shared";

/** `N known installs`, or nothing if the installs query failed. */
export async function ReachMeta() {
  const installsRes = await getInstalls();
  if (!installsRes.ok) return null;
  return `${formatNumber(installsRes.data.length)} known installs`;
}

export async function ReachSection({ now, since30d }: SectionTime) {
  const [installsRes, eventsRes] = await Promise.all([
    getInstalls(),
    getEventsSince(since30d),
  ]);

  const events = eventsRes.ok ? eventsRes.data : [];
  const { installs, installsBroken, dash } = telemetryHealth(
    installsRes,
    events.length
  );

  const activeIn = (days: number) => {
    const cutoff = now - days * DAY_MS;
    return installs.filter((i) => Date.parse(i.last_seen) >= cutoff).length;
  };
  const activeToday = activeIn(1);
  const active7d = activeIn(7);
  const active30d = activeIn(30);

  return (
    <>
      {installsBroken ? (
        <Notice
          label="install telemetry not recording"
          title={`${formatNumber(events.length)} events arrived from installs that were never registered.`}
        >
          <p>
            Every metric below that counts installs — active today/7d/30d, total
            installs, and all six environment breakdowns — reads zero for this
            reason, not because the app is unused. The client posts{" "}
            <code className="font-mono text-[0.85em]">source</code> and{" "}
            <code className="font-mono text-[0.85em]">channel</code> columns that
            the <code className="font-mono text-[0.85em]">installs</code> table
            does not have, so PostgREST rejects every heartbeat and the app logs
            it at debug level. Feature usage is unaffected and is real.
          </p>
        </Notice>
      ) : null}

      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        <StatCard
          label="Unique users total"
          value={dash ? "—" : formatNumber(installs.length)}
          hint={
            installsBroken
              ? "not recorded — see above"
              : "distinct installs ever seen"
          }
        />
        <StatCard
          label="Active 30d"
          value={dash ? "—" : formatNumber(active30d)}
          hint={
            !installsRes.ok
              ? installsRes.error
              : installsBroken
                ? "not recorded — see above"
                : "checked in this month"
          }
        />
        <StatCard
          label="Active 7d"
          value={dash ? "—" : formatNumber(active7d)}
          hint={installsBroken ? "not recorded — see above" : "checked in this week"}
        />
        <StatCard
          label="Active today"
          value={dash ? "—" : formatNumber(activeToday)}
          hint={installsBroken ? "not recorded — see above" : "checked in today"}
        />
      </div>
    </>
  );
}

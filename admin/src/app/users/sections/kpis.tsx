import { StatCard } from "@/components/stat-card";
import { getInstallDaysSince, getInstalls } from "@/lib/data";
import { lifecycleCounts, newInstallsPerDay, stickiness } from "@/lib/install-analytics";
import { formatNumber } from "@/lib/format";

const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * The KPI strip that answers the owner's first three questions in one
 * glance: how many users total (including the ones who left), how many are
 * active, and how many checked in today.
 *
 * "Checked in today" is worded deliberately, not "opened the app" or
 * "active today" — a heartbeat is the only signal this data has, sent at
 * daemon start and throttled to once per ~20h (see install-analytics.ts's
 * header comment). It is a real, honest lower bound on daily use.
 */
export async function UsersKpis() {
  const now = Date.now();
  const [installsRes, daysRes] = await Promise.all([
    getInstalls(),
    getInstallDaysSince(new Date(now - 30 * DAY_MS).toISOString().slice(0, 10)),
  ]);

  if (!installsRes.ok) {
    return (
      <div className="grid grid-cols-2 gap-2 lg:grid-cols-5">
        {["Total installs ever", "Active 7d", "Checked in today", "Lapsed >30d", "New 30d"].map(
          (label) => (
            <StatCard key={label} label={label} value="—" hint={installsRes.error} />
          )
        )}
      </div>
    );
  }

  const installs = installsRes.data;
  const counts = lifecycleCounts(installs, now);
  const daus = daysRes.ok ? stickiness(daysRes.data, now).dau : null;
  const newSeries = newInstallsPerDay(installs, now, 30);
  const new30d = newSeries.reduce((s, d) => s + d.count, 0);

  return (
    <div className="grid grid-cols-2 gap-2 lg:grid-cols-5">
      <StatCard
        label="Total installs ever"
        value={formatNumber(counts.total)}
        hint="every install row ever written"
      />
      <StatCard
        label="Active 7d"
        value={formatNumber(counts.active)}
        hint={
          counts.total > 0
            ? `${Math.round((counts.active / counts.total) * 100)}% of total`
            : undefined
        }
      />
      <StatCard
        label="Checked in today"
        value={daus === null ? "—" : formatNumber(daus)}
        hint={
          daysRes.ok
            ? "distinct installs, install_days"
            : "install_days not available"
        }
      />
      <StatCard
        label="Lapsed >30d"
        value={formatNumber(counts.lapsed)}
        hint={
          counts.lapseRatePct === null
            ? "likely uninstalled"
            : `${counts.lapseRatePct}% lapse rate · likely uninstalled`
        }
      />
      <StatCard
        label="New 30d"
        value={formatNumber(new30d)}
        hint="first-seen in the last 30 days"
      />
    </div>
  );
}

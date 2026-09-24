import { EmptyState } from "@/components/empty-state";
import { Panel, PanelHeader } from "@/components/panel";
import { getDailyCountrySince, getInstallDaysSince, getInstalls } from "@/lib/data";
import { dailyCheckins, newInstallsPerDay } from "@/lib/install-analytics";
import { formatDate, formatNumber } from "@/lib/format";

import { DauChartClient } from "./dau-chart-client";

const DAY_MS = 24 * 60 * 60 * 1000;
const WINDOW_DAYS = 90;

/**
 * Check-ins and new installs over the last 90 days, one bar chart each.
 *
 * Both series as dot-matrix bars, not a line: this admin's chart kit
 * (dither-kit) ships a `<Bar>` series and no line primitive, and adding one
 * for a single panel was not worth a new chart dependency — the grouping
 * instructions call for "no new runtime dependencies without strong need".
 * A daily bar reads the same information a line would (one value per day,
 * trend visible left to right); it just does not connect the dots.
 */
export async function DauChart() {
  const now = Date.now();
  const since = new Date(now - WINDOW_DAYS * DAY_MS).toISOString();
  const sinceDate = since.slice(0, 10);

  const [installsRes, daysRes, dailyCountryRes] = await Promise.all([
    getInstalls(),
    getInstallDaysSince(sinceDate),
    getDailyCountrySince(sinceDate),
  ]);

  if (!installsRes.ok) {
    return (
      <Panel>
        <PanelHeader title="Check-ins and new installs" />
        <EmptyState title="Couldn't load installs" description={installsRes.error} />
      </Panel>
    );
  }

  const checkins = dailyCheckins(
    daysRes.ok ? daysRes.data : [],
    dailyCountryRes.ok ? dailyCountryRes.data : [],
    now,
    WINDOW_DAYS
  );
  const newSeries = newInstallsPerDay(installsRes.data, now, WINDOW_DAYS);
  const newByDay = new Map(newSeries.map((d) => [d.day, d.count]));

  const data = checkins.map((c) => ({
    day: formatDate(c.day),
    checkins: c.count,
    new: newByDay.get(c.day) ?? 0,
  }));

  const usesFallback = checkins.some((c) => c.source === "daily_country");
  const totalCheckins = checkins.reduce((s, c) => s + c.count, 0);
  const totalNew = newSeries.reduce((s, d) => s + d.count, 0);

  return (
    <Panel>
      <PanelHeader
        title="Installs that checked in each day"
        meta={`${WINDOW_DAYS}d · ${formatNumber(totalCheckins)} check-ins`}
      />
      <p className="mb-2 max-w-3xl text-sm leading-snug text-stone-500">
        A check-in is one heartbeat, sent at most once per ~20h — this counts
        installs that were running that day, not every launch.
        {usesFallback
          ? " Grey-shaded days before install_days existed use the older daily_country ping tally instead (see the source column on the CSV export)."
          : null}
      </p>
      {data.every((d) => d.checkins === 0 && d.new === 0) ? (
        <EmptyState
          className="py-8"
          title="No check-ins recorded yet"
          description="Data arrives once install_days is populated by the app."
        />
      ) : (
        <DauChartClient data={data} totals={{ checkins: totalCheckins, new: totalNew }} />
      )}
    </Panel>
  );
}

import { buildCsv, csvFilename, csvResponse } from "@/lib/csv";
import { getDailyCountrySince, getInstallDaysSince, getInstalls } from "@/lib/data";
import { dailyCheckins, newInstallsPerDay } from "@/lib/install-analytics";

export const dynamic = "force-dynamic";
export const revalidate = 0;

const DEFAULT_DAYS = 90;
const MAX_DAYS = 365;

/**
 * GET /api/export/daily.csv?days=90
 *
 * One row per day: check-ins (DAU proxy, see install-analytics.ts), new
 * installs, and which table the check-in figure came from — the same
 * install_days / daily_country split the /users DAU chart shows, spelled out
 * per row instead of as a chart annotation, since a CSV reader cannot hover
 * a tooltip.
 *
 * Auth: see admin/README.md and the note in /api/export/installs — no
 * additional exposure beyond the page it mirrors.
 */
export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const daysParam = Number(searchParams.get("days"));
  const days = Number.isFinite(daysParam) && daysParam > 0
    ? Math.min(Math.floor(daysParam), MAX_DAYS)
    : DEFAULT_DAYS;

  const now = Date.now();
  const since = new Date(now - days * 24 * 60 * 60 * 1000).toISOString();
  const sinceDate = since.slice(0, 10);

  const [installsRes, installDaysRes, dailyCountryRes] = await Promise.all([
    getInstalls(),
    getInstallDaysSince(sinceDate),
    getDailyCountrySince(sinceDate),
  ]);

  if (!installsRes.ok) {
    return new Response(installsRes.error, { status: 502 });
  }

  const checkins = dailyCheckins(
    installDaysRes.ok ? installDaysRes.data : [],
    dailyCountryRes.ok ? dailyCountryRes.data : [],
    now,
    days
  );
  const newInstalls = newInstallsPerDay(installsRes.data, now, days);
  const newByDay = new Map(newInstalls.map((d) => [d.day, d.count]));

  const header = ["day", "check_ins", "new_installs", "check_ins_source"];
  const body = buildCsv(
    header,
    checkins.map((c) => [c.day, c.count, newByDay.get(c.day) ?? 0, c.source])
  );

  return csvResponse(body, csvFilename("daily", now));
}

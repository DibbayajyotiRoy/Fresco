import { buildCsv, csvFilename, csvResponse } from "@/lib/csv";
import { getInstallDaysSince, getInstalls } from "@/lib/data";
import { perCountryTable } from "@/lib/install-analytics";

export const dynamic = "force-dynamic";
export const revalidate = 0;

/**
 * GET /api/export/countries.csv
 *
 * The full per-country table shown on /countries — every country, not a
 * top-N cut, since a CSV is exactly the place to keep the long tail the UI
 * summarises away.
 *
 * Auth: see admin/README.md and the note in /api/export/installs — no
 * additional exposure beyond the page it mirrors.
 */
export async function GET() {
  const now = Date.now();
  // Full history, not just "since yesterday": `retained` needs to know
  // whether an install EVER checked in on a second distinct day, which can
  // be arbitrarily long ago — not just whether it did so recently.
  const [installsRes, daysRes] = await Promise.all([
    getInstalls(),
    getInstallDaysSince("1970-01-01"),
  ]);

  if (!installsRes.ok) {
    return new Response(installsRes.error, { status: 502 });
  }

  const rows = perCountryTable(installsRes.data, daysRes.ok ? daysRes.data : [], now);

  const header = [
    "country",
    "total",
    "retained",
    "retained_rate_pct",
    "active_7d",
    "checkins_today",
    "new_30d",
    "lapsed",
    "lapse_rate_pct",
    "top_version",
  ];

  const body = buildCsv(
    header,
    rows.map((r) => [
      r.country,
      r.total,
      r.retained,
      r.retainedRatePct,
      r.active7d,
      r.checkinsToday,
      r.new30d,
      r.lapsed,
      r.lapseRatePct,
      r.topVersion,
    ])
  );

  return csvResponse(body, csvFilename("countries", now));
}

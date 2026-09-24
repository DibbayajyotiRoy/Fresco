import { buildCsv, csvFilename, csvResponse } from "@/lib/csv";
import { getInstallDaysSince, getInstalls } from "@/lib/data";
import { lifecycleStatus } from "@/lib/install-analytics";
import { applyInstallsQuery, parseInstallsQuery } from "@/lib/installs-query";

export const dynamic = "force-dynamic";
export const revalidate = 0;

/**
 * GET /api/export/installs.csv?status=&country=&version=&channel=&q=
 *
 * Same filters as the /users installs table (parsed with the identical
 * `parseInstallsQuery`), so "export what I'm looking at" is literally true —
 * there is no second filter implementation to drift from the table's.
 * Unlike the table this ignores `page`/`pageSize`: an export is the whole
 * filtered set, capped by the same MAX_ROWS `getInstalls` already enforces.
 *
 * Auth: see admin/README.md — this dashboard has no login of its own, so
 * this route is exactly as exposed as every page and every other route
 * handler here (e.g. /api/support/unread). It reads through the same
 * service-role data layer and adds no new capability; whatever access-
 * controls the front door, controls this too.
 */
export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const query = parseInstallsQuery(Object.fromEntries(searchParams));
  const now = Date.now();

  const [installsRes, daysRes] = await Promise.all([
    getInstalls(),
    getInstallDaysSince(new Date(now - 30 * 24 * 60 * 60 * 1000).toISOString().slice(0, 10)),
  ]);

  if (!installsRes.ok) {
    return new Response(installsRes.error, { status: 502 });
  }

  const checkinCounts = daysRes.ok
    ? (() => {
        const m = new Map<string, number>();
        for (const d of daysRes.data) m.set(d.install_id, (m.get(d.install_id) ?? 0) + 1);
        return m;
      })()
    : null;

  // No pagination cap here beyond what getInstalls already enforces
  // (MAX_ROWS in data.ts) — an export wants every matching row, not a page.
  const { rows } = applyInstallsQuery(
    installsRes.data,
    { ...query, page: 1, pageSize: installsRes.data.length || 1 },
    now,
    checkinCounts
  );

  const header = [
    "install_id",
    "version",
    "distro",
    "compositor",
    "session",
    "backend",
    "decode",
    "monitor_count",
    "source",
    "channel",
    "country",
    "region",
    "city",
    "minimal",
    "first_seen",
    "last_seen",
    "status",
    "checkin_days_30d",
  ];

  const body = buildCsv(
    header,
    rows.map((r) => [
      r.install_id,
      r.version,
      r.distro,
      r.compositor,
      r.session,
      r.backend,
      r.decode,
      r.monitor_count,
      r.source,
      r.channel,
      r.country,
      r.region,
      r.city,
      r.minimal,
      r.first_seen,
      r.last_seen,
      lifecycleStatus(r.last_seen, now),
      r.checkinDays30d,
    ])
  );

  return csvResponse(body, csvFilename("installs", now));
}

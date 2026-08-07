import { getEventsSince, getInstalls } from "@/lib/data";
import { formatNumber } from "@/lib/format";
import type { SectionTime } from "./shared";

/**
 * The page title's meta line.
 *
 * Passed into `PageHeader`'s `action` slot rather than its `meta` prop, which
 * is a string and would hold the whole title back for two round-trips. The
 * markup below is `PageHeader`'s own meta span, so it lands in the same place
 * looking the same.
 */
export async function UsageHeaderMeta({ since30d }: Pick<SectionTime, "since30d">) {
  const [installsRes, eventsRes] = await Promise.all([
    getInstalls(),
    getEventsSince(since30d),
  ]);

  if (!installsRes.ok) return null;

  const installs = installsRes.data;
  const events = eventsRes.ok ? eventsRes.data : [];

  return (
    <span className="font-mono text-meta tracking-wide text-stone-400 uppercase tabular-nums">
      {`${formatNumber(installs.length)} installs · ${formatNumber(events.length)} events / 30d`}
    </span>
  );
}

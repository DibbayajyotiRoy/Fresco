import type { DistributionItem } from "@/components/distribution-list";
import type { DataResult } from "@/lib/data";
import type { Install } from "@/lib/types";

export const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * One clock for the whole page.
 *
 * Each band fetches its own data — the data layer dedupes identical calls
 * within a render — but "identical" is by argument, so every section has to
 * ask for the *same* 30-day window. Computing `Date.now()` per section would
 * produce a slightly different ISO cutoff in each one and turn a single query
 * back into five. The page computes these once and hands them down; it is a
 * parameter, not fetched data.
 */
export type SectionTime = {
  /** Milliseconds since epoch, taken once when the page shell rendered. */
  now: number;
  /** ISO timestamp of `now` minus 30 days — the event window for the page. */
  since30d: string;
};

/** Bucket freeform values into a top-N breakdown with an "Other" rollup. */
export function topDistribution(
  values: (string | null)[],
  n = 6
): DistributionItem[] {
  const counts = new Map<string, number>();
  for (const v of values) {
    const key = v?.trim() || "Unknown";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const sorted = [...counts.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value);

  if (sorted.length <= n) return sorted;
  const head = sorted.slice(0, n);
  const other = sorted.slice(n).reduce((s, i) => s + i.value, 0);
  return [...head, { label: "Other", value: other }];
}

/**
 * Cohorts (consent revision 2).
 *
 * Both tiers now write a real install row keyed by a random install id, so
 * every window de-duplicates properly and "unique users" is a plain distinct
 * count — no extrapolation, and no same-day-only caveat.
 *
 * The difference is depth, not countability: `minimal` rows carry identity,
 * country, version and packaging, and nothing describing the machine. The
 * environment breakdowns therefore run over full-consent rows only, because
 * counting a minimal row as "unknown distro" would misreport not-collected as
 * unknown and silently skew every percentage.
 */
export function cohorts(installs: Install[]): {
  full: Install[];
  minimal: Install[];
  detailShare: number | null;
} {
  const full = installs.filter((i) => !i.minimal);
  const minimal = installs.filter((i) => i.minimal);
  const detailShare =
    installs.length > 0
      ? Math.round((full.length / installs.length) * 100)
      : null;
  return { full, minimal, detailShare };
}

/**
 * Events can only be sent by an install, so events with zero recorded
 * installs is not a real zero — it means the heartbeat write is failing
 * while the event write succeeds. Surfaced rather than left to be misread
 * as "nobody is using the app".
 *
 * Three bands need this verdict (Reach states it, Downloads and Environment
 * dash out the figures it invalidates), so it lives here rather than being
 * re-derived — and re-worded — in each of them.
 */
export function telemetryHealth(
  installsRes: DataResult<Install[]>,
  eventCount: number
): { installs: Install[]; installsBroken: boolean; dash: boolean } {
  const installs = installsRes.ok ? installsRes.data : [];
  const installsBroken = installsRes.ok && installs.length === 0 && eventCount > 0;
  return { installs, installsBroken, dash: !installsRes.ok || installsBroken };
}

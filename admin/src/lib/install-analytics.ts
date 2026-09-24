/**
 * Pure aggregation over installs / install_days / daily_country.
 *
 * Deliberately free of Supabase, `server-only` and React: every function here
 * takes plain arrays and a clock, and returns plain data. That is what makes
 * them unit-testable with `node:test` (see install-analytics.test.ts) without
 * a database, a Next.js runtime, or fixtures wired through React Server
 * Components — and it is what `data.ts` calls after paginating the real
 * fetch, or what the fixtures module calls to render the same page from
 * synthetic data.
 *
 * Vocabulary, once, so it does not drift between functions:
 *   - "checked in" / "check-in" — a heartbeat landed (at most one every ~20h,
 *     throttled client-side; see supabase/schema.sql). Never "opened the app"
 *     or "was active" in the UI sense — that is not what this data can prove.
 *   - "install" — one row in `public.installs`, keyed by a random client id.
 *     Not a "user": one person can run more than one install (a workstation
 *     and a laptop), and one install can outlive a reinstall's new id.
 */

import type { DailyCountry, Install, InstallDay } from "@/lib/types";

const DAY_MS = 24 * 60 * 60 * 1000;

/** Lifecycle thresholds, in days since `last_seen`. Named so the boundary is
 *  visible at every call site instead of a bare "7" or "30". */
export const ACTIVE_WITHIN_DAYS = 7;
export const IDLE_WITHIN_DAYS = 30;

export type LifecycleStatus = "active" | "idle" | "lapsed";

/**
 * Active: checked in within the last `ACTIVE_WITHIN_DAYS` days.
 * Idle: checked in more recently than `IDLE_WITHIN_DAYS` days ago but not
 *   within `ACTIVE_WITHIN_DAYS`.
 * Lapsed: silent for more than `IDLE_WITHIN_DAYS` days — the closest this
 *   data can come to "uninstalled", inferred from heartbeat silence rather
 *   than observed directly (there is no uninstall event).
 */
export function lifecycleStatus(
  lastSeenIso: string,
  nowMs: number
): LifecycleStatus {
  const ageDays = (nowMs - Date.parse(lastSeenIso)) / DAY_MS;
  if (ageDays <= ACTIVE_WITHIN_DAYS) return "active";
  if (ageDays <= IDLE_WITHIN_DAYS) return "idle";
  return "lapsed";
}

export type LifecycleCounts = {
  total: number;
  active: number;
  idle: number;
  lapsed: number;
  /** Lapsed / total, 0–100, rounded. Null when there are no installs at all. */
  lapseRatePct: number | null;
};

/** Total ever, and the three-way lifecycle split, as of `nowMs`. This is the
 *  answer to "how many users, including the ones who stopped" — `total` is
 *  every install row ever written, `lapsed` is the best inference of who left. */
export function lifecycleCounts(
  installs: Pick<Install, "last_seen">[],
  nowMs: number
): LifecycleCounts {
  let active = 0;
  let idle = 0;
  let lapsed = 0;
  for (const i of installs) {
    const status = lifecycleStatus(i.last_seen, nowMs);
    if (status === "active") active++;
    else if (status === "idle") idle++;
    else lapsed++;
  }
  const total = installs.length;
  return {
    total,
    active,
    idle,
    lapsed,
    lapseRatePct: total > 0 ? Math.round((lapsed / total) * 100) : null,
  };
}

/** "YYYY-MM-DD" in UTC, the shared day key for every function below —
 *  `install_days.day` and `daily_country.day` are both plain dates with no
 *  timezone, so bucketing must not depend on the server's local time. */
export function dayKey(isoOrMs: string | number): string {
  const d = typeof isoOrMs === "number" ? new Date(isoOrMs) : new Date(isoOrMs);
  return d.toISOString().slice(0, 10);
}

function addDaysKey(key: string, days: number): string {
  const d = new Date(`${key}T00:00:00.000Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

/** Every day key from `daysBack - 1` days ago through today, oldest first. */
export function dayRange(nowMs: number, daysBack: number): string[] {
  const todayKey = dayKey(nowMs);
  const out: string[] = [];
  for (let i = daysBack - 1; i >= 0; i--) out.push(addDaysKey(todayKey, -i));
  return out;
}

export type DailyCount = {
  day: string;
  count: number;
  /** Which table this day's number came from — install_days is a real
   *  distinct-install count; daily_country is a ping tally from before that
   *  table existed, kept honest by labelling it rather than blending it in
   *  silently. */
  source: "install_days" | "daily_country";
};

/**
 * Daily check-ins (a proxy for DAU) over `dayRange(nowMs, daysBack)`.
 *
 * Counts distinct `install_id` per day from `install_days`. For any day
 * strictly before the earliest day `install_days` has ANY row for (i.e.
 * before this table existed), falls back to summing `daily_country.pings`
 * for that day — the old identifier-free tally — and labels those days
 * `"daily_country"` so a chart or table can say so rather than implying a
 * seamless history that was never collected.
 */
export function dailyCheckins(
  installDays: InstallDay[],
  dailyCountry: DailyCountry[],
  nowMs: number,
  daysBack: number
): DailyCount[] {
  const days = dayRange(nowMs, daysBack);

  const byDay = new Map<string, Set<string>>();
  let earliestInstallDayKey: string | null = null;
  for (const row of installDays) {
    const key = row.day.slice(0, 10);
    if (earliestInstallDayKey === null || key < earliestInstallDayKey) {
      earliestInstallDayKey = key;
    }
    const set = byDay.get(key) ?? new Set<string>();
    set.add(row.install_id);
    byDay.set(key, set);
  }

  const pingsByDay = new Map<string, number>();
  for (const row of dailyCountry) {
    const key = row.day.slice(0, 10);
    pingsByDay.set(key, (pingsByDay.get(key) ?? 0) + row.pings);
  }

  return days.map((day) => {
    const hasInstallDayHistory =
      earliestInstallDayKey !== null && day >= earliestInstallDayKey;
    if (hasInstallDayHistory) {
      return { day, count: byDay.get(day)?.size ?? 0, source: "install_days" };
    }
    return { day, count: pingsByDay.get(day) ?? 0, source: "daily_country" };
  });
}

/** New installs per day, from `first_seen`, over `dayRange(nowMs, daysBack)`. */
export function newInstallsPerDay(
  installs: Pick<Install, "first_seen">[],
  nowMs: number,
  daysBack: number
): { day: string; count: number }[] {
  const days = dayRange(nowMs, daysBack);
  const byDay = new Map<string, number>();
  for (const i of installs) {
    const key = dayKey(i.first_seen);
    byDay.set(key, (byDay.get(key) ?? 0) + 1);
  }
  return days.map((day) => ({ day, count: byDay.get(day) ?? 0 }));
}

export type Stickiness = {
  /** Distinct installs that checked in on the most recent day covered by
   *  `installDays` (or `nowMs`'s day if there is no data for it — a real
   *  zero, not a gap). */
  dau: number;
  /** Distinct installs across the trailing 7 days ending on that day. */
  wau: number;
  /** Distinct installs across the trailing 30 days ending on that day. */
  mau: number;
  /** DAU / MAU as a percentage, 0–100. Null when MAU is 0. */
  stickinessPct: number | null;
};

/**
 * DAU/WAU/MAU and stickiness, computed from `install_days` alone (the
 * `daily_country` fallback is a same-day-only tally with no identifier, so it
 * cannot feed a distinct weekly or monthly count — mixing it in here would
 * silently overcount).
 */
export function stickiness(installDays: InstallDay[], nowMs: number): Stickiness {
  const today = dayKey(nowMs);
  const since7 = addDaysKey(today, -6);
  const since30 = addDaysKey(today, -29);

  const dauSet = new Set<string>();
  const wauSet = new Set<string>();
  const mauSet = new Set<string>();
  for (const row of installDays) {
    const key = row.day.slice(0, 10);
    if (key === today) dauSet.add(row.install_id);
    if (key >= since7 && key <= today) wauSet.add(row.install_id);
    if (key >= since30 && key <= today) mauSet.add(row.install_id);
  }

  return {
    dau: dauSet.size,
    wau: wauSet.size,
    mau: mauSet.size,
    stickinessPct:
      mauSet.size > 0 ? Math.round((dauSet.size / mauSet.size) * 100) : null,
  };
}

export type RetentionCohort = {
  /** Monday of the cohort week, "YYYY-MM-DD". */
  weekStart: string;
  /** Installs first seen in this week. */
  size: number;
  /** Fraction (0–1) that checked in on day 1/7/30 after `weekStart`, or null
   *  when that horizon has not been reached yet (the day itself is in the
   *  future relative to `nowMs`). */
  d1: number | null;
  d7: number | null;
  d30: number | null;
};

function mondayOf(key: string): string {
  const d = new Date(`${key}T00:00:00.000Z`);
  const dow = d.getUTCDay(); // 0=Sun..6=Sat
  const back = dow === 0 ? 6 : dow - 1;
  d.setUTCDate(d.getUTCDate() - back);
  return d.toISOString().slice(0, 10);
}

/**
 * D1/D7/D30 retention by first-seen week, computed from `install_days`.
 *
 * A cohort's D-N figure needs a check-in exactly N days after each install's
 * OWN first-seen day (not the week start) among that install's own rows —
 * "checked in again a day/week/month later", not "checked in at all during
 * that later week". Horizons that have not been reached yet for a given
 * install are excluded from that install's contribution rather than counted
 * as a miss (a currently-2-day-old install cannot fail D30 — it has not had
 * the chance to pass or fail it).
 *
 * Returns `historySince`, the earliest day covered by `installDays`: when a
 * cohort's D30 window would need data from before that date, the caller
 * should render "collecting since <historySince>" instead of a number, since
 * `install_days` (unlike `installs.first_seen`/`last_seen`) has no backfilled
 * middle — only its two seeded rows per pre-existing install (see the
 * migration), so anything beyond day 0/last-seen is genuinely not there yet
 * for old installs.
 */
export function retentionCohorts(
  installDays: InstallDay[],
  installs: Pick<Install, "install_id" | "first_seen">[],
  nowMs: number
): { cohorts: RetentionCohort[]; historySince: string | null } {
  const daysByInstall = new Map<string, Set<string>>();
  let historySince: string | null = null;
  for (const row of installDays) {
    const key = row.day.slice(0, 10);
    if (historySince === null || key < historySince) historySince = key;
    const set = daysByInstall.get(row.install_id) ?? new Set<string>();
    set.add(key);
    daysByInstall.set(row.install_id, set);
  }

  const todayKey = dayKey(nowMs);
  const byWeek = new Map<string, { install_id: string; first_seen: string }[]>();
  for (const i of installs) {
    const week = mondayOf(dayKey(i.first_seen));
    const list = byWeek.get(week) ?? [];
    list.push({ install_id: i.install_id, first_seen: i.first_seen });
    byWeek.set(week, list);
  }

  function horizonRate(
    members: { install_id: string; first_seen: string }[],
    offset: number
  ): number | null {
    let reached = 0;
    let retained = 0;
    for (const m of members) {
      const startKey = dayKey(m.first_seen);
      const targetKey = addDaysKey(startKey, offset);
      if (targetKey > todayKey) continue; // horizon not reached yet
      reached++;
      const days = daysByInstall.get(m.install_id);
      if (days?.has(targetKey)) retained++;
    }
    return reached > 0 ? retained / reached : null;
  }

  const cohorts: RetentionCohort[] = [...byWeek.entries()]
    .map(([weekStart, members]) => ({
      weekStart,
      size: members.length,
      d1: horizonRate(members, 1),
      d7: horizonRate(members, 7),
      d30: horizonRate(members, 30),
    }))
    .sort((a, b) => (a.weekStart < b.weekStart ? 1 : -1));

  return { cohorts, historySince };
}

/**
 * Whether one install counts as "retained": it came back and checked in on a
 * SECOND distinct calendar day, not just the day it was created. That is the
 * line between a real user and a one-off try, a VM spun up once for testing,
 * or CI — all of which write exactly one install row and never again.
 *
 * Uses `install_days` when it has any coverage for this install id (the
 * precise answer: 2+ distinct days recorded). Falls back to comparing
 * `first_seen`/`last_seen` dates only for installs `install_days` has no
 * rows for at all — i.e. ones that predate the migration and have not
 * checked in since — since that is the only signal available for them.
 */
export function isRetained(
  install: Pick<Install, "install_id" | "first_seen" | "last_seen">,
  daysByInstall: Map<string, Set<string>>
): boolean {
  const days = daysByInstall.get(install.install_id);
  if (days && days.size > 0) return days.size >= 2;
  return dayKey(install.last_seen) > dayKey(install.first_seen);
}

function daysByInstallMap(installDays: InstallDay[]): Map<string, Set<string>> {
  const map = new Map<string, Set<string>>();
  for (const row of installDays) {
    const set = map.get(row.install_id) ?? new Set<string>();
    set.add(row.day.slice(0, 10));
    map.set(row.install_id, set);
  }
  return map;
}

export type ActivationFunnel = {
  /** Every install row ever written — distinct install ids, full stop. */
  installsEver: number;
  /** Installs with at least one recorded check-in. In this data model that
   *  is every install row (the row is itself written BY a heartbeat, so its
   *  mere existence proves one check-in) — kept as its own stage anyway
   *  because it is the natural place a future signal (e.g. an id generated
   *  client-side before its first successful heartbeat) would attach
   *  without reshaping the funnel. */
  activated: number;
  /** Checked in on a second distinct day — see `isRetained`. The headline
   *  "real users" number: it is what is left after one-off tries, VMs/CI
   *  and throwaway installs (all created-once, never-again install rows)
   *  are filtered out. */
  retained: number;
  active7d: number;
  active30d: number;
};

/** The installs -> activated -> retained -> active funnel. See
 *  `ActivationFunnel` for what each stage means and why. */
export function activationFunnel(
  installs: Install[],
  installDays: InstallDay[],
  nowMs: number
): ActivationFunnel {
  const daysByInstall = daysByInstallMap(installDays);
  let retained = 0;
  let active7d = 0;
  let active30d = 0;
  for (const i of installs) {
    if (isRetained(i, daysByInstall)) retained++;
    const ageDays = (nowMs - Date.parse(i.last_seen)) / DAY_MS;
    if (ageDays <= 7) active7d++;
    if (ageDays <= 30) active30d++;
  }
  return {
    installsEver: installs.length,
    activated: installs.length,
    retained,
    active7d,
    active30d,
  };
}

export type CountryRow = {
  country: string;
  total: number;
  /** Checked in on a second distinct day — see `isRetained`. Lets a country
   *  full of one-off tries be told apart from one full of people who stayed,
   *  even when both have the same `total`. */
  retained: number;
  retainedRatePct: number | null;
  active7d: number;
  checkinsToday: number;
  new30d: number;
  lapsed: number;
  lapseRatePct: number | null;
  topVersion: string | null;
};

/**
 * One row per country (raw code — display formatting, e.g. via
 * `countryLabel`, is the caller's job so this stays UI-free): install
 * counts, activity, and the most common version among that country's active
 * installs.
 */
export function perCountryTable(
  installs: Install[],
  installDays: InstallDay[],
  nowMs: number
): CountryRow[] {
  const today = dayKey(nowMs);
  const checkedInToday = new Set(
    installDays.filter((d) => d.day.slice(0, 10) === today).map((d) => d.install_id)
  );
  const daysByInstall = daysByInstallMap(installDays);

  const byCountry = new Map<string, Install[]>();
  for (const i of installs) {
    const key = i.country ?? "??";
    const list = byCountry.get(key) ?? [];
    list.push(i);
    byCountry.set(key, list);
  }

  const rows: CountryRow[] = [];
  for (const [country, list] of byCountry) {
    let active7d = 0;
    let lapsed = 0;
    let new30d = 0;
    let checkinsToday = 0;
    let retained = 0;
    const versionCounts = new Map<string, number>();
    for (const i of list) {
      const status = lifecycleStatus(i.last_seen, nowMs);
      if (status === "active") active7d++;
      if (status === "lapsed") lapsed++;
      if ((nowMs - Date.parse(i.first_seen)) / DAY_MS <= 30) new30d++;
      if (checkedInToday.has(i.install_id)) checkinsToday++;
      if (isRetained(i, daysByInstall)) retained++;
      if (status === "active" && i.version) {
        versionCounts.set(i.version, (versionCounts.get(i.version) ?? 0) + 1);
      }
    }
    let topVersion: string | null = null;
    let topCount = 0;
    for (const [v, c] of versionCounts) {
      if (c > topCount) {
        topVersion = v;
        topCount = c;
      }
    }
    rows.push({
      country,
      total: list.length,
      retained,
      retainedRatePct: list.length > 0 ? Math.round((retained / list.length) * 100) : null,
      active7d,
      checkinsToday,
      new30d,
      lapsed,
      lapseRatePct: list.length > 0 ? Math.round((lapsed / list.length) * 100) : null,
      topVersion,
    });
  }

  return rows.sort((a, b) => b.total - a.total);
}

/** Bucket freeform, possibly-null string values into counts. Unlike
 *  `topDistribution` in usage/sections/shared.ts (which rolls tails into
 *  "Other"), this keeps every distinct value — callers that want a top-N cut
 *  can slice the sorted result themselves. */
export function valueCounts(
  values: (string | null | undefined)[]
): { label: string; value: number }[] {
  const counts = new Map<string, number>();
  for (const v of values) {
    const key = v?.trim() || "Unknown";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value);
}

/** Version adoption among currently-active installs only — the question a
 *  maintainer deciding "can I drop support for an old version" actually
 *  asks, as opposed to every version anyone has ever run. */
export function versionAdoption(
  installs: Install[],
  nowMs: number
): { label: string; value: number }[] {
  const active = installs.filter(
    (i) => lifecycleStatus(i.last_seen, nowMs) === "active"
  );
  return valueCounts(active.map((i) => i.version));
}

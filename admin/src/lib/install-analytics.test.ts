/**
 * Unit tests for the pure aggregation functions in install-analytics.ts.
 *
 * Runs on Node's built-in test runner with its built-in TypeScript stripping
 * (Node 22.6+, no `--experimental-strip-types` flag needed on 22.18+; passed
 * explicitly by the `test` script for older 22.x — see package.json). No
 * devDependency added: `tsx`/`ts-node`/a test framework would be one more
 * thing to keep patched for what a handful of pure functions need, and this
 * repo's Node version already does the stripping for free.
 *
 * Run: `npm test` (from admin/).
 */

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ACTIVE_WITHIN_DAYS,
  IDLE_WITHIN_DAYS,
  activationFunnel,
  dailyCheckins,
  dayKey,
  dayRange,
  isRetained,
  lifecycleCounts,
  lifecycleStatus,
  newInstallsPerDay,
  perCountryTable,
  retentionCohorts,
  stickiness,
  valueCounts,
  versionAdoption,
} from "./install-analytics.ts";
import type { DailyCountry, Install, InstallDay } from "./types.ts";

const NOW = Date.parse("2026-09-24T12:00:00.000Z");
const DAY_MS = 24 * 60 * 60 * 1000;
const isoDaysAgo = (days: number) => new Date(NOW - days * DAY_MS).toISOString();

function install(overrides: Partial<Install> = {}): Install {
  return {
    install_id: "id-1",
    version: "1.1.40",
    distro: "ubuntu",
    compositor: "gnome-shell",
    session: "wayland",
    backend: "vulkan",
    decode: "hw",
    source: null,
    channel: "deb",
    country: "US",
    minimal: false,
    city: null,
    region: null,
    monitor_count: 1,
    first_seen: isoDaysAgo(10),
    last_seen: isoDaysAgo(1),
    ...overrides,
  };
}

test("lifecycleStatus: boundaries are inclusive on the active/idle side", () => {
  assert.equal(lifecycleStatus(isoDaysAgo(0), NOW), "active");
  assert.equal(lifecycleStatus(isoDaysAgo(ACTIVE_WITHIN_DAYS), NOW), "active");
  assert.equal(lifecycleStatus(isoDaysAgo(ACTIVE_WITHIN_DAYS + 0.5), NOW), "idle");
  assert.equal(lifecycleStatus(isoDaysAgo(IDLE_WITHIN_DAYS), NOW), "idle");
  assert.equal(lifecycleStatus(isoDaysAgo(IDLE_WITHIN_DAYS + 0.5), NOW), "lapsed");
  assert.equal(lifecycleStatus(isoDaysAgo(400), NOW), "lapsed");
});

test("lifecycleCounts: totals, split and lapse rate", () => {
  const installs = [
    install({ last_seen: isoDaysAgo(1) }), // active
    install({ last_seen: isoDaysAgo(5) }), // active
    install({ last_seen: isoDaysAgo(20) }), // idle
    install({ last_seen: isoDaysAgo(90) }), // lapsed
  ];
  const counts = lifecycleCounts(installs, NOW);
  assert.deepEqual(counts, {
    total: 4,
    active: 2,
    idle: 1,
    lapsed: 1,
    lapseRatePct: 25,
  });
});

test("lifecycleCounts: empty input has no NaN, no divide-by-zero", () => {
  const counts = lifecycleCounts([], NOW);
  assert.equal(counts.total, 0);
  assert.equal(counts.lapseRatePct, null);
});

test("dayRange: oldest to newest, correct length, ends on today", () => {
  const days = dayRange(NOW, 5);
  assert.equal(days.length, 5);
  assert.equal(days[days.length - 1], dayKey(NOW));
  // strictly ascending
  for (let i = 1; i < days.length; i++) assert.ok(days[i] > days[i - 1]);
});

test("newInstallsPerDay: buckets by first_seen, zero-fills days with no installs", () => {
  const installs = [
    install({ first_seen: isoDaysAgo(0) }),
    install({ first_seen: isoDaysAgo(0) }),
    install({ first_seen: isoDaysAgo(2) }),
  ];
  const series = newInstallsPerDay(installs, NOW, 4);
  const byDay = new Map(series.map((d) => [d.day, d.count]));
  assert.equal(byDay.get(dayKey(NOW)), 2);
  assert.equal(byDay.get(dayKey(NOW - 2 * DAY_MS)), 1);
  assert.equal(byDay.get(dayKey(NOW - 1 * DAY_MS)), 0);
  assert.equal(series.length, 4);
});

test("dailyCheckins: uses install_days when it covers the day, daily_country before that, and labels the source", () => {
  // install_days history starts 4 days ago; day -2 has no rows (a real zero
  // within that history), day -5 is before history began and must fall back.
  const installDays: InstallDay[] = [
    { install_id: "a", day: dayKey(NOW) },
    { install_id: "b", day: dayKey(NOW) },
    { install_id: "a", day: dayKey(NOW - 4 * DAY_MS) },
  ];
  const dailyCountry: DailyCountry[] = [
    {
      day: dayKey(NOW - 5 * DAY_MS),
      country: "US",
      version: "1.1.30",
      channel: "deb",
      pings: 7,
    },
  ];
  const series = dailyCheckins(installDays, dailyCountry, NOW, 6);
  const today = series.find((d) => d.day === dayKey(NOW))!;
  assert.equal(today.count, 2);
  assert.equal(today.source, "install_days");

  const beforeHistory = series.find((d) => d.day === dayKey(NOW - 5 * DAY_MS))!;
  assert.equal(beforeHistory.count, 7);
  assert.equal(beforeHistory.source, "daily_country");

  // A day with install_days coverage but zero rows is still a real zero from
  // that source, not a silent fallback to daily_country.
  const gapDay = series.find((d) => d.day === dayKey(NOW - 2 * DAY_MS))!;
  assert.equal(gapDay.count, 0);
  assert.equal(gapDay.source, "install_days");
});

test("stickiness: DAU/WAU/MAU are distinct-install counts, stickiness is DAU/MAU", () => {
  const installDays: InstallDay[] = [
    { install_id: "a", day: dayKey(NOW) },
    { install_id: "a", day: dayKey(NOW) }, // duplicate row, must not double count
    { install_id: "b", day: dayKey(NOW - 3 * DAY_MS) },
    { install_id: "c", day: dayKey(NOW - 20 * DAY_MS) },
    { install_id: "d", day: dayKey(NOW - 40 * DAY_MS) }, // outside 30d window
  ];
  const s = stickiness(installDays, NOW);
  assert.equal(s.dau, 1);
  assert.equal(s.wau, 2);
  assert.equal(s.mau, 3);
  assert.equal(s.stickinessPct, Math.round((1 / 3) * 100));
});

test("stickiness: zero MAU does not divide by zero", () => {
  const s = stickiness([], NOW);
  assert.equal(s.stickinessPct, null);
});

test("retentionCohorts: D1 counted only for installs whose horizon has passed", () => {
  // Cohort A: first seen 10 days ago, checked in again 1 day later -> retained.
  // Cohort A member 2: first seen 10 days ago, never checked in again -> not retained.
  const firstSeen = isoDaysAgo(10);
  const installs = [
    { install_id: "a1", first_seen: firstSeen },
    { install_id: "a2", first_seen: firstSeen },
    // Member whose D1 horizon has not arrived yet (first seen today).
    { install_id: "a3", first_seen: isoDaysAgo(0) },
  ];
  const startKey = dayKey(firstSeen);
  const d = new Date(`${startKey}T00:00:00.000Z`);
  d.setUTCDate(d.getUTCDate() + 1);
  const d1Key = d.toISOString().slice(0, 10);

  const installDays: InstallDay[] = [
    { install_id: "a1", day: startKey },
    { install_id: "a1", day: d1Key },
    { install_id: "a2", day: startKey },
  ];

  const { cohorts } = retentionCohorts(installDays, installs, NOW);
  // a3's week may be a different week than a1/a2's — find the cohort with 2
  // reached-horizon members (a1, a2); a3 alone has not reached D1 yet.
  const withTwo = cohorts.find((c) => c.size >= 2);
  assert.ok(withTwo, "expected a cohort containing a1 and a2");
  assert.equal(withTwo!.d1, 0.5);
});

test("valueCounts: null/blank collapse to Unknown, sorted descending", () => {
  const counts = valueCounts(["deb", "deb", null, "", "flatpak"]);
  assert.deepEqual(counts[0], { label: "deb", value: 2 });
  const unknown = counts.find((c) => c.label === "Unknown");
  assert.equal(unknown?.value, 2);
});

test("versionAdoption: only counts currently-active installs", () => {
  const installs = [
    install({ version: "1.1.40", last_seen: isoDaysAgo(1) }), // active
    install({ version: "1.1.30", last_seen: isoDaysAgo(90) }), // lapsed, excluded
  ];
  const adoption = versionAdoption(installs, NOW);
  assert.deepEqual(adoption, [{ label: "1.1.40", value: 1 }]);
});

test("isRetained: a second distinct check-in day retains, one day does not", () => {
  const oneOff = install({ install_id: "one-off", first_seen: isoDaysAgo(5), last_seen: isoDaysAgo(5) });
  const daysWithOneDay = new Map([["one-off", new Set([dayKey(isoDaysAgo(5))])]]);
  assert.equal(isRetained(oneOff, daysWithOneDay), false);

  const cameBack = install({ install_id: "came-back", first_seen: isoDaysAgo(5), last_seen: isoDaysAgo(1) });
  const daysWithTwo = new Map([
    ["came-back", new Set([dayKey(isoDaysAgo(5)), dayKey(isoDaysAgo(1))])],
  ]);
  assert.equal(isRetained(cameBack, daysWithTwo), true);
});

test("isRetained: falls back to first_seen/last_seen date comparison when install_days has no rows for this install", () => {
  const noHistory = install({
    install_id: "pre-migration",
    first_seen: isoDaysAgo(40),
    last_seen: isoDaysAgo(35), // different calendar day -> came back at least once
  });
  assert.equal(isRetained(noHistory, new Map()), true);

  const singleDay = install({
    install_id: "pre-migration-2",
    first_seen: isoDaysAgo(40),
    last_seen: isoDaysAgo(40), // same day, no fallback evidence of a return
  });
  assert.equal(isRetained(singleDay, new Map()), false);
});

test("activationFunnel: installsEver >= activated >= retained >= active7d, and stages are computed correctly", () => {
  const installs = [
    // one-off: single install_days row, never returns
    install({ install_id: "a", first_seen: isoDaysAgo(20), last_seen: isoDaysAgo(20) }),
    // retained: two distinct check-in days, still recently active
    install({ install_id: "b", first_seen: isoDaysAgo(10), last_seen: isoDaysAgo(1) }),
    // retained but not active7d (last check-in 20 days ago)
    install({ install_id: "c", first_seen: isoDaysAgo(40), last_seen: isoDaysAgo(20) }),
  ];
  const installDays: InstallDay[] = [
    { install_id: "a", day: dayKey(isoDaysAgo(20)) },
    { install_id: "b", day: dayKey(isoDaysAgo(10)) },
    { install_id: "b", day: dayKey(isoDaysAgo(1)) },
    { install_id: "c", day: dayKey(isoDaysAgo(40)) },
    { install_id: "c", day: dayKey(isoDaysAgo(20)) },
  ];

  const funnel = activationFunnel(installs, installDays, NOW);
  assert.equal(funnel.installsEver, 3);
  assert.equal(funnel.activated, 3);
  assert.equal(funnel.retained, 2); // b and c, not a
  assert.equal(funnel.active7d, 1); // b only
  assert.equal(funnel.active30d, 3);
});

test("perCountryTable: per-country totals, activity, and top version among active installs", () => {
  const installs = [
    install({ install_id: "u1", country: "US", version: "1.1.40", last_seen: isoDaysAgo(1) }),
    install({ install_id: "u2", country: "US", version: "1.1.40", last_seen: isoDaysAgo(1) }),
    install({ install_id: "u3", country: "US", version: "1.1.30", last_seen: isoDaysAgo(90) }),
    install({ install_id: "in1", country: "IN", version: "1.1.20", last_seen: isoDaysAgo(2) }),
    install({ install_id: "n1", country: null, version: "1.1.20", last_seen: isoDaysAgo(2) }),
  ];
  const installDays: InstallDay[] = [{ install_id: "u1", day: dayKey(NOW) }];

  const rows = perCountryTable(installs, installDays, NOW);
  const us = rows.find((r) => r.country === "US")!;
  assert.equal(us.total, 3);
  assert.equal(us.active7d, 2);
  assert.equal(us.lapsed, 1);
  assert.equal(us.lapseRatePct, 33);
  assert.equal(us.checkinsToday, 1);
  assert.equal(us.topVersion, "1.1.40");

  const unknown = rows.find((r) => r.country === "??")!;
  assert.equal(unknown.total, 1);

  // Sorted by total descending.
  assert.equal(rows[0].country, "US");
});

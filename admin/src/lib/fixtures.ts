import "server-only";

import type { DailyCountry, Install, InstallDay } from "@/lib/types";

/**
 * Deterministic synthetic telemetry, for running and screenshotting the admin
 * without a Supabase project.
 *
 * Gated on BOTH `ADMIN_FIXTURES === "1"` and `NODE_ENV !== "production"` —
 * checked again at the call site in `data.ts`, not just here, so a stray
 * import can never turn fixtures on in a deployed build by accident. This
 * module is otherwise inert: nothing here runs unless something explicitly
 * asks for `fixtureInstalls()` etc.
 *
 * "Deterministic" means a fixed seed, not `Math.random()` — the same install
 * ids, countries and check-in history every run, so a screenshot taken today
 * matches one taken tomorrow and a bug reproduces from "load /users with
 * fixtures" alone.
 */
export function fixturesEnabled(): boolean {
  return process.env.ADMIN_FIXTURES === "1" && process.env.NODE_ENV !== "production";
}

// mulberry32 — small, dependency-free, deterministic PRNG. Good enough for
// synthetic fixture data; never used for anything security-sensitive.
function mulberry32(seed: number) {
  let a = seed >>> 0;
  return function rand() {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const FIXTURE_SEED = 20260924;
const FIXTURE_INSTALL_COUNT = 2000;
const FIXTURE_HISTORY_DAYS = 180;

/** ~40 countries, weighted so a handful dominate like a real distribution
 *  (US/India/Germany/Brazil heavy tail), the rest a long tail. */
const FIXTURE_COUNTRIES: { code: string; weight: number }[] = [
  { code: "US", weight: 18 }, { code: "IN", weight: 12 }, { code: "DE", weight: 9 },
  { code: "BR", weight: 8 }, { code: "GB", weight: 7 }, { code: "FR", weight: 6 },
  { code: "RU", weight: 5 }, { code: "CA", weight: 5 }, { code: "PL", weight: 4 },
  { code: "ES", weight: 4 }, { code: "IT", weight: 4 }, { code: "NL", weight: 3 },
  { code: "AU", weight: 3 }, { code: "JP", weight: 3 }, { code: "MX", weight: 3 },
  { code: "ID", weight: 3 }, { code: "TR", weight: 3 }, { code: "UA", weight: 3 },
  { code: "SE", weight: 2 }, { code: "AR", weight: 2 }, { code: "CN", weight: 2 },
  { code: "KR", weight: 2 }, { code: "VN", weight: 2 }, { code: "PH", weight: 2 },
  { code: "ZA", weight: 2 }, { code: "EG", weight: 2 }, { code: "NG", weight: 2 },
  { code: "PT", weight: 2 }, { code: "CZ", weight: 2 }, { code: "RO", weight: 2 },
  { code: "GR", weight: 2 }, { code: "FI", weight: 1 }, { code: "NO", weight: 1 },
  { code: "DK", weight: 1 }, { code: "CH", weight: 1 }, { code: "AT", weight: 1 },
  { code: "BE", weight: 1 }, { code: "IE", weight: 1 }, { code: "NZ", weight: 1 },
  { code: "CL", weight: 1 },
];

const FIXTURE_VERSIONS = [
  "1.1.43", "1.1.43", "1.1.43", "1.1.42", "1.1.42", "1.1.40", "1.1.37", "1.1.30",
];
const FIXTURE_DISTROS = ["ubuntu", "fedora", "arch", "debian", "mint", "opensuse", "pop-os"];
const FIXTURE_COMPOSITORS = ["gnome-shell", "kwin", "sway", "hyprland", "mutter", "xfwm4"];
const FIXTURE_SESSIONS = ["wayland", "x11"];
const FIXTURE_BACKENDS = ["vulkan", "gl"];
const FIXTURE_DECODES = ["hw", "sw"];
const FIXTURE_CHANNELS = ["deb", "flatpak", "other"];
const FIXTURE_SOURCES = ["website", "github", "reddit", null];
const FIXTURE_CITIES: Record<string, [string, string][]> = {
  US: [["New York", "NY"], ["San Francisco", "CA"], ["Austin", "TX"]],
  IN: [["Bengaluru", "KA"], ["Mumbai", "MH"], ["Delhi", "DL"]],
  DE: [["Berlin", "BE"], ["Munich", "BY"]],
  BR: [["São Paulo", "SP"], ["Rio de Janeiro", "RJ"]],
  GB: [["London", "England"], ["Manchester", "England"]],
};

function weightedPick<T>(rand: () => number, items: { value: T; weight: number }[]): T {
  const total = items.reduce((s, i) => s + i.weight, 0);
  let r = rand() * total;
  for (const item of items) {
    r -= item.weight;
    if (r <= 0) return item.value;
  }
  return items[items.length - 1].value;
}

function pick<T>(rand: () => number, arr: T[]): T {
  return arr[Math.floor(rand() * arr.length)];
}

type FixtureData = {
  installs: Install[];
  installDays: InstallDay[];
  dailyCountry: DailyCountry[];
};

let cached: FixtureData | null = null;

/** Build (and cache) the synthetic dataset. Pure given the fixed seed and a
 *  fixed `nowMs` — regenerated only if `nowMs`'s day changes, so "today" in
 *  the fixtures tracks the real clock across a long-running dev server. */
export function buildFixtures(nowMs: number): FixtureData {
  const dayKey = new Date(nowMs).toISOString().slice(0, 10);
  if (cached && (cached as FixtureData & { _day?: string })._day === dayKey) {
    return cached;
  }

  const rand = mulberry32(FIXTURE_SEED);
  const DAY_MS = 24 * 60 * 60 * 1000;
  const countryItems = FIXTURE_COUNTRIES.map((c) => ({ value: c.code, weight: c.weight }));

  const installs: Install[] = [];
  const installDays: InstallDay[] = [];

  for (let n = 0; n < FIXTURE_INSTALL_COUNT; n++) {
    const id = `fx_${n.toString(36).padStart(6, "0")}`;
    const country = weightedPick(rand, countryItems);

    // first_seen spread over the fixture window, weighted toward more recent
    // (a growing user base), with a small share of very old accounts.
    const ageDays = Math.floor(Math.pow(rand(), 1.6) * FIXTURE_HISTORY_DAYS);
    const firstSeenMs = nowMs - ageDays * DAY_MS;

    // Lifecycle mix: ~55% active, ~20% idle, ~25% lapsed — a plausible real
    // split with a meaningful lapsed cohort to exercise the whole page.
    const lifecycleRoll = rand();
    let lastSeenAgeDays: number;
    if (lifecycleRoll < 0.55) {
      lastSeenAgeDays = Math.min(ageDays, Math.floor(rand() * 7));
    } else if (lifecycleRoll < 0.75) {
      lastSeenAgeDays = Math.min(ageDays, 8 + Math.floor(rand() * 22));
    } else {
      lastSeenAgeDays = Math.min(ageDays, 31 + Math.floor(rand() * 120));
    }
    const lastSeenMs = nowMs - lastSeenAgeDays * DAY_MS;

    const minimal = rand() < 0.12;
    const version = pick(rand, FIXTURE_VERSIONS);
    const channel = pick(rand, FIXTURE_CHANNELS);
    const cityOptions = FIXTURE_CITIES[country];
    const hasCity = !minimal && cityOptions && rand() < 0.6;
    const [city, region] = hasCity ? pick(rand, cityOptions) : [null, null];

    installs.push({
      install_id: id,
      version,
      distro: minimal ? null : pick(rand, FIXTURE_DISTROS),
      compositor: minimal ? null : pick(rand, FIXTURE_COMPOSITORS),
      session: minimal ? null : pick(rand, FIXTURE_SESSIONS),
      backend: minimal ? null : pick(rand, FIXTURE_BACKENDS),
      decode: minimal ? null : pick(rand, FIXTURE_DECODES),
      source: minimal ? null : pick(rand, FIXTURE_SOURCES),
      channel,
      country,
      minimal,
      city,
      region,
      monitor_count: minimal ? null : 1 + Math.floor(rand() * 3),
      first_seen: new Date(firstSeenMs).toISOString(),
      last_seen: new Date(lastSeenMs).toISOString(),
    });

    // Check-in history: roughly every 1–4 days between first_seen and
    // last_seen, capped so the fixture set stays a few tens of thousands of
    // rows rather than millions.
    let cursor = firstSeenMs;
    let guard = 0;
    while (cursor <= lastSeenMs && guard < 200) {
      installDays.push({
        install_id: id,
        day: new Date(cursor).toISOString().slice(0, 10),
      });
      cursor += (1 + Math.floor(rand() * 4)) * DAY_MS;
      guard++;
    }
    // Always seed the exact last_seen day, in case the stride above skipped
    // past it — otherwise "checked in today" could undercount active rows.
    const lastSeenDay = new Date(lastSeenMs).toISOString().slice(0, 10);
    if (installDays[installDays.length - 1]?.day !== lastSeenDay) {
      installDays.push({ install_id: id, day: lastSeenDay });
    }
  }

  const data: FixtureData & { _day: string } = {
    installs,
    installDays,
    // Empty: fixtures exist to demo the current schema, and daily_country is
    // the pre-install_days historical fallback — nothing to backfill here.
    dailyCountry: [],
    _day: dayKey,
  };
  cached = data;
  return data;
}

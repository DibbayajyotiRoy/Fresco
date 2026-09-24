/**
 * Pure filter / sort / paginate over an already-fetched `Install[]`, driven
 * entirely by plain strings so it can be built straight from a Next.js
 * `searchParams` object — no client-side JS is required for the table on
 * /users to filter, sort or page: it is a plain GET form and plain links,
 * and the URL is the only state.
 */

// Relative import, not "@/lib/install-analytics": Next.js resolves the "@/"
// alias at build time via tsconfig `paths`, but plain `node --test` (used by
// `npm test`, see package.json) only resolves relative specifiers. Keeping
// this one relative is what lets `installs-query.test.ts` import this module
// directly and have its own transitive import resolve too.
import type { Install } from "./types.ts";
import { type LifecycleStatus, lifecycleStatus } from "./install-analytics.ts";

export const PAGE_SIZES = [10, 25, 50, 100] as const;
export const DEFAULT_PAGE_SIZE = 25;

export type InstallSortKey = "last_seen" | "first_seen" | "install_id" | "country" | "version";

export type InstallsQuery = {
  status: LifecycleStatus | "all";
  country: string | "all";
  version: string | "all";
  channel: string | "all";
  /** Case-insensitive prefix match on install_id. */
  q: string;
  sort: InstallSortKey;
  dir: "asc" | "desc";
  page: number;
  pageSize: number;
};

type RawParams = Record<string, string | string[] | undefined>;

function one(v: string | string[] | undefined): string | undefined {
  return Array.isArray(v) ? v[0] : v;
}

const SORT_KEYS: InstallSortKey[] = ["last_seen", "first_seen", "install_id", "country", "version"];
const STATUSES: (LifecycleStatus | "all")[] = ["all", "active", "idle", "lapsed"];

/** Parse `searchParams` into a fully-defaulted query. Never throws — an
 *  unrecognised or malformed param just falls back to its default, so a
 *  hand-edited or stale URL degrades instead of erroring the page. */
export function parseInstallsQuery(params: RawParams): InstallsQuery {
  const status = one(params.status);
  const sort = one(params.sort);
  const dir = one(params.dir);
  const page = Number(one(params.page));
  const pageSize = Number(one(params.pageSize));

  return {
    status: STATUSES.includes(status as LifecycleStatus | "all")
      ? (status as LifecycleStatus | "all")
      : "all",
    country: one(params.country)?.trim() || "all",
    version: one(params.version)?.trim() || "all",
    channel: one(params.channel)?.trim() || "all",
    q: one(params.q)?.trim() ?? "",
    sort: SORT_KEYS.includes(sort as InstallSortKey) ? (sort as InstallSortKey) : "last_seen",
    dir: dir === "asc" ? "asc" : "desc",
    page: Number.isFinite(page) && page > 0 ? Math.floor(page) : 1,
    pageSize: (PAGE_SIZES as readonly number[]).includes(pageSize)
      ? pageSize
      : DEFAULT_PAGE_SIZE,
  };
}

export type InstallRow = Install & {
  status: LifecycleStatus;
  /** Check-ins recorded for this install in the last 30 days, from
   *  `install_days` — null when that history is not available (fixtures
   *  always provide it; a live deployment before the migration will not). */
  checkinDays30d: number | null;
};

export type InstallsPage = {
  rows: InstallRow[];
  /** Rows matching the filters, before pagination. */
  total: number;
  page: number;
  pageSize: number;
  pageCount: number;
};

/**
 * Filter, sort and slice one page of installs.
 *
 * `checkinCounts30d` is a precomputed `install_id -> count` map (build it
 * once per request with `install-analytics.ts`'s day-bucketing, not here —
 * this function stays a plain synchronous reducer over what it is given).
 */
export function applyInstallsQuery(
  installs: Install[],
  query: InstallsQuery,
  nowMs: number,
  checkinCounts30d: Map<string, number> | null
): InstallsPage {
  const q = query.q.toLowerCase();

  let rows: InstallRow[] = installs
    .map((i) => ({
      ...i,
      status: lifecycleStatus(i.last_seen, nowMs),
      checkinDays30d: checkinCounts30d?.get(i.install_id) ?? (checkinCounts30d ? 0 : null),
    }))
    .filter((i) => query.status === "all" || i.status === query.status)
    .filter((i) => query.country === "all" || (i.country ?? "??") === query.country)
    .filter((i) => query.version === "all" || i.version === query.version)
    .filter((i) => query.channel === "all" || i.channel === query.channel)
    .filter((i) => q === "" || i.install_id.toLowerCase().startsWith(q));

  const dirMul = query.dir === "asc" ? 1 : -1;
  rows = rows.sort((a, b) => {
    switch (query.sort) {
      case "install_id":
        return a.install_id.localeCompare(b.install_id) * dirMul;
      case "country":
        return (a.country ?? "").localeCompare(b.country ?? "") * dirMul;
      case "version":
        return (a.version ?? "").localeCompare(b.version ?? "") * dirMul;
      case "first_seen":
        return (Date.parse(a.first_seen) - Date.parse(b.first_seen)) * dirMul;
      case "last_seen":
      default:
        return (Date.parse(a.last_seen) - Date.parse(b.last_seen)) * dirMul;
    }
  });

  const total = rows.length;
  const pageCount = Math.max(1, Math.ceil(total / query.pageSize));
  const page = Math.min(query.page, pageCount);
  const start = (page - 1) * query.pageSize;
  const pageRows = rows.slice(start, start + query.pageSize);

  return { rows: pageRows, total, page, pageSize: query.pageSize, pageCount };
}

/** Build a new `URLSearchParams` string from the current query with one field
 *  overridden — every filter control and pager link is a plain `<a href>` or
 *  form built from this, so the table needs no client JS to be interactive. */
export function withQuery(
  current: RawParams,
  overrides: Partial<Record<string, string | number>>
): string {
  const params = new URLSearchParams();
  for (const [k, v] of Object.entries(current)) {
    const val = one(v);
    if (val) params.set(k, val);
  }
  for (const [k, v] of Object.entries(overrides)) {
    if (v === undefined || v === "" || v === "all") params.delete(k);
    else params.set(k, String(v));
  }
  // A filter/sort change always returns to page 1 unless page is the field
  // being set explicitly.
  if (!("page" in overrides)) params.delete("page");
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

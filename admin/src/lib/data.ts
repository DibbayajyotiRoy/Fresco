import "server-only";

import { cache } from "react";

import { getSupabaseAdmin } from "@/lib/supabase-admin";
import type {
  CatalogItem,
  DailyCountry,
  Feedback,
  Issue,
  Notification,
  Release,
  Repo,
  FeatureEvent,
  Install,
  SupportMessage,
  SupportThread,
  SupportUnread,
  TelemetryError,
  TelemetryEvent,
} from "@/lib/types";

/**
 * Every exported getter below is wrapped in React's `cache()`.
 *
 * The pages are split into Suspense-streamed sections, and sections are
 * independent by design — several of them in the same page legitimately need
 * the same rows (installs feed the overview tiles, the version breakdown AND
 * the map). Without dedupe each section issues its own query, so one 720ms
 * Supabase round-trip becomes three sequential ones and the page gets slower
 * the better it is factored. `cache()` collapses repeat calls made during a
 * single server render into one underlying query.
 *
 * It is per-request memoisation, not a cache in the "stale data" sense: the
 * store is created fresh for each render, so nothing leaks between users or
 * between refreshes, and a `router.refresh()` genuinely re-queries.
 *
 * Getters that take arguments dedupe on argument identity — see the note on
 * `getEventsSince`.
 */

const GITHUB_REPO = process.env.GITHUB_REPO || "DibbayajyotiRoy/fresco";

/**
 * Headers for a GitHub REST call. `anonymous` deliberately drops the token —
 * see `githubFetch`, which uses it to retry a rejected credential.
 */
function githubHeaders(options?: {
  anonymous?: boolean;
}): Record<string, string> {
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (!options?.anonymous && process.env.GITHUB_TOKEN) {
    headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  }
  return headers;
}

/**
 * Fetch one GitHub REST URL, retrying the SAME request once without the
 * Authorization header if the token is rejected.
 *
 * Every endpoint this file touches (repo, releases, issues) is a public read
 * that works fine unauthenticated — the token only buys a higher rate limit.
 * So a stale token must not be fatal: before this retry, one expired
 * GITHUB_TOKEN in .env.local blanked stars, total downloads AND the issue list
 * simultaneously, all three reporting "401 Unauthorized", which points at
 * GitHub rather than at the actual cause. Degrading to the anonymous rate
 * limit is strictly better than showing "—" for every GitHub number.
 *
 * Responses are cached for 60 seconds (`next: { revalidate: 60 }`), not
 * `no-store`. GitHub is the slowest thing this dashboard touches — the repo
 * endpoint costs ~700ms and the paginated releases endpoint ~1.1s per render —
 * and stars and download totals do not move second to second. A minute of
 * staleness is invisible on these numbers and makes repeat navigation
 * essentially free. Supabase queries, which back the parts of the dashboard
 * that genuinely are live, stay uncached.
 */
const GITHUB_REVALIDATE_SECONDS = 60;

async function githubFetch(url: string): Promise<Response> {
  const res = await fetch(url, {
    headers: githubHeaders(),
    next: { revalidate: GITHUB_REVALIDATE_SECONDS },
  });

  // Only a token we actually sent can be the thing being rejected.
  if (res.status !== 401 || !process.env.GITHUB_TOKEN) {
    return res;
  }

  // Whatever the retry says is the more honest answer: if it succeeds the token
  // was the only problem, and if it fails its status describes the repo itself
  // rather than the credential. Same revalidation as the first attempt — an
  // uncached retry would put the 700ms+ back on every render that needs it.
  return fetch(url, {
    headers: githubHeaders({ anonymous: true }),
    next: { revalidate: GITHUB_REVALIDATE_SECONDS },
  });
}

/**
 * Turn a failed GitHub response into something the maintainer can act on.
 * "GitHub API 403" tells you nothing; "you are rate limited, set a token"
 * tells you what to do next.
 */
function githubErrorMessage(res: Response): string {
  // GitHub uses 403 for rate limiting on the older path and 429 on the newer
  // one; the remaining-quota header is what distinguishes either from a plain
  // permission error.
  if (
    (res.status === 403 || res.status === 429) &&
    res.headers.get("x-ratelimit-remaining") === "0"
  ) {
    return `GitHub rate limit reached (${res.status}). Unauthenticated requests are capped at 60/hour — set a valid GITHUB_TOKEN in .env.local to raise it.`;
  }

  switch (res.status) {
    case 401:
      // We only ever surface a 401 after the unauthenticated retry above, so
      // the credential is no longer a candidate explanation.
      return `GitHub API 401 for ${GITHUB_REPO} even without a token — the repo may be private, or GITHUB_REPO may be wrong.`;
    case 404:
      return `GitHub repo not found: ${GITHUB_REPO} — check GITHUB_REPO in .env.local.`;
    default:
      return `GitHub API ${res.status}: ${res.statusText}`;
  }
}

export type DataResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: string };

const SUPABASE_MISSING = "Set SUPABASE_SERVICE_ROLE_KEY in .env.local";

/** Fetch all feedback rows, newest first. Deduped per render. */
export const getFeedback = cache(async (): Promise<DataResult<Feedback[]>> => {
  const supabase = getSupabaseAdmin();
  if (!supabase) {
    return { ok: false, error: SUPABASE_MISSING };
  }

  const { data, error } = await supabase
    .from("feedback")
    .select("id, created_at, rating, comment, app_version, os, ticket")
    .order("created_at", { ascending: false });

  if (error) {
    return { ok: false, error: error.message };
  }

  return { ok: true, data: (data ?? []) as Feedback[] };
});

/** Fetch all notifications, newest first. Deduped per render. */
export const getNotifications = cache(async (): Promise<
  DataResult<Notification[]>
> => {
  const supabase = getSupabaseAdmin();
  if (!supabase) {
    return { ok: false, error: SUPABASE_MISSING };
  }

  const { data, error } = await supabase
    .from("notifications")
    .select("id, created_at, title, body, url, published")
    .order("created_at", { ascending: false });

  if (error) {
    return { ok: false, error: error.message };
  }

  return { ok: true, data: (data ?? []) as Notification[] };
});

type GitHubRepo = {
  stargazers_count: number;
  forks_count: number;
  subscribers_count: number;
  open_issues_count: number;
  html_url: string;
  pushed_at: string | null;
};

/**
 * Fetch top-level repo stats (stars, forks, watchers, open issues).
 *
 * Deduped per render, and the underlying GitHub response is cached for 60s —
 * a star count a minute behind is not a number anyone can act on differently.
 */
export const getRepo = cache(async (): Promise<DataResult<Repo>> => {
  try {
    const res = await githubFetch(`https://api.github.com/repos/${GITHUB_REPO}`);

    if (!res.ok) {
      return { ok: false, error: githubErrorMessage(res) };
    }

    const r = (await res.json()) as GitHubRepo;
    return {
      ok: true,
      data: {
        stars: r.stargazers_count ?? 0,
        forks: r.forks_count ?? 0,
        watchers: r.subscribers_count ?? 0,
        openIssues: r.open_issues_count ?? 0,
        url: r.html_url,
        pushedAt: r.pushed_at,
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : "Unknown error";
    return { ok: false, error: `Failed to reach GitHub: ${message}` };
  }
});

type GitHubAsset = { name: string; download_count: number };
type GitHubRelease = {
  tag_name: string;
  name: string | null;
  published_at: string | null;
  draft: boolean;
  assets: GitHubAsset[];
};

/** GitHub's maximum for this endpoint; asking for more is silently clamped. */
const RELEASES_PER_PAGE = 100;
/**
 * Hard stop on paging. At 100 per page this covers 2000 releases, far past
 * anything real — it exists only so a misbehaving or looping API can never
 * hang a dashboard render.
 */
const MAX_RELEASE_PAGES = 20;

/**
 * Fetch GitHub releases and sum download counts per release.
 *
 * Paginated: total downloads is a headline number on the Overview page, and a
 * single `per_page=100` request would silently start under-reporting it the
 * moment the repo passes 100 releases — a wrong number nobody would notice,
 * which is worse than an error.
 *
 * Deduped per render, which matters most here: this is the single most
 * expensive call in the dashboard (a page or more of GitHub requests at ~1.1s
 * each) and both the Overview and Usage pages want it. The GitHub responses
 * themselves are cached for 60s on top of that.
 */
export const getReleases = cache(async (): Promise<DataResult<Release[]>> => {
  try {
    const raw: GitHubRelease[] = [];

    for (let page = 1; page <= MAX_RELEASE_PAGES; page++) {
      const res = await githubFetch(
        `https://api.github.com/repos/${GITHUB_REPO}/releases?per_page=${RELEASES_PER_PAGE}&page=${page}`
      );

      if (!res.ok) {
        return { ok: false, error: githubErrorMessage(res) };
      }

      const json = (await res.json()) as GitHubRelease[];
      raw.push(...json);

      // A short page is the last page — GitHub only sends fewer than asked
      // when it has run out.
      if (json.length < RELEASES_PER_PAGE) break;
    }

    const releases: Release[] = raw
      .filter((r) => !r.draft)
      .map((r) => ({
        tag: r.tag_name,
        name: r.name || r.tag_name,
        downloads: (r.assets ?? []).reduce(
          (sum, a) => sum + (a.download_count ?? 0),
          0
        ),
        publishedAt: r.published_at,
        // Kept per-file so the Usage page can separate `.deb` pulls from
        // `install.sh` fetches; the installer triggers both, so the total
        // double-counts one install.
        assets: (r.assets ?? []).map((a) => ({
          name: a.name,
          downloads: a.download_count ?? 0,
        })),
      }));

    // Oldest -> newest, so the bar chart reads left to right chronologically.
    releases.sort((a, b) => {
      const ta = a.publishedAt ? Date.parse(a.publishedAt) : 0;
      const tb = b.publishedAt ? Date.parse(b.publishedAt) : 0;
      return ta - tb;
    });

    return { ok: true, data: releases };
  } catch (err) {
    const message = err instanceof Error ? err.message : "Unknown error";
    return { ok: false, error: `Failed to reach GitHub: ${message}` };
  }
});

type GitHubIssue = {
  number: number;
  title: string;
  state: string;
  html_url: string;
  user: { login: string } | null;
  comments: number;
  created_at: string;
  labels: ({ name: string } | string)[];
  /** Present only on pull requests — GitHub lists PRs under /issues too. */
  pull_request?: unknown;
};

/**
 * Fetch OPEN GitHub issues for the repo, newest first. Pull requests (which the
 * issues endpoint also returns) are filtered out.
 *
 * Deduped per render; the GitHub response behind it is cached for 60s.
 */
export const getIssues = cache(async (): Promise<DataResult<Issue[]>> => {
  try {
    const res = await githubFetch(
      `https://api.github.com/repos/${GITHUB_REPO}/issues?state=open&per_page=50&sort=created&direction=desc`
    );

    if (!res.ok) {
      return { ok: false, error: githubErrorMessage(res) };
    }

    const json = (await res.json()) as GitHubIssue[];

    const issues: Issue[] = json
      .filter((i) => !i.pull_request)
      .map((i) => ({
        number: i.number,
        title: i.title,
        state: i.state,
        url: i.html_url,
        author: i.user?.login ?? null,
        comments: i.comments,
        createdAt: i.created_at,
        labels: (i.labels ?? []).map((l) => (typeof l === "string" ? l : l.name)),
      }));

    return { ok: true, data: issues };
  } catch (err) {
    const message = err instanceof Error ? err.message : "Unknown error";
    return { ok: false, error: `Failed to reach GitHub: ${message}` };
  }
});

/**
 * Fetch all install rows, most recently seen first.
 *
 * Deduped per render — nearly every section on the Overview and Usage pages
 * derives from this one query.
 */
export const getInstalls = cache(async (): Promise<DataResult<Install[]>> => {
  const supabase = getSupabaseAdmin();
  if (!supabase) {
    return { ok: false, error: SUPABASE_MISSING };
  }

  const { data, error } = await supabase
    .from("installs")
    .select(
      "install_id, version, distro, compositor, session, backend, decode, source, channel, country, city, region, minimal, monitor_count, first_seen, last_seen"
    )
    .order("last_seen", { ascending: false })
    .limit(10000);

  if (error) {
    return { ok: false, error: error.message };
  }

  return { ok: true, data: (data ?? []) as Install[] };
});

/**
 * Fetch telemetry events created since `sinceIso`, newest first.
 *
 * Deduped per render, but only across calls with an IDENTICAL argument:
 * `cache()` keys on the arguments compared with `Object.is`, so two sections
 * asking for "the last 30 days" share a result only if they pass the very same
 * string. Computing the cutoff independently in each section — say, from
 * `Date.now()` — produces two slightly different ISO strings and two queries.
 * Derive the cutoff once per page and pass it down.
 */
export const getEventsSince = cache(
  async (sinceIso: string): Promise<DataResult<TelemetryEvent[]>> => {
    const supabase = getSupabaseAdmin();
    if (!supabase) {
      return { ok: false, error: SUPABASE_MISSING };
    }

    const { data, error } = await supabase
      .from("events")
      .select("install_id, name, created_at")
      .gte("created_at", sinceIso)
      .order("created_at", { ascending: false })
      .limit(10000);

    if (error) {
      return { ok: false, error: error.message };
    }

    return { ok: true, data: (data ?? []) as TelemetryEvent[] };
  }
);

/**
 * Fetch events (with props) for a specific set of event names since
 * `sinceIso`, newest first. Filtered server-side so we never pull the
 * whole events table just to inspect a couple of features.
 *
 * Deduped per render on argument identity — and `names` is where that bites.
 * Arrays are compared by reference, so a call site that builds the list inline
 * (`getFeatureEventsSince(since, [...DEPTH_EVENTS])`, or any literal `[...]`)
 * hands over a fresh array every time and never shares a result, however equal
 * the contents. Passing a stable module-level constant is what makes two
 * sections asking the same question cost one query.
 */
export const getFeatureEventsSince = cache(
  async (
    sinceIso: string,
    names: string[]
  ): Promise<DataResult<FeatureEvent[]>> => {
    const supabase = getSupabaseAdmin();
    if (!supabase) {
      return { ok: false, error: SUPABASE_MISSING };
    }

    const { data, error } = await supabase
      .from("events")
      .select("install_id, name, props, created_at")
      .in("name", names)
      .gte("created_at", sinceIso)
      .order("created_at", { ascending: false })
      .limit(10000);

    if (error) {
      return { ok: false, error: error.message };
    }

    return { ok: true, data: (data ?? []) as FeatureEvent[] };
  }
);

/**
 * Fetch error reports created since `sinceIso`, newest first. Deduped per
 * render for callers passing an identical `sinceIso` (see `getEventsSince`).
 */
export const getErrorsSince = cache(
  async (sinceIso: string): Promise<DataResult<TelemetryError[]>> => {
    const supabase = getSupabaseAdmin();
    if (!supabase) {
      return { ok: false, error: SUPABASE_MISSING };
    }

    const { data, error } = await supabase
      .from("errors")
      .select("id, install_id, kind, detail, version, created_at")
      .gte("created_at", sinceIso)
      .order("created_at", { ascending: false })
      .limit(10000);

    if (error) {
      return { ok: false, error: error.message };
    }

    return { ok: true, data: (data ?? []) as TelemetryError[] };
  }
);

/** Fetch all catalog items, newest first. Deduped per render. */
export const getCatalogItems = cache(async (): Promise<
  DataResult<CatalogItem[]>
> => {
  const supabase = getSupabaseAdmin();
  if (!supabase) {
    return { ok: false, error: SUPABASE_MISSING };
  }

  const { data, error } = await supabase
    .from("catalog_items")
    .select(
      "id, created_at, content_type, title, category, tags, media_url, thumb_url, size_bytes, license, author, source_url, published, install_count"
    )
    .order("created_at", { ascending: false });

  if (error) {
    return { ok: false, error: error.message };
  }

  return { ok: true, data: (data ?? []) as CatalogItem[] };
});

/**
 * Fetch the country-only cohort's daily tallies since `sinceIso` (a date).
 *
 * Returns an empty list rather than an error when the table does not exist
 * yet: this ships ahead of the schema migration, and a dashboard that 500s
 * because one panel is early is worse than a panel that says "no data".
 *
 * Deduped per render for callers passing an identical `sinceDate` — a plain
 * date string, so this one shares easily across sections.
 */
export const getDailyCountrySince = cache(
  async (sinceDate: string): Promise<DataResult<DailyCountry[]>> => {
    const supabase = getSupabaseAdmin();
    if (!supabase) {
      return { ok: false, error: SUPABASE_MISSING };
    }

    const { data, error } = await supabase
      .from("daily_country")
      .select("day, country, version, channel, pings")
      .gte("day", sinceDate)
      .order("day", { ascending: false })
      .limit(10000);

    if (error) {
      // 42P01 = undefined_table: the migration has not been run yet.
      if (error.code === "42P01") {
        return { ok: true, data: [] };
      }
      return { ok: false, error: error.message };
    }

    return { ok: true, data: (data ?? []) as DailyCountry[] };
  }
);

/**
 * Every support thread, most recently active first, with its messages.
 *
 * Two queries rather than a join: the message list per thread is small, and
 * fetching them flat keeps the shape obvious. Returns empty rather than an
 * error when the tables do not exist yet, so this page works before the schema
 * migration is applied.
 *
 * Deduped per render — the pair of queries is what makes that worth doing.
 */
export const getSupportThreads = cache(async (): Promise<
  DataResult<{ thread: SupportThread; messages: SupportMessage[] }[]>
> => {
  const supabase = getSupabaseAdmin();
  if (!supabase) {
    return { ok: false, error: SUPABASE_MISSING };
  }

  const threadsRes = await supabase
    .from("support_threads")
    .select(
      "ticket, created_at, last_at, env, app_version, status, unread_for_maintainer, unread_for_user, origin, rating"
    )
    .order("last_at", { ascending: false })
    .limit(500);

  if (threadsRes.error) {
    if (threadsRes.error.code === "42P01") return { ok: true, data: [] };
    return { ok: false, error: threadsRes.error.message };
  }

  const threads = (threadsRes.data ?? []) as SupportThread[];
  if (threads.length === 0) return { ok: true, data: [] };

  const messagesRes = await supabase
    .from("support_messages")
    .select("id, ticket, sender, body, created_at")
    .in(
      "ticket",
      threads.map((t) => t.ticket)
    )
    .order("created_at", { ascending: true })
    .limit(5000);

  if (messagesRes.error) {
    return { ok: false, error: messagesRes.error.message };
  }

  const byTicket = new Map<string, SupportMessage[]>();
  for (const m of (messagesRes.data ?? []) as SupportMessage[]) {
    const list = byTicket.get(m.ticket) ?? [];
    list.push(m);
    byTicket.set(m.ticket, list);
  }

  return {
    ok: true,
    data: threads.map((thread) => ({
      thread,
      messages: byTicket.get(thread.ticket) ?? [],
    })),
  };
});

/** Just enough of a message to decide whether it is still waiting on a reply.
 *  `body` is deliberately not selected: this is polled every few seconds for a
 *  badge, and the text is never shown. */
type UnreadProbe = Pick<SupportMessage, "ticket" | "sender" | "created_at">;

const NOTHING_UNREAD: SupportUnread = {
  threads: 0,
  messages: 0,
  latestAt: null,
};

/**
 * How many support messages are waiting on the maintainer.
 *
 * Same two-query shape as `getSupportThreads`, narrowed twice over: the thread
 * query filters on the denormalised `unread_for_maintainer` flag, and when that
 * comes back empty — the normal case for an inbox at zero — it returns without
 * issuing the second query at all.
 *
 * "Unattended" is computed from the messages rather than trusted from the flag:
 * a message counts when it is from the user and arrived strictly after the
 * maintainer's last reply on that thread (so all of them, if the maintainer
 * never replied). That is what makes the badge count messages-to-read instead
 * of threads-to-open.
 *
 * Returns zeros rather than an error when the tables do not exist yet, so the
 * nav renders before the schema migration is applied.
 *
 * Deduped per render: this backs a badge in the nav, which is rendered on every
 * page alongside whatever else that page asks for.
 */
export const getSupportUnread = cache(async (): Promise<
  DataResult<SupportUnread>
> => {
  const supabase = getSupabaseAdmin();
  if (!supabase) {
    return { ok: false, error: SUPABASE_MISSING };
  }

  const threadsRes = await supabase
    .from("support_threads")
    .select("ticket")
    .eq("unread_for_maintainer", true)
    .limit(500);

  if (threadsRes.error) {
    // 42P01 = undefined_table: the migration has not been run yet.
    if (threadsRes.error.code === "42P01") {
      return { ok: true, data: NOTHING_UNREAD };
    }
    return { ok: false, error: threadsRes.error.message };
  }

  const tickets = ((threadsRes.data ?? []) as { ticket: string }[]).map(
    (t) => t.ticket
  );
  if (tickets.length === 0) {
    return { ok: true, data: NOTHING_UNREAD };
  }

  const messagesRes = await supabase
    .from("support_messages")
    .select("ticket, sender, created_at")
    .in("ticket", tickets)
    .order("created_at", { ascending: true })
    .limit(5000);

  if (messagesRes.error) {
    if (messagesRes.error.code === "42P01") {
      return { ok: true, data: NOTHING_UNREAD };
    }
    return { ok: false, error: messagesRes.error.message };
  }

  const rows = (messagesRes.data ?? []) as UnreadProbe[];

  // Pass one: the cutoff per thread. Threads with no maintainer message are
  // simply absent from the map, which reads as "no cutoff — everything counts".
  const repliedAt = new Map<string, number>();
  for (const m of rows) {
    if (m.sender !== "maintainer") continue;
    const at = Date.parse(m.created_at);
    const previous = repliedAt.get(m.ticket);
    if (previous === undefined || at > previous) {
      repliedAt.set(m.ticket, at);
    }
  }

  // Pass two: count user messages past that cutoff.
  const waitingThreads = new Set<string>();
  let messages = 0;
  let latestAtMs: number | null = null;
  let latestAt: string | null = null;

  for (const m of rows) {
    if (m.sender !== "user") continue;
    const at = Date.parse(m.created_at);
    const cutoff = repliedAt.get(m.ticket);
    if (cutoff !== undefined && at <= cutoff) continue;

    messages++;
    waitingThreads.add(m.ticket);
    if (latestAtMs === null || at > latestAtMs) {
      latestAtMs = at;
      latestAt = m.created_at;
    }
  }

  return {
    ok: true,
    // Counted from the messages, not from `tickets.length`: a thread can carry
    // a stale unread flag with nothing actually unanswered behind it, and the
    // badge should not claim work that is not there.
    data: { threads: waitingThreads.size, messages, latestAt },
  };
});

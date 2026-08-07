export type Feedback = {
  id: string;
  created_at: string;
  /** -1 = thumbs down, 1 = thumbs up */
  rating: number;
  comment: string | null;
  app_version: string | null;
  os: string | null;
  /** Support ticket, present only when the submitter ticked "let the
   *  maintainer reply". Null means do not contact — show no reply button. */
  ticket: string | null;
};

export type Notification = {
  id: string;
  created_at: string;
  title: string;
  body: string;
  url: string | null;
  published: boolean;
};

export type Issue = {
  /** Issue number, e.g. 42 */
  number: number;
  title: string;
  /** "open" | "closed" */
  state: string;
  /** Link to the issue on GitHub. */
  url: string;
  /** Reporter's GitHub login, or null. */
  author: string | null;
  comments: number;
  /** ISO created date. */
  createdAt: string;
  labels: string[];
};

export type Repo = {
  /** Stargazer count. */
  stars: number;
  /** Fork count. */
  forks: number;
  /** Watchers (subscribers) — people who get notifications. */
  watchers: number;
  /** Open issues + PRs, as GitHub reports them. */
  openIssues: number;
  /** Link to the repo on GitHub. */
  url: string;
  /** ISO timestamp of the last push, or null. */
  pushedAt: string | null;
};

/** One published file on a release, with its own counter. */
export type ReleaseAsset = {
  /** Asset filename as published, e.g. "fresco_1.1.37-1_amd64.deb" or
   *  "install.sh". */
  name: string;
  downloads: number;
};

export type Release = {
  /** Release tag, e.g. "v0.0.3" */
  tag: string;
  /** Display name (falls back to tag). */
  name: string;
  /** Total download count summed across all assets. */
  downloads: number;
  /** ISO publish date, or null for drafts. */
  publishedAt: string | null;
  /**
   * Per-file counts behind `downloads`.
   *
   * The total alone overstates installs: the one-liner installer fetches
   * `install.sh` AND then the `.deb` it points at, so a single install can
   * increment two counters. Keeping the filenames is the only way to say that
   * out loud instead of quietly presenting the conflated number as users.
   */
  assets: ReleaseAsset[];
};

export type Install = {
  install_id: string;
  version: string | null;
  distro: string | null;
  compositor: string | null;
  session: string | null;
  backend: string | null;
  decode: string | null;
  /** UTM-style download attribution (website/github/reddit/…), null for older installs. */
  source: string | null;
  /** Packaging channel (deb/flatpak/other). */
  channel: string | null;
  /** Two-letter country, resolved at the edge server-side. Null for installs
   *  predating it, or when the edge header did not arrive. */
  country: string | null;
  /** True when this row was written under the essential tier: identity and
   *  country are real, the environment columns are simply not collected (not
   *  "unknown"), and last_seen is truncated to the day. */
  minimal: boolean;
  /** Coarse place names, full-consent tier only. Client-supplied via the
   *  landing site's /api/geo, so unlike `country` these are spoofable — fine
   *  for a chart, never to be trusted for anything. Null on essential rows. */
  city: string | null;
  region: string | null;
  monitor_count: number | null;
  /** ISO timestamps. */
  first_seen: string;
  last_seen: string;
};

export type TelemetryEvent = {
  install_id: string | null;
  name: string;
  created_at: string;
};

export type FeatureEvent = {
  install_id: string | null;
  name: string;
  /** Raw props jsonb — e.g. { ok, source, kind } for add_from_link. */
  props: Record<string, unknown> | null;
  created_at: string;
};

export type TelemetryError = {
  id: number;
  install_id: string | null;
  kind: string;
  detail: string | null;
  version: string | null;
  created_at: string;
};

export type CatalogItem = {
  id: string;
  created_at: string;
  content_type: string;
  title: string;
  category: string;
  tags: string[];
  media_url: string;
  thumb_url: string | null;
  size_bytes: number;
  license: string;
  author: string;
  source_url: string | null;
  published: boolean;
  install_count: number;
};

/**
 * One bucket of the country-only cohort: everyone who declined the optional
 * statistics, on one day, in one country, on one version and channel.
 *
 * `pings` counts requests, NOT people. The client throttles itself to one ping
 * per ~20h, so a day's total is a close proxy for daily-active installs in this
 * cohort — but there is no identifier in this table, so it can never be
 * de-duplicated across days into a monthly figure the way `installs` can.
 */
export type DailyCountry = {
  /** ISO date (no time). */
  day: string;
  /** Two-letter code, or the '??' sentinel when the edge header did not
   *  arrive (this table's key columns are NOT NULL — see schema.sql). */
  country: string;
  version: string | null;
  channel: string | null;
  pings: number;
};

/** One anonymous support thread, as the maintainer sees it. */
export type SupportThread = {
  /** Random uuid the client generated. Not the telemetry install id, and not
   *  linkable to one — see src/support.rs. It is all we know about them. */
  ticket: string;
  created_at: string;
  last_at: string;
  /** The environment block the user chose to attach, if any. */
  env: string | null;
  app_version: string | null;
  status: "open" | "answered" | "closed";
  unread_for_maintainer: boolean;
  unread_for_user: boolean;
  /** "feedback" when the thread was opened by submitting a rating. */
  origin: "direct" | "feedback";
  /** -1 / 1 when it came from feedback, else null. */
  rating: number | null;
};

export type SupportMessage = {
  id: number;
  ticket: string;
  sender: "user" | "maintainer";
  body: string;
  created_at: string;
};

/**
 * How much support is waiting on the maintainer, for the nav badge.
 *
 * Counted in MESSAGES rather than threads: one person who sent four messages
 * while waiting is four things to read, and a badge that says "1" for that
 * undersells the queue. `threads` is kept alongside it so the badge can be
 * explained ("3 messages across 2 threads") without a second query.
 */
export type SupportUnread = {
  /** Threads with an unanswered user message. */
  threads: number;
  /** User messages that arrived after the maintainer's last reply, summed
   *  across those threads. This is the badge number. */
  messages: number;
  /** ISO timestamp of the newest unattended user message, or null. */
  latestAt: string | null;
};

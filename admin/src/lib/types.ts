export type Feedback = {
  id: string;
  created_at: string;
  /** -1 = thumbs down, 1 = thumbs up */
  rating: number;
  comment: string | null;
  app_version: string | null;
  os: string | null;
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

export type Release = {
  /** Release tag, e.g. "v0.0.3" */
  tag: string;
  /** Display name (falls back to tag). */
  name: string;
  /** Total download count summed across all assets. */
  downloads: number;
  /** ISO publish date, or null for drafts. */
  publishedAt: string | null;
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

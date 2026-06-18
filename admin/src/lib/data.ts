import "server-only";

import { getSupabaseAdmin } from "@/lib/supabase-admin";
import type { Feedback, Issue, Notification, Release } from "@/lib/types";

export type DataResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: string };

const SUPABASE_MISSING = "Set SUPABASE_SERVICE_ROLE_KEY in .env.local";

/** Fetch all feedback rows, newest first. */
export async function getFeedback(): Promise<DataResult<Feedback[]>> {
  const supabase = getSupabaseAdmin();
  if (!supabase) {
    return { ok: false, error: SUPABASE_MISSING };
  }

  const { data, error } = await supabase
    .from("feedback")
    .select("id, created_at, rating, comment, app_version, os")
    .order("created_at", { ascending: false });

  if (error) {
    return { ok: false, error: error.message };
  }

  return { ok: true, data: (data ?? []) as Feedback[] };
}

/** Fetch all notifications, newest first. */
export async function getNotifications(): Promise<DataResult<Notification[]>> {
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
}

type GitHubAsset = { download_count: number };
type GitHubRelease = {
  tag_name: string;
  name: string | null;
  published_at: string | null;
  draft: boolean;
  assets: GitHubAsset[];
};

/**
 * Fetch GitHub releases and sum download counts per release.
 * Always fetched fresh (`no-store`) so the dashboard reflects live counts.
 */
export async function getReleases(): Promise<DataResult<Release[]>> {
  const repo = process.env.GITHUB_REPO || "DibbayajyotiRoy/fresco";
  const token = process.env.GITHUB_TOKEN;

  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  try {
    const res = await fetch(
      `https://api.github.com/repos/${repo}/releases?per_page=100`,
      {
        headers,
        cache: "no-store",
      }
    );

    if (!res.ok) {
      return {
        ok: false,
        error: `GitHub API ${res.status}: ${res.statusText}`,
      };
    }

    const json = (await res.json()) as GitHubRelease[];

    const releases: Release[] = json
      .filter((r) => !r.draft)
      .map((r) => ({
        tag: r.tag_name,
        name: r.name || r.tag_name,
        downloads: r.assets.reduce(
          (sum, a) => sum + (a.download_count ?? 0),
          0
        ),
        publishedAt: r.published_at,
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
}

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
 * issues endpoint also returns) are filtered out. Fetched fresh (`no-store`).
 */
export async function getIssues(): Promise<DataResult<Issue[]>> {
  const repo = process.env.GITHUB_REPO || "DibbayajyotiRoy/fresco";
  const token = process.env.GITHUB_TOKEN;

  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  try {
    const res = await fetch(
      `https://api.github.com/repos/${repo}/issues?state=open&per_page=50&sort=created&direction=desc`,
      { headers, cache: "no-store" }
    );

    if (!res.ok) {
      return { ok: false, error: `GitHub API ${res.status}: ${res.statusText}` };
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
}

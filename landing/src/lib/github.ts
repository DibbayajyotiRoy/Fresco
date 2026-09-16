/**
 * Live release stats from GitHub, fetched server-side and revalidated every
 * REVALIDATE seconds (ISR), so a new release shows up on the site within that
 * window with no redeploy. The real numbers are rendered into the SSR HTML,
 * which matters for both SEO and GEO: AI answer engines and crawlers read the
 * static markup, so the download count and version must be in the initial
 * response.
 *
 * Version sources, in order:
 *   1. the REST API's release list (also gives downloads);
 *   2. the github.com/…/releases/latest redirect, served by github.com itself
 *      rather than the API, so it is not subject to the API's
 *      60-requests-an-hour anonymous limit that shared hosting IPs run into;
 *   3. FALLBACK_VERSION, a last resort only.
 * Set GITHUB_TOKEN in production to lift the API limit to 5,000 an hour.
 */

const REPO = "DibbayajyotiRoy/fresco";
/** Last resort, only when both the API and the redirect fail. */
const FALLBACK_VERSION = "1.1.42";
const RELEASES_LATEST = `https://github.com/${REPO}/releases/latest`;
/** Seconds between refreshes of the version, downloads and stars. */
const REVALIDATE = 600;

export type GitHubStats = {
  /** Latest published version without a leading "v", e.g. "1.1.42". */
  version: string;
  /** Total downloads across all release assets, or null if unavailable. */
  downloads: number | null;
  /** GitHub stargazer count, or null if unavailable. */
  stars: number | null;
  /** Direct download URL for the newest .deb asset, or the releases page. */
  debUrl: string;
};

type Asset = { name: string; download_count: number; browser_download_url: string };
type Release = { tag_name: string; prerelease: boolean; draft: boolean; assets: Asset[] };

const headers = (): Record<string, string> => {
  const h: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "User-Agent": "fresco-landing",
  };
  if (process.env.GITHUB_TOKEN) h.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  return h;
};

/** "…/releases/tag/v1.1.42" → "1.1.42", via the redirect github.com serves. */
async function versionFromRedirect(): Promise<string | null> {
  try {
    const res = await fetch(RELEASES_LATEST, {
      method: "HEAD",
      redirect: "follow",
      next: { revalidate: REVALIDATE },
    });
    const match = res.url.match(/\/releases\/tag\/v?([^/?#]+)$/);
    return match ? decodeURIComponent(match[1]) : null;
  } catch {
    return null;
  }
}

export async function getGitHubStats(): Promise<GitHubStats> {
  const fallback: GitHubStats = {
    version: FALLBACK_VERSION,
    downloads: null,
    stars: null,
    debUrl: RELEASES_LATEST,
  };

  try {
    const [releasesRes, repoRes] = await Promise.all([
      fetch(`https://api.github.com/repos/${REPO}/releases?per_page=100`, {
        headers: headers(),
        next: { revalidate: REVALIDATE },
      }),
      fetch(`https://api.github.com/repos/${REPO}`, {
        headers: headers(),
        next: { revalidate: REVALIDATE },
      }),
    ]);

    const stars = repoRes.ok
      ? ((await repoRes.json()) as { stargazers_count?: number }).stargazers_count ?? null
      : null;

    const releases = releasesRes.ok ? ((await releasesRes.json()) as Release[]) : null;
    if (!Array.isArray(releases) || releases.length === 0) {
      return { ...fallback, version: (await versionFromRedirect()) ?? FALLBACK_VERSION, stars };
    }

    const downloads = releases.reduce(
      (sum, r) => sum + r.assets.reduce((a, x) => a + (x.download_count ?? 0), 0),
      0,
    );
    const latest = releases.find((r) => !r.prerelease && !r.draft) ?? releases[0];
    const version = (latest.tag_name ?? FALLBACK_VERSION).replace(/^v/, "");
    const deb = latest.assets.find((a) => a.name.endsWith(".deb"));

    return {
      version,
      downloads: downloads || null,
      stars,
      debUrl: deb?.browser_download_url ?? RELEASES_LATEST,
    };
  } catch {
    return { ...fallback, version: (await versionFromRedirect()) ?? FALLBACK_VERSION };
  }
}

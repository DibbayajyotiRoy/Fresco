/**
 * Live release stats from the GitHub API, fetched server-side and revalidated
 * hourly (ISR). The real numbers are rendered into the SSR HTML, which matters
 * for both SEO and GEO: AI answer engines and crawlers read the static markup,
 * so the download count and version must be in the initial response.
 */

const REPO = "DibbayajyotiRoy/fresco";
const FALLBACK_VERSION = "0.0.9";
const RELEASES_LATEST = `https://github.com/${REPO}/releases/latest`;

export type GitHubStats = {
  /** Latest published version without a leading "v", e.g. "0.0.3". */
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
        next: { revalidate: 3600 },
      }),
      fetch(`https://api.github.com/repos/${REPO}`, {
        headers: headers(),
        next: { revalidate: 3600 },
      }),
    ]);

    const stars = repoRes.ok
      ? ((await repoRes.json()) as { stargazers_count?: number }).stargazers_count ?? null
      : null;

    if (!releasesRes.ok) return { ...fallback, stars };

    const releases = (await releasesRes.json()) as Release[];
    if (!Array.isArray(releases) || releases.length === 0) {
      return { ...fallback, stars };
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
    return fallback;
  }
}

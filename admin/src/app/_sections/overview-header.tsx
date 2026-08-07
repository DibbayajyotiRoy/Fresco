import { ArrowSquareOut, GithubLogo } from "@phosphor-icons/react/dist/ssr";

import { PageHeader } from "@/components/page-header";
import { Skeleton } from "@/components/skeleton";
import { getFeedback, getReleases } from "@/lib/data";
import { formatNumber } from "@/lib/format";

export const REPO_URL = "https://github.com/DibbayajyotiRoy/fresco";

function GithubLink() {
  return (
    <a
      href={REPO_URL}
      target="_blank"
      rel="noopener noreferrer"
      className="press inline-flex h-7 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-2 text-sm font-medium text-stone-700 transition-colors hover:bg-stone-100"
    >
      <GithubLogo className="size-3.5" weight="fill" />
      GitHub
      <ArrowSquareOut className="size-3 text-stone-400" />
    </a>
  );
}

/**
 * The header's meta line counts downloads and feedback, so it needs the two
 * slowest sources on the page. It streams on its own rather than holding the
 * title — and the GitHub link — hostage for a second.
 */
export async function OverviewHeader() {
  const [releasesRes, feedbackRes] = await Promise.all([
    getReleases(),
    getFeedback(),
  ]);

  const releases = releasesRes.ok ? releasesRes.data : [];
  const feedback = feedbackRes.ok ? feedbackRes.data : [];
  const totalDownloads = releases.reduce((s, r) => s + r.downloads, 0);

  return (
    <PageHeader
      title="Overview"
      meta={`${formatNumber(totalDownloads)} downloads · ${formatNumber(feedback.length)} feedback`}
      action={<GithubLink />}
    />
  );
}

/** Mirrors `PageHeader`'s box so the title and link do not move on swap. */
export function OverviewHeaderFallback() {
  return (
    <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2">
      <h1 className="font-serif text-2xl tracking-tight text-stone-900">
        Overview
      </h1>
      <div className="flex items-baseline gap-3">
        <Skeleton className="h-3 w-44" />
        <GithubLink />
      </div>
    </div>
  );
}

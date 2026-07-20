import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { IssuesTable } from "@/app/issues/issues-table";
import { getIssues } from "@/lib/data";
import { formatNumber } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function IssuesPage() {
  const res = await getIssues();
  const issues = res.ok ? res.data : [];
  const withComments = issues.filter((i) => i.comments > 0).length;

  return (
    <div className="space-y-3">
      <PageHeader
        title="Issues"
        meta={res.ok ? `${formatNumber(issues.length)} open` : undefined}
      />

      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        <StatCard
          label="Open issues"
          value={res.ok ? formatNumber(issues.length) : "—"}
          hint={res.ok ? undefined : res.error}
        />
        <StatCard
          label="With comments"
          value={res.ok ? formatNumber(withComments) : "—"}
        />
      </div>

      {!res.ok ? (
        <ErrorPanel title="Couldn't load issues" message={res.error} />
      ) : issues.length === 0 ? (
        <EmptyState
          title="No open issues"
          description="New issues opened in the repo will appear here."
        />
      ) : (
        <IssuesTable issues={issues} />
      )}
    </div>
  );
}

import { Bug, ChatCircle } from "@phosphor-icons/react/dist/ssr";

import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { Card, CardContent } from "@/components/ui/card";
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
    <>
      <PageHeader
        title="Issues"
        description="Open issues from the GitHub repository"
      />
      <div className="flex flex-1 flex-col gap-6 p-4 md:p-6">
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
          <StatCard
            label="Open issues"
            value={formatNumber(issues.length)}
            icon={Bug}
          />
          <StatCard
            label="With comments"
            value={formatNumber(withComments)}
            icon={ChatCircle}
          />
        </div>

        <Card>
          <CardContent>
            {!res.ok ? (
              <EmptyState
                title="Couldn't load issues"
                description={res.error}
              />
            ) : issues.length === 0 ? (
              <EmptyState
                title="No open issues"
                icon={Bug}
                description="New issues opened in the repo will appear here."
              />
            ) : (
              <IssuesTable issues={issues} />
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}

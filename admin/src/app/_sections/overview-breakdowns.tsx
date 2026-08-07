import { EmptyState } from "@/components/empty-state";
import { Panel, PanelHeader } from "@/components/panel";
import {
  DistributionList,
  type DistributionItem,
} from "@/components/distribution-list";
import { getFeedback } from "@/lib/data";

/** Bucket freeform values into a top-N breakdown with an "Other" rollup. */
function topDistribution(
  values: (string | null)[],
  n = 6
): DistributionItem[] {
  const counts = new Map<string, number>();
  for (const v of values) {
    const key = v?.trim() || "Unknown";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const sorted = [...counts.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value);

  if (sorted.length <= n) return sorted;
  const head = sorted.slice(0, n);
  const other = sorted.slice(n).reduce((s, i) => s + i.value, 0);
  return [...head, { label: "Other", value: other }];
}

/** Both breakdowns come off the one feedback query, so they share a boundary. */
export async function FeedbackBreakdowns() {
  const feedbackRes = await getFeedback();
  const feedback = feedbackRes.ok ? feedbackRes.data : [];

  const osDist = topDistribution(feedback.map((f) => f.os));
  const versionDist = topDistribution(feedback.map((f) => f.app_version));

  return (
    <>
      <Panel className="section-in">
        <PanelHeader title="Platform" meta="by feedback" />
        {osDist.length === 0 ? (
          <EmptyState
            className="py-6"
            title="No data yet"
            description="The OS field arrives with app feedback."
          />
        ) : (
          <DistributionList items={osDist} total={feedback.length} />
        )}
      </Panel>

      <Panel className="section-in">
        <PanelHeader title="App version" meta="by feedback" />
        {versionDist.length === 0 ? (
          <EmptyState
            className="py-6"
            title="No data yet"
            description="The version field arrives with app feedback."
          />
        ) : (
          <DistributionList items={versionDist} total={feedback.length} />
        )}
      </Panel>
    </>
  );
}

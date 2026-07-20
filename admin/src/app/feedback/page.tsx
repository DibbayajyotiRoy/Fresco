import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { FeedbackTable } from "@/app/feedback/feedback-table";
import { getFeedback } from "@/lib/data";
import { formatNumber } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function FeedbackPage() {
  const res = await getFeedback();
  const feedback = res.ok ? res.data : [];

  const up = feedback.filter((f) => f.rating > 0).length;
  const down = feedback.filter((f) => f.rating < 0).length;
  const satisfaction = up + down > 0 ? Math.round((up / (up + down)) * 100) : 0;

  return (
    <div className="space-y-3">
      <PageHeader
        title="Feedback"
        meta={res.ok ? `${formatNumber(feedback.length)} reports` : undefined}
      />

      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        <StatCard
          label="Total"
          value={res.ok ? formatNumber(feedback.length) : "—"}
          hint={res.ok ? undefined : res.error}
        />
        <StatCard label="Thumbs up" value={res.ok ? formatNumber(up) : "—"} />
        <StatCard
          label="Thumbs down"
          value={res.ok ? formatNumber(down) : "—"}
        />
        <StatCard
          label="Satisfaction"
          value={up + down > 0 ? `${satisfaction}%` : "—"}
          hint="up / (up + down)"
        />
      </div>

      {!res.ok ? (
        <ErrorPanel title="Couldn't load feedback" message={res.error} />
      ) : feedback.length === 0 ? (
        <EmptyState
          title="No feedback yet"
          description="Ratings submitted from the app will appear here."
        />
      ) : (
        <FeedbackTable feedback={feedback} />
      )}
    </div>
  );
}

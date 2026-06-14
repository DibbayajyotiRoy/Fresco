import {
  ChatCircle,
  Gauge,
  ThumbsDown,
  ThumbsUp,
} from "@phosphor-icons/react/dist/ssr";

import { PageHeader } from "@/components/page-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { Card, CardContent } from "@/components/ui/card";
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
    <>
      <PageHeader
        title="Feedback"
        description="Anonymous ratings and comments from the app"
      />
      <div className="flex flex-1 flex-col gap-6 p-4 md:p-6">
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
          <StatCard
            label="Total"
            value={formatNumber(feedback.length)}
            icon={ChatCircle}
          />
          <StatCard
            label="Thumbs up"
            value={formatNumber(up)}
            icon={ThumbsUp}
          />
          <StatCard
            label="Thumbs down"
            value={formatNumber(down)}
            icon={ThumbsDown}
          />
          <StatCard
            label="Satisfaction"
            value={up + down > 0 ? `${satisfaction}%` : "—"}
            hint="up / (up + down)"
            icon={Gauge}
          />
        </div>

        <Card>
          <CardContent>
            {!res.ok ? (
              <EmptyState
                title="Couldn't load feedback"
                description={res.error}
              />
            ) : feedback.length === 0 ? (
              <EmptyState
                title="No feedback yet"
                icon={ThumbsUp}
                description="Ratings submitted from the app will appear here."
              />
            ) : (
              <FeedbackTable feedback={feedback} />
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}

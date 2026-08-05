import { PageHeader } from "@/components/page-header";
import { ErrorPanel } from "@/components/error-panel";
import { StatCard } from "@/components/stat-card";
import { getSupportThreads } from "@/lib/data";
import { formatNumber } from "@/lib/format";

import { SupportInbox } from "./support-inbox";

export const dynamic = "force-dynamic";
export const revalidate = 0;

/**
 * Anonymous two-way support.
 *
 * Everything on this page is what a user chose to type plus, if they left the
 * box ticked, the setup summary they saw before sending. There is no name, no
 * email, no IP, and no telemetry install id — the ticket is generated
 * separately from telemetry precisely so a conversation can never be joined to
 * an environment profile. That is by design; see src/support.rs.
 */
export default async function SupportPage() {
  const res = await getSupportThreads();
  const conversations = res.ok ? res.data : [];

  const waiting = conversations.filter(
    (c) => c.thread.unread_for_maintainer
  ).length;
  const open = conversations.filter((c) => c.thread.status !== "closed").length;
  const messages = conversations.reduce((n, c) => n + c.messages.length, 0);

  return (
    <div className="space-y-3">
      <PageHeader
        title="Support"
        meta={
          res.ok
            ? `${formatNumber(conversations.length)} threads · ${formatNumber(messages)} messages`
            : undefined
        }
      />

      {!res.ok ? (
        <ErrorPanel title="Couldn't load support threads" message={res.error} />
      ) : (
        <>
          <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
            <StatCard
              label="Waiting on you"
              value={formatNumber(waiting)}
              hint="threads with an unanswered message"
            />
            <StatCard
              label="Open"
              value={formatNumber(open)}
              hint="not marked closed"
            />
            <StatCard
              label="Threads"
              value={formatNumber(conversations.length)}
              hint="conversations ever started"
            />
            <StatCard
              label="Messages"
              value={formatNumber(messages)}
              hint="both directions"
            />
          </div>

          <SupportInbox conversations={conversations} />
        </>
      )}
    </div>
  );
}

"use client";

import { useState, useTransition } from "react";
import { ArrowBendUpLeft, Check, CircleNotch } from "@phosphor-icons/react";

import { Panel, PanelHeader } from "@/components/panel";
import { EmptyState } from "@/components/empty-state";
import { Textarea } from "@/components/ui/textarea";
import { formatRelative, truncateId } from "@/lib/format";
import type { SupportMessage, SupportThread } from "@/lib/types";
import { replyToThread, setThreadStatus } from "./actions";

export type Conversation = {
  thread: SupportThread;
  messages: SupportMessage[];
};

/**
 * The maintainer's side of the anonymous support threads.
 *
 * There is deliberately no identity column: a ticket, what they wrote, and the
 * setup they chose to attach is the whole of what exists. That is the point of
 * the feature, not a gap in this UI.
 */
/** Ranking for the inbox: an unhappy user waiting on a reply is the whole
 *  reason this page exists, so they sort above everything else. */
function priority(c: Conversation): number {
  const waiting = c.thread.unread_for_maintainer ? 0 : 2;
  const unhappy = c.thread.rating != null && c.thread.rating < 0 ? 0 : 1;
  return waiting + unhappy;
}

export function SupportInbox({
  conversations: unsorted,
}: {
  conversations: Conversation[];
}) {
  const conversations = [...unsorted].sort(
    (a, b) =>
      priority(a) - priority(b) ||
      Date.parse(b.thread.last_at) - Date.parse(a.thread.last_at)
  );
  const [selected, setSelected] = useState<string | null>(
    conversations[0]?.thread.ticket ?? null
  );

  if (conversations.length === 0) {
    return (
      <Panel>
        <EmptyState
          title="No conversations yet"
          description="Threads appear here when someone writes from inside the app (menu → Message the maintainer). Nothing is sent until they do."
        />
      </Panel>
    );
  }

  const active = conversations.find((c) => c.thread.ticket === selected);

  return (
    <div className="grid grid-cols-1 gap-3 lg:grid-cols-[320px_1fr]">
      <Panel className="max-h-[70vh] overflow-y-auto">
        <PanelHeader
          title="Threads"
          meta={`${conversations.filter((c) => c.thread.unread_for_maintainer).length} waiting`}
        />
        <ul className="flex flex-col gap-1">
          {conversations.map(({ thread, messages }) => {
            const last = messages[messages.length - 1];
            const isActive = thread.ticket === selected;
            return (
              <li key={thread.ticket}>
                <button
                  type="button"
                  onClick={() => setSelected(thread.ticket)}
                  className={`w-full rounded-md border px-3 py-2 text-left transition-colors ${
                    isActive
                      ? "border-neutral-500 bg-neutral-900/60"
                      : "border-transparent hover:bg-neutral-900/40"
                  }`}
                >
                  <div className="flex items-baseline justify-between gap-2">
                    <span className="flex items-baseline gap-1.5 font-mono text-[0.7rem] uppercase tracking-widest text-neutral-400">
                      {thread.origin === "feedback" ? (
                        <span
                          title={
                            thread.rating != null && thread.rating < 0
                              ? "opened from negative feedback"
                              : "opened from positive feedback"
                          }
                          className={
                            thread.rating != null && thread.rating < 0
                              ? "text-red-400"
                              : "text-emerald-400"
                          }
                        >
                          {thread.rating != null && thread.rating < 0 ? "▼" : "▲"}
                        </span>
                      ) : null}
                      {truncateId(thread.ticket)}
                    </span>
                    {thread.unread_for_maintainer ? (
                      <span className="size-1.5 shrink-0 rounded-full bg-amber-400" />
                    ) : null}
                  </div>
                  <p className="mt-1 line-clamp-2 text-sm text-neutral-200">
                    {last?.body ?? "(empty)"}
                  </p>
                  <p className="mt-1 font-mono text-[0.7rem] text-neutral-500">
                    {formatRelative(thread.last_at)} · {thread.status}
                    {thread.app_version ? ` · v${thread.app_version}` : ""}
                  </p>
                </button>
              </li>
            );
          })}
        </ul>
      </Panel>

      {active ? <Thread key={active.thread.ticket} conversation={active} /> : null}
    </div>
  );
}

function Thread({ conversation }: { conversation: Conversation }) {
  const { thread, messages } = conversation;
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();

  function send() {
    const body = draft.trim();
    if (!body || pending) return;
    setError(null);
    startTransition(async () => {
      const result = await replyToThread(thread.ticket, body);
      if (result.ok) {
        setDraft("");
      } else {
        setError(result.error);
      }
    });
  }

  return (
    <Panel className="flex max-h-[70vh] flex-col">
      <PanelHeader
        title={truncateId(thread.ticket)}
        meta={`opened ${formatRelative(thread.created_at)}`}
      />

      {thread.env ? (
        <pre className="mb-3 whitespace-pre-wrap rounded-md border border-neutral-800 bg-neutral-950/60 px-3 py-2 font-mono text-[0.72rem] leading-relaxed text-neutral-400">
          {thread.env}
        </pre>
      ) : (
        <p className="mb-3 font-mono text-[0.72rem] text-neutral-500">
          No setup attached — they unticked the box.
        </p>
      )}

      <div className="flex-1 space-y-3 overflow-y-auto pr-1">
        {messages.map((m) => (
          <div
            key={m.id}
            className={
              m.sender === "maintainer" ? "flex justify-end" : "flex justify-start"
            }
          >
            <div
              className={`max-w-[85%] rounded-md px-3 py-2 ${
                m.sender === "maintainer"
                  ? "bg-neutral-800 text-neutral-100"
                  : "border border-neutral-800 bg-neutral-950/60 text-neutral-200"
              }`}
            >
              <p className="font-mono text-[0.65rem] uppercase tracking-widest text-neutral-500">
                {m.sender === "maintainer" ? "you" : "user"} ·{" "}
                {formatRelative(m.created_at)}
              </p>
              <p className="mt-1 whitespace-pre-wrap text-sm">{m.body}</p>
            </div>
          </div>
        ))}
      </div>

      <div className="mt-3 space-y-2 border-t border-neutral-800 pt-3">
        <Textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="Reply as the maintainer. They see this inside the app, and only they can see it."
          rows={3}
          onKeyDown={(e) => {
            // Ctrl/Cmd+Enter sends; plain Enter keeps making paragraphs,
            // because support replies are usually more than one line.
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
              e.preventDefault();
              send();
            }
          }}
        />
        {error ? <p className="text-sm text-red-400">{error}</p> : null}
        <div className="flex items-center justify-between gap-2">
          <button
            type="button"
            onClick={() =>
              startTransition(async () => {
                await setThreadStatus(
                  thread.ticket,
                  thread.status === "closed" ? "open" : "closed"
                );
              })
            }
            className="font-mono text-[0.7rem] uppercase tracking-widest text-neutral-500 transition-colors hover:text-neutral-200"
          >
            {thread.status === "closed" ? "Reopen" : "Mark closed"}
          </button>
          <button
            type="button"
            onClick={send}
            disabled={pending || draft.trim().length === 0}
            className="inline-flex items-center gap-1.5 rounded-md bg-neutral-100 px-3 py-1.5 text-sm font-medium text-neutral-900 transition-opacity disabled:opacity-40"
          >
            {pending ? (
              <CircleNotch className="size-4 animate-spin" weight="bold" />
            ) : thread.status === "answered" ? (
              <Check className="size-4" weight="bold" />
            ) : (
              <ArrowBendUpLeft className="size-4" weight="bold" />
            )}
            Send reply
          </button>
        </div>
      </div>
    </Panel>
  );
}

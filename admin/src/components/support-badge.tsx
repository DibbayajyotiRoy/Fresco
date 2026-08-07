"use client";

/* Live count of support messages still waiting on the maintainer, shown on the
 * Support nav item. Polls `/api/support/unread` at the house 10s cadence (see
 * auto-refresh.tsx), sleeps while the tab is backgrounded, and plays the
 * notify cue when the count *rises* — arrivals only, never the first read. */

import * as React from "react";

import { playNotify } from "@/lib/sound";

const ENDPOINT = "/api/support/unread";
const POLL_MS = 10_000;

/** The route answers HTTP 200 even when it fails, zeroing the counts and
 *  adding `error` — so a present `error` is treated as a failed poll, not as
 *  "the queue is empty". Errors never reach the nav. */
type UnreadPayload = {
  threads?: number;
  messages?: number;
  latestAt?: string | null;
  error?: string;
};

/**
 * Unattended support-message count, or `null` while unresolved. Callers hide
 * the badge on `null` so there is no flash of "0" before the first response.
 */
export function useSupportUnread(intervalMs: number = POLL_MS): number | null {
  const [count, setCount] = React.useState<number | null>(null);

  React.useEffect(() => {
    // `null` until the first *successful* poll lands. The baseline is what
    // keeps the mount silent: with no previous number there is no increase to
    // detect, so seeing 7 on arrival is a reading, not an event.
    let baseline: number | null = null;
    let alive = true;
    let controller: AbortController | null = null;

    async function poll() {
      if (!alive || document.hidden) return;

      // Only ever one request in flight; a slow one is dropped for the newer.
      controller?.abort();
      const ac = new AbortController();
      controller = ac;

      try {
        const res = await fetch(ENDPOINT, {
          cache: "no-store",
          signal: ac.signal,
        });
        if (!res.ok) return;
        const data = (await res.json()) as UnreadPayload;
        if (!alive || ac.signal.aborted) return;
        if (data?.error) return;
        if (typeof data?.messages !== "number" || !Number.isFinite(data.messages)) {
          return;
        }

        const next = Math.max(0, Math.trunc(data.messages));
        if (baseline !== null && next > baseline) {
          // Sound is decoration: a suspended AudioContext must never take the
          // badge down with it. `fire()` already swallows, belt-and-braces here.
          try {
            playNotify();
          } catch {
            /* audio is never load-bearing */
          }
        }
        baseline = next;
        setCount(next);
      } catch {
        /* Network blip, abort, or malformed body — keep whatever is on screen.
         * A failed poll must never blank an already-displayed count. */
      }
    }

    function onVisibility() {
      if (!document.hidden) void poll();
    }

    void poll();
    const id = setInterval(() => void poll(), intervalMs);
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      alive = false;
      clearInterval(id);
      document.removeEventListener("visibilitychange", onVisibility);
      controller?.abort();
    };
  }, [intervalMs]);

  return count;
}

/** `Support, 7 messages waiting` — the badge is decorative, so the count has
 *  to reach assistive tech through the link's own name. */
export function supportNavLabel(title: string, count: number | null): string | undefined {
  if (!count) return undefined;
  return `${title}, ${count} ${count === 1 ? "message" : "messages"} waiting`;
}

/** How long the arrival emphasis is held at full strength before it settles. */
const PULSE_MS = 550;

/**
 * The readout itself: sky-accented mono pill, tabular so it never jitters,
 * capped at `99+` so the nav cannot reflow. Renders nothing until a count has
 * resolved and is non-zero. Sits in flow as a sibling of the (md-collapsing)
 * label rather than inside it, so it stays pinned beside the icon in the
 * icon-only state; `min-w-4` on a 16px box makes single digits read as a dot.
 *
 * On an *arrival* it acknowledges itself: the hairline goes to full sky and the
 * pill swells 10% for half a second, then settles. The rule is the same one the
 * notify cue follows — increases only, and never the first resolved reading, so
 * the ten-second poll is silent and finding 7 waiting on page load is a state of
 * the world rather than an event. The previous count is tracked here rather than
 * threaded through props so `useSupportUnread`'s signature stays a plain number.
 *
 * Built from CSS transitions rather than a keyframe on purpose (§8): a keyframe
 * restarts from frame zero if a second arrival lands mid-play, which reads as a
 * stutter; a transition just retargets from wherever it currently is.
 */
export function SupportBadge({ count }: { count: number | null }) {
  const [pulsing, setPulsing] = React.useState(false);
  const previous = React.useRef<number | null>(null);

  React.useEffect(() => {
    const before = previous.current;
    previous.current = count;
    // `before === null` is the first resolved reading; `count <= before` covers
    // both a quiet poll and the maintainer working the queue down.
    if (before === null || count === null || count <= before) return;
    setPulsing(true);
    const id = setTimeout(() => setPulsing(false), PULSE_MS);
    return () => clearTimeout(id);
  }, [count]);

  if (!count) return null;
  return (
    <span
      aria-hidden
      className={
        "inline-flex h-4 min-w-4 shrink-0 items-center justify-center rounded-full border bg-sky-600/10 px-1 font-mono text-meta leading-none font-medium text-sky-700 tabular-nums " +
        "transition-[border-color,scale] " +
        // Snap up, settle down slowly: the rise has to be caught out of the
        // corner of the eye, the fall must not compete with reading the page.
        (pulsing
          ? "scale-110 border-sky-600 duration-150 ease-exit"
          : "scale-100 border-sky-600/40 duration-[400ms] ease-hover")
      }
    >
      {count > 99 ? "99+" : count}
    </span>
  );
}

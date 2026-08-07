"use client";

import * as React from "react";
import { useRouter } from "next/navigation";

/**
 * Near-real-time polling: periodically calls `router.refresh()` (an App Router
 * soft refresh) so the server components re-run with fresh data — no full page
 * reload, scroll position preserved. Data stays fetched server-side.
 *
 * The polling is visibility-aware, because a refresh is not free: each one
 * re-runs every query the page issues, and a dashboard left open in a
 * background tab would otherwise spend all day competing with the render the
 * user is actually waiting for. So the interval does nothing while the tab is
 * hidden, and a single refresh fires the moment it becomes visible again —
 * which is also when staleness starts to matter. Looking at the tab is the
 * event worth reacting to, not the clock.
 */
export function AutoRefresh({ intervalMs = 10000 }: { intervalMs?: number }) {
  const router = useRouter();

  React.useEffect(() => {
    const id = setInterval(() => {
      // Skip rather than pause: the next tick after the tab comes back is at
      // most one interval away, and `visibilitychange` has already refreshed.
      if (document.hidden) return;
      router.refresh();
    }, intervalMs);

    const onVisibilityChange = () => {
      if (!document.hidden) router.refresh();
    };

    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      clearInterval(id);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [router, intervalMs]);

  return null;
}

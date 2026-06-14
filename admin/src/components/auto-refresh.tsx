"use client";

import * as React from "react";
import { useRouter } from "next/navigation";

/**
 * Near-real-time polling: periodically calls `router.refresh()` (an App Router
 * soft refresh) so the server components re-run with fresh data — no full page
 * reload, scroll position preserved. Data stays fetched server-side.
 */
export function AutoRefresh({ intervalMs = 10000 }: { intervalMs?: number }) {
  const router = useRouter();

  React.useEffect(() => {
    const id = setInterval(() => {
      router.refresh();
    }, intervalMs);

    return () => clearInterval(id);
  }, [router, intervalMs]);

  return null;
}

import { NextResponse } from "next/server";

import { getSupportUnread } from "@/lib/data";

// Polled by the nav badge every few seconds — a cached answer is a wrong
// answer here, so opt out of every layer of Next's caching.
export const dynamic = "force-dynamic";
export const revalidate = 0;

/**
 * Support-inbox counts for the nav badge.
 *
 * Always answers 200, even when the query fails: this feeds the page chrome on
 * every route, and a failed fetch there would surface as a broken header
 * rather than as the one missing number it actually is. On failure the counts
 * come back as zeros (so the badge simply does not render) with the reason
 * attached under `error` for whoever is looking at the network tab.
 *
 * `getSupportUnread` reaches Supabase through the service-role client, which is
 * `server-only` — correct here, since a route handler never ships to the
 * browser.
 */
export async function GET() {
  const result = await getSupportUnread();

  const body = result.ok
    ? result.data
    : { threads: 0, messages: 0, latestAt: null, error: result.error };

  return NextResponse.json(body, {
    headers: { "cache-control": "no-store" },
  });
}

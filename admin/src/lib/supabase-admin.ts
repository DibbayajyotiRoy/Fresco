import "server-only";

import { createClient, type SupabaseClient } from "@supabase/supabase-js";

/**
 * Server-only Supabase client using the service_role key.
 *
 * NEVER import this from a client component. The service_role key bypasses
 * Row Level Security and must stay on the server.
 *
 * Returns `null` (instead of throwing) when the env is missing so that pages
 * can render a friendly empty state rather than crashing the whole app.
 */
let cached: SupabaseClient | null = null;

export function getSupabaseAdmin(): SupabaseClient | null {
  const url = process.env.NEXT_PUBLIC_SUPABASE_URL;
  const key = process.env.SUPABASE_SERVICE_ROLE_KEY;

  if (!url || !key) {
    return null;
  }

  if (cached) {
    return cached;
  }

  cached = createClient(url, key, {
    auth: {
      persistSession: false,
      autoRefreshToken: false,
    },
  });

  return cached;
}

export const MISSING_SUPABASE_MESSAGE =
  "Set SUPABASE_SERVICE_ROLE_KEY in .env.local";

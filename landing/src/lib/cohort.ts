import { COHORT } from "@/lib/site";

/**
 * Live opt-in telemetry cohort, fetched server-side from Supabase and
 * revalidated hourly (ISR), the same way lib/github.ts reads release stats.
 * The numbers land in the SSR HTML, so crawlers and answer engines read them.
 *
 * Source: the `public.public_stats()` RPC (supabase/schema.sql), which returns
 * aggregate counts only; install rows stay unreadable to the anon key. Until
 * that function is migrated, or when the env is missing, the network fails or
 * the reply is malformed, this returns the hand-maintained COHORT floors with
 * `live: false`, and the stats band renders them with a "+".
 *
 * Export names and the return shape are a contract: the stats band and the
 * testimonials section both import them.
 *
 * `users` / `countries`: distinct installs / distinct countries from the
 * opt-in telemetry. `active24h`: installs seen in the last 24 hours. `live`
 * is false when the numbers are the hand-maintained COHORT floors (render
 * them with a "+"); true when they are exact live counts. Null = unknown,
 * rendered as an em-dash, never guessed.
 */
export type CohortStats = {
  users: number | null;
  countries: number | null;
  active24h: number | null;
  /** Installs first seen in the last 24 hours ("started using"). Null until
   *  the public_stats() revision that returns `new_24h` is migrated. */
  new24h: number | null;
  live: boolean;
};

/** Raw RPC reply; every field is checked before it is trusted. */
type PublicStats = {
  users?: unknown;
  countries?: unknown;
  active_24h?: unknown;
  active_30d?: unknown;
  new_24h?: unknown;
};

const isCount = (v: unknown): v is number =>
  typeof v === "number" && Number.isInteger(v) && v >= 0;

export async function getCohortStats(): Promise<CohortStats> {
  const fallback: CohortStats = {
    users: COHORT.users,
    countries: COHORT.countries,
    active24h: null,
    new24h: null,
    live: false,
  };

  const url = process.env.SUPABASE_URL;
  const key = process.env.SUPABASE_ANON_KEY;
  if (!url || !key) return fallback;

  try {
    const res = await fetch(
      `${url.replace(/\/+$/, "")}/rest/v1/rpc/public_stats`,
      {
        method: "POST",
        headers: {
          apikey: key,
          Authorization: `Bearer ${key}`,
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: "{}",
        next: { revalidate: 3600 },
        signal: AbortSignal.timeout(4000),
      },
    );
    // 404 until public_stats() is migrated; any non-200 keeps the floors.
    if (!res.ok) return fallback;

    const data = (await res.json()) as PublicStats | null;
    if (
      !data ||
      !isCount(data.users) ||
      !isCount(data.countries) ||
      !isCount(data.active_24h)
    ) {
      return fallback;
    }

    return {
      users: data.users,
      countries: data.countries,
      active24h: data.active_24h,
      // Optional: older deployments of the function don't return it yet.
      new24h: isCount(data.new_24h) ? data.new_24h : null,
      live: true,
    };
  } catch {
    return fallback;
  }
}

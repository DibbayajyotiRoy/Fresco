import { NextResponse, type NextRequest } from "next/server";

/**
 * Coarse geolocation for the desktop app's telemetry, resolved from headers
 * Vercel injects at the edge.
 *
 * Why here and not in the database: Postgres only sees `CF-IPCountry`, which
 * Cloudflare fills in for free but which stops at the country. City and region
 * need `CF-IPCity`, a Cloudflare Enterprise header that Supabase does not
 * enable. Vercel populates the equivalents on every plan, and this site is
 * already deployed there — so the cheapest correct answer was a route on
 * infrastructure that already exists rather than a new service or a
 * rate-limited third-party API.
 *
 * The IP is never read, logged, or returned. Vercel resolves it before this
 * function runs; the handler only ever sees the resulting place names, and the
 * caller only ever learns about itself.
 *
 * Doubles as the diagnostic for what this deployment actually provides:
 *
 *     curl -s https://fresco.dibbayajyoti.com/api/geo
 *
 * `null` for a field means Vercel did not supply it, which is a real answer and
 * is reported as such rather than guessed at.
 */

/** Vercel percent-encodes header values, so "New Delhi" arrives as "New%20Delhi". */
function decode(value: string | null): string | null {
  if (!value) return null;
  let text = value;
  try {
    text = decodeURIComponent(value);
  } catch {
    // A malformed escape is not worth failing the whole request over: fall
    // back to the raw value, which is still usable, just uglier.
  }
  text = text.trim();
  return text.length > 0 ? text : null;
}

/** Two-letter country, or null. 'XX'/'T1' are Cloudflare/Vercel's "unknown"
 *  and Tor markers and are deliberately not passed through as places. */
function country(value: string | null): string | null {
  const code = decode(value)?.toUpperCase() ?? null;
  if (!code || code === "XX" || code === "T1" || !/^[A-Z]{2}$/.test(code)) {
    return null;
  }
  return code;
}

export function GET(request: NextRequest) {
  const h = request.headers;

  const body = {
    country: country(h.get("x-vercel-ip-country")),
    region: decode(h.get("x-vercel-ip-country-region")),
    city: decode(h.get("x-vercel-ip-city")),
    timezone: decode(h.get("x-vercel-ip-timezone")),
    // Deliberately absent from the response even though Vercel supplies them:
    // x-vercel-ip-latitude / x-vercel-ip-longitude. Coordinates are a
    // different category of data from a city name, nothing in Fresco has a use
    // for them, and the cheapest way to guarantee they are never stored is to
    // never hand them out. Do not add them without a reason that survives
    // being written into TERMS.md.
  };

  return NextResponse.json(body, {
    headers: {
      // Per-visitor by definition: a cached response would hand one user's
      // city to the next one.
      "cache-control": "no-store, max-age=0",
      // Called by the desktop app, which is not a browser origin.
      "access-control-allow-origin": "*",
    },
  });
}

# Fresco Admin

A private admin dashboard for **Fresco** (a Linux live-wallpaper app). Built
with Next.js (App Router), TypeScript, Tailwind CSS, and shadcn/ui.

## Pages

- **Overview** (`/`) — a KPI strip (stars, total downloads, latest version,
  feedback count, satisfaction, open issues), a downloads-per-release chart and
  release table, OS/app-version breakdowns derived from feedback, and recent
  feedback plus latest notifications side by side.
- **Catalog** (`/catalog`) — the curated wallpaper catalog: list, add,
  publish / unpublish and delete `catalog_items`. Metadata only; the media
  itself lives on GitHub Releases or R2.
- **Notifications** (`/notifications`) — create / edit / publish / delete the
  changelog and announcement messages pushed to the app, with a total and a
  published count.
- **Feedback** (`/feedback`) — browse anonymous 👍/👎 ratings and comments, with
  totals, a satisfaction figure and a sentiment filter.
- **Support** (`/support`) — the anonymous two-way support inbox: threads with
  their messages, counts for "waiting on you" / open / threads / messages, and
  replying or setting a thread's status from the thread view. Tickets are
  generated separately from telemetry, so a conversation can never be joined to
  an install.
- **Usage** (`/usage`) — anonymous telemetry. Reach (unique users, active
  today/7d/30d), a 3D globe of where installs run alongside country, region and
  city breakdowns, consent split, downloads vs known installs, per-feature event
  counts over 7d/30d with a link-add breakdown and per-install usage depth, and
  environment breakdowns (distro, compositor, session, backend, decode,
  monitors).
- **Reliability** (`/reliability`) — error reports from the last 30 days,
  grouped by kind × version, with counts, last-seen, latest detail, and a
  volume-derived severity. Stat cards cover errors 24h/7d and affected installs.
- **Issues** (`/issues`) — open GitHub issues for `GITHUB_REPO` (pull requests
  filtered out), with an open count and a with-comments count.

## Setup

1. Copy the env template and fill in the secret:

   ```bash
   cp .env.local.example .env.local
   ```

2. Open `.env.local` and set `SUPABASE_SERVICE_ROLE_KEY` to your Supabase
   **service_role** secret (Supabase dashboard → Project Settings → API).

   ```env
   NEXT_PUBLIC_SUPABASE_URL=https://mmoxgmvrpiaflfnsrynx.supabase.co
   SUPABASE_SERVICE_ROLE_KEY=   # service_role secret — paste it here
   GITHUB_REPO=DibbayajyotiRoy/fresco
   GITHUB_TOKEN=                 # optional, raises the GitHub rate limit
   ```

3. Install and run:

   ```bash
   npm install
   npm run dev
   ```

   Open http://localhost:3000.

### `GITHUB_TOKEN`

Optional. Without it, GitHub caps unauthenticated requests at 60/hour; a token
raises that to 5000/hour. An **invalid or expired** token used to be worse than
no token at all — a single stale value blanked stars, downloads and the issue
list at once with `401 Unauthorized`. The data layer now retries the same
request without the `Authorization` header whenever GitHub rejects the
credential, so a stale token degrades to the unauthenticated rate limit and the
numbers still render.

## Security note

> ⚠️ The **service_role** key bypasses Row Level Security and has full
> read/write access to your database.
>
> - It is read server-side only (`src/lib/supabase-admin.ts`, guarded by
>   `import "server-only"`) and is **never** exposed to the browser.
> - It is **not** prefixed with `NEXT_PUBLIC_`.
> - `.env.local` is gitignored — never commit your real key.

The dashboard has no authentication layer of its own, so any deployment must be
access-controlled by whatever hosts it.

If `SUPABASE_SERVICE_ROLE_KEY` is missing, the app does **not** crash — pages
render a "Set SUPABASE_SERVICE_ROLE_KEY in .env.local" empty state instead.

## Data model (Supabase)

Full definitions live in `../supabase/schema.sql`. The tables this dashboard
reads:

| Table              | Holds                                                                      |
| ------------------ | -------------------------------------------------------------------------- |
| `feedback`         | Anonymous 👍/👎 ratings with an optional comment, app version, OS, timezone, locale |
| `notifications`    | Changelog / announcement messages pushed to the app, with `kind` and a published flag |
| `catalog_items`    | Curated wallpaper metadata — title, category, content type, license, media URLs |
| `installs`         | One row per install: version, distro, compositor, session, backend, decode, monitors, source, channel, first/last seen |
| `events`           | Feature-usage events (`wallpaper_set`, `add_from_link`, …) with a `props` payload |
| `errors`           | Error reports — kind, detail, version, reporting install                    |
| `daily_country`    | Per-day country × version × channel ping counts for installs that declined full telemetry |
| `support_threads`  | One anonymous support conversation per ticket: attached environment, status, unread flags |
| `support_messages` | Individual messages in a thread, from either the user or the maintainer     |

Downloads come from the GitHub Releases API for `GITHUB_REPO`, summing
`assets[].download_count` per release (fetched fresh on every render, no
cache). Stars, forks, watchers and open issues come from the same API.

## Geography data

The Usage globe needs country polygons keyed by the same two-letter code the
telemetry stores, which no single package ships. `scripts/build-geo.mjs`
performs that join offline and writes two artefacts:

- `public/geo/countries.geojson` — world polygons tagged with ISO alpha-2
- `src/lib/countries.generated.ts` — alpha-2 → `{ name, lat, lon }`

```bash
node scripts/build-geo.mjs
```

Both outputs are committed, so a normal build never touches the network. Re-run
the script only when you want to refresh the source data.

## Real-time updates

The dashboard stays current with **near-real-time polling**: a small client
component (`src/components/auto-refresh.tsx`) calls the App Router's soft
`router.refresh()` on an interval (default 10s), which re-runs the server
components with fresh data — no full reload, scroll position preserved. The
pages are `force-dynamic` / `revalidate = 0`, and the GitHub fetch is
`cache: "no-store"`, so each refresh reflects live counts. The Support nav item
polls `/api/support/unread` separately for its badge count; that route is also
`force-dynamic` and answers 200 with zeros on failure, so a failed query cannot
break the page chrome.

Polling (rather than true push) is the correct secure choice here: the data is
read **server-side** with the Supabase `service_role` key, which must never
reach the browser. Real push (Supabase Realtime) would require exposing an
anon/client key with Row Level Security + auth, which this dashboard
deliberately does not do.

## Scripts

| Command                    | Description                                  |
| -------------------------- | -------------------------------------------- |
| `npm run dev`              | Start the dev server                         |
| `npm run build`            | Production build                             |
| `npm run start`            | Run the production build                     |
| `npm run lint`             | Lint                                         |
| `node scripts/build-geo.mjs` | Regenerate the committed geography artefacts |

## Notable dependencies

`three` and `react-globe.gl` power the 3D globe on the Usage page. It is
rendered client-side only (`next/dynamic` with `ssr: false`) because
`react-globe.gl` mounts a WebGL canvas and touches `window` on import.

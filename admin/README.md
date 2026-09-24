# Fresco Admin

A private admin dashboard for **Fresco** (a Linux live-wallpaper app). Built
with Next.js (App Router), TypeScript, Tailwind CSS, and shadcn/ui.

## Pages

- **Overview** (`/`) — a KPI strip (users, stars, total downloads, latest
  version, feedback count, satisfaction, open issues), a downloads-per-release
  chart and release table, OS/app-version breakdowns derived from feedback,
  and recent feedback plus latest notifications side by side. "Users" is real
  telemetry (installs that have checked in); the GitHub figures next to it
  count something different (asset fetches) and are deliberately not merged
  into one number — see the Usage page's "Downloads vs installs" band.
- **Users** (`/users`) — the main "how many people use Fresco, and are they
  staying" page: a KPI strip (total installs ever, active 7d, checked in
  today, lapsed >30d, new 30d), a 90-day chart of daily check-ins and new
  installs, an **activation funnel** (installs ever → activated → **retained**
  → active 7d/30d — see "What counts as a user" below), a lifecycle breakdown
  (active/idle/lapsed) with DAU/WAU/MAU and stickiness, a D1/D7/D30 retention
  cohort table by first-install week, and a searchable, filterable, sortable,
  paginated table of every install. Filters (status, country, version,
  channel, install-id prefix) and pagination are URL state — the table works
  with plain links and a GET form, no client JS required — and the same
  filters apply to the CSV export.
- **Countries** (`/countries`) — the full per-country table: every country an
  install has ever checked in from, with total / active-7d / checked-in-today
  / new-30d / lapsed / lapse-rate / top-version per country, plus a region
  rollup. The Usage page's globe stays a top-12 summary; this is the
  drilldown.
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

**The CSV export routes (`/api/export/*`) carry the same access model as
every page and every other route handler here** — they read through the same
`server-only` service-role client, add no new capability, and are exactly as
exposed as `/users` or `/api/support/unread`. They are not a bigger attack
surface than the dashboard already is; whatever access-controls the front
door (a reverse-proxy auth layer, a private network, a host-level login)
controls these routes too. If you deploy this dashboard publicly without such
a layer, every page **and** every export is already readable by anyone with
the URL — that was true before this change and is unchanged by it.

## Data model (Supabase)

Full definitions live in `../supabase/schema.sql`. The tables this dashboard
reads:

| Table              | Holds                                                                      |
| ------------------ | -------------------------------------------------------------------------- |
| `feedback`         | Anonymous 👍/👎 ratings with an optional comment, app version, OS, timezone, locale |
| `notifications`    | Changelog / announcement messages pushed to the app, with `kind` and a published flag |
| `catalog_items`    | Curated wallpaper metadata — title, category, content type, license, media URLs |
| `installs`         | One row per install: version, distro, compositor, session, backend, decode, monitors, source, channel, first/last seen |
| `install_days`     | One row per install per calendar day it checked in — the check-in history `installs` itself does not keep (its `first_seen`/`last_seen` are overwritten every heartbeat). Powers /users' daily check-ins, DAU/WAU/MAU, and retention cohorts. See "Migrations" below. |
| `events`           | Feature-usage events (`wallpaper_set`, `add_from_link`, …) with a `props` payload |
| `errors`           | Error reports — kind, detail, version, reporting install                    |
| `daily_country`    | Per-day country × version × channel ping counts for installs that declined full telemetry |
| `support_threads`  | One anonymous support conversation per ticket: attached environment, status, unread flags |
| `support_messages` | Individual messages in a thread, from either the user or the maintainer     |

Downloads come from the GitHub Releases API for `GITHUB_REPO`, summing
`assets[].download_count` per release (fetched fresh on every render, no
cache). Stars, forks, watchers and open issues come from the same API.

## What counts as a user

A **user is one `install_id`** (`src/telemetry.rs`'s `install_id()`): a
random id generated once and stored on disk next to the app's `config.toml`
(`~/.config/fresco/install-id` on a typical Linux desktop; see
`install_id_path()`). What that means in practice:

- **Re-running `install.sh` or upgrading/reinstalling the `.deb` does NOT
  create a new user.** The config directory survives a reinstall, so the same
  id is read back and reused — the install row's `first_seen` stays put and
  only `last_seen` moves.
- **A new id genuinely appears for**: a different OS user account on the same
  machine (each has its own `~/.config`), a different machine, deleting
  `~/.config/fresco` by hand, and the **Flatpak build**, which runs in a
  sandbox with its own, separate config directory — so one person running
  both the `.deb` and the Flatpak build shows up as two installs. That is a
  real limit of an anonymous, non-cross-referenced id, not a bug.
- **GitHub download counts are not users.** The Overview page's "Installs /
  download" figure exists specifically to keep that visible: the one-line
  installer re-downloads the `.deb` on every run, including retries and
  reinstalls, so download counts are always noisier and larger than install
  counts.

That single id, however, cannot by itself tell a real user from a one-off
`curl | sh` on someone's throwaway VM, a CI runner, or a five-second try that
was immediately uninstalled — all of those write exactly one install row and
never check in again, identically to a genuine user on day one. **Retained**
(an install that checked in on a *second* distinct calendar day — see
`isRetained` in `src/lib/install-analytics.ts`) is the line this dashboard
draws between the two, and is called out as the headline "real users" figure
on /users' activation funnel and as its own column on /countries. It is a
filter, not a perfect one: a genuine user's very first day always looks
identical to a one-off try until they come back, so "Retained" is always a
lower bound that catches up a day or more after the fact — worded as such
everywhere it appears.

## Migrations

This project has no migration tool (no dated-file runner, no `supabase
migrate`) — `../supabase/schema.sql` is a single idempotent file you paste
into the Supabase SQL editor, and every change to it is written so re-running
the whole file on any prior state is safe. New changes are still added there
first.

The one exception, for anything that needs its own name and date to point the
owner at (rather than "re-paste the whole schema"), is
`../supabase/migrations/`. As of this writing it holds one file:

- `2026-09-24_install_days.sql` — adds `public.install_days` (see the table
  above) and redefines `register_install` / `register_install_minimal` to
  also write a check-in row. Its content is also appended verbatim to the
  tail of `schema.sql`, so pasting the whole file still works; this copy
  exists so it has a name to reference. **The owner needs to run this in the
  Supabase SQL editor** — nothing in this repo runs it automatically.

## CSV export

`/api/export/installs.csv`, `/api/export/countries.csv` and
`/api/export/daily.csv` (route handlers under `src/app/api/export/`) stream
the same data the Users/Countries pages show, as CSV:

- `installs.csv` accepts the exact same `status` / `country` / `version` /
  `channel` / `q` query params as the /users table filters — "export what I'm
  looking at" downloads exactly the filtered set.
- `countries.csv` is the full per-country table, no filters.
- `daily.csv?days=90` is one row per day: check-ins, new installs, and which
  table the check-in figure came from (`install_days` or the older
  `daily_country` fallback — see `src/lib/install-analytics.ts`).

All three: UTF-8 with a BOM (so Excel opens non-ASCII country/city names
correctly), CRLF row endings, proper quoting, and any cell starting with
`= + - @` prefixed with a leading apostrophe to defuse spreadsheet formula
injection (`src/lib/csv.ts`). See the security note above for their access
model — unchanged from every other route here.

## Fixtures (dev only)

Set `ADMIN_FIXTURES=1` (and leave `NODE_ENV` unset or `development`) to render
every page from ~2,000 deterministic synthetic installs across ~40 countries,
with a full `install_days` history — no Supabase project needed. Useful for
running, screenshotting or testing the UI without credentials.

```bash
ADMIN_FIXTURES=1 npm run dev
```

`src/lib/fixtures.ts` generates the data with a fixed seed (same output every
run) and is checked at the call site in `src/lib/data.ts` for **both**
`ADMIN_FIXTURES === "1"` **and** `NODE_ENV !== "production"`, so it cannot
turn on in a deployed build by accident. Pages that read other tables (GitHub
issues/releases, feedback, notifications, support) still need their own env
or degrade to their existing empty/error state — fixtures cover installs /
install_days / daily_country only.

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
| `npm test`                 | Run the pure aggregation/CSV unit tests (Node's built-in test runner, no added test framework) |
| `node scripts/build-geo.mjs` | Regenerate the committed geography artefacts |

## Notable dependencies

`three` and `react-globe.gl` power the 3D globe on the Usage page. It is
rendered client-side only (`next/dynamic` with `ssr: false`) because
`react-globe.gl` mounts a WebGL canvas and touches `window` on import.

# Fresco Admin

A private, local-only admin dashboard for **Fresco** (a Linux live-wallpaper
app). Built with Next.js (App Router), TypeScript, Tailwind CSS, and shadcn/ui.

It does three things:

- **Overview** — total downloads (from GitHub release assets), feedback
  satisfaction, published-notification counts, a downloads-per-release chart,
  and recent activity.
- **Notifications** — create / edit / publish / delete the changelog and
  announcement messages pushed to the app.
- **Feedback** — browse anonymous 👍/👎 ratings and comments, with a
  sentiment filter and summary cards.

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

## Security note

> ⚠️ The **service_role** key bypasses Row Level Security and has full
> read/write access to your database. It must stay **local**:
>
> - It is read server-side only (`src/lib/supabase-admin.ts`, guarded by
>   `import "server-only"`) and is **never** exposed to the browser.
> - It is **not** prefixed with `NEXT_PUBLIC_`.
> - `.env.local` is gitignored — never commit your real key, and do not deploy
>   this dashboard to a public host with that key set.

If `SUPABASE_SERVICE_ROLE_KEY` is missing, the app does **not** crash — pages
render a "Set SUPABASE_SERVICE_ROLE_KEY in .env.local" empty state instead.

## Data model (Supabase)

- `feedback` — `id`, `created_at`, `rating` (`-1` = 👎, `1` = 👍), `comment`,
  `app_version`, `os`.
- `notifications` — `id`, `created_at`, `title`, `body`, `url`, `published`.

Downloads come from the GitHub Releases API for `GITHUB_REPO`, summing
`assets[].download_count` per release (fetched fresh on every render, no
cache).

## Real-time updates

The dashboard stays current with **near-real-time polling**: a small client
component (`src/components/auto-refresh.tsx`) calls the App Router's soft
`router.refresh()` on an interval (default 10s), which re-runs the server
components with fresh data — no full reload, scroll position preserved. The
pages are `force-dynamic` / `revalidate = 0`, and the GitHub fetch is
`cache: "no-store"`, so each refresh reflects live counts. A pulsing **"Live"**
pill in the page header signals this.

Polling (rather than true push) is the correct secure choice here: the data is
read **server-side** with the Supabase `service_role` key, which must never
reach the browser. Real push (Supabase Realtime) would require exposing an
anon/client key with Row Level Security + auth, which this local-only admin
deliberately does not do.

## Scripts

| Command         | Description                  |
| --------------- | ---------------------------- |
| `npm run dev`   | Start the dev server         |
| `npm run build` | Production build             |
| `npm run start` | Run the production build     |
| `npm run lint`  | Lint                         |

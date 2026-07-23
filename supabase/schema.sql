-- Fresco — Supabase schema for anonymous feedback + admin notifications.
-- Paste this into the Supabase dashboard → SQL Editor → Run.
--
-- Security model:
--   * The app uses the publishable (anon) key. RLS lets the anon role only
--     INSERT feedback and SELECT published notifications — it can never read
--     other users' feedback.
--   * Your admin dashboard uses the service_role (secret) key, which bypasses
--     RLS, to read all feedback and publish notifications. Keep that key local.

-- ── Feedback ────────────────────────────────────────────────────────────────
create table if not exists public.feedback (
    id          uuid primary key default gen_random_uuid(),
    created_at  timestamptz not null default now(),
    rating      smallint    not null check (rating in (-1, 1)),  -- 👎 / 👍
    comment     text,
    app_version text,
    os          text,
    -- Coarse "where are our users" columns (no identifiers): IANA timezone
    -- ("Asia/Kolkata") and locale ("en_IN.UTF-8"), sent by the client.
    timezone    text,
    locale      text
);

-- Additive columns for projects created before `timezone`/`locale` existed.
-- Run this BEFORE shipping a client that sends them: PostgREST rejects inserts
-- with unknown columns, which would break feedback submission entirely.
alter table public.feedback add column if not exists timezone text;
alter table public.feedback add column if not exists locale   text;

alter table public.feedback enable row level security;

-- Anyone with the anon key may submit feedback (but not read it).
drop policy if exists "anon can insert feedback" on public.feedback;
create policy "anon can insert feedback"
    on public.feedback for insert
    to anon
    with check (rating in (-1, 1));

grant insert on public.feedback to anon;

-- ── Notifications (admin → app) ──────────────────────────────────────────────
-- `kind` distinguishes a plain announcement ('info') from an auto-generated
-- update prompt ('update'); `version` carries the released version for 'update'
-- rows so the client can compare it against its own and self-update. `url` is
-- the link the notification opens (release page, or a direct asset for updates).
create table if not exists public.notifications (
    id          uuid primary key default gen_random_uuid(),
    created_at  timestamptz not null default now(),
    title       text    not null,
    body        text    not null,
    url         text,
    kind        text    not null default 'info' check (kind in ('info', 'update')),
    version     text,
    published   boolean not null default true
);

-- Additive columns for projects created before `kind`/`version` existed.
alter table public.notifications add column if not exists kind    text not null default 'info';
alter table public.notifications add column if not exists version text;
do $$ begin
    alter table public.notifications
        add constraint notifications_kind_check check (kind in ('info', 'update'));
exception when duplicate_object then null; end $$;

alter table public.notifications enable row level security;

-- The app may read only published notifications.
drop policy if exists "anon can read published notifications" on public.notifications;
create policy "anon can read published notifications"
    on public.notifications for select
    to anon
    using (published = true);

grant select on public.notifications to anon;

-- Make sure the anon role can use the schema (usually already granted).
grant usage on schema public to anon;

-- ── Realtime (event-driven push to clients) ──────────────────────────────────
-- The desktop app subscribes to row INSERTs over a Realtime websocket instead
-- of polling. Add the table to the realtime publication so inserts are pushed.
-- (RLS above still applies to what the anon role is allowed to receive.)
do $$ begin
    alter publication supabase_realtime add table public.notifications;
exception when duplicate_object then null; end $$;

-- ── Wallpaper catalog (curated — ROADMAP 3.1) ────────────────────────────────
-- Metadata only: media files live on a zero-egress host (GitHub Releases of a
-- dedicated wallpapers repo, or Cloudflare R2) — NEVER Supabase storage (the
-- free-tier egress cap dies at ~100 installs of one 20 MB video).
-- `content_type` is reserved for shaders from day 1 (ROADMAP 6.1) so they slot
-- in with zero migration. `license` is NOT NULL: every item legally attributable.
create table if not exists public.catalog_items (
    id            uuid primary key default gen_random_uuid(),
    created_at    timestamptz not null default now(),
    content_type  text not null default 'video'
                  check (content_type in ('video', 'image', 'shader')),
    title         text not null,
    category      text not null default 'other',
    tags          text[] not null default '{}',
    media_url     text not null,
    thumb_url     text,
    size_bytes    bigint not null default 0,
    width         integer,
    height        integer,
    duration_s    real,
    checksum      text,
    license       text not null,
    author        text not null default '',
    source_url    text,
    published     boolean not null default false,
    install_count bigint not null default 0
);

alter table public.catalog_items enable row level security;

-- The app may read only published items.
drop policy if exists "anon can read published catalog items" on public.catalog_items;
create policy "anon can read published catalog items"
    on public.catalog_items for select
    to anon
    using (published);

grant select on public.catalog_items to anon;

-- Engagement measurement with ZERO client telemetry: the app calls this RPC
-- once per completed download. SECURITY DEFINER so anon can bump the counter
-- without update rights on the table.
create or replace function public.catalog_count_install(item uuid)
returns void
language sql
security definer
set search_path = public
as $$
    update public.catalog_items
       set install_count = install_count + 1
     where id = item and published;
$$;

grant execute on function public.catalog_count_install(uuid) to anon;

-- ── Anonymous telemetry (installs / events / errors) ─────────────────────────
-- The app writes these with the anon key over REST. Write-only for anon: it
-- may INSERT (and, for installs, UPDATE — required by PostgREST upsert with
-- merge-duplicates) but never SELECT. The admin dashboard reads them with the
-- service_role key, which bypasses RLS.

-- One row per install, keyed by a random client-generated id. The app upserts
-- on launch to refresh `last_seen` and environment columns.
create table if not exists public.installs (
    install_id    text primary key,
    version       text,
    distro        text,
    compositor    text,
    session       text,
    backend       text,
    decode        text,
    monitor_count int,
    -- UTM-style download attribution, persisted by the install one-liner
    -- (website/github/reddit/…). Null for installs predating the tagging.
    source        text,
    -- Packaging channel, detected at runtime: deb / flatpak / other.
    channel       text,
    first_seen    timestamptz not null default now(),
    last_seen     timestamptz not null default now()
);

-- Additive columns for projects created before `source`/`channel` existed.
-- MUST be run before (or with) a client that sends them: PostgREST rejects
-- inserts naming unknown columns, so without these every heartbeat 400s and
-- the installs table stays empty while events keep arriving — which is
-- exactly what happened between 1.1.1 and this migration.
alter table public.installs add column if not exists source  text;
alter table public.installs add column if not exists channel text;

alter table public.installs enable row level security;

-- Installs are UPSERTED (one row per install, refreshed every heartbeat), not
-- plain-inserted. PostgREST's merge-duplicates compiles to
-- INSERT ... ON CONFLICT DO UPDATE, which Postgres will only execute if the
-- caller can READ the target row — even on an empty table with no conflict.
-- This table is deliberately unreadable by anon, so a direct anon upsert fails
-- RLS ("new row violates row-level security policy for table installs"). A
-- plain insert works; the upsert does not. That is the whole reason installs
-- stayed empty while events (plain inserts) flowed.
--
-- Rather than open the table up for SELECT (the anon key ships in every binary,
-- so that would make the install list world-readable), the upsert is done
-- inside a SECURITY DEFINER function. It runs as its owner, who is not subject
-- to RLS, so the internal read/write just works — while anon keeps NO direct
-- rights on the table at all: it cannot read installs, and cannot write
-- arbitrary rows; the only thing it may do is call this one function.
create or replace function public.register_install(
    p_install_id    text,
    p_version       text default null,
    p_distro        text default null,
    p_compositor    text default null,
    p_session       text default null,
    p_backend       text default null,
    p_decode        text default null,
    p_monitor_count int  default null,
    p_source        text default null,
    p_channel       text default null
) returns void
language sql
security definer
set search_path = ''
as $$
    insert into public.installs (
        install_id, version, distro, compositor, session,
        backend, decode, monitor_count, source, channel, last_seen
    ) values (
        p_install_id, p_version, p_distro, p_compositor, p_session,
        p_backend, p_decode, p_monitor_count, p_source, p_channel, now()
    )
    on conflict (install_id) do update set
        version       = excluded.version,
        distro        = excluded.distro,
        compositor    = excluded.compositor,
        session       = excluded.session,
        backend       = excluded.backend,
        decode        = excluded.decode,
        monitor_count = excluded.monitor_count,
        source        = excluded.source,
        channel       = excluded.channel,
        last_seen     = now();
$$;

-- Calling the function is the ONLY way anon touches installs. Strip the
-- default PUBLIC execute grant first, then grant it to anon alone.
revoke all on function public.register_install(
    text, text, text, text, text, text, text, int, text, text
) from public;
grant execute on function public.register_install(
    text, text, text, text, text, text, text, int, text, text
) to anon;

-- Retire the direct-write policies/grants: the upsert they were meant to serve
-- cannot work under RLS (see above), and the function fully replaces them.
drop policy if exists "anon can insert installs" on public.installs;
drop policy if exists "anon can update installs" on public.installs;
revoke insert, update on public.installs from anon;
-- RLS already denies anon every row (no SELECT policy), so this is inert
-- belt-and-suspenders, but it matches the intent: anon has NO direct table
-- rights — only execute on register_install().
revoke select on public.installs from anon;

create index if not exists installs_last_seen_idx
    on public.installs (last_seen);

-- Feature-usage events ("wallpaper_set", "schedule_created", …).
create table if not exists public.events (
    id         bigint generated always as identity primary key,
    install_id text,
    name       text not null,
    props      jsonb,
    version    text,
    created_at timestamptz not null default now()
);

alter table public.events enable row level security;

-- Anyone with the anon key may record events (but not read them).
drop policy if exists "anon can insert events" on public.events;
create policy "anon can insert events"
    on public.events for insert
    to anon
    with check (true);

grant insert on public.events to anon;

create index if not exists events_name_created_at_idx
    on public.events (name, created_at);

-- Error reports (crash kinds, backend failures, …).
create table if not exists public.errors (
    id         bigint generated always as identity primary key,
    install_id text,
    kind       text not null,
    detail     text,
    version    text,
    created_at timestamptz not null default now()
);

alter table public.errors enable row level security;

-- Anyone with the anon key may report errors (but not read them).
drop policy if exists "anon can insert errors" on public.errors;
create policy "anon can insert errors"
    on public.errors for insert
    to anon
    with check (true);

grant insert on public.errors to anon;

create index if not exists errors_kind_created_at_idx
    on public.errors (kind, created_at);

-- ── Download attribution (added after the initial telemetry tables) ──────────
-- `source` is the UTM-style tag the install one-liner persisted (website /
-- github / reddit / …); `channel` is the runtime-detected packaging
-- (deb / flatpak / other). Idempotent: safe to run on any state.
alter table public.installs add column if not exists source  text;
alter table public.installs add column if not exists channel text;

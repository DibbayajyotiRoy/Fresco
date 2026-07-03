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
    os          text
);

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

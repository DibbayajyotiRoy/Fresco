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

-- ── Country, resolved at the edge (never from a stored IP) ───────────────────
-- Supabase serves PostgREST from behind Cloudflare, which resolves the client
-- IP to a country and forwards it as the `CF-IPCountry` header. Reading that
-- header means the IP is geolocated *before* it reaches us and is never read,
-- logged, or stored by anything we own: the only thing that lands in the
-- database is a two-letter code, which cannot be narrowed to a person, a city,
-- or a network.
--
-- Deliberately server-side: the client never sends a country and cannot spoof
-- one, and there is no geo database of ours to keep current.
--
-- Returns null when the header is absent (direct-to-Postgres calls, local
-- development, or a proxy that strips it) and for 'XX' / 'T1', which
-- Cloudflare uses for unknown origins and the Tor network. Null is a real
-- answer here and is rendered as "Unknown" rather than guessed at.
create or replace function public.request_country()
returns text
language plpgsql
stable
as $$
declare
    headers json;
    code    text;
begin
    -- `request.headers` is unset outside PostgREST; the `true` makes that a
    -- null rather than an error, so direct SQL calls still work.
    headers := nullif(current_setting('request.headers', true), '')::json;
    if headers is null then
        return null;
    end if;

    code := coalesce(
        headers ->> 'cf-ipcountry',      -- Cloudflare, what Supabase sits behind
        headers ->> 'x-vercel-ip-country' -- in case it is ever fronted by Vercel
    );

    code := upper(nullif(trim(code), ''));
    if code is null or code in ('XX', 'T1') or code !~ '^[A-Z]{2}$' then
        return null;
    end if;
    return code;
end;
$$;

-- Full-consent installs carry their country on the existing row.
alter table public.installs add column if not exists country text;

create index if not exists installs_country_idx on public.installs (country);

-- Re-declare register_install with country stamped from the edge header.
-- Note there is no p_country parameter: the value is not the client's to send.
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
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_country text := public.request_country();
begin
    insert into public.installs (
        install_id, version, distro, compositor, session,
        backend, decode, monitor_count, source, channel, country, last_seen
    ) values (
        p_install_id, p_version, p_distro, p_compositor, p_session,
        p_backend, p_decode, p_monitor_count, p_source, p_channel,
        v_country, now()
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
        -- Keep the last known country if this ping arrived without a header,
        -- so a proxy that strips it does not blank an otherwise good row.
        country       = coalesce(excluded.country, public.installs.country),
        last_seen     = now();
end;
$$;

revoke all on function public.register_install(
    text, text, text, text, text, text, text, int, text, text
) from public;
grant execute on function public.register_install(
    text, text, text, text, text, text, text, int, text, text
) to anon;

-- ── Country-only cohort (users who declined full statistics) ─────────────────
-- A bare tally. There is no install id, no session id, no IP, and no row per
-- user anywhere in this table: a ping does nothing but add 1 to an integer on
-- a (day, country, version, channel) bucket that may already be shared with
-- thousands of other people. Nothing here can be traced back to an install,
-- and two pings from the same machine on the same day are indistinguishable
-- from two pings by different machines.
--
-- What that costs: this counts *daily pings*, not distinct users. The client
-- throttles itself to one ping per ~20h (the same marker the full heartbeat
-- uses), so a day's total is a close proxy for daily-active installs in this
-- cohort, but it cannot be de-duplicated across days into a monthly figure the
-- way the consenting cohort can. That is the deliberate trade: no identifier
-- means no cross-day linkage, in either direction.
-- Every key column is NOT NULL with a sentinel default, because a PRIMARY KEY
-- implies NOT NULL on its columns: leaving `country` nullable here would make
-- Postgres reject the exact pings whose country did not resolve, which are the
-- ones most worth counting (they are still a real user). '??' is that case,
-- and the dashboard renders it as "Unknown".
create table if not exists public.daily_country (
    day        date not null default current_date,
    country    text   not null default '??',
    version    text   not null default '',
    channel    text   not null default '',
    pings      bigint not null default 0,
    primary key (day, country, version, channel)
);

alter table public.daily_country enable row level security;

-- No direct rights for anon at all: the counter RPC is the only way in, and it
-- is write-only in effect (it returns void and never exposes a count).
revoke all on public.daily_country from anon;

create or replace function public.count_anonymous_ping(
    p_version text default null,
    p_channel text default null
) returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_country text := public.request_country();
begin
    insert into public.daily_country as d (day, country, version, channel, pings)
    values (
        current_date,
        coalesce(v_country, '??'),
        coalesce(p_version, ''),
        coalesce(p_channel, ''),
        1
    )
    on conflict (day, country, version, channel) do update
        set pings = d.pings + 1;
end;
$$;

revoke all on function public.count_anonymous_ping(text, text) from public;
grant execute on function public.count_anonymous_ping(text, text) to anon;

create index if not exists daily_country_day_idx on public.daily_country (day);

-- Diagnostic: returns the country this connection resolves to, so you can
-- confirm the edge header actually arrives in your project before trusting the
-- numbers. Returns null if it does not — see request_country() above.
--   curl -s "$URL/rest/v1/rpc/whats_my_country" -X POST \
--        -H "apikey: $ANON" -H "Authorization: Bearer $ANON"
create or replace function public.whats_my_country()
returns text
language sql
stable
as $$ select public.request_country(); $$;

grant execute on function public.whats_my_country() to anon;

-- ── Anonymous two-way support threads ────────────────────────────────────────
-- Lets the maintainer answer a specific user without either side knowing who
-- the other is. The user sees "Fresco maintainer"; the maintainer sees a
-- ticket, a message, and whatever environment the user chose to attach.
--
-- Identity model: a thread is addressed by a random `ticket` uuid that the
-- client generates and keeps in its state dir. Knowing the ticket is what
-- authorises reading and writing that thread — a capability, like an unguessable
-- link. 122 bits of entropy, so it cannot be found by trying.
--
-- The ticket is deliberately NOT the telemetry install id. Support has to work
-- for users who declined statistics, and a support conversation must never
-- become a way to attach a face to a telemetry row. The two ids are generated
-- separately, live in different files, and are never sent in the same request.
--
-- anon gets no table rights at all: the three RPCs below are the entire API,
-- and each one is scoped to a single ticket.
create table if not exists public.support_threads (
    ticket      uuid primary key,
    created_at  timestamptz not null default now(),
    last_at     timestamptz not null default now(),
    -- What the user chose to attach (app version, distro, desktop). Free text
    -- assembled by the client from the same fields as a bug report, capped.
    env         text,
    app_version text,
    -- Maintainer-side workflow only; never shown to the user.
    status      text not null default 'open'
                check (status in ('open', 'answered', 'closed')),
    -- Denormalised so the admin inbox can sort by "waiting on me" without a
    -- join over every message.
    unread_for_maintainer boolean not null default true,
    unread_for_user       boolean not null default false
);

create table if not exists public.support_messages (
    id         bigint generated always as identity primary key,
    ticket     uuid not null references public.support_threads(ticket) on delete cascade,
    sender     text not null check (sender in ('user', 'maintainer')),
    body       text not null,
    created_at timestamptz not null default now()
);

create index if not exists support_messages_ticket_idx
    on public.support_messages (ticket, created_at);
create index if not exists support_threads_last_at_idx
    on public.support_threads (last_at desc);

alter table public.support_threads  enable row level security;
alter table public.support_messages enable row level security;
revoke all on public.support_threads  from anon;
revoke all on public.support_messages from anon;

-- Open a thread (or add to it if the ticket already exists). Returns nothing:
-- the client already knows its ticket, and telling it anything else would leak.
create or replace function public.support_open(
    p_ticket      uuid,
    p_body        text,
    p_app_version text default null,
    p_env         text default null
) returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_body text := left(btrim(p_body), 4000);
    v_count int;
begin
    if v_body = '' then
        return;
    end if;

    insert into public.support_threads (ticket, env, app_version)
    values (p_ticket, left(p_env, 1000), left(p_app_version, 40))
    on conflict (ticket) do nothing;

    -- Flood guard: a thread is a conversation, not an upload channel. Counted
    -- per day so a stuck client cannot fill the table, while a real
    -- back-and-forth never hits it.
    select count(*) into v_count
      from public.support_messages
     where ticket = p_ticket
       and sender = 'user'
       and created_at > now() - interval '1 day';
    if v_count >= 50 then
        return;
    end if;

    insert into public.support_messages (ticket, sender, body)
    values (p_ticket, 'user', v_body);

    update public.support_threads
       set last_at = now(),
           unread_for_maintainer = true,
           status = case when status = 'closed' then 'open' else status end
     where ticket = p_ticket;
end;
$$;

-- Every message on one thread, oldest first. Scoped to the ticket passed in,
-- so a caller can only ever read the thread it already holds the id for.
create or replace function public.support_poll(p_ticket uuid)
returns table (sender text, body text, created_at timestamptz)
language sql
security definer
set search_path = ''
as $$
    select m.sender, m.body, m.created_at
      from public.support_messages m
     where m.ticket = p_ticket
     order by m.created_at;
$$;

-- Clear the user-side unread flag once the app has shown the replies.
create or replace function public.support_mark_read(p_ticket uuid)
returns void
language sql
security definer
set search_path = ''
as $$
    update public.support_threads
       set unread_for_user = false
     where ticket = p_ticket;
$$;

revoke all on function public.support_open(uuid, text, text, text) from public;
revoke all on function public.support_poll(uuid)                   from public;
revoke all on function public.support_mark_read(uuid)              from public;
grant execute on function public.support_open(uuid, text, text, text) to anon;
grant execute on function public.support_poll(uuid)                   to anon;
grant execute on function public.support_mark_read(uuid)              to anon;

-- Maintainer replies. Not granted to anon: the admin dashboard calls this with
-- the service_role key, so nothing shipped in the app can ever post as the
-- maintainer even if someone extracts the anon key from the binary.
create or replace function public.support_reply(p_ticket uuid, p_body text)
returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_body text := left(btrim(p_body), 4000);
begin
    if v_body = '' then
        return;
    end if;
    insert into public.support_messages (ticket, sender, body)
    values (p_ticket, 'maintainer', v_body);
    update public.support_threads
       set last_at = now(),
           unread_for_user = true,
           unread_for_maintainer = false,
           status = 'answered'
     where ticket = p_ticket;
end;
$$;

revoke all on function public.support_reply(uuid, text) from public;

-- ── Replying to feedback ─────────────────────────────────────────────────────
-- A 👍/👎 row is anonymous and one-way, which makes the most valuable feedback
-- ("does not work, wallpaper is just black") the least actionable: there is
-- nobody to ask what broke. The fix is to let the submitter opt into a reply
-- channel at the moment they submit, by attaching the support ticket they
-- already have (or one generated then and there).
--
-- `ticket` is null for feedback submitted without that box ticked, and for
-- every row that predates this. Null means "do not contact" and the admin UI
-- offers no reply affordance on those rows — the absence is the user's answer.
alter table public.feedback add column if not exists ticket uuid;
create index if not exists feedback_ticket_idx on public.feedback (ticket);

-- Where a thread came from, so the inbox can put an unhappy user first, and
-- what they rated if it began as feedback.
alter table public.support_threads add column if not exists origin text not null default 'direct';
alter table public.support_threads add column if not exists rating smallint;
do $$ begin
    alter table public.support_threads
        add constraint support_threads_origin_check check (origin in ('direct', 'feedback'));
exception when duplicate_object then null; end $$;

-- support_open gains origin/rating. Adding defaulted parameters creates a NEW
-- overload rather than replacing the old one, which would leave two callable
-- functions and an ambiguous grant, so drop the 4-arg version explicitly.
drop function if exists public.support_open(uuid, text, text, text);

create or replace function public.support_open(
    p_ticket      uuid,
    p_body        text,
    p_app_version text default null,
    p_env         text default null,
    p_origin      text default 'direct',
    p_rating      smallint default null
) returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_body text := left(btrim(p_body), 4000);
    v_count int;
begin
    if v_body = '' then
        return;
    end if;

    insert into public.support_threads (ticket, env, app_version, origin, rating)
    values (
        p_ticket,
        left(p_env, 1000),
        left(p_app_version, 40),
        case when p_origin = 'feedback' then 'feedback' else 'direct' end,
        p_rating
    )
    on conflict (ticket) do update set
        -- A thread that began as feedback keeps saying so; a later direct
        -- message must not erase why it was opened.
        origin = case
            when public.support_threads.origin = 'feedback' then 'feedback'
            else excluded.origin
        end,
        rating = coalesce(public.support_threads.rating, excluded.rating),
        env    = coalesce(excluded.env, public.support_threads.env);

    select count(*) into v_count
      from public.support_messages
     where ticket = p_ticket
       and sender = 'user'
       and created_at > now() - interval '1 day';
    if v_count >= 50 then
        return;
    end if;

    insert into public.support_messages (ticket, sender, body)
    values (p_ticket, 'user', v_body);

    update public.support_threads
       set last_at = now(),
           unread_for_maintainer = true,
           status = case when status = 'closed' then 'open' else status end
     where ticket = p_ticket;
end;
$$;

revoke all on function public.support_open(uuid, text, text, text, text, smallint) from public;
grant execute on function public.support_open(uuid, text, text, text, text, smallint) to anon;

-- ── Consent revision 2: unique users and country are not optional ────────────
-- Revision 1 split the tiers as "install id + everything" vs "country only, no
-- identifier". That made the declining cohort impossible to de-duplicate, so
-- it could never answer "how many people use Fresco".
--
-- Revision 2 moves the line. BOTH tiers now carry the random install id and the
-- country, so unique users are countable everywhere. What became optional is
-- the detail: the environment (distro, desktop, session, backend, monitors),
-- feature-usage events, error reports, and the precise time of use.
--
-- `minimal` marks a row written under the essential tier, so the dashboard can
-- report the split and never present an essential row's absent environment as
-- "unknown distro" when it is really "not collected".
alter table public.installs add column if not exists minimal boolean not null default false;

-- Essential-tier heartbeat: identity, country, version, packaging. Nothing
-- else, and the timestamp is deliberately truncated to the day.
--
-- Truncating is the literal implementation of "the exact time of use is
-- optional": an essential row records THAT someone was active on a date, never
-- at 21:47. Full-consent rows keep the precise timestamp, which is what the
-- optional tier buys. Do not "fix" this to now() — the imprecision is the
-- feature.
create or replace function public.register_install_minimal(
    p_install_id text,
    p_version    text default null,
    p_channel    text default null
) returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_country text := public.request_country();
begin
    insert into public.installs (
        install_id, version, channel, country, minimal, last_seen
    ) values (
        p_install_id, p_version, p_channel, v_country, true,
        date_trunc('day', now())
    )
    on conflict (install_id) do update set
        version   = excluded.version,
        channel   = excluded.channel,
        country   = coalesce(excluded.country, public.installs.country),
        -- An install that downgrades from full to essential consent keeps its
        -- previously collected environment (it was lawfully collected under
        -- the consent in force then) but stops refreshing it, and its
        -- timestamp drops to day precision from here on.
        minimal   = true,
        last_seen = date_trunc('day', now());
end;
$$;

revoke all on function public.register_install_minimal(text, text, text) from public;
grant execute on function public.register_install_minimal(text, text, text) to anon;

-- A full-consent ping re-asserts full precision and clears the minimal flag.
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
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_country text := public.request_country();
begin
    insert into public.installs (
        install_id, version, distro, compositor, session,
        backend, decode, monitor_count, source, channel, country, minimal, last_seen
    ) values (
        p_install_id, p_version, p_distro, p_compositor, p_session,
        p_backend, p_decode, p_monitor_count, p_source, p_channel,
        v_country, false, now()
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
        country       = coalesce(excluded.country, public.installs.country),
        minimal       = false,
        last_seen     = now();
end;
$$;

revoke all on function public.register_install(
    text, text, text, text, text, text, text, int, text, text
) from public;
grant execute on function public.register_install(
    text, text, text, text, text, text, text, int, text, text
) to anon;

-- `daily_country` and `count_anonymous_ping` are the revision-1 identifier-free
-- tally. They are superseded: the essential tier now writes a real install row,
-- so continuing to ping here would double-count every essential user. The
-- client stopped calling it in 1.1.37. Left in place (not dropped) so the
-- counts already gathered under revision 1 are not destroyed, and so clients
-- still on 1.1.36 keep working until they update. Safe to drop once the old
-- version's traffic reaches zero:
--     drop function if exists public.count_anonymous_ping(text, text);
--     drop table if exists public.daily_country;

-- ── Diagnostic: what geography does the edge actually give us? ────────────────
-- `whats_my_country` proved the country header arrives. City and region are a
-- different question: Cloudflare only populates CF-IPCity / CF-Region on paid
-- plans, and Supabase may not forward them at all. Rather than build city
-- collection and discover it is null everywhere, call this and look:
--     curl -s -X POST "$URL/rest/v1/rpc/request_geo_debug" \
--          -H "apikey: $ANON" -H "Authorization: Bearer $ANON"
-- Scoped to the geo headers only — it never returns auth headers or the IP,
-- and a caller can only ever see the headers of their own request.
create or replace function public.request_geo_debug()
returns jsonb
language plpgsql
stable
as $$
declare
    headers jsonb;
begin
    headers := nullif(current_setting('request.headers', true), '')::jsonb;
    if headers is null then
        return jsonb_build_object('error', 'no request.headers (called outside PostgREST)');
    end if;
    return jsonb_build_object(
        'country',   headers ->> 'cf-ipcountry',
        'city',      headers ->> 'cf-ipcity',
        'region',    headers ->> 'cf-region',
        'continent', headers ->> 'cf-ipcontinent',
        'timezone',  headers ->> 'cf-timezone'
    );
end;
$$;

grant execute on function public.request_geo_debug() to anon;

-- ── City and region (optional tier only) ─────────────────────────────────────
-- Country comes from Cloudflare's CF-IPCountry for free and is resolved
-- server-side, so it cannot be spoofed and is sent under both consent tiers.
-- City needs CF-IPCity, which is Cloudflare Enterprise-only and which Supabase
-- does not enable, so Postgres cannot see it at any price we are paying.
--
-- Vercel injects the equivalents on every plan, and the Fresco landing site is
-- already deployed there. The app therefore asks
-- https://fresco.dibbayajyoti.com/api/geo for its own city and passes it in.
-- That endpoint never reads, logs, or returns an IP: Vercel resolves it at the
-- edge before the handler runs.
--
-- Consequence, stated because it matters more than it looks: unlike `country`,
-- these two arrive from the client and are therefore SPOOFABLE. That is
-- acceptable — a wrong city skews a distribution chart and nothing else, and
-- nothing is authorised on it — but do not build anything that trusts it.
--
-- Only `register_install` accepts them. `register_install_minimal` deliberately
-- does not, so declining the optional statistics means the city is not merely
-- discarded but never sent.
alter table public.installs add column if not exists city   text;
alter table public.installs add column if not exists region text;

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
    p_channel       text default null,
    p_city          text default null,
    p_region        text default null
) returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_country text := public.request_country();
begin
    insert into public.installs (
        install_id, version, distro, compositor, session,
        backend, decode, monitor_count, source, channel,
        country, city, region, minimal, last_seen
    ) values (
        p_install_id, p_version, p_distro, p_compositor, p_session,
        p_backend, p_decode, p_monitor_count, p_source, p_channel,
        v_country, left(p_city, 80), left(p_region, 80), false, now()
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
        country       = coalesce(excluded.country, public.installs.country),
        city          = coalesce(excluded.city,   public.installs.city),
        region        = coalesce(excluded.region, public.installs.region),
        minimal       = false,
        last_seen     = now();
end;
$$;

-- The 10-argument version is superseded; drop it so there is exactly one
-- overload and the grant is unambiguous.
drop function if exists public.register_install(
    text, text, text, text, text, text, text, int, text, text
);

revoke all on function public.register_install(
    text, text, text, text, text, text, text, int, text, text, text, text
) from public;
grant execute on function public.register_install(
    text, text, text, text, text, text, text, int, text, text, text, text
) to anon;

-- Downgrading to the essential tier stops refreshing city/region, exactly as it
-- stops refreshing the environment. Clear them outright so an old city cannot
-- outlive the consent that collected it:
create or replace function public.register_install_minimal(
    p_install_id text,
    p_version    text default null,
    p_channel    text default null
) returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
    v_country text := public.request_country();
begin
    insert into public.installs (
        install_id, version, channel, country, minimal, last_seen
    ) values (
        p_install_id, p_version, p_channel, v_country, true,
        date_trunc('day', now())
    )
    on conflict (install_id) do update set
        version   = excluded.version,
        channel   = excluded.channel,
        country   = coalesce(excluded.country, public.installs.country),
        -- Withdrawing consent for the optional detail has to actually withdraw
        -- it. The environment columns are left as they were (collected
        -- lawfully under the consent then in force, and merely stale from
        -- here), but city and region are the precise-location fields the user
        -- just opted out of, so they are erased rather than frozen.
        city      = null,
        region    = null,
        minimal   = true,
        last_seen = date_trunc('day', now());
end;
$$;

revoke all on function public.register_install_minimal(text, text, text) from public;
grant execute on function public.register_install_minimal(text, text, text) to anon;

create index if not exists installs_city_idx on public.installs (city);

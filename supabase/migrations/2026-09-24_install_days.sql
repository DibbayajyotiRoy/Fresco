-- Fresco — install_days: one check-in row per install per day.
--
-- Also committed verbatim as the tail of ../schema.sql (paste-the-whole-file
-- into the SQL editor stays the one-step setup path); this file exists so the
-- one migration that matters right now has its own name and date, per the
-- convention documented in admin/README.md.
--
-- WHY: `public.installs` is upserted in place — first_seen/last_seen are
-- overwritten on every heartbeat, so there is no per-day history and no way
-- to answer "how many installs checked in today" or "what does day-1/7/30
-- retention look like" from that table alone. This adds the smallest table
-- that answers both: one (install_id, day) row per install per calendar day,
-- written by the same SECURITY DEFINER functions that already write
-- `installs`, so it costs the client nothing new to send.
--
-- Run this in the Supabase SQL editor. Idempotent — safe to run more than
-- once, and safe to run whether or not you already ran it as part of a full
-- schema.sql paste.

create table if not exists public.install_days (
    install_id text not null,
    day        date not null,
    primary key (install_id, day)
);

alter table public.install_days enable row level security;

-- Written only from inside register_install / register_install_minimal
-- (SECURITY DEFINER, not subject to RLS). anon gets no direct rights at all —
-- same shape as `installs` itself.
revoke all on public.install_days from anon;

create index if not exists install_days_day_idx on public.install_days (day);

-- Backfill: every install that already exists gets its first_seen day and its
-- last_seen day recorded (two rows, or one if they land on the same day).
-- This is the only history available for installs that predate this table —
-- it cannot reconstruct the days in between, which is why the dashboard's
-- retention view says "collecting since <date>" until real daily rows
-- accumulate.
insert into public.install_days (install_id, day)
select install_id, first_seen::date from public.installs
on conflict do nothing;

insert into public.install_days (install_id, day)
select install_id, last_seen::date from public.installs
on conflict do nothing;

-- ── register_install: latest body (city/region, 12 args), plus a check-in row ──
-- Verbatim from schema.sql's last `create or replace function
-- public.register_install(... 12 args ...)`, with exactly one addition: the
-- insert into install_days at the end.
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

    insert into public.install_days (install_id, day)
    values (p_install_id, current_date)
    on conflict do nothing;
end;
$$;

revoke all on function public.register_install(
    text, text, text, text, text, text, text, int, text, text, text, text
) from public;
grant execute on function public.register_install(
    text, text, text, text, text, text, text, int, text, text, text, text
) to anon;

-- ── register_install_minimal: latest body, plus a check-in row ────────────
-- Verbatim from schema.sql's last `create or replace function
-- public.register_install_minimal`, with the same one addition.
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
        city      = null,
        region    = null,
        minimal   = true,
        last_seen = date_trunc('day', now());

    insert into public.install_days (install_id, day)
    values (p_install_id, current_date)
    on conflict do nothing;
end;
$$;

revoke all on function public.register_install_minimal(text, text, text) from public;
grant execute on function public.register_install_minimal(text, text, text) to anon;

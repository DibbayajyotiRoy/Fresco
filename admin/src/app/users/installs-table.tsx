import Link from "next/link";

import { Badge } from "@/components/badges";
import { DataTable, NullCell, TBody, TD, TH, THead, TR } from "@/components/data-table";
import { EmptyState } from "@/components/empty-state";
import { getInstallDaysSince, getInstalls } from "@/lib/data";
import { countryLabel } from "@/lib/geo";
import { formatDate, formatNumber } from "@/lib/format";
import {
  PAGE_SIZES,
  applyInstallsQuery,
  parseInstallsQuery,
  withQuery,
  type InstallSortKey,
} from "@/lib/installs-query";
import { LifecycleBadge } from "./sections/lifecycle";

const DAY_MS = 24 * 60 * 60 * 1000;

type RawParams = Record<string, string | string[] | undefined>;

/** A sortable column header: a plain link that toggles direction, no client
 *  JS needed. Carries `aria-sort` for assistive tech and a visible arrow for
 *  sighted keyboard users. */
function SortHeader({
  label,
  sortKey,
  query,
  params,
  className,
}: {
  label: string;
  sortKey: InstallSortKey;
  query: ReturnType<typeof parseInstallsQuery>;
  params: RawParams;
  className?: string;
}) {
  const active = query.sort === sortKey;
  const nextDir = active && query.dir === "desc" ? "asc" : "desc";
  return (
    <TH className={className} aria-sort={active ? (query.dir === "asc" ? "ascending" : "descending") : "none"}>
      <Link
        href={withQuery(params, { sort: sortKey, dir: nextDir })}
        className="flex items-center gap-1 hover:text-stone-900"
      >
        {label}
        {active ? <span aria-hidden>{query.dir === "asc" ? "↑" : "↓"}</span> : null}
      </Link>
    </TH>
  );
}

/**
 * The paginated installs table on /users: every install, filterable by
 * lifecycle status, country, version and channel, searchable by install-id
 * prefix, sortable by clicking a column header — all of it plain links and a
 * GET form, so the whole table works with the URL alone (bookmarkable,
 * shareable, no JS required).
 */
export async function InstallsTable({ searchParams }: { searchParams: RawParams }) {
  const now = Date.now();
  const query = parseInstallsQuery(searchParams);

  const [installsRes, daysRes] = await Promise.all([
    getInstalls(),
    getInstallDaysSince(new Date(now - 30 * DAY_MS).toISOString().slice(0, 10)),
  ]);

  if (!installsRes.ok) {
    return <EmptyState title="Couldn't load installs" description={installsRes.error} />;
  }

  const checkinCounts = daysRes.ok
    ? (() => {
        const m = new Map<string, number>();
        for (const d of daysRes.data) m.set(d.install_id, (m.get(d.install_id) ?? 0) + 1);
        return m;
      })()
    : null;

  const page = applyInstallsQuery(installsRes.data, query, now, checkinCounts);

  const countries = [...new Set(installsRes.data.map((i) => i.country ?? "??"))].sort();
  const versions = [...new Set(installsRes.data.map((i) => i.version).filter((v): v is string => !!v))].sort().reverse();
  const channels = [...new Set(installsRes.data.map((i) => i.channel).filter((c): c is string => !!c))].sort();

  return (
    <div className="space-y-3">
      {/* Filters: a plain GET form — every control works without JS, and the
          result is a URL that can be bookmarked or pasted into a CSV export
          link (/api/export/installs.csv accepts the same query params). */}
      <form
        method="GET"
        className="flex flex-wrap items-end gap-2 rounded-lg border border-stone-200 bg-white p-3"
      >
        <FilterField label="Status" name="status" value={query.status}>
          <option value="all">All</option>
          <option value="active">Active</option>
          <option value="idle">Idle</option>
          <option value="lapsed">Lapsed</option>
        </FilterField>
        <FilterField label="Country" name="country" value={query.country}>
          <option value="all">All</option>
          {countries.map((c) => (
            <option key={c} value={c}>
              {countryLabel(c === "??" ? null : c)}
            </option>
          ))}
        </FilterField>
        <FilterField label="Version" name="version" value={query.version}>
          <option value="all">All</option>
          {versions.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </FilterField>
        <FilterField label="Channel" name="channel" value={query.channel}>
          <option value="all">All</option>
          {channels.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </FilterField>
        <label className="flex flex-col gap-1">
          <span className="font-mono text-meta text-stone-500">Install id starts with</span>
          <input
            type="text"
            name="q"
            defaultValue={query.q}
            placeholder="fx_0001a2"
            className="h-8 rounded-md border border-stone-200 bg-white px-2 font-mono text-sm text-stone-900 focus-visible:ring-2 focus-visible:ring-sky-600 focus-visible:outline-none"
          />
        </label>
        {/* Sort/pageSize survive as hidden fields so a filter change does not
            silently drop them. */}
        <input type="hidden" name="sort" value={query.sort} />
        <input type="hidden" name="dir" value={query.dir} />
        <input type="hidden" name="pageSize" value={query.pageSize} />
        <button
          type="submit"
          className="press h-8 rounded-md border border-stone-300 bg-stone-100 px-3 text-sm font-medium text-stone-900 hover:bg-stone-200"
        >
          Apply filters
        </button>
        {query.status !== "all" || query.country !== "all" || query.version !== "all" || query.channel !== "all" || query.q ? (
          <Link
            href="/users"
            className="h-8 px-2 text-sm text-stone-500 underline underline-offset-4 hover:text-stone-900"
          >
            Clear
          </Link>
        ) : null}
        <a
          href={`/api/export/installs.csv${withQuery(searchParams, {})}`}
          className="press ml-auto h-8 rounded-md border border-stone-300 bg-white px-3 text-sm font-medium text-stone-700 hover:bg-stone-100"
        >
          Export CSV
        </a>
      </form>

      {page.total === 0 ? (
        <EmptyState
          title="No installs match these filters"
          description="Try clearing a filter or widening the search."
        />
      ) : (
        <>
          <DataTable maxHeight="28rem">
            <THead sticky="container">
              <TR>
                <SortHeader label="Install" sortKey="install_id" query={query} params={searchParams} className="w-[110px]" />
                <SortHeader label="Country" sortKey="country" query={query} params={searchParams} className="w-[140px]" />
                <TH className="w-[140px]">Region / city</TH>
                <SortHeader label="Version" sortKey="version" query={query} params={searchParams} className="w-[90px]" />
                <TH className="w-[90px]">Channel</TH>
                <TH className="w-[100px]">Distro</TH>
                <SortHeader label="First seen" sortKey="first_seen" query={query} params={searchParams} className="w-[100px]" />
                <SortHeader label="Last seen" sortKey="last_seen" query={query} params={searchParams} className="w-[100px]" />
                <TH className="w-[90px]">Status</TH>
                <TH className="w-[90px] text-right">Check-ins 30d</TH>
              </TR>
            </THead>
            <TBody>
              {page.rows.map((r) => (
                <TR key={r.install_id}>
                  <TD className="font-mono text-sm" title={r.install_id}>
                    {r.install_id.length > 12 ? `${r.install_id.slice(0, 10)}…` : r.install_id}
                  </TD>
                  <TD className="truncate text-sm" title={countryLabel(r.country)}>
                    {countryLabel(r.country)}
                  </TD>
                  <TD className="truncate text-sm text-stone-500">
                    {r.city ? (r.region ? `${r.city}, ${r.region}` : r.city) : <NullCell />}
                  </TD>
                  <TD>{r.version ? <Badge label={r.version} /> : <NullCell />}</TD>
                  <TD className="text-sm text-stone-500">{r.channel ?? <NullCell />}</TD>
                  <TD className="text-sm text-stone-500">{r.distro ?? <NullCell />}</TD>
                  <TD className="font-mono text-sm text-stone-500">{formatDate(r.first_seen)}</TD>
                  <TD className="font-mono text-sm text-stone-500">{formatDate(r.last_seen)}</TD>
                  <TD>
                    <LifecycleBadge status={r.status} />
                  </TD>
                  <TD className="text-right text-sm">
                    {r.checkinDays30d === null ? <NullCell /> : formatNumber(r.checkinDays30d)}
                  </TD>
                </TR>
              ))}
            </TBody>
          </DataTable>

          <Pager searchParams={searchParams} page={page} />
        </>
      )}
    </div>
  );
}

function FilterField({
  label,
  name,
  value,
  children,
}: {
  label: string;
  name: string;
  value: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="font-mono text-meta text-stone-500">{label}</span>
      <select
        name={name}
        defaultValue={value}
        className="h-8 rounded-md border border-stone-200 bg-white px-2 text-sm text-stone-900 focus-visible:ring-2 focus-visible:ring-sky-600 focus-visible:outline-none"
      >
        {children}
      </select>
    </label>
  );
}

function Pager({
  searchParams,
  page,
}: {
  searchParams: RawParams;
  page: { total: number; page: number; pageSize: number; pageCount: number };
}) {
  const start = page.total === 0 ? 0 : (page.page - 1) * page.pageSize + 1;
  const end = Math.min(page.page * page.pageSize, page.total);

  return (
    <div className="flex flex-wrap items-center justify-between gap-2 font-mono text-meta text-stone-500">
      <span>
        {formatNumber(start)}–{formatNumber(end)} of {formatNumber(page.total)}
      </span>
      <div className="flex items-center gap-2">
        <form method="GET" className="flex items-center gap-1.5">
          {Object.entries(searchParams)
            .filter(([k]) => k !== "pageSize" && k !== "page")
            .map(([k, v]) => (
              <input key={k} type="hidden" name={k} value={Array.isArray(v) ? v[0] : v} />
            ))}
          <label className="flex items-center gap-1.5">
            <span>Rows</span>
            <select
              name="pageSize"
              defaultValue={String(page.pageSize)}
              className="h-7 rounded-md border border-stone-200 bg-white px-1.5 text-stone-900"
            >
              {PAGE_SIZES.map((size) => (
                <option key={size} value={size}>
                  {size}
                </option>
              ))}
            </select>
          </label>
          <button
            type="submit"
            className="press h-7 rounded-md border border-stone-200 bg-white px-2 text-stone-700 hover:bg-stone-100"
          >
            Go
          </button>
        </form>
        <Link
          aria-disabled={page.page <= 1}
          href={withQuery(searchParams, { page: Math.max(1, page.page - 1) })}
          className={
            "press h-7 rounded-md border border-stone-200 bg-white px-2 " +
            (page.page <= 1 ? "pointer-events-none opacity-40" : "hover:bg-stone-100")
          }
        >
          Prev
        </Link>
        <span>
          Page {page.page} of {page.pageCount}
        </span>
        <Link
          aria-disabled={page.page >= page.pageCount}
          href={withQuery(searchParams, { page: Math.min(page.pageCount, page.page + 1) })}
          className={
            "press h-7 rounded-md border border-stone-200 bg-white px-2 " +
            (page.page >= page.pageCount ? "pointer-events-none opacity-40" : "hover:bg-stone-100")
          }
        >
          Next
        </Link>
      </div>
    </div>
  );
}

import { Suspense } from "react";

import { PageHeader } from "@/components/page-header";
import { PanelSkeleton, StatRowSkeleton } from "@/components/skeleton";
import { getInstalls } from "@/lib/data";
import { formatNumber } from "@/lib/format";

import { DauChart } from "./sections/dau-chart";
import { UsersKpis } from "./sections/kpis";
import { LifecycleSection } from "./sections/lifecycle";
import { InstallsTable } from "./installs-table";

export const dynamic = "force-dynamic";
export const revalidate = 0;

type RawParams = Record<string, string | string[] | undefined>;

async function UsersMeta() {
  const res = await getInstalls();
  return res.ok ? `${formatNumber(res.data.length)} installs ever` : undefined;
}

/**
 * Users — the owner's "monitor users far more effectively" page.
 *
 * Answers, top to bottom: how many users total including the ones who left
 * (KPI strip + lifecycle), how many check in daily (DAU chart + stickiness),
 * are they staying (retention cohorts), and a searchable, filterable,
 * exportable list of every install. Country breakdown with full drilldown
 * lives on its own page, /countries — a full per-country table next to this
 * one would have made both too tall to scan.
 */
export default async function UsersPage({
  searchParams,
}: {
  searchParams: Promise<RawParams>;
}) {
  const params = await searchParams;

  return (
    <div className="space-y-4">
      <PageHeader
        title="Users"
        action={
          <Suspense fallback={null}>
            <UsersMeta />
          </Suspense>
        }
      />

      <Suspense fallback={<StatRowSkeleton count={5} className="grid grid-cols-2 gap-2 lg:grid-cols-5" />}>
        <UsersKpis />
      </Suspense>

      <Suspense fallback={<PanelSkeleton rows={6} />}>
        <DauChart />
      </Suspense>

      <Suspense
        fallback={
          <div className="grid grid-cols-1 gap-3 lg:grid-cols-5">
            <div className="lg:col-span-2">
              <PanelSkeleton rows={5} />
            </div>
            <div className="lg:col-span-3">
              <PanelSkeleton rows={6} />
            </div>
          </div>
        }
      >
        <LifecycleSection />
      </Suspense>

      <section className="space-y-2">
        <div className="border-t border-stone-200 pt-3">
          <h2 className="text-lg font-medium tracking-tight text-stone-900">
            All installs
          </h2>
          <p className="mt-1 max-w-3xl text-sm leading-snug text-stone-500">
            Filter by lifecycle, country, version or channel, search by
            install-id prefix, or export the filtered set as CSV.
          </p>
        </div>
        <Suspense fallback={<PanelSkeleton rows={8} />}>
          <InstallsTable searchParams={params} />
        </Suspense>
      </section>
    </div>
  );
}

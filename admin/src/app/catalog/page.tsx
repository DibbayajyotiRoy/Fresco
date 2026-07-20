import { PageHeader } from "@/components/page-header";
import { EmptyState } from "@/components/empty-state";
import { ErrorPanel } from "@/components/error-panel";
import { CatalogDialog } from "@/app/catalog/catalog-dialog";
import { CatalogTable } from "@/app/catalog/catalog-table";
import { getCatalogItems } from "@/lib/data";
import { formatNumber } from "@/lib/format";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function CatalogPage() {
  const res = await getCatalogItems();

  return (
    <div className="space-y-3">
      <PageHeader
        title="Catalog"
        meta={res.ok ? `${formatNumber(res.data.length)} items` : undefined}
        action={<CatalogDialog />}
      />
      {!res.ok ? (
        <ErrorPanel title="Couldn't load the catalog" message={res.error} />
      ) : res.data.length === 0 ? (
        <EmptyState
          title="No catalog items yet"
          description="Add the first wallpaper with the button above. Media goes on GitHub Releases / R2; only metadata lives here."
        />
      ) : (
        <CatalogTable items={res.data} />
      )}
    </div>
  );
}

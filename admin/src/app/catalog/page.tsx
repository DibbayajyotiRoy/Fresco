import { Images } from "@phosphor-icons/react/dist/ssr";

import { PageHeader } from "@/components/page-header";
import { EmptyState } from "@/components/empty-state";
import { Card, CardContent } from "@/components/ui/card";
import { CatalogDialog } from "@/app/catalog/catalog-dialog";
import { CatalogTable } from "@/app/catalog/catalog-table";
import { getCatalogItems } from "@/lib/data";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default async function CatalogPage() {
  const res = await getCatalogItems();

  return (
    <>
      <PageHeader
        title="Catalog"
        description="Curated wallpapers served to the in-app gallery"
        action={<CatalogDialog />}
      />
      <div className="flex flex-1 flex-col gap-6 p-4 md:p-6">
        <Card>
          <CardContent className="px-0">
            {!res.ok ? (
              <EmptyState
                title="Couldn't load the catalog"
                description={res.error}
                className="m-4"
              />
            ) : res.data.length === 0 ? (
              <EmptyState
                title="No catalog items yet"
                icon={Images}
                description="Add the first wallpaper with the button above. Media goes on GitHub Releases / R2; only metadata lives here."
                className="m-4"
              />
            ) : (
              <CatalogTable items={res.data} />
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}

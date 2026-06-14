import { createAHTMLRoute } from "@ahtmljs/next/handler";
import { snapshotsToMcp } from "@ahtmljs/next/mcp";
import { snapshotsToOpenApi } from "@ahtmljs/next/openapi";
import { allSnapshots, buildSnapshotForPath } from "@/lib/snapshots";
import { ahtmlConfig } from "@/lib/ahtml-config";

const ahtml = createAHTMLRoute(buildSnapshotForPath, ahtmlConfig);

export async function GET(
  req: Request,
  ctx: { params: Promise<{ path?: string[] }> }
): Promise<Response> {
  const url = new URL(req.url);
  const params = await ctx.params;
  const segments = params.path ?? [];

  if (segments.length === 1 && segments[0] === "mcp.json") {
    const site = `${url.protocol}//${url.host}`;
    const m = snapshotsToMcp({ name: "fresco", url: site }, allSnapshots(site));
    return new Response(JSON.stringify(m, null, 2), {
      status: 200,
      headers: {
        "content-type": "application/json",
        "cache-control": "public, max-age=3600",
      },
    });
  }

  if (segments.length === 1 && segments[0] === "openapi.json") {
    const site = `${url.protocol}//${url.host}`;
    const doc = snapshotsToOpenApi(
      { title: "fresco", baseUrl: site },
      allSnapshots(site)
    );
    return new Response(JSON.stringify(doc, null, 2), {
      status: 200,
      headers: {
        "content-type": "application/json",
        "cache-control": "public, max-age=3600",
      },
    });
  }

  if (url.searchParams.get("fmt") === "json") {
    const headers = new Headers(req.headers);
    headers.set("accept", "application/ahtml+json");
    req = new Request(req.url, { method: req.method, headers });
  }

  return ahtml.GET(req, ctx);
}

export async function HEAD(
  req: Request,
  ctx: { params: Promise<{ path?: string[] }> }
): Promise<Response> {
  return ahtml.HEAD(req, ctx);
}

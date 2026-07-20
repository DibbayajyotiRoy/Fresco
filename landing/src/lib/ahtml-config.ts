import type { AHTMLConfig } from "@ahtmljs/next";
import {
  GITHUB_URL,
  RELEASES_URL,
} from "@/lib/site";

// Canonical site URL. SITE_URL env wins so previews/prod can override.
export const SITE_URL = process.env.SITE_URL ?? "https://fresco.dibbayajyoti.com";

// Bug tracker, used as the policy contact and llms.txt issue links.
export const ISSUES_URL = `${GITHUB_URL}/issues`;

export const ahtmlConfig: AHTMLConfig = {
  site: SITE_URL,
  default_ttl: 3600,
  policy: {
    agents_welcome: true,
    license: "GPL-3.0-or-later",
    rate_limit: "300/min",
    contact: ISSUES_URL,
    terms_url: GITHUB_URL,
    republish: "attribution_only",
    caching: { allowed: true, ttl: 3600 },
  },
  routes: [{ path: "/", page_type: "home" }],
  emit_mcp: true,
  emit_openapi: true,
};

// Re-export the canonical URLs so the snapshot/llms layers have one source.
export { GITHUB_URL, RELEASES_URL };

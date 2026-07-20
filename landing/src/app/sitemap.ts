import type { MetadataRoute } from "next";
import { ALTERNATIVES } from "@/lib/alternatives";

const SITE_URL = process.env.SITE_URL ?? "https://fresco.dibbayajyoti.com";

export default function sitemap(): MetadataRoute.Sitemap {
  const now = new Date();
  return [
    {
      url: `${SITE_URL}/`,
      lastModified: now,
      changeFrequency: "weekly",
      priority: 1.0,
    },
    ...ALTERNATIVES.map((a) => ({
      url: `${SITE_URL}/alternatives/${a.slug}`,
      lastModified: now,
      changeFrequency: "monthly" as const,
      priority: 0.8,
    })),
  ];
}

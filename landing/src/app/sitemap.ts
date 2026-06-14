import type { MetadataRoute } from "next";

const SITE_URL = process.env.SITE_URL ?? "https://fresco.app";

export default function sitemap(): MetadataRoute.Sitemap {
  return [
    {
      url: `${SITE_URL}/`,
      lastModified: new Date(),
      changeFrequency: "weekly",
      priority: 1.0,
    },
  ];
}

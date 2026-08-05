import type { MetadataRoute } from "next";
import { ALTERNATIVES } from "@/lib/alternatives";
import { LOCALES, LOCALE_META, localePath } from "@/lib/i18n/config";

const SITE_URL = process.env.SITE_URL ?? "https://fresco.dibbayajyoti.com";

/**
 * The home page ships in every language, so each locale gets its own entry
 * carrying the full hreflang cluster. The competitor comparisons are English
 * only and appear once, without alternates.
 */
export default function sitemap(): MetadataRoute.Sitemap {
  const now = new Date();

  const languages = Object.fromEntries(
    LOCALES.map((locale) => [
      LOCALE_META[locale].hreflang,
      `${SITE_URL}${localePath(locale)}`,
    ]),
  );

  return [
    ...LOCALES.map((locale) => ({
      url: `${SITE_URL}${localePath(locale)}`,
      lastModified: now,
      changeFrequency: "weekly" as const,
      priority: locale === "en" ? 1.0 : 0.9,
      alternates: { languages },
    })),
    ...ALTERNATIVES.map((a) => ({
      url: `${SITE_URL}/alternatives/${a.slug}`,
      lastModified: now,
      changeFrequency: "monthly" as const,
      priority: 0.8,
    })),
  ];
}

import { AUTHOR } from "@/lib/content";
import { ALTERNATIVES } from "@/lib/alternatives";
import { GITHUB_URL, RELEASES_URL, LICENSE_URL } from "@/lib/site";
import { VIDEOS, embedUrl, posterUrl, watchUrl } from "@/lib/videos";
import type { Dictionary } from "@/lib/i18n";
import { LOCALE_META, localePath, type Locale } from "@/lib/i18n/config";

const SITE_URL = process.env.SITE_URL ?? "https://fresco.dibbayajyoti.com";

/**
 * Structured data for SEO and GEO. A single @graph carries the
 * SoftwareApplication (with a live version and download counter), the WebSite,
 * the maintainer (Person), the FAQPage, and a VideoObject per demo on the
 * page. AI answer engines and Google read this from the server-rendered HTML.
 *
 * Every locale emits its own graph, in its own language, with @id and url
 * scoped to that locale's URL, so the Japanese page does not claim to be the
 * same document as the English one.
 */
export function JsonLd({
  version,
  downloads,
  locale,
  dict,
}: {
  version: string;
  downloads: number | null;
  locale: Locale;
  dict: Dictionary;
}) {
  const lang = LOCALE_META[locale].hreflang;
  const pageUrl = `${SITE_URL}${localePath(locale)}`;

  const software: Record<string, unknown> = {
    "@type": "SoftwareApplication",
    "@id": `${pageUrl}#software`,
    name: "Fresco",
    applicationCategory: "UtilitiesApplication",
    operatingSystem: "Linux",
    description: dict.softwareDescription,
    inLanguage: lang,
    url: pageUrl,
    downloadUrl: RELEASES_URL,
    softwareVersion: version,
    releaseNotes: `${GITHUB_URL}/blob/main/CHANGELOG.md`,
    // No Review / aggregateRating here: the one community quote on the page is
    // a named testimonial, and emitting review rich-result markup off a single
    // quote violates Google's review-snippet policy.
    softwareRequirements:
      "Linux on X11 (including Deepin 25 DDE, verified on Deepin 25 Community build1), or a Wayland layer-shell compositor (COSMIC, Hyprland, Sway, KDE Plasma 6)",
    featureList: dict.featureList,
    screenshot: `${SITE_URL}/og.png`,
    image: `${SITE_URL}/og.png`,
    license: LICENSE_URL,
    isAccessibleForFree: true,
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    author: { "@type": "Person", name: AUTHOR.name, url: AUTHOR.portfolio },
    creator: { "@type": "Person", name: AUTHOR.name, url: AUTHOR.portfolio },
    codeRepository: GITHUB_URL,
    sameAs: [GITHUB_URL],
    programmingLanguage: ["Rust"],
    keywords: dict.meta.keywords.join(", "),
  };

  /**
   * One VideoObject per demo in <VideoShowcase />. Google needs name,
   * description, thumbnailUrl, and uploadDate at minimum; embedUrl is what
   * makes the result eligible for the video carousel. The videos themselves
   * are in English, so inLanguage stays "en" regardless of the page locale.
   */
  const videos = VIDEOS.map((video) => ({
    "@type": "VideoObject",
    "@id": `${pageUrl}#video-${video.id}`,
    name: video.title,
    description: video.description,
    thumbnailUrl: [posterUrl(video.id)],
    uploadDate: video.uploadDate,
    duration: video.duration,
    embedUrl: embedUrl(video.id),
    // No contentUrl: schema.org wants a direct media file there, and YouTube
    // does not expose one. embedUrl alone is sufficient for rich results.
    url: watchUrl(video.id),
    inLanguage: "en",
    isFamilyFriendly: true,
    author: { "@type": "Person", name: AUTHOR.name, url: AUTHOR.portfolio },
    publisher: { "@type": "Person", name: AUTHOR.name, url: AUTHOR.portfolio },
    about: { "@type": "SoftwareApplication", name: "Fresco", url: pageUrl },
  }));

  software.video = videos.map((video) => ({ "@id": video["@id"] }));

  if (typeof downloads === "number") {
    software.interactionStatistic = {
      "@type": "InteractionCounter",
      interactionType: "https://schema.org/DownloadAction",
      userInteractionCount: downloads,
    };
  }

  const graph = {
    "@context": "https://schema.org",
    "@graph": [
      software,
      ...videos,
      {
        "@type": "WebSite",
        name: "Fresco",
        url: pageUrl,
        inLanguage: lang,
      },
      {
        "@type": "Person",
        name: AUTHOR.name,
        url: AUTHOR.portfolio,
        sameAs: [AUTHOR.portfolio, AUTHOR.github],
      },
      // The competitor deep-dives exist in English only.
      {
        "@type": "ItemList",
        name: "Fresco alternative comparisons",
        inLanguage: "en",
        itemListElement: ALTERNATIVES.map((alt, i) => ({
          "@type": "ListItem",
          position: i + 1,
          name: `Fresco vs ${alt.tool}`,
          url: `${SITE_URL}/alternatives/${alt.slug}`,
        })),
      },
      {
        "@type": "FAQPage",
        "@id": `${pageUrl}#faq`,
        inLanguage: lang,
        mainEntity: dict.faq.items.map(({ q, a }) => ({
          "@type": "Question",
          name: q,
          acceptedAnswer: { "@type": "Answer", text: a },
        })),
      },
    ],
  };

  return (
    <script
      type="application/ld+json"
      dangerouslySetInnerHTML={{ __html: JSON.stringify(graph) }}
    />
  );
}

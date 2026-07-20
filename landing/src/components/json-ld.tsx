import { FAQ, FEATURE_LIST, AUTHOR } from "@/lib/content";
import { ALTERNATIVES } from "@/lib/alternatives";
import { GITHUB_URL, RELEASES_URL, LICENSE_URL } from "@/lib/site";

const SITE_URL = process.env.SITE_URL ?? "https://fresco.app";

/**
 * Structured data for SEO and GEO. A single @graph carries the
 * SoftwareApplication (with a live version and download counter), the WebSite,
 * the maintainer (Person), the FAQPage, and a HowTo install walkthrough. AI
 * answer engines and Google read this from the server-rendered HTML.
 */
export function JsonLd({
  version,
  downloads,
}: {
  version: string;
  downloads: number | null;
}) {
  const software: Record<string, unknown> = {
    "@type": "SoftwareApplication",
    name: "Fresco",
    applicationCategory: "UtilitiesApplication",
    operatingSystem: "Linux",
    description:
      "Fresco is a free, open-source live-wallpaper app for Linux. It sets video, GIF, image, slideshow, and playlist wallpapers as your animated desktop background, with hardware-accelerated playback. A free Wallpaper Engine alternative for Pop!_OS, Ubuntu, Linux Mint, Debian, and elementary OS, on X11 and on Wayland layer-shell compositors (COSMIC, Hyprland, Sway, KDE Plasma 6).",
    url: SITE_URL,
    downloadUrl: RELEASES_URL,
    softwareVersion: version,
    releaseNotes: `${GITHUB_URL}/blob/main/CHANGELOG.md`,
    softwareRequirements:
      "Linux on X11, or a Wayland layer-shell compositor (COSMIC, Hyprland, Sway, KDE Plasma 6)",
    featureList: FEATURE_LIST,
    screenshot: `${SITE_URL}/opengraph-image`,
    image: `${SITE_URL}/opengraph-image`,
    license: LICENSE_URL,
    isAccessibleForFree: true,
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    author: { "@type": "Person", name: AUTHOR.name, url: AUTHOR.portfolio },
    creator: { "@type": "Person", name: AUTHOR.name, url: AUTHOR.portfolio },
    codeRepository: GITHUB_URL,
    sameAs: [GITHUB_URL],
    programmingLanguage: ["Rust"],
    keywords:
      "live wallpaper linux, video wallpaper linux, animated wallpaper ubuntu, wallpaper engine alternative linux, hidamari alternative, live wallpaper wayland, hyprland live wallpaper, kde plasma live wallpaper",
  };

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
      {
        "@type": "WebSite",
        name: "Fresco",
        url: SITE_URL,
        inLanguage: "en",
      },
      {
        "@type": "Person",
        name: AUTHOR.name,
        url: AUTHOR.portfolio,
        sameAs: [AUTHOR.portfolio, AUTHOR.github],
      },
      {
        "@type": "ItemList",
        name: "Fresco alternative comparisons",
        itemListElement: ALTERNATIVES.map((alt, i) => ({
          "@type": "ListItem",
          position: i + 1,
          name: `Fresco vs ${alt.tool}`,
          url: `${SITE_URL}/alternatives/${alt.slug}`,
        })),
      },
      {
        "@type": "FAQPage",
        mainEntity: FAQ.map(({ q, a }) => ({
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

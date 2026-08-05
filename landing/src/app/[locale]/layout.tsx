import type { Metadata, Viewport } from "next";
import { notFound } from "next/navigation";
import { Inter, Instrument_Serif, JetBrains_Mono } from "next/font/google";
import { Analytics } from "@vercel/analytics/next";
import { SmoothScroll } from "@/components/smooth-scroll";
import { SoundProvider } from "@/components/sound-provider";
import { SiteNav } from "@/components/site-nav";
import { MadeBy } from "@roy-ui/ui/made-by";
import { getDictionary } from "@/lib/i18n";
import {
  isLocale,
  LOCALES,
  LOCALE_META,
  localePath,
  type Locale,
} from "@/lib/i18n/config";
import "../globals.css";

/* Three families, three lanes — self-hosted via next/font (no <link>). */
const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
});

const instrumentSerif = Instrument_Serif({
  variable: "--font-instrument-serif",
  weight: "400",
  style: ["normal", "italic"],
  subsets: ["latin"],
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains-mono",
  subsets: ["latin"],
});

/* Applied before CSS paints: html.dark + colorScheme, no flash. */
const THEME_SCRIPT = `(function(){try{var t=localStorage.getItem("fresco.theme");var d=t==="dark"||(t!=="light"&&window.matchMedia("(prefers-color-scheme: dark)").matches);var r=document.documentElement;r.classList.toggle("dark",d);r.style.colorScheme=d?"dark":"light";}catch(e){}})();`;

const SITE_URL = process.env.SITE_URL ?? "https://fresco.dibbayajyoti.com";

/** Search terms that read the same in every market, so every locale carries them. */
const SHARED_KEYWORDS = [
  "live wallpaper linux",
  "wallpaper engine linux alternative",
  "hidamari alternative",
  "komorebi alternative",
  "mpvpaper gui",
  "live wallpaper wayland",
  "hyprland live wallpaper",
  "kde plasma live wallpaper",
  "desktop lyrics linux",
  "audio visualiser desktop linux",
  "conky alternative",
  "GTK4",
  "Rust",
  "mpv",
];

/** Only the seven shipped locales are routable; anything else 404s. */
export const dynamicParams = false;

export function generateStaticParams() {
  return LOCALES.map((locale) => ({ locale }));
}

/**
 * hreflang for every language plus x-default. Emitted identically on all
 * locales so the cluster is self-consistent, which is what Google requires
 * before it will swap in the right language for a given searcher.
 */
function languageAlternates(): Record<string, string> {
  const map: Record<string, string> = {};
  for (const locale of LOCALES) {
    map[LOCALE_META[locale].hreflang] = localePath(locale);
  }
  map["x-default"] = "/";
  return map;
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ locale: string }>;
}): Promise<Metadata> {
  const { locale } = await params;
  if (!isLocale(locale)) return {};
  const dict = await getDictionary(locale);
  const meta = LOCALE_META[locale];

  return {
    metadataBase: new URL(SITE_URL),
    applicationName: "Fresco",
    title: { default: dict.meta.title, template: "%s | Fresco" },
    description: dict.meta.description,
    keywords: [...dict.meta.keywords, ...SHARED_KEYWORDS],
    authors: [
      { name: "Dibbayajyoti Roy", url: "https://github.com/DibbayajyotiRoy" },
    ],
    creator: "Dibbayajyoti Roy",
    category: "technology",
    manifest: "/favicon/site.webmanifest",
    icons: {
      icon: [{ url: "/logo.png", type: "image/png", sizes: "1024x1024" }],
      apple: [{ url: "/logo.png", sizes: "1024x1024" }],
    },
    robots: {
      index: true,
      follow: true,
      googleBot: {
        index: true,
        follow: true,
        "max-image-preview": "large",
        "max-snippet": -1,
        "max-video-preview": -1,
      },
    },
    alternates: {
      canonical: localePath(locale),
      languages: languageAlternates(),
      // GEO: agent-readable representations emitted by AHTML.
      types: {
        "text/markdown": "/llms.txt",
        "application/ahtml+text": "/ahtml",
        "application/ahtml+json": "/ahtml?fmt=json",
        "application/mcp+json": "/ahtml/mcp.json",
        "application/openapi+json": "/ahtml/openapi.json",
      },
    },
    openGraph: {
      title: dict.meta.ogTitle,
      description: dict.meta.ogDescription,
      url: `${SITE_URL}${localePath(locale)}`,
      siteName: "Fresco",
      locale: meta.ogLocale,
      alternateLocale: LOCALES.filter((l) => l !== locale).map(
        (l) => LOCALE_META[l].ogLocale,
      ),
      type: "website",
      images: [
        { url: "/og.png", width: 1200, height: 630, alt: dict.meta.ogImageAlt },
      ],
    },
    twitter: {
      card: "summary_large_image",
      title: dict.meta.ogTitle,
      description: dict.meta.twitterDescription,
      images: ["/og.png"],
    },
  };
}

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#fafaf9" },
    { media: "(prefers-color-scheme: dark)", color: "#0c0a09" },
  ],
};

export default async function RootLayout({
  children,
  params,
}: Readonly<{
  children: React.ReactNode;
  params: Promise<{ locale: string }>;
}>) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const typed = locale as Locale;
  const dict = await getDictionary(typed);

  return (
    <html lang={LOCALE_META[typed].htmlLang} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_SCRIPT }} />
      </head>
      <body
        className={`${inter.variable} ${instrumentSerif.variable} ${jetbrainsMono.variable} font-sans antialiased`}
      >
        <SoundProvider>
          <SiteNav locale={typed} dict={dict} />
          <SmoothScroll>{children}</SmoothScroll>
        </SoundProvider>
        <MadeBy
          name="Dibbayajyoti Roy"
          href="https://dibbayajyoti.com/"
          target="_blank"
          rel="noopener noreferrer"
          nameFont="var(--font-inter)"
        />
        <Analytics />
      </body>
    </html>
  );
}

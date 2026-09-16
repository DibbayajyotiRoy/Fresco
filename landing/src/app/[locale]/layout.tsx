import type { Metadata, Viewport } from "next";
import { notFound } from "next/navigation";
import { Inter, JetBrains_Mono } from "next/font/google";
import { Analytics } from "@vercel/analytics/next";
import { MotionProvider } from "@/components/motion/motion-provider";
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


const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains-mono",
  subsets: ["latin"],
});

/* Applied before CSS paints, no flash:
   1. html.dark + colorScheme: the visitor's pick, else the system preference.
      Light is the light side, dark is the dark side; both are authored.
   2. html.js-motion, only when motion is allowed: hides [data-reveal] blocks
      until the orchestrator wires them. Failsafe: if it has not booted within
      4s (script error, slow network), the class is dropped and everything
      shows in its final state.
   3. html.intro-pending, first visit per session only (with motion + JS):
      shows the 1-second brand intro. Hard cap: cleared after 2.5s no matter
      what, with the same event the intro fires (components/intro/intro-signal.ts). */
const THEME_SCRIPT = `(function(){var r=document.documentElement;try{var t=localStorage.getItem("fresco.theme");var d=t==="dark"||(t!=="light"&&window.matchMedia("(prefers-color-scheme: dark)").matches);r.classList.toggle("dark",d);r.style.colorScheme=d?"dark":"light";}catch(e){}try{if(!window.matchMedia("(prefers-reduced-motion: reduce)").matches){r.classList.add("js-motion");var seen=null;try{seen=sessionStorage.getItem("fresco.intro");}catch(e){}if(!seen){r.classList.add("intro-pending");setTimeout(function(){if(r.classList.contains("intro-pending")){r.classList.remove("intro-pending");window.dispatchEvent(new Event("fresco:intro-done"));}},2500);}setTimeout(function(){if(!r.classList.contains("motion-ready"))r.classList.remove("js-motion");},4000);}}catch(e){}})();`;

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
    { media: "(prefers-color-scheme: light)", color: "#f6f8fb" },
    { media: "(prefers-color-scheme: dark)", color: "#05060a" },
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
        className={`${inter.variable} ${jetbrainsMono.variable} font-sans antialiased`}
      >
        <SoundProvider>
          <SiteNav locale={typed} dict={dict} />
          <MotionProvider>{children}</MotionProvider>
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

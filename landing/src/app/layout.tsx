import type { Metadata, Viewport } from "next";
import { Inter, Instrument_Serif, JetBrains_Mono } from "next/font/google";
import { Analytics } from "@vercel/analytics/next";
import { SmoothScroll } from "@/components/smooth-scroll";
import { SoundProvider } from "@/components/sound-provider";
import { SiteNav } from "@/components/site-nav";
import { MadeBy } from "@roy-ui/ui/made-by";
import "./globals.css";

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

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  applicationName: "Fresco",
  title: {
    default:
      "Fresco - Live Wallpaper for Linux | Free Wallpaper Engine Alternative",
    template: "%s | Fresco",
  },
  description:
    "Free, open-source live wallpaper app for Linux. Browse a built-in wallpaper catalog, set videos or GIFs as your desktop, per-monitor wallpapers, day and night schedules. Hardware-accelerated on X11 and Wayland.",
  keywords: [
    "live wallpaper linux",
    "video wallpaper linux",
    "animated wallpaper ubuntu",
    "animated wallpaper pop os",
    "wallpaper engine linux",
    "wallpaper engine linux alternative",
    "hidamari alternative",
    "komorebi alternative",
    "mpvpaper gui",
    "gif wallpaper linux",
    "desktop slideshow linux",
    "live wallpaper linux mint",
    "live wallpaper wayland",
    "hyprland live wallpaper",
    "kde plasma live wallpaper",
    "sway wallpaper video",
    "linux wallpaper app",
    "wallpaper catalog linux",
    "day night wallpaper linux",
    "dual monitor wallpaper linux",
    "GTK4",
    "Rust",
    "mpv",
  ],
  authors: [
    { name: "Dibbayajyoti Roy", url: "https://github.com/DibbayajyotiRoy" },
  ],
  creator: "Dibbayajyoti Roy",
  category: "technology",
  manifest: "/favicon/site.webmanifest",
  icons: {
    icon: [
      { url: "/logo.png", type: "image/png", sizes: "1024x1024" },
    ],
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
    canonical: "/",
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
    title: "Fresco - Live Wallpapers for Linux",
    description:
      "Built-in wallpaper catalog, per-monitor wallpapers, day and night schedules, and hardware-accelerated near-zero-CPU playback on X11 and Wayland. A free Wallpaper Engine alternative.",
    url: SITE_URL,
    siteName: "Fresco",
    locale: "en_US",
    type: "website",
    images: [
      {
        url: "/og.png",
        width: 1200,
        height: 630,
        alt: "Fresco — Finally, a Linux wallpaper that just works.",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "Fresco - Live Wallpapers for Linux",
    description:
      "Hardware-accelerated live wallpapers for Linux, on X11 and Wayland. A free, open-source Wallpaper Engine alternative.",
    images: ["/og.png"],
  },
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#fafaf9" },
    { media: "(prefers-color-scheme: dark)", color: "#0c0a09" },
  ],
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_SCRIPT }} />
      </head>
      <body
        className={`${inter.variable} ${instrumentSerif.variable} ${jetbrainsMono.variable} font-sans antialiased`}
      >
        <SoundProvider>
          <SiteNav />
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

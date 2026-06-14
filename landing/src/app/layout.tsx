import type { Metadata, Viewport } from "next";
import { Geist, Geist_Mono, Bricolage_Grotesque } from "next/font/google";
import { Analytics } from "@vercel/analytics/next";
import { SmoothScroll } from "@/components/smooth-scroll";
import { MadeBy } from "@roy-ui/ui/made-by";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

const bricolage = Bricolage_Grotesque({
  variable: "--font-bricolage",
  subsets: ["latin"],
  display: "swap",
});

const SITE_URL = process.env.SITE_URL ?? "https://fresco.app";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  applicationName: "Fresco",
  title: {
    default:
      "Fresco - Live Wallpaper for Linux | Free Wallpaper Engine Alternative",
    template: "%s | Fresco",
  },
  description:
    "Fresco is a free, open-source live wallpaper app for Linux. Set videos, GIFs, images, slideshows, and playlists as your desktop background. GUI-first, hardware-accelerated, near-zero CPU. X11.",
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
    "GTK4",
    "Rust",
    "mpv",
  ],
  authors: [
    { name: "Dibbayajyoti Roy", url: "https://github.com/DibbayajyotiRoy" },
  ],
  creator: "Dibbayajyoti Roy",
  category: "technology",
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
      "Video, GIF, image, slideshow, and playlist wallpapers with hardware-accelerated, near-zero-CPU playback. A free Wallpaper Engine alternative.",
    url: SITE_URL,
    siteName: "Fresco",
    locale: "en_US",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "Fresco - Live Wallpapers for Linux",
    description:
      "Hardware-accelerated live wallpapers for Linux. A free, open-source Wallpaper Engine alternative.",
  },
};

export const viewport: Viewport = {
  themeColor: "#1c1c2e",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark" suppressHydrationWarning>
      <body
        className={`${geistSans.variable} ${geistMono.variable} ${bricolage.variable} font-sans antialiased`}
      >
        <SmoothScroll>{children}</SmoothScroll>
        <MadeBy
          name="Dibbayajyoti Roy"
          href="https://dibbayajyoti.com/"
          target="_blank"
          rel="noopener noreferrer"
          nameFont="var(--font-bricolage)"
        />
        <Analytics />
      </body>
    </html>
  );
}

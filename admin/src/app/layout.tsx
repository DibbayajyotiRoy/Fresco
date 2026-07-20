import type { Metadata } from "next";
import { Inter, Instrument_Serif, JetBrains_Mono } from "next/font/google";
import "./globals.css";

import { Topbar } from "@/components/topbar";
import { AutoRefresh } from "@/components/auto-refresh";
import { Toaster } from "@/components/toaster";
import { ConfirmDialogHost } from "@/components/confirm-dialog";

// Warm Terminal type lanes (§2.3): Inter for all UI, Instrument Serif for
// display only, JetBrains Mono for ids/timestamps/micro-labels. Self-hosted
// via next/font — never a <link>.
const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
  weight: ["400", "500", "600"],
});

const instrumentSerif = Instrument_Serif({
  variable: "--font-instrument-serif",
  subsets: ["latin"],
  weight: "400",
  style: ["normal", "italic"],
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains-mono",
  subsets: ["latin"],
  weight: ["400", "500"],
});

export const metadata: Metadata = {
  title: "Fresco Admin",
  description: "Admin dashboard for the Fresco live-wallpaper app.",
};

// Applied before CSS paints — no theme flash. Modes: light | dark | system,
// persisted under "theme.mode".
const THEME_SCRIPT = `(function(){try{var m=localStorage.getItem("theme.mode");var d=m==="dark"||((m===null||m==="system")&&window.matchMedia("(prefers-color-scheme: dark)").matches);var r=document.documentElement;r.classList.toggle("dark",d);r.style.colorScheme=d?"dark":"light";}catch(e){}})();`;

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
        className={`${inter.variable} ${instrumentSerif.variable} ${jetbrainsMono.variable} antialiased`}
      >
        <a href="#main" className="skip-link">
          Skip to content
        </a>
        <AutoRefresh />
        <Topbar />
        <main id="main" className="mx-auto max-w-[1600px] px-4 py-4">
          {children}
        </main>
        <Toaster />
        <ConfirmDialogHost />
      </body>
    </html>
  );
}

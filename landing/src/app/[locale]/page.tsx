import { notFound } from "next/navigation";
import { getGitHubStats } from "@/lib/github";
import { BootConsole } from "@/components/game/boot-console";
import { StatsStrip } from "@/components/stats-strip";
import { AtAGlance } from "@/components/at-a-glance";
import { Features } from "@/components/features";
import { Comparison } from "@/components/comparison";
import { WhatsNew } from "@/components/whats-new";
import { HowItWorks } from "@/components/how-it-works";
import { VideoShowcase } from "@/components/video-showcase";
import { Supported } from "@/components/supported";
import { Download } from "@/components/download";
import { Faq } from "@/components/faq";
import { SiteFooter } from "@/components/site-footer";
import { JsonLd } from "@/components/json-ld";
import { WHATS_NEW_VERSION } from "@/lib/content";
import { getDictionary } from "@/lib/i18n";
import { isLocale, type Locale } from "@/lib/i18n/config";

export default async function Home({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const typed = locale as Locale;

  const [stats, dict] = await Promise.all([
    getGitHubStats(),
    getDictionary(typed),
  ]);

  return (
    <>
      <main>
        <BootConsole dict={dict} />
        <StatsStrip stats={stats} dict={dict} />
        <AtAGlance dict={dict} />
        <Features dict={dict} />
        <Comparison dict={dict} />
        {/* Pinned, not stats.version — see WHATS_NEW_VERSION. */}
        <WhatsNew version={WHATS_NEW_VERSION} dict={dict} />
        <HowItWorks dict={dict} />
        <VideoShowcase dict={dict} />
        <Supported dict={dict} />
        <Download dict={dict} />
        <Faq dict={dict} />
      </main>
      <SiteFooter dict={dict} />
      <JsonLd
        version={stats.version}
        downloads={stats.downloads}
        locale={typed}
        dict={dict}
      />
    </>
  );
}

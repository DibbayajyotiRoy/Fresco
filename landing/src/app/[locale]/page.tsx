import { notFound } from "next/navigation";
import { getGitHubStats } from "@/lib/github";
import { getCohortStats } from "@/lib/cohort";
import { BootConsole } from "@/components/game/boot-console";
import { Testimonials } from "@/components/testimonials";
import { AtAGlance } from "@/components/at-a-glance";
import { Features } from "@/components/features";
import { Comparison } from "@/components/comparison";
import { HowItWorks } from "@/components/how-it-works";
import { VideoShowcase } from "@/components/video-showcase";
import { Supported } from "@/components/supported";
import { Download } from "@/components/download";
import { Faq } from "@/components/faq";
import { SiteFooter } from "@/components/site-footer";
import { JsonLd } from "@/components/json-ld";
import { PageMotion } from "@/components/motion/page-motion";
import { BrandIntro } from "@/components/intro/brand-intro";
import { getDictionary } from "@/lib/i18n";
import { isLocale, type Locale } from "@/lib/i18n/config";

/**
 * Conversion-first order: say what it is and hand over the download (hero),
 * prove it (numbers, people), show what you get (features, the real widgets),
 * remove doubt (how it works, compare, demos, where it runs), then close
 * (download) and answer the rest (at a glance, FAQ).
 */
export default async function Home({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const typed = locale as Locale;

  const [stats, cohort, dict] = await Promise.all([
    getGitHubStats(),
    getCohortStats(),
    getDictionary(typed),
  ]);

  return (
    <>
      {/* First visit per session: the 1-second brand intro. */}
      <BrandIntro />
      <main>
        {/* The hero's proof row carries the numbers; no separate stats band. */}
        <BootConsole dict={dict} stats={stats} cohort={cohort} locale={typed} />
        <Testimonials dict={dict} cohort={cohort} locale={typed} />
        {/* Desktop widgets (#whats-new) + multi-monitor. */}
        <Features dict={dict} />
        <HowItWorks dict={dict} />
        <Comparison dict={dict} />
        <VideoShowcase dict={dict} />
        <Supported dict={dict} />
        <Download dict={dict} />
        <AtAGlance dict={dict} version={stats.version} />
        <Faq dict={dict} />
      </main>
      <SiteFooter dict={dict} />
      <PageMotion />
      <JsonLd
        version={stats.version}
        downloads={stats.downloads}
        locale={typed}
        dict={dict}
      />
    </>
  );
}

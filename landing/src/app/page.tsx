import { getGitHubStats } from "@/lib/github";
import { SiteNav } from "@/components/site-nav";
import { Hero } from "@/components/hero";
import { StatsStrip } from "@/components/stats-strip";
import { Features } from "@/components/features";
import { Comparison } from "@/components/comparison";
import { WhatsNew } from "@/components/whats-new";
import { HowItWorks } from "@/components/how-it-works";
import { Supported } from "@/components/supported";
import { Download } from "@/components/download";
import { Faq } from "@/components/faq";
import { SiteFooter } from "@/components/site-footer";
import { JsonLd } from "@/components/json-ld";

export default async function Home() {
  const stats = await getGitHubStats();

  return (
    <>
      <SiteNav />
      <main>
        <Hero />
        <StatsStrip stats={stats} />
        <Features />
        <Comparison />
        <WhatsNew version={stats.version} />
        <HowItWorks />
        <Supported />
        <Download />
        <Faq />
      </main>
      <SiteFooter />
      <JsonLd version={stats.version} downloads={stats.downloads} />
    </>
  );
}

import { getGitHubStats } from "@/lib/github";
import { BootConsole } from "@/components/game/boot-console";
import { StatsStrip } from "@/components/stats-strip";
import { AtAGlance } from "@/components/at-a-glance";
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
      <main>
        <BootConsole />
        <StatsStrip stats={stats} />
        <AtAGlance />
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
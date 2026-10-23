import { Bug, Lightbulb } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SplitWords } from "@/components/motion/split-words";
import { GITHUB_URL } from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";

/**
 * "Report a problem": two links into the repo's GitHub issue forms
 * (.github/ISSUE_TEMPLATE/bug_report.yml, feature_request.yml). Keep the
 * template filenames in step with that folder.
 */
export function ReportIssue({ dict }: { dict: Dictionary }) {
  const t = dict.report;
  const newIssue = `${GITHUB_URL}/issues/new?template=`;

  return (
    <section
      id="report"
      aria-labelledby="report-title"
      className="border-t border-hairline py-16 sm:py-20"
    >
      <div className="wrap">
        <div className="mx-auto flex max-w-6xl flex-col gap-8 md:flex-row md:items-center md:justify-between">
          <div className="max-w-xl">
            <h2
              id="report-title"
              data-reveal="words"
              className="font-display text-display-sm text-ink"
            >
              <SplitWords text={t.title} />
            </h2>
            <p
              data-reveal="fade"
              data-delay="0.1"
              className="mt-3 text-lg text-ink-subtle"
            >
              {t.lead}
            </p>
            <p data-reveal="fade" data-delay="0.15" className="mt-2 text-base text-ink-muted">
              {t.tipBefore}
              <code className="rounded bg-raised px-1.5 py-0.5 font-mono text-sm text-ink">
                fresco doctor
              </code>
              {t.tipAfter}
            </p>
          </div>

          <div data-reveal="fade" data-delay="0.2" className="flex flex-col gap-3 sm:flex-row">
            <Button asChild size="lg" variant="secondary" className="font-medium">
              <a
                href={`${newIssue}bug_report.yml`}
                target="_blank"
                rel="noopener noreferrer"
              >
                <Bug aria-hidden />
                {t.bug}
              </a>
            </Button>
            <Button asChild size="lg" variant="secondary" className="font-medium">
              <a
                href={`${newIssue}feature_request.yml`}
                target="_blank"
                rel="noopener noreferrer"
              >
                <Lightbulb aria-hidden />
                {t.feature}
              </a>
            </Button>
          </div>
        </div>
      </div>
    </section>
  );
}

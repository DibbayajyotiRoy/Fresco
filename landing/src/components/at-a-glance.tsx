import "@/styles/spec.css";
import type { ReactNode } from "react";
import { CopyButton } from "@/components/copy-button";
import { GITHUB_URL, INSTALL_ONELINER, INSTALL_ONELINER_COPY } from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";

/**
 * "Fresco at a glance": a quote-verbatim <dl> for answer engines and
 * skimmers (what it is, platforms, widgets, license), closed by the install
 * one-liner. The command card is the page's one dark mono surface, in both
 * themes, so its text is a fixed light slate rather than a theme token.
 *
 * The license row deliberately stops at the source link: the tech stack
 * (`dict.glance.licenseTail`) is not shown on the page. The key stays in the
 * dictionaries.
 */
export function AtAGlance({
  dict,
  version,
}: {
  dict: Dictionary;
  version: string;
}) {
  const rows: { label: string; value: ReactNode }[] = [
    { label: dict.glance.labelWhat, value: dict.glance.what },
    { label: dict.glance.labelPlatforms, value: dict.glance.platforms },
    { label: dict.glance.labelWidgets, value: dict.glance.widgets },
    {
      label: dict.glance.labelLicense,
      value: (
        <>
          {dict.glance.licenseLead}{" "}
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="font-medium text-link underline decoration-accent/40 underline-offset-[0.2em] transition-colors duration-150 hover:decoration-current"
          >
            {dict.glance.licenseLink}
          </a>
          .
        </>
      ),
    },
  ];
  const rowClass =
    "grid gap-1.5 border-b border-hairline px-5 py-5 sm:grid-cols-[10rem_minmax(0,1fr)] sm:gap-8 sm:px-7 sm:py-6";

  return (
    <section aria-label={dict.glance.ariaLabel} className="border-b border-hairline py-24 sm:py-32">
      <div className="wrap">
        <div className="mx-auto grid max-w-6xl gap-10 lg:grid-cols-12 lg:gap-16">
          <div data-reveal="fade" className="lg:col-span-4">
            <h2 className="font-display text-display-sm text-ink first-letter:uppercase">
              {dict.glance.caption}
            </h2>
            <p className="mt-4 inline-flex items-center gap-2 rounded-full border border-hairline px-3 py-1 text-sm text-ink-muted">
              <span className="first-letter:uppercase">{dict.stats.version}</span>
              <span className="font-semibold tabular-nums text-ink">v{version}</span>
            </p>
          </div>

          <dl
            data-reveal="fade"
            data-delay="0.1"
            className="overflow-hidden rounded-[16px] border border-hairline bg-surface lg:col-span-8"
          >
            {rows.map((row) => (
              <div key={row.label} className={rowClass}>
                <dt className="text-sm font-semibold text-ink first-letter:uppercase sm:pt-0.5">
                  {row.label}
                </dt>
                <dd className="text-lg text-ink-subtle">{row.value}</dd>
              </div>
            ))}
            <div className={`${rowClass} border-b-0 sm:items-center`}>
              <dt className="text-sm font-semibold text-ink first-letter:uppercase">
                {dict.glance.labelInstall}
              </dt>
              <dd className="min-w-0">
                {/* `$` and the command share a top inset sized so a one-line
                    command centres on the 40px copy button, and a wrapped one
                    keeps the prompt on its first line. */}
                <div className="flex items-start gap-3 rounded-[10px] border border-white/10 bg-terminal py-2 pr-2 pl-4">
                  <span
                    aria-hidden
                    className="select-none py-2 font-mono text-sm leading-relaxed text-slate-400"
                  >
                    $
                  </span>
                  <code className="min-w-0 flex-1 whitespace-pre-wrap py-2 [overflow-wrap:anywhere] font-mono text-sm leading-relaxed text-slate-200">
                    {INSTALL_ONELINER}
                  </code>
                  <CopyButton
                    value={INSTALL_ONELINER_COPY}
                    copyLabel={dict.download.copy}
                    copiedLabel={dict.download.copied}
                    className="size-10 rounded-lg"
                  />
                </div>
              </dd>
            </div>
          </dl>
        </div>
      </div>
    </section>
  );
}

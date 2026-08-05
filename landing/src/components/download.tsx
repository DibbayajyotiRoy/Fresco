import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { CopyButton } from "@/components/copy-button";
import { AnimatedGlyph } from "@/components/animated-glyph";
import {
  APT_INSTALL,
  INSTALL_ONELINER,
  INSTALL_ONELINER_COPY,
  RELEASES_URL,
} from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";

function TerminalBlock({
  title,
  lines,
  copyLabel,
  copiedLabel,
}: {
  title: string;
  /** `copy` lets a line place a different string on the clipboard than it
   *  displays (the FRESCO_SOURCE-tagged installer). */
  lines: { code: string; copy?: string; comment?: string }[];
  copyLabel: string;
  copiedLabel: string;
}) {
  return (
    <div className="overflow-hidden rounded-md border border-stone-800 bg-terminal">
      <div className="flex items-center justify-between border-b border-stone-800 px-3 py-2">
        <span className="flex items-center gap-2 font-mono text-meta uppercase tracking-widest text-stone-400">
          <AnimatedGlyph name="scanline" className="text-sky-400" />
          {title}
        </span>
        <span className="font-mono text-meta tracking-wide text-stone-500">
          bash
        </span>
      </div>
      <div className="flex flex-col gap-3 px-3 py-3">
        {lines.map((line) => (
          <div key={line.code}>
            {line.comment ? (
              <p className="mb-1 font-mono text-meta text-stone-500">
                # {line.comment}
              </p>
            ) : null}
            <div className="flex items-start gap-2">
              <span aria-hidden className="select-none font-mono text-sm leading-relaxed text-stone-500">
                $
              </span>
              <code className="min-w-0 flex-1 whitespace-pre-wrap [overflow-wrap:anywhere] font-mono text-sm leading-relaxed text-stone-200">
                {line.code}
              </code>
              <CopyButton
                value={line.copy ?? line.code}
                copyLabel={copyLabel}
                copiedLabel={copiedLabel}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function Download({ dict }: { dict: Dictionary }) {
  return (
    <section id="download" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div className="max-w-2xl">
            <p className="instrument-label">{dict.download.kicker}</p>
            <h2 className="mt-3 font-serif text-display-sm text-ink">
              {dict.download.title}
            </h2>
          </div>
          <Badge variant="secondary">{dict.download.badge}</Badge>
        </div>

        <p className="mt-4 max-w-2xl text-pretty text-ink-subtle">
          {dict.download.lead}
        </p>

        <div className="mt-12">
          <Card className="flex flex-col p-7">
            <p className="instrument-label">{dict.download.cardTitle}</p>
            <p className="mt-3 text-sm text-ink-subtle">
              {dict.download.cardBody}
            </p>
            <div className="mt-4">
              <TerminalBlock
                title={dict.download.terminalTitle}
                copyLabel={dict.download.copy}
                copiedLabel={dict.download.copied}
                lines={[
                  { code: INSTALL_ONELINER, copy: INSTALL_ONELINER_COPY },
                  { code: APT_INSTALL, comment: dict.download.aptComment },
                ]}
              />
            </div>
            <div className="mt-6">
              <a
                href={RELEASES_URL}
                target="_blank"
                rel="noopener noreferrer"
                className="font-mono text-meta uppercase tracking-widest text-ink-subtle underline decoration-hairline-strong underline-offset-4 transition-colors hover:text-ink"
              >
                {dict.download.releases}
              </a>
            </div>
            <p className="mt-4 text-sm text-ink-subtle">
              {dict.download.gpuNote}
            </p>
          </Card>
        </div>
      </div>
    </section>
  );
}

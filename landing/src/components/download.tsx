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

function TerminalBlock({
  title,
  lines,
}: {
  title: string;
  /** `copy` lets a line place a different string on the clipboard than it
   *  displays (the FRESCO_SOURCE-tagged installer). */
  lines: { code: string; copy?: string; comment?: string }[];
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
              <CopyButton value={line.copy ?? line.code} />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function Download() {
  return (
    <section id="download" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div className="max-w-2xl">
            <p className="instrument-label">download</p>
            <h2 className="mt-3 font-serif text-display-sm text-ink">
              Deploy on Debian, Ubuntu, Pop!_OS, and Mint.
            </h2>
          </div>
          <Badge variant="secondary">x11 · wayland</Badge>
        </div>

        <p className="mt-4 max-w-2xl text-pretty text-ink-subtle">
          The official one-line installer or the .deb release. Either path
          copies to your clipboard and runs instantly. Fresco keeps playing
          after you close the window.
        </p>

        <div className="mt-12">
          <Card className="flex flex-col p-7">
            <p className="instrument-label">one-line install</p>
            <p className="mt-3 text-sm text-ink-subtle">
              Run this in a terminal. It downloads and installs the latest{" "}
              <code className="font-mono text-sm">.deb</code> for you — always
              the newest release:
            </p>
            <div className="mt-4">
              <TerminalBlock
                title="fresco install"
                lines={[
                  { code: INSTALL_ONELINER, copy: INSTALL_ONELINER_COPY },
                  {
                    code: APT_INSTALL,
                    comment: "already have the .deb downloaded?",
                  },
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
                Browse all releases
              </a>
            </div>
            <p className="mt-4 text-sm text-ink-subtle">
              For the lowest CPU usage, install your GPU&apos;s hardware-decode
              driver (Intel media VA driver, Mesa VA drivers, or the NVIDIA
              proprietary driver for NVDEC).
            </p>
          </Card>
        </div>

        </div>
    </section>
  );
}
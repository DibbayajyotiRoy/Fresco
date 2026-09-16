import { Fragment, type ComponentType, type ReactNode } from "react";
import {
  Cpu,
  Download as DownloadIcon,
  Package,
  Search,
  Store,
  Terminal,
} from "lucide-react";
import { CopyButton } from "@/components/copy-button";
import { SplitWords } from "@/components/motion/split-words";
import {
  APT_INSTALL,
  INSTALL_ONELINER,
  INSTALL_ONELINER_COPY,
  RELEASES_URL,
} from "@/lib/site";
import { cn } from "@/lib/utils";
import type { Dictionary } from "@/lib/i18n";
import "@/styles/finale.css";

/**
 * One shell command with its copy button. `--terminal` is dark in both
 * themes, so the text uses a fixed light slate ramp (slate-100 code,
 * slate-400 prompt: >= 7:1 on it). `copy` lets the clipboard carry a different
 * string than the one displayed (the FRESCO_SOURCE-tagged installer).
 */
function Command({
  code,
  copy,
  copyLabel,
  copiedLabel,
}: {
  code: string;
  copy?: string;
  copyLabel: string;
  copiedLabel: string;
}) {
  return (
    <div className="flex items-start gap-3 rounded-lg bg-terminal p-3.5 ring-1 ring-inset ring-white/10">
      <code className="min-w-0 flex-1 font-mono text-sm leading-relaxed text-slate-100 [overflow-wrap:anywhere]">
        <span aria-hidden className="select-none text-slate-400">
          ${" "}
        </span>
        {/* Soft wrap points after each "/" of a URL so it breaks between path
            segments rather than mid-word. Other commands wrap at spaces. */}
        {code.includes("://")
          ? code.split("/").map((part, i) => (
              <Fragment key={i}>
                {i > 0 ? (
                  <>
                    /<wbr />
                  </>
                ) : null}
                {part}
              </Fragment>
            ))
          : code}
      </code>
      <CopyButton
        value={copy ?? code}
        copyLabel={copyLabel}
        copiedLabel={copiedLabel}
      />
    </div>
  );
}

/** One install route: icon, title, then its action pinned to the bottom so
 *  the three actions line up across the row. */
function InstallCard({
  icon: Icon,
  title,
  titleClassName,
  className,
  children,
}: {
  icon: ComponentType<{ className?: string }>;
  title: string;
  titleClassName?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <li
      className={cn(
        "finale-card flex flex-col rounded-xl border border-hairline bg-surface p-6 sm:p-7",
        className,
      )}
    >
      <span
        aria-hidden
        className="flex size-10 items-center justify-center rounded-[10px] bg-accent/10 text-accent"
      >
        <Icon className="size-5" />
      </span>
      <h3
        className={cn(
          "mt-5 text-xl font-semibold tracking-tight text-ink",
          titleClassName,
        )}
      >
        {title}
      </h3>
      {children}
    </li>
  );
}

/**
 * The conversion close. Three equal routes, each with exactly one action:
 * paste the one-line installer, download the .deb, or install from the
 * deepin App Store. The .deb card carries the apt line because that is the
 * step that follows its download.
 */
export function Download({ dict }: { dict: Dictionary }) {
  const d = dict.download;

  return (
    <section
      id="download"
      aria-labelledby="download-title"
      className="finale-band border-b border-hairline py-24 sm:py-32"
    >
      <div className="wrap">
        <div className="mx-auto max-w-6xl">
          <div className="mx-auto flex max-w-3xl flex-col items-center text-center">
            <span className="inline-flex items-center rounded-full border border-accent/20 bg-surface px-3 py-1 text-sm font-medium capitalize text-accent">
              {d.badge}
            </span>
            <h2
              id="download-title"
              data-reveal="words"
              className="mt-6 font-display text-section text-ink"
            >
              <SplitWords text={d.title} />
            </h2>
            <p
              data-reveal="fade"
              data-delay="0.15"
              className="mt-5 max-w-2xl text-lg text-ink-subtle"
            >
              {d.lead}
            </p>
          </div>

          <ul
            data-reveal="stagger"
            className="mt-12 grid gap-4 sm:mt-16 md:grid-cols-2 lg:grid-cols-3 lg:gap-5"
          >
            <InstallCard
              icon={Terminal}
              title={d.cardTitle}
              titleClassName="first-letter:uppercase"
              className="md:col-span-2 lg:col-span-1"
            >
              <p className="mt-2 text-base leading-6 text-ink-subtle">
                {d.cardBody}
              </p>
              <div className="mt-auto pt-6">
                <Command
                  code={INSTALL_ONELINER}
                  copy={INSTALL_ONELINER_COPY}
                  copyLabel={d.copy}
                  copiedLabel={d.copied}
                />
              </div>
            </InstallCard>

            <InstallCard icon={Package} title=".deb">
              <p className="mt-2 text-base leading-6 text-ink-subtle">
                Debian · Ubuntu · Pop!_OS · Mint
              </p>
              <div className="mt-auto pt-6">
                <a
                  href={RELEASES_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-[10px] bg-primary px-5 text-base font-semibold text-primary-foreground shadow-[0_1px_2px_rgb(0_0_0/0.08)] transition-colors duration-150 hover:bg-primary/90 active:bg-primary/80"
                >
                  <DownloadIcon className="size-4" aria-hidden />
                  {dict.nav.cta}
                </a>
                <p className="mt-5 text-sm text-ink-subtle first-letter:uppercase">
                  {d.aptComment}
                </p>
                <div className="mt-2">
                  <Command
                    code={APT_INSTALL}
                    copyLabel={d.copy}
                    copiedLabel={d.copied}
                  />
                </div>
              </div>
            </InstallCard>

            <InstallCard icon={Store} title={d.storeLabel}>
              <div className="mt-auto pt-6">
                <p className="flex gap-3 rounded-lg bg-raised p-4 text-base leading-6 text-ink-muted">
                  <Search
                    aria-hidden
                    className="mt-1 size-4 shrink-0 text-ink-faint"
                  />
                  {d.storeBody}
                </p>
              </div>
            </InstallCard>
          </ul>

          <p className="mx-auto mt-10 max-w-2xl text-center text-sm text-ink-subtle">
            <Cpu
              aria-hidden
              className="mr-1.5 inline-block size-4 -translate-y-px align-middle text-ink-faint"
            />
            {d.gpuNote}
          </p>
        </div>
      </div>
    </section>
  );
}

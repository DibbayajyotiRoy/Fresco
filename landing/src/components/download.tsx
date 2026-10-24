import { Fragment, type ComponentType, type ReactNode } from "react";
import {
  Cpu,
  Download as DownloadIcon,
  Package,
  Store,
  Terminal,
} from "lucide-react";
import { CopyButton } from "@/components/copy-button";
import { SplitWords } from "@/components/motion/split-words";
import { DISTROS } from "@/lib/content";
import {
  APT_INSTALL,
  INSTALL_ONELINER,
  INSTALL_ONELINER_COPY,
  RELEASES_URL,
} from "@/lib/site";
import { cn } from "@/lib/utils";
import type { Dictionary } from "@/lib/i18n";
import "@/styles/finale.css";

function Dot({ live }: { live: boolean }) {
  return (
    <span
      aria-hidden
      className={`size-2 shrink-0 rounded-full ${live ? "bg-ok" : "bg-warn"}`}
    />
  );
}

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
    <div dir="ltr" className="flex items-start gap-3 rounded-lg bg-terminal p-3.5 ring-1 ring-inset ring-white/10">
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
 * Compact "works on" strip: the compositor/session chips with the live /
 * static-fallback legend, and the tested distros on one muted line. Used to
 * answer "will it run on my distro/desktop?" right where people are about to
 * install, instead of in its own band earlier on the page. Keeps the
 * `#supported` anchor (nav/footer/FAQ links to it) working.
 */
function WorksOn({ dict }: { dict: Dictionary }) {
  const s = dict.supported;

  /** Proper nouns, identical in every locale, except the translated X11 row. */
  const compositors: { name: string; live: boolean }[] = [
    { name: "COSMIC", live: true },
    { name: "Hyprland", live: true },
    { name: "Sway", live: true },
    { name: "KDE Plasma 6", live: true },
    { name: s.sessions.x11.label, live: true },
    { name: "Deepin DDE", live: true },
    { name: "GNOME Wayland", live: false },
  ];

  return (
    <div
      id="supported"
      data-reveal="fade"
      data-delay="0.2"
      className="mx-auto mt-10 max-w-4xl scroll-mt-24 sm:mt-12"
    >
      <h3 className="sr-only">{dict.download.worksOnTitle}</h3>
      <div className="flex flex-col items-center gap-3 sm:flex-row sm:flex-wrap sm:justify-center sm:gap-x-6 sm:gap-y-3">
        <ul className="flex flex-wrap items-center justify-center gap-2">
          {compositors.map((c) => (
            <li
              key={c.name}
              className="inline-flex items-center gap-2 rounded-full border border-hairline bg-surface px-3 py-1.5 text-sm font-medium text-ink-muted"
            >
              <Dot live={c.live} />
              {c.name}
              <span className="sr-only">: {c.live ? s.live : s.fallback}</span>
            </li>
          ))}
        </ul>
        {/* Visible key for the dots; each chip already announces its own
            status, so this is hidden from assistive tech. */}
        <p
          aria-hidden
          className="flex shrink-0 items-center gap-4 text-sm text-ink-subtle"
        >
          <span className="inline-flex items-center gap-1.5">
            <Dot live />
            {s.live}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <Dot live={false} />
            {s.fallback}
          </span>
        </p>
      </div>

      <p className="mt-4 text-center text-sm text-ink-faint">
        <span className="me-3 inline-block font-medium text-ink-subtle first-letter:uppercase">
          {s.distrosTitle(DISTROS.length)}
        </span>
        {DISTROS.map((name, i) => (
          <Fragment key={name}>
            {i > 0 ? " · " : null}
            <span className="whitespace-nowrap">{name}</span>
          </Fragment>
        ))}
      </p>
    </div>
  );
}

/**
 * The conversion close. Three equal routes, each with exactly one primary
 * action: paste the one-line installer, download the .deb, or open the
 * deepin App Store. The .deb card carries the apt line because that is the
 * step that follows its download. Directly under the lead sits the "works
 * on" strip, so the last doubt ("will it run on my distro/desktop?") is
 * answered right where people are about to install.
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

          <WorksOn dict={dict} />

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
                Debian · Ubuntu · Pop!_OS · Mint · Kali
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
              <p className="mt-2 text-base leading-6 text-ink-subtle">
                {d.storeDesc}
              </p>
              {/* No public web URL for the deepin App Store listing to link
                  out to, so this card is an honest 3-step guide instead of a
                  button that would go nowhere useful. */}
              <ol className="mt-auto flex flex-col gap-2.5 pt-6 text-sm leading-6 text-ink-muted">
                {[d.storeStep1, d.storeStep2, d.storeStep3].map(
                  (step, i) => (
                    <li
                      key={i}
                      className="flex items-start gap-3 rounded-lg bg-raised p-3.5"
                    >
                      <span
                        aria-hidden
                        className="flex size-6 shrink-0 items-center justify-center rounded-full bg-accent/10 text-sm font-semibold text-accent"
                      >
                        {i + 1}
                      </span>
                      <span className="pt-0.5">{step}</span>
                    </li>
                  ),
                )}
              </ol>
            </InstallCard>
          </ul>

          <p className="mx-auto mt-10 max-w-2xl text-center text-sm text-ink-subtle">
            <Cpu
              aria-hidden
              className="me-1.5 inline-block size-4 -translate-y-px align-middle text-ink-faint"
            />
            {d.gpuNote}
          </p>
        </div>
      </div>
    </section>
  );
}

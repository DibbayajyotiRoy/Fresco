/* Server component. One client island: DemoVideo (play/pause policy). The
   entrance is CSS only (styles/hero.css), so the copy and CTAs never wait on
   hydration. */
import { Fragment, type CSSProperties } from "react";
import { ArrowUpRight, Download, Github, Store } from "lucide-react";
import { DemoVideo } from "@/components/hero/demo-video";
import {
  AUTHOR_NAME,
  GITHUB_URL,
  PORTFOLIO_URL,
  RELEASES_URL,
} from "@/lib/site";
import { LOCALE_META, type Dictionary, type Locale } from "@/lib/i18n";
import type { GitHubStats } from "@/lib/github";
import type { CohortStats } from "@/lib/cohort";
import "@/styles/hero.css";

/** The telemetry footnote (sr-only), described by the proof row. */
const NOTE_ID = "hero-cohort-note";

/** Stagger index for the CSS entrance (styles/hero.css). */
const order = (i: number) => ({ "--i": i }) as CSSProperties;

type Proof = { id: string; value: string; label: string; telemetry?: boolean };

/**
 * Hero, conversion-first: what it is (headline), why (one line), how to get
 * it (Install, the Deepin App Store, star on GitHub), proof (live people,
 * then the project's numbers), then the product itself in a desktop-window
 * frame that rises in once the first-visit intro lifts.
 *
 * The pill names the LIVE latest release from GitHub (stats.version), not
 * WHATS_NEW_VERSION, which is deliberately pinned to the release the What's
 * New section describes.
 *
 * Data honesty: the proof row is the one place the numbers live. Telemetry
 * numbers are exact when `cohort.live`, otherwise the hand-maintained floors
 * with a "+". They carry an asterisk: a focusable toggletip holding the
 * telemetry note (title + a CSS hover/focus tip), and the row is described
 * by the same note for screen readers. Unknown numbers are left out, never
 * guessed. The "started using" pill only renders for a real, non-zero count.
 */
export function BootConsole({
  dict,
  stats,
  cohort,
  locale,
}: {
  dict: Dictionary;
  stats: GitHubStats;
  cohort: CohortStats;
  /** Formats the numbers (LOCALE_META[locale].numberLocale). */
  locale: Locale;
}) {
  const { hero } = dict;
  const numberLocale = LOCALE_META[locale].numberLocale;
  const fmt = (n: number) => n.toLocaleString(numberLocale);
  const floor = cohort.live ? "" : "+";
  /* Live pill: people active in the last 24h (new and returning), then how
     many of them are new. Each part renders only for a real, non-zero count. */
  const activeToday = cohort.active24h ? fmt(cohort.active24h) : null;
  const newToday = cohort.new24h ? fmt(cohort.new24h) : null;
  const showPill = Boolean(activeToday || newToday);
  const note = dict.stats.cohortNote;

  const proof: Proof[] = [];
  if (cohort.users !== null)
    proof.push({ id: "users", value: `${fmt(cohort.users)}${floor}`, label: dict.stats.users, telemetry: true });
  if (cohort.countries !== null)
    proof.push({ id: "countries", value: `${fmt(cohort.countries)}${floor}`, label: dict.stats.countries, telemetry: true });
  if (stats.downloads !== null)
    proof.push({ id: "downloads", value: fmt(stats.downloads), label: dict.stats.downloads });
  if (stats.stars !== null)
    proof.push({ id: "stars", value: fmt(stats.stars), label: dict.stats.stars });
  proof.push({ id: "license", value: "GPL-3.0", label: dict.stats.license });

  return (
    <section id="top" className="hero relative isolate overflow-clip">
      <div className="wrap flex flex-col items-center pb-16 pt-8 text-center sm:pb-24 sm:pt-12 lg:pt-14">
        <a
          href={RELEASES_URL}
          target="_blank"
          rel="noopener noreferrer"
          style={order(0)}
          className="hero-in group inline-flex h-8 items-center gap-2 rounded-full border border-hairline bg-surface pl-3.5 pr-3 text-[13px] font-medium text-ink-muted transition-colors duration-150 hover:border-hairline-strong hover:text-ink"
        >
          <span className="tabular-nums text-ink">v{stats.version}</span>
          <span aria-hidden className="text-ink-faint">
            ·
          </span>
          <span className="inline-block first-letter:uppercase">
            {dict.stats.version}
          </span>
          <ArrowUpRight
            className="size-3.5 text-ink-faint transition-transform duration-150 group-hover:-translate-y-px group-hover:translate-x-px"
            aria-hidden
          />
        </a>

        <h1
          style={order(1)}
          className="hero-in hero-title mt-5 max-w-[64rem] break-words font-display text-hero text-ink"
        >
          {hero.titleLead}
          {hero.titleGap}
          <span className="text-accent lg:block">{hero.titleEm}</span>
        </h1>

        <p
          style={order(2)}
          className="hero-in mt-4 max-w-[46rem] text-balance text-lg text-ink-subtle sm:mt-5 sm:text-[1.125rem] sm:leading-[1.8rem]"
        >
          {hero.body}
        </p>

        <div
          style={order(3)}
          className="hero-in mt-7 flex w-full flex-col items-stretch gap-3 sm:w-auto sm:flex-row sm:flex-wrap sm:items-center sm:justify-center"
        >
          <a
            href="#download"
            className="hero-press inline-flex h-12 items-center justify-center gap-2 rounded-[10px] bg-primary px-6 text-[15px] font-medium text-primary-foreground hover:bg-primary/90"
          >
            <Download className="size-4" aria-hidden />
            {hero.install}
          </a>
          <a
            href="#download"
            className="hero-press inline-flex h-12 items-center justify-center gap-2 rounded-[10px] border border-hairline-strong bg-surface px-5 text-[15px] font-medium text-ink hover:bg-raised"
          >
            <Store className="size-4 text-ink-subtle" aria-hidden />
            {dict.download.storeLabel}
          </a>
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="hero-press hidden h-12 items-center justify-center gap-2 rounded-[10px] px-3 text-[15px] font-medium text-ink-subtle hover:text-ink sm:inline-flex"
          >
            <Github className="size-4" aria-hidden />
            {hero.star}
          </a>
        </div>

        {showPill ? (
          <p
            style={order(4)}
            className="hero-in mt-5 inline-flex items-center gap-2 rounded-2xl border border-hairline bg-surface py-1 pl-2.5 pr-3 text-left text-[12px] text-ink-muted sm:mt-6 sm:gap-2.5 sm:rounded-full sm:py-1.5 sm:pl-3 sm:pr-3.5 sm:text-[13px]"
          >
            <span aria-hidden className="hero-live-dot shrink-0" />
            {activeToday ? (
              <span>
                {hero.activeToday(activeToday)}
                {newToday ? (
                  <>
                    <span aria-hidden className="mx-1.5 text-ink-faint">
                      ·
                    </span>
                    <span className="sr-only">, </span>
                    <span className="font-medium text-ink">
                      {hero.newToday(newToday)}
                    </span>
                  </>
                ) : null}
              </span>
            ) : newToday ? (
              hero.newUsers24h(newToday)
            ) : null}
          </p>
        ) : null}

        <p
          style={order(5)}
          aria-describedby={NOTE_ID}
          className={`hero-in relative flex max-w-[44rem] flex-wrap items-center justify-center gap-x-4 gap-y-1 text-[13px] text-ink-subtle sm:gap-x-2.5 ${
            showPill ? "mt-3" : "mt-6"
          }`}
        >
          {proof.map((item, i) => (
            <Fragment key={item.id}>
              {i > 0 ? (
                <span aria-hidden className="text-ink-faint max-sm:hidden">
                  ·
                </span>
              ) : null}
              <span className="whitespace-nowrap">
                <span className="font-medium tabular-nums text-ink">{item.value}</span>{" "}
                {item.label}
                {item.telemetry ? (
                  <button type="button" title={note} className="hero-note">
                    *
                    <span aria-hidden className="hero-note-tip">
                      {note}
                    </span>
                  </button>
                ) : null}
              </span>
            </Fragment>
          ))}
          <span id={NOTE_ID} className="sr-only">
            {note}
          </span>
        </p>

        <div className="hero-stage relative mt-8 w-full max-w-[64rem] sm:mt-12">
          <div aria-hidden className="hero-glow" />
          <div className="hero-frame overflow-hidden rounded-[16px] border border-hairline-strong bg-surface">
            <div
              aria-hidden
              className="flex h-8 items-center gap-1.5 border-b border-hairline bg-raised px-3.5 sm:h-10 sm:gap-2 sm:px-4"
            >
              <span className="size-2.5 rounded-full bg-hairline-strong" />
              <span className="size-2.5 rounded-full bg-hairline-strong" />
              <span className="size-2.5 rounded-full bg-hairline-strong" />
            </div>
            <DemoVideo />
          </div>

          <p className="mt-6 text-[13px] text-ink-subtle">
            {hero.builtBy}{" "}
            <a
              href={PORTFOLIO_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="group inline-flex items-center gap-0.5 font-medium text-ink underline-offset-4 hover:underline"
            >
              {AUTHOR_NAME}
              <ArrowUpRight
                className="size-3.5 text-ink-faint transition-transform duration-150 group-hover:-translate-y-px group-hover:translate-x-px"
                aria-hidden
              />
            </a>
          </p>
        </div>
      </div>
    </section>
  );
}

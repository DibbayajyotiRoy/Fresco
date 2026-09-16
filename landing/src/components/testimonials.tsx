import "@/styles/testimonials.css";
import { Download } from "lucide-react";
import type { Dictionary } from "@/lib/i18n";
import type { CohortStats } from "@/lib/cohort";
import { LOCALE_META, type Locale } from "@/lib/i18n/config";
import { COHORT } from "@/lib/site";
import { FIELD_QUOTES, HEADLINE_QUOTE, type FieldQuote } from "@/lib/testimonials";
import { Button } from "@/components/ui/button";
import { SplitWords } from "@/components/motion/split-words";
import { FieldReport } from "@/components/testimonials/field-report";
import { MarqueeRow } from "@/components/testimonials/marquee-row";
import { QuoteItem, TipCard } from "@/components/testimonials/quote-card";

/* Live marquee geometry, mirrored from styles/testimonials.css: cards cap
   at ITEM_REM wide with GAP_REM after each. */
const ITEM_REM = 22;
const GAP_REM = 1.5;
/** Widest screen the row must fill without showing its seam. */
const COVER_PX = 2560;

/**
 * Aria-hidden copies the row needs. The loop shifts by one period P, so a
 * screen of width W stays covered while W <= copies * P. Seven cards give
 * P ~ 2630px, so one copy.
 */
function loopCopies(n: number) {
  const period = n * (ITEM_REM + GAP_REM) * 16;
  return Math.max(1, Math.ceil(COVER_PX / period));
}

/** Title numbers: exact when live, floors with "+" otherwise or when unknown. */
function cohortNumbers(cohort: CohortStats, locale: Locale): [string, string] {
  const fmt = (n: number) => n.toLocaleString(LOCALE_META[locale].numberLocale);
  const { users, countries, live } = cohort;
  if (users === null || countries === null) {
    return [`${fmt(COHORT.users)}+`, `${fmt(COHORT.countries)}+`];
  }
  return live ? [fmt(users), fmt(countries)] : [`${fmt(users)}+`, `${fmt(countries)}+`];
}

/* Placeholders: the dictionary builds the sentence (word order differs per
   locale), then the numbers are cut back out to be set in the accent. */
const USERS_MARK = "{{users}}";
const COUNTRIES_MARK = "{{countries}}";
const MARKS = /(\{\{users\}\}|\{\{countries\}\})/;

/** The title with its two live numbers in the accent, rising as one heading. */
function TitleText({
  dict,
  users,
  countries,
}: {
  dict: Dictionary;
  users: string;
  countries: string;
}) {
  const parts = dict.testimonials.title(USERS_MARK, COUNTRIES_MARK).split(MARKS).filter(Boolean);
  return (
    <>
      {parts.map((part, i) =>
        part === USERS_MARK || part === COUNTRIES_MARK ? (
          <SplitWords
            key={i}
            text={part === USERS_MARK ? users : countries}
            className="text-accent"
          />
        ) : (
          <SplitWords key={i} text={part} />
        ),
      )}
    </>
  );
}

/** Country name in the page's language, or null if it cannot be resolved. */
function regionNamer(locale: Locale) {
  let names: Intl.DisplayNames | null = null;
  try {
    names = new Intl.DisplayNames([LOCALE_META[locale].htmlLang], { type: "region" });
  } catch {
    names = null;
  }
  return (code: string | null): string | null => {
    if (!code || !names) return null;
    try {
      return names.of(code) ?? null;
    } catch {
      return null;
    }
  };
}

const quoteKey = (q: FieldQuote) => `${q.date}-${q.country ?? "xx"}-${q.version}`;

/**
 * Social proof, straight after the stats: the live install count in the
 * title, the tip quote as the largest card beside the named Deepin field
 * report, one row of unedited feedback drifting left, then the Download
 * prompt while the proof is fresh.
 *
 * The drift is a CSS animation on the compositor (see MarqueeRow). The real
 * list renders once as a <ul>; the loop copy is aria-hidden. Quotes in the
 * visitor's own language lead the row. Without JS or under reduced motion
 * the row is a static wrapped grid. Never emitted as review markup (see
 * lib/testimonials.ts).
 */
export function Testimonials({
  dict,
  cohort,
  locale,
}: {
  dict: Dictionary;
  cohort: CohortStats;
  locale: Locale;
}) {
  const region = regionNamer(locale);
  const attribution = (code: string | null) => {
    const name = region(code);
    return name ? dict.testimonials.fromCountry(name) : dict.testimonials.anonymous;
  };
  const [users, countries] = cohortNumbers(cohort, locale);
  const pageLang = LOCALE_META[locale].htmlLang;
  const row = [...FIELD_QUOTES].sort(
    (a, b) => Number(b.lang === pageLang) - Number(a.lang === pageLang),
  );
  const copies = loopCopies(row.length);

  return (
    <section
      id="testimonials"
      aria-labelledby="testimonials-title"
      className="bg-paper py-24 sm:py-32"
    >
      <div className="wrap">
        <header className="max-w-3xl">
          <p className="text-sm font-medium text-accent">{dict.testimonials.kicker}</p>
          <h2
            id="testimonials-title"
            data-reveal="words"
            className="mt-3 font-display text-section text-ink text-balance [overflow-wrap:anywhere]"
          >
            <TitleText dict={dict} users={users} countries={countries} />
          </h2>
          <p
            data-reveal="fade"
            data-delay="0.1"
            className="mt-5 max-w-2xl text-lg text-ink-subtle text-pretty"
          >
            {dict.testimonials.lead}
          </p>
        </header>

        <div data-reveal="stagger" className="mt-14 grid gap-5 sm:mt-16 lg:grid-cols-12">
          <div className="lg:col-span-7">
            <TipCard quote={HEADLINE_QUOTE} attribution={attribution(HEADLINE_QUOTE.country)} />
          </div>
          <div className="lg:col-span-5">
            <FieldReport dict={dict} />
          </div>
        </div>
      </div>

      <div data-reveal="fade" className="mt-5">
        <MarqueeRow
          direction="left"
          copies={copies}
          pauseLabel={dict.testimonials.pause}
          playLabel={dict.testimonials.play}
        >
          <ul role="list" className="tm-group">
            {row.map((q) => (
              <li key={quoteKey(q)}>
                <QuoteItem quote={q} attribution={attribution(q.country)} />
              </li>
            ))}
          </ul>
          {Array.from({ length: copies }, (_, copy) => (
            <div key={copy} aria-hidden className="tm-group" data-dup="">
              {row.map((q) => (
                <div key={quoteKey(q)}>
                  <QuoteItem quote={q} attribution={attribution(q.country)} />
                </div>
              ))}
            </div>
          ))}
        </MarqueeRow>
      </div>

      <div className="wrap mt-10">
        <Button asChild size="lg" className="h-11 rounded-[10px] px-5 text-sm font-medium">
          <a href="#download">
            <Download aria-hidden />
            {dict.hero.install}
          </a>
        </Button>
      </div>
    </section>
  );
}

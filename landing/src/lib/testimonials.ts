/**
 * Anonymous field quotes for the testimonials section.
 *
 * Provenance, snapshot 2026-09-15, two sources:
 *   - the in-app `feedback` table (the thumbs up / down prompt inside
 *     Fresco): rows rated +1 only. Country comes from the reporter's IANA
 *     timezone; `os` is the table's value ("linux").
 *   - user support messages, vetted as positive, self-contained and
 *     anonymous. They carry no timezone, so no country (rendered as the
 *     anonymous label); `os` is the distro from the thread's environment.
 * Rows were skipped if they contained names, contact details, links,
 * personal details, profanity, a bug report or complaint, or needed context
 * to read. A comment present in both sources is listed once.
 *
 * Quoted VERBATIM: surrounding whitespace trimmed, nothing else. Casing,
 * punctuation and typos are the writer's own; do not "fix" them, and do not
 * translate them (each renders under its own `lang` on every locale).
 *
 * Anonymous: no row carries an identity, and none is inferred. Attribution is
 * by country only (null when unknown or the zone is ambiguous: UTC, Etc/*,
 * missing). `os`, `version` and `date` (UTC day of `created_at`) are shown
 * exactly as stored; a null `os` is simply omitted.
 *
 * Deliberately NOT emitted as schema.org Review / AggregateRating markup:
 * curated feedback is not a review corpus, and review rich-result markup off
 * hand-picked quotes is a policy violation.
 */
export type FieldQuote = {
  quote: string;
  /** BCP 47 language of the quote, from the reporter's locale. */
  lang: string;
  /** ISO 3166-1 alpha-2, or null when unknown / ambiguous. */
  country: string | null;
  os: string | null;
  version: string;
  /** YYYY-MM-DD, UTC. */
  date: string;
};

/** The headline card (feedback; timezone Indian/Maldives). There is no tip jar; never link one. */
export const HEADLINE_QUOTE: FieldQuote = {
  quote:
    "never before have i wanted to leave someone a tip so much yet been unable to do it",
  lang: "en",
  country: "MV",
  os: "linux",
  version: "1.1.41",
  date: "2026-09-10",
};

/** The marquee, in order (dealt alternately into two rows). */
export const FIELD_QUOTES: readonly FieldQuote[] = [
  {
    // support
    quote:
      "man now restore on login works, you are goated, if i could fix every linux problem as you did i would feel god like",
    lang: "en",
    country: null,
    os: "Linux Mint 22.3",
    version: "1.1.41",
    date: "2026-09-13",
  },
  {
    // feedback, Europe/London (also sent as a support message a second later)
    quote: "perfect for cosmic bro, keep it up",
    lang: "en",
    country: "GB",
    os: "linux",
    version: "1.1.41",
    date: "2026-09-04",
  },
  {
    // feedback, Europe/Berlin (locale en_NZ; country follows the timezone)
    quote:
      "Hi, your app is wonderful. I was only wondering whether its possible to keep a background video paused when Im working on sth. So it would not run in the background. :) thanks!",
    lang: "en",
    country: "DE",
    os: "linux",
    version: "1.1.40",
    date: "2026-09-07",
  },
  {
    // support
    quote:
      "Sure, I liked the project, the app really surprised me, especially how easy it is to use wallpapers.",
    lang: "en",
    country: null,
    os: "Pop!_OS 24.04 LTS",
    version: "1.1.40",
    date: "2026-08-31",
  },
  {
    // feedback, Asia/Baghdad
    quote: "I love it, Keep it up ;)",
    lang: "en",
    country: "IQ",
    os: "linux",
    version: "1.1.1",
    date: "2026-07-22",
  },
  {
    // support (locale zh_CN)
    quote: "很好",
    lang: "zh-Hans",
    country: null,
    os: "Ubuntu 24.04.4 LTS",
    version: "1.1.40",
    date: "2026-09-06",
  },
  {
    // support
    quote: "It is a great Tool. Thx for it.",
    lang: "en",
    country: null,
    os: "LMDE 7 (gigi)",
    version: "1.1.40",
    date: "2026-08-29",
  },
];

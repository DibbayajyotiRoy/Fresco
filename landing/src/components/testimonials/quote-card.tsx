import { SplitWords } from "@/components/motion/split-words";
import type { FieldQuote } from "@/lib/testimonials";
import { cn } from "@/lib/utils";

/**
 * Who said it, then one quiet meta line: the version and the OS exactly as
 * stored (a null os is omitted). Plain Inter; the date stays in the data.
 */
function Caption({ quote, attribution }: { quote: FieldQuote; attribution: string }) {
  return (
    <figcaption className="tm-foot">
      <span className="text-sm font-medium text-ink-subtle">{attribution}</span>
      <span className="text-[0.8125rem] text-ink-faint">
        <span className="whitespace-nowrap">v{quote.version}</span>
        {quote.os ? (
          <>
            {" · "}
            <span className="whitespace-nowrap">{quote.os}</span>
          </>
        ) : null}
      </span>
    </figcaption>
  );
}

/** Short quotes set larger, so the row has rhythm instead of a wall of text. */
function sizeFor(quote: string) {
  const n = Array.from(quote).length;
  if (n <= 12) return "tm-item-q--xl";
  if (n <= 40) return "tm-item-q--lg";
  return "tm-item-q--md";
}

/**
 * One anonymous quote in the marquee: the verbatim quote under its own
 * `lang`, then the caption. Server-safe (no hooks), so it renders in the
 * real list and in the aria-hidden loop copy.
 */
export function QuoteItem({
  quote,
  attribution,
}: {
  quote: FieldQuote;
  attribution: string;
}) {
  return (
    <figure className="tm-card">
      <blockquote lang={quote.lang} className="tm-card-body">
        <p className={cn("tm-q tm-item-q", sizeFor(quote.quote))}>{quote.quote}</p>
      </blockquote>
      <Caption quote={quote} attribution={attribution} />
    </figure>
  );
}

/**
 * The tip quote: the largest card, words rising on entrance. There is no
 * tip jar; never link one.
 */
export function TipCard({
  quote,
  attribution,
}: {
  quote: FieldQuote;
  attribution: string;
}) {
  return (
    <figure className="tm-card tm-card--tip">
      <blockquote lang={quote.lang} className="tm-card-body flex items-center">
        <p data-reveal="words" className="tm-q tm-tip-q">
          <SplitWords text={quote.quote} />
        </p>
      </blockquote>
      <Caption quote={quote} attribution={attribution} />
    </figure>
  );
}

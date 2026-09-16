import "@/styles/spec.css";
import Link from "next/link";
import { ArrowRight, ArrowUpRight, Check, X } from "lucide-react";
import { COMPARISON, type CompareCell } from "@/lib/content";
import { ALTERNATIVES } from "@/lib/alternatives";
import type { Dictionary } from "@/lib/i18n";
import { SpecHead } from "@/components/spec/spec-head";

function Cell({
  value,
  highlight,
  dict,
}: {
  value: CompareCell;
  highlight: boolean;
  dict: Dictionary;
}) {
  const tint = highlight ? "bg-accent/[0.06]" : "";
  const base = `border-t border-hairline px-3 py-3.5 text-center ${tint}`;

  if (value === true) {
    return (
      <td className={base}>
        <Check
          aria-hidden
          className={`mx-auto size-4 ${highlight ? "text-accent" : "text-ink-muted"}`}
          strokeWidth={highlight ? 2.75 : 2}
        />
        <span className="sr-only">{dict.compare.yes}</span>
      </td>
    );
  }
  if (value === false) {
    return (
      <td className={base}>
        <X aria-hidden className="mx-auto size-4 text-ink-faint" />
        <span className="sr-only">{dict.compare.no}</span>
      </td>
    );
  }
  return <td className={`${base} text-sm text-ink-subtle`}>{dict.compare.cells[value]}</td>;
}

/**
 * Compare: small bars of full "yes" marks per tool (from the same data as
 * the table), then the matrix with the Fresco column tinted and headed in
 * the accent. On narrow screens the matrix scrolls inside its own region
 * with the feature column pinned; the page never scrolls sideways.
 */
export function Comparison({ dict }: { dict: Dictionary }) {
  const total = COMPARISON.rows.length;
  const scores = COMPARISON.tools.map((_, i) =>
    COMPARISON.rows.reduce((s, r) => s + (r.values[i] === true ? 1 : 0), 0),
  );

  return (
    <section
      id="compare"
      aria-labelledby="compare-title"
      className="border-b border-hairline py-24 sm:py-32"
    >
      <div className="wrap">
        <div className="mx-auto max-w-6xl">
          <SpecHead
            id="compare-title"
            kicker={dict.compare.kicker}
            title={dict.compare.title}
            lead={dict.compare.lead}
          />

          <div className="mt-14 sm:mt-16">
            <p id="compare-meter" className="text-sm text-ink-faint first-letter:uppercase">
              {dict.compare.meter(COMPARISON.tools.length, total)}
            </p>
            <ul
              aria-labelledby="compare-meter"
              data-reveal="stagger"
              className="mt-4 grid grid-cols-2 gap-x-6 gap-y-5 sm:grid-cols-5"
            >
              {COMPARISON.tools.map((tool, i) => (
                <li key={tool} className={i === 0 ? "col-span-2 sm:col-span-1" : undefined}>
                  <p className="flex items-baseline justify-between gap-3 text-sm">
                    <span className={i === 0 ? "font-semibold text-accent" : "font-medium text-ink-muted"}>
                      {tool}
                    </span>
                    <span className="tabular-nums text-ink-faint">
                      {scores[i]}/{total}
                    </span>
                  </p>
                  <div aria-hidden className="mt-2 h-1.5 overflow-hidden rounded-full bg-hairline">
                    <div
                      className={`h-full rounded-full ${i === 0 ? "bg-accent" : "bg-ink-faint/60"}`}
                      style={{ width: `${(scores[i] / total) * 100}%` }}
                    />
                  </div>
                </li>
              ))}
            </ul>
          </div>

          <div
            role="region"
            aria-labelledby="compare-title"
            tabIndex={0}
            // `relative` makes this scroller the containing block of the
            // absolutely positioned sr-only "Yes"/"No" spans in the cells;
            // without it they escape the clip and widen the document.
            className="relative isolate mt-8 min-w-0 max-w-full overflow-x-auto overscroll-x-contain rounded-[16px] border border-hairline bg-surface"
          >
            <table className="w-full min-w-[44rem] table-fixed border-separate border-spacing-0 text-left">
              <caption className="sr-only">{dict.compare.title}</caption>
              <colgroup>
                <col className="w-[9.5rem] sm:w-[15rem] lg:w-[18rem]" />
                {COMPARISON.tools.map((tool) => (
                  <col key={tool} />
                ))}
              </colgroup>
              <thead>
                <tr>
                  <th
                    scope="col"
                    className="spec-sticky px-4 py-4 text-sm font-medium text-ink-faint sm:px-6"
                  >
                    {dict.compare.thFeature}
                  </th>
                  {COMPARISON.tools.map((tool, i) => (
                    <th
                      key={tool}
                      scope="col"
                      className={`px-3 py-4 text-center text-sm font-semibold ${
                        i === 0 ? "bg-primary text-primary-foreground" : "text-ink"
                      }`}
                    >
                      {tool}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {COMPARISON.rows.map((row) => (
                  <tr key={row.id}>
                    <th
                      scope="row"
                      className="spec-sticky border-t border-hairline px-4 py-3.5 text-sm font-normal text-ink sm:px-6 sm:text-base"
                    >
                      {dict.compare.rows[row.id]}
                    </th>
                    {row.values.map((value, i) => (
                      <Cell key={i} value={value} highlight={i === 0} dict={dict} />
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <p className="mt-4 text-sm text-ink-faint">{dict.compare.note}</p>

          <div className="mt-10 flex flex-col gap-6 border-t border-hairline pt-8 lg:flex-row lg:items-center lg:justify-between">
            {/* The competitor deep-dives are English-only, so they always
                link to the unprefixed URL rather than into the locale. */}
            <div className="flex flex-wrap items-center gap-2">
              <span className="mr-1 text-sm font-medium text-ink-muted">
                {dict.compare.detailLabel}
              </span>
              {ALTERNATIVES.map((alt) => (
                <Link
                  key={alt.slug}
                  href={`/alternatives/${alt.slug}`}
                  hrefLang="en"
                  className="inline-flex h-10 items-center gap-1.5 rounded-lg border border-hairline bg-surface px-3 text-sm font-medium text-ink-muted transition-colors duration-150 hover:border-hairline-strong hover:text-ink"
                >
                  {dict.compare.vs(alt.tool)}
                  <ArrowUpRight aria-hidden className="size-3.5 text-accent" />
                </Link>
              ))}
            </div>
            <a
              href="#download"
              className="inline-flex h-11 shrink-0 items-center justify-center gap-2 self-start rounded-[10px] bg-primary px-5 text-base font-semibold text-primary-foreground transition-[background-color,transform] duration-150 hover:bg-primary/90 active:scale-[0.98] lg:self-auto"
            >
              {dict.nav.cta}
              <ArrowRight aria-hidden className="size-4" />
            </a>
          </div>
        </div>
      </div>
    </section>
  );
}

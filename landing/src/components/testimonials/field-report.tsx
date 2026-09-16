import { TESTIMONIAL } from "@/lib/content";
import type { Dictionary } from "@/lib/i18n";

/**
 * The one named quote, published with the reviewer's written permission.
 * Verbatim and in English on every locale (translating it would misquote a
 * named person); only the labels and the reviewer's role are localised.
 * A clean card: the label, the quote, who wrote it, and the environment it
 * was verified on as one compact line.
 */
export function FieldReport({ dict }: { dict: Dictionary }) {
  return (
    <figure className="tm-card tm-card--report">
      <p className="flex items-center gap-2 text-sm font-medium text-accent">
        <span aria-hidden className="size-1.5 rounded-full bg-ok" />
        {dict.testimonials.namedLabel}
      </p>

      <blockquote lang="en" className="tm-card-body">
        <p className="tm-q tm-report-q">{TESTIMONIAL.quote}</p>
      </blockquote>

      <figcaption className="flex flex-col gap-4 border-t border-hairline pt-5">
        <p className="text-sm">
          <span className="block font-medium text-ink">{TESTIMONIAL.author}</span>
          <span className="mt-0.5 block text-ink-subtle">
            {dict.supported.testimonialRole}
          </span>
        </p>
        <div className="text-[0.8125rem]">
          <p className="text-ink-faint">{dict.supported.verifiedEnv}</p>
          <dl className="mt-1 flex flex-wrap gap-x-4 gap-y-1">
            {TESTIMONIAL.environment.map((row) => (
              <div key={row.id} className="flex gap-1.5">
                <dt className="text-ink-faint">{dict.supported.envLabels[row.id]}</dt>
                <dd className="text-ink-subtle [overflow-wrap:anywhere]">{row.value}</dd>
              </div>
            ))}
          </dl>
        </div>
      </figcaption>
    </figure>
  );
}

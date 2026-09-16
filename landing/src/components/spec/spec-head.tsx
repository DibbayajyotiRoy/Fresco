import { SplitWords } from "@/components/motion/split-words";

/**
 * Plain class join. Deliberately not `cn()`: tailwind-merge reads the custom
 * `text-section` size and `text-ink` colour as one conflict group and drops
 * the size.
 */
const cx = (...c: (string | false | undefined)[]) => c.filter(Boolean).join(" ");

/**
 * Section header: a small accent eyebrow, the title rising word by word,
 * then the lead. `id` labels the section (aria-labelledby). `meta` is an
 * optional quiet footnote line under the lead. Dictionary eyebrows are
 * lowercase, so the first letter is raised in CSS (a no-op for CJK).
 */
export function SpecHead({
  id,
  kicker,
  title,
  lead,
  meta,
  align = "start",
  className,
}: {
  id: string;
  kicker?: string;
  title: string;
  lead: string;
  meta?: string;
  align?: "start" | "center";
  className?: string;
}) {
  const center = align === "center";
  return (
    <header className={cx(center && "mx-auto max-w-3xl text-center", className)}>
      {kicker ? (
        <p className="text-sm font-semibold text-accent first-letter:uppercase">
          {kicker}
        </p>
      ) : null}
      <h2
        id={id}
        data-reveal="words"
        className={cx(
          "font-display text-section text-ink hyphens-auto [overflow-wrap:anywhere]",
          kicker && "mt-4",
          !center && "max-w-4xl",
        )}
      >
        <SplitWords text={title} />
      </h2>
      <p
        data-reveal="fade"
        data-delay="0.15"
        className={cx("mt-5 max-w-2xl text-lg text-ink-subtle", center && "mx-auto")}
      >
        {lead}
      </p>
      {meta ? (
        <p className="mt-4 text-sm text-ink-faint first-letter:uppercase">{meta}</p>
      ) : null}
    </header>
  );
}

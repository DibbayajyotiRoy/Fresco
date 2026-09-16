/**
 * Server-safe word splitter for staggered headings.
 *
 * Renders the unsplit text once for assistive tech (sr-only) and the split
 * words once for the eye (aria-hidden), so screen readers never hear a heading
 * word by word. Words are plain visible spans, so with JavaScript unavailable
 * or under reduced motion the heading simply reads in its final state.
 *
 * Latin scripts split on whitespace (punctuation stays on its word). CJK runs
 * have no spaces, so they go through Intl.Segmenter, with trailing
 * punctuation folded into the preceding word.
 *
 * Only pass plain strings: never split links or meaningful inline markup.
 */
const CJK = /[　-鿿가-힯＀-￯]/;

function tokens(text: string): string[] {
  const out: string[] = [];
  for (const chunk of text.split(/(\s+)/)) {
    if (!chunk) continue;
    if (/^\s+$/.test(chunk)) {
      out.push(" ");
      continue;
    }
    if (!CJK.test(chunk)) {
      out.push(chunk);
      continue;
    }
    const segmenter = new Intl.Segmenter(undefined, { granularity: "word" });
    for (const s of segmenter.segment(chunk)) {
      const last = out.length - 1;
      if (!s.isWordLike && last >= 0 && out[last] !== " ") out[last] += s.segment;
      else out.push(s.segment);
    }
  }
  return out;
}

export function SplitWords({
  text,
  className,
}: {
  text: string;
  className?: string;
}) {
  return (
    <>
      <span className="sr-only">{text}</span>
      <span aria-hidden="true" className={className}>
        {tokens(text).map((part, i) =>
          part === " " ? (
            " "
          ) : (
            <span key={i} className="sw">
              <span className="sw-i">{part}</span>
            </span>
          ),
        )}
      </span>
    </>
  );
}

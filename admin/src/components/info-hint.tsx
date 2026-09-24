/**
 * A small "(i)" affordance carrying one explanation, for a fact that matters
 * but does not belong in a card's permanent copy. Native `title` gives every
 * pointer user a tooltip for free; `aria-label` gives screen reader users the
 * same explanation as the element's accessible name, since `title` alone is
 * not reliably announced. Keyboard-focusable so the explanation is reachable
 * without a mouse.
 */
export function InfoHint({ label }: { label: string }) {
  return (
    <span
      tabIndex={0}
      title={label}
      className="inline-flex size-3.5 shrink-0 cursor-help items-center justify-center rounded-full border border-stone-300 text-[9px] leading-none text-stone-500 focus-visible:ring-2 focus-visible:ring-sky-600 focus-visible:outline-none"
      aria-label={label}
    >
      i
    </span>
  );
}

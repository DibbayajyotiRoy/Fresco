/**
 * Contract between the first-visit boot loader and anything that
 * choreographs the first viewport.
 *
 * The head script in app/[locale]/layout.tsx adds `html.intro-pending` (only
 * with JS, motion allowed, and no INTRO_KEY in sessionStorage) and removes it
 * itself after 6.5s, dispatching INTRO_DONE, so nothing can wait forever.
 * Keep the two string literals in sync with that script.
 */
export const INTRO_KEY = "fresco.intro";
export const INTRO_DONE = "fresco:intro-done";

export function isIntroPending(): boolean {
  return (
    typeof document !== "undefined" &&
    document.documentElement.classList.contains("intro-pending")
  );
}

/** Run `cb` once the loader has lifted (immediately if none is showing).
 *  Returns an unsubscribe for effect cleanup. */
export function whenIntroDone(cb: () => void): () => void {
  if (!isIntroPending()) {
    cb();
    return () => {};
  }
  let fired = false;
  const run = () => {
    if (fired) return;
    fired = true;
    cb();
  };
  window.addEventListener(INTRO_DONE, run, { once: true });
  return () => window.removeEventListener(INTRO_DONE, run);
}

export function finishIntro() {
  const root = document.documentElement;
  if (!root.classList.contains("intro-pending")) return;
  root.classList.remove("intro-pending");
  window.dispatchEvent(new Event(INTRO_DONE));
}

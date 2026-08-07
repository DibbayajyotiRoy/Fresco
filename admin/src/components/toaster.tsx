"use client";

import * as React from "react";

/* Imperative toast singleton (§4/§8): a module store + exported functions +
 * one <Toaster /> mount in the shell. Bottom-right, max 5, tone left rail. */

type ToastKind = "success" | "info" | "error";

type Toast = {
  id: number;
  kind: ToastKind;
  message: string;
};

type Listener = (toasts: Toast[]) => void;

let toasts: Toast[] = [];
let nextId = 1;
const listeners = new Set<Listener>();
const timers = new Map<number, ReturnType<typeof setTimeout>>();

function emit() {
  for (const l of listeners) l([...toasts]);
}

function dismiss(id: number) {
  toasts = toasts.filter((t) => t.id !== id);
  const timer = timers.get(id);
  if (timer) clearTimeout(timer);
  timers.delete(id);
  emit();
}

function push(kind: ToastKind, message: string) {
  const id = nextId++;
  toasts = [...toasts, { id, kind, message }].slice(-5);
  timers.set(
    id,
    setTimeout(() => dismiss(id), kind === "error" ? 8000 : 5000)
  );
  emit();
}

export const toast = {
  success: (m: string) => push("success", m),
  info: (m: string) => push("info", m),
  error: (m: string) => push("error", m),
};

const RAIL: Record<ToastKind, string> = {
  success: "bg-emerald-500",
  info: "bg-gray-500",
  error: "bg-orange-600",
};

const GLYPH: Record<ToastKind, string> = {
  success: "⠿",
  info: "⠶",
  error: "!",
};

/** A toast as the *view* sees it: the store's record plus whether it has been
 *  removed from the store and is only still on screen to play its exit. */
type Rendered = Toast & { leaving: boolean };

/** Exit is deliberately shorter than the 200ms entrance (§7) — a toast on its
 *  way out is no longer information, and lingering makes the stack feel gummy. */
const EXIT_MS = 120;

/**
 * One toast. The enter is a CSS transition rather than a keyframe (§8): toasts
 * are the most interruption-prone surface in the app, and a keyframe restarts
 * from frame zero when React re-renders the row mid-play, whereas a transition
 * retargets from wherever it currently is.
 */
function ToastRow({
  item,
  onDismiss,
}: {
  item: Rendered;
  onDismiss: () => void;
}) {
  const [entered, setEntered] = React.useState(false);

  React.useEffect(() => {
    // Two frames, not one. The row has to be committed *and painted* in its
    // "from" state before the class flips, or the browser coalesces both states
    // into a single style recalculation and there is nothing to transition
    // between — the toast would simply appear.
    let inner = 0;
    const outer = requestAnimationFrame(() => {
      inner = requestAnimationFrame(() => setEntered(true));
    });
    return () => {
      cancelAnimationFrame(outer);
      cancelAnimationFrame(inner);
    };
  }, []);

  const shown = entered && !item.leaving;

  return (
    <div
      className={
        "pointer-events-auto flex overflow-hidden rounded-lg border border-stone-200 bg-white shadow-lift " +
        // Tailwind v4 emits `translate` and `scale` as their own properties, so
        // naming only `transform` here would silently animate nothing.
        "transition-[opacity,translate,scale] " +
        (item.leaving
          ? "pointer-events-none duration-[120ms] ease-hover "
          : "duration-200 ease-exit ") +
        (shown
          ? "translate-y-0 scale-[1] opacity-100"
          : "translate-y-2 scale-[0.98] opacity-0")
      }
    >
      <div className={`w-1 shrink-0 ${RAIL[item.kind]}`} aria-hidden />
      <div className="flex min-w-0 flex-1 items-start gap-2 px-3 py-2">
        <span
          className="font-mono text-sm leading-5 text-stone-400"
          aria-hidden
        >
          {GLYPH[item.kind]}
        </span>
        <p className="min-w-0 flex-1 text-sm text-stone-900">{item.message}</p>
        <button
          type="button"
          onClick={onDismiss}
          /* `hover:text-foreground`, not `hover:text-stone-600`: the dark remap
           * in globals.css keys off the bare `.text-stone-600` class and never
           * matches the `hover:` variant, so a stone hover colour stayed
           * light-mode grey and disappeared against the dark surface. */
          className="press -mr-1 flex size-5 shrink-0 items-center justify-center rounded-md font-mono text-sm text-stone-400 transition-[color,background-color,transform] duration-150 ease-hover hover:bg-stone-100 hover:text-foreground"
          aria-label="Dismiss"
        >
          ×
        </button>
      </div>
    </div>
  );
}

export function Toaster() {
  const [rendered, setRendered] = React.useState<Rendered[]>([]);

  React.useEffect(() => {
    const l: Listener = (next) => {
      setRendered((prev) => {
        const live = new Set(next.map((t) => t.id));
        // A toast dropped by the store (dismissed, timed out, or pushed off the
        // end of the max-5 window) is kept in place for one exit transition.
        // Mapping over `prev` rather than rebuilding from `next` is what pins it
        // to its old slot — otherwise a dismissed middle toast would jump to the
        // bottom of the stack for the duration of its own fade.
        const merged: Rendered[] = prev.map((t) => ({
          ...t,
          leaving: !live.has(t.id),
        }));
        const seen = new Set(prev.map((t) => t.id));
        for (const t of next) {
          if (!seen.has(t.id)) merged.push({ ...t, leaving: false });
        }
        return merged;
      });
    };
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  }, []);

  // Sweep departed toasts once their exit has played. Keyed on the boolean, not
  // on the list, so a toast arriving mid-exit does not restart the timer.
  const hasLeaving = rendered.some((t) => t.leaving);
  React.useEffect(() => {
    if (!hasLeaving) return;
    const id = setTimeout(
      () => setRendered((prev) => prev.filter((t) => !t.leaving)),
      EXIT_MS
    );
    return () => clearTimeout(id);
  }, [hasLeaving]);

  return (
    <div
      aria-live="polite"
      /* The column is fixed at 340px wide for the whole height of its content;
       * without this it would swallow clicks in the bottom-right corner of the
       * page in the gaps between toasts. Rows opt back in individually. */
      className="pointer-events-none fixed right-4 bottom-4 z-[60] flex w-[340px] flex-col gap-2"
    >
      {rendered.map((t) => (
        <ToastRow key={t.id} item={t} onDismiss={() => dismiss(t.id)} />
      ))}
    </div>
  );
}

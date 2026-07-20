"use client";

import * as React from "react";

/* Promise-based ConfirmDialog singleton — the app's single modal for
 * destructive actions (§4). Cancel is auto-focused; ESC / backdrop cancel;
 * body scroll locked while open. Mount one <ConfirmDialogHost /> in the
 * shell and call `confirm(...)` from anywhere. */

type ConfirmOptions = {
  title: string;
  description?: string;
  confirmLabel?: string;
};

type Pending = ConfirmOptions & { resolve: (ok: boolean) => void };

let show: ((p: Pending) => void) | null = null;

export function confirm(options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    if (!show) {
      resolve(false);
      return;
    }
    show({ ...options, resolve });
  });
}

export function ConfirmDialogHost() {
  const [pending, setPending] = React.useState<Pending | null>(null);
  const cancelRef = React.useRef<HTMLButtonElement>(null);

  React.useEffect(() => {
    show = setPending;
    return () => {
      show = null;
    };
  }, []);

  const close = React.useCallback(
    (ok: boolean) => {
      pending?.resolve(ok);
      setPending(null);
    },
    [pending]
  );

  React.useEffect(() => {
    if (!pending) return;
    cancelRef.current?.focus();
    document.body.style.overflow = "hidden";
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(false);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      document.body.style.overflow = "";
      window.removeEventListener("keydown", onKey);
    };
  }, [pending, close]);

  if (!pending) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="animate-fade-in absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={() => close(false)}
        aria-hidden
      />
      <div
        role="alertdialog"
        aria-modal="true"
        aria-label={pending.title}
        className="animate-modal-pop relative w-full max-w-md rounded-xl border border-stone-200 bg-white p-4 shadow-lg"
      >
        <p className="font-mono text-meta tracking-widest text-rose-500 uppercase">
          ! destructive action
        </p>
        <h2 className="mt-1.5 text-lg font-semibold text-stone-900">
          {pending.title}
        </h2>
        {pending.description ? (
          <p className="mt-1 text-sm text-stone-500">{pending.description}</p>
        ) : null}
        <div className="mt-4 flex justify-end gap-2">
          <button
            ref={cancelRef}
            type="button"
            onClick={() => close(false)}
            className="h-7 rounded-md border border-stone-200 bg-white px-2.5 text-sm font-medium text-stone-700 transition-colors hover:bg-stone-100"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => close(true)}
            className="h-7 rounded-md border border-rose-500/40 bg-rose-500/10 px-2.5 text-sm font-medium text-rose-600 transition-colors hover:bg-rose-500/20 dark:text-rose-400"
          >
            {pending.confirmLabel ?? "Delete"}
          </button>
        </div>
      </div>
    </div>
  );
}

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

export function Toaster() {
  const [items, setItems] = React.useState<Toast[]>([]);

  React.useEffect(() => {
    const l: Listener = setItems;
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  }, []);

  return (
    <div
      aria-live="polite"
      className="fixed right-4 bottom-4 z-[60] flex w-[340px] flex-col gap-2"
    >
      {items.map((t) => (
        <div
          key={t.id}
          className="animate-modal-pop flex overflow-hidden rounded-sm border border-stone-200 bg-white shadow-lg"
        >
          <div className={`w-1 shrink-0 ${RAIL[t.kind]}`} aria-hidden />
          <div className="flex min-w-0 flex-1 items-start gap-2 px-3 py-2">
            <span
              className="font-mono text-sm leading-5 text-stone-400"
              aria-hidden
            >
              {GLYPH[t.kind]}
            </span>
            <p className="min-w-0 flex-1 text-sm text-stone-900">{t.message}</p>
            <button
              type="button"
              onClick={() => dismiss(t.id)}
              className="font-mono text-sm text-stone-400 transition-colors hover:text-stone-600"
              aria-label="Dismiss"
            >
              ×
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

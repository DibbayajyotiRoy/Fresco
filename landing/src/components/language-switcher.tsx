"use client";

import { useEffect, useRef, useState } from "react";
import { Check, Globe } from "lucide-react";
import {
  LOCALES,
  LOCALE_COOKIE,
  LOCALE_META,
  localePath,
  type Locale,
} from "@/lib/i18n/config";

/**
 * Language picker. Rendered as real <a> links, not a router push, so every
 * option is a crawlable URL and a middle-click opens the other language in a
 * tab. Picking one writes the preference cookie before navigating, so the
 * Accept-Language redirect on "/" never overrules a deliberate choice.
 *
 * Deep pages that exist only in English (the competitor comparisons) switch
 * back to the home page of the chosen language rather than offering a link
 * that would 404.
 */
export function LanguageSwitcher({
  locale,
  label,
}: {
  locale: Locale;
  label: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function onPointerDown(event: MouseEvent) {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  function remember(next: Locale) {
    document.cookie = `${LOCALE_COOKIE}=${next}; path=/; max-age=${60 * 60 * 24 * 365}; samesite=lax`;
  }

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={label}
        className="inline-flex h-8 items-center gap-1.5 rounded-sm border border-hairline px-2 font-mono text-meta uppercase tracking-wide text-ink-subtle transition-colors hover:border-hairline-strong hover:text-ink"
      >
        <Globe className="size-3.5" aria-hidden />
        <span className="normal-case">{LOCALE_META[locale].short}</span>
      </button>

      {open ? (
        <ul
          role="listbox"
          aria-label={label}
          className="absolute right-0 z-50 mt-1.5 min-w-[190px] overflow-hidden rounded-md border border-hairline bg-paper py-1 shadow-lg"
        >
          {LOCALES.map((option) => {
            const current = option === locale;
            return (
              <li key={option} role="option" aria-selected={current}>
                <a
                  href={localePath(option)}
                  hrefLang={LOCALE_META[option].hreflang}
                  lang={LOCALE_META[option].htmlLang}
                  onClick={() => remember(option)}
                  className={`flex items-center justify-between gap-3 px-3 py-1.5 text-sm transition-colors hover:bg-raised ${
                    current ? "text-ink" : "text-ink-subtle"
                  }`}
                >
                  {LOCALE_META[option].nativeName}
                  {current ? (
                    <Check className="size-3.5 text-accent" aria-hidden />
                  ) : null}
                </a>
              </li>
            );
          })}
        </ul>
      ) : null}
    </div>
  );
}

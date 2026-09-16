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

const PANEL_ID = "language-menu";

/**
 * Language picker: a disclosure button and a list of real <a> links, not a
 * router push, so every option is a crawlable URL and a middle-click opens
 * the other language in a tab. The list stays in the DOM (hidden) so
 * aria-controls always resolves. Picking one writes the preference cookie
 * before navigating, so the Accept-Language redirect on "/" never overrules
 * a deliberate choice.
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
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    function onPointerDown(event: PointerEvent) {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      setOpen(false);
      buttonRef.current?.focus();
    }
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  function remember(next: Locale) {
    document.cookie = `${LOCALE_COOKIE}=${next}; path=/; max-age=${60 * 60 * 24 * 365}; samesite=lax`;
  }

  return (
    <div ref={ref} className="relative">
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        aria-controls={PANEL_ID}
        aria-label={label}
        className="nav-press inline-flex h-9 min-w-9 items-center justify-center gap-1.5 rounded-lg border border-hairline px-2 text-[13px] font-medium text-ink-subtle hover:border-hairline-strong hover:text-ink sm:px-2.5"
      >
        <Globe className="size-4" aria-hidden />
        <span className="hidden sm:inline">{LOCALE_META[locale].short}</span>
      </button>

      <ul
        id={PANEL_ID}
        hidden={!open}
        className="nav-pop nav-float absolute right-0 top-full z-50 mt-2 min-w-[200px] rounded-xl border border-hairline bg-surface p-1"
      >
        {LOCALES.map((option) => {
          const current = option === locale;
          return (
            <li key={option}>
              <a
                href={localePath(option)}
                hrefLang={LOCALE_META[option].hreflang}
                lang={LOCALE_META[option].htmlLang}
                aria-current={current ? "true" : undefined}
                onClick={() => remember(option)}
                className={`flex items-center justify-between gap-3 rounded-lg px-3 py-2 text-[14px] transition-colors duration-150 hover:bg-raised ${
                  current ? "font-medium text-ink" : "text-ink-subtle hover:text-ink"
                }`}
              >
                {LOCALE_META[option].nativeName}
                {current ? (
                  <Check className="size-4 text-accent" aria-hidden />
                ) : null}
              </a>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

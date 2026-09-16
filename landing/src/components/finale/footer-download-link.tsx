"use client";

import { usePathname } from "next/navigation";
import { ArrowRight } from "lucide-react";
import { isLocale } from "@/lib/i18n/config";

/**
 * The footer's closing link. On a home page ("/" or "/<locale>") it jumps to
 * the Download section in place; anywhere else (the English-only
 * /alternatives pages) it goes to the home page's Download section. The path
 * test gives the same answer before and after middleware rewrites "/" onto
 * "/en", so server and client markup agree.
 */
export function FooterDownloadLink({ label }: { label: string }) {
  const segments = (usePathname() ?? "/").split("/").filter(Boolean);
  const home =
    segments.length === 0 || (segments.length === 1 && isLocale(segments[0]));

  return (
    <a
      href={home ? "#download" : "/#download"}
      className="group/dl inline-flex items-center gap-1.5 self-start text-base font-medium text-accent transition-colors duration-150 hover:text-accent-strong sm:self-auto"
    >
      {label}
      <ArrowRight
        aria-hidden
        className="size-4 transition-transform duration-150 ease-out group-hover/dl:translate-x-0.5 motion-reduce:transition-none"
      />
    </a>
  );
}

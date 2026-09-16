"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import { Menu, X } from "lucide-react";

type NavLink = { href: string; label: string };

const PANEL_ID = "site-menu";

/**
 * Disclosure menu for narrow screens (below lg, where the inline links do
 * not fit). The panel stays in the DOM (hidden) so aria-controls always
 * resolves. Escape or picking a link closes it and returns focus to the
 * toggle; a click outside closes it; growing to lg closes it.
 */
export function MobileMenu({ links, label }: { links: NavLink[]; label: string }) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setOpen(false);
      buttonRef.current?.focus();
    };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (panelRef.current?.contains(target) || buttonRef.current?.contains(target)) return;
      setOpen(false);
    };
    const wide = window.matchMedia("(min-width: 1024px)");
    const onWide = () => {
      if (wide.matches) setOpen(false);
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    wide.addEventListener("change", onWide);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
      wide.removeEventListener("change", onWide);
    };
  }, [open]);

  function pick() {
    setOpen(false);
    buttonRef.current?.focus({ preventScroll: true });
  }

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        aria-label={label}
        aria-expanded={open}
        aria-controls={PANEL_ID}
        onClick={() => setOpen((v) => !v)}
        className="nav-press inline-flex size-9 items-center justify-center rounded-lg border border-hairline text-ink-subtle hover:border-hairline-strong hover:text-ink lg:hidden"
      >
        {open ? <X className="size-4" aria-hidden /> : <Menu className="size-4" aria-hidden />}
      </button>

      <div
        ref={panelRef}
        id={PANEL_ID}
        hidden={!open}
        className="nav-panel absolute inset-x-0 top-full border-b border-hairline bg-paper lg:hidden"
      >
        <ul className="wrap py-2">
          {links.map((link) => (
            <li key={link.href} className="border-b border-hairline last:border-b-0">
              <Link
                href={link.href}
                onClick={pick}
                className="flex min-h-12 items-center text-[15px] font-medium text-ink transition-colors duration-150 hover:text-accent"
              >
                {link.label}
              </Link>
            </li>
          ))}
        </ul>
      </div>
    </>
  );
}

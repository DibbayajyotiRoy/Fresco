"use client";

import { useEffect, useRef } from "react";
import Image from "next/image";
import {
  INTRO_KEY,
  finishIntro,
  isIntroPending,
} from "@/components/intro/intro-signal";
import "@/styles/hero.css";

/** Overlay fade + lift, and the faster version when the visitor skips. */
const EXIT_MS = 240;
const SKIP_EXIT_MS = 140;
/** Safety: lift no later than this after hydration, fonts or not. */
const CAP_MS = 1000;

/**
 * First-visit brand intro, about one second: the Fresco mark and wordmark
 * fade and scale in while a thin accent line fills, then the overlay lifts
 * (fade + small rise) and hands over to the hero.
 *
 * All of the visible motion is CSS (styles/hero.css), so it starts at first
 * paint rather than at hydration. This effect only decides when to lift: as
 * soon as the line has filled and web fonts are ready (so the hero never
 * reflows under the reveal), or at CAP_MS. Any key, click, tap or wheel
 * lifts it at once. It then calls finishIntro(), which drops
 * html.intro-pending and releases the hero's entrance.
 *
 * Shown once per session and never under reduced motion or without JS: the
 * head script sets html.intro-pending only then, and clears it itself after
 * 2.5s regardless. Decorative, so aria-hidden.
 */
export function BrandIntro() {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const root = ref.current;
    if (!root || !isIntroPending()) return;
    try {
      sessionStorage.setItem(INTRO_KEY, "1");
    } catch {
      /* private mode: it just shows again next visit */
    }

    let active = true;
    let lifted = false;
    let exitTimer = 0;

    const lift = (fast = false) => {
      if (!active || lifted) return;
      lifted = true;
      const ms = fast ? SKIP_EXIT_MS : EXIT_MS;
      root.style.setProperty("--brand-intro-exit", `${ms}ms`);
      root.classList.add("brand-intro-out");
      exitTimer = window.setTimeout(finishIntro, ms);
    };

    const line = root.querySelector<HTMLElement>(".brand-intro-line");
    const filled = line?.getAnimations()[0]?.finished ?? Promise.resolve();
    const fonts = document.fonts?.ready ?? Promise.resolve();
    Promise.all([filled, fonts]).then(
      () => lift(),
      () => lift(),
    );
    const cap = window.setTimeout(() => lift(), CAP_MS);

    const skip = () => lift(true);
    const events = ["keydown", "pointerdown", "wheel", "touchstart"] as const;
    for (const e of events) window.addEventListener(e, skip, { passive: true });

    /* No finishIntro() here: Strict Mode re-runs this effect in development,
       and a real unmount mid-intro is covered by the head script's cap. */
    return () => {
      active = false;
      window.clearTimeout(cap);
      window.clearTimeout(exitTimer);
      for (const e of events) window.removeEventListener(e, skip);
    };
  }, []);

  return (
    <div ref={ref} className="brand-intro" aria-hidden="true">
      <div className="brand-intro-inner">
        <div className="brand-intro-mark">
          <Image
            src="/logo.png"
            width={44}
            height={44}
            alt=""
            priority
            className="rounded-[11px]"
          />
          <span className="font-display text-[1.75rem] leading-none text-ink">
            Fresco
          </span>
        </div>
        <div className="brand-intro-track">
          <div className="brand-intro-line" />
        </div>
      </div>
    </div>
  );
}

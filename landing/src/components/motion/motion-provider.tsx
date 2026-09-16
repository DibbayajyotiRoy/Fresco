"use client";

import { useEffect, type ReactNode } from "react";
import Lenis from "lenis";
import { gsap, ScrollTrigger } from "@/lib/gsap";
import "lenis/dist/lenis.css";

/**
 * The site's single smooth-scroll engine (Lenis), driven by GSAP's ticker so
 * ScrollTrigger and Lenis agree on every frame. Under prefers-reduced-motion
 * Lenis is never created: the page scrolls natively and nothing is scrubbed.
 * Measurements are refreshed once web fonts and media have settled.
 */
export function MotionProvider({ children }: { children: ReactNode }) {
  useEffect(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const refresh = () => ScrollTrigger.refresh();

    let lenis: Lenis | null = null;
    let tick: ((time: number) => void) | null = null;
    if (!reduce) {
      lenis = new Lenis({
        autoRaf: false,
        lerp: 0.1,
        smoothWheel: true,
        syncTouch: false,
        anchors: true,
      });
      lenis.on("scroll", ScrollTrigger.update);
      const l = lenis;
      tick = (time: number) => l.raf(time * 1000);
      gsap.ticker.add(tick);
      gsap.ticker.lagSmoothing(0);
    }

    document.fonts?.ready.then(refresh).catch(() => {});
    window.addEventListener("load", refresh);

    return () => {
      window.removeEventListener("load", refresh);
      if (tick) gsap.ticker.remove(tick);
      lenis?.destroy();
    };
  }, []);

  return <>{children}</>;
}

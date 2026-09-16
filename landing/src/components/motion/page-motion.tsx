"use client";

import { gsap, useGSAP } from "@/lib/gsap";

/**
 * One orchestrator for the page's declarative reveals. Sections stay server
 * components and opt in with data attributes:
 *
 *   data-reveal="words"    heading built with <SplitWords />: words rise out of their masks
 *   data-reveal="fade"     block rises 28px and fades in
 *   data-reveal="stagger"  direct children rise in sequence
 *   data-reveal="line"     hairline draws from the left
 *   data-parallax="0.2"    drifts by that fraction while crossing the viewport (scrubbed)
 *   data-delay="0.2"       optional delay, seconds
 *
 * Hidden-before-reveal comes from CSS (`html.js-motion [data-reveal]`), which
 * the head script sets only when motion is allowed, with a 4s failsafe that
 * shows everything if this never boots. Here each container is made visible
 * and its parts hidden in the same frame, so nothing flashes. Reduced motion
 * and no-JS visitors get final states, untouched.
 */
export function PageMotion() {
  useGSAP(() => {
    const mm = gsap.matchMedia();

    mm.add("(prefers-reduced-motion: no-preference)", () => {
      for (const el of gsap.utils.toArray<HTMLElement>("[data-reveal]")) {
        const delay = Number(el.dataset.delay ?? 0);
        const scrollTrigger = { trigger: el, start: "top 88%", once: true };
        gsap.set(el, { opacity: 1 });

        switch (el.dataset.reveal) {
          case "words":
            gsap.fromTo(
              el.querySelectorAll(".sw-i"),
              { yPercent: 115 },
              { yPercent: 0, duration: 1.1, ease: "expo.out", stagger: 0.045, delay, scrollTrigger },
            );
            break;
          case "fade":
            gsap.fromTo(
              el,
              { y: 28, autoAlpha: 0 },
              { y: 0, autoAlpha: 1, duration: 1, ease: "power3.out", delay, scrollTrigger },
            );
            break;
          case "stagger":
            gsap.fromTo(
              el.children,
              { y: 28, autoAlpha: 0 },
              { y: 0, autoAlpha: 1, duration: 0.9, ease: "power3.out", stagger: 0.07, delay, scrollTrigger },
            );
            break;
          case "line":
            gsap.fromTo(
              el,
              { scaleX: 0, transformOrigin: "left center" },
              { scaleX: 1, duration: 1.4, ease: "expo.inOut", delay, scrollTrigger },
            );
            break;
        }
      }

      for (const el of gsap.utils.toArray<HTMLElement>("[data-parallax]")) {
        const f = Number(el.dataset.parallax) || 0.15;
        gsap.fromTo(
          el,
          { yPercent: -f * 50 },
          {
            yPercent: f * 50,
            ease: "none",
            scrollTrigger: { trigger: el, start: "top bottom", end: "bottom top", scrub: true },
          },
        );
      }
    });

    document.documentElement.classList.add("motion-ready");
    return () => mm.revert();
  });

  return null;
}

"use client";

import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Viewscreen } from "@/components/showcase/viewscreen";
import type { WidgetId } from "@/components/showcase/widget-ids";

/** Auto-advance interval, in ms, while the stepper is left alone. */
const AUTO_MS = 5000;

export type ShowcaseStep = {
  id: WidgetId;
  title: string;
  body: string;
};

/**
 * "Painted into the wallpaper": a widget screen on the left, a compact ARIA
 * tablist of the five widget notes on the right. Selecting a step (click,
 * arrow keys, or auto-advance) crossfades the matching widget into the
 * screen. Auto-advance runs every `AUTO_MS` only while the section is on
 * screen, the tab is visible, motion is allowed, and the visitor has not
 * touched the stepper yet; any interaction stops it for good.
 *
 * Server markup renders the first step active; the whole thing still works
 * with JS disabled (a tablist with no script is just a labelled list, one
 * panel already open).
 */
export function Showcase({ steps }: { steps: ShowcaseStep[] }) {
  const [active, setActive] = useState(0);
  const [autoEnabled, setAutoEnabled] = useState(true);
  const [inView, setInView] = useState(false);
  const [tabVisible, setTabVisible] = useState(true);
  const [reduceMotion, setReduceMotion] = useState(false);

  const rootRef = useRef<HTMLDivElement>(null);
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);

  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const io = new IntersectionObserver(
      ([entry]) => setInView(entry?.isIntersecting ?? false),
      { threshold: 0.35 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    const onVisibility = () => setTabVisible(!document.hidden);
    onVisibility();
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = () => setReduceMotion(mq.matches);
    onChange();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const playing = autoEnabled && inView && tabVisible && !reduceMotion;

  useEffect(() => {
    if (!playing) return;
    const timer = window.setTimeout(() => {
      setActive((n) => (n + 1) % steps.length);
    }, AUTO_MS);
    return () => window.clearTimeout(timer);
  }, [playing, active, steps.length]);

  const stopAuto = () => setAutoEnabled(false);

  const select = (index: number) => {
    setActive(index);
    stopAuto();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const last = steps.length - 1;
    let next: number | null = null;
    if (event.key === "ArrowDown" || event.key === "ArrowRight") {
      next = active === last ? 0 : active + 1;
    } else if (event.key === "ArrowUp" || event.key === "ArrowLeft") {
      next = active === 0 ? last : active - 1;
    } else if (event.key === "Home") {
      next = 0;
    } else if (event.key === "End") {
      next = last;
    }
    if (next === null) return;
    event.preventDefault();
    select(next);
    tabRefs.current[next]?.focus();
  };

  return (
    <div
      ref={rootRef}
      className="mt-10 grid gap-8 sm:mt-12 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)] lg:items-center lg:gap-12 xl:gap-16"
    >
      <div data-reveal="fade" className="lg:self-start">
        <Viewscreen active={steps[active].id} />
      </div>

      <div
        role="tablist"
        aria-label={steps.map((s) => s.title).join(", ")}
        aria-orientation="vertical"
        data-reveal="fade"
        data-delay="0.1"
        onKeyDown={onKeyDown}
        onPointerDown={stopAuto}
        className="flex flex-col"
      >
        {steps.map((step, i) => {
          const isActive = i === active;
          return (
            <div
              key={step.id}
              className="border-b border-hairline py-4 first:pt-0 last:border-0 last:pb-0"
            >
              <button
                ref={(el) => {
                  tabRefs.current[i] = el;
                }}
                type="button"
                role="tab"
                id={`feature-tab-${step.id}`}
                aria-selected={isActive}
                aria-controls={`feature-panel-${step.id}`}
                tabIndex={isActive ? 0 : -1}
                onClick={() => select(i)}
                onFocus={stopAuto}
                className="flex w-full items-center gap-3 text-start"
              >
                <span
                  aria-hidden
                  className="grid size-7 shrink-0 place-items-center rounded-full border text-sm font-medium tabular-nums transition-colors duration-200 ease-out"
                  data-active={isActive}
                  style={{
                    borderColor: isActive ? "var(--accent)" : "var(--hairline-strong)",
                    color: isActive ? "var(--accent)" : "var(--ink-faint)",
                  }}
                >
                  {i + 1}
                </span>
                <span
                  className={
                    isActive
                      ? "text-lg font-semibold tracking-tight text-ink"
                      : "text-lg font-medium text-ink-subtle"
                  }
                >
                  {step.title}
                </span>
              </button>

              <div
                id={`feature-panel-${step.id}`}
                role="tabpanel"
                aria-labelledby={`feature-tab-${step.id}`}
                hidden={!isActive}
                className="ps-10"
              >
                <p className="mt-2 max-w-md text-base leading-7 text-pretty text-ink-subtle">
                  {step.body}
                </p>
                {playing && (
                  <span
                    aria-hidden
                    key={`${step.id}-${active}`}
                    className="sc-progress mt-3 block h-0.5 w-16 rounded-full bg-accent"
                    style={{ animationDuration: `${AUTO_MS}ms` }}
                  />
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

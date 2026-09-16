"use client";

import { useRef } from "react";
import { gsap, ScrollTrigger, useGSAP } from "@/lib/gsap";
import { Viewscreen } from "@/components/showcase/viewscreen";
import type { WidgetId } from "@/components/showcase/widget-ids";

/* Each widget paints in the way it lives on screen: the lyric card opens from
   its centre, the clock wipes on left to right, the visualiser rises from the
   floor, the record irises open. The shown shapes overshoot the box so the
   widgets' own drop shadows are not clipped. */
const HIDDEN: Record<WidgetId, string> = {
  lyrics: "inset(-20% 50% -20% 50%)",
  clock: "inset(-20% 120% -20% -20%)",
  visualizer: "inset(120% -8% -40% -8%)",
  disc: "circle(0% at 50% 50%)",
};
const SHOWN: Record<WidgetId, string> = {
  lyrics: "inset(-20% -8% -20% -8%)",
  clock: "inset(-20% -20% -20% -20%)",
  visualizer: "inset(-30% -8% -40% -8%)",
  disc: "circle(90% at 50% 50%)",
};

export type ShowcaseStep = {
  id: WidgetId;
  title: string;
  body: string;
};

/**
 * "Painted into the wallpaper": a sticky screen on the left, the four release
 * notes as tall scroll steps on the right. As each step crosses 55% of the
 * viewport its widget paints into the picture (clip-path) and stays;
 * scrolling back up un-paints it. The step being read is numbered in the
 * accent, earlier ones settle, later ones wait.
 *
 * Server markup is the final state: every widget painted, every step at full
 * strength. The choreography exists only on desktop with motion allowed; on
 * phones and under reduced motion the screen is not sticky, all four widgets
 * show at once and the notes list below it.
 */
export function Showcase({ steps }: { steps: ShowcaseStep[] }) {
  const ref = useRef<HTMLDivElement>(null);

  useGSAP(
    () => {
      const root = ref.current;
      if (!root) return;

      const mm = gsap.matchMedia();
      mm.add(
        "(prefers-reduced-motion: no-preference) and (min-width: 1024px)",
        () => {
          const items = gsap.utils.toArray<HTMLElement>("[data-step]", root);
          const ids = items.map((el) => el.dataset.step as WidgetId);
          const widgets = ids.map((id) =>
            root.querySelector<HTMLElement>(`[data-widget="${id}"]`),
          );
          const painted = ids.map(() => true);
          let current = Number.NaN;

          const apply = (n: number) => {
            if (n === current) return;
            current = n;
            items.forEach((item, i) => {
              item.dataset.state = i < n ? "done" : i === n ? "active" : "idle";
              const on = i <= n;
              const widget = widgets[i];
              if (!widget || painted[i] === on) return;
              painted[i] = on;
              gsap.to(
                widget,
                on
                  ? {
                      clipPath: SHOWN[ids[i]],
                      autoAlpha: 1,
                      duration: 0.9,
                      ease: "expo.out",
                      overwrite: true,
                    }
                  : {
                      clipPath: HIDDEN[ids[i]],
                      autoAlpha: 0,
                      duration: 0.3,
                      ease: "power3.out",
                      overwrite: true,
                    },
              );
            });
          };

          // Unpainted until their step is reached.
          widgets.forEach((widget, i) => {
            if (!widget) return;
            gsap.set(widget, { clipPath: HIDDEN[ids[i]], autoAlpha: 0 });
            painted[i] = false;
          });
          apply(-1);

          // One trigger per step, active from its start to the end of the
          // page. Several can flip in one frame (fast scroll, deep link), so
          // the active index is recomputed once after they have all updated.
          let raf = 0;
          const triggers: ScrollTrigger[] = [];
          const sync = () => {
            raf = 0;
            let n = -1;
            triggers.forEach((t, i) => {
              if (t.isActive) n = i;
            });
            apply(n);
          };
          const schedule = () => {
            if (!raf) raf = requestAnimationFrame(sync);
          };
          items.forEach((item) =>
            triggers.push(
              ScrollTrigger.create({
                trigger: item,
                start: "top 55%",
                end: "max",
                onToggle: schedule,
                onRefresh: schedule,
              }),
            ),
          );

          return () => {
            cancelAnimationFrame(raf);
            gsap.killTweensOf(widgets.filter(Boolean));
            gsap.set(widgets.filter(Boolean), {
              clearProps: "clipPath,opacity,visibility",
            });
            items.forEach((item) => delete item.dataset.state);
          };
        },
      );

      return () => mm.revert();
    },
    { scope: ref },
  );

  return (
    <div
      ref={ref}
      className="mt-14 grid gap-12 sm:mt-16 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)] lg:gap-16 xl:gap-20"
    >
      <div className="lg:self-start lg:motion-safe:sticky lg:motion-safe:top-24">
        <Viewscreen />
      </div>

      <ol className="grid gap-10 sm:grid-cols-2 lg:block lg:motion-safe:pb-[16dvh]">
        {steps.map((step, i) => (
          <li
            key={step.id}
            data-step={step.id}
            className="group relative pl-12 lg:pb-14 lg:last:pb-0 lg:motion-safe:min-h-[52dvh] lg:motion-safe:pb-0"
          >
            {/* Rail to the next step; fills in the accent once this one is done. */}
            <span
              aria-hidden
              className="absolute bottom-3 left-4 top-11 hidden w-px bg-hairline group-last:hidden lg:block"
            />
            <span
              aria-hidden
              className="absolute bottom-3 left-4 top-11 hidden w-px origin-top scale-y-0 bg-accent transition-transform duration-500 ease-[cubic-bezier(0.23,1,0.32,1)] group-last:hidden group-data-[state=done]:scale-y-100 lg:block"
            />
            <span
              aria-hidden
              className="absolute left-0 top-0 grid size-8 place-items-center rounded-full border border-hairline-strong bg-surface text-sm font-medium tabular-nums text-ink-muted transition-[background-color,border-color,color] duration-200 ease-out group-data-[state=active]:border-primary group-data-[state=active]:bg-primary group-data-[state=active]:text-primary-foreground group-data-[state=done]:border-accent group-data-[state=done]:text-accent group-data-[state=idle]:text-ink-faint"
            >
              {i + 1}
            </span>
            {/* h4: the showcase sits under the Features h2 and the widgets h3. */}
            <h4 className="pt-1 text-xl font-semibold tracking-tight text-ink transition-colors duration-200 ease-out group-data-[state=idle]:text-ink-subtle">
              {step.title}
            </h4>
            <p className="mt-2 max-w-md text-base leading-7 text-pretty text-ink-subtle">
              {step.body}
            </p>
          </li>
        ))}
      </ol>
    </div>
  );
}

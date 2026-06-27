"use client";

import { useRef } from "react";
import { motion, useReducedMotion } from "motion/react";
import {
  FolderMotion,
  ClickMotion,
  CloseMotion,
  type StepIconHandle,
} from "@/components/animated-step-icons";

const STEPS = [
  {
    n: "01",
    title: "Pick your media",
    description:
      "Open Fresco from your app menu and choose a video, GIF, image, folder, or playlist.",
    Icon: FolderMotion,
  },
  {
    n: "02",
    title: "Click Set",
    description:
      "Set it as your wallpaper. It starts playing on your desktop right away.",
    Icon: ClickMotion,
  },
  {
    n: "03",
    title: "Close the app",
    description:
      "Quit the window. A lightweight daemon keeps the wallpaper running, even after a reboot.",
    Icon: CloseMotion,
  },
] as const;

export function HowItWorks() {
  const reduce = useReducedMotion();
  const ref0 = useRef<StepIconHandle>(null);
  const ref1 = useRef<StepIconHandle>(null);
  const ref2 = useRef<StepIconHandle>(null);
  const refs = [ref0, ref1, ref2];

  return (
    <section
      id="how-it-works"
      className="border-b border-border bg-surface-1/40 py-20 sm:py-28"
    >
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="text-sm font-medium text-ink-subtle">How it works</p>
          <h2 className="mt-2 text-3xl font-semibold tracking-tight text-ink sm:text-4xl">
            Three clicks, then forget about it.
          </h2>
        </div>

        <motion.ol
          className="relative mt-16 grid gap-x-10 gap-y-14 md:grid-cols-3"
          initial={reduce ? false : "hidden"}
          whileInView="show"
          viewport={{ once: true, amount: 0.4 }}
          variants={{ hidden: {}, show: { transition: { staggerChildren: 0.18 } } }}
          onViewportEnter={() => {
            if (reduce) return;
            refs.forEach((r, i) =>
              setTimeout(() => r.current?.play(), 350 + i * 240),
            );
          }}
        >
          {/* Connector line that draws across the nodes on scroll (desktop). */}
          <motion.div
            aria-hidden
            className="pointer-events-none absolute left-[15%] right-[15%] top-7 hidden h-px origin-left bg-gradient-to-r from-hairline-strong via-hairline to-hairline-strong md:block"
            initial={reduce ? false : { scaleX: 0 }}
            whileInView={{ scaleX: 1 }}
            viewport={{ once: true, amount: 0.6 }}
            transition={{ duration: 1, ease: [0.16, 1, 0.3, 1] }}
          />

          {STEPS.map((step, i) => {
            const Icon = step.Icon;
            return (
              <motion.li
                key={step.n}
                className="group relative flex flex-col items-start text-left md:items-center md:text-center"
                variants={{
                  hidden: { opacity: 0, y: 26 },
                  show: {
                    opacity: 1,
                    y: 0,
                    transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] },
                  },
                }}
                onHoverStart={() => refs[i].current?.play()}
              >
                <div className="relative z-10 flex size-14 items-center justify-center rounded-2xl border border-border bg-surface-2 text-ink-muted shadow-none ring-1 ring-inset ring-white/5 transition-colors group-hover:border-hairline-strong">
                  <Icon ref={refs[i]} className="size-7" />
                  <span className="absolute -right-2 -top-2 flex size-5 items-center justify-center rounded-full border border-border/70 bg-background font-mono text-[10px] text-muted-foreground">
                    {i + 1}
                  </span>
                </div>
                <span className="mt-5 font-mono text-xs text-muted-foreground/70">
                  {step.n}
                </span>
                <h3 className="mt-2 text-lg font-semibold tracking-tight">
                  {step.title}
                </h3>
                <p className="mt-2 max-w-xs text-sm text-muted-foreground">
                  {step.description}
                </p>
              </motion.li>
            );
          })}
        </motion.ol>
      </div>
    </section>
  );
}

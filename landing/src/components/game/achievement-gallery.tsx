"use client";

import * as React from "react";
import { Star } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  ACHIEVEMENTS,
  type AchievementId,
  QUEST_KEY,
  XP_TOTAL,
} from "@/lib/game";
import { GITHUB_URL } from "@/lib/site";
import { cn } from "@/lib/utils";

type Earned = Record<AchievementId, boolean>;

function AchievementClientState() {
  const [earned, setEarned] = React.useState<Earned | null>(null);

  React.useEffect(() => {
    try {
      const raw = localStorage.getItem(QUEST_KEY);
      if (raw) setEarned(JSON.parse(raw) as Earned);
    } catch {
      /* ignore */
    }
  }, []);

  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {ACHIEVEMENTS.map((ach) => {
        // earned === null on server + first client render → canonical locked
        // state. Post-hydration the card re-renders with the real persisted
        // state — no text node drift, only class strings change.
        const isUnlocked = earned?.[ach.id] === true;
        const via = ach.via === "view" ? "scroll" : "interact";
        return (
          <div
            key={ach.id}
            className={cn(
              "flex flex-col gap-2 rounded-md border bg-paper p-4 transition-colors",
              isUnlocked
                ? "border-accent/60 bg-accent/[0.04]"
                : "border-hairline",
            )}
          >
            <div className="flex items-start justify-between gap-2">
              <span className="flex items-baseline gap-2">
                <span
                  className={cn(
                    "font-mono text-sm tabular-nums",
                    isUnlocked ? "text-accent" : "text-ink-faint",
                  )}
                >
                  {ach.code}
                </span>
                <span
                  className={cn(
                    "rounded-sm border px-1.5 py-0.5 font-mono text-meta uppercase tracking-wide",
                    isUnlocked
                      ? "border-accent/40 text-accent"
                      : "border-hairline text-ink-faint",
                  )}
                >
                  +{ach.xp} xp
                </span>
              </span>
              {isUnlocked ? (
                <span className="inline-flex items-center gap-1.5 font-mono text-meta uppercase tracking-widest text-accent">
                  <span aria-hidden className="size-1.5 rounded-full bg-accent" />
                  unlocked
                </span>
              ) : (
                <span className="inline-flex items-center rounded-sm border border-dashed border-hairline-strong px-1.5 py-0.5 font-mono text-meta uppercase text-ink-faint">
                  locked
                </span>
              )}
            </div>
            <h3 className="text-base font-medium text-ink">{ach.title}</h3>
            <p className="font-mono text-meta leading-relaxed text-ink-subtle">
              {ach.subtitle}
            </p>
            <span className="mt-2 font-mono text-meta uppercase tracking-widest text-ink-faint">
              via · {via}
            </span>
          </div>
        );
      })}
    </div>
  );
}

export function AchievementGallery() {
  return (
    <section id="quests" className="border-b border-hairline bg-surface py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="instrument-label">
            quests log · {ACHIEVEMENTS.length} missions · {XP_TOTAL} xp total
          </p>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            Your operator console record.
          </h2>
          <p className="mt-4 text-pretty text-ink-subtle">
            Open-source console. Nine missions talk directly to the page itself
            — scroll a section, cast an install, flip the theme, decoder any
            question. The record persists across visits on this device.
          </p>
        </div>

        <div className="mt-10">
          <AchievementClientState />
        </div>

        <div className="mt-10 flex flex-col items-center gap-4 rounded-md border border-hairline bg-paper p-7 text-center">
          <p className="max-w-xl text-pretty text-lg text-ink-muted">
            Fresco is free and GPL-3.0-licensed — a star on GitHub helps other
            Linux users find it.
          </p>
          <Button asChild size="lg" className="font-medium">
            <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer">
              <Star />
              Star on GitHub
            </a>
          </Button>
          <p className="font-mono text-meta uppercase tracking-widest text-ink-faint">
            browser extension: coming soon to the chrome and firefox stores —
            early adopters can{" "}
            <a
              href="https://github.com/DibbayajyotiRoy/fresco/tree/main/extension"
              target="_blank"
              rel="noopener noreferrer"
              className="text-ink-subtle underline decoration-hairline-strong underline-offset-4 transition-colors hover:text-ink"
            >
              load it unpacked
            </a>{" "}
            from github today
          </p>
          <p className="font-mono text-meta uppercase tracking-widest text-ink-faint">
            something missing?{" "}
            <a
              href={`${GITHUB_URL}/issues`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-ink-subtle underline decoration-hairline-strong underline-offset-4 transition-colors hover:text-ink"
            >
              tell us what to improve
            </a>
          </p>
        </div>
      </div>
    </section>
  );
}
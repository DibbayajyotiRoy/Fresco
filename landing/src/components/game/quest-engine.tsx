"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { AnimatedGlyph } from "@/components/animated-glyph";
import {
  ACHIEVEMENTS,
  type Achievement,
  type AchievementId,
  QUEST_EVENT,
  QUEST_KEY,
  XP_BY_ID,
  XP_TOTAL,
  levelFor,
} from "@/lib/game";
import { cn } from "@/lib/utils";
import { COHORT } from "@/lib/site";

type ToastState = {
  code: string;
  title: string;
  subtitle: string;
  xp: number;
} | null;

const BY_ID = Object.fromEntries(
  ACHIEVEMENTS.map((a) => [a.id, a]),
) as Record<AchievementId, Achievement>;

const VIEW_MAP = Object.fromEntries(
  ACHIEVEMENTS.filter((a) => a.via === "view").map((a) => [a.observes, a.id]),
) as Record<string, AchievementId>;

const QUEST_COUNT = ACHIEVEMENTS.length;

export function QuestEngine() {
  const [earned, setEarned] = useState<Record<AchievementId, boolean>>(
    {} as Record<AchievementId, boolean>,
  );
  const [hydrated, setHydrated] = useState(false);
  const [reduceMotion, setReduceMotion] = useState(false);
  const [toast, setToast] = useState<ToastState>(null);

  useEffect(() => {
    setReduceMotion(
      typeof window !== "undefined" &&
        window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    );
    try {
      const raw = localStorage.getItem(QUEST_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as Record<AchievementId, boolean>;
        setEarned(parsed);
      }
    } catch {
      /* ignore */
    }
    setHydrated(true);
  }, []);

  useEffect(() => {
    if (!hydrated) return;
    try {
      localStorage.setItem(QUEST_KEY, JSON.stringify(earned));
    } catch {
      /* ignore */
    }
  }, [earned, hydrated]);

  const unlock = useCallback((id: AchievementId) => {
    setEarned((prev) => {
      if (prev[id]) return prev;
      const ach = BY_ID[id];
      if (ach) {
        setToast({
          code: ach.code,
          title: ach.title,
          subtitle: ach.subtitle,
          xp: ach.xp,
        });
      }
      return { ...prev, [id]: true };
    });
  }, []);

  useEffect(() => {
    if (!hydrated) return;
    const observers: IntersectionObserver[] = [];
    for (const [observeId, achId] of Object.entries(VIEW_MAP)) {
      const el = document.getElementById(observeId);
      if (!el) continue;
      const obs = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            if (entry.isIntersecting) {
              unlock(achId);
              obs.disconnect();
            }
          }
        },
        { rootMargin: "-15% 0px -15% 0px", threshold: 0.05 },
      );
      obs.observe(el);
      observers.push(obs);
    }
    return () => observers.forEach((o) => o.disconnect());
  }, [hydrated, unlock]);

  useEffect(() => {
    if (!hydrated) return;
    function onQuest(event: Event) {
      const id = (event as CustomEvent<AchievementId>).detail;
      const ach = BY_ID[id];
      if (!ach || ach.via !== "event") return;
      unlock(id);
    }
    window.addEventListener(QUEST_EVENT, onQuest);
    return () => window.removeEventListener(QUEST_EVENT, onQuest);
  }, [hydrated, unlock]);

  useEffect(() => {
    if (!toast) return;
    const t = window.setTimeout(() => setToast(null), 4200);
    return () => window.clearTimeout(t);
  }, [toast]);

  const xp = useMemo(() => {
    let total = 0;
    for (const id of Object.keys(earned) as AchievementId[]) {
      if (earned[id]) total += XP_BY_ID[id];
    }
    return total;
  }, [earned]);

  const { level, next } = levelFor(xp);
  const questsUnlocked = useMemo(
    () => Object.values(earned).filter(Boolean).length,
    [earned],
  );
  const champion = xp >= XP_TOTAL;
  const pct = Math.min(100, Math.round((xp / XP_TOTAL) * 100));

  return (
    <>
      <a
        href="#quests"
        aria-label={`Fresco operator console. ${level.name}. ${xp} of ${XP_TOTAL} XP. ${questsUnlocked} of ${QUEST_COUNT} quests unlocked.`}
        className="fixed bottom-4 right-4 z-40 hidden w-[288px] items-center gap-3 rounded-md border border-hairline bg-surface/95 p-3 shadow-lg backdrop-blur sm:flex"
      >
        <span
          className="flex size-12 shrink-0 items-center justify-center rounded-md border border-accent/40 bg-accent/10"
          aria-hidden
        >
          <AnimatedGlyph
            name={champion ? "breathe" : "scanline"}
            active={!champion && !reduceMotion}
            staticChar="●"
            className="text-accent"
          />
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex items-center justify-between gap-2">
            <span className="instrument-label !text-ink-muted">
              {level.name}
            </span>
            <span className="font-mono text-meta tabular-nums text-ink-subtle">
              {xp}/{XP_TOTAL}
            </span>
          </span>
          <span
            className="mt-1 block h-1 rounded-full bg-raised"
            role="progressbar"
            aria-valuenow={pct}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <span
              className="block h-full rounded-full bg-accent transition-[width] duration-300"
              style={{ width: `${pct}%` }}
            />
          </span>
          <span className="mt-1.5 block font-mono text-meta uppercase tracking-wide text-ink-faint">
            {next
              ? `${questsUnlocked}/${QUEST_COUNT} quests · next: ${next.min - xp} xp`
              : `${questsUnlocked}/${QUEST_COUNT} quests · max rank`}
          </span>
          <span className="mt-0.5 block font-mono text-meta uppercase tracking-widest text-ink-faint">
            cohort · {COHORT.users} operators · {COHORT.deploys} deploys
          </span>
        </span>
      </a>

      {toast && (
        <div
          role="status"
          aria-live="polite"
          className={cn(
            "fixed bottom-4 left-1/2 z-50 w-[340px] -translate-x-1/2 overflow-hidden rounded-sm border border-hairline bg-surface shadow-lg sm:bottom-20 sm:left-auto sm:right-4 sm:translate-x-0",
            !reduceMotion && "animate-[rise-in_160ms_var(--ease-house)]",
          )}
        >
          <span className="flex">
            <span className="w-1 shrink-0 bg-accent" />
            <span className="min-w-0 flex-1">
              <span className="flex items-center gap-2 border-b border-hairline bg-accent/10 px-3 py-1.5">
                <AnimatedGlyph
                  name="pulse"
                  active={!reduceMotion}
                  staticChar="●"
                  className="text-accent"
                />
                <span className="instrument-label !text-accent">
                  QUEST UNLOCKED · +{toast.xp} XP
                </span>
              </span>
              <span className="block px-3 py-2.5">
                <span className="flex items-baseline gap-2">
                  <span className="font-mono text-sm text-accent">
                    {toast.code}
                  </span>
                  <span className="text-sm font-medium text-ink">
                    {toast.title}
                  </span>
                </span>
                <span className="mt-0.5 block font-mono text-meta text-ink-subtle">
                  {toast.subtitle}
                </span>
              </span>
            </span>
          </span>
        </div>
      )}
    </>
  );
}
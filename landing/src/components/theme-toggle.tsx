"use client";

import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";
import { usePlaySound } from "@/hooks/use-play-sound";
import { dispatchQuest } from "@/lib/game";

export function ThemeToggle() {
  const [dark, setDark] = useState<boolean | null>(null);
  const { play } = usePlaySound({ sound: "interaction.toggle" });

  useEffect(() => {
    setDark(document.documentElement.classList.contains("dark"));
  }, []);

  function toggle() {
    const next = !(dark ?? false);
    const root = document.documentElement;
    root.classList.toggle("dark", next);
    root.style.colorScheme = next ? "dark" : "light";
    try {
      localStorage.setItem("fresco.theme", next ? "dark" : "light");
    } catch {
      /* ignore */
    }
    setDark(next);
    play();
    dispatchQuest("night");
  }

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={dark ? "Switch to light theme" : "Switch to dark theme"}
      className="flex size-8 items-center justify-center rounded-sm border border-hairline text-ink-subtle transition-colors hover:bg-raised hover:text-ink"
    >
      {dark === null ? (
        <span className="font-mono text-meta text-ink-faint" aria-hidden>
          ◐
        </span>
      ) : dark ? (
        <Sun className="size-4" aria-hidden />
      ) : (
        <Moon className="size-4" aria-hidden />
      )}
    </button>
  );
}
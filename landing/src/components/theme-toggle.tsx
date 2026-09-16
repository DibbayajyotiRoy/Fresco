"use client";

import { Moon, Sun } from "lucide-react";
import { usePlaySound } from "@/hooks/use-play-sound";

/**
 * Sun/moon icon button. Toggles html.dark, sets color-scheme, persists
 * `fresco.theme` and plays interaction.toggle. The icon is chosen by CSS
 * from html.dark (which the head script sets before paint), so it is right
 * on first paint with no hydration flash: sun in light, moon in dark.
 */
export function ThemeToggle({ label }: { label: string }) {
  const { play } = usePlaySound({ sound: "interaction.toggle" });

  function toggle() {
    const root = document.documentElement;
    const next = !root.classList.contains("dark");
    root.classList.toggle("dark", next);
    root.style.colorScheme = next ? "dark" : "light";
    try {
      localStorage.setItem("fresco.theme", next ? "dark" : "light");
    } catch {
      /* ignore */
    }
    play();
  }

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={label}
      className="nav-press inline-flex size-9 items-center justify-center rounded-lg border border-hairline text-ink-subtle hover:border-hairline-strong hover:text-ink"
    >
      <Sun className="size-4 dark:hidden" aria-hidden />
      <Moon className="hidden size-4 dark:block" aria-hidden />
    </button>
  );
}

"use client";

import * as React from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Bell,
  Bug,
  ChartBar,
  ChatCircle,
  Desktop,
  Images,
  Moon,
  ShieldWarning,
  SpeakerHigh,
  SpeakerSlash,
  SquaresFour,
  Sun,
} from "@phosphor-icons/react/dist/ssr";

import { playNavRun, setSoundEnabled, soundEnabled } from "@/lib/sound";

const NAV = [
  { title: "Overview", href: "/", icon: SquaresFour },
  { title: "Catalog", href: "/catalog", icon: Images },
  { title: "Notifications", href: "/notifications", icon: Bell },
  { title: "Feedback", href: "/feedback", icon: ChatCircle },
  { title: "Usage", href: "/usage", icon: ChartBar },
  { title: "Reliability", href: "/reliability", icon: ShieldWarning },
  { title: "Issues", href: "/issues", icon: Bug },
] as const;

type ThemeMode = "light" | "dark" | "system";

function applyTheme(mode: ThemeMode) {
  const dark =
    mode === "dark" ||
    (mode === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

/** Theme toggle cycling light → dark → system, persisted as "theme.mode". */
function ThemeToggle() {
  const [mode, setMode] = React.useState<ThemeMode | null>(null);

  React.useEffect(() => {
    const stored = localStorage.getItem("theme.mode") as ThemeMode | null;
    setMode(stored === "light" || stored === "dark" ? stored : "system");
  }, []);

  React.useEffect(() => {
    if (mode !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme("system");
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [mode]);

  function cycle() {
    const next: ThemeMode =
      mode === "light" ? "dark" : mode === "dark" ? "system" : "light";
    setMode(next);
    localStorage.setItem("theme.mode", next);
    applyTheme(next);
  }

  return (
    <button
      type="button"
      onClick={cycle}
      title={`Theme: ${mode ?? "system"} (click to cycle)`}
      className="flex h-7 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-2 font-mono text-meta tracking-wide text-stone-500 uppercase transition-colors hover:bg-stone-100"
    >
      {mode === "light" ? (
        <Sun className="size-3.5" weight="fill" />
      ) : mode === "dark" ? (
        <Moon className="size-3.5" weight="fill" />
      ) : (
        <Desktop className="size-3.5" />
      )}
      <span className="hidden sm:inline">{mode ?? "…"}</span>
    </button>
  );
}

/** Sound toggle — persisted, default on; sounds already no-op under
 *  prefers-reduced-motion regardless of this switch. */
function SoundToggle() {
  const [on, setOn] = React.useState<boolean | null>(null);

  React.useEffect(() => {
    setOn(soundEnabled());
  }, []);

  function toggle() {
    const next = !(on ?? true);
    setOn(next);
    setSoundEnabled(next);
  }

  return (
    <button
      type="button"
      onClick={toggle}
      title={on ? "Sound on" : "Sound off"}
      aria-pressed={on ?? undefined}
      className="flex h-7 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-2 font-mono text-meta tracking-wide text-stone-500 uppercase transition-colors hover:bg-stone-100"
    >
      {on ? (
        <SpeakerHigh className="size-3.5" weight="fill" />
      ) : (
        <SpeakerSlash className="size-3.5" />
      )}
      <span className="hidden sm:inline">{on == null ? "…" : on ? "snd" : "mute"}</span>
    </button>
  );
}

export function Topbar() {
  const pathname = usePathname();

  return (
    <header className="sticky top-0 z-40 border-b border-stone-200 bg-white/95 backdrop-blur">
      <div className="mx-auto flex h-14 max-w-[1600px] items-center gap-4 px-4">
        <Link href="/" className="flex items-center gap-2">
          <span
            className="inline-block size-[10px] bg-sky-600"
            aria-hidden
          />
          <span className="font-serif text-lg text-stone-900">Fresco</span>
          <span className="font-mono text-meta tracking-widest text-stone-400 uppercase">
            admin
          </span>
        </Link>

        <nav
          aria-label="Primary"
          className="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto"
        >
          {NAV.map((item) => {
            const active =
              item.href === "/"
                ? pathname === "/"
                : pathname.startsWith(item.href);
            return (
              <Link
                key={item.href}
                href={item.href}
                aria-current={active ? "page" : undefined}
                onClick={() => {
                  if (!active) playNavRun();
                }}
                className={
                  "flex h-7 shrink-0 items-center gap-1.5 rounded-md px-2 text-sm font-medium transition-colors " +
                  (active
                    ? "bg-stone-100 text-stone-900"
                    : "text-stone-500 hover:bg-stone-100 hover:text-stone-900")
                }
              >
                <item.icon
                  className={
                    "size-3.5 " + (active ? "text-sky-600" : "text-stone-400")
                  }
                  weight={active ? "fill" : "regular"}
                />
                <span className="hidden md:inline">{item.title}</span>
              </Link>
            );
          })}
        </nav>

        <div className="flex shrink-0 items-center gap-1.5">
          <SoundToggle />
          <ThemeToggle />
        </div>
      </div>
    </header>
  );
}

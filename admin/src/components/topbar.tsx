"use client";

import * as React from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Bell,
  Bug,
  ChartBar,
  ChatCircle,
  ChatsCircle,
  Desktop,
  Images,
  Moon,
  ShieldWarning,
  SpeakerHigh,
  SpeakerSlash,
  SquaresFour,
  Sun,
} from "@phosphor-icons/react/dist/ssr";

import {
  SupportBadge,
  supportNavLabel,
  useSupportUnread,
} from "@/components/support-badge";
import { playNavRun, setSoundEnabled, soundEnabled } from "@/lib/sound";

const NAV = [
  { title: "Overview", href: "/", icon: SquaresFour },
  { title: "Catalog", href: "/catalog", icon: Images },
  { title: "Notifications", href: "/notifications", icon: Bell },
  { title: "Feedback", href: "/feedback", icon: ChatCircle },
  { title: "Support", href: "/support", icon: ChatsCircle },
  { title: "Usage", href: "/usage", icon: ChartBar },
  { title: "Reliability", href: "/reliability", icon: ShieldWarning },
  { title: "Issues", href: "/issues", icon: Bug },
] as const;

/* The two chrome toggles are the same control in two costumes, so the chrome
 * lives here once. `hover:text-foreground` rather than `hover:text-stone-900`:
 * the dark remap in globals.css keys off the bare `.text-stone-900` class and
 * never sees the `hover:` variant, so a stone hover colour would stay light-mode
 * ink in dark and vanish into the fill. Semantic tokens flip with `.dark`. */
const TOGGLE =
  "press flex h-7 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-2 font-mono text-meta tracking-wide text-stone-500 uppercase " +
  "transition-[color,background-color,border-color,transform] duration-150 ease-hover hover:bg-stone-100 hover:text-foreground";

type ThemeMode = "light" | "dark" | "system";

function applyTheme(mode: ThemeMode) {
  const dark =
    mode === "dark" ||
    (mode === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

/**
 * True once the page has scrolled away from the top.
 *
 * The header is sticky, so its divider is only doing work when there is content
 * sliding underneath it. At rest the hairline just chops the page in two below
 * the wordmark for no reason; revealing it on scroll is what makes the header
 * read as *lifted above* the page rather than welded to the top of it.
 */
function useScrolled() {
  const [scrolled, setScrolled] = React.useState(false);

  React.useEffect(() => {
    // A 2px deadzone: overscroll bounce and sub-pixel scroll restoration should
    // not flicker the divider on and off.
    const read = () => setScrolled(window.scrollY > 2);
    read();
    window.addEventListener("scroll", read, { passive: true });
    return () => window.removeEventListener("scroll", read);
  }, []);

  return scrolled;
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
      /* The label collapses below sm:, which would otherwise leave an icon-only
       * button with no accessible name at exactly the width where it matters. */
      aria-label={`Theme: ${mode ?? "system"}. Click to cycle.`}
      className={TOGGLE}
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
      /* `aria-pressed` already carries the state, so the name stays constant —
       * a name that changes with the state gets announced twice over. */
      aria-label="Sound effects"
      aria-pressed={on ?? undefined}
      className={TOGGLE}
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
  const supportUnread = useSupportUnread();
  const scrolled = useScrolled();

  return (
    <header
      className={
        "sticky top-0 z-40 border-b bg-white/95 backdrop-blur transition-[border-color,box-shadow] duration-200 ease-hover " +
        (scrolled ? "border-stone-200 shadow-panel" : "border-transparent")
      }
    >
      <div className="mx-auto flex h-14 max-w-[1600px] items-center gap-4 px-4">
        <Link href="/" className="press flex items-center gap-2 rounded-md">
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
            const badged = item.href === "/support";
            const count = badged ? supportUnread : null;
            return (
              <Link
                key={item.href}
                href={item.href}
                aria-current={active ? "page" : undefined}
                aria-label={supportNavLabel(item.title, count)}
                onClick={() => {
                  if (!active) playNavRun();
                }}
                /* Every item carries a transparent hairline so the active one
                 * can take a real border without the row reflowing. That border
                 * is the state's only carrier that survives the dark remap:
                 * `bg-stone-100` (active) and `hover:bg-stone-100` both resolve
                 * to #292524 in dark, so fill alone made hover indistinguishable
                 * from selection. */
                className={
                  "press flex h-7 shrink-0 items-center gap-1.5 rounded-md border px-2 text-sm font-medium " +
                  "transition-[color,background-color,border-color,transform] duration-150 ease-hover " +
                  (active
                    ? "border-stone-300 bg-stone-100 text-stone-900"
                    : "border-transparent text-stone-500 hover:bg-stone-100 hover:text-foreground")
                }
              >
                <item.icon
                  className={
                    "size-3.5 transition-colors duration-150 ease-hover " +
                    (active ? "text-sky-600" : "text-stone-400")
                  }
                  weight={active ? "fill" : "regular"}
                />
                <span className="hidden md:inline">{item.title}</span>
                <SupportBadge count={count} />
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

"use client";

import Image from "next/image";
import { useEffect, useRef, useState } from "react";
import { Download, Github } from "lucide-react";
import { Button } from "@/components/ui/button";
import { AnimatedGlyph } from "@/components/animated-glyph";
import { dispatchQuest } from "@/lib/game";
import { GITHUB_URL } from "@/lib/site";

const BOOT_LINES = [
  "frescod v1.1.1 spinning up",
  "probe session… wayland detected",
  "va-api driver ready",
  "control socket: EFFECTIVE",
  "renderer ONLINE · cpu 0.4%",
  "console online",
] as const;

export function BootConsole() {
  const [revealed, setRevealed] = useState(0);
  const [done, setDone] = useState(false);
  const dispatchedRef = useRef(false);

  useEffect(() => {
    const reduce = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    if (reduce) {
      setRevealed(BOOT_LINES.length);
      setDone(true);
      return;
    }
    let cancelled = false;
    const timers: number[] = [];
    let i = 0;
    const tick = () => {
      if (cancelled) return;
      i += 1;
      setRevealed(i);
      if (i >= BOOT_LINES.length) {
        timers.push(
          window.setTimeout(() => {
            if (cancelled) return;
            setDone(true);
          }, 400),
        );
        return;
      }
      timers.push(window.setTimeout(tick, 400));
    };
    timers.push(window.setTimeout(tick, 280));
    return () => {
      cancelled = true;
      timers.forEach((t) => window.clearTimeout(t));
    };
  }, []);

  useEffect(() => {
    if (done && !dispatchedRef.current) {
      dispatchedRef.current = true;
      dispatchQuest("boot");
    }
  }, [done]);

  return (
    <section id="top" className="relative border-b border-hairline">
      <div className="mx-auto grid max-w-6xl gap-12 px-5 pb-20 pt-12 sm:pt-20 lg:grid-cols-2 lg:items-center">
        <div className="flex flex-col items-start">
          <p className="instrument-label mb-6 inline-flex items-center gap-2 rounded-full border border-hairline bg-surface px-3 py-1">
            <AnimatedGlyph
              name="scanline"
              className="text-accent"
              staticChar="●"
            />
            console · gpl-3.0 · linux
          </p>

          <h1 className="max-w-3xl text-balance font-serif text-display text-ink">
            Live wallpaper,{" "}
            <em className="italic text-accent">decoded.</em>
          </h1>

          <p className="mt-6 max-w-[580px] text-pretty text-lg text-ink-subtle">
            Set any video, GIF, or image as your Linux desktop.
            Hardware-accelerated playback keeps CPU near zero, on X11 and
            Wayland. Close the app; the daemon keeps it playing.
          </p>

          <div className="mt-8 flex flex-col gap-3 sm:flex-row">
            <Button asChild size="lg" className="font-medium">
              <a href="#download">
                <Download />
                Install Fresco
              </a>
            </Button>
            <Button
              asChild
              size="lg"
              variant="secondary"
              className="font-medium"
            >
              <a
                href={GITHUB_URL}
                target="_blank"
                rel="noopener noreferrer"
              >
                <Github />
                Star on GitHub
              </a>
            </Button>
          </div>
        </div>

        <div className="w-full max-w-md justify-self-end lg:ml-auto">
          <div className="rounded-lg border border-stone-800 bg-terminal text-stone-200 shadow-lg overflow-hidden">
            <div className="flex items-center justify-between px-3 py-2 border-b border-stone-800">
              <span className="flex items-center gap-2">
                <AnimatedGlyph
                  name={done ? "breathe" : "scanline"}
                  active={!done}
                  staticChar="●"
                  className="text-sky-400"
                />
                <span className="font-mono text-meta uppercase tracking-widest text-stone-400">
                  frescod · startup
                </span>
              </span>
              <span className="flex items-center gap-1.5" aria-hidden>
                <span className="size-2 rounded-full border border-stone-700" />
                <span className="size-2 rounded-full border border-stone-700" />
                <span className="size-2 rounded-full border border-stone-700" />
              </span>
            </div>
            <div className="px-4 py-4 font-mono text-sm leading-relaxed">
              {BOOT_LINES.map((line, idx) => {
                if (idx >= revealed) return null;
                return (
                  <div key={line} className="text-stone-200">
                    <span className="text-stone-500 select-none">▸ </span>
                    {line}
                  </div>
                );
              })}
              {done && (
                <span className="mt-3 flex items-center gap-1.5 font-mono text-meta uppercase tracking-widest text-sky-400">
                  <span aria-hidden>●</span> operator session ready
                </span>
              )}
            </div>
          </div>
        </div>
      </div>

      <div className="mx-auto max-w-6xl px-5 pb-12">
        <div className="mx-auto max-w-4xl">
          <div className="rounded-lg border border-hairline bg-surface p-2">
            <div className="flex items-center justify-between px-2 py-1.5">
              <span className="instrument-label">fresco — library</span>
              <span className="flex items-center gap-1.5" aria-hidden>
                <span className="size-2 rounded-full border border-hairline-strong" />
                <span className="size-2 rounded-full border border-hairline-strong" />
                <span className="size-2 rounded-full border border-hairline-strong" />
              </span>
            </div>
            <div className="overflow-hidden rounded-md border border-hairline">
              <Image
                src="/screenshots/gallery.png"
                alt="Fresco wallpaper library on Linux showing video wallpapers with an active live wallpaper and hardware decode status"
                width={1280}
                height={720}
                priority
                className="h-auto w-full"
              />
            </div>
          </div>
          <p className="mt-3 text-center font-mono text-meta uppercase tracking-widest text-ink-faint">
            console preview — fresco library view (debian 12, wayland)
          </p>
        </div>
      </div>
    </section>
  );
}
"use client";

import { Volume2, VolumeX } from "lucide-react";
import { useFrescoSound } from "@/components/sound-provider";

/** Persisted sound toggle ("fresco.sound", default on). Icon-only: the
 *  translated label names it, aria-pressed carries the state. Silent by
 *  design: flipping it plays nothing, sound only confirms semantic events. */
export function SoundToggle({ label }: { label: string }) {
  const { on, setOn } = useFrescoSound();
  const Icon = on ? Volume2 : VolumeX;

  return (
    <button
      type="button"
      onClick={() => setOn(!on)}
      aria-pressed={on}
      aria-label={label}
      title={label}
      className="inline-flex size-9 items-center justify-center rounded-lg border border-hairline text-ink-subtle transition-colors duration-150 hover:border-hairline-strong hover:bg-raised hover:text-ink"
    >
      <Icon className="size-4" aria-hidden />
    </button>
  );
}

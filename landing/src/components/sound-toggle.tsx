"use client";

import { useFrescoSound } from "@/components/sound-provider";

/** Persisted sound toggle ("fresco.sound", default on). Silent by design:
 *  flipping it plays nothing — sound only ever confirms semantic events. */
export function SoundToggle() {
  const { on, setOn } = useFrescoSound();

  return (
    <button
      type="button"
      onClick={() => setOn(!on)}
      aria-pressed={on}
      className="font-mono text-meta uppercase tracking-widest text-ink-faint transition-colors hover:text-ink"
    >
      sound: {on ? "on" : "off"}
    </button>
  );
}

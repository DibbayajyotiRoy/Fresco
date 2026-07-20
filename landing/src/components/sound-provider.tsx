"use client";

import * as React from "react";
import { SensoryUIProvider } from "@/lib/provider";

/**
 * Sound layer (Warm Terminal §6), powered by the sensory-ui engine
 * (runtime-synthesized WebAudio, zero assets). Exactly two semantic events on
 * this site: "hero.complete" on copy-install-command and "interaction.toggle"
 * on theme toggle. Never hover, never scroll, never every click.
 *
 * The persisted user toggle ("fresco.sound", default on) drives the
 * provider's enabled flag; prefers-reduced-motion is inherited as a hard
 * no-op inside the engine. Sounds only ever fire from user gestures.
 */
const KEY = "fresco.sound";

const FrescoSoundContext = React.createContext<{
  on: boolean;
  setOn: (on: boolean) => void;
}>({ on: true, setOn: () => {} });

export function useFrescoSound() {
  return React.useContext(FrescoSoundContext);
}

export function SoundProvider({ children }: { children: React.ReactNode }) {
  const [on, setOnState] = React.useState(true);

  React.useEffect(() => {
    try {
      setOnState(localStorage.getItem(KEY) !== "off");
    } catch {
      /* default on */
    }
  }, []);

  const setOn = React.useCallback((next: boolean) => {
    setOnState(next);
    try {
      localStorage.setItem(KEY, next ? "on" : "off");
    } catch {
      /* ignore */
    }
  }, []);

  const ctx = React.useMemo(() => ({ on, setOn }), [on, setOn]);

  return (
    <FrescoSoundContext.Provider value={ctx}>
      <SensoryUIProvider
        config={{
          enabled: on,
          volume: 0.25,
          theme: "crisp",
          categories: {
            interaction: true,
            navigation: true,
            hero: true,
            overlay: false,
            notification: false,
          },
          reducedMotion: "inherit",
        }}
      >
        {children}
      </SensoryUIProvider>
    </FrescoSoundContext.Provider>
  );
}

"use client";

/* Warm Terminal sound cues (§6), played through the sensory-ui engine
 * (lazy singleton AudioContext, resume-if-suspended, cancel-previous —
 * spam-click safe). The cues themselves are custom SoundSynthesizer
 * functions: triangle oscillators, 5ms linear attack → exponential decay
 * to 0.001, master volume 0.25. Zero audio assets, CSP-safe.
 *
 * Sound confirms navigation-level transitions only — never hover, never
 * every click, never errors. Gated by a persisted user toggle (default on)
 * and a hard no-op under prefers-reduced-motion. */

import {
  playSound,
  type PlaySoundOptions,
  type SoundPlayback,
  type SoundSynthesizer,
} from "@/lib/engine";

const STORAGE_KEY = "fresco-admin.sound";
const MASTER = 0.25;

export function soundEnabled(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(STORAGE_KEY) !== "off";
}

export function setSoundEnabled(on: boolean) {
  localStorage.setItem(STORAGE_KEY, on ? "on" : "off");
}

function reducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** One triangle note into `out`: 5ms attack → exp decay to 0.001. */
function note(
  ctx: AudioContext,
  out: GainNode,
  freq: number,
  at: number,
  dur: number,
  gain: number
): OscillatorNode {
  const osc = ctx.createOscillator();
  const g = ctx.createGain();
  osc.type = "triangle";
  osc.frequency.value = freq;
  g.gain.setValueAtTime(0.0001, at);
  g.gain.linearRampToValueAtTime(gain, at + 0.005);
  g.gain.exponentialRampToValueAtTime(0.001, at + dur);
  osc.connect(g);
  g.connect(out);
  osc.start(at);
  osc.stop(at + dur + 0.02);
  return osc;
}

function synthFromNotes(
  freqs: number[],
  spacing: number,
  dur: number,
  gainMult: number
): SoundSynthesizer {
  return (ctx: AudioContext, opts: PlaySoundOptions): SoundPlayback => {
    const master = ctx.createGain();
    master.gain.value = MASTER * (opts.volume ?? 1);
    master.connect(ctx.destination);
    const now = ctx.currentTime;
    const oscs = freqs.map((f, i) =>
      note(ctx, master, f, now + i * spacing, dur, gainMult)
    );
    const last = oscs[oscs.length - 1];
    if (last) last.onended = () => opts.onEnd?.();
    return {
      stop: () => {
        for (const o of oscs) {
          try {
            o.stop();
          } catch {
            /* already stopped */
          }
        }
      },
    };
  };
}

/* Pentatonic C5 D5 E5 G5 A5 C6 */
const RUN = [523.25, 587.33, 659.25, 783.99, 880.0, 1046.5];

const navRunSynth = synthFromNotes(RUN, 0.08, 0.18, 0.8);
const tickSynth = synthFromNotes([659.25, 880.0], 0.07, 0.09, 0.5);

function fire(synth: SoundSynthesizer) {
  if (!soundEnabled() || reducedMotion()) return;
  playSound(synth).catch(() => {});
}

/** Page-navigation confirmation: pentatonic run at 0.8× gain, 80ms spacing. */
export function playNavRun() {
  fire(navRunSynth);
}

/** Tab / filter-group switch: short two-note tick. */
export function playTick() {
  fire(tickSynth);
}

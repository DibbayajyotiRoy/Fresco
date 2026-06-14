"use client";

import { forwardRef, useImperativeHandle } from "react";
import {
  motion,
  useAnimationControls,
  useReducedMotion,
  type Variants,
} from "motion/react";

/**
 * Animated step icons in the lucide-animated / pqoqubbw style: each exposes a
 * `play()` handle through its ref (so the stepper can fire them when it scrolls
 * into view), and also animates on hover. Paths are Lucide geometry. Everything
 * collapses to a static icon under prefers-reduced-motion.
 */
export type StepIconHandle = { play: () => void };
type IconProps = { className?: string };

const SVG = {
  width: 24,
  height: 24,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

/** Step 1: a folder that lifts and bounces open. */
export const FolderMotion = forwardRef<StepIconHandle, IconProps>(
  function FolderMotion({ className }, ref) {
    const controls = useAnimationControls();
    const reduce = useReducedMotion();
    useImperativeHandle(ref, () => ({
      play: () => {
        if (!reduce) controls.start("anim");
      },
    }));
    return (
      <motion.svg
        {...SVG}
        className={className}
        aria-hidden
        initial="normal"
        animate={controls}
        variants={{
          normal: { rotate: 0, y: 0, scale: 1 },
          anim: {
            rotate: [0, -9, 4, 0],
            y: [0, -2, 0],
            scale: [1, 1.08, 1],
            transition: { duration: 0.6, ease: "easeInOut" },
          },
        }}
        onMouseEnter={() => {
          if (!reduce) controls.start("anim");
        }}
      >
        <path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2" />
      </motion.svg>
    );
  },
);

/** Step 2: a pointer with click sparks that pulse. */
export const ClickMotion = forwardRef<StepIconHandle, IconProps>(
  function ClickMotion({ className }, ref) {
    const controls = useAnimationControls();
    const reduce = useReducedMotion();
    useImperativeHandle(ref, () => ({
      play: () => {
        if (!reduce) controls.start("anim");
      },
    }));
    const spark = (delay: number): Variants => ({
      normal: { opacity: 1, scale: 1 },
      anim: {
        opacity: [1, 0, 1],
        scale: [1, 0.2, 1],
        transition: { duration: 0.55, delay, ease: "easeInOut" },
      },
    });
    return (
      <motion.svg
        {...SVG}
        className={className}
        aria-hidden
        initial="normal"
        animate={controls}
        variants={{ normal: {}, anim: {} }}
        onMouseEnter={() => {
          if (!reduce) controls.start("anim");
        }}
      >
        <motion.path d="M14 4.1 12 6" variants={spark(0)} />
        <motion.path d="m5.1 8-2.9-.8" variants={spark(0.05)} />
        <motion.path d="m6 12-1.9 2" variants={spark(0.1)} />
        <motion.path d="M7.2 2.2 8 5.1" variants={spark(0.15)} />
        <motion.path
          d="M9.037 9.69a.498.498 0 0 1 .653-.653l11 4.5a.5.5 0 0 1-.074.949l-4.349 1.041a1 1 0 0 0-.74.739l-1.04 4.35a.5.5 0 0 1-.95.074z"
          variants={{
            normal: { x: 0, y: 0, scale: 1 },
            anim: {
              x: [0, 1.6, 0],
              y: [0, 1.6, 0],
              scale: [1, 0.9, 1],
              transition: { duration: 0.45, ease: "easeInOut" },
            },
          }}
        />
      </motion.svg>
    );
  },
);

/** Step 3: an X that redraws and spins shut. */
export const CloseMotion = forwardRef<StepIconHandle, IconProps>(
  function CloseMotion({ className }, ref) {
    const controls = useAnimationControls();
    const reduce = useReducedMotion();
    useImperativeHandle(ref, () => ({
      play: () => {
        if (!reduce) controls.start("anim");
      },
    }));
    return (
      <motion.svg
        {...SVG}
        className={className}
        aria-hidden
        initial="normal"
        animate={controls}
        variants={{
          normal: { rotate: 0, scale: 1 },
          anim: {
            rotate: [0, 90, 180],
            scale: [1, 0.85, 1],
            transition: { duration: 0.6, ease: "easeInOut" },
          },
        }}
        onMouseEnter={() => {
          if (!reduce) controls.start("anim");
        }}
      >
        <motion.path
          d="M18 6 6 18"
          variants={{
            normal: { pathLength: 1, opacity: 1 },
            anim: {
              pathLength: [0, 1],
              opacity: [0.3, 1],
              transition: { duration: 0.45, ease: "easeInOut" },
            },
          }}
        />
        <motion.path
          d="m6 6 12 12"
          variants={{
            normal: { pathLength: 1, opacity: 1 },
            anim: {
              pathLength: [0, 1],
              opacity: [0.3, 1],
              transition: { duration: 0.45, delay: 0.08, ease: "easeInOut" },
            },
          }}
        />
      </motion.svg>
    );
  },
);

"use client";

import * as React from "react";
import * as SwitchPrimitive from "@radix-ui/react-switch";

import { cn } from "@/lib/utils";

/**
 * Toggle, at house density and on the house accent.
 *
 * Three corrections to the stock shadcn version. It used `--primary` for the
 * checked fill, which is near-black ink here and reads as "disabled" rather
 * than "on" — sky is the one interactive accent, and a switch is the most
 * interactive control on the page. It used `transition-all`, which animates
 * every property including layout ones. And it had no press feedback, so
 * publishing a catalog item felt like nothing happened until the row updated.
 *
 * The thumb keeps its own transition rather than inheriting: it moves, while
 * the track only changes colour, and those want different curves.
 *
 * The checked fill stays sky-600 in dark mode rather than lifting to sky-400
 * the way sky text and borders do. That lift exists because thin strokes need
 * more luminance to read on a dark ground; a filled area does not, and 600
 * keeps a white thumb legible against it.
 */
function Switch({
  className,
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "peer inline-flex h-[18px] w-8 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-[2px] outline-none",
        "bg-stone-200 data-[state=checked]:bg-sky-600",
        "transition-colors duration-150 ease-out",
        // Press feedback on the whole control — see `.press` in globals.css.
        "press",
        "disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className={cn(
          "pointer-events-none block size-3.5 rounded-full bg-white shadow-sm ring-0",
          // The signature curve, so the thumb settles rather than stopping.
          "transition-transform duration-200 ease-[cubic-bezier(0.16,1,0.3,1)]",
          "data-[state=unchecked]:translate-x-0 data-[state=checked]:translate-x-[14px]"
        )}
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };

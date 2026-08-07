import * as React from "react";

import { cn } from "@/lib/utils";

/**
 * Text input, at house density.
 *
 * Restyled off the stock shadcn defaults, which fought the design system in
 * three ways: `h-9` (36px) next to 28px chrome controls, a `ring-[3px]` focus
 * halo drawn on top of the global 2px sky `outline` from globals.css — two
 * focus indicators on the same element — and semantic `border-input` tokens
 * where every other component here authors light-only stone utilities and
 * lets the dark remap do the rest.
 *
 * 32px rather than the chrome's 28px on purpose: this is a target you click
 * into and type in, not a button you glance at.
 */
function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        "flex h-8 w-full min-w-0 rounded-md border border-stone-200 bg-white px-2.5 text-sm text-stone-900 outline-none",
        "transition-colors duration-150 ease-out",
        "placeholder:text-stone-400",
        "hover:border-stone-300",
        // Focus is the global rule in globals.css — a 2px sky outline at 2px
        // offset. Nothing is added here; a second indicator only muddies it.
        "disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50",
        "aria-invalid:border-rose-500",
        "file:mr-2 file:inline-flex file:h-6 file:items-center file:border-0 file:bg-transparent file:p-0 file:text-sm file:font-medium file:text-stone-700",
        className
      )}
      {...props}
    />
  );
}

export { Input };

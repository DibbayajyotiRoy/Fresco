import * as React from "react";

import { cn } from "@/lib/utils";

/**
 * Multi-line input, matched to `Input`.
 *
 * Same three corrections as `Input`: house stone utilities instead of shadcn
 * semantic tokens, no `ring-[3px]` competing with the global focus outline,
 * and the house 13px type. `field-sizing-content` is kept — a reply box that
 * grows with the reply is exactly right for the support inbox, which is where
 * this is used.
 */
function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-16 w-full rounded-md border border-stone-200 bg-white px-2.5 py-2 text-sm leading-relaxed text-stone-900 outline-none",
        "transition-colors duration-150 ease-out",
        "placeholder:text-stone-400",
        "hover:border-stone-300",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "aria-invalid:border-rose-500",
        className
      )}
      {...props}
    />
  );
}

export { Textarea };

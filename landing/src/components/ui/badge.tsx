import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

/* Badges are instrument chips: mono 11px uppercase, hairline border, never
   interactive. The accent variant is reserved for the one accent lane. */
const badgeVariants = cva(
  "inline-flex items-center rounded-sm border px-2 py-0.5 font-mono text-meta font-medium uppercase tracking-wide",
  {
    variants: {
      variant: {
        default: "border-accent/40 bg-accent/10 text-accent",
        secondary: "border-hairline bg-raised text-ink-subtle",
        destructive: "border-danger/40 bg-danger/10 text-danger",
        outline: "border-hairline text-ink-subtle",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge, badgeVariants };

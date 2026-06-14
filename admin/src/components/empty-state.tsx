import type { Icon } from "@phosphor-icons/react";
import { WarningCircle } from "@phosphor-icons/react/dist/ssr";

import { cn } from "@/lib/utils";

export function EmptyState({
  title,
  description,
  icon: Icon = WarningCircle,
  className,
}: {
  title: string;
  description?: string;
  icon?: Icon;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-white/10 px-6 py-14 text-center",
        className
      )}
    >
      <div className="bg-muted/50 text-muted-foreground flex size-10 items-center justify-center rounded-full backdrop-blur-sm">
        <Icon className="size-5" weight="duotone" />
      </div>
      <div className="space-y-1">
        <p className="text-sm font-medium">{title}</p>
        {description ? (
          <p className="text-muted-foreground max-w-sm text-sm">{description}</p>
        ) : null}
      </div>
    </div>
  );
}

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
        "border-border flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed px-6 py-12 text-center",
        className
      )}
    >
      <div className="bg-muted text-muted-foreground flex size-10 items-center justify-center rounded-full">
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

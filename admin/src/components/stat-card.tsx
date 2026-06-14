import type { Icon } from "@phosphor-icons/react";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export function StatCard({
  label,
  value,
  hint,
  icon: Icon,
}: {
  label: string;
  value: string;
  hint?: string;
  icon: Icon;
}) {
  return (
    <Card className="gap-0 py-5">
      <CardHeader className="px-5">
        <CardDescription className="flex items-center gap-2 font-mono text-xs font-medium tracking-wide uppercase">
          <Icon className="text-brand size-4" weight="duotone" />
          {label}
        </CardDescription>
        <CardTitle className="mt-2 font-serif text-4xl font-semibold tabular-nums tracking-tight">
          {value}
        </CardTitle>
      </CardHeader>
      {hint ? (
        <CardContent className="px-5 pt-1">
          <p className="text-muted-foreground text-xs">{hint}</p>
        </CardContent>
      ) : null}
    </Card>
  );
}

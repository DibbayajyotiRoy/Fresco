import type { Icon } from "@phosphor-icons/react";
import { StatCard as RoyStatCard } from "@roy-ui/ui/stat-card";

export function StatCard({
  label,
  value,
  hint,
  icon: Icon,
  data,
}: {
  label: string;
  value: string;
  hint?: string;
  icon: Icon;
  /** Optional real sparkline series (>= 2 points). Never fabricated. */
  data?: number[];
}) {
  return (
    <RoyStatCard
      label={label}
      value={value}
      sub={hint}
      data={data}
      icon={<Icon weight="duotone" />}
      color="var(--brand)"
    />
  );
}

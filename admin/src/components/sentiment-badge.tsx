import { ThumbsDown, ThumbsUp } from "@phosphor-icons/react/dist/ssr";

import { cn } from "@/lib/utils";

export function SentimentBadge({ rating }: { rating: number }) {
  const up = rating > 0;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs font-medium",
        up
          ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-500"
          : "border-rose-500/30 bg-rose-500/10 text-rose-500"
      )}
    >
      {up ? (
        <ThumbsUp className="size-3" weight="bold" />
      ) : (
        <ThumbsDown className="size-3" weight="bold" />
      )}
      {up ? "Up" : "Down"}
    </span>
  );
}

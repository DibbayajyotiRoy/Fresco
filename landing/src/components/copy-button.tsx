"use client";

import * as React from "react";
import { Copy } from "lucide-react";
import { cn } from "@/lib/utils";
import { usePlaySound } from "@/hooks/use-play-sound";

export function CopyButton({
  value,
  className,
}: {
  value: string;
  className?: string;
}) {
  const [copied, setCopied] = React.useState(false);
  const { play } = usePlaySound({ sound: "hero.complete" });

  function onCopy() {
    navigator.clipboard.writeText(value).then(() => {
      setCopied(true);
      play();
      setTimeout(() => setCopied(false), 1600);
    });
  }

  return (
    <button
      type="button"
      onClick={onCopy}
      aria-label={copied ? "Copied" : "Copy to clipboard"}
      className={cn(
        "flex size-7 shrink-0 items-center justify-center rounded-sm border transition-colors",
        copied
          ? "border-ok/40 text-ok"
          : "border-stone-700 text-stone-400 hover:bg-stone-800 hover:text-stone-200",
        className
      )}
    >
      {copied ? (
        <span className="font-mono text-sm leading-none" aria-hidden>
          ✓
        </span>
      ) : (
        <Copy className="size-3.5" aria-hidden />
      )}
    </button>
  );
}
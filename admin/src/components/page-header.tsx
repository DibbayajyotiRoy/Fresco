import { SidebarTrigger } from "@/components/ui/sidebar";
import { Separator } from "@/components/ui/separator";

export function PageHeader({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: React.ReactNode;
}) {
  return (
    <header className="bg-background/60 sticky top-0 z-20 flex h-14 shrink-0 items-center gap-2 border-b border-white/10 px-4 backdrop-blur-xl">
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-1 h-5" />
      <div className="flex min-w-0 flex-col">
        <h1 className="truncate font-serif text-base font-medium leading-tight tracking-tight">
          {title}
        </h1>
        {description ? (
          <p className="text-muted-foreground truncate text-xs leading-tight">
            {description}
          </p>
        ) : null}
      </div>
      <div className="ml-auto flex items-center gap-3">
        <LiveIndicator />
        {action ? <div className="flex items-center gap-2">{action}</div> : null}
      </div>
    </header>
  );
}

/** Subtle pulsing dot + "Live" — the data auto-refreshes on an interval. */
function LiveIndicator() {
  return (
    <span className="border-brand/30 bg-brand/10 text-brand inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-xs font-medium">
      <span className="relative flex size-1.5">
        <span className="bg-brand absolute inline-flex size-full animate-ping rounded-full opacity-75" />
        <span className="bg-brand relative inline-flex size-1.5 rounded-full" />
      </span>
      Live
    </span>
  );
}

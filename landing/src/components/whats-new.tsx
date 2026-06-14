import { Palette, Images, Gauge, MousePointerClick, ArrowUpRight, Sparkles } from "lucide-react";
import { WHATS_NEW } from "@/lib/content";
import { GITHUB_URL } from "@/lib/site";

const ICONS: Record<string, React.ComponentType<{ className?: string }>> = {
  palette: Palette,
  images: Images,
  gauge: Gauge,
  pointer: MousePointerClick,
};

export function WhatsNew({ version }: { version: string }) {
  return (
    <section id="whats-new" className="border-b border-border/60 py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="rounded-2xl border border-border/70 bg-secondary/20 p-8 sm:p-12">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
            <div className="max-w-2xl">
              <span className="inline-flex items-center gap-1.5 rounded-full border border-primary/40 bg-primary/10 px-3 py-1 text-xs font-medium text-primary">
                <Sparkles className="size-3.5" aria-hidden />
                New in v{version}
              </span>
              <h2 className="mt-4 text-3xl font-semibold tracking-tight sm:text-4xl">
                Theming, polish, and a big performance pass.
              </h2>
            </div>
            <a
              href={`${GITHUB_URL}/blob/main/CHANGELOG.md`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex shrink-0 items-center gap-1 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
            >
              Full changelog
              <ArrowUpRight className="size-4" aria-hidden />
            </a>
          </div>

          <div className="mt-10 grid gap-x-8 gap-y-8 sm:grid-cols-2 lg:grid-cols-4">
            {WHATS_NEW.map((item) => {
              const Icon = ICONS[item.icon] ?? Sparkles;
              return (
                <div key={item.title}>
                  <div className="flex size-10 items-center justify-center rounded-lg bg-primary/15 text-primary">
                    <Icon className="size-5" />
                  </div>
                  <h3 className="mt-4 text-sm font-semibold tracking-tight">
                    {item.title}
                  </h3>
                  <p className="mt-1.5 text-sm text-muted-foreground">{item.body}</p>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </section>
  );
}

import { Layers, Volume2, RotateCw, PauseCircle, ArrowUpRight, Sparkles } from "lucide-react";
import { WHATS_NEW } from "@/lib/content";
import { GITHUB_URL } from "@/lib/site";

const ICONS: Record<string, React.ComponentType<{ className?: string }>> = {
  wayland: Layers,
  audio: Volume2,
  rotate: RotateCw,
  pause: PauseCircle,
};

export function WhatsNew({ version }: { version: string }) {
  return (
    <section id="whats-new" className="border-b border-border py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="rounded-2xl border border-border bg-surface-1 p-8 shadow-[inset_0_1px_0_rgba(255,255,255,0.05)] sm:p-12">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
            <div className="max-w-2xl">
              <span className="inline-flex items-center gap-1.5 rounded-full border border-lavender/40 bg-lavender/10 px-3 py-1 text-xs font-medium text-lavender-hover">
                <Sparkles className="size-3.5" aria-hidden />
                New in v{version}
              </span>
              <h2 className="mt-4 text-3xl font-semibold tracking-tight text-ink sm:text-4xl">
                Wayland, sound, and rotation just landed.
              </h2>
            </div>
            <a
              href={`${GITHUB_URL}/blob/main/CHANGELOG.md`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex shrink-0 items-center gap-1 text-sm font-medium text-ink-subtle transition-colors hover:text-ink"
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
                  <div className="flex size-10 items-center justify-center rounded-lg border border-border bg-surface-2 text-ink-muted">
                    <Icon className="size-5" />
                  </div>
                  <h3 className="mt-4 text-sm font-semibold tracking-tight text-ink">
                    {item.title}
                  </h3>
                  <p className="mt-1.5 text-sm text-ink-subtle">{item.body}</p>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </section>
  );
}

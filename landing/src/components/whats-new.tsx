import {
  LayoutGrid,
  MonitorSmartphone,
  SunMoon,
  Gauge,
  ArrowUpRight,
  Sparkles,
} from "lucide-react";
import { WHATS_NEW } from "@/lib/content";
import { GITHUB_URL } from "@/lib/site";

const ICONS: Record<string, React.ComponentType<{ className?: string }>> = {
  catalog: LayoutGrid,
  displays: MonitorSmartphone,
  schedule: SunMoon,
  quality: Gauge,
};

export function WhatsNew({ version }: { version: string }) {
  return (
    <section id="whats-new" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="rounded-md border border-hairline bg-surface p-8 sm:p-12">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
            <div className="max-w-2xl">
              <p className="instrument-label !text-ink-faint">
                what&apos;s new · v{version}
              </p>
              <h2 className="mt-4 font-serif text-display-sm text-ink">
                The catalog, per-display wallpapers, and schedules just landed.
              </h2>
              <p className="mt-3 max-w-2xl text-ink-subtle">
                What shipped in v{version}. Each entry here is reproduced in the
                CHANGELOG on GitHub.
              </p>
            </div>
            <a
              href={`${GITHUB_URL}/blob/main/CHANGELOG.md`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex shrink-0 items-center gap-1 font-mono text-meta uppercase tracking-widest text-ink-subtle transition-colors hover:text-ink"
            >
              Full changelog
              <ArrowUpRight className="size-3.5" aria-hidden />
            </a>
          </div>

          <div className="mt-10 grid gap-x-8 gap-y-8 sm:grid-cols-2 lg:grid-cols-4">
            {WHATS_NEW.map((item, i) => {
              const Icon = ICONS[item.icon] ?? Sparkles;
              return (
                <div key={item.title}>
                  <div className="flex size-9 items-center justify-center rounded-sm border border-hairline bg-raised text-ink-muted">
                    <Icon className="size-4" aria-hidden />
                  </div>
                  <span className="instrument-label !text-ink-faint mt-4 block">
                    patch {String(i + 1).padStart(2, "0")}
                  </span>
                  <h3 className="mt-1.5 text-sm font-semibold text-ink">
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
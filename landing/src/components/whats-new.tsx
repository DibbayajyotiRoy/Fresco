import { ArrowUpRight, MicVocal, Clock, AudioLines, Disc3 } from "lucide-react";
import { GITHUB_URL } from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";

/** Order and iconography of the release highlights; copy comes from the dict. */
const ITEMS = [
  { id: "lyrics", Icon: MicVocal },
  { id: "clock", Icon: Clock },
  { id: "visualizer", Icon: AudioLines },
  { id: "disc", Icon: Disc3 },
] as const;

export function WhatsNew({
  version,
  dict,
}: {
  version: string;
  dict: Dictionary;
}) {
  return (
    <section id="whats-new" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="rounded-md border border-hairline bg-surface p-8 sm:p-12">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
            <div className="max-w-2xl">
              <p className="instrument-label !text-ink-faint">
                {dict.whatsNew.kicker(version)}
              </p>
              <h2 className="mt-4 font-serif text-display-sm text-ink">
                {dict.whatsNew.title}
              </h2>
              <p className="mt-3 max-w-2xl text-ink-subtle">
                {dict.whatsNew.lead(version)}
              </p>
            </div>
            <a
              href={`${GITHUB_URL}/blob/main/CHANGELOG.md`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex shrink-0 items-center gap-1 font-mono text-meta uppercase tracking-widest text-ink-subtle transition-colors hover:text-ink"
            >
              {dict.whatsNew.changelog}
              <ArrowUpRight className="size-3.5" aria-hidden />
            </a>
          </div>

          <div className="mt-10 grid gap-x-8 gap-y-8 sm:grid-cols-2 lg:grid-cols-4">
            {ITEMS.map(({ id, Icon }, i) => {
              const item = dict.whatsNew.items[id];
              return (
                <div key={id}>
                  <div className="flex size-9 items-center justify-center rounded-sm border border-hairline bg-raised text-ink-muted">
                    <Icon className="size-4" aria-hidden />
                  </div>
                  <span className="instrument-label !text-ink-faint mt-4 block">
                    {dict.whatsNew.patch(String(i + 1).padStart(2, "0"))}
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

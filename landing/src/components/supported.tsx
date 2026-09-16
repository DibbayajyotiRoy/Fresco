import { SplitWords } from "@/components/motion/split-words";
import { DISTROS } from "@/lib/content";
import type { Dictionary } from "@/lib/i18n";

function Dot({ live }: { live: boolean }) {
  return (
    <span
      aria-hidden
      className={`size-2 shrink-0 rounded-full ${live ? "bg-ok" : "bg-warn"}`}
    />
  );
}

/**
 * Where Fresco runs, as a compact band: title and lead, one row of compositor
 * chips (green live, amber fallback, each with an sr-only status), and the
 * tested distros on one muted line. Sessions, formats and the Deepin/Treeland
 * caveat live in At a glance and the FAQ.
 */
export function Supported({ dict }: { dict: Dictionary }) {
  const s = dict.supported;

  /** Proper nouns, identical in every locale, except the translated X11 row. */
  const compositors: { name: string; live: boolean }[] = [
    { name: "COSMIC", live: true },
    { name: "Hyprland", live: true },
    { name: "Sway", live: true },
    { name: "KDE Plasma 6", live: true },
    { name: s.sessions.x11.label, live: true },
    { name: "Deepin DDE", live: true },
    { name: "GNOME Wayland", live: false },
  ];

  return (
    <section
      id="supported"
      aria-labelledby="supported-title"
      className="border-b border-hairline py-20 sm:py-24"
    >
      <div className="wrap">
        <div className="mx-auto max-w-6xl">
          <header className="grid gap-5 lg:grid-cols-12 lg:items-end lg:gap-12">
            <h2
              id="supported-title"
              data-reveal="words"
              className="font-display text-section text-ink lg:col-span-5"
            >
              <SplitWords text={s.title} />
            </h2>
            <p
              data-reveal="fade"
              data-delay="0.15"
              className="text-lg text-ink-subtle lg:col-span-7"
            >
              {s.lead}
            </p>
          </header>

          <div className="mt-10 flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between lg:gap-8">
            <ul className="flex flex-wrap gap-2">
              {compositors.map((c) => (
                <li
                  key={c.name}
                  className="inline-flex items-center gap-2 rounded-full border border-hairline bg-surface px-3 py-1.5 text-sm font-medium text-ink-muted"
                >
                  <Dot live={c.live} />
                  {c.name}
                  <span className="sr-only">
                    : {c.live ? s.live : s.fallback}
                  </span>
                </li>
              ))}
            </ul>
            {/* Visible key for the dots; each chip already announces its own
                status, so this is hidden from assistive tech. */}
            <p
              aria-hidden
              className="flex shrink-0 items-center gap-4 text-sm text-ink-subtle"
            >
              <span className="inline-flex items-center gap-1.5">
                <Dot live />
                {s.live}
              </span>
              <span className="inline-flex items-center gap-1.5">
                <Dot live={false} />
                {s.fallback}
              </span>
            </p>
          </div>

          <p className="mt-5 text-sm text-ink-faint">
            <span className="mr-3 inline-block font-medium text-ink-subtle first-letter:uppercase">
              {s.distrosTitle(DISTROS.length)}
            </span>
            {DISTROS.join(" · ")}
          </p>
        </div>
      </div>
    </section>
  );
}

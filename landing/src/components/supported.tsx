import { DISTROS, FORMATS, TESTIMONIAL } from "@/lib/content";
import type { Dictionary } from "@/lib/i18n";

/** Session rows, in order, with whether each gets a live wallpaper. */
const SESSIONS = [
  { id: "x11", ok: true },
  { id: "deepin", ok: true },
  { id: "wayland", ok: true },
  { id: "gnome", ok: false },
] as const;

/** Compositor health strip. Names are proper nouns, identical in every locale. */
const COMPOSITORS: { name: string; live: boolean }[] = [
  { name: "cosmic", live: true },
  { name: "hyprland", live: true },
  { name: "sway", live: true },
  { name: "kde plasma 6", live: true },
  { name: "x11", live: true },
  { name: "deepin dde", live: true },
  { name: "gnome wayland", live: false },
];

function HealthDot({
  name,
  live,
  liveLabel,
  fallbackLabel,
}: {
  name: string;
  live: boolean;
  liveLabel: string;
  fallbackLabel: string;
}) {
  return (
    <span className="inline-flex items-center gap-1.5 font-mono text-meta uppercase tracking-widest text-ink-subtle">
      <span
        aria-hidden
        className={`size-1.5 rounded-full ${live ? "bg-ok" : "bg-warn"}`}
      />
      {name}
      <span className="sr-only">: {live ? liveLabel : fallbackLabel}</span>
    </span>
  );
}

export function Supported({ dict }: { dict: Dictionary }) {
  return (
    <section id="supported" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
            <p className="instrument-label !text-ink-faint">
              {dict.supported.kicker}
            </p>
          </div>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            {dict.supported.title}
          </h2>
          <p className="mt-4 max-w-2xl text-pretty text-ink-subtle">
            {dict.supported.lead}
          </p>
          <p className="mt-3 font-mono text-meta uppercase tracking-widest text-ink-faint">
            {dict.supported.deployed(DISTROS.length, FORMATS.length)}
          </p>
        </div>

        <div className="mt-8 flex flex-wrap items-center gap-x-5 gap-y-3 rounded-sm border border-hairline bg-surface px-4 py-3">
          {COMPOSITORS.map((c) => (
            <HealthDot
              key={c.name}
              {...c}
              liveLabel={dict.supported.live}
              fallbackLabel={dict.supported.fallback}
            />
          ))}
        </div>

        <div className="mt-4 grid gap-4 lg:grid-cols-2">
          <div className="rounded-md border border-hairline bg-surface p-7">
            <h3 className="instrument-label">{dict.supported.sessionsTitle}</h3>
            <ul className="mt-5 flex flex-col gap-4">
              {SESSIONS.map((s) => {
                const copy = dict.supported.sessions[s.id];
                return (
                  <li key={s.id} className="flex gap-3">
                    <span
                      aria-hidden
                      className={`mt-1 font-mono text-sm leading-none ${
                        s.ok ? "text-ok" : "text-ink-faint"
                      }`}
                    >
                      {s.ok ? "✓" : "—"}
                    </span>
                    <span>
                      <span className="text-sm font-medium text-ink">
                        {copy.label}
                      </span>
                      <span className="block text-sm text-ink-subtle">
                        {copy.detail}
                      </span>
                      <span className="sr-only">
                        {s.ok ? dict.supported.live : dict.supported.fallback}
                      </span>
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>

          <div className="rounded-md border border-hairline bg-surface p-7">
            <p className="instrument-label mt-0">
              {dict.supported.distrosTitle(DISTROS.length)}
            </p>
            <ul className="mt-5 flex flex-wrap gap-2">
              {DISTROS.map((d) => (
                <li
                  key={d}
                  className="rounded-sm border border-hairline bg-raised px-2 py-0.5 font-mono text-meta text-ink-muted"
                >
                  {d}
                </li>
              ))}
            </ul>

            <p className="instrument-label mt-7">
              {dict.supported.formatsTitle(FORMATS.length)}
            </p>
            <ul className="mt-5 flex flex-wrap gap-2">
              {FORMATS.map((f) => (
                <li
                  key={f}
                  className="rounded-sm border border-hairline bg-raised px-2 py-0.5 font-mono text-meta text-ink-muted"
                >
                  {f}
                </li>
              ))}
            </ul>
          </div>
        </div>

        {/* The quote itself is reproduced verbatim, in the language the
            reviewer wrote it in, on every locale. Translating a testimonial
            would misquote a named person. */}
        <figure className="mt-4 rounded-md border border-hairline bg-surface">
          <figcaption className="instrument-label border-b border-hairline px-4 py-2.5">
            {dict.supported.fieldReport}
          </figcaption>
          <div className="grid gap-6 p-7 lg:grid-cols-[1fr_260px] lg:gap-8">
            <blockquote>
              <p
                lang="en"
                className="text-pretty font-serif text-xl leading-snug text-ink sm:text-2xl"
              >
                &ldquo;{TESTIMONIAL.quote}&rdquo;
              </p>
              <p className="mt-4 text-sm text-ink-muted">
                {TESTIMONIAL.author}
                <span className="block text-ink-subtle">
                  {dict.supported.testimonialRole}
                </span>
              </p>
            </blockquote>

            <dl className="self-start rounded-sm border border-hairline bg-raised px-4 py-3">
              <p className="instrument-label">{dict.supported.verifiedEnv}</p>
              <div className="mt-3 flex flex-col gap-2">
                {TESTIMONIAL.environment.map((row) => (
                  <div key={row.id} className="flex flex-col gap-0.5">
                    <dt className="font-mono text-meta uppercase tracking-wide text-ink-faint">
                      {dict.supported.envLabels[row.id]}
                    </dt>
                    <dd className="font-mono text-meta text-ink-muted">
                      {row.value}
                    </dd>
                  </div>
                ))}
              </div>
            </dl>
          </div>
        </figure>

        <p className="mt-4 font-mono text-meta tracking-wide text-ink-faint">
          {dict.supported.footnote}
        </p>
      </div>
    </section>
  );
}

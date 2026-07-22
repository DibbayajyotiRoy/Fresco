import { DISTROS, FORMATS } from "@/lib/content";

const SESSIONS: { label: string; detail: string; ok: boolean }[] = [
  {
    label: "X11 (any desktop)",
    detail: "GNOME, KDE, XFCE, MATE, Cinnamon, Budgie",
    ok: true,
  },
  {
    label: "Deepin 25 (DDE)",
    detail: "Auto DDE adaptation — icons stay visible",
    ok: true,
  },
  {
    label: "Wayland layer-shell",
    detail: "COSMIC, Hyprland, Sway, KDE Plasma 6, wlroots",
    ok: true,
  },
  {
    label: "GNOME on Wayland",
    detail: "Static-frame fallback (Mutter has no live surface)",
    ok: false,
  },
];

const COMPOSITORS: { name: string; live: boolean }[] = [
  { name: "cosmic", live: true },
  { name: "hyprland", live: true },
  { name: "sway", live: true },
  { name: "kde plasma 6", live: true },
  { name: "x11", live: true },
  { name: "deepin dde", live: true },
  { name: "gnome wayland", live: false },
];

function HealthDot({ name, live }: { name: string; live: boolean }) {
  return (
    <span className="inline-flex items-center gap-1.5 font-mono text-meta uppercase tracking-widest text-ink-subtle">
      <span
        aria-hidden
        className={`size-1.5 rounded-full ${live ? "bg-ok" : "bg-warn"}`}
      />
      {name}
      <span className="sr-only">
        {live ? ": live wallpaper" : ": static fallback"}
      </span>
    </span>
  );
}

export function Supported() {
  return (
    <section id="supported" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
            <p className="instrument-label !text-ink-faint">
              DEPLOYED ENVIRONMENTS
            </p>
          </div>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            Where Fresco runs.
          </h2>
          <p className="mt-4 max-w-2xl text-pretty text-ink-subtle">
            On any X11 desktop — including Deepin 25&apos;s DDE — and on Wayland
            layer-shell compositors — COSMIC, Hyprland, Sway, and KDE Plasma 6 —
            across the popular Debian and Ubuntu distributions. GNOME Wayland
            gets a static-frame fallback.
          </p>
          <p className="mt-3 font-mono text-meta uppercase tracking-widest text-ink-faint">
            deployed: 6 live compositors · 1 static fallback · {DISTROS.length}{" "}
            distros · {FORMATS.length} formats
          </p>
        </div>

        <div className="mt-8 flex flex-wrap items-center gap-x-5 gap-y-3 rounded-sm border border-hairline bg-surface px-4 py-3">
          {COMPOSITORS.map((c) => (
            <HealthDot key={c.name} {...c} />
          ))}
        </div>

        <div className="mt-4 grid gap-4 lg:grid-cols-2">
          <div className="rounded-md border border-hairline bg-surface p-7">
            <h3 className="instrument-label">sessions and compositors</h3>
            <ul className="mt-5 flex flex-col gap-4">
              {SESSIONS.map((s) => (
                <li key={s.label} className="flex gap-3">
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
                      {s.label}
                    </span>
                    <span className="block text-sm text-ink-subtle">
                      {s.detail}
                    </span>
                    <span className="sr-only">
                      {s.ok ? "Live wallpaper" : "Static fallback"}
                    </span>
                  </span>
                </li>
              ))}
            </ul>
          </div>

          <div className="rounded-md border border-hairline bg-surface p-7">
            <p className="instrument-label mt-0">
              tested distributions · {DISTROS.length}
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
              supported formats · {FORMATS.length}
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
      </div>
    </section>
  );
}
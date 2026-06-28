import { Check, Minus } from "lucide-react";
import { DISTROS, FORMATS } from "@/lib/content";

/**
 * Distro + session/compositor support, as crawlable content. Targets the
 * long-tail queries people actually search (live wallpaper hyprland, kde plasma
 * live wallpaper, sway, cosmic, pop os, mint) that the homepage otherwise only
 * mentions in passing.
 */
const SESSIONS: { label: string; detail: string; ok: boolean }[] = [
  {
    label: "X11 (any desktop)",
    detail: "GNOME, KDE, XFCE, MATE, Cinnamon, Budgie",
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

export function Supported() {
  return (
    <section id="supported" className="border-b border-border py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="text-sm font-medium text-ink-subtle">Compatibility</p>
          <h2 className="mt-2 text-3xl font-semibold tracking-tight text-ink sm:text-4xl">
            Where Fresco runs.
          </h2>
          <p className="mt-4 text-pretty text-ink-subtle">
            Live wallpapers on X11 and on Wayland layer-shell compositors, across
            the popular Debian and Ubuntu based distributions.
          </p>
        </div>

        <div className="mt-10 grid gap-5 lg:grid-cols-2">
          {/* Sessions / compositors */}
          <div className="rounded-2xl border border-border bg-surface-1 p-7">
            <h3 className="text-sm font-medium text-ink-subtle">
              Sessions and compositors
            </h3>
            <ul className="mt-5 flex flex-col gap-4">
              {SESSIONS.map((s) => (
                <li key={s.label} className="flex gap-3">
                  <span
                    className={`mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-full border ${
                      s.ok
                        ? "border-border bg-surface-2 text-ink"
                        : "border-border bg-surface-2 text-ink-tertiary"
                    }`}
                  >
                    {s.ok ? (
                      <Check className="size-3" aria-label="Live wallpaper" />
                    ) : (
                      <Minus className="size-3" aria-label="Static fallback" />
                    )}
                  </span>
                  <span>
                    <span className="text-sm font-medium text-ink">
                      {s.label}
                    </span>
                    <span className="block text-sm text-ink-subtle">
                      {s.detail}
                    </span>
                  </span>
                </li>
              ))}
            </ul>
          </div>

          {/* Distros + formats */}
          <div className="rounded-2xl border border-border bg-surface-1 p-7">
            <h3 className="text-sm font-medium text-ink-subtle">
              Tested distributions
            </h3>
            <ul className="mt-5 flex flex-wrap gap-2">
              {DISTROS.map((d) => (
                <li
                  key={d}
                  className="rounded-md border border-border bg-surface-2 px-2.5 py-1 text-xs font-medium text-ink-muted"
                >
                  {d}
                </li>
              ))}
            </ul>

            <h3 className="mt-7 text-sm font-medium text-ink-subtle">
              Supported formats
            </h3>
            <ul className="mt-5 flex flex-wrap gap-2">
              {FORMATS.map((f) => (
                <li
                  key={f}
                  className="rounded-md border border-border bg-surface-2 px-2.5 py-1 text-xs font-medium text-ink-muted"
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

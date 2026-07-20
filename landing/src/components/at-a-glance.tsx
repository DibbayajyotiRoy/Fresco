import { CopyButton } from "@/components/copy-button";
import {
  GITHUB_URL,
  INSTALL_ONELINER,
  INSTALL_ONELINER_COPY,
} from "@/lib/site";

/**
 * "Fresco at a glance" — a compact, quote-verbatim definition block for
 * answer engines and skimmers: what it is, platforms, license, install.
 * Instrument-panel grammar: hairline grid, mono labels, no decoration.
 */
const ROWS: { label: string; value: React.ReactNode }[] = [
  {
    label: "what it is",
    value:
      "Fresco is a free, open-source live wallpaper app for Linux: it sets video, GIF, image, slideshow, and playlist wallpapers as your animated desktop background, with GPU hardware decoding. A free Wallpaper Engine alternative and a GUI for mpvpaper on Wayland.",
  },
  {
    label: "platforms",
    value:
      "Any X11 desktop (Ubuntu, Pop!_OS, Linux Mint, Debian), plus Wayland layer-shell compositors: COSMIC, Hyprland, Sway, KDE Plasma 6. GNOME Wayland falls back to a static frame.",
  },
  {
    label: "license",
    value: (
      <>
        GPL-3.0, free forever.{" "}
        <a
          href={GITHUB_URL}
          target="_blank"
          rel="noopener noreferrer"
          className="text-link underline decoration-hairline-strong underline-offset-4 hover:decoration-current"
        >
          Source on GitHub
        </a>
        . Built with Rust, GTK4, and mpv.
      </>
    ),
  },
];

export function AtAGlance() {
  return (
    <section
      aria-label="Fresco at a glance"
      className="border-b border-hairline py-10"
    >
      <div className="mx-auto max-w-6xl px-5">
        <div className="rounded-md border border-hairline bg-surface">
          <p className="instrument-label border-b border-hairline px-4 py-2.5">
            fresco at a glance
          </p>
          <dl>
            {ROWS.map((row) => (
              <div
                key={row.label}
                className="grid grid-cols-1 gap-1 border-b border-hairline px-4 py-3 sm:grid-cols-[130px_1fr] sm:gap-4"
              >
                <dt className="font-mono text-meta font-medium uppercase tracking-wide text-ink-faint">
                  {row.label}
                </dt>
                <dd className="text-sm text-ink-muted">{row.value}</dd>
              </div>
            ))}
            <div className="grid grid-cols-1 gap-1 px-4 py-3 sm:grid-cols-[130px_1fr] sm:gap-4">
              <dt className="font-mono text-meta font-medium uppercase tracking-wide text-ink-faint">
                install
              </dt>
              <dd>
                <div className="flex items-start gap-2 rounded-sm border border-stone-800 bg-terminal px-3 py-2">
                  <span
                    aria-hidden
                    className="select-none font-mono text-sm leading-relaxed text-stone-500"
                  >
                    $
                  </span>
                  <code className="min-w-0 flex-1 whitespace-pre-wrap [overflow-wrap:anywhere] font-mono text-sm leading-relaxed text-stone-200">
                    {INSTALL_ONELINER}
                  </code>
                  <CopyButton value={INSTALL_ONELINER_COPY} />
                </div>
              </dd>
            </div>
          </dl>
        </div>
      </div>
    </section>
  );
}

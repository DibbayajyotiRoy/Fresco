import { FolderOpen, MousePointerClick, X } from "lucide-react";

const STEPS = [
  {
    n: "01",
    title: "Pick your media",
    description:
      "Open Fresco from your app menu and choose a video, GIF, image, folder, or playlist.",
    command: "fresco — add wallpaper.mp4",
    Icon: FolderOpen,
  },
  {
    n: "02",
    title: "Click Set",
    description:
      "Set it as your wallpaper. It starts playing on your desktop right away.",
    command: "fresco — set-as-wallpaper",
    Icon: MousePointerClick,
  },
  {
    n: "03",
    title: "Close the app",
    description:
      "Quit the window. A lightweight daemon keeps the wallpaper running, even after a reboot.",
    command: "frescod — detach",
    Icon: X,
  },
] as const;

export function HowItWorks() {
  return (
    <section
      id="how-it-works"
      className="border-b border-hairline bg-surface py-20 sm:py-28"
    >
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="instrument-label !text-ink-faint">how it works</p>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            Three clicks, then forget about it.
          </h2>
          <p className="mt-4 max-w-2xl text-pretty text-ink-subtle">
            Open Fresco, click add, click set, close. The daemon keeps the
            wallpaper running, even after you reboot.
          </p>
        </div>

        <ol className="relative mt-14 grid gap-x-10 gap-y-12 md:grid-cols-3">
          <div
            aria-hidden
            className="pointer-events-none absolute left-[15%] right-[15%] top-6 hidden h-px bg-accent/30 md:block"
          />

          {STEPS.map((step) => {
            const Icon = step.Icon;
            return (
              <li
                key={step.n}
                className="group relative flex flex-col items-start text-left md:items-center md:text-center"
              >
                <div className="relative z-10 flex size-12 items-center justify-center rounded-md border border-hairline bg-raised text-ink-muted transition-colors group-hover:border-accent/40">
                  <Icon className="size-5" aria-hidden />
                  <span className="absolute -right-2 -top-2 flex size-5 items-center justify-center rounded-full border border-hairline bg-paper font-mono text-meta tabular-nums text-ink-subtle">
                    {Number(step.n)}
                  </span>
                </div>
                <span className="instrument-label mt-5">step {step.n}</span>
                <h3 className="mt-2 text-lg font-semibold text-ink">
                  {step.title}
                </h3>
                <p className="mt-2 max-w-xs text-sm text-ink-subtle">
                  {step.description}
                </p>
                <code className="mt-3 block font-mono text-sm text-ink-faint">
                  {step.command}
                </code>
              </li>
            );
          })}
        </ol>
      </div>
    </section>
  );
}
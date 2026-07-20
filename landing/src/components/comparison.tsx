import Link from "next/link";
import { ArrowUpRight, Check, X } from "lucide-react";
import { COMPARISON, type CompareCell } from "@/lib/content";
import { ALTERNATIVES } from "@/lib/alternatives";

function Cell({ value, highlight }: { value: CompareCell; highlight: boolean }) {
  const base = highlight ? "bg-accent/[0.06]" : "";
  if (value === true) {
    return (
      <td className={`border-l border-hairline px-4 py-2.5 text-center ${base}`}>
        <Check
          className={`mx-auto size-4 ${highlight ? "text-accent" : "text-ink"}`}
          aria-hidden
        />
        <span className="sr-only">Yes</span>
      </td>
    );
  }
  if (value === false) {
    return (
      <td className={`border-l border-hairline px-4 py-2.5 text-center ${base}`}>
        <X className="mx-auto size-4 text-ink-faint" aria-hidden />
        <span className="sr-only">No</span>
      </td>
    );
  }
  return (
    <td
      className={`border-l border-hairline px-4 py-2.5 text-center text-sm text-ink-subtle ${base}`}
    >
      {value}
    </td>
  );
}

export function Comparison() {
  const scores = COMPARISON.tools.map((_, i) =>
    COMPARISON.rows.reduce((s, r) => s + (r.values[i] === true ? 1 : 0), 0),
  );
  const leader = scores.indexOf(Math.max(...scores));

  return (
    <section id="compare" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="instrument-label">briefing · +15 xp</p>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            Fresco vs the Linux wallpaper field.
          </h2>
          <p className="mt-4 text-pretty text-ink-subtle">
            Fresco is the only actively maintained Linux live-wallpaper app in
            this table that combines a GUI, hardware decoding, X11 and Wayland
            support, and a built-in catalog, free. Here is the full comparison
            with Hidamari, Komorebi, mpvpaper, and Wallpaper Engine.
          </p>
          <p className="mt-2 font-mono text-meta uppercase tracking-widest text-ink-faint">
            brief · {COMPARISON.tools.length} tools · {COMPARISON.rows.length}{" "}
            capabilities
          </p>
        </div>

        <div className="mt-6 flex flex-wrap items-baseline gap-x-6 gap-y-2 rounded-sm border border-hairline bg-raised/50 px-4 py-3 mb-6">
          {COMPARISON.tools.map((tool, i) => (
            <span
              key={tool}
              className="flex items-baseline gap-1.5 font-mono uppercase tracking-wide"
            >
              <span
                className={
                  i === 0 ? "text-accent" : i === leader ? "text-ink" : "text-ink-muted"
                }
              >
                {tool.toUpperCase()}
              </span>
              <span className="tabular-nums text-ink-faint">
                : {scores[i]}
                {i === leader ? ` / ${COMPARISON.rows.length}` : ""}
              </span>
            </span>
          ))}
        </div>

        <div className="overflow-x-auto rounded-md border border-hairline bg-surface">
          <table className="w-full min-w-[680px] border-collapse text-sm">
            <thead>
              <tr className="border-b-2 border-hairline">
                <th
                  scope="col"
                  className="instrument-label px-4 py-3 text-left font-semibold"
                >
                  Feature
                </th>
                {COMPARISON.tools.map((tool, i) => (
                  <th
                    key={tool}
                    scope="col"
                    className={`instrument-label border-l border-hairline px-4 py-3 text-center font-semibold ${
                      i === 0 ? "bg-accent/[0.06] !text-accent" : ""
                    }`}
                  >
                    {tool}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {COMPARISON.rows.map((row) => (
                <tr
                  key={row.label}
                  className="border-b border-hairline last:border-0 even:bg-raised/50"
                >
                  <th
                    scope="row"
                    className="px-4 py-2.5 text-left text-sm font-normal text-ink-muted"
                  >
                    {row.label}
                  </th>
                  {row.values.map((value, i) => (
                    <Cell key={i} value={value} highlight={i === 0} />
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <p className="mt-4 font-mono text-meta tracking-wide text-ink-faint">
          {COMPARISON.note}
        </p>

        <div className="mt-8 flex flex-wrap items-center gap-x-2 gap-y-3 text-sm">
          <span className="text-ink-subtle">Compare in detail:</span>
          {ALTERNATIVES.map((alt) => (
            <Link
              key={alt.slug}
              href={`/alternatives/${alt.slug}`}
              className="inline-flex items-center gap-1 rounded-sm border border-hairline bg-surface px-2.5 py-1 text-sm font-medium text-ink-muted transition-colors hover:border-hairline-strong hover:text-ink"
            >
              Fresco vs {alt.tool}
              <ArrowUpRight className="size-3.5" aria-hidden />
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}
import { Check, X } from "lucide-react";
import { COMPARISON, type CompareCell } from "@/lib/content";

function Cell({ value, highlight }: { value: CompareCell; highlight: boolean }) {
  const base = highlight ? "bg-primary/5" : "";
  if (value === true) {
    return (
      <td className={`px-4 py-3 text-center ${base}`}>
        <Check className="mx-auto size-4 text-primary" aria-hidden />
        <span className="sr-only">Yes</span>
      </td>
    );
  }
  if (value === false) {
    return (
      <td className={`px-4 py-3 text-center ${base}`}>
        <X className="mx-auto size-4 text-muted-foreground/40" aria-hidden />
        <span className="sr-only">No</span>
      </td>
    );
  }
  return (
    <td className={`px-4 py-3 text-center text-xs text-muted-foreground ${base}`}>
      {value}
    </td>
  );
}

export function Comparison() {
  return (
    <section id="compare" className="border-b border-border/60 py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="text-sm font-medium text-primary">Compare</p>
          <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
            Fresco vs other Linux options.
          </h2>
          <p className="mt-4 text-pretty text-muted-foreground">
            How Fresco compares to the live-wallpaper tools people usually try
            first on Linux.
          </p>
        </div>

        <div className="mt-10 overflow-x-auto rounded-2xl border border-border/70">
          <table className="w-full min-w-[680px] border-collapse text-sm">
            <thead>
              <tr className="border-b border-border/70">
                <th
                  scope="col"
                  className="px-4 py-4 text-left font-medium text-muted-foreground"
                >
                  Feature
                </th>
                {COMPARISON.tools.map((tool, i) => (
                  <th
                    key={tool}
                    scope="col"
                    className={`px-4 py-4 text-center font-semibold ${
                      i === 0 ? "bg-primary/5 text-primary" : "text-foreground"
                    }`}
                  >
                    {tool}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {COMPARISON.rows.map((row) => (
                <tr key={row.label} className="border-b border-border/50 last:border-0">
                  <th
                    scope="row"
                    className="px-4 py-3 text-left font-normal text-muted-foreground"
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

        <p className="mt-4 text-xs text-muted-foreground">{COMPARISON.note}</p>
      </div>
    </section>
  );
}

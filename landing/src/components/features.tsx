import type { Dictionary } from "@/lib/i18n";

/**
 * Row order and which entry is still in preview are structural, so they live
 * here; every string comes from the dictionary. Adding a row means adding its
 * id here and its copy to every locale (the Dictionary type enforces that).
 */
const ROW_ORDER = [
  "hwDecode",
  "sessions",
  "catalog",
  "video",
  "slideshow",
  "playlist",
  "lyrics",
  "visualiser",
  "editor",
  "audio",
  "displays",
  "schedule",
  "power",
  "newTab",
  "themes",
] as const;

/** Not shipped in a stable release yet: rendered as a muted badge. */
const SOON_ROWS = new Set<(typeof ROW_ORDER)[number]>(["newTab"]);

export function Features({ dict }: { dict: Dictionary }) {
  const total = ROW_ORDER.length;
  const soon = SOON_ROWS.size;
  const shipping = total - soon;

  return (
    <section id="features" className="hidden border-b border-hairline py-20 sm:block sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="max-w-2xl">
          <p className="instrument-label !text-ink-faint">
            {dict.features.kicker}
          </p>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            {dict.features.title}
          </h2>
          <p className="mt-4 text-pretty text-ink-subtle">
            {dict.features.lead}
          </p>
          <p className="mt-2 font-mono text-meta uppercase tracking-widest text-ink-faint">
            {dict.features.manifest(total)}
          </p>
        </div>

        <div className="mt-10 overflow-x-auto rounded-md border border-hairline bg-surface">
          <table className="w-full min-w-[720px] border-collapse">
            <thead>
              <tr className="border-b-2 border-hairline">
                <th
                  scope="col"
                  className="instrument-label w-[130px] px-4 py-3 text-left font-semibold"
                >
                  {dict.features.thCapability}
                </th>
                <th
                  scope="col"
                  className="instrument-label border-l border-hairline px-4 py-3 text-left font-semibold"
                >
                  {dict.features.thWhatYouGet}
                </th>
                <th
                  scope="col"
                  className="instrument-label w-[190px] border-l border-hairline px-4 py-3 text-right font-semibold"
                >
                  {dict.features.thStatus}
                </th>
              </tr>
            </thead>
            <tbody>
              {ROW_ORDER.map((id) => {
                const row = dict.features.rows[id];
                const isSoon = SOON_ROWS.has(id);
                return (
                  <tr
                    key={id}
                    className="border-b border-hairline transition-colors last:border-0 even:bg-raised/50 hover:bg-raised"
                  >
                    <th
                      scope="row"
                      className="px-4 py-2.5 text-left align-top font-mono text-meta font-medium uppercase tracking-wide text-ink-faint"
                    >
                      {row.tag}
                    </th>
                    <td className="border-l border-hairline px-4 py-2.5 align-top">
                      <span className="text-sm font-medium text-ink">
                        {row.title}
                        {dict.features.titleSuffix}
                      </span>{" "}
                      <span className="text-sm text-ink-subtle">
                        {row.description}
                      </span>
                    </td>
                    <td className="border-l border-hairline px-4 py-2.5 text-right align-top font-mono text-meta tracking-wide text-ink-subtle">
                      {isSoon ? (
                        <span className="inline-flex items-center rounded-sm border border-hairline bg-raised px-1.5 py-0.5 uppercase text-ink-faint">
                          {row.status}
                        </span>
                      ) : (
                        <>
                          <span aria-hidden className="mr-1.5 text-ok">
                            ✓
                          </span>
                          {row.status}
                        </>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        <p className="mt-4 font-mono text-meta tracking-wide text-ink-faint">
          {dict.features.footnote}
        </p>
        <p className="mt-3 font-mono text-meta uppercase tracking-widest text-ink-faint">
          {dict.features.tally(shipping, total, soon)}
        </p>
      </div>
    </section>
  );
}

# Design notes — Fresco admin

Adapted from the owner's general design brief (written for a different
product) to what actually transfers to this admin. This file is the record of
what was applied, what was kept as-is on purpose, and where a real tension
remains — not a restatement of the whole brief.

## What this pass followed

**Typography.** New copy (Users, Countries, the CSV routes, this file) is
sentence case throughout — page titles, card titles, descriptions, table
headers, button labels. No new ALL-CAPS eyebrows, no new tracked overlines, no
bracket-style chips. Card copy stays to one line unless a fact actually needs
explaining, in which case it's one short sentence under the title (see
`DauChart`, `LifecycleSection`) rather than a paragraph. Mono is used only for
identifiers and version strings (install ids, dates in tables, version
badges) — never for prose.

**Colour.** Every new element draws from the existing monochrome-graphite
palette and the existing `SeverityBadge` tones (`ok`/`warning`/`info`) for
lifecycle status — Active/Idle/Lapsed always pair a colour with the word
itself (`LifecycleBadge`), never colour alone. No gradients, glows, glass,
textures or ornamental shadows were added; the DAU/new-installs chart reuses
the house dither-kit grey-ramp bar chart with muted gridlines, a title that
states the takeaway ("Installs that checked in each day"), and exact totals
in the legend next to it, exactly as `downloads-chart.tsx` already does.

**Tables.** The installs table reuses `DataTable`/`THead sticky="container"`
with a fixed-height scroll box, an explicit "N–M of T" pager with 10/25/50/100
row-size options, and page/size/filters all live in the URL — not component
state. Numeric columns are right-aligned via the table's existing
`tabular-nums`. `NullCell` (a real "—", never a bare 0) is used everywhere a
metric was not measured (e.g. a country's "check-ins today" before
`install_days` existed, or D30 retention before that horizon has been
reached).

**Loading / empty states.** `loading.tsx` on both new routes mirrors the real
layout at the same heights as the shipped `Skeleton`/`PanelSkeleton`
components, so there is no layout shift when data lands. Every new panel that
can be empty has its own `EmptyState` with a specific, honest reason ("Needs
install_days history — see supabase/migrations/…", not a generic spinner).

**Motion.** No new animation was added. The filter form, sort links and pager
are plain HTML (`<form method="GET">`, `<Link href>`) with no transition
beyond what `.press`/`hover:` already apply house-wide, and nothing here
introduces a new duration, easing curve or animated dependency.

**Accessibility.** One `<h1>` per new page (via the existing `PageHeader`).
Every filter control is a labelled `<select>`/`<input>`. Sortable column
headers are real links with `aria-sort` and a visible arrow, not
click-handlers on a `<th>`. The whole installs table — filter, sort, page,
export — works with the URL alone; no interaction requires JavaScript, which
is the accessibility property "keyboard support" mostly reduces to when the
control is a link or a form in the first place. Status is never colour-only
(`SeverityBadge` always carries the word).

**No new runtime dependencies.** The DAU/new-installs chart reuses the
existing dither-kit `<BarChart>` rather than adding a line-chart library —
dither-kit ships bars only, and a single new panel was not reason enough to
pull in a charting dependency for a line primitive. See the comment in
`src/app/users/sections/dau-chart.tsx`.

## What was deliberately left as-is, and why

The shipped component library — `Panel`/`PanelHeader`, `Section`,
`PageHeader`'s meta line, `StatCard`'s label, `EmptyState`'s ribbon — uses an
11px uppercase tracked mono "instrument panel" idiom for labels and meta
text, and the page title is set in Instrument Serif. That is a different
typographic language from the brief above (which calls for one sans
typeface throughout, sentence case, no tracked overlines, mono reserved for
identifiers).

This pass did **not** restyle those shared components. Two reasons:

1. **Scope.** The brief for this task was explicit that existing pages must
   keep working, and the instruction to reuse shared components
   ("restyle rather than duplicate") assumes the components are being kept,
   not rewritten. Reskinning `Panel`/`StatCard`/`Section`/`PageHeader` is a
   global, cross-cutting change that touches every page in the app
   (Overview, Catalog, Notifications, Feedback, Support, Usage, Reliability,
   Issues) — a much larger diff than "add a Users page", with real risk of
   regressing pages this task was not asked to touch, for a visual language
   swap that has no functional payoff.
2. **Consistency now beats a half-migrated app.** New pages built in
   sentence-case sans against `Panel`s whose header is still tracked-mono-caps
   would look like two different products glued together — worse than
   either style applied consistently. Reusing the existing idiom for
   structural chrome (panel titles, section labels, page meta) keeps Users
   and Countries visually native to the rest of this admin today.

**What this means concretely:** `PanelHeader`, `Section`'s heading, and
`PageHeader`'s meta line on the new pages still render through the existing
components and therefore still carry their built-in uppercase mono style.
Every piece of copy this pass actually authored — descriptions, button
labels, empty-state text, table headers that are direct children (`TH`
labels like "Country", "Install", "Check-ins 30d") — is sentence case as
written above; it is the *chrome around* that copy, owned by shared
components, that is unchanged.

**Recommended follow-up**, if the owner wants the brief applied fully: a
dedicated pass that redesigns `Panel`, `Section`, `PageHeader`, `StatCard`,
`EmptyState`, `ErrorPanel` and `DataTable`'s `TH` to the new type scale and
sentence case in one commit, across every page at once (so nothing is left
half-migrated), and decides whether Instrument Serif is dropped from page
titles project-wide or kept as the one deliberate display accent. That is a
different, larger task from this one.

## Thresholds and vocabulary (so they don't drift)

- **Active**: checked in within 7 days (`ACTIVE_WITHIN_DAYS`,
  `src/lib/install-analytics.ts`).
- **Idle**: checked in within 30 days but not within 7.
- **Lapsed**: silent longer than 30 days — the closest inference of an
  uninstall this telemetry can make; there is no uninstall event, so this is
  always "likely", worded as such in the UI.
- **"Checked in"**, never "opened the app" or "was active" in the strong
  sense: a heartbeat is sent at daemon start, throttled to at most once per
  ~20h. The UI says "checked in" everywhere this matters (KPI labels, chart
  titles, CSV headers) rather than implying continuous usage data this
  telemetry does not have.

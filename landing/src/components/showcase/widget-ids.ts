/**
 * The five widgets, in narrative order. Kept out of the "use client" modules
 * so the server section can import the values too. The ids are the join keys
 * into `dict.whatsNew.items` (the pinned 1.1.36 four) and, for `worldClock`,
 * which shipped later, into `dict.features.worldClock`.
 */
export const WIDGET_IDS = [
  "lyrics",
  "clock",
  "worldClock",
  "visualizer",
  "disc",
] as const;

export type WidgetId = (typeof WIDGET_IDS)[number];

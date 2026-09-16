/**
 * The four release widgets, in narrative order. Kept out of the "use client"
 * modules so the server section can import the values too. The ids are the
 * join keys into `dict.whatsNew.items`.
 */
export const WIDGET_IDS = ["lyrics", "clock", "visualizer", "disc"] as const;

export type WidgetId = (typeof WIDGET_IDS)[number];

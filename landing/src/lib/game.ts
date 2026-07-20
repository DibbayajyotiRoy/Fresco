/**
 * Gamification layer for the Fresco marketing landing — the "Fresco Operator
 * Console" tour. A visitor earns XP by completing quests (visiting sections,
 * casting the install command, toggling theme, interrogating the FAQ, reaching
 * the footer), and unlocks nine achievements. XP thresholds map to operator
 * levels. Everything stays inside the Warm Terminal dialect: progress is a
 * mono metric with tabular figures, achievements are 11px uppercase mono chips
 * (sky when unlocked, dashed stone when locked), and the level name reads as an
 * instrument label.
 *
 * This module is pure data so both server-rendered UI (the achievement gallery)
 * and the client QuestEngine can share it without hydration drift.
 */

export type AchievementId =
  | "boot"
  | "specs"
  | "briefing"
  | "ritual"
  | "initiate"
  | "lore"
  | "night"
  | "sendoff"
  | "cast";

export type Achievement = {
  id: AchievementId;
  /** Short mission code, uppercase mono. */
  code: string;
  /** Display title. */
  title: string;
  /** One-line flavor, mono subtitle. */
  subtitle: string;
  /** XP earned on unlock. Total across all achievements sums to 100. */
  xp: number;
  /** How the unlock fires: "view" (section scrolls into view) or "event"
   *  (a custom window event `fresco:quest:<id>`). */
  via: "view" | "event";
  /** For via="view": the element id to observe. */
  observes?: string;
};

/**
 * The console roster. Order is the order rendered in the achievement gallery.
 * XP sums exactly to 100 so the level curve bottoms out at Champion === 100%.
 */
export const ACHIEVEMENTS: Achievement[] = [
  {
    id: "boot",
    code: "BOOT",
    title: "Console online",
    subtitle: "Boot sequence complete · frescod ready",
    xp: 10,
    via: "view",
    observes: "top",
  },
  {
    id: "specs",
    code: "SPECS",
    title: "Spec sheet loaded",
    subtitle: "Read the full feature manifest",
    xp: 15,
    via: "view",
    observes: "features",
  },
  {
    id: "briefing",
    code: "BRIEF",
    title: "Battle briefing",
    subtitle: "Fresco vs the Linux wallpaper field",
    xp: 15,
    via: "view",
    observes: "compare",
  },
  {
    id: "ritual",
    code: "RITUAL",
    title: "Set-and-forget ritual",
    subtitle: "Three clicks, then a daemon holds the line",
    xp: 10,
    via: "view",
    observes: "how-it-works",
  },
  {
    id: "initiate",
    code: "INIT",
    title: "Initiation reached",
    subtitle: "Install terminal opened",
    xp: 15,
    via: "view",
    observes: "download",
  },
  {
    id: "cast",
    code: "CAST",
    title: "Install cast",
    subtitle: "One-liner copied to the clipboard",
    xp: 15,
    via: "event",
  },
  {
    id: "lore",
    code: "LORE",
    title: "Lore interrogation",
    subtitle: "Opened a decoder question in the FAQ",
    xp: 10,
    via: "event",
  },
  {
    id: "night",
    code: "NIGHT",
    title: "Night operator",
    subtitle: "Flipped the console theme",
    xp: 5,
    via: "event",
  },
  {
    id: "sendoff",
    code: "SEND",
    title: "Mission signed off",
    subtitle: "Reached the console footer",
    xp: 5,
    via: "view",
    observes: "site-footer",
  },
];

export const XP_BY_ID: Record<AchievementId, number> = Object.fromEntries(
  ACHIEVEMENTS.map((a) => [a.id, a.xp]),
) as Record<AchievementId, number>;

export const XP_TOTAL = ACHIEVEMENTS.reduce((s, a) => s + a.xp, 0);

export type Level = { name: string; min: number };

/** Operator rank curve. The final rank unlocks at full XP. */
export const LEVELS: Level[] = [
  { name: "Novice Operator", min: 0 },
  { name: "Field Operator", min: 30 },
  { name: "Console Engineer", min: 60 },
  { name: "Architect", min: 90 },
  { name: "Champion", min: XP_TOTAL },
];

/** Largest level whose `min` is <= xp. "Champion" reads only at full XP. */
export function levelFor(xp: number): { level: Level; next: Level | null } {
  let level = LEVELS[0];
  for (const l of LEVELS) if (xp >= l.min) level = l;
  const idx = LEVELS.indexOf(level);
  const next = LEVELS[idx + 1] ?? null;
  return { level, next };
}

/** Persistence key for the QuestEngine. Namespaced per the spec convention. */
export const QUEST_KEY = "fresco.quest";

/** A tiny event bus dispatch + listener pair. */
export const QUEST_EVENT = "fresco:quest";

export function dispatchQuest(id: AchievementId) {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(QUEST_EVENT, { detail: id }));
}
export type SoundCategory =
  | "interaction"
  | "overlay"
  | "navigation"
  | "notification"
  | "hero";

export type InteractionRole =
  | "interaction.tap"
  | "interaction.subtle"
  | "interaction.toggle"
  | "interaction.confirm";

export type OverlayRole =
  | "overlay.open"
  | "overlay.close"
  | "overlay.expand"
  | "overlay.collapse";

export type NavigationRole =
  | "navigation.forward"
  | "navigation.backward"
  | "navigation.tab";

export type NotificationRole =
  | "notification.info"
  | "notification.success"
  | "notification.warning"
  | "notification.error";

export type HeroRole = "hero.complete" | "hero.milestone";

export type SoundRole =
  | InteractionRole
  | OverlayRole
  | NavigationRole
  | NotificationRole
  | HeroRole;

export const ALL_SOUND_ROLES: SoundRole[] = [
  "interaction.tap",
  "interaction.subtle",
  "interaction.toggle",
  "interaction.confirm",
  "overlay.open",
  "overlay.close",
  "overlay.expand",
  "overlay.collapse",
  "navigation.forward",
  "navigation.backward",
  "navigation.tab",
  "notification.info",
  "notification.success",
  "notification.warning",
  "notification.error",
  "hero.complete",
  "hero.milestone",
];

export function parseRole(role: SoundRole): {
  category: SoundCategory;
  name: string;
} {
  const [category, name] = role.split(".") as [SoundCategory, string];
  return { category, name };
}

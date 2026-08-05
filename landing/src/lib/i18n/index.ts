import { DEFAULT_LOCALE, isLocale, type Locale } from "./config";
import type { Dictionary } from "./dictionaries/en";

/**
 * Dictionaries are dynamically imported so each statically generated locale
 * pulls in only its own copy, instead of shipping all seven in one chunk.
 */
const LOADERS: Record<Locale, () => Promise<{ default: Dictionary }>> = {
  en: () => import("./dictionaries/en").then((m) => ({ default: m.en })),
  ja: () => import("./dictionaries/ja").then((m) => ({ default: m.ja })),
  "pt-br": () => import("./dictionaries/pt-br").then((m) => ({ default: m.ptBr })),
  es: () => import("./dictionaries/es").then((m) => ({ default: m.es })),
  de: () => import("./dictionaries/de").then((m) => ({ default: m.de })),
  fr: () => import("./dictionaries/fr").then((m) => ({ default: m.fr })),
  "zh-cn": () => import("./dictionaries/zh-cn").then((m) => ({ default: m.zhCn })),
};

/** Loads the dictionary for `locale`, falling back to English if unknown. */
export async function getDictionary(locale: string): Promise<Dictionary> {
  const key = isLocale(locale) ? locale : DEFAULT_LOCALE;
  return (await LOADERS[key]()).default;
}

export type { Dictionary };
export * from "./config";

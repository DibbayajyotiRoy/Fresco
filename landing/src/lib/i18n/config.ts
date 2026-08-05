/**
 * Locale registry. One entry per shipped language.
 *
 * `code` doubles as the URL segment (`/ja`, `/pt-br`). The default locale is
 * served WITHOUT a prefix so the established English URLs (`/`, indexed and
 * linked) never move; middleware rewrites those onto the `en` segment
 * internally. Everything else lives under its own prefix.
 *
 * `hreflang` is what goes in <link rel="alternate">, and it is deliberately
 * not always equal to `code`: Google wants `pt-BR` and `zh-Hans`, but a
 * lowercase, single-form URL segment is the web convention.
 */
export const LOCALES = [
  "en",
  "ja",
  "pt-br",
  "es",
  "de",
  "fr",
  "zh-cn",
] as const;

export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "en";

export type LocaleMeta = {
  /** BCP 47 tag for <link rel="alternate" hreflang> and the sitemap. */
  hreflang: string;
  /** Value for <html lang>. */
  htmlLang: string;
  /** Facebook-style locale for og:locale. */
  ogLocale: string;
  /** Endonym, shown in the language switcher. Never translated. */
  nativeName: string;
  /** Short label for the compact (mobile) switcher trigger. */
  short: string;
  /** Locale passed to Number.prototype.toLocaleString for the stats. */
  numberLocale: string;
};

export const LOCALE_META: Record<Locale, LocaleMeta> = {
  en: {
    hreflang: "en",
    htmlLang: "en",
    ogLocale: "en_US",
    nativeName: "English",
    short: "EN",
    numberLocale: "en-US",
  },
  ja: {
    hreflang: "ja",
    htmlLang: "ja",
    ogLocale: "ja_JP",
    nativeName: "日本語",
    short: "日本語",
    numberLocale: "ja-JP",
  },
  "pt-br": {
    hreflang: "pt-BR",
    htmlLang: "pt-BR",
    ogLocale: "pt_BR",
    nativeName: "Português (Brasil)",
    short: "PT",
    numberLocale: "pt-BR",
  },
  es: {
    hreflang: "es",
    htmlLang: "es",
    ogLocale: "es_ES",
    nativeName: "Español",
    short: "ES",
    numberLocale: "es-ES",
  },
  de: {
    hreflang: "de",
    htmlLang: "de",
    ogLocale: "de_DE",
    nativeName: "Deutsch",
    short: "DE",
    numberLocale: "de-DE",
  },
  fr: {
    hreflang: "fr",
    htmlLang: "fr",
    ogLocale: "fr_FR",
    nativeName: "Français",
    short: "FR",
    numberLocale: "fr-FR",
  },
  "zh-cn": {
    hreflang: "zh-Hans",
    htmlLang: "zh-Hans",
    ogLocale: "zh_CN",
    nativeName: "简体中文",
    short: "中文",
    numberLocale: "zh-CN",
  },
};

/** Cookie remembering an explicit pick, so auto-detect never overrides it. */
export const LOCALE_COOKIE = "fresco.locale";

export function isLocale(value: string): value is Locale {
  return (LOCALES as readonly string[]).includes(value);
}

/**
 * Path for `locale`. The default locale keeps the bare, already-indexed URL.
 * `path` is the locale-less remainder, always starting with "/" (or empty).
 */
export function localePath(locale: Locale, path = "/"): string {
  const rest = path === "/" ? "" : path;
  return locale === DEFAULT_LOCALE ? rest || "/" : `/${locale}${rest}`;
}

/**
 * Best supported locale for an Accept-Language header. Quality-ordered, and
 * tolerant of region subtags: `pt-PT` and `pt` both land on `pt-br` (the only
 * Portuguese we ship), `zh-SG` on `zh-cn`, `zh-TW` deliberately does NOT
 * (Traditional readers are better served by English than by Simplified).
 */
export function matchLocale(acceptLanguage: string | null): Locale | null {
  if (!acceptLanguage) return null;

  const ranked = acceptLanguage
    .split(",")
    .map((part) => {
      const [tag, ...params] = part.trim().split(";");
      const q = params
        .map((p) => p.trim())
        .find((p) => p.startsWith("q="))
        ?.slice(2);
      return { tag: tag.trim().toLowerCase(), q: q ? Number(q) : 1 };
    })
    .filter((entry) => entry.tag && !Number.isNaN(entry.q) && entry.q > 0)
    .sort((a, b) => b.q - a.q);

  for (const { tag } of ranked) {
    if (tag === "*") continue;
    if (isLocale(tag)) return tag;

    const [base, region] = tag.split("-");
    switch (base) {
      case "en":
        return "en";
      case "ja":
        return "ja";
      case "pt":
        return "pt-br";
      case "es":
        return "es";
      case "de":
        return "de";
      case "fr":
        return "fr";
      case "zh":
        // Traditional-script regions fall through to the next preference.
        if (region === "tw" || region === "hk" || region === "mo") break;
        return "zh-cn";
    }
  }

  return null;
}

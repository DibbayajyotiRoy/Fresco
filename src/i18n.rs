//! Runtime UI translation.
//!
//! Fresco builds its whole interface in Rust — there are no `.ui` templates and
//! no GResource bundle — so the usual GNOME toolchain (`xgettext` over XML +
//! `msgfmt` into `/usr/share/locale`) has nothing to scrape and nothing to
//! install. Rather than bolt a C `gettext` dependency and a build-time `.mo`
//! step onto a `cargo deb` pipeline that currently has neither, translations
//! are **plain JSON compiled into the binary**: one file per language under
//! `i18n/`, keyed by the English source string.
//!
//! That choice buys three things that matter for how Fresco ships:
//!
//! * The `.deb` and the Flatpak gain no new files and no new dependency — the
//!   catalog rides inside the executable, so there is no locale directory to
//!   get out of sync with the binary.
//! * The key *is* the English text, so an untranslated string degrades to
//!   correct English instead of to a bare `msgid` like `settings.fit.title`.
//! * [`tr`] hands back `&'static str`, which drops straight into every GTK
//!   setter that used to take a literal. Wrapping a call site is
//!   `set_title("Fit")` -> `set_title(t!("Fit"))`, with no allocation and no
//!   lifetime plumbing.
//!
//! The cost is that changing an English string silently orphans its
//! translation. `cargo test -p fresco i18n` guards the catalogs against
//! duplicate and empty entries, and `FRESCO_I18N_DEBUG=1` marks every
//! *untranslated* string at runtime so gaps are visible while testing rather
//! than after a release.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The UI language, as stored in `config.toml`.
///
/// `System` is the default and the only value most users ever have: it follows
/// `LC_ALL`/`LC_MESSAGES`/`LANG`. The explicit values exist because that
/// inference is wrong often enough to need an override — plenty of Chinese
/// desktop users run an English locale deliberately (toolchains and error
/// messages are easier to search that way) and still want a Chinese UI, and
/// the reverse is true for anyone reading English documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    /// Follow the desktop locale.
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    /// Simplified Chinese (简体中文).
    #[serde(rename = "zh-CN")]
    ChineseSimplified,
    /// Russian (Русский).
    #[serde(rename = "ru")]
    Russian,
    /// Hindi (हिन्दी).
    #[serde(rename = "hi")]
    Hindi,
    /// Brazilian Portuguese (Português do Brasil) — the only Portuguese
    /// catalog, so every Portuguese locale (`pt_PT` included) resolves here.
    #[serde(rename = "pt-BR")]
    PortugueseBrazil,
    /// Spanish (Español).
    #[serde(rename = "es")]
    Spanish,
    /// German (Deutsch).
    #[serde(rename = "de")]
    German,
    /// Indonesian (Bahasa Indonesia).
    #[serde(rename = "id")]
    Indonesian,
    /// French (Français).
    #[serde(rename = "fr")]
    French,
    /// Vietnamese (Tiếng Việt).
    #[serde(rename = "vi")]
    Vietnamese,
    /// Turkish (Türkçe).
    #[serde(rename = "tr")]
    Turkish,
    /// Romanian (Română).
    #[serde(rename = "ro")]
    Romanian,
    /// Bengali (বাংলা).
    #[serde(rename = "bn")]
    Bengali,
}

/// One row of [`REGISTRY`]: everything about a language that is not the
/// `Language` variant itself or its `#[serde(rename)]`.
///
/// Keeping this data in one table rather than one `match` per method is what
/// makes adding a language a single new row instead of an edit scattered
/// across `code`, `display_name`, `resolve` and `catalog_source` — the kind
/// of split edit that is easy to do three-quarters of and ship broken.
struct LangInfo {
    lang: Language,
    /// The BCP-47-ish tag used in `config.toml` and `FRESCO_LANG`. Must match
    /// the variant's `#[serde(rename)]` exactly — `serde_rename_matches_code`
    /// checks that.
    code: &'static str,
    /// The name shown in the language menu, written in its own language. Not
    /// used for `System`, whose entry in [`Language::display_name`] goes
    /// through [`tr`] instead.
    endonym: &'static str,
    /// The compiled-in catalog JSON, or `None` for `System`/`English`, which
    /// have no catalog of their own.
    source: Option<&'static str>,
}

/// Every language offered in Settings, in menu order: System, English, then
/// the rest in the order they were added.
const REGISTRY: [LangInfo; 14] = [
    LangInfo {
        lang: Language::System,
        code: "system",
        endonym: "",
        source: None,
    },
    LangInfo {
        lang: Language::English,
        code: "en",
        endonym: "English",
        source: None,
    },
    LangInfo {
        lang: Language::ChineseSimplified,
        code: "zh-CN",
        endonym: "简体中文",
        source: Some(include_str!("../i18n/zh-CN.json")),
    },
    LangInfo {
        lang: Language::Russian,
        code: "ru",
        endonym: "Русский",
        source: Some(include_str!("../i18n/ru.json")),
    },
    LangInfo {
        lang: Language::Hindi,
        code: "hi",
        endonym: "हिन्दी",
        source: Some(include_str!("../i18n/hi.json")),
    },
    LangInfo {
        lang: Language::PortugueseBrazil,
        code: "pt-BR",
        endonym: "Português (Brasil)",
        source: Some(include_str!("../i18n/pt-BR.json")),
    },
    LangInfo {
        lang: Language::Spanish,
        code: "es",
        endonym: "Español",
        source: Some(include_str!("../i18n/es.json")),
    },
    LangInfo {
        lang: Language::German,
        code: "de",
        endonym: "Deutsch",
        source: Some(include_str!("../i18n/de.json")),
    },
    LangInfo {
        lang: Language::Indonesian,
        code: "id",
        endonym: "Bahasa Indonesia",
        source: Some(include_str!("../i18n/id.json")),
    },
    LangInfo {
        lang: Language::French,
        code: "fr",
        endonym: "Français",
        source: Some(include_str!("../i18n/fr.json")),
    },
    LangInfo {
        lang: Language::Vietnamese,
        code: "vi",
        endonym: "Tiếng Việt",
        source: Some(include_str!("../i18n/vi.json")),
    },
    LangInfo {
        lang: Language::Turkish,
        code: "tr",
        endonym: "Türkçe",
        source: Some(include_str!("../i18n/tr.json")),
    },
    LangInfo {
        lang: Language::Romanian,
        code: "ro",
        endonym: "Română",
        source: Some(include_str!("../i18n/ro.json")),
    },
    LangInfo {
        lang: Language::Bengali,
        code: "bn",
        endonym: "বাংলা",
        source: Some(include_str!("../i18n/bn.json")),
    },
];

impl Language {
    /// Every language offered in Settings, in menu order. Kept as a plain
    /// array of variants — not `REGISTRY` itself — because call sites (the
    /// language menu, the test walk below) iterate it by value and have no
    /// business seeing the registry's internals.
    pub const ALL: [Language; REGISTRY.len()] = {
        let mut out = [Language::System; REGISTRY.len()];
        let mut i = 0;
        while i < REGISTRY.len() {
            out[i] = REGISTRY[i].lang;
            i += 1;
        }
        out
    };

    fn info(self) -> &'static LangInfo {
        REGISTRY
            .iter()
            .find(|row| row.lang == self)
            .expect("every Language variant has a REGISTRY row")
    }

    /// The BCP-47-ish tag used in `config.toml` and `FRESCO_LANG`.
    pub fn code(self) -> &'static str {
        self.info().code
    }

    /// The name shown in the language menu.
    ///
    /// Deliberately **not** run through [`tr`]: a language menu that renders in
    /// the language you are trying to leave is a trap. Every entry is written
    /// in its own language (the endonym), which is how every OS-level language
    /// picker behaves — except `System`, which describes a behaviour rather
    /// than naming a language and so does get translated.
    pub fn display_name(self) -> &'static str {
        match self {
            Language::System => tr("System"),
            other => other.info().endonym,
        }
    }

    /// Case-insensitive so `FRESCO_LANG=pt-br` (or `PT-BR`) matches the
    /// canonical `pt-BR` the same way a shell env var would be typed.
    fn from_code(code: &str) -> Option<Language> {
        REGISTRY
            .iter()
            .find(|row| row.code.eq_ignore_ascii_case(code))
            .map(|row| row.lang)
    }

    /// [`Language::from_code`], but an unrecognised code becomes `System`
    /// instead of `None`. Used by [`crate::config`] to deserialize
    /// `config.toml`, where a code this build has never heard of — a newer
    /// Fresco's language, or a typo — must not fail the whole file.
    pub(crate) fn from_code_lenient(code: &str) -> Language {
        Language::from_code(code).unwrap_or_else(|| {
            log::warn!("i18n: unknown language {code:?} in config.toml, falling back to System");
            Language::System
        })
    }

    /// The catalog to load, with `System` resolved against the desktop locale.
    /// `None` means "no catalog" — i.e. the untranslated English source.
    fn resolve(self) -> Option<Language> {
        match self {
            Language::English => None,
            Language::System => detect_locale_language(),
            other => Some(other),
        }
    }
}

/// Pull the language subtag out of a raw `LANG`/`LC_*` value.
///
/// "zh_CN.UTF-8" / "zh-Hans-CN" / "ru_RU.UTF-8" / "C.UTF-8" -> the language
/// subtag, lower-cased. Split out of [`detect_locale_language`] so the parsing
/// shape itself can be unit-tested without touching process-global env vars.
fn locale_subtag(raw: &str) -> String {
    raw.split(['.', '@'])
        .next()
        .unwrap_or(raw)
        .split(['_', '-'])
        .next()
        .unwrap_or(raw)
        .to_ascii_lowercase()
}

/// Map a language subtag onto a catalog we actually ship.
///
/// Every `zh` variant resolves to Simplified, including `zh_TW`/`zh_HK`. That
/// is a deliberate approximation and not a claim of Traditional support: a
/// Traditional reader gets Simplified Chinese, which is imperfect but far
/// closer to readable than falling back to English would be. If a `zh-TW`
/// catalog is ever added, this is the one place that needs to learn about it.
fn language_for_subtag(lang: &str) -> Option<Language> {
    match lang {
        "zh" => Some(Language::ChineseSimplified),
        "ru" => Some(Language::Russian),
        "hi" => Some(Language::Hindi),
        // `pt-BR` is the only Portuguese catalog: every Portuguese locale,
        // including `pt_PT`, gets it rather than falling back to English.
        "pt" => Some(Language::PortugueseBrazil),
        "es" => Some(Language::Spanish),
        "de" => Some(Language::German),
        // "in" is the obsolete ISO 639 code for Indonesian, still seen in the
        // wild (glibc shipped it as `in_ID` for years before `id_ID`).
        "id" | "in" => Some(Language::Indonesian),
        "fr" => Some(Language::French),
        "vi" => Some(Language::Vietnamese),
        "tr" => Some(Language::Turkish),
        "ro" => Some(Language::Romanian),
        "bn" => Some(Language::Bengali),
        // Ukrainian is not Russian: deliberately not mapped to `ru`.
        _ => None,
    }
}

/// Map the desktop locale onto a catalog we actually ship.
fn detect_locale_language() -> Option<Language> {
    let raw = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .filter(|v| !v.is_empty())?;
    language_for_subtag(&locale_subtag(&raw))
}

/// The active catalog, or `None` for English. Written once by [`init`].
static CATALOG: OnceLock<Option<&'static HashMap<&'static str, &'static str>>> = OnceLock::new();

/// Compiled-in catalog sources. Adding a language means adding a `Language`
/// variant and one [`REGISTRY`] row.
fn catalog_source(lang: Language) -> Option<&'static str> {
    lang.info().source
}

/// Parse a catalog once and leak it, so lookups can hand out `&'static str`.
///
/// The leak is bounded and intentional: at most one catalog per process, freed
/// by process exit. It is what lets [`tr`] return `&'static str` and therefore
/// what keeps every call site a drop-in replacement for a string literal.
fn load(lang: Language) -> Option<&'static HashMap<&'static str, &'static str>> {
    let src = catalog_source(lang)?;
    let parsed: HashMap<String, String> = match serde_json::from_str(src) {
        Ok(m) => m,
        Err(e) => {
            // A malformed catalog must never take the UI down: English is
            // always a correct rendering of every string.
            log::error!(
                "i18n: catalog {} is malformed, using English: {e}",
                lang.code()
            );
            return None;
        }
    };
    let map: HashMap<&'static str, &'static str> = parsed
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| {
            (
                Box::leak(k.into_boxed_str()) as &'static str,
                Box::leak(v.into_boxed_str()) as &'static str,
            )
        })
        .collect();
    Some(Box::leak(Box::new(map)))
}

/// Choose the UI language for this process. Call once, before any UI is built.
///
/// `FRESCO_LANG` overrides the stored setting (`FRESCO_LANG=zh-CN fresco`),
/// which is how the translation can be exercised without changing config —
/// useful in CI and when reproducing a translated-only bug report.
///
/// Calling this twice is a no-op: the first call wins, because strings already
/// handed out live for the process lifetime and swapping catalogs underneath
/// them would leave a half-translated window.
pub fn init(setting: Language) {
    let chosen = std::env::var("FRESCO_LANG")
        .ok()
        .filter(|v| !v.is_empty())
        .and_then(|v| Language::from_code(&v))
        .unwrap_or(setting);
    let _ = CATALOG.set(chosen.resolve().and_then(load));
}

/// True when a non-English catalog is active. Used to decide whether it is
/// worth telling the user a string is untranslated.
pub fn is_translated() -> bool {
    CATALOG.get().copied().flatten().is_some()
}

/// Translate a UI string, falling back to the English source.
///
/// The argument is the English text itself, so a missing entry renders as
/// correct English rather than as a key. Prefer the [`crate::t!`] macro at call
/// sites.
pub fn tr(msgid: &'static str) -> &'static str {
    match CATALOG.get().copied().flatten() {
        Some(cat) => match cat.get(msgid) {
            Some(s) => s,
            None => {
                if debug_marks() {
                    // Leaked, but only under an opt-in debug env var and only
                    // once per distinct missing string in practice.
                    Box::leak(format!("»{msgid}").into_boxed_str())
                } else {
                    msgid
                }
            }
        },
        None => msgid,
    }
}

/// `FRESCO_I18N_DEBUG=1` prefixes untranslated strings with `»` so gaps in a
/// catalog are visible by looking at the window instead of by reading a diff.
fn debug_marks() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        is_translated()
            && std::env::var("FRESCO_I18N_DEBUG")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false)
    })
}

/// Translate a string containing `{name}` placeholders and substitute them.
///
/// `format!` cannot be used for translated text because it requires a literal
/// format string, and a translation is chosen at runtime. Placeholders are
/// **named**, never positional, so a translator can reorder them — Chinese
/// frequently needs a different clause order than English, and `{0} {1}` would
/// silently force the English order.
///
/// ```ignore
/// trf("Remove {name}?", &[("name", &entry.name)]);
/// ```
///
/// An unknown placeholder is left as-is rather than panicking: a typo in a
/// translation should look wrong, not crash the settings dialog.
pub fn trf(msgid: &'static str, args: &[(&str, &str)]) -> String {
    let template = tr(msgid);
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match args.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                // Unbalanced brace: emit the remainder verbatim.
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Translate a UI string. Shorthand for [`tr`].
///
/// Returns `&'static str`, so it substitutes directly for the string literal it
/// replaces: `set_title("Fit")` becomes `set_title(t!("Fit"))`.
#[macro_export]
macro_rules! t {
    ($msgid:expr) => {
        $crate::i18n::tr($msgid)
    };
}

/// Translate a string with `{name}` placeholders. Shorthand for [`trf`].
///
/// ```ignore
/// tf!("{count} wallpapers", "count" => n.to_string())
/// ```
#[macro_export]
macro_rules! tf {
    ($msgid:expr, $($name:expr => $value:expr),+ $(,)?) => {
        $crate::i18n::trf($msgid, &[$(($name, ::std::convert::AsRef::<str>::as_ref(&$value))),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shipped catalog must parse, and must not contain entries that are
    /// worse than no entry at all. An empty value would render as an empty
    /// label — a blank button is strictly worse than an English one — and the
    /// loader drops those, so catch them here where the fix is obvious.
    #[test]
    fn catalogs_are_wellformed() {
        for lang in Language::ALL {
            let Some(src) = catalog_source(lang) else {
                continue;
            };
            let map: HashMap<String, String> = serde_json::from_str(src)
                .unwrap_or_else(|e| panic!("catalog {} is not valid JSON: {e}", lang.code()));
            assert!(!map.is_empty(), "catalog {} is empty", lang.code());
            for (k, v) in &map {
                assert!(!k.is_empty(), "catalog {} has an empty key", lang.code());
                assert!(
                    !v.trim().is_empty(),
                    "catalog {}: key {k:?} has an empty translation — delete the entry \
                     instead, so it falls back to English",
                    lang.code()
                );
            }
        }
    }

    /// Placeholders must survive translation. A translated string that drops a
    /// `{name}` loses a filename or a count from the sentence, so a catalog
    /// entry may only use placeholders its English source defines.
    #[test]
    fn translations_keep_their_placeholders() {
        fn names(s: &str) -> Vec<&str> {
            let mut out = Vec::new();
            let mut rest = s;
            while let Some(open) = rest.find('{') {
                let after = &rest[open + 1..];
                match after.find('}') {
                    Some(close) => {
                        out.push(&after[..close]);
                        rest = &after[close + 1..];
                    }
                    None => break,
                }
            }
            out.sort_unstable();
            out
        }
        for lang in Language::ALL {
            let Some(src) = catalog_source(lang) else {
                continue;
            };
            let map: HashMap<String, String> = serde_json::from_str(src).unwrap();
            for (k, v) in &map {
                assert_eq!(
                    names(k),
                    names(v),
                    "catalog {}: placeholders differ between source {k:?} and translation {v:?}",
                    lang.code()
                );
            }
        }
    }

    /// Walk the source and collect every string handed to `t!`/`tf!`, decoded
    /// the way rustc would decode it.
    ///
    /// Catalogs are keyed by the *runtime* string, but they are written by
    /// reading the *source*, and the two differ wherever a literal contains an
    /// escape: `t!("Choose\u{2026}")` looks up `Choose…`, so a catalog entry
    /// spelled `Choose\u{2026}` can never match. That failure is invisible —
    /// the string silently renders in English — which is exactly why it needs
    /// a test rather than a review.
    fn source_msgids() -> std::collections::HashSet<String> {
        fn decode(lit: &str) -> String {
            let mut out = String::with_capacity(lit.len());
            let mut cs = lit.chars().peekable();
            while let Some(c) = cs.next() {
                if c != '\\' {
                    out.push(c);
                    continue;
                }
                match cs.next() {
                    Some('u') => {
                        // \u{XXXX}
                        let mut hex = String::new();
                        if cs.peek() == Some(&'{') {
                            cs.next();
                            for h in cs.by_ref() {
                                if h == '}' {
                                    break;
                                }
                                hex.push(h);
                            }
                        }
                        if let Some(ch) =
                            u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                        {
                            out.push(ch);
                        }
                    }
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('0') => out.push('\0'),
                    Some(other) => out.push(other), // \\ \" \'
                    None => {}
                }
            }
            out
        }

        fn walk(dir: &std::path::Path, found: &mut std::collections::HashSet<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, found);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let Ok(src) = std::fs::read_to_string(&p) else {
                        continue;
                    };
                    for (idx, _) in src.match_indices("t!(") {
                        // Only `t!(` and `tf!(`, not e.g. `format!(`.
                        let before = src[..idx].chars().last();
                        let is_tf = src[..idx].ends_with('f');
                        if !is_tf && before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                            continue;
                        }
                        let rest = &src[idx + 3..];
                        let rest = rest.trim_start();
                        if !rest.starts_with('"') {
                            continue;
                        }
                        // Scan to the closing quote, honouring backslash escapes.
                        let body = &rest[1..];
                        let mut end = None;
                        let mut esc = false;
                        for (i, ch) in body.char_indices() {
                            if esc {
                                esc = false;
                            } else if ch == '\\' {
                                esc = true;
                            } else if ch == '"' {
                                end = Some(i);
                                break;
                            }
                        }
                        if let Some(end) = end {
                            found.insert(decode(&body[..end]));
                        }
                    }
                }
            }
        }

        let mut found = std::collections::HashSet::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut found,
        );
        found
    }

    /// No catalog entry may be unreachable.
    ///
    /// A key that matches nothing in the source is either a typo or the fossil
    /// of a reworded string — in both cases the UI shows English while the
    /// catalog claims otherwise. The reverse (a `t!` with no catalog entry) is
    /// fine and expected: that is the English fallback doing its job.
    ///
    /// Const label tables (`LYRIC_ANCHORS`, `POWER_LABELS`, `ONBOARDING_STEPS`
    /// and friends) are translated through `t!(*label)` at runtime, so their
    /// literals never appear inside a `t!("…")` here. They are allow-listed by
    /// checking the whole source for the literal instead.
    #[test]
    fn every_catalog_key_is_reachable_from_the_source() {
        let msgids = source_msgids();
        let all_src = {
            let mut s = String::new();
            fn cat(dir: &std::path::Path, out: &mut String) {
                let Ok(entries) = std::fs::read_dir(dir) else {
                    return;
                };
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        cat(&p, out);
                    } else if p.extension().is_some_and(|x| x == "rs") {
                        if let Ok(t) = std::fs::read_to_string(&p) {
                            // Skip comment lines. A doc comment that *mentions*
                            // a string (this module's own examples do) must not
                            // vouch for a catalog key — that is a false
                            // negative, and it hid a real bug once already.
                            for line in t.lines().filter(|l| !l.trim_start().starts_with("//")) {
                                out.push_str(line);
                                out.push('\n');
                            }
                        }
                    }
                }
            }
            cat(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
                &mut s,
            );
            s
        };

        for lang in Language::ALL {
            let Some(src) = catalog_source(lang) else {
                continue;
            };
            let map: HashMap<String, String> = serde_json::from_str(src).unwrap();
            let orphans: Vec<&String> = map
                .keys()
                .filter(|k| !msgids.contains(k.as_str()))
                // Const-table labels: present verbatim in the source, just not
                // inside a `t!("…")` call.
                .filter(|k| !all_src.contains(&format!("\"{k}\"")))
                .collect();
            assert!(
                orphans.is_empty(),
                "catalog {} has {} key(s) that no t!/tf! call site can ever look up \
                 (reworded source string, or an escape written into the key): {:?}",
                lang.code(),
                orphans.len(),
                orphans
            );
        }
    }

    #[test]
    fn locale_detection_reads_the_language_subtag() {
        // Sanity-check the parsing shape rather than the env, which is global.
        for (raw, want) in [
            ("zh_CN.UTF-8", Some(Language::ChineseSimplified)),
            ("zh", Some(Language::ChineseSimplified)),
            ("zh-Hans-CN", Some(Language::ChineseSimplified)),
            ("ru_RU.UTF-8", Some(Language::Russian)),
            ("ru_UA.UTF-8", Some(Language::Russian)),
            ("ru", Some(Language::Russian)),
            ("ru-RU", Some(Language::Russian)),
            ("hi_IN.UTF-8", Some(Language::Hindi)),
            ("pt_BR.UTF-8", Some(Language::PortugueseBrazil)),
            ("pt_PT", Some(Language::PortugueseBrazil)),
            ("es_MX", Some(Language::Spanish)),
            ("es_ES", Some(Language::Spanish)),
            ("es_AR", Some(Language::Spanish)),
            ("de_DE", Some(Language::German)),
            ("id_ID", Some(Language::Indonesian)),
            ("in_ID", Some(Language::Indonesian)),
            ("fr_FR", Some(Language::French)),
            ("fr_CA", Some(Language::French)),
            ("vi_VN", Some(Language::Vietnamese)),
            ("tr_TR", Some(Language::Turkish)),
            ("ro_RO", Some(Language::Romanian)),
            ("bn_BD", Some(Language::Bengali)),
            ("bn_IN", Some(Language::Bengali)),
            ("uk_UA", None),
            ("ur_PK", None),
            ("fil_PH", None),
            ("en_US.UTF-8", None),
            ("en_US", None),
            ("C.UTF-8", None),
            ("", None),
        ] {
            assert_eq!(
                language_for_subtag(&locale_subtag(raw)),
                want,
                "locale {raw:?}"
            );
        }
    }

    /// Every `REGISTRY` row's `code` must match its variant's
    /// `#[serde(rename)]` exactly — `code()` and config-file (de)serialization
    /// must always agree on the same spelling.
    #[test]
    fn serde_rename_matches_code() {
        for lang in Language::ALL {
            let serialized = serde_json::to_string(&lang).unwrap();
            assert_eq!(
                serialized,
                format!("{:?}", lang.code()),
                "Language::{lang:?} serializes as {serialized}, but code() says {}",
                lang.code()
            );
        }
    }

    /// zh-CN is the reference catalog. Every other shipped catalog must
    /// translate exactly the same key set — not a superset, not a subset.
    ///
    /// This is deliberately strict: a catalog with a missing key silently
    /// falls back to English for that string (harmless but incomplete), while
    /// one with an extra key is dead weight that `every_catalog_key_is_reachable_from_the_source`
    /// won't necessarily catch if the stray key happens to collide with a
    /// const-table literal. Keeping every catalog's key set identical to the
    /// reference is what makes "473 keys, fully translated" a fact you can
    /// check by diffing two `HashSet`s instead of eyeballing a JSON file.
    #[test]
    fn catalogs_share_the_reference_key_set() {
        let reference_src = catalog_source(Language::ChineseSimplified)
            .expect("zh-CN is the reference catalog and must exist");
        let reference: HashMap<String, String> = serde_json::from_str(reference_src).unwrap();
        let reference_keys: std::collections::HashSet<&str> =
            reference.keys().map(String::as_str).collect();

        for lang in Language::ALL {
            if lang == Language::ChineseSimplified {
                continue;
            }
            let Some(src) = catalog_source(lang) else {
                continue;
            };
            let map: HashMap<String, String> = serde_json::from_str(src).unwrap();
            let keys: std::collections::HashSet<&str> = map.keys().map(String::as_str).collect();
            let missing: Vec<&&str> = reference_keys.difference(&keys).collect();
            let extra: Vec<&&str> = keys.difference(&reference_keys).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "catalog {} does not match the reference key set — missing: {:?}, extra: {:?}",
                lang.code(),
                missing,
                extra
            );
        }
    }

    #[test]
    fn trf_substitutes_named_placeholders_and_leaves_unknown_ones() {
        assert_eq!(
            trf("Remove {name}?", &[("name", "clip.mp4")]),
            "Remove clip.mp4?"
        );
        assert_eq!(trf("{a} and {b}", &[("b", "2"), ("a", "1")]), "1 and 2");
        assert_eq!(trf("{missing} here", &[]), "{missing} here");
        assert_eq!(trf("unbalanced {", &[]), "unbalanced {");
        assert_eq!(trf("no placeholders", &[("a", "1")]), "no placeholders");
    }
}

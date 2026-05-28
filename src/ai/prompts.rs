use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    pub description: String,
    pub template: String,
    /// 1.2.12+ — ISO 639-1 language code (`en`, `ru`, `es`, `de`,
    /// `fr`) the prompt was authored for.  Optional for backward
    /// compatibility — `prompts.hjson` files from 1.2.11 and
    /// earlier omit it; absent means "untagged" and the resolver
    /// matches them in Pass 2 (back-compat).  See
    /// `Documentation/PROPOSALS/MULTILINGUAL_PROMPTS.md` §2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptLibrary {
    #[serde(default)]
    pub prompts: Vec<Prompt>,
}

impl PromptLibrary {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(Error::Io)?;
        serde_hjson::from_str(&raw).map_err(|e| Error::Config(e.to_string()))
    }

    /// Back-compat lookup: case-sensitive name match across every
    /// prompt regardless of language.  Used by code paths that
    /// haven't been migrated to the language-aware resolver yet,
    /// and by the prompts-editor TUI which loads the full library.
    pub fn find(&self, name: &str) -> Option<&Prompt> {
        self.prompts.iter().find(|p| p.name == name)
    }

    /// 1.2.12+ — language-aware lookup used by the three-pass
    /// resolver in `App::resolve_prompt`.
    ///
    /// `lang_filter` semantics:
    ///   * `Some("en")` — return only prompts whose `language`
    ///     field is `Some("en")` (case-insensitive).  Skip
    ///     untagged.  This is Pass 1 (strict same-language).
    ///   * `None`       — return only prompts WITHOUT a
    ///     `language` field set.  This is Pass 2 (untagged
    ///     back-compat).
    ///
    /// Pass 3 (any-language fallback) is implemented at the
    /// resolver level using `find` directly.
    pub fn find_lang(&self, name: &str, lang_filter: Option<&str>) -> Option<&Prompt> {
        self.prompts.iter().find(|p| {
            if p.name != name {
                return false;
            }
            match (lang_filter, p.language.as_deref()) {
                (Some(want), Some(have)) => have.eq_ignore_ascii_case(want),
                (None, None) => true,
                _ => false,
            }
        })
    }
}

/// 1.2.12+ — map inkhaven's user-facing `language` string
/// (`english` / `russian` / `french` / `german` / `spanish`) to
/// the ISO 639-1 two-letter code used by the prompt-language
/// resolver.  Unknown languages map to `"en"` since the embedded
/// floor is English.
pub fn iso_from_long(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "russian" => "ru",
        "french" => "fr",
        "german" => "de",
        "spanish" => "es",
        _ => "en",
    }
}

/// 1.2.12+ — inverse of `iso_from_long`.  Used by the AI pane
/// decoration to display the long-form language name when that
/// reads better than `ru` / `en`.  Falls back to the input for
/// unknown codes so callers can show the raw ISO string instead.
/// `#[allow(dead_code)]` because Phase A doesn't surface the
/// language in any UI yet — Phase C wires it into the AI pane
/// title.
#[allow(dead_code)]
pub fn iso_to_long(code: &str) -> &'static str {
    match code.to_lowercase().as_str() {
        "ru" => "russian",
        "fr" => "french",
        "de" => "german",
        "es" => "spanish",
        "en" => "english",
        _ => "english",
    }
}

/// 1.2.12+ — map whatlang's ISO 639-3 (`eng`, `rus`, …) to the
/// ISO 639-1 codes the resolver uses.  Returns `None` for
/// languages outside inkhaven's supported set so the caller
/// can fall back to the book language silently.
pub fn iso_from_alpha3(alpha3: &str) -> Option<&'static str> {
    match alpha3.to_lowercase().as_str() {
        "eng" => Some("en"),
        "rus" => Some("ru"),
        "fra" | "fre" => Some("fr"),
        "deu" | "ger" => Some("de"),
        "spa" => Some("es"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> PromptLibrary {
        PromptLibrary {
            prompts: vec![
                Prompt {
                    name: "grammar-check".into(),
                    description: "".into(),
                    template: "EN body".into(),
                    language: Some("en".into()),
                },
                Prompt {
                    name: "grammar-check".into(),
                    description: "".into(),
                    template: "RU body".into(),
                    language: Some("ru".into()),
                },
                Prompt {
                    name: "legacy-prompt".into(),
                    description: "".into(),
                    template: "untagged".into(),
                    language: None,
                },
            ],
        }
    }

    #[test]
    fn find_lang_strict_match_returns_in_language() {
        let l = lib();
        let p = l.find_lang("grammar-check", Some("ru")).unwrap();
        assert_eq!(p.template, "RU body");
    }

    #[test]
    fn find_lang_strict_match_is_case_insensitive() {
        let l = lib();
        let p = l.find_lang("grammar-check", Some("RU")).unwrap();
        assert_eq!(p.template, "RU body");
    }

    #[test]
    fn find_lang_strict_skips_untagged() {
        let l = lib();
        // legacy-prompt has language=None — must NOT match the
        // strict pass for ANY language.  Pass 2 (lang_filter=None)
        // is what picks it up.
        assert!(l.find_lang("legacy-prompt", Some("en")).is_none());
    }

    #[test]
    fn find_lang_none_filter_only_matches_untagged() {
        let l = lib();
        let p = l.find_lang("legacy-prompt", None).unwrap();
        assert_eq!(p.template, "untagged");
        // Even though grammar-check exists, it's tagged — the
        // untagged-only pass skips it.
        assert!(l.find_lang("grammar-check", None).is_none());
    }

    #[test]
    fn iso_from_long_maps_supported_languages() {
        assert_eq!(iso_from_long("English"), "en");
        assert_eq!(iso_from_long("russian"), "ru");
        assert_eq!(iso_from_long("FRENCH"), "fr");
        assert_eq!(iso_from_long(""), "en");
        assert_eq!(iso_from_long("klingon"), "en");
    }

    #[test]
    fn iso_from_alpha3_filters_unsupported() {
        assert_eq!(iso_from_alpha3("eng"), Some("en"));
        assert_eq!(iso_from_alpha3("rus"), Some("ru"));
        assert_eq!(iso_from_alpha3("fra"), Some("fr"));
        assert_eq!(iso_from_alpha3("fre"), Some("fr"));
        assert_eq!(iso_from_alpha3("ita"), None);
        assert_eq!(iso_from_alpha3("zho"), None);
    }
}

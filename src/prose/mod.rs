//! NARR-1 — narrative-voice (`prose`) profiling. Deterministic, zero-AI,
//! zero-runtime-dep statistical voice metrics over a book's chapters, in five
//! embedded languages (EN/RU/DE/FR/ES).
//!
//! This phase (N-P0) lands the language model: the [`ProseLanguage`] key, its
//! resolution chain, the [`SensoryChannel`] enum, the embedded word lists
//! (`lexicon`), and the lookup primitives ([`CompiledLexicon`]) + tokenizer the
//! metric passes build on. Metric formulas land in N-P1/N-P2.

mod lang_metrics;
mod lexicon;
mod metrics;
mod passive;
mod pipeline;
mod profile;
mod segment;
mod store;

use std::collections::{HashMap, HashSet};

/// The language key every language-sensitive metric is dispatched on. `Other`
/// books still get the language-agnostic Tier-1 rhythm metrics; language-keyed
/// metrics are reported as unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProseLanguage {
    En,
    Ru,
    De,
    Fr,
    Es,
    Other(String),
}

impl ProseLanguage {
    /// Map a config label (code or full name, any case) to a language. Empty →
    /// English. Unknown → `Other`.
    pub(crate) fn from_label(s: &str) -> ProseLanguage {
        match s.trim().to_lowercase().as_str() {
            "" | "en" | "eng" | "english" => ProseLanguage::En,
            "ru" | "rus" | "russian" | "русский" => ProseLanguage::Ru,
            "de" | "ger" | "german" | "deutsch" => ProseLanguage::De,
            "fr" | "fre" | "french" | "français" | "francais" => ProseLanguage::Fr,
            "es" | "spa" | "spanish" | "español" | "espanol" | "castellano" => {
                ProseLanguage::Es
            }
            other => ProseLanguage::Other(other.to_string()),
        }
    }

    /// Stable storage code (`en`/`ru`/`de`/`fr`/`es`/`other`).
    pub(crate) fn as_code(&self) -> &str {
        match self {
            ProseLanguage::En => "en",
            ProseLanguage::Ru => "ru",
            ProseLanguage::De => "de",
            ProseLanguage::Fr => "fr",
            ProseLanguage::Es => "es",
            ProseLanguage::Other(_) => "other",
        }
    }

    /// Whether the language has embedded word lists (i.e. language-sensitive
    /// metrics can be computed).
    pub(crate) fn is_supported(&self) -> bool {
        !matches!(self, ProseLanguage::Other(_))
    }
}

/// Resolve the active prose language: explicit `prose.language` override, then
/// the project language, then English. Returns the language plus an optional
/// note for the `prose profile` header (e.g. unsupported-language fallback).
pub(crate) fn resolve_prose_language(
    explicit: Option<&str>,
    project_language: &str,
) -> (ProseLanguage, Option<String>) {
    if let Some(code) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        let lang = ProseLanguage::from_label(code);
        let note = (!lang.is_supported()).then(|| {
            format!(
                "prose.language `{code}` is not an embedded language; \
                 Tier-1 rhythm metrics only"
            )
        });
        return (lang, note);
    }
    let proj = project_language.trim();
    if proj.is_empty() {
        return (
            ProseLanguage::En,
            Some(
                "prose_language not set; using EN word lists for \
                 language-sensitive metrics"
                    .into(),
            ),
        );
    }
    let lang = ProseLanguage::from_label(proj);
    let note = (!lang.is_supported()).then(|| {
        format!(
            "project language `{proj}` is not an embedded language; \
             language-sensitive metrics unavailable (Tier-1 rhythm metrics still computed)"
        )
    });
    (lang, note)
}

/// The five sensory vocabularies for Tier-2 channel balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SensoryChannel {
    Visual,
    Auditory,
    Olfactory,
    Tactile,
    Kinesthetic,
}

/// Lowercase, punctuation-trimmed, whitespace-split tokens. Internal hyphens and
/// apostrophes are preserved (`по-видимому`, `semble-t-il`, `l'ombre`); edge
/// punctuation, quotes, dashes, and ellipses are stripped. Unicode-aware.
pub(crate) fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Runtime lookup tables built once per profile computation from the embedded
/// [`lexicon::Lexicon`] for a language. Hash sets give O(1) membership; the tiny
/// bigram/trigram arrays are scanned linearly.
pub(crate) struct CompiledLexicon {
    modal_unigrams: HashSet<&'static str>,
    modal_bigrams: &'static [[&'static str; 2]],
    modal_trigrams: &'static [[&'static str; 3]],
    /// FID marker phrases pre-split into token sequences.
    interiority: Vec<Vec<&'static str>>,
    erlebte: HashSet<&'static str>,
    sensory: HashMap<&'static str, SensoryChannel>,
    passive_exceptions: HashSet<&'static str>,
}

impl CompiledLexicon {
    pub(crate) fn for_language(lang: &ProseLanguage) -> CompiledLexicon {
        let lx = lexicon::lexicon(lang);
        CompiledLexicon {
            modal_unigrams: lx.modal_unigrams.iter().copied().collect(),
            modal_bigrams: lx.modal_bigrams,
            modal_trigrams: lx.modal_trigrams,
            interiority: lx
                .interiority
                .iter()
                .map(|p| p.split_whitespace().collect())
                .collect(),
            erlebte: lx.erlebte_particles.iter().copied().collect(),
            sensory: lx.sensory.iter().copied().collect(),
            passive_exceptions: lx.passive_exceptions.iter().copied().collect(),
        }
    }

    /// Count epistemic-hedging hits over a token list: each unigram match, plus
    /// each bigram / trigram phrase whose tokens appear consecutively. A phrase
    /// counts once per occurrence (overlaps are intentionally allowed — they
    /// feed a density ratio, not a deduplicated list).
    pub(crate) fn count_modal_tokens(&self, tokens: &[&str]) -> usize {
        let mut hits = 0;
        for i in 0..tokens.len() {
            if self.modal_unigrams.contains(tokens[i]) {
                hits += 1;
            }
            if i + 1 < tokens.len() {
                for bi in self.modal_bigrams {
                    if tokens[i] == bi[0] && tokens[i + 1] == bi[1] {
                        hits += 1;
                    }
                }
            }
            if i + 2 < tokens.len() {
                for tri in self.modal_trigrams {
                    if tokens[i] == tri[0] && tokens[i + 1] == tri[1] && tokens[i + 2] == tri[2] {
                        hits += 1;
                    }
                }
            }
        }
        hits
    }

    /// Whether a sentence's tokens contain any FID marker phrase as a
    /// consecutive run. Token-level (not substring), so `she knew` never matches
    /// inside `he knew`.
    pub(crate) fn sentence_has_interiority(&self, tokens: &[&str]) -> bool {
        self.interiority
            .iter()
            .any(|phrase| contains_subsequence(tokens, phrase))
    }

    /// German *erlebte Rede* modal-particle hits (0 for non-DE lexicons, whose
    /// particle list is empty). The caller weights and gates these (only in
    /// non-interrogative third-person sentences) in N-P2.
    pub(crate) fn erlebte_particle_count(&self, tokens: &[&str]) -> usize {
        tokens.iter().filter(|t| self.erlebte.contains(*t)).count()
    }

    /// The sensory channel a token belongs to, if any.
    pub(crate) fn sensory_channel(&self, token: &str) -> Option<SensoryChannel> {
        self.sensory.get(token).copied()
    }

    /// Whether a token is on the language's passive-detection exclusion list
    /// (reflexive-only verbs in RU, `sein`+adjective collocations in DE, …).
    pub(crate) fn is_passive_exception(&self, token: &str) -> bool {
        self.passive_exceptions.contains(token)
    }
}

/// True when `needle` (non-empty) appears as a consecutive run in `haystack`.
fn contains_subsequence(haystack: &[&str], needle: &[&str]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|w| w.iter().zip(needle).all(|(a, b)| a == b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(text: &str) -> Vec<String> {
        tokenize(text)
    }
    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }

    // ── language resolution ──────────────────────────────────────────────
    #[test]
    fn resolution_chain() {
        // Explicit override wins.
        assert_eq!(resolve_prose_language(Some("de"), "english").0, ProseLanguage::De);
        // Falls through to project language.
        assert_eq!(resolve_prose_language(None, "russian").0, ProseLanguage::Ru);
        assert_eq!(resolve_prose_language(None, "Français").0, ProseLanguage::Fr);
        // Empty everything → En + note.
        let (lang, note) = resolve_prose_language(None, "");
        assert_eq!(lang, ProseLanguage::En);
        assert!(note.unwrap().contains("not set"));
        // Unknown → Other + note; rhythm-only.
        let (lang, note) = resolve_prose_language(Some("italian"), "english");
        assert_eq!(lang, ProseLanguage::Other("italian".into()));
        assert!(!lang.is_supported());
        assert!(note.unwrap().contains("rhythm"));
    }

    #[test]
    fn codes_round_trip() {
        for (label, code) in [
            ("English", "en"), ("ru", "ru"), ("Deutsch", "de"),
            ("francais", "fr"), ("español", "es"),
        ] {
            assert_eq!(ProseLanguage::from_label(label).as_code(), code);
        }
        assert_eq!(ProseLanguage::Other("x".into()).as_code(), "other");
    }

    // ── tokenizer ────────────────────────────────────────────────────────
    #[test]
    fn tokenizer_strips_edges_keeps_internal() {
        assert_eq!(toks("«Hello,» she — said…"), ["hello", "she", "said"]);
        // Internal hyphen / accents / Cyrillic preserved + lowercased.
        assert_eq!(toks("По-видимому, semble-t-il É"), ["по-видимому", "semble-t-il", "é"]);
    }

    // ── modal density scan, per language ─────────────────────────────────
    #[test]
    fn modal_en_unigrams() {
        let lx = CompiledLexicon::for_language(&ProseLanguage::En);
        let t = toks("She might have known, perhaps, but could not be sure.");
        // might + perhaps + could
        assert_eq!(lx.count_modal_tokens(&refs(&t)), 3);
    }

    #[test]
    fn modal_ru_bigram_and_trigram() {
        let lx = CompiledLexicon::for_language(&ProseLanguage::Ru);
        // bigram "должно быть" + unigram "вероятно" + trigram "судя по всему"
        let t = toks("Это, вероятно, должно быть так, судя по всему.");
        assert_eq!(lx.count_modal_tokens(&refs(&t)), 3);
    }

    #[test]
    fn modal_de_inflected_forms() {
        let lx = CompiledLexicon::for_language(&ProseLanguage::De);
        let t = toks("Sie könnten es wohl vermutlich gewusst haben.");
        // könnten + wohl + vermutlich
        assert_eq!(lx.count_modal_tokens(&refs(&t)), 3);
    }

    #[test]
    fn modal_fr_bigram_and_trigram() {
        let lx = CompiledLexicon::for_language(&ProseLanguage::Fr);
        // "sans doute" (bigram) + "on aurait dit" (trigram) + "apparemment" (unigram)
        let t = toks("Apparemment, sans doute, on aurait dit un rêve.");
        assert_eq!(lx.count_modal_tokens(&refs(&t)), 3);
    }

    #[test]
    fn modal_es_bigram_and_trigram() {
        let lx = CompiledLexicon::for_language(&ProseLanguage::Es);
        // "a lo mejor" (trigram) + "tal vez" (bigram) + "quizás" (unigram)
        let t = toks("A lo mejor, tal vez, quizás era cierto.");
        assert_eq!(lx.count_modal_tokens(&refs(&t)), 3);
    }

    // ── interiority, per language + the substring trap ───────────────────
    #[test]
    fn interiority_token_level_no_false_substring() {
        let lx = CompiledLexicon::for_language(&ProseLanguage::En);
        assert!(lx.sentence_has_interiority(&refs(&toks("she thought it was over"))));
        // "he knew" must NOT match inside "she knew" (token-level, not substring).
        assert!(lx.sentence_has_interiority(&refs(&toks("she knew the truth"))));
        assert!(!lx.sentence_has_interiority(&refs(&toks("the wind was cold"))));
    }

    #[test]
    fn interiority_other_languages() {
        for (lang, sent) in [
            (ProseLanguage::Ru, "ей казалось, что всё кончено"),
            (ProseLanguage::De, "sie dachte an den See"),
            (ProseLanguage::Fr, "elle pensait à lui"),
            (ProseLanguage::Es, "ella pensaba en voz baja"),
        ] {
            let lx = CompiledLexicon::for_language(&lang);
            assert!(
                lx.sentence_has_interiority(&refs(&toks(sent))),
                "{}",
                lang.as_code()
            );
        }
    }

    #[test]
    fn de_erlebte_particles_only_for_de() {
        let de = CompiledLexicon::for_language(&ProseLanguage::De);
        assert_eq!(de.erlebte_particle_count(&refs(&toks("das war ja doch wohl klar"))), 3);
        // Other languages have no particle list.
        let en = CompiledLexicon::for_language(&ProseLanguage::En);
        assert_eq!(en.erlebte_particle_count(&refs(&toks("yes indeed of course"))), 0);
    }

    // ── sensory channels, per language spot-check ────────────────────────
    #[test]
    fn sensory_channels_per_language() {
        for cases in [
            (ProseLanguage::En, "shadow", SensoryChannel::Visual),
            (ProseLanguage::En, "murmur", SensoryChannel::Auditory),
            (ProseLanguage::Ru, "запах", SensoryChannel::Olfactory),
            (ProseLanguage::De, "kalt", SensoryChannel::Tactile),
            (ProseLanguage::Fr, "tremblement", SensoryChannel::Kinesthetic),
            (ProseLanguage::Es, "silencio", SensoryChannel::Auditory),
        ] {
            let lx = CompiledLexicon::for_language(&cases.0);
            assert_eq!(lx.sensory_channel(cases.1), Some(cases.2), "{}", cases.1);
            assert_eq!(lx.sensory_channel("zzqq"), None);
        }
    }

    // ── passive exceptions present per language that needs them ───────────
    #[test]
    fn passive_exceptions_loaded() {
        let en = CompiledLexicon::for_language(&ProseLanguage::En);
        assert!(en.is_passive_exception("thought")); // not a passive despite -t
        let ru = CompiledLexicon::for_language(&ProseLanguage::Ru);
        assert!(ru.is_passive_exception("казалось")); // reflexive, not passive
        let de = CompiledLexicon::for_language(&ProseLanguage::De);
        assert!(de.is_passive_exception("klar")); // "war klar" not Zustandspassiv
    }
}

//! LANG-3 Tier 1 (RBMT) — English lemma → conlang headword mapping.
//!
//! The bridge that makes RBMT feasible for a conlang: because a LANG-1 lexicon
//! records each headword's English gloss, mapping *into* the conlang is a lookup
//! over those glosses (the reverse of how the lexicon is normally read). The
//! index is built once from the dictionary and matches an English lemma against
//! each entry's gloss, content-token-wise and article/infinitive-tolerant, with
//! an optional part-of-speech hint to break homograph ties (the noun *water* vs.
//! the verb *water*).
//!
//! Pure and deterministic.

use crate::language_entry::DictionaryEntry;

/// One indexed sense: the conlang headword, its part of speech, and the gloss
/// reduced to content tokens.
struct Sense {
    word: String,
    pos: String,
    tokens: Vec<String>,
}

/// A built index over a language's lexicon.
pub struct GlossIndex {
    senses: Vec<Sense>,
}

/// The result of mapping one English lemma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mapping {
    /// Found a headword whose gloss matches.
    Found { word: String, pos: String },
    /// No lexicon entry carries this meaning.
    Missing,
}

/// Lowercase, split on non-alphanumerics, drop articles and infinitive markers
/// across the supported working languages — the same reduction the gap finder
/// uses, so the two agree on what "covers" a concept.
fn content_tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .filter(|w| !is_stopword(w))
        .collect()
}

fn is_stopword(t: &str) -> bool {
    matches!(
        t,
        "to" | "the" | "a" | "an"                       // english
            | "le" | "la" | "les" | "un" | "une" | "des" | "du" | "de" // french
            | "der" | "die" | "das" | "ein" | "eine"    // german
            | "el" | "los" | "las" | "una" | "unos" | "unas" // spanish
    )
}

/// Does a part-of-speech string name the wanted broad class?
fn pos_is(pos: &str, want: PosHint) -> bool {
    let p = pos.to_lowercase();
    match want {
        PosHint::Noun => p.starts_with('n'),     // noun
        PosHint::Verb => p.starts_with('v'),     // verb
        PosHint::Adjective => p.starts_with("adj"), // adjective (not adverb)
    }
}

/// A coarse part-of-speech hint to disambiguate homographs and to classify a
/// source word during analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosHint {
    Noun,
    Verb,
    Adjective,
}

impl GlossIndex {
    /// Build the index from a language's dictionary entries.
    pub fn build(entries: &[DictionaryEntry]) -> Self {
        let senses = entries
            .iter()
            .filter(|e| !e.word.trim().is_empty() && !e.translation.trim().is_empty())
            .map(|e| Sense {
                word: e.word.clone(),
                pos: e.pos.clone(),
                tokens: content_tokens(&e.translation),
            })
            .collect();
        GlossIndex { senses }
    }

    /// Does the lexicon record a sense of the given broad class carrying this
    /// English lemma? Used during analysis to classify a source word (is *bright*
    /// an adjective here? is *see* a verb?).
    pub fn has_sense(&self, lemma: &str, hint: PosHint) -> bool {
        let needle = lemma.to_lowercase();
        self.senses.iter().any(|s| pos_is(&s.pos, hint) && s.tokens.iter().any(|t| t == &needle))
    }

    /// Map one English lemma to a conlang headword, preferring senses whose part
    /// of speech matches `hint`. An exact single-token gloss beats a multi-word
    /// gloss that merely contains the lemma.
    pub fn map(&self, lemma: &str, hint: PosHint) -> Mapping {
        let needle = lemma.to_lowercase();

        // Pass 1: exact single-token gloss with a matching POS (the cleanest hit).
        if let Some(s) = self.senses.iter().find(|s| {
            pos_is(&s.pos, hint) && s.tokens.len() == 1 && s.tokens[0] == needle
        }) {
            return Mapping::Found { word: s.word.clone(), pos: s.pos.clone() };
        }
        // Pass 2: the lemma appears among a multi-word gloss's content tokens,
        // POS matching.
        if let Some(s) =
            self.senses.iter().find(|s| pos_is(&s.pos, hint) && s.tokens.iter().any(|t| t == &needle))
        {
            return Mapping::Found { word: s.word.clone(), pos: s.pos.clone() };
        }
        // Pass 3: ignore the POS hint — any sense carrying the meaning.
        if let Some(s) = self
            .senses
            .iter()
            .find(|s| s.tokens.len() == 1 && s.tokens[0] == needle)
            .or_else(|| self.senses.iter().find(|s| s.tokens.iter().any(|t| t == &needle)))
        {
            return Mapping::Found { word: s.word.clone(), pos: s.pos.clone() };
        }
        Mapping::Missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(word: &str, pos: &str, translation: &str) -> DictionaryEntry {
        DictionaryEntry {
            word: word.into(),
            pos: pos.into(),
            translation: translation.into(),
            ..Default::default()
        }
    }

    #[test]
    fn maps_by_gloss_with_pos_hint() {
        let entries = vec![
            entry("kira", "noun", "bird"),
            entry("nami", "verb", "to see"),
            entry("pata", "noun", "stone"),
        ];
        let idx = GlossIndex::build(&entries);
        assert_eq!(
            idx.map("bird", PosHint::Noun),
            Mapping::Found { word: "kira".into(), pos: "noun".into() }
        );
        // "to see" reduces to the content token "see".
        assert_eq!(
            idx.map("see", PosHint::Verb),
            Mapping::Found { word: "nami".into(), pos: "verb".into() }
        );
        assert_eq!(idx.map("dragon", PosHint::Noun), Mapping::Missing);
    }

    #[test]
    fn pos_hint_breaks_homograph_ties() {
        let entries = vec![
            entry("móru", "noun", "water"),
            entry("móruta", "verb", "to water"),
        ];
        let idx = GlossIndex::build(&entries);
        assert_eq!(
            idx.map("water", PosHint::Verb),
            Mapping::Found { word: "móruta".into(), pos: "verb".into() }
        );
        assert_eq!(
            idx.map("water", PosHint::Noun),
            Mapping::Found { word: "móru".into(), pos: "noun".into() }
        );
    }
}

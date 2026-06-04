//! Shared text-normalisation helpers.
//!
//! Every Snowball-backed surface (echo, tension,
//! continuity-bible, concordance, style-warnings, the
//! lexicon name-highlighter) reduces a word to a
//! comparison key the same way: lowercase, fold Russian
//! `ё`→`е`, then stem.  Before 1.2.20 each surface
//! inlined that reduction, and three of them forgot the
//! `ё`-fold (the B.1 bug).  Centralising it here is the
//! structural fix: there is now exactly one place the
//! fold can be present or absent.

use rust_stemmers::Stemmer;

/// Lowercase a word and fold Russian `ё`→`е`.
///
/// The Russian Snowball algorithm assumes `ё` and `е`
/// are unified (real text uses them interchangeably), so
/// `пошёл` must fold to `пошел` before stemming or the
/// two spellings produce different stems.  `ё` appears
/// only in Cyrillic, so the fold is a no-op for every
/// other language.
pub fn fold_lower(word: &str) -> String {
    word.to_lowercase().replace('ё', "е")
}

/// Reduce a word to its comparison key: [`fold_lower`]
/// then stem with the supplied Snowball stemmer when one
/// is active.  With no stemmer (language without a
/// Snowball algorithm, or stemming disabled) the
/// folded-lowercase form is the key.
///
/// This is the canonical token→key reduction shared by
/// every single-stemmer surface.  Callers own
/// tokenization, trimming, and any length/stop-word
/// gating; this owns only the fold-and-stem.
pub fn normalize_stem(word: &str, stemmer: &Option<Stemmer>) -> String {
    let lc = fold_lower(word);
    match stemmer {
        Some(s) => s.stem(&lc).into_owned(),
        None => lc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_stemmers::{Algorithm, Stemmer};

    #[test]
    fn fold_lower_folds_yo_and_lowercases() {
        assert_eq!(fold_lower("ПошёЛ"), "пошел");
        assert_eq!(fold_lower("Hello"), "hello");
        // No ё → pure lowercase.
        assert_eq!(fold_lower("Москва"), "москва");
    }

    #[test]
    fn normalize_stem_without_stemmer_is_fold_lower() {
        let none: Option<Stemmer> = None;
        assert_eq!(normalize_stem("ПошёЛ", &none), "пошел");
    }

    #[test]
    fn normalize_stem_unifies_yo_and_e_spellings() {
        let ru = Some(Stemmer::create(Algorithm::Russian));
        // ё and е spellings of the same word must produce
        // one key — the whole point of the fold.
        assert_eq!(
            normalize_stem("пошёл", &ru),
            normalize_stem("пошел", &ru)
        );
    }
}

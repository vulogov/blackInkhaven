//! NARR-1 — the `VoiceProfile` value type and the text → profile computation
//! that ties the Tier-1 (rhythm) and language-sensitive (Tier-1+2) metrics
//! together. Pure: no I/O. Persistence lives in `store`, extraction in
//! `pipeline`.

use std::hash::{Hash, Hasher};

use super::{CompiledLexicon, ProseLanguage, lang_metrics, metrics, passive, segment, tokenize};

/// What a profile describes: the whole book, or chapter `n` (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VoiceScope {
    Book,
    Chapter(u32),
}

impl VoiceScope {
    pub(crate) fn as_str(&self) -> String {
        match self {
            VoiceScope::Book => "book".into(),
            VoiceScope::Chapter(n) => format!("chapter:{n}"),
        }
    }
    pub(crate) fn chapter_ord(&self) -> Option<u32> {
        match self {
            VoiceScope::Book => None,
            VoiceScope::Chapter(n) => Some(*n),
        }
    }
    pub(crate) fn parse(s: &str) -> Option<VoiceScope> {
        if s == "book" {
            Some(VoiceScope::Book)
        } else {
            s.strip_prefix("chapter:")
                .and_then(|n| n.parse().ok())
                .map(VoiceScope::Chapter)
        }
    }
}

/// Tier-2 metrics (deep pass only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VoiceTier2 {
    /// `[visual, auditory, olfactory, tactile, kinesthetic]`.
    pub sensory: [f32; 5],
    pub active_passive_ratio: f32,
}

/// A deterministic voice fingerprint for one scope.
#[derive(Debug, Clone)]
pub(crate) struct VoiceProfile {
    pub scope: VoiceScope,
    pub prose_language: ProseLanguage,
    pub word_count: u32,
    pub sentence_count: u32,
    // Tier-1 (language-agnostic)
    pub p10: f32,
    pub p25: f32,
    pub p50: f32,
    pub p75: f32,
    pub p90: f32,
    pub cv: f32,
    pub burstiness: f32,
    pub mattr: f32,
    // Tier-1 (language-sensitive; None for unsupported languages)
    pub modal_density: Option<f32>,
    pub interiority_ratio: Option<f32>,
    pub de_erlebte_rede_particle_density: Option<f32>,
    // Tier-2 (None unless a deep pass over a supported language)
    pub tier2: Option<VoiceTier2>,
    /// `DefaultHasher` of the stripped text — the staleness key.
    pub text_hash: u64,
}

/// Stable u64 content hash of the stripped scope text (the invalidation key).
pub(crate) fn hash_text(text: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Compute the full profile for a unit of stripped prose `text`. `deep` enables
/// the Tier-2 sensory + active/passive metrics (only meaningful for a supported
/// language). Language-agnostic rhythm metrics are always computed.
pub(crate) fn compute_profile(
    text: &str,
    scope: VoiceScope,
    lang: &ProseLanguage,
    deep: bool,
    mattr_window: usize,
) -> VoiceProfile {
    let lx = CompiledLexicon::for_language(lang);
    compute_profile_with(text, scope, lang, &lx, deep, mattr_window)
}

/// As [`compute_profile`], but with a caller-supplied [`CompiledLexicon`] — the
/// pipeline builds it once per refresh with the project's `prose.extra_*`
/// tokens folded in.
pub(crate) fn compute_profile_with(
    text: &str,
    scope: VoiceScope,
    lang: &ProseLanguage,
    lx: &CompiledLexicon,
    deep: bool,
    mattr_window: usize,
) -> VoiceProfile {
    let sentences = segment::split_sentences(text, lang);

    let mut lengths: Vec<usize> = sentences
        .iter()
        .map(|s| s.split_whitespace().count())
        .filter(|&n| n > 0)
        .collect();
    let word_count: u32 = lengths.iter().map(|&n| n as u32).sum();
    let sentence_count = lengths.len() as u32;
    lengths.sort_unstable();

    let tokens = tokenize(text);
    let token_refs: Vec<&str> = tokens.iter().map(String::as_str).collect();

    let (modal_density, interiority_ratio, de_density) = {
        let m = lang_metrics::modal_density(text, lang, lx);
        let (i, d) = lang_metrics::interiority(&sentences, lang, lx);
        (m, i, d)
    };

    let tier2 = (deep && lang.is_supported()).then(|| VoiceTier2 {
        sensory: lang_metrics::sensory_balance(text, lang, lx).unwrap_or([0.0; 5]),
        active_passive_ratio: passive::passive_ratio(&sentences, lang, lx).unwrap_or(0.0),
    });

    VoiceProfile {
        scope,
        prose_language: lang.clone(),
        word_count,
        sentence_count,
        p10: metrics::percentile(&lengths, 10.0),
        p25: metrics::percentile(&lengths, 25.0),
        p50: metrics::percentile(&lengths, 50.0),
        p75: metrics::percentile(&lengths, 75.0),
        p90: metrics::percentile(&lengths, 90.0),
        cv: metrics::coefficient_of_variation(&lengths),
        burstiness: metrics::burstiness(&lengths),
        mattr: metrics::mattr(&token_refs, mattr_window),
        modal_density,
        interiority_ratio,
        de_erlebte_rede_particle_density: de_density,
        tier2,
        text_hash: hash_text(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage::{De, En, Other};

    #[test]
    fn scope_round_trips() {
        assert_eq!(VoiceScope::parse("book"), Some(VoiceScope::Book));
        assert_eq!(VoiceScope::parse("chapter:7"), Some(VoiceScope::Chapter(7)));
        assert_eq!(VoiceScope::Chapter(3).as_str(), "chapter:3");
        assert_eq!(VoiceScope::Chapter(3).chapter_ord(), Some(3));
        assert_eq!(VoiceScope::Book.chapter_ord(), None);
        assert_eq!(VoiceScope::parse("junk"), None);
    }

    #[test]
    fn compute_supported_vs_other() {
        let text = "She might have known. The wind was cold and bright. She thought so.";
        let p = compute_profile(text, VoiceScope::Chapter(1), &En, true, 100);
        assert_eq!(p.sentence_count, 3);
        assert!(p.modal_density.is_some());
        assert!(p.interiority_ratio.unwrap() > 0.0);
        assert!(p.tier2.is_some());
        assert!(p.de_erlebte_rede_particle_density.is_none()); // EN, not DE
        assert_ne!(p.text_hash, 0);

        // Other language → Tier-1 only; language-sensitive None.
        let o = compute_profile(text, VoiceScope::Book, &Other("it".into()), true, 100);
        assert!(o.modal_density.is_none());
        assert!(o.interiority_ratio.is_none());
        assert!(o.tier2.is_none());
        assert!(o.cv >= 0.0); // rhythm still computed
    }

    #[test]
    fn de_particle_density_present() {
        let text = "Sie dachte nach. Das war ja doch klar.";
        let p = compute_profile(text, VoiceScope::Book, &De, false, 100);
        assert!(p.de_erlebte_rede_particle_density.unwrap() > 0.0);
        assert!(p.tier2.is_none()); // not a deep pass
    }
}

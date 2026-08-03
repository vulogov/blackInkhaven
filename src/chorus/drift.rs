//! CHORUS-1 (CH-P3) — per-character voice drift.
//!
//! Does a character sound like *themselves* across their arc? The narrator's
//! voice can drift chapter-to-chapter (NARR-1 `prose drift`); a character's can
//! too — Mara clipped and guarded in Act I, suddenly loquacious in Act III.
//!
//! This reuses NARR-1's drift machinery verbatim: each character's per-chapter
//! profiles are scoped `Chapter(ord)` (built in `voices::character_profiles`),
//! so `prose::violations::violations` compares them directly. The one difference
//! from the narrator is the **baseline**: a character is measured against their
//! *first well-attributed chapter* (where their voice is established), not the
//! book's chapter 1 — a character rarely appears in chapter 1.

use crate::chorus::voices::CharacterVoice;
use crate::config::ProseThresholds;
use crate::prose::violations::{Violation, violations};

/// The metrics where a character's per-chapter voice drifts from their first
/// appearance beyond `thr`. Empty when the character has fewer than two
/// well-attributed chapters (drift needs a baseline and a comparison).
pub(crate) fn character_drift(voice: &CharacterVoice, thr: &ProseThresholds) -> Vec<Violation> {
    if voice.per_chapter.len() < 2 {
        return Vec::new();
    }
    // Baseline = the character's first well-attributed chapter (their arc start).
    let Some(baseline_ord) = voice.per_chapter.iter().filter_map(|p| p.scope.chapter_ord()).min()
    else {
        return Vec::new();
    };
    violations(&voice.per_chapter, baseline_ord, thr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chorus::voices::Confidence;
    use crate::prose::{CompiledLexicon, ProseLanguage, VoiceScope, compute_profile_with};

    fn chap(ord: u32, text: &str) -> crate::prose::VoiceProfile {
        let lx = CompiledLexicon::for_language_with(&ProseLanguage::En, &[], &[]);
        compute_profile_with(text, VoiceScope::Chapter(ord), &ProseLanguage::En, &lx, false, 100)
    }

    fn voice(name: &str, per_chapter: Vec<crate::prose::VoiceProfile>) -> CharacterVoice {
        CharacterVoice {
            name: name.into(),
            profile: chap(0, "placeholder aggregate"),
            confidence: Confidence::High,
            utterances: 40,
            per_chapter,
        }
    }

    #[test]
    fn a_voice_that_shifts_across_the_arc_drifts() {
        // Ch.3 (first appearance): uniform, clipped. Ch.9: highly varied + hedged.
        let early = chap(3, "He ran. She ate. They sat. We read. I slept. You won. He fell.");
        let late = chap(
            9,
            "Perhaps he might possibly have wandered far across the wide and shadowed plain, \
             wondering whether the road would ever bend. Yes.",
        );
        let v = voice("Mara", vec![early, late]);
        let drift = character_drift(&v, &ProseThresholds::default());
        // Baseline is ch.3 (her first), the shift shows in ch.9.
        assert!(drift.iter().any(|x| x.metric == "sent_len_cv" && x.chapter == 9), "{drift:?}");
    }

    #[test]
    fn a_steady_voice_does_not_drift() {
        let text = "She walked home slowly. It was late and very cold outside tonight.";
        let v = voice("Joren", vec![chap(2, text), chap(5, text)]);
        assert!(character_drift(&v, &ProseThresholds::default()).is_empty());
    }

    #[test]
    fn one_chapter_is_not_enough_to_drift() {
        let v = voice("Sela", vec![chap(4, "A single chapter of dialogue only, nothing to compare.")]);
        assert!(character_drift(&v, &ProseThresholds::default()).is_empty());
    }
}

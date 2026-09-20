//! CANON-2 (CG-P1) — deterministic grounding at creation. Free, no model: infer
//! which existing decisions a new one rests on, from **kind rules** plus
//! **salient-word overlap** between gists. Conservative by design (precision over
//! recall): a spurious ground distorts `impact`, so the bar to draw an edge is a
//! shared *content* word under a kind rule that makes sense, nothing looser.
//!
//! Multilingual by construction — tokens are stemmed and stop-words filtered
//! against the project language (`built_in_stop_words` + the language stemmer),
//! the same machinery echo detection uses, so "leviathan" ~ "leviathan's" and
//! "tremor" ~ "tremors" match, and function words never do.

use std::collections::{BTreeSet, HashSet};

use rust_stemmers::Stemmer;
use unicode_segmentation::UnicodeSegmentation;

use smysl::Uid;

use super::model::NarrativeKind;
use crate::config::{built_in_stop_words, parse_stemmer_language};
use crate::prose::ProseLanguage;

/// A live decision a new one might rest on.
pub(super) struct Candidate {
    pub uid: Uid,
    pub kind: NarrativeKind,
    pub gist: String,
}

/// The stemmer/stop-word language name (`built_in_stop_words` /
/// `parse_stemmer_language` want a full name — "french", not "fr"). `Other`
/// falls back to English so an unknown language still gets the 4-char floor.
pub(super) fn stemmer_language_name(lang: &ProseLanguage) -> &'static str {
    match lang {
        ProseLanguage::En => "english",
        ProseLanguage::Ru => "russian",
        ProseLanguage::De => "german",
        ProseLanguage::Fr => "french",
        ProseLanguage::Es => "spanish",
        ProseLanguage::Other(_) => "english",
    }
}

/// Recording order within a batch: ground-providers before the decisions that
/// rest on them, so a dependent (a reveal, a plot point) can ground on a
/// same-batch provider (a world-fact, a setup) as well as on the ledger.
pub(super) fn ground_rank(kind: NarrativeKind) -> u8 {
    use NarrativeKind::*;
    match kind {
        WorldFact => 0,
        Setup => 1,
        CharacterTrait => 2,
        PlotPoint => 3,
        Reveal => 4,
    }
}

/// Whether a `dependent` decision may rest on a `provider` (the ground
/// direction). The rules are the honest narrative dependencies: a reveal pays off
/// a setup and happens in the established world; a plot point turns on the world's
/// facts. Setups, world-facts, and character-traits are roots here — nothing
/// deterministic grounds them (a human can, via `canon ground`, CG-P3).
fn kind_may_ground(dependent: NarrativeKind, provider: NarrativeKind) -> bool {
    use NarrativeKind::*;
    match dependent {
        Reveal => matches!(provider, Setup | WorldFact),
        PlotPoint => matches!(provider, WorldFact),
        Setup | WorldFact | CharacterTrait => false,
    }
}

/// The salient tokens of a gist: Unicode words of at least 4 characters, stemmed,
/// minus the project language's stop-words. The 4-char floor (on the raw token)
/// drops most function words that slip the stop list across languages ("the",
/// "and", "una", "der", "dass", "sea"); stemming lets an inflected or possessive
/// form match its root.
pub(super) fn salient_tokens(gist: &str, language: &str) -> BTreeSet<String> {
    let stemmer = parse_stemmer_language(language).map(Stemmer::create);
    let stem = |w: &str| crate::text::normalize_stem(w, &stemmer);
    let stops: HashSet<String> = built_in_stop_words(language).iter().map(|s| stem(s)).collect();
    gist.unicode_words()
        .filter(|w| w.chars().count() >= 4)
        .map(|w| stem(w))
        .filter(|s| !s.is_empty() && !stops.contains(s))
        .collect()
}

/// The decisions `new` rests on: existing candidates whose kind may ground `new`
/// and whose gist shares at least one salient token with it. Empty when `new` has
/// no salient tokens (a very short/all-stopword gist) — better no edge than a
/// noisy one.
pub(super) fn infer_grounds(
    new_kind: NarrativeKind,
    new_gist: &str,
    language: &str,
    candidates: &[Candidate],
) -> Vec<Uid> {
    let new_tokens = salient_tokens(new_gist, language);
    if new_tokens.is_empty() {
        return Vec::new();
    }
    candidates
        .iter()
        .filter(|c| kind_may_ground(new_kind, c.kind))
        .filter(|c| salient_tokens(&c.gist, language).intersection(&new_tokens).next().is_some())
        .map(|c| c.uid)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uid(n: u128) -> Uid {
        // A deterministic throwaway uid for candidate identity in tests.
        smysl::canonical_uid(
            &smysl::UnitCoreBuilder::new(
                NarrativeKind::WorldFact.schema_id(),
                format!("candidate {n}"),
                smysl::Status::Speculative,
            )
            .build()
            .unwrap(),
        )
    }

    #[test]
    fn kind_rules_are_directional() {
        use NarrativeKind::*;
        assert!(kind_may_ground(Reveal, Setup));
        assert!(kind_may_ground(Reveal, WorldFact));
        assert!(kind_may_ground(PlotPoint, WorldFact));
        assert!(!kind_may_ground(PlotPoint, Setup), "a plot point doesn't rest on a setup");
        assert!(!kind_may_ground(WorldFact, WorldFact), "world-facts are roots");
        assert!(!kind_may_ground(Reveal, CharacterTrait));
    }

    #[test]
    fn shared_content_word_grounds_across_inflection() {
        let leviathan = Candidate {
            uid: uid(1),
            kind: NarrativeKind::WorldFact,
            gist: "the city floats on a sleeping leviathan".into(),
        };
        let unrelated = Candidate {
            uid: uid(2),
            kind: NarrativeKind::WorldFact,
            gist: "the harbour freezes each winter".into(),
        };
        let cands = [leviathan, unrelated];
        // A plot point that names the leviathan (possessive) grounds the world-fact
        // (stemming bridges "leviathan" ~ "leviathan's"); the winter fact is untouched.
        let grounds = infer_grounds(
            NarrativeKind::PlotPoint,
            "the escape uses the sea gate in the leviathan's flank",
            "english",
            &cands,
        );
        assert_eq!(grounds, vec![uid(1)], "grounds only the leviathan world-fact");
    }

    #[test]
    fn a_reveal_grounds_a_setup_that_shares_a_word() {
        let setup = Candidate {
            uid: uid(3),
            kind: NarrativeKind::Setup,
            gist: "the tremors shake the lower city".into(),
        };
        let grounds = infer_grounds(
            NarrativeKind::Reveal,
            "the tremors are the beast waking",
            "english",
            &[setup],
        );
        assert_eq!(grounds, vec![uid(3)], "the reveal pays off the tremor setup");
    }

    #[test]
    fn no_shared_content_word_means_no_edge() {
        let wf = Candidate {
            uid: uid(4),
            kind: NarrativeKind::WorldFact,
            gist: "the moons rise together at midsummer".into(),
        };
        let grounds =
            infer_grounds(NarrativeKind::PlotPoint, "the escape uses the sea gate", "english", &[wf]);
        assert!(grounds.is_empty(), "no shared salient word → no ground");
    }
}

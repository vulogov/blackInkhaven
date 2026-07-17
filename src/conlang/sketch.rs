//! LING-1 L-P4 — the grammar sketch.
//!
//! A one-page, human-readable overview of a language, assembled deterministically
//! from everything the analysis suite already computes: the phoneme inventory and
//! its naturalness, vowel harmony and any restricted distributions, the syllable
//! shape, the typological profile, and the lexicon. It is the dashboard rendered
//! as prose — a quick answer to "so what *is* this language?" — and, unlike the
//! AI grammar book, it is instant and always current. No provider needed.

use std::fmt::Write;

use crate::conlang::types::Phonology;
use crate::conlang::types::grammar::GrammarSpec;
use crate::conlang::{distribution, harmony, metrics, naturalness, universals};
use crate::language_entry::DictionaryEntry;

/// Build the sketch text for `name` from its phonology, lexicon and grammar spec.
pub fn sketch(
    name: &str,
    phon: &Phonology,
    entries: &[DictionaryEntry],
    spec: &GrammarSpec,
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{name} — a sketch\n");

    // ── Phonology ──
    let nat = naturalness::naturalness(phon);
    let met = metrics::metrics(phon, entries);
    let _ = write!(
        s,
        "Phonology. {} phonemes ({} consonants, {} vowels), a {} inventory",
        nat.phoneme_count,
        nat.consonants,
        nat.vowels,
        size_word(nat.size_class),
    );
    if nat.phoneme_count > 0 {
        let _ = write!(s, " (naturalness {:.2})", nat.score);
    }
    let _ = writeln!(s, ".");

    if !nat.voicing_gaps.is_empty() {
        let _ = writeln!(s, "  A voicing gap: {} lack{} a counterpart.", nat.voicing_gaps.join(", "),
            if nat.voicing_gaps.len() == 1 { "s" } else { "" });
    }
    if !nat.missing_common.is_empty() {
        let _ = writeln!(s, "  Missing some near-universal segments: {}.", nat.missing_common.join(", "));
    }

    let har = harmony::analyze(phon, entries);
    let harmonic: Vec<String> = har
        .dimensions
        .iter()
        .filter(|d| d.verdict == "strong" || d.verdict == "tendency")
        .map(|d| format!("{} harmony ({})", d.name, d.verdict))
        .collect();
    if !harmonic.is_empty() {
        let _ = writeln!(s, "  Shows {}.", harmonic.join(" and "));
    }

    let dist = distribution::distribution(phon, entries);
    if !dist.restricted.is_empty() {
        let restricted: Vec<String> =
            dist.restricted.iter().map(|d| format!("{} ({})", d.ipa, d.restriction.as_deref().unwrap_or(""))).collect();
        let _ = writeln!(s, "  Restricted distributions: {}.", restricted.join(", "));
    }

    if met.analyzable_words > 0 {
        let _ = writeln!(
            s,
            "  A typical word runs to about {:.1} syllables; phonotactic saturation {:.0}%.",
            met.mean_moras.max(1.0),
            met.syllable_saturation * 100.0,
        );
    }

    // ── Typology ──
    let typ = universals::survey(spec);
    let _ = write!(s, "\nTypology. ");
    match &typ.word_order {
        Some(wo) => {
            let _ = write!(s, "{} word order", wo.to_uppercase());
        }
        None => {
            let _ = write!(s, "Word order unspecified");
        }
    }
    if let Some(mt) = &typ.morphological_type {
        let _ = write!(s, ", {mt}");
    }
    let branch = match typ.branch {
        universals::Branch::HeadInitial => Some("head-initial"),
        universals::Branch::HeadFinal => Some("head-final"),
        universals::Branch::Mixed => Some("mixed-branching"),
        universals::Branch::Unknown => None,
    };
    if let Some(b) = branch {
        let _ = write!(s, ", {b}");
    }
    let _ = writeln!(s, ".");
    let violations = typ
        .checks
        .iter()
        .filter(|c| c.verdict == universals::Verdict::Violated)
        .count();
    if violations > 0 {
        let _ = writeln!(s, "  {violations} typological universal(s) run against the cross-linguistic grain — a marked, deliberate profile.");
    } else if !typ.disharmonic.is_empty() {
        let _ = writeln!(s, "  Disharmonic on: {}.", typ.disharmonic.join(", "));
    }
    if !spec.verb_classes.is_empty() {
        let _ = writeln!(s, "  {} verb class(es) declared.", spec.verb_classes.len());
    }

    // ── Lexicon ──
    let _ = write!(s, "\nLexicon. {} dictionary entr", entries.len());
    let _ = writeln!(s, "{}.", if entries.len() == 1 { "y" } else { "ies" });
    let pairs = crate::conlang::pairs::minimal_pairs(phon, entries, 1);
    if let Some((feat, count)) = pairs.contrast_load.first() {
        let _ = writeln!(s, "  Its heaviest contrast is [{feat}] ({count} minimal pair(s)).");
    }

    s
}

fn size_word(c: naturalness::SizeClass) -> &'static str {
    match c {
        naturalness::SizeClass::Small => "small",
        naturalness::SizeClass::Typical => "typical",
        naturalness::SizeClass::Large => "large",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "p", kind: "consonant" }, { ipa: "b", kind: "consonant" },
                { ipa: "t", kind: "consonant" }, { ipa: "d", kind: "consonant" },
                { ipa: "k", kind: "consonant" }, { ipa: "m", kind: "consonant" },
                { ipa: "n", kind: "consonant" }, { ipa: "s", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }, { ipa: "u", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn entry(w: &str) -> DictionaryEntry {
        DictionaryEntry { word: w.into(), pos: "noun".into(), translation: "x".into(), ..Default::default() }
    }

    fn spec() -> GrammarSpec {
        let mut g = BTreeMap::new();
        g.insert("word_order".to_string(), "sov".to_string());
        g.insert("morphological_type".to_string(), "agglutinative".to_string());
        GrammarSpec { grammar: g, ..Default::default() }
    }

    #[test]
    fn a_defined_language_produces_a_multi_section_sketch() {
        let text = sketch("Eldar", &phon(), &[entry("pata"), entry("kibu"), entry("tasi")], &spec());
        assert!(text.contains("Eldar — a sketch"));
        assert!(text.contains("Phonology."));
        assert!(text.contains("11 phonemes"));
        assert!(text.contains("Typology."));
        assert!(text.contains("SOV word order"));
        assert!(text.contains("agglutinative"));
        assert!(text.contains("Lexicon."));
    }

    #[test]
    fn an_empty_language_sketches_gracefully() {
        let empty = Phonology::from_hjson("{ phonemes: [] }").unwrap().unwrap();
        let text = sketch("Void", &empty, &[], &GrammarSpec::default());
        assert!(text.contains("Void — a sketch"));
        assert!(text.contains("0 phonemes"));
        assert!(text.contains("Word order unspecified"));
    }
}

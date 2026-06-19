//! Derived-form proposals (LANG-1 P3.3).
//!
//! Apply the language's derivational rules to a root to coin new lexemes
//! (agent nouns, diminutives, verbal nouns…). Forward generation reusing the
//! affix-application + allophony path; the results are *proposals* — the CLI
//! offers them and only commits on `--yes`. Pure and deterministic.

use crate::conlang::phonology::allophony_eval;
use crate::conlang::types::morphology::{AffixPosition, Morphology};
use crate::conlang::types::Phonology;

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedForm {
    pub rule: String,
    pub form: String,
    pub gloss: String,
    pub pos: String,
}

/// Propose derived lexemes for `root` (gloss `root_gloss`, part of speech
/// `root_pos`). A rule fires when its `from_pos` is unset or matches
/// `root_pos`.
pub fn generate(
    phon: &Phonology,
    morph: &Morphology,
    root: &str,
    root_gloss: &str,
    root_pos: &str,
) -> Vec<DerivedForm> {
    morph
        .derivations
        .iter()
        .filter(|r| r.from_pos.as_deref().is_none_or(|p| p.eq_ignore_ascii_case(root_pos)))
        .map(|r| {
            let underlying = match r.position {
                AffixPosition::Prefix => format!("{}{root}", r.form),
                AffixPosition::Suffix => format!("{root}{}", r.form),
                // Infix / circumfix: P3.x — no-op for now.
                _ => root.to_string(),
            };
            let surface = allophony_eval::surface_form(phon, &phon.segment(&underlying));
            let form = render(phon, &surface);

            let gloss = match &r.gloss_template {
                Some(t) if t.contains("{}") => t.replace("{}", root_gloss),
                _ if !r.gloss.is_empty() => format!("{root_gloss}.{}", r.gloss),
                _ => format!("{root_gloss} ({})", r.name),
            };

            DerivedForm { rule: r.name.clone(), form, gloss, pos: r.to_pos.clone() }
        })
        .collect()
}

fn render(phon: &Phonology, seq: &[String]) -> String {
    seq.iter()
        .map(|ipa| phon.phoneme(ipa).map(|p| p.grapheme()).unwrap_or(ipa.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "r", kind: "consonant" }, { ipa: "n", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }, { ipa: "o", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn morph() -> Morphology {
        let body = r#"{
            derivations: [
                { name: "agent", form: "ron", position: "suffix", from_pos: "verb", to_pos: "noun",
                  gloss_template: "one who {}s" }
                { name: "diminutive", gloss: "DIM", form: "i", position: "suffix", to_pos: "noun" }
                { name: "negation", form: "na", position: "prefix", from_pos: "adjective", to_pos: "adjective" }
            ]
        }"#;
        Morphology::from_hjson(body).unwrap().unwrap()
    }

    #[test]
    fn applies_pos_matching_rules() {
        let d = generate(&phon(), &morph(), "kata", "build", "verb");
        // agent (verb → noun) + diminutive (any). negation requires adjective.
        assert_eq!(d.len(), 2);
        let agent = d.iter().find(|x| x.rule == "agent").unwrap();
        assert_eq!(agent.form, "kataron");
        assert_eq!(agent.gloss, "one who builds");
        assert_eq!(agent.pos, "noun");
        let dim = d.iter().find(|x| x.rule == "diminutive").unwrap();
        assert_eq!(dim.form, "katai");
        assert_eq!(dim.gloss, "build.DIM");
    }

    #[test]
    fn prefix_derivation_and_any_pos() {
        let d = generate(&phon(), &morph(), "tana", "good", "adjective");
        // negation (adjective, prefix) + diminutive (any).
        let neg = d.iter().find(|x| x.rule == "negation").unwrap();
        assert_eq!(neg.form, "natana");
        assert_eq!(neg.gloss, "good (negation)");
    }
}

//! Deterministic word generation (LANG-1 P1.1).
//!
//! Given a phonology and a template role, realize a template into a phoneme
//! sequence and accept it only if it passes the phonotactic validator. The
//! whole path is a pure function of `(phonology, role, seed)` — no global
//! RNG — so output is reproducible and property-testable. A tiny inlined
//! SplitMix64 supplies the randomness (no new dependency).

use crate::conlang::phonology::validator;
use crate::conlang::types::{Phonology, SyllableTemplate, TemplateRole};

/// How many seed-stepped attempts to make a single word legal before giving
/// up. Bounded so impossible constraint sets terminate instead of looping.
const RETRY_BUDGET: u64 = 64;

/// Generate one word for `role`, deterministic in `seed`. Returns `None`
/// when the role has no templates, or when no candidate satisfied the
/// constraints within the retry budget.
pub fn generate_word(phon: &Phonology, role: TemplateRole, seed: u64) -> Option<String> {
    let templates = phon.templates_for(role);
    if templates.is_empty() {
        return None;
    }
    for step in 0..RETRY_BUDGET {
        let mut rng = SplitMix::new(seed.wrapping_add(step.wrapping_mul(0x1000_0001)));
        if let Some(seq) = realize(phon, templates, &mut rng) {
            // Phonotactics constrain the *underlying* form; allophony then
            // derives the surface form the author actually sees.
            if validator::is_legal(phon, &seq) {
                let surface = crate::conlang::phonology::allophony_eval::surface_form(phon, &seq);
                return Some(render(phon, &surface));
            }
        }
    }
    None
}

/// Generate `count` words for `role`. Word `i` is seeded by `i`, so the
/// whole batch is reproducible for a given phonology. Candidates that can't
/// satisfy the constraints are skipped (so the result may be shorter than
/// `count`); duplicates are allowed — the lexicon layer dedups later.
pub fn generate_words(phon: &Phonology, role: TemplateRole, count: usize) -> Vec<String> {
    (0..count as u64)
        .filter_map(|i| generate_word(phon, role, i))
        .collect()
}

/// Realize a (weighted) template into a phoneme sequence by filling each
/// atom from its class. Returns `None` if a required class is empty or the
/// realization came out empty.
fn realize(phon: &Phonology, templates: &[SyllableTemplate], rng: &mut SplitMix) -> Option<Vec<String>> {
    let tmpl = weighted_pick(templates, rng)?;
    let mut out = Vec::new();
    for atom in &tmpl.pattern {
        if atom.is_optional() && !rng.next_bool() {
            continue;
        }
        let members = phon.class_members(atom.class_name());
        if members.is_empty() {
            if atom.is_optional() {
                continue;
            }
            return None;
        }
        let idx = (rng.next_u64() as usize) % members.len();
        out.push(members[idx].clone());
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Pick a template proportional to its weight (negative weights clamp to 0).
/// Falls back to the first template when all weights are zero.
fn weighted_pick<'a>(ts: &'a [SyllableTemplate], rng: &mut SplitMix) -> Option<&'a SyllableTemplate> {
    let total: f32 = ts.iter().map(|t| t.weight.max(0.0)).sum();
    if total <= 0.0 {
        return ts.first();
    }
    let mut r = rng.next_f32() * total;
    for t in ts {
        let w = t.weight.max(0.0);
        if r < w {
            return Some(t);
        }
        r -= w;
    }
    ts.last()
}

/// Render a phoneme sequence to its written form: each phoneme's grapheme
/// (romanization when set, else IPA), concatenated.
fn render(phon: &Phonology, seq: &[String]) -> String {
    seq.iter()
        .map(|ipa| phon.phoneme(ipa).map(|p| p.grapheme()).unwrap_or(ipa.as_str()))
        .collect()
}

/// Inlined SplitMix64 — deterministic, seedable, dependency-free.
struct SplitMix {
    state: u64,
}

impl SplitMix {
    fn new(seed: u64) -> Self {
        Self { state: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A float in `[0, 1)` from the top 24 bits.
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind, PhonotacticConstraint};

    /// A small CV(C) language: stops + liquids + /s/, three vowels.
    fn lang() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "k", kind: "consonant" }, { ipa: "s", kind: "consonant" },
                { ipa: "r", kind: "consonant" }, { ipa: "l", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }, { ipa: "u", kind: "vowel" }
            ],
            classes: { C: ["p","t","k","s","r","l"], V: ["a","i","u"] },
            templates: { root: [ { pattern: "C V (C)", weight: 1.0 }, { pattern: "C V", weight: 2.0 } ] },
            constraints: [ { kind: "max_cluster_size", value: 1 }, { kind: "no_geminate" } ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    #[test]
    fn parses_the_phonology_block() {
        let p = lang();
        assert_eq!(p.phonemes.len(), 9);
        assert_eq!(p.class_members("C").len(), 6);
        assert_eq!(p.templates_for(TemplateRole::Root).len(), 2);
        assert_eq!(p.constraints.len(), 2);
    }

    #[test]
    fn generation_is_deterministic_per_seed() {
        let p = lang();
        let a = generate_word(&p, TemplateRole::Root, 7);
        let b = generate_word(&p, TemplateRole::Root, 7);
        assert_eq!(a, b);
        assert!(a.is_some());
    }

    #[test]
    fn every_generated_word_satisfies_the_constraints() {
        // Property: across many seeds, no word violates the declared
        // phonotactics (max cluster 1 → no two adjacent consonants; no
        // geminates). We re-derive the phoneme sequence by greedy longest
        // grapheme match to validate (all graphemes here are 1 char).
        let p = lang();
        let words = generate_words(&p, TemplateRole::Root, 200);
        assert!(words.len() > 150, "most seeds should yield a word, got {}", words.len());
        for w in &words {
            let seq: Vec<String> = w.chars().map(|c| c.to_string()).collect();
            assert!(
                validator::is_legal(&p, &seq),
                "generated `{w}` violates its own phonotactics",
            );
            // CV(C): never starts or ends mid-cluster; never longer than 3.
            assert!(!w.is_empty() && w.chars().count() <= 3, "unexpected shape: {w}");
        }
    }

    #[test]
    fn unknown_role_yields_nothing() {
        let p = lang();
        assert!(generate_word(&p, TemplateRole::Suffix, 0).is_none());
        assert!(generate_words(&p, TemplateRole::Suffix, 10).is_empty());
    }

    #[test]
    fn romanization_is_used_when_present() {
        let p = Phonology {
            phonemes: vec![
                Phoneme { ipa: "ʃ".into(), romanize: Some("sh".into()), kind: PhonemeKind::Consonant, sonority: None },
                Phoneme { ipa: "a".into(), romanize: None, kind: PhonemeKind::Vowel, sonority: None },
            ],
            classes: [("C".to_string(), vec!["ʃ".to_string()]), ("V".to_string(), vec!["a".to_string()])]
                .into_iter()
                .collect(),
            templates: [("root".to_string(), vec![SyllableTemplate {
                pattern: vec![
                    crate::conlang::types::template::TemplateAtom::Class("C".into()),
                    crate::conlang::types::template::TemplateAtom::Class("V".into()),
                ],
                weight: 1.0,
            }])]
            .into_iter()
            .collect(),
            constraints: vec![PhonotacticConstraint::NoGeminate],
            allophony: Vec::new(),
            stress: None,
            max_word_syllables: 4,
        };
        let w = generate_word(&p, TemplateRole::Root, 1).unwrap();
        assert_eq!(w, "sha"); // ʃ → "sh", a → "a"
    }
}

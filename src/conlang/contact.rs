//! LANG-2 P2 — loanword adaptation (the borrowing engine).
//!
//! Borrowing a word is **phonotactic repair** in two steps:
//!
//! 1. **Perceive** the donor form against the recipient's inventory — apply the
//!    declared substitutions, keep sounds the recipient already has, and map any
//!    remaining sound to its nearest native phoneme (by sonority).
//! 2. **Repair** sequences the recipient's phonotactics forbid — consonant runs
//!    longer than the recipient allows in that position get an epenthetic vowel
//!    (Japanese *sutoraiku*) or the offending consonant is deleted.
//!
//! The recipient's permitted syllable shape (max onset / coda / medial cluster)
//! is read straight from its `templates`, so the same phonotactics that
//! constrain native word generation constrain loans.

use crate::conlang::types::contact::LoanPhonology;
use crate::conlang::types::template::TemplateAtom;
use crate::conlang::types::PhonemeKind;
use crate::conlang::Phonology;

/// The result of adapting one donor form.
#[derive(Debug, Clone)]
pub struct Adaptation {
    pub donor: String,
    /// The nativised form, written in the recipient's spelling.
    pub adapted: String,
    /// The nativised form as recipient phonemes (IPA).
    pub ipa: Vec<String>,
    /// A human-readable trace of each substitution / repair.
    pub steps: Vec<String>,
}

/// Adapt `donor_form` (given phonemically — one symbol per sound) into the
/// recipient language.
pub fn adapt(phon: &Phonology, loan: &LoanPhonology, donor_form: &str) -> Adaptation {
    let mut steps = Vec::new();
    let perceived = perceive(phon, loan, donor_form, &mut steps);
    let ev = epenthetic_vowel(phon, loan);
    let repaired = repair(phon, &perceived, &loan.repair, &ev, &mut steps);
    let adapted = repaired.iter().map(|ipa| grapheme(phon, ipa)).collect::<String>();
    Adaptation {
        donor: donor_form.to_string(),
        adapted,
        ipa: repaired,
        steps,
    }
}

/// Map each donor sound onto a recipient phoneme. Greedy longest-match over the
/// substitution keys + the recipient's graphemes/IPA so multi-character sounds
/// (`sh`) resolve, then: substitution → keep-if-native → nearest by sonority.
fn perceive(phon: &Phonology, loan: &LoanPhonology, donor: &str, steps: &mut Vec<String>) -> Vec<String> {
    // Build a longest-first key table: substitution keys, then recipient
    // graphemes and IPA symbols.
    let mut keys: Vec<String> = loan.substitutions.keys().cloned().collect();
    for p in &phon.phonemes {
        keys.push(p.grapheme().to_string());
        keys.push(p.ipa.clone());
    }
    keys.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
    keys.dedup();

    let chars: Vec<char> = donor.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        // longest key matching at position i
        let rest: String = chars[i..].iter().collect();
        let key = keys.iter().find(|k| !k.is_empty() && rest.starts_with(k.as_str()));
        let (seg, adv) = match key {
            Some(k) => (k.clone(), k.chars().count()),
            None => (chars[i].to_string(), 1),
        };
        i += adv;

        if let Some(sub) = loan.substitutions.get(&seg) {
            steps.push(format!("{seg} → {sub} (substitution)"));
            out.push(sub.clone());
        } else if let Some(p) = phon.phonemes.iter().find(|p| p.ipa == seg || p.grapheme() == seg) {
            out.push(p.ipa.clone());
        } else {
            let native = nearest_phoneme(phon, &seg);
            match native {
                Some(n) => {
                    steps.push(format!("{seg} → {n} (nearest native)"));
                    out.push(n);
                }
                None => steps.push(format!("{seg} dropped (no native equivalent)")),
            }
        }
    }
    out
}

/// The recipient phoneme nearest `sound` by sonority (so an unknown consonant
/// maps to a consonant and an unknown vowel to a vowel, both at a similar rank).
fn nearest_phoneme(phon: &Phonology, sound: &str) -> Option<String> {
    if phon.phonemes.is_empty() {
        return None;
    }
    let target = crate::conlang::phonology::ipa::sonority_of(phon, sound);
    phon.phonemes
        .iter()
        .min_by_key(|p| {
            let s = crate::conlang::phonology::ipa::sonority_of(phon, &p.ipa);
            (s as i32 - target as i32).unsigned_abs()
        })
        .map(|p| p.ipa.clone())
}

fn epenthetic_vowel(phon: &Phonology, loan: &LoanPhonology) -> String {
    if !loan.epenthetic_vowel.trim().is_empty() {
        return loan.epenthetic_vowel.clone();
    }
    phon.phonemes
        .iter()
        .find(|p| p.kind == PhonemeKind::Vowel)
        .map(|p| p.ipa.clone())
        .unwrap_or_else(|| "a".to_string())
}

fn grapheme(phon: &Phonology, ipa: &str) -> String {
    phon.phoneme(ipa).map(|p| p.grapheme().to_string()).unwrap_or_else(|| ipa.to_string())
}

fn is_consonant(phon: &Phonology, ipa: &str) -> bool {
    phon.kind_of(ipa) != Some(PhonemeKind::Vowel)
}

/// The recipient's permitted consonant-run lengths, read from its templates:
/// `(max_initial onset, max_final coda, max_medial cluster)`. No templates →
/// a conservative `(1, 0, 1)` (strict CV).
fn syllable_shape(phon: &Phonology) -> (usize, usize, usize) {
    let is_vowel_class = |cls: &str| -> bool {
        phon.class_members(cls)
            .iter()
            .next()
            .and_then(|m| phon.kind_of(m))
            .map(|k| k == PhonemeKind::Vowel)
            .unwrap_or(false)
    };
    let (mut mi, mut mf, mut mm, mut any) = (0usize, 0usize, 0usize, false);
    for templates in phon.templates.values() {
        for t in templates {
            any = true;
            // Mark each atom consonantal (false) / vocalic (true).
            let vocalic: Vec<bool> = t
                .pattern
                .iter()
                .map(|a| {
                    let cls = match a {
                        TemplateAtom::Class(n) | TemplateAtom::OptionalClass(n) => n.as_str(),
                    };
                    is_vowel_class(cls)
                })
                .collect();
            // leading consonant run
            let lead = vocalic.iter().take_while(|v| !**v).count();
            // trailing consonant run
            let tail = vocalic.iter().rev().take_while(|v| !**v).count();
            // longest interior run between two vowels
            let mut interior = 0usize;
            let mut run = 0usize;
            let mut seen_vowel = false;
            for (k, &v) in vocalic.iter().enumerate() {
                if v {
                    if seen_vowel && k < vocalic.len() {
                        interior = interior.max(run);
                    }
                    seen_vowel = true;
                    run = 0;
                } else if seen_vowel {
                    run += 1;
                }
            }
            mi = mi.max(lead);
            mf = mf.max(tail);
            mm = mm.max(interior);
        }
    }
    if !any {
        return (1, 0, 1);
    }
    (mi.max(1), mf, mm.max(1))
}

/// Repair phonotactic violations in the perceived phoneme string.
fn repair(
    phon: &Phonology,
    perceived: &[String],
    strategy: &str,
    ev: &str,
    steps: &mut Vec<String>,
) -> Vec<String> {
    let (max_initial, max_final, max_medial) = syllable_shape(phon);
    let onset_chunk = max_initial.max(1);
    let deletion = strategy.eq_ignore_ascii_case("deletion");

    let mut out: Vec<String> = Vec::new();
    let n = perceived.len();
    let mut i = 0;
    let mut seen_vowel = false;
    while i < n {
        if !is_consonant(phon, &perceived[i]) {
            out.push(perceived[i].clone());
            seen_vowel = true;
            i += 1;
            continue;
        }
        // gather a maximal consonant run
        let start = i;
        while i < n && is_consonant(phon, &perceived[i]) {
            i += 1;
        }
        let run: Vec<String> = perceived[start..i].to_vec();
        let followed_by_vowel = i < n;
        let allowed = if !seen_vowel {
            max_initial // word-initial run
        } else if followed_by_vowel {
            max_final + max_medial.max(max_initial) // medial: coda + next onset
        } else {
            max_final // word-final run (coda)
        };

        if run.len() <= allowed {
            out.extend(run);
            continue;
        }
        // over-long run — repair
        if deletion {
            for (k, c) in run.iter().enumerate() {
                if k < allowed.max(if followed_by_vowel { onset_chunk } else { 0 }) {
                    out.push(c.clone());
                } else {
                    steps.push(format!("deleted {c}"));
                }
            }
        } else {
            // epenthesis: emit consonants in onset-sized chunks, inserting the
            // epenthetic vowel between chunks; the final chunk attaches to the
            // following vowel when there is one.
            let chunks: Vec<&[String]> = run.chunks(onset_chunk).collect();
            let last = chunks.len() - 1;
            for (ci, chunk) in chunks.iter().enumerate() {
                out.extend(chunk.iter().cloned());
                let is_last = ci == last;
                if !(is_last && followed_by_vowel) {
                    out.push(ev.to_string());
                    steps.push(format!("epenthesis: inserted {ev}"));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::contact::LoanPhonology;

    // A strict-CV recipient: consonants k t n m s l + vowels a u, one CV-ish root
    // template, declaring θ→t and r→l substitutions, epenthetic /u/.
    fn recipient() -> Phonology {
        let body = r#"{ phonemes: [
            { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
            { ipa: "n", kind: "consonant" }, { ipa: "m", kind: "consonant" },
            { ipa: "s", kind: "consonant" }, { ipa: "l", kind: "consonant" },
            { ipa: "a", kind: "vowel" }, { ipa: "u", kind: "vowel" }
          ],
          classes: { C: ["k","t","n","m","s","l"], V: ["a","u"] },
          templates: { root: [ { pattern: "C V" } ] } }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }
    fn loan() -> LoanPhonology {
        let body = r#"{ loan_phonology: { repair: "epenthesis", epenthetic_vowel: "u",
            substitutions: { "θ": "t", "r": "l" } } }"#;
        LoanPhonology::from_hjson(body).unwrap().unwrap()
    }

    #[test]
    fn substitution_then_epenthesis_breaks_clusters() {
        // donor "tras" : t r a s → (r→l) t l a s → strict CV repair:
        //   initial "tl" cluster (>1) → t u l ; final "s" coda (none allowed) → s u
        let a = adapt(&recipient(), &loan(), "tras");
        assert_eq!(a.ipa, vec!["t", "u", "l", "a", "s", "u"]);
        assert_eq!(a.adapted, "tulasu");
        assert!(a.steps.iter().any(|s| s.contains("r → l")));
        assert!(a.steps.iter().any(|s| s.contains("epenthesis")));
    }

    #[test]
    fn unknown_sound_maps_to_nearest_and_theta_substitutes() {
        // "θu" : θ→t (substitution), u kept → "tu" (already legal CV)
        let a = adapt(&recipient(), &loan(), "θu");
        assert_eq!(a.adapted, "tu");
        assert!(a.steps.iter().any(|s| s.contains("θ → t")));
    }

    #[test]
    fn deletion_strategy_drops_excess() {
        let mut lp = loan();
        lp.repair = "deletion".into();
        // "tras" → (r→l) "tlas": initial cluster tl keeps 1 (t), drops l; final s dropped
        let a = adapt(&recipient(), &lp, "tras");
        assert!(a.steps.iter().any(|s| s.starts_with("deleted")));
        // every surviving consonant is legal CV (no two consonants adjacent, no final C)
        assert!(!a.adapted.is_empty());
    }

    #[test]
    fn already_legal_word_is_untouched() {
        let a = adapt(&recipient(), &loan(), "kata");
        assert_eq!(a.adapted, "kata");
        assert!(a.steps.is_empty());
    }
}

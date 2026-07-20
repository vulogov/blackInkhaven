//! Lexicon analysis (LANG-1 P2.1).
//!
//! Deterministic consistency + dedup checks over a language's dictionary,
//! bridging the P1 phonology engine to the existing 1.2.13 `DictionaryEntry`
//! model. This is the *deterministic half* of the generation dedup gate
//! (LANG-1 §P2): the AI lexicon pipeline will reuse `analyze` to reject a
//! proposal that collides with the existing lexicon. It surfaces:
//!
//!   * **phonotactic violations** — a headword whose segmented form breaks
//!     the language's own constraints (often a loanword / proper noun, so
//!     advisory),
//!   * **homophones** — distinct entries that share a *surface* form (after
//!     allophony),
//!   * **duplicate meanings** — distinct entries with the same gloss (an
//!     accidental synonym).
//!
//! Semantic near-synonymy (e.g. "stone" vs "rock" via embeddings) is the
//! AI-assisted half and lands with the generation pipeline.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::conlang::phonology::{allophony_eval, validator};
use crate::conlang::types::Phonology;
use crate::language_entry::DictionaryEntry;

#[derive(Debug, Serialize)]
pub struct LexiconReport {
    pub total: usize,
    pub phonotactic_violations: Vec<Violation>,
    pub homophones: Vec<Collision>,
    pub duplicate_meanings: Vec<Collision>,
}

#[derive(Debug, Serialize)]
pub struct Violation {
    pub headword: String,
    pub underlying: String,
}

#[derive(Debug, Serialize)]
pub struct Collision {
    /// The shared key — a surface form (homophones) or a gloss (synonyms).
    pub key: String,
    pub members: Vec<Member>,
}

#[derive(Debug, Serialize)]
pub struct Member {
    pub headword: String,
    pub gloss: String,
}

impl LexiconReport {
    pub fn issue_count(&self) -> usize {
        self.phonotactic_violations.len() + self.homophones.len() + self.duplicate_meanings.len()
    }
}

/// Query criteria over the rich entry fields (LANG-1 P2.4). A `None` field is
/// "don't care"; tag matches are case-insensitive membership; `text` is a
/// case-insensitive substring over the headword + gloss.
#[derive(Debug, Default)]
pub struct Filter<'a> {
    pub register: Option<&'a str>,
    pub domain: Option<&'a str>,
    pub era: Option<&'a str>,
    pub pos: Option<&'a str>,
    pub text: Option<&'a str>,
}

impl Filter<'_> {
    fn matches(&self, e: &DictionaryEntry) -> bool {
        let tag = |needle: Option<&str>, hay: &[String]| {
            needle.is_none_or(|n| hay.iter().any(|x| x.eq_ignore_ascii_case(n)))
        };
        tag(self.register, &e.registers)
            && tag(self.domain, &e.domain)
            && self.era.is_none_or(|q| e.era.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(q)))
            && self.pos.is_none_or(|q| e.pos.eq_ignore_ascii_case(q))
            && self.text.is_none_or(|q| {
                let q = q.to_lowercase();
                e.word.to_lowercase().contains(&q) || e.translation.to_lowercase().contains(&q)
            })
    }
}

/// Filter a dictionary by the rich fields, preserving order.
pub fn filter<'a>(entries: &'a [DictionaryEntry], f: &Filter) -> Vec<&'a DictionaryEntry> {
    entries.iter().filter(|e| f.matches(e)).collect()
}

// ── Manuscript undefined-word scan (LANG-1 P2.7) ──────────────────────────

#[derive(Debug, Serialize)]
pub struct UndefinedReport {
    pub candidates: Vec<UndefinedWord>,
    pub paragraphs_scanned: usize,
    /// Paragraphs that contained ≥1 known conlang word (the scanned context).
    pub conlang_paragraphs: usize,
}

#[derive(Debug, Serialize)]
pub struct UndefinedWord {
    pub word: String,
    pub count: usize,
}

/// True when `word_lc` (lowercased) reads entirely as this language's
/// phonemes (no stray non-inventory characters) and satisfies its
/// phonotactics — i.e. it *looks like* a valid word of the language. Shared with
/// the corpus engine's manuscript-prose ingestion.
pub(crate) fn looks_conlang(phon: &Phonology, word_lc: &str) -> bool {
    let seq = phon.segment(word_lc);
    // ≥2 phonemes avoids flagging stray single letters ("a", "i") that
    // happen to be in the inventory.
    seq.len() >= 2
        && seq.iter().all(|s| phon.phoneme(s).is_some())
        && validator::is_legal(phon, &seq)
}

/// Scan manuscript paragraphs (each a list of words) for **candidate
/// undefined conlang words**: words that look like the language (segment
/// fully into its inventory + pass phonotactics) but aren't in the lexicon.
///
/// Precision guard: only paragraphs that already contain ≥1 *known* lexicon
/// word are scanned, so prose written entirely in the working language is
/// skipped — the candidates come from genuine conlang passages. `known` is
/// the set of lowercased lexicon surface forms. Heuristic (best for a
/// distinct inventory); the author reviews the list.
pub fn scan_undefined(
    phon: &Phonology,
    known: &std::collections::HashSet<String>,
    paragraphs: &[Vec<String>],
) -> UndefinedReport {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut conlang_paragraphs = 0;

    for words in paragraphs {
        let in_context = words.iter().any(|w| known.contains(&w.to_lowercase()));
        if !in_context {
            continue;
        }
        conlang_paragraphs += 1;
        for w in words {
            let lc = w.to_lowercase();
            if known.contains(&lc) || !looks_conlang(phon, &lc) {
                continue;
            }
            *counts.entry(lc).or_default() += 1;
        }
    }

    let mut candidates: Vec<UndefinedWord> =
        counts.into_iter().map(|(word, count)| UndefinedWord { word, count }).collect();
    candidates.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.word.cmp(&b.word)));

    UndefinedReport {
        candidates,
        paragraphs_scanned: paragraphs.len(),
        conlang_paragraphs,
    }
}

/// Audit a dictionary against its phonology. Pure; the phonology may be empty
/// (`Phonology::default()`), in which case the phonotactic check is skipped
/// and homophones reduce to spelling collisions.
pub fn analyze(phon: &Phonology, entries: &[DictionaryEntry]) -> LexiconReport {
    let mut by_surface: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut by_gloss: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut violations: Vec<Violation> = Vec::new();
    let check_phonotactics = !phon.constraints.is_empty();

    for (i, e) in entries.iter().enumerate() {
        let head = e.word.trim();
        if head.is_empty() {
            continue;
        }
        let underlying = phon.segment(head);
        if check_phonotactics && !validator::is_legal(phon, &underlying) {
            violations.push(Violation {
                headword: head.to_string(),
                underlying: underlying.join(""),
            });
        }
        let surface = allophony_eval::surface_form(phon, &underlying).join("");
        by_surface.entry(surface).or_default().push(i);

        let gloss = e.translation.trim().to_lowercase();
        if !gloss.is_empty() {
            by_gloss.entry(gloss).or_default().push(i);
        }
    }

    let members = |ids: &[usize]| -> Vec<Member> {
        ids.iter()
            .map(|&i| Member {
                headword: entries[i].word.trim().to_string(),
                gloss: entries[i].translation.trim().to_string(),
            })
            .collect()
    };
    let collisions = |map: BTreeMap<String, Vec<usize>>| -> Vec<Collision> {
        map.into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .map(|(key, ids)| Collision { key, members: members(&ids) })
            .collect()
    };

    LexiconReport {
        total: entries.len(),
        phonotactic_violations: violations,
        homophones: collisions(by_surface),
        duplicate_meanings: collisions(by_gloss),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind, PhonotacticConstraint};

    fn entry(word: &str, gloss: &str) -> DictionaryEntry {
        DictionaryEntry {
            word: word.into(),
            pos: "noun".into(),
            translation: gloss.into(),
            ..Default::default()
        }
    }

    fn phon() -> Phonology {
        let mut p = Phonology {
            phonemes: vec![
                Phoneme { ipa: "k".into(), romanize: Some("k".into()), kind: PhonemeKind::Consonant, sonority: None },
                Phoneme { ipa: "t".into(), romanize: Some("t".into()), kind: PhonemeKind::Consonant, sonority: None },
                Phoneme { ipa: "a".into(), romanize: Some("a".into()), kind: PhonemeKind::Vowel, sonority: None },
            ],
            ..Default::default()
        };
        // Forbid consonant clusters so "kta" is illegal.
        p.constraints = vec![PhonotacticConstraint::MaxClusterSize(1)];
        p
    }

    #[test]
    fn flags_duplicate_meanings() {
        let p = phon();
        let entries = vec![entry("kata", "stone"), entry("taka", "stone"), entry("kaka", "water")];
        let r = analyze(&p, &entries);
        assert_eq!(r.duplicate_meanings.len(), 1);
        assert_eq!(r.duplicate_meanings[0].key, "stone");
        assert_eq!(r.duplicate_meanings[0].members.len(), 2);
    }

    #[test]
    fn flags_homophones() {
        let p = phon();
        // Two entries, same form, different meaning → homophones.
        let entries = vec![entry("kata", "stone"), entry("kata", "river")];
        let r = analyze(&p, &entries);
        assert_eq!(r.homophones.len(), 1);
        assert_eq!(r.homophones[0].members.len(), 2);
    }

    #[test]
    fn flags_phonotactic_violations() {
        let p = phon();
        let entries = vec![entry("kata", "ok"), entry("kta", "bad-cluster")];
        let r = analyze(&p, &entries);
        assert_eq!(r.phonotactic_violations.len(), 1);
        assert_eq!(r.phonotactic_violations[0].headword, "kta");
    }

    #[test]
    fn clean_lexicon_has_no_issues() {
        let p = phon();
        let entries = vec![entry("kata", "stone"), entry("taka", "water")];
        assert_eq!(analyze(&p, &entries).issue_count(), 0);
    }

    fn rich(word: &str, gloss: &str, registers: &[&str], domain: &[&str], era: Option<&str>) -> DictionaryEntry {
        DictionaryEntry {
            word: word.into(),
            pos: "noun".into(),
            translation: gloss.into(),
            registers: registers.iter().map(|s| s.to_string()).collect(),
            domain: domain.iter().map(|s| s.to_string()).collect(),
            era: era.map(String::from),
            ..Default::default()
        }
    }

    #[test]
    fn scan_flags_undefined_only_in_conlang_context() {
        let p = phon(); // inventory k/t/a, max cluster 1
        let known: std::collections::HashSet<String> =
            ["kata", "taka"].into_iter().map(String::from).collect();
        let words = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        let paragraphs = vec![
            // conlang context (has "kata"): "tata" looks conlang + unknown → flagged.
            words("the hero said kata then tata"),
            // working-language only (no known word): skipped entirely.
            words("she walked into the room quietly"),
            // conlang context again: "tata" repeats; "ktt" fails phonotactics → not flagged.
            words("taka tata ktt"),
        ];
        let r = scan_undefined(&p, &known, &paragraphs);
        assert_eq!(r.conlang_paragraphs, 2);
        assert_eq!(r.candidates.len(), 1);
        assert_eq!(r.candidates[0].word, "tata");
        assert_eq!(r.candidates[0].count, 2);
    }

    #[test]
    fn filter_by_rich_fields() {
        let entries = vec![
            rich("makil", "sword", &["formal"], &["weapon"], Some("third_age")),
            rich("nen", "water", &[], &["nature"], Some("first_age")),
            rich("gurth", "death", &["sacred"], &["weapon"], Some("third_age")),
        ];
        let by_domain = filter(&entries, &Filter { domain: Some("weapon"), ..Default::default() });
        assert_eq!(by_domain.len(), 2);
        let by_reg = filter(&entries, &Filter { register: Some("Formal"), ..Default::default() });
        assert_eq!(by_reg.len(), 1);
        assert_eq!(by_reg[0].word, "makil");
        let by_era_and_domain = filter(
            &entries,
            &Filter { era: Some("third_age"), domain: Some("weapon"), ..Default::default() },
        );
        assert_eq!(by_era_and_domain.len(), 2);
        let by_text = filter(&entries, &Filter { text: Some("wat"), ..Default::default() });
        assert_eq!(by_text.len(), 1);
        assert_eq!(by_text[0].word, "nen");
        // Empty filter returns everything.
        assert_eq!(filter(&entries, &Filter::default()).len(), 3);
    }
}

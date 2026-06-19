//! AI-assisted lexicon generation (LANG-1 P2.2).
//!
//! The deterministic scaffolding around the AI dictionary generator. The
//! invariant (LANG-1 §P2): **forms obey the language, meanings come from the
//! AI, nothing duplicates.** The deterministic word generator (P1.1) builds a
//! pool of phonotactically-valid candidate forms; the AI assigns each a
//! concept/gloss/POS; then every proposal passes the **dedup gate** before it
//! can be queued — rejecting a form that is illegal, a surface-homophone of
//! an existing/kept entry, or a gloss that duplicates one. The AI call itself
//! lives in the CLI (thin, like `lang bootstrap`); everything here is pure.

use std::collections::HashSet;

use serde::Deserialize;

use crate::conlang::generate::word;
use crate::conlang::phonology::{allophony_eval, validator};
use crate::conlang::types::{Phonology, TemplateRole};
use crate::language_entry::DictionaryEntry;

/// One AI-proposed entry (the JSON the model returns).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LexProposal {
    pub form: String,
    pub gloss: String,
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub example: String,
    /// Register tag the AI assigned (P2.5; may be empty).
    #[serde(default)]
    pub register: String,
    /// Semantic-domain tags the AI assigned (P2.5; may be empty).
    #[serde(default)]
    pub domain: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// The form breaks the language's phonotactics.
    Illegal,
    /// The form is a surface-homophone of an existing or already-kept entry.
    Homophone,
    /// The gloss duplicates an existing or already-kept entry's meaning.
    DuplicateMeaning,
}

impl RejectReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Illegal => "phonotactically illegal",
            Self::Homophone => "homophone of an existing word",
            Self::DuplicateMeaning => "duplicates an existing meaning",
        }
    }
}

/// Generate a pool of distinct, phonotactically-valid candidate forms for the
/// AI to choose from, excluding any that collide (by surface form) with the
/// existing lexicon. The AI is asked to pick from this pool, but the dedup
/// gate re-checks its output regardless.
pub fn build_pool(phon: &Phonology, existing: &[DictionaryEntry], target: usize) -> Vec<String> {
    let taken: HashSet<String> = existing
        .iter()
        .map(|e| surface_key(phon, e.word.trim()))
        .collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut pool = Vec::new();
    // Over-generate so the AI has options after dedup.
    for w in word::generate_words(phon, TemplateRole::Root, target.saturating_mul(4).max(8)) {
        let key = surface_key(phon, &w);
        if taken.contains(&key) || !seen.insert(key) {
            continue;
        }
        pool.push(w);
    }
    pool
}

/// The dedup/consistency gate. Walks proposals in order, keeping each only if
/// it is legal and collides with neither the existing lexicon nor an
/// already-kept proposal. Returns `(kept, rejected-with-reason)`.
pub fn dedup(
    phon: &Phonology,
    existing: &[DictionaryEntry],
    proposals: Vec<LexProposal>,
) -> (Vec<LexProposal>, Vec<(LexProposal, RejectReason)>) {
    let mut surfaces: HashSet<String> =
        existing.iter().map(|e| surface_key(phon, e.word.trim())).collect();
    let mut glosses: HashSet<String> = existing
        .iter()
        .map(|e| e.translation.trim().to_lowercase())
        .filter(|g| !g.is_empty())
        .collect();

    let check_phonotactics = !phon.constraints.is_empty();
    let mut kept = Vec::new();
    let mut rejected = Vec::new();

    for p in proposals {
        let form = p.form.trim();
        if form.is_empty() {
            rejected.push((p, RejectReason::Illegal));
            continue;
        }
        let underlying = phon.segment(form);
        if check_phonotactics && !validator::is_legal(phon, &underlying) {
            rejected.push((p, RejectReason::Illegal));
            continue;
        }
        let skey = surface_key(phon, form);
        if surfaces.contains(&skey) {
            rejected.push((p, RejectReason::Homophone));
            continue;
        }
        let gkey = p.gloss.trim().to_lowercase();
        if !gkey.is_empty() && glosses.contains(&gkey) {
            rejected.push((p, RejectReason::DuplicateMeaning));
            continue;
        }
        surfaces.insert(skey);
        if !gkey.is_empty() {
            glosses.insert(gkey);
        }
        kept.push(p);
    }
    (kept, rejected)
}

/// Surface form (after allophony) of a written word — the homophone key.
fn surface_key(phon: &Phonology, word: &str) -> String {
    allophony_eval::surface_form(phon, &phon.segment(word)).join("")
}

/// Cosine similarity of two equal-length vectors; 0 for a zero vector.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// The *semantic* half of the dedup gate: reject a proposal whose gloss is a
/// near-synonym (cosine > `threshold`) of an existing or already-accepted
/// gloss — catching "stone" vs "rock" that the string-level `dedup` misses.
/// `existing_vecs` are the existing entries' gloss embeddings; `kept_vecs`
/// aligns 1:1 with `kept`. Pure; the embedding itself is the caller's job.
pub fn semantic_filter(
    kept: Vec<LexProposal>,
    existing_vecs: &[Vec<f32>],
    kept_vecs: &[Vec<f32>],
    threshold: f32,
) -> (Vec<LexProposal>, Vec<(LexProposal, f32)>) {
    let mut accepted_vecs: Vec<Vec<f32>> = existing_vecs.to_vec();
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for (i, p) in kept.into_iter().enumerate() {
        let v = &kept_vecs[i];
        let max = accepted_vecs.iter().map(|e| cosine(v, e)).fold(0.0f32, f32::max);
        if max > threshold {
            rejected.push((p, max));
        } else {
            accepted_vecs.push(v.clone());
            accepted.push(p);
        }
    }
    (accepted, rejected)
}

/// Extract the AI's proposals from a (possibly fenced / prose-wrapped) JSON
/// reply shaped `{ "entries": [ … ] }`.
pub fn parse_proposals(raw: &str) -> Result<Vec<LexProposal>, String> {
    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(default)]
        entries: Vec<LexProposal>,
    }
    let start = raw.find('{').ok_or("no JSON object in reply")?;
    let end = raw.rfind('}').ok_or("no closing brace in reply")?;
    if end < start {
        return Err("malformed braces".into());
    }
    let w: Wrapper = serde_json::from_str(&raw[start..=end]).map_err(|e| e.to_string())?;
    Ok(w.entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind, PhonotacticConstraint};

    fn ph(ipa: &str, kind: PhonemeKind) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: Some(ipa.into()), kind, sonority: None }
    }

    fn phon() -> Phonology {
        let mut p = Phonology {
            phonemes: vec![
                ph("k", PhonemeKind::Consonant), ph("t", PhonemeKind::Consonant),
                ph("r", PhonemeKind::Consonant), ph("a", PhonemeKind::Vowel),
                ph("i", PhonemeKind::Vowel),
            ],
            ..Default::default()
        };
        p.classes = [
            ("C".to_string(), vec!["k", "t", "r"].into_iter().map(String::from).collect()),
            ("V".to_string(), vec!["a", "i"].into_iter().map(String::from).collect()),
        ]
        .into_iter()
        .collect();
        p.templates = [(
            "root".to_string(),
            vec![serde_hjson::from_str(r#"{ "pattern": "C V (C) V" }"#).unwrap()],
        )]
        .into_iter()
        .collect();
        p.constraints = vec![PhonotacticConstraint::MaxClusterSize(1)];
        p
    }

    fn entry(word: &str, gloss: &str) -> DictionaryEntry {
        DictionaryEntry { word: word.into(), pos: "noun".into(), translation: gloss.into(), ..Default::default() }
    }

    fn prop(form: &str, gloss: &str) -> LexProposal {
        LexProposal {
            form: form.into(),
            gloss: gloss.into(),
            pos: "noun".into(),
            example: String::new(),
            register: String::new(),
            domain: Vec::new(),
        }
    }

    #[test]
    fn rejects_illegal_homophone_and_duplicate_meaning() {
        let p = phon();
        let existing = vec![entry("kara", "stone")];
        let proposals = vec![
            prop("tira", "river"),  // ok
            prop("krta", "gizmo"),  // illegal cluster
            prop("kara", "rock"),   // homophone of existing "kara"
            prop("tika", "stone"),  // duplicate meaning of existing "stone"
            prop("tira", "lake"),   // homophone of kept "tira"
        ];
        let (kept, rejected) = dedup(&p, &existing, proposals);
        assert_eq!(kept, vec![prop("tira", "river")]);
        let reasons: Vec<_> = rejected.iter().map(|(_, r)| *r).collect();
        assert_eq!(
            reasons,
            vec![
                RejectReason::Illegal,
                RejectReason::Homophone,
                RejectReason::DuplicateMeaning,
                RejectReason::Homophone,
            ]
        );
    }

    #[test]
    fn pool_excludes_existing_and_is_distinct() {
        let p = phon();
        let existing = vec![entry("ka", "x")];
        let pool = build_pool(&p, &existing, 10);
        assert!(!pool.is_empty());
        let mut seen = HashSet::new();
        for w in &pool {
            assert!(seen.insert(w.clone()), "pool has a duplicate: {w}");
            assert_ne!(w, "ka", "pool must exclude existing words");
        }
    }

    #[test]
    fn semantic_filter_rejects_near_synonyms() {
        // 3 proposals; the 2nd is a near-synonym of an existing gloss vector,
        // the 3rd is a near-synonym of the 1st kept proposal (intra-batch).
        let existing = vec![vec![1.0, 0.0, 0.0]]; // "rock"
        let kept = vec![prop("a", "water"), prop("b", "stone"), prop("c", "aqua")];
        let kept_vecs = vec![
            vec![0.0, 1.0, 0.0],   // water — distinct
            vec![0.99, 0.01, 0.0], // stone ≈ rock → reject vs existing
            vec![0.0, 0.99, 0.01], // aqua ≈ water → reject vs kept "water"
        ];
        let (acc, rej) = semantic_filter(kept, &existing, &kept_vecs, 0.9);
        assert_eq!(acc, vec![prop("a", "water")]);
        assert_eq!(rej.len(), 2);
        assert!(rej.iter().all(|(_, sim)| *sim > 0.9));
    }

    #[test]
    fn cosine_basics() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }

    #[test]
    fn parse_tolerates_fences_and_prose() {
        let raw = "Here:\n```json\n{ \"entries\": [ { \"form\": \"kara\", \"gloss\": \"stone\", \
                   \"pos\": \"noun\" } ] }\n```\ndone";
        let ps = parse_proposals(raw).unwrap();
        assert_eq!(ps, vec![prop("kara", "stone")]);
    }
}

//! LANG-3 Tier 2 (retrieval) — a translation memory over `(English → conlang)`
//! pairs (RFC Amendment A1).
//!
//! The first, dependency-free cut of the retrieval datastore: a list of
//! author-confirmed (or synthetic-corpus) translations, looked up by the English
//! source. An **exact** normalized match is returned as translation memory; a
//! **near** match (by token overlap) is returned as a candidate to surface. The
//! semantic upgrade — embedding the source with `fastembed` and querying the
//! HNSW `VectorEngine` — slots in behind this same `lookup` in P2; the merge
//! policy that consumes a hit ([`super::apply_memory`]) does not change.
//!
//! Persisted as the `.inkhaven/` sidecar JSON, in the advisory-sidecar pattern
//! (atomic writes), so the prose books are never touched.
//!
//! Pure and deterministic.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One remembered translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pair {
    english: String,
    conlang: String,
}

/// A language's translation memory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranslationMemory {
    pairs: Vec<Pair>,
}

/// The result of looking an English source up in the memory.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryHit {
    /// An exact (normalized) match — an author-confirmed translation.
    Exact { conlang: String },
    /// A near match by token overlap (Jaccard ≥ threshold).
    Fuzzy { conlang: String, score: f32, english: String },
    /// Nothing close enough.
    None,
}

/// A near match must share at least this fraction of its tokens with the query.
const FUZZY_THRESHOLD: f32 = 0.5;

/// Lowercase content tokens, for normalization and overlap scoring.
fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Token-set Jaccard similarity, `0.0..=1.0`.
fn jaccard(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.iter().filter(|t| b.contains(t)).count();
    let union = a.len() + b.len() - inter;
    if union == 0 {
        0.0
    } else {
        inter as f32 / union as f32
    }
}

impl TranslationMemory {
    /// Add (or update) a remembered translation. Re-remembering the same English
    /// replaces the prior target — a correction supersedes.
    pub fn add(&mut self, english: &str, conlang: &str) {
        let key = tokens(english);
        if let Some(p) = self.pairs.iter_mut().find(|p| tokens(&p.english) == key) {
            p.conlang = conlang.to_string();
            p.english = english.to_string();
        } else {
            self.pairs.push(Pair { english: english.to_string(), conlang: conlang.to_string() });
        }
    }

    /// Look an English source up: exact (normalized) first, then the best near
    /// match above the threshold.
    pub fn lookup(&self, english: &str) -> MemoryHit {
        let q = tokens(english);
        if let Some(p) = self.pairs.iter().find(|p| tokens(&p.english) == q) {
            return MemoryHit::Exact { conlang: p.conlang.clone() };
        }
        let mut best: Option<(&Pair, f32)> = None;
        for p in &self.pairs {
            let s = jaccard(&q, &tokens(&p.english));
            if s >= FUZZY_THRESHOLD && best.map(|(_, b)| s > b).unwrap_or(true) {
                best = Some((p, s));
            }
        }
        match best {
            Some((p, score)) => MemoryHit::Fuzzy {
                conlang: p.conlang.clone(),
                score,
                english: p.english.clone(),
            },
            None => MemoryHit::None,
        }
    }

    /// How many translations are remembered.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether the memory is empty.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Every `(english, conlang)` pair, for listing/export.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pairs.iter().map(|p| (p.english.as_str(), p.conlang.as_str()))
    }

    /// The sidecar path for a language's memory.
    pub fn sidecar_path(project_root: &Path, language: &str) -> PathBuf {
        project_root
            .join(".inkhaven")
            .join("translation-memory")
            .join(format!("{}.json", language.to_lowercase()))
    }

    /// Load a language's memory (empty if none on disk).
    pub fn load(project_root: &Path, language: &str) -> std::io::Result<Self> {
        let path = Self::sidecar_path(project_root, language);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// Save a language's memory atomically.
    pub fn save(&self, project_root: &Path, language: &str) -> std::io::Result<()> {
        let path = Self::sidecar_path(project_root, language);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&path, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_is_normalized() {
        let mut m = TranslationMemory::default();
        m.add("The bird sees the stone.", "kira nami pata");
        // Punctuation / case / articles normalize to the same token set.
        assert_eq!(
            m.lookup("the bird sees the stone"),
            MemoryHit::Exact { conlang: "kira nami pata".into() }
        );
    }

    #[test]
    fn re_adding_supersedes() {
        let mut m = TranslationMemory::default();
        m.add("the bird flies", "kira aaa");
        m.add("the bird flies", "kira bbb"); // a correction
        assert_eq!(m.len(), 1);
        assert_eq!(m.lookup("the bird flies"), MemoryHit::Exact { conlang: "kira bbb".into() });
    }

    #[test]
    fn near_match_is_fuzzy() {
        let mut m = TranslationMemory::default();
        m.add("the bird sees the stone", "kira nami pata");
        match m.lookup("the bird sees a stone") {
            MemoryHit::Fuzzy { conlang, score, .. } => {
                assert_eq!(conlang, "kira nami pata");
                assert!(score >= 0.5 && score < 1.0);
            }
            other => panic!("expected fuzzy, got {other:?}"),
        }
    }

    #[test]
    fn unrelated_is_a_miss() {
        let mut m = TranslationMemory::default();
        m.add("the bird sees the stone", "kira nami pata");
        assert_eq!(m.lookup("a dragon burns the tower"), MemoryHit::None);
    }
}

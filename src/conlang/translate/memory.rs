//! LANG-3 Tier 2 (retrieval) — a translation memory over `(English → conlang)`
//! pairs (RFC Amendment A1).
//!
//! The retrieval datastore: a list of author-confirmed (or synthetic-corpus)
//! translations, looked up by the English source. The strategy lives in one
//! place — [`TranslationMemory::best`] — so the merge policy that consumes a hit
//! ([`super::apply_memory`]) never changes as retrieval improves: an **exact**
//! normalized match first, then (when a query embedding is supplied and the
//! pairs carry cached vectors) the best **semantic** cosine match, then a
//! **lexical** token-overlap match. Each pair caches its English source's
//! embedding (computed once by the app layer via the in-tree `fastembed`), so a
//! lookup only embeds the query.
//!
//! Persisted as the `.inkhaven/` sidecar JSON, in the advisory-sidecar pattern
//! (atomic writes), so the prose books are never touched.
//!
//! Pure and deterministic.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One remembered translation. `embedding` is the cached vector of the English
/// source (empty until computed by the app layer, which owns the embedder);
/// caching it means a lookup only embeds the *query*, not the whole memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pair {
    english: String,
    conlang: String,
    #[serde(default)]
    embedding: Vec<f32>,
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

/// A semantic match must reach at least this cosine similarity. Semantic hits
/// only ever surface as *alternatives* (never override the primary), so a
/// generous threshold is low-stakes.
const SEMANTIC_THRESHOLD: f32 = 0.82;

/// Cosine similarity of two vectors, `-1.0..=1.0` (0 if shapes differ / empty).
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

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
    /// replaces the prior target — a correction supersedes — and clears any
    /// cached embedding so it is recomputed.
    pub fn add(&mut self, english: &str, conlang: &str) {
        let key = tokens(english);
        if let Some(p) = self.pairs.iter_mut().find(|p| tokens(&p.english) == key) {
            p.conlang = conlang.to_string();
            p.english = english.to_string();
            p.embedding.clear();
        } else {
            self.pairs.push(Pair {
                english: english.to_string(),
                conlang: conlang.to_string(),
                embedding: Vec::new(),
            });
        }
    }

    /// The English sources whose embedding has not been computed yet — the app
    /// layer embeds these and feeds them back via [`Self::set_embedding`].
    pub fn needs_embeddings(&self) -> Vec<String> {
        self.pairs.iter().filter(|p| p.embedding.is_empty()).map(|p| p.english.clone()).collect()
    }

    /// Cache the embedding of a remembered English source.
    pub fn set_embedding(&mut self, english: &str, embedding: Vec<f32>) {
        let key = tokens(english);
        if let Some(p) = self.pairs.iter_mut().find(|p| tokens(&p.english) == key) {
            p.embedding = embedding;
        }
    }

    /// The retrieval strategy, in one place so the merge policy never changes as
    /// it improves: an **exact** (normalized-token) match first; then, if a query
    /// embedding is given and the memory has cached vectors, the best **semantic**
    /// (cosine) match; then a **lexical** (token-overlap) near match. The exact
    /// path needs no embedding, so a seeded sentence costs nothing.
    pub fn best(&self, english: &str, query_embedding: Option<&[f32]>) -> MemoryHit {
        let q = tokens(english);
        if let Some(p) = self.pairs.iter().find(|p| tokens(&p.english) == q) {
            return MemoryHit::Exact { conlang: p.conlang.clone() };
        }
        // Semantic, when a query vector is supplied and any pair is embedded.
        if let Some(qv) = query_embedding {
            let mut best: Option<(&Pair, f32)> = None;
            for p in &self.pairs {
                if p.embedding.is_empty() {
                    continue;
                }
                let s = cosine(qv, &p.embedding);
                if s >= SEMANTIC_THRESHOLD && best.map(|(_, b)| s > b).unwrap_or(true) {
                    best = Some((p, s));
                }
            }
            if let Some((p, score)) = best {
                return MemoryHit::Fuzzy {
                    conlang: p.conlang.clone(),
                    score,
                    english: p.english.clone(),
                };
            }
        }
        // Lexical fallback.
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
            m.best("the bird sees the stone", None),
            MemoryHit::Exact { conlang: "kira nami pata".into() }
        );
    }

    #[test]
    fn re_adding_supersedes() {
        let mut m = TranslationMemory::default();
        m.add("the bird flies", "kira aaa");
        m.add("the bird flies", "kira bbb"); // a correction
        assert_eq!(m.len(), 1);
        assert_eq!(m.best("the bird flies", None), MemoryHit::Exact { conlang: "kira bbb".into() });
    }

    #[test]
    fn near_match_is_fuzzy() {
        let mut m = TranslationMemory::default();
        m.add("the bird sees the stone", "kira nami pata");
        match m.best("the bird sees a stone", None) {
            MemoryHit::Fuzzy { conlang, score, .. } => {
                assert_eq!(conlang, "kira nami pata");
                assert!(score >= 0.5 && score < 1.0);
            }
            other => panic!("expected fuzzy, got {other:?}"),
        }
    }

    #[test]
    fn semantic_match_uses_cosine_when_lexical_fails() {
        let mut m = TranslationMemory::default();
        m.add("the warrior raises his sword", "AAA");
        m.add("the bird sees the stone", "BBB");
        // Crafted embeddings: the query is near the first pair, far from the second.
        m.set_embedding("the warrior raises his sword", vec![1.0, 0.0, 0.0]);
        m.set_embedding("the bird sees the stone", vec![0.0, 1.0, 0.0]);
        let q = [0.96, 0.1, 0.0];
        // "a soldier lifts a blade" shares no tokens with either — only the
        // semantic path can match it.
        match m.best("a soldier lifts a blade", Some(&q)) {
            MemoryHit::Fuzzy { conlang, score, .. } => {
                assert_eq!(conlang, "AAA");
                assert!(score >= 0.82);
            }
            other => panic!("expected a semantic match, got {other:?}"),
        }
        // Without the query vector it is a miss (no lexical overlap).
        assert_eq!(m.best("a soldier lifts a blade", None), MemoryHit::None);
    }

    #[test]
    fn unrelated_is_a_miss() {
        let mut m = TranslationMemory::default();
        m.add("the bird sees the stone", "kira nami pata");
        assert_eq!(m.best("a dragon burns the tower", None), MemoryHit::None);
    }
}

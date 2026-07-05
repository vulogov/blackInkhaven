//! HAIKU-2 — semantic poem selection via the project's fastembed engine.
//!
//! Called by [`super::emit_with_context`] when the embedding engine is already
//! warm; falls back to [`super::next_for_lang`] (HAIKU-1 rotation) otherwise.
//!
//! The poem embeddings are computed once per process and cached in a `OnceLock`.
//! Only the *middle* line of each poem is embedded — it is the longest line by
//! RFC definition and carries the most semantic content. Cosine similarity picks
//! the poem nearest the writing context; ties break on the rotation counter, so
//! selection stays deterministic.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::HAIKU_TABLE;

/// Cached embeddings: `(iso_lang, poem_slot)` → unit vector. Built on the first
/// semantic call; shared across all subsequent calls.
static POEM_EMBEDDINGS: OnceLock<HashMap<(&'static str, usize), Vec<f32>>> = OnceLock::new();

/// Cosine similarity of two equal-length, L2-normalised vectors (fastembed's
/// default output is normalised, so this is a plain dot product).
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Warm the poem-embedding cache using `embed_fn` (a wrapper over
/// `store.embed_batch`, so this module has no `Store` dependency). No-op once
/// warm. Only the middle line of each poem is embedded.
pub fn warm_cache(embed_fn: impl Fn(&[&str]) -> anyhow::Result<Vec<Vec<f32>>>) -> anyhow::Result<()> {
    if POEM_EMBEDDINGS.get().is_some() {
        return Ok(());
    }
    let mut keys: Vec<(&'static str, usize)> = Vec::new();
    let mut texts: Vec<&str> = Vec::new();
    for h in HAIKU_TABLE.iter() {
        for (slot, poem) in h.poems.iter().enumerate() {
            keys.push((h.lang, slot));
            texts.push(poem[1]); // middle line
        }
    }
    let vecs = embed_fn(&texts)?;
    let map: HashMap<(&'static str, usize), Vec<f32>> = keys.into_iter().zip(vecs).collect();
    let _ = POEM_EMBEDDINGS.set(map); // no-op on race
    Ok(())
}

/// Select the poem for `iso_lang` whose middle-line embedding is most similar to
/// `query_vec`. Ties break on `tiebreak_slot` (the HAIKU-1 rotation slot the
/// caller pre-computed). `None` if the cache is cold (race) — caller falls back.
pub fn select(
    iso_lang: &str,
    query_vec: &[f32],
    tiebreak_slot: usize,
) -> Option<[&'static str; 3]> {
    let cache = POEM_EMBEDDINGS.get()?;
    let lang = if HAIKU_TABLE.iter().any(|h| h.lang == iso_lang) {
        iso_lang
    } else {
        "en" // fallback matches HAIKU-1
    };
    // Highest cosine wins; strict `>` keeps the tiebreak slot on an exact tie.
    let (best_slot, _score) = (0..5)
        .filter_map(|slot| cache.get(&(lang, slot)).map(|v| (slot, cosine(query_vec, v))))
        .fold((tiebreak_slot % 5, f32::NEG_INFINITY), |(bs, bsc), (s, sc)| {
            if sc > bsc {
                (s, sc)
            } else {
                (bs, bsc)
            }
        });
    let table = HAIKU_TABLE.iter().find(|h| h.lang == lang)?;
    Some(table.poems[best_slot % 5])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic stand-in for the real model: encodes text length in the
    /// first component, then L2-normalises. Enough to exercise the selection.
    fn dummy_embed(texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut v = vec![0f32; 384];
                v[0] = t.len() as f32 / 100.0;
                v[1] = 1.0;
                let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
                v.iter_mut().for_each(|x| *x /= norm);
                v
            })
            .collect())
    }

    #[test]
    fn warm_then_select_returns_a_valid_poem() {
        warm_cache(dummy_embed).unwrap();
        let query = dummy_embed(&["a test query about frost"]).unwrap().remove(0);
        let lines = select("en", &query, 0).expect("warm cache selects");
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| !l.is_empty()));
    }

    #[test]
    fn unknown_lang_falls_back_to_english() {
        warm_cache(dummy_embed).unwrap();
        let query = dummy_embed(&["query"]).unwrap().remove(0);
        // `zh` is unsupported → English poems, no panic.
        let lines = select("zh", &query, 2).expect("falls back to en");
        assert_eq!(lines.len(), 3);
    }
}

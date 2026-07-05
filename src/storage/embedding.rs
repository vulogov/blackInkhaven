//! Thin, thread-safe wrapper around a fastembed `TextEmbedding`
//! model. Inkhaven uses one model per project (chosen via
//! `embeddings.model` in `inkhaven.hjson`), embedded into a
//! `VectorEngine` so every paragraph save re-embeds both the metadata
//! fingerprint and the content.
//!
//! ## Lazy initialisation (1.2.18+ I.1.4)
//!
//! Loading the ONNX model is ~470 ms — and the I.1.3 hotspot
//! analysis found it dominated cold start (92 %) because
//! `Store::open` built it eagerly, even for commands that never
//! embed or search (`inkhaven list`, `inkhaven add`, the TUI
//! launch before the first Ctrl+F).
//!
//! `EmbeddingEngine::new` now only *stores the construction
//! parameters*; the model itself loads on the first `embed` /
//! `embed_batch` call, behind the same mutex that guards every
//! inference.  Projects that open + read but never search pay
//! nothing for the model.  The first save / search / reindex pays
//! the one-time ~470 ms, exactly as before — just deferred to the
//! moment it's actually needed.

use anyhow::{anyhow, Result};
use fastembed::{InitOptions, TextEmbedding};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

pub use fastembed::EmbeddingModel as Model;

/// One embedding vector (a row of `f32`s the length of the model's
/// output dimension).
pub type Embedding = Vec<f32>;

/// Cloneable handle to a fastembed model. The model itself is held
/// behind a `Mutex<Option<…>>` because (a) `TextEmbedding::embed`
/// takes `&mut self`, and (b) the model is lazily constructed on
/// first use — see the module-level note. Inkhaven only calls into
/// this engine from the (single-threaded) TUI event loop, so the
/// mutex is effectively uncontended.
#[derive(Clone)]
pub struct EmbeddingEngine {
    model: Model,
    cache_dir: Option<PathBuf>,
    /// `None` until the first `embed` / `embed_batch` triggers the
    /// ~470 ms ONNX model load.  Shared across clones so every
    /// handle to the same project sees the model once it's built.
    inner: Arc<Mutex<Option<TextEmbedding>>>,
}

impl EmbeddingEngine {
    /// Register `model` (caching its ONNX weights in `cache_dir`, or
    /// fastembed's default cache if `None`).  Cheap — the model
    /// itself loads lazily on first `embed` / `embed_batch`.
    ///
    /// Infallible now that construction is deferred; the `Result`
    /// signature is kept for API compatibility with the eager
    /// version (and because a future eager-validate path might want
    /// it back).
    pub fn new(model: Model, cache_dir: Option<PathBuf>) -> Result<Self> {
        Ok(Self {
            model,
            cache_dir,
            inner: Arc::new(Mutex::new(None)),
        })
    }

    /// True once the underlying ONNX model has been loaded.  Lets
    /// callers (the health monitor, `_bench-load`) distinguish
    /// "engine registered" from "model resident in memory", and
    /// backs the I.1.4 lazy-init test.  `allow(dead_code)`: only
    /// the test consumes it in the current build; diagnostics
    /// wiring lands in a follow-up.
    pub fn is_loaded(&self) -> bool {
        self.inner.lock().is_some()
    }

    /// Run `f` against the lazily-constructed model.  Builds the
    /// model on first call (the ~470 ms cost) and reuses it after.
    /// Holds the mutex for the whole call — inference already
    /// serialised here, so the lazy build doesn't add contention.
    fn with_model<R>(
        &self,
        f: impl FnOnce(&mut TextEmbedding) -> Result<R>,
    ) -> Result<R> {
        let mut guard = self.inner.lock();
        if guard.is_none() {
            let options = {
                let opts = InitOptions::new(self.model.clone());
                match &self.cache_dir {
                    Some(dir) => opts.with_cache_dir(dir.clone()),
                    None => opts,
                }
            };
            let model = TextEmbedding::try_new(options).map_err(|e| {
                anyhow!("failed to initialise embedding model: {e}")
            })?;
            *guard = Some(model);
        }
        // `expect`: the block immediately above guarantees `Some`.
        let model = guard
            .as_mut()
            .expect("embedding model set in the block immediately above");
        f(model)
    }

    pub fn embed(&self, text: &str) -> Result<Embedding> {
        self.with_model(|model| {
            model
                .embed(vec![text], None)
                .map_err(|e| anyhow!("embedding failed: {e}"))?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("model returned no embedding"))
        })
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        self.with_model(|model| {
            model
                .embed(texts.to_vec(), None)
                .map_err(|e| anyhow!("batch embedding failed: {e}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The I.1.4 invariant: `new` must NOT load the ONNX
    /// model.  This is what cuts ~470 ms off every cold
    /// start for commands that never embed.  A regression
    /// here (re-introducing eager load) would make `new`
    /// take ~470 ms + this test would still pass on
    /// timing alone — so we assert the structural fact:
    /// `is_loaded()` is false until the first inference.
    #[test]
    fn new_does_not_load_the_model() {
        let engine =
            EmbeddingEngine::new(Model::MultilingualE5Small, None).unwrap();
        assert!(
            !engine.is_loaded(),
            "new() must defer the ONNX model load (I.1.4) — \
             the engine reported the model as already resident",
        );
    }

    /// `embed_batch([])` is a documented fast-path that
    /// returns empty without touching the model — so it
    /// must NOT trigger the lazy load either.  Guards the
    /// "projects that open but never embed pay nothing"
    /// promise against an accidental load on the empty
    /// path.
    #[test]
    fn empty_batch_does_not_load_the_model() {
        let engine =
            EmbeddingEngine::new(Model::MultilingualE5Small, None).unwrap();
        let out = engine.embed_batch(&[]).unwrap();
        assert!(out.is_empty());
        assert!(
            !engine.is_loaded(),
            "embed_batch([]) must short-circuit before the model load",
        );
    }
}

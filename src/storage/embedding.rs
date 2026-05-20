//! Thin, thread-safe wrapper around a fastembed `TextEmbedding`
//! model. Inkhaven uses one model per project (chosen via
//! `embeddings.model` in `inkhaven.hjson`), embedded into a
//! `VectorEngine` so every paragraph save re-embeds both the metadata
//! fingerprint and the content.

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
/// behind a `Mutex` because `TextEmbedding::embed` takes `&mut self`;
/// inkhaven only calls into this engine from the (single-threaded) TUI
/// event loop, so the mutex is effectively uncontended.
#[derive(Clone)]
pub struct EmbeddingEngine {
    inner: Arc<Mutex<TextEmbedding>>,
}

impl EmbeddingEngine {
    /// Load `model`, caching its ONNX weights in `cache_dir` (or
    /// fastembed's default cache if `None`).
    pub fn new(model: Model, cache_dir: Option<PathBuf>) -> Result<Self> {
        let options = {
            let opts = InitOptions::new(model);
            match cache_dir {
                Some(dir) => opts.with_cache_dir(dir),
                None => opts,
            }
        };

        let model = TextEmbedding::try_new(options)
            .map_err(|e| anyhow!("failed to initialise embedding model: {e}"))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(model)),
        })
    }

    pub fn embed(&self, text: &str) -> Result<Embedding> {
        self.inner
            .lock()
            .embed(vec![text], None)
            .map_err(|e| anyhow!("embedding failed: {e}"))?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("model returned no embedding"))
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        self.inner
            .lock()
            .embed(texts.to_vec(), None)
            .map_err(|e| anyhow!("batch embedding failed: {e}"))
    }
}

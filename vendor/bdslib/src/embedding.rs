use crate::common::error::{err_msg, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

pub use fastembed::EmbeddingModel as Model;

/// A single embedding vector produced by the model.
pub type Embedding = Vec<f32>;

/// Thread-safe wrapper around a fastembed [`TextEmbedding`] model.
///
/// The underlying model requires exclusive access per inference call (`&mut self`),
/// so it is held behind an `Arc<Mutex<_>>`. The engine can be cloned cheaply
/// and shared across threads.
#[derive(Clone)]
pub struct EmbeddingEngine {
    inner: Arc<Mutex<TextEmbedding>>,
}

impl EmbeddingEngine {
    /// Load `model`, caching its files in `cache_dir`.
    ///
    /// Pass `None` for `cache_dir` to use fastembed's default
    /// (`$HOME/.cache/huggingface/hub` or the `HF_HOME` env var).
    /// The first call for a given model downloads its ONNX weights;
    /// subsequent calls load from cache.
    ///
    /// ```text
    /// let engine = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
    /// let engine = EmbeddingEngine::new(Model::BGESmallENV15, Some("/data/models".into()))?;
    /// ```
    pub fn new(model: EmbeddingModel, cache_dir: Option<PathBuf>) -> Result<Self> {
        let options = {
            let opts = InitOptions::new(model);
            match cache_dir {
                Some(dir) => opts.with_cache_dir(dir),
                None => opts,
            }
        };

        let model = TextEmbedding::try_new(options)
            .map_err(|e| err_msg(format!("Failed to initialise embedding model: {e}")))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(model)),
        })
    }

    /// Embed a single string and return its vector.
    pub fn embed(&self, text: &str) -> Result<Embedding> {
        self.inner
            .lock()
            .embed(vec![text], None)
            .map_err(|e| err_msg(format!("Embedding failed: {e}")))?
            .into_iter()
            .next()
            .ok_or_else(|| err_msg("Model returned no embedding"))
    }

    /// Embed multiple strings in a single ONNX inference pass.
    ///
    /// Significantly faster than calling [`embed`] N times because the
    /// underlying model processes the whole batch in one matrix operation.
    /// Returns one vector per input in the same order. Returns an empty `Vec`
    /// when `texts` is empty.
    ///
    /// [`embed`]: EmbeddingEngine::embed
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        self.inner
            .lock()
            .embed(texts.to_vec(), None)
            .map_err(|e| err_msg(format!("Batch embedding failed: {e}")))
    }

    /// Compute the cosine similarity between two strings.
    ///
    /// The two embeddings are generated concurrently in Rayon worker threads.
    /// Because a single model instance serialises inference behind a mutex,
    /// the computations run back-to-back in the background rather than truly
    /// in parallel; the calling thread blocks only until both are complete.
    ///
    /// Returns a value in `[-1.0, 1.0]`: 1.0 means identical direction,
    /// 0.0 means orthogonal, -1.0 means opposite.
    pub fn compare_texts(&self, a: &str, b: &str) -> Result<f32> {
        let engine_a = self.inner.clone();
        let engine_b = self.inner.clone();
        let text_a = a.to_owned();
        let text_b = b.to_owned();

        let (res_a, res_b) = rayon::join(
            move || -> Result<Embedding> {
                engine_a
                    .lock()
                    .embed(vec![text_a.as_str()], None)
                    .map_err(|e| err_msg(format!("Embedding A failed: {e}")))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| err_msg("No embedding returned for A"))
            },
            move || -> Result<Embedding> {
                engine_b
                    .lock()
                    .embed(vec![text_b.as_str()], None)
                    .map_err(|e| err_msg(format!("Embedding B failed: {e}")))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| err_msg("No embedding returned for B"))
            },
        );

        Self::compare_embeddings(&res_a?, &res_b?)
    }

    /// Compute cosine similarity between two pre-computed embeddings.
    ///
    /// Returns `Err` if the vectors have different dimensions or if either
    /// is a zero vector (undefined cosine similarity).
    pub fn compare_embeddings(a: &[f32], b: &[f32]) -> Result<f32> {
        if a.len() != b.len() {
            return Err(err_msg(format!(
                "Embedding dimension mismatch: {} vs {}",
                a.len(),
                b.len()
            )));
        }
        if a.is_empty() {
            return Err(err_msg("Cannot compare empty embedding vectors"));
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return Err(err_msg("Cannot compare zero-length embedding vectors"));
        }

        Ok(dot / (norm_a * norm_b))
    }
}

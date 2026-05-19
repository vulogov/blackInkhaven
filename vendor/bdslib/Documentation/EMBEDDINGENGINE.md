# EmbeddingEngine API

`EmbeddingEngine` provides a simplified interface for generating text embeddings and computing semantic similarity via [fastembed](https://github.com/Anush008/fastembed-rs). It wraps a fastembed [`TextEmbedding`](https://docs.rs/fastembed) model behind an `Arc<Mutex<_>>`, making instances cheaply cloneable and safe to share across threads.

All methods return `bdslib::common::error::Result<T>` — an alias for `Result<T, easy_error::Error>` defined in the shared [`common::error`](COMMON.md) module.

## Construction

```rust
EmbeddingEngine::new(model: EmbeddingModel, cache_dir: Option<PathBuf>) -> Result<EmbeddingEngine>
```

Loads `model`, downloading its ONNX weights on the first call and reading from cache on subsequent calls.

| Parameter | Value | Behaviour |
|---|---|---|
| `model` | Any `EmbeddingModel` variant | The model to load (e.g. `Model::AllMiniLML6V2`) |
| `cache_dir` | `Some(path)` | Download / cache model files to this directory |
| `cache_dir` | `None` | Use fastembed's default (`$HF_HOME` or `~/.cache/huggingface/hub`) |

Returns `Err` if the model cannot be downloaded or initialised.

```rust
use bdslib::{EmbeddingEngine, embedding::Model};

// Default cache location
let engine = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;

// Custom cache directory
let engine = EmbeddingEngine::new(Model::BGESmallENV15, Some("/data/models".into()))?;
```

`EmbeddingEngine` is `Clone`. Cloning is cheap — both instances share the same underlying model via `Arc`.

---

## Methods

### `embed`

```rust
fn embed(&self, text: &str) -> Result<Embedding>
```

Encodes `text` into a dense float vector. The vector dimension is fixed by the model (e.g. 384 for `AllMiniLML6V2`).

```rust
let vector: Vec<f32> = engine.embed("the quick brown fox")?;
println!("dimension: {}", vector.len()); // 384
```

---

### `compare_texts`

```rust
fn compare_texts(&self, a: &str, b: &str) -> Result<f32>
```

Embeds both strings and returns their cosine similarity. The two inference calls are submitted to Rayon worker threads via `rayon::join`; because the underlying model serialises inference behind a mutex, they run back-to-back in the background while the calling thread blocks until both are done.

Returns a value in `[-1.0, 1.0]`:

| Score | Meaning |
|---|---|
| `1.0` | Identical direction — semantically equivalent |
| `0.0` | Orthogonal — no semantic relationship |
| `-1.0` | Opposite direction |

```rust
let sim = engine.compare_texts(
    "the cat sat on the mat",
    "a cat rested on a rug",
)?;
println!("similarity: {sim:.3}"); // e.g. 0.872
```

---

### `compare_embeddings`

```rust
fn compare_embeddings(a: &[f32], b: &[f32]) -> Result<f32>
```

Static method. Computes cosine similarity between two pre-computed embedding vectors without invoking any model. Useful when embeddings are stored and compared offline.

For standalone vector math without the `EmbeddingEngine` struct, the same operation is available as `bdslib::common::math::cosine_similarity`.

Returns `Err` if:
- `a` and `b` have different lengths
- Either vector is empty
- Either vector is a zero vector (cosine similarity is undefined)

```rust
let e1 = engine.embed("first sentence")?;
let e2 = engine.embed("second sentence")?;
let sim = EmbeddingEngine::compare_embeddings(&e1, &e2)?;
```

---

## Available models

`Model` is a re-export of `fastembed::EmbeddingModel`. Common choices:

| Variant | Dimensions | Notes |
|---|---|---|
| `Model::AllMiniLML6V2` | 384 | Small, fast; good general-purpose baseline (**default**) |
| `Model::AllMiniLML6V2Q` | 384 | Quantized variant of the above (~6 MB, slightly lower fidelity) |
| `Model::BGESmallENV15` | 384 | Strong retrieval performance |
| `Model::BGEBaseENV15` | 768 | Higher quality, larger download |
| `Model::BGELargeENV15` | 1024 | Highest quality, largest download |
| `Model::MultilingualE5Small` | 384 | Multilingual; useful for non-English corpora |
| `Model::NomicEmbedTextV15` | 768 | Longer context window |
| `Model::JinaEmbeddingsV2BaseEN` | 768 | 8K-token context |

See `fastembed::EmbeddingModel` for the full list (~40 variants). All variants
are downloaded on first use and cached automatically.

---

## Configuring the model via `bds.hjson`

When `bdsnode` (or `bdscli`) initialises the global database via
`ShardsManager::new(config_path)`, the embedding model is read from two
optional config keys:

```hjson
{
  // Variant name from `fastembed::EmbeddingModel`, matching Rust's Debug
  // form (case-insensitive).  Defaults to "AllMiniLML6V2" when absent.
  embedding_model: "BGESmallENV15"

  // Optional override for the fastembed model cache directory.
  // Defaults to ~/.cache/huggingface/hub or $HF_HOME.
  embedding_cache_dir: "/var/lib/bdslib/models"
}
```

The resolved name is reported back in `v2/status` as `embedding_model`
and surfaced on the bdsweb Dashboard so operators can confirm what's
loaded without re-parsing the config file.

### Dimension lock-in

The HNSW vector index dimension is **fixed at first vector insert**.
Switching `embedding_model` on an existing dbpath will break vector
search because the existing HNSW indexes don't know how to store the new
dimension. To switch models, rebuild the dbpath:

```bash
bdsnode --new --config bds.hjson
```

This is by design — embedding migration would require re-embedding the
entire corpus, which is a separate operation outside the scope of the
config knob. Pick the model when you set up a deployment; treat changes
as a fresh install.

`with_embedding` (the test/library entry point that takes a pre-loaded
`EmbeddingEngine`) bypasses this config layer, so test fixtures can still
mix-and-match models per-test.

---

## Thread safety

`EmbeddingEngine` is `Send + Sync`. Wrap in `Arc` to share across threads, or use `Clone` directly — both result in a shared underlying model:

```rust
use std::sync::Arc;

let engine = Arc::new(EmbeddingEngine::new(Model::AllMiniLML6V2, None)?);

let e = engine.clone();
std::thread::spawn(move || {
    let emb = e.embed("from another thread").unwrap();
    println!("dimension: {}", emb.len());
});
```

Concurrent `embed` calls are serialised by the internal mutex. If high throughput matters, consider running multiple independent model instances rather than sharing one.

---

## Error handling

All methods return `bdslib::common::error::Result<T>`. Use `?` to propagate or inspect the message directly:

```rust
match engine.embed("hello") {
    Ok(v)  => println!("dim: {}", v.len()),
    Err(e) => eprintln!("error: {e}"),
}
```

See [`common::error`](COMMON.md) for the shared error type.

---

## Type alias

```rust
pub type Embedding = Vec<f32>;
```

`Embedding` is the concrete return type of `embed` — a heap-allocated vector of 32-bit floats.

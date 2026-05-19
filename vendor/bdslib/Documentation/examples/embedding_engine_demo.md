# embedding_engine_demo.rs

**File:** `examples/embedding_engine_demo.rs`

Demonstrates the `EmbeddingEngine`: generating vector embeddings from text, comparing embeddings with cosine similarity, and finding nearest neighbours.

## What it demonstrates

| Operation | Description |
|---|---|
| `EmbeddingEngine::new()` | Load the default embedding model (AllMiniLML6V2, 384 dimensions) |
| `embed(text)` | Generate a `Vec<f32>` embedding for a text string |
| `compare_texts(a, b)` | Compute cosine similarity between two text strings |
| `compare_embeddings(a, b)` | Compute cosine similarity between two pre-computed vectors |
| Clone sharing | Cloned engines share the loaded model — no double-loading |

## Sections in the demo

1. **Single embed** — embed one sentence, print dimension count
2. **Text comparison** — compare semantically related vs. unrelated pairs
3. **Nearest-neighbour** — embed a query, compare against a small corpus, find the highest-scoring match
4. **Clone sharing** — verify that two clones produce identical embeddings for the same input

## Key concepts

**AllMiniLML6V2** — the default model produces 384-dimensional float vectors. It is loaded from disk once and shared via `Arc` across clones.

**Cosine similarity** — the comparison functions return a value in `[-1.0, 1.0]`. Values near 1.0 indicate semantic similarity; values near 0.0 indicate orthogonality; negative values indicate semantic opposition.

**Nearest-neighbour** — manually comparing a query vector against a list of stored embeddings using `compare_embeddings` is the foundation of the vector search that `VectorEngine` wraps.

## Example output

```
embedding dimension: 384
"database" vs "storage": 0.87
"database" vs "cooking":  0.12
nearest: "DuckDB is an analytical database"
```

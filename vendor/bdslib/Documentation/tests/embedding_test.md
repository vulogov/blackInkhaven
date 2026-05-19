# embedding_test.rs

**File:** `tests/embedding_test.rs`  
**Module:** `bdslib::EmbeddingEngine` — vector embedding and cosine similarity

Tests the `EmbeddingEngine` for correctness of cosine similarity math, model output dimensions, semantic consistency, and thread safety.

## `compare_embeddings` (pure math, no model)

| Test | Description |
|---|---|
| `test_compare_identical_vectors` | Same vector → similarity ≈ 1.0 |
| `test_compare_orthogonal_vectors` | Perpendicular vectors → similarity ≈ 0.0 |
| `test_compare_opposite_vectors` | Antiparallel vectors → similarity ≈ −1.0 |
| `test_compare_embeddings_is_symmetric` | `compare(a,b) == compare(b,a)` |
| `test_compare_embeddings_result_in_range` | Result is always in `[-1.0, 1.0]` |
| `test_compare_embeddings_dimension_mismatch_errors` | Different-length vectors return `Err` |
| `test_compare_embeddings_empty_vectors_error` | Empty vectors return `Err` |
| `test_compare_embeddings_zero_vector_error` | Zero-norm vector returns `Err` |

## `embed` (model: AllMiniLML6V2)

| Test | Description |
|---|---|
| `test_embed_returns_nonempty_vector` | Embedding is never empty |
| `test_embed_dimension_is_384` | AllMiniLML6V2 produces 384-dimensional vectors |
| `test_embed_consistent_dimension_across_calls` | All embeddings from the same model share dimension |
| `test_embed_same_text_gives_similar_embedding` | Identical text → similarity > 0.999 |
| `test_embed_different_texts_differ` | Unrelated texts → similarity < 0.95 |

## `compare_texts`

| Test | Description |
|---|---|
| `test_compare_texts_result_in_range` | Similarity in `[-1.0, 1.0]` |
| `test_compare_texts_same_text_is_near_one` | Same text scores ≈ 1.0 |
| `test_compare_texts_semantic_similarity` | Semantically related pairs score higher than unrelated |
| `test_compare_texts_is_symmetric` | `compare_texts(a,b) == compare_texts(b,a)` |
| `test_compare_texts_matches_manual_pipeline` | `compare_texts(a,b)` equals `compare_embeddings(embed(a), embed(b))` |

## Concurrency and cloning

| Test | Description |
|---|---|
| `test_concurrent_embed_returns_consistent_results` | 8 threads embedding same text produce identical vectors |
| `test_engine_is_clone` | Cloned engines share model state; produce identical results |

## Notes

- Tests requiring the model (`embed`, `compare_texts`, concurrency) load AllMiniLML6V2 from disk. They are slower than pure-math tests.
- The `compare_embeddings` tests validate the math layer independently of the model, enabling fast unit testing of the similarity computation.

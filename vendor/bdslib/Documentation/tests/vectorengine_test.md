# vectorengine_test.rs

**File:** `tests/vectorengine_test.rs`  
**Module:** `bdslib::VectorEngine` — HNSW vector storage and similarity search

Tests raw vector storage, document storage, similarity search, reranking, JSON fingerprinting, and concurrent access.

## Construction and clone

| Test | Description |
|---|---|
| `test_new_creates_store` | Creates a new vector store |
| `test_new_file_backed_survives_reopen` | Data persists across close and reopen |
| `test_clone_shares_state` | Clones see the same HNSW index |

## `store_vector`

| Test | Description |
|---|---|
| `test_store_vector_basic` | Store and retrieve a vector |
| `test_store_vector_with_metadata` | Metadata fields survive the roundtrip |
| `test_store_vector_upsert_replaces_existing` | Same ID replaces vector and metadata |
| `test_store_multiple_vectors` | Multiple vectors stored and searchable |

## `store_document` (requires `EmbeddingEngine`)

| Test | Description |
|---|---|
| `test_store_document_without_embedding_engine_is_error` | Returns `Err` if no `EmbeddingEngine` attached |

## `search` and reranking

| Test | Description |
|---|---|
| `test_search_returns_correct_limit` | Results capped at requested limit |
| `test_search_nearest_is_most_similar` | Most similar vector ranks first |
| `test_search_result_has_score_and_metadata` | Results include `_score` and metadata fields |
| `test_search_reranked_identity_same_as_search` | `IdentityReranker` preserves plain search order |
| `test_search_reranked_mmr_respects_limit` | MMR reranking respects the limit |
| `test_search_reranked_score_reranker_applies_custom_scoring` | Custom scoring functions are applied |
| `test_search_reranked_candidate_pool_clamped_to_limit` | `candidate_pool < limit` is handled safely |

## `sync`

| Test | Description |
|---|---|
| `test_sync_does_not_error_on_empty_store` | Syncing empty store succeeds |
| `test_sync_does_not_error_after_inserts` | Syncing after inserts succeeds |

## Concurrency

| Test | Description |
|---|---|
| `test_concurrent_store_and_search` | 8 threads storing vectors concurrently; all searches succeed |

## `json_fingerprint`

| Test | Verifies |
|---|---|
| `test_fingerprint_string_value` | String returned as-is |
| `test_fingerprint_number_value` | Numbers stringified |
| `test_fingerprint_bool_value` | Booleans stringified |
| `test_fingerprint_null_is_empty` | `null` → empty string |
| `test_fingerprint_flat_object_includes_field_names` | `"field: value"` format |
| `test_fingerprint_different_field_names_produce_different_fingerprints` | Same value, different keys → different fingerprints |
| `test_fingerprint_nested_object_uses_dot_path` | `"meta.author: value"` format |
| `test_fingerprint_deeply_nested_object` | Deep paths work |
| `test_fingerprint_array_uses_index_notation` | `"tags[0]: value"` format |
| `test_fingerprint_array_of_objects` | `"items[0].name: value"` format |
| `test_fingerprint_top_level_array` | `"[0]: value"` format |
| `test_fingerprint_skips_null_fields` | `null` object fields are omitted |
| `test_fingerprint_boolean_field` | `"active: true"` / `"deleted: false"` |
| `test_fingerprint_empty_object_is_empty` | `{}` → `""` |
| `test_fingerprint_empty_array_is_empty` | `[]` → `""` |
| `test_fingerprint_mixed_document` | Complex document with all JSON types |

## `search_json` and `search_json_reranked` (require `EmbeddingEngine`)

| Test | Description |
|---|---|
| `test_search_json_without_embedding_engine_is_error` | Returns `Err` if no model attached |
| `test_search_json_reranked_without_embedding_engine_is_error` | Reranked version also requires model |

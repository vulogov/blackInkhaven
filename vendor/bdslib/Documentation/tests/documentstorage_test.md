# documentstorage_test.rs

**File:** `tests/documentstorage_test.rs`  
**Module:** `bdslib::DocumentStorage` — unified document store (JSON metadata + blob content + HNSW vector index)

Tests the full lifecycle of `DocumentStorage`: construction, CRUD operations on metadata and binary content, vector indexing via `add_document_with_vectors` and the `store_*_vector` helpers, unified similarity search, clone sharing semantics, persistence across reopens, and the `results_to_strings` / `search_document_strings` convenience layer.

## Construction and helpers

| Test | Description |
|---|---|
| `test_new_creates_root_directory` | `DocumentStorage::new()` creates the given root path when it does not yet exist |
| `test_new_creates_directory_layout` | After construction, `metadata.db`, `blobs.db`, and `vectors/` subdirectory are all present under the root |
| `test_new_accepts_existing_directory` | Opening the same root path a second time succeeds without error |
| `test_clone_shares_stores` | A clone of a store immediately sees metadata and blob content written through the original handle |

## `add_document` — basic CRUD

| Test | Description |
|---|---|
| `test_add_document_returns_non_nil_uuid` | `add_document()` returns a non-nil UUID |
| `test_add_document_metadata_roundtrip` | JSON metadata stored by `add_document` is retrieved identically via `get_metadata` |
| `test_add_document_content_roundtrip` | Raw byte content stored by `add_document` is retrieved identically via `get_content` |
| `test_add_document_empty_content` | An empty byte slice is stored and retrieved without error |
| `test_add_document_binary_content` | All byte values 0–255 survive the roundtrip intact |
| `test_add_document_nested_metadata` | Deeply nested JSON (objects, arrays, booleans) survives storage and retrieval with correct structure |
| `test_add_document_returns_unique_uuids` | Two successive adds return distinct UUIDs |
| `test_add_document_uuids_are_time_ordered` | Later adds produce larger UUIDv7 values, confirming monotonic ordering |

## `add_document` without embedding

| Test | Description |
|---|---|
| `test_add_document_without_embedding_not_in_vector_search` | Documents added via `add_document()` on a store with no `EmbeddingEngine` produce no vector entry; `search_document` returns an empty result set |

## Clone sharing (metadata/blob AND vector index)

| Test | Description |
|---|---|
| `test_clone_shares_vector_index` | A document added with vectors through the original handle is immediately findable via `search_document` on the clone, confirming the HNSW index is shared by reference |

## `add_document_with_vectors`

| Test | Description |
|---|---|
| `test_add_with_vectors_metadata_roundtrip` | After `add_document_with_vectors`, both `get_metadata` and `get_content` return the values that were supplied |
| `test_add_with_vectors_searchable_via_meta` | A document whose meta vector is close to the query vector appears in `search_document` results |
| `test_add_with_vectors_searchable_via_content` | A document whose content vector is close to the query vector appears in `search_document` results |
| `test_search_document_nearest_first` | When two documents are inserted with different meta vector distances from a query, the closer document appears first in results |
| `test_search_document_returns_metadata_and_content` | Each result object contains `metadata` (with all stored fields), `document` (decoded content string), and `score` |
| `test_search_document_finds_via_both_meta_and_content` | Two documents — one with meta close to the query, one with content close — both appear in a single `search_document` call, confirming unified search across both indexes |

## `get_*` (non-existent)

| Test | Description |
|---|---|
| `test_get_metadata_nonexistent_returns_none` | `get_metadata` returns `Ok(None)` for an unknown UUID |
| `test_get_content_nonexistent_returns_none` | `get_content` returns `Ok(None)` for an unknown UUID |

## `update_metadata` / `update_content`

| Test | Description |
|---|---|
| `test_update_metadata_changes_value` | After `update_metadata`, `get_metadata` reflects the new JSON value |
| `test_update_metadata_nonexistent_is_ok` | Calling `update_metadata` on an unknown UUID returns `Ok(())` without error |
| `test_update_content_changes_value` | After `update_content`, `get_content` returns the new byte payload |
| `test_update_content_nonexistent_is_ok` | Calling `update_content` on an unknown UUID returns `Ok(())` without error |

## `delete_document`

| Test | Description |
|---|---|
| `test_delete_removes_metadata` | After `delete_document`, `get_metadata` returns `None` for the deleted UUID |
| `test_delete_removes_content` | After `delete_document`, `get_content` returns `None` for the deleted UUID |
| `test_delete_removes_from_vector_index` | A document inserted with vectors is found by `search_document` before deletion and absent from results after; a second document at a different vector position is unaffected |
| `test_delete_nonexistent_is_ok` | Deleting an unknown UUID returns `Ok(())` |
| `test_delete_does_not_affect_other_documents` | Deleting document A leaves document B's metadata and content fully accessible |

## `store_metadata_vector` / `store_content_vector` helpers

| Test | Description |
|---|---|
| `test_store_metadata_vector_makes_document_searchable` | Calling `store_metadata_vector` on a document added without vectors makes it discoverable via `search_document` against the indexed vector |
| `test_store_content_vector_makes_document_searchable` | Calling `store_content_vector` on a document added without vectors makes it discoverable via `search_document` against the indexed vector |

## `search_document` — core search behaviour

| Test | Description |
|---|---|
| `test_search_document_returns_empty_on_empty_store` | `search_document` on a freshly created store with no entries returns an empty `Vec` |
| `test_search_document_respects_limit` | With 5 documents indexed and a limit of 2, the result count does not exceed 2 |
| `test_search_document_result_has_score` | A self-query against an indexed meta vector yields a score greater than 0.9 (cosine similarity ≈ 1.0) |
| `test_search_document_result_has_id_field` | Each result contains an `id` field whose string value matches the UUID returned by `add_document_with_vectors` |
| `test_search_document_json_without_embedding_returns_err` | `search_document_json` returns an error whose message contains "EmbeddingEngine" when no embedding engine is configured |
| `test_search_document_text_without_embedding_returns_err` | `search_document_text` returns an error whose message contains "EmbeddingEngine" when no embedding engine is configured |

## `sync`

| Test | Description |
|---|---|
| `test_sync_empty_store_is_ok` | `sync()` on an empty store completes without error |
| `test_sync_after_adds_is_ok` | `sync()` after adding documents with vectors completes without error |

## Persistence across reopens

| Test | Description |
|---|---|
| `test_metadata_survives_reopen` | JSON metadata written in one store handle is readable after the handle is dropped and the path is reopened as a new `DocumentStorage` |
| `test_content_survives_reopen` | Byte content written in one store handle is readable after the handle is dropped and the path is reopened |
| `test_vector_index_survives_reopen` | A document added with vectors and followed by `sync()` is still findable via `search_document` after the store is closed and reopened |

## `results_to_strings` / `search_document_strings` / `search_document_*_strings`

| Test | Description |
|---|---|
| `test_results_to_strings_empty` | `results_to_strings(&[])` returns an empty `Vec` |
| `test_results_to_strings_contains_field_values` | The fingerprint string for a result contains both the metadata field value ("fingerprint test") and the document content ("hello world") |
| `test_results_to_strings_includes_score` | The fingerprint string for a result contains the literal text "score" |
| `test_search_document_strings_returns_same_count_as_search_document` | `search_document_strings` returns the same number of entries as the equivalent `search_document` call |
| `test_search_document_strings_returns_strings` | The returned strings contain the metadata and content values from the stored document |
| `test_search_document_json_strings_without_embedding_returns_err` | `search_document_json_strings` returns an error when no embedding engine is configured |
| `test_search_document_text_strings_without_embedding_returns_err` | `search_document_text_strings` returns an error when no embedding engine is configured |

## `add_document_from_file` — RAG ingestion

Helper used by these tests: `write_tmp_file(content)` — writes a string to a temp file and returns the `TempDir` guard plus the file path.

### Construction and errors

| Test | Description |
|---|---|
| `test_from_file_nonexistent_path_returns_err` | Passing a path that does not exist returns `Err` |
| `test_from_file_returns_non_nil_uuid` | On success, returns a non-nil UUID for the document-level record |

### Document-level metadata

| Test | Description |
|---|---|
| `test_from_file_doc_metadata_has_required_fields` | The document-level metadata record contains `name`, `path`, `slice`, `n_chunks`, and `chunks` fields |
| `test_from_file_doc_metadata_stores_overlap_param` | The `overlap` value supplied at call time is stored verbatim in the document metadata |

### Chunk count

| Test | Description |
|---|---|
| `test_from_file_n_chunks_matches_chunks_list_length` | The `n_chunks` field equals the length of the `chunks` array |
| `test_from_file_multiple_chunks_for_large_text` | A text clearly larger than `slice` produces more than one chunk |
| `test_from_file_single_chunk_when_text_fits_in_slice` | A short text that fits in one slice produces exactly one chunk |
| `test_from_file_more_overlap_produces_more_chunks` | Higher overlap percentage produces more (or equal) chunks than lower overlap for the same text |

### Per-chunk storage

| Test | Description |
|---|---|
| `test_from_file_chunks_have_blob_and_metadata` | Every UUID in `chunks` has a corresponding blob entry and a metadata entry |
| `test_from_file_all_chunks_are_stored_in_blob` | `get_content` returns `Some` for every chunk UUID in the `chunks` list |
| `test_from_file_chunk_metadata_fields` | Each chunk's metadata contains `document_name`, `document_id`, `chunk_index`, and `n_chunks` |

### Text coverage and ordering

| Test | Description |
|---|---|
| `test_from_file_all_words_present_across_chunks` | Every word from the original text appears in at least one chunk (no text is dropped) |
| `test_from_file_chunk_order_matches_document_order` | The last word of chunk `i` appears before the first word of chunk `i+1` in the original text, confirming order preservation |
| `test_from_file_chunk_uuids_are_time_ordered` | The chunk UUIDs in the `chunks` list are monotonically increasing (UUIDv7 ordering matches document order) |

### Overlap behaviour

| Test | Description |
|---|---|
| `test_from_file_overlap_adjacent_chunks_share_content` | When overlap > 0, adjacent chunks share at least one common sentence or phrase at their boundary |
| `test_from_file_zero_overlap_no_shared_sentences` | When overlap = 0, adjacent chunks do not share any sentence-final text |

### Boundary handling

| Test | Description |
|---|---|
| `test_from_file_paragraph_boundary_is_respected` | A two-paragraph text with a paragraph-level split point produces at least two chunks whose boundaries align with the paragraph break |

### RAG retrieval pattern

| Test | Description |
|---|---|
| (covered by `test_from_file_chunks_have_blob_and_metadata` + ordering tests) | Simulates the RAG pattern: search returns a chunk result; `document_id` from chunk metadata resolves to the document-level record which in turn holds the ordered `chunks` list for context expansion |

---

## Live embedding model (`#[ignore]`)

> **`test_with_embedding_add_document_indexes_vectors`** — marked `#[ignore]` because it downloads and runs the `AllMiniLML6V2` model at test time.
>
> Uses `DocumentStorage::with_embedding` to create a store with a live `EmbeddingEngine`. Adds two documents (one Rust/systems, one Python/ML) via plain `add_document` and verifies that `search_document_json` ranks the Rust document first for a systems-domain query and `search_document_text` ranks the Python document first for a machine-learning query. Confirms end-to-end automatic vector indexing from raw JSON metadata and text content.

## Coverage summary

- Directory layout and idempotent construction
- Full CRUD (add, get, update, delete) for both metadata (JSON) and content (bytes)
- Binary data integrity (null bytes, all byte values 0–255)
- UUIDv7 generation properties (non-nil, uniqueness, monotonic ordering)
- Clone semantics: metadata/blob store and HNSW vector index are shared by reference across clones
- Unified vector search across separate metadata and content HNSW indexes; nearest-first ordering; per-result `score` and `id` fields
- `store_metadata_vector` / `store_content_vector` as deferred indexing paths for documents added without an embedding engine
- Correct `Ok(None)` / `Ok(())` behaviour for all operations on unknown UUIDs
- Deletion removes records from all three stores (metadata, blobs, vector index) without disturbing other documents
- Persistence: metadata, blobs, and the vector index (post-`sync`) all survive a close-and-reopen cycle
- `results_to_strings` and `search_document_strings` produce consistent, human-readable fingerprints matching the underlying `search_document` results
- Graceful errors (with "EmbeddingEngine" message) for JSON- and text-query paths when no embedding engine is present
- `add_document_from_file`: file-load errors, document metadata fields, chunk count under varied slice/overlap settings, per-chunk blob and metadata storage, full text coverage across chunks, document-order preservation, UUIDv7 monotonic chunk ordering, overlap content sharing, paragraph boundary alignment

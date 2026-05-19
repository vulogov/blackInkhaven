use bdslib::vectorengine::{json_fingerprint, SearchResult};
use bdslib::VectorEngine;
use serde_json::json;
use tempfile::TempDir;
use vecstore::reranking::{IdentityReranker, MMRReranker, ScoreReranker};

// ── helpers ───────────────────────────────────────────────────────────────────

fn tmp_engine() -> (TempDir, VectorEngine) {
    let dir = TempDir::new().unwrap();
    let engine = VectorEngine::new(dir.path().to_str().unwrap()).unwrap();
    (dir, engine)
}

fn vec3(x: f32, y: f32, z: f32) -> Vec<f32> {
    vec![x, y, z]
}

// ── construction ──────────────────────────────────────────────────────────────

#[test]
fn test_new_creates_store() {
    let (_dir, _engine) = tmp_engine();
}

#[test]
fn test_new_file_backed_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_str().unwrap();

    {
        let engine = VectorEngine::new(path).unwrap();
        engine
            .store_vector("v1", vec3(1.0, 0.0, 0.0), None)
            .unwrap();
        engine.sync().unwrap();
    }

    let engine2 = VectorEngine::new(path).unwrap();
    let results = engine2.search(vec3(1.0, 0.0, 0.0), 5).unwrap();
    assert!(
        results.iter().any(|r| r.id == "v1"),
        "v1 must survive reopen"
    );
}

#[test]
fn test_clone_shares_state() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("v1", vec3(1.0, 0.0, 0.0), None)
        .unwrap();
    let clone = engine.clone();
    let results = clone.search(vec3(1.0, 0.0, 0.0), 5).unwrap();
    assert!(results.iter().any(|r| r.id == "v1"));
}

// ── store_vector ──────────────────────────────────────────────────────────────

#[test]
fn test_store_vector_basic() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("a", vec3(1.0, 0.0, 0.0), None)
        .unwrap();
    let results = engine.search(vec3(1.0, 0.0, 0.0), 1).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "a");
}

#[test]
fn test_store_vector_with_metadata() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector(
            "a",
            vec3(1.0, 0.0, 0.0),
            Some(json!({"label": "test", "score": 0.9})),
        )
        .unwrap();
    let results = engine.search(vec3(1.0, 0.0, 0.0), 1).unwrap();
    assert_eq!(results[0].id, "a");
    assert_eq!(results[0].metadata.fields["label"], json!("test"));
    assert_eq!(results[0].metadata.fields["score"], json!(0.9));
}

#[test]
fn test_store_vector_upsert_replaces_existing() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("a", vec3(1.0, 0.0, 0.0), None)
        .unwrap();
    engine
        .store_vector("a", vec3(0.0, 1.0, 0.0), Some(json!({"updated": true})))
        .unwrap();

    let results = engine.search(vec3(0.0, 1.0, 0.0), 3).unwrap();
    let hit = results.iter().find(|r| r.id == "a").unwrap();
    assert_eq!(hit.metadata.fields["updated"], json!(true));
}

#[test]
fn test_store_multiple_vectors() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("x", vec3(1.0, 0.0, 0.0), None)
        .unwrap();
    engine
        .store_vector("y", vec3(0.0, 1.0, 0.0), None)
        .unwrap();
    engine
        .store_vector("z", vec3(0.0, 0.0, 1.0), None)
        .unwrap();

    let results = engine.search(vec3(1.0, 0.0, 0.0), 3).unwrap();
    assert_eq!(results[0].id, "x"); // closest to query
}

// ── store_document ────────────────────────────────────────────────────────────

#[test]
fn test_store_document_without_embedding_engine_is_error() {
    let (_dir, engine) = tmp_engine();
    let result = engine.store_document("doc1", json!({"text": "hello world"}));
    assert!(
        result.is_err(),
        "store_document without EmbeddingEngine must return Err"
    );
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("EmbeddingEngine"), "{msg}");
}

// Note: store_document with_embedding tests require a live model download,
// so they are marked ignored and run explicitly with --ignored.
#[test]
#[ignore]
fn test_store_document_with_embedding_engine() {
    use bdslib::embedding::Model;
    use bdslib::EmbeddingEngine;

    let dir = TempDir::new().unwrap();
    let emb = EmbeddingEngine::new(Model::AllMiniLML6V2, None).unwrap();
    let engine = VectorEngine::with_embedding(dir.path().to_str().unwrap(), emb).unwrap();

    engine
        .store_document("doc1", json!({"title": "Rust programming", "body": "systems language"}))
        .unwrap();
    engine
        .store_document("doc2", json!({"title": "Python data science", "body": "machine learning"}))
        .unwrap();

    let emb2 = bdslib::EmbeddingEngine::new(Model::AllMiniLML6V2, None).unwrap();
    let query_vec = emb2.embed("Rust systems programming").unwrap();
    let results = engine.search(query_vec, 2).unwrap();

    assert!(!results.is_empty());
    assert_eq!(results[0].id, "doc1"); // semantically closest
}

// ── search ────────────────────────────────────────────────────────────────────

#[test]
fn test_search_returns_correct_limit() {
    let (_dir, engine) = tmp_engine();
    for i in 0..5 {
        let v = vec![i as f32, 0.0, 0.0];
        engine.store_vector(&format!("v{i}"), v, None).unwrap();
    }
    let results = engine.search(vec3(1.0, 0.0, 0.0), 2).unwrap();
    assert!(results.len() <= 2);
}

#[test]
fn test_search_nearest_is_most_similar() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("close", vec3(0.99, 0.01, 0.0), None)
        .unwrap();
    engine
        .store_vector("far", vec3(0.0, 0.0, 1.0), None)
        .unwrap();

    let results = engine.search(vec3(1.0, 0.0, 0.0), 2).unwrap();
    assert_eq!(results[0].id, "close");
}

#[test]
fn test_search_result_has_score_and_metadata() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("a", vec3(1.0, 0.0, 0.0), Some(json!({"key": "val"})))
        .unwrap();

    let results = engine.search(vec3(1.0, 0.0, 0.0), 1).unwrap();
    assert_eq!(results.len(), 1);
    // Querying a vector against itself: cosine similarity ≈ 1.0
    assert!(results[0].score > 0.9, "self-query should have similarity ≈ 1.0, got {}", results[0].score);
    assert_eq!(results[0].metadata.fields["key"], json!("val"));
}

// ── search_reranked ───────────────────────────────────────────────────────────

#[test]
fn test_search_reranked_identity_same_as_search() {
    let (_dir, engine) = tmp_engine();
    for i in 0..5 {
        engine
            .store_vector(&format!("v{i}"), vec![i as f32, 0.0, 0.0], None)
            .unwrap();
    }

    let plain = engine.search(vec3(4.0, 0.0, 0.0), 3).unwrap();
    let reranked = engine
        .search_reranked(vec3(4.0, 0.0, 0.0), "", 3, 5, &IdentityReranker)
        .unwrap();

    // Identity reranker must preserve order and IDs
    let plain_ids: Vec<_> = plain.iter().map(|r| r.id.as_str()).collect();
    let reranked_ids: Vec<_> = reranked.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(plain_ids, reranked_ids);
}

#[test]
fn test_search_reranked_mmr_respects_limit() {
    let (_dir, engine) = tmp_engine();
    for i in 0..10 {
        engine
            .store_vector(&format!("v{i}"), vec![i as f32, 0.0, 0.0], None)
            .unwrap();
    }
    let reranker = MMRReranker::new(0.7);
    let results = engine
        .search_reranked(vec3(5.0, 0.0, 0.0), "", 3, 8, &reranker)
        .unwrap();
    assert!(results.len() <= 3);
}

#[test]
fn test_search_reranked_score_reranker_applies_custom_scoring() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("low", vec3(1.0, 0.0, 0.0), Some(json!({"rank": 1})))
        .unwrap();
    engine
        .store_vector("high", vec3(0.9, 0.1, 0.0), Some(json!({"rank": 100})))
        .unwrap();

    // Rerank by metadata "rank" field
    let reranker = ScoreReranker::new(|n: &SearchResult| {
        n.metadata
            .fields
            .get("rank")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32
    });

    let results = engine
        .search_reranked(vec3(1.0, 0.0, 0.0), "", 2, 2, &reranker)
        .unwrap();
    assert_eq!(results[0].id, "high"); // rank=100 wins
}

#[test]
fn test_search_reranked_candidate_pool_clamped_to_limit() {
    // candidate_pool < limit should not panic — impl clamps pool to max(pool, limit)
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("a", vec3(1.0, 0.0, 0.0), None)
        .unwrap();
    let results = engine
        .search_reranked(vec3(1.0, 0.0, 0.0), "", 5, 1, &IdentityReranker)
        .unwrap();
    assert!(!results.is_empty());
}

// ── sync ──────────────────────────────────────────────────────────────────────

#[test]
fn test_sync_does_not_error_on_empty_store() {
    let (_dir, engine) = tmp_engine();
    engine.sync().unwrap();
}

#[test]
fn test_sync_does_not_error_after_inserts() {
    let (_dir, engine) = tmp_engine();
    engine
        .store_vector("a", vec3(1.0, 0.0, 0.0), None)
        .unwrap();
    engine.sync().unwrap();
}

// ── concurrency ───────────────────────────────────────────────────────────────

#[test]
fn test_concurrent_store_and_search() {
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    let engine = Arc::new(VectorEngine::new(dir.path().to_str().unwrap()).unwrap());

    let writers: Vec<_> = (0..8u32)
        .map(|i| {
            let e = engine.clone();
            std::thread::spawn(move || {
                e.store_vector(&format!("v{i}"), vec![i as f32, 0.0, 0.0], None)
                    .unwrap();
            })
        })
        .collect();

    for h in writers {
        h.join().unwrap();
    }

    let results = engine.search(vec3(4.0, 0.0, 0.0), 8).unwrap();
    assert!(!results.is_empty());
}

// ── json_fingerprint ──────────────────────────────────────────────────────────

#[test]
fn test_fingerprint_string_value() {
    assert_eq!(json_fingerprint(&json!("hello")), "hello");
}

#[test]
fn test_fingerprint_number_value() {
    assert_eq!(json_fingerprint(&json!(42)), "42");
}

#[test]
fn test_fingerprint_bool_value() {
    assert_eq!(json_fingerprint(&json!(true)), "true");
}

#[test]
fn test_fingerprint_null_is_empty() {
    assert_eq!(json_fingerprint(&json!(null)), "");
}

#[test]
fn test_fingerprint_flat_object_includes_field_names() {
    let fp = json_fingerprint(&json!({"title": "Rust", "year": 2015}));
    assert!(fp.contains("title: Rust"), "got: {fp}");
    assert!(fp.contains("year: 2015"), "got: {fp}");
}

#[test]
fn test_fingerprint_different_field_names_produce_different_fingerprints() {
    let fp_title = json_fingerprint(&json!({"title": "Rust"}));
    let fp_body  = json_fingerprint(&json!({"body":  "Rust"}));
    assert_ne!(
        fp_title, fp_body,
        "same value under different keys must produce different fingerprints"
    );
}

#[test]
fn test_fingerprint_nested_object_uses_dot_path() {
    let fp = json_fingerprint(&json!({"meta": {"author": "Alice", "year": 2024}}));
    assert!(fp.contains("meta.author: Alice"), "got: {fp}");
    assert!(fp.contains("meta.year: 2024"), "got: {fp}");
}

#[test]
fn test_fingerprint_deeply_nested_object() {
    let fp = json_fingerprint(&json!({"a": {"b": {"c": "deep"}}}));
    assert!(fp.contains("a.b.c: deep"), "got: {fp}");
}

#[test]
fn test_fingerprint_array_uses_index_notation() {
    let fp = json_fingerprint(&json!({"tags": ["rust", "systems"]}));
    assert!(fp.contains("tags[0]: rust"), "got: {fp}");
    assert!(fp.contains("tags[1]: systems"), "got: {fp}");
}

#[test]
fn test_fingerprint_array_of_objects() {
    let fp = json_fingerprint(&json!({"items": [{"name": "foo"}, {"name": "bar"}]}));
    assert!(fp.contains("items[0].name: foo"), "got: {fp}");
    assert!(fp.contains("items[1].name: bar"), "got: {fp}");
}

#[test]
fn test_fingerprint_top_level_array() {
    let fp = json_fingerprint(&json!(["alpha", "beta"]));
    assert!(fp.contains("[0]: alpha"), "got: {fp}");
    assert!(fp.contains("[1]: beta"), "got: {fp}");
}

#[test]
fn test_fingerprint_skips_null_fields() {
    let fp = json_fingerprint(&json!({"present": "yes", "absent": null}));
    assert!(fp.contains("present: yes"), "got: {fp}");
    assert!(!fp.contains("absent"), "null field must be absent from fingerprint, got: {fp}");
}

#[test]
fn test_fingerprint_boolean_field() {
    let fp = json_fingerprint(&json!({"active": true, "deleted": false}));
    assert!(fp.contains("active: true"), "got: {fp}");
    assert!(fp.contains("deleted: false"), "got: {fp}");
}

#[test]
fn test_fingerprint_empty_object_is_empty() {
    assert_eq!(json_fingerprint(&json!({})), "");
}

#[test]
fn test_fingerprint_empty_array_is_empty() {
    assert_eq!(json_fingerprint(&json!([])), "");
}

#[test]
fn test_fingerprint_mixed_document() {
    let doc = json!({
        "title": "Rust programming",
        "meta": {
            "version": 2,
            "stable": true
        },
        "tags": ["systems", "safe"],
        "deprecated": null
    });
    let fp = json_fingerprint(&doc);
    assert!(fp.contains("title: Rust programming"), "got: {fp}");
    assert!(fp.contains("meta.version: 2"), "got: {fp}");
    assert!(fp.contains("meta.stable: true"), "got: {fp}");
    assert!(fp.contains("tags[0]: systems"), "got: {fp}");
    assert!(fp.contains("tags[1]: safe"), "got: {fp}");
    assert!(!fp.contains("deprecated"), "null field must be absent, got: {fp}");
}

// ── search_json ───────────────────────────────────────────────────────────────

#[test]
fn test_search_json_without_embedding_engine_is_error() {
    let (_dir, engine) = tmp_engine();
    let result = engine.search_json(&json!({"title": "test"}), 5);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("EmbeddingEngine"));
}

#[test]
fn test_search_json_reranked_without_embedding_engine_is_error() {
    let (_dir, engine) = tmp_engine();
    let result = engine.search_json_reranked(&json!({"title": "test"}), 5, 10, &IdentityReranker);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("EmbeddingEngine"));
}

// Live model tests — run with: cargo test -- --ignored
#[test]
#[ignore]
fn test_search_json_finds_semantically_similar_documents() {
    use bdslib::embedding::Model;
    use bdslib::EmbeddingEngine;

    let dir = TempDir::new().unwrap();
    let emb = EmbeddingEngine::new(Model::AllMiniLML6V2, None).unwrap();
    let engine = VectorEngine::with_embedding(dir.path().to_str().unwrap(), emb).unwrap();

    engine
        .store_document(
            "rust",
            json!({"title": "Rust programming language", "body": "systems memory safety"}),
        )
        .unwrap();
    engine
        .store_document(
            "python",
            json!({"title": "Python data science", "body": "machine learning numpy"}),
        )
        .unwrap();

    let results = engine
        .search_json(&json!({"title": "systems language", "body": "memory safe"}), 2)
        .unwrap();

    assert!(!results.is_empty());
    assert_eq!(results[0].id, "rust");
}

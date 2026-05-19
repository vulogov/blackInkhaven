use bdslib::EmbeddingEngine;
use bdslib::embedding::Model;
use std::sync::{Arc, OnceLock};

fn engine() -> &'static EmbeddingEngine {
    static ENGINE: OnceLock<EmbeddingEngine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        EmbeddingEngine::new(Model::AllMiniLML6V2, None)
            .expect("failed to load AllMiniLML6V2")
    })
}

// ── compare_embeddings: pure math, no model ──────────────────────────────────

#[test]
fn test_compare_identical_vectors() {
    let v = vec![1.0f32, 0.0, 0.0];
    let sim = EmbeddingEngine::compare_embeddings(&v, &v).unwrap();
    assert!((sim - 1.0).abs() < 1e-6, "identical vectors must have sim ≈ 1.0, got {sim}");
}

#[test]
fn test_compare_orthogonal_vectors() {
    let a = vec![1.0f32, 0.0, 0.0];
    let b = vec![0.0f32, 1.0, 0.0];
    let sim = EmbeddingEngine::compare_embeddings(&a, &b).unwrap();
    assert!(sim.abs() < 1e-6, "orthogonal vectors must have sim ≈ 0.0, got {sim}");
}

#[test]
fn test_compare_opposite_vectors() {
    let a = vec![1.0f32, 0.0, 0.0];
    let b = vec![-1.0f32, 0.0, 0.0];
    let sim = EmbeddingEngine::compare_embeddings(&a, &b).unwrap();
    assert!((sim + 1.0).abs() < 1e-6, "opposite vectors must have sim ≈ -1.0, got {sim}");
}

#[test]
fn test_compare_embeddings_is_symmetric() {
    let a = vec![0.6f32, 0.8, 0.0];
    let b = vec![0.0f32, 0.6, 0.8];
    let ab = EmbeddingEngine::compare_embeddings(&a, &b).unwrap();
    let ba = EmbeddingEngine::compare_embeddings(&b, &a).unwrap();
    assert!((ab - ba).abs() < 1e-6, "cosine similarity must be symmetric");
}

#[test]
fn test_compare_embeddings_result_in_range() {
    let a = vec![0.3f32, 0.4, 0.5, 0.6];
    let b = vec![-0.1f32, 0.9, 0.2, -0.5];
    let sim = EmbeddingEngine::compare_embeddings(&a, &b).unwrap();
    assert!(sim >= -1.0 && sim <= 1.0, "similarity must be in [-1, 1], got {sim}");
}

#[test]
fn test_compare_embeddings_dimension_mismatch_errors() {
    let a = vec![1.0f32, 0.0];
    let b = vec![1.0f32, 0.0, 0.0];
    let result = EmbeddingEngine::compare_embeddings(&a, &b);
    assert!(result.is_err(), "dimension mismatch must return Err");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("mismatch"), "error message should mention mismatch, got: {msg}");
}

#[test]
fn test_compare_embeddings_empty_vectors_error() {
    let result = EmbeddingEngine::compare_embeddings(&[], &[]);
    assert!(result.is_err(), "empty vectors must return Err");
}

#[test]
fn test_compare_embeddings_zero_vector_error() {
    let a = vec![0.0f32, 0.0, 0.0];
    let b = vec![1.0f32, 0.0, 0.0];
    let result = EmbeddingEngine::compare_embeddings(&a, &b);
    assert!(result.is_err(), "zero-norm vector must return Err");
}

// ── embed: model-dependent ────────────────────────────────────────────────────

#[test]
fn test_embed_returns_nonempty_vector() {
    let emb = engine().embed("hello world").unwrap();
    assert!(!emb.is_empty(), "embedding must not be empty");
}

#[test]
fn test_embed_dimension_is_384() {
    let emb = engine().embed("test sentence for dimension check").unwrap();
    assert_eq!(emb.len(), 384, "AllMiniLML6V2 must produce 384-dim embeddings");
}

#[test]
fn test_embed_consistent_dimension_across_calls() {
    let e1 = engine().embed("first sentence").unwrap();
    let e2 = engine().embed("a completely different second sentence with more words").unwrap();
    assert_eq!(e1.len(), e2.len(), "all embeddings from the same model must have the same dimension");
}

#[test]
fn test_embed_same_text_gives_similar_embedding() {
    let e1 = engine().embed("the quick brown fox").unwrap();
    let e2 = engine().embed("the quick brown fox").unwrap();
    let sim = EmbeddingEngine::compare_embeddings(&e1, &e2).unwrap();
    assert!(sim > 0.999, "identical text must embed to (near-)identical vector, got sim={sim}");
}

#[test]
fn test_embed_different_texts_differ() {
    let e1 = engine().embed("cat sat on the mat").unwrap();
    let e2 = engine().embed("quantum chromodynamics and nuclear physics").unwrap();
    let sim = EmbeddingEngine::compare_embeddings(&e1, &e2).unwrap();
    assert!(sim < 0.95, "unrelated texts should have lower similarity, got sim={sim}");
}

// ── compare_texts ─────────────────────────────────────────────────────────────

#[test]
fn test_compare_texts_result_in_range() {
    let sim = engine().compare_texts("hello world", "goodbye world").unwrap();
    assert!(sim >= -1.0 && sim <= 1.0, "result must be in [-1, 1], got {sim}");
}

#[test]
fn test_compare_texts_same_text_is_near_one() {
    let sim = engine().compare_texts("the quick brown fox", "the quick brown fox").unwrap();
    assert!(sim > 0.999, "same text must score ≈ 1.0, got {sim}");
}

#[test]
fn test_compare_texts_semantic_similarity() {
    let similar = engine()
        .compare_texts("the cat sat on the mat", "a cat rested on a rug")
        .unwrap();
    let unrelated = engine()
        .compare_texts("the cat sat on the mat", "quantum mechanics describes subatomic particles")
        .unwrap();
    assert!(
        similar > unrelated,
        "semantically similar pair ({similar:.3}) must score higher than unrelated pair ({unrelated:.3})"
    );
}

#[test]
fn test_compare_texts_is_symmetric() {
    let ab = engine().compare_texts("rust programming language", "systems software in rust").unwrap();
    let ba = engine().compare_texts("systems software in rust", "rust programming language").unwrap();
    assert!((ab - ba).abs() < 1e-5, "compare_texts must be symmetric, got {ab} vs {ba}");
}

#[test]
fn test_compare_texts_matches_manual_pipeline() {
    let e = engine();
    let a = "fast text search";
    let b = "full text retrieval";
    let direct = e.compare_texts(a, b).unwrap();
    let ea = e.embed(a).unwrap();
    let eb = e.embed(b).unwrap();
    let manual = EmbeddingEngine::compare_embeddings(&ea, &eb).unwrap();
    assert!(
        (direct - manual).abs() < 1e-4,
        "compare_texts must match embed+compare_embeddings, got {direct} vs {manual}"
    );
}

// ── concurrency ───────────────────────────────────────────────────────────────

#[test]
fn test_concurrent_embed_returns_consistent_results() {
    let engine = Arc::new(EmbeddingEngine::new(Model::AllMiniLML6V2, None).unwrap());
    let text = "concurrency test sentence";
    let reference = engine.embed(text).unwrap();

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let e = engine.clone();
            std::thread::spawn(move || e.embed(text).unwrap())
        })
        .collect();

    for handle in handles {
        let result = handle.join().expect("thread panicked");
        let sim = EmbeddingEngine::compare_embeddings(&reference, &result).unwrap();
        assert!(sim > 0.999, "concurrent embed must return same vector, got sim={sim}");
    }
}

#[test]
fn test_engine_is_clone() {
    let e1 = engine();
    let text = "clone test";
    // EmbeddingEngine is Clone; both copies must produce the same embedding
    let cloned = e1.clone();
    let emb1 = e1.embed(text).unwrap();
    let emb2 = cloned.embed(text).unwrap();
    let sim = EmbeddingEngine::compare_embeddings(&emb1, &emb2).unwrap();
    assert!(sim > 0.999, "clone must share the same model state, got sim={sim}");
}

use bdslib::common::error::Result;
use bdslib::embedding::Model;
use bdslib::{EmbeddingEngine, VectorEngine};
use serde_json::json;
use tempfile::TempDir;
use vecstore::reranking::{IdentityReranker, MMRReranker};

fn main() -> Result<()> {
    // ── raw vector store (no embedding) ──────────────────────────────────────

    println!("=== Raw vector store ===\n");
    let raw_dir = TempDir::new().map_err(|e| bdslib::common::error::err_msg(e.to_string()))?;
    let raw = VectorEngine::new(raw_dir.path().to_str().unwrap())?;

    // Store 3-dimensional vectors with metadata
    raw.store_vector("a", vec![1.0, 0.0, 0.0], Some(json!({ "label": "X-axis" })))?;
    raw.store_vector("b", vec![0.0, 1.0, 0.0], Some(json!({ "label": "Y-axis" })))?;
    raw.store_vector("c", vec![0.0, 0.0, 1.0], Some(json!({ "label": "Z-axis" })))?;
    raw.store_vector(
        "d",
        vec![0.9, 0.1, 0.0],
        Some(json!({ "label": "near X" })),
    )?;

    // Search nearest to [1, 0, 0]
    let results = raw.search(vec![1.0, 0.0, 0.0], 3)?;
    println!("nearest to [1,0,0]:");
    for r in &results {
        let label = r
            .metadata
            .fields
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        println!("  {} ({label})  score={:.4}", r.id, r.score);
    }
    println!();

    // Upsert: overwrite "a" and search again
    raw.store_vector("a", vec![0.5, 0.5, 0.0], Some(json!({ "label": "XY plane" })))?;
    let results = raw.search(vec![1.0, 0.0, 0.0], 2)?;
    println!("after upsert of 'a' to [0.5,0.5,0]:");
    for r in &results {
        println!("  {} score={:.4}", r.id, r.score);
    }
    println!();

    // Re-ranked search
    let reranker = IdentityReranker;
    let results = raw.search_reranked(vec![1.0, 0.0, 0.0], "", 2, 4, &reranker)?;
    println!("score-reranked (pool=4, limit=2):");
    for r in &results {
        println!("  {} score={:.4}", r.id, r.score);
    }
    println!();

    // Persist
    raw.sync()?;
    println!("store synced to disk\n");

    // ── document store (with embedding) ──────────────────────────────────────

    println!("=== Document store (AllMiniLML6V2) ===\n");
    println!("Loading model...");
    let emb_engine = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
    let vec_dir = TempDir::new().map_err(|e| bdslib::common::error::err_msg(e.to_string()))?;
    let doc = VectorEngine::with_embedding(vec_dir.path().to_str().unwrap(), emb_engine)?;
    println!("Ready\n");

    // Index a small document corpus
    let corpus = [
        (
            "doc-rust",
            json!({
                "title": "The Rust Programming Language",
                "author": "Steve Klabnik",
                "tags": ["systems", "memory-safety", "ownership"],
            }),
        ),
        (
            "doc-vec",
            json!({
                "title": "Vector Embeddings and Semantic Search",
                "author": "Jane Doe",
                "tags": ["nlp", "embeddings", "search"],
            }),
        ),
        (
            "doc-sql",
            json!({
                "title": "Analytical Queries with DuckDB",
                "author": "Mark Smith",
                "tags": ["sql", "olap", "analytics"],
            }),
        ),
        (
            "doc-fts",
            json!({
                "title": "Full-Text Search with Tantivy",
                "author": "Alice Brown",
                "tags": ["search", "inverted-index", "rust"],
            }),
        ),
        (
            "doc-safety",
            json!({
                "title": "Memory Safety Without Garbage Collection",
                "author": "Bob Lee",
                "tags": ["rust", "safety", "systems"],
            }),
        ),
    ];

    println!("Indexing {} documents...", corpus.len());
    for (id, doc_json) in &corpus {
        doc.store_document(id, doc_json.clone())?;
        println!("  stored {id}");
    }
    println!();

    // search_json: find documents related to Rust memory safety
    let query = json!({ "title": "memory safety", "tags": ["rust"] });
    println!("search_json query: {query}");
    let results = doc.search_json(&query, 3)?;
    println!("top-3 results:");
    for (rank, r) in results.iter().enumerate() {
        let title = r
            .metadata
            .fields
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        println!("  #{} {} — \"{title}\"  score={:.4}", rank + 1, r.id, r.score);
    }
    println!();

    // search_json_reranked with MMR (diversity-aware)
    let query2 = json!({ "title": "search", "tags": ["rust"] });
    println!("search_json_reranked (MMR) query: {query2}");
    let mmr = MMRReranker::new(0.7);
    let results = doc.search_json_reranked(&query2, 3, 5, &mmr)?;
    println!("top-3 MMR results:");
    for (rank, r) in results.iter().enumerate() {
        let title = r
            .metadata
            .fields
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        println!("  #{} {} — \"{title}\"  score={:.4}", rank + 1, r.id, r.score);
    }
    println!();

    // json_fingerprint: inspect what gets embedded
    println!("-- json_fingerprint examples --");
    use bdslib::vectorengine::json_fingerprint;
    let sample = json!({
        "title": "Rust",
        "meta": { "year": 2015, "tags": ["systems", "safe"] }
    });
    println!("  input : {sample}");
    println!("  output: \"{}\"", json_fingerprint(&sample));
    println!();

    // ── cloning shares the store ──────────────────────────────────────────────

    println!("-- clone shares underlying store --");
    let clone = doc.clone();
    let results_orig = doc.search_json(&json!({ "title": "Rust" }), 1)?;
    let results_clone = clone.search_json(&json!({ "title": "Rust" }), 1)?;
    assert_eq!(
        results_orig[0].id, results_clone[0].id,
        "both clones see the same store"
    );
    println!("  original top-1: {}", results_orig[0].id);
    println!("  clone    top-1: {}", results_clone[0].id);
    println!("  match: true");

    doc.sync()?;
    println!("\nDocument store synced to disk");

    Ok(())
}

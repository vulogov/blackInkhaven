use bdslib::common::error::{err_msg, Result};
use bdslib::{embedding::Model, results_to_strings, DocumentStorage, EmbeddingEngine};
use serde_json::json;
use tempfile::TempDir;

fn main() -> Result<()> {
    // ── 1. Creating a store ───────────────────────────────────────────────────

    println!("=== 1. Creating a store ===\n");

    let root_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let root_path = root_dir.path().to_str().unwrap().to_string();
    println!("root: {root_path}");

    let store = DocumentStorage::new(&root_path)?;
    println!("DocumentStorage opened successfully");
    println!();

    // ── 2. add_document — three docs with varied metadata shapes ─────────────

    println!("=== 2. add_document ===\n");

    // Flat metadata
    let meta1 = json!({ "title": "Rust Programming Language", "author": "Steve Klabnik", "year": 2019 });
    let content1 = b"Rust is a systems programming language focused on safety, speed, and concurrency.";
    let id1 = store.add_document(meta1, content1)?;
    println!("add_document (flat meta)   -> {id1}");

    // Nested metadata
    let meta2 = json!({
        "title": "The Art of Unix Programming",
        "author": { "first": "Eric", "last": "Raymond" },
        "publisher": { "name": "Addison-Wesley", "year": 2003 }
    });
    let content2 = b"Unix philosophy: write programs that do one thing well.";
    let id2 = store.add_document(meta2, content2)?;
    println!("add_document (nested meta) -> {id2}");

    // Metadata with arrays
    let meta3 = json!({
        "title": "Designing Data-Intensive Applications",
        "tags": ["databases", "distributed-systems", "scalability"],
        "chapters": 12
    });
    let content3 = b"The big ideas behind reliable, scalable, and maintainable systems.";
    let id3 = store.add_document(meta3, content3)?;
    println!("add_document (array meta)  -> {id3}");
    println!();

    // ── 3. get_metadata / get_content ────────────────────────────────────────

    println!("=== 3. get_metadata / get_content ===\n");

    for (label, id, expected_content) in [
        ("doc1", id1, content1.as_ref()),
        ("doc2", id2, content2.as_ref()),
        ("doc3", id3, content3.as_ref()),
    ] {
        let meta = store.get_metadata(id)?.expect("metadata must exist");
        let bytes = store.get_content(id)?.expect("content must exist");
        let text = String::from_utf8_lossy(&bytes);
        println!("{label} ({id})");
        println!("  metadata : {meta}");
        println!("  content  : {text}");
        println!("  roundtrip: {}", bytes == expected_content);
        println!();
    }

    // ── 4. update_metadata / update_content ──────────────────────────────────

    println!("=== 4. update_metadata / update_content ===\n");

    // Update doc1's metadata
    let new_meta1 = json!({ "title": "The Rust Programming Language", "edition": 2, "open_source": true });
    store.update_metadata(id1, new_meta1)?;
    let confirmed_meta = store.get_metadata(id1)?.expect("doc1 must exist");
    println!("update_metadata(doc1)");
    println!("  new metadata: {confirmed_meta}");
    println!();

    // Update doc2's content
    let new_content2 = b"Rule of Modularity: write simple parts connected by clean interfaces.";
    store.update_content(id2, new_content2)?;
    let confirmed_content = store.get_content(id2)?.expect("doc2 must exist");
    println!("update_content(doc2)");
    println!("  new content : {}", String::from_utf8_lossy(&confirmed_content));
    println!();

    // ── 5. delete_document ────────────────────────────────────────────────────

    println!("=== 5. delete_document ===\n");

    store.delete_document(id3)?;
    let after_delete = store.get_metadata(id3)?;
    println!("delete_document(doc3)  ->  get_metadata returns: {after_delete:?}");
    println!("  confirmed gone: {}", after_delete.is_none());
    println!();

    // ── 6. add_document_with_vectors ─────────────────────────────────────────

    println!("=== 6. add_document_with_vectors ===\n");

    // Three-dimensional unit vectors along each axis
    let meta_a = json!({ "title": "Alpha", "topic": "machine learning" });
    let content_a = b"Deep neural networks for image recognition.";
    let meta_vec_a = vec![1.0_f32, 0.0, 0.0]; // x-axis
    let cont_vec_a = vec![1.0_f32, 0.0, 0.0];
    let vid_a = store.add_document_with_vectors(meta_a, content_a, meta_vec_a, cont_vec_a)?;
    println!("add_document_with_vectors (x-axis) -> {vid_a}");

    let meta_b = json!({ "title": "Beta", "topic": "natural language processing" });
    let content_b = b"Transformer models for text understanding and generation.";
    let meta_vec_b = vec![0.0_f32, 1.0, 0.0]; // y-axis
    let cont_vec_b = vec![0.0_f32, 1.0, 0.0];
    let vid_b = store.add_document_with_vectors(meta_b, content_b, meta_vec_b, cont_vec_b)?;
    println!("add_document_with_vectors (y-axis) -> {vid_b}");

    let meta_c = json!({ "title": "Gamma", "topic": "computer vision" });
    let content_c = b"Convolutional neural networks for visual perception.";
    let meta_vec_c = vec![0.0_f32, 0.0, 1.0]; // z-axis
    let cont_vec_c = vec![0.0_f32, 0.0, 1.0];
    let vid_c = store.add_document_with_vectors(meta_c, content_c, meta_vec_c, cont_vec_c)?;
    println!("add_document_with_vectors (z-axis) -> {vid_c}");
    println!();

    // ── 7. search_document ────────────────────────────────────────────────────

    println!("=== 7. search_document ===\n");

    // Query close to x-axis — should rank Alpha highest
    let query = vec![0.9_f32, 0.3, 0.1];
    let results = store.search_document(query.clone(), 3)?;
    println!("query vector: [{:.1}, {:.1}, {:.1}]  limit: 3", 0.9, 0.3, 0.1);
    println!("results ({} found):", results.len());
    for r in &results {
        println!(
            "  id={id}  score={score:.4}  title={title}  document={doc}",
            id    = r["id"],
            score = r["score"].as_f64().unwrap_or(0.0),
            title = r["metadata"]["title"],
            doc   = r["document"].as_str().unwrap_or(""),
        );
    }
    println!();

    // ── 8. results_to_strings ─────────────────────────────────────────────────

    println!("=== 8. results_to_strings ===\n");

    let fingerprints = results_to_strings(&results);
    println!("results_to_strings ({} fingerprints):", fingerprints.len());
    for fp in &fingerprints {
        println!("  {fp}");
    }
    println!();

    // ── 9. search_document_strings ────────────────────────────────────────────

    println!("=== 9. search_document_strings ===\n");

    // Query close to y-axis — should rank Beta highest
    let query_y = vec![0.1_f32, 0.9, 0.2];
    let str_results = store.search_document_strings(query_y, 3)?;
    println!("query vector: [{:.1}, {:.1}, {:.1}]  limit: 3", 0.1, 0.9, 0.2);
    println!("search_document_strings ({} results):", str_results.len());
    for s in &str_results {
        println!("  {s}");
    }
    println!();

    // ── 10. store_metadata_vector / store_content_vector ─────────────────────

    println!("=== 10. store_metadata_vector / store_content_vector ===\n");

    // Add a doc without vectors first
    let meta_d = json!({ "title": "Delta", "topic": "reinforcement learning" });
    let content_d = b"Policy gradient methods for sequential decision making.";
    let vid_d = store.add_document(meta_d.clone(), content_d)?;
    println!("add_document (no vectors) -> {vid_d}");

    // Post-hoc vector indexing — place it near the x-axis so it competes with Alpha
    let posthoc_meta_vec  = vec![0.95_f32, 0.2, 0.05];
    let posthoc_cont_vec  = vec![0.95_f32, 0.1, 0.0];
    store.store_metadata_vector(vid_d, posthoc_meta_vec, meta_d)?;
    store.store_content_vector(vid_d, posthoc_cont_vec)?;
    println!("store_metadata_vector + store_content_vector done");

    // Search again near x-axis — Delta should now appear
    let results_after = store.search_document(vec![0.9_f32, 0.1, 0.0], 4)?;
    let titles: Vec<&str> = results_after
        .iter()
        .map(|r| r["metadata"]["title"].as_str().unwrap_or("?"))
        .collect();
    println!("search after post-hoc indexing (top 4 titles): {titles:?}");
    let found_delta = results_after.iter().any(|r| r["id"].as_str() == Some(&vid_d.to_string()));
    println!("  Delta in results: {found_delta}");
    println!();

    // ── 11. sync + reopen ─────────────────────────────────────────────────────

    println!("=== 11. sync + reopen ===\n");

    store.sync()?;
    println!("sync() done");

    // Drop the store explicitly by shadowing; then reopen from the same path.
    drop(store);
    println!("store dropped");

    let store2 = DocumentStorage::new(&root_path)?;
    println!("store reopened from {root_path}");

    // Verify Alpha (vid_a) is still searchable after reopen
    let reopen_results = store2.search_document(vec![1.0_f32, 0.0, 0.0], 2)?;
    println!("search after reopen (limit 2):");
    for r in &reopen_results {
        println!(
            "  id={id}  score={score:.4}  title={title}",
            id    = r["id"],
            score = r["score"].as_f64().unwrap_or(0.0),
            title = r["metadata"]["title"],
        );
    }
    let found_alpha = reopen_results.iter().any(|r| r["id"].as_str() == Some(&vid_a.to_string()));
    println!("  Alpha survives reopen: {found_alpha}");
    println!();

    // ── 12. Clone sharing ─────────────────────────────────────────────────────

    println!("=== 12. Clone sharing ===\n");

    let clone = store2.clone();

    // Add a new document via the original store2
    let meta_e = json!({ "title": "Epsilon", "topic": "graph neural networks" });
    let content_e = b"Graph convolutional networks for relational data.";
    let meta_vec_e = vec![0.6_f32, 0.6, 0.5];
    let cont_vec_e = vec![0.6_f32, 0.5, 0.6];
    let vid_e = store2.add_document_with_vectors(meta_e, content_e, meta_vec_e, cont_vec_e)?;
    println!("original wrote doc Epsilon -> {vid_e}");

    // Read back via the clone
    let via_clone_meta = clone.get_metadata(vid_e)?.expect("clone must see same data");
    let via_clone_content = clone.get_content(vid_e)?.expect("clone must see same data");
    println!("clone read metadata : {via_clone_meta}");
    println!("clone read content  : {}", String::from_utf8_lossy(&via_clone_content));

    // Search via the clone — Epsilon should appear near its insertion vector
    let clone_results = clone.search_document(vec![0.6_f32, 0.6, 0.5], 2)?;
    println!("clone search (limit 2):");
    for r in &clone_results {
        println!(
            "  id={id}  score={score:.4}  title={title}",
            id    = r["id"],
            score = r["score"].as_f64().unwrap_or(0.0),
            title = r["metadata"]["title"],
        );
    }
    let found_epsilon = clone_results
        .iter()
        .any(|r| r["id"].as_str() == Some(&vid_e.to_string()));
    println!("  Epsilon visible via clone: {found_epsilon}");
    println!();

    // ── 13. Freeform text search with with_embedding ──────────────────────────
    //
    // All previous sections used pre-computed vectors. This section uses an
    // EmbeddingEngine so that:
    //   - add_document() embeds both metadata and content automatically
    //   - search_document_text(query_string) embeds the freeform query and
    //     searches — no vectors needed from the caller
    //
    // Requires network access on first run to download AllMiniLML6V2 (~23 MB).

    println!("=== 13. Freeform text search (with_embedding) ===\n");
    println!("Loading AllMiniLML6V2 embedding model...");

    let emb_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let emb_root = emb_dir.path().to_str().unwrap();
    let engine = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
    let emb_store = DocumentStorage::with_embedding(emb_root, engine)?;
    println!("Model loaded. DocumentStorage ready.\n");

    // Index five books — metadata + raw text content. Both are embedded
    // automatically: metadata via json_fingerprint(), content as plain UTF-8.
    let books: &[_] = &[
        (
            json!({"title": "The Rust Programming Language", "author": "Klabnik", "domain": "systems"}),
            "Memory safety without garbage collection. Ownership, borrowing, and lifetimes for safe concurrent systems programming.",
        ),
        (
            json!({"title": "Python Machine Learning", "author": "Raschka", "domain": "ml"}),
            "Scikit-learn, Keras, and TensorFlow for training, evaluating, and deploying classification and regression models.",
        ),
        (
            json!({"title": "Designing Data-Intensive Applications", "author": "Kleppmann", "domain": "distributed"}),
            "Replication, partitioning, transactions, consensus, and the principles behind reliable large-scale distributed systems.",
        ),
        (
            json!({"title": "The Linux Programming Interface", "author": "Kerrisk", "domain": "systems"}),
            "System calls, file I/O, processes, signals, threads, and TCP/IP sockets on Linux and POSIX-compliant Unix systems.",
        ),
        (
            json!({"title": "Deep Learning", "author": "Goodfellow", "domain": "ml"}),
            "Feedforward networks, convolutional nets, recurrent architectures, and generative adversarial models from first principles.",
        ),
    ];
    for (meta, content) in books {
        let id = emb_store.add_document(meta.clone(), content.as_bytes())?;
        println!("  indexed {:40} -> {id}", meta["title"].as_str().unwrap_or("?"));
    }
    println!();

    // ── search_document_text: freeform string query ───────────────────────────
    // The query string is embedded on the fly. Both the ":meta" and ":content"
    // vector entries for every stored document are searched; results are
    // deduplicated by UUID and each entry shows the full metadata object and
    // the raw content decoded to UTF-8.

    let query = "concurrent memory-safe systems programming";
    println!("search_document_text(\"{query}\", limit=3)");
    println!("Each result: [score] title | metadata.domain | document content\n");

    let results = emb_store.search_document_text(query, 3)?;
    for r in &results {
        println!(
            "  [{score:.3}]  {title:<45}  domain={domain}",
            score  = r["score"].as_f64().unwrap_or(0.0),
            title  = r["metadata"]["title"].as_str().unwrap_or("?"),
            domain = r["metadata"]["domain"].as_str().unwrap_or("?"),
        );
        println!("           content: {}", r["document"].as_str().unwrap_or(""));
        println!();
    }

    // ── search_document_json: structured metadata query ───────────────────────
    // Use a JSON object as the query. json_fingerprint() converts it to a
    // "path: value" string before embedding, so field names are part of the
    // semantic signal — matching stored metadata fingerprints.

    let meta_query = json!({"domain": "ml", "topic": "neural networks training"});
    println!("search_document_json({{domain: ml, topic: neural networks training}}, limit=2)");
    println!("Each result: [score] title | raw document content\n");

    let json_results = emb_store.search_document_json(&meta_query, 2)?;
    for r in &json_results {
        println!(
            "  [{score:.3}]  {title}",
            score = r["score"].as_f64().unwrap_or(0.0),
            title = r["metadata"]["title"].as_str().unwrap_or("?"),
        );
        println!("           content: {}", r["document"].as_str().unwrap_or(""));
        println!();
    }

    // ── search_document_text_strings: freeform query → fingerprint strings ────
    // Same search as above but each result is serialised through json_fingerprint
    // before returning — ready to be re-embedded or ingested into an FTS index.

    let query2 = "distributed systems replication fault tolerance";
    println!("search_document_text_strings(\"{query2}\", limit=2)");
    println!("Each result: json_fingerprint of {{id, metadata, document, score}}\n");

    let str_results = emb_store.search_document_text_strings(query2, 2)?;
    for (i, s) in str_results.iter().enumerate() {
        println!("  [{i}] {s}");
    }
    println!();

    Ok(())
}

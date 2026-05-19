use bdslib::common::error::{err_msg, Result};
use bdslib::{embedding::Model, DocumentStorage, EmbeddingEngine};
use std::fs;
use tempfile::TempDir;
use uuid::Uuid;

// ── Source documents ──────────────────────────────────────────────────────────

const DOC_RUST: &str = "\
Rust: Memory Safety Without Garbage Collection

Rust is a systems programming language that achieves memory safety through a \
compile-time ownership system rather than a garbage collector. Every value in \
Rust has exactly one owner at a time. When the owner goes out of scope the \
value is dropped and its memory is freed automatically. This deterministic \
cleanup means no GC pauses and no dangling pointers.

The borrowing rules enforce that you may have either one mutable reference or \
any number of immutable references to a value at the same time, but never \
both. The compiler verifies these constraints statically, turning data races \
into compile errors. Code that would race at runtime simply does not compile.

Lifetimes are annotations that tell the compiler how long references are \
valid. Most of the time the compiler infers them automatically through \
lifetime elision rules. Explicit lifetime annotations appear only when the \
compiler cannot determine the relationship between input and output lifetimes \
from context.

Concurrency in Rust builds on the same ownership primitives. The Send and \
Sync marker traits track which types can be transferred or shared across \
thread boundaries. Types that wrap non-thread-safe data are automatically not \
Send or Sync, and the compiler refuses to move them across threads. Arc and \
Mutex compose safely because their implementations are tied to these markers.

The trait system provides a form of polymorphism that compiles to direct \
dispatch in most cases. Trait objects with dynamic dispatch use vtables \
similar to C++ virtual functions but with an explicit syntax that makes the \
indirection visible. Zero-cost abstractions mean that idiomatic high-level \
code typically compiles to the same machine code as hand-written low-level \
code.

Rust's standard library provides a rich set of collections, I/O primitives, \
and concurrency tools. The cargo build tool handles dependency resolution, \
compilation, testing, and documentation in a single unified workflow. \
Integration with LLVM gives Rust access to aggressive optimisations and \
support for a wide variety of target architectures.";

const DOC_DISTRIBUTED: &str = "\
Distributed Systems: Consensus and Replication

A distributed system is a collection of independent computers that appear to \
users as a single coherent system. The fundamental challenge is achieving \
agreement among nodes that can fail or be partitioned by network failures. \
The CAP theorem states that a distributed data store can provide at most two \
of three guarantees: consistency, availability, and partition tolerance. \
Network partitions are unavoidable, so designers must choose between \
consistency and availability during a partition.

The Raft consensus algorithm was designed explicitly for understandability. A \
Raft cluster elects a single leader that handles all client requests. The \
leader appends entries to a replicated log and commits them once a majority \
of followers acknowledge receipt. Leadership is maintained by periodic \
heartbeats; followers that stop receiving heartbeats hold a randomised \
election timeout before calling a new election.

Paxos is the older and more widely studied consensus protocol. Classic Paxos \
achieves consensus on a single value through a two-phase prepare/accept \
protocol. Multi-Paxos extends this to a replicated log by reusing the prepare \
phase across many rounds after a stable leader is established. Paxos is \
notoriously difficult to implement correctly because the original paper leaves \
many practical details unspecified.

Replication strategies vary by consistency model. Synchronous replication \
waits for acknowledgement from all replicas before confirming a write, \
providing strong consistency at the cost of latency and availability. \
Asynchronous replication confirms writes immediately and propagates changes in \
the background, offering lower latency but risking data loss on leader \
failure. Quorum-based replication balances both by requiring acknowledgement \
from a majority of replicas.

Vector clocks track causal relationships between events across nodes without \
relying on synchronised physical clocks. A vector clock is a tuple of \
counters, one per node. When a node sends a message it increments its own \
counter and attaches the entire vector. The receiver merges the incoming \
vector by taking the component-wise maximum and then incrementing its own \
counter. Two events are causally related if one vector dominates the other.";

const DOC_ML: &str = "\
Deep Learning: Foundations and Modern Architectures

A neural network is a computational graph of parameterised linear \
transformations interleaved with non-linear activation functions. The \
universal approximation theorem guarantees that a sufficiently wide single \
hidden layer network can approximate any continuous function on a compact \
domain. In practice, depth is more parameter-efficient than width: deep \
narrow networks generalise better than shallow wide ones with the same total \
parameter count.

Training proceeds by computing a loss that measures disagreement between the \
network predictions and the ground-truth labels. Backpropagation applies the \
chain rule of calculus to propagate loss gradients back through every layer. \
Stochastic gradient descent and its variants — momentum, RMSProp, Adam — use \
these gradients to update parameters in directions that reduce the loss. \
Learning rate schedules warm up slowly, hold, and decay to help the optimiser \
escape sharp minima.

Convolutional neural networks exploit spatial locality and translation \
invariance. A convolutional layer applies a bank of learned filters across \
the input using shared weights, drastically reducing the number of parameters \
compared to a fully connected layer of the same receptive field. Pooling \
layers downsample feature maps. Residual connections bypass stacks of layers \
with identity shortcuts, allowing gradients to flow through very deep \
networks without vanishing.

Attention mechanisms compute a weighted average of value vectors, where the \
weights are derived from the similarity between query and key vectors. The \
transformer architecture replaces recurrence entirely with multi-head \
self-attention, enabling full parallelism over sequence positions during \
training. Positional encodings inject order information that self-attention \
otherwise ignores. Pre-trained transformer language models such as BERT and \
GPT learn powerful contextual representations that transfer across tasks.

Generative models learn the data distribution to produce new samples. \
Variational autoencoders encode inputs into a latent distribution and decode \
samples from it, optimising a lower bound on the log-likelihood. Generative \
adversarial networks pit a generator against a discriminator in a minimax \
game. Diffusion models corrupt data by adding noise over many steps and train \
a denoiser to reverse the process, achieving state-of-the-art image quality.";

// ── helpers ───────────────────────────────────────────────────────────────────

fn preview(s: &str, chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= chars {
        trimmed.to_string()
    } else {
        format!("{}…", trimmed.chars().take(chars).collect::<String>())
    }
}

fn chunk_text(store: &DocumentStorage, doc_meta: &serde_json::Value, idx: usize) -> String {
    let id_str = doc_meta["chunks"][idx].as_str().unwrap_or("");
    let id: Uuid = id_str.parse().unwrap();
    let bytes = store.get_content(id).unwrap().unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    // ── 1. Generate document files ────────────────────────────────────────────

    println!("=== 1. Generate document files ===\n");

    let doc_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;

    let files = [
        ("rust.txt",         "Rust: Memory Safety Without Garbage Collection", DOC_RUST),
        ("distributed.txt",  "Distributed Systems: Consensus and Replication",  DOC_DISTRIBUTED),
        ("ml.txt",           "Deep Learning: Foundations and Modern Architectures", DOC_ML),
    ];

    let mut file_paths: Vec<String> = Vec::new();
    for (filename, title, content) in &files {
        let path = doc_dir.path().join(filename);
        fs::write(&path, content).map_err(|e| err_msg(e.to_string()))?;
        let path_str = path.to_str().unwrap().to_string();
        println!("  {filename:<20} {:>5} chars   {title}", content.len());
        file_paths.push(path_str);
    }
    println!();

    // ── 2. Ingest with add_document_from_file ─────────────────────────────────

    println!("=== 2. Ingest with add_document_from_file ===\n");

    let store_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let store_path = store_dir.path().to_str().unwrap().to_string();
    let store = DocumentStorage::new(&store_path)?;

    println!("  {:<20}  {:<10}  {:<8}  id", "document", "slice", "overlap");
    println!("  {}", "-".repeat(80));

    let settings = [
        (300usize, 20.0f32),   // rust
        (250usize, 15.0f32),   // distributed
        (280usize, 25.0f32),   // ml
    ];

    let mut doc_ids: Vec<Uuid> = Vec::new();
    for ((path, (filename, title, _)), (slice, overlap)) in
        file_paths.iter().zip(files.iter()).zip(settings.iter())
    {
        let _ = title;
        let doc_id = store.add_document_from_file(path, files[doc_ids.len()].1, *slice, *overlap)?;
        let meta = store.get_metadata(doc_id)?.unwrap();
        println!(
            "  {filename:<20}  slice={slice:<4}  overlap={overlap:<4}  id={doc_id}  n_chunks={}",
            meta["n_chunks"]
        );
        doc_ids.push(doc_id);
    }
    println!();

    let (rust_id, dist_id, ml_id) = (doc_ids[0], doc_ids[1], doc_ids[2]);

    // ── 3. Document-level metadata ────────────────────────────────────────────

    println!("=== 3. Document-level metadata ===\n");

    let rust_meta = store.get_metadata(rust_id)?.unwrap();
    let n_chunks  = rust_meta["n_chunks"].as_u64().unwrap_or(0) as usize;

    println!("  document : {}", rust_meta["name"]);
    println!("  path     : {}", rust_meta["path"]);
    println!("  slice    : {}  overlap: {}", rust_meta["slice"], rust_meta["overlap"]);
    println!("  n_chunks : {n_chunks}");
    println!();

    let chunks = rust_meta["chunks"].as_array().unwrap();
    let show_n = chunks.len().min(3);
    for i in 0..show_n {
        println!("  chunks[{i}] : {}", chunks[i].as_str().unwrap_or("?"));
    }
    if chunks.len() > show_n {
        println!("  ...");
        println!(
            "  chunks[{}]: {}",
            chunks.len() - 1,
            chunks.last().and_then(|v| v.as_str()).unwrap_or("?")
        );
    }
    println!();

    // ── 4. Per-chunk inspection ───────────────────────────────────────────────

    println!("=== 4. Per-chunk inspection ===\n");

    for i in 0..n_chunks.min(3) {
        let chunk_id: Uuid = chunks[i].as_str().unwrap().parse().unwrap();
        let chunk_meta = store.get_metadata(chunk_id)?.unwrap();
        let chunk_bytes = store.get_content(chunk_id)?.unwrap();
        let chunk_text  = String::from_utf8_lossy(&chunk_bytes);

        println!("  chunks[{i}]  id={chunk_id}");
        println!(
            "    document_name  : {}",
            chunk_meta["document_name"].as_str().unwrap_or("?")
        );
        println!("    document_id    : {}", chunk_meta["document_id"].as_str().unwrap_or("?"));
        println!(
            "    chunk_index    : {}  /  n_chunks: {}",
            chunk_meta["chunk_index"], chunk_meta["n_chunks"]
        );
        println!("    content preview: {}", preview(&chunk_text, 90));
        println!();
    }

    // ── 5. Overlap: adjacent chunks share content at their boundary ───────────

    println!("=== 5. Overlap evidence ===\n");

    if n_chunks >= 3 {
        let a_text = chunk_text(&store, &rust_meta, 1);
        let b_text = chunk_text(&store, &rust_meta, 2);

        // Find the longest suffix of a that is a prefix of b (word-level).
        let a_words: Vec<&str> = a_text.split_whitespace().collect();
        let b_words: Vec<&str> = b_text.split_whitespace().collect();
        let mut shared_words = 0usize;
        'outer: for window in 1..=a_words.len().min(20) {
            let suffix = &a_words[a_words.len() - window..];
            if b_words.starts_with(suffix) {
                shared_words = window;
            } else if shared_words > 0 {
                break 'outer;
            }
        }

        println!("  Comparing chunks[1] and chunks[2] (Rust document, overlap=20 %):\n");
        println!("  tail of chunks[1]  (last 80 chars): \"{}\"", {
            let s = a_text.trim();
            let n = s.len();
            &s[n.saturating_sub(80)..]
        });
        println!("  head of chunks[2]  (first 80 chars): \"{}\"", {
            let s = b_text.trim();
            &s[..s.len().min(80)]
        });
        if shared_words > 0 {
            let shared: Vec<&str> = a_words[a_words.len() - shared_words..].to_vec();
            println!("\n  shared word(s) at boundary: {:?}", shared.join(" "));
        }
    }
    println!();

    // ── 6. RAG retrieval — context window expansion ───────────────────────────

    println!("=== 6. RAG retrieval — context window expansion ===\n");

    // Simulate a retrieval hit on the middle chunk of the Rust document.
    let hit_index = n_chunks / 2;
    let hit_id: Uuid = chunks[hit_index].as_str().unwrap().parse().unwrap();
    let hit_meta = store.get_metadata(hit_id)?.unwrap();

    println!("  Simulated retrieval hit:");
    println!("    chunk UUID       : {hit_id}");
    println!(
        "    document_name    : {}",
        hit_meta["document_name"].as_str().unwrap_or("?")
    );
    println!(
        "    chunk_index      : {}  /  n_chunks: {}",
        hit_meta["chunk_index"], hit_meta["n_chunks"]
    );
    println!("    content preview  : {}", preview(&chunk_text(&store, &rust_meta, hit_index), 80));
    println!();

    // Step 1: extract document_id from chunk metadata
    let doc_id_str = hit_meta["document_id"].as_str().unwrap();
    let doc_id: Uuid = doc_id_str.parse().unwrap();
    println!("  Step 1 — extract document_id from chunk metadata: {doc_id_str}");

    // Step 2: load document-level metadata to get the ordered chunks list
    let doc_meta = store.get_metadata(doc_id)?.unwrap();
    println!("  Step 2 — load document metadata: name={}", doc_meta["name"]);
    println!(
        "           n_chunks={}, chunks list has {} entries",
        doc_meta["n_chunks"],
        doc_meta["chunks"].as_array().map_or(0, |a| a.len())
    );

    // Step 3: fetch neighbouring chunks (index ±1) to expand the context window
    let doc_chunks = doc_meta["chunks"].as_array().unwrap();
    let lo = hit_index.saturating_sub(1);
    let hi = (hit_index + 1).min(doc_chunks.len() - 1);
    println!("  Step 3 — expand context window: chunks[{lo}..={hi}]\n");

    let mut context = String::new();
    for idx in lo..=hi {
        let text = chunk_text(&store, &doc_meta, idx);
        context.push_str(text.trim());
        context.push(' ');
    }
    let context = context.trim().to_string();
    println!("  Assembled context ({} chars):", context.len());
    println!("  \"{}\"", preview(&context, 280));
    println!();

    // ── 7. Semantic search with EmbeddingEngine ───────────────────────────────
    //
    // Requires network access on first run to download AllMiniLML6V2 (~23 MB).
    // add_document_from_file embeds each chunk automatically: ":content" from
    // chunk text, ":meta" from the json_fingerprint of per-chunk metadata.
    // search_document_text embeds the query string on the fly and searches both
    // slots across all chunks, returning chunk-level results with full metadata
    // and raw content in each hit.

    println!("=== 7. Semantic search with EmbeddingEngine ===\n");
    println!("Loading AllMiniLML6V2 embedding model...");

    let emb_dir   = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let emb_root  = emb_dir.path().to_str().unwrap();
    let engine    = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
    let emb_store = DocumentStorage::with_embedding(emb_root, engine)?;
    println!("Model loaded. DocumentStorage with embedding ready.\n");

    println!("Ingesting three documents:");
    let mut emb_doc_ids: Vec<Uuid> = Vec::new();
    for ((path, (filename, title, _)), (slice, overlap)) in
        file_paths.iter().zip(files.iter()).zip(settings.iter())
    {
        let doc_id = emb_store.add_document_from_file(path, title, *slice, *overlap)?;
        let meta   = emb_store.get_metadata(doc_id)?.unwrap();
        println!(
            "  {filename:<20}  n_chunks={}  doc_id={doc_id}",
            meta["n_chunks"]
        );
        emb_doc_ids.push(doc_id);
    }
    println!();

    // ── Freeform text query — each result is a chunk ──────────────────────────

    let query = "memory safe concurrent systems programming";
    println!("search_document_text(\"{query}\", limit=4)");
    println!(
        "  {:<7}  {:<8}  {:<24}  {}",
        "score", "chunk", "document", "content"
    );
    println!("  {}", "-".repeat(88));

    let results = emb_store.search_document_text(query, 4)?;
    let top_result = results.first().cloned();

    for r in &results {
        let score       = r["score"].as_f64().unwrap_or(0.0);
        let doc_name    = r["metadata"]["document_name"].as_str().unwrap_or("?");
        let chunk_index = r["metadata"]["chunk_index"].as_u64().unwrap_or(0);
        let n_ch        = r["metadata"]["n_chunks"].as_u64().unwrap_or(0);
        let content     = r["document"].as_str().unwrap_or("");
        println!(
            "  [{score:.3}]  [{chunk_index}/{n_ch}]   {doc_name:<24}  {}",
            preview(content, 48)
        );
    }
    println!();

    // ── RAG context expansion from a search hit ───────────────────────────────

    if let Some(top) = top_result {
        println!("RAG expansion from the top search result:\n");

        let hit_chunk_index = top["metadata"]["chunk_index"].as_u64().unwrap_or(0) as usize;
        let hit_doc_id_str  = top["metadata"]["document_id"].as_str().unwrap_or("");
        let hit_doc_id: Uuid = hit_doc_id_str.parse().unwrap();

        println!("  hit   chunk_index={hit_chunk_index}");
        println!("  hit   document_name={}", top["metadata"]["document_name"].as_str().unwrap_or("?"));
        println!("  hit   document_id={hit_doc_id_str}");
        println!();

        let doc_meta   = emb_store.get_metadata(hit_doc_id)?.unwrap();
        let doc_chunks = doc_meta["chunks"].as_array().unwrap();
        let lo = hit_chunk_index.saturating_sub(1);
        let hi = (hit_chunk_index + 1).min(doc_chunks.len() - 1);

        println!("  Expanding to context window chunks[{lo}..={hi}]:\n");

        let mut rag_context = String::new();
        for idx in lo..=hi {
            let chunk_id: Uuid = doc_chunks[idx].as_str().unwrap().parse().unwrap();
            let bytes = emb_store.get_content(chunk_id)?.unwrap_or_default();
            let text  = String::from_utf8_lossy(&bytes);
            rag_context.push_str(text.trim());
            rag_context.push(' ');
            println!(
                "  chunks[{idx}]: {}",
                preview(text.trim(), 80)
            );
        }
        println!("\n  Full assembled context ({} chars):", rag_context.trim().len());
        println!("  \"{}\"", preview(rag_context.trim(), 300));
        println!();
    }

    // ── 8. Fingerprinted output: search_document_text_strings ─────────────────

    println!("=== 8. Fingerprinted output (search_document_text_strings) ===\n");

    let query2 = "consensus replication fault tolerance distributed";
    println!("search_document_text_strings(\"{query2}\", limit=3)");
    println!("Each result is a json_fingerprint string: path: value …\n");

    let str_results = emb_store.search_document_text_strings(query2, 3)?;
    for (i, s) in str_results.iter().enumerate() {
        // Print up to 120 chars per line so the output stays readable.
        println!("  [{i}] {}", preview(s, 120));
    }
    println!();

    // ── 9. sync + reopen ─────────────────────────────────────────────────────

    println!("=== 9. sync + reopen ===\n");

    emb_store.sync()?;
    println!("sync() done");

    // Drop the store, then reopen from the same path.
    drop(emb_store);
    println!("embedding store dropped");

    // Reopen with a fresh engine so search_document_text is available.
    // The model is already cached locally from section 7.
    let engine2       = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
    let store_reopened = DocumentStorage::with_embedding(emb_root, engine2)?;
    println!("store reopened (with_embedding) from {emb_root}");

    // Verify the first chunk of the Rust document is still accessible.
    let rust_emb_meta = store_reopened.get_metadata(emb_doc_ids[0])?.unwrap();
    let emb_chunks    = rust_emb_meta["chunks"].as_array().unwrap();
    let first_chunk_id: Uuid = emb_chunks[0].as_str().unwrap().parse().unwrap();
    let first_chunk_bytes = store_reopened.get_content(first_chunk_id)?;
    println!(
        "first chunk of Rust doc accessible after reopen: {}",
        first_chunk_bytes.is_some()
    );
    if let Some(bytes) = first_chunk_bytes {
        println!(
            "  content preview: {}",
            preview(&String::from_utf8_lossy(&bytes), 80)
        );
    }

    // Confirm vector search still returns results (HNSW index survives restart).
    let reopen_results = store_reopened.search_document_text("ownership borrowing lifetimes", 2)?;
    println!("\nvector search after reopen ({} results):", reopen_results.len());
    for r in &reopen_results {
        println!(
            "  [{:.3}]  chunk={}/{}  {}",
            r["score"].as_f64().unwrap_or(0.0),
            r["metadata"]["chunk_index"],
            r["metadata"]["n_chunks"],
            r["metadata"]["document_name"].as_str().unwrap_or("?")
        );
    }
    println!();

    // Store document counts summary.
    println!("Summary:");
    let all_ids = [rust_id, dist_id, ml_id];
    let mut total_chunks = 0usize;
    for id in &all_ids {
        let meta = store.get_metadata(*id)?.unwrap();
        let n    = meta["n_chunks"].as_u64().unwrap_or(0) as usize;
        total_chunks += n;
        println!(
            "  {}  n_chunks={}",
            meta["name"].as_str().unwrap_or("?"),
            n
        );
    }
    println!("  total chunks stored: {total_chunks}");
    println!();

    Ok(())
}

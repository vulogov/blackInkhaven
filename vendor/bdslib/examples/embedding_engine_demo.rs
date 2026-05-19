use bdslib::common::error::Result;
use bdslib::embedding::Model;
use bdslib::EmbeddingEngine;

fn main() -> Result<()> {
    println!("Loading AllMiniLML6V2 (384-dim)...");
    let engine = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
    println!("EmbeddingEngine ready\n");

    // --- embed a single string ---
    let text = "Rust is a systems programming language focused on safety";
    let vector = engine.embed(text)?;
    println!("embed(\"{text}\")");
    println!("  dimension : {}", vector.len());
    println!("  first 6   : {:?}", &vector[..6]);
    println!();

    // --- compare two similar sentences ---
    let pairs = [
        (
            "the cat sat on the mat",
            "a cat rested on a rug",
            "similar",
        ),
        (
            "full-text search enables fast keyword lookup",
            "Tantivy is a search engine library written in Rust",
            "related",
        ),
        (
            "the cat sat on the mat",
            "quantum chromodynamics describes the strong nuclear force",
            "unrelated",
        ),
    ];

    println!("-- compare_texts --");
    for (a, b, label) in &pairs {
        let sim = engine.compare_texts(a, b)?;
        println!("  [{label:8}]  {sim:+.4}");
        println!("    A: \"{a}\"");
        println!("    B: \"{b}\"");
    }
    println!();

    // --- compare pre-computed embeddings ---
    println!("-- compare_embeddings (pre-computed) --");
    let sentences = [
        "Memory safety in Rust is enforced at compile time",
        "Rust prevents data races and dangling pointers",
        "DuckDB is an in-process analytical SQL database",
    ];
    let embeddings: Vec<_> = sentences
        .iter()
        .map(|s| engine.embed(s))
        .collect::<Result<_>>()?;

    for i in 0..sentences.len() {
        for j in (i + 1)..sentences.len() {
            let sim = EmbeddingEngine::compare_embeddings(&embeddings[i], &embeddings[j])?;
            println!("  sim({i},{j}) = {sim:+.4}");
            println!("    [{i}] \"{}\"", sentences[i]);
            println!("    [{j}] \"{}\"", sentences[j]);
        }
    }
    println!();

    // --- nearest neighbour over a small corpus ---
    println!("-- nearest-neighbour search --");
    let corpus = [
        "Rust ownership model prevents memory leaks",
        "Fast full-text search with inverted indices",
        "Vector embeddings encode semantic meaning",
        "SQL databases store structured tabular data",
        "Garbage collection pauses cause latency spikes",
    ];
    let corpus_embeddings: Vec<_> = corpus
        .iter()
        .map(|s| engine.embed(s))
        .collect::<Result<_>>()?;

    let query = "semantic similarity via dense vectors";
    let query_emb = engine.embed(query)?;
    println!("  query: \"{query}\"");

    let mut scored: Vec<(f32, &str)> = corpus_embeddings
        .iter()
        .zip(corpus.iter())
        .map(|(emb, text)| -> Result<(f32, &str)> {
            let sim = EmbeddingEngine::compare_embeddings(&query_emb, emb)?;
            Ok((sim, *text))
        })
        .collect::<Result<_>>()?;
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    for (rank, (sim, text)) in scored.iter().enumerate() {
        println!("  #{} {sim:+.4}  \"{text}\"", rank + 1);
    }
    println!();

    // --- clone shares the model ---
    println!("-- clone shares underlying model --");
    let clone = engine.clone();
    let e1 = engine.embed("shared model test")?;
    let e2 = clone.embed("shared model test")?;
    let sim = EmbeddingEngine::compare_embeddings(&e1, &e2)?;
    println!("  original vs clone similarity: {sim:+.4}  (expected ≈ 1.0)");

    Ok(())
}

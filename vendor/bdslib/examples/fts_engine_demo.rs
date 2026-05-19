use bdslib::common::error::Result;
use bdslib::FTSEngine;

fn main() -> Result<()> {
    let engine = FTSEngine::new(":memory:")?;
    println!("FTSEngine ready (in-memory)");

    // --- index documents ---
    let docs = [
        "Rust is a systems programming language focused on safety and performance",
        "Tantivy is a full-text search engine library written in Rust",
        "DuckDB is an in-process analytical database with SQL support",
        "Full-text search enables fast keyword lookup across large document sets",
        "Memory safety in Rust is enforced at compile time without a garbage collector",
    ];

    let mut ids = Vec::new();
    for text in &docs {
        let id = engine.add_document(text)?;
        ids.push(id);
        println!("  indexed [{id}]  \"{}...\"", &text[..40]);
    }
    println!();

    // --- basic term search ---
    println!("-- search: \"Rust\" --");
    for id in engine.search("Rust", 10)? {
        println!("  {id}");
    }

    // --- boolean AND ---
    println!("\n-- search: \"Rust AND safety\" --");
    for id in engine.search("Rust AND safety", 10)? {
        println!("  {id}");
    }

    // --- phrase search ---
    println!("\n-- search: phrase \"full-text search\" --");
    for id in engine.search("\"full-text search\"", 10)? {
        println!("  {id}");
    }

    // --- limit ---
    println!("\n-- search: \"Rust\" with limit=1 --");
    let top1 = engine.search("Rust", 1)?;
    println!("  returned {} result(s)", top1.len());

    // --- drop a document then confirm it is gone ---
    let dropped_id = ids[1]; // "Tantivy is a full-text search..."
    println!("\n-- dropping document {dropped_id} --");
    engine.drop_document(dropped_id)?;

    let after_drop = engine.search("Tantivy", 10)?;
    if after_drop.is_empty() {
        println!("  search for \"Tantivy\" returns no results (document removed)");
    }

    // --- drop non-existent UUID is silent ---
    let phantom = uuid::Uuid::now_v7();
    engine.drop_document(phantom)?;
    println!("  drop of unknown UUID succeeded silently");

    // --- UUIDv7 ordering ---
    println!("\n-- UUIDv7 insertion order --");
    let sorted = {
        let mut v = ids.clone();
        v.sort();
        v
    };
    let monotonic = ids.windows(2).all(|w| w[0] < w[1]);
    println!("  IDs monotonically increasing: {monotonic}");
    println!("  earliest: {}", sorted.first().unwrap());
    println!("  latest:   {}", sorted.last().unwrap());

    // --- sync ---
    engine.sync()?;
    println!("\nSync complete.");

    Ok(())
}

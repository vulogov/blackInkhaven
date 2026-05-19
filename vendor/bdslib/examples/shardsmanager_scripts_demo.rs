/// shardsmanager_scripts_demo — BUND script registry on `ShardsManager`.
///
/// Walks through the public API of the script store:
///
///   - `script_add(metadata, script)`   create
///   - `scripts()`                       list (id, schedule)
///   - `script(id)`                      fetch body
///   - `update_script(id, meta, body)`   update
///   - `script_delete(id)`               delete
///
/// Run with:
///
/// ```bash
/// cargo run --example shardsmanager_scripts_demo
/// ```

use bdslib::{EmbeddingEngine, ShardsManager};
use fastembed::EmbeddingModel;
use serde_json::json;
use tempfile::TempDir;

fn print_section(title: &str) {
    println!("\n=== {title} ===");
}

fn main() {
    let dir = TempDir::new().expect("tempdir");
    let config_path = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    std::fs::write(
        &config_path,
        format!(
            "{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n  similarity_threshold: 0.99\n}}"
        ),
    )
    .unwrap();

    let engine = EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None)
        .expect("embedding engine");
    let mgr = ShardsManager::with_embedding(config_path.to_str().unwrap(), engine)
        .expect("ShardsManager");

    // ── 1. Add a few scripts ──────────────────────────────────────────────────

    print_section("1. script_add");

    let id_hello = mgr.script_add(
        json!({
            "name": "hello",
            "schedule": "*/5 * * * *",
            "owner": "demo",
            "tags": ["greeting"],
        }),
        "// say hello\n\"hello\" println.",
    ).unwrap();
    println!("added 'hello' → {id_hello}");

    let id_report = mgr.script_add(
        json!({
            "name": "daily_report",
            "schedule": "0 9 * * *",
            "owner": "ops",
        }),
        "// daily ops report\ntime.now println.",
    ).unwrap();
    println!("added 'daily_report' → {id_report}");

    let id_cleanup = mgr.script_add(
        json!({
            "name": "cleanup",
            "schedule": "0 0 * * 0",
        }),
        "// weekly cleanup\n0 println.",
    ).unwrap();
    println!("added 'cleanup' → {id_cleanup}");

    // ── 2. List scripts ───────────────────────────────────────────────────────

    print_section("2. scripts() — (id, schedule)");
    for (id, schedule) in mgr.scripts().unwrap() {
        println!("  {id}  schedule={schedule}");
    }

    print_section("2b. scripts_with_metadata() — full metadata for UIs");
    for (id, meta) in mgr.scripts_with_metadata().unwrap() {
        let name = meta.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let sched = meta.get("schedule").and_then(|v| v.as_str()).unwrap_or("?");
        println!("  {id}  name={name}  schedule={sched}");
    }

    // ── 3. Fetch a script body ────────────────────────────────────────────────

    print_section("3. script(id)");
    let body = mgr.script(id_hello).unwrap().expect("hello body");
    println!("body of 'hello':\n{body}");

    let missing = uuid::Uuid::now_v7();
    println!("script({missing}) (missing) → {:?}", mgr.script(missing).unwrap());

    // ── 4. Update a script ────────────────────────────────────────────────────

    print_section("4. update_script");
    mgr.update_script(
        id_hello,
        json!({
            "name": "hello",
            "schedule": "*/10 * * * *",
            "owner": "demo",
            "version": 2,
        }),
        "// hello v2\n\"hello v2\" println.",
    ).unwrap();
    println!("after update body:\n{}", mgr.script(id_hello).unwrap().unwrap());
    let meta = mgr.script_metadata(id_hello).unwrap().unwrap();
    println!("after update schedule: {}", meta["schedule"]);
    println!("after update version:  {}", meta["version"]);

    // ── 5. Delete a script ────────────────────────────────────────────────────

    print_section("5. script_delete");
    mgr.script_delete(id_cleanup).unwrap();
    println!("after delete:");
    for (id, sched) in mgr.scripts().unwrap() {
        println!("  {id}  schedule={sched}");
    }

    // ── 6. Validation errors ──────────────────────────────────────────────────

    print_section("6. validation errors");
    match mgr.script_add(json!({ "name": "no-schedule" }), "noop") {
        Ok(_)  => println!("  unexpected: should have failed"),
        Err(e) => println!("  missing schedule  → {e}"),
    }
    match mgr.script_add(json!({ "schedule": "*/5 * * * *" }), "noop") {
        Ok(_)  => println!("  unexpected: should have failed"),
        Err(e) => println!("  missing name      → {e}"),
    }
    match mgr.script_add(json!("not an object"), "noop") {
        Ok(_)  => println!("  unexpected: should have failed"),
        Err(e) => println!("  non-object        → {e}"),
    }

    println!("\nDone.");
}

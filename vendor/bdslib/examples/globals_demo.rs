/// globals_demo — end-to-end walkthrough of the process-wide `DB` singleton.
///
/// Sections:
///   1. Initialisation  — init_db() with an explicit config path
///   2. Telemetry batch — generate 80 metric documents, ingest via get_db()
///   3. Log-entry batch — generate 60 syslog / HTTP / traceback docs, ingest
///   4. Templated batch — IoT sensor readings via Generator::templated()
///   5. Catalog         — inspect shards registered in the catalog
///   6. FTS search      — keyword search across the ingested window
///   7. Vector search   — semantic search across the ingested window
///   8. sync_db()       — flush all shards to disk
///   9. Singleton proof — helper that uses get_db() with no DB parameter
use bdslib::common::generator::Generator;
use bdslib::{get_db, init_db, sync_db};
use serde_json::{json, Value};
use std::fs;

// ── display helpers ───────────────────────────────────────────────────────────

fn section(title: &str) {
    println!("\n{}", "─".repeat(70));
    println!("  {title}");
    println!("{}", "─".repeat(70));
}

fn subsection(title: &str) {
    println!("\n  ▸ {title}");
}

fn show_doc(label: &str, doc: &Value) {
    let pretty = serde_json::to_string_pretty(doc).unwrap();
    let indented = pretty
        .lines()
        .enumerate()
        .map(|(i, l)| if i == 0 { l.to_string() } else { format!("    {l}") })
        .collect::<Vec<_>>()
        .join("\n");
    println!("  [{label}] {indented}");
}

// ── singleton helper ──────────────────────────────────────────────────────────

/// Returns the number of documents retrievable from the global DB.
/// Demonstrates that any code can reach the DB without passing it as a parameter.
fn total_cached_shards() -> usize {
    match get_db() {
        Ok(db) => db
            .cache()
            .info()
            .list_all()
            .map(|v| v.len())
            .unwrap_or(0),
        Err(_) => 0,
    }
}

// ── config writer ─────────────────────────────────────────────────────────────

fn write_config(dir: &tempfile::TempDir) -> String {
    let db_path = dir.path().join("db");
    fs::create_dir_all(&db_path).unwrap();
    let config_path = dir.path().join("bds.hjson");
    fs::write(
        &config_path,
        format!(
            "{{\n  dbpath: \"{}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n}}\n",
            db_path.display()
        ),
    )
    .unwrap();
    config_path.to_str().unwrap().to_string()
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── 1. Initialisation ─────────────────────────────────────────────────────
    section("1. Initialisation  (init_db)");

    let tmp = tempfile::TempDir::new()?;
    let config_path = write_config(&tmp);
    println!("  config : {config_path}");

    // Calling get_db() before init_db() returns a descriptive error.
    match get_db() {
        Err(e) => println!("  get_db() before init: {e}"),
        Ok(_)  => println!("  get_db() before init: (already initialized)"),
    }

    // init_db(None) would look for BDS_CONFIG; here we supply the path directly.
    init_db(Some(&config_path))?;
    println!("  init_db(Some(path)) → OK");

    // A second call must fail — the singleton can only be set once per process.
    match init_db(Some(&config_path)) {
        Err(e) => println!("  init_db() again     → {e}"),
        Ok(())  => println!("  init_db() again     → (unexpected Ok)"),
    }

    // ── 2. Telemetry batch ────────────────────────────────────────────────────
    section("2. Telemetry batch  (Generator::telemetry → add_batch)");

    let g = Generator::new();
    let telemetry = g.telemetry("2h", 80);
    println!("  generated {} metric documents", telemetry.len());

    let uuids = get_db()?.add_batch(telemetry)?;
    println!("  stored   {} documents  (first id: {})", uuids.len(), uuids[0]);

    // ── 3. Log-entry batch ────────────────────────────────────────────────────
    section("3. Log-entry batch  (Generator::log_entries → add_batch)");

    let logs = g.log_entries("2h", 60);
    let syslog_n = logs.iter().filter(|d| d["data"].get("pid").is_some()).count();
    let http_n   = logs.iter().filter(|d| d["data"].get("method").is_some()).count();
    let tb_n     = logs.iter().filter(|d| d["data"].get("exception_type").is_some()).count();
    println!("  generated {} log documents:", logs.len());
    println!("    {syslog_n:3}  syslog");
    println!("    {http_n:3}  HTTP access-log");
    println!("    {tb_n:3}  Python traceback");

    let log_uuids = get_db()?.add_batch(logs)?;
    println!("  stored {} log documents", log_uuids.len());

    // ── 4. Templated batch ────────────────────────────────────────────────────
    section("4. Templated batch  (Generator::templated → add_batch)");

    let iot_template = r#"{
        "timestamp": "$timestamp",
        "key":       "$choice(sensor.temperature,sensor.humidity,sensor.pressure,sensor.co2)",
        "data": {
            "value":     "$float(0.0,100.0)",
            "unit":      "$choice(celsius,percent,hpa,ppm)",
            "device_id": "$choice(sensor-01,sensor-02,sensor-03,sensor-04)",
            "building":  "$choice(HQ,Annex,Warehouse)",
            "floor":     "$int(1,5)",
            "alert":     "$bool",
            "reading_id":"$uuid"
        }
    }"#;

    let iot_docs = g.templated("1h", iot_template, 40);
    println!("  generated {} IoT sensor readings", iot_docs.len());
    subsection("Sample IoT document");
    show_doc("iot", &iot_docs[0]);

    let iot_uuids = get_db()?.add_batch(iot_docs)?;
    println!("\n  stored {} IoT documents", iot_uuids.len());

    // ── 5. Catalog ────────────────────────────────────────────────────────────
    section("5. Catalog  (ShardsCache::info)");

    let shards = get_db()?.cache().info().list_all()?;
    println!("  registered shards: {}", shards.len());
    for info in &shards {
        println!(
            "    start={:<12}  path={}",
            info.start_time
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            info.path,
        );
    }

    // ── 6. FTS search ─────────────────────────────────────────────────────────
    section("6. Full-text search  (ShardsManager::search_fts)");

    for query in ["sshd", "GET", "Error"] {
        let hits = get_db()?.search_fts("3h", query)?;
        println!("  query={query:8}  hits={}", hits.len());
        if let Some(doc) = hits.first() {
            let key = doc["key"].as_str().unwrap_or("?");
            println!("    top hit key: {key}");
        }
    }

    // ── 7. Vector search ──────────────────────────────────────────────────────
    section("7. Vector search  (ShardsManager::search_vector)");

    subsection("Semantic query: CPU overloaded");
    let cpu_query = json!({
        "key": "cpu.usage",
        "data": "cpu is fully saturated and overloaded"
    });
    let cpu_hits = get_db()?.search_vector("3h", &cpu_query)?;
    println!("  hits: {}", cpu_hits.len());
    for doc in cpu_hits.iter().take(3) {
        let key   = doc["key"].as_str().unwrap_or("?");
        let score = doc.get("_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        println!("    score={score:.4}  key={key}");
    }

    subsection("Semantic query: network packet loss");
    let net_query = json!({
        "key":  "net.packets_dropped",
        "data": "network interface dropping packets, high packet loss"
    });
    let net_hits = get_db()?.search_vector("3h", &net_query)?;
    println!("  hits: {}", net_hits.len());
    for doc in net_hits.iter().take(3) {
        let key   = doc["key"].as_str().unwrap_or("?");
        let score = doc.get("_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        println!("    score={score:.4}  key={key}");
    }

    // ── 8. sync_db() ──────────────────────────────────────────────────────────
    section("8. sync_db()  (flush all open shards)");

    sync_db()?;
    println!("  sync_db() → OK  (all WAL data flushed to disk)");

    // ── 9. Singleton proof ────────────────────────────────────────────────────
    section("9. Singleton proof  (get_db() from a standalone helper)");

    let shard_count = total_cached_shards();
    println!("  total_cached_shards() → {shard_count}  (called with no DB parameter)");
    println!("  get_db() resolves the same OnceLock<ShardsManager> from any call site.");

    println!();
    Ok(())
}

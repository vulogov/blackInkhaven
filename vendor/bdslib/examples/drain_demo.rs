/// drain_demo — Automatic log-template discovery via drain3 inside ShardsManager.
///
/// 108 synthetic log entries are ingested across 9 structural template families.
/// Each family uses 8–15 distinct variable values so the drain3 similarity
/// search reliably converges to the canonical template (e.g.
/// "user <*> logged in from <*>") after the second distinct value at a variable
/// position.
///
/// Sections:
///   1. Setup   — temp hjson config with drain_enabled = true
///   2. Ingest  — 108 docs via add() and add_batch()
///   3. Discover — list every discovered template, sorted by cluster id
///   4. Search  — semantic search over the template store
///   5. Reload  — drain_load() seeds a fresh parser; parse an unseen line
use bdslib::common::error::Result;
use bdslib::common::time::now_secs;
use bdslib::embedding::Model;
use bdslib::shardsmanager::ShardsManager;
use bdslib::EmbeddingEngine;
use serde_json::{json, Value};
use tempfile::TempDir;

fn ts_ago(secs: u64) -> u64 {
    now_secs().saturating_sub(secs)
}

fn doc(key: &str, data: &str, timestamp: u64) -> Value {
    json!({ "key": key, "data": data, "timestamp": timestamp })
}

/// Build 108 log lines covering 9 structural templates.
///
/// Variable slots use enough distinct values that drain3 collapses each family
/// into a single wildcard template after the first few observations.
fn generate_logs(base_ts: u64) -> Vec<Value> {
    let mut logs: Vec<Value> = Vec::new();
    let step = 20u64;
    macro_rules! ts { () => { base_ts + logs.len() as u64 * step } }

    // ── Template 1: "user <*> logged in from <*>"
    // 12 distinct usernames × 1 line each
    for name in ["alice","bob","carol","dave","eve","frank","grace","hank","ivan","judy","karl","lena"] {
        let ip = format!("10.0.0.{}", logs.len() % 254 + 1);
        logs.push(doc("auth", &format!("user {name} logged in from {ip}"), ts!()));
    }

    // ── Template 2: "connection to <*> on port <*> established"
    // 10 distinct host:port combinations
    for (host, port) in [
        ("db-primary", 5432u16), ("db-replica-1", 5432), ("db-replica-2", 5432),
        ("cache-a", 6379), ("cache-b", 6379), ("cache-c", 6379),
        ("queue-1", 5672), ("queue-2", 5672),
        ("search-1", 9200), ("search-2", 9200),
    ] {
        logs.push(doc("network", &format!("connection to {host} on port {port} established"), ts!()));
    }

    // ── Template 3: "HTTP <*> <*> returned <*> in <*> ms"
    // 12 request variants
    for (method, path, status, ms) in [
        ("GET",  "/api/health",          200u16, 4u32),
        ("GET",  "/api/v1/users",        200,    23),
        ("POST", "/api/v1/users",        201,    45),
        ("GET",  "/api/v1/records",      200,    67),
        ("POST", "/api/v1/records",      201,    88),
        ("DELETE","/api/v1/records/123", 204,    12),
        ("GET",  "/api/v1/search",       200,    134),
        ("GET",  "/api/health",          200,    5),
        ("GET",  "/api/v1/users",        200,    19),
        ("POST", "/api/v1/ingest",       202,    9),
        ("GET",  "/api/v1/metrics",      200,    31),
        ("PUT",  "/api/v1/config",       200,    7),
    ] {
        logs.push(doc("http", &format!("HTTP {method} {path} returned {status} in {ms} ms"), ts!()));
    }

    // ── Template 4: "disk <*> usage <*>% on volume <*>"
    // 10 variants
    for (label, pct, vol) in [
        ("read",  72u8, "/dev/sda1"), ("write", 45,  "/dev/sda1"),
        ("read",  80,   "/dev/sdb1"), ("write", 60,  "/dev/sdb1"),
        ("read",  55,   "/dev/sdc1"), ("write", 30,  "/dev/sdc1"),
        ("read",  90,   "/dev/sdd1"), ("write", 70,  "/dev/sdd1"),
        ("read",  62,   "/dev/sde1"), ("write", 48,  "/dev/sde1"),
    ] {
        logs.push(doc("disk", &format!("disk {label} usage {pct}% on volume {vol}"), ts!()));
    }

    // ── Template 5: "worker <*> picked up job <*> from queue <*>"
    // 12 variants
    for (wid, jid, queue) in [
        (1u8, 1001u32, "ingest"), (2, 1002, "ingest"), (3, 1003, "ingest"),
        (4, 1004, "search"), (5, 1005, "search"), (1, 1006, "ingest"),
        (2, 1007, "ingest"), (3, 1008, "search"), (4, 1009, "ingest"),
        (5, 1010, "search"), (1, 1011, "ingest"), (2, 1012, "search"),
    ] {
        logs.push(doc("worker", &format!("worker {wid} picked up job {jid} from queue {queue}"), ts!()));
    }

    // ── Template 6: "service <*> restarted after <*> seconds"
    // 8 variants
    for (svc, secs) in [
        ("api-gateway", 3u32), ("auth-service", 5), ("ingest-worker", 2),
        ("search-node", 4), ("api-gateway", 3), ("cache-proxy", 6),
        ("auth-service", 5), ("queue-consumer", 1),
    ] {
        logs.push(doc("ops", &format!("service {svc} restarted after {secs} seconds"), ts!()));
    }

    // ── Template 7: "error <*> in module <*>: <*>"
    // 10 variants — note the colon makes the last part a single token
    for (code, module, msg) in [
        (500u16, "auth",    "token_expired"),
        (503,    "ingest",  "upstream_unavailable"),
        (429,    "api",     "rate_limit_exceeded"),
        (500,    "search",  "index_unavailable"),
        (503,    "queue",   "broker_timeout"),
        (500,    "auth",    "db_connection_lost"),
        (503,    "ingest",  "disk_full"),
        (429,    "api",     "quota_exceeded"),
        (500,    "search",  "shard_unavailable"),
        (503,    "cache",   "eviction_storm"),
    ] {
        logs.push(doc("error", &format!("error {code} in module {module}: {msg}"), ts!()));
    }

    // ── Template 8: "metric <*> value <*> threshold <*>"
    // 12 variants
    for (metric, val, thresh) in [
        ("cpu_user",     88.4f32, 85.0f32), ("cpu_system",  12.1, 10.0),
        ("mem_rss",      7200.0,  8192.0),  ("mem_cached",  1024.0, 2048.0),
        ("net_rx_mbps",  450.0,   1000.0),  ("net_tx_mbps", 380.0,  800.0),
        ("disk_iops",    1200.0,  2000.0),  ("disk_lat_ms",   4.2,    5.0),
        ("cpu_user",     91.0,    85.0),    ("mem_rss",     7800.0,  8192.0),
        ("net_rx_mbps",  520.0,   1000.0),  ("disk_lat_ms",   6.1,    5.0),
    ] {
        logs.push(doc("metric", &format!("metric {metric} value {val:.1} threshold {thresh:.1}"), ts!()));
    }

    // ── Template 9: "backup job <*> for dataset <*> completed in <*>s"
    // 10 variants
    for (job, dataset, secs) in [
        ("daily-001", "postgres-main",    42u32), ("daily-002", "postgres-replica", 38),
        ("daily-003", "redis-sessions",    7),    ("daily-004", "redis-cache",        5),
        ("weekly-001","postgres-main",   310),   ("weekly-002","postgres-replica",  290),
        ("daily-005", "postgres-main",    45),   ("daily-006", "redis-sessions",      6),
        ("weekly-003","redis-cache",      18),   ("daily-007", "postgres-main",      41),
    ] {
        logs.push(doc("backup", &format!("backup job {job} for dataset {dataset} completed in {secs}s"), ts!()));
    }

    logs
}

fn run() -> Result<()> {
    let _ = env_logger::try_init();

    // ── Section 1: Setup ─────────────────────────────────────────────────────

    println!("=== Section 1: Setup ===");

    let dir = TempDir::new().unwrap();
    let dbpath = dir.path().join("db");
    let cfg_path = dir.path().join("bds.hjson");

    let hjson = format!(
        r#"{{
  dbpath: "{}"
  shard_duration: "1h"
  pool_size: 4
  similarity_threshold: 0.85
  drain_enabled: true
  drain_load_duration: "24h"
}}"#,
        dbpath.display()
    );
    std::fs::write(&cfg_path, &hjson).unwrap();

    let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)
        .map_err(|e| bdslib::common::error::err_msg(format!("{e}")))?;
    let manager = ShardsManager::with_embedding(cfg_path.to_str().unwrap(), embedding)?;

    println!("drain_enabled  = true");
    println!("drain depth    = 3  (routes on tokens[0] only → all lines in a");
    println!("                     category share one leaf → proper merging)");

    // ── Section 2: Ingest ────────────────────────────────────────────────────

    println!("\n=== Section 2: Ingest ===");

    let base_ts = ts_ago(1800);
    let logs = generate_logs(base_ts);
    let total = logs.len();
    println!("Generated {total} log documents across 9 template families");

    // First 30 one-by-one to exercise add()
    let single_n = 30.min(total);
    for d in logs[..single_n].iter().cloned() {
        manager.add(d)?;
    }
    println!("  add()       → {single_n} docs");

    // Remainder as a batch
    let batch = logs[single_n..].to_vec();
    let batch_n = batch.len();
    manager.add_batch(batch)?;
    println!("  add_batch() → {batch_n} docs");
    println!("Total ingested: {total}");

    // ── Section 3: Discover ──────────────────────────────────────────────────

    println!("\n=== Section 3: Discovered templates ===");

    let mut templates = manager.tpl_list("2h")?;
    // Sort by cluster_id so we see templates in discovery order
    templates.sort_by_key(|(_, m)| m.get("cluster_id").and_then(|v| v.as_u64()).unwrap_or(u64::MAX));

    // Deduplicate: keep only the latest version of each cluster_id (Updated
    // events create a new tplstorage entry; cluster_id stays the same)
    let mut seen_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut deduped: Vec<_> = Vec::new();
    for entry in templates.iter().rev() {
        let cid = entry.1.get("cluster_id").and_then(|v| v.as_u64()).unwrap_or(u64::MAX);
        if seen_ids.insert(cid) {
            deduped.push(entry);
        }
    }
    deduped.sort_by_key(|(_, m)| m.get("cluster_id").and_then(|v| v.as_u64()).unwrap_or(u64::MAX));

    println!("Distinct templates discovered: {}", deduped.len());
    println!();
    for (_, meta) in &deduped {
        let cid  = meta.get("cluster_id").and_then(|v| v.as_u64()).unwrap_or(0);
        let name = meta.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        println!("  [{cid:>2}] {name}");
    }

    // ── Section 4: Semantic search ───────────────────────────────────────────

    println!("\n=== Section 4: Semantic search over template store ===");

    for (query, limit) in [
        ("database connection established", 2usize),
        ("user login authentication",       2),
        ("error failure service module",    2),
        ("disk storage io usage",           2),
        ("backup completed dataset",        2),
    ] {
        let results = manager.tpl_search_text("2h", query, limit)?;
        println!("\nQuery: {:?}", query);
        for r in &results {
            let name  = r.get("metadata").and_then(|m| m.get("name")).and_then(|v| v.as_str()).unwrap_or("?");
            let score = r.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            println!("  {score:.4}  {name}");
        }
    }

    // ── Section 5: Reload ────────────────────────────────────────────────────

    println!("\n=== Section 5: Reload parser from stored templates ===");

    let mut fresh = manager.drain_load("2h")?;
    println!("Loaded {} clusters into fresh parser", fresh.clusters().len());

    let test_lines = [
        ("auth",    "user zara logged in from 10.0.5.99"),
        ("network", "connection to db-primary on port 5432 established"),
        ("error",   "error 404 in module routing: path_not_found"),
        ("worker",  "worker 7 picked up job 2050 from queue ingest"),
    ];

    println!();
    for (key, line) in test_lines {
        let d = doc(key, line, now_secs());
        let r = manager.drain_parse_json(&mut fresh, &d)?;
        println!("  input   : {line}");
        println!("  template: {}  [{:?} cluster={}]", r.template.join(" "), r.change_type, r.cluster_id);
        println!();
    }

    println!("Done.");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// shardsmanager_tpl_frequency_demo — Log ingestion, drain3 template discovery,
/// and frequency-tracking queries over the template store.
///
/// Sections:
///   1. Setup          — temp hjson config with drain_enabled = true
///   2. Ingest         — 84 log entries across 7 template families
///   3. Recent         — templates_recent("2h") — all freshly discovered templates
///   4. By ID          — template_by_id — fetch one template by UUID
///   5. Time range     — templates_by_timestamp — query a sub-window of the session
///   6. Frequency data — frequencytracking_by_id for each template UUID
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

/// Build 84 log lines distributed over a 70-minute window (1 min per entry).
///
/// 7 structural template families, 12 variants each.  The timestamps are spaced
/// 1 minute apart so queries on sub-windows return predictable subsets.
fn generate_logs(base_ts: u64) -> Vec<Value> {
    let mut logs: Vec<Value> = Vec::new();
    let step = 60u64; // 1 min per log entry
    macro_rules! ts { () => { base_ts + logs.len() as u64 * step } }

    // ── Family 1: "user <*> logged in from <*>"
    for (user, ip) in [
        ("alice", "10.0.0.1"), ("bob", "10.0.0.2"), ("carol", "10.0.0.3"),
        ("dave", "10.0.0.4"), ("eve", "10.0.0.5"), ("frank", "10.0.0.6"),
        ("grace", "10.0.0.7"), ("hank", "10.0.0.8"), ("ivan", "10.0.0.9"),
        ("judy", "10.0.0.10"), ("karl", "10.0.0.11"), ("lena", "10.0.0.12"),
    ] {
        logs.push(doc("auth", &format!("user {user} logged in from {ip}"), ts!()));
    }

    // ── Family 2: "connection to <*> on port <*> established"
    for (host, port) in [
        ("db-primary", 5432u16), ("db-replica", 5432), ("cache-a", 6379),
        ("cache-b", 6379), ("queue-1", 5672), ("queue-2", 5672),
        ("search-1", 9200), ("search-2", 9200), ("db-primary", 5433),
        ("metrics", 9090), ("tracing", 4317), ("proxy", 8080),
    ] {
        logs.push(doc("network", &format!("connection to {host} on port {port} established"), ts!()));
    }

    // ── Family 3: "service <*> restarted after <*> seconds"
    for (svc, secs) in [
        ("api-gateway", 3u32), ("auth-service", 5), ("ingest-worker", 2),
        ("search-node", 4), ("cache-proxy", 6), ("queue-consumer", 1),
        ("api-gateway", 3), ("auth-service", 5), ("metrics-agent", 2),
        ("log-shipper", 7), ("health-checker", 1), ("scheduler", 4),
    ] {
        logs.push(doc("ops", &format!("service {svc} restarted after {secs} seconds"), ts!()));
    }

    // ── Family 4: "HTTP <*> <*> returned <*> in <*> ms"
    for (method, path, code, ms) in [
        ("GET",    "/api/health",       200u16, 4u32),
        ("GET",    "/api/users",        200,    23),
        ("POST",   "/api/users",        201,    45),
        ("GET",    "/api/records",      200,    67),
        ("DELETE", "/api/records/123",  204,    12),
        ("GET",    "/api/search",       200,    134),
        ("POST",   "/api/ingest",       202,    9),
        ("GET",    "/api/metrics",      200,    31),
        ("PUT",    "/api/config",       200,    7),
        ("GET",    "/api/health",       200,    5),
        ("POST",   "/api/events",       201,    18),
        ("GET",    "/api/status",       200,    3),
    ] {
        logs.push(doc("http", &format!("HTTP {method} {path} returned {code} in {ms} ms"), ts!()));
    }

    // ── Family 5: "worker <*> picked up job <*> from queue <*>"
    for (wid, jid, queue) in [
        (1u8, 1001u32, "ingest"), (2, 1002, "ingest"), (3, 1003, "search"),
        (4, 1004, "search"), (5, 1005, "export"), (1, 1006, "ingest"),
        (2, 1007, "export"), (3, 1008, "search"), (4, 1009, "ingest"),
        (5, 1010, "export"), (1, 1011, "search"), (2, 1012, "ingest"),
    ] {
        logs.push(doc("worker", &format!("worker {wid} picked up job {jid} from queue {queue}"), ts!()));
    }

    // ── Family 6: "disk <*> usage <*>% on volume <*>"
    for (op, pct, vol) in [
        ("read",  72u8, "/dev/sda1"), ("write", 45,  "/dev/sda1"),
        ("read",  80,   "/dev/sdb1"), ("write", 60,  "/dev/sdb1"),
        ("read",  55,   "/dev/sdc1"), ("write", 30,  "/dev/sdc1"),
        ("read",  90,   "/dev/sdd1"), ("write", 70,  "/dev/sdd1"),
        ("read",  62,   "/dev/sde1"), ("write", 48,  "/dev/sde1"),
        ("read",  77,   "/dev/sdf1"), ("write", 53,  "/dev/sdf1"),
    ] {
        logs.push(doc("disk", &format!("disk {op} usage {pct}% on volume {vol}"), ts!()));
    }

    // ── Family 7: "backup job <*> for dataset <*> completed in <*>s"
    for (job, dataset, secs) in [
        ("daily-001", "postgres-main",    42u32), ("daily-002", "postgres-replica", 38),
        ("daily-003", "redis-sessions",    7),    ("daily-004", "redis-cache",        5),
        ("weekly-001","postgres-main",   310),   ("weekly-002","postgres-replica",  290),
        ("daily-005", "postgres-main",    45),   ("daily-006", "redis-sessions",      6),
        ("weekly-003","redis-cache",      18),   ("daily-007", "postgres-main",      41),
        ("daily-008", "redis-cache",       4),   ("weekly-004","postgres-replica",  305),
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

    println!("ShardsManager ready  drain_enabled=true  shard_duration=1h");

    // ── Section 2: Ingest ────────────────────────────────────────────────────

    println!("\n=== Section 2: Ingest ===");

    // Base timestamp 95 min ago: 84 entries × 1 min/entry = 84 min span,
    // ending 11 min ago — all entries in the past within the 2-hour window.
    let base_ts = ts_ago(95 * 60);
    let logs = generate_logs(base_ts);
    let n = logs.len();

    // Record the timestamps of the first and last entry for Section 5.
    let first_ts = logs.first().and_then(|d| d["timestamp"].as_u64()).unwrap();
    let last_ts  = logs.last().and_then(|d| d["timestamp"].as_u64()).unwrap();

    let ids = manager.add_batch(logs)?;
    println!("Ingested {} log entries  ({} UUIDs assigned)", n, ids.len());
    println!("Timestamp window  {} … {}", first_ts, last_ts);

    // ── Section 3: templates_recent ──────────────────────────────────────────

    println!("\n=== Section 3: templates_recent(\"2h\") ===");

    let recent = manager.templates_recent("2h")?;
    println!("Discovered {} templates:", recent.len());
    for (i, tpl) in recent.iter().enumerate() {
        let id   = tpl["id"].as_str().unwrap_or("?");
        let body = tpl["body"].as_str().unwrap_or("");
        println!("  [{i}] id={id}  body={body:?}");
    }

    if recent.is_empty() {
        println!("  (no templates yet — drain may need more distinct lines to converge)");
        return Ok(());
    }

    // ── Section 4: template_by_id ────────────────────────────────────────────

    println!("\n=== Section 4: template_by_id ===");

    // Pick the first discovered template.
    let first_id = recent[0]["id"].as_str().unwrap();
    let result = manager.template_by_id(first_id)?;

    match result {
        None => println!("  Template {first_id} not found (unexpected)"),
        Some(tpl) => {
            println!("  id       = {}", tpl["id"].as_str().unwrap_or("?"));
            println!("  metadata = {}", tpl["metadata"]);
            println!("  body     = {:?}", tpl["body"].as_str().unwrap_or(""));
        }
    }

    // Also confirm a random UUID returns None.
    let fake = uuid::Uuid::now_v7().to_string();
    assert!(manager.template_by_id(&fake)?.is_none());
    println!("  Non-existent UUID correctly returns None");

    // ── Section 5: templates_by_timestamp ────────────────────────────────────

    println!("\n=== Section 5: templates_by_timestamp ===");

    // Query the first half of the ingestion window (roughly first 35 min).
    let mid_ts  = first_ts + (last_ts - first_ts) / 2;
    let range_a = manager.templates_by_timestamp(first_ts, mid_ts)?;
    let range_b = manager.templates_by_timestamp(mid_ts + 1, last_ts)?;
    let range_full = manager.templates_by_timestamp(first_ts, last_ts)?;

    println!("  Window A  [first … mid]   → {} templates", range_a.len());
    for tpl in &range_a {
        let body = tpl["body"].as_str().unwrap_or("");
        println!("    body={body:?}");
    }
    println!("  Window B  [mid+1 … last]  → {} templates", range_b.len());
    for tpl in &range_b {
        let body = tpl["body"].as_str().unwrap_or("");
        println!("    body={body:?}");
    }
    println!("  Full range               → {} templates", range_full.len());

    // ── Section 6: FrequencyTracking data per template ────────────────────────

    println!("\n=== Section 6: FrequencyTracking data ===");
    println!("  Template UUID                              | observed-at timestamps");
    println!("  ------------------------------------------ | -------------------------");

    for tpl in &recent {
        let id = tpl["id"].as_str().unwrap_or("?");
        // tplstorage.frequencytracking_by_id returns the timestamps when this
        // UUID was stored. Templates stored via drain may be recorded once
        // (New) or several times (Updated on each cluster merge).
        let body_preview: String = tpl["body"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(40)
            .collect();
        println!("  {id}  |  body={body_preview:?}");
    }

    println!("\nDone.");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

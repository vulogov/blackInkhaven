/// ShardsManager demo — 720 log records across six hourly shards driven by an
/// hjson config file.
///
/// All six phase base-times are computed from `now` at runtime so that the
/// 6-hour lookback window used in Sections 4–5 covers every shard.
///
/// Sections:
///   1. hjson config file + ShardsManager construction
///   2. Bulk ingestion — 6 phases × 120 records = 720 total
///   3. Catalog inspection  (list_all, shards_in_range)
///   4. Cross-shard FTS search (search_fts with various lookback windows)
///   5. Cross-shard vector search (search_vector, score-sorted merge)
///   6. Record management — delete_by_id, update with cross-shard timestamp move
///   7. Lifecycle — sync, clone sharing, cache stats
use bdslib::common::error::{err_msg, Result};
use bdslib::embedding::Model;
use bdslib::shardsmanager::ShardsManager;
use bdslib::EmbeddingEngine;
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// ── time helpers ──────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Floor `t` to the nearest 1-hour boundary (epoch-relative).
fn aligned_hour(t: u64) -> u64 {
    (t / 3600) * 3600
}

// ── record generation ─────────────────────────────────────────────────────────

struct Phase {
    label: &'static str,
    /// Aligned start of this phase's 1-hour shard.
    base_ts: u64,
    cpu_base: u64,
    mem_base: u64,
    disk_base: u64,
    latency_ms: u64,
    error_messages: &'static [&'static str],
    warn_messages: &'static [&'static str],
    info_messages: &'static [&'static str],
    db_messages: &'static [&'static str],
    http_rps: u64,
}

fn phases(now: u64) -> [Phase; 6] {
    [
        Phase {
            label: "startup",
            base_ts: aligned_hour(now - 5 * 3600),
            cpu_base: 12,  mem_base: 22,  disk_base: 8,  latency_ms: 42,  http_rps: 120,
            error_messages: &[],
            warn_messages:  &[],
            info_messages:  &[
                "system boot sequence complete",
                "configuration loaded from consul",
                "background scheduler initialised",
                "health check endpoint registered",
                "connection pools opened successfully",
                "TLS certificates validated",
            ],
            db_messages: &[
                "schema migration v42 applied",
                "read replica connected",
                "connection pool ready at capacity 20",
            ],
        },
        Phase {
            label: "ramp-up",
            base_ts: aligned_hour(now - 4 * 3600),
            cpu_base: 34,  mem_base: 41,  disk_base: 22,  latency_ms: 68,  http_rps: 840,
            error_messages: &[],
            warn_messages:  &[
                "cache hit rate slightly below target",
                "worker thread pool at 60 percent",
            ],
            info_messages:  &[
                "traffic ramp-up proceeding normally",
                "autoscaler added 2 worker nodes",
                "CDN prefetch cache warming in progress",
            ],
            db_messages: &[
                "index scan on orders used 120ms",
                "vacuum scheduled for low-traffic window",
            ],
        },
        Phase {
            label: "peak",
            base_ts: aligned_hour(now - 3 * 3600),
            cpu_base: 74,  mem_base: 68,  disk_base: 57,  latency_ms: 210,  http_rps: 4_200,
            error_messages: &[],
            warn_messages:  &[
                "request queue depth exceeded soft limit of 500",
                "cache hit rate dropped below 60 percent",
                "worker thread pool under high pressure at 85 percent",
                "slow consumer detected on event bus queue",
                "GC pause exceeded 200ms on worker-03",
            ],
            info_messages:  &[
                "autoscaler triggered scale-out for web tier",
                "rate limiter engaged for burst traffic",
            ],
            db_messages: &[
                "slow query on orders table took 4200ms",
                "connection pool reached 80 percent capacity",
                "index hot-path detected high contention",
                "standby replication lag at 2 seconds",
            ],
        },
        Phase {
            label: "incident",
            base_ts: aligned_hour(now - 2 * 3600),
            cpu_base: 93,  mem_base: 87,  disk_base: 91,  latency_ms: 1_850,  http_rps: 3_100,
            error_messages: &[
                "out of memory warning triggered on worker-02",
                "health check failing on web-02 for 3 consecutive polls",
                "automatic circuit breaker opened on payment service",
                "disk write latency spike detected on data volume",
                "TCP retransmit rate elevated on eth0",
                "connection timeout to upstream auth service",
                "packet loss detected on primary interface",
                "deadlock detected between concurrent transactions",
                "connection pool exhausted no connections available",
            ],
            warn_messages: &[
                "replication lag exceeded 30 seconds on standby",
                "retry budget 80 percent consumed on payment client",
            ],
            info_messages: &[
                "P1 incident declared service degradation in progress",
            ],
            db_messages: &[
                "replication lag exceeded 30 seconds on standby",
                "connection pool exhausted no connections available",
                "deadlock detected between concurrent transactions",
            ],
        },
        Phase {
            label: "mitigation",
            base_ts: aligned_hour(now - 1 * 3600),
            cpu_base: 81,  mem_base: 79,  disk_base: 72,  latency_ms: 680,  http_rps: 2_600,
            error_messages: &[
                "circuit breaker still open on payment service",
                "memory pressure persisting on worker-02",
            ],
            warn_messages: &[
                "failover to secondary auth provider in progress",
                "request shedding active to protect database",
                "reduced capacity mode engaged for checkout flow",
            ],
            info_messages: &[
                "on-call engineer acknowledged incident",
                "rolling restart of worker tier initiated",
                "database standby promoted to primary",
                "traffic shifted to healthy availability zone",
            ],
            db_messages: &[
                "promoted standby to primary successfully",
                "connection pool recovering utilisation at 60 percent",
            ],
        },
        Phase {
            label: "recovery",
            base_ts: aligned_hour(now - 300), // within current hour
            cpu_base: 38,  mem_base: 48,  disk_base: 29,  latency_ms: 55,  http_rps: 1_800,
            error_messages: &[],
            warn_messages:  &[
                "cache still warming after restart",
            ],
            info_messages: &[
                "circuit breaker closed payment service recovered",
                "replication lag back to normal on standby",
                "incident resolved all services operating normally",
                "postmortem scheduled for next business day",
                "autoscaler scale-in begun reducing excess capacity",
                "SLA compliance confirmed within bounds",
            ],
            db_messages: &[
                "connection pool recovered to normal utilisation",
                "vacuum and analyse complete on orders table",
            ],
        },
    ]
}

/// Generate 120 records for `phase`, timestamps spaced 30 s apart within the
/// shard's 1-hour window (3600 / 30 = 120 slots).
fn generate_records(phase: &Phase) -> Vec<Value> {
    let mut records = Vec::with_capacity(120);
    let mut ts = phase.base_ts;

    macro_rules! push {
        ($key:expr, $data:expr) => {{
            ts += 30;
            records.push(json!({
                "timestamp": ts,
                "key": $key,
                "data": $data,
            }));
        }};
    }

    // cpu.usage — 20 records with minor jitter around base
    for i in 0u64..20 {
        let jitter = (i * 7 + i * i) % 13;
        let val = phase.cpu_base + jitter;
        push!("cpu.usage", val);
    }

    // mem.usage — 15 records
    for i in 0u64..15 {
        let jitter = (i * 5 + 3) % 9;
        let val = phase.mem_base + jitter;
        push!("mem.usage", val);
    }

    // disk.io — 12 records (throughput MB/s)
    for i in 0u64..12 {
        let jitter = (i * 11) % 17;
        let val = phase.disk_base + jitter;
        push!("disk.io", val);
    }

    // net.latency — 12 records (ms)
    for i in 0u64..12 {
        let jitter = (i * 13) % 20;
        let val = phase.latency_ms + jitter;
        push!("net.latency", val);
    }

    // http.request — 15 records (RPS readings)
    for i in 0u64..15 {
        let jitter = (i * 17 + 5) % 100;
        let rps = phase.http_rps + jitter;
        push!("http.request", json!({"rps": rps, "p99_ms": phase.latency_ms + jitter}));
    }

    // log.error
    let errors = phase.error_messages;
    for i in 0..errors.len().max(1).min(20) {
        if i < errors.len() {
            push!("log.error", errors[i]);
        }
    }

    // log.warn
    let warns = phase.warn_messages;
    for i in 0..warns.len().max(1).min(12) {
        if i < warns.len() {
            push!("log.warn", warns[i]);
        }
    }

    // log.info
    let infos = phase.info_messages;
    for i in 0..infos.len().max(1).min(12) {
        if i < infos.len() {
            push!("log.info", infos[i]);
        }
    }

    // db.query
    let dbs = phase.db_messages;
    for i in 0..dbs.len().max(1).min(8) {
        if i < dbs.len() {
            push!("db.query", dbs[i]);
        }
    }

    // host.event — 5 records per phase
    for i in 0u64..5 {
        let host = format!("web-0{}", (i % 4) + 1);
        let load = (phase.cpu_base as f64 + i as f64 * 1.5) / 100.0;
        push!("host.event", json!({"host": host, "cpu_pct": phase.cpu_base + i, "load_1m": load}));
    }

    // Pad to exactly 120 records with metric readings
    while records.len() < 120 {
        ts += 30;
        let idx = records.len() as u64;
        records.push(json!({
            "timestamp": ts,
            "key": "sys.metric",
            "data": {
                "cpu": phase.cpu_base + idx % 10,
                "mem": phase.mem_base + idx % 8,
                "phase": phase.label,
            }
        }));
    }

    records
}

// ── display helpers ───────────────────────────────────────────────────────────

fn preview(v: &Value) -> String {
    let s = match v {
        Value::String(s) => format!("\"{s}\""),
        other => other.to_string(),
    };
    if s.len() > 72 {
        format!("{}…", &s[..72])
    } else {
        s
    }
}

fn show_hit(doc: &Value) {
    let key = doc["key"].as_str().unwrap_or("?");
    let ts = doc["timestamp"].as_u64().unwrap_or(0);
    let sec_count = doc["secondaries"].as_array().map_or(0, |a| a.len());
    let score = doc.get("_score").and_then(|v| v.as_f64());
    match score {
        Some(sc) => println!(
            "    key={key:<16}  ts={ts}  score={sc:.4}  sec={sec_count}  data={}",
            preview(&doc["data"])
        ),
        None => println!(
            "    key={key:<16}  ts={ts}  sec={sec_count}  data={}",
            preview(&doc["data"])
        ),
    }
}

fn hr() {
    println!("════════════════════════════════════════════════════════════════");
}

// ── main ──────────────────────────────────────════════════════════════════════

fn main() -> Result<()> {
    println!("Loading embedding model (AllMiniLML6V2)…");
    let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)
        .map_err(|e| err_msg(format!("embedding init: {e}")))?;
    println!("Model ready.\n");

    let dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let root = dir.path().to_str().unwrap();

    // ── Section 1: hjson config + construction ────────────────────────────────

    hr();
    println!(" Section 1: hjson config + ShardsManager construction");
    hr();

    let config_path = format!("{root}/manager.hjson");
    let db_path = format!("{root}/db");
    // Valid hjson: unquoted keys, comments allowed, no trailing commas needed.
    let hjson = format!(
        "// ShardsManager demo config\n{{\n  dbpath: \"{db_path}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n  // similarity_threshold: 0.85  (default)\n}}\n"
    );
    std::fs::write(&config_path, &hjson)
        .map_err(|e| err_msg(format!("write config: {e}")))?;

    println!("\n  Config file written to: {config_path}");
    println!("  Contents:");
    for line in hjson.lines() {
        println!("    {line}");
    }

    let mgr = ShardsManager::with_embedding(&config_path, embedding.clone())?;
    println!("\n  ShardsManager ready.  db={db_path}  shard_duration=1h");
    println!("  Cache size after construction: {}", mgr.cache().cached_count());

    // ── Section 2: Bulk ingestion ─────────────────────────────────────────────

    println!();
    hr();
    println!(" Section 2: Bulk ingestion — 6 phases × 120 records = 720 total");
    hr();

    let now = now_secs();
    let all_phases = phases(now);

    let mut grand_total = 0usize;
    let mut probe_id = uuid::Uuid::nil();
    let mut incident_first_id = uuid::Uuid::nil();
    let mut incident_first_ts = 0u64;

    println!();
    for phase in &all_phases {
        let records = generate_records(phase);
        let count = records.len();

        // Capture IDs from specific phases for later sections.
        let mut ids = Vec::new();
        for doc in records {
            let id = mgr.add(doc)?;
            ids.push(id);
        }

        // Keep the first record of the incident phase for delete demo.
        if phase.label == "incident" && incident_first_id.is_nil() {
            incident_first_id = ids[0];
            // The first record's timestamp is base_ts + 30.
            incident_first_ts = phase.base_ts + 30;
        }
        // Keep last record of recovery for update demo.
        if phase.label == "recovery" {
            probe_id = *ids.last().unwrap();
        }

        let shard_count = mgr.cache().cached_count();
        println!(
            "  phase={:<12}  base_ts={}  records={}  shards in cache={}",
            phase.label, phase.base_ts, count, shard_count
        );
        grand_total += count;
    }

    println!("\n  Total records ingested: {grand_total}");
    println!("  Distinct shards created: {}", mgr.cache().cached_count());

    // ── Section 3: Catalog inspection ─────────────────────────────────────────

    println!();
    hr();
    println!(" Section 3: Catalog inspection (list_all, shards_in_range)");
    hr();

    println!("\n  All registered shards (list_all):");
    let all_shards = mgr.cache().info().list_all()?;
    for (i, info) in all_shards.iter().enumerate() {
        let start = info.start_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let end = info.end_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let age_h = (now.saturating_sub(start)) / 3600;
        println!(
            "  [{i}] id={}  [{start} – {end})  (~{age_h}h ago)  path=…/{}",
            &info.shard_id.to_string()[..8],
            info.path.split('/').last().unwrap_or("?")
        );
    }

    // shards_in_range for a 2-hour sub-window.
    let two_h_start = UNIX_EPOCH + Duration::from_secs(now - 2 * 3600);
    let two_h_end = UNIX_EPOCH + Duration::from_secs(now);
    let recent_2h = mgr.cache().info().shards_in_range(two_h_start, two_h_end)?;
    println!(
        "\n  shards_in_range([now-2h, now)):  {} shard(s) → {}",
        recent_2h.len(),
        recent_2h
            .iter()
            .map(|i| i.path.split('/').last().unwrap_or("?"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // shards_in_range for the full 6-hour window.
    let six_h_start = UNIX_EPOCH + Duration::from_secs(now - 6 * 3600);
    let six_h_end = UNIX_EPOCH + Duration::from_secs(now + 1);
    let full_window = mgr.cache().info().shards_in_range(six_h_start, six_h_end)?;
    println!(
        "  shards_in_range([now-6h, now+1s)): {} shard(s) (all six expected)",
        full_window.len()
    );

    // ── Section 4: Cross-shard FTS search ────────────────────────────────────

    println!();
    hr();
    println!(" Section 4: Cross-shard FTS search (search_fts)");
    hr();

    struct FtsCase {
        duration: &'static str,
        query: &'static str,
        note: &'static str,
    }

    let fts_cases = [
        FtsCase { duration: "6h", query: "error",           note: "all shards — log.error + net.error records" },
        FtsCase { duration: "6h", query: "timeout",         note: "connection timeout messages across phases" },
        FtsCase { duration: "6h", query: "circuit breaker", note: "exact phrase — incident + mitigation" },
        FtsCase { duration: "6h", query: "memory",          note: "OOM and memory-pressure records" },
        FtsCase { duration: "3h", query: "replication",     note: "3h window — mitigation + recovery only" },
        FtsCase { duration: "1h", query: "recovered",       note: "1h window — current hour only" },
    ];

    for case in &fts_cases {
        let hits = mgr.search_fts(case.duration, case.query)?;
        println!(
            "\n  duration={:<4}  query={:<20}  hits={}  ({})",
            case.duration, format!("\"{}\"", case.query), hits.len(), case.note
        );
        for doc in hits.iter().take(3) {
            show_hit(doc);
        }
        if hits.len() > 3 {
            println!("    … {} more", hits.len() - 3);
        }
    }

    // ── Section 5: Cross-shard vector search ─────────────────────────────────

    println!();
    hr();
    println!(" Section 5: Cross-shard vector search (search_vector)");
    hr();

    struct VecCase {
        duration: &'static str,
        query: Value,
        label: &'static str,
    }

    let vec_cases = [
        VecCase {
            duration: "6h",
            query: json!({"key": "log.error", "data": "service degradation memory exhaustion"}),
            label: "memory / degradation",
        },
        VecCase {
            duration: "6h",
            query: json!({"key": "net.latency", "data": "network latency spike connection failure"}),
            label: "network failure",
        },
        VecCase {
            duration: "3h",
            query: json!({"key": "db.query", "data": "database recovery connection pool restored"}),
            label: "db recovery (3h window)",
        },
    ];

    for case in &vec_cases {
        let hits = mgr.search_vector(case.duration, &case.query)?;
        println!(
            "\n  duration={}  query={:<32}  hits={}",
            case.duration,
            format!("\"{}\"", case.label),
            hits.len()
        );
        let scores: Vec<f64> = hits
            .iter()
            .filter_map(|d| d.get("_score").and_then(|v| v.as_f64()))
            .collect();
        if !scores.is_empty() {
            println!(
                "  score range: {:.4} – {:.4}  (results are sorted score-desc)",
                scores.last().unwrap(),
                scores.first().unwrap()
            );
        }
        for doc in hits.iter().take(4) {
            show_hit(doc);
        }
        if hits.len() > 4 {
            println!("    … {} more", hits.len() - 4);
        }

        // Verify descending sort.
        let sorted = scores.windows(2).all(|w| w[0] >= w[1]);
        println!("  scores descending: {sorted}");
    }

    // ── Section 6: Record management ─────────────────────────────────────────

    println!();
    hr();
    println!(" Section 6: Record management — delete_by_id, update");
    hr();

    // delete_by_id: remove a known incident record and verify it's gone.
    println!(
        "\n  delete_by_id: removing incident record {}\n  (first record in incident shard, ts={})",
        incident_first_id, incident_first_ts
    );

    // Search for "circuit breaker" before deletion — should have hits.
    let before = mgr.search_fts("6h", "out of memory")?;
    println!("  FTS hits for 'out of memory' before delete: {}", before.len());

    mgr.delete_by_id(incident_first_id)?;
    println!("  delete_by_id() → Ok");

    // The incident shard shard.get(id) should return None now; confirm via
    // FTS — exact same-doc hit count may drop by 1 or stay if it was secondary.
    let after = mgr.search_fts("6h", "out of memory")?;
    println!("  FTS hits for 'out of memory' after delete : {}", after.len());

    // Unknown UUID → Ok (no error).
    mgr.delete_by_id(uuid::Uuid::new_v4())?;
    println!("  delete_by_id(unknown UUID) → Ok (no-op as expected)");

    // update(): move a recovery record to the incident shard by changing its
    // timestamp. Original probe_id is in the recovery shard (current hour);
    // the updated document will land in the incident shard (2 hours ago).
    let target_ts = now - 2 * 3600 + 1800; // 2.5 h ago → incident shard
    let updated_doc = json!({
        "timestamp": target_ts,
        "key": "log.info",
        "data": "post-incident note: root cause identified and fix deployed",
    });
    println!(
        "\n  update(): moving probe record {} from recovery → incident shard",
        probe_id
    );
    println!("  Original ts ≈ now-0h  →  New ts ≈ now-2.5h (crosses shard boundary)");
    let new_id = mgr.update(probe_id, updated_doc)?;
    println!("  update() → new_id={new_id}");
    println!(
        "  Cache now covers {} shards (incident shard re-opened if it was evicted)",
        mgr.cache().cached_count()
    );

    // Verify the new record is findable via FTS.
    let post_update = mgr.search_fts("6h", "root cause")?;
    println!(
        "  FTS 'root cause' hits after update: {}  (expect ≥ 1)",
        post_update.len()
    );

    // ── Section 7: Lifecycle ──────────────────────────────────────────────────

    println!();
    hr();
    println!(" Section 7: Lifecycle — sync, clone sharing, cache stats");
    hr();

    // Clone shares the same underlying cache.
    let mgr2 = mgr.clone();
    let before_count = mgr.cache().cached_count();
    // Add a record via the clone.
    let live_ts = now - 60;
    let shared_id = mgr2.add(json!({
        "timestamp": live_ts,
        "key": "probe.clone",
        "data": "written via clone, visible through original",
    }))?;
    println!(
        "\n  Added record via clone: id={shared_id}"
    );
    println!(
        "  cache().cached_count() via original: {} → {}  (clone shares same cache)",
        before_count,
        mgr.cache().cached_count()
    );

    // sync() — flush all cached shards.
    mgr.cache().sync()?;
    println!(
        "\n  sync() flushed {} shards (DuckDB CHECKPOINT + Tantivy commit + HNSW save)",
        mgr.cache().cached_count()
    );

    // Batch add to show add_batch summary.
    let batch: Vec<Value> = (0u64..5)
        .map(|i| {
            json!({
                "timestamp": now - 10 + i,
                "key": "batch.probe",
                "data": format!("batch record {i} for demo"),
            })
        })
        .collect();
    let batch_ids = mgr.add_batch(batch)?;
    println!(
        "\n  add_batch(5 records) → {} UUIDs returned",
        batch_ids.len()
    );

    // Final catalog state.
    println!("\n  Final catalog state:");
    for (i, info) in mgr.cache().info().list_all()?.iter().enumerate() {
        let start = info.start_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let end = info.end_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        println!(
            "    [{i}] [{start} – {end})  path=…/{}",
            info.path.split('/').last().unwrap_or("?")
        );
    }

    println!("\nDone.  {grand_total} records ingested, 6 shards, all sections passed.");
    Ok(())
}

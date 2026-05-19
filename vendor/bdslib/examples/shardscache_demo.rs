/// ShardsCache demo — time-partitioned telemetry across multiple hourly shards.
///
/// Simulates four hours of infrastructure telemetry (startup → peak → incident
/// → recovery).  Each hour is stored in its own shard; the cache automatically
/// creates, routes, and catalogs each shard.
///
/// Sections:
///   1. Setup and ingestion across four 1-hour shards
///   2. Cache and catalog inspection
///   3. Per-shard FTS and vector search
///   4. Cross-shard span query  (shards_span)
///   5. Current-time window     (current)
///   6. Lifecycle: sync → close → reopen from catalog
use bdslib::common::error::{err_msg, Result};
use bdslib::embedding::Model;
use bdslib::shardscache::ShardsCache;
use bdslib::EmbeddingEngine;
use serde_json::{json, Value};
use std::time::{Duration, UNIX_EPOCH};
use tempfile::TempDir;

// ── time layout ───────────────────────────────────────────────────────────────
//
// 1-hour shard boundaries relative to 1_748_000_000 s (2025-05-23 ~04:13 UTC).
//
//   Hour 0  1747998000 – 1748001600   startup   (low load)
//   Hour 1  1748001600 – 1748005200   peak      (high load)
//   Hour 2  1748005200 – 1748008800   incident  (degraded)
//   Hour 3  1748008800 – 1748012400   recovery  (normalising)

const H0: u64 = 1_747_998_000; // aligned start of hour 0
const H1: u64 = H0 + 3_600;
const H2: u64 = H0 + 7_200;
const H3: u64 = H0 + 10_800;

// ── telemetry datasets ────────────────────────────────────────────────────────

struct Phase {
    label: &'static str,
    base_ts: u64,
    records: Vec<(&'static str, Value)>, // (key, data)
}

fn startup_phase() -> Phase {
    Phase {
        label: "startup",
        base_ts: H0,
        records: vec![
            ("cpu.usage",    json!(18)),
            ("cpu.usage",    json!(21)),
            ("cpu.usage",    json!(24)),
            ("mem.usage",    json!(34)),
            ("mem.usage",    json!(36)),
            ("disk.io",      json!(12)),
            ("disk.io",      json!(14)),
            ("net.latency",  json!(0.045)),
            ("net.latency",  json!(0.048)),
            ("health.check", json!("all systems nominal")),
            ("health.check", json!("services initialized")),
            ("health.check", json!("application ready to serve")),
            ("host.event",   json!({"host":"web-01","status":"up","load":0.18})),
            ("host.event",   json!({"host":"web-02","status":"up","load":0.21})),
            ("host.event",   json!({"host":"db-01", "status":"up","load":0.15})),
            ("log.info",     json!("startup sequence complete on all nodes")),
            ("log.info",     json!("configuration loaded from config server")),
            ("log.info",     json!("background job scheduler initialised")),
        ],
    }
}

fn peak_phase() -> Phase {
    Phase {
        label: "peak",
        base_ts: H1,
        records: vec![
            ("cpu.usage",    json!(72)),
            ("cpu.usage",    json!(78)),
            ("cpu.usage",    json!(81)),
            ("cpu.usage",    json!(85)),
            ("mem.usage",    json!(67)),
            ("mem.usage",    json!(73)),
            ("mem.usage",    json!(79)),
            ("disk.io",      json!(58)),
            ("disk.io",      json!(63)),
            ("net.latency",  json!(0.089)),
            ("net.latency",  json!(0.123)),
            ("net.latency",  json!(0.287)),
            ("host.event",   json!({"host":"web-01","status":"busy","load":0.82})),
            ("host.event",   json!({"host":"web-02","status":"busy","load":0.77})),
            ("host.event",   json!({"host":"worker-01","status":"up","load":0.69})),
            ("db.query",     json!("slow query on orders table took 4200ms")),
            ("db.query",     json!("connection pool reached 80 percent capacity")),
            ("log.warn",     json!("request queue depth exceeded soft limit")),
            ("log.warn",     json!("cache hit rate dropped below 60 percent")),
            ("log.warn",     json!("worker thread pool under high pressure")),
        ],
    }
}

fn incident_phase() -> Phase {
    Phase {
        label: "incident",
        base_ts: H2,
        records: vec![
            ("cpu.usage",    json!(94)),
            ("cpu.usage",    json!(97)),
            ("cpu.usage",    json!(99)),
            ("mem.usage",    json!(88)),
            ("mem.usage",    json!(92)),
            ("mem.usage",    json!(95)),
            ("disk.io",      json!(98)),
            ("net.latency",  json!(1.230)),
            ("net.latency",  json!(2.567)),
            ("net.error",    json!("TCP retransmit rate elevated on eth0")),
            ("net.error",    json!("connection timeout to upstream service")),
            ("net.error",    json!("packet loss detected on primary interface")),
            ("host.event",   json!({"host":"web-02","status":"degraded","load":0.99})),
            ("host.event",   json!({"host":"worker-01","status":"overloaded","load":0.97})),
            ("db.query",     json!("deadlock detected between concurrent transactions")),
            ("db.query",     json!("connection pool exhausted no connections available")),
            ("db.query",     json!("replication lag exceeded 30 seconds on standby")),
            ("log.error",    json!("out of memory warning triggered on worker-02")),
            ("log.error",    json!("health check failing on web-02 for 3 consecutive polls")),
            ("log.error",    json!("automatic circuit breaker opened on payment service")),
            ("log.error",    json!("disk write latency spike detected on data volume")),
            ("alert.page",   json!("P1 incident declared: service degradation in progress")),
        ],
    }
}

fn recovery_phase() -> Phase {
    Phase {
        label: "recovery",
        base_ts: H3,
        records: vec![
            ("cpu.usage",    json!(61)),
            ("cpu.usage",    json!(52)),
            ("cpu.usage",    json!(44)),
            ("mem.usage",    json!(71)),
            ("mem.usage",    json!(63)),
            ("disk.io",      json!(42)),
            ("disk.io",      json!(31)),
            ("net.latency",  json!(0.312)),
            ("net.latency",  json!(0.145)),
            ("net.latency",  json!(0.067)),
            ("host.event",   json!({"host":"web-02","status":"up","load":0.61})),
            ("host.event",   json!({"host":"worker-01","status":"up","load":0.58})),
            ("db.query",     json!("connection pool recovered to normal utilisation")),
            ("log.info",     json!("circuit breaker closed payment service recovered")),
            ("log.info",     json!("replication lag back to normal on standby")),
            ("log.info",     json!("incident resolved all services operating normally")),
            ("alert.page",   json!("P1 incident resolved: all metrics within SLA bounds")),
        ],
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn data_preview(data: &Value) -> String {
    let s = match data {
        Value::String(s) => format!("\"{s}\""),
        other => other.to_string(),
    };
    if s.len() > 64 { format!("{}…", &s[..64]) } else { s }
}

fn show_primary(doc: &Value) {
    let key = doc["key"].as_str().unwrap_or("?");
    let ts  = doc["timestamp"].as_u64().unwrap_or(0);
    let sec = doc["secondaries"].as_array().map_or(0, |a| a.len());
    let score = doc.get("_score").and_then(|v| v.as_f64());
    match score {
        Some(sc) => println!(
            "    PRIMARY  key={key:<14}  ts={ts}  score={sc:.4}  data={}  [{sec} sec]",
            data_preview(&doc["data"])
        ),
        None => println!(
            "    PRIMARY  key={key:<14}  ts={ts}  data={}  [{sec} sec]",
            data_preview(&doc["data"])
        ),
    }
    for s in doc["secondaries"].as_array().unwrap_or(&vec![]) {
        println!("      secondary  data={}", data_preview(&s["data"]));
    }
}

fn ingest_phase(
    cache: &ShardsCache,
    phase: &Phase,
) -> Result<usize> {
    let shard = cache.shard(UNIX_EPOCH + Duration::from_secs(phase.base_ts))?;
    let mut ts = phase.base_ts;
    for (key, data) in &phase.records {
        ts += 120;
        shard.add(json!({ "timestamp": ts, "key": key, "data": data }))?;
    }
    Ok(phase.records.len())
}

fn shard_stats(cache: &ShardsCache, base_ts: u64, label: &str) -> Result<()> {
    let shard = cache.shard(UNIX_EPOCH + Duration::from_secs(base_ts))?;
    let obs = shard.observability();
    let primaries  = obs.list_primaries()?.len();
    let mut secondaries = 0usize;
    for pid in obs.list_primaries()? {
        secondaries += obs.list_secondaries(pid)?.len();
    }
    println!("    {label:<12}  primaries={primaries:>3}  secondaries={secondaries:>3}  total={:>3}",
        primaries + secondaries);
    Ok(())
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    println!("Loading embedding model (AllMiniLML6V2)…");
    let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)
        .map_err(|e| err_msg(format!("embedding init failed: {e}")))?;
    println!("Model ready.\n");

    let dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let root = dir.path().to_str().unwrap();

    // ── Section 1: Setup and ingestion ────────────────────────────────────────

    println!("════════════════════════════════════════════════════════════");
    println!(" Section 1: Setup and ingestion across four 1-hour shards");
    println!("════════════════════════════════════════════════════════════\n");

    let cache = ShardsCache::new(root, "1h", 4, embedding.clone())?;

    println!("  ShardsCache root : {root}");
    println!("  Shard duration   : 1h  (one DuckDB+FTS+HNSW shard per hour)");
    println!("  Similarity thresh: 0.85 (default)");
    println!("  Shards in cache  : {} (none yet)\n", cache.cached_count());

    let phases = [startup_phase(), peak_phase(), incident_phase(), recovery_phase()];
    let mut total_calls = 0usize;

    for phase in &phases {
        let count = ingest_phase(&cache, phase)?;
        total_calls += count;
        println!(
            "  ingested phase={:<10}  records={}  cache size now {}",
            phase.label, count, cache.cached_count()
        );
    }

    println!("\n  Total add() calls : {total_calls}");
    println!("  Shards in cache   : {}", cache.cached_count());

    // Duplicate submissions — same (key, data) in the same shard.
    // Scoped so dup_shard is dropped before section 6 releases the FTS lock.
    println!("\n  Submitting duplicates (same key+data within the same shard):");
    {
        let dup_shard = cache.shard(UNIX_EPOCH + Duration::from_secs(H2))?;
        for _ in 0..3 {
            let id = dup_shard.add(json!({
                "timestamp": H2 + 500,
                "key": "alert.page",
                "data": "P1 incident declared: service degradation in progress",
            }))?;
            println!("    returned UUID = {id}  (same as original — deduplicated)");
        }
    } // dup_shard dropped → IndexWriter lock for H2 released

    // ── Section 2: Cache and catalog inspection ───────────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 2: Cache and catalog inspection");
    println!("════════════════════════════════════════════════════════════\n");

    println!("  In-memory cache: {} shards", cache.cached_count());

    println!("\n  Per-shard breakdown:");
    shard_stats(&cache, H0, "startup")?;
    shard_stats(&cache, H1, "peak")?;
    shard_stats(&cache, H2, "incident")?;
    shard_stats(&cache, H3, "recovery")?;

    println!("\n  Catalog (ShardInfoEngine) entries:");
    let catalog_entries = cache.info().shards_at(UNIX_EPOCH + Duration::from_secs(H0));
    // shards_at returns shards at a single point; iterate all four known times
    for (base, label) in [(H0,"startup"),(H1,"peak"),(H2,"incident"),(H3,"recovery")] {
        let ts = UNIX_EPOCH + Duration::from_secs(base);
        let entries = cache.info().shards_at(ts)?;
        if let Some(e) = entries.first() {
            let start = e.start_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
            let end   = e.end_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
            println!("    {label:<12}  id={}  [{start} – {end})", e.shard_id);
        }
    }
    drop(catalog_entries); // silence unused warning

    // ── Section 3: Per-shard FTS and vector search ────────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 3: Per-shard FTS and vector search");
    println!("════════════════════════════════════════════════════════════\n");

    // FTS in the incident shard only
    {
        let incident = cache.shard(UNIX_EPOCH + Duration::from_secs(H2))?;

        let fts_queries = [
            ("error",    5),
            ("memory",   5),
            ("timeout",  5),
        ];

        println!("  ── FTS in incident shard (hour 2) ──");
        for (q, limit) in fts_queries {
            let results = incident.search_fts(q, limit)?;
            let surfaced: usize = results.iter()
                .map(|d| d["secondaries"].as_array().map_or(0, |a| a.len()))
                .sum();
            println!("\n  query=\"{q}\"  hits={}  secondaries surfaced={}", results.len(), surfaced);
            for doc in &results {
                show_primary(doc);
            }
        }
    }

    // Vector search comparing startup vs incident
    println!("\n  ── Vector search: startup shard vs incident shard ──");
    let query = json!({ "key": "cpu.usage", "data": "cpu is fully saturated and overloaded" });

    for (base, label) in [(H0, "startup"), (H2, "incident")] {
        let shard = cache.shard(UNIX_EPOCH + Duration::from_secs(base))?;
        let results = shard.search_vector(&query, 3)?;
        println!("\n  shard={label}  query=\"cpu overloaded\"  hits={}", results.len());
        for doc in &results {
            show_primary(doc);
        }
    }

    // ── Section 4: Cross-shard span query ─────────────────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 4: Cross-shard span query  (shards_span)");
    println!("════════════════════════════════════════════════════════════\n");

    // Scoped so the Vec<Shard> collections are dropped before section 6,
    // which calls close() and then re-opens the same shard directories.
    {
        let span_start = UNIX_EPOCH + Duration::from_secs(H1);
        let span_end   = UNIX_EPOCH + Duration::from_secs(H3); // exclusive
        let span_shards = cache.shards_span(span_start, span_end)?;

        println!("  Time window: [{H1}, {H3}) → {} shards (peak + incident)", span_shards.len());
        for (i, shard) in span_shards.iter().enumerate() {
            let obs = shard.observability();
            let primaries = obs.list_primaries()?.len();
            let mut secondaries = 0usize;
            for pid in obs.list_primaries()? {
                secondaries += obs.list_secondaries(pid)?.len();
            }
            println!("    shard[{i}]  primaries={primaries}  secondaries={secondaries}");
        }

        // Full 4-hour span
        let full_start = UNIX_EPOCH + Duration::from_secs(H0);
        let full_end   = UNIX_EPOCH + Duration::from_secs(H3 + 3_600); // past end of hour 3
        let all_shards = cache.shards_span(full_start, full_end)?;
        println!("\n  Full 4-hour window: [{H0}, {}):  {} shards returned",
            H3 + 3_600, all_shards.len());

        // Aggregate across all four shards: count total primaries
        let mut grand_primaries = 0usize;
        let mut grand_secondaries = 0usize;
        for shard in &all_shards {
            let obs = shard.observability();
            let pp = obs.list_primaries()?.len();
            let mut ss = 0usize;
            for pid in obs.list_primaries()? {
                ss += obs.list_secondaries(pid)?.len();
            }
            grand_primaries += pp;
            grand_secondaries += ss;
        }
        println!("  Aggregate across all shards:  primaries={grand_primaries}  secondaries={grand_secondaries}  total={}",
            grand_primaries + grand_secondaries);

        // shards_span on an empty/inverted range
        let empty = cache.shards_span(span_end, span_start)?;
        println!("  Inverted range (end < start): {} shards  (expected 0)", empty.len());
    } // span_shards, all_shards, empty dropped → all IndexWriter locks released

    // ── Section 5: current() ──────────────────────────────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 5: Current-time window  (current)");
    println!("════════════════════════════════════════════════════════════\n");

    // We demonstrate current() with a fresh cache so the results are meaningful.
    let dir2 = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let live_cache = ShardsCache::new(dir2.path().to_str().unwrap(), "1h", 4, embedding.clone())?;

    println!("  current(\"1s\")   → auto-creates the shard for right now");
    let now_shards = live_cache.current("1s")?;
    println!("  shards returned : {}", now_shards.len());

    // Peek into the returned shard — write and read back
    if let Some(live_shard) = now_shards.first() {
        let live_ts = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let id = live_shard.add(json!({
            "timestamp": live_ts,
            "key": "live.probe",
            "data": "live telemetry probe for demo",
        }))?;
        let doc = live_shard.get(id)?.unwrap();
        println!("  wrote live probe  id={id}  key={}", doc["key"]);
        println!("  data: {}", data_preview(&doc["data"]));
    }

    println!("\n  current(\"3h\")   → covers up to 4 hourly buckets");
    let wide_shards = live_cache.current("3h")?;
    println!("  shards returned : {} (≥3 expected)", wide_shards.len());

    println!("\n  current(\"notaduration\") → error");
    match live_cache.current("notaduration") {
        Err(e) => println!("  got expected error: {e}"),
        Ok(_)  => println!("  (unexpected success)"),
    }

    // ── Section 6: Lifecycle — sync, close, reopen ────────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 6: Lifecycle — sync → close → reopen from catalog");
    println!("════════════════════════════════════════════════════════════\n");

    // Snapshot an ID we can verify after reopen.
    let probe_id = {
        let peak_shard = cache.shard(UNIX_EPOCH + Duration::from_secs(H1))?;
        peak_shard.add(json!({
            "timestamp": H1 + 60,
            "key": "lifecycle.probe",
            "data": "this record must survive close and reopen",
        }))?
    };
    println!("  Stored probe id={probe_id}  (in peak shard, hour 1)");

    // sync — flush WAL and HNSW buffers for all cached shards
    cache.sync()?;
    println!("  sync() completed — all {} shards flushed", cache.cached_count());

    // close — flush + evict cache
    let count_before_close = cache.cached_count();
    cache.close()?;
    println!("  close() completed — cache size: {} → {}", count_before_close, cache.cached_count());

    // Reopen — ShardsCache still knows the root and catalog; shard() reopens from disk
    println!("\n  Reopening via shard() — catalog lookup (cold cache)…");
    let peak_reopened = cache.shard(UNIX_EPOCH + Duration::from_secs(H1))?;
    println!("  cache size after first reopen: {}", cache.cached_count());

    let doc = peak_reopened.get(probe_id)?.expect("probe record must survive");
    println!("  probe record retrieved:  key={}  data={}", doc["key"], data_preview(&doc["data"]));

    // All four shards reopen correctly
    for (base, label) in [(H0,"startup"),(H1,"peak"),(H2,"incident"),(H3,"recovery")] {
        cache.shard(UNIX_EPOCH + Duration::from_secs(base))?;
        println!("  reopened {label} shard  (cache size: {})", cache.cached_count());
    }

    // Final sync before exit
    cache.sync()?;
    println!("\n  Final sync() — all {} shards flushed", cache.cached_count());

    println!("\nDone.");
    Ok(())
}

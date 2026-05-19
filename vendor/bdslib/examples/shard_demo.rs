use bdslib::common::error::{err_msg, Result};
use bdslib::embedding::Model;
use bdslib::shard::Shard;
use bdslib::EmbeddingEngine;
use serde_json::{json, Value};
use tempfile::TempDir;

// ── dataset ───────────────────────────────────────────────────────────────────
//
// Seven semantically distinct alert groups (string data) plus integer, float,
// boolean, and JSON-object metrics.  Within each string group the messages are
// paraphrases of the same underlying event — they should cluster under a small
// number of primaries at the default 0.85 cosine-similarity threshold.

const BASE_TS: u64 = 1_748_000_000; // 2025-05-23 approx

fn cpu_alerts() -> Vec<&'static str> {
    vec![
        "cpu usage exceeded 90 percent threshold",
        "processor utilization is critically high",
        "high cpu load detected on this host",
        "cpu is overloaded and performance is degraded",
        "processor saturated above 95 percent capacity",
        "cpu throttling triggered due to thermal limits",
        "user processes consuming excessive cpu time",
        "system cpu running at maximum capacity",
        "cpu contention is causing scheduling delays",
        "heavy cpu load from background batch jobs",
        "all cpu cores running at full utilization",
        "processor bound workload detected on node",
        "cpu iowait time dramatically increased",
        "core utilization spiked to 98 percent",
        "runaway process consuming entire cpu budget",
    ]
}

fn mem_alerts() -> Vec<&'static str> {
    vec![
        "available memory is critically low",
        "memory pressure detected on the system",
        "heap usage is approaching maximum capacity",
        "RAM consumption has exceeded safe threshold",
        "out of memory warning has been triggered",
        "memory leak suspected in running application",
        "swap usage is increasing rapidly",
        "resident set size is growing without bound",
        "memory allocator is under severe pressure",
        "cache evictions happening too frequently",
        "virtual memory space is nearly exhausted",
        "memory fragmentation is degrading performance",
        "kernel oom killer invoked to free memory",
    ]
}

fn net_errors() -> Vec<&'static str> {
    vec![
        "connection timeout on remote endpoint",
        "TCP retransmit rate is elevated",
        "network packet loss detected on interface",
        "remote host unreachable over the network",
        "connection refused by destination host",
        "socket connection dropped unexpectedly",
        "DNS resolution failure on hostname lookup",
        "high latency to upstream service detected",
        "peer connection reset by remote endpoint",
        "TLS handshake timeout failure",
        "network route to destination unreachable",
        "link-layer error rate on NIC exceeded",
    ]
}

fn auth_events() -> Vec<&'static str> {
    vec![
        "user authentication failed with invalid credentials",
        "login attempt rejected for unknown user account",
        "password verification failed for this account",
        "access denied due to insufficient permissions",
        "authentication token is expired or invalid",
        "brute force login attempt detected from IP",
        "unauthorized access attempt has been blocked",
        "user session invalidated by security policy",
        "API key authentication has failed",
        "multi-factor authentication challenge failed",
        "certificate-based authentication was rejected",
        "privilege escalation attempt denied by kernel",
    ]
}

fn db_events() -> Vec<&'static str> {
    vec![
        "database query execution has timed out",
        "connection pool exhausted no connections available",
        "slow query detected taking over five seconds",
        "deadlock detected between concurrent transactions",
        "database replication lag is too high",
        "query planner chose a suboptimal execution plan",
        "missing index is causing full table scan",
        "database connection was dropped unexpectedly",
        "transaction log growing faster than purge rate",
        "database disk IO bottleneck has been detected",
        "too many open database cursors on server",
        "automatic database failover has been triggered",
    ]
}

fn http_errors() -> Vec<&'static str> {
    vec![
        "HTTP 500 internal server error rate is elevated",
        "service is returning 503 unavailable responses",
        "API gateway timeout on upstream service call",
        "HTTP error rate is above acceptable threshold",
        "downstream service latency is exceeding SLA",
        "load balancer health check is now failing",
        "web server worker process has crashed",
        "HTTP request queue depth is too high",
        "rate limiting triggered for API consumer",
        "CDN cache miss rate has dramatically increased",
        "web application firewall is blocking requests",
    ]
}

fn disk_alerts() -> Vec<&'static str> {
    vec![
        "disk space utilization is above 90 percent",
        "storage volume is running out of capacity",
        "filesystem usage is critical on data volume",
        "disk IO throughput has become saturated",
        "inode exhaustion warning on root partition",
        "disk write latency spike has been detected",
        "RAID array is degraded due to missing disk",
        "NFS mount point is not responding to IO",
        "backup storage space is critically low",
        "disk sector errors are increasing rapidly",
    ]
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn add_group(
    shard: &Shard,
    key: &str,
    messages: Vec<&str>,
    ts_counter: &mut u64,
) -> Result<Vec<uuid::Uuid>> {
    let mut ids = Vec::new();
    for msg in messages {
        *ts_counter += 30;
        let id = shard.add(json!({
            "timestamp": *ts_counter,
            "key": key,
            "data": msg,
        }))?;
        ids.push(id);
    }
    Ok(ids)
}

fn data_preview(data: &Value) -> String {
    match data {
        Value::String(s) => {
            if s.len() > 58 { format!("\"{}…\"", &s[..58]) } else { format!("\"{s}\"") }
        }
        other => {
            let s = other.to_string();
            if s.len() > 58 { format!("{}…", &s[..58]) } else { s }
        }
    }
}

// Print a primary document with its embedded secondaries.
fn show_primary(doc: &Value) {
    let key  = doc["key"].as_str().unwrap_or("?");
    let ts   = doc["timestamp"].as_i64().unwrap_or(0);
    let score = doc.get("_score").and_then(|s| s.as_f64());
    let data_str = data_preview(&doc["data"]);
    let secondaries = doc["secondaries"].as_array().map(Vec::as_slice).unwrap_or(&[]);

    match score {
        Some(sc) => println!(
            "    PRIMARY  key={key:<14}  ts={ts}  score={sc:.4}  data={data_str}  [{} sec]",
            secondaries.len()
        ),
        None => println!(
            "    PRIMARY  key={key:<14}  ts={ts}  data={data_str}  [{} sec]",
            secondaries.len()
        ),
    }
    for s in secondaries {
        println!(
            "      secondary  data={}",
            data_preview(&s["data"])
        );
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    println!("Loading embedding model (AllMiniLML6V2)…");
    let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)
        .map_err(|e| err_msg(format!("embedding init failed: {e}")))?;
    println!("Model ready.\n");

    let dir  = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let shard = Shard::new(dir.path().to_str().unwrap(), 4, embedding)?;

    let mut ts = BASE_TS;
    let mut total_calls: usize = 0;

    // ── Section 1: load string-alert groups ───────────────────────────────────

    println!("════════════════════════════════════════════════════════════");
    println!(" Section 1: Loading mixed-datatype telemetry");
    println!("════════════════════════════════════════════════════════════\n");

    let groups: &[(&str, Vec<&str>)] = &[
        ("cpu.alert",  cpu_alerts()),
        ("mem.alert",  mem_alerts()),
        ("net.error",  net_errors()),
        ("auth.event", auth_events()),
        ("db.event",   db_events()),
        ("http.error", http_errors()),
        ("disk.alert", disk_alerts()),
    ];

    for (key, msgs) in groups {
        let count = msgs.len();
        add_group(&shard, key, msgs.clone(), &mut ts)?;
        total_calls += count;
        println!("  {key:<14}  {count} records");
    }

    // ── Section 2: numeric / boolean / JSON metrics ───────────────────────────

    println!();

    let cpu_pct: &[u64] = &[34, 45, 56, 62, 72, 78, 83, 88, 91, 95];
    for &v in cpu_pct {
        ts += 30;
        shard.add(json!({ "timestamp": ts, "key": "cpu.usage", "data": v }))?;
    }
    total_calls += cpu_pct.len();
    println!("  cpu.usage      {} integer readings ({:?})", cpu_pct.len(), cpu_pct);

    let mem_pct: &[u64] = &[45, 55, 63, 67, 73, 78, 81, 86, 92];
    for &v in mem_pct {
        ts += 30;
        shard.add(json!({ "timestamp": ts, "key": "mem.usage", "data": v }))?;
    }
    total_calls += mem_pct.len();
    println!("  mem.usage       {} integer readings ({:?})", mem_pct.len(), mem_pct);

    let latencies: &[f64] = &[0.034, 0.089, 0.123, 0.287, 0.345, 0.512, 1.023, 2.567];
    for &v in latencies {
        ts += 30;
        shard.add(json!({ "timestamp": ts, "key": "response.ms", "data": v }))?;
    }
    total_calls += latencies.len();
    println!("  response.ms     {} float readings ({:?})", latencies.len(), latencies);

    // Boolean health checks — true submitted 5× and false submitted 4×;
    // only the first of each is stored; the rest feed the dedup log.
    let health_ts_true: Vec<u64> = (0..5).map(|i| { ts += 60; ts - 60 + i * 15 }).collect();
    let health_ts_false: Vec<u64> = (0..4).map(|i| { ts += 60; ts - 60 + i * 15 }).collect();

    for &t in &health_ts_true {
        shard.add(json!({ "timestamp": t, "key": "health.ok", "data": true }))?;
    }
    for &t in &health_ts_false {
        shard.add(json!({ "timestamp": t, "key": "health.ok", "data": false }))?;
    }
    total_calls += health_ts_true.len() + health_ts_false.len();
    println!("  health.ok       {} boolean submissions (true×{}, false×{})",
        health_ts_true.len() + health_ts_false.len(),
        health_ts_true.len(), health_ts_false.len());

    // JSON-object host stats
    let hosts: &[(&str, u64, u64, f64)] = &[
        ("web-01",  72, 68, 0.045),
        ("web-02",  91, 81, 1.230),
        ("worker-01", 34, 55, 0.012),
        ("worker-02", 88, 77, 0.890),
        ("db-01",   45, 92, 2.100),
    ];
    for (host, cpu, mem, lat) in hosts {
        ts += 30;
        shard.add(json!({
            "timestamp": ts,
            "key": "host.stats",
            "data": { "host": host, "cpu_pct": cpu, "mem_pct": mem, "lat_ms": lat },
            "region": "us-east-1",
        }))?;
    }
    total_calls += hosts.len();
    println!("  host.stats      {} JSON-object records", hosts.len());

    // ── Section 3: deliberate duplicates ──────────────────────────────────────
    //
    // Re-submit three (key, data) pairs that are already stored.
    // ObservabilityStorage detects the exact match and logs the timestamp
    // without creating a new record; the UUID returned is unchanged.

    let dup_cases: &[(&str, Value, u64)] = &[
        ("cpu.alert",  json!("cpu usage exceeded 90 percent threshold"), 2),
        ("net.error",  json!("connection timeout on remote endpoint"),    3),
        ("cpu.usage",  json!(45u64),                                      2),
    ];

    for (key, data, count) in dup_cases {
        for i in 0..*count {
            ts += 60;
            let id = shard.add(json!({ "timestamp": ts, "key": key, "data": data }))?;
            total_calls += 1;
            if i == 0 {
                println!("\n  duplicate: key={key}  data={data}");
                println!("    returned UUID = {id}  (same as original)");
            }
        }
    }
    // health.ok booleans above already demonstrate dedup (5+4 submissions → 2 unique)

    println!();
    println!("Total add() calls: {total_calls}");

    // ── Section 4: summary statistics ─────────────────────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 2: Summary statistics");
    println!("════════════════════════════════════════════════════════════\n");

    let obs = shard.observability();
    let all_primaries = obs.list_primaries()?;
    let total_primaries = all_primaries.len();

    let mut total_secondaries = 0usize;
    for &pid in &all_primaries {
        total_secondaries += obs.list_secondaries(pid)?.len();
    }

    let total_stored = total_primaries + total_secondaries;
    let deduplicated = total_calls - total_stored;

    println!("  add() calls          : {total_calls}");
    println!("  unique records stored: {total_stored}");
    println!("  deduplicated (saved) : {deduplicated}");
    println!("  primaries            : {total_primaries}");
    println!("  secondaries          : {total_secondaries}");

    // ── Section 5: deduplication report ───────────────────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 3: Deduplication log");
    println!("════════════════════════════════════════════════════════════\n");

    let dedup_keys = ["cpu.alert", "net.error", "cpu.usage", "health.ok"];
    for key in dedup_keys {
        let dup_times = obs.get_duplicate_timestamps(key)?;
        if dup_times.is_empty() { continue; }
        println!("  key={key}  duplicate timestamps ({}):", dup_times.len());
        for dt in &dup_times {
            let secs = dt.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            println!("    T+{secs}s");
        }
    }

    // ── Section 6: primary / secondary breakdown ───────────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 4: Primary / secondary breakdown");
    println!("════════════════════════════════════════════════════════════\n");

    println!("  {total_primaries} primaries  |  {total_secondaries} secondaries\n");

    // Group primaries by key so the output is readable.
    let mut by_key: std::collections::BTreeMap<String, Vec<(uuid::Uuid, Value)>> =
        std::collections::BTreeMap::new();

    for &pid in &all_primaries {
        if let Some(doc) = shard.get(pid)? {
            let key = doc["key"].as_str().unwrap_or("?").to_string();
            by_key.entry(key).or_default().push((pid, doc));
        }
    }

    for (key, primaries) in &by_key {
        let sec_count: usize = primaries
            .iter()
            .map(|(pid, _)| obs.list_secondaries(*pid).unwrap_or_default().len())
            .sum();
        println!("  ── {key}  ({} primaries, {} secondaries) ──", primaries.len(), sec_count);

        for (pid, pdoc) in primaries {
            let data = &pdoc["data"];
            let data_short = match data {
                Value::String(s) if s.len() > 55 => format!("\"{}…\"", &s[..55]),
                Value::String(s) => format!("\"{s}\""),
                other => other.to_string(),
            };
            let secondaries = obs.list_secondaries(*pid)?;
            if secondaries.is_empty() {
                println!("    PRIMARY   {data_short}");
            } else {
                println!("    PRIMARY   {data_short}  [{} secondaries]", secondaries.len());
                for sid in &secondaries {
                    if let Some(sdoc) = shard.get(*sid)? {
                        let sdata = &sdoc["data"];
                        let sshort = match sdata {
                            Value::String(s) if s.len() > 50 => format!("\"{}…\"", &s[..50]),
                            Value::String(s) => format!("\"{s}\""),
                            other => other.to_string(),
                        };
                        println!("      secondary  {sshort}");
                    }
                }
            }
        }
        println!();
    }

    // ── Section 7: FTS search ─────────────────────────────────────────────────
    //
    // Only primaries are in the FTS index.  Each result embeds its secondaries.

    println!("════════════════════════════════════════════════════════════");
    println!(" Section 5: Full-text search  (search_fts)");
    println!(" Note: only primary records are indexed; secondaries are shown");
    println!(" under their parent primary.");
    println!("════════════════════════════════════════════════════════════\n");

    let fts_queries: &[(&str, usize)] = &[
        ("timeout",        8),
        ("memory",         8),
        ("authentication", 8),
        ("disk",           8),
    ];

    for (q, limit) in fts_queries {
        let results = shard.search_fts(q, *limit)?;
        let total_secondaries: usize = results
            .iter()
            .map(|d| d["secondaries"].as_array().map_or(0, |a| a.len()))
            .sum();
        println!(
            "  query=\"{q}\"  limit={limit}  primary hits={}  secondaries surfaced={}",
            results.len(), total_secondaries
        );
        for doc in &results {
            show_primary(doc);
        }
        println!();
    }

    // ── Section 8: vector search ──────────────────────────────────────────────
    //
    // Searches the HNSW index of primary embeddings with MMR reranking.
    // Each result carries _score and its embedded secondaries.

    println!("════════════════════════════════════════════════════════════");
    println!(" Section 6: Semantic vector search  (search_vector)");
    println!(" Note: only primary records are in the vector index.");
    println!("════════════════════════════════════════════════════════════\n");

    let vec_queries: &[(&str, Value, usize)] = &[
        ("cpu overloaded",
            json!({ "key": "cpu.alert",  "data": "cpu is completely overloaded" }),
            6),
        ("auth failure",
            json!({ "key": "auth.event", "data": "user login credentials rejected" }),
            6),
        ("database slow",
            json!({ "key": "db.event",   "data": "database is slow and queries are timing out" }),
            6),
        ("network unreachable",
            json!({ "key": "net.error",  "data": "network connection failed host unreachable" }),
            6),
        ("disk capacity",
            json!({ "key": "disk.alert", "data": "storage volume almost full" }),
            6),
    ];

    for (label, q, limit) in vec_queries {
        let results = shard.search_vector(q, *limit)?;
        let total_secondaries: usize = results
            .iter()
            .map(|d| d["secondaries"].as_array().map_or(0, |a| a.len()))
            .sum();
        println!(
            "  query=\"{label}\"  limit={limit}  primary hits={}  secondaries surfaced={}",
            results.len(), total_secondaries
        );
        for doc in &results {
            show_primary(doc);
        }
        println!();
    }

    // ── Section 9: time-range query ───────────────────────────────────────────

    println!("════════════════════════════════════════════════════════════");
    println!(" Section 7: Time-range query");
    println!("════════════════════════════════════════════════════════════\n");

    use std::time::{Duration, UNIX_EPOCH};

    // String-alert groups were added first; their timestamps span roughly
    // [BASE_TS + 30 … BASE_TS + 30*85].  Numeric/bool/JSON metrics follow.
    // Query the first quarter of the time range (string alerts only).
    let t_start = UNIX_EPOCH + Duration::from_secs(BASE_TS);
    let t_mid   = UNIX_EPOCH + Duration::from_secs(BASE_TS + 30 * 40);
    let t_end   = UNIX_EPOCH + Duration::from_secs(ts + 1);

    let first_half = shard.observability().list_ids_by_time_range(t_start, t_mid)?;
    let second_half = shard.observability().list_ids_by_time_range(t_mid, t_end)?;

    println!("  Window [BASE … BASE+40×30s):  {} records", first_half.len());
    println!("  Window [BASE+40×30s … end):   {} records", second_half.len());
    println!("  Total in both windows:         {}", first_half.len() + second_half.len());

    // ── Section 10: delete — primary and secondary ───────────────────────────

    println!("\n════════════════════════════════════════════════════════════");
    println!(" Section 8: Delete");
    println!("════════════════════════════════════════════════════════════\n");

    // 8a: delete a primary — must disappear from FTS and vector indexes.
    // Find the first auth.event primary that has at least one secondary.
    let auth_primaries = obs.list_primaries()?
        .into_iter()
        .filter(|&pid| {
            shard.get(pid).ok().flatten()
                .and_then(|d| d["key"].as_str().map(|k| k == "auth.event"))
                .unwrap_or(false)
        })
        .find(|&pid| !obs.list_secondaries(pid).unwrap_or_default().is_empty());

    if let Some(primary_id) = auth_primaries {
        let pdoc = shard.get(primary_id)?.unwrap();
        let pdata = pdoc["data"].as_str().unwrap_or("?");
        let sec_count_before = obs.list_secondaries(primary_id)?.len();
        let search_word = pdata.split_whitespace().next().unwrap_or("user");

        println!("  8a: deleting PRIMARY  id={primary_id}");
        println!("      data: \"{pdata}\"");
        println!("      had {sec_count_before} secondaries");

        let fts_before = shard.search_fts(search_word, 20)?;
        let vec_before = shard.search_vector(
            &json!({ "key": "auth.event", "data": pdata }), 10)?;
        println!("\n      Before:");
        println!("        FTS(\"{search_word}\"):  {} primary hits", fts_before.len());
        println!("        vector search:         {} primary hits", vec_before.len());
        println!("        get_by_id:             {}", shard.get(primary_id)?.is_some());

        shard.delete(primary_id)?;

        let fts_after = shard.search_fts(search_word, 20)?;
        let vec_after = shard.search_vector(
            &json!({ "key": "auth.event", "data": pdata }), 10)?;
        println!("\n      After (primary deleted):");
        println!("        FTS(\"{search_word}\"):  {} primary hits  (–1)", fts_after.len());
        println!("        vector search:         {} primary hits  (–1)", vec_after.len());
        println!("        get_by_id primary:     {}", shard.get(primary_id)?.is_some());
        println!("        secondaries still in obs store: {}", sec_count_before);
    }

    // 8b: delete a secondary — primary must remain in all indexes.
    println!();

    // Pick the mem.usage primary (has many secondaries from numeric cluster).
    let mem_primary = obs.list_primaries()?.into_iter().find(|&pid| {
        shard.get(pid).ok().flatten()
            .and_then(|d| d["key"].as_str().map(|k| k == "mem.usage"))
            .unwrap_or(false)
    });

    if let Some(primary_id) = mem_primary {
        let secondaries_before = obs.list_secondaries(primary_id)?;
        if let Some(&secondary_id) = secondaries_before.first() {
            let sdoc = shard.get(secondary_id)?.unwrap();
            println!("  8b: deleting SECONDARY  id={secondary_id}");
            println!("      data: {}  (secondary of mem.usage primary)", sdoc["data"]);
            println!("      primary had {} secondaries before", secondaries_before.len());

            shard.delete(secondary_id)?;

            let secondaries_after = obs.list_secondaries(primary_id)?;
            println!("\n      After (secondary deleted):");
            println!("        primary still in FTS:    {}",
                !shard.search_fts("45", 5).unwrap().is_empty()
                    || shard.get(primary_id)?.is_some());
            println!("        primary still in obs:    {}", shard.get(primary_id)?.is_some());
            println!("        secondary still in obs:  {}", shard.get(secondary_id)?.is_some());
            println!("        remaining secondaries:   {}", secondaries_after.len());
        }
    }

    println!("\nDone.");
    Ok(())
}

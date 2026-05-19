/// ShardsManager + DocumentStorage demo — RAG pattern combining telemetry with documents.
///
/// The scenario: an operations AI assistant for a payment-processing platform.
/// Telemetry (metrics, logs, events) arrives continuously via ShardsManager shards.
/// Runbooks and post-mortems live in the embedded DocumentStorage at dbpath/docstore.
/// When an alert fires the assistant queries both stores: shard search for live incident
/// context, document search for the relevant runbook chunks. The two result sets are
/// assembled into a prompt-ready context block — the core RAG pattern.
///
/// Sections:
///   1.  Construction  — hjson config, ShardsManager::with_embedding, docstore location
///   2.  Telemetry ingestion  — 4 phases × 30 records (peak → incident → mitigation → recovery)
///   3.  Small runbooks (doc_add)  — three short procedure documents, direct retrieval
///   4.  Large documents (doc_add_from_file)  — chunked runbook + post-mortem, chunk inspection
///   5.  Semantic document search  — doc_search_text, doc_search_json
///   6.  RAG: alert → runbook retrieval  — FTS finds incident, doc search finds chunks,
///       context-window expansion assembles the runbook passage
///   7.  Hybrid telemetry+document search  — vector search + doc search in parallel,
///       combined output ready for an LLM prompt
///   8.  Document management  — doc_update_metadata, doc_update_content, doc_delete
///   9.  Fingerprinted output + doc_sync
use bdslib::common::error::{err_msg, Result};
use bdslib::embedding::Model;
use bdslib::shardsmanager::ShardsManager;
use bdslib::EmbeddingEngine;
use serde_json::{json, Value as JsonValue};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use uuid::Uuid;

// ── small runbook documents ────────────────────────────────────────────────────
// These are stored with doc_add as single-record documents (no chunking needed).

const RUNBOOK_CIRCUIT_BREAKER: &str = "\
Circuit Breaker Quick Reference

A circuit breaker prevents cascading failures by stopping calls to an \
unhealthy downstream service. When the error rate exceeds the configured \
threshold the breaker opens and subsequent calls fail fast without contacting \
the overloaded service. State transitions: CLOSED (normal operation) to OPEN \
(failing, no calls pass) to HALF-OPEN (probe calls allowed) back to CLOSED \
(recovered) or OPEN (still failing).

Immediate actions when the payment service circuit breaker opens: check the \
downstream payment processor status dashboard; review recent error logs for \
the affected service; do not force-close the breaker manually unless \
explicitly instructed by the processor team; alert the on-call engineer for \
the downstream service. Monitor the half-open probe interval — the breaker \
resets automatically when consecutive probes succeed. The payment service \
circuit breaker is configured with error_threshold=50%, probe_interval=30s, \
and reset_timeout=60s.";

const RUNBOOK_DB_CONNECTIONS: &str = "\
Database Connection Pool Emergency Response

Symptoms: connection pool exhausted, no connections available, query timeouts \
exceeding SLA, application logs showing pool acquisition timeout errors.

Immediate triage: check current pool utilisation in pg_stat_activity; \
identify long-running queries blocking connections; kill blocking queries \
that have been running over 5 minutes. Check whether the application server \
count has increased recently due to an autoscaler event — each new instance \
consumes pool connections at startup.

Recovery: once blocking queries are terminated the pool recovers within one \
pool_recycle_interval which defaults to 30 seconds. Monitor pg_stat_activity \
until active connections fall below 70 percent of maximum pool capacity. \
Consider temporarily reducing pool_max_size for non-critical background jobs \
to free connections for user-facing request handlers.";

const RUNBOOK_MEMORY_PRESSURE: &str = "\
Memory Pressure Quick Response

Alert triggers: RSS exceeds 85 percent of host memory, OOM killer \
invocations, GC pause time over 500 milliseconds in the past 5 minutes.

Immediate actions: identify the top memory consumer with ps aux sorted by \
memory; check for memory leak indicators including heap growth trend and \
finalizer queue depth. If the process is a payment service worker trigger a \
graceful restart via the control plane — do not SIGKILL, it may leave the \
queue in an inconsistent state. If the OOM killer has already fired collect \
a heap dump from the surviving replica before restarting it.

Do not restart all replicas simultaneously. Use rolling restart to preserve \
capacity. After restarting confirm that the queue depth is draining normally \
and that memory usage stabilises below 60 percent within 5 minutes of restart.";

// ── large runbook and post-mortem documents ────────────────────────────────────
// These are stored with doc_add_from_file and chunked automatically.

const PAYMENT_INCIDENT_RUNBOOK: &str = "\
Payment Service Incident Runbook

This runbook covers P1 and P2 incidents affecting the payment processing \
service. Follow this procedure for any alert involving payment service circuit \
breaker trips, elevated error rates on the /payment/process endpoint, \
or SLA breaches on transaction throughput.

Phase 1: Immediate Triage (0 to 5 minutes)
Acknowledge the alert in PagerDuty within 5 minutes. Check the payment \
service Grafana dashboard for error rate, latency p99, and transaction \
throughput. Identify whether the incident is confined to a single \
availability zone or spans multiple zones.

Determine the blast radius: how many transactions per minute are failing? \
Is the failure total or partial? Check the circuit breaker state in the \
service mesh console. If OPEN, the service is not passing any calls to the \
downstream payment processor. If HALF-OPEN, recovery probes are in progress \
and the situation may resolve within one reset_timeout cycle.

Review the last 15 minutes of structured logs for error patterns. Common \
failure modes include: upstream processor timeout, database connection pool \
exhaustion, authentication service degradation, TLS certificate expiry, \
and configuration drift introduced by a recent deployment.

Phase 2: Contain and Mitigate (5 to 20 minutes)
If a recent deployment caused the incident: initiate an immediate rollback \
via the CI/CD pipeline. Do not wait for a hotfix to be written. Rollback \
typically completes in 4 to 6 minutes and is the lowest-risk mitigation \
for deployment-caused incidents.

If the payment processor upstream is degraded: engage the secondary processor \
by setting PAYMENT_PROCESSOR=secondary in the feature flag store. This routes \
transactions to a higher-fee backup processor but maintains end-user \
availability. Notify the finance team of the processor switch so they can \
reconcile fees.

If database connection pool is exhausted: follow the Database Connection Pool \
Emergency Response runbook. As a first step reduce the maximum connection \
count for non-critical background jobs such as the reconciliation worker and \
the analytics exporter.

If memory pressure is causing OOM restarts: trigger a rolling restart of the \
payment service pods using kubectl rollout restart. Monitor pod readiness \
status before proceeding to the next pod in the rolling update.

Phase 3: Recovery Verification (20 to 40 minutes)
Monitor the error rate for a sustained 10-minute window at below 0.1 percent \
before declaring recovery. Verify transaction throughput has returned to the \
pre-incident baseline within 15 percent. Check that the circuit breaker state \
has transitioned to CLOSED and remains CLOSED for at least 5 minutes. Confirm \
replication lag on the payment database has returned to below 1 second.

Phase 4: Post-Incident Actions
Open a P1 incident ticket with: a precise timeline of events, blast radius \
metrics including total affected transactions, the root cause hypothesis, \
the immediate fix applied, and follow-up action items with owners and \
deadlines. Schedule a postmortem meeting within 48 hours of resolution. \
Update this runbook if any steps were unclear, missing, or contradictory.

Escalation contacts: Payment Platform team in Slack channel payment-oncall. \
VP Engineering if the incident exceeds 30 minutes of customer impact. \
Payment processor account manager if their upstream SLA has been breached.";

const MEMORY_POSTMORTEM: &str = "\
Memory Exhaustion Post-Mortem: Payment Worker OOM Incident

Executive Summary
Payment-processing workers on three of four hosts experienced an \
out-of-memory condition caused by an unbounded in-memory request queue \
combined with a sudden traffic spike. The incident lasted 47 minutes and \
resulted in payment processing degradation affecting approximately 12,000 \
transactions during the peak evening window.

Timeline of Events
T minus 60 minutes: traffic begins increasing above normal evening peak. \
Memory usage rising on worker-02 and worker-03 but within alert thresholds. \
Autoscaler adds two additional application servers but memory growth \
continues on existing workers.

T plus 0 minutes: alert fires for memory pressure exceeding 85 percent \
on worker-02. On-call engineer acknowledges alert and begins investigation.

T plus 8 minutes: OOM killer fires on worker-02. The process restarts \
automatically but the unbounded queue has not been flushed on exit, so \
memory climbs again within minutes of restart.

T plus 14 minutes: worker-03 also hits the OOM threshold and is killed. \
Two of four payment workers are now in a restart loop. Circuit breaker \
opens on the payment service, rejecting all incoming transaction requests.

T plus 22 minutes: on-call engineer identifies the queue as root cause by \
inspecting heap dumps captured during the OOM event. Queue depth at time \
of crash was 480,000 items, each holding a full serialised transaction \
object averaging 8 kilobytes.

T plus 35 minutes: feature flag updated to enable queue backpressure. New \
incoming requests now receive HTTP 503 with Retry-After header rather than \
being enqueued. Memory usage stabilises on surviving workers.

T plus 47 minutes: circuit breaker closes. All four workers healthy. Service \
fully recovered. Queue drains normally over the following 20 minutes.

Root Cause Analysis
The payment processing queue had no maximum depth limit. Under sudden traffic \
spikes the application accepted incoming requests faster than the downstream \
processor could acknowledge them. At 480,000 queued items consuming \
approximately 3.8 gigabytes the container exceeded its 3 GB memory limit.

The backpressure mechanism existed in the codebase but was gated behind a \
feature flag that defaulted to disabled. The flag had not been enabled in \
production because load testing was still in progress at the time of the \
incident.

Contributing factors: no alerting on queue depth independent of memory \
usage; OOM restart did not flush the queue before exit, causing the restart \
loop; autoscaler scaled on CPU rather than queue depth, so new instances \
inherited the overloaded queue instead of sharing it.

Corrective Actions
Enable queue backpressure flag in production immediately. Add monitoring \
alert on queue depth exceeding 50,000 items which gives approximately \
5 minutes of warning before OOM at peak ingestion rate. Modify OOM exit \
hook to flush and discard queue before process exit to prevent restart loops. \
Update autoscaler policy to react to queue depth metric in addition to CPU \
and memory. Complete load testing for backpressure under artificial 10x \
traffic spike before the next peak season to verify the mechanism holds.";

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn aligned_hour(t: u64) -> u64 {
    (t / 3600) * 3600
}

fn hr() {
    println!("════════════════════════════════════════════════════════════════");
}

fn preview_str(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn preview_val(v: &JsonValue) -> String {
    let s = match v {
        JsonValue::String(s) => format!("\"{s}\""),
        other => other.to_string(),
    };
    if s.len() > 72 { format!("{}…", &s[..72]) } else { s }
}

fn show_telemetry(doc: &JsonValue) {
    let key   = doc["key"].as_str().unwrap_or("?");
    let ts    = doc["timestamp"].as_u64().unwrap_or(0);
    let score = doc.get("_score").and_then(|v| v.as_f64());
    let secs  = doc["secondaries"].as_array().map_or(0, |a| a.len());
    match score {
        Some(sc) => println!(
            "    key={key:<18}  ts={ts}  score={sc:.4}  sec={secs}  data={}",
            preview_val(&doc["data"])
        ),
        None => println!(
            "    key={key:<18}  ts={ts}  sec={secs}  data={}",
            preview_val(&doc["data"])
        ),
    }
}

fn show_doc_hit(r: &JsonValue) {
    let score   = r["score"].as_f64().unwrap_or(0.0);
    let doc_name = r["metadata"]["document_name"]
        .as_str()
        .or_else(|| r["metadata"]["name"].as_str())
        .unwrap_or("?");
    let chunk_idx = r["metadata"]["chunk_index"].as_u64();
    let n_chunks  = r["metadata"]["n_chunks"].as_u64();
    let content   = r["document"].as_str().unwrap_or("");
    match (chunk_idx, n_chunks) {
        (Some(ci), Some(nc)) => println!(
            "    [{score:.3}]  chunk [{ci}/{nc}]  {doc_name}"),
        _ => println!(
            "    [{score:.3}]  {doc_name}"),
    }
    println!("           {}", preview_str(content, 90));
}

fn fetch_chunk_text(mgr: &ShardsManager, doc_meta: &JsonValue, idx: usize) -> String {
    let id_str = doc_meta["chunks"][idx].as_str().unwrap_or("");
    let id: Uuid = id_str.parse().unwrap();
    let bytes = mgr.doc_get_content(id).unwrap().unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

// ── telemetry record generation ────────────────────────────────────────────────

struct Phase {
    label:    &'static str,
    base_ts:  u64,
    cpu:      u64,
    mem:      u64,
    latency:  u64,
    log_errors: &'static [&'static str],
    log_warns:  &'static [&'static str],
    log_infos:  &'static [&'static str],
}

fn telemetry_phases(now: u64) -> [Phase; 4] {
    [
        Phase {
            label: "peak",
            base_ts: aligned_hour(now - 3 * 3600),
            cpu: 74, mem: 68, latency: 210,
            log_errors: &[],
            log_warns: &[
                "request queue depth exceeded soft limit of 500",
                "cache hit rate dropped below 60 percent",
                "worker thread pool under high pressure at 85 percent",
            ],
            log_infos: &[
                "autoscaler triggered scale-out for web tier",
                "rate limiter engaged for burst traffic",
            ],
        },
        Phase {
            label: "incident",
            base_ts: aligned_hour(now - 2 * 3600),
            cpu: 93, mem: 87, latency: 1_850,
            log_errors: &[
                "out of memory warning triggered on worker-02",
                "health check failing on web-02 for 3 consecutive polls",
                "automatic circuit breaker opened on payment service",
                "connection pool exhausted no connections available",
                "deadlock detected between concurrent transactions",
                "TCP retransmit rate elevated on eth0",
                "disk write latency spike detected on data volume",
            ],
            log_warns: &[
                "replication lag exceeded 30 seconds on standby",
                "retry budget 80 percent consumed on payment client",
            ],
            log_infos: &[
                "P1 incident declared service degradation in progress",
            ],
        },
        Phase {
            label: "mitigation",
            base_ts: aligned_hour(now - 1 * 3600),
            cpu: 81, mem: 79, latency: 680,
            log_errors: &[
                "circuit breaker still open on payment service",
                "memory pressure persisting on worker-02",
            ],
            log_warns: &[
                "failover to secondary payment processor in progress",
                "request shedding active to protect database",
            ],
            log_infos: &[
                "on-call engineer acknowledged incident",
                "rolling restart of payment worker tier initiated",
                "database standby promoted to primary",
            ],
        },
        Phase {
            label: "recovery",
            base_ts: aligned_hour(now - 300),
            cpu: 38, mem: 48, latency: 55,
            log_errors: &[],
            log_warns: &["cache still warming after restart"],
            log_infos: &[
                "circuit breaker closed payment service recovered",
                "replication lag back to normal on standby",
                "connection pool recovered to normal utilisation",
                "incident resolved all services operating normally",
                "postmortem scheduled for next business day",
            ],
        },
    ]
}

fn generate_phase_records(p: &Phase) -> Vec<JsonValue> {
    let mut records = Vec::new();
    let mut ts = p.base_ts;

    macro_rules! push {
        ($key:expr, $data:expr) => {{
            ts += 60;
            records.push(json!({"timestamp": ts, "key": $key, "data": $data}));
        }};
    }

    // Metrics
    for i in 0u64..6 {
        push!("cpu.usage",  p.cpu  + (i * 3) % 11);
        push!("mem.usage",  p.mem  + (i * 2) % 9);
        push!("net.latency", p.latency + (i * 7) % 50);
        push!("http.request", json!({"rps": 1200u64 + i * 100, "p99_ms": p.latency}));
    }

    for msg in p.log_errors { push!("log.error", msg); }
    for msg in p.log_warns  { push!("log.warn",  msg); }
    for msg in p.log_infos  { push!("log.info",  msg); }

    // Host events
    for i in 0u64..3 {
        let host = format!("worker-0{}", i + 1);
        push!("host.event", json!({"host": host, "cpu_pct": p.cpu + i, "mem_pct": p.mem}));
    }

    records
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    println!("Loading AllMiniLML6V2 embedding model…");
    let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)
        .map_err(|e| err_msg(format!("embedding init: {e}")))?;
    println!("Model ready.\n");

    let root_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let root     = root_dir.path().to_str().unwrap();
    let doc_dir  = TempDir::new().map_err(|e| err_msg(e.to_string()))?;

    // ── Section 1: Construction ───────────────────────────────────────────────

    hr();
    println!(" Section 1: Construction — config, ShardsManager, docstore location");
    hr();

    let config_path = format!("{root}/manager.hjson");
    let db_path     = format!("{root}/db");
    let hjson = format!(
        "{{\n  dbpath: \"{db_path}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n}}\n"
    );
    fs::write(&config_path, &hjson).map_err(|e| err_msg(e.to_string()))?;

    let mgr = ShardsManager::with_embedding(&config_path, embedding)?;

    println!("\n  ShardsManager ready");
    println!("  dbpath:          {db_path}");
    println!("  shard_duration:  1h");
    println!("  docstore path:   {db_path}/docstore   ← embedded DocumentStorage");
    println!("  embedding model: AllMiniLML6V2 (shared by shards and docstore)");
    println!();

    // ── Section 2: Telemetry ingestion ────────────────────────────────────────

    hr();
    println!(" Section 2: Telemetry ingestion — 4 phases via ShardsManager");
    hr();

    let now = now_secs();
    let phases = telemetry_phases(now);
    let mut total = 0usize;

    println!();
    for phase in &phases {
        let docs  = generate_phase_records(phase);
        let count = docs.len();
        mgr.add_batch(docs)?;
        total += count;
        println!(
            "  phase={:<12}  base_ts={}  records={}  shards cached={}",
            phase.label, phase.base_ts, count, mgr.cache().cached_count()
        );
    }
    println!("\n  Total records ingested: {total}");

    // ── Section 3: Small runbooks (doc_add) ───────────────────────────────────

    hr();
    println!(" Section 3: Small runbooks — doc_add (single-record documents)");
    hr();
    println!();

    let meta_cb = json!({
        "name": "Circuit Breaker Quick Reference",
        "category": "runbook", "service": "payment",
        "severity": "P1", "type": "reference",
    });
    let id_cb = mgr.doc_add(meta_cb, RUNBOOK_CIRCUIT_BREAKER.as_bytes())?;
    println!("  doc_add: Circuit Breaker Quick Reference   → {id_cb}");

    let meta_db = json!({
        "name": "Database Connection Pool Emergency",
        "category": "runbook", "service": "database",
        "severity": "P1", "type": "procedure",
    });
    let id_db = mgr.doc_add(meta_db, RUNBOOK_DB_CONNECTIONS.as_bytes())?;
    println!("  doc_add: Database Connection Pool Emergency → {id_db}");

    let meta_mem = json!({
        "name": "Memory Pressure Quick Response",
        "category": "runbook", "service": "payment",
        "severity": "P2", "type": "procedure",
    });
    let id_mem = mgr.doc_add(meta_mem, RUNBOOK_MEMORY_PRESSURE.as_bytes())?;
    println!("  doc_add: Memory Pressure Quick Response    → {id_mem}");

    // Direct retrieval — round-trip verification
    println!();
    let retrieved_meta = mgr.doc_get_metadata(id_cb)?.unwrap();
    let retrieved_bytes = mgr.doc_get_content(id_cb)?.unwrap();
    println!("  doc_get_metadata({id_cb}):");
    println!("    name:     {}", retrieved_meta["name"]);
    println!("    category: {}  service: {}  severity: {}",
        retrieved_meta["category"], retrieved_meta["service"], retrieved_meta["severity"]);
    println!("  doc_get_content: {} bytes  preview: \"{}\"",
        retrieved_bytes.len(),
        preview_str(&String::from_utf8_lossy(&retrieved_bytes), 72));

    // ── Section 4: Large documents (doc_add_from_file) ────────────────────────

    println!();
    hr();
    println!(" Section 4: Large documents — doc_add_from_file (chunked storage)");
    hr();
    println!();

    // Write source files
    let runbook_path = doc_dir.path().join("payment_runbook.txt");
    let postmortem_path = doc_dir.path().join("memory_postmortem.txt");
    fs::write(&runbook_path, PAYMENT_INCIDENT_RUNBOOK).map_err(|e| err_msg(e.to_string()))?;
    fs::write(&postmortem_path, MEMORY_POSTMORTEM).map_err(|e| err_msg(e.to_string()))?;

    println!("  Source files written:");
    println!("    payment_runbook.txt    {} chars", PAYMENT_INCIDENT_RUNBOOK.len());
    println!("    memory_postmortem.txt  {} chars", MEMORY_POSTMORTEM.len());
    println!();

    // Ingest — different slice/overlap settings per document
    let runbook_id = mgr.doc_add_from_file(
        runbook_path.to_str().unwrap(),
        "Payment Service Incident Runbook",
        220, 20.0,
    )?;
    let runbook_meta = mgr.doc_get_metadata(runbook_id)?.unwrap();
    println!("  doc_add_from_file: Payment Service Incident Runbook");
    println!("    doc_id:   {runbook_id}");
    println!("    slice:    220  overlap: 20.0%  n_chunks: {}", runbook_meta["n_chunks"]);

    let pm_id = mgr.doc_add_from_file(
        postmortem_path.to_str().unwrap(),
        "Memory Exhaustion Post-Mortem",
        260, 15.0,
    )?;
    let pm_meta = mgr.doc_get_metadata(pm_id)?.unwrap();
    println!("  doc_add_from_file: Memory Exhaustion Post-Mortem");
    println!("    doc_id:   {pm_id}");
    println!("    slice:    260  overlap: 15.0%  n_chunks: {}", pm_meta["n_chunks"]);

    // Inspect chunk structure of the payment runbook
    println!();
    println!("  Chunk structure — Payment Service Incident Runbook:");
    let rb_chunks = runbook_meta["chunks"].as_array().unwrap();
    println!("    total chunks: {}", rb_chunks.len());
    for i in 0..rb_chunks.len().min(3) {
        let cid: Uuid = rb_chunks[i].as_str().unwrap().parse().unwrap();
        let cmeta  = mgr.doc_get_metadata(cid)?.unwrap();
        let cbytes = mgr.doc_get_content(cid)?.unwrap();
        println!("    chunks[{i}]  id={cid}");
        println!("      chunk_index={} / n_chunks={}",
            cmeta["chunk_index"], cmeta["n_chunks"]);
        println!("      {} bytes  preview: \"{}\"",
            cbytes.len(),
            preview_str(&String::from_utf8_lossy(&cbytes), 80));
    }
    if rb_chunks.len() > 3 {
        println!("    … {} more chunks", rb_chunks.len() - 3);
    }

    // ── Section 5: Semantic document search ───────────────────────────────────

    println!();
    hr();
    println!(" Section 5: Semantic document search — doc_search_text, doc_search_json");
    hr();

    let searches: &[(&str, usize, &str)] = &[
        ("circuit breaker payment service reset procedure", 3, "circuit breaker runbook"),
        ("memory exhaustion OOM queue depth restart", 3, "memory OOM post-mortem"),
        ("database connection pool exhausted recovery", 3, "db connection pool runbook"),
    ];

    for (query, limit, label) in searches {
        println!("\n  doc_search_text(\"{label}\", limit={limit}):");
        let results = mgr.doc_search_text(query, *limit)?;
        for r in &results { show_doc_hit(r); }
    }

    // JSON metadata query — field names participate in the semantic signal
    println!("\n  doc_search_json({{category: runbook, service: payment, severity: P1}}, limit=3):");
    let json_results = mgr.doc_search_json(
        &json!({"category": "runbook", "service": "payment", "severity": "P1"}),
        3,
    )?;
    for r in &json_results { show_doc_hit(r); }

    // ── Section 6: RAG — alert → runbook retrieval ────────────────────────────

    println!();
    hr();
    println!(" Section 6: RAG — telemetry alert triggers runbook retrieval");
    hr();

    // Step 1: telemetry alert — FTS finds incident records
    println!("\n  Step 1: FTS alert query — search_fts(\"6h\", \"circuit breaker\")");
    let alert_hits = mgr.search_fts("6h", "circuit breaker")?;
    println!("  {} matching telemetry records:", alert_hits.len());
    for doc in alert_hits.iter().take(4) { show_telemetry(doc); }
    if alert_hits.len() > 4 { println!("    … {} more", alert_hits.len() - 4); }

    // Construct a runbook query from the alert context
    let rag_query = "circuit breaker payment service open triage mitigation";
    println!("\n  Step 2: construct runbook query from alert context");
    println!("  query: \"{rag_query}\"");

    // Step 3: semantic search over the document store
    println!("\n  Step 3: doc_search_text(query, limit=4)");
    let doc_hits = mgr.doc_search_text(rag_query, 4)?;
    for r in &doc_hits { show_doc_hit(r); }

    // Step 4: find the first chunked result and expand its context window.
    // Small documents (added via doc_add) have no "document_id" in metadata;
    // chunked documents (from doc_add_from_file) carry "document_id" so we can
    // walk the ordered chunk list from the document-level record.
    let chunk_hit = doc_hits.iter().find(|r| {
        r["metadata"]["document_id"].as_str().is_some()
    });

    if let Some(top) = chunk_hit {
        println!("\n  Step 4: context-window expansion from top chunked hit");

        let hit_chunk_index = top["metadata"]["chunk_index"].as_u64().unwrap_or(0) as usize;
        let hit_doc_id_str  = top["metadata"]["document_id"].as_str().unwrap();
        let hit_doc_id: Uuid = hit_doc_id_str.parse().unwrap();
        let hit_score       = top["score"].as_f64().unwrap_or(0.0);
        let hit_doc_name    = top["metadata"]["document_name"].as_str().unwrap_or("?");

        println!("    hit:  score={hit_score:.3}  chunk_index={hit_chunk_index}");
        println!("    doc:  {hit_doc_name}");
        println!("    doc_id: {hit_doc_id_str}");

        // Load document-level metadata → ordered chunk list
        let doc_meta   = mgr.doc_get_metadata(hit_doc_id)?.unwrap();
        let doc_chunks = doc_meta["chunks"].as_array().unwrap();
        let n_chunks   = doc_chunks.len();
        let lo = hit_chunk_index.saturating_sub(1);
        let hi = (hit_chunk_index + 1).min(n_chunks - 1);

        println!("    n_chunks={n_chunks}  expanding to chunks[{lo}..={hi}]");

        let mut context_parts: Vec<String> = Vec::new();
        for idx in lo..=hi {
            let text = fetch_chunk_text(&mgr, &doc_meta, idx);
            println!("    chunks[{idx}]: {}", preview_str(&text, 80));
            context_parts.push(text);
        }
        let expanded_context = context_parts.iter()
            .map(|s| s.trim())
            .collect::<Vec<_>>()
            .join(" ");

        // Step 5: assemble the LLM prompt context block
        println!();
        println!("  Step 5: combined prompt context block");
        println!("  ┌─────────────────────────────────────────────────────────────");
        println!("  │ [TELEMETRY ALERT CONTEXT]");
        for doc in alert_hits.iter().take(3) {
            let key      = doc["key"].as_str().unwrap_or("?");
            let data_str = doc["data"].to_string();
            let data     = doc["data"].as_str().unwrap_or(&data_str);
            println!("  │   {key}: {}", preview_str(data, 65));
        }
        println!("  │");
        println!("  │ [RUNBOOK CONTEXT — {hit_doc_name}  chunks {lo}–{hi}/{n_chunks}]");
        println!("  │   ({} chars)", expanded_context.len());
        println!("  │   \"{}\"", preview_str(&expanded_context, 220));
        println!("  └─────────────────────────────────────────────────────────────");
    } else if let Some(top) = doc_hits.first() {
        // All hits are small documents — use the whole content as context.
        println!("\n  Step 4: top hit is a whole document (not chunked)");
        let doc_name  = top["metadata"]["name"].as_str().unwrap_or("?");
        let hit_score = top["score"].as_f64().unwrap_or(0.0);
        let content   = top["document"].as_str().unwrap_or("");
        println!("    doc:  {doc_name}  score={hit_score:.3}");
        println!("\n  Step 5: combined prompt context block");
        println!("  ┌─────────────────────────────────────────────────────────────");
        println!("  │ [TELEMETRY ALERT CONTEXT]");
        for doc in alert_hits.iter().take(3) {
            let key      = doc["key"].as_str().unwrap_or("?");
            let data_str = doc["data"].to_string();
            let data     = doc["data"].as_str().unwrap_or(&data_str);
            println!("  │   {key}: {}", preview_str(data, 65));
        }
        println!("  │");
        println!("  │ [RUNBOOK CONTEXT — {doc_name}]");
        println!("  │   \"{}\"", preview_str(content, 220));
        println!("  └─────────────────────────────────────────────────────────────");
    }

    // ── Section 7: Hybrid vector+document search ──────────────────────────────

    println!();
    hr();
    println!(" Section 7: Hybrid telemetry+document search for LLM prompt assembly");
    hr();

    // Vector search over telemetry shards
    let vec_query = json!({"key": "log.error", "data": "connection pool exhausted database"});
    println!("\n  vectorsearch(\"6h\", connection pool exhausted, limit=5):");
    let vec_hits = mgr.vectorsearch("6h", &vec_query, 5)?;
    println!("  {} telemetry hits (uuid, ts, score):", vec_hits.len());
    for (id, ts, score) in vec_hits.iter().take(4) {
        println!("    {id}  ts={ts}  score={score:.4}");
    }

    // Parallel: semantic document search for the same failure mode
    let db_rag_query = "database connection pool exhausted recovery no connections available";
    println!("\n  doc_search_text(\"{}\", limit=3):", &db_rag_query[..55]);
    let db_doc_hits = mgr.doc_search_text(db_rag_query, 3)?;
    for r in &db_doc_hits { show_doc_hit(r); }

    // Assemble combined output
    if let Some(top_doc) = db_doc_hits.first() {
        let doc_score   = top_doc["score"].as_f64().unwrap_or(0.0);
        let doc_name    = top_doc["metadata"]["document_name"]
            .as_str()
            .or_else(|| top_doc["metadata"]["name"].as_str())
            .unwrap_or("?");
        let chunk_idx   = top_doc["metadata"]["chunk_index"].as_u64().unwrap_or(0) as usize;
        let doc_id_str  = top_doc["metadata"]["document_id"]
            .as_str()
            .unwrap_or("");

        // Context expansion for the top doc hit
        let (lo, hi, expanded) = if !doc_id_str.is_empty() {
            let doc_id: Uuid = doc_id_str.parse().unwrap();
            let doc_m = mgr.doc_get_metadata(doc_id)?.unwrap();
            let chunks = doc_m["chunks"].as_array().unwrap();
            let lo = chunk_idx.saturating_sub(1);
            let hi = (chunk_idx + 1).min(chunks.len() - 1);
            let mut parts = Vec::new();
            for idx in lo..=hi {
                parts.push(fetch_chunk_text(&mgr, &doc_m, idx));
            }
            let ctx = parts.iter().map(|s| s.trim()).collect::<Vec<_>>().join(" ");
            (lo, hi, ctx)
        } else {
            // small doc: use the document content directly
            let content = top_doc["document"].as_str().unwrap_or("").to_string();
            (0, 0, content)
        };

        println!();
        println!("  Combined RAG prompt context:");
        println!("  ┌─────────────────────────────────────────────────────────────");
        println!("  │ [LIVE TELEMETRY — {} matching records via vector search]",
            vec_hits.len());
        for (id, ts, score) in vec_hits.iter().take(3) {
            println!("  │   id={id}  ts={ts}  score={score:.4}");
        }
        println!("  │");
        println!("  │ [RUNBOOK — {doc_name}]");
        println!("  │   score={doc_score:.3}  chunks[{lo}–{hi}]  ({} chars):",
            expanded.len());
        println!("  │   \"{}\"", preview_str(&expanded, 220));
        println!("  └─────────────────────────────────────────────────────────────");
    }

    // ── Section 8: Document management ────────────────────────────────────────

    println!();
    hr();
    println!(" Section 8: Document management — doc_update_*, doc_delete");
    hr();
    println!();

    // doc_update_metadata: add a revision field to the circuit breaker runbook
    let updated_meta = json!({
        "name": "Circuit Breaker Quick Reference",
        "category": "runbook", "service": "payment",
        "severity": "P1", "type": "reference",
        "revision": 2, "reviewed_by": "oncall-platform",
    });
    mgr.doc_update_metadata(id_cb, updated_meta)?;
    let after_meta = mgr.doc_get_metadata(id_cb)?.unwrap();
    println!("  doc_update_metadata(circuit_breaker_runbook):");
    println!("    revision: {}  reviewed_by: {}",
        after_meta["revision"], after_meta["reviewed_by"]);

    // doc_update_content: append a note to the memory pressure runbook
    let new_content = format!(
        "{RUNBOOK_MEMORY_PRESSURE}\n\nRevision note: queue backpressure feature flag \
         is now enabled in production by default as of the last deployment."
    );
    mgr.doc_update_content(id_mem, new_content.as_bytes())?;
    let after_bytes = mgr.doc_get_content(id_mem)?.unwrap();
    println!("\n  doc_update_content(memory_pressure_runbook):");
    println!("    content length before: {} bytes", RUNBOOK_MEMORY_PRESSURE.len());
    println!("    content length after : {} bytes", after_bytes.len());

    // doc_delete: remove the database connection pool runbook
    println!("\n  doc_delete(db_connections_runbook)  id={id_db}");
    mgr.doc_delete(id_db)?;
    let after_delete = mgr.doc_get_metadata(id_db)?;
    println!("    doc_get_metadata after delete: {:?}", after_delete);
    println!("    confirmed gone: {}", after_delete.is_none());

    // Verify remaining two small docs still searchable
    let remaining = mgr.doc_search_text("circuit breaker memory pressure payment", 5)?;
    println!("\n  doc_search_text after delete ({} results, db runbook absent):",
        remaining.len());
    for r in &remaining {
        let doc_name = r["metadata"]["document_name"]
            .as_str()
            .or_else(|| r["metadata"]["name"].as_str())
            .unwrap_or("?");
        let score = r["score"].as_f64().unwrap_or(0.0);
        println!("    [{score:.3}]  {doc_name}");
    }

    // ── Section 9: Fingerprinted output + doc_sync ────────────────────────────

    println!();
    hr();
    println!(" Section 9: Fingerprinted strings + doc_sync");
    hr();
    println!();

    // doc_search_text_strings: flat canonical output ready for re-embedding or FTS
    let fp_query = "payment service incident response phase triage";
    println!("  doc_search_text_strings(\"{fp_query}\", limit=3)");
    println!("  Each result: json_fingerprint of {{id, metadata, document, score}}\n");
    let fp_results = mgr.doc_search_text_strings(fp_query, 3)?;
    for (i, s) in fp_results.iter().enumerate() {
        println!("  [{i}] {}", preview_str(s, 120));
    }
    println!();

    // doc_sync — flush HNSW index for the docstore
    mgr.doc_sync()?;
    println!("  doc_sync() — HNSW vector index flushed to disk");

    // Clone sharing: add a doc via clone, visible through original
    let mgr2 = mgr.clone();
    let clone_id = mgr2.doc_add(
        json!({"name": "Clone-written probe", "category": "test"}),
        b"This document was added through a ShardsManager clone.",
    )?;
    let via_original = mgr.doc_get_metadata(clone_id)?;
    println!("  Clone: doc_add via clone → {clone_id}");
    println!("  Original sees clone's doc: {}", via_original.is_some());

    println!();
    hr();
    println!(" Done.");
    println!("  Telemetry:  {total} records across {} shards", mgr.cache().cached_count());
    println!("  Docstore:   3 small runbooks + 2 large chunked documents");
    println!("  RAG:        FTS alert → semantic chunk search → context expansion → prompt block");
    hr();

    Ok(())
}

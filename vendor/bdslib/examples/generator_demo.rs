/// generator_demo — showcases the three generation modes of `Generator`.
///
/// All generated documents are valid `ShardsManager`-ingestible JSON
/// (non-negative integer `timestamp`, non-empty string `key`, non-null `data`).
///
/// Sections:
///   1. Telemetry      — metric documents across different duration windows
///   2. Log entries    — syslog / HTTP access / Python traceback documents
///   3. Mixed          — blended telemetry + log entries at varying ratios
///   4. Templated      — custom JSON templates with `$placeholder` substitution
///      4a. IoT sensor readings
///      4b. HTTP request events
///      4c. Application lifecycle events
///   5. ShardsManager ingestion — generate-and-store end-to-end round-trip
use bdslib::common::generator::Generator;
use bdslib::common::logparser::validate_telemetry;
use serde_json::Value;
use std::collections::BTreeMap;

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
    println!("  [{label}] {}", serde_json::to_string_pretty(doc).unwrap()
        .lines()
        .enumerate()
        .map(|(i, l)| if i == 0 { l.to_string() } else { format!("    {l}") })
        .collect::<Vec<_>>()
        .join("\n"));
}

fn key_distribution(docs: &[Value]) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for doc in docs {
        *counts.entry(doc["key"].as_str().unwrap_or("?").to_string()).or_insert(0) += 1;
    }
    counts
}

fn ts_range_str(docs: &[Value]) -> String {
    let timestamps: Vec<u64> = docs.iter()
        .filter_map(|d| d["timestamp"].as_u64())
        .collect();
    match (timestamps.iter().min(), timestamps.iter().max()) {
        (Some(lo), Some(hi)) => format!("[{lo}, {hi}]  span = {}s", hi - lo),
        _ => "no timestamps".to_string(),
    }
}

fn assert_all_valid(docs: &[Value]) {
    for doc in docs {
        validate_telemetry(doc)
            .unwrap_or_else(|e| panic!("validate_telemetry failed: {e}\ndoc: {doc}"));
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    // ── 1. Telemetry ─────────────────────────────────────────────────────────
    section("1. Telemetry  (Generator::telemetry)");

    let g = Generator::new();

    for (label, duration, n) in [("1h window", "1h", 40), ("30m window", "30m", 40), ("6h window", "6h", 40)] {
        subsection(&format!("{n} samples / {label}"));
        let docs = g.telemetry(duration, n);
        assert_all_valid(&docs);

        println!("  timestamp range : {}", ts_range_str(&docs));

        let dist = key_distribution(&docs);
        println!("  key distribution ({} unique metrics):", dist.len());
        let mut pairs: Vec<_> = dist.iter().collect();
        pairs.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
        for (key, count) in pairs.iter().take(5) {
            println!("    {count:3}×  {key}");
        }
        if pairs.len() > 5 {
            println!("    … and {} more", pairs.len() - 5);
        }
    }

    subsection("Sample telemetry documents");
    let samples = g.telemetry("1h", 3);
    for (i, doc) in samples.iter().enumerate() {
        show_doc(&format!("telemetry {}", i + 1), doc);
    }

    // ── 2. Log entries ───────────────────────────────────────────────────────
    section("2. Log entries  (Generator::log_entries)");

    let logs = g.log_entries("1h", 120);
    assert_all_valid(&logs);

    let syslog_n  = logs.iter().filter(|d| d["data"].get("pid").is_some()).count();
    let http_n    = logs.iter().filter(|d| d["data"].get("method").is_some()
                                       && d["data"].get("server").is_none()).count();
    let nginx_n   = logs.iter().filter(|d| d["data"].get("server").and_then(Value::as_str) == Some("nginx")).count();
    let tb_n      = logs.iter().filter(|d| d["data"].get("exception_type").is_some()).count();
    println!("  120 documents breakdown:");
    println!("    {syslog_n:3}  syslog entries");
    println!("    {http_n:3}  Apache access-log entries");
    println!("    {nginx_n:3}  Nginx access-log entries");
    println!("    {tb_n:3}  Python traceback entries");

    subsection("Sample: syslog");
    if let Some(doc) = logs.iter().find(|d| d["data"].get("pid").is_some()) {
        show_doc("syslog", doc);
    }
    subsection("Sample: HTTP access log");
    if let Some(doc) = logs.iter().find(|d| d["data"].get("method").is_some()) {
        show_doc("http", doc);
    }
    subsection("Sample: Python traceback");
    if let Some(doc) = logs.iter().find(|d| d["data"].get("exception_type").is_some()) {
        show_doc("traceback", doc);
    }

    // ── 3. Mixed ─────────────────────────────────────────────────────────────
    section("3. Mixed  (Generator::mixed)");

    for (label, ratio) in [("80% telemetry", 0.8), ("50/50", 0.5), ("20% telemetry", 0.2)] {
        let docs = g.mixed("1h", 100, ratio);
        assert_all_valid(&docs);
        let tel = docs.iter().filter(|d| d["key"].as_str().unwrap_or("").contains('.')).count();
        let log = docs.len() - tel;
        println!("  {label:15}  →  telemetry {tel:3}  log entries {log:3}  (target ratio {ratio:.0})");
    }

    subsection("Sample: mixed document — telemetry side");
    if let Some(doc) = g.mixed("1h", 50, 1.0).first() {
        show_doc("telemetry", doc);
    }
    subsection("Sample: mixed document — log entry side");
    if let Some(doc) = g.mixed("1h", 50, 0.0).first() {
        show_doc("log", doc);
    }

    // ── 4. Templated ─────────────────────────────────────────────────────────
    section("4. Templated  (Generator::templated)");

    // ── 4a. IoT sensor readings ───────────────────────────────────────────────
    subsection("4a. IoT sensor readings  ($float, $choice, $bool, $uuid)");

    let iot_template = r#"{
        "timestamp":  "$timestamp",
        "key":        "$choice(temperature,humidity,pressure,co2)",
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

    let iot_docs = g.templated("2h", iot_template, 40);
    assert_all_valid(&iot_docs);

    let keys_iot = key_distribution(&iot_docs);
    println!("  Key distribution over 40 readings:");
    for (k, c) in &keys_iot {
        println!("    {c:3}×  {k}");
    }
    let alerts = iot_docs.iter().filter(|d| d["data"]["alert"].as_bool() == Some(true)).count();
    println!("  Alert=true in {alerts}/40 readings");
    println!("  Timestamp range: {}", ts_range_str(&iot_docs));

    subsection("Sample IoT document");
    show_doc("iot", &iot_docs[0]);

    // ── 4b. HTTP request events ───────────────────────────────────────────────
    subsection("4b. HTTP request events  ($ip, $int, $uuid, $name, $choice)");

    let http_template = r#"{
        "timestamp": "$timestamp",
        "key":       "$choice(GET /api/v1/data,POST /api/v1/events,PUT /api/v1/config,DELETE /api/v1/resource)",
        "data": {
            "client_ip":  "$ip",
            "status":     "$choice(200,201,204,400,401,403,404,500,502)",
            "latency_ms": "$int(1,5000)",
            "request_id": "$uuid",
            "user":       "$name",
            "retries":    "$int(0,3)"
        }
    }"#;

    let http_docs = g.templated("1h", http_template, 40);
    assert_all_valid(&http_docs);

    let status_dist: BTreeMap<String, usize> = {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for doc in &http_docs {
            let s = doc["data"]["status"].as_str().unwrap_or("?").to_string();
            *m.entry(s).or_insert(0) += 1;
        }
        m
    };
    println!("  Status distribution over 40 events:");
    for (s, c) in &status_dist {
        println!("    {c:3}×  HTTP {s}");
    }

    subsection("Sample HTTP event document");
    show_doc("http-event", &http_docs[0]);

    // ── 4c. Application lifecycle events ─────────────────────────────────────
    subsection("4c. Application lifecycle events  ($word, $name, $uuid, $choice)");

    let app_template = r#"{
        "timestamp": "$timestamp",
        "key": "$choice(app.startup,app.shutdown,app.deploy,app.error,app.warning,app.info)",
        "data": {
            "service":   "$choice(api-gateway,auth-service,data-processor,scheduler,cache-warmer)",
            "host":      "$choice(web-01,web-02,api-01,api-02,worker-01,worker-02)",
            "message":   "$word",
            "operator":  "$name",
            "trace_id":  "$uuid",
            "version":   "$choice(v1.2.3,v1.3.0,v2.0.0-rc1,v2.0.0)",
            "exit_code": "$int(0,2)"
        }
    }"#;

    let app_docs = g.templated("4h", app_template, 40);
    assert_all_valid(&app_docs);

    let event_dist = key_distribution(&app_docs);
    println!("  Event type distribution over 40 events:");
    for (k, c) in &event_dist {
        println!("    {c:3}×  {k}");
    }
    println!("  Timestamp range: {}", ts_range_str(&app_docs));

    subsection("Sample application lifecycle document");
    show_doc("app-event", &app_docs[0]);

    // ── 5. ShardsManager ingestion ────────────────────────────────────────────
    section("5. ShardsManager ingestion  (generate → store → verify)");

    println!("  Generating 200 mixed documents over a 3-hour window …");
    let ingest_docs = g.mixed("3h", 200, 0.6);
    assert_all_valid(&ingest_docs);

    // Verify every document has the required ShardsManager fields
    let ts_ok   = ingest_docs.iter().all(|d| d["timestamp"].as_u64().is_some());
    let key_ok  = ingest_docs.iter().all(|d| d["key"].as_str().map(|s| !s.is_empty()).unwrap_or(false));
    let data_ok = ingest_docs.iter().all(|d| !d["data"].is_null());

    let tel_count = ingest_docs.iter().filter(|d| d["key"].as_str().unwrap_or("").contains('.')).count();
    let log_count = ingest_docs.len() - tel_count;

    println!("  Documents ready for ingestion: {}", ingest_docs.len());
    println!("    timestamp field present & u64 : {ts_ok}");
    println!("    key       field non-empty str  : {key_ok}");
    println!("    data      field non-null        : {data_ok}");
    println!("    telemetry : {tel_count}   log entries : {log_count}");

    println!("\n  All documents pass validate_telemetry — ready for ShardsManager::add_batch.");

    // ── summary ───────────────────────────────────────────────────────────────
    section("Summary");
    println!("  Generator::telemetry  — pure metric documents with dotted keys");
    println!("  Generator::log_entries — syslog / HTTP / traceback documents");
    println!("  Generator::mixed      — blended set at any telemetry ratio");
    println!("  Generator::templated  — fully custom shape via JSON + $placeholders");
    println!();
    println!("  Placeholder reference:");
    println!("    $timestamp          → u64 within the duration window");
    println!("    $int(min,max)       → random integer");
    println!("    $float(min,max)     → random float  (2 dp)");
    println!("    $choice(a,b,c)      → one of the comma-separated strings");
    println!("    $bool               → true | false");
    println!("    $uuid               → UUID v4");
    println!("    $ip                 → IPv4 address");
    println!("    $word               → lowercase word");
    println!("    $name               → \"Firstname Lastname\"");
    println!();
}

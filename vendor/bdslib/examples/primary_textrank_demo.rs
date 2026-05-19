/// primary_textrank_demo — TextRank summarisation over primary records.
///
/// Demonstrates `ShardsManager::summary_for_recent` and
/// `ShardsManager::summary_for_query` on a synthetic corpus that mixes:
///
///   * structured numeric telemetry (filtered out by the body extractor)
///   * structured numeric measurements via `data["value"]` (also filtered)
///   * text records via `data["value"]` (used as TextRank input)
///   * text records via `data["raw"]` (also used as input)
///   * a cluster of structurally similar log entries that should dominate the
///     summary
///
/// Run with:
///
///     cargo run --example primary_textrank_demo

use bdslib::{EmbeddingEngine, ShardsManager, TextRankConfig};
use fastembed::EmbeddingModel;
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use uuid::Uuid;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn print_section(title: &str) {
    println!("\n=== {title} ===");
}

fn main() {
    // ── Section 0: Setup ─────────────────────────────────────────────────────

    print_section("Section 0 — Setup");

    let dir = TempDir::new().expect("tempdir");
    let cfg = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    std::fs::write(
        &cfg,
        format!(
            "{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n  similarity_threshold: 0.99\n}}"
        ),
    )
    .unwrap();
    let engine = EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None).unwrap();
    let mgr = ShardsManager::with_embedding(cfg.to_str().unwrap(), engine).unwrap();
    println!("Created ShardsManager at {}", dbpath);

    // ── Section 1: Ingest a mixed corpus ─────────────────────────────────────

    print_section("Section 1 — Ingesting mixed records");

    let t = now();
    // Numeric telemetry — must be filtered out by the body extractor.
    mgr.add(json!({ "timestamp": t - 600, "key": "cpu.usage",    "data": 12.5 })).unwrap();
    mgr.add(json!({ "timestamp": t - 590, "key": "cpu.usage",    "data": 13.0 })).unwrap();
    mgr.add(json!({ "timestamp": t - 580, "key": "mem.used_pct", "data": { "value": 81.0 } })).unwrap();
    mgr.add(json!({ "timestamp": t - 570, "key": "mem.used_pct", "data": { "value": 84.0 } })).unwrap();

    // Recurring web-server failure cluster — drives the summary.
    mgr.add(json!({ "timestamp": t - 500, "key": "log.web", "data": { "value": "nginx upstream timeout 502 service=auth" } })).unwrap();
    mgr.add(json!({ "timestamp": t - 480, "key": "log.web", "data": { "value": "nginx upstream timeout 502 service=billing" } })).unwrap();
    mgr.add(json!({ "timestamp": t - 460, "key": "log.web", "data": { "value": "nginx upstream timeout 502 service=catalog" } })).unwrap();
    mgr.add(json!({ "timestamp": t - 440, "key": "log.web", "data": { "value": "nginx upstream timeout 502 service=auth" } })).unwrap();

    // Auth events — secondary cluster.
    mgr.add(json!({ "timestamp": t - 400, "key": "log.auth", "data": { "raw": "user alice logged in successfully" } })).unwrap();
    mgr.add(json!({ "timestamp": t - 380, "key": "log.auth", "data": { "raw": "user bob logged in successfully" } })).unwrap();
    mgr.add(json!({ "timestamp": t - 360, "key": "log.auth", "data": { "raw": "user carol logged in successfully" } })).unwrap();

    // One-off noise — should rank lowest.
    mgr.add(json!({ "timestamp": t - 200, "key": "log.cron", "data": { "value": "scheduled cron job started" } })).unwrap();
    mgr.add(json!({ "timestamp": t - 100, "key": "log.misc", "data": { "value": "the cafeteria menu has been updated" } })).unwrap();

    println!("Ingested 13 records (4 numeric, 9 text)");

    // ── Section 2: summary_for_recent ────────────────────────────────────────

    print_section("Section 2 — summary_for_recent (last 1 hour)");

    let txn = Uuid::now_v7();
    let cfg_default = TextRankConfig::default();
    let summary = mgr
        .summary_for_recent(txn, Duration::from_secs(3600), &cfg_default)
        .unwrap();
    println!("Default config (auto-sized to ~30%):\n  {summary}");

    let cfg_capped = TextRankConfig { max_sentences: 2, ..TextRankConfig::default() };
    let summary = mgr
        .summary_for_recent(txn, Duration::from_secs(3600), &cfg_capped)
        .unwrap();
    println!("\nCapped to 2 sentences:\n  {summary}");

    // Numeric-only window: fixed lookback that only catches the four numeric
    // records.  The summariser must return the empty string.
    let numeric_only = mgr
        .summary_for_recent(txn, Duration::from_secs(700), &cfg_default)
        .unwrap();
    let numeric_window_test_passed = !numeric_only.contains("12.5") && !numeric_only.contains("81");
    println!("\nNumeric values leaked into summary? {}", !numeric_window_test_passed);

    // ── Section 3: summary_for_query ─────────────────────────────────────────

    print_section("Section 3 — summary_for_query");

    let summary = mgr
        .summary_for_query(txn, "nginx upstream timeout", &cfg_capped)
        .unwrap();
    println!("Query \"nginx upstream timeout\" (top-2):\n  {summary}");

    let summary = mgr
        .summary_for_query(txn, "user logged in", &cfg_capped)
        .unwrap();
    println!("\nQuery \"user logged in\" (top-2):\n  {summary}");

    // ── Section 4: Edge cases ────────────────────────────────────────────────

    print_section("Section 4 — Edge cases");

    // Vacant window
    let none = mgr
        .summary_for_recent(txn, Duration::from_secs(1), &cfg_default)
        .unwrap();
    println!("Vacant 1-second window → {:?}", none);

    // Query with no matches whatsoever
    let no_match = mgr
        .summary_for_query(txn, "completely unrelated quantum chromodynamics", &cfg_default)
        .unwrap();
    println!("Off-topic query → {:?}", no_match);

    println!("\nDone.");
}

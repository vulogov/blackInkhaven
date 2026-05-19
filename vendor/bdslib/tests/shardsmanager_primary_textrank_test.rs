//! Tests for `ShardsManager::summary_for_recent` and
//! `ShardsManager::summary_for_query` — extractive TextRank summarisation
//! over primary observability records.

use bdslib::{EmbeddingEngine, ShardsManager, TextRankConfig};
use fastembed::EmbeddingModel;
use serde_json::json;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use uuid::Uuid;

// ── shared fastembed model ────────────────────────────────────────────────────

static ENGINE: OnceLock<EmbeddingEngine> = OnceLock::new();

fn get_engine() -> &'static EmbeddingEngine {
    ENGINE.get_or_init(|| EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None).unwrap())
}

// ── fixtures ──────────────────────────────────────────────────────────────────

fn tmp_manager(duration: &str) -> (TempDir, ShardsManager) {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    let content = format!(
        "{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"{duration}\"\n  pool_size: 4\n  similarity_threshold: 0.99\n}}"
    );
    std::fs::write(&config_path, content).unwrap();
    let mgr =
        ShardsManager::with_embedding(config_path.to_str().unwrap(), get_engine().clone()).unwrap();
    (dir, mgr)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn add_text_record(mgr: &ShardsManager, key: &str, ts: u64, text: &str) -> Uuid {
    mgr.add(json!({
        "timestamp": ts,
        "key":       key,
        "data":      { "value": text },
    }))
    .unwrap()
}

fn add_text_record_via_raw(mgr: &ShardsManager, key: &str, ts: u64, text: &str) -> Uuid {
    mgr.add(json!({
        "timestamp": ts,
        "key":       key,
        "data":      { "raw": text },
    }))
    .unwrap()
}

fn add_numeric_record(mgr: &ShardsManager, key: &str, ts: u64, n: f64) -> Uuid {
    mgr.add(json!({
        "timestamp": ts,
        "key":       key,
        "data":      n,
    }))
    .unwrap()
}

fn add_numeric_value_record(mgr: &ShardsManager, key: &str, ts: u64, n: f64) -> Uuid {
    mgr.add(json!({
        "timestamp": ts,
        "key":       key,
        "data":      { "value": n },
    }))
    .unwrap()
}

// ── summary_for_recent ────────────────────────────────────────────────────────

#[test]
fn summary_for_recent_empty_window_returns_empty_string() {
    let (_dir, mgr) = tmp_manager("1h");
    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_recent(txn, Duration::from_secs(60), &TextRankConfig::default())
        .unwrap();
    assert_eq!(out, "");
}

#[test]
fn summary_for_recent_skips_numeric_data() {
    // All records are numeric → the function must return an empty string,
    // not panic, and not crash on stop-word-only fingerprints.
    let (_dir, mgr) = tmp_manager("1h");
    let now = now_secs();
    add_numeric_record(&mgr, "cpu.usage", now - 10, 12.5);
    add_numeric_record(&mgr, "cpu.usage", now - 20, 13.5);
    add_numeric_value_record(&mgr, "mem.used_pct", now - 30, 81.0);

    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_recent(txn, Duration::from_secs(3600), &TextRankConfig::default())
        .unwrap();
    assert_eq!(out, "", "all-numeric window must produce empty summary; got {out:?}");
}

#[test]
fn summary_for_recent_extracts_value_string() {
    let (_dir, mgr) = tmp_manager("1h");
    let now = now_secs();
    // Distinct-but-similar text records — at threshold 0.99, none are
    // collapsed to secondaries, so every one reaches the summariser.
    add_text_record(&mgr, "log.app", now - 10, "user alice logged in successfully");
    add_text_record(&mgr, "log.app", now - 20, "user bob logged in successfully");
    add_text_record(&mgr, "log.app", now - 30, "user carol logged in successfully");
    add_text_record(&mgr, "log.app", now - 40, "scheduled cron job started");

    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_recent(txn, Duration::from_secs(3600), &TextRankConfig::default())
        .unwrap();
    let lower = out.to_lowercase();
    assert!(
        lower.contains("user") && lower.contains("logged"),
        "summary should surface the recurring login pattern, got: {out:?}"
    );
}

#[test]
fn summary_for_recent_falls_back_to_raw_when_value_missing() {
    let (_dir, mgr) = tmp_manager("1h");
    let now = now_secs();
    add_text_record_via_raw(&mgr, "log.sys", now - 10, "kernel watchdog reset");
    add_text_record_via_raw(&mgr, "log.sys", now - 20, "kernel watchdog reset");
    add_text_record_via_raw(&mgr, "log.sys", now - 30, "kernel watchdog reset");

    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_recent(txn, Duration::from_secs(3600), &TextRankConfig::default())
        .unwrap();
    assert!(
        out.to_lowercase().contains("watchdog"),
        "raw fallback should surface watchdog token, got: {out:?}"
    );
}

#[test]
fn summary_for_recent_mixes_text_and_numeric_skips_numeric() {
    let (_dir, mgr) = tmp_manager("1h");
    let now = now_secs();
    // Half the data is numeric noise that must be filtered out.
    add_numeric_record(&mgr, "cpu.usage", now - 10, 12.5);
    add_numeric_record(&mgr, "cpu.usage", now - 20, 13.0);
    add_numeric_value_record(&mgr, "mem.used_pct", now - 30, 81.0);
    add_text_record(&mgr, "log.app", now - 40, "database connection refused");
    add_text_record(&mgr, "log.app", now - 50, "database connection refused");
    add_text_record(&mgr, "log.app", now - 60, "database connection refused");

    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_recent(txn, Duration::from_secs(3600), &TextRankConfig::default())
        .unwrap();
    assert!(
        out.to_lowercase().contains("database") && out.to_lowercase().contains("connection"),
        "text records should drive the summary, numerics should be ignored; got: {out:?}"
    );
    // Numeric values must not appear as bare numbers.
    assert!(!out.contains("12.5"), "numeric value 12.5 leaked: {out:?}");
    assert!(!out.contains("81"),   "numeric value 81 leaked: {out:?}");
}

#[test]
fn summary_for_recent_respects_lookback_window() {
    let (_dir, mgr) = tmp_manager("1h");
    let now = now_secs();
    // In-window text record.
    add_text_record(&mgr, "log.app", now - 30, "fresh event in window");
    // Out-of-window text record (10 minutes ago) — must not appear.
    add_text_record(&mgr, "log.app", now - 600, "ancient event from long ago");

    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_recent(txn, Duration::from_secs(60), &TextRankConfig::default())
        .unwrap();
    assert!(out.contains("fresh event"),  "in-window record missing: {out:?}");
    assert!(!out.contains("ancient"),     "out-of-window record leaked: {out:?}");
}

// ── summary_for_query ─────────────────────────────────────────────────────────

#[test]
fn summary_for_query_empty_store_returns_empty() {
    let (_dir, mgr) = tmp_manager("1h");
    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_query(txn, "anything", &TextRankConfig::default())
        .unwrap();
    assert_eq!(out, "");
}

#[test]
fn summary_for_query_skips_numeric_results() {
    let (_dir, mgr) = tmp_manager("1h");
    let now = now_secs();
    add_numeric_record(&mgr, "cpu.usage", now - 10, 12.5);
    add_numeric_record(&mgr, "cpu.usage", now - 20, 13.5);
    add_numeric_value_record(&mgr, "mem.used_pct", now - 30, 81.0);

    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_query(txn, "cpu utilisation", &TextRankConfig::default())
        .unwrap();
    assert_eq!(out, "", "numeric-only matches must yield empty summary; got {out:?}");
}

#[test]
fn summary_for_query_returns_text_for_relevant_records() {
    let (_dir, mgr) = tmp_manager("1h");
    let now = now_secs();
    // Cluster of structurally similar text records that match the query.
    add_text_record(&mgr, "log.web", now - 10, "nginx upstream connection refused 502");
    add_text_record(&mgr, "log.web", now - 20, "nginx upstream connection refused 502");
    add_text_record(&mgr, "log.web", now - 30, "nginx upstream connection refused 502");
    // Unrelated records.
    add_numeric_record(&mgr, "cpu.usage", now - 40, 5.0);
    add_text_record(&mgr, "log.cron", now - 50, "scheduled cron job started");

    let txn = Uuid::now_v7();
    let out = mgr
        .summary_for_query(txn, "nginx upstream connection refused", &TextRankConfig::default())
        .unwrap();
    let lower = out.to_lowercase();
    assert!(
        lower.contains("nginx") || lower.contains("upstream") || lower.contains("connection"),
        "query summary should surface matched text record tokens, got: {out:?}"
    );
}

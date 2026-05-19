/// lsa_primary_textrank_demo — LSA extractive summarisation over primary records.
///
/// Demonstrates `ShardsManager::summary_lsa_for_recent` and
/// `ShardsManager::summary_lsa_for_query` applied to a synthetic mix of text
/// events and numeric measurements. The demo mirrors `primary_textrank_demo.rs`
/// but uses the LSA backend instead of TextRank.
///
/// Run with:
///
/// ```bash
/// cargo run --example lsa_primary_textrank_demo
/// ```

use bdslib::{EmbeddingEngine, LsaConfig, ShardsManager};
use fastembed::EmbeddingModel;
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use uuid::Uuid;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn main() {
    let dir = TempDir::new().expect("tempdir");
    let config_path = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    std::fs::write(
        &config_path,
        format!(
            "{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n  similarity_threshold: 0.99\n}}"
        ),
    )
    .unwrap();

    let engine = EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None)
        .expect("embedding engine");
    let mgr = ShardsManager::with_embedding(config_path.to_str().unwrap(), engine)
        .expect("ShardsManager");

    let now = now_secs();

    // ── Ingest mix of text events and numeric measurements ────────────────────

    println!("Ingesting records…");

    // Recurring error theme (3×).
    for i in 0..3u64 {
        mgr.add(json!({
            "timestamp": now - i * 15 - 5,
            "key":       "log.web",
            "data":      { "value": "nginx upstream connection refused 502" },
        })).unwrap();
    }

    // Recurring login theme (3×).
    for i in 0..3u64 {
        mgr.add(json!({
            "timestamp": now - i * 20 - 10,
            "key":       "log.auth",
            "data":      { "value": format!("user alice logged in from 10.0.0.{}", i + 1) },
        })).unwrap();
    }

    // Numeric measurements — must be ignored by the summariser.
    for i in 0..5u64 {
        mgr.add(json!({
            "timestamp": now - i * 12 - 3,
            "key":       "cpu.usage",
            "data":      (12.5 + i as f64),
        })).unwrap();
        mgr.add(json!({
            "timestamp": now - i * 12 - 6,
            "key":       "mem.used_pct",
            "data":      { "value": (80.0 + i as f64) },
        })).unwrap();
    }

    // One-off noise line.
    mgr.add(json!({
        "timestamp": now - 90,
        "key":       "log.cron",
        "data":      { "raw": "scheduled cron job completed successfully" },
    })).unwrap();

    // ── summary_lsa_for_recent ─────────────────────────────────────────────────

    println!("\n=== summary_lsa_for_recent (default config, 1h window) ===");
    let summary = mgr
        .summary_lsa_for_recent(Uuid::now_v7(), Duration::from_secs(3600), &LsaConfig::default())
        .unwrap();
    println!("Summary:\n  {summary}");

    println!("\n=== summary_lsa_for_recent (max_sentences=2) ===");
    let cfg = LsaConfig { max_sentences: 2, ..LsaConfig::default() };
    let summary = mgr
        .summary_lsa_for_recent(Uuid::now_v7(), Duration::from_secs(3600), &cfg)
        .unwrap();
    println!("Summary:\n  {summary}");

    println!("\n=== summary_lsa_for_recent (n_concepts=1, captures single dominant theme) ===");
    let cfg = LsaConfig { n_concepts: 1, max_sentences: 2, ..LsaConfig::default() };
    let summary = mgr
        .summary_lsa_for_recent(Uuid::now_v7(), Duration::from_secs(3600), &cfg)
        .unwrap();
    println!("Summary:\n  {summary}");

    // ── summary_lsa_for_query ──────────────────────────────────────────────────

    println!("\n=== summary_lsa_for_query — 'nginx upstream error' ===");
    let summary = mgr
        .summary_lsa_for_query(Uuid::now_v7(), "nginx upstream error", &LsaConfig::default())
        .unwrap();
    println!("Summary:\n  {summary}");

    println!("\n=== summary_lsa_for_query — 'user login authentication' ===");
    let summary = mgr
        .summary_lsa_for_query(Uuid::now_v7(), "user login authentication", &LsaConfig::default())
        .unwrap();
    println!("Summary:\n  {summary}");

    println!("\n=== summary_lsa_for_query — empty store query ===");
    let empty_dir = TempDir::new().unwrap();
    let empty_cfg = empty_dir.path().join("c.hjson");
    std::fs::write(&empty_cfg, format!("{{ dbpath: \"{}\"\n shard_duration: \"1h\" pool_size: 4 similarity_threshold: 0.99 }}", empty_dir.path().join("db").display())).unwrap();
    let empty_engine = EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None).unwrap();
    let empty_mgr = ShardsManager::with_embedding(empty_cfg.to_str().unwrap(), empty_engine).unwrap();
    let out = empty_mgr.summary_lsa_for_query(Uuid::now_v7(), "anything", &LsaConfig::default()).unwrap();
    println!("Empty store → {:?}", out);

    println!("\nDone.");
}

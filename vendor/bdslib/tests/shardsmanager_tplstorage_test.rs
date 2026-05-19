use bdslib::{EmbeddingEngine, ShardsManager};
use fastembed::EmbeddingModel;
use serde_json::json;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// ── shared model ──────────────────────────────────────────────────────────────

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
        "{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"{duration}\"\n  pool_size: 4\n}}"
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

fn tpl_meta(name: &str, ts: u64) -> serde_json::Value {
    json!({ "name": name, "timestamp": ts, "type": "template" })
}

fn tpl_body(name: &str) -> Vec<u8> {
    format!("Template body for: {name}").into_bytes()
}

// ── template_by_id ────────────────────────────────────────────────────────────

#[test]
fn test_template_by_id_finds_stored_template() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts = now_secs();
    let id = mgr.tpl_add(tpl_meta("runbook.cpu", ts), &tpl_body("cpu")).unwrap();
    let result = mgr.template_by_id(&id.to_string()).unwrap();
    assert!(result.is_some(), "stored template must be found");
}

#[test]
fn test_template_by_id_returns_id_metadata_body_fields() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts = now_secs();
    let meta = tpl_meta("alert.disk", ts);
    let body = b"Check disk usage on all volumes";
    let id = mgr.tpl_add(meta.clone(), body).unwrap();
    let result = mgr.template_by_id(&id.to_string()).unwrap().unwrap();

    assert_eq!(result["id"].as_str().unwrap(), id.to_string());
    assert_eq!(result["metadata"]["name"], "alert.disk");
    assert_eq!(result["body"].as_str().unwrap(), "Check disk usage on all volumes");
}

#[test]
fn test_template_by_id_nonexistent_returns_none() {
    let (_dir, mgr) = tmp_manager("1h");
    let fake_id = uuid::Uuid::now_v7().to_string();
    let result = mgr.template_by_id(&fake_id).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_template_by_id_invalid_uuid_returns_err() {
    let (_dir, mgr) = tmp_manager("1h");
    let err = mgr.template_by_id("not-a-uuid").unwrap_err();
    assert!(err.to_string().contains("invalid template id"));
}

#[test]
fn test_template_by_id_scans_across_shards() {
    // Two templates routed to different hourly shards.
    let (_dir, mgr) = tmp_manager("1h");
    let ts1: u64 = 1_748_001_600; // hour boundary
    let ts2: u64 = 1_748_005_200; // next hour boundary
    let id1 = mgr.tpl_add(tpl_meta("tpl.a", ts1), b"body a").unwrap();
    let id2 = mgr.tpl_add(tpl_meta("tpl.b", ts2), b"body b").unwrap();

    // Both must be findable regardless of which shard they live in.
    assert!(mgr.template_by_id(&id1.to_string()).unwrap().is_some());
    assert!(mgr.template_by_id(&id2.to_string()).unwrap().is_some());
}

// ── templates_by_timestamp ────────────────────────────────────────────────────

#[test]
fn test_templates_by_timestamp_returns_matching_templates() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts: u64 = 1_748_000_000;
    mgr.tpl_add(tpl_meta("tpl.x", ts), b"body x").unwrap();

    let results = mgr.templates_by_timestamp(ts - 100, ts + 100).unwrap();
    assert!(!results.is_empty(), "template within range must be returned");
    assert!(results.iter().any(|r| r["metadata"]["name"] == "tpl.x"));
}

#[test]
fn test_templates_by_timestamp_excludes_outside_range() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts_in: u64  = 1_748_000_500;
    let ts_out: u64 = 1_748_001_600; // different shard
    mgr.tpl_add(tpl_meta("inside",  ts_in),  b"inside").unwrap();
    mgr.tpl_add(tpl_meta("outside", ts_out), b"outside").unwrap();

    let results = mgr.templates_by_timestamp(ts_in - 10, ts_in + 10).unwrap();
    assert!(results.iter().any(|r| r["metadata"]["name"] == "inside"),
        "inside template must appear");
    assert!(!results.iter().any(|r| r["metadata"]["name"] == "outside"),
        "outside template must not appear");
}

#[test]
fn test_templates_by_timestamp_empty_store_returns_empty() {
    let (_dir, mgr) = tmp_manager("1h");
    let results = mgr.templates_by_timestamp(1_000_000, 2_000_000).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_templates_by_timestamp_boundary_inclusive() {
    let (_dir, mgr) = tmp_manager("1h");
    let start: u64 = 1_748_000_100;
    let end: u64   = 1_748_000_200;
    mgr.tpl_add(tpl_meta("at.start", start), b"s").unwrap();
    mgr.tpl_add(tpl_meta("at.end",   end),   b"e").unwrap();

    let results = mgr.templates_by_timestamp(start, end).unwrap();
    assert!(results.iter().any(|r| r["metadata"]["name"] == "at.start"),
        "template at start boundary must be included");
    assert!(results.iter().any(|r| r["metadata"]["name"] == "at.end"),
        "template at end boundary must be included");
}

#[test]
fn test_templates_by_timestamp_result_has_id_metadata_body() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts: u64 = 1_748_000_000;
    let id = mgr.tpl_add(tpl_meta("schema.check", ts), b"check body").unwrap();

    let results = mgr.templates_by_timestamp(ts - 1, ts + 1).unwrap();
    let found = results.iter().find(|r| r["id"] == id.to_string()).unwrap();
    assert!(found.get("id").is_some());
    assert!(found.get("metadata").is_some());
    assert!(found.get("body").is_some());
    assert_eq!(found["body"], "check body");
}

#[test]
fn test_templates_by_timestamp_deduplicates_across_shards() {
    // Two templates with the same timestamp in a single shard; confirm no duplication.
    let (_dir, mgr) = tmp_manager("1h");
    let ts: u64 = 1_748_000_000;
    mgr.tpl_add(tpl_meta("a", ts), b"a").unwrap();
    mgr.tpl_add(tpl_meta("b", ts), b"b").unwrap();

    let results = mgr.templates_by_timestamp(ts - 1, ts + 1).unwrap();
    let ids: Vec<_> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len(), "no duplicate template ids in results");
}

// ── templates_recent ──────────────────────────────────────────────────────────

#[test]
fn test_templates_recent_returns_freshly_added_template() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts = now_secs();
    let id = mgr.tpl_add(tpl_meta("runbook.fresh", ts), b"fresh body").unwrap();

    let results = mgr.templates_recent("5min").unwrap();
    assert!(results.iter().any(|r| r["id"] == id.to_string()),
        "freshly added template must appear in recent(5min)");
}

#[test]
fn test_templates_recent_excludes_old_template() {
    let (_dir, mgr) = tmp_manager("1h");
    let old_ts = now_secs().saturating_sub(7200); // 2 hours ago — in a different shard
    let new_ts = now_secs();
    mgr.tpl_add(tpl_meta("old.tpl", old_ts), b"old").unwrap();
    let new_id = mgr.tpl_add(tpl_meta("new.tpl", new_ts), b"new").unwrap();

    let results = mgr.templates_recent("1h").unwrap();
    assert!(results.iter().any(|r| r["id"] == new_id.to_string()),
        "recent template must be present");
    assert!(!results.iter().any(|r| r["metadata"]["name"] == "old.tpl"),
        "old template must not appear in 1h recent window");
}

#[test]
fn test_templates_recent_empty_store_returns_empty() {
    let (_dir, mgr) = tmp_manager("1h");
    let results = mgr.templates_recent("1h").unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_templates_recent_invalid_duration_returns_err() {
    let (_dir, mgr) = tmp_manager("1h");
    // Trigger shard creation so there is something to iterate over.
    let ts = now_secs();
    let _ = mgr.tpl_add(tpl_meta("x", ts), b"x").unwrap();
    let err = mgr.templates_recent("not-a-duration").unwrap_err();
    assert!(err.to_string().contains("invalid duration"));
}

#[test]
fn test_templates_recent_result_has_id_metadata_body() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts = now_secs();
    let id = mgr.tpl_add(tpl_meta("runbook.net", ts), b"check networking").unwrap();

    let results = mgr.templates_recent("2min").unwrap();
    let found = results.iter().find(|r| r["id"] == id.to_string()).unwrap();
    assert_eq!(found["metadata"]["name"], "runbook.net");
    assert_eq!(found["body"].as_str().unwrap(), "check networking");
}

#[test]
fn test_templates_recent_multiple_shards() {
    // Templates at current time and 3 hours ago (different shards).
    // Only the current one should appear in a 1h window.
    let (_dir, mgr) = tmp_manager("1h");
    let now = now_secs();
    let three_h_ago = now.saturating_sub(3 * 3600);

    let id_now = mgr.tpl_add(tpl_meta("now.tpl", now), b"now").unwrap();
    let _id_old = mgr.tpl_add(tpl_meta("old.tpl", three_h_ago), b"old").unwrap();

    let results = mgr.templates_recent("1h").unwrap();
    assert!(results.iter().any(|r| r["id"] == id_now.to_string()));
    assert!(!results.iter().any(|r| r["metadata"]["name"] == "old.tpl"));
}

#[test]
fn test_templates_recent_various_duration_formats() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts = now_secs();
    let id = mgr.tpl_add(tpl_meta("dur.test", ts), b"body").unwrap();
    let id_str = id.to_string();

    for dur in &["30s", "5min", "1h", "1days"] {
        let results = mgr.templates_recent(dur).unwrap();
        assert!(results.iter().any(|r| r["id"] == id_str),
            "template must appear in recent({dur})");
    }
}

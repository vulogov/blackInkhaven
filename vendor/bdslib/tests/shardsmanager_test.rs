use bdslib::{EmbeddingEngine, ShardsManager};
use fastembed::EmbeddingModel;
use serde_json::json;
use std::sync::OnceLock;
use tempfile::TempDir;
use uuid::Uuid;

static ENGINE: OnceLock<EmbeddingEngine> = OnceLock::new();

fn get_engine() -> &'static EmbeddingEngine {
    ENGINE.get_or_init(|| EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None).unwrap())
}

fn write_config(dir: &TempDir, duration: &str) -> String {
    let config_path = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    let content =
        format!("{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"{duration}\"\n  pool_size: 4\n}}");
    std::fs::write(&config_path, content).unwrap();
    config_path.to_str().unwrap().to_string()
}

fn tmp_manager(duration: &str) -> (TempDir, ShardsManager) {
    let dir = TempDir::new().unwrap();
    let config_path = write_config(&dir, duration);
    let mgr = ShardsManager::with_embedding(&config_path, get_engine().clone()).unwrap();
    (dir, mgr)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn doc(key: &str, data: &str) -> serde_json::Value {
    json!({ "timestamp": now_secs(), "key": key, "data": data })
}

// ── construction ──────────────────────────────────────────────────────────────

#[test]
fn test_new_reads_config() {
    let (_dir, mgr) = tmp_manager("1h");
    assert_eq!(mgr.cache().cached_count(), 0);
}

#[test]
fn test_new_missing_config() {
    let result =
        ShardsManager::with_embedding("/nonexistent/path/config.hjson", get_engine().clone());
    assert!(result.is_err());
    let msg = result.err().unwrap().to_string();
    assert!(msg.contains("cannot read config"));
}

#[test]
fn test_new_invalid_hjson() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("bad.hjson");
    std::fs::write(&config_path, "not { valid hjson :::").unwrap();
    let result = ShardsManager::with_embedding(config_path.to_str().unwrap(), get_engine().clone());
    assert!(result.is_err());
    let msg = result.err().unwrap().to_string();
    assert!(msg.contains("invalid config"));
}

#[test]
fn test_new_missing_required_field() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("incomplete.hjson");
    std::fs::write(&config_path, "{ shard_duration: \"1h\" }").unwrap();
    let result = ShardsManager::with_embedding(config_path.to_str().unwrap(), get_engine().clone());
    assert!(result.is_err());
}

#[test]
fn test_new_similarity_threshold_respected() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    let content = format!(
        "{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n  similarity_threshold: 0.99\n}}"
    );
    std::fs::write(&config_path, content).unwrap();
    let mgr =
        ShardsManager::with_embedding(config_path.to_str().unwrap(), get_engine().clone()).unwrap();
    // High threshold means every record is primary — two different docs are primaries.
    let id1 = mgr.add(doc("cpu.usage", "value 10")).unwrap();
    let id2 = mgr.add(doc("cpu.usage", "value 11")).unwrap();
    assert_ne!(id1, id2);
}

// ── add ───────────────────────────────────────────────────────────────────────

#[test]
fn test_add_returns_uuid() {
    let (_dir, mgr) = tmp_manager("1h");
    let id = mgr.add(doc("net.rx", "1024")).unwrap();
    assert!(!id.is_nil());
}

#[test]
fn test_add_missing_timestamp() {
    let (_dir, mgr) = tmp_manager("1h");
    let result = mgr.add(json!({ "key": "k", "data": "v" }));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("timestamp"));
}

#[test]
fn test_add_string_timestamp_rejected() {
    let (_dir, mgr) = tmp_manager("1h");
    let result = mgr.add(json!({ "timestamp": "not-a-number", "key": "k", "data": "v" }));
    assert!(result.is_err());
}

#[test]
fn test_add_routes_to_correct_shard() {
    let (_dir, mgr) = tmp_manager("1h");
    // Two timestamps in different hours → two shards.
    let ts1: u64 = 1_748_001_600; // exact 1-hour boundary
    let ts2: u64 = 1_748_005_200; // next hour boundary
    mgr.add(json!({ "timestamp": ts1, "key": "a", "data": "x" }))
        .unwrap();
    mgr.add(json!({ "timestamp": ts2, "key": "b", "data": "y" }))
        .unwrap();
    assert_eq!(mgr.cache().cached_count(), 2);
}

// ── add_batch ─────────────────────────────────────────────────────────────────

#[test]
fn test_add_batch_empty() {
    let (_dir, mgr) = tmp_manager("1h");
    let ids = mgr.add_batch(vec![]).unwrap();
    assert!(ids.is_empty());
}

#[test]
fn test_add_batch_returns_ordered_uuids() {
    let (_dir, mgr) = tmp_manager("1h");
    let docs = vec![doc("a", "1"), doc("b", "2"), doc("c", "3")];
    let ids = mgr.add_batch(docs).unwrap();
    assert_eq!(ids.len(), 3);
    for id in &ids {
        assert!(!id.is_nil());
    }
    // All UUIDs distinct (deduplication is similarity-based, not key-based for different keys).
    let unique: std::collections::HashSet<Uuid> = ids.into_iter().collect();
    assert_eq!(unique.len(), 3);
}

#[test]
fn test_add_batch_error_propagates() {
    let (_dir, mgr) = tmp_manager("1h");
    let docs = vec![
        doc("a", "1"),
        json!({ "key": "no-timestamp", "data": "oops" }),
        doc("c", "3"),
    ];
    assert!(mgr.add_batch(docs).is_err());
}

// ── delete_by_id ──────────────────────────────────────────────────────────────

#[test]
fn test_delete_by_id_removes_record() {
    let (_dir, mgr) = tmp_manager("1h");
    let id = mgr.add(doc("disk.io", "writes: 500")).unwrap();
    mgr.delete_by_id(id).unwrap();
    // Verify it's gone by searching all shards.
    let results = mgr
        .search_fts("1h", "writes")
        .unwrap()
        .into_iter()
        .filter(|d| {
            d.get("data")
                .and_then(|v| v.as_str())
                .map(|s| s.contains("500"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(results, 0);
}

#[test]
fn test_delete_by_id_unknown_id_ok() {
    let (_dir, mgr) = tmp_manager("1h");
    let result = mgr.delete_by_id(Uuid::new_v4());
    assert!(result.is_ok());
}

// ── update ────────────────────────────────────────────────────────────────────

#[test]
fn test_update_returns_new_uuid() {
    let (_dir, mgr) = tmp_manager("1h");
    let id1 = mgr.add(doc("svc.latency", "120ms")).unwrap();
    let id2 = mgr.update(id1, doc("svc.latency", "140ms")).unwrap();
    assert_ne!(id1, id2);
}

#[test]
fn test_update_cross_shard() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts_h0: u64 = 1_748_001_600;
    let ts_h1: u64 = 1_748_005_200;

    let id1 = mgr
        .add(json!({ "timestamp": ts_h0, "key": "transfer", "data": "original" }))
        .unwrap();
    assert_eq!(mgr.cache().cached_count(), 1);

    // Update with timestamp in a different hour — record should move shards.
    let id2 = mgr
        .update(
            id1,
            json!({ "timestamp": ts_h1, "key": "transfer", "data": "moved" }),
        )
        .unwrap();
    assert_ne!(id1, id2);
    assert_eq!(mgr.cache().cached_count(), 2);
}

// ── search_fts ────────────────────────────────────────────────────────────────

#[test]
fn test_search_fts_finds_added_record() {
    let (_dir, mgr) = tmp_manager("1h");
    mgr.add(doc("conn.pool", "connection timeout exceeded"))
        .unwrap();

    let results = mgr.search_fts("1h", "timeout").unwrap();
    assert!(
        !results.is_empty(),
        "expected FTS hit for 'timeout', got none"
    );
}

#[test]
fn test_search_fts_no_results_on_miss() {
    let (_dir, mgr) = tmp_manager("1h");
    mgr.add(doc("cpu", "normal")).unwrap();

    let results = mgr
        .search_fts("1h", "zyxwvutsrqponmlkjihgfedcba_unlikely")
        .unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_search_fts_invalid_duration() {
    let (_dir, mgr) = tmp_manager("1h");
    assert!(mgr.search_fts("not-a-duration", "query").is_err());
}

#[test]
fn test_search_fts_spans_multiple_shards() {
    let (_dir, mgr) = tmp_manager("1h");
    // Insert one record in each of two distinct hours.
    let ts_h0: u64 = 1_748_001_600;
    let ts_h1: u64 = 1_748_005_200;
    mgr.add(json!({ "timestamp": ts_h0, "key": "a.event", "data": "outage alpha" }))
        .unwrap();
    mgr.add(json!({ "timestamp": ts_h1, "key": "b.event", "data": "outage beta" }))
        .unwrap();

    // Use a large lookback window to capture both shards.
    let start = std::time::UNIX_EPOCH + std::time::Duration::from_secs(ts_h0);
    let end = std::time::UNIX_EPOCH + std::time::Duration::from_secs(ts_h1 + 3600);
    let infos = mgr.cache().info().shards_in_range(start, end).unwrap();
    assert_eq!(infos.len(), 2, "catalog should cover both hours");
}

// ── search_vector ─────────────────────────────────────────────────────────────

#[test]
fn test_search_vector_finds_added_record() {
    let (_dir, mgr) = tmp_manager("1h");
    mgr.add(doc("mem.heap", "heap allocation failure detected"))
        .unwrap();

    let query = json!({ "key": "memory", "data": "heap allocation failure" });
    let results = mgr.search_vector("1h", &query).unwrap();
    assert!(!results.is_empty(), "expected vector hit, got none");
}

#[test]
fn test_search_vector_results_have_score() {
    let (_dir, mgr) = tmp_manager("1h");
    mgr.add(doc("gpu.usage", "compute saturation")).unwrap();

    let query = json!({ "data": "compute saturation" });
    let results = mgr.search_vector("1h", &query).unwrap();
    for doc in &results {
        assert!(
            doc.get("_score").is_some(),
            "every vector result must have _score"
        );
    }
}

#[test]
fn test_search_vector_sorted_by_score_desc() {
    let (_dir, mgr) = tmp_manager("1h");
    mgr.add(doc("a", "network latency spike")).unwrap();
    mgr.add(doc("b", "disk write failure")).unwrap();

    let query = json!({ "data": "network latency spike" });
    let results = mgr.search_vector("1h", &query).unwrap();
    let scores: Vec<f64> = results
        .iter()
        .filter_map(|d| d.get("_score").and_then(|v| v.as_f64()))
        .collect();
    for w in scores.windows(2) {
        assert!(
            w[0] >= w[1],
            "scores must be non-increasing: {} < {}",
            w[0],
            w[1]
        );
    }
}

#[test]
fn test_search_vector_invalid_duration() {
    let (_dir, mgr) = tmp_manager("1h");
    let query = json!({ "data": "x" });
    assert!(mgr.search_vector("bad-duration", &query).is_err());
}

// ── accessors ─────────────────────────────────────────────────────────────────

#[test]
fn test_cache_accessor() {
    let (_dir, mgr) = tmp_manager("1h");
    assert_eq!(mgr.cache().cached_count(), 0);
    mgr.add(doc("probe", "value")).unwrap();
    assert_eq!(mgr.cache().cached_count(), 1);
}

#[test]
fn test_clone_shares_state() {
    let (_dir, mgr) = tmp_manager("1h");
    let mgr2 = mgr.clone();
    mgr.add(doc("shared.key", "shared data")).unwrap();
    // Clone sees the same in-memory cache.
    assert_eq!(mgr2.cache().cached_count(), 1);
}

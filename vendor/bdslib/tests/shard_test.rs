use bdslib::embedding::Model;
use bdslib::observability::ObservabilityStorageConfig;
use bdslib::shard::Shard;
use bdslib::EmbeddingEngine;
use serde_json::json;
use std::sync::OnceLock;
use tempfile::TempDir;

// ── shared model ──────────────────────────────────────────────────────────────

static ENGINE: OnceLock<EmbeddingEngine> = OnceLock::new();

fn engine() -> &'static EmbeddingEngine {
    ENGINE.get_or_init(|| EmbeddingEngine::new(Model::AllMiniLML6V2, None).unwrap())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn tmp_shard() -> (TempDir, Shard) {
    let dir = TempDir::new().unwrap();
    let shard = Shard::new(dir.path().to_str().unwrap(), 4, engine().clone()).unwrap();
    (dir, shard)
}

// threshold < -1.0: the second record is always classified as secondary.
// threshold > 1.0:  every record is always classified as primary.
fn tmp_shard_threshold(threshold: f32) -> (TempDir, Shard) {
    let dir = TempDir::new().unwrap();
    let shard = Shard::with_config(
        dir.path().to_str().unwrap(),
        4,
        engine().clone(),
        ObservabilityStorageConfig { similarity_threshold: threshold },
    )
    .unwrap();
    (dir, shard)
}

fn tel(key: &str, data: serde_json::Value, ts: u64) -> serde_json::Value {
    json!({ "timestamp": ts, "key": key, "data": data })
}

fn secondary_ids(doc: &serde_json::Value) -> Vec<String> {
    doc["secondaries"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|s| s["id"].as_str().map(str::to_string))
        .collect()
}

// ── add / get / delete ────────────────────────────────────────────────────────

#[test]
fn test_add_returns_uuid_and_get_retrieves_doc() {
    let (_dir, shard) = tmp_shard();
    let id = shard.add(tel("cpu", json!(72), 1000)).unwrap();
    let doc = shard.get(id).unwrap().expect("should exist");
    assert_eq!(doc["key"], "cpu");
    assert_eq!(doc["data"], 72);
}

#[test]
fn test_get_nonexistent_returns_none() {
    let (_dir, shard) = tmp_shard();
    assert!(shard.get(uuid::Uuid::now_v7()).unwrap().is_none());
}

#[test]
fn test_get_by_key_returns_all_records() {
    let (_dir, shard) = tmp_shard();
    shard.add(tel("k", json!(1), 1000)).unwrap();
    shard.add(tel("k", json!(2), 2000)).unwrap();
    shard.add(tel("other", json!(3), 3000)).unwrap();
    // get_by_key returns all records regardless of primary/secondary status
    let docs = shard.get_by_key("k").unwrap();
    assert_eq!(docs.len(), 2);
}

// ── delete ────────────────────────────────────────────────────────────────────

#[test]
fn test_delete_primary_removes_from_all_indexes() {
    let (_dir, shard) = tmp_shard();
    // First record in an empty store is always primary.
    let id = shard.add(tel("k", json!("hello world"), 1000)).unwrap();
    assert!(shard.observability().is_primary(id).unwrap());

    shard.delete(id).unwrap();

    assert!(shard.get(id).unwrap().is_none());
    assert!(shard.search_fts("hello", 10).unwrap().is_empty());
    assert!(shard.search_vector(&json!({ "data": "hello world" }), 10).unwrap().is_empty());
}

#[test]
fn test_delete_secondary_leaves_primary_in_indexes() {
    // threshold -1.1 → second record is always secondary.
    let (_dir, shard) = tmp_shard_threshold(-1.1);
    let primary_id   = shard.add(tel("k", json!("primary alpha"), 1000)).unwrap();
    let secondary_id = shard.add(tel("k", json!("secondary beta"), 2000)).unwrap();

    assert!( shard.observability().is_primary(primary_id).unwrap());
    assert!(!shard.observability().is_primary(secondary_id).unwrap());

    shard.delete(secondary_id).unwrap();

    // Secondary is gone from observability
    assert!(shard.get(secondary_id).unwrap().is_none());

    // Primary is still in all indexes
    assert!(shard.get(primary_id).unwrap().is_some());
    assert_eq!(shard.search_fts("alpha", 10).unwrap().len(), 1);
    assert!(!shard.search_vector(&json!({ "data": "primary alpha" }), 5).unwrap().is_empty());
}

#[test]
fn test_delete_nonexistent_is_ok() {
    let (_dir, shard) = tmp_shard();
    assert!(shard.delete(uuid::Uuid::now_v7()).is_ok());
}

// ── secondary not indexed in FTS / vector ─────────────────────────────────────

#[test]
fn test_secondary_not_in_fts() {
    let (_dir, shard) = tmp_shard_threshold(-1.1);
    shard.add(tel("k", json!("first record indexed"),    1000)).unwrap();
    shard.add(tel("k", json!("second record secondary"), 2000)).unwrap();

    // "first" is in FTS index → found
    assert_eq!(shard.search_fts("first", 10).unwrap().len(), 1);
    // "second" is not in FTS index (secondary) → not found
    assert!(shard.search_fts("second", 10).unwrap().is_empty());
}

#[test]
fn test_secondary_not_in_vector() {
    let (_dir, shard) = tmp_shard_threshold(-1.1);
    shard.add(tel("k", json!("spacecraft telemetry primary"),   1000)).unwrap();
    shard.add(tel("k", json!("spacecraft telemetry secondary"), 2000)).unwrap();

    let results = shard
        .search_vector(&json!({ "data": "spacecraft telemetry" }), 10)
        .unwrap();

    // Only one entry in the vector index (the primary); secondary is never indexed.
    assert_eq!(results.len(), 1);
}

// ── search results embed secondaries ─────────────────────────────────────────

#[test]
fn test_search_fts_result_has_secondaries_field() {
    let (_dir, shard) = tmp_shard_threshold(-1.1);
    let primary_id   = shard.add(tel("k", json!("primary uniqueword"), 1000)).unwrap();
    let secondary_id = shard.add(tel("k", json!("secondary record"),   2000)).unwrap();

    let results = shard.search_fts("uniqueword", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"].as_str().unwrap(),
               primary_id.to_string());

    let sec_ids = secondary_ids(&results[0]);
    assert_eq!(sec_ids.len(), 1);
    assert_eq!(sec_ids[0], secondary_id.to_string());
}

#[test]
fn test_search_vector_result_has_secondaries_field() {
    let (_dir, shard) = tmp_shard_threshold(-1.1);
    let primary_id   = shard.add(tel("k", json!("deep ocean exploration"), 1000)).unwrap();
    let secondary_id = shard.add(tel("k", json!("ocean depth research"),   2000)).unwrap();

    let results = shard
        .search_vector(&json!({ "data": "ocean exploration" }), 5)
        .unwrap();
    assert!(!results.is_empty());

    let top = &results[0];
    assert_eq!(top["id"].as_str().unwrap(), primary_id.to_string());
    assert!(top.get("_score").is_some());
    assert!(top.get("secondaries").is_some());

    let sec_ids = secondary_ids(top);
    assert_eq!(sec_ids.len(), 1);
    assert_eq!(sec_ids[0], secondary_id.to_string());
}

#[test]
fn test_search_fts_no_secondaries_when_primary_has_none() {
    let (_dir, shard) = tmp_shard();
    // With default threshold two very different records are both primaries.
    shard.add(tel("k", json!("network timeout error"),   1000)).unwrap();
    shard.add(tel("k", json!("database disk full warning"), 2000)).unwrap();

    let results = shard.search_fts("timeout", 10).unwrap();
    assert_eq!(results.len(), 1);
    let secondaries = results[0]["secondaries"].as_array().unwrap();
    assert!(secondaries.is_empty());
}

#[test]
fn test_search_vector_no_secondaries_when_primary_has_none() {
    let (_dir, shard) = tmp_shard();
    shard.add(tel("k", json!("network timeout error"), 1000)).unwrap();

    let results = shard.search_vector(&json!({ "data": "network timeout" }), 5).unwrap();
    assert!(!results.is_empty());
    let secondaries = results[0]["secondaries"].as_array().unwrap();
    assert!(secondaries.is_empty());
}

#[test]
fn test_search_fts_multiple_secondaries_per_primary() {
    let (_dir, shard) = tmp_shard_threshold(-1.1);
    let pid  = shard.add(tel("k", json!("primary indexword"),    1000)).unwrap();
    let sid1 = shard.add(tel("k", json!("secondary one"),        2000)).unwrap();
    let sid2 = shard.add(tel("k", json!("secondary two"),        3000)).unwrap();
    let sid3 = shard.add(tel("k", json!("secondary three"),      4000)).unwrap();

    let results = shard.search_fts("indexword", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"].as_str().unwrap(), pid.to_string());

    let mut sec_ids = secondary_ids(&results[0]);
    sec_ids.sort();
    let mut expected = vec![sid1.to_string(), sid2.to_string(), sid3.to_string()];
    expected.sort();
    assert_eq!(sec_ids, expected);
}

// ── search_fts ────────────────────────────────────────────────────────────────

#[test]
fn test_search_fts_finds_keyword_in_data() {
    let (_dir, shard) = tmp_shard();
    shard.add(tel("log", json!("connection timeout error"),  1000)).unwrap();
    shard.add(tel("log", json!("disk space critically low"), 2000)).unwrap();

    let results = shard.search_fts("timeout", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["data"], "connection timeout error");
}

#[test]
fn test_search_fts_no_match_returns_empty() {
    let (_dir, shard) = tmp_shard();
    shard.add(tel("k", json!("hello world"), 1000)).unwrap();
    assert!(shard.search_fts("zzznomatch", 10).unwrap().is_empty());
}

#[test]
fn test_search_fts_respects_limit() {
    let (_dir, shard) = tmp_shard_threshold(1.1); // all primaries → all indexed
    for i in 0..5u64 {
        shard.add(tel("k", json!(format!("error event {i}")), 1000 + i)).unwrap();
    }
    let results = shard.search_fts("error", 3).unwrap();
    assert!(results.len() <= 3);
}

#[test]
fn test_search_fts_returns_full_documents_with_metadata() {
    let (_dir, shard) = tmp_shard();
    shard.add(json!({
        "timestamp": 1000,
        "key": "alert",
        "data": "memory pressure detected",
        "host": "node-01",
    })).unwrap();

    let results = shard.search_fts("memory", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["host"], "node-01");
    // secondaries field is always present
    assert!(results[0].get("secondaries").is_some());
}

// ── search_vector ─────────────────────────────────────────────────────────────

#[test]
fn test_search_vector_returns_score_and_secondaries() {
    let (_dir, shard) = tmp_shard();
    shard.add(tel("k", json!("the quick brown fox"), 1000)).unwrap();

    let results = shard.search_vector(&json!({ "data": "quick fox running" }), 5).unwrap();
    assert!(!results.is_empty());
    for doc in &results {
        let score = doc["_score"].as_f64().expect("_score must be present");
        assert!((0.0..=1.0).contains(&score));
        assert!(doc.get("secondaries").is_some());
    }
}

#[test]
fn test_search_vector_top_result_is_most_similar() {
    let (_dir, shard) = tmp_shard();
    let id_related = shard
        .add(tel("k", json!("connection refused network error"), 1000))
        .unwrap();
    shard.add(tel("k", json!("apple pie recipe flour sugar butter"), 2000)).unwrap();

    let results = shard
        .search_vector(&json!({ "data": "network connection refused" }), 5)
        .unwrap();

    assert!(!results.is_empty());
    let top_id: uuid::Uuid = results[0]["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(top_id, id_related);
}

#[test]
fn test_search_vector_scores_descending() {
    let (_dir, shard) = tmp_shard_threshold(1.1); // all primaries → all indexed
    shard.add(tel("k", json!("server CPU overload"),        1000)).unwrap();
    shard.add(tel("k", json!("high load average on host"),  2000)).unwrap();
    shard.add(tel("k", json!("baking recipes collection"),  3000)).unwrap();

    let results = shard.search_vector(&json!({ "data": "CPU usage high" }), 5).unwrap();
    let scores: Vec<f64> = results.iter().map(|d| d["_score"].as_f64().unwrap()).collect();
    for w in scores.windows(2) {
        assert!(w[0] >= w[1], "scores out of order: {:?}", scores);
    }
}

#[test]
fn test_search_vector_respects_limit() {
    let (_dir, shard) = tmp_shard_threshold(1.1); // all primaries
    for i in 0..8u64 {
        shard.add(tel("k", json!(format!("server error event {i}")), 1000 + i)).unwrap();
    }
    let results = shard.search_vector(&json!({ "data": "server error" }), 3).unwrap();
    assert!(results.len() <= 3);
}

// ── dedup pass-through ────────────────────────────────────────────────────────

#[test]
fn test_add_duplicate_returns_same_uuid() {
    let (_dir, shard) = tmp_shard();
    let id1 = shard.add(tel("k", json!("same data"), 1000)).unwrap();
    let id2 = shard.add(tel("k", json!("same data"), 2000)).unwrap();
    assert_eq!(id1, id2);
    assert_eq!(shard.get_by_key("k").unwrap().len(), 1);
}

// ── clone shares state ────────────────────────────────────────────────────────

#[test]
fn test_clone_shares_all_indexes() {
    let (_dir, shard) = tmp_shard();
    let clone = shard.clone();

    let id = shard.add(tel("k", json!("clone test"), 1000)).unwrap();

    assert!(clone.get(id).unwrap().is_some());
    assert_eq!(clone.search_fts("clone", 10).unwrap().len(), 1);
    assert!(!clone.search_vector(&json!({ "data": "clone test" }), 5).unwrap().is_empty());
}

// ── custom config ─────────────────────────────────────────────────────────────

#[test]
fn test_with_config_all_primaries_all_indexed() {
    let (_dir, shard) = tmp_shard_threshold(1.1);
    shard.add(tel("k", json!("alpha"), 1000)).unwrap();
    shard.add(tel("k", json!("beta"),  2000)).unwrap();
    shard.add(tel("k", json!("gamma"), 3000)).unwrap();

    // All three are primaries → all indexed in FTS and vector
    assert_eq!(shard.observability().list_primaries().unwrap().len(), 3);
    // Each keyword unique to its record → 3 distinct FTS hits possible
    assert_eq!(shard.search_fts("alpha", 10).unwrap().len(), 1);
    assert_eq!(shard.search_fts("beta",  10).unwrap().len(), 1);
    assert_eq!(shard.search_fts("gamma", 10).unwrap().len(), 1);
}

#[test]
fn test_with_config_secondaries_not_indexed() {
    let (_dir, shard) = tmp_shard_threshold(-1.1);
    shard.add(tel("k", json!("primary indexme"),    1000)).unwrap();
    shard.add(tel("k", json!("secondary notindexed"), 2000)).unwrap();
    shard.add(tel("k", json!("secondary alsonotindexed"), 3000)).unwrap();

    // Two secondaries, only one primary
    assert_eq!(shard.observability().list_primaries().unwrap().len(), 1);

    // Only the primary appears in FTS
    assert_eq!(shard.search_fts("indexme", 10).unwrap().len(), 1);
    assert!(shard.search_fts("notindexed", 10).unwrap().is_empty());
    assert!(shard.search_fts("alsonotindexed", 10).unwrap().is_empty());

    // Only one entry in the vector index
    let vec_results = shard.search_vector(&json!({ "data": "primary" }), 10).unwrap();
    assert_eq!(vec_results.len(), 1);
    // That one result carries both secondaries
    assert_eq!(vec_results[0]["secondaries"].as_array().unwrap().len(), 2);
}

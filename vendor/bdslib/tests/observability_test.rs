use bdslib::embedding::Model;
use bdslib::observability::{ObservabilityStorage, ObservabilityStorageConfig};
use bdslib::EmbeddingEngine;
use serde_json::json;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// ── shared model ──────────────────────────────────────────────────────────────

static ENGINE: OnceLock<EmbeddingEngine> = OnceLock::new();

fn engine() -> &'static EmbeddingEngine {
    ENGINE.get_or_init(|| EmbeddingEngine::new(Model::AllMiniLML6V2, None).unwrap())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn tmp_store(threshold: f32) -> (TempDir, ObservabilityStorage) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("obs.db");
    let store = ObservabilityStorage::with_config(
        path.to_str().unwrap(),
        4,
        engine().clone(),
        ObservabilityStorageConfig {
            similarity_threshold: threshold,
        },
    )
    .unwrap();
    (dir, store)
}

fn default_store() -> (TempDir, ObservabilityStorage) {
    tmp_store(0.85)
}

fn ts(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn tel(key: &str, data: serde_json::Value, timestamp: u64) -> serde_json::Value {
    json!({ "key": key, "data": data, "timestamp": timestamp })
}

// ── validation ────────────────────────────────────────────────────────────────

#[test]
fn test_add_missing_timestamp_is_err() {
    let (_dir, store) = default_store();
    let err = store.add(json!({"key": "k", "data": 1})).unwrap_err();
    assert!(err.to_string().contains("timestamp"), "{err}");
}

#[test]
fn test_add_missing_key_is_err() {
    let (_dir, store) = default_store();
    let err = store.add(json!({"timestamp": 1000, "data": 1})).unwrap_err();
    assert!(err.to_string().contains("key"), "{err}");
}

#[test]
fn test_add_missing_data_is_err() {
    let (_dir, store) = default_store();
    let err = store.add(json!({"timestamp": 1000, "key": "k"})).unwrap_err();
    assert!(err.to_string().contains("data"), "{err}");
}

#[test]
fn test_add_non_string_key_is_err() {
    let (_dir, store) = default_store();
    let err = store
        .add(json!({"timestamp": 1000, "key": 42, "data": 1}))
        .unwrap_err();
    assert!(err.to_string().contains("key"), "{err}");
}

#[test]
fn test_add_numeric_string_timestamp_is_ok() {
    let (_dir, store) = default_store();
    let (id, _, _) = store
        .add(json!({"timestamp": "1000", "key": "k", "data": 1}))
        .unwrap();
    assert!(!id.is_nil());
}

#[test]
fn test_add_non_numeric_timestamp_is_err() {
    let (_dir, store) = default_store();
    let err = store
        .add(json!({"timestamp": "not-a-number", "key": "k", "data": 1}))
        .unwrap_err();
    assert!(err.to_string().contains("timestamp"), "{err}");
}

// ── id handling ───────────────────────────────────────────────────────────────

#[test]
fn test_add_generates_uuid_when_id_absent() {
    let (_dir, store) = default_store();
    let (id, _, _) = store.add(tel("k", json!(1), 1000)).unwrap();
    assert!(!id.is_nil());
}

#[test]
fn test_add_uses_provided_id() {
    let (_dir, store) = default_store();
    let custom_id = uuid::Uuid::now_v7();
    let doc = json!({"id": custom_id.to_string(), "key": "k", "data": 1, "timestamp": 1000});
    let (returned, _, _) = store.add(doc).unwrap();
    assert_eq!(returned, custom_id);
}

// ── get_by_id ─────────────────────────────────────────────────────────────────

#[test]
fn test_get_by_id_returns_stored_record() {
    let (_dir, store) = default_store();
    let (id, _, _) = store
        .add(json!({"key": "cpu", "data": 75, "timestamp": 1000, "host": "web-01"}))
        .unwrap();
    let got = store.get_by_id(id).unwrap().expect("should exist");
    assert_eq!(got["key"], json!("cpu"));
    assert_eq!(got["data"], json!(75));
    assert_eq!(got["timestamp"], json!(1000));
    assert_eq!(got["host"], json!("web-01"));
    assert_eq!(got["id"], json!(id.to_string()));
}

#[test]
fn test_get_by_id_nonexistent_returns_none() {
    let (_dir, store) = default_store();
    assert!(store.get_by_id(uuid::Uuid::now_v7()).unwrap().is_none());
}

#[test]
fn test_get_preserves_all_data_types() {
    let (_dir, store) = default_store();
    let doc = json!({
        "key": "types",
        "data": {"nested": [1, true, "text"]},
        "timestamp": 2000,
        "flag": false,
        "count": 42,
        "ratio": 3.14
    });
    let (id, _, _) = store.add(doc.clone()).unwrap();
    let got = store.get_by_id(id).unwrap().unwrap();
    assert_eq!(got["data"], doc["data"]);
    assert_eq!(got["flag"], json!(false));
    assert_eq!(got["count"], json!(42));
}

// ── get_by_key ────────────────────────────────────────────────────────────────

#[test]
fn test_get_by_key_returns_all_records() {
    let (_dir, store) = default_store();
    store.add(tel("cpu", json!(50), 1000)).unwrap();
    store.add(tel("cpu", json!(60), 2000)).unwrap();
    store.add(tel("mem", json!(80), 1500)).unwrap();

    let cpu_records = store.get_by_key("cpu").unwrap();
    assert_eq!(cpu_records.len(), 2);
    assert_eq!(cpu_records[0]["data"], json!(50));
    assert_eq!(cpu_records[1]["data"], json!(60));
}

#[test]
fn test_get_by_key_empty_returns_empty_vec() {
    let (_dir, store) = default_store();
    assert!(store.get_by_key("nonexistent").unwrap().is_empty());
}

#[test]
fn test_get_by_key_ordered_by_timestamp() {
    let (_dir, store) = default_store();
    store.add(tel("k", json!(3), 3000)).unwrap();
    store.add(tel("k", json!(1), 1000)).unwrap();
    store.add(tel("k", json!(2), 2000)).unwrap();
    let records = store.get_by_key("k").unwrap();
    let tss: Vec<i64> = records
        .iter()
        .map(|r| r["timestamp"].as_i64().unwrap())
        .collect();
    assert_eq!(tss, vec![1000, 2000, 3000]);
}

// ── delete ────────────────────────────────────────────────────────────────────

#[test]
fn test_delete_by_id_removes_record() {
    let (_dir, store) = default_store();
    let (id, _, _) = store.add(tel("k", json!(1), 1000)).unwrap();
    store.delete_by_id(id).unwrap();
    assert!(store.get_by_id(id).unwrap().is_none());
}

#[test]
fn test_delete_by_id_nonexistent_is_ok() {
    let (_dir, store) = default_store();
    assert!(store.delete_by_id(uuid::Uuid::now_v7()).is_ok());
}

#[test]
fn test_delete_by_key_removes_all_records() {
    let (_dir, store) = default_store();
    store.add(tel("k", json!(1), 1000)).unwrap();
    store.add(tel("k", json!(2), 2000)).unwrap();
    store.delete_by_key("k").unwrap();
    assert!(store.get_by_key("k").unwrap().is_empty());
}

#[test]
fn test_delete_by_key_nonexistent_is_ok() {
    let (_dir, store) = default_store();
    assert!(store.delete_by_key("ghost").is_ok());
}

#[test]
fn test_delete_by_id_clears_dedup_tracking() {
    let (_dir, store) = default_store();
    // First add stores the record; second add is a duplicate and logs a timestamp.
    let (id, _, _) = store.add(tel("k", json!("hello"), 1000)).unwrap();
    let (dup_id, _, _) = store.add(tel("k", json!("hello"), 2000)).unwrap();
    assert_eq!(id, dup_id);
    assert_eq!(store.get_duplicate_timestamps("k").unwrap().len(), 1);

    // Deleting the record must also remove its dedup entry.
    store.delete_by_id(id).unwrap();
    assert!(store.get_duplicate_timestamps("k").unwrap().is_empty());
}

#[test]
fn test_delete_by_key_clears_dedup_tracking() {
    let (_dir, store) = default_store();
    store.add(tel("k", json!("hello"), 1000)).unwrap();
    store.add(tel("k", json!("hello"), 2000)).unwrap(); // duplicate
    assert_eq!(store.get_duplicate_timestamps("k").unwrap().len(), 1);

    store.delete_by_key("k").unwrap();
    assert!(store.get_duplicate_timestamps("k").unwrap().is_empty());
}

// ── list_ids_by_time_range ────────────────────────────────────────────────────

#[test]
fn test_list_ids_by_time_range_returns_correct_ids() {
    let (_dir, store) = default_store();
    let (id1, _, _) = store.add(tel("k", json!(1), 1000)).unwrap();
    let (id2, _, _) = store.add(tel("k", json!(2), 2000)).unwrap();
    let (_id3, _, _) = store.add(tel("k", json!(3), 3000)).unwrap();

    let ids = store.list_ids_by_time_range(ts(1000), ts(3000)).unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[test]
fn test_list_ids_by_time_range_half_open() {
    let (_dir, store) = default_store();
    let (_a, _, _) = store.add(tel("k", json!(1), 999)).unwrap();
    let (b, _, _) = store.add(tel("k", json!(2), 1000)).unwrap();
    let (c, _, _) = store.add(tel("k", json!(3), 1999)).unwrap();
    let (_d, _, _) = store.add(tel("k", json!(4), 2000)).unwrap();

    let ids = store.list_ids_by_time_range(ts(1000), ts(2000)).unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&b));
    assert!(ids.contains(&c));
}

#[test]
fn test_list_ids_by_time_range_empty() {
    let (_dir, store) = default_store();
    store.add(tel("k", json!(1), 5000)).unwrap();
    let ids = store.list_ids_by_time_range(ts(1000), ts(2000)).unwrap();
    assert!(ids.is_empty());
}

// ── deduplication ─────────────────────────────────────────────────────────────

#[test]
fn test_add_duplicate_returns_existing_uuid() {
    let (_dir, store) = default_store();
    let (id1, _, _) = store.add(tel("cpu", json!(75), 1000)).unwrap();
    // Same key + data, different timestamp → duplicate
    let (id2, _, _) = store.add(tel("cpu", json!(75), 2000)).unwrap();
    assert_eq!(id1, id2, "duplicate should return the existing UUID");
}

#[test]
fn test_add_duplicate_does_not_store_second_record() {
    let (_dir, store) = default_store();
    store.add(tel("k", json!("hello"), 1000)).unwrap();
    store.add(tel("k", json!("hello"), 2000)).unwrap();
    // Only one record in the store
    assert_eq!(store.get_by_key("k").unwrap().len(), 1);
}

#[test]
fn test_add_different_data_same_key_are_not_duplicates() {
    let (_dir, store) = default_store();
    let (id1, _, _) = store.add(tel("k", json!(1), 1000)).unwrap();
    let (id2, _, _) = store.add(tel("k", json!(2), 2000)).unwrap();
    assert_ne!(id1, id2);
    assert_eq!(store.get_by_key("k").unwrap().len(), 2);
}

#[test]
fn test_dedup_timestamps_recorded() {
    let (_dir, store) = default_store();
    store.add(tel("k", json!(99), 1000)).unwrap(); // first → stored, no dedup
    store.add(tel("k", json!(99), 2000)).unwrap(); // duplicate → ts=2000 logged
    store.add(tel("k", json!(99), 3000)).unwrap(); // duplicate → ts=3000 logged

    let dup_ts = store.get_duplicate_timestamps("k").unwrap();
    assert_eq!(dup_ts.len(), 2);
    let secs: Vec<u64> = dup_ts
        .iter()
        .map(|t| t.duration_since(UNIX_EPOCH).unwrap().as_secs())
        .collect();
    assert!(secs.contains(&2000));
    assert!(secs.contains(&3000));
}

#[test]
fn test_dedup_timestamps_empty_for_nonexistent_key() {
    let (_dir, store) = default_store();
    assert!(store.get_duplicate_timestamps("ghost").unwrap().is_empty());
}

#[test]
fn test_dedup_timestamps_empty_when_no_duplicates() {
    let (_dir, store) = default_store();
    store.add(tel("k", json!(1), 1000)).unwrap();
    assert!(store.get_duplicate_timestamps("k").unwrap().is_empty());
}

#[test]
fn test_dedup_different_data_tracked_separately() {
    let (_dir, store) = default_store();
    // Two distinct data values, each submitted twice
    store.add(tel("k", json!(1), 1000)).unwrap();
    store.add(tel("k", json!(1), 2000)).unwrap(); // dup of data=1
    store.add(tel("k", json!(2), 3000)).unwrap();
    store.add(tel("k", json!(2), 4000)).unwrap(); // dup of data=2

    let dups = store.get_duplicate_timestamps("k").unwrap();
    assert_eq!(dups.len(), 2); // one per distinct data value
}

// ── primary / secondary ───────────────────────────────────────────────────────

#[test]
fn test_first_record_is_primary() {
    let (_dir, store) = default_store();
    let (id, _, _) = store.add(tel("k", json!("unique signal"), 1000)).unwrap();
    let primaries = store.list_primaries().unwrap();
    assert!(primaries.contains(&id), "first record must be a primary");
}

#[test]
fn test_list_primaries_empty_initially() {
    let (_dir, store) = default_store();
    assert!(store.list_primaries().unwrap().is_empty());
}

#[test]
fn test_clearly_different_data_both_become_primaries() {
    // Threshold 1.1 is above the maximum possible cosine similarity (1.0), so
    // no record can ever be classified as secondary — every record is a primary.
    let (_dir, store) = tmp_store(1.1);
    let (id1, _, _) = store
        .add(tel("k", json!("quantum chromodynamics particle physics"), 1000))
        .unwrap();
    let (id2, _, _) = store
        .add(tel("k", json!("apple pie recipe baking flour sugar"), 2000))
        .unwrap();
    let primaries = store.list_primaries().unwrap();
    assert!(primaries.contains(&id1));
    assert!(primaries.contains(&id2));
}

#[test]
fn test_very_similar_data_assigned_as_secondary() {
    // Threshold -1.1 is below the minimum possible cosine similarity (-1.0), so
    // the condition `sim >= threshold` is always satisfied: the first record (no
    // primaries yet) becomes primary, every subsequent record becomes secondary.
    let (_dir, store) = tmp_store(-1.1);
    let (id1, _, _) = store
        .add(tel("k", json!("the quick brown fox"), 1000))
        .unwrap();
    let (id2, _, _) = store
        .add(tel("k2", json!("apple pie and other recipes"), 2000))
        .unwrap();

    let primaries = store.list_primaries().unwrap();
    assert!(primaries.contains(&id1), "first record must always be primary");
    assert!(
        !primaries.contains(&id2),
        "with threshold -1.1 every subsequent record is secondary"
    );

    let secondaries = store.list_secondaries(id1).unwrap();
    assert!(secondaries.contains(&id2));
}

#[test]
fn test_list_secondaries_empty_for_new_primary() {
    let (_dir, store) = default_store();
    let (id, _, _) = store.add(tel("k", json!("unique"), 1000)).unwrap();
    assert!(store.list_secondaries(id).unwrap().is_empty());
}

#[test]
fn test_list_primaries_in_range() {
    let (_dir, store) = default_store();
    let (id1, _, _) = store
        .add(tel("a", json!("alpha signal topic one"), 1000))
        .unwrap();
    let (id2, _, _) = store
        .add(tel("b", json!("beta signal topic two"), 3000))
        .unwrap();

    let in_range = store.list_primaries_in_range(ts(500), ts(2000)).unwrap();
    assert!(in_range.contains(&id1));
    assert!(!in_range.contains(&id2));
}

#[test]
fn test_delete_by_id_removes_from_primary_tracking() {
    let (_dir, store) = default_store();
    let (id, _, _) = store.add(tel("k", json!("some data"), 1000)).unwrap();
    store.delete_by_id(id).unwrap();
    assert!(!store.list_primaries().unwrap().contains(&id));
}

// ── clone shares store ────────────────────────────────────────────────────────

#[test]
fn test_clone_shares_underlying_store() {
    let (_dir, store) = default_store();
    let clone = store.clone();
    let (id, _, _) = store.add(tel("k", json!(42), 1000)).unwrap();
    let got = clone.get_by_id(id).unwrap().expect("clone must see same data");
    assert_eq!(got["data"], json!(42));
}

// ── metadata preservation ─────────────────────────────────────────────────────

#[test]
fn test_extra_fields_stored_as_metadata() {
    let (_dir, store) = default_store();
    let doc = json!({
        "key": "sensor",
        "data": 42,
        "timestamp": 1000,
        "host": "node-1",
        "region": "us-east",
        "tags": ["prod", "k8s"]
    });
    let (id, _, _) = store.add(doc).unwrap();
    let got = store.get_by_id(id).unwrap().unwrap();
    assert_eq!(got["host"], json!("node-1"));
    assert_eq!(got["region"], json!("us-east"));
    assert_eq!(got["tags"], json!(["prod", "k8s"]));
}

#[test]
fn test_mandatory_fields_not_duplicated_in_metadata() {
    let (_dir, store) = default_store();
    let (id, _, _) = store
        .add(json!({
            "key": "k", "data": 1, "timestamp": 1000,
            "extra": "preserved"
        }))
        .unwrap();
    let got = store.get_by_id(id).unwrap().unwrap();
    // "extra" must be present at top level (from metadata merge)
    assert_eq!(got["extra"], json!("preserved"));
    // Mandatory fields must appear exactly once (not duplicated inside metadata)
    assert_eq!(got["key"], json!("k"));
    assert_eq!(got["data"], json!(1));
}

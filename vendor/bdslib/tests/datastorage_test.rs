use bdslib::datastorage::{BlobStorage, JsonStorage, JsonStorageConfig};
use serde_json::json;
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

fn tmp_blobs() -> (TempDir, BlobStorage) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("blobs.db");
    let store = BlobStorage::new(path.to_str().unwrap(), 4).unwrap();
    (dir, store)
}

fn tmp_json(cfg: JsonStorageConfig) -> (TempDir, JsonStorage) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("json.db");
    let store = JsonStorage::new(path.to_str().unwrap(), 4, cfg).unwrap();
    (dir, store)
}

fn default_json_store() -> (TempDir, JsonStorage) {
    tmp_json(JsonStorageConfig::default())
}

fn keyed_json_store(field: &str) -> (TempDir, JsonStorage) {
    tmp_json(JsonStorageConfig {
        key_field: Some(field.to_string()),
        default_key: "fallback".to_string(),
    })
}

// ── BlobStorage ───────────────────────────────────────────────────────────────

#[test]
fn test_blob_add_returns_uuid() {
    let (_dir, store) = tmp_blobs();
    let id = store.add_blob(b"hello").unwrap();
    assert!(!id.is_nil());
}

#[test]
fn test_blob_add_and_get_roundtrip() {
    let (_dir, store) = tmp_blobs();
    let data = b"the quick brown fox";
    let id = store.add_blob(data).unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert_eq!(got, data);
}

#[test]
fn test_blob_get_nonexistent_returns_none() {
    let (_dir, store) = tmp_blobs();
    let fake = uuid::Uuid::now_v7();
    assert!(store.get_blob(fake).unwrap().is_none());
}

#[test]
fn test_blob_update_changes_data() {
    let (_dir, store) = tmp_blobs();
    let id = store.add_blob(b"original").unwrap();
    store.update_blob(id, b"updated").unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert_eq!(got, b"updated");
}

#[test]
fn test_blob_update_nonexistent_is_ok() {
    let (_dir, store) = tmp_blobs();
    let fake = uuid::Uuid::now_v7();
    assert!(store.update_blob(fake, b"data").is_ok());
}

#[test]
fn test_blob_drop_removes_record() {
    let (_dir, store) = tmp_blobs();
    let id = store.add_blob(b"bye").unwrap();
    store.drop_blob(id).unwrap();
    assert!(store.get_blob(id).unwrap().is_none());
}

#[test]
fn test_blob_drop_nonexistent_is_ok() {
    let (_dir, store) = tmp_blobs();
    let fake = uuid::Uuid::now_v7();
    assert!(store.drop_blob(fake).is_ok());
}

#[test]
fn test_blob_empty_payload() {
    let (_dir, store) = tmp_blobs();
    let id = store.add_blob(b"").unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert!(got.is_empty());
}

#[test]
fn test_blob_binary_data_with_null_bytes() {
    let (_dir, store) = tmp_blobs();
    let data: Vec<u8> = (0u8..=255).collect();
    let id = store.add_blob(&data).unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert_eq!(got, data);
}

#[test]
fn test_blob_add_produces_unique_uuids() {
    let (_dir, store) = tmp_blobs();
    let id1 = store.add_blob(b"a").unwrap();
    let id2 = store.add_blob(b"b").unwrap();
    assert_ne!(id1, id2);
}

#[test]
fn test_blob_uuids_are_time_ordered() {
    let (_dir, store) = tmp_blobs();
    let id1 = store.add_blob(b"first").unwrap();
    let id2 = store.add_blob(b"second").unwrap();
    assert!(id1 < id2, "UUIDv7 values should be monotonically increasing");
}

#[test]
fn test_blob_clone_shares_store() {
    let (_dir, store) = tmp_blobs();
    let clone = store.clone();
    let id = store.add_blob(b"shared").unwrap();
    let got = clone.get_blob(id).unwrap().expect("clone should see the same data");
    assert_eq!(got, b"shared");
}

#[test]
fn test_blob_update_then_drop() {
    let (_dir, store) = tmp_blobs();
    let id = store.add_blob(b"v1").unwrap();
    store.update_blob(id, b"v2").unwrap();
    store.drop_blob(id).unwrap();
    assert!(store.get_blob(id).unwrap().is_none());
}

// ── BlobStorage::add_blob_with_key ────────────────────────────────────────────

#[test]
fn test_blob_with_key_roundtrip() {
    let (_dir, store) = tmp_blobs();
    let id = uuid::Uuid::now_v7();
    store.add_blob_with_key(id, b"fixed key payload").unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert_eq!(got, b"fixed key payload");
}

#[test]
fn test_blob_with_key_preserves_supplied_uuid() {
    let (_dir, store) = tmp_blobs();
    let id = uuid::Uuid::now_v7();
    store.add_blob_with_key(id, b"data").unwrap();
    // get_blob must find the record under exactly the supplied UUID
    assert!(store.get_blob(id).unwrap().is_some());
}

#[test]
fn test_blob_with_key_duplicate_key_returns_err() {
    let (_dir, store) = tmp_blobs();
    let id = uuid::Uuid::now_v7();
    store.add_blob_with_key(id, b"first").unwrap();
    // Second insert with the same key must fail (PRIMARY KEY violation)
    assert!(store.add_blob_with_key(id, b"second").is_err());
    // The original data must be untouched
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert_eq!(got, b"first");
}

#[test]
fn test_blob_with_key_update_changes_data() {
    let (_dir, store) = tmp_blobs();
    let id = uuid::Uuid::now_v7();
    store.add_blob_with_key(id, b"original").unwrap();
    store.update_blob(id, b"replaced").unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert_eq!(got, b"replaced");
}

#[test]
fn test_blob_with_key_drop_removes_record() {
    let (_dir, store) = tmp_blobs();
    let id = uuid::Uuid::now_v7();
    store.add_blob_with_key(id, b"temporary").unwrap();
    store.drop_blob(id).unwrap();
    assert!(store.get_blob(id).unwrap().is_none());
}

#[test]
fn test_blob_with_key_drop_and_reinsert() {
    // After drop the same key can be reused — timestamps reset to now.
    let (_dir, store) = tmp_blobs();
    let id = uuid::Uuid::now_v7();
    store.add_blob_with_key(id, b"v1").unwrap();
    store.drop_blob(id).unwrap();
    store.add_blob_with_key(id, b"v2").unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist after reinsert");
    assert_eq!(got, b"v2");
}

#[test]
fn test_blob_with_key_empty_payload() {
    let (_dir, store) = tmp_blobs();
    let id = uuid::Uuid::now_v7();
    store.add_blob_with_key(id, b"").unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert!(got.is_empty());
}

#[test]
fn test_blob_with_key_binary_data_full_byte_range() {
    let (_dir, store) = tmp_blobs();
    let data: Vec<u8> = (0u8..=255).collect();
    let id = uuid::Uuid::now_v7();
    store.add_blob_with_key(id, &data).unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert_eq!(got, data);
}

#[test]
fn test_blob_with_key_coexists_with_add_blob() {
    // Records inserted by add_blob and add_blob_with_key must not interfere.
    let (_dir, store) = tmp_blobs();
    let fixed_id = uuid::Uuid::now_v7();
    store.add_blob_with_key(fixed_id, b"fixed").unwrap();
    let auto_id = store.add_blob(b"auto").unwrap();
    assert_ne!(fixed_id, auto_id);
    assert_eq!(store.get_blob(fixed_id).unwrap().unwrap(), b"fixed");
    assert_eq!(store.get_blob(auto_id).unwrap().unwrap(), b"auto");
}

#[test]
fn test_blob_with_key_nil_uuid_is_valid() {
    // Nil UUID is syntactically valid; the store must accept it.
    let (_dir, store) = tmp_blobs();
    let id = uuid::Uuid::nil();
    store.add_blob_with_key(id, b"nil key").unwrap();
    let got = store.get_blob(id).unwrap().expect("should exist");
    assert_eq!(got, b"nil key");
}

#[test]
fn test_blob_with_key_multiple_distinct_keys() {
    let (_dir, store) = tmp_blobs();
    let ids: Vec<uuid::Uuid> = (0..5).map(|_| uuid::Uuid::now_v7()).collect();
    for (i, &id) in ids.iter().enumerate() {
        store.add_blob_with_key(id, &[i as u8]).unwrap();
    }
    for (i, &id) in ids.iter().enumerate() {
        let got = store.get_blob(id).unwrap().expect("should exist");
        assert_eq!(got, &[i as u8]);
    }
}

// ── JsonStorage (default config — no key field) ───────────────────────────────

#[test]
fn test_json_add_returns_uuid() {
    let (_dir, store) = default_json_store();
    let id = store.add_json(json!({"x": 1})).unwrap();
    assert!(!id.is_nil());
}

#[test]
fn test_json_add_and_get_roundtrip() {
    let (_dir, store) = default_json_store();
    let doc = json!({"name": "Alice", "age": 30});
    let id = store.add_json(doc.clone()).unwrap();
    let got = store.get_json(id).unwrap().expect("should exist");
    assert_eq!(got, doc);
}

#[test]
fn test_json_get_nonexistent_returns_none() {
    let (_dir, store) = default_json_store();
    let fake = uuid::Uuid::now_v7();
    assert!(store.get_json(fake).unwrap().is_none());
}

#[test]
fn test_json_update_changes_document() {
    let (_dir, store) = default_json_store();
    let id = store.add_json(json!({"v": 1})).unwrap();
    store.update_json(id, json!({"v": 2})).unwrap();
    let got = store.get_json(id).unwrap().expect("should exist");
    assert_eq!(got["v"], json!(2));
}

#[test]
fn test_json_update_nonexistent_is_ok() {
    let (_dir, store) = default_json_store();
    let fake = uuid::Uuid::now_v7();
    assert!(store.update_json(fake, json!({})).is_ok());
}

#[test]
fn test_json_drop_removes_record() {
    let (_dir, store) = default_json_store();
    let id = store.add_json(json!({"gone": true})).unwrap();
    store.drop_json(id).unwrap();
    assert!(store.get_json(id).unwrap().is_none());
}

#[test]
fn test_json_drop_nonexistent_is_ok() {
    let (_dir, store) = default_json_store();
    let fake = uuid::Uuid::now_v7();
    assert!(store.drop_json(fake).is_ok());
}

#[test]
fn test_json_clone_shares_store() {
    let (_dir, store) = default_json_store();
    let clone = store.clone();
    let id = store.add_json(json!({"shared": true})).unwrap();
    let got = clone.get_json(id).unwrap().expect("clone should see same store");
    assert_eq!(got["shared"], json!(true));
}

#[test]
fn test_json_default_key_deduplicates() {
    // No key_field → all docs share "default" key → add_json is always an upsert.
    let (_dir, store) = default_json_store();
    let id1 = store.add_json(json!({"n": 1})).unwrap();
    let id2 = store.add_json(json!({"n": 2})).unwrap();
    // Same key → id2 == id1 (upsert preserved the original UUID)
    assert_eq!(id1, id2);
    let got = store.get_json(id1).unwrap().expect("should exist");
    assert_eq!(got["n"], json!(2));
}

#[test]
fn test_json_preserves_nested_structure() {
    let (_dir, store) = default_json_store();
    let doc = json!({
        "user": { "name": "Bob", "roles": ["admin", "user"] },
        "active": true,
        "score": 9.5
    });
    let id = store.add_json(doc.clone()).unwrap();
    let got = store.get_json(id).unwrap().expect("should exist");
    assert_eq!(got, doc);
}

#[test]
fn test_json_add_produces_unique_uuids_with_different_keys() {
    let (_dir, store) = keyed_json_store("id");
    let id1 = store.add_json(json!({"id": "a", "v": 1})).unwrap();
    let id2 = store.add_json(json!({"id": "b", "v": 2})).unwrap();
    assert_ne!(id1, id2);
}

// ── JsonStorage with key_field ────────────────────────────────────────────────

#[test]
fn test_json_key_field_deduplicates_by_extracted_key() {
    let (_dir, store) = keyed_json_store("id");
    let id1 = store.add_json(json!({"id": "user-1", "name": "Alice"})).unwrap();
    let id2 = store.add_json(json!({"id": "user-1", "name": "Alicia"})).unwrap();
    // Same key → upsert → same UUID
    assert_eq!(id1, id2);
    let got = store.get_json(id1).unwrap().expect("should exist");
    assert_eq!(got["name"], json!("Alicia"));
}

#[test]
fn test_json_key_field_different_keys_produce_different_uuids() {
    let (_dir, store) = keyed_json_store("id");
    let id1 = store.add_json(json!({"id": "x"})).unwrap();
    let id2 = store.add_json(json!({"id": "y"})).unwrap();
    assert_ne!(id1, id2);
}

#[test]
fn test_json_key_field_falls_back_to_default_when_missing() {
    let (_dir, store) = keyed_json_store("id");
    // doc has no "id" field → uses "fallback" key
    let id1 = store.add_json(json!({"other": 1})).unwrap();
    let id2 = store.add_json(json!({"other": 2})).unwrap();
    // Both resolve to "fallback" → upsert
    assert_eq!(id1, id2);
}

#[test]
fn test_json_key_field_nested_path() {
    let (_dir, store) = keyed_json_store("meta.id");
    let id1 = store
        .add_json(json!({"meta": {"id": "nested-1"}, "v": 1}))
        .unwrap();
    let id2 = store
        .add_json(json!({"meta": {"id": "nested-1"}, "v": 2}))
        .unwrap();
    assert_eq!(id1, id2);
    let got = store.get_json(id1).unwrap().unwrap();
    assert_eq!(got["v"], json!(2));
}

#[test]
fn test_json_key_field_numeric_value() {
    let (_dir, store) = keyed_json_store("code");
    let id1 = store.add_json(json!({"code": 42, "msg": "first"})).unwrap();
    let id2 = store.add_json(json!({"code": 42, "msg": "second"})).unwrap();
    assert_eq!(id1, id2);
}

#[test]
fn test_json_key_field_bool_value() {
    let (_dir, store) = keyed_json_store("active");
    let id_t = store.add_json(json!({"active": true,  "x": 1})).unwrap();
    let id_f = store.add_json(json!({"active": false, "x": 2})).unwrap();
    // "true" vs "false" → different keys → different UUIDs
    assert_ne!(id_t, id_f);
}

#[test]
fn test_json_update_key_stays_consistent() {
    let (_dir, store) = keyed_json_store("src");
    let id = store.add_json(json!({"src": "A", "v": 0})).unwrap();
    // update changes the document but the key is re-extracted from the new doc
    store.update_json(id, json!({"src": "A", "v": 99})).unwrap();
    // Original key still resolves to same UUID
    let id2 = store.add_json(json!({"src": "A", "v": 1})).unwrap();
    assert_eq!(id, id2);
}

#[test]
fn test_json_single_quotes_in_value_are_handled() {
    let (_dir, store) = keyed_json_store("name");
    let doc = json!({"name": "O'Brien", "v": 1});
    let id = store.add_json(doc.clone()).unwrap();
    let got = store.get_json(id).unwrap().expect("should exist");
    assert_eq!(got, doc);
}

#[test]
fn test_json_single_quotes_in_default_key() {
    // default_key with a single quote must not cause SQL errors
    let (_dir, store) = tmp_json(JsonStorageConfig {
        key_field: None,
        default_key: "it's fine".to_string(),
    });
    let id = store.add_json(json!({"x": 1})).unwrap();
    assert!(!id.is_nil());
}

// ── BlobStorage::add_blob_with_string_key (+ companions) ─────────────────────

#[test]
fn test_blob_string_key_roundtrip() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("my-key", b"hello string key").unwrap();
    let got = store.get_blob_by_string_key("my-key").unwrap().expect("should exist");
    assert_eq!(got, b"hello string key");
}

#[test]
fn test_blob_string_key_get_nonexistent_returns_none() {
    let (_dir, store) = tmp_blobs();
    assert!(store.get_blob_by_string_key("no-such-key").unwrap().is_none());
}

#[test]
fn test_blob_string_key_duplicate_returns_err() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("dup", b"first").unwrap();
    assert!(store.add_blob_with_string_key("dup", b"second").is_err());
    // original data must be untouched
    let got = store.get_blob_by_string_key("dup").unwrap().expect("should exist");
    assert_eq!(got, b"first");
}

#[test]
fn test_blob_string_key_update_changes_data() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("upd", b"original").unwrap();
    store.update_blob_by_string_key("upd", b"replaced").unwrap();
    let got = store.get_blob_by_string_key("upd").unwrap().expect("should exist");
    assert_eq!(got, b"replaced");
}

#[test]
fn test_blob_string_key_update_nonexistent_is_ok() {
    let (_dir, store) = tmp_blobs();
    assert!(store.update_blob_by_string_key("ghost", b"data").is_ok());
}

#[test]
fn test_blob_string_key_drop_removes_record() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("gone", b"bye").unwrap();
    store.drop_blob_by_string_key("gone").unwrap();
    assert!(store.get_blob_by_string_key("gone").unwrap().is_none());
}

#[test]
fn test_blob_string_key_drop_nonexistent_is_ok() {
    let (_dir, store) = tmp_blobs();
    assert!(store.drop_blob_by_string_key("nothing").is_ok());
}

#[test]
fn test_blob_string_key_drop_and_reinsert() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("reuse", b"v1").unwrap();
    store.drop_blob_by_string_key("reuse").unwrap();
    store.add_blob_with_string_key("reuse", b"v2").unwrap();
    let got = store.get_blob_by_string_key("reuse").unwrap().expect("should exist after reinsert");
    assert_eq!(got, b"v2");
}

#[test]
fn test_blob_string_key_empty_payload() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("empty-data", b"").unwrap();
    let got = store.get_blob_by_string_key("empty-data").unwrap().expect("should exist");
    assert!(got.is_empty());
}

#[test]
fn test_blob_string_key_binary_full_byte_range() {
    let (_dir, store) = tmp_blobs();
    let data: Vec<u8> = (0u8..=255).collect();
    store.add_blob_with_string_key("binary", &data).unwrap();
    let got = store.get_blob_by_string_key("binary").unwrap().expect("should exist");
    assert_eq!(got, data);
}

#[test]
fn test_blob_string_key_with_single_quote() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("it's a key", b"payload").unwrap();
    let got = store.get_blob_by_string_key("it's a key").unwrap().expect("should exist");
    assert_eq!(got, b"payload");
}

#[test]
fn test_blob_string_key_with_double_single_quotes() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("O''Brien", b"data").unwrap();
    let got = store.get_blob_by_string_key("O''Brien").unwrap().expect("should exist");
    assert_eq!(got, b"data");
}

#[test]
fn test_blob_string_key_with_backslash() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key(r"path\to\key", b"val").unwrap();
    let got = store.get_blob_by_string_key(r"path\to\key").unwrap().expect("should exist");
    assert_eq!(got, b"val");
}

#[test]
fn test_blob_string_key_unicode() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("日本語キー", b"utf8 value").unwrap();
    let got = store.get_blob_by_string_key("日本語キー").unwrap().expect("should exist");
    assert_eq!(got, b"utf8 value");
}

#[test]
fn test_blob_string_key_long_key() {
    let (_dir, store) = tmp_blobs();
    let long_key = "k".repeat(4096);
    store.add_blob_with_string_key(&long_key, b"long key data").unwrap();
    let got = store.get_blob_by_string_key(&long_key).unwrap().expect("should exist");
    assert_eq!(got, b"long key data");
}

#[test]
fn test_blob_string_key_multiple_distinct_keys() {
    let (_dir, store) = tmp_blobs();
    let keys = ["alpha", "beta", "gamma", "delta", "epsilon"];
    for (i, &k) in keys.iter().enumerate() {
        store.add_blob_with_string_key(k, &[i as u8]).unwrap();
    }
    for (i, &k) in keys.iter().enumerate() {
        let got = store.get_blob_by_string_key(k).unwrap().expect("should exist");
        assert_eq!(got, &[i as u8]);
    }
}

#[test]
fn test_blob_string_key_different_keys_are_independent() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("key-a", b"aaa").unwrap();
    store.add_blob_with_string_key("key-b", b"bbb").unwrap();
    store.drop_blob_by_string_key("key-a").unwrap();
    let got = store.get_blob_by_string_key("key-b").unwrap().expect("key-b should survive");
    assert_eq!(got, b"bbb");
    assert!(store.get_blob_by_string_key("key-a").unwrap().is_none());
}

#[test]
fn test_blob_string_key_update_then_drop() {
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("cycle", b"v1").unwrap();
    store.update_blob_by_string_key("cycle", b"v2").unwrap();
    store.drop_blob_by_string_key("cycle").unwrap();
    assert!(store.get_blob_by_string_key("cycle").unwrap().is_none());
}

#[test]
fn test_blob_string_key_coexists_with_uuid_blob() {
    // String-keyed and UUID-keyed records share the table without interference.
    let (_dir, store) = tmp_blobs();
    store.add_blob_with_string_key("str-key", b"string side").unwrap();
    let uuid_id = store.add_blob(b"uuid side").unwrap();
    assert_eq!(
        store.get_blob_by_string_key("str-key").unwrap().unwrap(),
        b"string side",
    );
    assert_eq!(store.get_blob(uuid_id).unwrap().unwrap(), b"uuid side");
}

#[test]
fn test_blob_string_key_clone_shares_store() {
    let (_dir, store) = tmp_blobs();
    let clone = store.clone();
    store.add_blob_with_string_key("shared", b"visible").unwrap();
    let got = clone.get_blob_by_string_key("shared").unwrap().expect("clone should see same store");
    assert_eq!(got, b"visible");
}

//! Tests for `ShardsManager::script_*` — BUND script registry.

use bdslib::{EmbeddingEngine, ShardsManager};
use fastembed::EmbeddingModel;
use serde_json::json;
use std::sync::OnceLock;
use tempfile::TempDir;

// ── shared fastembed model ────────────────────────────────────────────────────

static ENGINE: OnceLock<EmbeddingEngine> = OnceLock::new();

fn get_engine() -> &'static EmbeddingEngine {
    ENGINE.get_or_init(|| EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None).unwrap())
}

fn tmp_manager() -> (TempDir, ShardsManager) {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    let content = format!(
        "{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n  similarity_threshold: 0.99\n}}"
    );
    std::fs::write(&config_path, content).unwrap();
    let mgr =
        ShardsManager::with_embedding(config_path.to_str().unwrap(), get_engine().clone()).unwrap();
    (dir, mgr)
}

// ── script_add ────────────────────────────────────────────────────────────────

#[test]
fn script_add_succeeds_with_valid_metadata() {
    let (_dir, mgr) = tmp_manager();
    let id = mgr.script_add(
        json!({ "name": "hello", "schedule": "*/5 * * * *" }),
        "2 2 + .",
    ).unwrap();
    assert!(!id.to_string().is_empty());
}

#[test]
fn script_add_preserves_extra_metadata() {
    let (_dir, mgr) = tmp_manager();
    let id = mgr.script_add(
        json!({
            "name": "report",
            "schedule": "0 9 * * *",
            "owner": "alice",
            "tags": ["daily", "stats"],
        }),
        "// daily report",
    ).unwrap();
    let meta = mgr.script_metadata(id).unwrap().expect("metadata");
    assert_eq!(meta["owner"], "alice");
    assert_eq!(meta["tags"][0], "daily");
}

#[test]
fn script_add_rejects_missing_name() {
    let (_dir, mgr) = tmp_manager();
    let err = mgr.script_add(
        json!({ "schedule": "*/5 * * * *" }),
        "noop",
    ).unwrap_err();
    assert!(err.to_string().contains("name"), "expected 'name' error, got: {err}");
}

#[test]
fn script_add_rejects_missing_schedule() {
    let (_dir, mgr) = tmp_manager();
    let err = mgr.script_add(
        json!({ "name": "no-schedule" }),
        "noop",
    ).unwrap_err();
    assert!(err.to_string().contains("schedule"), "expected 'schedule' error, got: {err}");
}

#[test]
fn script_add_rejects_empty_name() {
    let (_dir, mgr) = tmp_manager();
    let err = mgr.script_add(
        json!({ "name": "   ", "schedule": "*/5 * * * *" }),
        "noop",
    ).unwrap_err();
    assert!(err.to_string().contains("name"), "expected 'name' empty error, got: {err}");
}

#[test]
fn script_add_rejects_non_object_metadata() {
    let (_dir, mgr) = tmp_manager();
    let err = mgr.script_add(json!("just a string"), "noop").unwrap_err();
    assert!(err.to_string().contains("object"), "expected 'object' error, got: {err}");
}

// ── scripts (list) ────────────────────────────────────────────────────────────

#[test]
fn scripts_returns_id_schedule_pairs() {
    let (_dir, mgr) = tmp_manager();
    let id1 = mgr.script_add(json!({ "name": "a", "schedule": "*/1 * * * *" }), "1").unwrap();
    let id2 = mgr.script_add(json!({ "name": "b", "schedule": "0 0 * * *" }),    "2").unwrap();

    let listed = mgr.scripts().unwrap();
    assert_eq!(listed.len(), 2);

    let map: std::collections::HashMap<_, _> = listed.into_iter().collect();
    assert_eq!(map.get(&id1).unwrap(), "*/1 * * * *");
    assert_eq!(map.get(&id2).unwrap(), "0 0 * * *");
}

#[test]
fn scripts_with_metadata_returns_full_metadata() {
    let (_dir, mgr) = tmp_manager();
    let id = mgr.script_add(json!({ "name": "x", "schedule": "*/5 * * * *" }), "noop").unwrap();
    let listed = mgr.scripts_with_metadata().unwrap();
    let (lid, meta) = &listed[0];
    assert_eq!(*lid, id);
    assert_eq!(meta["name"], "x");
    assert_eq!(meta["schedule"], "*/5 * * * *");
}

#[test]
fn scripts_empty_when_none_added() {
    let (_dir, mgr) = tmp_manager();
    assert!(mgr.scripts().unwrap().is_empty());
}

// ── script (get) ──────────────────────────────────────────────────────────────

#[test]
fn script_returns_body_verbatim() {
    let (_dir, mgr) = tmp_manager();
    let body = "// hello bund\n2 2 + .\n";
    let id = mgr.script_add(json!({ "name": "h", "schedule": "*/1 * * * *" }), body).unwrap();
    let got = mgr.script(id).unwrap().expect("script body");
    assert_eq!(got, body);
}

#[test]
fn script_returns_none_for_missing_id() {
    let (_dir, mgr) = tmp_manager();
    let missing = uuid::Uuid::now_v7();
    assert!(mgr.script(missing).unwrap().is_none());
}

// ── update_script ─────────────────────────────────────────────────────────────

#[test]
fn update_script_replaces_metadata_and_body() {
    let (_dir, mgr) = tmp_manager();
    let id = mgr.script_add(
        json!({ "name": "v1", "schedule": "*/5 * * * *" }),
        "old body",
    ).unwrap();

    mgr.update_script(
        id,
        json!({ "name": "v2", "schedule": "0 0 * * *" }),
        "new body",
    ).unwrap();

    assert_eq!(mgr.script(id).unwrap().unwrap(), "new body");
    let meta = mgr.script_metadata(id).unwrap().unwrap();
    assert_eq!(meta["name"],     "v2");
    assert_eq!(meta["schedule"], "0 0 * * *");
}

#[test]
fn update_script_validates_metadata() {
    let (_dir, mgr) = tmp_manager();
    let id = mgr.script_add(
        json!({ "name": "v1", "schedule": "*/5 * * * *" }),
        "body",
    ).unwrap();

    let err = mgr.update_script(id, json!({ "name": "no-sched" }), "body").unwrap_err();
    assert!(err.to_string().contains("schedule"), "expected schedule error, got: {err}");

    // Original record must remain untouched.
    let meta = mgr.script_metadata(id).unwrap().unwrap();
    assert_eq!(meta["schedule"], "*/5 * * * *");
}

// ── script_delete ─────────────────────────────────────────────────────────────

#[test]
fn script_delete_removes_record() {
    let (_dir, mgr) = tmp_manager();
    let id = mgr.script_add(
        json!({ "name": "del", "schedule": "*/5 * * * *" }),
        "body",
    ).unwrap();
    mgr.script_delete(id).unwrap();
    assert!(mgr.script(id).unwrap().is_none());
    assert!(mgr.script_metadata(id).unwrap().is_none());
    assert!(mgr.scripts().unwrap().is_empty());
}

#[test]
fn script_delete_is_idempotent() {
    let (_dir, mgr) = tmp_manager();
    let missing = uuid::Uuid::now_v7();
    // Deleting a non-existent script must not error.
    mgr.script_delete(missing).unwrap();
}

// ── persistence across reopen ─────────────────────────────────────────────────

#[test]
fn scripts_persist_across_reopen() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    let content = format!(
        "{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n  similarity_threshold: 0.99\n}}"
    );
    std::fs::write(&config_path, content).unwrap();

    let id = {
        let mgr =
            ShardsManager::with_embedding(config_path.to_str().unwrap(), get_engine().clone()).unwrap();
        mgr.script_add(
            json!({ "name": "persistent", "schedule": "0 * * * *" }),
            "// persisted",
        ).unwrap()
    };

    let mgr2 =
        ShardsManager::with_embedding(config_path.to_str().unwrap(), get_engine().clone()).unwrap();
    assert_eq!(mgr2.script(id).unwrap().unwrap(), "// persisted");
    assert_eq!(mgr2.scripts().unwrap().len(), 1);
}

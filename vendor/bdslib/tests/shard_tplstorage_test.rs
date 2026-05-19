use bdslib::embedding::Model;
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

fn tpl_meta(name: &str, tags: &[&str]) -> serde_json::Value {
    json!({
        "name":        name,
        "tags":        tags,
        "type":        "template",
        "timestamp":   1_748_000_000u64,
        "created_at":  1_748_000_000u64,
    })
}

fn tpl_body(name: &str) -> Vec<u8> {
    format!("Template body for: {name}").into_bytes()
}

// ── add / get ─────────────────────────────────────────────────────────────────

#[test]
fn test_tpl_add_returns_non_nil_uuid() {
    let (_dir, shard) = tmp_shard();
    let id = shard.tpl_add(tpl_meta("alert", &["ops"]), &tpl_body("alert")).unwrap();
    assert!(!id.is_nil());
}

#[test]
fn test_tpl_get_metadata_roundtrip() {
    let (_dir, shard) = tmp_shard();
    let meta = tpl_meta("incident-report", &["sre", "oncall"]);
    let id = shard.tpl_add(meta.clone(), b"body content").unwrap();

    let stored = shard.tpl_get_metadata(id).unwrap().expect("should exist");
    assert_eq!(stored["name"], "incident-report");
    assert_eq!(stored["tags"][0], "sre");
    assert_eq!(stored["type"], "template");
}

#[test]
fn test_tpl_get_body_roundtrip() {
    let (_dir, shard) = tmp_shard();
    let body = b"Alert: {{ service }} is down. Action: page on-call.";
    let id = shard.tpl_add(tpl_meta("alert-page", &[]), body).unwrap();

    let stored = shard.tpl_get_body(id).unwrap().expect("should exist");
    assert_eq!(stored, body);
}

#[test]
fn test_tpl_get_metadata_nonexistent_returns_none() {
    let (_dir, shard) = tmp_shard();
    let result = shard.tpl_get_metadata(uuid::Uuid::now_v7()).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_tpl_get_body_nonexistent_returns_none() {
    let (_dir, shard) = tmp_shard();
    let result = shard.tpl_get_body(uuid::Uuid::now_v7()).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_tpl_add_multiple_each_has_unique_uuid() {
    let (_dir, shard) = tmp_shard();
    let id1 = shard.tpl_add(tpl_meta("a", &[]), b"body a").unwrap();
    let id2 = shard.tpl_add(tpl_meta("b", &[]), b"body b").unwrap();
    let id3 = shard.tpl_add(tpl_meta("c", &[]), b"body c").unwrap();
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);
}

// ── delete ────────────────────────────────────────────────────────────────────

#[test]
fn test_tpl_delete_removes_metadata_and_body() {
    let (_dir, shard) = tmp_shard();
    let id = shard.tpl_add(tpl_meta("runbook", &["ops"]), b"runbook steps").unwrap();
    assert!(shard.tpl_get_metadata(id).unwrap().is_some());
    assert!(shard.tpl_get_body(id).unwrap().is_some());

    shard.tpl_delete(id).unwrap();

    assert!(shard.tpl_get_metadata(id).unwrap().is_none());
    assert!(shard.tpl_get_body(id).unwrap().is_none());
}

#[test]
fn test_tpl_delete_nonexistent_is_ok() {
    let (_dir, shard) = tmp_shard();
    assert!(shard.tpl_delete(uuid::Uuid::now_v7()).is_ok());
}

#[test]
fn test_tpl_delete_removed_from_search() {
    let (_dir, shard) = tmp_shard();
    let id = shard
        .tpl_add(
            tpl_meta("unique-deletion-target", &[]),
            b"kubernetes pod crash loop back off detected",
        )
        .unwrap();

    let before = shard.tpl_search_text("kubernetes pod crash", 10).unwrap();
    assert!(!before.is_empty(), "should find template before delete");

    shard.tpl_delete(id).unwrap();

    let after = shard.tpl_search_text("kubernetes pod crash", 10).unwrap();
    assert!(
        after.iter().all(|r| r["id"].as_str().unwrap_or("") != id.to_string()),
        "deleted template must not appear in search results"
    );
}

// ── update ────────────────────────────────────────────────────────────────────

#[test]
fn test_tpl_update_metadata_changes_name() {
    let (_dir, shard) = tmp_shard();
    let id = shard.tpl_add(tpl_meta("original-name", &[]), b"body").unwrap();

    let mut updated = shard.tpl_get_metadata(id).unwrap().unwrap();
    updated.as_object_mut().unwrap()
        .insert("name".to_owned(), json!("updated-name"));
    shard.tpl_update_metadata(id, updated).unwrap();

    let stored = shard.tpl_get_metadata(id).unwrap().unwrap();
    assert_eq!(stored["name"], "updated-name");
}

#[test]
fn test_tpl_update_metadata_preserves_other_fields() {
    let (_dir, shard) = tmp_shard();
    let meta = json!({
        "name": "original",
        "tags": ["sre"],
        "type": "template",
        "timestamp": 1_748_000_000u64,
        "created_at": 1_748_000_000u64,
        "extra_field": "keep-me",
    });
    let id = shard.tpl_add(meta, b"body").unwrap();

    let mut updated = shard.tpl_get_metadata(id).unwrap().unwrap();
    updated.as_object_mut().unwrap()
        .insert("name".to_owned(), json!("renamed"));
    shard.tpl_update_metadata(id, updated).unwrap();

    let stored = shard.tpl_get_metadata(id).unwrap().unwrap();
    assert_eq!(stored["name"], "renamed");
    assert_eq!(stored["extra_field"], "keep-me");
    assert_eq!(stored["tags"][0], "sre");
}

#[test]
fn test_tpl_update_body_changes_content() {
    let (_dir, shard) = tmp_shard();
    let id = shard.tpl_add(tpl_meta("tmpl", &[]), b"original body").unwrap();

    shard.tpl_update_body(id, b"completely new body").unwrap();

    let stored = shard.tpl_get_body(id).unwrap().unwrap();
    assert_eq!(stored, b"completely new body");
}

#[test]
fn test_tpl_update_body_reflected_in_search() {
    let (_dir, shard) = tmp_shard();
    let id = shard
        .tpl_add(tpl_meta("tmpl", &[]), b"original unrelated content here")
        .unwrap();

    shard
        .tpl_update_body(id, b"database replication lag exceeded threshold alert runbook")
        .unwrap();

    let results = shard.tpl_search_text("database replication lag", 5).unwrap();
    assert!(!results.is_empty(), "updated body should be findable via search");
    assert_eq!(results[0]["id"].as_str().unwrap(), id.to_string());
}

// ── list ──────────────────────────────────────────────────────────────────────

#[test]
fn test_tpl_list_empty_store() {
    let (_dir, shard) = tmp_shard();
    let all = shard.tpl_list().unwrap();
    assert!(all.is_empty());
}

#[test]
fn test_tpl_list_returns_all_stored() {
    let (_dir, shard) = tmp_shard();
    let id1 = shard.tpl_add(tpl_meta("t1", &[]), b"body1").unwrap();
    let id2 = shard.tpl_add(tpl_meta("t2", &[]), b"body2").unwrap();
    let id3 = shard.tpl_add(tpl_meta("t3", &[]), b"body3").unwrap();

    let all = shard.tpl_list().unwrap();
    assert_eq!(all.len(), 3);

    let ids: Vec<_> = all.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
    assert!(ids.contains(&id3));
}

#[test]
fn test_tpl_list_metadata_contains_expected_fields() {
    let (_dir, shard) = tmp_shard();
    shard
        .tpl_add(
            json!({
                "name": "slo-breach",
                "tags": ["sre", "alerts"],
                "type": "template",
                "timestamp": 1_748_000_000u64,
                "created_at": 1_748_000_000u64,
            }),
            b"SLO breach runbook",
        )
        .unwrap();

    let all = shard.tpl_list().unwrap();
    assert_eq!(all.len(), 1);
    let (_, meta) = &all[0];
    assert_eq!(meta["name"], "slo-breach");
    assert_eq!(meta["tags"][0], "sre");
}

#[test]
fn test_tpl_list_excludes_deleted() {
    let (_dir, shard) = tmp_shard();
    shard.tpl_add(tpl_meta("keep", &[]), b"kept").unwrap();
    let del_id = shard.tpl_add(tpl_meta("delete-me", &[]), b"gone").unwrap();

    shard.tpl_delete(del_id).unwrap();

    let all = shard.tpl_list().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1["name"], "keep");
}

// ── search ────────────────────────────────────────────────────────────────────

#[test]
fn test_tpl_search_text_finds_relevant_template() {
    let (_dir, shard) = tmp_shard();
    shard
        .tpl_add(
            tpl_meta("cpu-alert", &["ops"]),
            b"CPU utilization exceeded threshold. Check top, look for runaway processes.",
        )
        .unwrap();
    shard
        .tpl_add(
            tpl_meta("disk-alert", &["ops"]),
            b"Disk space is low. Run df -h and identify large files to remove.",
        )
        .unwrap();

    let results = shard.tpl_search_text("cpu runaway process utilization", 5).unwrap();
    assert!(!results.is_empty());
    let top_name = results[0]["metadata"]["name"].as_str().unwrap_or("");
    assert_eq!(top_name, "cpu-alert", "most relevant result should be cpu-alert");
}

#[test]
fn test_tpl_search_text_result_has_score_and_document() {
    let (_dir, shard) = tmp_shard();
    shard
        .tpl_add(
            tpl_meta("oncall", &[]),
            b"On-call response procedure for service outages.",
        )
        .unwrap();

    let results = shard.tpl_search_text("service outage response", 5).unwrap();
    assert!(!results.is_empty());

    let r = &results[0];
    assert!(r.get("id").is_some(), "result must have 'id'");
    assert!(r.get("metadata").is_some(), "result must have 'metadata'");
    assert!(r.get("document").is_some(), "result must have 'document'");
    assert!(r.get("score").is_some(), "result must have 'score'");

    let score = r["score"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&score));
}

#[test]
fn test_tpl_search_scores_descending() {
    let (_dir, shard) = tmp_shard();
    shard
        .tpl_add(
            tpl_meta("net-runbook", &[]),
            b"Network connectivity failure runbook: check routes, DNS, firewall rules.",
        )
        .unwrap();
    shard
        .tpl_add(
            tpl_meta("db-runbook", &[]),
            b"Database slowdown runbook: check query plans, index usage, connection pool.",
        )
        .unwrap();
    shard
        .tpl_add(
            tpl_meta("baking", &[]),
            b"Chocolate cake recipe: flour, eggs, butter, sugar, cocoa powder.",
        )
        .unwrap();

    let results = shard.tpl_search_text("network DNS firewall connectivity", 10).unwrap();
    assert!(!results.is_empty());

    let scores: Vec<f64> = results.iter()
        .map(|r| r["score"].as_f64().unwrap_or(0.0))
        .collect();
    for w in scores.windows(2) {
        assert!(w[0] >= w[1], "scores not descending: {:?}", scores);
    }
}

#[test]
fn test_tpl_search_respects_limit() {
    let (_dir, shard) = tmp_shard();
    for i in 0..8u32 {
        shard
            .tpl_add(
                tpl_meta(&format!("alert-{i}"), &["ops"]),
                format!("Alert response template number {i} for server incident").as_bytes(),
            )
            .unwrap();
    }

    let results = shard.tpl_search_text("server alert response", 3).unwrap();
    assert!(results.len() <= 3);
}

#[test]
fn test_tpl_search_json_finds_by_metadata_fields() {
    let (_dir, shard) = tmp_shard();
    shard
        .tpl_add(
            json!({
                "name":       "k8s-crashloop",
                "tags":       ["kubernetes", "ops"],
                "type":       "template",
                "timestamp":  1_748_000_000u64,
                "created_at": 1_748_000_000u64,
            }),
            b"Pod is in CrashLoopBackOff. Check logs: kubectl logs <pod> -p",
        )
        .unwrap();

    let query = json!({ "name": "kubernetes pod crash loop" });
    let results = shard.tpl_search_json(&query, 5).unwrap();
    assert!(!results.is_empty());
}

// ── reindex ───────────────────────────────────────────────────────────────────

#[test]
fn test_tpl_reindex_returns_stored_count() {
    let (_dir, shard) = tmp_shard();
    shard.tpl_add(tpl_meta("t1", &[]), b"body one").unwrap();
    shard.tpl_add(tpl_meta("t2", &[]), b"body two").unwrap();
    shard.tpl_add(tpl_meta("t3", &[]), b"body three").unwrap();

    let count = shard.tpl_reindex().unwrap();
    assert_eq!(count, 3);
}

#[test]
fn test_tpl_reindex_search_still_works_after() {
    let (_dir, shard) = tmp_shard();
    shard
        .tpl_add(
            tpl_meta("post-reindex-target", &[]),
            b"memory out of memory oom killer invoked",
        )
        .unwrap();

    shard.tpl_reindex().unwrap();

    let results = shard.tpl_search_text("memory oom killer", 5).unwrap();
    assert!(!results.is_empty(), "search should work after reindex");
}

// ── sync ──────────────────────────────────────────────────────────────────────

#[test]
fn test_tpl_sync_does_not_error() {
    let (_dir, shard) = tmp_shard();
    shard.tpl_add(tpl_meta("t", &[]), b"body").unwrap();
    assert!(shard.sync().is_ok(), "sync (including tplstorage) must succeed");
}

// ── isolation from telemetry ──────────────────────────────────────────────────

#[test]
fn test_tpl_independent_of_telemetry() {
    let (_dir, shard) = tmp_shard();

    // Add telemetry — must not affect template list.
    shard.add(json!({ "timestamp": 1_000u64, "key": "cpu", "data": 85 })).unwrap();
    shard.add(json!({ "timestamp": 2_000u64, "key": "mem", "data": "pressure" })).unwrap();

    assert!(shard.tpl_list().unwrap().is_empty(), "telemetry must not pollute tplstorage");

    // Add a template — must not appear in observability search.
    let tpl_id = shard
        .tpl_add(tpl_meta("runbook", &[]), b"restart the service")
        .unwrap();

    // FTS index contains only fingerprints of telemetry records, not template bodies.
    let fts = shard.search_fts("restart", 10).unwrap();
    assert!(fts.is_empty(), "template body must not appear in telemetry FTS index");

    // The telemetry vector index holds only telemetry embeddings; the template UUID
    // must never appear in vector search results, even for semantically close queries.
    // (The search may return the two telemetry records as nearest neighbours, but
    // it must not return the template.)
    let vec = shard.search_vector(&json!({ "data": "restart the service" }), 5).unwrap();
    assert!(
        vec.iter().all(|d| d["id"].as_str().unwrap_or("") != tpl_id.to_string()),
        "template UUID must not appear in telemetry vector index"
    );
}

#[test]
fn test_tpl_and_telemetry_coexist_in_same_shard() {
    let (_dir, shard) = tmp_shard();

    let tel_id = shard
        .add(json!({ "timestamp": 1_000u64, "key": "cpu", "data": "high load" }))
        .unwrap();
    let tpl_id = shard
        .tpl_add(tpl_meta("cpu-runbook", &["ops"]), b"high cpu runbook steps")
        .unwrap();

    assert!(shard.get(tel_id).unwrap().is_some(), "telemetry record must be accessible");
    assert!(shard.tpl_get_metadata(tpl_id).unwrap().is_some(), "template must be accessible");

    // Each index only contains its own records.
    assert_eq!(shard.tpl_list().unwrap().len(), 1);

    let fts = shard.search_fts("load", 10).unwrap();
    assert!(
        fts.iter().all(|d| d["id"].as_str().unwrap_or("") == tel_id.to_string()),
        "only telemetry records in FTS index"
    );
}

// ── clone ─────────────────────────────────────────────────────────────────────

#[test]
fn test_tpl_clone_shares_storage() {
    let (_dir, shard) = tmp_shard();
    let clone = shard.clone();

    let id = shard.tpl_add(tpl_meta("shared", &[]), b"shared body").unwrap();

    // The clone must see the template added through the original handle.
    let meta = clone.tpl_get_metadata(id).unwrap();
    assert!(meta.is_some(), "clone must see templates added through original");
    assert_eq!(meta.unwrap()["name"], "shared");

    let body = clone.tpl_get_body(id).unwrap();
    assert_eq!(body.unwrap(), b"shared body");
}

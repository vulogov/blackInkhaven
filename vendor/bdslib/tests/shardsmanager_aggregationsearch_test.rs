use bdslib::{EmbeddingEngine, ShardsManager};
use fastembed::EmbeddingModel;
use serde_json::json;
use std::sync::OnceLock;
use tempfile::TempDir;

static ENGINE: OnceLock<EmbeddingEngine> = OnceLock::new();

fn get_engine() -> &'static EmbeddingEngine {
    ENGINE.get_or_init(|| EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None).unwrap())
}

fn write_config(dir: &TempDir) -> String {
    let config_path = dir.path().join("config.hjson");
    let dbpath = dir.path().join("db").to_str().unwrap().to_string();
    let content =
        format!("{{\n  dbpath: \"{dbpath}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n}}");
    std::fs::write(&config_path, content).unwrap();
    config_path.to_str().unwrap().to_string()
}

fn tmp_manager() -> (TempDir, ShardsManager) {
    let dir = TempDir::new().unwrap();
    let config_path = write_config(&dir);
    let mgr = ShardsManager::with_embedding(&config_path, get_engine().clone()).unwrap();
    (dir, mgr)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn tel(key: &str, data: &str) -> serde_json::Value {
    json!({ "timestamp": now_secs(), "key": key, "data": data })
}

// ── result structure ──────────────────────────────────────────────────────────

#[test]
fn test_aggregationsearch_returns_both_keys() {
    let (_dir, mgr) = tmp_manager();
    let result = mgr.aggregationsearch("1h", "anything").unwrap();
    assert!(
        result.get("observability").is_some(),
        "result must have 'observability' key"
    );
    assert!(
        result.get("documents").is_some(),
        "result must have 'documents' key"
    );
}

#[test]
fn test_aggregationsearch_both_keys_are_arrays() {
    let (_dir, mgr) = tmp_manager();
    let result = mgr.aggregationsearch("1h", "anything").unwrap();
    assert!(
        result["observability"].is_array(),
        "'observability' must be a JSON array"
    );
    assert!(
        result["documents"].is_array(),
        "'documents' must be a JSON array"
    );
}

// ── empty store ───────────────────────────────────────────────────────────────

#[test]
fn test_aggregationsearch_empty_store_returns_empty_arrays() {
    let (_dir, mgr) = tmp_manager();
    let result = mgr.aggregationsearch("1h", "memory heap exhaustion").unwrap();
    assert_eq!(
        result["observability"].as_array().unwrap().len(),
        0,
        "no telemetry ingested — observability must be empty"
    );
    assert_eq!(
        result["documents"].as_array().unwrap().len(),
        0,
        "no documents stored — documents must be empty"
    );
}

// ── telemetry (observability) results ─────────────────────────────────────────

#[test]
fn test_aggregationsearch_observability_finds_telemetry() {
    let (_dir, mgr) = tmp_manager();
    mgr.add(tel("svc.error", "payment circuit breaker opened")).unwrap();

    let result = mgr
        .aggregationsearch("1h", "payment circuit breaker")
        .unwrap();
    let obs = result["observability"].as_array().unwrap();
    assert!(
        !obs.is_empty(),
        "matching telemetry record must appear in 'observability'"
    );
}

#[test]
fn test_aggregationsearch_observability_results_have_score() {
    let (_dir, mgr) = tmp_manager();
    mgr.add(tel("db.pool", "connection pool exhausted")).unwrap();

    let result = mgr
        .aggregationsearch("1h", "connection pool exhausted")
        .unwrap();
    let obs = result["observability"].as_array().unwrap();
    for hit in obs {
        assert!(
            hit.get("_score").is_some(),
            "every observability hit must carry '_score': {hit}"
        );
    }
}

#[test]
fn test_aggregationsearch_observability_scores_descending() {
    let (_dir, mgr) = tmp_manager();
    mgr.add(tel("net.latency", "network timeout detected on ingress")).unwrap();
    mgr.add(tel("disk.io", "disk write latency elevated")).unwrap();
    mgr.add(tel("cpu.temp", "thermal throttling engaged")).unwrap();

    let result = mgr
        .aggregationsearch("1h", "network timeout")
        .unwrap();
    let scores: Vec<f64> = result["observability"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|h| h.get("_score").and_then(|v| v.as_f64()))
        .collect();
    for w in scores.windows(2) {
        assert!(
            w[0] >= w[1],
            "observability scores must be non-increasing: {} < {}",
            w[0],
            w[1]
        );
    }
}

#[test]
fn test_aggregationsearch_observability_results_have_id_and_timestamp() {
    let (_dir, mgr) = tmp_manager();
    mgr.add(tel("auth.svc", "token validation failed")).unwrap();

    let result = mgr.aggregationsearch("1h", "token validation").unwrap();
    let obs = result["observability"].as_array().unwrap();
    for hit in obs {
        assert!(
            hit.get("id").is_some(),
            "observability hit missing 'id': {hit}"
        );
        assert!(
            hit.get("timestamp").is_some(),
            "observability hit missing 'timestamp': {hit}"
        );
    }
}

// ── document-store results ────────────────────────────────────────────────────

#[test]
fn test_aggregationsearch_documents_finds_added_doc() {
    let (_dir, mgr) = tmp_manager();
    mgr.doc_add(
        json!({"name": "OOM Runbook", "category": "runbook"}),
        b"Respond to out-of-memory events by restarting the affected pod",
    )
    .unwrap();

    let result = mgr
        .aggregationsearch("1h", "out of memory pod restart")
        .unwrap();
    let docs = result["documents"].as_array().unwrap();
    assert!(
        !docs.is_empty(),
        "matching document must appear in 'documents'"
    );
}

#[test]
fn test_aggregationsearch_documents_results_have_id_and_score() {
    let (_dir, mgr) = tmp_manager();
    mgr.doc_add(
        json!({"name": "DB Failover Guide"}),
        b"Primary database failover: promote replica, update DNS, notify on-call",
    )
    .unwrap();

    let result = mgr
        .aggregationsearch("1h", "database failover replica")
        .unwrap();
    let docs = result["documents"].as_array().unwrap();
    for hit in docs {
        assert!(hit.get("id").is_some(), "document hit missing 'id': {hit}");
        assert!(
            hit.get("score").is_some(),
            "document hit missing 'score': {hit}"
        );
    }
}

#[test]
fn test_aggregationsearch_documents_results_have_metadata_and_content() {
    let (_dir, mgr) = tmp_manager();
    mgr.doc_add(
        json!({"name": "Cache Eviction Runbook"}),
        b"When cache hit rate drops below 60%, flush stale keys and warm the cache",
    )
    .unwrap();

    let result = mgr
        .aggregationsearch("1h", "cache hit rate eviction")
        .unwrap();
    let docs = result["documents"].as_array().unwrap();
    for hit in docs {
        assert!(
            hit.get("metadata").is_some(),
            "document hit missing 'metadata': {hit}"
        );
        assert!(
            hit.get("document").is_some(),
            "document hit missing 'document': {hit}"
        );
    }
}

// ── combined results ──────────────────────────────────────────────────────────

#[test]
fn test_aggregationsearch_populates_both_sides_independently() {
    let (_dir, mgr) = tmp_manager();

    mgr.add(tel("payment.svc", "circuit breaker tripped on payment gateway"))
        .unwrap();
    mgr.doc_add(
        json!({"name": "Payment Circuit Breaker Runbook"}),
        b"When the payment gateway circuit breaker trips, check downstream latency first",
    )
    .unwrap();

    let result = mgr
        .aggregationsearch("1h", "payment circuit breaker")
        .unwrap();

    let obs = result["observability"].as_array().unwrap();
    let docs = result["documents"].as_array().unwrap();
    assert!(
        !obs.is_empty(),
        "telemetry hit expected in 'observability'"
    );
    assert!(!docs.is_empty(), "document hit expected in 'documents'");
}

// ── error handling ────────────────────────────────────────────────────────────

#[test]
fn test_aggregationsearch_invalid_duration_errors() {
    let (_dir, mgr) = tmp_manager();
    let result = mgr.aggregationsearch("not-a-duration", "query");
    assert!(
        result.is_err(),
        "invalid duration must propagate as an error"
    );
}

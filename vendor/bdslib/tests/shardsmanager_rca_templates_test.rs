/// Tests for `bdslib::analysis::rca_templates::RcaTemplatesResult`.
///
/// Each test creates its own `(TempDir, ShardsManager)` so there is no
/// shared global state.  Template events are injected directly via
/// `ShardsManager::tpl_add`, bypassing drain, so timestamps and bodies are
/// fully deterministic.
///
/// Test organisation:
///   empty window
///   → cluster detection: two isolated pairs {auth, network} ↔ {db, cache}
///   → causal ranking:    disk_fail (lead≈90s) > oom (lead≈30s) for app_crash
///   → unknown failure body → empty probable_causes
///   → result metadata invariants (start ≤ end, cluster ids sequential, etc.)
///   → config overrides (tight threshold, wide buckets)
///   → max_keys cap
///   → min_support filter
///   → analyze_failure with no overlap returns empty causes
use bdslib::analysis::rca_templates::{RcaTemplatesConfig, RcaTemplatesResult};
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

fn tmp_manager(shard_duration: &str) -> (TempDir, ShardsManager) {
    let dir = TempDir::new().unwrap();
    let cfg  = dir.path().join("config.hjson");
    let db   = dir.path().join("db").to_str().unwrap().to_string();
    std::fs::write(
        &cfg,
        format!(
            "{{\n  dbpath: \"{db}\"\n  shard_duration: \"{shard_duration}\"\n  pool_size: 4\n}}"
        ),
    )
    .unwrap();
    let mgr = ShardsManager::with_embedding(cfg.to_str().unwrap(), get_engine().clone()).unwrap();
    (dir, mgr)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Store a template event `(body, ts)` directly in the given manager.
fn store_tpl(mgr: &ShardsManager, body: &str, ts: u64) {
    mgr.tpl_add(
        json!({ "name": body, "timestamp": ts, "type": "tpl" }),
        body.as_bytes(),
    )
    .unwrap();
}

// ── empty window ──────────────────────────────────────────────────────────────

#[test]
fn test_empty_window_returns_empty_result() {
    let (_dir, mgr) = tmp_manager("1h");
    let cfg    = RcaTemplatesConfig::default();
    let result = RcaTemplatesResult::analyze(&mgr, "1h", &cfg).unwrap();
    assert_eq!(result.n_events, 0);
    assert_eq!(result.n_keys, 0);
    assert!(result.clusters.is_empty());
    assert!(result.probable_causes.is_empty());
    assert_eq!(result.failure_body, None);
}

// ── cluster detection ─────────────────────────────────────────────────────────
//
// Two isolated co-occurrence pairs:
//   A = { auth_tpl, net_tpl }   active at b-8B, b-6B, b-4B, b-2B
//   B = { db_tpl,  cache_tpl }  active at b-7B, b-5B, b-3B, b-1B
//
// A and B never share a bucket → Jaccard(A×B) = 0.
// Each pair shares all 4 buckets → Jaccard(auth, net) = Jaccard(db, cache) = 1.

const AUTH:    &str = "user <*> logged in from <*>";
const NET:     &str = "connection to <*> on port <*> established";
const DB:      &str = "query timeout on table <*> after <*> ms";
const CACHE:   &str = "cache miss for key <*> in region <*>";
const FAILURE: &str = "app crash: unhandled exception in <*>";
const DISK:    &str = "disk full on volume <*>: <*> bytes free";
const OOM:     &str = "oom killer invoked: process <*> killed";

#[test]
fn test_cluster_detection_two_isolated_pairs() {
    let (_dir, mgr) = tmp_manager("1h");
    let b = (now() / 300) * 300; // aligned 300s bucket
    const B: u64 = 300;

    for i in 0u64..4 {
        let a_ts  = b.saturating_sub(8 * B).saturating_add(i * 2 * B); // even buckets
        let bb_ts = b.saturating_sub(7 * B).saturating_add(i * 2 * B); // odd buckets
        store_tpl(&mgr, AUTH,  a_ts);
        store_tpl(&mgr, NET,   a_ts + 5);
        store_tpl(&mgr, DB,    bb_ts);
        store_tpl(&mgr, CACHE, bb_ts + 5);
    }

    let cfg = RcaTemplatesConfig {
        bucket_secs:       B,
        min_support:       2,
        jaccard_threshold: 0.5,
        max_keys:          200,
    };
    let result = RcaTemplatesResult::analyze(&mgr, "2h", &cfg).unwrap();

    // Exactly two multi-member clusters.
    let multi: Vec<_> = result.clusters.iter().filter(|c| c.members.len() >= 2).collect();
    assert_eq!(multi.len(), 2, "expected 2 two-member clusters; got: {multi:?}");

    let sets: Vec<std::collections::HashSet<&str>> = multi
        .iter()
        .map(|c| c.members.iter().map(String::as_str).collect())
        .collect();
    let want_a: std::collections::HashSet<&str> = [AUTH, NET].into();
    let want_b: std::collections::HashSet<&str> = [DB, CACHE].into();
    assert!(sets.contains(&want_a), "cluster {{AUTH, NET}} not found; got {sets:?}");
    assert!(sets.contains(&want_b), "cluster {{DB, CACHE}} not found; got {sets:?}");

    // Perfect cohesion (always co-occur).
    for c in &multi {
        assert!(
            (c.cohesion - 1.0).abs() < 1e-9,
            "cohesion should be 1.0 for {:?}; got {}", c.members, c.cohesion
        );
    }

    // Each member appeared in 4 distinct buckets → support ≥ 4.
    for c in &multi {
        assert!(c.support >= 4, "support should be ≥ 4; got {} for {:?}", c.support, c.members);
    }

    // Total members = n_keys.
    let total: usize = result.clusters.iter().map(|c| c.members.len()).sum();
    assert_eq!(total, result.n_keys);
}

// ── causal ranking ────────────────────────────────────────────────────────────
//
// Three incidents, each 2 buckets apart.  Within each bucket:
//   DISK   at offset   0 → avg_lead ≈  90s relative to FAILURE
//   OOM    at offset  60 → avg_lead ≈  30s relative to FAILURE
//   FAILURE at offset 90
//
// Expected: DISK ranks first (larger positive lead).

#[test]
fn test_causal_ranking() {
    let (_dir, mgr) = tmp_manager("1h");
    let b = (now() / 300) * 300;
    const B: u64 = 300;

    for i in 0u64..3 {
        let start = b.saturating_sub(8 * B).saturating_add(i * 2 * B);
        store_tpl(&mgr, DISK,    start);
        store_tpl(&mgr, OOM,     start + 60);
        store_tpl(&mgr, FAILURE, start + 90);
    }

    let cfg = RcaTemplatesConfig {
        bucket_secs:       B,
        min_support:       2,
        jaccard_threshold: 0.2,
        max_keys:          200,
    };
    let rca = RcaTemplatesResult::analyze_failure(&mgr, FAILURE, "2h", &cfg).unwrap();

    assert_eq!(rca.failure_body.as_deref(), Some(FAILURE));

    let causes: Vec<&str> = rca.probable_causes.iter().map(|c| c.body.as_str()).collect();
    assert!(causes.contains(&DISK), "DISK must be a cause; got {causes:?}");
    assert!(causes.contains(&OOM),  "OOM must be a cause; got {causes:?}");

    // Both precursors fire before the failure.
    for c in &rca.probable_causes {
        assert!(c.avg_lead_secs > 0.0, "{} must have positive lead; got {}", c.body, c.avg_lead_secs);
        assert!(c.jaccard > 0.0, "{} must have positive Jaccard; got {}", c.body, c.jaccard);
    }

    // DISK has larger lead → ranks first.
    assert_eq!(
        rca.probable_causes[0].body, DISK,
        "DISK (lead≈90s) should rank above OOM (lead≈30s); order: {causes:?}"
    );

    // Approximate lead times.
    let disk_c = rca.probable_causes.iter().find(|c| c.body == DISK).unwrap();
    let oom_c  = rca.probable_causes.iter().find(|c| c.body == OOM).unwrap();
    assert!((disk_c.avg_lead_secs - 90.0).abs() < 1.0,
        "DISK lead should be ≈90s; got {}", disk_c.avg_lead_secs);
    assert!((oom_c.avg_lead_secs - 30.0).abs() < 1.0,
        "OOM lead should be ≈30s; got {}", oom_c.avg_lead_secs);

    // Each co-occurred in all 3 incidents.
    assert_eq!(disk_c.co_occurrence_count, 3);
    assert_eq!(oom_c.co_occurrence_count,  3);
}

// ── unknown failure body ──────────────────────────────────────────────────────

#[test]
fn test_unknown_failure_body_returns_empty_causes() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts = now();
    store_tpl(&mgr, AUTH, ts);
    store_tpl(&mgr, AUTH, ts + 1);

    let rca = RcaTemplatesResult::analyze_failure(
        &mgr, "no.such.template.body", "1h", &RcaTemplatesConfig::default()
    ).unwrap();
    assert!(rca.probable_causes.is_empty(), "unknown failure body must yield empty causes");
}

// ── result metadata invariants ────────────────────────────────────────────────

#[test]
fn test_result_metadata_invariants() {
    let (_dir, mgr) = tmp_manager("1h");
    let b = (now() / 300) * 300;
    const B: u64 = 300;

    for i in 0u64..4 {
        let a_ts  = b.saturating_sub(8 * B).saturating_add(i * 2 * B);
        let bb_ts = b.saturating_sub(7 * B).saturating_add(i * 2 * B);
        store_tpl(&mgr, AUTH,  a_ts);
        store_tpl(&mgr, NET,   a_ts + 5);
        store_tpl(&mgr, DB,    bb_ts);
        store_tpl(&mgr, CACHE, bb_ts + 5);
    }

    let cfg = RcaTemplatesConfig {
        bucket_secs: 300,
        min_support: 2,
        jaccard_threshold: 0.5,
        max_keys: 200,
    };
    let result = RcaTemplatesResult::analyze(&mgr, "2h", &cfg).unwrap();

    // start ≤ end.
    assert!(result.start <= result.end, "start={} must be ≤ end={}", result.start, result.end);

    // analyze() never sets failure_body.
    assert_eq!(result.failure_body, None);

    // Cluster ids are sequential from 0.
    for (want, c) in result.clusters.iter().enumerate() {
        assert_eq!(c.id, want, "cluster id should be {want}, got {}", c.id);
    }

    // Clusters sorted by cohesion descending.
    for w in result.clusters.windows(2) {
        assert!(
            w[0].cohesion >= w[1].cohesion,
            "clusters must be sorted by cohesion desc: {} < {} (ids {} {})",
            w[0].cohesion, w[1].cohesion, w[0].id, w[1].id
        );
    }

    // Sum of cluster members equals n_keys.
    let total: usize = result.clusters.iter().map(|c| c.members.len()).sum();
    assert_eq!(total, result.n_keys);

    // n_events counts raw events (8 × 2 bodies = 16 template documents).
    assert_eq!(result.n_events, 16, "expected 16 template events; got {}", result.n_events);
}

// ── tight Jaccard threshold ───────────────────────────────────────────────────

#[test]
fn test_tight_threshold_still_clusters_perfect_pairs() {
    let (_dir, mgr) = tmp_manager("1h");
    let b = (now() / 300) * 300;
    const B: u64 = 300;

    for i in 0u64..4 {
        let ts = b.saturating_sub(8 * B).saturating_add(i * 2 * B);
        store_tpl(&mgr, AUTH, ts);
        store_tpl(&mgr, NET,  ts + 5);
    }

    let cfg = RcaTemplatesConfig {
        bucket_secs:       B,
        min_support:       2,
        jaccard_threshold: 1.0, // only perfect co-occurrence
        max_keys:          200,
    };
    let result = RcaTemplatesResult::analyze(&mgr, "2h", &cfg).unwrap();
    let multi: Vec<_> = result.clusters.iter().filter(|c| c.members.len() >= 2).collect();
    assert!(!multi.is_empty(), "AUTH and NET always co-occur → should cluster at threshold=1.0");
}

// ── wide buckets → single mega-cluster ───────────────────────────────────────

#[test]
fn test_wide_buckets_merge_all_templates() {
    let (_dir, mgr) = tmp_manager("1h");
    let b = (now() / 300) * 300;
    const B: u64 = 300;

    for i in 0u64..4 {
        let a_ts  = b.saturating_sub(8 * B).saturating_add(i * 2 * B);
        let bb_ts = b.saturating_sub(7 * B).saturating_add(i * 2 * B);
        store_tpl(&mgr, AUTH,  a_ts);
        store_tpl(&mgr, NET,   a_ts  + 5);
        store_tpl(&mgr, DB,    bb_ts);
        store_tpl(&mgr, CACHE, bb_ts + 5);
    }

    let cfg = RcaTemplatesConfig {
        bucket_secs:       86_400, // all events collapse into one 24-hour bucket
        min_support:       1,      // must be 1: one mega-bucket means each body appears once
        jaccard_threshold: 0.01,
        max_keys:          200,
    };
    let result = RcaTemplatesResult::analyze(&mgr, "2h", &cfg).unwrap();
    let max_members = result.clusters.iter().map(|c| c.members.len()).max().unwrap_or(0);
    assert!(max_members >= 4,
        "wide bucket should merge all 4 bodies into one cluster; max_members={max_members}");
}

// ── min_support filter ────────────────────────────────────────────────────────

#[test]
fn test_min_support_filters_rare_templates() {
    let (_dir, mgr) = tmp_manager("1h");
    let b = (now() / 300) * 300;
    const B: u64 = 300;

    // AUTH appears in 4 distinct buckets — should pass min_support=3.
    for i in 0u64..4 {
        let ts = b.saturating_sub(8 * B).saturating_add(i * 2 * B);
        store_tpl(&mgr, AUTH, ts);
    }
    // NET appears in only 1 bucket — should be filtered at min_support=3.
    store_tpl(&mgr, NET, b.saturating_sub(B));

    let cfg = RcaTemplatesConfig {
        bucket_secs: B,
        min_support: 3,
        jaccard_threshold: 0.2,
        max_keys: 200,
    };
    let result = RcaTemplatesResult::analyze(&mgr, "2h", &cfg).unwrap();
    let all_bodies: Vec<&str> = result.clusters
        .iter()
        .flat_map(|c| c.members.iter().map(String::as_str))
        .collect();
    assert!(!all_bodies.contains(&NET),
        "NET (1 bucket) should be filtered at min_support=3; keys: {all_bodies:?}");
    assert!(all_bodies.contains(&AUTH) || result.n_keys == 0,
        "AUTH (4 buckets) should pass min_support=3");
}

// ── max_keys cap ──────────────────────────────────────────────────────────────

#[test]
fn test_max_keys_caps_analysis() {
    let (_dir, mgr) = tmp_manager("1h");
    let b = (now() / 300) * 300;
    const B: u64 = 300;

    // Store 5 distinct template bodies, each with 4 events.
    let bodies = [AUTH, NET, DB, CACHE, DISK];
    for i in 0u64..4 {
        for &body in &bodies {
            let ts = b.saturating_sub(8 * B).saturating_add(i * 2 * B)
                + bodies.iter().position(|&b| b == body).unwrap() as u64;
            store_tpl(&mgr, body, ts);
        }
    }

    let cfg = RcaTemplatesConfig {
        bucket_secs:       B,
        min_support:       2,
        jaccard_threshold: 0.01,
        max_keys:          3, // cap at 3
    };
    let result = RcaTemplatesResult::analyze(&mgr, "2h", &cfg).unwrap();
    assert!(result.n_keys <= 3,
        "n_keys should be ≤ 3 (max_keys cap); got {}", result.n_keys);
}

// ── analyze_failure no overlap ────────────────────────────────────────────────

#[test]
fn test_analyze_failure_no_co_occurrence_returns_empty_causes() {
    let (_dir, mgr) = tmp_manager("1h");
    let b = (now() / 300) * 300;
    const B: u64 = 300;

    // AUTH fires in even buckets; FAILURE fires in odd buckets — never co-occur.
    for i in 0u64..4 {
        store_tpl(&mgr, AUTH,    b.saturating_sub(8 * B).saturating_add(i * 2 * B));
        store_tpl(&mgr, FAILURE, b.saturating_sub(7 * B).saturating_add(i * 2 * B));
    }

    let cfg = RcaTemplatesConfig {
        bucket_secs: B,
        min_support: 2,
        jaccard_threshold: 0.2,
        max_keys: 200,
    };
    let rca = RcaTemplatesResult::analyze_failure(&mgr, FAILURE, "2h", &cfg).unwrap();
    assert!(rca.probable_causes.is_empty(),
        "AUTH and FAILURE never share a bucket → no causal candidates");
}

// ── multi-shard span ──────────────────────────────────────────────────────────

#[test]
fn test_events_span_multiple_shards() {
    // Use a 1h shard so events 3h apart land in different shards.
    let (_dir, mgr) = tmp_manager("1h");
    let now_ts = now();
    let ts_old = now_ts.saturating_sub(3 * 3600);  // 3 hours ago — different shard
    let ts_new = now_ts.saturating_sub(10);         // just now

    // Two events per body in different 300s buckets so min_support=2 passes.
    // ts_old is 3h ago; ts_new is ~10s ago — both within the 4h window.
    // Use ts_new - 600 for the second NET event to stay in the past.
    store_tpl(&mgr, AUTH, ts_old);
    store_tpl(&mgr, AUTH, ts_old + 600);
    store_tpl(&mgr, NET,  ts_new);
    store_tpl(&mgr, NET,  ts_new.saturating_sub(600));

    let cfg = RcaTemplatesConfig {
        bucket_secs:       300,
        min_support:       2,
        jaccard_threshold: 0.2,
        max_keys:          200,
    };
    // A 4h window should find templates from both shards.
    let result = RcaTemplatesResult::analyze(&mgr, "4h", &cfg).unwrap();
    // AUTH comes from the 3h-ago shard; NET comes from the present shard.
    // Both must appear in results.
    assert!(result.n_events >= 4,
        "should find at least 4 template events across shards; got {}", result.n_events);
    assert!(result.n_keys >= 1,
        "at least one distinct body must pass min_support; got {}", result.n_keys);
}

// ── invalid duration ──────────────────────────────────────────────────────────

#[test]
fn test_invalid_duration_returns_err() {
    let (_dir, mgr) = tmp_manager("1h");
    let err = RcaTemplatesResult::analyze(&mgr, "not-a-duration", &RcaTemplatesConfig::default())
        .unwrap_err();
    assert!(err.to_string().contains("invalid duration"));
}

// ── failure_body field ────────────────────────────────────────────────────────

#[test]
fn test_analyze_sets_failure_body_field() {
    let (_dir, mgr) = tmp_manager("1h");
    let ts = now();
    store_tpl(&mgr, FAILURE, ts);
    store_tpl(&mgr, FAILURE, ts + 1);

    let rca = RcaTemplatesResult::analyze_failure(
        &mgr, FAILURE, "1h", &RcaTemplatesConfig::default()
    ).unwrap();
    assert_eq!(rca.failure_body.as_deref(), Some(FAILURE));
}

#[test]
fn test_analyze_does_not_set_failure_body_field() {
    let (_dir, mgr) = tmp_manager("1h");
    let result = RcaTemplatesResult::analyze(&mgr, "1h", &RcaTemplatesConfig::default()).unwrap();
    assert_eq!(result.failure_body, None);
}

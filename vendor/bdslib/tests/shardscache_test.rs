use bdslib::embedding::Model;
use bdslib::observability::ObservabilityStorageConfig;
use bdslib::shardscache::ShardsCache;
use bdslib::EmbeddingEngine;
use serde_json::json;
use std::sync::OnceLock;
use std::time::{Duration, UNIX_EPOCH};
use tempfile::TempDir;

// ── shared model ──────────────────────────────────────────────────────────────

static ENGINE: OnceLock<EmbeddingEngine> = OnceLock::new();

fn engine() -> &'static EmbeddingEngine {
    ENGINE.get_or_init(|| EmbeddingEngine::new(Model::AllMiniLML6V2, None).unwrap())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn at(secs: u64) -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn tmp_cache(duration: &str) -> (TempDir, ShardsCache) {
    let dir = TempDir::new().unwrap();
    let cache = ShardsCache::new(dir.path().to_str().unwrap(), duration, 4, engine().clone())
        .unwrap();
    (dir, cache)
}

fn tmp_cache_threshold(duration: &str, threshold: f32) -> (TempDir, ShardsCache) {
    let dir = TempDir::new().unwrap();
    let cache = ShardsCache::with_config(
        dir.path().to_str().unwrap(),
        duration,
        4,
        engine().clone(),
        ObservabilityStorageConfig { similarity_threshold: threshold },
        16,
    )
    .unwrap();
    (dir, cache)
}

fn tel(key: &str, data: serde_json::Value, ts: u64) -> serde_json::Value {
    json!({ "timestamp": ts, "key": key, "data": data })
}

// ── construction ──────────────────────────────────────────────────────────────

#[test]
fn test_new_creates_root_directory() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("cache");
    ShardsCache::new(root.to_str().unwrap(), "1day", 4, engine().clone()).unwrap();
    assert!(root.exists());
}

#[test]
fn test_new_creates_catalog_db() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("cache");
    ShardsCache::new(root.to_str().unwrap(), "1day", 4, engine().clone()).unwrap();
    assert!(root.join("shards_info.db").exists());
}

#[test]
fn test_invalid_duration_string_is_error() {
    let dir = TempDir::new().unwrap();
    let result = ShardsCache::new(
        dir.path().to_str().unwrap(),
        "notaduration",
        4,
        engine().clone(),
    );
    assert!(result.is_err());
}

#[test]
fn test_zero_duration_is_error() {
    let dir = TempDir::new().unwrap();
    let result = ShardsCache::new(
        dir.path().to_str().unwrap(),
        "0s",
        4,
        engine().clone(),
    );
    assert!(result.is_err());
}

#[test]
fn test_with_config_accepts_custom_threshold() {
    let (_dir, _cache) = tmp_cache_threshold("1day", 0.92);
}

#[test]
fn test_various_duration_formats_are_accepted() {
    let dir = TempDir::new().unwrap();
    // humantime accepts many formats
    for dur in &["1h", "30min", "1day", "7days", "3600s"] {
        let root = dir.path().join(dur.replace(' ', "_"));
        ShardsCache::new(root.to_str().unwrap(), dur, 2, engine().clone()).unwrap();
    }
}

// ── shard() – auto-create ─────────────────────────────────────────────────────

#[test]
fn test_shard_auto_creates_for_uncovered_timestamp() {
    let (_dir, cache) = tmp_cache("1day");
    let shard = cache.shard(at(1_700_000_000)).unwrap();
    let id = shard
        .add(tel("k", json!(1), 1_700_000_000))
        .unwrap();
    assert!(shard.get(id).unwrap().is_some());
}

#[test]
fn test_shard_auto_create_registers_in_catalog() {
    let (_dir, cache) = tmp_cache("1day");
    cache.shard(at(1_700_000_000)).unwrap();
    assert!(cache.info().shard_exists_at(at(1_700_000_000)).unwrap());
}

#[test]
fn test_shard_auto_create_increments_cached_count() {
    let (_dir, cache) = tmp_cache("1day");
    assert_eq!(cache.cached_count(), 0);
    cache.shard(at(1_700_000_000)).unwrap();
    assert_eq!(cache.cached_count(), 1);
}

#[test]
fn test_shard_creates_subdirectory_on_disk() {
    let (dir, cache) = tmp_cache("1day");
    cache.shard(at(1_700_000_000)).unwrap();
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    // root contains shards_info.db + at least one shard directory
    assert!(entries.len() >= 2);
}

// ── shard() – interval alignment ─────────────────────────────────────────────

#[test]
fn test_shard_same_interval_same_instance() {
    let (_dir, cache) = tmp_cache("1h");
    // 1_699_999_200 is a 1-hour aligned boundary; 1_700_001_000 is in the same bucket.
    let s1 = cache.shard(at(1_699_999_200)).unwrap();
    let s2 = cache.shard(at(1_700_001_000)).unwrap();
    let id = s1.add(tel("x", json!(1), 1_699_999_200)).unwrap();
    assert!(s2.get(id).unwrap().is_some());
}

#[test]
fn test_shard_different_intervals_different_instances() {
    let (_dir, cache) = tmp_cache("1h");
    let s1 = cache.shard(at(1_700_000_000)).unwrap();
    let s2 = cache.shard(at(1_700_007_200)).unwrap(); // 2 hours later
    let id = s1.add(tel("x", json!(1), 1_700_000_000)).unwrap();
    assert!(s2.get(id).unwrap().is_none());
}

#[test]
fn test_shard_two_auto_creates_each_has_own_catalog_entry() {
    let (_dir, cache) = tmp_cache("1h");
    cache.shard(at(1_700_000_000)).unwrap();
    cache.shard(at(1_700_007_200)).unwrap();
    assert_eq!(cache.cached_count(), 2);
}

// ── shard() – cache hit ───────────────────────────────────────────────────────

#[test]
fn test_second_call_hits_cache_not_catalog() {
    let (_dir, cache) = tmp_cache("1day");
    cache.shard(at(1_700_000_000)).unwrap();
    let count_before = cache.cached_count();
    cache.shard(at(1_700_000_001)).unwrap(); // same day bucket
    assert_eq!(cache.cached_count(), count_before);
}

#[test]
fn test_cache_hit_returns_same_shared_data() {
    let (_dir, cache) = tmp_cache("1day");
    // 1_699_920_000 is a 1-day aligned boundary; 1_699_960_000 is ~11 h later, same bucket.
    let s1 = cache.shard(at(1_699_920_000)).unwrap();
    let id = s1.add(tel("metric", json!(99), 1_699_920_000)).unwrap();
    let s2 = cache.shard(at(1_699_960_000)).unwrap();
    assert!(s2.get(id).unwrap().is_some());
}

// ── shard() – catalog hit (cold cache) ───────────────────────────────────────

#[test]
fn test_shard_catalog_hit_after_close() {
    let (dir, cache) = tmp_cache("1day");
    let id = {
        let shard = cache.shard(at(1_700_000_000)).unwrap();
        let id = shard.add(tel("k", json!(1), 1_700_000_000)).unwrap();
        cache.close().unwrap();
        id
        // local `shard` clone dropped → IndexWriter lock released
    };
    assert_eq!(cache.cached_count(), 0);

    let shard = cache.shard(at(1_700_000_000)).unwrap();
    assert!(shard.get(id).unwrap().is_some());
    assert_eq!(cache.cached_count(), 1);

    drop(dir);
}

#[test]
fn test_shard_catalog_hit_does_not_duplicate_catalog_entry() {
    let (_dir, cache) = tmp_cache("1day");
    cache.shard(at(1_700_000_000)).unwrap();
    cache.close().unwrap();
    cache.shard(at(1_700_000_000)).unwrap();

    let entries = cache.info().shards_at(at(1_700_000_000)).unwrap();
    assert_eq!(entries.len(), 1);
}

// ── primary/secondary with custom threshold ───────────────────────────────────

#[test]
fn test_shard_with_all_primary_threshold_indexes_all() {
    let (_dir, cache) = tmp_cache_threshold("1day", 1.1);
    let shard = cache.shard(at(1_700_000_000)).unwrap();
    for i in 0..5u64 {
        shard
            .add(tel("metric", json!(i), 1_700_000_000 + i))
            .unwrap();
    }
    let results = shard.search_fts("metric", 10).unwrap();
    assert_eq!(results.len(), 5);
}

#[test]
fn test_shard_with_all_secondary_threshold_indexes_only_first() {
    let (_dir, cache) = tmp_cache_threshold("1day", -1.1);
    let shard = cache.shard(at(1_700_000_000)).unwrap();
    for i in 0..5u64 {
        shard
            .add(tel("metric", json!(i), 1_700_000_000 + i))
            .unwrap();
    }
    let results = shard.search_fts("metric", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["secondaries"].as_array().unwrap().len(), 4);
}

// ── shards_span ───────────────────────────────────────────────────────────────

#[test]
fn test_shards_span_empty_for_inverted_range() {
    let (_dir, cache) = tmp_cache("1h");
    let shards = cache.shards_span(at(2_000), at(1_000)).unwrap();
    assert!(shards.is_empty());
}

#[test]
fn test_shards_span_empty_for_equal_bounds() {
    let (_dir, cache) = tmp_cache("1h");
    let shards = cache.shards_span(at(1_000), at(1_000)).unwrap();
    assert!(shards.is_empty());
}

#[test]
fn test_shards_span_single_interval() {
    let (_dir, cache) = tmp_cache("1h");
    // [1_699_999_200, 1_700_001_000) lies entirely within one 1-hour bucket.
    let shards = cache
        .shards_span(at(1_699_999_200), at(1_700_001_000))
        .unwrap();
    assert_eq!(shards.len(), 1);
}

#[test]
fn test_shards_span_exact_boundary_is_single() {
    let (_dir, cache) = tmp_cache("1h");
    // end == aligned_start + shard_duration → the end bucket is not included.
    let start = at(1_699_999_200);
    let end = at(1_699_999_200 + 3_600); // exact end of the first bucket
    let shards = cache.shards_span(start, end).unwrap();
    assert_eq!(shards.len(), 1);
}

#[test]
fn test_shards_span_crosses_two_intervals() {
    let (_dir, cache) = tmp_cache("1h");
    // [1_699_999_200, 1_700_005_000) spans bucket 1 and part of bucket 2.
    let shards = cache
        .shards_span(at(1_699_999_200), at(1_700_005_000))
        .unwrap();
    assert_eq!(shards.len(), 2);
}

#[test]
fn test_shards_span_crosses_three_intervals() {
    let (_dir, cache) = tmp_cache("1h");
    // 3 hours: [1_699_999_200, 1_700_009_600) → three 1-hour buckets.
    let shards = cache
        .shards_span(at(1_699_999_200), at(1_699_999_200 + 3 * 3_600))
        .unwrap();
    assert_eq!(shards.len(), 3);
}

#[test]
fn test_shards_span_populates_cache() {
    let (_dir, cache) = tmp_cache("1h");
    cache
        .shards_span(at(1_699_999_200), at(1_700_005_000))
        .unwrap();
    assert_eq!(cache.cached_count(), 2);
}

#[test]
fn test_shards_span_data_visible_across_returned_shards() {
    let (_dir, cache) = tmp_cache_threshold("1h", 1.1);
    // Add records in each of two buckets and verify cross-shard isolation.
    let shards = cache
        .shards_span(at(1_699_999_200), at(1_700_005_000))
        .unwrap();
    assert_eq!(shards.len(), 2);
    let id0 = shards[0].add(tel("k", json!(0), 1_699_999_200)).unwrap();
    let id1 = shards[1].add(tel("k", json!(1), 1_700_003_600)).unwrap();
    // Each ID is visible only in its own shard.
    assert!(shards[0].get(id0).unwrap().is_some());
    assert!(shards[0].get(id1).unwrap().is_none());
    assert!(shards[1].get(id1).unwrap().is_some());
    assert!(shards[1].get(id0).unwrap().is_none());
}

// ── current ───────────────────────────────────────────────────────────────────

#[test]
fn test_current_returns_at_least_one_shard() {
    let (_dir, cache) = tmp_cache("1h");
    let shards = cache.current("1s").unwrap();
    assert!(!shards.is_empty());
}

#[test]
fn test_current_longer_span_may_return_multiple_shards() {
    let (_dir, cache) = tmp_cache("1h");
    // 3 hours from now will cover at least 2 and at most 4 one-hour buckets.
    let shards = cache.current("3h").unwrap();
    assert!(shards.len() >= 3);
}

#[test]
fn test_current_invalid_duration_is_error() {
    let (_dir, cache) = tmp_cache("1h");
    assert!(cache.current("notaduration").is_err());
}

// ── sync ──────────────────────────────────────────────────────────────────────

#[test]
fn test_sync_empty_cache_is_ok() {
    let (_dir, cache) = tmp_cache("1day");
    assert!(cache.sync().is_ok());
}

#[test]
fn test_sync_populated_cache_is_ok() {
    let (_dir, cache) = tmp_cache("1day");
    cache.shard(at(1_700_000_000)).unwrap();
    cache.shard(at(1_700_090_000)).unwrap();
    assert!(cache.sync().is_ok());
}

// ── close ─────────────────────────────────────────────────────────────────────

#[test]
fn test_close_empty_cache_is_ok() {
    let (_dir, cache) = tmp_cache("1day");
    assert!(cache.close().is_ok());
}

#[test]
fn test_close_clears_cache() {
    let (_dir, cache) = tmp_cache("1day");
    cache.shard(at(1_700_000_000)).unwrap();
    assert_eq!(cache.cached_count(), 1);
    cache.close().unwrap();
    assert_eq!(cache.cached_count(), 0);
}

#[test]
fn test_close_data_persists_on_disk() {
    let (dir, cache) = tmp_cache("1day");
    let id = {
        let shard = cache.shard(at(1_700_000_000)).unwrap();
        let id = shard.add(tel("k", json!(42), 1_700_000_000)).unwrap();
        cache.close().unwrap();
        id
        // `shard` Arc dropped here; IndexWriter lock released
    };

    let cache2 =
        ShardsCache::new(dir.path().to_str().unwrap(), "1day", 4, engine().clone()).unwrap();
    let shard2 = cache2.shard(at(1_700_000_000)).unwrap();
    assert!(shard2.get(id).unwrap().is_some());
}

// ── clone shares state ────────────────────────────────────────────────────────

#[test]
fn test_clone_shares_cache() {
    let (_dir, cache) = tmp_cache("1day");
    let clone = cache.clone();
    cache.shard(at(1_700_000_000)).unwrap();
    assert_eq!(clone.cached_count(), 1);
}

#[test]
fn test_clone_shares_shard_data() {
    let (_dir, cache) = tmp_cache("1day");
    let clone = cache.clone();
    let shard = cache.shard(at(1_700_000_000)).unwrap();
    let id = shard.add(tel("k", json!(7), 1_700_000_000)).unwrap();

    let shard2 = clone.shard(at(1_700_000_000)).unwrap();
    assert!(shard2.get(id).unwrap().is_some());
}

// ── accessors ─────────────────────────────────────────────────────────────────

#[test]
fn test_cached_count_starts_at_zero() {
    let (_dir, cache) = tmp_cache("1day");
    assert_eq!(cache.cached_count(), 0);
}

#[test]
fn test_info_accessor_reflects_auto_created_shards() {
    let (_dir, cache) = tmp_cache("1h");
    cache.shard(at(1_700_000_000)).unwrap();
    assert!(cache.info().shard_exists_at(at(1_700_000_000)).unwrap());
}

use bdslib::ShardInfoEngine;
use bdslib::shardsinfo::ShardInfo;
use std::time::{Duration, UNIX_EPOCH};
use tempfile::TempDir;

fn at(secs: u64) -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn in_memory() -> ShardInfoEngine {
    ShardInfoEngine::new(":memory:", 4).expect("in-memory ShardInfoEngine failed")
}

// ── construction ──────────────────────────────────────────────────────────────

#[test]
fn test_new_in_memory_succeeds() {
    let _engine = in_memory();
}

#[test]
fn test_new_file_backed_creates_db() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("shards.db");
    let _engine = ShardInfoEngine::new(path.to_str().unwrap(), 4)
        .expect("file-backed ShardInfoEngine failed");
}

#[test]
fn test_new_file_backed_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("shards.db");

    let t_start = at(1_700_000_000);
    let t_end   = at(1_700_003_600);

    {
        let engine = ShardInfoEngine::new(path.to_str().unwrap(), 2).unwrap();
        engine.add_shard("/data/shard1", t_start, t_end).unwrap();
    }

    let engine2 = ShardInfoEngine::new(path.to_str().unwrap(), 2).unwrap();
    assert!(engine2.shard_exists_at(at(1_700_001_800)).unwrap());
}

// ── add_shard ─────────────────────────────────────────────────────────────────

#[test]
fn test_add_shard_returns_uuid() {
    let engine = in_memory();
    let id = engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    assert_eq!(id.get_version_num(), 7);
}

#[test]
fn test_add_shard_ids_are_unique() {
    let engine = in_memory();
    let id1 = engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    let id2 = engine.add_shard("/data/s2", at(3_000), at(4_000)).unwrap();
    assert_ne!(id1, id2);
}

#[test]
fn test_add_shard_start_equal_end_is_error() {
    let engine = in_memory();
    assert!(engine.add_shard("/data/s", at(1_000), at(1_000)).is_err());
}

#[test]
fn test_add_shard_start_after_end_is_error() {
    let engine = in_memory();
    assert!(engine.add_shard("/data/s", at(2_000), at(1_000)).is_err());
}

#[test]
fn test_add_shard_path_with_single_quote_is_safe() {
    let engine = in_memory();
    let id = engine
        .add_shard("/data/user's-shard", at(1_000), at(2_000))
        .unwrap();
    let shards = engine.shards_at(at(1_500)).unwrap();
    assert_eq!(shards.len(), 1);
    assert_eq!(shards[0].shard_id, id);
    assert_eq!(shards[0].path, "/data/user's-shard");
}

// ── shards_at ─────────────────────────────────────────────────────────────────

#[test]
fn test_shards_at_empty_returns_empty_vec() {
    let engine = in_memory();
    assert!(engine.shards_at(at(1_000)).unwrap().is_empty());
}

#[test]
fn test_shards_at_timestamp_inside_interval() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    let result = engine.shards_at(at(1_500)).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "/data/s1");
}

#[test]
fn test_shards_at_timestamp_on_start_boundary_inclusive() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    let result = engine.shards_at(at(1_000)).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_shards_at_timestamp_on_end_boundary_exclusive() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    assert!(engine.shards_at(at(2_000)).unwrap().is_empty());
}

#[test]
fn test_shards_at_timestamp_before_interval() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    assert!(engine.shards_at(at(999)).unwrap().is_empty());
}

#[test]
fn test_shards_at_timestamp_after_interval() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    assert!(engine.shards_at(at(2_001)).unwrap().is_empty());
}

#[test]
fn test_shards_at_returns_multiple_overlapping_shards() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(3_000)).unwrap();
    engine.add_shard("/data/s2", at(2_000), at(4_000)).unwrap();
    engine.add_shard("/data/s3", at(5_000), at(6_000)).unwrap();

    let result = engine.shards_at(at(2_500)).unwrap();
    assert_eq!(result.len(), 2);
    let paths: Vec<_> = result.iter().map(|s| s.path.as_str()).collect();
    assert!(paths.contains(&"/data/s1"));
    assert!(paths.contains(&"/data/s2"));
}

#[test]
fn test_shards_at_results_ordered_by_start_time() {
    let engine = in_memory();
    engine.add_shard("/data/late",  at(2_000), at(5_000)).unwrap();
    engine.add_shard("/data/early", at(1_000), at(5_000)).unwrap();

    let result = engine.shards_at(at(3_000)).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].path, "/data/early");
    assert_eq!(result[1].path, "/data/late");
}

#[test]
fn test_shards_at_returned_fields_round_trip() {
    let engine = in_memory();
    let t_start = at(1_700_000_000);
    let t_end   = at(1_700_003_600);
    let id = engine.add_shard("/mnt/data/shard42", t_start, t_end).unwrap();

    let result = engine.shards_at(at(1_700_001_800)).unwrap();
    assert_eq!(result.len(), 1);
    let info: &ShardInfo = &result[0];
    assert_eq!(info.shard_id, id);
    assert_eq!(info.path, "/mnt/data/shard42");
    assert_eq!(info.start_time, t_start);
    assert_eq!(info.end_time, t_end);
}

// ── shard_exists_at ───────────────────────────────────────────────────────────

#[test]
fn test_shard_exists_at_empty_is_false() {
    let engine = in_memory();
    assert!(!engine.shard_exists_at(at(1_000)).unwrap());
}

#[test]
fn test_shard_exists_at_inside_interval_is_true() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    assert!(engine.shard_exists_at(at(1_500)).unwrap());
}

#[test]
fn test_shard_exists_at_start_boundary_is_true() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    assert!(engine.shard_exists_at(at(1_000)).unwrap());
}

#[test]
fn test_shard_exists_at_end_boundary_is_false() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    assert!(!engine.shard_exists_at(at(2_000)).unwrap());
}

#[test]
fn test_shard_exists_at_outside_interval_is_false() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();
    assert!(!engine.shard_exists_at(at(500)).unwrap());
    assert!(!engine.shard_exists_at(at(3_000)).unwrap());
}

// ── thread safety ─────────────────────────────────────────────────────────────

#[test]
fn test_clone_shares_state() {
    let engine = in_memory();
    engine.add_shard("/data/s1", at(1_000), at(2_000)).unwrap();

    let clone = engine.clone();
    assert!(clone.shard_exists_at(at(1_500)).unwrap());
}

#[test]
fn test_concurrent_add_and_query() {
    use std::sync::Arc;

    let engine = Arc::new(in_memory());
    let mut handles = Vec::new();

    for i in 0..8u64 {
        let e = engine.clone();
        handles.push(std::thread::spawn(move || {
            let base = 1_000_000 + i * 10_000;
            e.add_shard(&format!("/data/shard{i}"), at(base), at(base + 3_600))
                .unwrap();
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    // every shard covers its own midpoint
    for i in 0..8u64 {
        let base = 1_000_000 + i * 10_000;
        assert!(engine.shard_exists_at(at(base + 1_800)).unwrap());
    }
}

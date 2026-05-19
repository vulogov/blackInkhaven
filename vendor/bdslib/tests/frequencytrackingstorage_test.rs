use bdslib::FrequencyTracking;
use bdslib::common::time::now_secs;
use tempfile::TempDir;

// ── fixtures ──────────────────────────────────────────────────────────────────

fn memory_ft() -> FrequencyTracking {
    FrequencyTracking::new(":memory:", 4).unwrap()
}

fn file_ft() -> (TempDir, FrequencyTracking) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ft.db");
    let ft = FrequencyTracking::new(path.to_str().unwrap(), 4).unwrap();
    (dir, ft)
}

// ── add / add_with_timestamp ──────────────────────────────────────────────────

#[test]
fn test_add_records_observation() {
    let ft = memory_ft();
    ft.add("api.login").unwrap();
    let ts = ft.by_id("api.login").unwrap();
    assert_eq!(ts.len(), 1);
}

#[test]
fn test_add_with_timestamp_explicit() {
    let ft = memory_ft();
    ft.add_with_timestamp(1_000_000, "event.a").unwrap();
    let ts = ft.by_id("event.a").unwrap();
    assert_eq!(ts, vec![1_000_000u64]);
}

#[test]
fn test_add_same_id_multiple_times() {
    let ft = memory_ft();
    ft.add_with_timestamp(100, "x").unwrap();
    ft.add_with_timestamp(200, "x").unwrap();
    ft.add_with_timestamp(300, "x").unwrap();
    let ts = ft.by_id("x").unwrap();
    assert_eq!(ts, vec![100u64, 200, 300]);
}

#[test]
fn test_add_same_timestamp_same_id_duplicate() {
    // Duplicate (ts, id) pairs must both be recorded (no dedup).
    let ft = memory_ft();
    ft.add_with_timestamp(500, "dup").unwrap();
    ft.add_with_timestamp(500, "dup").unwrap();
    let ts = ft.by_id("dup").unwrap();
    assert_eq!(ts.len(), 2, "each add produces a separate row");
}

#[test]
fn test_add_id_with_special_characters() {
    let ft = memory_ft();
    ft.add("key with 'quotes' and spaces").unwrap();
    let ts = ft.by_id("key with 'quotes' and spaces").unwrap();
    assert_eq!(ts.len(), 1);
}

// ── by_id ─────────────────────────────────────────────────────────────────────

#[test]
fn test_by_id_unknown_returns_empty() {
    let ft = memory_ft();
    let ts = ft.by_id("never.seen").unwrap();
    assert!(ts.is_empty());
}

#[test]
fn test_by_id_returns_ascending_order() {
    let ft = memory_ft();
    // Insert out of order.
    ft.add_with_timestamp(300, "k").unwrap();
    ft.add_with_timestamp(100, "k").unwrap();
    ft.add_with_timestamp(200, "k").unwrap();
    assert_eq!(ft.by_id("k").unwrap(), vec![100u64, 200, 300]);
}

#[test]
fn test_by_id_does_not_return_other_ids() {
    let ft = memory_ft();
    ft.add_with_timestamp(1, "a").unwrap();
    ft.add_with_timestamp(2, "b").unwrap();
    let ts = ft.by_id("a").unwrap();
    assert_eq!(ts, vec![1u64]);
}

// ── by_timestamp ──────────────────────────────────────────────────────────────

#[test]
fn test_by_timestamp_returns_ids_at_exact_second() {
    let ft = memory_ft();
    ft.add_with_timestamp(1000, "alpha").unwrap();
    ft.add_with_timestamp(1000, "beta").unwrap();
    ft.add_with_timestamp(2000, "gamma").unwrap();
    let ids = ft.by_timestamp(1000).unwrap();
    assert_eq!(ids, vec!["alpha", "beta"]);
}

#[test]
fn test_by_timestamp_empty_returns_empty_vec() {
    let ft = memory_ft();
    let ids = ft.by_timestamp(9_999_999).unwrap();
    assert!(ids.is_empty());
}

#[test]
fn test_by_timestamp_deduplicates_ids() {
    // Same id inserted twice at the same timestamp → appears once in result.
    let ft = memory_ft();
    ft.add_with_timestamp(42, "dedup_id").unwrap();
    ft.add_with_timestamp(42, "dedup_id").unwrap();
    let ids = ft.by_timestamp(42).unwrap();
    assert_eq!(ids, vec!["dedup_id"]);
}

#[test]
fn test_by_timestamp_sorted_alphabetically() {
    let ft = memory_ft();
    ft.add_with_timestamp(99, "zebra").unwrap();
    ft.add_with_timestamp(99, "apple").unwrap();
    ft.add_with_timestamp(99, "mango").unwrap();
    assert_eq!(ft.by_timestamp(99).unwrap(), vec!["apple", "mango", "zebra"]);
}

// ── time_range ────────────────────────────────────────────────────────────────

#[test]
fn test_time_range_returns_ids_in_window() {
    let ft = memory_ft();
    ft.add_with_timestamp(100, "early").unwrap();
    ft.add_with_timestamp(500, "mid").unwrap();
    ft.add_with_timestamp(900, "late").unwrap();
    ft.add_with_timestamp(1000, "after").unwrap();
    let ids = ft.time_range(200, 950).unwrap();
    assert!(ids.contains(&"mid".to_string()));
    assert!(ids.contains(&"late".to_string()));
    assert!(!ids.contains(&"early".to_string()));
    assert!(!ids.contains(&"after".to_string()));
}

#[test]
fn test_time_range_inclusive_on_both_ends() {
    let ft = memory_ft();
    ft.add_with_timestamp(100, "start_boundary").unwrap();
    ft.add_with_timestamp(200, "end_boundary").unwrap();
    let ids = ft.time_range(100, 200).unwrap();
    assert!(ids.contains(&"start_boundary".to_string()));
    assert!(ids.contains(&"end_boundary".to_string()));
}

#[test]
fn test_time_range_no_match_returns_empty() {
    let ft = memory_ft();
    ft.add_with_timestamp(100, "x").unwrap();
    let ids = ft.time_range(200, 300).unwrap();
    assert!(ids.is_empty());
}

#[test]
fn test_time_range_deduplicates_ids() {
    let ft = memory_ft();
    // "repeated" fires three times in the window.
    ft.add_with_timestamp(100, "repeated").unwrap();
    ft.add_with_timestamp(150, "repeated").unwrap();
    ft.add_with_timestamp(200, "repeated").unwrap();
    let ids = ft.time_range(50, 250).unwrap();
    assert_eq!(ids, vec!["repeated".to_string()]);
}

#[test]
fn test_time_range_point_interval() {
    let ft = memory_ft();
    ft.add_with_timestamp(777, "point").unwrap();
    let ids = ft.time_range(777, 777).unwrap();
    assert_eq!(ids, vec!["point".to_string()]);
}

// ── recent ────────────────────────────────────────────────────────────────────

#[test]
fn test_recent_returns_freshly_added_id() {
    let ft = memory_ft();
    ft.add("hot.metric").unwrap();
    let ids = ft.recent("1min").unwrap();
    assert!(ids.contains(&"hot.metric".to_string()));
}

#[test]
fn test_recent_excludes_old_record() {
    let ft = memory_ft();
    let old_ts = now_secs().saturating_sub(7200); // 2 hours ago
    ft.add_with_timestamp(old_ts, "stale.metric").unwrap();
    ft.add("fresh.metric").unwrap();
    let ids = ft.recent("1h").unwrap();
    assert!(ids.contains(&"fresh.metric".to_string()));
    assert!(!ids.contains(&"stale.metric".to_string()));
}

#[test]
fn test_recent_empty_store_returns_empty() {
    let ft = memory_ft();
    let ids = ft.recent("1h").unwrap();
    assert!(ids.is_empty());
}

#[test]
fn test_recent_invalid_duration_returns_err() {
    let ft = memory_ft();
    let result = ft.recent("not-a-duration");
    assert!(result.is_err());
}

#[test]
fn test_recent_various_duration_formats() {
    let ft = memory_ft();
    ft.add("id.a").unwrap();
    assert!(!ft.recent("30s").unwrap().is_empty());
    assert!(!ft.recent("5min").unwrap().is_empty());
    assert!(!ft.recent("1h").unwrap().is_empty());
    assert!(!ft.recent("7days").unwrap().is_empty());
}

// ── clone / shared state ──────────────────────────────────────────────────────

#[test]
fn test_clone_shares_underlying_store() {
    let ft1 = memory_ft();
    let ft2 = ft1.clone();
    ft1.add_with_timestamp(1, "written_by_ft1").unwrap();
    let ids = ft2.by_timestamp(1).unwrap();
    assert!(ids.contains(&"written_by_ft1".to_string()), "clone must see same data");
}

// ── sync ─────────────────────────────────────────────────────────────────────

#[test]
fn test_sync_on_file_db() {
    let (_dir, ft) = file_ft();
    ft.add("persisted").unwrap();
    ft.sync().unwrap();
    let ts = ft.by_id("persisted").unwrap();
    assert_eq!(ts.len(), 1);
}

// ── delete ────────────────────────────────────────────────────────────────────

#[test]
fn test_delete_removes_all_tracking_for_id() {
    let ft = memory_ft();
    ft.add_with_timestamp(100, "target").unwrap();
    ft.add_with_timestamp(200, "target").unwrap();
    ft.add_with_timestamp(100, "other").unwrap();
    ft.delete("target").unwrap();
    assert!(ft.by_id("target").unwrap().is_empty(), "all rows for 'target' must be gone");
    assert_eq!(ft.by_id("other").unwrap(), vec![100u64], "'other' must be unaffected");
}

#[test]
fn test_delete_unknown_id_is_noop() {
    let ft = memory_ft();
    ft.add_with_timestamp(1, "keep").unwrap();
    ft.delete("nonexistent").unwrap();
    assert_eq!(ft.by_id("keep").unwrap(), vec![1u64]);
}

#[test]
fn test_delete_special_characters() {
    let ft = memory_ft();
    ft.add("key with 'quotes'").unwrap();
    ft.delete("key with 'quotes'").unwrap();
    assert!(ft.by_id("key with 'quotes'").unwrap().is_empty());
}

// ── persistence across open/close ────────────────────────────────────────────

#[test]
fn test_data_persists_across_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ft.db");

    {
        let ft = FrequencyTracking::new(path.to_str().unwrap(), 4).unwrap();
        ft.add_with_timestamp(12345, "persisted_id").unwrap();
        ft.sync().unwrap();
    }

    let ft2 = FrequencyTracking::new(path.to_str().unwrap(), 4).unwrap();
    let ts = ft2.by_id("persisted_id").unwrap();
    assert_eq!(ts, vec![12345u64]);
}

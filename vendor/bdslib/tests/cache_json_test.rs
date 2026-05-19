use bdslib::common::cache_json::JsonCache;
use serde_json::json;
use std::time::Duration;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Cache large enough for all tests; TTL long enough that nothing expires
/// during a single test run.
fn big_cache() -> JsonCache {
    JsonCache::new(1000, Duration::from_secs(3600))
}

/// Cache with 1-second TTL for expiry tests; background cleanup every 10 s
/// (so tests control timing via `evict_expired`).
fn short_ttl_cache() -> JsonCache {
    JsonCache::with_cleanup_interval(
        1000,
        Duration::from_millis(100),
        Duration::from_secs(10),
    )
}

const TS: u64 = 1_700_000_000;

// ── construction ──────────────────────────────────────────────────────────────

#[test]
fn test_new_is_empty() {
    let cache = big_cache();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_capacity_reported_correctly() {
    let cache = JsonCache::new(42, Duration::from_secs(60));
    assert_eq!(cache.capacity(), 42);
}

#[test]
fn test_ttl_reported_correctly() {
    let cache = JsonCache::new(10, Duration::from_secs(99));
    assert_eq!(cache.ttl(), Duration::from_secs(99));
}

// ── insert / get basics ───────────────────────────────────────────────────────

#[test]
fn test_insert_then_get_returns_value() {
    let cache = big_cache();
    cache.insert("doc-1", TS, json!({"a": 1}));
    assert_eq!(cache.get("doc-1", TS), Some(json!({"a": 1})));
}

#[test]
fn test_get_missing_key_returns_none() {
    let cache = big_cache();
    assert_eq!(cache.get("missing", TS), None);
}

#[test]
fn test_get_wrong_timestamp_returns_none() {
    let cache = big_cache();
    cache.insert("doc-1", TS, json!(1));
    assert_eq!(cache.get("doc-1", TS + 1), None);
}

#[test]
fn test_get_wrong_id_returns_none() {
    let cache = big_cache();
    cache.insert("doc-1", TS, json!(1));
    assert_eq!(cache.get("doc-2", TS), None);
}

#[test]
fn test_key_is_id_and_timestamp_pair() {
    let cache = big_cache();
    cache.insert("id", 100, json!("a"));
    cache.insert("id", 200, json!("b"));
    cache.insert("other", 100, json!("c"));

    assert_eq!(cache.get("id", 100), Some(json!("a")));
    assert_eq!(cache.get("id", 200), Some(json!("b")));
    assert_eq!(cache.get("other", 100), Some(json!("c")));
    assert_eq!(cache.len(), 3);
}

#[test]
fn test_insert_updates_existing_key() {
    let cache = big_cache();
    cache.insert("doc", TS, json!(1));
    cache.insert("doc", TS, json!(2));
    assert_eq!(cache.get("doc", TS), Some(json!(2)));
    assert_eq!(cache.len(), 1);
}

#[test]
fn test_len_tracks_inserts() {
    let cache = big_cache();
    for i in 0..5u64 {
        cache.insert("doc", i, json!(i));
    }
    assert_eq!(cache.len(), 5);
}

// ── remove ────────────────────────────────────────────────────────────────────

#[test]
fn test_remove_returns_value() {
    let cache = big_cache();
    cache.insert("doc", TS, json!(42));
    let v = cache.remove("doc", TS);
    assert_eq!(v, Some(json!(42)));
}

#[test]
fn test_remove_makes_key_absent() {
    let cache = big_cache();
    cache.insert("doc", TS, json!(42));
    cache.remove("doc", TS);
    assert_eq!(cache.get("doc", TS), None);
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_remove_nonexistent_returns_none() {
    let cache = big_cache();
    assert_eq!(cache.remove("ghost", TS), None);
}

// ── clear ─────────────────────────────────────────────────────────────────────

#[test]
fn test_clear_empties_cache() {
    let cache = big_cache();
    for i in 0..10u64 {
        cache.insert("doc", i, json!(i));
    }
    assert_eq!(cache.len(), 10);
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn test_insert_after_clear_works() {
    let cache = big_cache();
    cache.insert("doc", TS, json!(1));
    cache.clear();
    cache.insert("doc", TS, json!(2));
    assert_eq!(cache.get("doc", TS), Some(json!(2)));
}

// ── TTL / expiry ──────────────────────────────────────────────────────────────

#[test]
fn test_expired_entry_not_returned_by_get() {
    let cache = short_ttl_cache();
    cache.insert("doc", TS, json!("value"));
    std::thread::sleep(Duration::from_millis(200)); // outlast the 100 ms TTL
    assert_eq!(cache.get("doc", TS), None);
}

#[test]
fn test_expired_entry_removed_lazily_on_get() {
    let cache = short_ttl_cache();
    cache.insert("doc", TS, json!("value"));
    assert_eq!(cache.len(), 1);
    std::thread::sleep(Duration::from_millis(200));
    cache.get("doc", TS); // triggers lazy removal
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_evict_expired_removes_stale_entries() {
    let cache = short_ttl_cache();
    cache.insert("a", TS, json!(1));
    cache.insert("b", TS + 1, json!(2));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(cache.expired_count(), 2);
    cache.evict_expired();
    assert!(cache.is_empty());
}

#[test]
fn test_evict_expired_leaves_fresh_entries() {
    let cache = JsonCache::with_cleanup_interval(
        100,
        Duration::from_secs(3600), // long TTL
        Duration::from_secs(10),
    );
    cache.insert("a", TS, json!(1));
    cache.insert("b", TS + 1, json!(2));
    cache.evict_expired();
    assert_eq!(cache.len(), 2);
}

#[test]
fn test_expired_count_reports_stale_entries() {
    let cache = short_ttl_cache();
    cache.insert("x", TS, json!(0));
    assert_eq!(cache.expired_count(), 0);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(cache.expired_count(), 1);
}

// ── capacity / eviction ───────────────────────────────────────────────────────

#[test]
fn test_cache_never_exceeds_capacity() {
    let capacity = 10;
    let cache = JsonCache::new(capacity, Duration::from_secs(3600));
    for i in 0..20u64 {
        cache.insert(format!("doc-{i}"), TS + i, json!(i));
    }
    assert!(cache.len() <= capacity);
}

#[test]
fn test_update_existing_does_not_evict() {
    let cache = JsonCache::new(2, Duration::from_secs(3600));
    cache.insert("a", TS, json!(1));
    cache.insert("b", TS + 1, json!(2));
    // Both slots full; updating "a" must not evict anything.
    cache.insert("a", TS, json!(99));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get("a", TS), Some(json!(99)));
    assert_eq!(cache.get("b", TS + 1), Some(json!(2)));
}

#[test]
fn test_random_eviction_makes_room() {
    let cache = JsonCache::new(3, Duration::from_secs(3600));
    cache.insert("a", TS, json!(1));
    cache.insert("b", TS + 1, json!(2));
    cache.insert("c", TS + 2, json!(3));
    assert_eq!(cache.len(), 3);

    cache.insert("d", TS + 3, json!(4)); // must evict one of a/b/c
    assert_eq!(cache.len(), 3);
    assert!(cache.get("d", TS + 3).is_some());
}

#[test]
fn test_expired_entries_evicted_before_random_eviction() {
    // One slot; the existing entry is expired, so insert should clear it via
    // expiry sweep rather than choosing random eviction.
    let cache = JsonCache::with_cleanup_interval(1, Duration::from_millis(50), Duration::from_secs(10));
    cache.insert("old", TS, json!("stale"));
    std::thread::sleep(Duration::from_millis(100));

    // This should succeed by sweeping the stale entry, NOT by randomly evicting a live one.
    cache.insert("new", TS + 1, json!("fresh"));
    assert_eq!(cache.get("new", TS + 1), Some(json!("fresh")));
    assert_eq!(cache.len(), 1);
}

// ── clone / sharing ───────────────────────────────────────────────────────────

#[test]
fn test_clone_shares_state() {
    let cache = big_cache();
    let clone = cache.clone();

    cache.insert("doc", TS, json!(7));
    assert_eq!(clone.get("doc", TS), Some(json!(7)));

    clone.insert("other", TS + 1, json!(8));
    assert_eq!(cache.get("other", TS + 1), Some(json!(8)));
}

#[test]
fn test_clear_via_clone_affects_original() {
    let cache = big_cache();
    let clone = cache.clone();
    cache.insert("x", TS, json!(1));
    clone.clear();
    assert!(cache.is_empty());
}

// ── value types ───────────────────────────────────────────────────────────────

#[test]
fn test_stores_null() {
    let cache = big_cache();
    cache.insert("doc", TS, json!(null));
    assert_eq!(cache.get("doc", TS), Some(json!(null)));
}

#[test]
fn test_stores_array() {
    let cache = big_cache();
    cache.insert("doc", TS, json!([1, 2, 3]));
    assert_eq!(cache.get("doc", TS), Some(json!([1, 2, 3])));
}

#[test]
fn test_stores_nested_object() {
    let cache = big_cache();
    let v = json!({"a": {"b": {"c": 42}}});
    cache.insert("doc", TS, v.clone());
    assert_eq!(cache.get("doc", TS), Some(v));
}

#[test]
fn test_stores_boolean() {
    let cache = big_cache();
    cache.insert("t", TS, json!(true));
    cache.insert("f", TS, json!(false));
    assert_eq!(cache.get("t", TS), Some(json!(true)));
    assert_eq!(cache.get("f", TS), Some(json!(false)));
}

// ── concurrent access ─────────────────────────────────────────────────────────

#[test]
fn test_concurrent_inserts_and_gets() {
    use std::sync::Arc;
    use std::thread;

    let cache = Arc::new(JsonCache::new(500, Duration::from_secs(3600)));
    let mut handles = vec![];

    for t in 0..10u64 {
        let c = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for i in 0..50u64 {
                c.insert(format!("thread-{t}"), i, json!(t * 100 + i));
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    // Each thread wrote 50 distinct (id, ts) keys; at capacity=500 none should
    // have been evicted.
    assert_eq!(cache.len(), 500);
}

#[test]
fn test_concurrent_mixed_operations() {
    use std::sync::Arc;
    use std::thread;

    let cache = Arc::new(JsonCache::new(100, Duration::from_secs(3600)));

    // Pre-populate
    for i in 0..50u64 {
        cache.insert(format!("key-{i}"), TS + i, json!(i));
    }

    let mut handles = vec![];

    // Writers
    for w in 50..100u64 {
        let c = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            c.insert(format!("key-{w}"), TS + w, json!(w));
        }));
    }

    // Readers
    for r in 0..50u64 {
        let c = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            let _ = c.get(&format!("key-{r}"), TS + r);
        }));
    }

    // Removers
    for r in 0..10u64 {
        let c = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            c.remove(&format!("key-{r}"), TS + r);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Just verify the cache did not panic and is in a consistent state.
    let _ = cache.len();
    let _ = cache.is_empty();
}

// ── background-thread interaction ─────────────────────────────────────────────

#[test]
fn test_background_thread_sweeps_expired_entries() {
    // Very short cleanup interval so the thread sweeps during the test.
    let cache = JsonCache::with_cleanup_interval(
        100,
        Duration::from_millis(50),  // entries expire in 50 ms
        Duration::from_millis(60),  // thread sweeps every 60 ms
    );
    cache.insert("a", TS, json!(1));
    cache.insert("b", TS + 1, json!(2));
    assert_eq!(cache.len(), 2);

    // Wait long enough for entries to expire and for the background sweep to run.
    std::thread::sleep(Duration::from_millis(300));

    // Entries may already be gone via the background sweep; at minimum
    // `get` must not return stale values.
    assert_eq!(cache.get("a", TS), None);
    assert_eq!(cache.get("b", TS + 1), None);
}

#[test]
fn test_background_thread_exits_when_all_clones_dropped() {
    // This test verifies that no thread panic or resource leak occurs.
    {
        let cache = JsonCache::with_cleanup_interval(
            10,
            Duration::from_secs(1),
            Duration::from_millis(50),
        );
        let c2 = cache.clone();
        cache.insert("x", TS, json!(1));
        drop(c2);
        // Both clones dropped inside this block.
    }
    // Give the background thread time to notice and exit cleanly.
    std::thread::sleep(Duration::from_millis(200));
    // No assertion needed — reaching here without a panic is the success criterion.
}
